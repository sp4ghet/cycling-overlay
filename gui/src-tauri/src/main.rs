#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod binary;
mod preview;
mod progress;
mod session;
mod state;
mod watcher;

#[tauri::command]
fn hello_from_rust() -> String {
    "Hello from Rust".into()
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state::AppState::default())
        .manage(preview::TextState::default())
        .manage(watcher::WatcherHandle::default())
        .invoke_handler(tauri::generate_handler![
            hello_from_rust,
            session::session_load,
            session::session_save,
            binary::probe_ffmpeg,
            binary::probe_cli,
            state::load_activity,
            state::load_layout,
            preview::preview_frame,
            watcher::watch_layout,
            watcher::unwatch_layout,
        ])
        .setup(|_app| Ok(()))
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
