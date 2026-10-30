//! The current weather: what the screen shows, and fetching it.

mod follow;
pub mod locate;
pub mod open_meteo;

pub use follow::{EVERY, STALE, Schedule, Source, follow};

use chrono::NaiveDate;
use clap::ValueEnum;

/// Where to show the weather for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Location {
    pub latitude: f64,
    pub longitude: f64,
}

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

/// One day's forecast.
#[derive(Debug, Clone, PartialEq)]
pub struct Day {
    /// The date where the weather is.
    pub date: NaiveDate,
    /// The day's main condition; None as for `Weather`.
    pub condition: Option<Condition>,
    /// In °C.
    pub high: f32,
    pub low: f32,
}

/// The weather now, and the next few days' forecast, today first.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub current: Weather,
    pub days: Vec<Day>,
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

#[cfg(test)]
mod tests;
