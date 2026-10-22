use std::process::ExitCode;

use clap::Parser;
#[cfg(debug_assertions)]
use now_playing::debug;
use now_playing::snapcast::{self, Stream};
use now_playing::state::{State, now_playing};
use now_playing::ui;

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
    /// Print state changes instead of opening the window.
    #[cfg(debug_assertions)]
    #[arg(long)]
    print: bool,
    /// Replay a log (see `tests/fixtures/`) instead of connecting.
    #[cfg(debug_assertions)]
    #[arg(long, value_name = "LOG", value_parser = read_log)]
    fake: Option<String>,
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
    #[cfg(debug_assertions)]
    if args.print {
        follow(&args, &client, debug::printer());
        return ExitCode::SUCCESS;
    }

    // Covers whose host doesn't resolve are fetched from the server's host.
    let art_host = match args.server.rsplit_once(':') {
        Some((host, _)) => host.to_owned(),
        None => args.server.clone(),
    };
    let result = ui::run(&art_host, move |mut updates| {
        follow(&args, &client, |stream| {
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

/// Feeds our client's stream to `on_stream`, from the server. Returns only
/// at the end of a `--fake` log.
fn follow(args: &Args, client: &str, on_stream: impl FnMut(Option<&Stream>)) {
    #[cfg(debug_assertions)]
    if let Some(log) = &args.fake {
        eprintln!("now-playing: replaying a log for client {client}");
        return debug::replay(log, client, on_stream);
    }
    eprintln!("now-playing: following client {client} on {}", args.server);
    snapcast::run(&args.server, client, on_stream)
}

/// The contents of the log at `path`, for `--fake`.
#[cfg(debug_assertions)]
fn read_log(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| e.to_string())
}

fn hostname() -> Result<String, String> {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|name| name.trim().to_owned())
        .map_err(|e| e.to_string())
}
