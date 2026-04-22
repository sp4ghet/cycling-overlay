#![cfg(feature = "ffmpeg-tests")]

use assert_cmd::Command;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    std::env::current_dir()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn emits_json_progress_and_done_lines() {
    let ws = workspace_root();
    let tmp = tempfile::TempDir::new().unwrap();
    let out = tmp.path().join("overlay.mov");

    let assert = Command::cargo_bin("gpx-overlay")
        .unwrap()
        .args([
            "render",
            "--input",
            ws.join("examples").join("short.gpx").to_str().unwrap(),
            "--layout",
            ws.join("examples").join("layout.json").to_str().unwrap(),
            "--output",
            out.to_str().unwrap(),
            "--from",
            "0:00",
            "--to",
            "0:01",
            "--progress-json",
        ])
        .assert()
        .success();

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).to_string();
    assert!(
        stderr.contains(r#""type":"progress""#),
        "no progress JSON lines in stderr:\n{}",
        stderr
    );
    assert!(
        stderr.contains(r#""type":"done""#),
        "no done JSON line in stderr:\n{}",
        stderr
    );
}
