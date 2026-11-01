//! Follows Kodi's JSON-RPC notifications, reporting when playback starts
//! and when its screensaver starts and stops, and pauses Kodi, wakes it,
//! or asks for or loads a profile, when asked.
//!
//! One thread owns the connection and the socket's writer. Everything it
//! reacts to arrives on one channel: messages from a reader thread, and
//! commands from whoever holds a [`Controller`].

use std::io::{self, BufReader, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use now_playing_core::net::{Backoff, connect};
use serde_json::{Value, json};

use super::Listener;
use crate::action::Request;
use crate::quiet::Player;

/// Follows Kodi's JSON-RPC TCP port at `address`, reporting each play or
/// resume, its screensaver, the current profile when asked, and each
/// disconnection, and carries out the commands that arrive in `inbox`.
pub struct Kodi {
    pub address: String,
    pub inbox: Inbox,
}

impl Listener for Kodi {
    /// Kodi often isn't running, so a refused connection is normal: only the
    /// first failure of each outage is logged.
    fn run(self: Box<Self>, requests: Sender<Request>) -> ! {
        let address = &self.address;
        let mut backoff = Backoff::new();
        let mut quiet = false;
        let mut connection = 0;
        loop {
            match connect(address) {
                Ok(stream) => {
                    eprintln!("kodi: connected to {address}");
                    backoff.reset();
                    connection += 1;
                    let result = follow(stream, connection, &self.inbox, &mut |event| {
                        let request = match event {
                            Event::Started => {
                                eprintln!("kodi: playback started");
                                Request::Started(Player::Kodi)
                            }
                            Event::Screensaver(on) => {
                                let what = if on { "started" } else { "stopped" };
                                eprintln!("kodi: screensaver {what}");
                                Request::Screensaver(on)
                            }
                            Event::Profile(label) => Request::Profile(label),
                        };
                        // The dispatch loop outlives the listeners.
                        let _ = requests.send(request);
                    });
                    let reason = result.err().map_or("closed".to_owned(), |e| e.to_string());
                    eprintln!("kodi: {address}: disconnected ({reason}); retrying quietly");
                    let _ = requests.send(Request::KodiDisconnected);
                    quiet = true;
                }
                Err(e) if !quiet => {
                    eprintln!("kodi: {address}: {e}; retrying quietly");
                    quiet = true;
                }
                Err(_) => {}
            }
            thread::sleep(backoff.next_delay());
        }
    }
}

/// What a connection reports.
#[derive(Debug, PartialEq, Eq)]
enum Event {
    /// Playback started or resumed.
    Started,
    /// The screensaver started (`true`) or stopped.
    Screensaver(bool),
    /// The reply to `Controller::ask_profile`: the current profile's name.
    Profile(String),
}

/// Everything the connection thread reacts to, on its one channel.
enum Input {
    /// From the reader thread: a message from connection number
    /// `connection`.
    Message { connection: u64, message: Value },
    /// From the reader thread: the socket closed or failed.
    Closed { connection: u64, error: io::Error },
    /// From the dispatch loop, through a `Controller`.
    Pause,
    /// Likewise.
    Noop,
    /// Likewise.
    AskProfile,
    /// Likewise: the profile to load.
    LoadProfile(String),
}

/// What other threads hold to ask the connection to do things. It can't
/// fake a message.
#[derive(Clone)]
pub struct Controller(Sender<Input>);

impl Controller {
    /// Asks the connection to pause every active player. Never blocks; does
    /// nothing once the connection thread has gone.
    pub fn pause(&self) {
        let _ = self.0.send(Input::Pause);
    }

    /// Asks the connection to send Kodi a no-op action, which counts as the
    /// user doing something: it ends Kodi's screensaver, or holds it off.
    /// Never blocks; does nothing once the connection thread has gone.
    pub fn noop(&self) {
        let _ = self.0.send(Input::Noop);
    }

    /// Asks the connection for the current profile, which it reports as a
    /// [`Request::Profile`]. Never blocks; does nothing once the connection
    /// thread has gone.
    pub fn ask_profile(&self) {
        let _ = self.0.send(Input::AskProfile);
    }

    /// Asks the connection to load the profile called `name`. Kodi closes
    /// the connection as it does. Never blocks; does nothing once the
    /// connection thread has gone.
    ///
    /// Never ask for the profile already loaded: Kodi stops that profile's
    /// services, its JSON-RPC server among them, and never starts them
    /// again.
    pub fn load_profile(&self, name: &str) {
        let _ = self.0.send(Input::LoadProfile(name.to_owned()));
    }
}

/// The connection thread's end of [`channel`].
pub struct Inbox {
    rx: Receiver<Input>,
    /// For reader threads. Holding it also keeps `rx` open with no
    /// `Controller` left.
    tx: Sender<Input>,
}

impl Inbox {
    /// Takes the commands waiting, by name, as "pause" or "load Guest".
    #[cfg(test)]
    pub fn take(&self) -> Vec<String> {
        let inputs = self.rx.try_iter();
        let names = inputs.map(|input| match input {
            Input::Message { .. } => "message".to_owned(),
            Input::Closed { .. } => "closed".to_owned(),
            Input::Pause => "pause".to_owned(),
            Input::Noop => "noop".to_owned(),
            Input::AskProfile => "profile?".to_owned(),
            Input::LoadProfile(name) => format!("load {name}"),
        });
        names.collect()
    }

    /// Takes the commands waiting, returning how many were pauses.
    #[cfg(test)]
    pub fn take_pauses(&self) -> usize {
        self.take().iter().filter(|name| *name == "pause").count()
    }

    /// Drops everything waiting: commands asked for while Kodi was closed,
    /// and anything left from an old connection.
    fn discard_queued(&self) {
        while self.rx.try_recv().is_ok() {}
    }
}

/// A channel for commands into a connection.
pub fn channel() -> (Controller, Inbox) {
    let (tx, rx) = mpsc::channel();
    (Controller(tx.clone()), Inbox { rx, tx })
}

/// One connection: runs until it fails or Kodi closes it.
///
/// Commands waiting in `inbox` are dropped first: Kodi is often not
/// running, and a pause from while it was closed mustn't hit it as it comes
/// up.
fn follow(
    stream: TcpStream,
    connection: u64,
    inbox: &Inbox,
    on_event: &mut impl FnMut(Event),
) -> io::Result<()> {
    let socket = Socket(stream);
    inbox.discard_queued();
    let reader = socket.0.try_clone()?;
    let tx = inbox.tx.clone();
    thread::spawn(move || read(reader, connection, tx));

    let mut follow = Follow::new(connection, &socket.0);
    loop {
        // `inbox` holds a sender, so this never fails.
        let input = inbox.rx.recv().expect("inbox sender dropped");
        follow.input(input, on_event)?;
    }
}

/// Shuts the socket down when the connection ends, however it ends, so its
/// reader thread's `read()` returns and the thread exits.
struct Socket(TcpStream);

impl Drop for Socket {
    fn drop(&mut self) {
        let _ = self.0.shutdown(Shutdown::Both);
    }
}

/// Forwards `stream`'s messages into the inbox until it closes or fails, or
/// the inbox has gone.
fn read(stream: TcpStream, connection: u64, inbox: Sender<Input>) {
    let result = read_messages(stream, |message| {
        let input = Input::Message {
            connection,
            message,
        };
        inbox.send(input).is_ok()
    });
    let error = result.err().unwrap_or_else(|| {
        io::Error::new(io::ErrorKind::UnexpectedEof, "Kodi closed the connection")
    });
    let _ = inbox.send(Input::Closed { connection, error });
}

/// Reads messages until `reader` ends, handing each to `forward` until it
/// returns false. Kodi doesn't separate its messages, so they're split by
/// parsing.
fn read_messages(reader: impl Read, mut forward: impl FnMut(Value) -> bool) -> io::Result<()> {
    let messages = serde_json::Deserializer::from_reader(BufReader::new(reader));
    for message in messages.into_iter::<Value>() {
        if !forward(message?) {
            break;
        }
    }
    Ok(())
}

/// One connection's state on the connection thread, apart from its socket:
/// what it handles is testable with any writer.
struct Follow<W> {
    connection: u64,
    out: W,
    /// The last request id used.
    id: u64,
    /// The id of the latest `Player.GetActivePlayers`, until it's
    /// answered. A newer one replaces it: its reply pauses the same players,
    /// so only one is needed.
    asked: Option<u64>,
    /// Likewise for `Profiles.GetCurrentProfile`.
    asked_profile: Option<u64>,
}

impl<W: Write> Follow<W> {
    fn new(connection: u64, out: W) -> Self {
        Follow {
            connection,
            out,
            id: 0,
            asked: None,
            asked_profile: None,
        }
    }

    #[cfg(test)]
    fn into_out(self) -> W {
        self.out
    }

    /// Handles one input. An error ends the connection.
    fn input(&mut self, input: Input, on_event: &mut impl FnMut(Event)) -> io::Result<()> {
        match input {
            // From a connection that has ended.
            Input::Message { connection, .. } | Input::Closed { connection, .. }
                if connection != self.connection => {}
            Input::Message { message, .. } => self.message(&message, on_event)?,
            Input::Closed { error, .. } => return Err(error),
            // Which players are active is asked each time: after a
            // reconnect, or with Kodi playing since before we started, no
            // `Player.OnPlay` has told us.
            Input::Pause => {
                self.id += 1;
                let request = json!({
                    "id": self.id,
                    "jsonrpc": "2.0",
                    "method": "Player.GetActivePlayers",
                });
                writeln!(self.out, "{request}")?;
                self.asked = Some(self.id);
            }
            Input::Noop => {
                self.id += 1;
                writeln!(self.out, "{}", noop_request(self.id))?;
            }
            Input::AskProfile => {
                self.id += 1;
                let request = json!({
                    "id": self.id,
                    "jsonrpc": "2.0",
                    "method": "Profiles.GetCurrentProfile",
                });
                writeln!(self.out, "{request}")?;
                self.asked_profile = Some(self.id);
            }
            Input::LoadProfile(name) => {
                self.id += 1;
                let request = json!({
                    "id": self.id,
                    "jsonrpc": "2.0",
                    "method": "Profiles.LoadProfile",
                    "params": {"profile": name},
                });
                writeln!(self.out, "{request}")?;
            }
        }
        Ok(())
    }

    fn message(&mut self, message: &Value, on_event: &mut impl FnMut(Event)) -> io::Result<()> {
        let event = match message["method"].as_str() {
            Some("Player.OnPlay" | "Player.OnResume") => Some(Event::Started),
            Some("GUI.OnScreensaverActivated") => Some(Event::Screensaver(true)),
            Some("GUI.OnScreensaverDeactivated") => Some(Event::Screensaver(false)),
            _ => None,
        };
        if let Some(event) = event {
            on_event(event);
            return Ok(());
        }
        // Only replies have an id.
        let id = &message["id"];
        if id.as_u64().is_some_and(|id| self.asked_profile == Some(id)) {
            self.asked_profile = None;
            match message["result"]["label"].as_str() {
                Some(label) => on_event(Event::Profile(label.to_owned())),
                None => eprintln!("kodi: no profile in {message}"),
            }
            return Ok(());
        }
        let asked = id.as_u64().is_some_and(|id| self.asked == Some(id));
        if asked {
            self.asked = None;
        }
        if let Some(error) = message.get("error") {
            eprintln!("kodi: error reply to request {id}: {error}");
            return Ok(());
        }
        // Notifications, and `Player.PlayPause` replies.
        if !asked {
            return Ok(());
        }
        let players: Vec<u64> = message["result"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|player| player["playerid"].as_u64())
            .collect();
        if players.is_empty() {
            eprintln!("kodi: nothing to pause");
        }
        for player in players {
            self.id += 1;
            writeln!(self.out, "{}", pause_request(self.id, player))?;
        }
        Ok(())
    }
}

/// The request that pauses `player`, and the only place one is made.
///
/// Kodi has no `Player.Pause`: `Player.PlayPause` with `"play": false` sets
/// the speed to 0, and does nothing to a paused player. Without `play` it
/// would toggle.
fn pause_request(id: u64, player: u64) -> String {
    json!({
        "id": id,
        "jsonrpc": "2.0",
        "method": "Player.PlayPause",
        "params": {"playerid": player, "play": false},
    })
    .to_string()
}

/// The request that sends Kodi a no-op action: Kodi counts any action as
/// the user doing something, and this one does nothing else.
fn noop_request(id: u64) -> String {
    json!({
        "id": id,
        "jsonrpc": "2.0",
        "method": "Input.ExecuteAction",
        "params": {"action": "noop"},
    })
    .to_string()
}

#[cfg(test)]
mod tests;
