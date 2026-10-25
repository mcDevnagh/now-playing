//! Brings Now Playing or Kodi to the front in Steam Gaming Mode when they
//! start playing.

mod action;
mod app;
mod config;
mod gamescope;
mod listener;
mod startup;
mod steam;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by the UxPlay listener, still to come")
)]
mod windows;

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
    let loaded = Config::load(&path)
        .and_then(|config| Ok((listener::from_config(&config)?, config.launch_at_start())));
    let (listeners, at_start) = match loaded {
        Ok(loaded) => loaded,
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
    if !at_start.is_empty() {
        let display = display.clone();
        let show = show.clone();
        thread::spawn(move || startup::run(at_start, &display, show));
    }
    for request in requests {
        action::handle(request, &display, args.dry_run);
    }
    unreachable!("the channel's sender is kept alive by `show`")
}
