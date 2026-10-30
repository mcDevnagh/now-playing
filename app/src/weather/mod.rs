//! The current weather, as the screen shows it.

use clap::ValueEnum;

/// What the weather is doing: Breezy Weather's 12 conditions, which every
/// source it supports is reduced to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Condition {
    Clear,
    PartlyCloudy,
    Cloudy,
    Rain,
    Snow,
    Wind,
    Fog,
    Haze,
    Sleet,
    Hail,
    Thunder,
    Thunderstorm,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Weather {
    /// None if the source reported a condition outside the 12.
    pub condition: Option<Condition>,
    pub is_day: bool,
    /// In °C, as sources report it; converted for display.
    pub temperature: f32,
}

/// The units temperatures are shown in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum Units {
    #[default]
    Fahrenheit,
    Celsius,
}

impl Units {
    /// `celsius` in these units.
    pub fn from_celsius(self, celsius: f32) -> f32 {
        match self {
            Units::Fahrenheit => celsius * 9.0 / 5.0 + 32.0,
            Units::Celsius => celsius,
        }
    }

    /// `degrees` in these units, in °C.
    pub fn to_celsius(self, degrees: f32) -> f32 {
        match self {
            Units::Fahrenheit => (degrees - 32.0) * 5.0 / 9.0,
            Units::Celsius => degrees,
        }
    }
}
