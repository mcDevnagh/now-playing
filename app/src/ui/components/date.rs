//! Today's date, e.g. `Fri 10/30`, centred.

use std::rc::Rc;

use chrono::NaiveDate;
use sdl3::rect::Rect as SdlRect;
use sdl3::render::{Texture, WindowCanvas};

use super::super::text::{Face, Fonts};
use super::super::texture::{self, Creator};
use super::{Bounds, TOP_TEXT_OPACITY, TOP_TEXT_SIZE, alpha};

/// `date` as shown: the day's name, then month/day, e.g. `Fri 10/30`.
pub fn date_text(date: NaiveDate) -> String {
    date.format("%a %-m/%-d").to_string()
}

/// The top left of `text` in `face`, centred in `bounds`: on the same line
/// as the clock, which is vertically centred in the same band.
pub fn place(face: &Face, text: &str, bounds: Bounds) -> (f32, f32) {
    (
        bounds.x + (bounds.w - face.width(text)) / 2.0,
        bounds.y + (bounds.h - face.height()) / 2.0,
    )
}

pub struct Date<'t> {
    creator: &'t Creator,
    fonts: Rc<Fonts>,
    /// The text last drawn, the height it was drawn for, and its texture.
    /// Kept until the date or the bounds change.
    drawn: Option<(String, f32, Texture<'t>)>,
}

impl<'t> Date<'t> {
    pub fn new(creator: &'t Creator, fonts: Rc<Fonts>) -> Date<'t> {
        Date {
            creator,
            fonts,
            drawn: None,
        }
    }

    pub fn draw(
        &mut self,
        canvas: &mut WindowCanvas,
        bounds: Bounds,
        today: NaiveDate,
    ) -> Result<(), String> {
        let text = date_text(today);
        let face = Face::new(&self.fonts.regular, TOP_TEXT_SIZE * bounds.h);
        let (x, y) = place(&face, &text, bounds);
        let current = self
            .drawn
            .as_ref()
            .is_some_and(|(t, h, _)| *t == text && *h == bounds.h);
        if !current {
            let texture = texture::text(self.creator, &face.rasterize(&text))?;
            self.drawn = Some((text, bounds.h, texture));
        }
        let (_, _, texture) = self.drawn.as_mut().expect("date just drawn");
        texture.set_alpha_mod(alpha(TOP_TEXT_OPACITY));
        let query = texture.query();
        let dst = SdlRect::new(
            x.round() as i32,
            y.round() as i32,
            query.width,
            query.height,
        );
        bounds.clip(canvas, |canvas| {
            canvas.copy(texture, None, dst).map_err(|e| e.to_string())
        })
    }
}
