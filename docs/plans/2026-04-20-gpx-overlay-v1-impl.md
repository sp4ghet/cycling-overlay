# gpx-overlay v1 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Ship a Rust CLI `gpx-overlay render` that produces a transparent ProRes 4444 `.mov` overlay from a hand-written `layout.json` plus a FIT or GPX activity file, for a selected time range.

**Architecture:** Cargo workspace with four crates — `activity` (parsing + sample model + smoothing/derived metrics + interpolation), `layout` (serde schema + validation), `render` (pure per-frame rasterizer), `cli` (argparse + orchestration + parallel render + ffmpeg pipe). `render` is pure; every frame is a function of `(layout, activity, t)`. Per-frame parallelism via Rayon with a reorder buffer in front of ffmpeg stdin.

**Tech Stack:** Rust 2021, `fitparser`, `gpx`, `tiny-skia`, `cosmic-text`, `rayon`, `serde`/`serde_json`, `clap` (derive), `indicatif`, `chrono`. ffmpeg (external binary) for encoding. Golden-image tests via `image` crate.

**Prerequisite:** ffmpeg must be on `PATH`. Verify with `ffmpeg -version` before Task 25.

**Design doc:** See `docs/plans/2026-04-20-gpx-overlay-design.md` for full context.

---

## Testing strategy

- **Unit tests** live next to their code (`#[cfg(test)] mod tests`).
- **Sample-constructing helper** `Activity::from_samples(samples)` lets most tests skip real parsing.
- **Golden-image tests** for render widgets: render a known scene, compare to a checked-in PNG with per-channel tolerance of 2 (accounts for font/AA variance). On mismatch, write `actual.png` and `diff.png` beside the golden for inspection.
- **One integration test** in `crates/cli/tests/end_to_end.rs` invokes `ffmpeg` and `ffprobe`; guarded by `#[cfg_attr(not(feature = "ffmpeg-tests"), ignore)]` so it only runs when explicitly requested.
- **Fixtures** in `examples/`: `short.gpx` (hand-written, ~20 points with altitude, HR not present), and `ride.fit` (seeded at repo root — a real ride supplied by the user; tests use a subslice when a short fixture is needed). Inter variable font is pre-staged at repo root `assets/Inter-VariableFont.ttf`; Task 16 moves it into `crates/render/assets/`.

---

## Task 1: Bootstrap Cargo workspace

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `crates/activity/Cargo.toml`, `crates/activity/src/lib.rs`
- Create: `crates/layout/Cargo.toml`, `crates/layout/src/lib.rs`
- Create: `crates/render/Cargo.toml`, `crates/render/src/lib.rs`
- Create: `crates/cli/Cargo.toml`, `crates/cli/src/main.rs`
- Create: `rust-toolchain.toml` (pin to `stable`)

**Step 1: Write workspace `Cargo.toml`**

```toml
[workspace]
resolver = "2"
members = ["crates/activity", "crates/layout", "crates/render", "crates/cli"]

[workspace.package]
version = "0.1.0"
edition = "2021"
rust-version = "1.75"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
thiserror = "1"
anyhow = "1"
```

**Step 2: Create each crate's `Cargo.toml`**

`crates/activity/Cargo.toml`:

```toml
[package]
name = "activity"
version.workspace = true
edition.workspace = true

[dependencies]
serde.workspace = true
chrono.workspace = true
thiserror.workspace = true
gpx = "0.10"
fitparser = "0.9"
```

`crates/layout/Cargo.toml`:

```toml
[package]
name = "layout"
version.workspace = true
edition.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
```

`crates/render/Cargo.toml`:

```toml
[package]
name = "render"
version.workspace = true
edition.workspace = true

[dependencies]
activity = { path = "../activity" }
layout = { path = "../layout" }
tiny-skia = "0.11"
cosmic-text = "0.12"
anyhow.workspace = true

[dev-dependencies]
image = { version = "0.25", default-features = false, features = ["png"] }
```

`crates/cli/Cargo.toml`:

```toml
[package]
name = "gpx-overlay"
version.workspace = true
edition.workspace = true

[[bin]]
name = "gpx-overlay"
path = "src/main.rs"

[dependencies]
activity = { path = "../activity" }
layout = { path = "../layout" }
render = { path = "../render" }
clap = { version = "4", features = ["derive"] }
indicatif = "0.17"
rayon = "1"
anyhow.workspace = true
num_cpus = "1"

[features]
ffmpeg-tests = []
```

**Step 3: Minimal `lib.rs` / `main.rs` stubs**

Each `lib.rs`: empty file is fine (add `pub use` as items land). `cli/src/main.rs`:

```rust
fn main() {
    println!("gpx-overlay");
}
```

**Step 4: Create `rust-toolchain.toml`**

```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
```

**Step 5: Verify workspace builds**

Run: `cargo check --workspace`
Expected: all four crates compile cleanly.

**Step 6: Commit**

```bash
git add Cargo.toml crates/ rust-toolchain.toml
git commit -m "Bootstrap Cargo workspace with four crates"
```

---

## Task 2: Test fixtures — hand-written short.gpx

**Files:**
- Create: `examples/short.gpx`

**Step 1: Write a 20-point GPX fixture**

A short out-and-back along a 100 m elevation rise, 1 Hz samples, 20 s total. Put realistic lat/lon (pick a small area near 0,0 to keep coords readable). Include `<time>`, `<ele>`, and `<extensions>` with `hr` and `power` on a few points (we'll treat HR presence as partial — some points have it, some don't).

Use a literal hand-written XML file; don't generate. Must parse with the `gpx` crate (schema 1.1).

**Step 2: Verify it parses**

Run (after Task 3, skip for now): `cargo test -p activity gpx_fixture_loads`

**Step 3: Commit**

```bash
git add examples/short.gpx
git commit -m "Add short.gpx test fixture"
```

---

## Task 3: `activity::Sample` and `activity::Activity` types

**Files:**
- Modify: `crates/activity/src/lib.rs`
- Create: `crates/activity/src/sample.rs`

**Step 1: Write failing test**

In `crates/activity/src/sample.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use std::time::Duration;

    #[test]
    fn from_samples_builds_activity() {
        let samples = vec![
            Sample { t: Duration::from_secs(0), lat: 0.0, lon: 0.0,
                     altitude_m: Some(100.0), speed_mps: None,
                     heart_rate_bpm: None, cadence_rpm: None,
                     power_w: None, distance_m: None },
        ];
        let a = Activity::from_samples(Utc.timestamp_opt(0, 0).unwrap(), samples);
        assert_eq!(a.samples.len(), 1);
        assert_eq!(a.duration(), Duration::from_secs(0));
    }
}
```

**Step 2: Run test, verify it fails**

Run: `cargo test -p activity`
Expected: FAIL with undefined types.

**Step 3: Implement**

```rust
use chrono::{DateTime, Utc};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    pub t: Duration,
    pub lat: f64,
    pub lon: f64,
    pub altitude_m: Option<f32>,
    pub speed_mps: Option<f32>,
    pub heart_rate_bpm: Option<u8>,
    pub cadence_rpm: Option<u8>,
    pub power_w: Option<u16>,
    pub distance_m: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct Activity {
    pub start_time: DateTime<Utc>,
    pub samples: Vec<Sample>,
}

impl Activity {
    pub fn from_samples(start_time: DateTime<Utc>, samples: Vec<Sample>) -> Self {
        Self { start_time, samples }
    }

    pub fn duration(&self) -> Duration {
        self.samples.last().map(|s| s.t).unwrap_or_default()
    }
}
```

Re-export from `lib.rs`:

```rust
mod sample;
pub use sample::{Activity, Sample};
```

**Step 4: Run tests, verify pass**

Run: `cargo test -p activity`
Expected: 1 passing.

**Step 5: Commit**

```bash
git add crates/activity
git commit -m "Add Sample and Activity core types to activity crate"
```

---

## Task 4: GPX parser

**Files:**
- Create: `crates/activity/src/gpx_parse.rs`
- Modify: `crates/activity/src/lib.rs`

**Step 1: Write failing test**

```rust
#[test]
fn gpx_fixture_loads() {
    let a = load_gpx(std::path::Path::new("../../examples/short.gpx")).unwrap();
    assert!(a.samples.len() >= 2);
    assert!(a.samples.iter().any(|s| s.altitude_m.is_some()));
    assert_eq!(a.samples[0].t, std::time::Duration::ZERO);
}
```

**Step 2: Run, verify fails**

Run: `cargo test -p activity gpx_fixture_loads`
Expected: FAIL (undefined `load_gpx`).

**Step 3: Implement**

Use `gpx::read` over a `BufReader<File>`. Iterate `gpx.tracks[i].segments[j].points`; extract lat/lon, elevation, time, and any `extensions` HR/power. Record `start_time` from the first point's `time`; compute each `t` as `point_time - start_time`. Pull HR and power from extensions using `gpx`'s generic XML node access (fall back to `None` when absent).

**Step 4: Run, verify pass**

Run: `cargo test -p activity`

**Step 5: Commit**

```bash
git add crates/activity
git commit -m "Parse GPX into unified Sample model"
```

---

## Task 5: FIT parser

**Files:**
- Create: `crates/activity/src/fit_parse.rs`
- Modify: `crates/activity/src/lib.rs`

**Step 1: Write failing test**

```rust
#[test]
fn fit_fixture_loads() {
    let a = load_fit(std::path::Path::new("../../examples/ride.fit")).unwrap();
    assert!(a.samples.len() >= 2);
    assert!(a.samples.iter().any(|s| s.power_w.is_some())
         || a.samples.iter().any(|s| s.heart_rate_bpm.is_some()));
}
```

**Step 2: Run, verify fails.**

**Step 3: Implement**

Use `fitparser::from_reader`. Filter for `MesgType::Record`. Map fields by name:

| FIT field | Sample field | Scaling |
|-----------|--------------|---------|
| `timestamp` | drives `t` (first record = start_time) | seconds |
| `position_lat`/`position_long` | lat/lon | semicircles → degrees: `v as f64 * (180.0 / 2_147_483_648.0)` |
| `altitude` or `enhanced_altitude` | `altitude_m` | already meters |
| `speed` or `enhanced_speed` | `speed_mps` | already m/s |
| `heart_rate` | `heart_rate_bpm` | bpm |
| `cadence` | `cadence_rpm` | rpm |
| `power` | `power_w` | watts |
| `distance` | `distance_m` | meters |

Prefer `enhanced_*` over base fields when present.

**Step 4: Run, verify pass.**

**Step 5: Commit**

```bash
git add crates/activity
git commit -m "Parse FIT into unified Sample model"
```

---

## Task 6: Haversine helper and derived distance fill-in

**Files:**
- Create: `crates/activity/src/geo.rs`
- Modify: `crates/activity/src/sample.rs` (add fill method)

**Step 1: Write failing test**

```rust
#[test]
fn haversine_km_matches_known() {
    // London → Paris is ~344 km
    let d = haversine_m(51.5074, -0.1278, 48.8566, 2.3522);
    assert!((d - 343_550.0).abs() < 1_000.0);
}

#[test]
fn fill_distance_cumulates() {
    let samples = vec![
        Sample { t: Duration::ZERO, lat: 0.0, lon: 0.0, distance_m: None, ..Sample::blank() },
        Sample { t: Duration::from_secs(1), lat: 0.0, lon: 0.001, distance_m: None, ..Sample::blank() },
    ];
    let mut a = Activity::from_samples(Utc::now(), samples);
    a.fill_derived_distance();
    assert_eq!(a.samples[0].distance_m, Some(0.0));
    assert!(a.samples[1].distance_m.unwrap() > 100.0);
}
```

Add `Sample::blank()` test helper.

**Step 2: Run, verify fails.**

**Step 3: Implement haversine (R = 6371000 m) and `fill_derived_distance`:**
- If all samples already have `distance_m`, return.
- Else set `samples[0].distance_m = Some(0.0)`, then walk pairwise, adding haversine to the running total.

**Step 4: Run, verify pass.**

**Step 5: Commit**

```bash
git add crates/activity
git commit -m "Add haversine + cumulative distance fill"
```

---

## Task 7: Derived speed from distance (finite difference)

**Files:**
- Modify: `crates/activity/src/sample.rs`

**Step 1: Write failing test**

Build an activity whose distance grows 10 m/s. After `fill_derived_speed()`, all `speed_mps` should be ~10.0.

**Step 2: Run, verify fails.**

**Step 3: Implement.** For each sample, if `speed_mps.is_none()`, compute central difference over a 3-sample window on `distance_m` and `t`. Endpoints use forward/backward difference. Requires `distance_m` to be filled first; assert in docs.

**Step 4: Run, verify pass.**

**Step 5: Commit**

```bash
git add crates/activity
git commit -m "Derive missing speed from distance"
```

---

## Task 8: Moving-average smoothing

**Files:**
- Create: `crates/activity/src/smooth.rs`

**Step 1: Write failing test**

```rust
#[test]
fn moving_avg_flattens_noise() {
    let ts: Vec<Duration> = (0..10).map(|i| Duration::from_secs(i)).collect();
    let vs = [1.0, 3.0, 1.0, 3.0, 1.0, 3.0, 1.0, 3.0, 1.0, 3.0];
    let out = moving_avg_time(&ts, &vs, Duration::from_secs(3));
    // middle values smooth to ~2.0
    for v in &out[2..8] { assert!((v - 2.0).abs() < 0.2); }
}
```

**Step 2: Run, verify fails.**

**Step 3: Implement `moving_avg_time(ts, vs, window)`** — for each index `i`, average all `vs[j]` whose `ts[j]` is within `window` of `ts[i]`. Use a two-pointer sliding window for O(n) cost.

Add `Activity::smooth_speed(window)` and `smooth_altitude(window)` wrappers that apply it and write back into the samples.

**Step 4: Run, verify pass.**

**Step 5: Commit**

```bash
git add crates/activity
git commit -m "Add time-windowed moving-average smoothing"
```

---

## Task 9: Elevation gain with hysteresis

**Files:**
- Modify: `crates/activity/src/sample.rs`

**Step 1: Write failing test**

A 5-minute altitude trace that zig-zags ±1 m but ascends 50 m overall should report ~50 m gain, not 500 m. A trace that climbs +100 m then descends −100 m should report +100 m gain.

**Step 2: Run, verify fails.**

**Step 3: Implement `elev_gain_cum(samples, threshold_m)`** — returns `Vec<Option<f32>>` of cumulative gain. Keep a "last confirmed elevation" anchor; only emit a gain delta when current altitude exceeds anchor by > threshold (3 m default) *upward*. On downward moves, update anchor only when drop exceeds threshold.

Store result in a new `Sample` field `elev_gain_cum_m: Option<f32>`.

**Step 4: Run, verify pass.**

**Step 5: Commit**

```bash
git add crates/activity
git commit -m "Compute cumulative elevation gain with 3m hysteresis"
```

---

## Task 10: Gradient from smoothed altitude

**Files:**
- Modify: `crates/activity/src/sample.rs`

**Step 1: Write failing test**

A constant 10% slope (altitude rises 10 m per 100 m of distance) should yield gradient ≈ 10.0 in the middle of the activity.

**Step 2: Run, verify fails.**

**Step 3: Implement.** Add `gradient_pct: Option<f32>` to `Sample`. For each `i`, find samples `j < i < k` such that `distance_m[k] - distance_m[j] ≈ 50 m`, compute `(altitude[k] - altitude[j]) / (distance[k] - distance[j]) * 100`. Endpoints use whatever window is available.

**Step 4: Run, verify pass.**

**Step 5: Commit**

```bash
git add crates/activity
git commit -m "Compute gradient from smoothed altitude over 50m windows"
```

---

## Task 11: Sample interpolation at time t

**Files:**
- Create: `crates/activity/src/interp.rs`

**Step 1: Write failing test**

```rust
#[test]
fn interpolates_speed_linearly() {
    let s = vec![
        mk(0, Some(10.0)),
        mk(10, Some(20.0)),
    ];
    let a = Activity::from_samples(Utc::now(), s);
    let mid = a.sample_at(Duration::from_secs(5));
    assert!((mid.speed_mps.unwrap() - 15.0).abs() < 0.01);
}
```

**Step 2: Run, verify fails.**

**Step 3: Implement `Activity::sample_at(t) -> Sample`.** Binary-search for the bracketing pair. Linear interp for `speed_mps`, `altitude_m`, `heart_rate_bpm`, `power_w`, `distance_m`, `elev_gain_cum_m`, `gradient_pct`. Nearest-neighbor for `cadence_rpm`, `lat`, `lon` (actually linear for lat/lon is fine for short gaps; we'll pick linear). Clamp to endpoints outside range.

**Step 4: Run, verify pass.**

**Step 5: Commit**

```bash
git add crates/activity
git commit -m "Add time-based sample interpolation"
```

---

## Task 12: `Activity::prepare()` — apply full derivation pipeline

**Files:**
- Modify: `crates/activity/src/sample.rs`

**Step 1: Write failing test**

Load `short.gpx`, call `prepare()`, assert `speed_mps`, `distance_m`, `gradient_pct`, `elev_gain_cum_m` are all `Some(_)` for mid-activity samples.

**Step 2: Run, verify fails.**

**Step 3: Implement** — one-shot method that runs the pipeline in the correct order:
1. `fill_derived_distance`
2. `smooth_altitude(5s)`
3. `fill_derived_speed` then `smooth_speed(3s)`
4. `fill_gradient`
5. `fill_elev_gain(3.0)`

**Step 4: Run, verify pass.**

**Step 5: Commit**

```bash
git add crates/activity
git commit -m "Compose derivation pipeline in Activity::prepare"
```

---

## Task 13: Layout schema types

**Files:**
- Modify: `crates/layout/src/lib.rs`

**Step 1: Write failing test**

Round-trip: serialize a `Layout` to JSON, parse it back, assert equality. The JSON should match the schema in the design doc.

**Step 2: Run, verify fails.**

**Step 3: Implement:**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Layout {
    pub version: u32,
    pub canvas: Canvas,
    pub units: Units,
    pub theme: Theme,
    pub widgets: Vec<Widget>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Canvas { pub width: u32, pub height: u32, pub fps: u32 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Units {
    pub speed: SpeedUnit, pub distance: DistanceUnit,
    pub elevation: ElevationUnit, pub temp: TempUnit,
}

// Enums for units: Kmh/Mph, Km/Mi, M/Ft, C/F — serde rename_all = "lowercase"

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Theme {
    pub font: String,
    pub fg: String,
    pub accent: String,
    pub shadow: Option<Shadow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Shadow { pub blur: f32, pub color: String }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Rect { pub x: i32, pub y: i32, pub w: u32, pub h: u32 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Widget {
    Readout(ReadoutWidget),
    Course(CourseWidget),
    ElevationProfile(ElevationProfileWidget),
}
// Each variant carries id, rect, and its specific fields.
```

**Step 4: Run, verify pass.**

**Step 5: Commit**

```bash
git add crates/layout
git commit -m "Add Layout schema types with serde"
```

---

## Task 14: Layout validation

**Files:**
- Create: `crates/layout/src/validate.rs`

**Step 1: Write failing tests**

- `version != 1` → error
- widget rect overflows canvas → error
- unknown metric string on a `Readout` → error
- valid layout → Ok

**Step 2: Run, verify fails.**

**Step 3: Implement `Layout::validate(&self, available_metrics: &[Metric]) -> Result<Vec<Warning>, ValidationError>`.** `Metric` is an enum in `activity` (add it if not yet there) covering all `Sample` fields. Warnings include "metric X referenced but absent in this activity".

**Step 4: Run, verify pass.**

**Step 5: Commit**

```bash
git add crates/layout crates/activity
git commit -m "Add Layout validation with available-metrics check"
```

---

## Task 15: `render::render_frame` skeleton

**Files:**
- Modify: `crates/render/src/lib.rs`
- Create: `crates/render/src/frame.rs`

**Step 1: Write failing test**

```rust
#[test]
fn empty_layout_renders_transparent() {
    let layout = minimal_layout_no_widgets(100, 100, 30);
    let activity = activity_with_one_sample();
    let mut pix = Pixmap::new(100, 100).unwrap();
    render_frame(&layout, &activity, Duration::ZERO, &mut pix).unwrap();
    // Every pixel fully transparent
    assert!(pix.data().chunks_exact(4).all(|p| p[3] == 0));
}
```

**Step 2: Run, verify fails.**

**Step 3: Implement.** Signature:

```rust
pub fn render_frame(
    layout: &Layout,
    activity: &Activity,
    t: Duration,
    pixmap: &mut tiny_skia::Pixmap,
) -> anyhow::Result<()>
```

Body: `pixmap.fill(Color::TRANSPARENT)`. Dispatch over widgets will be added in following tasks.

**Step 4: Run, verify pass.**

**Step 5: Commit**

```bash
git add crates/render
git commit -m "Add render_frame skeleton with transparent fill"
```

---

## Task 16: Text rendering helper (cosmic-text)

**Files:**
- Create: `crates/render/src/text.rs`

**Step 1: Write failing test**

Render "HELLO" at (10, 50) with size 32 into a 200×100 pixmap; assert at least one pixel in the expected region is non-transparent.

**Step 2: Run, verify fails.**

**Step 3: Implement `TextCtx`** — holds `FontSystem` and `SwashCache` (expensive, build once, reuse). Method `draw(&mut self, pixmap: &mut Pixmap, text: &str, x: f32, y: f32, size: f32, color: Color)` that shapes and blits glyphs.

Move the pre-staged `assets/Inter-VariableFont.ttf` (at repo root) into `crates/render/assets/Inter-VariableFont.ttf` via `git mv`. Load it into `FontSystem` so tests don't depend on system fonts.

**Step 4: Run, verify pass.**

**Step 5: Commit**

```bash
git add crates/render
git commit -m "Add cosmic-text wrapper with bundled Inter font"
```

---

## Task 17: Readout widget

**Files:**
- Create: `crates/render/src/widgets/readout.rs`
- Create: `crates/render/tests/golden/readout_speed.png` (checked in)
- Create: `crates/render/tests/golden.rs`

**Step 1: Write failing golden test**

```rust
#[test]
fn readout_speed_matches_golden() {
    let layout = single_readout_layout("speed", 1.0, 42.5);
    let activity = activity_with_speed(Duration::ZERO, 42.5);
    let mut pix = Pixmap::new(400, 200).unwrap();
    render_frame(&layout, &activity, Duration::ZERO, &mut pix).unwrap();
    assert_golden(&pix, "readout_speed.png");
}
```

`assert_golden` loads the expected PNG, compares channel-wise with tolerance 2, writes `actual.png`/`diff.png` on mismatch.

**Step 2: Run, verify fails** (no readout impl, no golden yet).

**Step 3: Implement `render_readout(w, &ReadoutWidget, activity, t, units, theme)`.** Draw the label (small, accent color, uppercase) at the top of the rect, the value (large, fg) below. Value formatting respects `decimals` and converts to display units. Missing values → "--".

**Step 4: Generate the golden once**

Run the test; on first failure, copy the written `actual.png` to `golden/readout_speed.png`. Re-run; should pass.

**Step 5: Run, verify pass.**

**Step 6: Commit**

```bash
git add crates/render
git commit -m "Add readout widget with golden-image test"
```

---

## Task 18: Course widget (polyline + moving dot)

**Files:**
- Create: `crates/render/src/widgets/course.rs`
- Create: `crates/render/tests/golden/course_mid.png`

**Step 1: Write failing golden test** — rectangular course, dot at t=activity.duration()/2 should sit near the midpoint of the polyline.

**Step 2: Run, verify fails.**

**Step 3: Implement.**
- Compute lat/lon bbox across all samples.
- Project each lat/lon into the widget's `Rect` preserving aspect ratio (center and pad).
- Draw polyline with `tiny-skia::PathBuilder` + `Stroke { width: line_width, .. }`.
- Draw filled circle at the projected position for `activity.sample_at(t)`.

Handle missing lat/lon (indoor activity) → render nothing inside the widget rect (no crash).

**Step 4: Generate golden, run, verify pass.**

**Step 5: Commit**

```bash
git add crates/render
git commit -m "Add course polyline + moving dot widget"
```

---

## Task 19: Elevation profile widget

**Files:**
- Create: `crates/render/src/widgets/elevation_profile.rs`
- Create: `crates/render/tests/golden/elev_mid.png`

**Step 1: Write failing golden test.**

**Step 2: Run, verify fails.**

**Step 3: Implement.**
- X-axis = cumulative distance (sample), Y-axis = altitude (inverted so higher = up).
- Filled area under curve (accent color, half-alpha) + stroke on top.
- Vertical marker at the current sample's distance.

Handle missing altitude → render nothing.

**Step 4: Generate golden, run, verify pass.**

**Step 5: Commit**

```bash
git add crates/render
git commit -m "Add elevation profile widget"
```

---

## Task 20: CLI argument parsing with clap

**Files:**
- Modify: `crates/cli/src/main.rs`
- Create: `crates/cli/src/args.rs`

**Step 1: Write failing test**

Unit test `Args::parse_time_spec("01:23:45")` → `Duration::from_secs(5025)`. Also `"90"` → `Duration::from_secs(90)`, `"02:30"` → `Duration::from_secs(150)`, ISO timestamps → error handled at higher level.

**Step 2: Run, verify fails.**

**Step 3: Implement** clap derive struct with subcommand `Render`, plus a free function `parse_time_spec(&str) -> Result<Duration, _>`. Register size parser for `1920x1080`.

**Step 4: Run, verify pass.**

**Step 5: Commit**

```bash
git add crates/cli
git commit -m "Add clap arg parser and time-spec helper"
```

---

## Task 21: Dry-run mode

**Files:**
- Modify: `crates/cli/src/main.rs`
- Create: `crates/cli/src/dry_run.rs`

**Step 1: Write failing test** — integration test invokes the binary (`assert_cmd`) with `--dry-run` and checks stdout contains "duration", "frames", "widgets".

Add dev-dep: `assert_cmd = "2"`, `predicates = "3"`.

**Step 2: Run, verify fails.**

**Step 3: Implement.** Load activity (calling `prepare()`), load layout, validate, print:

```
Activity: 20.0 s, 20 samples
Available metrics: lat, lon, altitude, distance, speed, gradient, elev_gain
Layout: 3 widgets (1 readout, 1 course, 1 elevation_profile)
Time range: 0:00:00 → 0:00:20 (20.0 s)
Output: overlay.mov, 1920x1080 @ 30 fps (600 frames)
Warnings:
  - widget 'hr_readout' refs metric 'heart_rate' which is absent in activity
```

Exit 0 on success, 2 on parse errors, 1 on validation errors.

**Step 4: Run, verify pass.**

**Step 5: Commit**

```bash
git add crates/cli
git commit -m "Add --dry-run summary and validation report"
```

---

## Task 22: Frame scheduler and reorder buffer

**Files:**
- Create: `crates/cli/src/pipeline.rs`

**Step 1: Write failing test** — push frames indexed `[2, 0, 1, 4, 3]` into the reorder buffer with capacity 8; pop returns them as `[0, 1, 2, 3, 4]`.

**Step 2: Run, verify fails.**

**Step 3: Implement `ReorderBuffer`** — `BTreeMap<u64, Vec<u8>>`, `next_expected: u64`, `cap: usize`. `push(idx, bytes)` blocks (via `Condvar` or a bounded mpsc; use a `Mutex<BTreeMap>` + `Condvar`) if size >= cap. `drain_ready()` pops consecutive entries starting at `next_expected` and returns them.

Also define frame scheduler: iterator of `(idx, t)` pairs over the render range.

**Step 4: Run, verify pass.**

**Step 5: Commit**

```bash
git add crates/cli
git commit -m "Add frame scheduler and capacity-bounded reorder buffer"
```

---

## Task 23: ffmpeg subprocess wrapper

**Files:**
- Create: `crates/cli/src/ffmpeg.rs`

**Step 1: Write failing test** (feature-gated): spawn ffmpeg, write 10 frames of solid red, assert the output file exists and `ffprobe` reports 10 frames, `yuva444p10le`.

**Step 2: Run, verify fails.**

**Step 3: Implement `FfmpegWriter`:**
- `new(width, height, fps, qscale, out_path) -> Result<Self>` — spawns ffmpeg with the args from the design doc; keeps `child` and `stdin` handles.
- `write_frame(&mut self, rgba: &[u8]) -> io::Result<()>` — writes exactly `width * height * 4` bytes to stdin.
- `finish(self) -> Result<()>` — drops stdin, waits for child, checks exit code, returns stderr on failure.

**Step 4: Run with feature, verify pass.**

```bash
cargo test -p gpx-overlay --features ffmpeg-tests
```

**Step 5: Commit**

```bash
git add crates/cli
git commit -m "Add ffmpeg subprocess writer"
```

---

## Task 24: Parallel render loop with progress bar

**Files:**
- Modify: `crates/cli/src/main.rs`
- Modify: `crates/cli/src/pipeline.rs`

**Step 1: Write failing test** (feature-gated) — render 2 seconds of `short.gpx` with the example layout to a temp `.mov`; ffprobe: 60 frames, 1920×1080, ProRes, alpha channel present.

**Step 2: Run, verify fails.**

**Step 3: Implement.** Orchestration in `cmd_render`:
1. Load + prepare activity
2. Load + validate layout (merge CLI canvas overrides)
3. Compute frame range
4. Build `indicatif::ProgressBar`
5. Spawn `FfmpegWriter`
6. Rayon `par_bridge` over the scheduler iterator. Per-thread `Pixmap` via `thread_local!`. For each `(idx, t)`: render into pixmap, `buffer.push(idx, pixmap.data().to_vec())`, increment progress.
7. A dedicated flusher thread drains the reorder buffer and calls `writer.write_frame` in order.
8. After render join, `writer.finish()`.

**Step 4: Run with feature, verify pass.**

**Step 5: Commit**

```bash
git add crates/cli
git commit -m "Wire up parallel render loop with ffmpeg output"
```

---

## Task 25: End-to-end integration test + example layout

**Files:**
- Create: `examples/layout.json`
- Create: `crates/cli/tests/end_to_end.rs`

**Step 1: Write the example layout.**

Include one readout per metric available in `short.gpx` (speed, distance, elevation, elevation gain, gradient, elapsed time), a course widget, and an elevation profile widget.

**Step 2: Write failing test** — spawns the binary with `short.gpx`, `layout.json`, outputs to a temp path, asserts ffprobe.

**Step 3: Run with feature, verify pass.**

```bash
cargo test -p gpx-overlay --features ffmpeg-tests end_to_end
```

**Step 4: Commit**

```bash
git add crates/cli examples/layout.json
git commit -m "Add end-to-end test with example layout"
```

---

## Task 26: README and usage docs

**Files:**
- Create: `README.md`

**Step 1: Write minimum README** — install prereqs (Rust, ffmpeg), `cargo build --release`, usage example, link to the design doc and example layout. No screenshots for v1.

**Step 2: Commit**

```bash
git add README.md
git commit -m "Add README with install and usage"
```

---

## Verification checklist before declaring v1 done

- [ ] `cargo test --workspace` passes (all non-ffmpeg tests)
- [ ] `cargo test --workspace --features ffmpeg-tests` passes
- [ ] `cargo clippy --workspace -- -D warnings` clean
- [ ] `cargo fmt --check` clean
- [ ] Golden images exist for all three widget types
- [ ] `gpx-overlay render --input examples/short.gpx --layout examples/layout.json --output /tmp/out.mov` produces a playable transparent .mov in a video editor (manual verification — document outcome)
- [ ] `--dry-run` output matches expected format
