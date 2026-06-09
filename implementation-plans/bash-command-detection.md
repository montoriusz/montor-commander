# Implementation Plan: Command Completion Detection via OSC 133 Shell Integration

## Goal

Detect when commands finish executing in the bash PTY (`src-tauri/src/lib.rs`),
including capturing **exit codes**, by adopting the OSC 133 shell-integration
protocol. Design the byte-stream parser generically so REPL support (Python via
`PYTHONSTARTUP`, Node via a bootstrap script) can be layered in later **without
changing the parser**.

## Background / Approach

A PTY only carries raw bytes; the shell does not natively announce command
boundaries. We instrument bash to emit invisible OSC 133 escape sequences around
prompts and commands, then parse those markers out of the PTY output stream in
the Rust backend and forward structured events to the frontend.

OSC 133 markers we care about:

| Marker | Sequence | Meaning |
| --- | --- | --- |
| A | `ESC ] 133 ; A ST` | Prompt start |
| B | `ESC ] 133 ; B ST` | Prompt end / command input start |
| C | `ESC ] 133 ; C ST` | Command output start (command began executing) |
| D | `ESC ] 133 ; D ; <exit_code> ST` | Command finished, with exit status |

`ESC` = `0x1b`. `ST` (string terminator) may be BEL (`0x07`) or `ESC \`
(`0x1b 0x5c`). The parser must accept **both** terminators.

## Architecture Changes

### Current model (to be changed)
- Frontend polls `read_from_pty` on every animation frame and writes raw bytes
  to xterm.js.
- No backend awareness of command boundaries.

### Target model
- A **backend reader thread** owns the PTY reader. It:
  1. Scans the byte stream for OSC 133 markers.
  2. Emits the raw bytes to the frontend for display (so the markers, which are
     invisible, are stripped or passed through — see "Marker handling" below).
  3. Emits structured Tauri events (`command-started`, `command-finished`) to the
     frontend.
- Frontend subscribes to events instead of (or in addition to) polling.

> Migration note: keeping `read_from_pty` working during the transition is fine,
> but the reader thread and the polling loop must not both consume the same
> reader. Pick one consumer. Recommended: move fully to the push-based event
> model and delete the polling loop.

## Detailed Steps

### 1. Inject OSC 133 integration into bash

Do **not** edit the user's `~/.bashrc`. Instead launch bash with a custom rcfile
that first sources the user's normal startup, then appends the integration hooks.

**Option A (recommended): bundled rcfile written to a temp path at startup.**

Write a file (e.g. to `std::env::temp_dir()`) containing:

```bash
# Source the user's normal interactive config first
if [ -f ~/.bashrc ]; then . ~/.bashrc; fi

__osc133_prompt_start() { printf '\001\033]133;A\007\002'; }
__osc133_cmd_start()    { printf '\033]133;B\007'; }
__osc133_cmd_done()     { printf '\033]133;C;%s\007' "$?"; }

# Mark prompt start (A) and emit D for the *previous* command's exit code.
# PROMPT_COMMAND runs right before PS1 is shown.
PROMPT_COMMAND='__osc133_cmd_done; '"$PROMPT_COMMAND"
PS0='$(__osc133_cmd_start)'"$PS0"          # PS0 is printed after reading a command, before execution -> C marker
PS1="$PS1"'\[$(__osc133_prompt_start)\]'
```

Notes:
- `PS0` (bash >= 4.4) is emitted **after** the command line is accepted but
  **before** execution — perfect for the `C` (output-start) marker.
- `PROMPT_COMMAND` runs before the prompt; capture `$?` there for the `D` marker.
  Order matters: capture `$?` as the very first thing so other PROMPT_COMMAND
  entries don't clobber it.
- `\001`/`\002` (and `\[`/`\]` in PS1) tell bash these bytes are non-printing so
  line-wrapping math stays correct.

Spawn with:
```rust
let mut cmd = CommandBuilder::new("bash");
cmd.arg("--rcfile");
cmd.arg(rcfile_path);
cmd.arg("-i");
cmd.env("TERM", "xterm-256color");
```

**Option B (fallback): write the setup commands to the PTY** immediately after
spawn. Simpler but pollutes scrollback briefly and is racier. Prefer Option A.

### 2. Implement the OSC 133 parser (Rust)

Create a new module, e.g. `src-tauri/src/osc133.rs`.

Requirements:
- Operates on a streaming byte feed (bytes arrive in arbitrary chunks; a marker
  may be split across two reads). Maintain a small carry-over buffer for a
  partial escape sequence at the end of a chunk.
- Recognizes `ESC ] 133 ; <payload> ST` where ST is BEL or `ESC \`.
- Parses payload into an enum:

```rust
pub enum ShellEvent {
    PromptStart,                 // A
    CommandStart,                // B
    CommandFinished { exit_code: i32 }, // C;<code>
}
```

- Returns both: (a) the cleaned byte stream to forward to the terminal, and
  (b) any `ShellEvent`s detected. Decide on marker handling:
  - **Strip** the 133 markers from the forwarded stream (recommended — they are
    control-only and xterm.js doesn't need them), OR
  - **Pass through** (xterm.js ignores unknown OSC). Stripping is cleaner and
    avoids edge cases.
- Be defensive: malformed/partial `C` payloads (missing or non-numeric exit code)
  should not panic; default to `exit_code: -1` or skip.

Add unit tests covering: full marker in one chunk, marker split across chunks,
both ST terminators, the `C` exit-code parse, and interleaving with normal text.

### 3. Reader thread + event emission (Rust, `lib.rs`)

- In `run()`, after creating the PTY, capture an `AppHandle` (via
  `tauri::Builder::setup`) so the reader thread can emit events.
- Spawn a dedicated thread that loops reading from the master reader, feeds bytes
  through the OSC 133 parser, then:
  - `app.emit("pty-output", cleaned_bytes_as_string)` for display, and
  - `app.emit("command-finished", payload)` / `command-started` for markers.
- Define serializable payload structs (`serde::Serialize`), e.g.:

```rust
#[derive(Clone, serde::Serialize)]
struct CommandFinishedPayload { exit_code: i32 }
```

- Remove the `reader` from `AppState` if the thread owns it exclusively, or wrap
  appropriately. Avoid two consumers of the same reader.
- Keep `write_to_pty`, `resize_pty`, `create_shell` largely as-is. `create_shell`
  changes to spawn bash with the `--rcfile` argument (step 1).

### 4. Frontend changes (`src/main.ts`)

- Import `listen` from `@tauri-apps/api/event`.
- Replace the `requestAnimationFrame(readFromPty)` polling loop with:

```ts
import { listen } from '@tauri-apps/api/event';

await listen<string>('pty-output', (e) => { void writeToTerminal(e.payload); });
await listen<{ exit_code: number }>('command-finished', (e) => {
  // hook point: update prompt status, show exit code, etc.
  console.debug('command finished', e.payload.exit_code);
});
```

- Remove `read_from_pty` invocation. (Backend command can be deleted too, or kept
  as a no-op during migration.)
- Ensure listeners are registered **before** `initShell()` so no early output is
  missed (or buffer in backend until a "frontend ready" signal — simplest is to
  register listeners first, then call `create_shell`).

### 5. Capabilities / permissions

- Emitting events from Rust to the frontend uses `core:event` permissions.
  Verify `src-tauri/capabilities/default.json` allows event listening; add
  `"core:event:default"` to the `permissions` array if event delivery is blocked.

## Edge Cases & Considerations

- **Bash builtins** (`cd`, `export`) still trigger PS0/PROMPT_COMMAND, so they are
  reported correctly — an advantage of the prompt-hook approach over process-tree
  watching.
- **Multi-line commands / heredocs**: PS0 fires once when the full command is
  submitted; PROMPT_COMMAND fires once when the prompt returns. Correct by design.
- **Programs that take over the terminal** (vim, less, REPLs): bash's hooks go
  silent until the program exits, then a single `D` reports the program's exit
  code. Per-command tracking inside those programs is out of scope (see REPL
  section).
- **Markers split across read chunks**: handled by the parser's carry-over buffer.
- **Exit code of `-1`/unknown**: define a sentinel and document it.
- **PS0 requires bash >= 4.4.** macOS ships bash 3.2; since we target Linux this
  is fine, but note it. (If macOS support is later needed, fall back to a
  `trap DEBUG`-based `C` marker.)

## Future: REPL Support (design for it now, implement later)

Keep the parser **program-agnostic**: it detects OSC 133 markers regardless of
who emits them. To add REPL support later, only the *injection* side changes:

- **Python** (`python`): inject via `PYTHONSTARTUP` pointing at a script that sets
  `sys.ps1`/`sys.ps2` to objects whose `__str__` emits A/B markers, and wraps
  `sys.displayhook` / `sys.excepthook` to emit `D;0` (success) or `D;1` (raised).
  REPLs have no real exit code; map success/exception to 0/1.
- **Node** (`node`): launch with a bootstrap (`node -r ./repl-integration.js`)
  that calls `repl.start(...)` and monkey-patches `r.eval` to emit `C` before and
  `D;<0|1>` after each evaluation.
- Because the backend parser is unchanged, REPL markers flow through the existing
  `command-started` / `command-finished` events automatically.

Decisions already made for the future work:
- REPL support is a separate, opt-in tier.
- "Succeeded vs. raised" is an acceptable stand-in for an exit code in REPLs.

## Acceptance Criteria

1. Running `ls`, `false`, `sleep 1; echo done` in the app emits a
   `command-finished` event with the correct exit code (0, 1, 0 respectively).
2. The OSC 133 markers are **not** visible in the terminal output.
3. No double-consumption of the PTY reader; output displays correctly via the
   event stream.
4. Parser unit tests pass, including the split-chunk and dual-terminator cases.
5. The user's `~/.bashrc` is sourced and unmodified on disk.

## Files Touched

- `src-tauri/src/lib.rs` — reader thread, event emission, `create_shell` rcfile.
- `src-tauri/src/osc133.rs` — **new** parser module (+ `mod osc133;`).
- `src-tauri/src/...` — rcfile contents (embed via `include_str!` from a
  `src-tauri/assets/bash-integration.sh` file, written to temp at runtime).
- `src/main.ts` — switch from polling to `listen`-based events.
- `src-tauri/capabilities/default.json` — event permission if needed.
- `src-tauri/Cargo.toml` — no new deps expected (serde/tauri already present).
`
