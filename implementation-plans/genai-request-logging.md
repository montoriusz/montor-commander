# Implementation Plan: Optional logging of `genai` requests and formatted messages

## Goals

1. Add **optional** logging that captures what we send to and receive from `genai`: the rendered/formatted messages (system prompt + each user/assistant turn) and the model's reply.
2. Logging is **off by default in release**, and **on by default in dev mode** (`pnpm tauri dev` / debug builds), with an env-var override so it can be toggled without recompiling.
3. Keep all `genai` concerns inside `src-tauri/src/chat/generation.rs`; only the subscriber setup lives in the app entry point.

Scope is the Rust/`genai` side only. No frontend or terminal-interaction changes.

## Current state (relevant facts)

- All generation lives in `src-tauri/src/chat/generation.rs`. `generate_assistant_reply` builds a `ChatRequest` via `build_history`, calls `client.exec_chat(MODEL, req, Some(&options))`, and parses `response.first_text()` into `AssistantOutput`.
- `build_history` renders the system prompt and each user turn with Askama templates and appends them to the `ChatRequest`. `ChatRequest` derives `Serialize`, so the whole formatted prompt can be serialized.
- There is **no logging infrastructure today**: no `tracing` subscriber, no `tauri-plugin-log`, no `println!`/`eprintln!` in `src-tauri`.
- `genai 0.6.5` depends on `tracing ^0.1` and emits its own events internally, but they are dropped because no subscriber is registered.
- `genai` also offers `ChatOptions::with_capture_raw_body(true)`, which populates `ChatResponse.captured_raw_body: Option<serde_json::Value>` with the raw provider HTTP response body.
- `main.rs` already loads `.env` via `dotenvy::dotenv().ok()` before `app_lib::run()`, so any env var we introduce is available from `.env`.

---

## Design decisions

| Topic | Decision |
| ----- | -------- |
| Mechanism | Use `tracing` (already in the dependency tree) + a `tracing-subscriber` installed once at startup. Our own `tracing::debug!`/`trace!` calls in `generation.rs` flow through the same subscriber, and `genai`'s internal events become visible too. |
| Default behavior | Enabled in debug builds (`cfg!(debug_assertions)`, true under `pnpm tauri dev`), disabled in release builds. |
| Override | `RUST_LOG` env var (read from `.env`). If set, it always wins over the default. This lets users force logging on in release or off in dev. |
| What we log | (a) the serialized `ChatRequest` (system + all formatted turns), (b) the model name, (c) the raw response text and parsed `AssistantOutput`. Optionally the raw provider body behind a higher verbosity level. |
| Log target | A dedicated target/module path (`app_lib::chat::generation`) so filtering is easy, e.g. `RUST_LOG=app_lib::chat::generation=debug`. |
| Raw body capture | Gate `with_capture_raw_body(true)` on whether `trace`-level logging for our target is enabled, to avoid extra allocation when logging is off. |

> Rationale for `tracing` over plain `println!`: it composes with `genai`'s internal instrumentation, supports per-module level filtering via `RUST_LOG`, and is the idiomatic choice for an async Tokio/Tauri backend.

---

## Phase 1 — Dependencies

In `src-tauri/Cargo.toml`, add:

```toml
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }
```

- `env-filter` enables `EnvFilter` (`RUST_LOG` parsing).
- `tracing` itself is already transitively present, but add it as a direct dependency since we call its macros.

> Alternative considered: `tauri-plugin-log`. Rejected for now to keep the change minimal and avoid a frontend-facing log channel we don't need; revisit if logs should surface in the webview.

---

## Phase 2 — Subscriber setup at startup

Add a small, self-contained initializer and call it once at the start of `run()` in `src-tauri/src/lib.rs` (before building the Tauri app).

```rust
fn init_logging() {
    use tracing_subscriber::{fmt, EnvFilter};

    // Default level: verbose for our generation module in debug builds, off otherwise.
    let default_directive = if cfg!(debug_assertions) {
        "app_lib::chat::generation=debug"
    } else {
        "off"
    };

    // RUST_LOG always wins when present; otherwise fall back to the default.
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(default_directive));

    // try_init so repeated init (e.g. tests) does not panic.
    let _ = fmt().with_env_filter(filter).try_init();
}
```

- Call `init_logging();` as the first line of `run()`.
- Use `try_init()` (not `init()`) so it is a no-op if a subscriber is already set (important for unit tests that may also initialize logging).

> Note: `main.rs` calls `dotenvy::dotenv().ok()` before `run()`, so `RUST_LOG` defined in `.env` is honored.

---

## Phase 3 — Emit logs in `generation.rs`

Add `tracing` calls around the request/response in `generate_assistant_reply` (and optionally in `build_history`). All log statements use the module's default target (`app_lib::chat::generation`).

1. **Request / formatted messages** — after `build_history(store)?`:

   ```rust
   if tracing::enabled!(tracing::Level::DEBUG) {
       match serde_json::to_string_pretty(&req) {
           Ok(json) => tracing::debug!(model = MODEL, request = %json, "genai request"),
           Err(e) => tracing::debug!(model = MODEL, error = %e, "failed to serialize genai request for logging"),
       }
   }
   ```

   The serialized `ChatRequest` includes the rendered system prompt and every formatted user/assistant turn.

2. **Raw-body capture (optional, trace level)** — build options conditionally:

   ```rust
   let mut options = ChatOptions::default().with_response_format(response_format());
   if tracing::enabled!(tracing::Level::TRACE) {
       options = options.with_capture_raw_body(true);
   }
   ```

3. **Response** — after `exec_chat`:

   ```rust
   tracing::debug!(raw = %raw, "genai raw reply text");
   if let Some(body) = &response.captured_raw_body {
       tracing::trace!(raw_body = %body, "genai raw provider body");
   }
   ```

   And after parsing, optionally:

   ```rust
   tracing::debug!(msg = %parsed.msg, commandline = %parsed.commandline, "parsed assistant output");
   ```

- Keep these as `debug!`/`trace!` so they are silent unless the filter enables them.
- Do not log credentials; `ChatRequest` does not contain the API key (it comes from the env/client), so serializing the request is safe.

---

## Phase 4 — Imports / cleanup

- Add `use tracing;` usage via fully-qualified macros (no extra `use` strictly needed for `tracing::debug!`).
- Ensure `init_logging` is private to `lib.rs`.
- Confirm `response` is bound as a value we can read `captured_raw_body` from (it already is: `let response = client.exec_chat(...)`).

---

## Phase 5 — Verification

1. `cd src-tauri && cargo check` — compiles with new deps.
2. `cd src-tauri && cargo test` — existing `generation.rs` tests still pass (subscriber init is a no-op via `try_init`).
3. Manual dev run: `pnpm tauri dev`, send a chat message, confirm the formatted request and reply appear in the terminal output.
4. Override checks:
   - `RUST_LOG=off pnpm tauri dev` → no genai logs (override disables the dev default).
   - In a release build, set `RUST_LOG=app_lib::chat::generation=debug` → logs appear (override enables in release).
   - `RUST_LOG=app_lib::chat::generation=trace` → raw provider body is also logged.

---

## Out of scope / follow-ups

- Routing logs to a file or to the webview (would use `tauri-plugin-log`).
- Persisting request/response transcripts to disk for replay.
- Redaction beyond "don't log secrets" (none currently needed since the request carries no credentials).
