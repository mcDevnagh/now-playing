//! The current weather: its condition's icon, then the temperature,
//! centred.

use std::rc::Rc;

use sdl3::rect::Rect as SdlRect;
use sdl3::render::{Texture, WindowCanvas};

use super::super::icons::{Icon, Icons};
use super::super::text::{Face, Fonts};
use super::super::texture::{self, Creator};
use super::{Bounds, TOP_TEXT_OPACITY, TOP_TEXT_SIZE, alpha};
use crate::weather::{self as model, Units};

/// The icon's side, as a fraction of the bounds' height.
pub const ICON_SIZE: f32 = 0.5;
/// Between the icon and the temperature, as a fraction of the bounds'
/// height. Small, as the icons have room around them.
const GAP: f32 = 0.04;

/// The temperature as shown, e.g. `57°`: rounded, in `units`.
pub fn temperature_text(celsius: f32, units: Units) -> String {
    let degrees = units.from_celsius(celsius).round();
    // Not `-0°`.
    let degrees = if degrees == 0.0 { 0.0 } else { degrees };
    format!("{degrees}°")
}

#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    /// The icon and where it goes, if the condition has one.
    pub icon: Option<(Icon, Bounds)>,
    pub text: String,
    /// The top left of the rasterised temperature.
    pub text_x: f32,
    pub text_y: f32,
    pub text_width: f32,
}

/// The icon and the temperature to its right, centred together in the
/// bounds, on the same line as the clock; the icon centred on the digits.
pub fn layout(fonts: &Fonts, weather: &model::Weather, units: Units, bounds: Bounds) -> Layout {
    let face = Face::new(&fonts.regular, TOP_TEXT_SIZE * bounds.h);
    let text = temperature_text(weather.temperature, units);
    let text_width = face.width(&text);
    let side = ICON_SIZE * bounds.h;
    let icon_width = match weather.condition {
        Some(_) => side + GAP * bounds.h,
        None => 0.0,
    };
    let left = bounds.x + (bounds.w - (icon_width + text_width)) / 2.0;
    let text_x = left + icon_width;
    let text_y = bounds.y + (bounds.h - face.height()) / 2.0;
    let icon = weather.condition.map(|condition| {
        let middle = text_y + face.middle_of('0');
        let place = Bounds {
            x: left,
            y: middle - side / 2.0,
            w: side,
            h: side,
        };
        (Icon::new(condition, weather.is_day), place)
    });
    Layout {
        icon,
        text,
        text_x,
        text_y,
        text_width,
    }
}

pub struct Weather<'t> {
    creator: &'t Creator,
    fonts: Rc<Fonts>,
    units: Units,
    icons: Icons<'t>,
    /// The temperature last drawn, the height it was drawn for, and its
    /// texture.
    text: Option<(String, f32, Texture<'t>)>,
}

impl<'t> Weather<'t> {
    pub fn new(creator: &'t Creator, fonts: Rc<Fonts>, units: Units) -> Weather<'t> {
        Weather {
            creator,
            fonts,
            units,
            icons: Icons::new(creator),
            text: None,
        }
    }

    /// Draws `weather` in `bounds`; nothing if there's none.
    pub fn draw(
        &mut self,
        canvas: &mut WindowCanvas,
        bounds: Bounds,
        weather: Option<&model::Weather>,
    ) -> Result<(), String> {
        let Some(weather) = weather else {
            return Ok(());
        };
        let layout = layout(&self.fonts, weather, self.units, bounds);
        let current = self
            .text
            .as_ref()
            .is_some_and(|(t, h, _)| *t == layout.text && *h == bounds.h);
        if !current {
            let face = Face::new(&self.fonts.regular, TOP_TEXT_SIZE * bounds.h);
            let texture = texture::text(self.creator, &face.rasterize(&layout.text))?;
            self.text = Some((layout.text.clone(), bounds.h, texture));
        }
        let icon = match layout.icon {
            Some((icon, place)) => Some((self.icons.get(icon)?, place)),
            None => None,
        };
        let (_, _, text) = self.text.as_mut().expect("temperature just drawn");
        bounds.clip(canvas, |canvas| {
            if let Some((icon, place)) = icon {
                canvas
                    .copy(icon, None, place.sdl())
                    .map_err(|e| e.to_string())?;
            }
            text.set_alpha_mod(alpha(TOP_TEXT_OPACITY));
            let query = text.query();
            let dst = SdlRect::new(
                layout.text_x.round() as i32,
                layout.text_y.round() as i32,
                query.width,
                query.height,
            );
            canvas.copy(text, None, dst).map_err(|e| e.to_string())
        })
    }
}
