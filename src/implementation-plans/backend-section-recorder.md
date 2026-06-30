# Backend Section Recorder

Move terminal capture ownership to the backend. Record raw PTY output in
per-prompt-command sections so they can be (1) re-rendered into a separate
xterm instance and (2) parsed on the backend into text for the LLM — replacing
the current frontend-extracted `payload.terminal`.

## Motivation

Today:

- The PTY reader thread (`src-tauri/src/terminal.rs`) splits the byte stream on
  OSC133 boundaries via `osc133::scan`, forwards bytes to the frontend, and
  discards the section structure.
- The frontend xterm buffer is the only place section text exists. At send time
  `src/app/chat/chat-store.ts` reads the live buffer through
  `TerminalSections.getSectionShapshots` / `readFragment` and ships it as
  `ChatUserMessagePayload.terminal`.

Both requested changes collapse to a single root change: **the backend records
raw output per section.** That one artifact (raw bytes + intra-section marker
offsets) serves both re-render and LLM parsing.

## Design

### Source of truth: `RecordedSection` (raw bytes + offsets)

New module `src-tauri/src/recorder.rs`:

```rust
pub struct RecordedSection {
    pub aid: String,
    pub raw: Vec<u8>,          // every raw byte attributed to this section, in order
    pub off_prompt_end: Option<usize>,   // byte offset of OSC133 B within `raw`
    pub off_command_start: Option<usize>,// byte offset of OSC133 C within `raw`
    pub off_command_finish: Option<usize>,// byte offset of OSC133 D within `raw`
    pub exit_code: Option<i32>,
    pub cols: u16,             // terminal width when captured (for deterministic re-parse)
    pub rows: u16,
}

pub struct SessionRecorder {
    sections: Vec<RecordedSection>,         // ordered
    by_aid: HashMap<String, usize>,         // aid -> index into `sections`
    current_aid: Option<String>,
    cols: u16,
    rows: u16,
}
```

Feeding (in the reader thread, alongside the existing channel send): for each
`Segment` from `osc133::scan`,

- `Segment::Passthrough(bytes)` → append to the current section's `raw`.
- `Segment::Sequence { bytes, event }` →
  - `PromptStarted { aid }` → start/select section for `aid` (create if new),
    make it current, then append `bytes` to it.
  - `PromptEnded` → record `off_prompt_end = raw.len()` (before append), append.
  - `CommandStarted` → record `off_command_start`, append.
  - `CommandFinished { exit_code }` → record `off_command_finish` + exit_code,
    append.
  - Events carrying `Some(aid)` update `current_aid` first (mirrors the
    frontend `registerMarkingPoint` "lastAid" behaviour in
    `terminal-sections.ts`).

`resize_pty` updates `recorder.cols/rows`; new sections inherit current size.
(Width changes mid-section are rare; capturing per-section is good enough and
matches today's behaviour where the live buffer reflows.)

The recorder is shared state. Two options:

- A) Hold `Arc<Mutex<SessionRecorder>>` inside `TerminalSession`, fed by the
  reader thread, read by chat commands via `State<TerminalSession>`.
- B) Manage a separate `State<SessionRecorder>`.

Go with **A** — the reader thread and the recorder already share a lifetime, and
it keeps the wiring local to `terminal.rs`.

### Change 1 — re-render per section

Expose a command:

```rust
#[tauri::command]
fn read_section_raw(aid: String, state: State<TerminalSession>) -> Result<String, String>
```

returns the section's `raw` (lossy UTF-8, same convention as `TerminalEvent::Output`).
The frontend writes it into a **separate** `Terminal` instance
(`terminal.write(raw)`) to reproduce the visual section. Optionally add
`read_sections_raw(from_aid, to_aid)` for ranges.

Re-render uses raw PTY bytes (not xterm's serialized buffer) so it stays
backend-owned and is the same artifact the LLM parser consumes.

### Change 2 — backend parse for the LLM

Add the `vt100` crate. At send time the backend builds the `terminal` markup by
parsing each relevant section's raw byte ranges through a fresh `vt100::Parser`
sized to that section's `cols`:

- prompt text  = parse `raw[0 .. off_prompt_end]`
- command text = parse `raw[off_prompt_end .. off_command_start]`
- output text  = parse `raw[off_command_start .. off_command_finish]`

Parsing each fragment in isolation avoids needing cursor coordinates (the
frontend `readFragment` needed buffer markers; we don't). Extract text from the
resulting screen (+ generous scrollback for long output). This produces the same
`<prompt>/<command>/<output>` markup that `formatTerminalSections` builds today —
move that formatting into Rust (`recorder.rs` or a helper in `chat.rs`).

Section range resolution moves server-side: the backend finds the previous
user message's `term_sect` in the store (the current "previousMarker" logic in
`chat-store.ts`) and asks the recorder for sections from there to `current_aid`.

### Snapshot semantics

`send_chat_message` computes the markup from the recorder **at send time** and
stores it in `ChatMessage::User.terminal` exactly as today. History stays an
immutable snapshot; `generation.rs::build_history` is unchanged. We are only
changing *who* produces the snapshot (backend, not frontend).

## Touch points

Backend:

- `src-tauri/Cargo.toml` — add `vt100`.
- `src-tauri/src/recorder.rs` — new: `SessionRecorder`, feed logic, fragment
  parsing, markup builder.
- `src-tauri/src/terminal.rs` — hold recorder in `TerminalSession`; feed it in
  `spawn_reader_thread`; update `cols/rows` in `resize_pty`; add
  `read_section_raw`.
- `src-tauri/src/chat.rs` — `ChatUserMessagePayload` shrinks to `{ msg }`;
  `send_chat_message` takes `State<TerminalSession>`, derives `terminal`,
  `current_sect`, `cmdline` from the recorder; `append_user` updated.
- `src-tauri/src/lib.rs` — register `read_section_raw`.
- `src-tauri/src/osc133.rs` — unchanged (already provides everything).

Frontend:

- `src/app/chat/chat-store.ts` — `send` sends only `{ msg }`; drop
  `formatTerminalSections` and section extraction.
- `src/app/terminal/terminal-sections.ts` — keep marker tracking + decorations;
  remove text-extraction (`getSectionShapshots`, `readFragment`) if unused
  elsewhere (verify usages first).
- New separate xterm instance/component for re-rendering a stored section (wired
  to `read_section_raw`). Scope TBD with UI requirements.
- `pnpm tauri-typegen` to regenerate bindings after the payload/command changes.

## Phasing

1. Recorder + feed in reader thread; `read_section_raw`; manual re-render check.
2. Backend markup builder + `vt100`; switch `send_chat_message` to recorder;
   shrink payload; regen bindings; update `chat-store.ts`.
3. Frontend re-render UI for stored sections.
4. (Optional) Persist sections to disk under `chat-{id}/` for restore across
   restarts (mirror `jsonl_store`).

## Open decisions

- **VT parser**: `vt100` (screen+scrollback, simplest extraction) vs `vte`
  (lower-level, more work) vs `alacritty_terminal` (heavier). Plan assumes
  `vt100`.
- **Persistence**: in-memory MVP (phase 1-3) vs persisted now (phase 4).
- **Fragment-isolation parsing**: parse A→B / B→C / C→D independently
  (no cursor coords) vs single emulator with coordinate capture. Plan assumes
  isolation.
- **Payload reduction**: send only `{ msg }` (backend owns range/cmdline) vs
  keep `currentSect` from the frontend.
