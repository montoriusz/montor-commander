pub mod chat;
mod jsonl_store;
mod osc133;
mod terminal;

use chat::ChatSession;
use tauri::Manager;

/// Initialize the `tracing` logging subscriber.
///
/// Defaults to `app_lib::chat::generation=debug` in debug builds and `off` in
/// release builds. `RUST_LOG` (read from the environment via `dotenvy` in
/// `main.rs`) always wins when present, enabling opt-in logging for release or
/// opt-out for dev. Uses `try_init` so repeated initialization (e.g. in tests)
/// is a no-op rather than a panic.
fn init_logging() {
    use tracing_subscriber::{EnvFilter, fmt};

    let default_directive = if cfg!(debug_assertions) {
        "app_lib::chat::generation=debug"
    } else {
        "off"
    };

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_directive));

    let _ = fmt().with_env_filter(filter).try_init();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_logging();

    let terminal_session = terminal::build_session();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Create chat session. The terminal reader is spawned lazily by
            // `create_shell`, so dev HMR can re-point the channel without
            // respawning the shell.
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data dir");
            let chat_session = ChatSession::new(&data_dir).expect("failed to create chat session");
            app.manage(chat_session);

            Ok(())
        })
        .manage(terminal_session)
        .invoke_handler(tauri::generate_handler![
            terminal::write_to_pty,
            terminal::resize_pty,
            terminal::create_shell,
            chat::get_chat_session,
            chat::read_chat_messages,
            chat::send_chat_message,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
