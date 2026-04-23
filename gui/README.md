# cycling-overlay GUI

Desktop wrapper (Tauri 2 + Svelte + TypeScript) around the `cycling-overlay` CLI.

## Development

Prerequisites:
- Rust toolchain (workspace root has one)
- Node 20+ and `npm`
- `cargo-tauri` CLI: `cargo install tauri-cli --version '^2.0' --locked`
- `ffmpeg` on `PATH` (for actual exports; startup banner warns if missing)

To run the dev server + Tauri window:

```sh
cd gui
npm install              # first time only
cargo tauri dev
```

The backend lives at `gui/src-tauri/`; the frontend at `gui/src/`. Vite serves on port 5173; Tauri opens a native window that loads from that URL. `beforeDevCommand` auto-starts Vite.

## Build

```sh
cd gui
npm run build            # compiles the Svelte bundle to gui/build/
cargo tauri build        # bundles the native app; outputs to target/release/bundle/
```

## Layout

- `src/App.svelte` — window shell
- `src/components/` — PreviewPane, Seekbar, Sidebar, CodecSelect, ExportFooter, StartupBanner
- `src/lib/types.ts` — TS interfaces mirroring backend serde types
- `src/lib/tauri.ts` — typed wrappers for `invoke` and event listeners
- `src/lib/stores.ts` — Svelte writables (session, preview, export)
- `src/lib/preview-dispatcher.ts` — monotonic-id preview request dispatcher (latest-wins)
- `src-tauri/src/` — backend modules: session, binary, progress, state, preview, watcher, export

## Manual test checklist

Run before releasing:

1. **Cold start, no session file** — app opens with empty input/layout/output fields; no banners unless ffmpeg/CLI are missing.
2. **Load activity** — picking a `.fit` or `.gpx` file renders a preview at t=0 and auto-fills `from=0` / `to=duration`.
3. **Scrub seekbar** — dragging updates the preview smoothly (downscaled during drag). Releasing produces a full-resolution frame identical to a real export frame at that time.
4. **Edit layout in external editor** — save a change to the loaded JSON; preview refreshes within ~200ms. Introduce a parse error (e.g. delete a comma); red banner appears and the previous preview remains.
5. **Export a short clip** — pick a 10-second range, click Export; progress bar reaches 100%; resulting file plays in a video player.
6. **Cancel mid-export** — click Cancel; UI returns to idle within ~1s; partial output file remains on disk.
7. **Kill ffmpeg externally during export** — open Task Manager / `pkill ffmpeg`; error event appears in the log pane with the CLI's non-zero exit.
8. **Missing ffmpeg on launch** — rename `ffmpeg` off `PATH`, launch; red banner visible; Export button disabled.
9. **Session persistence** — set all fields, close the app, reopen; fields repopulate. Session file lives in the OS app-config dir (`%APPDATA%\com.cycling-overlay.app\session.json` on Windows).

## Architecture

See `docs/plans/2026-04-23-tauri-gui-design.md` for the design doc, and `docs/plans/2026-04-23-tauri-gui-impl.md` for the implementation plan that produced this.
