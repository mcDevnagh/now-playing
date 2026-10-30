//! Made-up weather, to see the weather and forecast without fetching them.

use chrono::{Days, Local};
use clap::ValueEnum;

use crate::weather::{Condition, Day, Report, Units, Weather};

/// Weather given with `--fake-weather`.
#[derive(Clone)]
pub struct FakeWeather {
    condition: Condition,
    /// In `--units`.
    temperature: f32,
    night: bool,
}

impl FakeWeather {
    /// The weather now as given, and five days of made-up forecast from
    /// today, through several conditions, around the given temperature.
    pub fn report(&self, units: Units) -> Report {
        let temperature = units.to_celsius(self.temperature);
        let today = Local::now().date_naive();
        let forecast = [
            (self.condition, 2.0),
            (Condition::PartlyCloudy, 3.5),
            (Condition::Rain, -1.0),
            (Condition::Cloudy, 0.5),
            (Condition::Snow, -6.0),
        ];
        let days = forecast
            .into_iter()
            .zip(0..)
            .map(|((condition, warmer), i)| Day {
                date: today + Days::new(i),
                condition: Some(condition),
                high: temperature + warmer,
                low: temperature + warmer - 8.0,
            })
            .collect();
        Report {
            current: Weather {
                condition: Some(self.condition),
                is_day: !self.night,
                temperature,
            },
            days,
        }
    }
}

/// `CONDITION,TEMP[,night]`, for `--fake-weather`.
pub fn parse_fake_weather(arg: &str) -> Result<FakeWeather, String> {
    let mut parts = arg.split(',');
    let condition = parts.next().unwrap_or_default();
    let condition = Condition::from_str(condition, true).map_err(|_| {
        let names: Vec<_> = Condition::value_variants()
            .iter()
            .filter_map(|c| c.to_possible_value())
            .map(|v| v.get_name().to_owned())
            .collect();
        format!(
            "unknown condition {condition:?}; one of: {}",
            names.join(", ")
        )
    })?;
    let temperature = parts
        .next()
        .ok_or("no temperature")?
        .parse()
        .map_err(|e| format!("bad temperature: {e}"))?;
    let night = match parts.next() {
        None => false,
        Some("night") => true,
        Some(other) => return Err(format!("expected `night`, not {other:?}")),
    };
    if parts.next().is_some() {
        return Err("too many parts".to_owned());
    }
    Ok(FakeWeather {
        condition,
        temperature,
        night,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fake_weather() {
        let fake = parse_fake_weather("partly-cloudy,57,night").unwrap();
        let report = fake.report(Units::Fahrenheit);
        assert_eq!(report.days.len(), 5);
        assert_eq!(report.days[0].date, Local::now().date_naive());
        let weather = report.current;
        assert_eq!(weather.condition, Some(Condition::PartlyCloudy));
        assert!(!weather.is_day);
        assert!((weather.temperature - 13.89).abs() < 0.01);

        let fake = parse_fake_weather("Rain,-3").unwrap();
        let weather = fake.report(Units::Celsius).current;
        assert_eq!(weather.condition, Some(Condition::Rain));
        assert!(weather.is_day);
        assert_eq!(weather.temperature, -3.0);

        for bad in [
            "",
            "drizzle,10",
            "rain",
            "rain,warm",
            "rain,10,day",
            "rain,10,night,x",
        ] {
            assert!(parse_fake_weather(bad).is_err(), "{bad:?}");
        }
    }
}
