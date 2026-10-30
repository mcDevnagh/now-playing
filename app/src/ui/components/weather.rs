//! The current weather: its condition's icon, then the temperature,
//! right-aligned.
//!
//! The icons are Breezy Weather's; see `assets/weather/README.md`.

use std::rc::Rc;

use sdl3::rect::Rect as SdlRect;
use sdl3::render::{Texture, WindowCanvas};

use super::super::text::{Face, Fonts};
use super::super::texture::{self, Creator};
use super::{Bounds, TOP_TEXT_OPACITY, TOP_TEXT_SIZE, alpha};
use crate::art::Image;
use crate::weather::{self as model, Condition, Units};

/// The icon's side, as a fraction of the bounds' height.
const ICON_SIZE: f32 = 0.5;
/// Between the icon and the temperature, as a fraction of the bounds'
/// height. Small, as the icons have room around them.
const GAP: f32 = 0.04;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    ClearDay,
    ClearNight,
    PartlyCloudyDay,
    PartlyCloudyNight,
    Cloudy,
    Rain,
    Snow,
    Sleet,
    Hail,
    Thunder,
    Thunderstorm,
    Fog,
    Haze,
    Wind,
}

impl Icon {
    pub const ALL: [Icon; 14] = [
        Icon::ClearDay,
        Icon::ClearNight,
        Icon::PartlyCloudyDay,
        Icon::PartlyCloudyNight,
        Icon::Cloudy,
        Icon::Rain,
        Icon::Snow,
        Icon::Sleet,
        Icon::Hail,
        Icon::Thunder,
        Icon::Thunderstorm,
        Icon::Fog,
        Icon::Haze,
        Icon::Wind,
    ];

    /// The icon for `condition`. Only clear and partly cloudy skies have
    /// night versions, as in Breezy Weather.
    pub fn new(condition: Condition, is_day: bool) -> Icon {
        match condition {
            Condition::Clear if is_day => Icon::ClearDay,
            Condition::Clear => Icon::ClearNight,
            Condition::PartlyCloudy if is_day => Icon::PartlyCloudyDay,
            Condition::PartlyCloudy => Icon::PartlyCloudyNight,
            Condition::Cloudy => Icon::Cloudy,
            Condition::Rain => Icon::Rain,
            Condition::Snow => Icon::Snow,
            Condition::Sleet => Icon::Sleet,
            Condition::Hail => Icon::Hail,
            Condition::Thunder => Icon::Thunder,
            Condition::Thunderstorm => Icon::Thunderstorm,
            Condition::Fog => Icon::Fog,
            Condition::Haze => Icon::Haze,
            Condition::Wind => Icon::Wind,
        }
    }

    fn png(self) -> &'static [u8] {
        macro_rules! icon {
            ($name:literal) => {
                include_bytes!(concat!("../../../assets/weather/weather_", $name, ".png"))
            };
        }
        match self {
            Icon::ClearDay => icon!("clear_day"),
            Icon::ClearNight => icon!("clear_night"),
            Icon::PartlyCloudyDay => icon!("partly_cloudy_day"),
            Icon::PartlyCloudyNight => icon!("partly_cloudy_night"),
            Icon::Cloudy => icon!("cloudy"),
            Icon::Rain => icon!("rain"),
            Icon::Snow => icon!("snow"),
            Icon::Sleet => icon!("sleet"),
            Icon::Hail => icon!("hail"),
            Icon::Thunder => icon!("thunder"),
            Icon::Thunderstorm => icon!("thunderstorm"),
            Icon::Fog => icon!("fog"),
            Icon::Haze => icon!("haze"),
            Icon::Wind => icon!("wind"),
        }
    }

    /// The bundled PNG, decoded.
    pub fn decode(self) -> Result<Image, String> {
        let image = image::load_from_memory(self.png()).map_err(|e| e.to_string())?;
        let rgba = image.to_rgba8();
        Ok(Image {
            width: rgba.width(),
            height: rgba.height(),
            rgba: rgba.into_raw(),
        })
    }
}

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

/// The temperature's right edge on the bounds' right edge, on the same
/// line as the clock; the icon to its left, centred on the digits.
pub fn layout(fonts: &Fonts, weather: &model::Weather, units: Units, bounds: Bounds) -> Layout {
    let face = Face::new(&fonts.regular, TOP_TEXT_SIZE * bounds.h);
    let text = temperature_text(weather.temperature, units);
    let text_width = face.width(&text);
    let text_x = bounds.x + bounds.w - text_width;
    let text_y = bounds.y + (bounds.h - face.height()) / 2.0;
    let icon = weather.condition.map(|condition| {
        let side = ICON_SIZE * bounds.h;
        let middle = text_y + face.middle_of('0');
        let place = Bounds {
            x: text_x - GAP * bounds.h - side,
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
    /// Each icon's texture, in `Icon::ALL`'s order, uploaded when first
    /// shown.
    icons: [Option<Texture<'t>>; Icon::ALL.len()],
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
            icons: Default::default(),
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
            Some((icon, place)) => Some((uploaded(&mut self.icons, self.creator, icon)?, place)),
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

/// `icon`'s texture from `icons`, uploading it first if it isn't yet.
fn uploaded<'a, 't>(
    icons: &'a mut [Option<Texture<'t>>; Icon::ALL.len()],
    creator: &'t Creator,
    icon: Icon,
) -> Result<&'a Texture<'t>, String> {
    let index = Icon::ALL
        .iter()
        .position(|&i| i == icon)
        .expect("every icon is in ALL");
    let slot = &mut icons[index];
    if slot.is_none() {
        *slot = Some(texture::rgba(creator, &icon.decode()?)?);
    }
    Ok(slot.as_ref().expect("icon just uploaded"))
}
