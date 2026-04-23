use assert_cmd::Command;
use predicates::str;

/// Integration tests invoke the actual binary via `assert_cmd::Command::cargo_bin`.
/// Tests run with CWD = crates/cli, so `examples/short.gpx` lives two parents up.
fn workspace_root() -> std::path::PathBuf {
    let p = std::env::current_dir().unwrap();
    p.parent().unwrap().parent().unwrap().to_path_buf()
}

#[test]
fn dry_run_on_short_gpx_succeeds() {
    let ws = workspace_root();
    let input = ws.join("examples").join("short.gpx");

    let layout_json = r##"{
        "version": 1,
        "canvas": { "width": 1920, "height": 1080, "fps": 30 },
        "units": { "speed": "kmh", "distance": "km", "elevation": "m", "temp": "c" },
        "theme": { "font": "Inter", "fg": "#ffffff", "accent": "#ffcc00", "shadow": null },
        "widgets": [
            { "type": "readout", "id": "spd",
              "metric": "speed",
              "rect": { "x": 100, "y": 900, "w": 300, "h": 120 },
              "label": "SPEED", "decimals": 1, "font_size": 72.0 }
        ]
    }"##;
    let tmp = tempfile::TempDir::new().unwrap();
    let layout_path = tmp.path().join("layout.json");
    std::fs::write(&layout_path, layout_json).unwrap();
    let output_path = tmp.path().join("out.mov");

    Command::cargo_bin("cycling-overlay")
        .unwrap()
        .args([
            "render",
            "--input",
            input.to_str().unwrap(),
            "--layout",
            layout_path.to_str().unwrap(),
            "--output",
            output_path.to_str().unwrap(),
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(str::contains("Activity:"))
        .stdout(str::contains("widgets"))
        .stdout(str::contains("frames"));
}

#[test]
fn dry_run_fails_on_missing_input() {
    let tmp = tempfile::TempDir::new().unwrap();
    let output_path = tmp.path().join("out.mov");

    Command::cargo_bin("cycling-overlay")
        .unwrap()
        .args([
            "render",
            "--input",
            "/nonexistent/file.gpx",
            "--layout",
            "/nonexistent/layout.json",
            "--output",
            output_path.to_str().unwrap(),
            "--dry-run",
        ])
        .assert()
        .failure();
}
