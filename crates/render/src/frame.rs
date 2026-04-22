use activity::Activity;
use layout::{Layout, Widget};
use std::time::Duration;
use tiny_skia::{Color, Pixmap};

use crate::text::TextCtx;

/// Render one frame of the overlay into `pixmap`.
///
/// This is the pure per-frame entry point — given an immutable layout and
/// activity plus a time `t`, it clears the pixmap to transparent and draws
/// every widget from `layout.widgets` into it.
///
/// The caller is responsible for allocating the pixmap and for keeping a
/// long-lived `TextCtx` around. Constructing a `TextCtx` is expensive (parses
/// the bundled TTF), so the caller should reuse one per thread across frames.
/// `pixmap.width()` and `pixmap.height()` must match
/// `layout.canvas.width`/`height`.
pub fn render_frame(
    layout: &Layout,
    activity: &Activity,
    t: Duration,
    text_ctx: &mut TextCtx,
    pixmap: &mut Pixmap,
    background: Color,
) -> anyhow::Result<()> {
    if pixmap.width() != layout.canvas.width || pixmap.height() != layout.canvas.height {
        anyhow::bail!(
            "pixmap size {}x{} does not match layout canvas {}x{}",
            pixmap.width(),
            pixmap.height(),
            layout.canvas.width,
            layout.canvas.height,
        );
    }
    pixmap.fill(background);
    let rider = layout.rider.as_ref();

    for widget in &layout.widgets {
        match widget {
            Widget::Readout {
                id: _,
                metric,
                rect,
                label,
                decimals,
                font_size,
                label_font_size,
                unit_font_size,
            } => {
                crate::widgets::readout::render_readout(
                    pixmap,
                    text_ctx,
                    &layout.theme,
                    &layout.units,
                    rider,
                    *rect,
                    metric,
                    label,
                    *decimals,
                    *font_size,
                    *label_font_size,
                    *unit_font_size,
                    activity,
                    t,
                );
            }
            Widget::Course {
                id: _,
                rect,
                line_width,
                dot_radius,
            } => {
                crate::widgets::course::render_course(
                    pixmap,
                    &layout.theme,
                    *rect,
                    *line_width,
                    *dot_radius,
                    activity,
                    t,
                );
            }
            Widget::ElevationProfile { id: _, rect } => {
                crate::widgets::elevation_profile::render_elevation_profile(
                    pixmap,
                    &layout.theme,
                    *rect,
                    activity,
                    t,
                );
            }
            Widget::Bar {
                id: _,
                metric,
                rect,
                min,
                max,
                show_text,
                decimals,
            } => {
                crate::widgets::bar::render_bar(
                    pixmap,
                    text_ctx,
                    &layout.theme,
                    &layout.units,
                    rider,
                    *rect,
                    metric,
                    *min,
                    *max,
                    *show_text,
                    *decimals,
                    activity,
                    t,
                );
            }
            Widget::Meter { .. } => {
                // Implemented in Task 4.
            }
            Widget::Gauge { .. } => {
                // Implemented in Task 6.
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use activity::{Activity, Sample};
    use chrono::{TimeZone, Utc};
    use layout::{Canvas, DistanceUnit, ElevationUnit, Layout, SpeedUnit, TempUnit, Theme, Units};
    use std::time::Duration;
    use tiny_skia::Pixmap;

    fn minimal_layout(w: u32, h: u32, fps: u32) -> Layout {
        Layout {
            version: 1,
            canvas: Canvas {
                width: w,
                height: h,
                fps,
            },
            units: Units {
                speed: SpeedUnit::Kmh,
                distance: DistanceUnit::Km,
                elevation: ElevationUnit::M,
                temp: TempUnit::C,
            },
            theme: Theme {
                font: "Inter".into(),
                fg: "#ffffff".into(),
                accent: "#ffcc00".into(),
                shadow: None,
            },
            rider: None,
            widgets: vec![],
        }
    }

    fn one_sample_activity() -> Activity {
        let s = Sample {
            t: Duration::ZERO,
            lat: 0.0,
            lon: 0.0,
            altitude_m: Some(100.0),
            speed_mps: Some(5.0),
            heart_rate_bpm: None,
            cadence_rpm: None,
            power_w: None,
            distance_m: Some(0.0),
            elev_gain_cum_m: Some(0.0),
            gradient_pct: Some(0.0),
        };
        Activity::from_samples(Utc.timestamp_opt(0, 0).unwrap(), vec![s])
    }

    #[test]
    fn empty_layout_renders_transparent() {
        let layout = minimal_layout(100, 100, 30);
        let activity = one_sample_activity();
        let mut ctx = TextCtx::new();
        let mut pix = Pixmap::new(100, 100).unwrap();
        // Pre-fill with red so a successful clear is observable.
        pix.fill(tiny_skia::Color::from_rgba8(255, 0, 0, 255));
        render_frame(
            &layout,
            &activity,
            Duration::ZERO,
            &mut ctx,
            &mut pix,
            Color::TRANSPARENT,
        )
        .unwrap();
        // Every pixel must be fully transparent after render.
        assert!(
            pix.data().chunks_exact(4).all(|p| p[3] == 0),
            "found non-transparent pixel"
        );
    }

    #[test]
    fn render_frame_fails_on_mismatched_pixmap_size() {
        let layout = minimal_layout(200, 100, 30);
        let activity = one_sample_activity();
        let mut ctx = TextCtx::new();
        let mut pix = Pixmap::new(100, 100).unwrap();
        let r = render_frame(
            &layout,
            &activity,
            Duration::ZERO,
            &mut ctx,
            &mut pix,
            Color::TRANSPARENT,
        );
        assert!(r.is_err());
    }
}
