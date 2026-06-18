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
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AssistantOutput {
    /// Natural-language reply shown in the chat.
    pub msg: String,
    /// Suggested commandline to replace the user's commandline. Omitted when no suggestion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commandline: Option<String>,
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
    terminal: Option<&'a str>,
    commandline: Option<&'a str>,
    msg: &'a str,
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
                "msg": {
                    "type": "string",
                    "description": "Assistant reply to the user."
                },
                "commandline": {
                    "type": ["string", "null"],
                    "description": "Suggested bash commandline to replace the user's current commandline, or null."
                }
            },
            "required": ["msg"],
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
                commandline,
                ..
            } => {
                let rendered = UserTurnTemplate {
                    terminal: terminal.as_deref(),
                    commandline: commandline.as_deref(),
                    msg,
                }
                .render()
                .map_err(|e| format!("failed to render user turn: {e}"))?;
                req = req.append_message(GenaiChatMessage::user(rendered));
            }
            ChatMessage::Assistant {
                msg, commandline, ..
            } => {
                // Replay assistant turns as the same JSON shape the model
                // produces, so the history stays consistent with the contract.
                let prior = AssistantOutput {
                    msg: msg.clone(),
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
) -> Result<u32, String> {
    let req = build_history(store)?;

    let options = ChatOptions::default().with_response_format(response_format());
    let response = client
        .exec_chat(MODEL, req, Some(&options))
        .await
        .map_err(|e| e.to_string())?;

    let raw = response.first_text().unwrap_or("{}");
    let parsed: AssistantOutput = serde_json::from_str(raw)
        .map_err(|e| format!("failed to parse assistant output: {e}; raw: {raw}"))?;

    let message = ChatMessage::Assistant {
        id: String::new(),
        commandline: parsed.commandline,
        msg: parsed.msg,
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
            terminal: Some("<prompt>$</prompt><command>ls</command><output>file.txt</output>"),
            commandline: Some("cat file.txt"),
            msg: "What does this file contain?",
        }
        .render()
        .unwrap();

        assert!(rendered.contains("<terminal>"));
        assert!(rendered.contains("<commandline>cat file.txt</commandline>"));
        assert!(rendered.contains("<msg>What does this file contain?</msg>"));
        // Content inside tags must be unescaped.
        assert!(rendered.contains("<prompt>$</prompt>"));
    }

    #[test]
    fn user_turn_template_skips_optional_fields() {
        let rendered = UserTurnTemplate {
            terminal: None,
            commandline: None,
            msg: "hello",
        }
        .render()
        .unwrap();

        assert!(!rendered.contains("<terminal>"));
        assert!(rendered.contains("<commandline></commandline>"));
        assert!(rendered.contains("<msg>hello</msg>"));
    }

    #[test]
    fn user_turn_template_empty_msg_skips_msg_tag() {
        let rendered = UserTurnTemplate {
            terminal: None,
            commandline: Some("ls"),
            msg: "",
        }
        .render()
        .unwrap();

        assert!(!rendered.contains("<msg>"));
        assert!(rendered.contains("<commandline>ls</commandline>"));
    }

    #[test]
    fn assistant_output_roundtrip() {
        let output = AssistantOutput {
            msg: "Try this".into(),
            commandline: Some("ls -la".into()),
        };
        let json = serde_json::to_string(&output).unwrap();
        let parsed: AssistantOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.msg, "Try this");
        assert_eq!(parsed.commandline.as_deref(), Some("ls -la"));
    }

    #[test]
    fn assistant_output_commandline_omitted() {
        let output = AssistantOutput {
            msg: "No command needed".into(),
            commandline: None,
        };
        let json = serde_json::to_string(&output).unwrap();
        // skip_serializing_if should omit the field entirely.
        assert!(!json.contains("commandline"));
        let parsed: AssistantOutput = serde_json::from_str(&json).unwrap();
        assert!(parsed.commandline.is_none());
    }
}
