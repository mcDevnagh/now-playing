//! The next few days' forecast: a column per day, today on the left, each
//! with the day's name (`Today` for today), its icon, and its high and
//! low.

use std::rc::Rc;

use chrono::NaiveDate;
use sdl3::rect::Rect as SdlRect;
use sdl3::render::{Texture, WindowCanvas};

use super::super::icons::{Icon, Icons};
use super::super::text::{Face, Fonts};
use super::super::texture::{self, Creator};
use super::weather::temperature_text;
use super::{Bounds, TOP_TEXT_OPACITY, alpha};
use crate::weather::{Day, Units};

/// The most days shown.
pub const DAYS: usize = 5;
/// The icons' side, as a fraction of the bounds' height. The forecast's
/// bounds are twice as high as the current weather's, so this is half its
/// `ICON_SIZE`: the same size on screen.
pub const ICON_SIZE: f32 = super::weather::ICON_SIZE / 2.0;
/// The text's size, as a fraction of the bounds' height: smaller than the
/// current temperature's.
const TEXT_SIZE: f32 = 0.09;
/// Between the icon and the text above and below it, as a fraction of the
/// bounds' height. Small, as the icons have room around them.
const GAP: f32 = 0.02;

/// The first `DAYS` of `days` from `today` on: a forecast fetched before
/// midnight still starts with today after it.
pub fn from(days: &[Day], today: NaiveDate) -> &[Day] {
    let start = days.iter().position(|day| day.date >= today);
    let days = &days[start.unwrap_or(days.len())..];
    &days[..days.len().min(DAYS)]
}

/// `date` as the forecast names it: `Today`, else e.g. `Wed`.
pub fn day_text(date: NaiveDate, today: NaiveDate) -> String {
    if date == today {
        return "Today".to_owned();
    }
    date.format("%a").to_string()
}

/// A day's high and low, e.g. `61°/48°`.
pub fn high_low_text(day: &Day, units: Units) -> String {
    format!(
        "{}/{}",
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    pub day: Line,
    /// The icon and where it goes, if the day's condition has one. Always
    /// a day icon, never a night one.
    pub icon: Option<(Icon, Bounds)>,
    pub high_low: Line,
}

/// The first `DAYS` of `days`, a column each, spread evenly across the
/// bounds and centred in them: the day's name, the icon, then the high and
/// low.
pub fn layout(
    fonts: &Fonts,
    days: &[Day],
    today: NaiveDate,
    units: Units,
    bounds: Bounds,
) -> Vec<Column> {
    let days = &days[..days.len().min(DAYS)];
    let face = Face::new(&fonts.regular, TEXT_SIZE * bounds.h);
    let side = ICON_SIZE * bounds.h;
    let gap = GAP * bounds.h;
    let block = face.height() + gap + side + gap + face.height();
    let top = bounds.y + (bounds.h - block) / 2.0;
    let width = bounds.w / days.len().max(1) as f32;
    let line = |text: String, centre: f32, y: f32| {
        let width = face.width(&text);
        Line {
            x: centre - width / 2.0,
            y,
            width,
            text,
        }
    };
    days.iter()
        .enumerate()
        .map(|(i, day)| {
            let centre = bounds.x + (i as f32 + 0.5) * width;
            let icon_y = top + face.height() + gap;
            let icon = day.condition.map(|condition| {
                let place = Bounds {
                    x: centre - side / 2.0,
                    y: icon_y,
                    w: side,
                    h: side,
                };
                (Icon::new(condition, true), place)
            });
            Column {
                day: line(day_text(day.date, today), centre, top),
                icon,
                high_low: line(high_low_text(day, units), centre, icon_y + side + gap),
            }
        })
        .collect()
}

/// Each column's rasterised text: its name, and its high and low.
struct Drawn<'t> {
    days: Vec<Day>,
    today: NaiveDate,
    bounds: Bounds,
    layout: Vec<Column>,
    textures: Vec<(Texture<'t>, Texture<'t>)>,
}

pub struct Forecast<'t> {
    creator: &'t Creator,
    fonts: Rc<Fonts>,
    units: Units,
    icons: Icons<'t>,
    /// Kept until the days, the date or the bounds change.
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

    /// Draws `days`, from `today` on, in `bounds`; nothing if there are
    /// none.
    pub fn draw(
        &mut self,
        canvas: &mut WindowCanvas,
        bounds: Bounds,
        days: &[Day],
        today: NaiveDate,
    ) -> Result<(), String> {
        if days.is_empty() {
            return Ok(());
        }
        let current = self
            .drawn
            .as_ref()
            .is_some_and(|d| d.days == days && d.today == today && d.bounds == bounds);
        if !current {
            let layout = layout(&self.fonts, days, today, self.units, bounds);
            let face = Face::new(&self.fonts.regular, TEXT_SIZE * bounds.h);
            let text = |line: &Line| texture::text(self.creator, &face.rasterize(&line.text));
            let textures = layout
                .iter()
                .map(|column| Ok((text(&column.day)?, text(&column.high_low)?)))
                .collect::<Result<_, String>>()?;
            self.drawn = Some(Drawn {
                days: days.to_vec(),
                today,
                bounds,
                layout,
                textures,
            });
        }
        let drawn = self.drawn.as_mut().expect("forecast just drawn");
        let icons = &mut self.icons;
        bounds.clip(canvas, |canvas| {
            for (column, (day, high_low)) in drawn.layout.iter().zip(&mut drawn.textures) {
                if let Some((icon, place)) = column.icon {
                    canvas
                        .copy(icons.get(icon)?, None, place.sdl())
                        .map_err(|e| e.to_string())?;
                }
                for (line, texture) in [(&column.day, day), (&column.high_low, high_low)] {
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
            }
            Ok(())
        })
    }
}
