//! Keeping the current weather up to date, on its own thread.

use std::thread;
use std::time::{Duration, Instant};

use ureq::Agent;

use super::{Location, Report, locate, open_meteo};

const TIMEOUT: Duration = Duration::from_secs(20);
const MAX_BYTES: u64 = 1024 * 1024;
/// How often Open-Meteo's current weather changes: it reports an
/// `interval` of 900 s.
pub const EVERY: Duration = Duration::from_secs(15 * 60);
/// Weather this old is no longer shown.
pub const STALE: Duration = Duration::from_secs(60 * 60);
/// The first wait after a failure, doubling up to `EVERY`.
const FIRST_RETRY: Duration = Duration::from_secs(30);

/// Where weather and locations come from: the real services, or test
/// servers.
pub struct Source {
    agent: Agent,
    weather_base: String,
    locate_url: String,
}

impl Source {
    pub fn new() -> Source {
        Source::with_urls(open_meteo::BASE_URL, locate::URL)
    }

    pub fn with_urls(weather_base: &str, locate_url: &str) -> Source {
        let agent = Agent::config_builder()
            .timeout_global(Some(TIMEOUT))
            .user_agent(concat!("now-playing/", env!("CARGO_PKG_VERSION")))
            .build()
            .into();
        Source {
            agent,
            weather_base: weather_base.to_owned(),
            locate_url: locate_url.to_owned(),
        }
    }

    /// The weather now and the forecast at `location`.
    pub fn report(&self, location: Location) -> Result<Report, String> {
        let url = open_meteo::url(&self.weather_base, location);
        open_meteo::parse(&self.get(&url)?).map_err(|e| format!("{url}: {e}"))
    }

    pub fn locate(&self) -> Result<locate::Place, String> {
        locate::parse(&self.get(&self.locate_url)?).map_err(|e| format!("{}: {e}", self.locate_url))
    }

    fn get(&self, url: &str) -> Result<String, String> {
        self.agent
            .get(url)
            .call()
            .and_then(|mut response| {
                response
                    .body_mut()
                    .with_config()
                    .limit(MAX_BYTES)
                    .read_to_string()
            })
            .map_err(|e| format!("{url}: {e}"))
    }
}

impl Default for Source {
    fn default() -> Source {
        Source::new()
    }
}

/// When to fetch next, and when what's shown has gone stale.
#[derive(Debug)]
pub struct Schedule {
    /// When the weather last came, if it's still shown.
    shown_since: Option<Instant>,
    retry: Duration,
}

impl Schedule {
    pub fn new() -> Schedule {
        Schedule {
            shown_since: None,
            retry: FIRST_RETRY,
        }
    }

    /// Weather came at `now`: the wait until the next fetch.
    pub fn fetched(&mut self, now: Instant) -> Duration {
        self.shown_since = Some(now);
        self.retry = FIRST_RETRY;
        EVERY
    }

    /// A fetch failed at `now`: the wait until the next try, and whether
    /// the weather shown is now too old to keep.
    pub fn failed(&mut self, now: Instant) -> (Duration, bool) {
        let stale = self
            .shown_since
            .is_some_and(|since| now.duration_since(since) >= STALE);
        if stale {
            self.shown_since = None;
        }
        let retry = self.retry;
        self.retry = (retry * 2).min(EVERY);
        (retry, stale)
    }
}

impl Default for Schedule {
    fn default() -> Schedule {
        Schedule::new()
    }
}

/// Feeds the weather at `location` to `on_weather` every `EVERY`, and
/// None once it's `STALE`. Without a location, finds one from the IP
/// address first. Never returns.
pub fn follow(location: Option<Location>, mut on_weather: impl FnMut(Option<Report>)) {
    let source = Source::new();
    let location = location.unwrap_or_else(|| find(&source));
    let mut schedule = Schedule::new();
    loop {
        let wait = match source.report(location) {
            Ok(report) => {
                on_weather(Some(report));
                schedule.fetched(Instant::now())
            }
            Err(e) => {
                eprintln!("now-playing: no weather: {e}");
                let (wait, stale) = schedule.failed(Instant::now());
                if stale {
                    on_weather(None);
                }
                wait
            }
        };
        thread::sleep(wait);
    }
}

/// The location from the IP address, trying until it's found.
fn find(source: &Source) -> Location {
    let mut retry = FIRST_RETRY;
    loop {
        match source.locate() {
            Ok(place) => {
                let Location {
                    latitude,
                    longitude,
                } = place.location;
                let city = place.city.map(|c| format!("{c}, ")).unwrap_or_default();
                eprintln!(
                    "now-playing: weather for {city}{latitude}, {longitude}, from the IP \
                     address; pass --latitude and --longitude to change it"
                );
                return place.location;
            }
            Err(e) => eprintln!("now-playing: can't find where this is for the weather: {e}"),
        }
        thread::sleep(retry);
        retry = (retry * 2).min(EVERY);
    }
}
