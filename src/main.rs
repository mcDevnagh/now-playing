use std::process::ExitCode;

use now_playing::snapcast;
use now_playing::state::{State, now_playing};

const DEFAULT_SERVER: &str = "192.168.1.160:1705";
const USAGE: &str = "usage: now-playing [--server HOST:PORT] [--client ID] --print";

struct Args {
    server: String,
    /// `None`: this machine's hostname.
    client: Option<String>,
    print: bool,
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
    if !args.print {
        eprintln!("now-playing: there is no window yet; use --print\n{USAGE}");
        return ExitCode::FAILURE;
    }

    eprintln!("now-playing: following client {client} on {}", args.server);
    let mut shown = None;
    snapcast::run(&args.server, &client, |stream| {
        let state = stream.map_or(State::Idle, now_playing);
        if shown.as_ref() != Some(&state) {
            println!("{}", describe(&state));
            shown = Some(state);
        }
    })
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut parsed = Args {
        server: DEFAULT_SERVER.to_owned(),
        client: None,
        print: false,
    };
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--server" => parsed.server = value()?,
            "--client" => parsed.client = Some(value()?),
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
