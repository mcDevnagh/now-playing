//! Brings Now Playing or Kodi to the front in Steam Gaming Mode when they
//! start playing.

mod action;
mod config;
mod listener;
mod registry;
mod rules;

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::mpsc;

use clap::Parser;
use now_playing_core::hostname;

use config::Config;
use listener::Event;
use registry::{Listener, Registry, Trigger};
use rules::MusicEdges;

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
    let config = match Config::load(&path) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("switcher: {e}");
            return ExitCode::FAILURE;
        }
    };
    let registry = Registry::new(&config);
    if !args.dry_run {
        eprintln!("switcher: actions aren't implemented yet; running as --dry-run");
    }

    let (events, received) = mpsc::channel();
    for listener in &registry.listeners {
        match listener {
            Listener::Snapcast { server, client } => {
                let client = match client.clone().map_or_else(hostname, Ok) {
                    Ok(client) => client,
                    Err(e) => {
                        eprintln!("switcher: no snapcast.client given and no hostname: {e}");
                        return ExitCode::FAILURE;
                    }
                };
                listener::snapcast::spawn(server.clone(), client, events.clone());
            }
            Listener::Kodi { address } => {
                eprintln!("switcher: Kodi ({address}) isn't supported yet; ignoring [kodi]");
            }
        }
    }

    let mut music = MusicEdges::default();
    for event in received {
        let trigger = match event {
            Event::Music(playing) if music.update(playing) => Trigger::MusicStarted,
            Event::Music(_) => continue,
        };
        eprintln!("switcher: {trigger}");
        for &action in registry.actions(trigger) {
            action::run(action);
        }
    }
    unreachable!("the channel's sender is kept alive by `events`")
}
