mod generation;

use crate::jsonl_store::{JsonlStore, ReadPage};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, State};

// ---------------------------------------------------------------------------
// ChatMessage (stored in JSONL)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum ChatMessage {
    User {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        commandline: Option<String>,
        msg: String,
        ts: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        terminal: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        terminal_marker: Option<String>,
    },
    Assistant {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        id: String,

        // TODO: allow multiple suggestions
        #[serde(default, skip_serializing_if = "Option::is_none")]
        commandline: Option<String>,
        msg: String,
        ts: String,
        model: String,
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
    terminal: Option<String>,
    terminal_marker: Option<String>,
    commandline: Option<String>,
    msg: Option<String>,
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
    store: JsonlStore<ChatMessage>,
    client: genai::Client,
    generating: Arc<AtomicBool>,
    // Saved for reconstructing the store in spawned tasks.
    store_dir: PathBuf,
    store_file: String,
}

impl ChatSession {
    /// Create a new chat session.
    pub fn new(app_data_dir: &std::path::Path) -> Result<Self, String> {
        let id = Utc::now().format("%Y-%m-%d_%H%M%S").to_string();

        let store_file = format!("chat-{id}.jsonl");

        let store = JsonlStore::new(app_data_dir, &store_file)
            .map_err(|e| e.to_string())?
            .with_on_read(|msg: &mut ChatMessage, offset: u32| {
                msg.set_id(offset.to_string());
            });

        let client = genai::Client::builder().build();

        Ok(Self {
            id,
            store,
            client,
            generating: Arc::new(AtomicBool::new(false)),
            store_dir: app_data_dir.to_path_buf(),
            store_file,
        })
    }

    /// Append a user message and return its byte-offset ID.
    pub fn append_user(&self, payload: ChatUserMessagePayload) -> Result<String, String> {
        let ts = now_timestamp();
        let message = ChatMessage::User {
            id: String::new(),
            msg: payload.msg.unwrap_or_default(),
            ts,
            terminal: payload.terminal,
            terminal_marker: payload.terminal_marker,
            commandline: payload.commandline,
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

    // Append and broadcast the user message.
    let user_id = session.append_user(payload).inspect_err(|_| {
        session.generating.store(false, Ordering::SeqCst);
    })?;

    emit_messages_changed(&app, user_id);

    // Capture what the spawned task needs, then release the State borrow.
    let generating = session.generating.clone();
    let client = session.client.clone();
    let store_dir = session.store_dir.clone();
    let store_file = session.store_file.clone();

    tauri::async_runtime::spawn(async move {
        // Reconstruct a store handle for reading/writing from this task.
        let store = JsonlStore::<ChatMessage>::new(&store_dir, &store_file)
            .expect("failed to reopen chat store")
            .with_on_read(|_msg: &mut ChatMessage, _offset: u32| {});

        let ts = now_timestamp();

        match generation::generate_assistant_reply(&client, &store, &ts).await {
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
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", secs.as_secs())
}
