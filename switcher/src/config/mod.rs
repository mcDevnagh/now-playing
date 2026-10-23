//! The TOML config: which apps to bring to the front, and on what.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use serde::Deserialize;

const DEFAULT_KODI_ADDRESS: &str = "127.0.0.1:9090";

#[derive(Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub kodi: Option<Kodi>,
    pub snapcast: Option<Snapcast>,
}

/// Kodi playback → bring Kodi to the front.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Kodi {
    /// Kodi's gamescope ID.
    pub app: u32,
    /// Kodi's JSON-RPC TCP address.
    #[serde(default = "default_kodi_address")]
    pub address: String,
}

/// Music starting on our Snapcast client → bring an app to the front, or
/// launch it.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapcast {
    /// The gamescope ID to bring to the front.
    pub app: Option<u32>,
    /// The Steam launch ID to start if it isn't running.
    pub launch: Option<u64>,
    /// snapserver's JSON-RPC address.
    pub server: String,
    /// The client to follow [default: this machine's hostname].
    pub client: Option<String>,
}

fn default_kodi_address() -> String {
    DEFAULT_KODI_ADDRESS.to_owned()
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
        if let Some(snapcast) = &config.snapcast {
            if snapcast.app.is_none() && snapcast.launch.is_none() {
                return Err("[snapcast] needs `app`, `launch` or both".to_owned());
            }
            if snapcast.server.is_empty() {
                return Err("[snapcast] `server` is empty".to_owned());
            }
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
