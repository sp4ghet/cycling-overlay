#![cfg(feature = "ffmpeg-tests")]

use assert_cmd::Command;
use std::path::PathBuf;
use std::process::Command as StdCommand;

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
fn render_short_gpx_produces_valid_mov() {
    let ws = workspace_root();
    let tmp = tempfile::TempDir::new().unwrap();
    let out = tmp.path().join("overlay.mov");

    // NOTE: cannot use --size 640x360 here because the layout's rects are
    // sized for 1920x1080 and the CLI `--size` override applies before
    // validation, so they'd overflow. Render at the layout's native size.
    Command::cargo_bin("cycling-overlay")
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
            "0:02",
        ])
        .assert()
        .success();

    assert!(out.exists(), "output .mov not created");

    // ffprobe the result.
    let probe = StdCommand::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height,r_frame_rate,nb_read_frames,codec_name,pix_fmt",
            "-count_frames",
            "-of",
            "default=noprint_wrappers=1",
        ])
        .arg(&out)
        .output()
        .expect("ffprobe to run");
    assert!(
        probe.status.success(),
        "ffprobe failed: {}",
        String::from_utf8_lossy(&probe.stderr)
    );
    let s = String::from_utf8_lossy(&probe.stdout);
    assert!(s.contains("codec_name=prores"), "{}", s);
    assert!(s.contains("width=1920"), "{}", s);
    assert!(s.contains("height=1080"), "{}", s);
    // Alpha channel present — yuva444p10le or yuva444p12le depending on ffmpeg version.
    assert!(s.contains("pix_fmt=yuva444p"), "{}", s);
    assert!(s.contains("r_frame_rate=30/1"), "{}", s);
    // 2 seconds at 30fps = 60 frames.
    assert!(s.contains("nb_read_frames=60"), "{}", s);
}
