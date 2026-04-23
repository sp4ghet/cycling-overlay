use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;

pub struct WatcherHandle(pub Mutex<Option<RecommendedWatcher>>);

impl Default for WatcherHandle {
    fn default() -> Self {
        Self(Mutex::new(None))
    }
}

const DEBOUNCE: Duration = Duration::from_millis(150);

/// Watch the layout file at `path`. On each modify event, wait `DEBOUNCE`,
/// re-read + re-parse the file, update AppState, and emit either
/// `layout-reloaded` or `layout-error` (payload = error message).
///
/// Replaces any prior watcher. Pass a new path to switch targets.
#[tauri::command]
pub fn watch_layout(
    app: AppHandle,
    handle: tauri::State<WatcherHandle>,
    path: PathBuf,
) -> Result<(), String> {
    let (tx, rx) = channel::<notify::Result<Event>>();
    let mut watcher = notify::recommended_watcher(tx).map_err(|e| e.to_string())?;
    watcher
        .watch(&path, RecursiveMode::NonRecursive)
        .map_err(|e| e.to_string())?;
    *handle.0.lock().map_err(|e| e.to_string())? = Some(watcher);

    let app_clone = app.clone();
    let path_clone = path.clone();
    std::thread::spawn(move || {
        // Reset-timer debounce: block on the first event, then drain any
        // further events until the channel goes quiet for DEBOUNCE. This
        // reliably coalesces editor rename-replace bursts (the first event
        // can fire on the old inode; we wait out the window and read the
        // final file state) — the previous "fire then ignore for 150ms"
        // strategy dropped everything after event 1 of the burst.
        loop {
            // Wait for the first interesting event.
            let first = match rx.recv() {
                Ok(Ok(ev)) => ev,
                Ok(Err(_)) => continue, // notify internal error, ignore
                Err(_) => return,        // channel closed (watcher dropped)
            };
            if !matches!(first.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                continue;
            }
            // Quiet-period drain: each event restarts the timeout.
            loop {
                match rx.recv_timeout(DEBOUNCE) {
                    Ok(_) => continue, // more activity — reset timer
                    Err(RecvTimeoutError::Timeout) => break, // quiet — reload
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }

            match std::fs::read_to_string(&path_clone)
                .map_err(|e| e.to_string())
                .and_then(|s| {
                    serde_json::from_str::<layout::Layout>(&s).map_err(|e| e.to_string())
                }) {
                Ok(new_layout) => {
                    if let Some(state) = app_clone.try_state::<AppState>() {
                        state.set_layout(new_layout, path_clone.clone());
                    }
                    let _ = app_clone.emit("layout-reloaded", ());
                }
                Err(msg) => {
                    let _ = app_clone.emit("layout-error", msg);
                }
            }
        }
    });
    Ok(())
}

/// Drop the current watcher, if any.
#[tauri::command]
pub fn unwatch_layout(handle: tauri::State<WatcherHandle>) -> Result<(), String> {
    *handle.0.lock().map_err(|e| e.to_string())? = None;
    Ok(())
}
