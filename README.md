# cycling-overlay

Render ride/run telemetry — speed, HR, power, cadence, distance, elevation, course map, gradient, W/kg, time — as a video overlay you can drop onto your action-cam footage in any editor. Takes a GPX or FIT activity file plus a JSON layout, outputs a transparent-background MOV (ProRes 4444) or a chromakey-fill MP4 (H.264 / HEVC).

## Highlights

- **Two interfaces.** A desktop GUI for picking files, scrubbing the timeline, and clicking Export; a CLI for scripting and automation. Both share the same rendering core.
- **Live preview.** The GUI shows a single frame of your layout at any point in the activity. Scrub the seekbar to pick from/to times; tweak the layout JSON and the preview refreshes as you save.
- **Alpha or chromakey output.** ProRes 4444 keeps true transparency. H.264 / HEVC (with NVENC GPU acceleration if you have an NVIDIA GPU) are smaller and faster — render onto a solid color and chromakey it out in your NLE.
- **Widget catalog.** Numeric readouts, linear meters, radial gauges (all with tick marks + numbers + four indicator styles), horizontal bars, course map, elevation profile. See [`docs/layouts.md`](docs/layouts.md).

## Prerequisites

- **`ffmpeg`** on your `PATH`. Any recent build works; needs `prores_ks` if you want transparent output (ships in any full ffmpeg build). The GUI has a banner + "Set path…" picker if it can't find ffmpeg automatically.
- **NVIDIA GPU + recent drivers** for the `h264_nvenc` / `hevc_nvenc` codecs (optional).
- **Rust stable** to build from source (see below).

## Desktop GUI (recommended)

The GUI wraps the CLI with file pickers, a scrubbable preview, codec presets, progress / cancel, and live layout reload.

Build + launch:

```sh
cargo install tauri-cli --version '^2.0' --locked   # one-time
cd gui
npm install                                          # one-time
cargo tauri dev
```

Then: **Browse…** your activity file (GPX or FIT), **Browse…** a layout JSON (try `examples/layout-cycling.json`), pick an output path, drag the seekbar to pick from/to, click **Export**. The progress bar tracks frames written to ffmpeg; hit **Cancel** to abort.

Full GUI dev instructions (Vite dev server, Tauri config, manual test checklist): [`gui/README.md`](gui/README.md).

## Command line

Build the CLI:

```sh
cargo build --release
```

Render:

```sh
./target/release/cycling-overlay render \
    --input examples/short.gpx \
    --layout examples/layout.json \
    --output out.mov
```

`out.mov` is a transparent ProRes 4444 clip at the canvas size the layout declares. Drop it onto your NLE timeline — alpha Just Works.

### Codec choice

| `--codec` | Container | When to pick it |
| --- | --- | --- |
| `prores4444` *(default)* | `.mov` | Transparent alpha; drop directly onto a timeline. Largest files. |
| `h264_nvenc` | `.mp4` / `.mov` | Fast, NVIDIA GPU acceleration. Chromakey fill. |
| `hevc_nvenc` | `.mp4` / `.mov` | Smallest files with GPU acceleration. Chromakey fill. |
| `h264` | `.mp4` / `.mov` | No NVIDIA GPU; CPU-encoded. Small files. Chromakey fill. |

For the chromakey codecs the overlay renders on a solid fill color (default magenta `#ff00ff`) that you key out in your editor. Override with `--chromakey "#00ff00"` etc.

### Useful flags

| Flag | Description |
| --- | --- |
| `-i, --input <PATH>` | Activity file (`.gpx` or `.fit`). |
| `-l, --layout <PATH>` | Layout JSON file. |
| `-o, --output <PATH>` | Output video path. |
| `--from <TIME>` | Start: `HH:MM:SS`, `MM:SS`, or seconds (fractional OK). |
| `--to <TIME>` | End (defaults to activity end). |
| `--codec <CODEC>` | See table above. |
| `--crf <N>` | Quality for H.264 / HEVC (lower = better, default 20). |
| `--qscale <N>` | Quality for ProRes (lower = larger, default 11). |
| `--chromakey <#rrggbb>` | Fill color for non-alpha codecs. |
| `--fps <N>` | Override the layout's frame rate. |
| `--size <WxH>` | Override the canvas pixel dimensions. (See caveat below.) |
| `--progress-json` | Emit one JSON line per frame to stderr (used by the GUI). |
| `--dry-run` | Parse and validate, don't render. |

Full help: `cycling-overlay render --help`.

## Layouts

Layouts are JSON files that define canvas dimensions, units, theme, and the list of widgets. See [`docs/layouts.md`](docs/layouts.md) for the authoring reference, and the files in [`examples/`](examples/) for working starting points you can copy and modify.

Iterate by keeping the GUI open with the layout loaded — the watcher auto-reloads when you save, so you can tune positions, colors, and tick intervals interactively.

## Known limitations

- **Only the bundled fonts** (Inter for UI, Roboto for numerics) are available. System fonts are not loaded.
- **`--size` does not rescale widget rects.** It overrides the canvas dimensions only; shrinking a 1920×1080-designed layout to 640×360 fails validation because widgets fall outside. Edit widget rects in the JSON or scale them proportionally.
- **GPX without extensions** carries no HR / power / cadence — those widgets will show `--`.
- **macOS distribution** is unsigned for v1; right-click → Open the first time you launch a packaged build.

## License

This project is released under the [MIT License](LICENSE.md).

The bundled **Inter** and **Roboto** fonts are distributed under the [SIL Open Font License 1.1](https://openfontlicense.org). See [`crates/render/assets/Inter-OFL.txt`](crates/render/assets/Inter-OFL.txt) and [`crates/render/assets/Roboto-OFL.txt`](crates/render/assets/Roboto-OFL.txt) for the full terms. If you redistribute this project or a binary built from it, those licenses travel with the fonts.

## Further reading

- [`docs/layouts.md`](docs/layouts.md) — layout authoring guide.
- [`gui/README.md`](gui/README.md) — GUI build + manual test checklist.
- [`docs/plans/`](docs/plans/) — historic design and implementation plans.

## Project structure

```
crates/
    activity/      # GPX + FIT parsing, sampling, derived metrics
    layout/        # Layout schema + validation
    render/        # Frame compositing (tiny-skia + cosmic-text), widgets
    cli/           # The cycling-overlay binary
gui/
    src/           # Svelte + TypeScript frontend
    src-tauri/     # Tauri 2 Rust backend
examples/          # Sample activity + sample layouts
docs/              # Layout reference + design docs
```

## Testing

Default (skips ffmpeg integration tests):

```sh
cargo test --workspace
```

Full suite (requires `ffmpeg` and `ffprobe` on `PATH`):

```sh
cargo test --workspace --features ffmpeg-tests
```
