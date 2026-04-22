use std::path::{Path, PathBuf};
use std::process::Command;

pub fn sibling_of(exe_path: &Path, binary_name: &str) -> Option<PathBuf> {
    let candidate = exe_path.parent()?.join(binary_name);
    if candidate.exists() {
        Some(candidate)
    } else {
        None
    }
}

pub fn on_path(binary_name: &str) -> Option<PathBuf> {
    which::which(binary_name).ok()
}

/// Resolution order: override > sibling-of-exe > PATH.
pub fn resolve(
    override_path: Option<&Path>,
    exe_path: &Path,
    binary_name: &str,
) -> Option<PathBuf> {
    if let Some(p) = override_path {
        if p.exists() {
            return Some(p.to_path_buf());
        }
    }
    if let Some(p) = sibling_of(exe_path, binary_name) {
        return Some(p);
    }
    on_path(binary_name)
}

#[tauri::command]
pub fn probe_ffmpeg(override_path: Option<PathBuf>) -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let binary_name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    let path = resolve(override_path.as_deref(), &exe, binary_name).ok_or_else(|| {
        format!("{} not found (checked sibling, PATH, override)", binary_name)
    })?;
    let output = Command::new(&path)
        .arg("-version")
        .output()
        .map_err(|e| format!("failed to run {}: {}", path.display(), e))?;
    if !output.status.success() {
        return Err(format!("{} -version exited non-zero", path.display()));
    }
    Ok(path)
}

#[tauri::command]
pub fn probe_cli(override_path: Option<PathBuf>) -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let binary_name = if cfg!(windows) {
        "gpx-overlay.exe"
    } else {
        "gpx-overlay"
    };
    resolve(override_path.as_deref(), &exe, binary_name)
        .ok_or_else(|| format!("{} not found", binary_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    #[test]
    fn sibling_finds_exe() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("gui.exe");
        File::create(&exe).unwrap();
        let sib = dir.path().join("gpx-overlay.exe");
        File::create(&sib).unwrap();
        assert_eq!(sibling_of(&exe, "gpx-overlay.exe").unwrap(), sib);
    }

    #[test]
    fn sibling_missing_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("gui.exe");
        File::create(&exe).unwrap();
        assert!(sibling_of(&exe, "nope.exe").is_none());
    }

    #[test]
    fn override_takes_precedence_over_sibling() {
        let dir = tempfile::tempdir().unwrap();
        let override_path = dir.path().join("custom.exe");
        File::create(&override_path).unwrap();
        let exe = dir.path().join("gui.exe");
        File::create(&exe).unwrap();
        let sib = dir.path().join("gpx-overlay.exe");
        File::create(&sib).unwrap();
        assert_eq!(
            resolve(Some(&override_path), &exe, "gpx-overlay.exe").unwrap(),
            override_path
        );
    }
}
