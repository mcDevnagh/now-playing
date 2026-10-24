//! Which apps Steam is running, from its processes.

use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::Path;

pub mod ui;

/// Whether Steam is running `app`. Steam starts each app under its `reaper`
/// process, as `reaper SteamLaunch AppId=<gamescope ID> -- <command>`, so
/// this looks for that in `/proc`. Apps' windows don't say which app they
/// belong to, and may be X11 or Wayland.
pub fn is_running(app: u32) -> io::Result<bool> {
    is_running_in(Path::new("/proc"), app)
}

fn is_running_in(proc: &Path, app: u32) -> io::Result<bool> {
    for entry in fs::read_dir(proc)? {
        let path = entry?.path();
        let is_pid = path
            .file_name()
            .and_then(OsStr::to_str)
            .is_some_and(|name| name.bytes().all(|b| b.is_ascii_digit()));
        // A process can exit mid-scan, so an unreadable one is skipped.
        if is_pid && fs::read(path.join("cmdline")).is_ok_and(|cmdline| launches(&cmdline, app)) {
            return Ok(true);
        }
    }
    Ok(false)
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
