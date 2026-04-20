use activity::Activity;
use layout::{Rect, Theme};
use std::time::Duration;
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

/// Render an altitude-vs-distance profile into `pixmap`.
///
/// Shape:
/// - X axis = cumulative `distance_m` across the activity.
/// - Y axis = `altitude_m`, inverted so higher altitude is drawn higher (smaller y).
/// - Filled area under the curve in the accent color at half alpha.
/// - Curve stroked on top in the fg color.
/// - A vertical marker line at the current time's distance position (accent, full alpha).
///
/// Samples missing either `distance_m` or `altitude_m` are skipped; the polyline
/// simply connects across the gap. If fewer than two valid points remain, or if
/// the whole activity has no altitude data, this renders nothing. Likewise for a
/// zero-span distance range.
pub fn render_elevation_profile(
    pixmap: &mut Pixmap,
    theme: &Theme,
    rect: Rect,
    activity: &Activity,
    t: Duration,
) {
    // Collect (distance, altitude) pairs, skipping samples missing either.
    let pts: Vec<(f64, f32)> = activity
        .samples
        .iter()
        .filter_map(|s| match (s.distance_m, s.altitude_m) {
            (Some(d), Some(a)) => Some((d, a)),
            _ => None,
        })
        .collect();

    if pts.len() < 2 {
        return;
    }

    // Distance bounds come from the min/max of kept points — not pts[0]/pts.last().
    // The samples list is time-ordered, but a skipped first/last sample would
    // leave a gap. min/max is robust either way.
    let mut min_d = f64::INFINITY;
    let mut max_d = f64::NEG_INFINITY;
    let mut min_a = f32::INFINITY;
    let mut max_a = f32::NEG_INFINITY;
    for &(d, a) in &pts {
        if d < min_d {
            min_d = d;
        }
        if d > max_d {
            max_d = d;
        }
        if a < min_a {
            min_a = a;
        }
        if a > max_a {
            max_a = a;
        }
    }
    let d_span = max_d - min_d;
    if d_span < 1e-9 {
        return;
    }
    // At least 1m of altitude span so flat routes still render as a flat band
    // rather than collapsing onto the baseline.
    let a_span = (max_a - min_a).max(1.0);

    // Rect in f32 for pixel math.
    let rx = rect.x as f32;
    let ry = rect.y as f32;
    let rw = rect.w as f32;
    let rh = rect.h as f32;

    let project = |d: f64, a: f32| -> (f32, f32) {
        let x = rx + ((d - min_d) / d_span) as f32 * rw;
        // Invert Y so higher altitude sits higher (smaller y).
        let y = ry + rh - ((a - min_a) / a_span) * rh;
        (x, y)
    };

    // Build the stroked curve and the filled area path.
    let mut curve = PathBuilder::new();
    let mut area = PathBuilder::new();
    let baseline_y = ry + rh;

    let (x0, y0) = project(pts[0].0, pts[0].1);
    curve.move_to(x0, y0);
    area.move_to(x0, baseline_y);
    area.line_to(x0, y0);

    for &(d, a) in &pts[1..] {
        let (x, y) = project(d, a);
        curve.line_to(x, y);
        area.line_to(x, y);
    }
    let (x_last, _) = project(pts.last().unwrap().0, pts.last().unwrap().1);
    area.line_to(x_last, baseline_y);
    area.close();

    let fg = crate::widgets::parse_hex(&theme.fg).unwrap_or(Color::WHITE);
    let accent = crate::widgets::parse_hex(&theme.accent).unwrap_or(fg);

    // Half-alpha accent for the fill. tiny-skia's Color is non-premultiplied
    // (premultiplication happens lazily inside Paint), so set_alpha(0.5) is
    // safe — no need to rebuild from rgba8.
    let mut accent_half = accent;
    accent_half.set_alpha(0.5);

    // Fill first so the stroke sits on top.
    if let Some(path) = area.finish() {
        let mut paint = Paint::default();
        paint.set_color(accent_half);
        paint.anti_alias = true;
        pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
    }

    // Stroke the curve on top of the fill.
    if let Some(path) = curve.finish() {
        let mut paint = Paint::default();
        paint.set_color(fg);
        paint.anti_alias = true;
        let stroke = Stroke {
            width: 2.0,
            ..Default::default()
        };
        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }

    // Vertical marker at the current distance. Clamp so out-of-range t doesn't
    // draw outside the rect.
    let cur = activity.sample_at(t);
    if let (Some(d_cur), Some(_)) = (cur.distance_m, cur.altitude_m) {
        let clamped_d = d_cur.clamp(min_d, max_d);
        let mx = rx + ((clamped_d - min_d) / d_span) as f32 * rw;
        let mut marker = PathBuilder::new();
        marker.move_to(mx, ry);
        marker.line_to(mx, ry + rh);
        if let Some(path) = marker.finish() {
            let mut paint = Paint::default();
            paint.set_color(accent);
            paint.anti_alias = true;
            let stroke = Stroke {
                width: 2.0,
                ..Default::default()
            };
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use activity::Sample;
    use chrono::{TimeZone, Utc};

    fn theme() -> Theme {
        Theme {
            font: "Inter".into(),
            fg: "#ffffff".into(),
            accent: "#ffcc00".into(),
            shadow: None,
        }
    }

    fn mk(secs: u64, distance: Option<f64>, altitude: Option<f32>) -> Sample {
        Sample {
            t: Duration::from_secs(secs),
            lat: 0.0,
            lon: 0.0,
            altitude_m: altitude,
            speed_mps: None,
            heart_rate_bpm: None,
            cadence_rpm: None,
            power_w: None,
            distance_m: distance,
            elev_gain_cum_m: None,
            gradient_pct: None,
        }
    }

    #[test]
    fn no_altitude_renders_nothing() {
        // Every sample has distance but no altitude — nothing to draw.
        let samples = vec![
            mk(0, Some(0.0), None),
            mk(10, Some(100.0), None),
            mk(20, Some(200.0), None),
        ];
        let activity = Activity::from_samples(Utc.timestamp_opt(0, 0).unwrap(), samples);
        let mut pix = Pixmap::new(64, 64).unwrap();
        render_elevation_profile(
            &mut pix,
            &theme(),
            Rect {
                x: 0,
                y: 0,
                w: 64,
                h: 64,
            },
            &activity,
            Duration::from_secs(10),
        );
        assert!(
            pix.data().chunks_exact(4).all(|p| p[3] == 0),
            "expected transparent pixmap when altitude is missing"
        );
    }

    #[test]
    fn no_distance_renders_nothing() {
        let samples = vec![
            mk(0, None, Some(10.0)),
            mk(10, None, Some(20.0)),
            mk(20, None, Some(30.0)),
        ];
        let activity = Activity::from_samples(Utc.timestamp_opt(0, 0).unwrap(), samples);
        let mut pix = Pixmap::new(64, 64).unwrap();
        render_elevation_profile(
            &mut pix,
            &theme(),
            Rect {
                x: 0,
                y: 0,
                w: 64,
                h: 64,
            },
            &activity,
            Duration::from_secs(10),
        );
        assert!(pix.data().chunks_exact(4).all(|p| p[3] == 0));
    }

    #[test]
    fn zero_distance_span_renders_nothing() {
        // All samples at distance 0 (e.g. stationary), but with altitude.
        let samples = vec![
            mk(0, Some(0.0), Some(10.0)),
            mk(10, Some(0.0), Some(20.0)),
            mk(20, Some(0.0), Some(30.0)),
        ];
        let activity = Activity::from_samples(Utc.timestamp_opt(0, 0).unwrap(), samples);
        let mut pix = Pixmap::new(64, 64).unwrap();
        render_elevation_profile(
            &mut pix,
            &theme(),
            Rect {
                x: 0,
                y: 0,
                w: 64,
                h: 64,
            },
            &activity,
            Duration::from_secs(10),
        );
        assert!(pix.data().chunks_exact(4).all(|p| p[3] == 0));
    }

    #[test]
    fn empty_samples_renders_nothing() {
        let activity = Activity::from_samples(Utc.timestamp_opt(0, 0).unwrap(), vec![]);
        let mut pix = Pixmap::new(64, 64).unwrap();
        render_elevation_profile(
            &mut pix,
            &theme(),
            Rect {
                x: 0,
                y: 0,
                w: 64,
                h: 64,
            },
            &activity,
            Duration::ZERO,
        );
        assert!(pix.data().chunks_exact(4).all(|p| p[3] == 0));
    }

    #[test]
    fn real_profile_draws_pixels() {
        // Triangle profile rising then falling.
        let mut samples = Vec::new();
        for i in 0..=10 {
            let d = i as f64 * 50.0;
            let a = if i <= 5 {
                i as f32 * 10.0
            } else {
                (10 - i) as f32 * 10.0
            };
            samples.push(mk(i as u64, Some(d), Some(a)));
        }
        let activity = Activity::from_samples(Utc.timestamp_opt(0, 0).unwrap(), samples);
        let mut pix = Pixmap::new(64, 64).unwrap();
        render_elevation_profile(
            &mut pix,
            &theme(),
            Rect {
                x: 0,
                y: 0,
                w: 64,
                h: 64,
            },
            &activity,
            Duration::from_secs(5),
        );
        assert!(pix.data().chunks_exact(4).any(|p| p[3] > 0));
    }
}
