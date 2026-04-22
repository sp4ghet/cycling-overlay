use activity::{Activity, Metric};
use layout::{Indicator, IndicatorKind, Rect, Theme, Ticks, Units};
use std::time::Duration;
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

use crate::text::TextCtx;
use crate::widgets::meter::{pull_value, unit_suffix};
use crate::widgets::scale::{angle_lerp, frac, nice_major_interval, tick_values, to_skia_angle};

// Tick/number geometry constants. Task 10 may factor these into a shared
// primitives module alongside `draw_line`.
const MAJOR_TICK_LEN_RATIO: f32 = 0.6; // relative to `thickness`
const MINOR_TICK_LEN_RATIO: f32 = 0.3;
const NUMBER_FONT_SIZE_MIN: f32 = 12.0;
const NUMBER_GAP: f32 = 10.0; // pixels between tick outer end and number's inner edge

/// Render a radial gauge widget.
///
/// v3 (Task 9): 270° default arc with Fill / Rect / Arrow / Needle markers,
/// optional `fill_under` accent arc, major/minor ticks with number labels, and
/// optional `show_value` center label with unit suffix.
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
    show_value: bool,
    value_font_size: Option<f32>,
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
                // Place the label's *center* radially so its inner edge
                // clears the tick outer end by NUMBER_GAP. The text is
                // rendered horizontally, so its inner edge relative to the
                // radial direction is roughly half its font size away.
                let r_label = r_tick_inner + major_len + NUMBER_GAP + number_font_size * 0.5;
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

        // Unit label — placed on the same line as the max-value tick
        // number, just past its right edge. Keeps the unit visually
        // attached to the scale rather than floating in a corner.
        if ticks.show_numbers {
            let suffix = unit_suffix(metric, units);
            if !suffix.is_empty() {
                let deg_max = angle_lerp(start_deg, end_deg, 1.0);
                let s_max = to_skia_angle(deg_max).to_radians();
                let (cos_m, sin_m) = (s_max.cos(), s_max.sin());
                let r_label = r_tick_inner + major_len + NUMBER_GAP + number_font_size * 0.5;
                let cx_label = cx + r_label * cos_m;
                let cy_label = cy - r_label * sin_m;

                let max_label = format!("{:.*}", ticks.decimals as usize, max);
                let max_label_w = text_ctx.measure_width(&max_label, number_font_size);

                let baseline_adjust = number_font_size * 0.85;
                let number_draw_x = cx_label - max_label_w * 0.5;
                let number_draw_y = cy_label - baseline_adjust + number_font_size * 0.5;
                const UNIT_GAP: f32 = 6.0;
                let unit_x = number_draw_x + max_label_w + UNIT_GAP;
                let unit_y = number_draw_y;
                text_ctx.draw(pixmap, suffix, unit_x, unit_y, number_font_size, fg);
            }
        }
    }

    // Radial marker: Rect / Arrow / Needle. Fill already rendered above.
    // We draw after the ticks so the marker sits on top of any coincident
    // tick line. Skip when we have no current value — missing samples still
    // get a track, ticks, and "--" in the center (if show_value is on).
    if let Some(v) = current {
        if !matches!(indicator.kind, IndicatorKind::Fill) {
            let f = frac(v, min, max);
            let deg = angle_lerp(start_deg, end_deg, f);
            let s = to_skia_angle(deg).to_radians();
            let (cos_a, sin_a) = (s.cos(), s.sin());
            // Outward-pointing unit vector from center (y flipped for screen).
            let outward = (cos_a, -sin_a);
            // Tangent to arc, 90° CCW from outward.
            let tangent = (-outward.1, outward.0);

            match indicator.kind {
                IndicatorKind::Fill => unreachable!(),
                IndicatorKind::Rect => {
                    // Small filled rect centered on the mid-track radius,
                    // tangent to the arc. tw = half-width along tangent,
                    // th = half-height along outward (overhangs the track).
                    let center_rad = r_outer - thickness * 0.5;
                    let center = (cx + center_rad * outward.0, cy + center_rad * outward.1);
                    let tw = thickness * 0.125;
                    let th = thickness * 0.55;
                    let half_tan = (tangent.0 * tw, tangent.1 * tw);
                    let half_out = (outward.0 * th, outward.1 * th);
                    let c1 = (
                        center.0 - half_tan.0 - half_out.0,
                        center.1 - half_tan.1 - half_out.1,
                    );
                    let c2 = (
                        center.0 + half_tan.0 - half_out.0,
                        center.1 + half_tan.1 - half_out.1,
                    );
                    let c3 = (
                        center.0 + half_tan.0 + half_out.0,
                        center.1 + half_tan.1 + half_out.1,
                    );
                    let c4 = (
                        center.0 - half_tan.0 + half_out.0,
                        center.1 - half_tan.1 + half_out.1,
                    );
                    fill_quad(pixmap, c1, c2, c3, c4, fg);
                }
                IndicatorKind::Arrow => {
                    // Small triangle sitting just outside the track with its
                    // apex pointing radially inward at the current value.
                    // The arrow is intentionally close to the gauge — it may
                    // overlap one tick number at the current angle but reads
                    // as a clear pointer rather than floating far away.
                    let apex_rad = r_outer + 2.0;
                    let base_rad = apex_rad + thickness * 0.6;
                    let half_base = thickness * 0.3;
                    let apex = (cx + apex_rad * outward.0, cy + apex_rad * outward.1);
                    let base_mid = (cx + base_rad * outward.0, cy + base_rad * outward.1);
                    let base_a = (
                        base_mid.0 - tangent.0 * half_base,
                        base_mid.1 - tangent.1 * half_base,
                    );
                    let base_b = (
                        base_mid.0 + tangent.0 * half_base,
                        base_mid.1 + tangent.1 * half_base,
                    );
                    draw_triangle(pixmap, apex, base_a, base_b, fg);
                }
                IndicatorKind::Needle => {
                    // Hub first so the needle line draws on top of it — the
                    // needle then visually anchors at the hub's center.
                    let hub_r = thickness * 0.4;
                    let mut pb = PathBuilder::new();
                    pb.push_circle(cx, cy, hub_r);
                    if let Some(path) = pb.finish() {
                        let mut paint = Paint::default();
                        paint.set_color(fg);
                        paint.anti_alias = true;
                        pixmap.fill_path(
                            &path,
                            &paint,
                            FillRule::Winding,
                            Transform::identity(),
                            None,
                        );
                    }

                    // Needle runs from the hub center out past the outer edge
                    // of the track (classic speedometer look).
                    let r_tip = r_outer + thickness * 0.3;
                    let tip = (cx + r_tip * outward.0, cy + r_tip * outward.1);
                    draw_line(pixmap, cx, cy, tip.0, tip.1, fg, 2.5);
                }
            }
        }
    }

    // show_value: center-aligned horizontally. When the indicator is a
    // Needle, the hub occupies the center, so the label drops below it;
    // otherwise it sits centered at (cx, cy). Unit suffix appended
    // (e.g. "40.0 km/h"). "--" placeholder when the metric is missing.
    if show_value {
        let font_size = value_font_size.unwrap_or((rect.w.min(rect.h) as f32) * 0.15);
        let suffix = unit_suffix(metric, units);
        let text = match current {
            Some(v) => {
                if suffix.is_empty() {
                    format!("{:.*}", ticks.decimals as usize, v)
                } else {
                    format!("{:.*} {}", ticks.decimals as usize, v, suffix)
                }
            }
            None => "--".to_string(),
        };
        let text_w = text_ctx.measure_width(&text, font_size);
        let draw_x = cx - text_w * 0.5;
        // cosmic-text positions glyphs from the top of the layout box.
        let draw_y = match indicator.kind {
            IndicatorKind::Needle => {
                // Top of text sits just below the hub with a small gap.
                let hub_r = thickness * 0.4;
                cy + hub_r + 4.0
            }
            _ => cy - font_size * 0.35,
        };
        text_ctx.draw(pixmap, &text, draw_x, draw_y, font_size, fg);
    }
}

/// Stroke a circular arc from `start_deg_user` to `end_deg_user` (user
/// convention: 0° up, CW+) at the given center and radius. Approximates the
/// arc with a polyline — roughly 2° per segment is smooth at overlay
/// resolutions (~180 segments per full 360° sweep).
#[allow(clippy::too_many_arguments)]
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

// Local copy of meter.rs's `draw_triangle`, used by the Arrow marker.
// Same rationale as `draw_line` — kept local until Task 10's primitives pass.
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

// Fill a (possibly rotated) quadrilateral defined by four corner points in
// order. Used by the Rect marker where the quad is tangent to the arc — a
// plain axis-aligned `Rect::from_xywh` won't do.
fn fill_quad(
    pixmap: &mut Pixmap,
    a: (f32, f32),
    b: (f32, f32),
    c: (f32, f32),
    d: (f32, f32),
    color: Color,
) {
    let mut pb = PathBuilder::new();
    pb.move_to(a.0, a.1);
    pb.line_to(b.0, b.1);
    pb.line_to(c.0, c.1);
    pb.line_to(d.0, d.1);
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

#[cfg(test)]
mod tests {
    // Defer visual correctness to the golden test; no pure-math unit tests
    // needed for this task.
}
