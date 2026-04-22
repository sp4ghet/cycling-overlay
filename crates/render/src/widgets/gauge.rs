use activity::{Activity, Metric};
use layout::{Indicator, IndicatorKind, Rect, Theme, Ticks, Units};
use std::time::Duration;
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Stroke, Transform};

use crate::text::TextCtx;
use crate::widgets::meter::pull_value;
use crate::widgets::scale::{angle_lerp, frac, nice_major_interval, tick_values, to_skia_angle};

// Tick/number geometry constants. Task 10 may factor these into a shared
// primitives module alongside `draw_line`.
const MAJOR_TICK_LEN_RATIO: f32 = 0.6; // relative to `thickness`
const MINOR_TICK_LEN_RATIO: f32 = 0.3;
const NUMBER_FONT_SIZE_MIN: f32 = 12.0;
const NUMBER_GAP: f32 = 4.0; // pixels between tick outer end and number

/// Render a radial gauge widget.
///
/// v2 (Task 8): default 270° arc with Fill indicator plus major/minor ticks
/// and (optional) number labels. Still no markers or center label — those
/// land in Task 9.
#[allow(clippy::too_many_arguments)]
pub fn render_gauge(
    pixmap: &mut Pixmap,
    text_ctx: &mut TextCtx,
    theme: &Theme,
    units: &Units,
    rect: Rect,
    metric_name: &str,
    min: f32,
    max: f32,
    start_deg: f32,
    end_deg: f32,
    indicator: Indicator,
    ticks: Ticks,
    _show_value: bool, // Task 9
    _value_font_size: Option<f32>,
    activity: &Activity,
    t: Duration,
) {
    let Some(metric) = Metric::from_str(metric_name) else {
        return;
    };
    let sample = activity.sample_at(t);
    let current = pull_value(metric, &sample, units);

    let fg = super::parse_hex(&theme.fg).unwrap_or(Color::WHITE);
    let accent = super::parse_hex(&theme.accent).unwrap_or(fg);

    // Geometry: largest centered square inside rect, with padding to keep
    // the arc *and* tick numbers inside the rect. The padding budget is a
    // conservative fixed value — tick_len + label height + NUMBER_GAP
    // depends on `thickness`, which depends on `r_outer`, which depends on
    // `padding`, so we break the cycle with a generous constant. Task 10
    // polish can iterate if the golden exposes clipping.
    let cx = rect.x as f32 + rect.w as f32 * 0.5;
    let cy = rect.y as f32 + rect.h as f32 * 0.5;
    let padding = 24.0;
    let r_outer = (rect.w.min(rect.h) as f32) * 0.5 - padding;
    let thickness = (r_outer * 0.15).max(4.0);
    let r_center = r_outer - thickness * 0.5;

    // Full arc (track): stroke the polyline from start_deg to end_deg.
    draw_arc_stroke(pixmap, cx, cy, r_center, thickness, start_deg, end_deg, fg);

    // Fill portion: stroke a shorter arc from start_deg to the current angle,
    // using accent color. Skip when metric is missing.
    if let Some(v) = current {
        if matches!(indicator.kind, IndicatorKind::Fill) || indicator.fill_under {
            let f = frac(v, min, max);
            if f > 0.0 {
                let cur_deg = angle_lerp(start_deg, end_deg, f);
                draw_arc_stroke(
                    pixmap, cx, cy, r_center, thickness, start_deg, cur_deg, accent,
                );
            }
        }
    }

    // Ticks + numbers. Mirrors meter.rs's logic but lays out radially:
    // each tick is a line segment along the radial direction, starting at
    // the outer edge of the track and extending outward.
    let major_every = ticks
        .major_every
        .unwrap_or_else(|| nice_major_interval(min, max));
    let minor_every = ticks.minor_every.unwrap_or(major_every / 5.0);

    if major_every > 0.0 && minor_every > 0.0 {
        let major_len = thickness * MAJOR_TICK_LEN_RATIO;
        let minor_len = thickness * MINOR_TICK_LEN_RATIO;
        let r_tick_inner = r_outer; // ticks start at the outer edge of the track

        // Draw minor ticks first (short) so major ticks (longer) overdraw
        // them where they coincide.
        for v in tick_values(min, max, minor_every) {
            let vf = frac(v, min, max);
            let deg = angle_lerp(start_deg, end_deg, vf);
            let s = to_skia_angle(deg).to_radians();
            let (cos, sin) = (s.cos(), s.sin());
            let p1 = (cx + r_tick_inner * cos, cy - r_tick_inner * sin);
            let p2 = (
                cx + (r_tick_inner + minor_len) * cos,
                cy - (r_tick_inner + minor_len) * sin,
            );
            draw_line(pixmap, p1.0, p1.1, p2.0, p2.1, fg, 1.5);
        }

        // Number font size scales with thickness. Clamp to a readable
        // minimum and a modest maximum so very large gauges don't get
        // cartoonish digits.
        let number_font_size = (thickness * 0.8).clamp(NUMBER_FONT_SIZE_MIN, 20.0);

        for v in tick_values(min, max, major_every) {
            let vf = frac(v, min, max);
            let deg = angle_lerp(start_deg, end_deg, vf);
            let s = to_skia_angle(deg).to_radians();
            let (cos, sin) = (s.cos(), s.sin());
            let p1 = (cx + r_tick_inner * cos, cy - r_tick_inner * sin);
            let p2 = (
                cx + (r_tick_inner + major_len) * cos,
                cy - (r_tick_inner + major_len) * sin,
            );
            draw_line(pixmap, p1.0, p1.1, p2.0, p2.1, fg, 2.0);

            if ticks.show_numbers {
                let text = format!("{:.*}", ticks.decimals as usize, v);
                // Place the number just outside the tick, roughly centered
                // on the tick's outer endpoint. Text is horizontal (not
                // rotated to follow the arc) — rotation is deferred polish.
                let r_label = r_tick_inner + major_len + NUMBER_GAP;
                let cx_label = cx + r_label * cos;
                let cy_label = cy - r_label * sin;

                let text_w = text_ctx.measure_width(&text, number_font_size);
                // cosmic-text positions glyphs with the layout box's
                // top-left at (x, y). The visible baseline sits roughly
                // 0.85 * font_size below the top; to vertically-center the
                // x-height on `cy_label`, pull up by that baseline offset
                // then push back down by half the font size.
                let baseline_adjust = number_font_size * 0.85;
                let draw_x = cx_label - text_w * 0.5;
                let draw_y = cy_label - baseline_adjust + number_font_size * 0.5;
                text_ctx.draw(pixmap, &text, draw_x, draw_y, number_font_size, fg);
            }
        }
    }
}

/// Stroke a circular arc from `start_deg_user` to `end_deg_user` (user
/// convention: 0° up, CW+) at the given center and radius. Approximates the
/// arc with a polyline — roughly 2° per segment is smooth at overlay
/// resolutions (~180 segments per full 360° sweep).
fn draw_arc_stroke(
    pixmap: &mut Pixmap,
    cx: f32,
    cy: f32,
    radius: f32,
    stroke_w: f32,
    start_deg_user: f32,
    end_deg_user: f32,
    color: Color,
) {
    // Normalize the end angle so a "less than start" end means "wraps through top."
    let user_start = start_deg_user;
    let user_end = if end_deg_user >= start_deg_user {
        end_deg_user
    } else {
        end_deg_user + 360.0
    };
    let span = (user_end - user_start).abs();
    if span < 1e-4 {
        return; // degenerate sweep
    }
    let steps = ((span / 2.0).ceil() as i32).max(1);

    let mut pb = PathBuilder::new();
    for i in 0..=steps {
        let u = user_start + (user_end - user_start) * (i as f32 / steps as f32);
        let s = to_skia_angle(u).to_radians();
        let x = cx + radius * s.cos();
        let y = cy - radius * s.sin(); // flip math-y to screen-y
        if i == 0 {
            pb.move_to(x, y);
        } else {
            pb.line_to(x, y);
        }
    }
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;
        let stroke = Stroke {
            width: stroke_w,
            line_cap: tiny_skia::LineCap::Butt,
            ..Default::default()
        };
        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }
}

// Local copy of meter.rs's `draw_line`. Kept intentionally local for now —
// Task 10 polish may factor this (and meter.rs's twin) into a shared
// `primitives` module. Forcing the refactor now would bloat this task's
// diff for no runtime benefit.
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

#[cfg(test)]
mod tests {
    // Defer visual correctness to the golden test; no pure-math unit tests
    // needed for this task.
}
