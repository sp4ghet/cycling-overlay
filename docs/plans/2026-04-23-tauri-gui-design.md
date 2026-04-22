# Tauri GUI Design

**Date:** 2026-04-23
**Status:** Design approved, awaiting implementation plan

## Goal

A desktop GUI (Tauri + Svelte) that wraps the existing `gpx-overlay` CLI so users can render overlay videos without the command line. v1 is a thin wrapper with live single-frame preview — no layout editing.

## Scope (v1)

- Pick input activity, layout JSON, output path via file pickers.
- Configure codec, quality, chromakey color, time range.
- Scrubbable seekbar with live preview — downscaled while dragging, full-res on release.
- Hot-reload the loaded layout JSON when its file changes on disk.
- Export by shelling out to the existing CLI; show progress bar + log pane; support cancel.
- Remember last session's paths and settings across launches.

**Explicitly out of scope:** layout editing/drag-and-drop canvas, recent-files list, keyboard shortcuts, app signing/installer polish, multi-window.

## Architecture

**Hybrid renderer strategy:**
- **Preview** — Tauri backend calls the `render` crate directly as a library. Produces a `Pixmap` per request, encodes to PNG, returns as data URL to the frontend.
- **Export** — Tauri backend spawns the existing `gpx-overlay` CLI binary as a subprocess, parses its stderr for progress, forwards log lines to the frontend.

**No modifications required** to existing `activity`, `layout`, `render`, or `cli` crates for v1. The GUI is purely additive.

## Project Layout

New top-level `gui/` directory alongside `crates/` and `examples/`:

```
gui/
├── src/                    # Svelte + TypeScript frontend
│   ├── App.svelte
│   ├── lib/
│   └── main.ts
├── src-tauri/              # Tauri Rust backend
│   ├── src/
│   │   ├── main.rs
│   │   ├── commands.rs     # #[tauri::command] handlers
│   │   ├── preview.rs      # single-frame render pipeline
│   │   ├── export.rs       # spawn CLI, parse stderr
│   │   ├── watcher.rs      # notify-based layout file watching
│   │   └── session.rs      # persist/restore last-session state
│   ├── Cargo.toml
│   └── tauri.conf.json
├── package.json
├── svelte.config.js
└── vite.config.ts
```

**Backend dependencies (`src-tauri/Cargo.toml`):**
- `tauri` ~2.0
- Path deps: `activity`, `layout`, `render` from `../../crates/*`
- `notify` ~6.0 for file watching
- `tokio` for async command handlers
- `serde` / `serde_json` for state + IPC

**Frontend:** Svelte + TypeScript, built with Vite.

## Window Layout

Single preview-dominant window:

```
┌─────────────────────────────────────────────────────────┐
├─────────────────────────┬───────────────────────────────┤
│                         │  Sidebar (fixed ~320px)       │
│                         │                               │
│                         │  Input file    [Browse...]    │
│    Preview pane         │  Layout file   [Browse...]    │
│    (checkerboard bg     │  Output path   [Browse...]    │
│     for alpha)          │                               │
│                         │  Codec: [dropdown w/ desc]    │
│                         │  Quality: [slider]            │
│                         │  Chromakey: [color picker]    │
│                         │                               │
│                         │  From: [MM:SS]  To: [MM:SS]   │
│                         │                               │
│                         │  [  Export  ]                 │
├─────────────────────────┤                               │
│ [──●────────────────]   │                               │
│ 02:15 / 1:23:45         │                               │
├─────────────────────────┴───────────────────────────────┤
│ Progress: [████████░░░░░] 450/900 frames · ETA 0:30     │
│ ┌─ Log ─────────────────────────────────────────────┐   │
│ │ frame=450 fps=73.2 q=-0.0 size=3248kB ...         │   │
│ └───────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

The progress+log footer collapses to ~0 height when export is idle.

### Svelte Components

- `App.svelte` — layout shell, store wiring, startup checks.
- `PreviewPane.svelte` — `<img>` with checkerboard background.
- `Seekbar.svelte` — throttled `scrub` + `scrub-end` events.
- `Sidebar.svelte` — composes pickers, codec selector, time fields, export button.
- `CodecSelect.svelte` — dropdown exposes codec names (`prores`, `h264_nvenc`, `libx264`, `hevc_nvenc`) with per-option description text (e.g. "prores: transparent", "h264_nvenc: NVIDIA GPU acceleration", "libx264: smaller filesize, no GPU needed"). Chromakey color picker shown only when codec lacks alpha support.
- `ExportFooter.svelte` — progress bar, cancel button, collapsible log pane.

### Stores

- `session` — paths, codec, quality, chromakey, from/to. Persisted.
- `preview` — current `t`, last rendered image, rendering flag.
- `export` — status (idle/running/done/error), progress, log lines.

## Preview Pipeline

- `Activity` parsed once on input load, held in backend state.
- `Layout` parsed on layout load / watcher refresh, held in backend state.
- Each scrub tick → `preview_frame { t, downscale, request_id }` command → `Pixmap` → PNG data URL.
- Frontend throttles scrub to ~15fps (67ms) during drag; sends one more request on release with `downscale: false`.
- **Downscale during drag:** backend renders at the preview pane's CSS size (frontend reports it in each request). Release renders at the layout's configured output resolution, CSS-scaled for display.
- **Latest-wins**: monotonic `request_id`; frontend ignores out-of-order responses.

No frame cache — renders are fast enough, and layout hot-reload would invalidate a cache anyway.

## Export Pipeline

**CLI binary resolution** (checked in order):
1. Sibling of the GUI executable.
2. `PATH` lookup.
3. User-configured override path (stored in session, editable via a small "Settings" control below the sidebar).

Same pattern for `ffmpeg`, but `ffmpeg` presence is the CLI's concern — the GUI only probes `ffmpeg -version` at launch and shows a warning banner if missing.

**Spawn + stream:**
- `tokio::process::Command` with piped stdout/stderr.
- Task reads stderr line-by-line. Lines matching ffmpeg's `frame=N fps=F ...` pattern update progress; others appended to log.
- Tauri events: `export-progress { frame, fps, eta_seconds }`, `export-log { line, stream }`.
- Progress math: `total_frames = (to - from).as_secs_f32() * fps`; `eta = (total - frame) / current_fps`.

**Cancel:**
- Frontend → `export_cancel` command.
- Backend sends SIGTERM (Unix) / `taskkill /T` (Windows) to the child. The `/T` is required to also kill the ffmpeg process that the CLI spawned.
- Existing CLI Ctrl-C handler cleans up its ffmpeg pipe and temp files.

**Completion states:** `success`, `canceled`, `error`. Error dialog shows last ~50 log lines; partial output file left on disk in all cases.

## Session Persistence

Location: Tauri app-config dir (OS-appropriate). Single file: `session.json`.

```json
{
  "input_path": "...",
  "layout_path": "...",
  "output_path": "...",
  "codec": "h264_nvenc",
  "quality": 23,
  "chromakey": "#00FF00",
  "from_seconds": 0,
  "to_seconds": 5025,
  "cli_path_override": null
}
```

- **Load on startup**: apply to stores; probe each path with `fs::metadata`; missing paths cleared with a warning toast.
- **Save on change**: Svelte store subscription, 500ms debounced. No explicit save button.

## Layout File Watching

Backend owns a `notify::RecommendedWatcher`. Loading a layout file replaces the watched path.

On modify event:
1. Debounce ~150ms (editors often emit rename+replace as multiple events).
2. Re-parse the layout file.
3. Success → update in-memory `Layout`, emit `layout-reloaded` → frontend re-requests current preview frame.
4. Parse failure → emit `layout-error { message }` → frontend shows error in log pane, keeps last good layout.

Input activity and output path are **not** watched.

## Error Handling

**Startup:**
- `ffmpeg` missing → banner + "Set path" button; export disabled.
- CLI binary missing → banner; preview works, export disabled.

**File-load:**
- Parser errors → toast with message; previous file (if any) stays loaded.
- Picker cancelled → no-op.

**Preview:**
- Render error → Tauri command returns error; frontend shows red border on preview pane with message and Retry button.
- `t` out of range → already clamped by `Activity::sample_at`; no special case.

**Export:**
- CLI non-zero exit → error dialog with last ~50 log lines; full transcript in log pane.
- ffmpeg mid-run failure → surfaces as CLI non-zero exit, same path.
- Cancelled → status `canceled`, partial output left on disk.

**OS:**
- Windows: `taskkill /T` to kill CLI + ffmpeg together.
- macOS: v1 ships unsigned; document "right-click → Open" workaround.

## Testing

**Covered by automated tests:**
- Backend pure functions: session roundtrip, argv composition, stderr parser, ETA math, binary resolution order.
- Preview smoke test: Tauri command path produces same `Pixmap` as the `render` crate's direct API (uses fixture activity + layout).
- Watcher debounce: `notify` mock events coalesce as expected.
- Svelte components: `Seekbar` throttling, `CodecSelect` conditional fields (`@testing-library/svelte` + `vitest`).

**Manual test checklist** (run before release):
1. Cold start with no session → empty state.
2. Load activity → preview at t=0, from/to auto-fill to full duration.
3. Scrub seekbar → downscaled previews smooth; release produces full-res frame.
4. Edit loaded layout in external editor → preview reloads within ~200ms.
5. Export a 10s clip → progress to 100%, file plays.
6. Cancel mid-export → UI idle within ~1s, partial file on disk.
7. Kill ffmpeg externally during export → error dialog with tail of log.

**Not covered in v1:** end-to-end UI tests (Tauri WebDriver), visual scrub interaction.
