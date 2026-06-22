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

#[derive(Template)]
#[template(path = "system_prompt.md")]
struct SystemPromptTemplate;

#[derive(Template)]
#[template(path = "user_turn.md")]
struct UserTurnTemplate<'a> {
    terminal: &'a str,
    commandline: Option<&'a str>,
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
                    "description": "Suggested bash commandline to replace the user's current commandline. Use an empty string when there is no command to suggest."
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
/// Assistant turns are replayed as the same JSON shape used for structured
/// output (`{ "msg": …, "commandline": … }`) to keep history consistent
/// with the output contract.
fn build_history(store: &JsonlStore<ChatMessage>) -> Result<ChatRequest, String> {
    let system = SystemPromptTemplate
        .render()
        .map_err(|e| format!("failed to render system prompt: {e}"))?;
    let mut req = ChatRequest::new(vec![]).with_system(system);

    let page = store.read(0, None).map_err(|e| e.to_string())?;

    for msg in &page.items {
        match msg {
            ChatMessage::User {
                msg,
                terminal,
                cmdline,
                ..
            } => {
                let rendered = UserTurnTemplate {
                    terminal: terminal.as_ref(),
                    commandline: cmdline.as_deref(),
                    message: msg,
                }
                .render()
                .map_err(|e| format!("failed to render user turn: {e}"))?;
                req = req.append_message(GenaiChatMessage::user(rendered));
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
        }
    }

    Ok(req)
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
    terminal_section: &str,
) -> Result<u32, String> {
    let req = build_history(store)?;

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
        term_sect: Some(terminal_section.to_string()),
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
            terminal: "<prompt>$</prompt>\n<command>ls</command>\n<output>\nfile.txt\nfile2.txt\n</output>",
            commandline: Some("cat file.txt"),
            message: "What does this file contain?",
        }
        .render()
        .unwrap();

        assert!(rendered.contains("<terminal>\n<prompt>$</prompt>\n<command>ls</command>\n<output>\nfile.txt\nfile2.txt\n</output>\n</terminal>"));
        assert!(rendered.contains("<commandline>cat file.txt</commandline>"));
        assert!(rendered.contains("<user_message>What does this file contain?</user_message>"));
        // Content inside tags must be unescaped.
        assert!(rendered.contains("<prompt>$</prompt>"));
    }

    #[test]
    fn user_turn_template_skips_optional_fields() {
        let rendered = UserTurnTemplate {
            terminal: "",
            commandline: None,
            message: "hello",
        }
        .render()
        .unwrap();

        assert!(rendered.contains("<terminal>\n</terminal>"));
        assert!(!rendered.contains("<commandline>"));
        assert!(rendered.contains("<user_message>hello</user_message>"));
    }

    #[test]
    fn user_turn_template_empty_msg_skips_msg_tag() {
        let rendered = UserTurnTemplate {
            terminal: "",
            commandline: Some("ls"),
            message: "",
        }
        .render()
        .unwrap();

        assert!(!rendered.contains("<user_message>"));
        assert!(rendered.contains("<commandline>ls</commandline>"));
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
        assert!(json.contains("\"msg\":"));
        assert!(json.contains("\"commandline\":\"\""));
        let parsed: AssistantOutput = serde_json::from_str(&json).unwrap();
        assert!(parsed.commandline.is_empty());
    }
}
