#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod binary;
mod session;

#[tauri::command]
fn hello_from_rust() -> String {
    "Hello from Rust".into()
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            hello_from_rust,
            session::session_load,
            session::session_save,
            binary::probe_ffmpeg,
            binary::probe_cli,
        ])
        .setup(|_app| Ok(()))
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
