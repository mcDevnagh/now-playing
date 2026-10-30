//! Today's forecast, right-aligned: two small lines, the date and the high
//! and low, together about one line of the clock's text high, then its
//! icon.

use std::rc::Rc;

use chrono::NaiveDate;
use sdl3::rect::Rect as SdlRect;
use sdl3::render::{Texture, WindowCanvas};

use super::super::icons::{Icon, Icons};
use super::super::text::{Face, Fonts};
use super::super::texture::{self, Creator};
use super::weather::{GAP, ICON_SIZE, temperature_text};
use super::{Bounds, TOP_TEXT_OPACITY, TOP_TEXT_SIZE, alpha};
use crate::weather::{Day, Units};

/// The two lines' size, as a fraction of the clock's.
const SMALL: f32 = 0.45;
/// The space between the two lines' digits, as a fraction of their height.
const LINE_SPACE: f32 = 0.35;

/// Today's forecast in `days`, if there is one.
pub fn today(days: &[Day], today: NaiveDate) -> Option<&Day> {
    days.iter().find(|day| day.date == today)
}

/// The first line: the day's name and month/day, e.g. `Fri 10/30`.
pub fn date_text(date: NaiveDate) -> String {
    date.format("%a %-m/%-d").to_string()
}

/// The second line: the high and low, e.g. `61° / 46°`.
pub fn high_low_text(day: &Day, units: Units) -> String {
    format!(
        "{} / {}",
        temperature_text(day.high, units),
        temperature_text(day.low, units)
    )
}

/// One line of text; `x`, `y` are the top left of its rasterised bitmap.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    /// The icon and where it goes, if the day's condition has one. Always
    /// a day icon, never a night one.
    pub icon: Option<(Icon, Bounds)>,
    pub date: Line,
    pub high_low: Line,
}

/// The small face, for a band `height` pixels high.
fn small_face(fonts: &Fonts, height: f32) -> Face<'_> {
    Face::new(&fonts.regular, SMALL * TOP_TEXT_SIZE * height)
}

/// The icon on the bounds' right edge, the two lines right-aligned to its
/// left, mirroring the current weather on the left. The icon and the pair
/// of lines are both centred on the clock's digits, so all three line up.
pub fn layout(fonts: &Fonts, day: &Day, units: Units, bounds: Bounds) -> Layout {
    let clock = Face::new(&fonts.regular, TOP_TEXT_SIZE * bounds.h);
    let middle = bounds.y + (bounds.h - clock.height()) / 2.0 + clock.middle_of('0');
    let face = small_face(fonts, bounds.h);
    let date = date_text(day.date);
    let high_low = high_low_text(day, units);
    let side = ICON_SIZE * bounds.h;
    let right = bounds.x + bounds.w;
    let text_right = match day.condition {
        Some(_) => right - side - GAP * bounds.h,
        None => right,
    };
    // Spaced and centred by the digits' ink: the lines' boxes leave room
    // for accents above and descenders below, which would push them apart.
    let (ink_top, ink_bottom) = face.ink_of('0');
    let digits = ink_bottom - ink_top;
    let space = LINE_SPACE * digits;
    let top = middle - (2.0 * digits + space) / 2.0 - ink_top;
    let line = |text: String, y: f32| {
        let width = face.width(&text);
        Line {
            x: text_right - width,
            width,
            height: face.height(),
            y,
            text,
        }
    };
    let icon = day.condition.map(|condition| {
        let place = Bounds {
            x: right - side,
            y: middle - side / 2.0,
            w: side,
            h: side,
        };
        (Icon::new(condition, true), place)
    });
    Layout {
        icon,
        date: line(date, top),
        high_low: line(high_low, top + digits + space),
    }
}

/// A layout with its two lines rasterised.
struct Drawn<'t> {
    day: Day,
    bounds: Bounds,
    layout: Layout,
    date: Texture<'t>,
    high_low: Texture<'t>,
}

pub struct Forecast<'t> {
    creator: &'t Creator,
    fonts: Rc<Fonts>,
    units: Units,
    icons: Icons<'t>,
    /// Kept until the day or the bounds change.
    drawn: Option<Drawn<'t>>,
}

impl<'t> Forecast<'t> {
    pub fn new(creator: &'t Creator, fonts: Rc<Fonts>, units: Units) -> Forecast<'t> {
        Forecast {
            creator,
            fonts,
            units,
            icons: Icons::new(creator),
            drawn: None,
        }
    }

    /// Draws `day`'s forecast in `bounds`; nothing if there's none.
    pub fn draw(
        &mut self,
        canvas: &mut WindowCanvas,
        bounds: Bounds,
        day: Option<&Day>,
    ) -> Result<(), String> {
        let Some(day) = day else {
            return Ok(());
        };
        let current = self
            .drawn
            .as_ref()
            .is_some_and(|d| d.day == *day && d.bounds == bounds);
        if !current {
            let layout = layout(&self.fonts, day, self.units, bounds);
            let face = small_face(&self.fonts, bounds.h);
            let text = |line: &Line| texture::text(self.creator, &face.rasterize(&line.text));
            self.drawn = Some(Drawn {
                day: day.clone(),
                bounds,
                date: text(&layout.date)?,
                high_low: text(&layout.high_low)?,
                layout,
            });
        }
        let drawn = self.drawn.as_mut().expect("forecast just drawn");
        let icon = match drawn.layout.icon {
            Some((icon, place)) => Some((self.icons.get(icon)?, place)),
            None => None,
        };
        bounds.clip(canvas, |canvas| {
            if let Some((icon, place)) = icon {
                canvas
                    .copy(icon, None, place.sdl())
                    .map_err(|e| e.to_string())?;
            }
            let lines = [
                (&drawn.layout.date, &mut drawn.date),
                (&drawn.layout.high_low, &mut drawn.high_low),
            ];
            for (line, texture) in lines {
                texture.set_alpha_mod(alpha(TOP_TEXT_OPACITY));
                let query = texture.query();
                let dst = SdlRect::new(
                    line.x.round() as i32,
                    line.y.round() as i32,
                    query.width,
                    query.height,
                );
                canvas.copy(texture, None, dst).map_err(|e| e.to_string())?;
            }
            Ok(())
        })
    }
}
