use std::process::ExitCode;

use now_playing::snapcast::{self, Stream};
use now_playing::state::{State, now_playing};
use now_playing::ui;

const DEFAULT_SERVER: &str = "192.168.1.160:1705";
const USAGE: &str = "usage: now-playing [--server HOST:PORT] [--client ID] [--fake LOG] [--print]";

struct Args {
    server: String,
    /// `None`: this machine's hostname.
    client: Option<String>,
    /// A log to replay instead of connecting (see `tests/fixtures/`).
    fake: Option<String>,
    /// Print state changes instead of opening the window.
    print: bool,
}

/// Where states come from.
enum Source {
    Server(String),
    /// The contents of a log to replay.
    Fake(String),
}

impl Source {
    /// Feeds our client's stream to `on_stream`. Returns only at the end of
    /// a fake log.
    fn follow(&self, client: &str, on_stream: impl FnMut(Option<&Stream>)) {
        match self {
            Source::Server(addr) => snapcast::run(addr, client, on_stream),
            Source::Fake(log) => snapcast::fake(log, client, on_stream),
        }
    }
}

fn main() -> ExitCode {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(e) => {
            eprintln!("now-playing: {e}\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    let client = match args.client.map_or_else(hostname, Ok) {
        Ok(client) => client,
        Err(e) => {
            eprintln!("now-playing: no --client given and no hostname: {e}");
            return ExitCode::FAILURE;
        }
    };
    let source = match args.fake {
        Some(path) => match std::fs::read_to_string(&path) {
            Ok(log) => Source::Fake(log),
            Err(e) => {
                eprintln!("now-playing: {path}: {e}");
                return ExitCode::FAILURE;
            }
        },
        None => Source::Server(args.server),
    };

    eprintln!("now-playing: following client {client}");
    if args.print {
        let mut shown = None;
        source.follow(&client, |stream| {
            let state = stream.map_or(State::Idle, now_playing);
            if shown.as_ref() != Some(&state) {
                println!("{}", describe(&state));
                shown = Some(state);
            }
        });
        return ExitCode::SUCCESS;
    }

    let result = ui::run(move |mut updates| {
        source.follow(&client, |stream| {
            updates.send(stream.map_or(State::Idle, now_playing));
        });
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("now-playing: {e}");
            ExitCode::FAILURE
        }
    }
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut parsed = Args {
        server: DEFAULT_SERVER.to_owned(),
        client: None,
        fake: None,
        print: false,
    };
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--server" => parsed.server = value()?,
            "--client" => parsed.client = Some(value()?),
            "--fake" => parsed.fake = Some(value()?),
            "--print" => parsed.print = true,
            _ => return Err(format!("unknown argument {arg}")),
        }
    }
    Ok(parsed)
}

fn hostname() -> Result<String, String> {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|name| name.trim().to_owned())
        .map_err(|e| e.to_string())
}

/// One line for `--print`.
fn describe(state: &State) -> String {
    let (status, track) = match state {
        State::Idle => return "idle".to_owned(),
        State::Playing(track) => ("playing", track),
        State::Paused(track) => ("paused", track),
    };
    let mut line = format!("{status}: {}", track.title);
    if !track.artists.is_empty() {
        line += &format!(" | {}", track.artists.join(", "));
    }
    if let Some(album) = &track.album {
        line += &format!(" | {album}");
    }
    if let (Some(position), Some(duration)) = (track.position, track.duration) {
        line += &format!(" | {}/{}", clock(position), clock(duration));
    }
    if let Some(art) = &track.art_url {
        line += &format!(" | {art}");
    }
    line
}

fn clock(seconds: f64) -> String {
    let seconds = seconds as u64;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}
