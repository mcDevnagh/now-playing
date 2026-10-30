//! The weather from Open-Meteo (https://open-meteo.com): free, no
//! key, and Breezy Weather's suggestion where there's no national source.

use chrono::NaiveDate;
use serde::Deserialize;

use super::{Condition, Day, Location, Report, Weather};

pub const BASE_URL: &str = "https://api.open-meteo.com";

/// The request for the current weather and the daily forecast at
/// `location`, in °C (Open-Meteo's default). The days are the location's
/// own (`timezone=auto`), 7 of them from today.
pub fn url(base: &str, location: Location) -> String {
    format!(
        "{base}/v1/forecast?latitude={}&longitude={}\
         &current=temperature_2m,weather_code,is_day\
         &daily=weather_code,temperature_2m_max,temperature_2m_min&timezone=auto",
        location.latitude, location.longitude
    )
}

#[derive(Deserialize)]
struct Response {
    current: Current,
    /// Without it, the current weather still counts.
    daily: Option<Daily>,
}

#[derive(Deserialize)]
struct Current {
    temperature_2m: Option<f32>,
    weather_code: Option<u8>,
    is_day: Option<u8>,
}

/// One list per field, a value per day.
#[derive(Deserialize)]
struct Daily {
    time: Vec<String>,
    weather_code: Vec<Option<u8>>,
    temperature_2m_max: Vec<Option<f32>>,
    temperature_2m_min: Vec<Option<f32>>,
}

/// The weather in a response to `url`. Days without a high and low are
/// left out.
pub fn parse(body: &str) -> Result<Report, String> {
    let response: Response = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let current = response.current;
    let current = Weather {
        condition: current.weather_code.and_then(condition),
        // Without it, day: most icons are the same either way.
        is_day: current.is_day != Some(0),
        temperature: current.temperature_2m.ok_or("no temperature")?,
    };
    let mut days = Vec::new();
    let Some(daily) = response.daily else {
        return Ok(Report { current, days });
    };
    for (i, date) in daily.time.iter().enumerate() {
        let date = NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .map_err(|e| format!("date {date:?}: {e}"))?;
        let value = |list: &[Option<f32>]| list.get(i).copied().flatten();
        let (Some(high), Some(low)) = (
            value(&daily.temperature_2m_max),
            value(&daily.temperature_2m_min),
        ) else {
            continue;
        };
        days.push(Day {
            date,
            condition: daily
                .weather_code
                .get(i)
                .copied()
                .flatten()
                .and_then(condition),
            high,
            low,
        });
    }
    Ok(Report { current, days })
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
