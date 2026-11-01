//! Quiets the other players when one starts, so two never play over each
//! other: Kodi, Snapcast, and UxPlay mirroring a device's screen. Runs
//! beside Kodi, started by `htpc/kodi-steam`.

mod action;
mod config;
mod listener;
mod quiet;
mod windows;

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::mpsc;
use std::thread;

use clap::Parser;

use config::Config;

/// Quiets the other players when one starts.
#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// The config file [default: $XDG_CONFIG_HOME/now-playing/switcher.toml].
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,
    /// Log what it would do, without pausing or muting.
    #[arg(long)]
    dry_run: bool,
}

fn main() -> ExitCode {
    let args = Args::parse();
    let Some(path) = args.config.or_else(config::default_path) else {
        eprintln!("switcher: no --config given, and neither $XDG_CONFIG_HOME nor $HOME is set");
        return ExitCode::FAILURE;
    };
    let display = std::env::var("DISPLAY")
        .ok()
        .filter(|display| !display.is_empty());
    let loaded = Config::load(&path).and_then(|config| listener::from_config(&config, display));
    let (listeners, dispatch) = match loaded {
        Ok(loaded) => loaded,
        Err(e) => {
            eprintln!("switcher: {e}");
            return ExitCode::FAILURE;
        }
    };

    let (tx, requests) = mpsc::channel();
    for listener in listeners {
        let tx = tx.clone();
        thread::spawn(move || listener.run(tx));
    }
    // Keeps the channel open with no listeners, as with a config that
    // follows nothing.
    let _tx = tx;
    dispatch.run(requests, args.dry_run)
}
