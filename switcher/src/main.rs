//! Brings Now Playing or Kodi to the front in Steam Gaming Mode when they
//! start playing.

mod action;
mod app;
mod config;
mod gamescope;
mod listener;
mod steam;

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::mpsc;
use std::thread;

use clap::Parser;

use config::Config;

/// Brings Now Playing or Kodi to the front when they start playing.
#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// The config file [default: $XDG_CONFIG_HOME/now-playing/switcher.toml].
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,
    /// Log what each action would do, without doing it.
    #[arg(long)]
    dry_run: bool,
}

fn main() -> ExitCode {
    let args = Args::parse();
    let Some(path) = args.config.or_else(config::default_path) else {
        eprintln!("switcher: no --config given, and neither $XDG_CONFIG_HOME nor $HOME is set");
        return ExitCode::FAILURE;
    };
    let listeners = match Config::load(&path).and_then(|config| listener::from_config(&config)) {
        Ok(listeners) => listeners,
        Err(e) => {
            eprintln!("switcher: {e}");
            return ExitCode::FAILURE;
        }
    };

    let (show, requests) = mpsc::channel();
    for mut listener in listeners {
        let show = show.clone();
        thread::spawn(move || listener.run(show));
    }

    let display = gamescope::display();
    for app in requests {
        action::show(app, &display, args.dry_run);
    }
    unreachable!("the channel's sender is kept alive by `show`")
}
