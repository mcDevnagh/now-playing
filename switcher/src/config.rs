//! The TOML config: which listeners to start, and the app each one shows.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::app::App;

const DEFAULT_KODI_ADDRESS: &str = "127.0.0.1:9090";

#[derive(Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub kodi: Option<Kodi>,
    pub snapcast: Option<Snapcast>,
}

/// Kodi playback → show Kodi.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(try_from = "RawKodi")]
pub struct Kodi {
    pub app: App,
    /// Kodi's JSON-RPC TCP address.
    pub address: String,
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
}

/// A `[kodi]` table as written, checked on the way to [`Kodi`].
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawKodi {
    app: Option<u32>,
    launch: Option<u64>,
    address: Option<String>,
}

/// A `[snapcast]` table as written, checked on the way to [`Snapcast`].
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSnapcast {
    app: Option<u32>,
    launch: Option<u64>,
    server: String,
    client: Option<String>,
}

impl TryFrom<RawKodi> for Kodi {
    type Error = String;

    fn try_from(raw: RawKodi) -> Result<Self, String> {
        Ok(Self {
            app: App::new(raw.app, raw.launch).map_err(|e| format!("[kodi] {e}"))?,
            address: raw
                .address
                .unwrap_or_else(|| DEFAULT_KODI_ADDRESS.to_owned()),
        })
    }
}

impl TryFrom<RawSnapcast> for Snapcast {
    type Error = String;

    fn try_from(raw: RawSnapcast) -> Result<Self, String> {
        if raw.server.is_empty() {
            return Err("[snapcast] `server` is empty".to_owned());
        }
        Ok(Self {
            app: App::new(raw.app, raw.launch).map_err(|e| format!("[snapcast] {e}"))?,
            server: raw.server,
            client: raw.client,
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
        let config: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        if config.kodi.is_none() && config.snapcast.is_none() {
            return Err("needs a [kodi] or [snapcast] table".to_owned());
        }
        Ok(config)
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
