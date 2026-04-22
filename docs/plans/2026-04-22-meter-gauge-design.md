# Meter and Gauge widgets — Design

Date: 2026-04-22
Status: Draft, pre-implementation

## Goal

Add two new widget types for visualizing instantaneous scalar metrics (speed, power, cadence, HR, gradient):

- `Widget::Meter` — linear bar with tick marks, numbers, and a configurable indicator. Horizontal or vertical.
- `Widget::Gauge` — radial (arc-based) version of the same. Classic speedometer look by default.

Both share a common indicator vocabulary (fill, rect marker, arrow marker, needle, optionally combined with a filled track) and a common tick configuration (major + minor intervals, numbers on majors).

## Non-goals (v1)

- Explicit tick list / custom labels (FTP marker, zone boundaries). Deferred to a future `annotations` field.
- Color zones (HR zones, power sweet-spot bands).
- Smoothed needle motion with inertia — breaks the pure-function property of `render_frame`.
- Custom marker color separate from `theme.accent`.
- Auto min/max from activity data. v1 requires explicit `min` / `max` in the schema.

The existing `Widget::Bar` (cumulative distance / elev_gain progress) is untouched — it has different semantics (auto-max, monotone progress) that don't map cleanly onto the new widget types.

## Schema additions

Two new variants on the `Widget` enum (internally tagged, same as existing variants):

```rust
Widget::Meter {
    id: String,
    metric: String,
    rect: Rect,
    min: f32,
    max: f32,
    #[serde(default)]
    orientation: Orientation,    // Horizontal (default) | Vertical
    #[serde(default)]
    indicator: Indicator,
    #[serde(default)]
    ticks: Ticks,
    #[serde(default)]
    show_value: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    value_font_size: Option<f32>,
}

Widget::Gauge {
    id: String,
    metric: String,
    rect: Rect,
    min: f32,
    max: f32,
    #[serde(default = "default_gauge_start_deg")] // -135
    start_deg: f32,
    #[serde(default = "default_gauge_end_deg")]   //  135
    end_deg: f32,
    #[serde(default)]
    indicator: Indicator,
    #[serde(default)]
    ticks: Ticks,
    #[serde(default)]
    show_value: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    value_font_size: Option<f32>,
}
```

Three shared sub-types:

```rust
#[derive(Default)]
pub enum Orientation {
    #[default] Horizontal,
    Vertical,
}

#[derive(Default)]
pub struct Indicator {
    #[serde(default)] pub kind: IndicatorKind,
    #[serde(default)] pub fill_under: bool,
}

#[derive(Default)]
pub enum IndicatorKind {
    #[default] Fill,
    Rect,
    Arrow,
    Needle,
}

pub struct Ticks {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub major_every: Option<f32>,  // None → auto-compute
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minor_every: Option<f32>,  // None → major / 5
    #[serde(default = "default_true")]
    pub show_numbers: bool,
    #[serde(default)]
    pub decimals: u32,
}
```

`Indicator` is a struct rather than a flat enum so `{ kind: Needle, fill_under: true }` gives the classic speedometer (filled track under moving needle) without a combinatorial set of enum variants. Setting `fill_under: true` with `kind: Fill` is a no-op.

## Angle convention (Gauge only)

User-facing angles: **0° = up (12 o'clock), positive clockwise**. The gauge sweeps clockwise from `start_deg` to `end_deg`.

Default `start_deg = -135`, `end_deg = 135` gives a 270° sweep missing the bottom quadrant — the classic car speedometer.

Internal conversion to tiny-skia's convention (0° = right, counterclockwise positive for `PathBuilder::push_arc`):

```rust
fn to_skia_angle(deg_up_cw: f32) -> f32 { 90.0 - deg_up_cw }
```

If `end_deg < start_deg` (e.g., `start=315, end=45`), we add 360° to `end_deg` internally so the sweep wraps through the top cleanly.

## Rendering

Shared per-frame logic:

1. Parse `metric` via `Metric::from_str`; look up current value in `activity.sample_at(t)`. If missing → skip rendering the indicator (but still draw track + ticks).
2. Compute `frac = ((current - min) / (max - min)).clamp(0.0, 1.0)`.
3. Map `frac` to position:
   - Meter horizontal: `x = rect.x + frac * rect.w`.
   - Meter vertical: `y = rect.y + (1.0 - frac) * rect.h`.
   - Gauge: `angle = start_deg + frac * (end_deg_normalized - start_deg)`.
4. Draw in this order (back to front):
   1. Track outline / background (fg color, thin line or transparent track).
   2. Filled portion (if `indicator.fill_under` or `kind: Fill`) — accent color.
   3. Major tick marks + numbers, then minor ticks — fg color.
   4. Marker shape (if `kind != Fill`) — fg color.
   5. Optional center value text (`show_value`) — fg color.

### Indicator shapes

- **Fill**: filled track from start to `frac`. No marker.
- **Rect**: filled rectangle at the current position. Meter: width = `thickness * 0.2`, full track thickness. Gauge: rectangle tangent to the arc, rotated to match the current angle.
- **Arrow**: triangle pointing at the current position from outside the track. Meter: triangle above (horizontal) or on the side opposite the numbers (vertical). Gauge: triangle outside the arc pointing radially inward.
- **Needle**: thin line through the position. Meter: perpendicular line across the track, extending `thickness * 0.4` beyond both sides. Gauge: line from the arc's center radially out to slightly past the arc radius.

### Track geometry

**Meter**. Track = `thickness` pixels across the centerline of the rect. Default thickness = `min(rect.w, rect.h) * 0.4`. Fill direction: horizontal left→right, vertical bottom→top.

**Gauge**. The gauge fits the largest centered square inside `rect`. Center = rect midpoint. Radius = `min(rect.w, rect.h) * 0.5 - padding`, where `padding = track_thickness + tick_length + number_height` so ticks and numbers stay inside the rect.

### Tick placement

**Auto-computed major interval** when `major_every` is None:

```
raw = (max - min) / 6
magnitude = 10.0_f32.powf(raw.log10().floor())
normalized = raw / magnitude
nice = match normalized {
    x if x < 1.5 => 1.0,
    x if x < 3.0 => 2.0,
    x if x < 7.0 => 5.0,
    _            => 10.0,
}
major_every = nice * magnitude
```

This picks round numbers (1, 2, 5, 10, 20, 50, …) that divide the range into roughly 6 intervals. Minor default = `major / 5`.

**Tick value walker**: iterate `value = min + k * major_every` for `k = 0, 1, ...` while `value <= max + epsilon`. Convert each to position via the same lerp used for the indicator.

**Tick-side**. Numbers and ticks sit on the default-readable side of the track:

- Meter horizontal: below the track (`tick_side: Below`).
- Meter vertical: right of the track (`tick_side: Right`).
- Gauge: outside the arc. Numbers rotated so their baseline is tangent to the arc and upright (no upside-down text on the bottom of the arc — clamp text rotation to `[-90°, 90°]` and flip with a 180° offset beyond that).

### Center value label (`show_value`)

When true, formats the current metric value via the same path `Readout` uses (`format_metric`) and draws it centered in the rect. Font size defaults to `min(rect.w, rect.h) * 0.15`. Off by default — pair with a separate `Readout` widget if you want finer typography control.

## Colors

All colors from `theme`:

- `theme.fg` — track outline, tick marks, tick numbers, marker shapes (when `fill_under` is false or for the on-top marker).
- `theme.accent` — filled portion (fill indicator or `fill_under`).

No per-widget color overrides in v1.

## Testing

Following the established pattern for existing widgets:

### Unit tests (pure math, no tiny-skia)

- `frac` clamping: below min → 0, above max → 1.
- `to_skia_angle`: 0° up → 90° skia, 90° CW → 0° skia, -90° (CCW 90) → 180° skia.
- `angle_lerp`: wraps through top when `end_deg < start_deg`.
- `nice_major_interval`: asserts canonical values — `range 100 → 20`, `range 60 → 10`, `range 5 → 1`, `range 0.3 → 0.05`.
- Tick-value walker covers inclusive endpoints.

### Golden image tests

Four new goldens:

- `meter_speed_fill.png` — horizontal meter, fill indicator, major+minor ticks, no numbers.
- `meter_power_needle.png` — vertical meter, `kind: Needle, fill_under: true`, numbers on.
- `gauge_speed_classic.png` — default 270° sweep, `kind: Needle, fill_under: true`, `show_value: true`.
- `gauge_power_arrow.png` — full 360° (start=-180, end=180) sweep, `kind: Arrow`, `fill_under: false`.

Generation uses the existing "delete PNG, rerun test twice" dance.

### Integration

The existing end-to-end render test continues to work (new widgets aren't in the reference `examples/layout.json`). Optionally add one Meter + one Gauge to `layout-cycling.json` so the CLI path exercises them.

## Wiring

**`layout` crate**: add the two variants and three sub-types. Extend `Widget::id()` / `Widget::rect()` match arms. Validation: `metric` must resolve via `Metric::from_str`; rect bounds check (already generic).

**`render` crate**: add `widgets::meter::render_meter` and `widgets::gauge::render_gauge`. Shared helpers (tick walker, nice-number, angle conversion) in `widgets/scale.rs` or similar. Dispatch from `render_frame`'s widget match.

**`cli` crate**: no changes — `Layout` deserialization picks up new variants via serde `#[serde(tag = "type")]`.

## Open questions / deferred

- Custom marker color.
- Color zones (`zones: Vec<{ from, to, color }>`).
- Explicit tick list / annotations.
- Smoothed needle motion.
- Auto min/max from activity samples.
