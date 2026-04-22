# Meter and Gauge widgets — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Ship two new visualization widgets — `Widget::Meter` (linear bar) and `Widget::Gauge` (radial arc) — for instantaneous scalar metrics, with configurable indicator style (fill / rect / arrow / needle, optionally combined with a filled track) and tick marks (major + minor, numbers on majors).

**Architecture:** Follows the existing widget pattern. Schema additions land in the `layout` crate; rendering lands in `render::widgets::meter` and `render::widgets::gauge`, with shared scale math in `render::widgets::scale`. Dispatch hooks into the existing `render_frame` match. No changes to `activity` or `cli` crates beyond the pattern-match arms `serde` handles for free.

**Tech Stack:** Rust 2021, `tiny-skia` 0.11 (existing dep), `serde` (existing), cosmic-text for value labels (existing via `TextCtx`).

**Design doc:** `docs/plans/2026-04-22-meter-gauge-design.md`.

**Prerequisite:** Tasks run from the project root (no worktree for this feature — user elected to land on `main` directly). Each task has a single logical commit at the end.

---

## Testing strategy recap

- **Unit tests** for pure math in `render::widgets::scale` — `frac`, `nice_major_interval`, `tick_values`, `to_skia_angle`, `angle_lerp`.
- **Per-widget unit tests** verify non-pixel properties — no crashes on missing metric, returns early on degenerate ranges, etc.
- **Golden-image tests** (existing `assert_golden` helper) for visual correctness. One per widget type to start; add variant coverage in later tasks.
- **Schema round-trip tests** for serde on the new sub-types and Widget variants.

---

## Task 1: Add shared sub-types to layout crate

**Files:**
- Modify: `crates/layout/src/lib.rs`

**Step 1: Write the failing test**

At the bottom of the existing `#[cfg(test)] mod tests { ... }` block in `crates/layout/src/lib.rs`:

```rust
#[test]
fn orientation_serde_snake_case() {
    let j = serde_json::to_string(&Orientation::Horizontal).unwrap();
    assert_eq!(j, "\"horizontal\"");
    let j = serde_json::to_string(&Orientation::Vertical).unwrap();
    assert_eq!(j, "\"vertical\"");
    let h: Orientation = serde_json::from_str("\"horizontal\"").unwrap();
    assert_eq!(h, Orientation::Horizontal);
}

#[test]
fn indicator_defaults_to_fill() {
    let ind: Indicator = serde_json::from_str("{}").unwrap();
    assert_eq!(ind.kind, IndicatorKind::Fill);
    assert!(!ind.fill_under);
}

#[test]
fn indicator_kind_roundtrip() {
    for (name, k) in [
        ("fill", IndicatorKind::Fill),
        ("rect", IndicatorKind::Rect),
        ("arrow", IndicatorKind::Arrow),
        ("needle", IndicatorKind::Needle),
    ] {
        let quoted = format!("\"{}\"", name);
        let parsed: IndicatorKind = serde_json::from_str(&quoted).unwrap();
        assert_eq!(parsed, k);
        assert_eq!(serde_json::to_string(&k).unwrap(), quoted);
    }
}

#[test]
fn ticks_defaults() {
    let t: Ticks = serde_json::from_str("{}").unwrap();
    assert_eq!(t.major_every, None);
    assert_eq!(t.minor_every, None);
    assert!(t.show_numbers);
    assert_eq!(t.decimals, 0);
}
```

**Step 2: Run, verify fails**

Run: `cargo test -p layout orientation_serde_snake_case`
Expected: FAIL (undefined types).

**Step 3: Implement**

Add near the top of `crates/layout/src/lib.rs`, just above the `Widget` enum:

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Orientation {
    #[default]
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum IndicatorKind {
    #[default]
    Fill,
    Rect,
    Arrow,
    Needle,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub struct Indicator {
    #[serde(default)]
    pub kind: IndicatorKind,
    #[serde(default)]
    pub fill_under: bool,
}

fn default_show_numbers() -> bool { true }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Ticks {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub major_every: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minor_every: Option<f32>,
    #[serde(default = "default_show_numbers")]
    pub show_numbers: bool,
    #[serde(default)]
    pub decimals: u32,
}

impl Default for Ticks {
    fn default() -> Self {
        Self {
            major_every: None,
            minor_every: None,
            show_numbers: true,
            decimals: 0,
        }
    }
}
```

**Step 4: Run, verify pass**

Run: `cargo test -p layout`
Expected: all new + existing tests pass.

**Step 5: Commit**

```bash
git add crates/layout/src/lib.rs
git commit -m "Add Orientation, Indicator, Ticks sub-types for new widgets"
```

---

## Task 2: Add Widget::Meter and Widget::Gauge variants

**Files:**
- Modify: `crates/layout/src/lib.rs` (enum + helpers)
- Modify: `crates/layout/src/validate.rs` (Widget pattern matches if any)
- Modify: `crates/render/src/frame.rs` (empty dispatch arms so it compiles)

**Step 1: Write the failing test**

Extend the existing `tests` module in `crates/layout/src/lib.rs`:

```rust
#[test]
fn meter_round_trip() {
    let json = r#"{
        "type": "meter",
        "id": "spd",
        "metric": "speed",
        "rect": { "x": 0, "y": 0, "w": 100, "h": 20 },
        "min": 0.0,
        "max": 60.0
    }"#;
    let w: Widget = serde_json::from_str(json).unwrap();
    match w {
        Widget::Meter { metric, min, max, orientation, .. } => {
            assert_eq!(metric, "speed");
            assert_eq!(min, 0.0);
            assert_eq!(max, 60.0);
            assert_eq!(orientation, Orientation::Horizontal); // default
        }
        _ => panic!("expected Meter"),
    }
}

#[test]
fn gauge_defaults_to_classic_sweep() {
    let json = r#"{
        "type": "gauge",
        "id": "spd_g",
        "metric": "speed",
        "rect": { "x": 0, "y": 0, "w": 200, "h": 200 },
        "min": 0.0,
        "max": 60.0
    }"#;
    let w: Widget = serde_json::from_str(json).unwrap();
    match w {
        Widget::Gauge { start_deg, end_deg, .. } => {
            assert_eq!(start_deg, -135.0);
            assert_eq!(end_deg, 135.0);
        }
        _ => panic!("expected Gauge"),
    }
}
```

**Step 2: Run, verify fails**

Run: `cargo test -p layout meter_round_trip`
Expected: FAIL (undefined variants).

**Step 3: Implement**

In `crates/layout/src/lib.rs`, add helpers and extend the `Widget` enum:

```rust
fn default_gauge_start_deg() -> f32 { -135.0 }
fn default_gauge_end_deg() -> f32 { 135.0 }
```

Add variants to the existing `Widget` enum (preserve existing variants):

```rust
Meter {
    id: String,
    metric: String,
    rect: Rect,
    min: f32,
    max: f32,
    #[serde(default)]
    orientation: Orientation,
    #[serde(default)]
    indicator: Indicator,
    #[serde(default)]
    ticks: Ticks,
    #[serde(default)]
    show_value: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    value_font_size: Option<f32>,
},
Gauge {
    id: String,
    metric: String,
    rect: Rect,
    min: f32,
    max: f32,
    #[serde(default = "default_gauge_start_deg")]
    start_deg: f32,
    #[serde(default = "default_gauge_end_deg")]
    end_deg: f32,
    #[serde(default)]
    indicator: Indicator,
    #[serde(default)]
    ticks: Ticks,
    #[serde(default)]
    show_value: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    value_font_size: Option<f32>,
},
```

Extend `Widget::id()` and `Widget::rect()`:

```rust
pub fn id(&self) -> &str {
    match self {
        Widget::Readout { id, .. }
        | Widget::Course { id, .. }
        | Widget::ElevationProfile { id, .. }
        | Widget::Bar { id, .. }
        | Widget::Meter { id, .. }
        | Widget::Gauge { id, .. } => id,
    }
}

pub fn rect(&self) -> Rect {
    match self {
        Widget::Readout { rect, .. }
        | Widget::Course { rect, .. }
        | Widget::ElevationProfile { rect, .. }
        | Widget::Bar { rect, .. }
        | Widget::Meter { rect, .. }
        | Widget::Gauge { rect, .. } => *rect,
    }
}
```

In `crates/render/src/frame.rs`, extend the match in `render_frame` with empty arms (bodies land in later tasks):

```rust
Widget::Meter { .. } => {
    // Implemented in Task 4.
}
Widget::Gauge { .. } => {
    // Implemented in Task 6.
}
```

**Step 4: Run, verify pass**

Run: `cargo test --workspace`
Expected: new tests pass; existing tests unaffected.

**Step 5: Commit**

```bash
git add crates/layout/src/lib.rs crates/render/src/frame.rs
git commit -m "Add Widget::Meter and Widget::Gauge schema variants"
```

---

## Task 3: Shared scale math module

**Files:**
- Create: `crates/render/src/widgets/scale.rs`
- Modify: `crates/render/src/widgets/mod.rs` (add `pub mod scale;`)

**Step 1: Write the failing tests**

Create `crates/render/src/widgets/scale.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frac_clamps() {
        assert_eq!(frac(-5.0, 0.0, 100.0), 0.0);
        assert_eq!(frac(50.0, 0.0, 100.0), 0.5);
        assert_eq!(frac(150.0, 0.0, 100.0), 1.0);
    }

    #[test]
    fn nice_interval_picks_round_numbers() {
        assert_eq!(nice_major_interval(0.0, 100.0), 20.0);
        assert_eq!(nice_major_interval(0.0, 60.0), 10.0);
        assert_eq!(nice_major_interval(0.0, 5.0), 1.0);
        assert_eq!(nice_major_interval(0.0, 0.3), 0.05);
        assert_eq!(nice_major_interval(0.0, 200.0), 50.0);
    }

    #[test]
    fn tick_values_inclusive() {
        let vs: Vec<f32> = tick_values(0.0, 10.0, 2.0).collect();
        assert_eq!(vs, vec![0.0, 2.0, 4.0, 6.0, 8.0, 10.0]);
    }

    #[test]
    fn to_skia_angle_conversions() {
        // User: 0° = up, clockwise. Skia: 0° = right, counterclockwise.
        assert!((to_skia_angle(0.0) - 90.0).abs() < 1e-4);
        assert!((to_skia_angle(90.0) - 0.0).abs() < 1e-4);
        assert!((to_skia_angle(-90.0) - 180.0).abs() < 1e-4);
        assert!((to_skia_angle(180.0) - (-90.0)).abs() < 1e-4);
    }

    #[test]
    fn angle_lerp_no_wrap() {
        assert!((angle_lerp(-135.0, 135.0, 0.5) - 0.0).abs() < 1e-4);
        assert!((angle_lerp(-135.0, 135.0, 0.0) - (-135.0)).abs() < 1e-4);
        assert!((angle_lerp(-135.0, 135.0, 1.0) - 135.0).abs() < 1e-4);
    }

    #[test]
    fn angle_lerp_wraps_through_top() {
        // Start 315, end 45 should sweep through top (i.e., 315 -> 360 -> 45).
        let mid = angle_lerp(315.0, 45.0, 0.5);
        // At frac=0.5 we expect 0° (top), possibly reported as 360.0.
        assert!(mid.rem_euclid(360.0) < 1e-4 || (mid.rem_euclid(360.0) - 360.0).abs() < 1e-4);
    }
}
```

**Step 2: Run, verify fails**

Run: `cargo test -p render scale`
Expected: FAIL — compile error, functions undefined.

**Step 3: Implement**

Prepend the test module with:

```rust
/// Fraction of `v` between `min` and `max`, clamped to [0, 1].
pub(crate) fn frac(v: f32, min: f32, max: f32) -> f32 {
    if max <= min {
        return 0.0;
    }
    ((v - min) / (max - min)).clamp(0.0, 1.0)
}

/// Pick a "nice" major-tick interval that divides `max - min` into roughly
/// 6 segments and lands on a round number (1, 2, 5, 10, 20, 50, ...).
pub(crate) fn nice_major_interval(min: f32, max: f32) -> f32 {
    let range = (max - min).abs();
    if range <= 0.0 {
        return 1.0;
    }
    let raw = range / 6.0;
    let magnitude = 10f32.powf(raw.log10().floor());
    let normalized = raw / magnitude;
    let nice = if normalized < 1.5 {
        1.0
    } else if normalized < 3.0 {
        2.0
    } else if normalized < 7.0 {
        5.0
    } else {
        10.0
    };
    nice * magnitude
}

/// Walk values from `min` to `max` inclusive at `step`. Avoids accumulation
/// error by integer indexing.
pub(crate) fn tick_values(min: f32, max: f32, step: f32) -> impl Iterator<Item = f32> {
    let n = ((max - min) / step).round() as i64;
    (0..=n).map(move |k| min + (k as f32) * step)
}

/// Convert from user-facing angle (0° up, clockwise) to tiny-skia's
/// (0° right, counterclockwise).
pub(crate) fn to_skia_angle(deg_up_cw: f32) -> f32 {
    90.0 - deg_up_cw
}

/// Linearly interpolate angles from `start_deg` to `end_deg` at `frac`
/// in [0, 1]. If `end_deg < start_deg`, adds 360° so the sweep wraps
/// clockwise through the top.
pub(crate) fn angle_lerp(start_deg: f32, end_deg: f32, frac: f32) -> f32 {
    let end_eff = if end_deg >= start_deg {
        end_deg
    } else {
        end_deg + 360.0
    };
    start_deg + (end_eff - start_deg) * frac
}
```

In `crates/render/src/widgets/mod.rs`, add `pub mod scale;`.

**Step 4: Run, verify pass**

Run: `cargo test -p render`
Expected: all 5 new tests pass; existing tests unaffected.

**Step 5: Commit**

```bash
git add crates/render/src/widgets/scale.rs crates/render/src/widgets/mod.rs
git commit -m "Add scale helpers shared by meter and gauge"
```

---

## Task 4: Meter widget — horizontal fill

**Files:**
- Create: `crates/render/src/widgets/meter.rs`
- Modify: `crates/render/src/widgets/mod.rs` (add `pub mod meter;`)
- Modify: `crates/render/src/frame.rs` (wire the dispatch arm)
- Create: `crates/render/tests/golden/meter_speed_fill.png` (generated, then committed)
- Modify: `crates/render/tests/golden.rs` (new test)

**Scope**: horizontal orientation only, `Fill` indicator only, no ticks, no numbers, no markers, no value label. Just the colored track. Subsequent tasks add the rest.

**Step 1: Write failing golden test**

Append to `crates/render/tests/golden.rs`:

```rust
#[test]
fn meter_speed_fill_matches_golden() {
    use activity::{Activity, Sample};
    use chrono::{TimeZone, Utc};
    use layout::{
        Canvas, DistanceUnit, ElevationUnit, Layout, Rect, SpeedUnit, TempUnit, Theme, Units, Widget,
    };
    use render::{render_frame, TextCtx};
    use std::time::Duration;
    use tiny_skia::{Color, Pixmap};

    let layout = Layout {
        version: 1,
        canvas: Canvas { width: 600, height: 80, fps: 30 },
        units: Units {
            speed: SpeedUnit::Kmh, distance: DistanceUnit::Km,
            elevation: ElevationUnit::M, temp: TempUnit::C,
        },
        theme: Theme {
            font: "Inter".into(), fg: "#ffffff".into(),
            accent: "#ffcc00".into(), shadow: None,
        },
        rider: None,
        widgets: vec![Widget::Meter {
            id: "spd".into(),
            metric: "speed".into(),
            rect: Rect { x: 20, y: 20, w: 560, h: 40 },
            min: 0.0,
            max: 60.0,
            orientation: layout::Orientation::Horizontal,
            indicator: layout::Indicator::default(), // Fill
            ticks: layout::Ticks::default(),
            show_value: false,
            value_font_size: None,
        }],
    };

    let sample = Sample {
        t: Duration::ZERO, lat: 0.0, lon: 0.0,
        altitude_m: None, speed_mps: Some(30.0 / 3.6), // 30 km/h => 50%
        heart_rate_bpm: None, cadence_rpm: None, power_w: None,
        distance_m: None, elev_gain_cum_m: None, gradient_pct: None,
    };
    let activity = Activity::from_samples(Utc.timestamp_opt(0, 0).unwrap(), vec![sample]);

    let mut ctx = TextCtx::new();
    let mut pix = Pixmap::new(600, 80).unwrap();
    render_frame(&layout, &activity, Duration::ZERO, &mut ctx, &mut pix, Color::TRANSPARENT).unwrap();

    assert_golden(&pix, "meter_speed_fill.png");
}
```

**Step 2: Run, verify fails**

Run: `cargo test -p render --test golden meter_speed_fill`
Expected: FAIL (`Widget::Meter` dispatch is a no-op so the output is entirely transparent; golden doesn't exist; assert_golden panics with "wrote new golden").

**Step 3: Implement**

Create `crates/render/src/widgets/meter.rs`:

```rust
use activity::{Activity, Metric};
use layout::{Indicator, IndicatorKind, Orientation, Rect, Theme, Ticks};
use std::time::Duration;
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Transform};

use crate::widgets::scale::frac;

/// Render a linear meter widget.
#[allow(clippy::too_many_arguments)]
pub fn render_meter(
    pixmap: &mut Pixmap,
    theme: &Theme,
    rect: Rect,
    metric_name: &str,
    min: f32,
    max: f32,
    orientation: Orientation,
    indicator: Indicator,
    _ticks: Ticks,     // Task 5
    _show_value: bool, // Task 6
    activity: &Activity,
    t: Duration,
) {
    let _ = orientation; // Task 5 adds vertical support.

    let Some(metric) = Metric::from_str(metric_name) else { return; };
    let sample = activity.sample_at(t);
    let Some(current) = pull_value(metric, &sample) else { return; };

    let fg = super::parse_hex(&theme.fg).unwrap_or(Color::WHITE);
    let accent = super::parse_hex(&theme.accent).unwrap_or(fg);

    let f = frac(current, min, max);

    // Track geometry: centered band within the rect.
    let thickness = (rect.h as f32 * 0.5).min(rect.w as f32 * 0.5);
    let track_y = rect.y as f32 + (rect.h as f32 - thickness) * 0.5;
    let track_x = rect.x as f32;
    let track_w = rect.w as f32;

    // Fill.
    if matches!(indicator.kind, IndicatorKind::Fill) || indicator.fill_under {
        let filled_w = track_w * f;
        if filled_w > 0.0 {
            let mut pb = PathBuilder::new();
            pb.push_rect(tiny_skia::Rect::from_xywh(track_x, track_y, filled_w, thickness).unwrap());
            if let Some(path) = pb.finish() {
                let mut paint = Paint::default();
                paint.set_color(accent);
                paint.anti_alias = true;
                pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
            }
        }
    }

    // Track outline.
    let mut pb = PathBuilder::new();
    pb.push_rect(tiny_skia::Rect::from_xywh(track_x, track_y, track_w, thickness).unwrap());
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(fg);
        paint.anti_alias = true;
        let stroke = tiny_skia::Stroke { width: 1.5, ..Default::default() };
        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }
}

fn pull_value(m: Metric, s: &activity::Sample) -> Option<f32> {
    match m {
        Metric::Speed => s.speed_mps.map(|v| v * 3.6), // km/h for now; unit conversion in Task 5
        Metric::HeartRate => s.heart_rate_bpm.map(|v| v as f32),
        Metric::Power => s.power_w.map(|v| v as f32),
        Metric::Cadence => s.cadence_rpm.map(|v| v as f32),
        Metric::Altitude => s.altitude_m,
        Metric::Distance => s.distance_m.map(|v| v as f32),
        Metric::Gradient => s.gradient_pct,
        Metric::ElevGain => s.elev_gain_cum_m,
        _ => None,
    }
}
```

Add `pub mod meter;` to `crates/render/src/widgets/mod.rs`.

Update `crates/render/src/frame.rs` Meter arm:

```rust
Widget::Meter {
    id: _,
    metric,
    rect,
    min,
    max,
    orientation,
    indicator,
    ticks,
    show_value,
    value_font_size: _,
} => {
    crate::widgets::meter::render_meter(
        pixmap, &layout.theme, *rect, metric, *min, *max,
        *orientation, *indicator, *ticks, *show_value,
        activity, t,
    );
}
```

**Step 4: Generate golden + pass**

Run: `cargo test -p render --test golden meter_speed_fill`
First run: FAIL, writes `tests/golden/meter_speed_fill.png`. Inspect the PNG — expect a white-outlined rectangle with the left 50% filled yellow.

Re-run: PASS.

**Step 5: Commit**

```bash
git add crates/render/src/widgets/meter.rs crates/render/src/widgets/mod.rs crates/render/src/frame.rs crates/render/tests/golden.rs crates/render/tests/golden/meter_speed_fill.png
git commit -m "Add Meter widget skeleton with horizontal fill"
```

---

## Task 5: Meter widget — ticks, numbers, vertical, unit conversion

**Files:**
- Modify: `crates/render/src/widgets/meter.rs`
- Regenerate: `crates/render/tests/golden/meter_speed_fill.png` (ticks now draw)

**Step 1: Extend behavior (non-TDD — golden captures the visual)**

- Handle `Orientation::Vertical`: track runs along the y axis, fills bottom → top.
- Draw major + minor ticks (using `scale::nice_major_interval` for auto defaults).
- Draw tick numbers (using `TextCtx`). Numbers use `ticks.decimals` for formatting.
- Apply per-metric unit conversion (speed m/s → km/h or mph via `Units`).

Update `render_meter` signature to take `units: &layout::Units` and `text_ctx: &mut crate::TextCtx` — thread both from `frame.rs`.

Full implementation notes (keep to the design doc):

```rust
// Draw minor ticks first (shorter), then majors (longer + number).
// Horizontal: tick is a vertical line segment below the track.
// Vertical: tick is a horizontal line segment to the right of the track.
// Tick length: major = thickness * 0.5, minor = thickness * 0.25.
// Number placement: 4px gap past the far end of the major tick.
```

Unit conversion: reuse the same speed/distance/elevation conversions already in `readout.rs`. Extract `format_metric_scalar(m, value_in_metric_base, units) -> (String, &'static str)` into a shared helper (`render::widgets::format`) if needed.

**Step 2: Add a failing test for vertical orientation**

```rust
#[test]
fn meter_power_vertical_matches_golden() {
    // similar setup, orientation: Vertical, metric: power,
    // indicator: { kind: Needle, fill_under: true }, etc.
}
```

(Golden PNG to be generated on first run.)

**Step 3: Implement → generate goldens → verify pass**

Run: `cargo test -p render --test golden`
On first run with the new widget behavior, the existing `meter_speed_fill.png` must be regenerated (it now has ticks). Delete it, re-run twice.

**Step 4: Verify all workspace tests pass**

Run: `cargo test --workspace`

**Step 5: Commit**

```bash
git add crates/render/src/widgets/meter.rs crates/render/src/frame.rs crates/render/tests/golden.rs crates/render/tests/golden/meter_speed_fill.png crates/render/tests/golden/meter_power_vertical.png
git commit -m "Add ticks, numbers, vertical orientation to Meter"
```

---

## Task 6: Meter widget — remaining indicator variants (rect, arrow, needle) + show_value

**Files:**
- Modify: `crates/render/src/widgets/meter.rs`

**Scope**: implement `IndicatorKind::Rect`, `Arrow`, `Needle`. Honor `indicator.fill_under`. Implement `show_value` (centered text above the track for horizontal, beside the track for vertical).

**Step 1: Write failing tests**

Unit tests for the marker geometry:

```rust
#[test]
fn rect_marker_centered_on_value() {
    // Compute the center x of the rect marker when current = halfway.
    // Assert it lands at rect.x + rect.w / 2.
}

#[test]
fn show_value_places_text_inside_rect() {
    // Render to a pixmap, then assert at least one non-transparent pixel
    // in the expected text region.
}
```

Adjust scope of the tests to what's realistic — golden images are the primary visual check; unit tests just catch regressions on math.

**Step 2: Implement**

Add helper functions for each marker shape. Horizontal versions first, vertical is a rotation-equivalent swap.

Pattern for `Rect` marker (horizontal):

```rust
let pos_x = track_x + track_w * f;
let marker_w = thickness * 0.2;
let rect_marker = tiny_skia::Rect::from_xywh(
    pos_x - marker_w / 2.0, track_y, marker_w, thickness,
).unwrap();
// Fill as before.
```

`Arrow` (horizontal, pointing down at track): triangle whose tip is at `(pos_x, track_y)` and base sits above the track. Use `PathBuilder::move_to` / `line_to` / `close`.

`Needle` (horizontal): perpendicular stroke across the track, extending `thickness * 0.4` beyond top and bottom.

**Step 3: Add golden for each marker type**

Consider just one combined golden with all three markers stacked (three meters in the same image) if you prefer tighter golden coverage.

**Step 4: Run, verify pass**

Run: `cargo test --workspace`
Expected: all golden tests pass.

**Step 5: Commit**

```bash
git add crates/render/src/widgets/meter.rs crates/render/tests/golden/
git commit -m "Add rect, arrow, needle indicators and show_value to Meter"
```

---

## Task 7: Gauge widget — arc + fill indicator

**Files:**
- Create: `crates/render/src/widgets/gauge.rs`
- Modify: `crates/render/src/widgets/mod.rs` (add `pub mod gauge;`)
- Modify: `crates/render/src/frame.rs` (wire Gauge arm)
- Modify: `crates/render/tests/golden.rs` (new test)

**Scope**: arc + fill indicator only. No ticks yet, no markers, no value label. Default 270° sweep.

**Step 1: Write failing golden test**

```rust
#[test]
fn gauge_speed_fill_matches_golden() {
    let layout = Layout { /* 300x300 canvas, Widget::Gauge with metric=speed, min=0, max=60 */ };
    // activity with speed = 30 km/h = 50%
    // render, assert_golden("gauge_speed_fill.png")
}
```

**Step 2: Implement**

Create `crates/render/src/widgets/gauge.rs`:

```rust
use activity::{Activity, Metric};
use layout::{Indicator, IndicatorKind, Rect, Theme, Ticks};
use std::time::Duration;
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Stroke, Transform};

use crate::widgets::scale::{angle_lerp, frac, to_skia_angle};

#[allow(clippy::too_many_arguments)]
pub fn render_gauge(
    pixmap: &mut Pixmap,
    theme: &Theme,
    rect: Rect,
    metric_name: &str,
    min: f32,
    max: f32,
    start_deg: f32,
    end_deg: f32,
    indicator: Indicator,
    _ticks: Ticks,     // Task 8
    _show_value: bool, // Task 9
    activity: &Activity,
    t: Duration,
) {
    let Some(metric) = Metric::from_str(metric_name) else { return; };
    let sample = activity.sample_at(t);
    let Some(current) = super::meter::pull_value(metric, &sample) else { return; };

    let fg = super::parse_hex(&theme.fg).unwrap_or(Color::WHITE);
    let accent = super::parse_hex(&theme.accent).unwrap_or(fg);

    let f = frac(current, min, max);

    // Center + radius.
    let cx = rect.x as f32 + rect.w as f32 * 0.5;
    let cy = rect.y as f32 + rect.h as f32 * 0.5;
    let padding = 16.0;
    let r_outer = (rect.w.min(rect.h) as f32) * 0.5 - padding;
    let thickness = r_outer * 0.15;
    let r_inner = r_outer - thickness;

    // Track: draw full arc in fg.
    draw_arc_stroke(pixmap, cx, cy, r_outer - thickness * 0.5, thickness, start_deg, end_deg, fg);

    // Fill or fill_under: draw filled arc from start up to current.
    if matches!(indicator.kind, IndicatorKind::Fill) || indicator.fill_under {
        let cur_deg = angle_lerp(start_deg, end_deg, f);
        draw_arc_stroke(pixmap, cx, cy, r_outer - thickness * 0.5, thickness, start_deg, cur_deg, accent);
    }

    let _ = r_inner; // Used by Task 9 center label layout.
}

fn draw_arc_stroke(
    pixmap: &mut Pixmap,
    cx: f32, cy: f32,
    radius: f32,
    stroke_w: f32,
    start_deg_user: f32, end_deg_user: f32,
    color: Color,
) {
    use crate::widgets::scale::to_skia_angle;

    // tiny-skia's PathBuilder doesn't have a native arc; we approximate with
    // a polyline. ~2 degrees per segment is smooth at overlay resolutions.
    let user_start = start_deg_user;
    let user_end = if end_deg_user >= start_deg_user { end_deg_user } else { end_deg_user + 360.0 };
    let steps = (((user_end - user_start).abs() / 2.0).ceil() as i32).max(1);

    let mut pb = PathBuilder::new();
    for i in 0..=steps {
        let u = user_start + (user_end - user_start) * (i as f32 / steps as f32);
        let s = to_skia_angle(u).to_radians();
        let x = cx + radius * s.cos();
        let y = cy - radius * s.sin(); // y axis flipped for screen coords
        if i == 0 { pb.move_to(x, y); } else { pb.line_to(x, y); }
    }
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;
        let stroke = Stroke { width: stroke_w, line_cap: tiny_skia::LineCap::Butt, ..Default::default() };
        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }
}
```

Note the y-axis flip: tiny-skia uses screen coords (y down), and cosmic-text / our user convention works upward. The formula `y = cy - r * sin(skia_angle)` maps the standard math convention onto pixmap pixels so "0° up" really is up.

Make `meter::pull_value` `pub(crate)` or move it to a shared helper.

**Step 3: Wire dispatch in frame.rs Gauge arm**.

**Step 4: Generate golden, pass**

First run: writes `gauge_speed_fill.png`. Inspect — expect a 270° arc (missing bottom), with the first 50% (from -135° clockwise to 0°/top) in accent yellow and the rest in fg white.

**Step 5: Commit**

```bash
git add crates/render/src/widgets/gauge.rs crates/render/src/widgets/mod.rs crates/render/src/frame.rs crates/render/src/widgets/meter.rs crates/render/tests/golden.rs crates/render/tests/golden/gauge_speed_fill.png
git commit -m "Add Gauge widget skeleton with fill indicator"
```

---

## Task 8: Gauge widget — radial ticks and numbers

**Files:**
- Modify: `crates/render/src/widgets/gauge.rs`

**Scope**: Major + minor ticks drawn as radial line segments outside the arc. Numbers on majors, rotated to be tangent to the arc and upright (clamp rotation to [-90°, 90°], flip past that).

**Step 1: Add to `render_gauge` after the track stroke but before the indicator.**

Pseudocode:

```rust
let major_every = ticks.major_every.unwrap_or_else(|| nice_major_interval(min, max));
let minor_every = ticks.minor_every.unwrap_or(major_every / 5.0);

for v in tick_values(min, max, minor_every) {
    let vf = frac(v, min, max);
    let deg = angle_lerp(start_deg, end_deg, vf);
    let skia_rad = to_skia_angle(deg).to_radians();
    let is_major = (((v - min) / major_every).round() - ((v - min) / major_every)).abs() < 1e-4;
    let tick_len = if is_major { thickness * 0.6 } else { thickness * 0.3 };
    // line from (cx + r_outer*cos, cy - r_outer*sin) outward by tick_len
    // draw with fg color, stroke width 1.5
    if is_major && ticks.show_numbers {
        // format v with ticks.decimals, measure width, rotate upright, place just past the tick
    }
}
```

Handle the number-rotation upright rule: if `deg` normalized to (-180, 180] has absolute value > 90°, add 180° to the glyph rotation so text isn't upside-down. TextCtx currently has no rotation support — this adds a small responsibility. Two options:

A. Extend `TextCtx::draw` with an optional rotation parameter.
B. Skip rotation in v1; render numbers horizontal. Tick placement still radial, but numbers read left-to-right. Uglier on the far sides of the arc but zero new plumbing.

**Recommend option B for first pass.** Extending TextCtx is a separate task; ship the 270° gauge with horizontal-text numbers and revisit if visually bad.

**Step 2: Write (or regenerate) goldens.**

Golden `gauge_speed_fill.png` now has ticks — regenerate.

**Step 3: Commit**

```bash
git add crates/render/src/widgets/gauge.rs crates/render/tests/golden/gauge_speed_fill.png
git commit -m "Add ticks and numbers to Gauge"
```

---

## Task 9: Gauge widget — markers (rect, arrow, needle) + show_value

**Files:**
- Modify: `crates/render/src/widgets/gauge.rs`

**Marker placement**: all three markers use the angle `deg = angle_lerp(start_deg, end_deg, frac)`.

- **Rect**: tangent to the arc, centered on the tick radius. Pre-rotation rect `(−w/2, −h/2, w, h)` where `w = thickness * 0.3`, `h = thickness`. Rotate by `(90° − deg)` so the rect's long axis is radial. Translate to the arc position.
- **Arrow**: triangle with tip on the arc pointing inward. Base sits `thickness * 0.6` outside the arc. Same rotation math.
- **Needle**: line from center to arc + small overshoot. `(x1, y1) = center`, `(x2, y2) = center + r * unit_vec(deg)`.

Use `tiny_skia::Transform::from_rotate_at` for rotations.

**show_value**: render current value centered at `(cx, cy)` with the formatter. Default font size = `min(rect.w, rect.h) * 0.15`. Value_font_size override honored.

**Step 1: Add failing goldens for at least two variants.**

Proposed: `gauge_speed_classic.png` (needle + fill_under + show_value), `gauge_power_arrow.png` (arrow, no fill, 360° sweep).

**Step 2: Implement, generate goldens, pass.**

**Step 3: Commit**

```bash
git add crates/render/src/widgets/gauge.rs crates/render/tests/golden/gauge_speed_classic.png crates/render/tests/golden/gauge_power_arrow.png
git commit -m "Add markers, fill_under combined, show_value to Gauge"
```

---

## Task 10: Polish — example layout + README

**Files:**
- Modify: `examples/layout-cycling.json` (add a Meter and a Gauge to demonstrate)
- Modify: `README.md` (mention the new widgets)

**Step 1: Add widgets to `examples/layout-cycling.json`.** Pick positions that don't collide with existing widgets. E.g., a horizontal speed meter at the top of the screen and a radial power gauge in an empty quadrant.

**Step 2: Dry-run validates.**

```bash
cargo run --release -p gpx-overlay -- render --input examples/ride.fit --layout examples/layout-cycling.json --output /tmp/out.mov --dry-run
```

**Step 3: Update README** — add the two new widget type names to the widget list and link to the design/impl docs.

**Step 4: Run the full test suite one more time.**

```bash
cargo test --workspace
cargo test --workspace --features gpx-overlay/ffmpeg-tests
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

Expected: everything green.

**Step 5: Commit**

```bash
git add examples/layout-cycling.json README.md
git commit -m "Wire Meter and Gauge into example layout and README"
```

---

## Verification checklist before declaring done

- [ ] `cargo test --workspace` passes
- [ ] `cargo test --workspace --features gpx-overlay/ffmpeg-tests` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] `cargo fmt --check` clean
- [ ] All four new goldens exist in `crates/render/tests/golden/`
- [ ] `examples/layout-cycling.json` exercises both new widgets and validates via `--dry-run`
- [ ] Manual smoke render produces a playable overlay that shows the new widgets where expected
