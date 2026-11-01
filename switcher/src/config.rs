//! The TOML config: which players to follow, and whether each one's starts
//! quiet the others.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde::de::IgnoredAny;

const DEFAULT_KODI_ADDRESS: &str = "127.0.0.1:9090";
/// The `WM_CLASS` class of GStreamer's video window, which UxPlay opens.
const DEFAULT_UXPLAY_WINDOW_CLASS: &str = "GStreamer";
/// Keys from when the switcher brought apps to the front, which it no
/// longer does. A config with any of them is refused, rather than run
/// without doing what it asks.
const REMOVED_KEYS: [&str; 5] = [
    "gamescope_id",
    "steam_game_id",
    "show_on_playback",
    "launch_at_start",
    "launch_at_start_wait",
];

#[derive(Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub kodi: Option<Kodi>,
    pub snapcast: Option<Snapcast>,
    pub uxplay: Option<Uxplay>,
}

/// Kodi playback.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(try_from = "RawKodi")]
pub struct Kodi {
    /// Quiet the other players when this one starts.
    pub quiet_others: bool,
    /// Kodi's JSON-RPC TCP address.
    pub address: String,
}

/// Our Snapcast client starting to play.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(try_from = "RawSnapcast")]
pub struct Snapcast {
    /// Quiet the other players when this one starts.
    pub quiet_others: bool,
    /// snapserver's JSON-RPC address.
    pub server: String,
    /// The client to follow [default: this machine's hostname].
    pub client: Option<String>,
}

/// UxPlay's video window opening, as a device starts mirroring.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(try_from = "RawUxplay")]
pub struct Uxplay {
    /// Quiet the other players when this one starts.
    pub quiet_others: bool,
    /// The `WM_CLASS` class of UxPlay's video window.
    pub window_class: String,
    /// Keep Kodi from going idle while a device mirrors.
    pub keep_kodi_awake: bool,
}

/// A `[kodi]` table as written, checked on the way to [`Kodi`].
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawKodi {
    #[serde(default)]
    quiet_others: bool,
    address: Option<String>,
}

/// A `[snapcast]` table as written, checked on the way to [`Snapcast`].
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSnapcast {
    #[serde(default)]
    quiet_others: bool,
    server: String,
    client: Option<String>,
}

/// A `[uxplay]` table as written, checked on the way to [`Uxplay`].
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawUxplay {
    #[serde(default)]
    quiet_others: bool,
    window_class: Option<String>,
    #[serde(default = "yes")]
    keep_kodi_awake: bool,
}

fn yes() -> bool {
    true
}

impl From<RawKodi> for Kodi {
    fn from(raw: RawKodi) -> Self {
        Self {
            quiet_others: raw.quiet_others,
            address: raw
                .address
                .unwrap_or_else(|| DEFAULT_KODI_ADDRESS.to_owned()),
        }
    }
}

impl TryFrom<RawSnapcast> for Snapcast {
    type Error = String;

    fn try_from(raw: RawSnapcast) -> Result<Self, String> {
        if raw.server.is_empty() {
            return Err("[snapcast] `server` is empty".to_owned());
        }
        Ok(Self {
            quiet_others: raw.quiet_others,
            server: raw.server,
            client: raw.client,
        })
    }
}

impl TryFrom<RawUxplay> for Uxplay {
    type Error = String;

    fn try_from(raw: RawUxplay) -> Result<Self, String> {
        if raw.window_class.as_deref() == Some("") {
            return Err("[uxplay] `window_class` is empty".to_owned());
        }
        Ok(Self {
            quiet_others: raw.quiet_others,
            window_class: raw
                .window_class
                .unwrap_or_else(|| DEFAULT_UXPLAY_WINDOW_CLASS.to_owned()),
            keep_kodi_awake: raw.keep_kodi_awake,
        })
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
        // Keys only first, so an old config's removed keys are all named,
        // then typed, for errors that point at the line. (Not as a
        // `toml::Table`: an old `steam_game_id` is too big for its i64.)
        if let Ok(keys) = toml::from_str::<BTreeMap<String, Keys>>(text) {
            check_removed(&keys)?;
        }
        let config: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        if config.kodi.is_none() && config.snapcast.is_none() && config.uxplay.is_none() {
            return Err("needs a [kodi], [snapcast] or [uxplay] table".to_owned());
        }
        Ok(config)
    }
}

/// A top-level value's keys, if it's a table.
#[derive(Deserialize)]
#[serde(untagged)]
enum Keys {
    Table(BTreeMap<String, IgnoredAny>),
    Other(IgnoredAny),
}

/// An error naming every [`REMOVED_KEYS`] key in the player tables, if
/// there are any, so an old config is fixed in one go.
fn check_removed(config: &BTreeMap<String, Keys>) -> Result<(), String> {
    let found: Vec<String> = ["kodi", "snapcast", "uxplay"]
        .into_iter()
        .filter_map(|name| match config.get(name)? {
            Keys::Table(table) => Some((name, table)),
            Keys::Other(_) => None,
        })
        .flat_map(|(name, table)| {
            REMOVED_KEYS
                .into_iter()
                .filter(|key| table.contains_key(*key))
                .map(move |key| format!("{name}.{key}"))
        })
        .collect();
    if found.is_empty() {
        return Ok(());
    }
    Err(format!(
        "the switcher no longer brings apps to the front, so remove {}",
        found.join(", ")
    ))
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
