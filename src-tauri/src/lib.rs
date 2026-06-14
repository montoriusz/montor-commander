mod osc133;

use osc133::ShellEvent;
use portable_pty::{native_pty_system, CommandBuilder, PtyPair, PtySize};
use std::{
    io::{Read, Write},
    sync::Arc,
    thread,
};
use tauri::{async_runtime::Mutex as AsyncMutex, AppHandle, Emitter, State};

/// Bash integration script embedded at compile time.
static BASH_INTEGRATION: &str = include_str!("../assets/bash-integration.sh");

struct AppState {
    pty_pair: Arc<AsyncMutex<PtyPair>>,
    writer: Arc<AsyncMutex<Box<dyn Write + Send>>>,
}

#[derive(Clone, serde::Serialize)]
struct PtyOutputPayload {
    data: String,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandFinishedPayload {
    exit_code: Option<i32>,
    aid: Option<String>,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ShellEventPayload {
    aid: Option<String>,
}

#[tauri::command]
async fn create_shell(state: State<'_, AppState>) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut cmd = CommandBuilder::new("powershell.exe");

    #[cfg(not(target_os = "windows"))]
    let mut cmd = {
        // Write the bash integration script to a temp file.
        let rcfile_path = std::env::temp_dir().join("tauri_terminal_bash_integration.sh");
        std::fs::write(&rcfile_path, BASH_INTEGRATION).map_err(|e| e.to_string())?;

        let mut c = CommandBuilder::new("bash");
        c.arg("--rcfile");
        c.arg(rcfile_path);
        c.arg("-i");
        c
    };

    #[cfg(target_os = "windows")]
    cmd.env("TERM", "cygwin");

    #[cfg(not(target_os = "windows"))]
    cmd.env("TERM", "xterm-256color");

    let mut child = state
        .pty_pair
        .lock()
        .await
        .slave
        .spawn_command(cmd)
        .map_err(|e| e.to_string())?;

    thread::spawn(move || {
        let _ = child.wait();
    });

    Ok(())
}

#[tauri::command]
async fn write_to_pty(data: &str, state: State<'_, AppState>) -> Result<(), ()> {
    write!(state.writer.lock().await, "{}", data).map_err(|_| ())
}

#[tauri::command]
async fn resize_pty(rows: u16, cols: u16, state: State<'_, AppState>) -> Result<(), ()> {
    state
        .pty_pair
        .lock()
        .await
        .master
        .resize(PtySize {
            rows,
            cols,
            ..Default::default()
        })
        .map_err(|_| ())
}

fn emit_shell_event(app: &AppHandle, event: ShellEvent) {
    match event {
        ShellEvent::PromptStarted { aid } => {
            let _ = app.emit("prompt-started", ShellEventPayload { aid });
        }
        ShellEvent::PromptEnded { aid } => {
            let _ = app.emit("prompt-ended", ShellEventPayload { aid });
        }
        ShellEvent::CommandStarted { aid } => {
            let _ = app.emit("command-started", ShellEventPayload { aid });
        }
        ShellEvent::CommandFinished { exit_code, aid } => {
            let _ = app.emit(
                "command-finished",
                CommandFinishedPayload { exit_code, aid },
            );
        }
    }
}

fn spawn_reader_thread(app: AppHandle, reader: Box<dyn Read + Send>) {
    thread::spawn(move || {
        let mut reader = reader;
        let mut buf = [0u8; 4096];
        let mut carry: Vec<u8> = Vec::new();

        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let chunk = &buf[..n];
                    // Detect OSC 133 sequences and emit shell-integration events;
                    // the raw bytes are forwarded unchanged so xterm.js sees them too.
                    osc133::scan(&mut carry, chunk, |event| emit_shell_event(&app, event));
                    let data = String::from_utf8_lossy(chunk).into_owned();
                    let _ = app.emit("pty-output", PtyOutputPayload { data });
                }
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let pty_system = native_pty_system();

    let pty_pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .unwrap();

    let reader = pty_pair.master.try_clone_reader().unwrap();
    let writer = pty_pair.master.take_writer().unwrap();

    tauri::Builder::default()
        .setup(|app| {
            spawn_reader_thread(app.handle().clone(), reader);
            Ok(())
        })
        .manage(AppState {
            pty_pair: Arc::new(AsyncMutex::new(pty_pair)),
            writer: Arc::new(AsyncMutex::new(writer)),
        })
        .invoke_handler(tauri::generate_handler![
            write_to_pty,
            resize_pty,
            create_shell,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
