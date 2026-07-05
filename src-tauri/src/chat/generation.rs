use askama::Template;
use genai::chat::{
    ChatMessage as GenaiChatMessage, ChatOptions, ChatRequest, ChatResponseFormat, JsonSpec,
};
use serde::{Deserialize, Serialize};

use crate::chat::ChatMessage;
use crate::jsonl_store::JsonlStore;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const MODEL: &str = "gemini-3.1-flash-lite";

// ---------------------------------------------------------------------------
// Structured output type
// ---------------------------------------------------------------------------

/// Shape the LLM is asked to produce via structured-output JSON schema.
///
/// Both fields are always present in the contract. An empty string is the
/// sentinel for "nothing here": no reply text (`message`) or no command suggestion
/// (`commandline`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AssistantOutput {
    /// Natural-language reply shown in the chat. Empty when there is nothing to say.
    #[serde(default)]
    pub message: String,
    /// Suggested commandline to replace the user's commandline. Empty when no suggestion.
    #[serde(default)]
    pub commandline: String,
}

// ---------------------------------------------------------------------------
// Askama templates
// ---------------------------------------------------------------------------

/// The chat system prompt template.
///
/// The single `sysinfo` field carries the output of the per-shell
/// `*-sysinfo.sh` probe (run by [`crate::shell::Shell::sysinfo`] and cached on
/// the [`crate::shell::Shell`] owned by `ChatSession`) so the model can tailor
/// its command suggestions to the user's host and available tools. The probe's
/// `key: value` schema is identical across shells, so the template does not
/// branch on shell kind.
#[derive(Template)]
#[template(path = "system_prompt.md")]
struct SystemPromptTemplate<'a> {
    sysinfo: &'a str,
}

#[derive(Template)]
#[template(path = "user_turn.md")]
struct UserTurnTemplate<'a> {
    terminal: &'a str,
    message: &'a str,
}

// ---------------------------------------------------------------------------
// JSON schema for structured output
// ---------------------------------------------------------------------------

fn response_format() -> ChatResponseFormat {
    let spec = JsonSpec::new(
        "terminal_assistant_reply",
        serde_json::json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "Assistant reply to the user. Use an empty string when there is nothing to say."
                },
                "commandline": {
                    "type": "string",
                    "description": "Suggested shell commandline to replace the user's current commandline. Use an empty string when there is no command to suggest."
                }
            },
            "required": ["message", "commandline"],
            "additionalProperties": false
        }),
    );
    ChatResponseFormat::JsonSpec(spec)
}

// ---------------------------------------------------------------------------
// History builder
// ---------------------------------------------------------------------------

/// Build a `ChatRequest` from all messages in the store, rendering the system
/// prompt via Askama and each user turn via `UserTurnTemplate`.
///
/// `TerminalSection` messages (persisted by the PTY reader as commands finish)
/// are accumulated and attached to the next user turn's `<terminal>` block, so
/// each user turn sees the terminal activity since the previous one — matching
/// the system prompt's contract without any frontend extraction.
///
/// Assistant turns are replayed as the same JSON shape used for structured
/// output (`{ "msg": …, "commandline": … }`) to keep history consistent
/// with the output contract.
fn build_history(store: &JsonlStore<ChatMessage>, sysinfo: &str) -> Result<ChatRequest, String> {
    let system = SystemPromptTemplate { sysinfo }
        .render()
        .map_err(|e| format!("failed to render system prompt: {e}"))?;
    let mut req = ChatRequest::new(vec![]).with_system(system);

    let page = store.read(0, None).map_err(|e| e.to_string())?;

    // Accumulated `<prompt>/<commandline>/<output>` rendering of the terminal
    // sections seen since the previous user turn. This includes live
    // snapshots persisted by `send_chat_message` at send time. The command may:
    // - not been provided yet,
    // - not been executed yet (the user has typed the command but not yet pressed enter),
    // - executed but not finished yet - then the snapshot also carries the partial output
    //   captured so far)
    let mut terminal_buf = String::new();

    for msg in &page.items {
        match msg {
            ChatMessage::User { msg, .. } => {
                let rendered = UserTurnTemplate {
                    terminal: &terminal_buf,
                    message: msg,
                }
                .render()
                .map_err(|e| format!("failed to render user turn: {e}"))?;
                req = req.append_message(GenaiChatMessage::user(rendered));
                terminal_buf.clear();
            }
            ChatMessage::Assistant {
                msg,
                cmdline: commandline,
                ..
            } => {
                // Replay assistant turns as the same JSON shape the model
                // produces, so the history stays consistent with the contract.
                let prior = AssistantOutput {
                    message: msg.clone(),
                    commandline: commandline.clone(),
                };
                let content = serde_json::to_string(&prior)
                    .map_err(|e| format!("failed to serialize assistant history: {e}"))?;
                req = req.append_message(GenaiChatMessage::assistant(content));
            }
            ChatMessage::TerminalSection { .. } => {
                if !terminal_buf.is_empty() {
                    terminal_buf.push('\n');
                }
                terminal_buf.push_str(&render_section(msg));
            }
        }
    }

    Ok(req)
}

/// Render one [`ChatMessage::TerminalSection`] into the
/// `<prompt>/<commandline>/<output>` snippet expected inside a user turn's
/// `<terminal>` block (see `system_prompt.md`).
///
/// A single code path serves both persisted-finished sections (the reader
/// thread persisted them after OSC 133 `D`) and the live snapshot persisted by
/// `send_chat_message` at send time — both are `TerminalSection` records, so
/// there is no longer a `Live`/`Finished` distinction at the rendering layer.
/// The same `executed`/`exit_code` fields drive the markup in both cases: a
/// live snapshot is `executed=false, exit_code=None` (so its `<output>`, if any,
/// is rendered `finished="false"` with no `exit-code`), while a finished
/// section reflects what the recorder captured.
fn render_section(section: &ChatMessage) -> String {
    let ChatMessage::TerminalSection {
        prompt,
        cmdline: command,
        output,
        executed,
        exit_code,
        ..
    } = section
    else {
        return String::new();
    };
    let executed = *executed;
    let exit_code = *exit_code;

    let mut s = format!("<prompt>{prompt}</prompt>");
    if !command.is_empty() || executed {
        s.push_str(&format!(
            "<commandline executed=\"{executed}\">{command}</commandline>"
        ));
    }
    // `finished` is only `"true"` when the recorder captured an exit code
    // (then carried in `exit-code`). A live snapshot, or a section closed
    // without an exit code (e.g. terminated by a signal), is rendered
    // `finished="false"` and emits no `exit-code` attribute.
    if !output.is_empty() {
        let (finished, exit_attr) = match exit_code {
            Some(code) => ("true", format!(" exit-code=\"{code}\"")),
            None => ("false", String::new()),
        };
        s.push_str(&format!(
            "\nn<output finished=\"{finished}\"{exit_attr}>\n{output}\n</output>"
        ));
    }
    s
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Generate an assistant reply by:
/// 1. Building the request (system prompt + history) with structured output.
/// 2. Calling genai.
/// 3. Parsing the structured response into `AssistantOutput`.
/// 4. Writing the resulting `ChatMessage::Assistant` to the store.
///
/// Returns the new message ID on success.
pub(crate) async fn generate_assistant_reply(
    client: &genai::Client,
    store: &JsonlStore<ChatMessage>,
    now_ts: &str,
    sysinfo: &str,
) -> Result<u32, String> {
    let req = build_history(store, sysinfo)?;

    // Log the serialized `ChatRequest` (system prompt + all formatted turns) at
    // debug level. `ChatRequest` derives `Serialize` and carries no credentials
    // (the API key lives on the client), so serializing it is safe.
    if tracing::enabled!(tracing::Level::DEBUG) {
        match serde_json::to_string_pretty(&req) {
            Ok(json) => tracing::debug!(model = MODEL, request = %json, "genai request"),
            Err(e) => tracing::debug!(
                model = MODEL,
                error = %e,
                "failed to serialize genai request for logging"
            ),
        }
    }

    let options = ChatOptions::default().with_response_format(response_format());

    let response = client
        .exec_chat(MODEL, req, Some(&options))
        .await
        .map_err(|e| e.to_string())?;

    let raw = response.first_text().unwrap_or("{}");
    tracing::debug!(raw = %raw, "genai raw reply text");
    if let Some(body) = &response.captured_raw_body {
        tracing::trace!(raw_body = %body, "genai raw provider body");
    }

    let parsed: AssistantOutput = serde_json::from_str(raw)
        .map_err(|e| format!("failed to parse assistant output: {e}; raw: {raw}"))?;

    let message = ChatMessage::Assistant {
        id: String::new(),
        cmdline: parsed.commandline,
        msg: parsed.message,
        ts: now_ts.to_string(),
        model: MODEL.to_string(),
    };

    let id = store.write(message).map_err(|e| e.to_string())?;
    Ok(id)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_turn_template_renders_all_fields() {
        let rendered = UserTurnTemplate {
            terminal: "<prompt>$</prompt>\n<commandline>ls</commandline>\n<output>\nfile.txt\nfile2.txt\n</output>",
            message: "What does this file contain?",
        }
        .render()
        .unwrap();

        assert!(rendered.contains("<terminal>\n<prompt>$</prompt>\n<commandline>ls</commandline>\n<output>\nfile.txt\nfile2.txt\n</output>\n</terminal>"));
        assert!(rendered.contains("<user_message>What does this file contain?</user_message>"));
        // Content inside tags must be unescaped.
        assert!(rendered.contains("<prompt>$</prompt>"));
    }

    #[test]
    fn user_turn_template_renders_empty_terminal() {
        let rendered = UserTurnTemplate {
            terminal: "",
            message: "hello",
        }
        .render()
        .unwrap();

        assert!(rendered.contains("<terminal>\n</terminal>"));
        assert!(rendered.contains("<user_message>hello</user_message>"));
    }

    #[test]
    fn user_turn_template_empty_msg_skips_msg_tag() {
        let rendered = UserTurnTemplate {
            terminal: "",
            message: "",
        }
        .render()
        .unwrap();

        assert!(!rendered.contains("<user_message>"));
        assert!(rendered.contains("<terminal>\n</terminal>"));
    }

    #[test]
    fn render_section_live_without_command_emits_prompt_only() {
        let s = render_section(&ts_live_section("1", "$ ", ""));
        assert_eq!(s, "<prompt>$ </prompt>");
    }

    #[test]
    fn render_section_live_with_command_emits_command_as_not_executed() {
        let s = render_section(&ts_live_section("1", "user@h:~$ ", "ls -la"));
        assert_eq!(
            s,
            "<prompt>user@h:~$ </prompt><commandline executed=\"false\">ls -la</commandline>\n"
        );
    }

    #[test]
    fn render_section_live_with_partial_output_marks_output_unfinished() {
        // Snapshot sent while a long-running command is in progress: OSC 133 `C`
        // has fired (so `executed=true`), but no `D` has, so `exit_code=None`.
        // The output streamed so far is rendered with `finished="false"` and
        // no `exit-code`.
        let section = ts_section_full("1", "$ ", "./slow-binary", "building…", true, None);
        let s = render_section(&section);
        assert_eq!(
            s,
            "<prompt>$ </prompt><commandline executed=\"true\">./slow-binary</commandline>\n<output finished=\"false\">\nbuilding…\n</output>"
        );
    }

    #[test]
    fn render_section_finished_without_exit_code_marks_output_unfinished() {
        let s = render_section(&ts_section_full("1", "$ ", "ls", "a\nb", true, None));
        assert_eq!(
            s,
            "<prompt>$ </prompt><commandline executed=\"true\">ls</commandline>\n<output finished=\"false\">\na\nb\n</output>"
        );
    }

    #[test]
    fn render_section_finished_skips_output_tag_when_empty_even_with_exit_code() {
        let s = render_section(&ts_section_full("1", "$ ", "false", "", true, Some(1)));
        assert_eq!(
            s,
            "<prompt>$ </prompt><commandline executed=\"true\">false</commandline>\n"
        );
    }

    #[test]
    fn render_section_finished_emits_exit_code_with_output() {
        let s = render_section(&ts_section_full(
            "1",
            "$ ",
            "ls /nope",
            "ls: cannot access '/nope': No such file or directory",
            true,
            Some(2),
        ));
        assert_eq!(
            s,
            "<prompt>$ </prompt><commandline executed=\"true\">ls /nope</commandline>\n<output finished=\"true\" exit-code=\"2\">\nls: cannot access '/nope': No such file or directory\n</output>"
        );
    }

    #[test]
    fn system_prompt_embeds_sysinfo_block() {
        let probe = "# host\nos: Linux\n# session\nshell: bash\n";
        let rendered = SystemPromptTemplate { sysinfo: probe }.render().unwrap();

        // The probe content is embedded verbatim inside a fenced block under
        // the `# Host environment` heading, so the model can read the host/tool
        // hints as data, not instructions. The opening fence is immediately
        // followed by the probe's first section header; the shell kind is
        // reported in the `# session` block.
        assert!(
            rendered.contains("# Host environment"),
            "host-environment heading missing: {rendered}"
        );
        assert!(
            rendered.contains("```\n# host\nos: Linux"),
            "probe must appear inside the opening fence: {rendered}"
        );
        assert!(
            rendered.contains("shell: bash"),
            "shell kind must be reported: {rendered}"
        );
        // The prompt stays shell-agnostic: it must not hardcode \"Bash session\".
        assert!(
            !rendered.contains("an interactive Bash session"),
            "system prompt must not hardcode a single shell: {rendered}"
        );
    }

    #[test]
    fn system_prompt_renders_with_empty_sysinfo() {
        // `collect_sysinfo` returns an empty string on failure; the template
        // must still render (the model just loses the environment hints).
        let rendered = SystemPromptTemplate { sysinfo: "" }.render().unwrap();
        assert!(rendered.contains("# Host environment"));
    }

    fn ts_section_full(
        aid: &str,
        prompt: &str,
        command: &str,
        output: &str,
        executed: bool,
        exit_code: Option<i32>,
    ) -> ChatMessage {
        ChatMessage::TerminalSection {
            id: String::new(),
            ts: "t".into(),
            aid: aid.into(),
            exit_code,
            executed,
            cols: 80,
            rows: 24,
            raw: String::new(),
            prompt: prompt.into(),
            cmdline: command.into(),
            output: output.into(),
        }
    }

    /// A persisted live snapshot: an unexecuted `TerminalSection` (no exit code,
    /// no output by default) carrying the prompt + typed command of the section
    /// the user was typing into when they sent the turn. Matches what
    /// `send_chat_message` writes before a `User` message.
    fn ts_live_section(aid: &str, prompt: &str, command: &str) -> ChatMessage {
        ts_section_full(aid, prompt, command, "", false, None)
    }

    fn ts_section(aid: &str, prompt: &str, command: &str, output: &str) -> ChatMessage {
        ts_section_full(aid, prompt, command, output, true, Some(0))
    }

    #[test]
    fn build_history_attaches_terminal_sections_to_user_turns() {
        use crate::jsonl_store::JsonlStore;
        let dir = tempfile::tempdir().unwrap();
        let store: JsonlStore<ChatMessage> = JsonlStore::new(&dir.path().join("m.jsonl"));

        // Section before the first user turn attaches to that turn.
        store.write(ts_section("1-1", "$ ", "ls", "a\nb")).unwrap();
        store
            .write(ChatMessage::User {
                id: String::new(),
                ts: "t".into(),
                msg: "first".into(),
            })
            .unwrap();
        // A second finished section, followed by a live snapshot persisted at
        // send time (the unexecuted commandline the user was typing), attaches
        // to the next user turn — `build_history` accumulates both into the
        // same `<terminal>` block.
        store.write(ts_section("1-2", "$ ", "pwd", "/x")).unwrap();
        store
            .write(ts_live_section("1-3", "user@host:~/proj$ ", "cd /y"))
            .unwrap();
        store
            .write(ChatMessage::User {
                id: String::new(),
                ts: "t".into(),
                msg: "second".into(),
            })
            .unwrap();

        let req = build_history(&store, "").unwrap();
        let json = serde_json::to_string(&req).unwrap();

        // Both finished-section commands are present.
        assert!(json.contains(r#"<commandline executed=\"true\">ls</commandline>"#));
        assert!(json.contains(r#"<commandline executed=\"true\">pwd</commandline>"#));
        // The live (unexecuted) commandline persisted at send time is rendered.
        assert!(
            json.contains(r#"<commandline executed=\"false\">cd /y</commandline>"#),
            "live commandline dropped: {json}"
        );
        // Both user messages are present.
        assert!(json.contains("<user_message>first</user_message>"));
        assert!(json.contains("<user_message>second</user_message>"));
        // Isolation: each section attaches to the *next* user turn only.
        // Serialized order is system, turn1 (<terminal>ls</terminal><user_message>first),
        // turn2 (<terminal>pwd + live...</terminal><user_message>second).
        let i_ls = json.find("ls</command>").unwrap();
        let i_first = json.find("<user_message>first").unwrap();
        let i_pwd = json.find("pwd</command>").unwrap();
        let i_second = json.find("<user_message>second").unwrap();
        assert!(
            i_ls < i_first,
            "ls section should precede the first user turn"
        );
        assert!(
            i_first < i_pwd,
            "ls section should not bleed into the second turn"
        );
        assert!(i_pwd < i_second, "pwd section should precede its user turn");
    }

    #[test]
    fn build_history_attaches_live_snapshot_to_user_turn() {
        use crate::jsonl_store::JsonlStore;
        let dir = tempfile::tempdir().unwrap();
        let store: JsonlStore<ChatMessage> = JsonlStore::new(&dir.path().join("m.jsonl"));

        // A finished section (executed) before the turn.
        store
            .write(ts_section("1-1", "$ ", "pwd", "/home/u"))
            .unwrap();
        // `send_chat_message` persists the *live* section the user was typing
        // into as an unexecuted `TerminalSection` BEFORE the user message — with
        // a distinct prompt and an unexecuted commandline.
        store
            .write(ts_live_section("1-2", "user@host:~/proj$", "ls -la"))
            .unwrap();
        store
            .write(ChatMessage::User {
                id: String::new(),
                ts: "t".into(),
                msg: "what does this do?".into(),
            })
            .unwrap();

        let req = build_history(&store, "").unwrap();
        let json = serde_json::to_string(&req).unwrap();

        // Finished section context is present.
        assert!(json.contains(r#"<commandline executed=\"true\">pwd</commandline>"#));
        // The live prompt is rendered into the <terminal> block with the
        // unexecuted commandline as `<commandline executed="false">`, both
        // anchored to the turn that asked about it.
        assert!(
            json.contains("<prompt>user@host:~/proj$</prompt>"),
            "live prompt missing: {json}"
        );
        assert!(json.contains(r#"<commandline executed=\"false\">ls -la</commandline>"#));
        assert!(json.contains("<user_message>what does this do?</user_message>"));
    }

    #[test]
    fn assistant_output_roundtrip() {
        let output = AssistantOutput {
            message: "Try this".into(),
            commandline: "ls -la".into(),
        };
        let json = serde_json::to_string(&output).unwrap();
        let parsed: AssistantOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.message, "Try this");
        assert_eq!(parsed.commandline, "ls -la");
    }

    #[test]
    fn assistant_output_renders_both_fields_when_empty() {
        let output = AssistantOutput {
            message: "No command needed".into(),
            commandline: String::new(),
        };
        let json = serde_json::to_string(&output).unwrap();
        // Both fields are always rendered; empty is the "nothing here" sentinel.
        assert!(json.contains("\"message\":"));
        assert!(json.contains("\"commandline\":\"\""));
        let parsed: AssistantOutput = serde_json::from_str(&json).unwrap();
        assert!(parsed.commandline.is_empty());
    }

    #[test]
    fn assistant_output_deserializes_empty_object_to_empty_fields() {
        // `generate_assistant_reply` parses `response.first_text().unwrap_or("{}")`,
        // so an empty/missing provider reply must deserialize to "nothing here".
        let parsed: AssistantOutput = serde_json::from_str("{}").unwrap();
        assert!(parsed.message.is_empty());
        assert!(parsed.commandline.is_empty());
    }

    #[test]
    fn render_section_finished_with_executed_false_emits_unexecuted_command() {
        // OSC 133 C never fired before the section was finished — the recorder
        // still persisted it, but `executed` is `false`.
        let s = render_section(&ts_section_full(
            "1",
            "$ ",
            "./slow-binary",
            "killed by signal",
            false,
            None,
        ));
        assert_eq!(
            s,
            "<prompt>$ </prompt><commandline executed=\"false\">./slow-binary</commandline>\n<output finished=\"false\">\nkilled by signal\n</output>"
        );
    }

    #[test]
    fn build_history_replays_assistant_turns_as_structured_output_json() {
        use crate::jsonl_store::JsonlStore;
        let dir = tempfile::tempdir().unwrap();
        let store: JsonlStore<ChatMessage> = JsonlStore::new(&dir.path().join("m.jsonl"));

        store
            .write(ChatMessage::User {
                id: String::new(),
                ts: "t".into(),
                msg: "hello?".into(),
            })
            .unwrap();
        store
            .write(ChatMessage::Assistant {
                id: String::new(),
                ts: "t".into(),
                cmdline: "ls".into(),
                msg: "try this".into(),
                model: "test-model".into(),
            })
            .unwrap();
        store
            .write(ChatMessage::User {
                id: String::new(),
                ts: "t".into(),
                msg: "again".into(),
            })
            .unwrap();

        let req = build_history(&store, "").unwrap();
        let json = serde_json::to_string(&req).unwrap();

        // The replayed assistant turn is the same JSON shape the model
        // produces (`{ "message": ..., "commandline": ... }`),
        // matching the structured-output contract.
        assert!(
            json.contains(r#"{\"message\":\"try this\",\"commandline\":\"ls\"}"#),
            "assistant replay shape unexpected: {json}"
        );
        // The two user turns still render their messages, and the order is
        // preserved (system, user1, assistant, user2).
        let i_hello = json.find("hello?").unwrap();
        let i_replay = json.find(r#"{\"message\":\"try this\"#).unwrap();
        let i_again = json.find("again").unwrap();
        assert!(
            i_hello < i_replay,
            "user1 should precede the assistant replay"
        );
        assert!(i_replay < i_again, "assistant replay should precede user2");
    }
}
