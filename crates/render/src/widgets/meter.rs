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
/// v2 (Task 5): horizontal + vertical, `IndicatorKind::Fill`, major + minor
/// ticks, optional tick numbers. Remaining indicator kinds (`Rect`, `Arrow`,
/// `Needle`), the `fill_under` combined mode, and `show_value` land in Task 6.
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
    _show_value: bool,       // Task 6
    activity: &Activity,
    t: Duration,
) {
    let Some(metric) = Metric::from_str(metric_name) else {
        return;
    };
    let sample = activity.sample_at(t);
    let Some(current) = pull_value(metric, &sample, units) else {
        return;
    };

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

    // Fill portion.
    if matches!(indicator.kind, IndicatorKind::Fill) || indicator.fill_under {
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
/// Not called from render code yet (tick numbers dropped the unit suffix);
/// retained for Task 6 (`show_value`) / Task 7 (shared-formatter refactor)
/// and exercised by the unit tests below.
#[allow(dead_code)]
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
}
