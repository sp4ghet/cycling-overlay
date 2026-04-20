//! Thin wrapper over `cosmic-text` that rasterizes shaped text directly into
//! a `tiny_skia::Pixmap`.
//!
//! The goal is to give the widget-drawing code in Task 17+ a single
//! `TextCtx::draw` entry point that handles font loading, shaping, glyph
//! rasterization, and source-over compositing without each widget needing to
//! touch cosmic-text internals.
//!
//! Only the bundled Inter font is loaded — system fonts are deliberately
//! skipped so rendering is deterministic across machines (important for test
//! stability and for producing the same output on a CI runner as on a dev
//! box).

use cosmic_text::{
    Attrs, Buffer, Color as CtColor, Family, FontSystem, Metrics, Shaping, SwashCache,
};
use tiny_skia::{Color, Pixmap};

/// Bundled copy of Inter — baked into the binary so the render crate has no
/// runtime font dependency.
const INTER_VARIABLE_FONT: &[u8] = include_bytes!("../assets/Inter-VariableFont.ttf");

/// Owns the cosmic-text `FontSystem` + glyph cache.
///
/// Reuse a single `TextCtx` across frames — the swash cache amortizes glyph
/// rasterization cost, and re-creating the `FontSystem` on every frame would
/// re-parse the TTF each time.
pub struct TextCtx {
    font_system: FontSystem,
    swash_cache: SwashCache,
}

impl Default for TextCtx {
    fn default() -> Self {
        Self::new()
    }
}

impl TextCtx {
    /// Build a `TextCtx` with only the bundled Inter font loaded.
    pub fn new() -> Self {
        let mut db = cosmic_text::fontdb::Database::new();
        db.load_font_data(INTER_VARIABLE_FONT.to_vec());
        let font_system = FontSystem::new_with_locale_and_db("en-US".into(), db);
        Self {
            font_system,
            swash_cache: SwashCache::new(),
        }
    }

    /// Draw `text` onto `pixmap` with its layout box anchored at `(x, y)`
    /// (top-left), at `font_size` pixels, composited in `color`.
    ///
    /// Uses source-over compositing against whatever is already in the
    /// pixmap. Coordinates outside the pixmap are clipped.
    pub fn draw(
        &mut self,
        pixmap: &mut Pixmap,
        text: &str,
        x: f32,
        y: f32,
        font_size: f32,
        color: Color,
    ) {
        let metrics = Metrics::new(font_size, font_size * 1.2);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        {
            let mut bw = buffer.borrow_with(&mut self.font_system);
            // Size the layout box so text doesn't silently wrap to zero width.
            bw.set_size(
                Some(pixmap.width() as f32),
                Some(pixmap.height() as f32),
            );
            bw.set_text(
                text,
                Attrs::new().family(Family::Name("Inter")),
                Shaping::Advanced,
            );
            bw.shape_until_scroll(true);
        }

        let rgba = color.to_color_u8();
        // The `color` passed to Buffer::draw supplies RGB; the callback's
        // Color's alpha channel carries the per-pixel glyph coverage.
        let fill = CtColor::rgba(rgba.red(), rgba.green(), rgba.blue(), 255);

        let pw = pixmap.width() as i32;
        let ph = pixmap.height() as i32;
        let stride = pixmap.width() as usize * 4;
        let x_i = x.round() as i32;
        let y_i = y.round() as i32;

        buffer.draw(
            &mut self.font_system,
            &mut self.swash_cache,
            fill,
            |gx, gy, gw, gh, c| {
                let cov = c.a();
                if cov == 0 {
                    return;
                }
                let cov_f = cov as f32 / 255.0;
                // Pre-multiply source color by coverage for correct
                // source-over alpha blending.
                let src_r = c.r() as f32 * cov_f;
                let src_g = c.g() as f32 * cov_f;
                let src_b = c.b() as f32 * cov_f;
                let inv = 1.0 - cov_f;

                let data = pixmap.data_mut();
                for row in 0..gh as i32 {
                    let py = y_i + gy + row;
                    if py < 0 || py >= ph {
                        continue;
                    }
                    for col in 0..gw as i32 {
                        let px = x_i + gx + col;
                        if px < 0 || px >= pw {
                            continue;
                        }
                        let idx = (py as usize) * stride + (px as usize) * 4;
                        let dr = data[idx] as f32;
                        let dg = data[idx + 1] as f32;
                        let db_ = data[idx + 2] as f32;
                        let da = data[idx + 3] as f32;
                        data[idx] = (src_r + dr * inv).round().clamp(0.0, 255.0) as u8;
                        data[idx + 1] = (src_g + dg * inv).round().clamp(0.0, 255.0) as u8;
                        data[idx + 2] = (src_b + db_ * inv).round().clamp(0.0, 255.0) as u8;
                        data[idx + 3] =
                            (cov as f32 + da * inv).round().clamp(0.0, 255.0) as u8;
                    }
                }
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_skia::Pixmap;

    #[test]
    fn draws_non_transparent_pixels() {
        let mut ctx = TextCtx::new();
        let mut pix = Pixmap::new(200, 100).unwrap();
        ctx.draw(&mut pix, "HELLO", 10.0, 50.0, 32.0, tiny_skia::Color::WHITE);
        // At least one pixel in the expected region should be non-transparent.
        let data = pix.data();
        let mut any_nontransparent = false;
        for i in 0..data.len() / 4 {
            if data[i * 4 + 3] != 0 {
                any_nontransparent = true;
                break;
            }
        }
        assert!(any_nontransparent, "no text pixels drawn");
    }

    #[test]
    fn respects_draw_position_roughly() {
        // Text at x=150 shouldn't leak into the x=[0..10] region.
        let mut ctx = TextCtx::new();
        let mut pix = Pixmap::new(200, 100).unwrap();
        ctx.draw(&mut pix, "HI", 150.0, 50.0, 32.0, tiny_skia::Color::WHITE);
        let data = pix.data();
        // Sample the leftmost column's pixels — they should all be transparent.
        let w = pix.width() as usize;
        for y in 0..pix.height() as usize {
            for x in 0..10 {
                let i = (y * w + x) * 4;
                assert_eq!(
                    data[i + 3],
                    0,
                    "pixel at x={} y={} should be transparent",
                    x,
                    y
                );
            }
        }
    }
}
