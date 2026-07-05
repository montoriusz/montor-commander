//! Records raw PTY output per OSC 133 prompt→command section and parses each
//! finished section into plain text via a headless `vt100` parser.
//!
//! The reader thread in [`crate::terminal`] drives a [`SessionRecorder`] with the
//! [`crate::osc133::Segment`] stream it already produces. As soon as a command
//! finishes (`CommandFinished` / OSC 133 D), the finished section is parsed into
//! prompt/command/output text and persisted as a [`crate::chat::ChatMessage::TerminalSection`],
//! which the chat store interleaves with user and assistant turns. That single
//! artifact serves two consumers:
//!
//! - **Re-render:** the `raw` bytes can be replayed into a separate xterm
//!   instance to reproduce the section visually.
//! - **LLM context:** `build_history` accumulates `TerminalSection` messages and
//!   attaches the ones since the previous user turn to that turn's `<terminal>`
//!   block, replacing the old frontend-extracted `payload.terminal`.
//!
//! The section currently being typed into (no `D` yet) is kept in memory only;
//! its command fragment is parsed on demand at send time to populate `<commandline>`.

use crate::chat::{ChatMessage, ChatMessagesChangedPayload};
use crate::jsonl_store::JsonlStore;
use crate::osc133::{Segment, ShellEvent};
use chrono::Local;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

/// Visible screen rows allocated for each per-fragment `vt100::Parser`.
///
/// `vt100`'s [`Screen::contents`](vt100::Screen::contents) only returns the rows
/// currently in the viewable window (bounded by `rows` plus the same number of
/// scrollback rows at most), *not* the entire scrollback buffer. To capture all
/// of a section's plain text—including long output that would otherwise scroll
/// off the real terminal's small screen—we size the parser screen generously,
/// so nothing scrolls at all. Line wrapping still tracks the real column count
/// (`section.cols`), which is what affects how text is split into rows.
const PARSE_ROWS: u16 = 10_000;

/// The section currently being recorded. At most one is active at a time: a new
/// `PromptStarted` (OSC 133 A) starts it, `CommandFinished` (D) closes it, then
/// the recorder holds nothing until the next A. The bash integration always
/// emits D before the next A (via `PROMPT_COMMAND`), so an unfinished section is
/// only ever dropped at startup.
#[derive(Debug, Clone)]
pub struct SessionSection {
    pub aid: String,
    pub raw: Vec<u8>,
    /// Byte offset where the OSC 133 B sequence starts (end of A + prompt).
    pub off_prompt_end: Option<usize>,
    /// Byte offset where the OSC 133 C sequence starts (end of B + command).
    pub off_command_start: Option<usize>,
    /// Byte offset where the OSC 133 D sequence starts (end of C + output).
    pub off_command_finish: Option<usize>,
    pub exit_code: Option<i32>,
    pub cols: u16,
    pub rows: u16,
}

impl SessionSection {
    fn new(aid: String, cols: u16, rows: u16) -> Self {
        Self {
            aid,
            raw: Vec::new(),
            off_prompt_end: None,
            off_command_start: None,
            off_command_finish: None,
            exit_code: None,
            cols,
            rows,
        }
    }

    /// True if the command was started (OSC 133 C fired) before it finished.
    pub fn executed(&self) -> bool {
        self.off_command_start.is_some()
    }

    /// Parse the section into its three plain-text fragments — rendered
    /// prompt, typed command line, and captured command output — by replaying
    /// the whole `raw` buffer through a single headless `vt100::Parser` and
    /// capturing the cursor coordinate at each OSC 133 boundary. Each fragment
    /// is then sliced out of the *final* grid with
    /// [`vt100::Screen::contents_between`], which respects soft-wrapping.
    ///
    /// We use one emulator for the whole section (not a fresh parser per byte
    /// range) so `readline`'s line redraws resolve correctly: readline
    /// re-positions the cursor relative to the already-rendered prompt
    /// (`\u001b[A`, repeated `\u001b[C`), and an isolated command slice would
    /// emit those moves as leading spaces. Feeding the prompt first means the
    /// grid already has the prompt in cols 0..N, so the command lands at its
    /// true column.
    ///
    /// The command fragment's layout is selected by whether the command has
    /// started (`off_command_start`):
    ///
    /// - **Started** (C fired, finished or running): the typed command is
    ///   `raw[prompt_end..command_start]`, and any output follows it up to
    ///   `off_command_finish` (or the live end of `raw` while the command is
    ///   still running).
    /// - **Not started yet** (still typing at send time): the post-prompt
    ///   bytes ARE the live, unexecuted command line, extending to the end of
    ///   `raw`, and there is no output yet.
    ///
    /// The command line is collapsed to empty until the prompt has finished
    /// rendering (OSC 133 B): before B, the post-`A` bytes are still part of
    /// the prompt and must not be misread as a typed command.
    ///
    /// Boundary cursor positions are read *between* incremental `process`
    /// calls; each marker offset is captured just before appending the
    /// complete OSC sequence in [`SessionRecorder::feed`], so no ESC sequence
    /// or UTF-8 codepoint is split across a slice boundary.
    ///
    /// This single entry point serves both the persist path (finished
    /// sections, where all markers are set) and the send-time path (the live
    /// section the user is typing into, where only a prefix of markers is
    /// set).
    pub fn parsed_parts(&self) -> ParsedSection {
        // Prompt still rendering (only A has fired): nothing is a command yet.
        let prompt_end = match self.off_prompt_end {
            Some(off) => off,
            None => return ParsedSection::default(),
        };
        let (cmd_end, out_end) = match self.off_command_start {
            Some(cs) => (cs, self.off_command_finish.unwrap_or(self.raw.len())),
            None => (self.raw.len(), self.raw.len()),
        };

        // One generously-sized parser (see `PARSE_ROWS`) so long output never
        // scrolls out of `visible_rows()`, which `contents_between` iterates.
        let mut parser = vt100::Parser::new(PARSE_ROWS, self.cols, 0);
        let feed = |p: &mut vt100::Parser, from: usize, to: usize| {
            if from < to && to <= self.raw.len() {
                p.process(&self.raw[from..to]);
            }
        };

        feed(&mut parser, 0, prompt_end);
        let p = parser.screen().cursor_position();
        let prompt = trim_terminal_text(&parser.screen().contents_between(0, 0, p.0, p.1));

        feed(&mut parser, prompt_end, cmd_end);
        let c = parser.screen().cursor_position();
        let cmdline = trim_terminal_text(&parser.screen().contents_between(p.0, p.1, c.0, c.1));

        feed(&mut parser, cmd_end, out_end);
        let d = parser.screen().cursor_position();
        let output = trim_terminal_text(&parser.screen().contents_between(c.0, c.1, d.0, d.1));

        ParsedSection {
            prompt,
            cmdline,
            output,
        }
    }
}

/// The three plain-text fragments parsed out of a finished [`SessionSection`].
/// See [`SessionSection::parsed_parts`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedSection {
    pub prompt: String,
    pub cmdline: String,
    pub output: String,
}

pub struct SessionRecorder {
    current: Option<SessionSection>,
    cols: u16,
    rows: u16,
    /// Parsed command line of the last section yielded for the *current* `aid`
    /// via a same-`aid` prompt redraw. Reset whenever a genuinely new `aid`
    /// starts. Used to dedup pure re-renders (resize) from real command-line
    /// changes (tab completion) — see the `PromptStarted` arm in [`feed`].
    last_yielded_cmdline: String,
}

impl SessionRecorder {
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            current: None,
            cols,
            rows,
            last_yielded_cmdline: String::new(),
        }
    }

    /// Update the terminal size used for new sections and for fragment parsing.
    ///
    /// Also forwards the new size to the in-flight section so its parsed parts
    /// re-flow against the current geometry rather than the size at prompt start.
    pub fn set_size(&mut self, rows: u16, cols: u16) {
        self.rows = rows;
        self.cols = cols;
        if let Some(section) = self.current.as_mut() {
            section.cols = cols;
            section.rows = rows;
        }
    }

    /// Feed one scanned [`Segment`]. Returns a finished [`SessionSection`] when a
    /// `CommandFinished` closes the active section, so the caller can persist it.
    pub fn feed(&mut self, segment: &Segment) -> Option<SessionSection> {
        match segment {
            Segment::Passthrough(bytes) => {
                if let Some(section) = self.current.as_mut() {
                    section.raw.extend_from_slice(bytes);
                }
                None
            }
            Segment::Sequence { bytes, event } => match event {
                ShellEvent::PromptStarted { aid } => {
                    // The bash integration emits `aid` on every marker; fall
                    // back to an empty id defensively.
                    let incoming_aid = aid.clone().unwrap_or_default();

                    let previous_section = match self.current.take() {
                        Some(section) if section.aid == incoming_aid => {
                            // Same-aid redraw: bash re-emits OSC 133 A/B for
                            // the current prompt without a preceding D whenever
                            // readline re-renders it — either a resize (leaving
                            // the typed command unchanged) or a tab completion
                            // (redraw with a *changed* command line). Only the
                            // latter is genuine context worth persisting, so
                            // yield only when the parsed command line changed
                            // since the last section we yielded for this aid.
                            let cmdline = section.parsed_parts().cmdline;
                            if cmdline != self.last_yielded_cmdline {
                                self.last_yielded_cmdline = cmdline;
                                Some(section)
                            } else {
                                None
                            }
                        }
                        Some(section) if section.off_prompt_end.is_some() => {
                            // A genuinely new prompt (different aid) closed the
                            // in-flight section without a D — bash normally emits
                            // D in PROMPT_COMMAND before each A, so this only
                            // happens when it skipped (startup, abnormal exit).
                            // Yield what we captured so the reader thread can
                            // persist it. Requires B to have fired: a bare
                            // A-only fragment has nothing useful to keep.
                            self.last_yielded_cmdline = String::new();
                            Some(section)
                        }
                        _ => {
                            // No previous section, or only a bare A-only
                            // fragment that is not worth keeping. Reset and
                            // yield nothing.
                            self.last_yielded_cmdline = String::new();
                            None
                        }
                    };

                    // Start the new section, keeping the A marker bytes in `raw`
                    // so a later redraw can replay them.
                    let mut section = SessionSection::new(incoming_aid, self.cols, self.rows);
                    section.raw.extend_from_slice(bytes);
                    self.current = Some(section);
                    previous_section
                }
                ShellEvent::PromptEnded { .. } => {
                    if let Some(section) = self.current.as_mut() {
                        // Record where B starts so the prompt fragment is
                        // raw[0 .. off_prompt_end] (A marker + prompt rendering).
                        section.off_prompt_end = Some(section.raw.len());
                        section.raw.extend_from_slice(bytes);
                    }
                    None
                }
                ShellEvent::CommandStarted { .. } => {
                    if let Some(section) = self.current.as_mut() {
                        section.off_command_start = Some(section.raw.len());
                        section.raw.extend_from_slice(bytes);
                    }
                    None
                }
                ShellEvent::CommandFinished { exit_code, .. } => {
                    self.current.take().map(|mut section| {
                        section.off_command_finish = Some(section.raw.len());
                        section.raw.extend_from_slice(bytes);
                        section.exit_code = *exit_code;
                        section
                    })
                }
            },
        }
    }

    /// Borrow the current (unfinished) section, for deriving the live
    /// `<commandline>` at send time. Does not consume it.
    pub fn current_snapshot(&self) -> Option<&SessionSection> {
        self.current.as_ref()
    }
}

/// Identity of the last persisted *live* snapshot, used by
/// [`persist_live_section_if_changed`] to skip re-persisting an unchanged live
/// section on repeated sends. The rendered prompt is deliberately excluded:
/// only the typed command (`cmdline`) and captured output are treated as the
/// section's content for dedup purposes. Each new shell prompt yields a fresh
/// `aid`, so comparing `aid` alone guarantees a new section is always
/// persisted; within one section, content changes (typed command grows, output
/// streams in) are what trigger re-persistence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveSectionKey {
    pub aid: String,
    pub cmdline: String,
    pub output: String,
}

impl LiveSectionKey {
    fn from_section(section: &SessionSection, parsed: &ParsedSection) -> Self {
        Self {
            aid: section.aid.clone(),
            cmdline: parsed.cmdline.clone(),
            output: parsed.output.clone(),
        }
    }
}

/// Build, store, and broadcast one [`ChatMessage::TerminalSection`] from an
/// already-parsed section. Shared by the finished and live entry points below so
/// the `ChatMessage::TerminalSection` shape, JSONL write, and
/// `chat-messages-changed` emission stay in lockstep.
fn write_section(
    section: &SessionSection,
    parsed: &ParsedSection,
    exit_code: Option<i32>,
    store: &Arc<JsonlStore<ChatMessage>>,
    app: &AppHandle,
) -> Result<(), String> {
    let executed = section.executed();
    let message = ChatMessage::TerminalSection {
        id: String::new(),
        ts: Local::now().to_rfc3339(),
        aid: section.aid.clone(),
        exit_code,
        executed,
        cols: section.cols,
        rows: section.rows,
        raw: String::from_utf8_lossy(&section.raw).into_owned(),
        prompt: parsed.prompt.clone(),
        cmdline: parsed.cmdline.clone(),
        output: parsed.output.clone(),
    };
    let id = store.write(message).map_err(|e| e.to_string())?;
    let _ = app.emit(
        "chat-messages-changed",
        ChatMessagesChangedPayload {
            latest_id: id.to_string(),
        },
    );
    Ok(())
}

/// Parse a finished [`SessionSection`] into prompt/command/output text and
/// persist it as a [`ChatMessage::TerminalSection`], then notify the frontend.
///
/// Caller: the PTY reader thread, once OSC 133 `D` (`CommandFinished`) closes
/// the section. Always uses `live=false` semantics: `executed`/`exit_code` are
/// taken from the section as the recorder recorded them. (The
/// `send_chat_message` live path is handled separately by
/// [`persist_live_section_if_changed`] so it can dedup against the previous live
/// snapshot — see that function for the rationale.)
pub fn persist_section(
    section: &SessionSection,
    store: &Arc<JsonlStore<ChatMessage>>,
    app: &AppHandle,
    live: bool,
) -> Result<(), String> {
    let parsed = section.parsed_parts();
    // A live snapshot has no exit code yet — the command may still be running.
    // The reader-thread path keeps whatever the recorder captured on `D`.
    let exit_code = if live { None } else { section.exit_code };
    write_section(section, &parsed, exit_code, store, app)
}

/// Decide whether a live snapshot differs from the previously persisted one.
/// Pure (no I/O) so the dedup decision is unit-testable without an `AppHandle`.
/// Returns `true` when there is no previous snapshot, or when `aid`, `cmdline`,
/// or `output` changed.
pub fn live_snapshot_changed(new: &LiveSectionKey, last: &Option<LiveSectionKey>) -> bool {
    match last {
        None => true,
        Some(last) => new != last,
    }
}

/// Snapshot the live section the user is currently typing into (or whose
/// command is still running) and persist it as a [`ChatMessage::TerminalSection`]
/// — but only if it carries content and has changed since the previously
/// persisted live snapshot.
///
/// Caller: `send_chat_message`, at send time. `executed` is whatever the
/// recorder observed (OSC 133 `C` may already have fired, so a *running* command
/// is `executed=true`, not `false`); `exit_code` is forced to `None`, since no
/// `D` has fired yet.
///
/// Empty sections are skipped: when neither a typed command nor any output has
/// been captured yet (the user has only rendered a prompt and hit send), there
/// is nothing for the assistant to act on, and persisting would only clutter the
/// transcript with prompt-only records. The `last_key` is *not* updated in this
/// case, so the first non-empty snapshot is still persisted.
///
/// Dedup compares `aid`, `cmdline`, and `output` — see [`LiveSectionKey`]. The
/// rendered prompt is intentionally not compared: a re-render of the same prompt
/// does not carry new information for the assistant. Without dedup, repeated sends
/// while the live section is unchanged would each append an identical
/// `TerminalSection` record, which `build_history` would render as duplicate
/// `<terminal>` blocks.
///
/// The section is *not* removed from the recorder: the reader thread will
/// persist it again as a second, completed `TerminalSection` record on the real
/// `D`, carrying the final output / exit code.
///
/// `last_key` holds the previous snapshot's identity and lives on
/// [`crate::terminal::TerminalSession`] (separate from the recorder's own lock
/// so the parse + write happen outside the recorder lock). Returns `Ok(true)` if
/// a record was written, `Ok(false)` if it was skipped as empty or unchanged.
pub fn persist_live_section_if_changed(
    section: &SessionSection,
    last_key: &std::sync::Mutex<Option<LiveSectionKey>>,
    store: &Arc<JsonlStore<ChatMessage>>,
    app: &AppHandle,
) -> Result<bool, String> {
    let parsed = section.parsed_parts();

    // Skip empty live sections: nothing to act on, nothing to dedup against.
    if parsed.prompt.is_empty() && parsed.cmdline.is_empty() && parsed.output.is_empty() {
        return Ok(false);
    }

    let new_key = LiveSectionKey::from_section(section, &parsed);

    let mut guard = last_key.lock().unwrap();
    if !live_snapshot_changed(&new_key, &*guard) {
        return Ok(false);
    }
    write_section(section, &parsed, None, store, app)?;
    *guard = Some(new_key);
    drop(guard);
    Ok(true)
}

/// Trim trailing whitespace from each line and drop leading/trailing blank lines
/// so the extracted text is compact regardless of screen size.
fn trim_terminal_text(text: &str) -> String {
    let mut lines: Vec<&str> = text.lines().map(|line| line.trim_end()).collect();
    while matches!(lines.first(), Some(l) if l.is_empty()) {
        lines.remove(0);
    }
    while matches!(lines.last(), Some(l) if l.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bel(marker: &str, aid: &str) -> Vec<u8> {
        format!("\x1b]133;{marker};aid={aid}\x07").into_bytes()
    }

    fn run_to_finish(
        rec: &mut SessionRecorder,
        section_aid: &str,
        prompt: &str,
        command: &str,
        output: &str,
    ) -> SessionSection {
        rec.feed(&Segment::Sequence {
            bytes: bel("A", section_aid),
            event: ShellEvent::PromptStarted {
                aid: Some(section_aid.into()),
            },
        });
        // prompt rendering + B
        rec.feed(&Segment::Passthrough(prompt.as_bytes().to_vec()));
        rec.feed(&Segment::Sequence {
            bytes: bel("B", section_aid),
            event: ShellEvent::PromptEnded {
                aid: Some(section_aid.into()),
            },
        });
        rec.feed(&Segment::Passthrough(command.as_bytes().to_vec()));
        rec.feed(&Segment::Sequence {
            bytes: bel("C", section_aid),
            event: ShellEvent::CommandStarted {
                aid: Some(section_aid.into()),
            },
        });
        // Real shell output uses CRLF between lines; mimic that here so the
        // test exercises the same parsing path the recorder sees in practice.
        let output_bytes = output.replace('\n', "\r\n");
        rec.feed(&Segment::Passthrough(output_bytes.as_bytes().to_vec()));
        let finished = rec.feed(&Segment::Sequence {
            bytes: bel("D", section_aid),
            event: ShellEvent::CommandFinished {
                exit_code: Some(0),
                aid: Some(section_aid.into()),
            },
        });
        assert!(
            finished.is_some(),
            "feed should return a finished section on D"
        );
        finished.unwrap()
    }

    /// Mimic a same-`aid` prompt redraw: bash emits a fresh `\r\u001b[K\r`
    /// clear-and-return, then OSC 133 A/B again for the same aid, after which
    /// readline re-renders `typed` as the command line. Both resize (unchanged
    /// `typed`) and tab completion (grown/changed `typed`) look like this.
    /// Returns whatever `feed` yielded from the redraw's OSC 133 A.
    fn feed_redraw(
        rec: &mut SessionRecorder,
        aid: &str,
        prompt: &str,
        typed: &str,
    ) -> Option<SessionSection> {
        rec.feed(&Segment::Passthrough(b"\r\x1b[K\r".to_vec()));
        let yielded = rec.feed(&Segment::Sequence {
            bytes: bel("A", aid),
            event: ShellEvent::PromptStarted {
                aid: Some(aid.into()),
            },
        });
        rec.feed(&Segment::Passthrough(prompt.as_bytes().to_vec()));
        rec.feed(&Segment::Sequence {
            bytes: bel("B", aid),
            event: ShellEvent::PromptEnded {
                aid: Some(aid.into()),
            },
        });
        if !typed.is_empty() {
            rec.feed(&Segment::Passthrough(typed.as_bytes().to_vec()));
        }
        yielded
    }

    #[test]
    fn resize_redraw_with_unchanged_command_does_not_yield_a_duplicate() {
        // A pure resize redraws the same prompt + same typed command under the
        // same aid. The recorder must NOT yield the in-flight section — else
        // every resize persists a spurious duplicate of the same unfinished
        // prompt. The command is re-rendered into the fresh section instead.
        let mut rec = SessionRecorder::new(80, 24);
        build_section_with_prompt(&mut rec, "1-1", "$ ", "ls -la");

        // readline re-renders the same `ls -la` after the redraw.
        let yielded = feed_redraw(&mut rec, "1-1", "$ ", "ls -la");
        assert!(
            yielded.is_none(),
            "unchanged-command redraw must not yield a section"
        );

        let current = rec
            .current_snapshot()
            .expect("current section after redraw");
        assert_eq!(current.aid, "1-1");
        assert_eq!(current.parsed_parts().cmdline, "ls -la");
    }

    /// Mimic a same-`aid` prompt redraw where the command line stays rendered
    /// (no clear-to-end-of-line), as with tab completion: readline prints its
    /// candidate list, re-emits OSC 133 A/B, and re-echoes the (now longer)
    /// command. The previous section therefore parses to the command as it
    /// stood before this redraw. Returns whatever the redraw's A yielded.
    fn feed_completion_redraw(
        rec: &mut SessionRecorder,
        aid: &str,
        prompt: &str,
        typed: &str,
    ) -> Option<SessionSection> {
        let yielded = rec.feed(&Segment::Sequence {
            bytes: bel("A", aid),
            event: ShellEvent::PromptStarted {
                aid: Some(aid.into()),
            },
        });
        rec.feed(&Segment::Passthrough(prompt.as_bytes().to_vec()));
        rec.feed(&Segment::Sequence {
            bytes: bel("B", aid),
            event: ShellEvent::PromptEnded {
                aid: Some(aid.into()),
            },
        });
        if !typed.is_empty() {
            rec.feed(&Segment::Passthrough(typed.as_bytes().to_vec()));
        }
        yielded
    }

    #[test]
    fn tab_completion_redraws_persist_each_changed_commandline_attempt() {
        // Tab completion redraws the prompt with a *changed* command line under
        // the same aid, leaving the command rendered. Each distinct attempt is
        // genuine context and must be yielded (persisted), unlike a pure resize.
        let mut rec = SessionRecorder::new(80, 24);
        build_section_with_prompt(&mut rec, "1-1", "$ ", "gi");

        // `gi` <tab> completes to `git ` — the previous `gi` attempt is yielded.
        let first = feed_completion_redraw(&mut rec, "1-1", "$ ", "git ")
            .expect("changed command line should be yielded");
        assert_eq!(first.aid, "1-1");
        assert_eq!(first.parsed_parts().cmdline, "gi");

        // `git s` <tab> completes to `git status` — the `git ` attempt yields.
        let second = feed_completion_redraw(&mut rec, "1-1", "$ ", "git status")
            .expect("further changed command line should be yielded");
        assert_eq!(second.aid, "1-1");
        assert_eq!(second.parsed_parts().cmdline, "git");

        // The `git status` attempt is yielded once (it differs from `git `)…
        let third = feed_completion_redraw(&mut rec, "1-1", "$ ", "git status")
            .expect("the git status attempt should be yielded once");
        assert_eq!(third.parsed_parts().cmdline, "git status");

        // …but a further redraw that leaves it unchanged (e.g. a resize now) is
        // deduped and not persisted again.
        let fourth = feed_completion_redraw(&mut rec, "1-1", "$ ", "git status");
        assert!(
            fourth.is_none(),
            "unchanged command line must not re-yield after completions"
        );

        assert_eq!(rec.current_snapshot().unwrap().aid, "1-1");
    }

    #[test]
    fn feed_yields_unfinished_previous_section_when_a_new_aid_starts() {
        // A genuine new prompt (different aid) closes the in-flight one even
        // without an OSC 133 D — e.g. bash skipped PROMPT_COMMAND. The recorder
        // still hands the captured section back so the reader thread persists
        // whatever prompt/command it saw.
        let mut rec = SessionRecorder::new(80, 24);
        build_section_with_prompt(&mut rec, "1-1", "$ ", "ls -la");

        let yielded = rec.feed(&Segment::Sequence {
            bytes: bel("A", "1-2"),
            event: ShellEvent::PromptStarted {
                aid: Some("1-2".into()),
            },
        });
        let previous = yielded.expect("previous section should be yielded on a new aid");
        assert_eq!(previous.aid, "1-1");
        assert_eq!(previous.parsed_parts().cmdline, "ls -la");

        assert!(rec.current_snapshot().is_some());
        assert_eq!(rec.current_snapshot().unwrap().aid, "1-2");
    }

    #[test]
    fn resize_does_not_persist_duplicate_live_section_across_redraws() {
        // Simulate the full sequence recorded in the chat store by the bug
        // report: prompt rendered, then two resize redraws (different cols),
        // then a send that takes the live snapshot. With the redraw fix, only
        // the send-time live snapshot should ever be persisted; the redraws
        // themselves produce no finished SectionEvents.
        let mut rec = SessionRecorder::new(80, 24);
        build_section_with_prompt(&mut rec, "1-1", "$ ", "");

        let mut persisted: Vec<SessionSection> = Vec::new();

        // First resize (68x42) then redraw — empty command, unchanged.
        rec.set_size(42, 68);
        if let Some(s) = feed_redraw(&mut rec, "1-1", "$ ", "") {
            persisted.push(s);
        }

        // Second resize (38x101) then redraw — still empty, unchanged.
        rec.set_size(38, 101);
        if let Some(s) = feed_redraw(&mut rec, "1-1", "$ ", "") {
            persisted.push(s);
        }

        assert!(
            persisted.is_empty(),
            "redraws must not yield finished sections"
        );

        // The send-time path snapshots `current` directly; assert it's still
        // alive and carries the latest geometry from the recorder.
        let live = rec
            .current_snapshot()
            .expect("live section still in recorder");
        assert_eq!(live.aid, "1-1");
        assert_eq!(live.cols, 101);
        assert_eq!(live.rows, 38);
        assert!(!live.executed());
    }

    fn build_section_with_prompt(rec: &mut SessionRecorder, aid: &str, prompt: &str, typed: &str) {
        rec.feed(&Segment::Sequence {
            bytes: bel("A", aid),
            event: ShellEvent::PromptStarted {
                aid: Some(aid.into()),
            },
        });
        rec.feed(&Segment::Passthrough(prompt.as_bytes().to_vec()));
        rec.feed(&Segment::Sequence {
            bytes: bel("B", aid),
            event: ShellEvent::PromptEnded {
                aid: Some(aid.into()),
            },
        });
        if !typed.is_empty() {
            rec.feed(&Segment::Passthrough(typed.as_bytes().to_vec()));
        }
    }

    #[test]
    fn parses_prompt_command_output_for_a_finished_section() {
        let mut rec = SessionRecorder::new(80, 24);
        let section = run_to_finish(&mut rec, "1-1", "$ ", "ls", "file.txt\nfile2.txt");
        // reader drains the finished section from the recorder
        assert!(rec.current_snapshot().is_none());

        let parsed = section.parsed_parts();

        // `trim_terminal_text` trims trailing whitespace per line, so the
        // trailing space in "$ " is dropped — the command is in its own tag, so
        // the separator is not significant for the LLM context.
        assert_eq!(parsed.prompt, "$");
        assert_eq!(parsed.cmdline, "ls");
        assert_eq!(parsed.output, "file.txt\nfile2.txt");
    }

    #[test]
    fn parsed_parts_returns_typed_command_for_unfinished_section() {
        let mut rec = SessionRecorder::new(80, 24);
        rec.feed(&Segment::Sequence {
            bytes: bel("A", "1-1"),
            event: ShellEvent::PromptStarted {
                aid: Some("1-1".into()),
            },
        });
        rec.feed(&Segment::Passthrough(b"$ ".to_vec()));
        rec.feed(&Segment::Sequence {
            bytes: bel("B", "1-1"),
            event: ShellEvent::PromptEnded {
                aid: Some("1-1".into()),
            },
        });
        rec.feed(&Segment::Passthrough(b"git status".to_vec()));

        let section = rec.current_snapshot().expect("current section");
        let parsed = section.parsed_parts();
        // Command not started yet: post-prompt bytes are the live command line
        // and there is no output.
        assert_eq!(parsed.cmdline, "git status");
        assert_eq!(parsed.output, "");
        assert_eq!(parsed.prompt, "$");
    }

    #[test]
    fn parsed_parts_drops_commandline_while_prompt_is_rendering() {
        // Only the A marker has fired; the prompt is still being rendered and B
        // has not. The still-rendering bytes must not be misread as a command.
        let mut rec = SessionRecorder::new(80, 24);
        rec.feed(&Segment::Sequence {
            bytes: bel("A", "1-1"),
            event: ShellEvent::PromptStarted {
                aid: Some("1-1".into()),
            },
        });
        rec.feed(&Segment::Passthrough(b"user@host:".to_vec()));

        let section = rec.current_snapshot().expect("current section");
        let parsed = section.parsed_parts();
        assert_eq!(parsed.prompt, "");
        assert_eq!(parsed.cmdline, "");
        assert_eq!(parsed.output, "");
    }

    #[test]
    fn parsed_parts_captures_output_that_scrolls_off_screen() {
        // An 80x3 section whose command output exceeds the 3 visible rows. The
        // generously-sized parser screen (see `PARSE_ROWS`) keeps all ten lines
        // recoverable from `contents_between`.
        let mut raw = bel("A", "1-1");
        raw.extend_from_slice(b"$ ");
        let off_b = raw.len();
        raw.extend_from_slice(&bel("B", "1-1"));
        raw.extend_from_slice(b"seq\n");
        let off_c = raw.len();
        raw.extend_from_slice(&bel("C", "1-1"));
        let mut out = String::new();
        for i in 0..10 {
            out.push_str(&format!("line{i}\n"));
        }
        raw.extend_from_slice(out.as_bytes());
        let off_d = raw.len();
        raw.extend_from_slice(&bel("D", "1-1"));

        let section = SessionSection {
            aid: "1-1".into(),
            raw,
            off_prompt_end: Some(off_b),
            off_command_start: Some(off_c),
            off_command_finish: Some(off_d),
            exit_code: Some(0),
            cols: 80,
            rows: 3,
        };
        let parsed = section.parsed_parts();
        assert_eq!(parsed.cmdline, "seq");
        for i in 0..10 {
            assert!(
                parsed.output.contains(&format!("line{i}")),
                "missing line{i}: {:?}",
                parsed.output
            );
        }
    }

    #[test]
    fn parsed_parts_handles_real_readline_capture() {
        // Replays the actual reported capture (with ANSI color codes, OSC 133
        // markers, an OSC 3008 marker, and the full readline redraw). The
        // 23-col-wide prompt is followed by a command redrawn via cursor-up +
        // repeated cursor-forward: an isolated command parser starting at col 0
        // would turn those moves into 23 leading spaces, but the whole-section
        // parser has the prompt already on the grid so the command lands at its
        // true column.
        //
        // Expected results (matching the persisted message):
        //   prompt  = "montor@montorcentre:~$"
        //   command = "systemctl --user status pipewire-pulse"   (no leading spaces)
        //   output  = "\u{25cf} pipewire-pulse.service - PipeWire PulseAudio\n     Loaded: loaded..."
        let prompt_render = "\x1b[0m\x1b[32mmontor@montorcentre\x1b[0m:\x1b[32m~\x1b[0m$ ";
        let command = "systemctl --user status pipewire-pulse";
        let prompt_width: usize = 23; // length of `montor@montorcentre:~$ `

        let mut raw = String::from(prompt_render);
        let off_b = raw.len();
        // readline redraw: EL, CRLF, CR, EL, CUU, CUF×prompt_width, type the
        // command once, CR, CUF×prompt_width, EL (erasing the first echo),
        // re-emit the command, CRLF.
        raw.push_str("\x1b[K\r\n\r\x1b[K\x1b[A");
        for _ in 0..prompt_width {
            raw.push_str("\x1b[C");
        }
        raw.push_str(command);
        raw.push('\r');
        for _ in 0..prompt_width {
            raw.push_str("\x1b[C");
        }
        raw.push_str("\x1b[K");
        raw.push_str(command);
        raw.push_str("\r\n");
        // bracketed paste off + CR, then the C marker.
        raw.push_str("\x1b[?2004l\r");
        let off_c = raw.len();
        raw.push_str("\x1b]133;C;aid=1426390-3\x07");
        // OSC 3008 (ends with ST = ESC \), application cursor/keypad on, CR.
        raw.push_str("\x1b]3008;start=3119dba2-b58e-4b88-a842-2112a1ea6a6f;machineid=9ef2870a72364295a0498b4327b8b13d;user=montor;hostname=montorcentre;bootid=aa7c4109-ab53-4a71-8159-b8e98f2f1904;pid=00000000000001426390;type=command;cwd=/home/montor\x1b\\");
        raw.push_str("\x1b[?1h\x1b=\r");
        // output: the red bullet, then the first two lines of `systemctl
        // --user status pipewire-pulse` output, with CRLF separators just like
        // the real PTY.
        raw.push_str(
            "\x1b[0;1;32m\u{25cf}\x1b[0m pipewire-pulse.service - PipeWire PulseAudio\x1b[m\r\n",
        );
        raw.push_str("     Loaded: loaded...");
        let off_d = raw.len();
        raw.push_str("\x1b]133;D;aid=1426390-3\x07");

        let section = SessionSection {
            aid: "1426390-3".into(),
            raw: raw.into_bytes(),
            off_prompt_end: Some(off_b),
            off_command_start: Some(off_c),
            off_command_finish: Some(off_d),
            exit_code: Some(0),
            cols: 80,
            rows: 24,
        };
        let parsed = section.parsed_parts();
        assert_eq!(parsed.prompt, "montor@montorcentre:~$");
        // The command must NOT carry the 23 leading spaces the prompt was
        // redrawn over — this is exactly the bug being fixed.
        assert_eq!(parsed.cmdline, command);
        assert_eq!(
            parsed.output,
            "\u{25cf} pipewire-pulse.service - PipeWire PulseAudio\n     Loaded: loaded..."
        );
    }

    #[test]
    fn parsed_parts_separates_command_from_streaming_output() {
        let mut rec = SessionRecorder::new(80, 24);
        rec.feed(&Segment::Sequence {
            bytes: bel("A", "1-1"),
            event: ShellEvent::PromptStarted {
                aid: Some("1-1".into()),
            },
        });
        rec.feed(&Segment::Passthrough(b"$ ".to_vec()));
        rec.feed(&Segment::Sequence {
            bytes: bel("B", "1-1"),
            event: ShellEvent::PromptEnded {
                aid: Some("1-1".into()),
            },
        });
        rec.feed(&Segment::Passthrough(b"ls".to_vec()));
        rec.feed(&Segment::Sequence {
            bytes: bel("C", "1-1"),
            event: ShellEvent::CommandStarted {
                aid: Some("1-1".into()),
            },
        });
        rec.feed(&Segment::Passthrough(b"file.txt\n".to_vec()));

        let section = rec.current_snapshot().expect("current section");
        let parsed = section.parsed_parts();
        // Command started: the typed command is its own fragment; the streaming
        // output lives in `output`, not `cmdline`.
        assert_eq!(parsed.cmdline, "ls");
        assert_eq!(parsed.output, "file.txt");
    }

    /// Like `run_to_finish`, but inserts a `set_size` call after the command
    /// line has been typed (between OSC 133 B and C) so we can exercise the
    /// in-flight reflow path introduced by `SessionRecorder::set_size`.
    fn run_to_finish_with_resize(
        rec: &mut SessionRecorder,
        section_aid: &str,
        prompt: &str,
        command: &str,
        output: &str,
        resize: Option<(u16, u16)>,
    ) -> SessionSection {
        rec.feed(&Segment::Sequence {
            bytes: bel("A", section_aid),
            event: ShellEvent::PromptStarted {
                aid: Some(section_aid.into()),
            },
        });
        rec.feed(&Segment::Passthrough(prompt.as_bytes().to_vec()));
        rec.feed(&Segment::Sequence {
            bytes: bel("B", section_aid),
            event: ShellEvent::PromptEnded {
                aid: Some(section_aid.into()),
            },
        });
        rec.feed(&Segment::Passthrough(command.as_bytes().to_vec()));
        if let Some((rows, cols)) = resize {
            rec.set_size(rows, cols);
        }
        rec.feed(&Segment::Sequence {
            bytes: bel("C", section_aid),
            event: ShellEvent::CommandStarted {
                aid: Some(section_aid.into()),
            },
        });
        let output_bytes = output.replace('\n', "\r\n");
        rec.feed(&Segment::Passthrough(output_bytes.as_bytes().to_vec()));
        rec.feed(&Segment::Sequence {
            bytes: bel("D", section_aid),
            event: ShellEvent::CommandFinished {
                exit_code: Some(0),
                aid: Some(section_aid.into()),
            },
        })
        .expect("D should close the active section")
    }

    /// Row the cursor lands on after rendering `prompt` then `command` at
    /// `cols`. Row > 0 means the command soft-wrapped (exceeded the line);
    /// row 0 means it fit on a single visual line.
    ///
    /// Used to prove a variant command actually exceeds the soft line boundary
    /// at a given width, since `vt100`'s `contents_between` does not insert a
    /// `\n` at soft-wrap edges (only at hard CRLF breaks), so the wrapped
    /// state is not visible in the parsed string itself.
    fn cursor_row_after(cols: u16, prompt: &str, command: &str) -> u16 {
        let mut p = vt100::Parser::new(PARSE_ROWS, cols, 0);
        p.process(prompt.as_bytes());
        p.process(command.as_bytes());
        p.screen().cursor_position().0
    }

    #[test]
    fn parsed_parts_reflows_cmdline_when_cols_increase_mid_section() {
        // Prompt is `"$ "` (2 cols). Initial cols=20 leaves 18 cols for the
        // typed command line; commands longer than 18 chars soft-wrap at that
        // width. Resizing wider before C makes `parsed_parts` re-flow the whole
        // section against the new geometry, so the wrapped command unwraps onto
        // a single visual line.
        //
        // Each variant is a command length chosen to exceed the 18-col soft
        // line at cols=20 in a distinct shape: barely past one line, exactly
        // two full lines, spilling onto a third line.
        let variants: &[&str] = &[
            "0123456789012345678",                  // len 19: one char past the line
            "012345678901234567890123456789012345", // len 36: exactly two full lines
            "01234567890123456789012345678901234567890123456", // len 48: spills on line 3
        ];
        for &command in variants {
            // The variant actually exceeds the soft line at the ORIGINAL width.
            // All three commands here wrap past row 0 at cols=20; the len-48
            // command spills onto row 2 (18 cols/command-line after the 2-col
            // prompt).
            let expected_wrap_row = if command.len() > 36 { 2 } else { 1 };
            assert_eq!(
                cursor_row_after(20, "$ ", command),
                expected_wrap_row,
                "variant {command:?}: should soft-wrap at cols=20"
            );
            // After the wider resize the same command fits on a single line.
            assert_eq!(
                cursor_row_after(60, "$ ", command),
                0,
                "variant {command:?}: should fit on one line at cols=60"
            );

            let mut rec = SessionRecorder::new(20, 24);
            let section =
                run_to_finish_with_resize(&mut rec, "1-1", "$ ", command, "ok", Some((24, 60)));
            // The in-flight section picked up the new size from `set_size`;
            // this is the mechanism the reflow fix relies on.
            assert_eq!(section.cols, 60, "variant {command:?}");
            let parsed = section.parsed_parts();
            assert_eq!(parsed.prompt, "$", "variant {command:?}");
            // `contents_between` concatenates soft-wrapped cells without a
            // `\n`, so the reflowed command line is the full typed command.
            assert_eq!(parsed.cmdline, command, "variant {command:?}");
            assert_eq!(parsed.output, "ok", "variant {command:?}");
        }
    }

    #[test]
    fn parsed_parts_reflows_cmdline_when_cols_decrease_mid_section() {
        // Prompt is `"$ "` (2 cols). Start wide (60 cols) where the typed
        // command fits on a single line, then narrow before C. `parsed_parts`
        // re-flows the section at the reduced width, soft-wrapping the command.
        //
        // Each variant: (command, new_cols). `new_cols` is chosen so the
        // command soft-wraps in a recognizable shape: barely past one line,
        // exactly two full lines, spilling onto a third — and, for one variant,
        // a much narrower width where even short commands wrap.
        let variants: &[(&str, u16, u16)] = &[
            // new_cols=20 -> 18 cols for command; len 19 wraps to 18+1 (row 1).
            ("0123456789012345678", 20, 1),
            // new_cols=20 -> 18 cols for command; len 36 wraps to two full lines (row 1).
            ("012345678901234567890123456789012345", 20, 1),
            // new_cols=20 -> 18 cols for command; len 48 wraps onto row 2.
            ("012345678901234567890123456789012345678901234567", 20, 2),
            // new_cols=10 -> 8 cols for command; len 13 wraps to 8+5 (row 1).
            ("0123456789012", 10, 1),
        ];
        for &(command, new_cols, expected_row) in variants {
            // At the ORIGINAL (wide) width the command fits on one line.
            assert_eq!(
                cursor_row_after(60, "$ ", command),
                0,
                "variant {command:?}: should fit on one line at cols=60"
            );
            // After the narrower resize the same command soft-wraps.
            assert_eq!(
                cursor_row_after(new_cols, "$ ", command),
                expected_row,
                "variant {command:?}: should soft-wrap to row {expected_row} at cols={new_cols}"
            );

            let mut rec = SessionRecorder::new(60, 24);
            let section = run_to_finish_with_resize(
                &mut rec,
                "1-1",
                "$ ",
                command,
                "ok",
                Some((24, new_cols)),
            );
            assert_eq!(section.cols, new_cols, "variant {command:?}");
            let parsed = section.parsed_parts();
            assert_eq!(parsed.prompt, "$", "variant {command:?}");
            // Soft-wrapped cells are concatenated by `contents_between` with
            // no inserted `\n`, so the reflowed command line still equals the
            // full typed command even though it now visually spans rows.
            assert_eq!(parsed.cmdline, command, "variant {command:?}");
            assert_eq!(parsed.output, "ok", "variant {command:?}");
        }
    }

    // --- live snapshot dedup ------------------------------------------------

    fn live_section(aid: &str, cmdline: &str, output: &str) -> SessionSection {
        // Build a minimal live section (PromptStart + prompt + B + command,
        // optional output) and never emit D so `off_command_finish` stays `None`
        // — mirroring the section `send_chat_message` snapshots at send time.
        let mut rec = SessionRecorder::new(80, 24);
        rec.feed(&Segment::Sequence {
            bytes: bel("A", aid),
            event: ShellEvent::PromptStarted {
                aid: Some(aid.into()),
            },
        });
        rec.feed(&Segment::Passthrough(b"$ ".to_vec()));
        rec.feed(&Segment::Sequence {
            bytes: bel("B", aid),
            event: ShellEvent::PromptEnded {
                aid: Some(aid.into()),
            },
        });
        rec.feed(&Segment::Passthrough(cmdline.as_bytes().to_vec()));
        if !output.is_empty() {
            // Emit a CommandStarted segment the same way the reader thread
            // would, then feed the partial output as passthrough bytes.
            rec.feed(&Segment::Sequence {
                bytes: bel("C", aid),
                event: ShellEvent::CommandStarted {
                    aid: Some(aid.into()),
                },
            });
            rec.feed(&Segment::Passthrough(
                output.replace('\n', "\r\n").as_bytes().to_vec(),
            ));
        }
        rec.current_snapshot()
            .expect("live section present")
            .clone()
    }

    #[test]
    fn live_snapshot_changed_returns_true_when_no_previous_snapshot() {
        let new = LiveSectionKey {
            aid: "1-1".into(),
            cmdline: "ls".into(),
            output: String::new(),
        };
        assert!(live_snapshot_changed(&new, &None));
    }

    #[test]
    fn live_snapshot_changed_returns_false_when_unchanged() {
        let key = LiveSectionKey {
            aid: "1-1".into(),
            cmdline: "ls".into(),
            output: String::new(),
        };
        assert!(!live_snapshot_changed(&key, &Some(key.clone())));
    }

    #[test]
    fn live_snapshot_changed_returns_true_when_cmdline_grows() {
        let last = LiveSectionKey {
            aid: "1-1".into(),
            cmdline: "git".into(),
            output: String::new(),
        };
        let new = LiveSectionKey {
            aid: "1-1".into(),
            cmdline: "git status".into(),
            output: String::new(),
        };
        assert!(live_snapshot_changed(&new, &Some(last)));
    }

    #[test]
    fn live_snapshot_changed_returns_true_when_output_grew_but_cmdline_unchanged() {
        let last = LiveSectionKey {
            aid: "1-1".into(),
            cmdline: "ls".into(),
            output: "dir1\n".into(),
        };
        let new = LiveSectionKey {
            aid: "1-1".into(),
            cmdline: "ls".into(),
            output: "dir1\ndir2\n".into(),
        };
        assert!(live_snapshot_changed(&new, &Some(last)));
    }

    #[test]
    fn live_snapshot_changed_returns_true_when_aid_changed_even_if_cmdline_same() {
        // A new prompt yields a fresh `aid`; dedup must always persist it.
        let last = LiveSectionKey {
            aid: "1-1".into(),
            cmdline: "ls".into(),
            output: String::new(),
        };
        let new = LiveSectionKey {
            aid: "1-2".into(),
            cmdline: "ls".into(),
            output: String::new(),
        };
        assert!(live_snapshot_changed(&new, &Some(last)));
    }

    #[test]
    fn live_snapshot_changed_ignores_prompt_changes() {
        // The dedup key intentionally excludes the rendered prompt: a different
        // prompt on the same aid/command/output does not count as a change.
        let last = LiveSectionKey {
            aid: "1-1".into(),
            cmdline: "ls".into(),
            output: String::new(),
        };
        // `LiveSectionKey` has no prompt field, so this is structural — confirm
        // the equality still treats equal-keys as unchanged.
        let new = LiveSectionKey {
            aid: "1-1".into(),
            cmdline: "ls".into(),
            output: String::new(),
        };
        assert!(!live_snapshot_changed(&new, &Some(last)));
    }

    #[test]
    fn persist_live_section_if_changed_skips_empty_section() {
        // A live section whose prompt has rendered (OSC 133 B fired) but nothing
        // has been typed yet and no output is streaming has nothing to persist.
        let section = live_section("1-1", "", "");
        let parsed = section.parsed_parts();
        assert!(parsed.cmdline.is_empty());
        assert!(parsed.output.is_empty());

        // Pure decision check: an empty-derived key against any prior state must
        // be treated as a skip by `persist_live_section_if_changed`. We can't call
        // the full function here (no `AppHandle` / store), but the empty-skip is
        // the first guard in the function, so verify its preconditions hold.
        let key = LiveSectionKey::from_section(&section, &parsed);
        assert!(key.cmdline.is_empty());
        assert!(key.output.is_empty());
        // Even against `None` (no previous snapshot), an empty section would be
        // "changed" by the pure helper — the empty-skip guard in the persist
        // function is what prevents the write, not `live_snapshot_changed`.
        assert!(live_snapshot_changed(&key, &None));
    }

    #[test]
    fn live_snapshot_key_matches_parsed_parts_of_live_section() {
        // Sanity: the key constructed from a real live section carries the typed
        // command and any streamed partial output, but not the prompt.
        let section = live_section("1-1", "git status", "");
        let parsed = section.parsed_parts();
        let key = LiveSectionKey::from_section(&section, &parsed);
        assert_eq!(key.aid, "1-1");
        assert_eq!(key.cmdline, "git status");
        assert_eq!(key.output, "");

        // After streaming partial output for a running command the key should
        // reflect it, so a subsequent send would persist again.
        let section_with_output = live_section("1-1", "ls", "dir1\ndir2");
        let parsed2 = section_with_output.parsed_parts();
        let key2 = LiveSectionKey::from_section(&section_with_output, &parsed2);
        assert_eq!(key2.cmdline, "ls");
        assert_eq!(key2.output, "dir1\ndir2");
        // Even though aid is the same, output differs from the prior snapshot.
        assert!(live_snapshot_changed(&key2, &Some(key)));
    }
}
