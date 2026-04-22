use activity::{Activity, Metric, Sample};
use layout::{
    DistanceUnit, ElevationUnit, Indicator, IndicatorKind, Orientation, Rect, SpeedUnit, Theme,
    Ticks, Units,
};
use std::time::Duration;
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

use crate::text::TextCtx;
use crate::widgets::scale::{frac, nice_major_interval, tick_values};

/// Render a linear meter widget into `pixmap`.
///
/// v3 (Task 6): adds `IndicatorKind::Rect`, `Arrow`, and `Needle` markers for
/// both orientations, honors `indicator.fill_under` under any non-Fill marker,
/// and renders the current value + unit string when `show_value` is true.
#[allow(clippy::too_many_arguments)]
pub fn render_meter(
    pixmap: &mut Pixmap,
    text_ctx: &mut TextCtx,
    theme: &Theme,
    units: &Units,
    rect: Rect,
    metric_name: &str,
    min: f32,
    max: f32,
    orientation: Orientation,
    indicator: Indicator,
    ticks: Ticks,
    show_value: bool,
    value_font_size: Option<f32>,
    activity: &Activity,
    t: Duration,
) {
    let Some(metric) = Metric::from_str(metric_name) else {
        return;
    };
    let sample = activity.sample_at(t);
    let current_opt = pull_value(metric, &sample, units);
    // Early-out only when the metric is completely absent *and* we're not
    // asked to show a value. When `show_value` is on, we still want to draw
    // the track, ticks, and "--" placeholder so the widget doesn't visually
    // disappear on missing samples.
    if current_opt.is_none() && !show_value {
        return;
    }
    let current = current_opt.unwrap_or(min);

    let fg = super::parse_hex(&theme.fg).unwrap_or(Color::WHITE);
    let accent = super::parse_hex(&theme.accent).unwrap_or(fg);

    let f = frac(current, min, max);

    // Track geometry differs by orientation. For horizontal, the track is a
    // thin band centered vertically within the rect and fills left → right.
    // For vertical, the track is a thin band centered horizontally within the
    // rect and fills bottom → top.
    let (track_x, track_y, track_w, track_h, thickness) = match orientation {
        Orientation::Horizontal => {
            let thickness = (rect.h as f32 * 0.5).min(rect.w as f32 * 0.5);
            let track_y = rect.y as f32 + (rect.h as f32 - thickness) * 0.5;
            let track_x = rect.x as f32;
            let track_w = rect.w as f32;
            (track_x, track_y, track_w, thickness, thickness)
        }
        Orientation::Vertical => {
            let thickness = (rect.w as f32 * 0.5).min(rect.h as f32 * 0.5);
            let track_x = rect.x as f32 + (rect.w as f32 - thickness) * 0.5;
            let track_y = rect.y as f32;
            let track_h = rect.h as f32;
            (track_x, track_y, thickness, track_h, thickness)
        }
    };

    // Fill portion. Only draw when we actually have a value — a missing
    // sample with show_value=true still gets an empty track + "--" label.
    let has_value = current_opt.is_some();
    if has_value && (matches!(indicator.kind, IndicatorKind::Fill) || indicator.fill_under) {
        match orientation {
            Orientation::Horizontal => {
                let filled_w = track_w * f;
                if filled_w > 0.0 {
                    draw_rect(pixmap, track_x, track_y, filled_w, track_h, accent);
                }
            }
            Orientation::Vertical => {
                let filled_h = track_h * f;
                if filled_h > 0.0 {
                    let y = track_y + (track_h - filled_h);
                    draw_rect(pixmap, track_x, y, track_w, filled_h, accent);
                }
            }
        }
    }

    // Track outline (always drawn).
    stroke_rect(pixmap, track_x, track_y, track_w, track_h, fg, 1.5);

    // Ticks + numbers.
    let major_every = ticks
        .major_every
        .unwrap_or_else(|| nice_major_interval(min, max));
    let minor_every = ticks.minor_every.unwrap_or(major_every / 5.0);

    if major_every > 0.0 && minor_every > 0.0 {
        let minor_len = thickness * 0.25;
        let major_len = thickness * 0.5;

        // Draw minor ticks first so major ticks (longer) overdraw them where
        // they coincide.
        for v in tick_values(min, max, minor_every) {
            let tf = frac(v, min, max);
            draw_tick(
                pixmap,
                orientation,
                track_x,
                track_y,
                track_w,
                track_h,
                tf,
                minor_len,
                fg,
            );
        }

        // Font size for tick numbers: scale with thickness, clamp to a
        // readable minimum. A 40-high horizontal canvas with thickness=20
        // lands at 12.0, which reads cleanly.
        let number_font_size = (thickness * 0.5).max(12.0);

        for v in tick_values(min, max, major_every) {
            let tf = frac(v, min, max);
            draw_tick(
                pixmap,
                orientation,
                track_x,
                track_y,
                track_w,
                track_h,
                tf,
                major_len,
                fg,
            );

            if ticks.show_numbers {
                let label = format!("{:.*}", ticks.decimals as usize, v);
                draw_tick_number(
                    pixmap,
                    text_ctx,
                    orientation,
                    track_x,
                    track_y,
                    track_w,
                    track_h,
                    tf,
                    major_len,
                    number_font_size,
                    &label,
                    fg,
                );
            }
        }

        // Unit label — identifies what the scale measures. Placed just
        // past the max-value tick's number so it stays close to the scale
        // (top of a vertical meter, right end of a horizontal meter) and
        // doesn't collide with the track on high-aspect-ratio rects.
        if ticks.show_numbers {
            let suffix = unit_suffix(metric, units);
            if !suffix.is_empty() {
                let max_label = format!("{:.*}", ticks.decimals as usize, max);
                let max_label_w = text_ctx.measure_width(&max_label, number_font_size);
                const TICK_GAP: f32 = 4.0; // matches draw_tick_number
                const UNIT_GAP: f32 = 6.0;
                match orientation {
                    Orientation::Horizontal => {
                        // Max tick sits at the right end of the track with
                        // its number centered on that x. Unit follows the
                        // number's right edge on the same baseline.
                        let number_right = track_x + track_w + max_label_w * 0.5;
                        let x = number_right + UNIT_GAP;
                        let y = track_y + track_h + major_len + TICK_GAP;
                        text_ctx.draw(pixmap, suffix, x, y, number_font_size, fg);
                    }
                    Orientation::Vertical => {
                        // Max tick sits at the top; its number is
                        // left-anchored to the right of the tick end. Unit
                        // continues past the number on the same line.
                        let number_left = track_x + track_w + major_len + TICK_GAP;
                        let x = number_left + max_label_w + UNIT_GAP;
                        let y = track_y - number_font_size * 0.5;
                        text_ctx.draw(pixmap, suffix, x, y, number_font_size, fg);
                    }
                }
            }
        }
    }

    // Non-Fill markers (Rect / Arrow / Needle). We draw these after the
    // track + ticks so the marker sits on top. Only draw when we have a
    // current value — a missing sample renders the empty track + "--".
    if has_value && !matches!(indicator.kind, IndicatorKind::Fill) {
        match orientation {
            Orientation::Horizontal => {
                let pos_x = track_x + track_w * f;
                match indicator.kind {
                    IndicatorKind::Fill => unreachable!(),
                    IndicatorKind::Rect => {
                        let marker_w = (thickness * 0.2).max(2.0);
                        draw_rect(
                            pixmap,
                            pos_x - marker_w * 0.5,
                            track_y,
                            marker_w,
                            thickness,
                            fg,
                        );
                    }
                    IndicatorKind::Arrow => {
                        let half_base = thickness * 0.3;
                        let height = thickness * 0.4;
                        draw_triangle(
                            pixmap,
                            (pos_x, track_y),
                            (pos_x - half_base, track_y - height),
                            (pos_x + half_base, track_y - height),
                            fg,
                        );
                    }
                    IndicatorKind::Needle => {
                        let overshoot = thickness * 0.4;
                        draw_line(
                            pixmap,
                            pos_x,
                            track_y - overshoot,
                            pos_x,
                            track_y + thickness + overshoot,
                            fg,
                            2.0,
                        );
                    }
                }
            }
            Orientation::Vertical => {
                // f=0 at bottom, f=1 at top.
                let pos_y = track_y + track_h * (1.0 - f);
                match indicator.kind {
                    IndicatorKind::Fill => unreachable!(),
                    IndicatorKind::Rect => {
                        let marker_h = (thickness * 0.2).max(2.0);
                        draw_rect(
                            pixmap,
                            track_x,
                            pos_y - marker_h * 0.5,
                            thickness,
                            marker_h,
                            fg,
                        );
                    }
                    IndicatorKind::Arrow => {
                        // Ticks go to the right of the vertical track, so we
                        // place the arrow on the left pointing inward (right).
                        // Apex at the track's left edge, base off to the left.
                        let half_base = thickness * 0.3;
                        let width = thickness * 0.4;
                        draw_triangle(
                            pixmap,
                            (track_x, pos_y),
                            (track_x - width, pos_y - half_base),
                            (track_x - width, pos_y + half_base),
                            fg,
                        );
                    }
                    IndicatorKind::Needle => {
                        let overshoot = thickness * 0.4;
                        draw_line(
                            pixmap,
                            track_x - overshoot,
                            pos_y,
                            track_x + thickness + overshoot,
                            pos_y,
                            fg,
                            2.0,
                        );
                    }
                }
            }
        }
    }

    // show_value: render "VAL UNIT" (or "-- UNIT") outside the track band.
    if show_value {
        let suffix = unit_suffix(metric, units);
        let value_str = match current_opt {
            Some(v) => format!("{:.*}", ticks.decimals as usize, v),
            None => "--".to_string(),
        };
        let text = if suffix.is_empty() {
            value_str
        } else {
            format!("{} {}", value_str, suffix)
        };

        match orientation {
            Orientation::Horizontal => {
                let font_size = value_font_size.unwrap_or(rect.h as f32 * 0.2);
                let w = text_ctx.measure_width(&text, font_size);
                let x = rect.x as f32 + (rect.w as f32 - w) * 0.5;
                // Sit the label above the track band. TextCtx::draw's `y` is
                // the top of the layout box; place the top so the baseline
                // lands a small gap above the track top.
                let y = track_y - font_size * 1.05;
                text_ctx.draw(pixmap, &text, x, y, font_size, fg);
            }
            Orientation::Vertical => {
                let font_size = value_font_size.unwrap_or(rect.w as f32 * 0.2);
                let w = text_ctx.measure_width(&text, font_size);
                // Center vertically within the rect, right-anchor against
                // the track's left edge (a small gap keeps it off the track
                // outline).
                let x = track_x - 8.0 - w;
                let y = rect.y as f32 + (rect.h as f32 - font_size) * 0.5;
                text_ctx.draw(pixmap, &text, x, y, font_size, fg);
            }
        }
    }
}

fn draw_rect(pixmap: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, color: Color) {
    let mut pb = PathBuilder::new();
    let Some(r) = tiny_skia::Rect::from_xywh(x, y, w, h) else {
        return;
    };
    pb.push_rect(r);
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;
        pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

fn stroke_rect(pixmap: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, color: Color, width: f32) {
    let mut pb = PathBuilder::new();
    let Some(r) = tiny_skia::Rect::from_xywh(x, y, w, h) else {
        return;
    };
    pb.push_rect(r);
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;
        let stroke = Stroke {
            width,
            ..Default::default()
        };
        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_tick(
    pixmap: &mut Pixmap,
    orientation: Orientation,
    track_x: f32,
    track_y: f32,
    track_w: f32,
    track_h: f32,
    tf: f32,
    tick_len: f32,
    color: Color,
) {
    // Horizontal: ticks extend downward from the bottom edge of the track.
    // Vertical: ticks extend rightward from the right edge of the track.
    match orientation {
        Orientation::Horizontal => {
            let x = track_x + track_w * tf;
            let y0 = track_y + track_h;
            draw_line(pixmap, x, y0, x, y0 + tick_len, color, 1.5);
        }
        Orientation::Vertical => {
            // f=0 at bottom, f=1 at top.
            let y = track_y + track_h * (1.0 - tf);
            let x0 = track_x + track_w;
            draw_line(pixmap, x0, y, x0 + tick_len, y, color, 1.5);
        }
    }
}

fn draw_triangle(pixmap: &mut Pixmap, a: (f32, f32), b: (f32, f32), c: (f32, f32), color: Color) {
    let mut pb = PathBuilder::new();
    pb.move_to(a.0, a.1);
    pb.line_to(b.0, b.1);
    pb.line_to(c.0, c.1);
    pb.close();
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;
        pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

fn draw_line(pixmap: &mut Pixmap, x0: f32, y0: f32, x1: f32, y1: f32, color: Color, width: f32) {
    let mut pb = PathBuilder::new();
    pb.move_to(x0, y0);
    pb.line_to(x1, y1);
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;
        let stroke = Stroke {
            width,
            ..Default::default()
        };
        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_tick_number(
    pixmap: &mut Pixmap,
    text_ctx: &mut TextCtx,
    orientation: Orientation,
    track_x: f32,
    track_y: f32,
    track_w: f32,
    track_h: f32,
    tf: f32,
    tick_len: f32,
    font_size: f32,
    label: &str,
    color: Color,
) {
    const GAP: f32 = 4.0;
    match orientation {
        Orientation::Horizontal => {
            // Center number horizontally under the tick, top-anchored just
            // below the tick end.
            let w = text_ctx.measure_width(label, font_size);
            let x_center = track_x + track_w * tf;
            let x = x_center - w * 0.5;
            let y = track_y + track_h + tick_len + GAP;
            text_ctx.draw(pixmap, label, x, y, font_size, color);
        }
        Orientation::Vertical => {
            // Left-anchored to the right of the tick end, vertically centered
            // on the tick. TextCtx::draw uses `y` as the top of the layout
            // box; the visible glyph baseline sits ≈0.85 * font_size below,
            // so pulling up by ~half the font size centers the typical-height
            // digit on the tick line.
            let y_line = track_y + track_h * (1.0 - tf);
            let x = track_x + track_w + tick_len + GAP;
            let y = y_line - font_size * 0.5;
            text_ctx.draw(pixmap, label, x, y, font_size, color);
        }
    }
}

/// Pull a scalar value from `sample` for a given metric, converted into the
/// layout's display units.
///
/// This is the canonical "what number does this metric want the widget to
/// show" function for Meter (and eventually Gauge). Speed, distance,
/// altitude, and elev_gain all convert through `units`. Cadence, power, HR,
/// and gradient have no unit variants and return their native values.
///
/// Returns `None` if the metric has no value on the sample (or if it's a
/// synthetic metric like TimeElapsed, which Meter/Gauge don't render).
pub(crate) fn pull_value(m: Metric, s: &Sample, units: &Units) -> Option<f32> {
    match m {
        Metric::Speed => s.speed_mps.map(|v| match units.speed {
            SpeedUnit::Kmh => v * 3.6,
            SpeedUnit::Mph => v * 2.236_936_3,
        }),
        Metric::HeartRate => s.heart_rate_bpm.map(|v| v as f32),
        Metric::Power => s.power_w.map(|v| v as f32),
        Metric::Cadence => s.cadence_rpm.map(|v| v as f32),
        Metric::Altitude => s.altitude_m.map(|v| match units.elevation {
            ElevationUnit::M => v,
            ElevationUnit::Ft => v * 3.280_84,
        }),
        Metric::Distance => s.distance_m.map(|v| match units.distance {
            DistanceUnit::Km => (v / 1000.0) as f32,
            DistanceUnit::Mi => (v / 1609.344) as f32,
        }),
        Metric::Gradient => s.gradient_pct,
        Metric::ElevGain => s.elev_gain_cum_m.map(|v| match units.elevation {
            ElevationUnit::M => v,
            ElevationUnit::Ft => v * 3.280_84,
        }),
        _ => None, // TimeElapsed / TimeOfDay / PowerToWeight — not supported by Meter/Gauge yet.
    }
}

/// Return the short unit suffix for a metric (e.g. `"km/h"`, `"m"`, `"%"`).
/// Mirrors `pull_value`'s unit conversion so the two stay in lockstep.
///
/// Used by `show_value` to append a unit to the current-value string and
/// reused by Gauge in Task 7; a shared-formatter refactor is deferred.
pub(crate) fn unit_suffix(m: Metric, units: &Units) -> &'static str {
    match m {
        Metric::Speed => match units.speed {
            SpeedUnit::Kmh => "km/h",
            SpeedUnit::Mph => "mph",
        },
        Metric::HeartRate => "bpm",
        Metric::Power => "W",
        Metric::Cadence => "rpm",
        Metric::Altitude => match units.elevation {
            ElevationUnit::M => "m",
            ElevationUnit::Ft => "ft",
        },
        Metric::Distance => match units.distance {
            DistanceUnit::Km => "km",
            DistanceUnit::Mi => "mi",
        },
        Metric::Gradient => "%",
        Metric::ElevGain => match units.elevation {
            ElevationUnit::M => "m",
            ElevationUnit::Ft => "ft",
        },
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use layout::{DistanceUnit, ElevationUnit, SpeedUnit, TempUnit, Units};

    fn kmh_units() -> Units {
        Units {
            speed: SpeedUnit::Kmh,
            distance: DistanceUnit::Km,
            elevation: ElevationUnit::M,
            temp: TempUnit::C,
        }
    }

    fn blank_sample_with_speed(mps: f32) -> Sample {
        Sample {
            t: Duration::ZERO,
            lat: 0.0,
            lon: 0.0,
            altitude_m: None,
            speed_mps: Some(mps),
            heart_rate_bpm: None,
            cadence_rpm: None,
            power_w: None,
            distance_m: None,
            elev_gain_cum_m: None,
            gradient_pct: None,
        }
    }

    #[test]
    fn pull_value_speed_kmh() {
        let s = blank_sample_with_speed(10.0); // 10 m/s
        let v = pull_value(Metric::Speed, &s, &kmh_units()).unwrap();
        assert!((v - 36.0).abs() < 0.01);
    }

    #[test]
    fn pull_value_speed_mph() {
        let mut u = kmh_units();
        u.speed = SpeedUnit::Mph;
        let s = blank_sample_with_speed(10.0);
        let v = pull_value(Metric::Speed, &s, &u).unwrap();
        assert!((v - 22.369).abs() < 0.01);
    }

    #[test]
    fn pull_value_distance_km() {
        let mut s = blank_sample_with_speed(0.0);
        s.speed_mps = None;
        s.distance_m = Some(2500.0);
        let v = pull_value(Metric::Distance, &s, &kmh_units()).unwrap();
        assert!((v - 2.5).abs() < 0.001);
    }

    #[test]
    fn pull_value_altitude_ft() {
        let mut u = kmh_units();
        u.elevation = ElevationUnit::Ft;
        let mut s = blank_sample_with_speed(0.0);
        s.speed_mps = None;
        s.altitude_m = Some(100.0);
        let v = pull_value(Metric::Altitude, &s, &u).unwrap();
        assert!((v - 328.084).abs() < 0.01);
    }

    #[test]
    fn unit_suffix_handles_kmh() {
        assert_eq!(unit_suffix(Metric::Speed, &kmh_units()), "km/h");
    }

    #[test]
    fn unit_suffix_handles_mph() {
        let mut u = kmh_units();
        u.speed = SpeedUnit::Mph;
        assert_eq!(unit_suffix(Metric::Speed, &u), "mph");
    }

    #[test]
    fn unit_suffix_gradient_is_percent() {
        assert_eq!(unit_suffix(Metric::Gradient, &kmh_units()), "%");
    }

    // Pure-math anchors for the marker geometry. We don't render here —
    // the golden tests cover the actual pixel layout — but these lock in the
    // scale::frac contract the marker code relies on so a refactor of
    // `frac` wouldn't silently shift every marker by half a pixel.
    #[test]
    fn rect_marker_centered_on_value() {
        // rect.w=100, min=0, max=100, current=50 → frac=0.5, marker center at
        // rect.x + 50. We test frac directly.
        assert!((frac(50.0, 0.0, 100.0) - 0.5).abs() < 1e-4);
    }

    #[test]
    fn arrow_marker_anchored_at_value() {
        // 25% along a 0..=80 range should land at frac=0.25, i.e. the arrow
        // apex sits 25% along the track.
        assert!((frac(20.0, 0.0, 80.0) - 0.25).abs() < 1e-4);
    }

    #[test]
    fn needle_marker_clamps_above_range() {
        // Values above max clamp to 1.0 so the needle sits at the far end
        // instead of shooting past the track.
        assert!((frac(500.0, 0.0, 100.0) - 1.0).abs() < 1e-4);
    }
}
