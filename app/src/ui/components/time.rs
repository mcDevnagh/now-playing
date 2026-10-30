//! The time of day.

use std::rc::Rc;
use std::time::Duration;

use chrono::{DateTime, Local, Timelike};
use sdl3::rect::Rect as SdlRect;
use sdl3::render::{Texture, WindowCanvas};

use super::super::text::{Face, Fonts};
use super::super::texture::{self, Creator};
use super::{Bounds, alpha};

/// The clock's size, as a fraction of its bounds' height.
const SIZE: f32 = 0.4;
const OPACITY: f32 = 0.85;

/// `now` as the clock shows it, on a 12-hour clock, e.g. `9:05 am`.
pub fn clock_text(now: &DateTime<Local>) -> String {
    now.format("%-I:%M %P").to_string()
}

/// How long from `now` until the clock next changes.
pub fn until_next_minute(now: &DateTime<Local>) -> Duration {
    let into_minute = Duration::new(now.second().into(), now.nanosecond() % 1_000_000_000);
    Duration::from_secs(60).saturating_sub(into_minute)
}

/// The clock, left-aligned and vertically centred in its bounds.
pub struct Time<'t> {
    creator: &'t Creator,
    fonts: Rc<Fonts>,
    /// The text last drawn, the height it was drawn for, and its texture.
    /// Kept until the minute or the bounds change.
    drawn: Option<(String, f32, Texture<'t>)>,
}

impl<'t> Time<'t> {
    pub fn new(creator: &'t Creator, fonts: Rc<Fonts>) -> Time<'t> {
        Time {
            creator,
            fonts,
            drawn: None,
        }
    }

    pub fn draw(
        &mut self,
        canvas: &mut WindowCanvas,
        bounds: Bounds,
        now: &DateTime<Local>,
    ) -> Result<(), String> {
        let text = clock_text(now);
        let face = Face::new(&self.fonts.regular, SIZE * bounds.h);
        let current = self
            .drawn
            .as_ref()
            .is_some_and(|(t, h, _)| *t == text && *h == bounds.h);
        if !current {
            let texture = texture::text(self.creator, &face.rasterize(&text))?;
            self.drawn = Some((text, bounds.h, texture));
        }
        let (_, _, texture) = self.drawn.as_mut().expect("clock just drawn");
        texture.set_alpha_mod(alpha(OPACITY));
        let query = texture.query();
        let dst = SdlRect::new(
            bounds.x.round() as i32,
            (bounds.y + (bounds.h - face.height()) / 2.0).round() as i32,
            query.width,
            query.height,
        );
        bounds.clip(canvas, |canvas| {
            canvas.copy(texture, None, dst).map_err(|e| e.to_string())
        })
    }
}
