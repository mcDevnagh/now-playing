use std::process::ExitCode;

#[cfg(debug_assertions)]
use now_playing::debug;
use now_playing::snapcast::{self, Stream};
use now_playing::state::{State, now_playing};
use now_playing::ui;

const DEFAULT_SERVER: &str = "192.168.1.160:1705";
#[cfg(not(debug_assertions))]
const USAGE: &str = "usage: now-playing [--server HOST:PORT] [--client ID]";
#[cfg(debug_assertions)]
const USAGE: &str = "usage: now-playing [--server HOST:PORT] [--client ID] [--print] [--fake LOG]";

struct Args {
    server: String,
    /// `None`: this machine's hostname.
    client: Option<String>,
    /// Debug builds only: print state changes instead of opening the window.
    #[cfg(debug_assertions)]
    print: bool,
    /// Debug builds only: the contents of a log to replay instead of
    /// connecting (see `tests/fixtures/`).
    #[cfg(debug_assertions)]
    fake: Option<String>,
}

fn main() -> ExitCode {
    let mut args = match parse_args(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(e) => {
            eprintln!("now-playing: {e}\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };
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

    let result = ui::run(move |mut updates| {
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

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut parsed = Args {
        server: DEFAULT_SERVER.to_owned(),
        client: None,
        #[cfg(debug_assertions)]
        print: false,
        #[cfg(debug_assertions)]
        fake: None,
    };
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--server" => parsed.server = value()?,
            "--client" => parsed.client = Some(value()?),
            #[cfg(debug_assertions)]
            "--print" => parsed.print = true,
            #[cfg(debug_assertions)]
            "--fake" => {
                let path = value()?;
                let log = std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
                parsed.fake = Some(log);
            }
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
