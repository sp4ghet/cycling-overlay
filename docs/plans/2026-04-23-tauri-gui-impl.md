# Tauri GUI v1 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task.

**Goal:** Build a cross-platform desktop GUI (Tauri 2.x + Svelte + TypeScript) that wraps the existing `gpx-overlay` CLI with a scrubbable preview seekbar, codec-aware export controls, log pane, and layout-file hot-reload. Design doc: `docs/plans/2026-04-23-tauri-gui-design.md`.

**Architecture:** Hybrid renderer. Preview uses the `render` crate as a library (single-frame pixmap → PNG data URL). Export spawns the existing `gpx-overlay` CLI as a subprocess and streams structured progress back via Tauri events.

**Tech stack:** Tauri 2.x backend (Rust), `notify` for layout file watching, `tokio` for async command handlers. Svelte 4 + TypeScript + Vite frontend. `@tauri-apps/plugin-dialog` for file pickers. `@testing-library/svelte` + `vitest` for frontend unit tests.

**Working directory:** `gui/` (new top-level directory). Backend at `gui/src-tauri/`. Frontend at `gui/src/`.

**Prerequisites:** `node` (≥20), `npm`, Rust toolchain (already present), `cargo-tauri` CLI (`cargo install tauri-cli --version '^2.0.0' --locked`), `ffmpeg` on PATH for runtime testing.

---

## Task 1: Scaffold Tauri backend directory and Cargo workspace integration

**Files:**
- Create: `gui/src-tauri/Cargo.toml`
- Create: `gui/src-tauri/build.rs`
- Create: `gui/src-tauri/src/main.rs`
- Create: `gui/src-tauri/tauri.conf.json`
- Create: `gui/src-tauri/icons/` (empty placeholder — will copy Tauri defaults)
- Modify: `Cargo.toml` (workspace root) — add `gui/src-tauri` to `members`

**Step 1: Add member to workspace root**

In `Cargo.toml`:
```toml
members = ["crates/activity", "crates/layout", "crates/render", "crates/cli", "gui/src-tauri"]
```

**Step 2: Create `gui/src-tauri/Cargo.toml`**

```toml
[package]
name = "gpx-overlay-gui"
version = "0.1.0"
edition = "2021"
description = "Desktop GUI for gpx-overlay"

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-dialog = "2"
serde = { workspace = true }
serde_json = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
tokio = { version = "1", features = ["full"] }
notify = "6"
activity = { path = "../../crates/activity" }
layout = { path = "../../crates/layout" }
render = { path = "../../crates/render" }

[features]
default = ["custom-protocol"]
custom-protocol = ["tauri/custom-protocol"]
```

**Step 3: Create `gui/src-tauri/build.rs`**

```rust
fn main() {
    tauri_build::build()
}
```

**Step 4: Create minimal `gui/src-tauri/src/main.rs`**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|_app| Ok(()))
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Step 5: Create `gui/src-tauri/tauri.conf.json`**

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "gpx-overlay",
  "version": "0.1.0",
  "identifier": "com.gpx-overlay.app",
  "build": {
    "frontendDist": "../build",
    "devUrl": "http://localhost:5173",
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build"
  },
  "app": {
    "windows": [
      {
        "title": "gpx-overlay",
        "width": 1280,
        "height": 800,
        "minWidth": 960,
        "minHeight": 600,
        "resizable": true
      }
    ],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ]
  }
}
```

**Step 6: Copy default Tauri icons**

Placeholder icons can come from `tauri-cli`'s init output or an existing Tauri project. For now, create an empty `gui/src-tauri/icons/` directory — bundle step will complain but `cargo build -p gpx-overlay-gui` should succeed without them.

**Step 7: Verify backend compiles**

Run: `cargo build -p gpx-overlay-gui`
Expected: compiles (may warn about missing icons; that's fine for dev builds — we can't bundle without them but we don't bundle yet).

**Step 8: Commit**

```bash
git add Cargo.toml gui/src-tauri/
git commit -m "Scaffold Tauri backend crate"
```

---

## Task 2: Initialize Svelte + TypeScript + Vite frontend

**Files:**
- Create: `gui/package.json`
- Create: `gui/vite.config.ts`
- Create: `gui/svelte.config.js`
- Create: `gui/tsconfig.json`
- Create: `gui/src/main.ts`
- Create: `gui/src/App.svelte`
- Create: `gui/src/app.css`
- Create: `gui/index.html`
- Create: `gui/.gitignore` (covers `node_modules`, `build`, `dist`)

**Step 1: Create `gui/package.json`**

```json
{
  "name": "gpx-overlay-gui",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "vite build",
    "preview": "vite preview",
    "check": "svelte-check --tsconfig ./tsconfig.json",
    "test": "vitest run"
  },
  "devDependencies": {
    "@sveltejs/vite-plugin-svelte": "^3.1.0",
    "@testing-library/svelte": "^5.2.0",
    "@tsconfig/svelte": "^5.0.0",
    "@types/node": "^20.0.0",
    "jsdom": "^24.0.0",
    "svelte": "^4.2.0",
    "svelte-check": "^3.8.0",
    "tslib": "^2.6.0",
    "typescript": "^5.5.0",
    "vite": "^5.4.0",
    "vitest": "^2.0.0"
  },
  "dependencies": {
    "@tauri-apps/api": "^2",
    "@tauri-apps/plugin-dialog": "^2"
  }
}
```

**Step 2: Create `gui/vite.config.ts`**

```ts
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    target: "es2021",
    outDir: "build",
    emptyOutDir: true,
  },
  test: {
    environment: "jsdom",
    globals: true,
  },
});
```

**Step 3: Create `gui/svelte.config.js`**

```js
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";
export default { preprocess: vitePreprocess() };
```

**Step 4: Create `gui/tsconfig.json`**

```json
{
  "extends": "@tsconfig/svelte/tsconfig.json",
  "compilerOptions": {
    "target": "ES2021",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "resolveJsonModule": true,
    "allowImportingTsExtensions": false,
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "types": ["vitest/globals"]
  },
  "include": ["src/**/*.ts", "src/**/*.svelte", "src/**/*.d.ts"]
}
```

**Step 5: Create `gui/index.html`**

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>gpx-overlay</title>
  </head>
  <body>
    <div id="app"></div>
    <script type="module" src="/src/main.ts"></script>
  </body>
</html>
```

**Step 6: Create `gui/src/main.ts`**

```ts
import "./app.css";
import App from "./App.svelte";

const app = new App({ target: document.getElementById("app")! });
export default app;
```

**Step 7: Create `gui/src/App.svelte`**

```svelte
<script lang="ts">
  let message = "gpx-overlay GUI";
</script>

<main>
  <h1>{message}</h1>
</main>

<style>
  main {
    padding: 2rem;
    font-family: system-ui, -apple-system, sans-serif;
  }
</style>
```

**Step 8: Create `gui/src/app.css`**

```css
html, body { margin: 0; padding: 0; height: 100%; background: #111; color: #eee; }
* { box-sizing: border-box; }
```

**Step 9: Create `gui/.gitignore`**

```
node_modules
build
dist
.vite
*.log
```

**Step 10: Install dependencies**

Run: `cd gui && npm install`
Expected: installs packages, creates `package-lock.json`.

**Step 11: Verify frontend builds**

Run: `cd gui && npm run build`
Expected: emits files to `gui/build/`.

**Step 12: Commit**

```bash
git add gui/package.json gui/package-lock.json gui/vite.config.ts gui/svelte.config.js gui/tsconfig.json gui/index.html gui/src/ gui/.gitignore
git commit -m "Scaffold Svelte + Vite frontend"
```

---

## Task 3: Wire Tauri to serve Vite; verify end-to-end window launch

**Files:**
- Modify: `gui/src-tauri/src/main.rs` — add a `hello_from_rust` command
- Modify: `gui/src/App.svelte` — call the command, display response

**Step 1: Add command to backend**

In `gui/src-tauri/src/main.rs`:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[tauri::command]
fn hello_from_rust() -> String {
    "Hello from Rust".into()
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![hello_from_rust])
        .setup(|_app| Ok(()))
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Step 2: Call from Svelte**

Replace `gui/src/App.svelte` body:

```svelte
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  let message = "loading…";
  onMount(async () => { message = await invoke<string>("hello_from_rust"); });
</script>

<main><h1>{message}</h1></main>
```

**Step 3: Launch the app**

Run (from repo root): `cargo tauri dev --config gui/src-tauri/tauri.conf.json`
*(Or: `cd gui && npx @tauri-apps/cli dev`.)*
Expected: Vite dev server starts on 5173, Tauri opens a window showing "Hello from Rust". Close the window to exit.

**Step 4: Commit**

```bash
git add gui/src-tauri/src/main.rs gui/src/App.svelte
git commit -m "Wire Tauri command -> Svelte invocation"
```

---

## Task 4: Add `--progress-json` flag to `gpx-overlay` CLI

The existing CLI uses `indicatif::ProgressBar` (overwrites a line, not parseable). Add a machine-readable progress mode that emits one JSON line per frame to stderr; the GUI's export pipeline will consume this.

**Files:**
- Modify: `crates/cli/src/args.rs` — add `progress_json: bool` field on `RenderArgs`
- Modify: `crates/cli/src/run.rs` — gate `ProgressBar`-based output on `!args.progress_json`; when enabled, emit `{"type":"progress","frame":N,"total":T}` per frame and `{"type":"done"}` at end. Errors go to `{"type":"error","message":"..."}`.
- Test: `crates/cli/tests/progress_json.rs` — spawn the binary on a tiny fixture, assert stderr contains `{"type":"progress"` and ends with `{"type":"done"}`.

**Step 1: Add the arg**

In `RenderArgs` (`crates/cli/src/args.rs`):
```rust
/// Emit one JSON line per progress event to stderr instead of a
/// drawn progress bar. Used by the GUI to stream progress.
#[arg(long, default_value_t = false)]
pub progress_json: bool,
```

**Step 2: Use a small enum for the progress sink**

In `crates/cli/src/run.rs`, replace direct `pb.inc(1)` calls with a sink abstraction. Introduce at the top of the module:

```rust
enum Progress {
    Bar(indicatif::ProgressBar),
    Json { total: u64 },
}

impl Progress {
    fn inc(&self, frame: u64) {
        match self {
            Progress::Bar(pb) => pb.inc(1),
            Progress::Json { total } => {
                eprintln!(r#"{{"type":"progress","frame":{},"total":{}}}"#, frame, total);
            }
        }
    }
    fn finish(&self) {
        match self {
            Progress::Bar(pb) => pb.finish_and_clear(),
            Progress::Json { .. } => eprintln!(r#"{{"type":"done"}}"#),
        }
    }
}
```

Replace `pb.inc(1)` with `progress.inc(frame_idx)` where `frame_idx` is the monotonically increasing count (track it in the flusher thread). Replace `pb.finish_*` with `progress.finish()`.

**Step 3: Build `Progress` based on flag**

```rust
let progress = if args.progress_json {
    Progress::Json { total }
} else {
    let pb = ProgressBar::new(total);
    pb.set_style(ProgressStyle::with_template("...").unwrap());
    Progress::Bar(pb)
};
```

**Step 4: Test**

Create `crates/cli/tests/progress_json.rs`:

```rust
use std::process::Command;

#[test]
fn emits_json_progress_lines() {
    // Assumes an example fixture exists; if not, skip this task's test and
    // rely on manual verification. (See crates/cli/tests for existing fixtures.)
    let output = Command::new(env!("CARGO_BIN_EXE_gpx-overlay"))
        .args([
            "render",
            "-i", "../../examples/sample.fit",
            "-l", "../../examples/layout-simple.json",
            "-o", "/tmp/test.mov",
            "--fps", "10",
            "--from", "0",
            "--to", "1",
            "--progress-json",
            "--dry-run",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(r#""type":"progress""#) || stderr.contains(r#""type":"done""#),
        "no JSON progress lines: {}",
        stderr
    );
}
```

Note: this test can be skipped if no fixture exists — verify manually with a real ride + layout. If a fixture is added later, turn this into a real integration test.

**Step 5: Verify**

Run: `cargo test -p gpx-overlay`
Expected: existing tests pass + new test (or skip) passes.

Manually: `cargo run -p gpx-overlay -- render -i <fit> -l <layout> -o /tmp/t.mov --from 0 --to 1 --progress-json` — should print JSON lines to stderr.

**Step 6: Commit**

```bash
git add crates/cli/src/args.rs crates/cli/src/run.rs crates/cli/tests/progress_json.rs
git commit -m "Add --progress-json flag to CLI for GUI consumption"
```

---

## Task 5: Backend session state module

**Files:**
- Create: `gui/src-tauri/src/session.rs`
- Modify: `gui/src-tauri/src/main.rs` — declare `mod session;`, register commands

**Step 1: Write the failing tests**

Create `gui/src-tauri/src/session.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SessionState {
    pub input_path: Option<PathBuf>,
    pub layout_path: Option<PathBuf>,
    pub output_path: Option<PathBuf>,
    pub codec: String,      // CLI's rename_all=snake_case names: prores_4444, h264, h264_nvenc, hevc_nvenc
    pub quality: u32,       // CRF for libx264/NVENC, qscale for ProRes
    pub chromakey: String,  // "#RRGGBB"
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
        assert_eq!(s.quality, 20); // default
    }

    #[test]
    fn unknown_fields_ignored() {
        // Forward-compat: newer session files shouldn't crash older binaries.
        let s = load_from_str(r#"{"codec":"prores_4444","future_field":42}"#);
        assert!(s.is_ok());
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p gpx-overlay-gui session::`
Expected: compile error (module not declared).

**Step 3: Wire into `main.rs`**

Add `mod session;` at the top of `gui/src-tauri/src/main.rs`.

**Step 4: Add Tauri commands for load/save**

Append to `session.rs`:

```rust
use std::fs;
use std::path::Path;

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
    if !path.exists() { return Ok(SessionState::default()); }
    let s = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    load_from_str(&s).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn session_save(app: tauri::AppHandle, state: SessionState) -> Result<(), String> {
    let path = session_path(&app).map_err(|e| e.to_string())?;
    let s = to_string(&state).map_err(|e| e.to_string())?;
    fs::write(&path, s).map_err(|e| e.to_string())
}
```

Add `use tauri::Manager;` at the top of `session.rs` so `app.path()` resolves.

Register in `main.rs`:
```rust
.invoke_handler(tauri::generate_handler![
    hello_from_rust,
    session::session_load,
    session::session_save,
])
```

**Step 5: Verify**

Run: `cargo test -p gpx-overlay-gui`
Expected: 3 session tests pass.

Run: `cargo build -p gpx-overlay-gui`
Expected: clean build.

**Step 6: Commit**

```bash
git add gui/src-tauri/src/session.rs gui/src-tauri/src/main.rs
git commit -m "Add session state module with load/save commands"
```

---

## Task 6: Binary resolution module (`binary.rs`)

Locate the `gpx-overlay` CLI binary and `ffmpeg`. Order: sibling-of-GUI-exe → PATH → user-configured override.

**Files:**
- Create: `gui/src-tauri/src/binary.rs`
- Modify: `gui/src-tauri/src/main.rs` — declare `mod binary;`, register commands

**Step 1: Write tests first**

Create `gui/src-tauri/src/binary.rs`:

```rust
use std::path::{Path, PathBuf};

pub fn sibling_of(exe_path: &Path, binary_name: &str) -> Option<PathBuf> {
    let candidate = exe_path.parent()?.join(binary_name);
    if candidate.exists() { Some(candidate) } else { None }
}

pub fn on_path(binary_name: &str) -> Option<PathBuf> {
    which::which(binary_name).ok()
}

/// Resolution order: override → sibling-of-exe → PATH.
pub fn resolve(
    override_path: Option<&Path>,
    exe_path: &Path,
    binary_name: &str,
) -> Option<PathBuf> {
    if let Some(p) = override_path {
        if p.exists() { return Some(p.to_path_buf()); }
    }
    if let Some(p) = sibling_of(exe_path, binary_name) {
        return Some(p);
    }
    on_path(binary_name)
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
    fn sibling_missing() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("gui.exe");
        File::create(&exe).unwrap();
        assert!(sibling_of(&exe, "nope.exe").is_none());
    }

    #[test]
    fn override_takes_precedence() {
        let dir = tempfile::tempdir().unwrap();
        let override_path = dir.path().join("custom.exe");
        File::create(&override_path).unwrap();
        // Sibling exists too but should be ignored
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
```

**Step 2: Add dependencies**

Add to `gui/src-tauri/Cargo.toml`:
```toml
which = "6"

[dev-dependencies]
tempfile = "3"
```

**Step 3: Add Tauri commands**

Append to `binary.rs`:
```rust
use std::process::Command;

#[tauri::command]
pub fn probe_ffmpeg(override_path: Option<PathBuf>) -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let binary_name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    let path = resolve(override_path.as_deref(), &exe, binary_name)
        .ok_or_else(|| format!("{} not found (checked sibling, PATH, override)", binary_name))?;
    // Confirm it actually runs
    let status = Command::new(&path)
        .arg("-version")
        .output()
        .map_err(|e| format!("failed to run {}: {}", path.display(), e))?;
    if !status.status.success() {
        return Err(format!("{} -version exited non-zero", path.display()));
    }
    Ok(path)
}

#[tauri::command]
pub fn probe_cli(override_path: Option<PathBuf>) -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let binary_name = if cfg!(windows) { "gpx-overlay.exe" } else { "gpx-overlay" };
    resolve(override_path.as_deref(), &exe, binary_name)
        .ok_or_else(|| format!("{} not found", binary_name))
}
```

**Step 4: Register commands**

In `main.rs`:
```rust
.invoke_handler(tauri::generate_handler![
    hello_from_rust,
    session::session_load,
    session::session_save,
    binary::probe_ffmpeg,
    binary::probe_cli,
])
```

**Step 5: Verify**

Run: `cargo test -p gpx-overlay-gui binary::`
Expected: all 3 tests pass.

**Step 6: Commit**

```bash
git add gui/src-tauri/
git commit -m "Add binary resolution + ffmpeg/CLI probe commands"
```

---

## Task 7: Backend progress parser (`progress.rs`)

Parse JSON lines emitted by the CLI (per Task 4). Produce `ProgressEvent` enum for the export pipeline to forward as Tauri events.

**Files:**
- Create: `gui/src-tauri/src/progress.rs`
- Modify: `gui/src-tauri/src/main.rs` — declare `mod progress;`

**Step 1: Write failing tests**

Create `gui/src-tauri/src/progress.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProgressLine {
    Progress { frame: u64, total: u64 },
    Done,
    Error { message: String },
}

/// Parse one stderr line. Returns `None` for non-JSON lines (e.g. warnings,
/// ffmpeg chatter when CLI pipes it). Returns `Some(Err(...))` only when the
/// line looks like JSON but fails to parse a known shape.
pub fn parse_line(line: &str) -> Option<ProgressLine> {
    let trimmed = line.trim();
    if !trimmed.starts_with('{') { return None; }
    serde_json::from_str::<ProgressLine>(trimmed).ok()
}

pub fn eta_seconds(frame: u64, total: u64, elapsed_secs: f64) -> Option<f64> {
    if frame == 0 || frame >= total { return None; }
    let rate = frame as f64 / elapsed_secs;
    if rate <= 0.0 { return None; }
    Some((total - frame) as f64 / rate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_progress() {
        let p = parse_line(r#"{"type":"progress","frame":42,"total":900}"#).unwrap();
        assert_eq!(p, ProgressLine::Progress { frame: 42, total: 900 });
    }

    #[test]
    fn parses_done() {
        assert_eq!(parse_line(r#"{"type":"done"}"#).unwrap(), ProgressLine::Done);
    }

    #[test]
    fn parses_error() {
        let e = parse_line(r#"{"type":"error","message":"boom"}"#).unwrap();
        assert_eq!(e, ProgressLine::Error { message: "boom".into() });
    }

    #[test]
    fn non_json_ignored() {
        assert!(parse_line("warning: widget 'x' missing metric").is_none());
        assert!(parse_line("").is_none());
    }

    #[test]
    fn eta_math() {
        // 450 frames done of 900, 10s elapsed → rate 45fps → 450/45 = 10s ETA
        assert_eq!(eta_seconds(450, 900, 10.0).unwrap(), 10.0);
    }

    #[test]
    fn eta_none_at_start_or_end() {
        assert!(eta_seconds(0, 900, 0.0).is_none());
        assert!(eta_seconds(900, 900, 10.0).is_none());
    }
}
```

**Step 2: Declare module**

Add `mod progress;` to `main.rs`.

**Step 3: Verify**

Run: `cargo test -p gpx-overlay-gui progress::`
Expected: 6 tests pass.

**Step 4: Commit**

```bash
git add gui/src-tauri/src/progress.rs gui/src-tauri/src/main.rs
git commit -m "Add progress line parser"
```

---

## Task 8: Backend state + load commands for Activity and Layout

Backend owns the parsed `Activity` and `Layout` for the duration of the session. Commands: `load_activity`, `load_layout`, each mutate state and return summary info (e.g. activity duration) to the frontend.

**Files:**
- Create: `gui/src-tauri/src/state.rs`
- Modify: `gui/src-tauri/src/main.rs` — register `.manage(AppState::default())` and commands

**Step 1: Design `AppState`**

```rust
use std::path::PathBuf;
use std::sync::Mutex;
use activity::Activity;
use layout::Layout;
use serde::Serialize;

#[derive(Default)]
pub struct AppState {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    pub activity: Option<Activity>,
    pub activity_path: Option<PathBuf>,
    pub layout: Option<Layout>,
    pub layout_path: Option<PathBuf>,
}

impl AppState {
    pub fn with_activity<R>(&self, f: impl FnOnce(&Activity) -> R) -> Option<R> {
        self.inner.lock().ok()?.activity.as_ref().map(f)
    }
    pub fn with_layout<R>(&self, f: impl FnOnce(&Layout) -> R) -> Option<R> {
        self.inner.lock().ok()?.layout.as_ref().map(f)
    }
    pub fn set_activity(&self, a: Activity, p: PathBuf) {
        let mut g = self.inner.lock().unwrap();
        g.activity = Some(a);
        g.activity_path = Some(p);
    }
    pub fn set_layout(&self, l: Layout, p: PathBuf) {
        let mut g = self.inner.lock().unwrap();
        g.layout = Some(l);
        g.layout_path = Some(p);
    }
    pub fn layout_path(&self) -> Option<PathBuf> {
        self.inner.lock().ok()?.layout_path.clone()
    }
}

#[derive(Serialize)]
pub struct ActivityInfo {
    pub duration_seconds: f64,
    pub sample_count: usize,
    pub metrics_present: Vec<String>,
}

#[derive(Serialize)]
pub struct LayoutInfo {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub widget_count: usize,
    pub warnings: Vec<String>,
}
```

**Step 2: Add commands**

```rust
#[tauri::command]
pub fn load_activity(
    state: tauri::State<AppState>,
    path: PathBuf,
) -> Result<ActivityInfo, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    let mut activity = match ext.as_deref() {
        Some("fit") => activity::load_fit(&path).map_err(|e| e.to_string())?,
        Some("gpx") => activity::load_gpx(&path).map_err(|e| e.to_string())?,
        _ => return Err("unsupported file type (expected .fit or .gpx)".into()),
    };
    activity.prepare();
    let duration = activity
        .samples
        .last()
        .map(|s| s.t.as_secs_f64())
        .unwrap_or(0.0);
    let sample_count = activity.samples.len();
    let metrics_present: Vec<String> = activity::Metric::ALL
        .iter()
        .filter(|m| activity::metric_present_on_activity(**m, &activity))
        .map(|m| m.as_str().to_string())
        .collect();
    state.set_activity(activity, path);
    Ok(ActivityInfo { duration_seconds: duration, sample_count, metrics_present })
}

#[tauri::command]
pub fn load_layout(
    state: tauri::State<AppState>,
    path: PathBuf,
) -> Result<LayoutInfo, String> {
    let s = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let layout: Layout = serde_json::from_str(&s).map_err(|e| format!("parse error: {}", e))?;
    let info = LayoutInfo {
        width: layout.canvas.width,
        height: layout.canvas.height,
        fps: layout.canvas.fps,
        widget_count: layout.widgets.len(),
        warnings: vec![], // TODO: run validate if activity loaded
    };
    state.set_layout(layout, path);
    Ok(info)
}
```

**Note**: `activity::Metric::ALL` and `Metric::as_str` may not exist — inspect `crates/activity/src/metric.rs` and adjust (e.g. hardcode a list, or add an `ALL` const if missing). Keep the change minimal.

**Step 3: Wire into main.rs**

```rust
mod state;
// ...
tauri::Builder::default()
    .plugin(tauri_plugin_dialog::init())
    .manage(state::AppState::default())
    .invoke_handler(tauri::generate_handler![
        hello_from_rust,
        session::session_load,
        session::session_save,
        binary::probe_ffmpeg,
        binary::probe_cli,
        state::load_activity,
        state::load_layout,
    ])
    .run(tauri::generate_context!())
    .expect("...");
```

**Step 4: Verify**

Run: `cargo build -p gpx-overlay-gui`
Expected: clean build.

**Step 5: Commit**

```bash
git add gui/src-tauri/src/state.rs gui/src-tauri/src/main.rs
git commit -m "Add backend state + load_activity/load_layout commands"
```

---

## Task 9: `preview_frame` command — render single frame to PNG data URL

**Files:**
- Create: `gui/src-tauri/src/preview.rs`
- Modify: `gui/src-tauri/src/main.rs` — register `preview_frame`, register `TextCtx` as managed state
- Modify: `gui/src-tauri/Cargo.toml` — add `png` dep (or use `tiny-skia`'s `encode_png`)

**Design note:** We keep a single `TextCtx` in managed state (not one per thread — preview is single-threaded). A `Mutex<TextCtx>` is fine since only one preview renders at a time.

**Step 1: Add PNG encoding dep**

In `gui/src-tauri/Cargo.toml`:
```toml
tiny-skia = "0.11"
base64 = "0.22"
```

(tiny-skia's `Pixmap::encode_png` returns a Vec<u8> — no separate `png` crate needed.)

**Step 2: Write the module**

Create `gui/src-tauri/src/preview.rs`:

```rust
use base64::Engine;
use std::time::Duration;
use tauri::State;
use tiny_skia::{Color, Pixmap};

use crate::state::AppState;

pub struct TextState(pub std::sync::Mutex<render::TextCtx>);

impl Default for TextState {
    fn default() -> Self { Self(std::sync::Mutex::new(render::TextCtx::new())) }
}

#[tauri::command]
pub fn preview_frame(
    app_state: State<AppState>,
    text_state: State<TextState>,
    t_seconds: f64,
    downscale_width: Option<u32>,
) -> Result<String, String> {
    let inner = app_state
        .with_layout(|l| (l.canvas.width, l.canvas.height))
        .ok_or("no layout loaded")?;
    let (src_w, src_h) = inner;
    let png_bytes = app_state
        .with_activity(|_a| ())
        .ok_or("no activity loaded")?;
    let _ = png_bytes; // suppress warning; existence-check only

    // Render full-res into a scratch pixmap.
    let mut pixmap = Pixmap::new(src_w, src_h).ok_or("pixmap alloc failed")?;
    let mut text = text_state.0.lock().map_err(|e| e.to_string())?;

    // Borrow layout + activity for the render. Need to lock AppState once.
    // AppState's current API returns owned clones; use a dedicated lock path.
    app_state
        .with_both(|layout, activity| {
            render::render_frame(
                layout,
                activity,
                Duration::from_secs_f64(t_seconds.max(0.0)),
                &mut text,
                &mut pixmap,
                Color::TRANSPARENT,
            )
        })
        .ok_or("no activity/layout loaded")?
        .map_err(|e| e.to_string())?;

    // If downscale requested, CPU-scale with tiny_skia's transform.
    let final_png = if let Some(dw) = downscale_width {
        if dw < src_w {
            let scale = dw as f32 / src_w as f32;
            let dh = (src_h as f32 * scale).round() as u32;
            let mut small = Pixmap::new(dw, dh).ok_or("pixmap alloc failed")?;
            small.draw_pixmap(
                0, 0,
                pixmap.as_ref(),
                &tiny_skia::PixmapPaint::default(),
                tiny_skia::Transform::from_scale(scale, scale),
                None,
            );
            small.encode_png().map_err(|e| e.to_string())?
        } else {
            pixmap.encode_png().map_err(|e| e.to_string())?
        }
    } else {
        pixmap.encode_png().map_err(|e| e.to_string())?
    };

    let b64 = base64::engine::general_purpose::STANDARD.encode(&final_png);
    Ok(format!("data:image/png;base64,{}", b64))
}
```

**Step 3: Extend `AppState` with a combined accessor**

In `state.rs`, add:

```rust
impl AppState {
    pub fn with_both<R>(&self, f: impl FnOnce(&Layout, &Activity) -> R) -> Option<R> {
        let g = self.inner.lock().ok()?;
        let l = g.layout.as_ref()?;
        let a = g.activity.as_ref()?;
        Some(f(l, a))
    }
}
```

**Step 4: Register in main.rs**

```rust
mod preview;
// ...
.manage(state::AppState::default())
.manage(preview::TextState::default())
.invoke_handler(tauri::generate_handler![
    hello_from_rust,
    session::session_load,
    session::session_save,
    binary::probe_ffmpeg,
    binary::probe_cli,
    state::load_activity,
    state::load_layout,
    preview::preview_frame,
])
```

**Step 5: Verify**

Run: `cargo build -p gpx-overlay-gui`
Expected: clean build.

**Step 6: Commit**

```bash
git add gui/src-tauri/src/preview.rs gui/src-tauri/src/state.rs gui/src-tauri/src/main.rs gui/src-tauri/Cargo.toml
git commit -m "Add preview_frame command returning PNG data URL"
```

---

## Task 10: Layout file watcher (`watcher.rs`)

Watch the currently-loaded layout file with `notify`; on modify, re-parse and emit a Tauri event (`layout-reloaded` on success, `layout-error` on parse failure).

**Files:**
- Create: `gui/src-tauri/src/watcher.rs`
- Modify: `gui/src-tauri/src/main.rs` — register the watcher setup and the `watch_layout` command

**Step 1: Implement the watcher**

Create `gui/src-tauri/src/watcher.rs`:

```rust
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;

pub struct WatcherHandle(pub Mutex<Option<RecommendedWatcher>>);
impl Default for WatcherHandle {
    fn default() -> Self { Self(Mutex::new(None)) }
}

const DEBOUNCE: Duration = Duration::from_millis(150);

#[tauri::command]
pub fn watch_layout(
    app: AppHandle,
    handle: tauri::State<WatcherHandle>,
    path: PathBuf,
) -> Result<(), String> {
    use std::sync::mpsc::channel;
    let (tx, rx) = channel::<notify::Result<Event>>();
    let mut w = notify::recommended_watcher(tx).map_err(|e| e.to_string())?;
    w.watch(&path, RecursiveMode::NonRecursive)
        .map_err(|e| e.to_string())?;
    *handle.0.lock().unwrap() = Some(w);

    let app_clone = app.clone();
    let path_clone = path.clone();
    std::thread::spawn(move || {
        let mut last_fire: Option<Instant> = None;
        while let Ok(res) = rx.recv() {
            let Ok(ev) = res else { continue };
            if !matches!(ev.kind, EventKind::Modify(_) | EventKind::Create(_)) { continue; }
            if let Some(t) = last_fire {
                if t.elapsed() < DEBOUNCE { continue; }
            }
            last_fire = Some(Instant::now());
            // Brief pause to allow editors doing rename-replace to finish
            std::thread::sleep(DEBOUNCE);
            // Re-parse
            match std::fs::read_to_string(&path_clone)
                .map_err(|e| e.to_string())
                .and_then(|s| serde_json::from_str::<layout::Layout>(&s).map_err(|e| e.to_string()))
            {
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

#[tauri::command]
pub fn unwatch_layout(handle: tauri::State<WatcherHandle>) {
    *handle.0.lock().unwrap() = None;
}
```

**Step 2: Register**

In `main.rs`:
```rust
mod watcher;
// ...
.manage(watcher::WatcherHandle::default())
// ... add watch_layout, unwatch_layout to generate_handler![]
```

**Step 3: Verify**

Run: `cargo build -p gpx-overlay-gui`
Expected: clean build.

Manual: not unit-testable without tauri context. Verified in integration later.

**Step 4: Commit**

```bash
git add gui/src-tauri/src/watcher.rs gui/src-tauri/src/main.rs
git commit -m "Add notify-based layout file watcher"
```

---

## Task 11: Export command — spawn CLI, stream progress

Spawn `gpx-overlay render --progress-json ...` as a subprocess; stream stderr line-by-line; emit Tauri events.

**Files:**
- Create: `gui/src-tauri/src/export.rs`
- Modify: `gui/src-tauri/src/main.rs`

**Step 1: Implement export module**

Create `gui/src-tauri/src/export.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Mutex;
use std::time::Instant;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

use crate::progress::{parse_line, ProgressLine};

#[derive(Deserialize)]
pub struct ExportArgs {
    pub cli_path: PathBuf,
    pub input: PathBuf,
    pub layout: PathBuf,
    pub output: PathBuf,
    pub codec: String,
    pub quality: u32,
    pub chromakey: String,
    pub from_seconds: f64,
    pub to_seconds: f64,
}

#[derive(Serialize, Clone)]
pub struct ProgressPayload { pub frame: u64, pub total: u64, pub fps: f64, pub eta_seconds: Option<f64> }

#[derive(Serialize, Clone)]
pub struct LogPayload { pub line: String, pub stream: &'static str }

#[derive(Serialize, Clone)]
pub struct DonePayload { pub status: String, pub message: Option<String> }

pub struct ExportHandle(pub Mutex<Option<Child>>);
impl Default for ExportHandle { fn default() -> Self { Self(Mutex::new(None)) } }

fn fmt_time(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    format!("{:02}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
}

#[tauri::command]
pub async fn start_export(
    app: AppHandle,
    handle: tauri::State<'_, ExportHandle>,
    args: ExportArgs,
) -> Result<(), String> {
    // Build CLI argv
    let mut cmd = Command::new(&args.cli_path);
    cmd.arg("render")
        .arg("-i").arg(&args.input)
        .arg("-l").arg(&args.layout)
        .arg("-o").arg(&args.output)
        .arg("--codec").arg(&args.codec)
        .arg("--crf").arg(args.quality.to_string())  // NVENC/libx264
        .arg("--qscale").arg(args.quality.to_string()) // prores
        .arg("--chromakey").arg(&args.chromakey)
        .arg("--from").arg(fmt_time(args.from_seconds))
        .arg("--to").arg(fmt_time(args.to_seconds))
        .arg("--progress-json")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let stderr = child.stderr.take().ok_or("no stderr")?;
    *handle.0.lock().unwrap() = Some(child);

    let app_clone = app.clone();
    let handle_ref = app.state::<ExportHandle>();
    tauri::async_runtime::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        let started = Instant::now();
        while let Ok(Some(line)) = reader.next_line().await {
            match parse_line(&line) {
                Some(ProgressLine::Progress { frame, total }) => {
                    let elapsed = started.elapsed().as_secs_f64();
                    let fps = if elapsed > 0.0 { frame as f64 / elapsed } else { 0.0 };
                    let eta = crate::progress::eta_seconds(frame, total, elapsed);
                    let _ = app_clone.emit("export-progress", ProgressPayload { frame, total, fps, eta_seconds: eta });
                }
                Some(ProgressLine::Done) => {
                    let _ = app_clone.emit("export-done", DonePayload { status: "success".into(), message: None });
                }
                Some(ProgressLine::Error { message }) => {
                    let _ = app_clone.emit("export-done", DonePayload { status: "error".into(), message: Some(message) });
                }
                None => {
                    let _ = app_clone.emit("export-log", LogPayload { line, stream: "stderr" });
                }
            }
        }
        // Reap exit status
        if let Some(mut child) = app_clone.state::<ExportHandle>().0.lock().unwrap().take() {
            if let Ok(status) = child.wait().await {
                if !status.success() {
                    let _ = app_clone.emit("export-done", DonePayload {
                        status: "error".into(),
                        message: Some(format!("exited with code {:?}", status.code())),
                    });
                }
            }
        }
        drop(handle_ref);
    });
    Ok(())
}

#[cfg(windows)]
fn kill_tree(child: &mut Child) {
    if let Some(id) = child.id() {
        let _ = std::process::Command::new("taskkill")
            .args(["/F", "/T", "/PID", &id.to_string()])
            .output();
    }
}

#[cfg(not(windows))]
fn kill_tree(child: &mut Child) {
    // On Unix, CLI's SIGTERM handler should clean up ffmpeg child
    if let Some(id) = child.id() {
        unsafe { libc::kill(id as i32, libc::SIGTERM); }
    }
}

#[tauri::command]
pub fn cancel_export(
    app: AppHandle,
    handle: tauri::State<ExportHandle>,
) -> Result<(), String> {
    if let Some(mut child) = handle.0.lock().unwrap().take() {
        kill_tree(&mut child);
        let _ = app.emit("export-done", DonePayload { status: "canceled".into(), message: None });
    }
    Ok(())
}
```

**Step 2: Add unix libc dep**

In `gui/src-tauri/Cargo.toml`:
```toml
[target.'cfg(unix)'.dependencies]
libc = "0.2"
```

**Step 3: Register**

In `main.rs`:
```rust
mod export;
// ...
.manage(export::ExportHandle::default())
// ... add start_export, cancel_export to generate_handler![]
```

**Step 4: Verify**

Run: `cargo build -p gpx-overlay-gui`
Expected: clean build.

**Step 5: Commit**

```bash
git add gui/src-tauri/
git commit -m "Add export spawn + cancel commands"
```

---

## Task 12: Frontend stores + Tauri bindings layer

**Files:**
- Create: `gui/src/lib/stores.ts`
- Create: `gui/src/lib/tauri.ts` (typed wrappers for backend commands + event subscriptions)
- Create: `gui/src/lib/types.ts`

**Step 1: Types mirroring backend**

`gui/src/lib/types.ts`:
```ts
export interface SessionState {
  input_path: string | null;
  layout_path: string | null;
  output_path: string | null;
  codec: string;
  quality: number;
  chromakey: string;
  from_seconds: number;
  to_seconds: number | null;
  cli_path_override: string | null;
}
export interface ActivityInfo {
  duration_seconds: number;
  sample_count: number;
  metrics_present: string[];
}
export interface LayoutInfo {
  width: number;
  height: number;
  fps: number;
  widget_count: number;
  warnings: string[];
}
export interface ProgressPayload { frame: number; total: number; fps: number; eta_seconds: number | null; }
export interface LogPayload { line: string; stream: string; }
export interface DonePayload { status: "success" | "canceled" | "error"; message: string | null; }
```

**Step 2: Typed command wrappers**

`gui/src/lib/tauri.ts`:
```ts
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { SessionState, ActivityInfo, LayoutInfo, ProgressPayload, LogPayload, DonePayload } from "./types";

export const sessionLoad = () => invoke<SessionState>("session_load");
export const sessionSave = (state: SessionState) => invoke<void>("session_save", { state });
export const probeFfmpeg = (overridePath?: string) => invoke<string>("probe_ffmpeg", { overridePath: overridePath ?? null });
export const probeCli = (overridePath?: string) => invoke<string>("probe_cli", { overridePath: overridePath ?? null });
export const loadActivity = (path: string) => invoke<ActivityInfo>("load_activity", { path });
export const loadLayout = (path: string) => invoke<LayoutInfo>("load_layout", { path });
export const previewFrame = (tSeconds: number, downscaleWidth?: number) =>
  invoke<string>("preview_frame", { tSeconds, downscaleWidth: downscaleWidth ?? null });
export const watchLayout = (path: string) => invoke<void>("watch_layout", { path });
export const unwatchLayout = () => invoke<void>("unwatch_layout");
export const startExport = (args: unknown) => invoke<void>("start_export", { args });
export const cancelExport = () => invoke<void>("cancel_export");

export const onLayoutReloaded = (fn: () => void): Promise<UnlistenFn> => listen("layout-reloaded", fn);
export const onLayoutError = (fn: (msg: string) => void): Promise<UnlistenFn> =>
  listen<string>("layout-error", (e) => fn(e.payload));
export const onExportProgress = (fn: (p: ProgressPayload) => void): Promise<UnlistenFn> =>
  listen<ProgressPayload>("export-progress", (e) => fn(e.payload));
export const onExportLog = (fn: (p: LogPayload) => void): Promise<UnlistenFn> =>
  listen<LogPayload>("export-log", (e) => fn(e.payload));
export const onExportDone = (fn: (p: DonePayload) => void): Promise<UnlistenFn> =>
  listen<DonePayload>("export-done", (e) => fn(e.payload));
```

**Step 3: Writable stores with persistence**

`gui/src/lib/stores.ts`:
```ts
import { writable, type Writable } from "svelte/store";
import type { SessionState, ActivityInfo, LayoutInfo, ProgressPayload } from "./types";
import { sessionSave } from "./tauri";

const defaultSession: SessionState = {
  input_path: null, layout_path: null, output_path: null,
  codec: "prores_4444", quality: 20, chromakey: "#00ff00",
  from_seconds: 0, to_seconds: null, cli_path_override: null,
};

export const session: Writable<SessionState> = writable(defaultSession);
export const activityInfo: Writable<ActivityInfo | null> = writable(null);
export const layoutInfo: Writable<LayoutInfo | null> = writable(null);

export const previewT: Writable<number> = writable(0);
export const previewImage: Writable<string | null> = writable(null);
export const previewBusy: Writable<boolean> = writable(false);

export type ExportStatus = "idle" | "running" | "success" | "canceled" | "error";
export const exportStatus: Writable<ExportStatus> = writable("idle");
export const exportProgress: Writable<ProgressPayload | null> = writable(null);
export const exportLog: Writable<string[]> = writable([]);

// Persist session changes (debounced ~500ms)
let saveTimer: number | undefined;
session.subscribe((s) => {
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => { sessionSave(s).catch(console.error); }, 500) as unknown as number;
});
```

**Step 4: Verify**

Run: `cd gui && npm run check`
Expected: 0 errors.

**Step 5: Commit**

```bash
git add gui/src/lib/
git commit -m "Add Svelte stores + typed Tauri bindings"
```

---

## Task 13: App shell layout with sidebar, preview, seekbar, footer

**Files:**
- Modify: `gui/src/App.svelte` — compose main layout + stub children
- Create: `gui/src/components/PreviewPane.svelte` (placeholder)
- Create: `gui/src/components/Seekbar.svelte` (placeholder)
- Create: `gui/src/components/Sidebar.svelte` (placeholder)
- Create: `gui/src/components/ExportFooter.svelte` (placeholder)

**Step 1: Placeholder components**

Each placeholder is a `<section>` with a temporary label, e.g. `Sidebar.svelte`:
```svelte
<aside class="sidebar"><h2>Sidebar</h2></aside>
<style>.sidebar{padding:1rem;background:#222;height:100%;}</style>
```

**Step 2: App shell grid**

`gui/src/App.svelte`:
```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import PreviewPane from "./components/PreviewPane.svelte";
  import Seekbar from "./components/Seekbar.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import ExportFooter from "./components/ExportFooter.svelte";
  import { sessionLoad } from "./lib/tauri";
  import { session } from "./lib/stores";

  onMount(async () => {
    try { session.set(await sessionLoad()); } catch (e) { console.error(e); }
  });
</script>

<div class="app">
  <main class="main">
    <PreviewPane />
    <Seekbar />
  </main>
  <Sidebar />
  <ExportFooter />
</div>

<style>
  .app {
    display: grid;
    grid-template-columns: 1fr 320px;
    grid-template-rows: 1fr auto;
    grid-template-areas:
      "main sidebar"
      "footer footer";
    height: 100vh;
  }
  .main { grid-area: main; display: flex; flex-direction: column; min-width: 0; min-height: 0; }
  :global(.sidebar) { grid-area: sidebar; overflow-y: auto; }
  :global(.footer) { grid-area: footer; }
</style>
```

Add matching `class="sidebar"` / `class="footer"` inside placeholder components.

**Step 3: Verify**

Run: `cd gui && npm run dev` (or `cargo tauri dev`)
Expected: window shows four labeled regions.

**Step 4: Commit**

```bash
git add gui/src/
git commit -m "Add app shell layout with placeholder components"
```

---

## Task 14: PreviewPane component with checkerboard background

**Files:**
- Modify: `gui/src/components/PreviewPane.svelte`

**Step 1: Component**

```svelte
<script lang="ts">
  import { previewImage, previewBusy } from "../lib/stores";
</script>

<section class="preview" class:busy={$previewBusy}>
  {#if $previewImage}
    <img src={$previewImage} alt="Preview frame" />
  {:else}
    <div class="empty">Load an activity and layout to see a preview.</div>
  {/if}
</section>

<style>
  .preview {
    flex: 1 1 auto;
    min-height: 0;
    display: flex;
    justify-content: center;
    align-items: center;
    background-color: #333;
    background-image:
      linear-gradient(45deg, #444 25%, transparent 25%),
      linear-gradient(-45deg, #444 25%, transparent 25%),
      linear-gradient(45deg, transparent 75%, #444 75%),
      linear-gradient(-45deg, transparent 75%, #444 75%);
    background-size: 20px 20px;
    background-position: 0 0, 0 10px, 10px -10px, -10px 0px;
  }
  .preview img { max-width: 100%; max-height: 100%; object-fit: contain; }
  .empty { color: #888; }
  .busy { opacity: 0.8; }
</style>
```

**Step 2: Verify**

Run: `cd gui && npm run check`
Expected: 0 errors.

**Step 3: Commit**

```bash
git add gui/src/components/PreviewPane.svelte
git commit -m "Implement PreviewPane with checkerboard bg"
```

---

## Task 15: Seekbar component with throttled scrub + latest-wins preview

**Files:**
- Modify: `gui/src/components/Seekbar.svelte`
- Create: `gui/src/lib/preview-dispatcher.ts` — monotonic-id scrub dispatcher
- Test: `gui/src/lib/preview-dispatcher.test.ts`

**Step 1: Dispatcher with latest-wins semantics**

`gui/src/lib/preview-dispatcher.ts`:
```ts
import { previewFrame } from "./tauri";
import { previewImage, previewBusy } from "./stores";

let nextId = 0;
let lastReceived = -1;

export async function requestPreview(t: number, downscaleWidth?: number) {
  const id = nextId++;
  previewBusy.set(true);
  try {
    const url = await previewFrame(t, downscaleWidth);
    if (id > lastReceived) {
      lastReceived = id;
      previewImage.set(url);
    }
  } catch (e) { console.error(e); }
  finally { previewBusy.set(false); }
}

// test hook
export function __reset() { nextId = 0; lastReceived = -1; }
```

**Step 2: Test**

`gui/src/lib/preview-dispatcher.test.ts`:
```ts
import { describe, it, expect, vi, beforeEach } from "vitest";
vi.mock("./tauri", () => ({ previewFrame: vi.fn() }));
vi.mock("./stores", () => ({
  previewImage: { set: vi.fn() },
  previewBusy: { set: vi.fn() },
}));

import { requestPreview, __reset } from "./preview-dispatcher";
import { previewFrame } from "./tauri";
import { previewImage } from "./stores";

describe("preview-dispatcher latest-wins", () => {
  beforeEach(() => { __reset(); vi.clearAllMocks(); });

  it("applies responses in order when they arrive in order", async () => {
    (previewFrame as any)
      .mockResolvedValueOnce("A")
      .mockResolvedValueOnce("B");
    await requestPreview(1);
    await requestPreview(2);
    expect(previewImage.set).toHaveBeenLastCalledWith("B");
  });

  it("drops stale responses", async () => {
    let resolveA: (v: string) => void = () => {};
    (previewFrame as any)
      .mockImplementationOnce(() => new Promise<string>((r) => { resolveA = r; }))
      .mockResolvedValueOnce("B");
    const a = requestPreview(1);
    const b = requestPreview(2);
    await b;                         // B resolves, id=1
    resolveA("A_STALE");              // A resolves, id=0 → should be dropped
    await a;
    // last call to set should still be B
    const calls = (previewImage.set as any).mock.calls.map((c: any) => c[0]);
    expect(calls[calls.length - 1]).toBe("B");
  });
});
```

**Step 3: Seekbar component**

`gui/src/components/Seekbar.svelte`:
```svelte
<script lang="ts">
  import { activityInfo, previewT, session } from "../lib/stores";
  import { requestPreview } from "../lib/preview-dispatcher";

  let duration = 0;
  $: duration = $activityInfo?.duration_seconds ?? 0;

  let dragging = false;
  let previewWidth = 800; // updated by parent via ResizeObserver if desired

  let lastTick = 0;
  const THROTTLE_MS = 67; // ~15fps

  function fmt(s: number) {
    const t = Math.max(0, Math.floor(s));
    return `${String(Math.floor(t/3600)).padStart(2,"0")}:${String(Math.floor(t/60)%60).padStart(2,"0")}:${String(t%60).padStart(2,"0")}`;
  }

  function onInput(e: Event) {
    const v = Number((e.target as HTMLInputElement).value);
    $previewT = v;
    if (!dragging) return;
    const now = performance.now();
    if (now - lastTick < THROTTLE_MS) return;
    lastTick = now;
    requestPreview(v, previewWidth);
  }
  function onDown() { dragging = true; }
  function onUp() {
    if (!dragging) return;
    dragging = false;
    requestPreview($previewT); // full-res
  }
</script>

<section class="seekbar">
  <input
    type="range" min="0" max={duration} step="0.1"
    value={$previewT}
    on:input={onInput}
    on:mousedown={onDown}
    on:mouseup={onUp}
    on:touchstart={onDown}
    on:touchend={onUp}
    disabled={!$activityInfo}
  />
  <div class="labels">
    <span>{fmt($previewT)}</span>
    <span>{fmt(duration)}</span>
  </div>
</section>

<style>
  .seekbar { padding: 0.5rem 1rem; background: #1a1a1a; }
  .seekbar input { width: 100%; }
  .labels { display: flex; justify-content: space-between; font-size: 0.85rem; color: #aaa; }
</style>
```

**Step 4: Verify**

Run: `cd gui && npm test`
Expected: 2 dispatcher tests pass.

Run: `cd gui && npm run check`
Expected: 0 errors.

**Step 5: Commit**

```bash
git add gui/src/
git commit -m "Add Seekbar with throttled scrub + latest-wins preview"
```

---

## Task 16: Sidebar file pickers; auto-load and auto-fill from/to

**Files:**
- Modify: `gui/src/components/Sidebar.svelte`

**Step 1: Component**

```svelte
<script lang="ts">
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { session, activityInfo, layoutInfo, previewT } from "../lib/stores";
  import { loadActivity, loadLayout, watchLayout, previewFrame } from "../lib/tauri";
  import { previewImage } from "../lib/stores";

  async function pickInput() {
    const path = await open({ filters: [{ name: "Activity", extensions: ["fit", "gpx"] }] });
    if (typeof path !== "string") return;
    try {
      const info = await loadActivity(path);
      $activityInfo = info;
      $session = { ...$session, input_path: path, from_seconds: 0, to_seconds: info.duration_seconds };
      $previewT = 0;
      const url = await previewFrame(0);
      $previewImage = url;
    } catch (e) { console.error(e); }
  }

  async function pickLayout() {
    const path = await open({ filters: [{ name: "Layout JSON", extensions: ["json"] }] });
    if (typeof path !== "string") return;
    try {
      const info = await loadLayout(path);
      $layoutInfo = info;
      $session = { ...$session, layout_path: path };
      await watchLayout(path);
      if ($activityInfo) {
        $previewImage = await previewFrame($previewT);
      }
    } catch (e) { console.error(e); }
  }

  async function pickOutput() {
    const path = await save({ filters: [{ name: "Video", extensions: ["mov", "mp4", "mkv"] }] });
    if (typeof path === "string") $session = { ...$session, output_path: path };
  }
</script>

<aside class="sidebar">
  <div class="row">
    <label>Input</label>
    <button on:click={pickInput}>Browse…</button>
    <div class="path">{$session.input_path ?? "—"}</div>
  </div>

  <div class="row">
    <label>Layout</label>
    <button on:click={pickLayout}>Browse…</button>
    <div class="path">{$session.layout_path ?? "—"}</div>
  </div>

  <div class="row">
    <label>Output</label>
    <button on:click={pickOutput}>Browse…</button>
    <div class="path">{$session.output_path ?? "—"}</div>
  </div>

  <!-- Codec selector, quality, chromakey, from/to, Export button added in Task 17/18 -->
</aside>

<style>
  .sidebar { padding: 1rem; background: #222; height: 100%; }
  .row { display: flex; flex-direction: column; margin-bottom: 1rem; gap: 0.25rem; }
  .path { font-size: 0.85rem; color: #888; word-break: break-all; }
  button { padding: 0.3rem 0.8rem; background: #333; color: #eee; border: 1px solid #444; cursor: pointer; }
</style>
```

**Step 2: Verify**

Run: `cargo tauri dev`
Expected: picking an activity file loads it and shows a preview at t=0; picking a layout also triggers preview.

**Step 3: Commit**

```bash
git add gui/src/components/Sidebar.svelte
git commit -m "Sidebar file pickers; auto-load activity/layout/preview"
```

---

## Task 17: CodecSelect component with descriptions + conditional chromakey/quality

**Files:**
- Create: `gui/src/components/CodecSelect.svelte`
- Modify: `gui/src/components/Sidebar.svelte` — embed CodecSelect + time fields

**Step 1: CodecSelect**

`gui/src/components/CodecSelect.svelte`:
```svelte
<script lang="ts">
  import { session } from "../lib/stores";

  const CODECS = [
    { id: "prores_4444", label: "prores", desc: "transparent alpha (largest files)" },
    { id: "h264_nvenc",  label: "h264_nvenc", desc: "fast, NVIDIA GPU acceleration" },
    { id: "hevc_nvenc",  label: "hevc_nvenc", desc: "smallest files, NVIDIA GPU acceleration" },
    { id: "h264",        label: "h264", desc: "no NVIDIA GPU, small filesize (CPU encode)" },
  ];

  $: selected = CODECS.find((c) => c.id === $session.codec) ?? CODECS[0];
  $: isAlpha = $session.codec === "prores_4444";
</script>

<div class="row">
  <label>Codec</label>
  <select bind:value={$session.codec}>
    {#each CODECS as c}
      <option value={c.id}>{c.label} — {c.desc}</option>
    {/each}
  </select>
</div>

<div class="row">
  <label>Quality ({isAlpha ? "qscale, lower = larger" : "CRF, lower = better"})</label>
  <input type="number" min="0" max="51" bind:value={$session.quality} />
</div>

{#if !isAlpha}
  <div class="row">
    <label>Chromakey color</label>
    <input type="color" bind:value={$session.chromakey} />
  </div>
{/if}

<style>
  .row { display: flex; flex-direction: column; gap: 0.25rem; margin-bottom: 0.75rem; }
  select, input { padding: 0.3rem; background: #333; color: #eee; border: 1px solid #444; }
</style>
```

**Step 2: Test conditional chromakey field**

`gui/src/components/CodecSelect.test.ts`:
```ts
import { render, fireEvent } from "@testing-library/svelte";
import { describe, it, expect } from "vitest";
import CodecSelect from "./CodecSelect.svelte";

describe("CodecSelect", () => {
  it("hides chromakey when codec is prores_4444", () => {
    const { queryByLabelText } = render(CodecSelect);
    // Default session codec is prores_4444
    expect(queryByLabelText(/Chromakey/)).toBeNull();
  });
  it("shows chromakey for h264_nvenc", async () => {
    const { getByRole, findByLabelText } = render(CodecSelect);
    const select = getByRole("combobox");
    await fireEvent.change(select, { target: { value: "h264_nvenc" } });
    expect(await findByLabelText(/Chromakey/)).toBeInTheDocument();
  });
});
```

**Step 3: Embed in sidebar + add time fields**

Insert into `Sidebar.svelte` after output picker:
```svelte
<CodecSelect />

<div class="row two-col">
  <div>
    <label>From (s)</label>
    <input type="number" min="0" bind:value={$session.from_seconds} />
  </div>
  <div>
    <label>To (s)</label>
    <input type="number" min="0" bind:value={$session.to_seconds} />
  </div>
</div>
```

**Step 4: Verify**

Run: `cd gui && npm test`
Expected: CodecSelect tests pass (may need to add `@testing-library/jest-dom` to satisfy `toBeInTheDocument`; if skipped, use `expect(await findByLabelText(...)).not.toBeNull()`).

**Step 5: Commit**

```bash
git add gui/src/
git commit -m "Add CodecSelect with conditional chromakey + time fields"
```

---

## Task 18: ExportFooter with progress bar + cancel + collapsible log

**Files:**
- Modify: `gui/src/components/ExportFooter.svelte`
- Modify: `gui/src/components/Sidebar.svelte` — add Export button + handler

**Step 1: Sidebar Export button**

```svelte
<script lang="ts">
  // … existing imports …
  import { startExport } from "../lib/tauri";
  import { probeCli } from "../lib/tauri";
  import { exportStatus } from "../lib/stores";

  async function doExport() {
    const cliPath = await probeCli($session.cli_path_override ?? undefined);
    if (!$session.input_path || !$session.layout_path || !$session.output_path) return;
    exportStatus.set("running");
    await startExport({
      cli_path: cliPath,
      input: $session.input_path,
      layout: $session.layout_path,
      output: $session.output_path,
      codec: $session.codec,
      quality: $session.quality,
      chromakey: $session.chromakey,
      from_seconds: $session.from_seconds,
      to_seconds: $session.to_seconds ?? 0,
    });
  }
</script>

<!-- … -->
<button class="primary" on:click={doExport} disabled={$exportStatus === "running"}>Export</button>
```

**Step 2: ExportFooter**

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { exportStatus, exportProgress, exportLog } from "../lib/stores";
  import { onExportProgress, onExportLog, onExportDone, cancelExport } from "../lib/tauri";

  let collapsed = true;

  onMount(() => {
    const ps: Array<() => void> = [];
    onExportProgress((p) => { exportProgress.set(p); }).then((u) => ps.push(u));
    onExportLog((p) => { exportLog.update((l) => [...l.slice(-500), p.line]); }).then((u) => ps.push(u));
    onExportDone((p) => { exportStatus.set(p.status); }).then((u) => ps.push(u));
    return () => ps.forEach((u) => u());
  });

  $: pct = $exportProgress ? Math.round(($exportProgress.frame / Math.max(1, $exportProgress.total)) * 100) : 0;
  $: etaLabel = $exportProgress?.eta_seconds ? `${Math.round($exportProgress.eta_seconds)}s` : "—";
</script>

<footer class="footer" class:running={$exportStatus === "running"}>
  {#if $exportStatus === "running" || $exportProgress}
    <div class="top">
      <div class="bar"><div class="fill" style="width:{pct}%"/></div>
      <div class="stats">
        {#if $exportProgress}{$exportProgress.frame} / {$exportProgress.total} · {$exportProgress.fps.toFixed(1)} fps · ETA {etaLabel}{/if}
      </div>
      {#if $exportStatus === "running"}
        <button on:click={cancelExport}>Cancel</button>
      {/if}
      <button on:click={() => collapsed = !collapsed}>{collapsed ? "Show log" : "Hide log"}</button>
    </div>
    {#if !collapsed}
      <div class="log">
        {#each $exportLog as line}
          <div class="logline">{line}</div>
        {/each}
      </div>
    {/if}
  {/if}
</footer>

<style>
  .footer { background: #1a1a1a; }
  .top { display: flex; gap: 0.5rem; align-items: center; padding: 0.5rem 1rem; }
  .bar { flex: 1; background: #222; height: 10px; border-radius: 5px; overflow: hidden; }
  .fill { height: 100%; background: #4cf; transition: width 0.1s linear; }
  .log { max-height: 200px; overflow-y: auto; font-family: monospace; font-size: 12px; padding: 0.5rem 1rem; background: #111; }
  .logline { color: #aaa; }
</style>
```

**Step 3: Verify**

Manual: run an export on a short clip; confirm progress bar animates to 100%, cancel works, log lines appear.

**Step 4: Commit**

```bash
git add gui/src/components/
git commit -m "Add ExportFooter with progress + cancel + log pane"
```

---

## Task 19: Startup banners — ffmpeg / CLI probe

**Files:**
- Create: `gui/src/components/StartupBanner.svelte`
- Modify: `gui/src/App.svelte` — probe both on mount, render banners

**Step 1: Banner component**

```svelte
<script lang="ts">
  export let kind: "ffmpeg" | "cli";
  export let onSetPath: () => void;
</script>

<div class="banner">
  <span>{kind} not found. Export is disabled until it's configured.</span>
  <button on:click={onSetPath}>Set path…</button>
</div>

<style>
  .banner { background: #a00; color: white; padding: 0.5rem 1rem; display: flex; justify-content: space-between; }
  button { background: white; color: #a00; border: 0; padding: 0.2rem 0.6rem; cursor: pointer; }
</style>
```

**Step 2: Wire in App.svelte**

```svelte
<script lang="ts">
  // … existing …
  import StartupBanner from "./components/StartupBanner.svelte";
  import { probeFfmpeg, probeCli } from "./lib/tauri";
  import { open } from "@tauri-apps/plugin-dialog";

  let ffmpegMissing = false;
  let cliMissing = false;

  onMount(async () => {
    // existing sessionLoad + …
    probeFfmpeg($session.cli_path_override ?? undefined).catch(() => (ffmpegMissing = true));
    probeCli($session.cli_path_override ?? undefined).catch(() => (cliMissing = true));
  });

  async function setCliPath() {
    const path = await open({ multiple: false });
    if (typeof path === "string") $session = { ...$session, cli_path_override: path };
    cliMissing = false;
    probeCli(path as string).catch(() => (cliMissing = true));
  }
  // ffmpeg override similarly if desired; v1 can defer and rely on PATH
</script>

{#if ffmpegMissing}<StartupBanner kind="ffmpeg" onSetPath={() => {}} />{/if}
{#if cliMissing}<StartupBanner kind="cli" onSetPath={setCliPath} />{/if}
```

Also disable the Export button when either is missing.

**Step 3: Commit**

```bash
git add gui/src/
git commit -m "Add startup banners for missing ffmpeg/CLI"
```

---

## Task 20: Layout hot-reload wiring on frontend

**Files:**
- Modify: `gui/src/App.svelte` — subscribe to `layout-reloaded` / `layout-error`, re-request preview

**Step 1: Subscribe in App**

```svelte
<script lang="ts">
  // … existing …
  import { onLayoutReloaded, onLayoutError } from "./lib/tauri";
  import { requestPreview } from "./lib/preview-dispatcher";
  import { previewT, exportLog } from "./lib/stores";

  let layoutError: string | null = null;

  onMount(async () => {
    // … existing …
    const u1 = await onLayoutReloaded(async () => {
      layoutError = null;
      await requestPreview($previewT);
    });
    const u2 = await onLayoutError((msg) => {
      layoutError = msg;
      exportLog.update((l) => [...l, `[layout-error] ${msg}`]);
    });
    return () => { u1(); u2(); };
  });
</script>

{#if layoutError}
  <div class="layout-error">Layout parse error: {layoutError}</div>
{/if}

<style>
  .layout-error { background: #844; color: white; padding: 0.3rem 1rem; font-size: 0.85rem; }
</style>
```

**Step 2: Verify**

Manual: open the app with a layout loaded, edit `examples/layout-cycling.json` in external editor, save. Preview should refresh within ~200ms. Introduce a deliberate JSON error (remove a comma); confirm error banner appears and previous preview stays.

**Step 3: Commit**

```bash
git add gui/src/App.svelte
git commit -m "Wire layout hot-reload events to preview + error banner"
```

---

## Task 21: Manual test checklist + README in `gui/`

**Files:**
- Create: `gui/README.md`

**Step 1: README**

```markdown
# gpx-overlay GUI

Desktop wrapper (Tauri + Svelte) around the `gpx-overlay` CLI.

## Development

```sh
cd gui
npm install
cargo tauri dev --config gui/src-tauri/tauri.conf.json
```

## Manual test checklist

Run before releasing:

1. **Cold start, no session file** — app opens with empty input/layout/output fields.
2. **Load activity** — picking a `.fit` or `.gpx` file shows a preview at t=0 and auto-fills `from=0`, `to=duration`.
3. **Scrub seekbar** — dragging updates the preview smoothly (downscaled). Release produces a full-resolution frame identical to an exported frame at that time.
4. **Edit layout in external editor** — save a change to the loaded JSON; preview refreshes within ~200ms. Introduce a parse error; red banner appears and previous preview remains.
5. **Export short clip** — pick a 10-second range, click Export; progress bar reaches 100%; resulting file plays in a video player.
6. **Cancel mid-export** — click Cancel; UI returns to idle within ~1s; partial output file remains on disk.
7. **Kill ffmpeg externally during export** — open Task Manager, kill `ffmpeg.exe`; error banner appears with tail of log.
8. **Missing ffmpeg on launch** — rename `ffmpeg.exe` on PATH, launch; red banner shown; Export button disabled.
9. **Session persistence** — set all fields, close the app, reopen; fields repopulate.

## Architecture

See `docs/plans/2026-04-23-tauri-gui-design.md`.
```

**Step 2: Commit**

```bash
git add gui/README.md
git commit -m "Add GUI README + manual test checklist"
```

---

## After all tasks complete

Use `superpowers:finishing-a-development-branch` to:

1. Run `cargo test --workspace` and `cd gui && npm test && npm run check` — all green.
2. Present options: merge to main / push + PR / keep as-is / discard.
3. On chosen option, clean up worktree.
