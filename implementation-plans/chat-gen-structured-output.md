# Implementation Plan: Extract chat generation into its own module with structured output

## Goals

1. Move all genai/generation logic out of `src-tauri/src/chat.rs` into a dedicated module.
2. Switch genai execution from free-text to **structured output** that yields `{ msg, commandline }` for `ChatMessage::Assistant`.
3. Add Askama templates for the **system prompt** (Terminal Assistant for Bash) and for rendering a **user turn** (`terminal` + `commandline` + `msg`).

Scope is the Rust/genai side only. No frontend or terminal-interaction changes.

## Current state (relevant facts)

- `chat.rs` mixes three concerns: the `ChatMessage` model + JSONL persistence, the Tauri commands, and the genai generation (`MODEL`, `build_history`, `exec_chat`, assistant message construction).
- `askama = "0.16.0"` is already in `Cargo.toml` but unused; no `templates/` dir or `askama.toml` exists yet.
- genai `0.6.5` supports structured output via `ChatOptions::with_response_format(ChatResponseFormat::JsonSpec(JsonSpec::new(name, schema)))`; the model's reply is returned as a JSON string from `response.first_text()`.
- `ChatMessage::Assistant` already has a `commandline: Option<String>` field, currently always written as `None`.

---

## Phase 1 — Module extraction

Rust allows `chat.rs` to coexist with a `chat/` directory holding its submodules. Use that to avoid renaming the existing module path (`crate::chat`).

### New files

- `src-tauri/src/chat/generation.rs` — all generation logic.
- (Templates live under `src-tauri/templates/`, see Phase 3.)

### `chat.rs` changes

- Add `mod generation;` near the top.
- **Move out** of `chat.rs` into `generation.rs`:
  - `const MODEL`.
  - `fn build_history` (renamed/refactored — see Phase 2 & 4).
  - The genai call block currently inside the spawned task in `send_chat_message` (the `client.exec_chat(...)` → parse → build `ChatMessage::Assistant` → `store.write` portion).
- **Keep** in `chat.rs`:
  - `ChatMessage` enum + `set_id`.
  - All Tauri command payload structs, event payload structs, and `emit_*` helpers.
  - `ChatSession`, `append_user`, `read_page`.
  - The Tauri commands (`get_chat_session`, `read_chat_messages`, `send_chat_message`).
  - `now_timestamp` (or move to a shared spot if `generation.rs` needs it — prefer keeping it in `chat.rs` and passing the timestamp in, or expose it `pub(crate)`).

### Public surface of `generation.rs`

Expose a single entry point the spawned task calls, e.g.:

```rust
pub(crate) async fn generate_assistant_reply(
    client: &genai::Client,
    store: &JsonlStore<ChatMessage>,
) -> Result<u32, String>;
```

It internally: builds the request (system + history), calls genai with structured output, parses the result, constructs `ChatMessage::Assistant`, writes it to the store, and returns the new message id. The spawned task in `send_chat_message` then only handles `Ok(id) => emit_messages_changed` / `Err(e) => emit_generation_error` and the `generating` flag. This keeps Tauri/event concerns in `chat.rs` and generation concerns in `generation.rs`.

> The `// TODO: fix` comment in the current spawned task should be resolved as part of this move (the store is reopened per task; keep that behavior but route through `generate_assistant_reply`).

---

## Phase 2 — Structured output

### Output type

In `generation.rs`:

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AssistantOutput {
    /// Natural-language reply shown in the chat.
    message: String,
    /// Suggested commandline to replace the user's commandline. Omitted when no suggestion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    commandline: Option<String>,
}
```

### JSON schema + request options

Build a `JsonSpec` describing the same shape and attach it via `ChatOptions`:

```rust
fn response_format() -> ChatResponseFormat {
    let spec = JsonSpec::new(
        "terminal_assistant_reply",
        serde_json::json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "Assistant reply to the user."
                },
                "commandline": {
                    "type": ["string", "null"],
                    "description": "Suggested bash commandline to replace the user's current commandline, or null."
                }
            },
            "required": ["msg"]
        }),
    );
    ChatResponseFormat::JsonSpec(spec)
}
```

- Name uses only `_`/`-` (OpenAI constraint, harmless for Gemini).
- Consider `spec.schema_with_additional_properties_false()` if a stricter provider is ever used; not required for the current Gemini model but cheap to add.

### Call site

```rust
let options = ChatOptions::default().with_response_format(response_format());
let response = client.exec_chat(MODEL, req, Some(&options)).await.map_err(|e| e.to_string())?;
let raw = response.first_text().unwrap_or("{}");
let parsed: AssistantOutput = serde_json::from_str(raw)
    .map_err(|e| format!("failed to parse assistant output: {e}; raw: {raw}"))?;

let message = ChatMessage::Assistant {
    id: String::new(),
    commandline: parsed.commandline,
    msg: parsed.msg,
    ts: now_timestamp(),
    model: MODEL.to_string(),
};
```

(Confirm the exact `ChatOptions`/`with_response_format` signature against `genai 0.6.5` during implementation; the third arg of `exec_chat` is `Option<&ChatOptions>`.)

---

## Phase 3 — Askama templates

### Setup

- Create `src-tauri/templates/`.
- Askama's default search path is `templates/` relative to the crate root (`src-tauri/`), so no `askama.toml` is strictly required. Add `askama.toml` only if a custom path is desired.

### 3a. System prompt — `templates/system_prompt.txt`

Static (no template vars needed, but keep it a template for consistency and easy interpolation later). Content should:

- Establish role: an assistant embedded in a Bash terminal app that helps the user run and understand shell commands.
- Describe the **expected user input format** so the model knows how to read each user turn:
  - A `<terminal>…</terminal>` block containing a root-less, unescaped XML-like stream of:
    - `<prompt>…</prompt>` — the bash prompt.
    - `<command>…</command>` — a command the user ran.
    - `<output>…</output>` — terminal output captured after that command.
  - A `<commandline>…</commandline>` block with the user's current, not-yet-executed commandline (may be empty).
  - An optional `<user_message>…</user_message>` block with the user's chat message.
- Explain the **expected output contract**: respond with structured JSON `{ "msg": string, "commandline"?: string }`.
  - `msg` is the conversational reply.
  - `commandline`, when present, is a single suggested bash commandline intended to replace the user's current commandline; omit it when no command suggestion is appropriate.
- Note: the terminal stream is informational context; tags are not escaped and may contain arbitrary content.

Rust binding:

```rust
#[derive(Template)]
#[template(path = "system_prompt.txt")]
struct SystemPromptTemplate;
```

### 3b. User turn — `templates/user_turn.txt`

Renders a single user message into the documented format. Inputs map from `ChatMessage::User`:

```rust
#[derive(Template)]
#[template(path = "user_turn.txt")]
struct UserTurnTemplate<'a> {
    terminal: Option<&'a str>,
    commandline: Option<&'a str>,
    msg: &'a str,
}
```

Template body (whitespace-trimmed via Askama `{%- -%}` where helpful):

```
{% if let Some(terminal) = terminal %}<terminal>{{ terminal|safe }}</terminal>
{% endif %}<commandline>{{ commandline.unwrap_or("")|safe }}</commandline>
{% if !msg.is_empty() %}<user_message>{{ msg|safe }}</user_message>{% endif %}
```

- Use the `safe` filter (or disable escaping) because the terminal content is a raw, unescaped XML-like stream that must be quoted verbatim. Askama HTML-escapes by default for `.html` templates; using a `.txt` extension avoids HTML auto-escaping, but still apply `safe`/verify the configured escaper to guarantee no escaping.
- `terminal` and `msg` are optional/empty-skippable, mirroring the serde `skip_serializing_if` on the model.

> Note `ChatMessage::User` also has `terminal_marker`; it is bookkeeping only and is **not** rendered into the prompt.

---

## Phase 4 — Rewire `build_history`

Move into `generation.rs` and update:

1. Prepend a system message rendered from `SystemPromptTemplate`:
   ```rust
   let system = SystemPromptTemplate.render().map_err(|e| e.to_string())?;
   let mut req = ChatRequest::new(vec![]).with_system(system);
   ```
   (Or `GenaiChatMessage::system(...)` as the first appended message — pick whichever the genai version exposes cleanly.)
2. For each stored `ChatMessage::User`, render `UserTurnTemplate` and append as `GenaiChatMessage::user(rendered)`.
3. For each stored `ChatMessage::Assistant`, append `GenaiChatMessage::assistant(...)`. Decide what text to send back as the assistant's prior turn:
   - Simplest: serialize the stored `{ msg, commandline }` back to the same JSON shape used for structured output, so history stays consistent with the contract.
   - Alternative: send just `msg`. Recommend the JSON form for consistency; document the choice in code.

---

## Phase 5 — Imports / cleanup

- Add to `generation.rs`: `use askama::Template;`, `use genai::chat::{ChatOptions, ChatRequest, ChatResponseFormat, JsonSpec, ChatMessage as GenaiChatMessage};`, `use serde::{Deserialize, Serialize};`, plus `crate::jsonl_store::JsonlStore` and `crate::chat::ChatMessage` (or re-export within the module tree).
- Remove now-unused imports from `chat.rs` (`ChatRequest`, `GenaiChatMessage`, `MODEL` if fully moved).
- Ensure `now_timestamp` is reachable from `generation.rs` (`pub(crate)` or pass the value in).

---

## Validation

- `cd src-tauri && cargo check` — must pass.
- `cd src-tauri && cargo build` to confirm Askama templates compile (template compile errors surface here).
- Optional: a unit test in `generation.rs` that renders `UserTurnTemplate` with sample `terminal`/`commandline`/`msg` and asserts the produced string contains the verbatim, unescaped tags.
- Optional: a unit test that round-trips `AssistantOutput` through `serde_json` to confirm the schema/struct agree.
- `pnpm build` is unaffected (no frontend changes), but run `pnpm tauri-typegen` only if any `#[tauri::command]` signature changed — it does **not** in this plan, so typegen is not required.
