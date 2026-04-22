use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SessionState {
    pub input_path: Option<PathBuf>,
    pub layout_path: Option<PathBuf>,
    pub output_path: Option<PathBuf>,
    pub codec: String,
    pub quality: u32,
    pub chromakey: String,
    pub from_seconds: f64,
    pub to_seconds: Option<f64>,
    pub cli_path_override: Option<PathBuf>,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            input_path: None,
            layout_path: None,
            output_path: None,
            codec: "prores_4444".into(),
            quality: 20,
            chromakey: "#00ff00".into(),
            from_seconds: 0.0,
            to_seconds: None,
            cli_path_override: None,
        }
    }
}

pub fn load_from_str(s: &str) -> anyhow::Result<SessionState> {
    Ok(serde_json::from_str(s)?)
}

pub fn to_string(s: &SessionState) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(s)?)
}

use std::fs;
use tauri::Manager;

pub fn session_path(app: &tauri::AppHandle) -> anyhow::Result<PathBuf> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| anyhow::anyhow!("no app_config_dir: {}", e))?;
    fs::create_dir_all(&dir)?;
    Ok(dir.join("session.json"))
}

#[tauri::command]
pub fn session_load(app: tauri::AppHandle) -> Result<SessionState, String> {
    let path = session_path(&app).map_err(|e| e.to_string())?;
    if !path.exists() {
        return Ok(SessionState::default());
    }
    let s = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    load_from_str(&s).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn session_save(app: tauri::AppHandle, state: SessionState) -> Result<(), String> {
    let path = session_path(&app).map_err(|e| e.to_string())?;
    let s = to_string(&state).map_err(|e| e.to_string())?;
    fs::write(&path, s).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_serializes_and_parses() {
        let s = SessionState::default();
        let json = to_string(&s).unwrap();
        let back: SessionState = load_from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn parses_partial_json_fills_defaults() {
        let s = load_from_str(r#"{"codec":"h264_nvenc"}"#).unwrap();
        assert_eq!(s.codec, "h264_nvenc");
        assert_eq!(s.quality, 20);
    }

    #[test]
    fn unknown_fields_ignored() {
        let s = load_from_str(r#"{"codec":"prores_4444","future_field":42}"#);
        assert!(s.is_ok());
    }
}
