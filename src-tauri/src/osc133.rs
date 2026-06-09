/// Scanner for OSC 133 shell-integration sequences in a PTY byte stream.
///
/// The scanner detects complete OSC 133 sequences across chunk boundaries but
/// does **not** strip them — the raw bytes should still be forwarded to the
/// terminal emulator unchanged. When a recognised sequence is found the
/// caller-supplied callback is invoked.

/// OSC number for shell integration markers.
const OSC_SHELL_INTEGRATION: &[u8] = b"133;";

/// Maximum carry-buffer size. If exceeded, the oldest bytes are discarded to
/// prevent unbounded growth from a malformed or unterminated sequence.
const MAX_CARRY: usize = 512;

// ── Byte-level constants ──────────────────────────────────────────────

/// ESC — starts all escape sequences.
const ESC: u8 = 0x1B;
/// BEL — one of two OSC terminators.
const BEL: u8 = 0x07;
/// `]` — introduces an OSC after ESC.
const BRACKET: u8 = b']';
/// `\` — second byte of the ST (String Terminator) sequence ESC \.
const ST_SECOND: u8 = b'\\';

// ── OSC 133 parameter identifiers ─────────────────────────────────────

const MARKER_PROMPT: &[u8] = b"A";
const MARKER_PROMPT_END: &[u8] = b"B";
const MARKER_COMMAND: &[u8] = b"C";
const MARKER_FINISHED: &[u8] = b"D";

/// A recognised OSC 133 shell-integration event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellEvent {
    PromptStarted {
        aid: Option<u64>,
    },
    PromptEnded {
        aid: Option<u64>,
    },
    CommandStarted {
        aid: Option<u64>,
    },
    CommandFinished {
        exit_code: Option<i32>,
        aid: Option<u64>,
    },
}

/// Invoke `callback` for every complete OSC 133 sequence found in
/// `carry + chunk`.
///
/// Already-consumed bytes are drained from `carry` on return. The caller
/// should still forward the original `chunk` verbatim to the terminal.
pub fn scan(carry: &mut Vec<u8>, chunk: &[u8], mut callback: impl FnMut(ShellEvent)) {
    carry.extend_from_slice(chunk);

    let mut pos = 0;

    'outer: while pos < carry.len() {
        // Fast skip: advance past bytes that cannot start an escape.
        if carry[pos] != ESC {
            pos += 1;
            continue;
        }

        // ESC at end of buffer – could be start of ESC ] …; keep it.
        if pos + 1 >= carry.len() {
            break;
        }

        // ESC followed by something other than ] — not an OSC.
        if carry[pos + 1] != BRACKET {
            pos += 2;
            continue;
        }

        // ESC ] found — scan forward for BEL or ST.
        let content_start = pos + 2;
        let mut k = content_start;

        while k < carry.len() {
            if carry[k] == BEL {
                // BEL-terminated OSC.
                let osc = &carry[content_start..k];
                if osc.starts_with(OSC_SHELL_INTEGRATION) {
                    emit_event(&osc[OSC_SHELL_INTEGRATION.len()..], &mut callback);
                }
                pos = k + 1;
                continue 'outer;
            }

            if carry[k] == ESC {
                if k + 1 < carry.len() {
                    if carry[k + 1] == ST_SECOND {
                        // ST-terminated OSC.
                        let osc = &carry[content_start..k];
                        if osc.starts_with(OSC_SHELL_INTEGRATION) {
                            emit_event(&osc[OSC_SHELL_INTEGRATION.len()..], &mut callback);
                        }
                        pos = k + 2;
                        continue 'outer;
                    }
                    // ESC followed by something other than \ — not ST;
                    // will be picked up as a new potential sequence start later.
                } else {
                    // Incomplete potential ST; wait for more data.
                    break;
                }
            }

            k += 1;
        }

        // No terminator found yet — incomplete sequence; stop scanning.
        break;
    }

    // Remove everything we have fully processed.
    if pos > 0 {
        carry.drain(..pos);
    }

    // Safety valve: discard old carry data to prevent unbounded growth.
    if carry.len() > MAX_CARRY {
        carry.drain(..carry.len() - MAX_CARRY);
    }
}

/// Try to extract an `aid=<value>` parameter from a slice of semicolon-separated
/// parameter segments. Returns the parsed `u64` if found.
/// TODO: switch to string aid
fn parse_aid(params: &[&[u8]]) -> Option<u64> {
    for param in params {
        if let Some(aid_val) = param.strip_prefix(b"aid=") {
            return std::str::from_utf8(aid_val)
                .ok()
                .and_then(|s| s.parse().ok());
        }
    }
    None
}

/// Translate the parameter portion of an OSC 133 sequence into a [`ShellEvent`]
/// and invoke `callback`.
///
/// The parameter format is `<marker>[;<param>...]` where `<param>` is either
/// an exit code (for `D`) or a `key=value` pair like `aid=<id>`.
fn emit_event(param: &[u8], callback: &mut impl FnMut(ShellEvent)) {
    let mut parts = param.split(|&b| b == b';');

    let marker = match parts.next() {
        Some(m) => m,
        None => return,
    };

    // Collect the remaining semicolon-separated segments.
    let remainder: Vec<&[u8]> = parts.collect();

    let aid = parse_aid(&remainder);

    match marker {
        MARKER_PROMPT => callback(ShellEvent::PromptStarted { aid }),
        MARKER_PROMPT_END => callback(ShellEvent::PromptEnded { aid }),
        MARKER_COMMAND => callback(ShellEvent::CommandStarted { aid }),
        MARKER_FINISHED => {
            // For D, the first non-aid parameter (if any) is the exit code.
            let exit_code = remainder.iter().find_map(|p| {
                if p.strip_prefix(b"aid=").is_some() {
                    None // skip aid= params
                } else {
                    std::str::from_utf8(p).ok().and_then(|s| s.parse().ok())
                }
            });
            callback(ShellEvent::CommandFinished { exit_code, aid });
        }
        _ => {} // unknown marker, ignore
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper that runs `scan` and collects all events into a Vec.
    fn scan_collect(carry: &mut Vec<u8>, chunk: &[u8]) -> Vec<ShellEvent> {
        let mut events = Vec::new();
        scan(carry, chunk, |e| events.push(e));
        events
    }

    #[test]
    fn single_bel_terminated_prompt() {
        let mut carry = Vec::new();
        let seq = b"\x1b]133;A\x07";
        let events = scan_collect(&mut carry, seq);
        assert_eq!(events, vec![ShellEvent::PromptStarted { aid: None }]);
        assert!(carry.is_empty());
    }

    #[test]
    fn single_st_terminated_command() {
        let mut carry = Vec::new();
        let seq = b"\x1b]133;C\x1b\\";
        let events = scan_collect(&mut carry, seq);
        assert_eq!(events, vec![ShellEvent::CommandStarted { aid: None }]);
        assert!(carry.is_empty());
    }

    #[test]
    fn finished_with_exit_code() {
        let mut carry = Vec::new();
        let seq = b"\x1b]133;D;0\x07";
        let events = scan_collect(&mut carry, seq);
        assert_eq!(
            events,
            vec![ShellEvent::CommandFinished {
                exit_code: Some(0),
                aid: None
            }]
        );
        assert!(carry.is_empty());
    }

    #[test]
    fn finished_without_exit_code() {
        let mut carry = Vec::new();
        let seq = b"\x1b]133;D\x07";
        let events = scan_collect(&mut carry, seq);
        assert_eq!(
            events,
            vec![ShellEvent::CommandFinished {
                exit_code: None,
                aid: None
            }]
        );
    }

    #[test]
    fn sequence_split_across_chunks() {
        let mut carry = Vec::new();
        let part1 = b"\x1b]133;";
        let part2 = b"A\x07";
        assert!(scan_collect(&mut carry, part1).is_empty());
        let events = scan_collect(&mut carry, part2);
        assert_eq!(events, vec![ShellEvent::PromptStarted { aid: None }]);
        assert!(carry.is_empty());
    }

    #[test]
    fn non_osc133_sequences_ignored() {
        let mut carry = Vec::new();
        let seq = b"\x1b]0;title\x07some text\x1b]133;A\x07";
        let events = scan_collect(&mut carry, seq);
        assert_eq!(events, vec![ShellEvent::PromptStarted { aid: None }]);
    }

    #[test]
    fn mixed_events_in_one_chunk() {
        let mut carry = Vec::new();
        let seq = b"\x1b]133;A\x07\x1b]133;C\x07\x1b]133;D;127\x07";
        let events = scan_collect(&mut carry, seq);
        assert_eq!(
            events,
            vec![
                ShellEvent::PromptStarted { aid: None },
                ShellEvent::CommandStarted { aid: None },
                ShellEvent::CommandFinished {
                    exit_code: Some(127),
                    aid: None
                },
            ]
        );
    }

    #[test]
    fn esc_at_chunk_boundary() {
        let mut carry = Vec::new();
        let part1 = b"hello\x1b";
        let part2 = b"]133;A\x07";
        assert!(scan_collect(&mut carry, part1).is_empty());
        let events = scan_collect(&mut carry, part2);
        assert_eq!(events, vec![ShellEvent::PromptStarted { aid: None }]);
    }

    #[test]
    fn max_carry_safety_valve() {
        let mut carry = Vec::new();
        // Feed 600 bytes of garbage (no valid OSC terminator).
        let garbage = vec![b'X'; 600];
        scan_collect(&mut carry, &garbage);
        assert!(carry.len() <= MAX_CARRY);
    }

    #[test]
    fn prompt_with_aid() {
        let mut carry = Vec::new();
        let seq = b"\x1b]133;A;aid=42\x07";
        let events = scan_collect(&mut carry, seq);
        assert_eq!(events, vec![ShellEvent::PromptStarted { aid: Some(42) }]);
    }

    #[test]
    fn command_with_aid() {
        let mut carry = Vec::new();
        let seq = b"\x1b]133;C;aid=7\x07";
        let events = scan_collect(&mut carry, seq);
        assert_eq!(events, vec![ShellEvent::CommandStarted { aid: Some(7) }]);
    }

    #[test]
    fn finished_with_exit_code_and_aid() {
        let mut carry = Vec::new();
        let seq = b"\x1b]133;D;0;aid=3\x07";
        let events = scan_collect(&mut carry, seq);
        assert_eq!(
            events,
            vec![ShellEvent::CommandFinished {
                exit_code: Some(0),
                aid: Some(3)
            }]
        );
    }

    #[test]
    fn finished_with_aid_only() {
        let mut carry = Vec::new();
        let seq = b"\x1b]133;D;aid=5\x07";
        let events = scan_collect(&mut carry, seq);
        assert_eq!(
            events,
            vec![ShellEvent::CommandFinished {
                exit_code: None,
                aid: Some(5)
            }]
        );
    }

    #[test]
    fn prompt_end_marker() {
        let mut carry = Vec::new();
        let seq = b"\x1b]133;B;aid=1\x07";
        let events = scan_collect(&mut carry, seq);
        assert_eq!(events, vec![ShellEvent::PromptEnded { aid: Some(1) }]);
    }

    #[test]
    fn full_command_cycle_with_aid() {
        let mut carry = Vec::new();
        let seq =
            b"\x1b]133;A;aid=1\x07\x1b]133;B;aid=1\x07\x1b]133;C;aid=1\x07\x1b]133;D;0;aid=1\x07";
        let events = scan_collect(&mut carry, seq);
        assert_eq!(
            events,
            vec![
                ShellEvent::PromptStarted { aid: Some(1) },
                ShellEvent::PromptEnded { aid: Some(1) },
                ShellEvent::CommandStarted { aid: Some(1) },
                ShellEvent::CommandFinished {
                    exit_code: Some(0),
                    aid: Some(1)
                },
            ]
        );
    }
}
