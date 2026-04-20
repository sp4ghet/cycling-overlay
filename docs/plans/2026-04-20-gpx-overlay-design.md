# gpx-overlay — Design

Date: 2026-04-20
Status: Draft, pre-implementation

## Goal

A Rust CLI that takes a GPX or FIT activity file and renders a transparent video overlay (ProRes 4444 in a `.mov`) for a selected time range. The overlay shows configurable metric widgets — speed, gradient, power, heart rate, cadence, elevation, cumulative elevation gain, distance, total distance, time, elapsed time, a course polyline with a moving dot, and an elevation profile. Intended to be dropped onto an editor timeline (Premiere, Resolve) and composited over action-cam footage.

## Non-goals (v1)

- Graphical layout designer. Layouts are hand-written JSON for now.
- Map tile backgrounds for the course widget — route polyline only.
- Non-text widget types (bars, dials, rolling graphs).
- Speed ramping / timelapse.
- Multiple built-in themes or preset layouts.

A Tauri GUI is the expected v2. The architecture leaves room for it without rework.

## Architecture

Cargo workspace with four crates:

```
gpx-overlay/
├── crates/
│   ├── activity/   # FIT + GPX parsing, sample model, smoothing, interpolation
│   ├── layout/     # Layout schema (serde types)
│   ├── render/     # Per-frame rasterizer — (layout, activity, t) → RGBA
│   └── cli/        # Binary: argparse, orchestration, ffmpeg piping
└── examples/       # Sample FIT/GPX, reference layout.json, integration tests
```

**Separation principle:** `render` is pure. Given a layout, an activity, and a timestamp, it returns a single RGBA frame. It knows nothing about video, ffmpeg, or file I/O. This buys us:

- Golden-image testing of widgets in isolation.
- Trivial per-frame parallelism.
- A future Tauri GUI calls `render` directly with no extra porting work.

### Dependency picks

- `fitparser` — FIT parsing
- `gpx` — GPX parsing
- `tiny-skia` — 2D rasterization (pure Rust, no C deps)
- `cosmic-text` — font shaping and rasterization
- `rayon` — per-frame parallelism
- `serde` / `serde_json` — layout schema
- `clap` (derive) — CLI
- `indicatif` — progress reporting
- `chrono` — timestamps
- `num_cpus` — default thread count
- ffmpeg invoked as a subprocess via `std::process::Command`

## Activity data pipeline

### Unified sample model

FIT and GPX are normalized into one struct:

```rust
struct Sample {
    t: Duration,              // from activity start
    lat: f64,
    lon: f64,
    altitude_m: Option<f32>,
    speed_mps: Option<f32>,
    heart_rate_bpm: Option<u8>,
    cadence_rpm: Option<u8>,
    power_w: Option<u16>,
    distance_m: Option<f64>,  // cumulative
}

struct Activity {
    samples: Vec<Sample>,
    start_time: DateTime<Utc>,
}
```

Every metric is `Option<_>`. The renderer degrades gracefully — missing values display as `--`.

### Derived metrics (computed once at load)

- `distance_m` if absent — cumulative haversine between consecutive lat/lon.
- `speed_mps` if absent — finite difference of distance over time.
- `gradient_pct` — rolling slope of altitude over distance (window ~50 m to tame GPS jitter).
- `elev_gain_cum_m` — cumulative positive altitude delta, hysteresis-filtered (ignore sub-3 m wiggles) so GPS noise doesn't inflate climbing.

### Smoothing

Applied per metric with sensible defaults, overridable via the layout or CLI:

- Speed: 3 s moving average
- Altitude: 5 s moving average
- Gradient: computed from smoothed altitude
- HR / power / cadence: 3 s, off by default (already sensor-smoothed)

### Interpolation

At 30 fps we sample between the typically 1 Hz source data. Linear interpolation for continuous metrics (speed, altitude, HR, power, distance, elevation gain); nearest-neighbor for cadence.

### Time range

CLI `--from` / `--to` accept `HH:MM:SS` / `MM:SS` offsets from activity start, seconds, or an ISO wall-clock timestamp (to sync to footage).

## Layout schema

A single JSON file. Versioned, hand-editable.

```json
{
  "version": 1,
  "canvas": { "width": 1920, "height": 1080, "fps": 30 },
  "units": { "speed": "kmh", "distance": "km", "elevation": "m", "temp": "c" },
  "theme": {
    "font": "Inter",
    "fg": "#ffffff",
    "accent": "#ffcc00",
    "shadow": { "blur": 4, "color": "#000000cc" }
  },
  "widgets": [
    {
      "id": "speed_readout",
      "type": "readout",
      "metric": "speed",
      "rect": { "x": 80, "y": 900, "w": 260, "h": 120 },
      "label": "SPEED",
      "decimals": 1,
      "font_size": 72
    },
    {
      "id": "course_map",
      "type": "course",
      "rect": { "x": 1560, "y": 60, "w": 300, "h": 300 },
      "line_width": 4,
      "dot_radius": 8
    },
    {
      "id": "elev_profile",
      "type": "elevation_profile",
      "rect": { "x": 80, "y": 60, "w": 500, "h": 120 }
    }
  ]
}
```

**v1 widget types:** `readout`, `course`, `elevation_profile`.

The widget enum is `#[serde(tag = "type")]`. Adding `bar`, `dial`, `graph` later is a new variant — no breaking changes to existing layouts.

## Rendering pipeline

Three concurrent stages:

```
┌─────────────┐    ┌──────────────────┐    ┌──────────────┐
│ Frame       │───▶│ Parallel render  │───▶│ Reorder      │───▶ ffmpeg stdin
│ scheduler   │    │ pool (Rayon)     │    │ buffer       │
│ (frame_idx) │    │ layout+activity  │    │ (by idx)     │
└─────────────┘    │     → RGBA       │    └──────────────┘
                   └──────────────────┘
```

**Scheduler** emits `frame_idx = 0..N` where `N = round((to - from) * fps)`. For each, `t = from + frame_idx / fps`.

**Render pool** — Rayon `par_iter` with a per-thread scratch `tiny_skia::Pixmap`, allocated once and reused per frame to avoid 200k+ allocations on a long render. Each frame is a pure `render_frame(&layout, &activity, t, &mut pixmap)`.

**Reorder buffer** — Rayon doesn't guarantee output order. A `BTreeMap<u64, Vec<u8>>` holds out-of-order frames until the next expected index arrives, then flushes contiguously to ffmpeg's stdin. Capped at 64 frames (~512 MB at 1080p RGBA); full buffer backpressures the render pool.

### ffmpeg invocation

```
ffmpeg -y \
  -f rawvideo -pix_fmt rgba -s 1920x1080 -framerate 30 -i - \
  -c:v prores_ks -profile:v 4444 -pix_fmt yuva444p10le \
  -vendor apl0 -qscale:v 11 \
  overlay.mov
```

`prores_ks` is ffmpeg's native ProRes encoder — works on Windows, no extra libraries. `qscale 11` is near-lossless.

### Progress and errors

- `indicatif` progress bar tracks frames emitted vs. frames flushed.
- ffmpeg non-zero exit → capture stderr, fail the run.
- Renderer panic in one frame → propagate, never emit a silent black frame.

## CLI

```
gpx-overlay render \
  --input ride.fit \
  --layout layout.json \
  --output overlay.mov \
  [--from 00:00:00] [--to 02:15:30] \
  [--fps 30] [--size 1920x1080] \
  [--threads 8] \
  [--qscale 11] \
  [--dry-run]
```

**Precedence:** CLI flags win over layout `canvas`. Lets you reuse one layout at different resolutions without editing JSON.

**`--dry-run`** parses inputs, validates the layout against the activity's available metrics, prints a summary (duration, frame count, estimated output size), and exits.

**Layout validation up front** (before frame 0):

- Widget `metric` refs exist on the `Sample` type.
- Widget rects fit the canvas.
- `version` matches (refuse unknown versions).
- Warn (not fail) if a referenced metric is `None` for the entire activity.

**Exit codes:** 0 success, 1 usage error, 2 parse error, 3 render/ffmpeg error.

## Testing

**`activity`** — unit tests with hand-crafted FIT and GPX fixtures. Cover:
- Missing-metric handling
- Unit conversions
- Derived distance/speed match sensor values within tolerance
- Elevation gain hysteresis (fabricate a noisy altitude track, assert gain ≈ sum of real climbs)
- Interpolation at sample boundaries and midpoints

**`layout`** — serde round-trip, schema validation (overflowing rects, bad metric refs, unknown versions).

**`render`** — golden-image tests. For a fixed activity + layout + `t`, compare the rendered frame to a checked-in PNG within a small pixel tolerance.

**`cli`** — integration test: render 2 s of a sample activity to `.mov`, then `ffprobe` to assert dimensions, fps, duration, codec, and alpha presence.

### Fixtures to commit

- 30-second GPX with all basic metrics
- 30-second FIT with power + HR + cadence
- Short GPX with no altitude (degradation path)
- One reference `layout.json` exercising every v1 widget

## Open questions / future work

- Tauri GUI, consuming `render` directly
- Additional widget types (bars, dials, graphs, zone indicators)
- Map tile backgrounds for `course`
- Speed ramping for timelapse overlays
- Theme presets and built-in layouts
