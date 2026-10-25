//! Which apps Steam is running, from its processes.

use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub mod ui;

/// Whether Steam is running `app`. Steam starts each app under its `reaper`
/// process, as `reaper SteamLaunch AppId=<gamescope ID> -- <command>`, so
/// this looks for that in `/proc`. Apps' windows don't say which app they
/// belong to, and may be X11 or Wayland.
pub fn is_running(app: u32) -> io::Result<bool> {
    is_running_in(Path::new("/proc"), app)
}

fn is_running_in(proc: &Path, app: u32) -> io::Result<bool> {
    Ok(reaper_in(proc, app)?.is_some())
}

/// The X display `app`'s windows are on: the `DISPLAY` Steam started it
/// with, which in Gaming Mode is the apps' display, not Steam's own. An
/// error says why there's none, such as the app not running.
pub fn display_of(app: u32) -> Result<String, String> {
    display_of_in(Path::new("/proc"), app)
}

fn display_of_in(proc: &Path, app: u32) -> Result<String, String> {
    let reaper = reaper_in(proc, app)
        .map_err(|e| format!("{}: {e}", proc.display()))?
        .ok_or_else(|| format!("app {app} isn't running"))?;
    let environ = fs::read(reaper.join("environ"))
        .map_err(|e| format!("{}: {e}", reaper.join("environ").display()))?;
    display(&environ).ok_or_else(|| format!("app {app} has no DISPLAY"))
}

/// The `/proc/<pid>` of Steam's `reaper` launching `app`, if it's running.
fn reaper_in(proc: &Path, app: u32) -> io::Result<Option<PathBuf>> {
    for entry in fs::read_dir(proc)? {
        let path = entry?.path();
        let is_pid = path
            .file_name()
            .and_then(OsStr::to_str)
            .is_some_and(|name| name.bytes().all(|b| b.is_ascii_digit()));
        // A process can exit mid-scan, so an unreadable one is skipped.
        if is_pid && fs::read(path.join("cmdline")).is_ok_and(|cmdline| launches(&cmdline, app)) {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

/// `DISPLAY` in `environ`, as in `/proc/<pid>/environ`; `None` if it's
/// missing or empty.
fn display(environ: &[u8]) -> Option<String> {
    environ
        .split(|&b| b == 0)
        .find_map(|var| var.strip_prefix(b"DISPLAY="))
        .filter(|display| !display.is_empty())
        .map(|display| String::from_utf8_lossy(display).into_owned())
}

/// Whether `cmdline`, as in `/proc/<pid>/cmdline`, is Steam's reaper
/// launching `app`.
fn launches(cmdline: &[u8], app: u32) -> bool {
    let mut args = cmdline.split(|&b| b == 0);
    let is_reaper = args
        .next()
        .is_some_and(|arg0| arg0.rsplit(|&b| b == b'/').next() == Some(b"reaper"));
    let app_id = format!("AppId={app}");
    is_reaper
        && args.next() == Some(b"SteamLaunch")
        && args
            .take_while(|&arg| arg != b"--")
            .any(|arg| arg == app_id.as_bytes())
}

#[cfg(test)]
mod tests;
