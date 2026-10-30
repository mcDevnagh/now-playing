use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use super::*;

/// A real response, captured on 2026-10-30.
const OPEN_METEO: &str = r#"{"latitude":41.364597,"longitude":-75.83869,"generationtime_ms":0.21827220916748047,"utc_offset_seconds":0,"timezone":"GMT","timezone_abbreviation":"GMT","elevation":397.0,"current_units":{"time":"iso8601","interval":"seconds","temperature_2m":"°C","weather_code":"wmo code","is_day":""},"current":{"time":"2026-10-30T19:00","interval":900,"temperature_2m":12.6,"weather_code":2,"is_day":0}}"#;

/// IP.SB's response, in its real shape, with a made-up address and place.
const IP_SB: &str = r#"{"organization":"Example Broadband","longitude":-1.2577,"city":"Oxford","timezone":"Europe\/London","isp":"Example Broadband","offset":0,"region":"England","asn":64500,"asn_organization":"Example Broadband","country":"United Kingdom","ip":"203.0.113.7","latitude":51.752,"continent_code":"EU","country_code":"GB","region_code":"ENG"}"#;

const HERE: Location = Location {
    latitude: 51.752,
    longitude: -1.2577,
};

#[test]
fn parses_open_meteo() {
    let weather = open_meteo::parse(OPEN_METEO).unwrap();
    assert_eq!(
        weather,
        Weather {
            condition: Some(Condition::PartlyCloudy),
            is_day: false,
            temperature: 12.6,
        }
    );
}

#[test]
fn open_meteo_without_a_temperature_is_an_error() {
    let body = r#"{"current":{"temperature_2m":null,"weather_code":0,"is_day":1}}"#;
    assert!(open_meteo::parse(body).is_err());
    assert!(open_meteo::parse("").is_err());
    // Without coordinates Open-Meteo answers with an empty body.
    assert!(open_meteo::parse("{}").is_err());
}

#[test]
fn an_unknown_code_is_just_the_temperature() {
    let body = r#"{"current":{"temperature_2m":-4.0,"weather_code":200,"is_day":1}}"#;
    let weather = open_meteo::parse(body).unwrap();
    assert_eq!(weather.condition, None);
    assert!(weather.is_day);
    assert_eq!(weather.temperature, -4.0);
}

#[test]
fn maps_wmo_codes_as_breezy_weather_does() {
    use Condition::*;

    let expected: &[(&[u8], Condition)] = &[
        (&[0, 1], Clear),
        (&[2], PartlyCloudy),
        (&[3], Cloudy),
        (&[45, 48], Fog),
        (&[51, 53, 55, 56, 57, 61, 63, 65, 66, 67, 80, 81, 82], Rain),
        (&[71, 73, 75, 85, 86], Snow),
        (&[77], Sleet),
        (&[95, 96, 99], Thunderstorm),
    ];
    let mut known = 0;
    for code in 0..=u8::MAX {
        let want = expected
            .iter()
            .find(|(codes, _)| codes.contains(&code))
            .map(|&(_, condition)| condition);
        assert_eq!(open_meteo::condition(code), want, "code {code}");
        known += want.is_some() as usize;
    }
    assert_eq!(known, 28);
}

#[test]
fn asks_open_meteo_for_the_current_weather() {
    assert_eq!(
        open_meteo::url("https://api.open-meteo.com", HERE),
        "https://api.open-meteo.com/v1/forecast?latitude=51.752&longitude=-1.2577\
         &current=temperature_2m,weather_code,is_day"
    );
}

#[test]
fn parses_ip_sb() {
    let place = locate::parse(IP_SB).unwrap();
    assert_eq!(place.location, HERE);
    assert_eq!(place.city.as_deref(), Some("Oxford"));
}

#[test]
fn ip_sb_without_a_location_is_an_error() {
    assert!(locate::parse(r#"{"latitude":0,"longitude":0,"city":""}"#).is_err());
    assert!(locate::parse(r#"{"ip":"203.0.113.7"}"#).is_err());
    let place = locate::parse(r#"{"latitude":10.5,"longitude":-66.9,"city":""}"#).unwrap();
    assert_eq!(place.city, None);
}

#[test]
fn fetches_every_15_minutes_and_retries_sooner() {
    let start = Instant::now();
    let mut schedule = Schedule::new();
    // Failures before any weather: nothing to go stale, retries doubling.
    assert_eq!(schedule.failed(start), (Duration::from_secs(30), false));
    assert_eq!(schedule.failed(start), (Duration::from_secs(60), false));
    assert_eq!(schedule.fetched(start), EVERY);
    // A success resets the retries.
    assert_eq!(schedule.failed(start).0, Duration::from_secs(30));
    for _ in 0..10 {
        assert!(schedule.failed(start).0 <= EVERY);
    }
    assert_eq!(schedule.failed(start).0, EVERY);
}

#[test]
fn weather_goes_stale_after_an_hour_of_failures() {
    let start = Instant::now();
    let mut schedule = Schedule::new();
    schedule.fetched(start);
    assert!(!schedule.failed(start + EVERY).1);
    assert!(!schedule.failed(start + STALE - Duration::from_secs(1)).1);
    assert!(schedule.failed(start + STALE).1);
    // Only once: it's gone.
    assert!(!schedule.failed(start + STALE + EVERY).1);
    // Back once weather comes again.
    schedule.fetched(start + 2 * STALE);
    assert!(schedule.failed(start + 3 * STALE).1);
}

/// Serves `body` as JSON to one request, and sends back the request line.
fn serve_once(body: &'static str) -> (u16, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut request = String::new();
        reader.read_line(&mut request).unwrap();
        let mut line = String::new();
        while reader.read_line(&mut line).unwrap() > 2 {
            line.clear();
        }
        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(header.as_bytes()).unwrap();
        stream.write_all(body.as_bytes()).unwrap();
        tx.send(request.trim_end().to_owned()).unwrap();
    });
    (port, rx)
}

#[test]
fn fetches_the_weather() {
    let (port, request) = serve_once(OPEN_METEO);
    let source = Source::with_urls(&format!("http://127.0.0.1:{port}"), "unused");
    let weather = source.current(HERE).unwrap();
    assert_eq!(weather.condition, Some(Condition::PartlyCloudy));
    assert_eq!(
        request.recv().unwrap(),
        "GET /v1/forecast?latitude=51.752&longitude=-1.2577\
         &current=temperature_2m,weather_code,is_day HTTP/1.1"
    );
}

#[test]
fn finds_the_location() {
    let (port, request) = serve_once(IP_SB);
    let source = Source::with_urls("unused", &format!("http://127.0.0.1:{port}/geoip"));
    assert_eq!(source.locate().unwrap().location, HERE);
    assert_eq!(request.recv().unwrap(), "GET /geoip HTTP/1.1");
}

#[test]
fn a_failed_fetch_says_where() {
    // A port that was free a moment ago: nothing listens there now.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let source = Source::with_urls(&format!("http://127.0.0.1:{port}"), "unused");
    let error = source.current(HERE).unwrap_err();
    assert!(error.contains(&format!("127.0.0.1:{port}")), "{error}");
}
