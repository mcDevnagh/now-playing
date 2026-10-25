//! The TOML config: which listeners to start, and the app each one shows.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

use crate::app::App;

const DEFAULT_KODI_ADDRESS: &str = "127.0.0.1:9090";
/// The `WM_CLASS` class of GStreamer's video window, which UxPlay opens.
const DEFAULT_UXPLAY_WINDOW_CLASS: &str = "GStreamer";
/// How long an app launched at start has to come to the front, in seconds.
const DEFAULT_LAUNCH_AT_START_WAIT: u64 = 30;

#[derive(Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub kodi: Option<Kodi>,
    pub snapcast: Option<Snapcast>,
    pub uxplay: Option<Uxplay>,
}

/// Kodi playback → show Kodi.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(try_from = "RawKodi")]
pub struct Kodi {
    pub app: App,
    /// Kodi's JSON-RPC TCP address.
    pub address: String,
    /// Launch the app each time Gaming Mode starts, then go home once it's in
    /// front, or after this long.
    pub launch_at_start: Option<Duration>,
}

/// Our Snapcast client starting to play → show an app.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(try_from = "RawSnapcast")]
pub struct Snapcast {
    pub app: App,
    /// snapserver's JSON-RPC address.
    pub server: String,
    /// The client to follow [default: this machine's hostname].
    pub client: Option<String>,
    /// Launch the app each time Gaming Mode starts, then go home once it's in
    /// front, or after this long.
    pub launch_at_start: Option<Duration>,
}

/// UxPlay's video window opening, as a device starts mirroring → show
/// UxPlay.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(try_from = "RawUxplay")]
pub struct Uxplay {
    pub app: App,
    /// The `WM_CLASS` class of UxPlay's video window.
    pub window_class: String,
    /// Launch the app each time Gaming Mode starts, then go home once it's in
    /// front, or after this long.
    pub launch_at_start: Option<Duration>,
}

/// A `[kodi]` table as written, checked on the way to [`Kodi`].
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawKodi {
    app: Option<u32>,
    launch: Option<u64>,
    address: Option<String>,
    #[serde(default)]
    launch_at_start: bool,
    launch_at_start_wait: Option<u64>,
}

/// A `[snapcast]` table as written, checked on the way to [`Snapcast`].
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSnapcast {
    app: Option<u32>,
    launch: Option<u64>,
    server: String,
    client: Option<String>,
    #[serde(default)]
    launch_at_start: bool,
    launch_at_start_wait: Option<u64>,
}

/// A `[uxplay]` table as written, checked on the way to [`Uxplay`].
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawUxplay {
    app: Option<u32>,
    launch: Option<u64>,
    window_class: Option<String>,
    #[serde(default)]
    launch_at_start: bool,
    launch_at_start_wait: Option<u64>,
}

impl TryFrom<RawKodi> for Kodi {
    type Error = String;

    fn try_from(raw: RawKodi) -> Result<Self, String> {
        let app = App::new(raw.app, raw.launch).map_err(|e| format!("[kodi] {e}"))?;
        Ok(Self {
            app,
            address: raw
                .address
                .unwrap_or_else(|| DEFAULT_KODI_ADDRESS.to_owned()),
            launch_at_start: launch_at_start(
                "kodi",
                app,
                raw.launch_at_start,
                raw.launch_at_start_wait,
            )?,
        })
    }
}

impl TryFrom<RawSnapcast> for Snapcast {
    type Error = String;

    fn try_from(raw: RawSnapcast) -> Result<Self, String> {
        if raw.server.is_empty() {
            return Err("[snapcast] `server` is empty".to_owned());
        }
        let app = App::new(raw.app, raw.launch).map_err(|e| format!("[snapcast] {e}"))?;
        Ok(Self {
            app,
            server: raw.server,
            client: raw.client,
            launch_at_start: launch_at_start(
                "snapcast",
                app,
                raw.launch_at_start,
                raw.launch_at_start_wait,
            )?,
        })
    }
}

impl TryFrom<RawUxplay> for Uxplay {
    type Error = String;

    fn try_from(raw: RawUxplay) -> Result<Self, String> {
        if raw.window_class.as_deref() == Some("") {
            return Err("[uxplay] `window_class` is empty".to_owned());
        }
        let app = App::new(raw.app, raw.launch).map_err(|e| format!("[uxplay] {e}"))?;
        Ok(Self {
            app,
            window_class: raw
                .window_class
                .unwrap_or_else(|| DEFAULT_UXPLAY_WINDOW_CLASS.to_owned()),
            launch_at_start: launch_at_start(
                "uxplay",
                app,
                raw.launch_at_start,
                raw.launch_at_start_wait,
            )?,
        })
    }
}

/// A table's `launch_at_start` and `launch_at_start_wait`, checked:
/// launching needs a launch ID, and the wait is only for launching.
fn launch_at_start(
    table: &str,
    app: App,
    launch_at_start: bool,
    wait: Option<u64>,
) -> Result<Option<Duration>, String> {
    match (launch_at_start, wait) {
        (true, _) if app.launch.is_none() => {
            Err(format!("[{table}] `launch_at_start` needs `launch`"))
        }
        (true, wait) => Ok(Some(Duration::from_secs(
            wait.unwrap_or(DEFAULT_LAUNCH_AT_START_WAIT),
        ))),
        (false, Some(_)) => Err(format!(
            "[{table}] `launch_at_start_wait` needs `launch_at_start = true`"
        )),
        (false, None) => Ok(None),
    }
}

impl Config {
    /// Reads and checks the config at `path`. Errors name the file.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Parses and checks a config file's contents.
    pub fn parse(text: &str) -> Result<Self, String> {
        let config: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        if config.kodi.is_none() && config.snapcast.is_none() && config.uxplay.is_none() {
            return Err("needs a [kodi], [snapcast] or [uxplay] table".to_owned());
        }
        Ok(config)
    }

    /// The apps to launch once Steam's UI is up, each with how long it has
    /// to come to the front: those whose tables set `launch_at_start`.
    pub fn launch_at_start(&self) -> Vec<(App, Duration)> {
        let kodi = self.kodi.as_ref().map(|t| (t.app, t.launch_at_start));
        let snapcast = self.snapcast.as_ref().map(|t| (t.app, t.launch_at_start));
        let uxplay = self.uxplay.as_ref().map(|t| (t.app, t.launch_at_start));
        kodi.into_iter()
            .chain(snapcast)
            .chain(uxplay)
            .filter_map(|(app, wait)| Some((app, wait?)))
            .collect()
    }
}

/// `$XDG_CONFIG_HOME/now-playing/switcher.toml`, falling back to
/// `~/.config/now-playing/switcher.toml`. `None` without either variable.
pub fn default_path() -> Option<PathBuf> {
    default_path_from(
        std::env::var_os("XDG_CONFIG_HOME"),
        std::env::var_os("HOME"),
    )
}

fn default_path_from(xdg_config_home: Option<OsString>, home: Option<OsString>) -> Option<PathBuf> {
    // The XDG spec says to ignore an empty or relative value.
    let config = xdg_config_home
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
        .or_else(|| home.map(|home| PathBuf::from(home).join(".config")))?;
    Some(config.join("now-playing").join("switcher.toml"))
}

#[cfg(test)]
mod tests;
