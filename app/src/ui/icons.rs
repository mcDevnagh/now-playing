//! Breezy Weather's weather icons; see `assets/weather/README.md`.

use sdl3::render::Texture;

use super::texture::{self, Creator};
use crate::art::Image;
use crate::weather::Condition;

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
                include_bytes!(concat!("../../assets/weather/weather_", $name, ".png"))
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

/// The icons' textures, each uploaded when first shown.
pub struct Icons<'t> {
    creator: &'t Creator,
    /// In `Icon::ALL`'s order.
    textures: [Option<Texture<'t>>; Icon::ALL.len()],
}

impl<'t> Icons<'t> {
    pub fn new(creator: &'t Creator) -> Icons<'t> {
        Icons {
            creator,
            textures: Default::default(),
        }
    }

    /// `icon`'s texture, uploading it first if it isn't yet.
    pub fn get(&mut self, icon: Icon) -> Result<&Texture<'t>, String> {
        let index = Icon::ALL
            .iter()
            .position(|&i| i == icon)
            .expect("every icon is in ALL");
        let slot = &mut self.textures[index];
        if slot.is_none() {
            *slot = Some(texture::rgba(self.creator, &icon.decode()?)?);
        }
        Ok(slot.as_ref().expect("icon just uploaded"))
    }
}
