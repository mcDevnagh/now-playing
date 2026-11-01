//! Running the screensaver's command as a child, and stopping it.
//!
//! A thread per child waits for it. Dropping the [`Child`] stops the
//! program; it quitting on its own is reported to the dispatch loop.

use std::io;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use crate::action::Request;

/// How often the waiting thread checks whether the child has quit.
const POLL: Duration = Duration::from_millis(100);

/// A running child. Dropping it kills the program, and waits for it on
/// the waiting thread.
pub struct Child {
    /// Which child this is, to tell its report from an older one's.
    pub id: u64,
    /// Dropped to stop it.
    _stop: Sender<()>,
}

/// Starts `command` as child `id`, with this process's environment, which
/// is what makes gamescope show its window as part of Kodi's app. It
/// quitting on its own is reported as [`Request::ScreensaverExited`].
pub fn spawn(command: &[String], id: u64, requests: Sender<Request>) -> io::Result<Child> {
    let (program, args) = command.split_first().expect("checked by the config");
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .spawn()?;
    let (stop, stopped) = mpsc::channel::<()>();
    thread::spawn(move || {
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    // The dispatch loop outlives its children.
                    let _ = requests.send(Request::ScreensaverExited {
                        id,
                        status: status.to_string(),
                    });
                    return;
                }
                Ok(None) => {}
                Err(e) => eprintln!("switcher: screensaver {id}: {e}"),
            }
            match stopped.recv_timeout(POLL) {
                Err(RecvTimeoutError::Timeout) => {}
                Ok(()) | Err(RecvTimeoutError::Disconnected) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return;
                }
            }
        }
    });
    Ok(Child { id, _stop: stop })
}

#[cfg(test)]
mod tests;
