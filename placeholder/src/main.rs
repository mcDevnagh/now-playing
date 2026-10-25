//! A placeholder window, for an app that opens no window until it's used,
//! such as UxPlay until a device casts to it. Started beside the app, it
//! gives Steam a window to show for it instead of its loading screen.

mod window;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

/// A full-screen black window, with an image centred at its own size, that
/// ignores input. Quits on SIGTERM.
#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// An image (PNG or JPEG) to draw centred, unscaled.
    #[arg(long, value_name = "PATH")]
    image: Option<PathBuf>,
    /// The window's title.
    #[arg(long, default_value = "AirPlay")]
    title: String,
}

fn main() -> ExitCode {
    let args = Args::parse();
    match window::run(&args.title, args.image.as_deref()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("now-playing-placeholder: {e}");
            ExitCode::FAILURE
        }
    }
}
