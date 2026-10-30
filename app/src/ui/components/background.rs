//! The blurred, darkened cover behind everything.

use sdl3::pixels::Color;
use sdl3::render::{Texture, WindowCanvas};

use super::Bounds;

/// How much the blurred background is darkened: to 35% brightness, as on
/// the old web page.
const DIM: u8 = 166;

pub struct Background;

impl Background {
    /// Fills `bounds` with `blur`, the tiny cover, stretched (cropped to
    /// fill them, like CSS `background-size: cover`) and darkened. Without
    /// one, they're left black.
    pub fn draw(
        &self,
        canvas: &mut WindowCanvas,
        bounds: Bounds,
        blur: Option<&mut Texture>,
    ) -> Result<(), String> {
        bounds.clip(canvas, |canvas| {
            canvas.set_draw_color(Color::BLACK);
            canvas.fill_rect(bounds.sdl()).map_err(|e| e.to_string())?;
            let Some(blur) = blur else { return Ok(()) };
            let side = bounds.w.max(bounds.h);
            let fill = Bounds {
                x: bounds.x + (bounds.w - side) / 2.0,
                y: bounds.y + (bounds.h - side) / 2.0,
                w: side,
                h: side,
            };
            canvas
                .copy(blur, None, fill.sdl())
                .map_err(|e| e.to_string())?;
            canvas.set_draw_color(Color::RGBA(0, 0, 0, DIM));
            canvas.fill_rect(bounds.sdl()).map_err(|e| e.to_string())
        })
    }
}
