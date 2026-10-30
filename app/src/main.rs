use std::process::ExitCode;

use clap::Parser;
#[cfg(debug_assertions)]
use now_playing::debug;
use now_playing::snapcast::{self, Inbox, Stream};
use now_playing::state::{State, now_playing};
use now_playing::ui;
use now_playing::weather::{self, Location, Units};
use now_playing_core::hostname;

const DEFAULT_SERVER: &str = "192.168.1.160:1705";

/// Full-screen "Now Playing" for the TV a Snapcast client plays on.
#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// snapserver's JSON-RPC address.
    #[arg(long, value_name = "HOST:PORT", default_value = DEFAULT_SERVER)]
    server: String,
    /// The Snapcast client to follow, by id or host name [default: this
    /// machine's hostname].
    #[arg(long, value_name = "ID")]
    client: Option<String>,
    /// The units temperatures are shown in.
    #[arg(long, value_enum, default_value_t)]
    units: Units,
    /// Where to show the weather for, in degrees north [default: found
    /// from this machine's public IP address].
    #[arg(long, requires = "longitude", allow_negative_numbers = true, value_parser = latitude)]
    latitude: Option<f64>,
    /// Where to show the weather for, in degrees east.
    #[arg(long, requires = "latitude", allow_negative_numbers = true, value_parser = longitude)]
    longitude: Option<f64>,
    /// Print state changes instead of opening the window.
    #[cfg(debug_assertions)]
    #[arg(long)]
    print: bool,
    /// Replay a log (see `core/tests/fixtures/`) instead of connecting.
    #[cfg(debug_assertions)]
    #[arg(long, value_name = "LOG", value_parser = read_log)]
    fake: Option<String>,
    /// Show this weather instead of fetching it, with a made-up forecast: a
    /// condition, the temperature in `--units`, and `night` for the night
    /// icons, e.g. `partly-cloudy,57,night`.
    #[cfg(debug_assertions)]
    #[arg(long, value_name = "CONDITION,TEMP[,night]", value_parser = debug::parse_fake_weather)]
    fake_weather: Option<debug::FakeWeather>,
}

fn main() -> ExitCode {
    let mut args = Args::parse();
    let client = match args.client.take().map_or_else(hostname, Ok) {
        Ok(client) => client,
        Err(e) => {
            eprintln!("now-playing: no --client given and no hostname: {e}");
            return ExitCode::FAILURE;
        }
    };
    let (controller, inbox) = snapcast::channel();
    #[cfg(debug_assertions)]
    if args.print {
        follow(&args, &client, inbox, debug::printer());
        return ExitCode::SUCCESS;
    }

    // Covers whose host doesn't resolve are fetched from the server's host.
    let art_host = match args.server.rsplit_once(':') {
        Some((host, _)) => host.to_owned(),
        None => args.server.clone(),
    };
    let location = args
        .latitude
        .zip(args.longitude)
        .map(|(latitude, longitude)| Location {
            latitude,
            longitude,
        });
    #[cfg(debug_assertions)]
    let fake_weather = args
        .fake_weather
        .as_ref()
        .map(|fake| fake.report(args.units));
    let follow_weather = move |mut updates: ui::WeatherUpdates| {
        #[cfg(debug_assertions)]
        if let Some(fake) = fake_weather {
            return updates.send(Some(fake));
        }
        weather::follow(location, |weather| updates.send(weather));
    };
    let units = args.units;
    let result = ui::run(
        &art_host,
        controller,
        units,
        follow_weather,
        move |mut updates| {
            follow(&args, &client, inbox, |stream| {
                updates.send(stream.map_or(State::Idle, now_playing));
            });
        },
    );
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("now-playing: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Feeds our client's stream to `on_stream`, from the server, and sends it
/// the commands that arrive in `inbox`. Returns only at the end of a
/// `--fake` log.
fn follow(args: &Args, client: &str, inbox: Inbox, on_stream: impl FnMut(Option<&Stream>)) {
    #[cfg(debug_assertions)]
    if let Some(log) = &args.fake {
        eprintln!("now-playing: replaying a log for client {client}");
        return debug::replay(log, client, inbox, on_stream);
    }
    eprintln!("now-playing: following client {client} on {}", args.server);
    snapcast::run_with_controller(&args.server, client, inbox, on_stream)
}

/// The contents of the log at `path`, for `--fake`.
#[cfg(debug_assertions)]
fn read_log(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| e.to_string())
}

/// A latitude, for `--latitude`.
fn latitude(arg: &str) -> Result<f64, String> {
    degrees(arg, 90.0)
}

/// A longitude, for `--longitude`.
fn longitude(arg: &str) -> Result<f64, String> {
    degrees(arg, 180.0)
}

fn degrees(arg: &str, max: f64) -> Result<f64, String> {
    let degrees: f64 = arg.parse().map_err(|e| format!("{e}"))?;
    if !(-max..=max).contains(&degrees) {
        return Err(format!("not between -{max} and {max}"));
    }
    Ok(degrees)
}
