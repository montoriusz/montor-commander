mod generation;

use crate::jsonl_store::{JsonlStore, ReadPage};
use chrono::{Local, Utc};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, State};

// ---------------------------------------------------------------------------
// ChatMessage (stored in JSONL)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ChatMessage {
    User {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        id: String,

        ts: String,

        #[serde(default, skip_serializing_if = "String::is_empty")]
        terminal: String,

        /// Section of the terminal that was last executed.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        term_sect: Option<String>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        cmdline: Option<String>,

        msg: String,
    },
    Assistant {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        id: String,

        ts: String,

        #[serde(default, skip_serializing_if = "String::is_empty")]
        cmdline: String,

        msg: String,

        model: String,

        /// Target terminal section for the assistant's response.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        term_sect: Option<String>,
    },
}

impl ChatMessage {
    fn set_id(&mut self, id: String) {
        match self {
            ChatMessage::User { id: field, .. } | ChatMessage::Assistant { id: field, .. } => {
                *field = id;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tauri command payloads
// ---------------------------------------------------------------------------

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSessionInfo {
    pub id: String,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatPage {
    pub messages: Vec<ChatMessage>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatUserMessagePayload {
    terminal: String,
    last_executed_sect: Option<String>,
    current_sect: String,
    cmdline: Option<String>,
    msg: String,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessagesChangedPayload {
    pub latest_id: String,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatGenerationErrorPayload {
    pub message: String,
}

// ---------------------------------------------------------------------------
// Event helpers (also ensure typegen discovers the event names)
// ---------------------------------------------------------------------------

fn emit_messages_changed(app: &AppHandle, latest_id: String) {
    let _ = app.emit(
        "chat-messages-changed",
        ChatMessagesChangedPayload { latest_id },
    );
}

fn emit_generation_error(app: &AppHandle, message: String) {
    let _ = app.emit(
        "chat-generation-error",
        ChatGenerationErrorPayload { message },
    );
}

// ---------------------------------------------------------------------------
// ChatSession
// ---------------------------------------------------------------------------

pub struct ChatSession {
    pub id: String,
    store: Arc<JsonlStore<ChatMessage>>,
    generating: Arc<AtomicBool>,
}

impl ChatSession {
    /// Create a new chat session.
    pub fn new(app_data_dir: &std::path::Path) -> Result<Self, String> {
        let id = Utc::now().format("%Y-%m-%d_%H%M%S").to_string();

        let session_dir = &app_data_dir.join(format!("chat-{id}"));
        std::fs::create_dir_all(&session_dir).map_err(|e| e.to_string())?;

        let store = JsonlStore::new(&session_dir.join("messages.jsonl")).with_on_read(
            |msg: &mut ChatMessage, offset: u32| {
                msg.set_id(offset.to_string());
            },
        );

        Ok(Self {
            id,
            store: Arc::new(store),
            generating: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Append a user message and return its byte-offset ID.
    pub fn append_user(&self, payload: ChatUserMessagePayload) -> Result<String, String> {
        let ts = now_timestamp();
        let message = ChatMessage::User {
            id: String::new(),
            ts,
            terminal: payload.terminal,
            term_sect: payload.last_executed_sect,
            msg: payload.msg,
            cmdline: payload.cmdline,
        };
        let id = self.store.write(message).map_err(|e| e.to_string())?;
        Ok(id.to_string())
    }

    /// Read messages after `after_cursor` (byte offset).
    /// Pass `None` to read from the beginning.
    pub fn read_page(&self, after_cursor: Option<u32>) -> Result<ReadPage<ChatMessage>, String> {
        let start = after_cursor.unwrap_or(0);
        self.store.read(start, None).map_err(|e| e.to_string())
    }
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_chat_session(session: State<'_, ChatSession>) -> Result<ChatSessionInfo, String> {
    Ok(ChatSessionInfo {
        id: session.id.clone(),
    })
}

#[tauri::command]
pub fn read_chat_messages(
    after_cursor: Option<String>,
    session: State<'_, ChatSession>,
) -> Result<ChatPage, String> {
    let cursor: Option<u32> = after_cursor
        .as_deref()
        .map(str::parse)
        .transpose()
        .map_err(|_| "invalid cursor format".to_string())?;

    let page = session.read_page(cursor)?;
    let next_cursor = if page.items.is_empty() && page.next_id == cursor.unwrap_or(0) {
        // Nothing new — return the same cursor the client sent (or null)
        after_cursor
    } else {
        Some(page.next_id.to_string())
    };

    Ok(ChatPage {
        messages: page.items,
        next_cursor,
    })
}

#[tauri::command]
pub async fn send_chat_message(
    payload: ChatUserMessagePayload,
    app: AppHandle,
    session: State<'_, ChatSession>,
) -> Result<(), String> {
    // Reject if already generating.
    if session
        .generating
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("already generating".to_string());
    }

    let current_section = payload.current_sect.clone();

    // Append and broadcast the user message.
    let user_id = session.append_user(payload).inspect_err(|_| {
        session.generating.store(false, Ordering::SeqCst);
    })?;

    emit_messages_changed(&app, user_id);

    // Capture what the spawned task needs, then release the State borrow.
    let generating = session.generating.clone();
    let store = Arc::clone(&session.store);
    let client = genai::Client::builder().build();

    tauri::async_runtime::spawn(async move {
        let ts = now_timestamp();

        match generation::generate_assistant_reply(&client, &store, &ts, &current_section).await {
            Ok(assistant_id) => {
                emit_messages_changed(&app, assistant_id.to_string());
            }
            Err(e) => {
                emit_generation_error(&app, e);
            }
        }

        generating.store(false, Ordering::SeqCst);
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn now_timestamp() -> String {
    Local::now().to_rfc3339()
}
