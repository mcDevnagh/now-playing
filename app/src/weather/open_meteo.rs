//! The current weather from Open-Meteo (https://open-meteo.com): free, no
//! key, and Breezy Weather's suggestion where there's no national source.

use serde::Deserialize;

use super::{Condition, Location, Weather};

pub const BASE_URL: &str = "https://api.open-meteo.com";

/// The request for the current weather at `location`, in °C (Open-Meteo's
/// default).
pub fn url(base: &str, location: Location) -> String {
    format!(
        "{base}/v1/forecast?latitude={}&longitude={}&current=temperature_2m,weather_code,is_day",
        location.latitude, location.longitude
    )
}

#[derive(Deserialize)]
struct Response {
    current: Current,
}

#[derive(Deserialize)]
struct Current {
    temperature_2m: Option<f32>,
    weather_code: Option<u8>,
    is_day: Option<u8>,
}

/// The current weather in a response to `url`.
pub fn parse(body: &str) -> Result<Weather, String> {
    let response: Response = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let current = response.current;
    Ok(Weather {
        condition: current.weather_code.and_then(condition),
        // Without it, day: most icons are the same either way.
        is_day: current.is_day != Some(0),
        temperature: current.temperature_2m.ok_or("no temperature")?,
    })
}

/// A WMO weather code as Breezy Weather reads Open-Meteo's: None for codes
/// it doesn't use. Open-Meteo never reports wind, haze, hail or thunder
/// this way.
pub fn condition(code: u8) -> Option<Condition> {
    Some(match code {
        // Clear sky, mainly clear
        0 | 1 => Condition::Clear,
        2 => Condition::PartlyCloudy,
        // Overcast
        3 => Condition::Cloudy,
        // Fog, depositing rime fog
        45 | 48 => Condition::Fog,
        // Drizzle, freezing drizzle, rain, freezing rain, rain showers
        51 | 53 | 55 | 56 | 57 | 61 | 63 | 65 | 66 | 67 | 80 | 81 | 82 => Condition::Rain,
        // Snow fall, snow showers
        71 | 73 | 75 | 85 | 86 => Condition::Snow,
        // Snow grains
        77 => Condition::Sleet,
        // Thunderstorm, with slight or heavy hail
        95 | 96 | 99 => Condition::Thunderstorm,
        _ => return None,
    })
}
