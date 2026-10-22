//! Turns snapserver's JSON-RPC lines into our client's stream, without any
//! I/O.
//!
//! Only our stream is kept. Anything that may change which stream that is
//! (group or client changes) asks for a fresh `Server.GetStatus` instead of
//! being applied, so other streams never need tracking.

use serde::de::DeserializeOwned;
use serde_json::Value;

use super::{Server, Stream};

/// What a line meant for us.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    /// Nothing to do: a notification about something else.
    Ignored,
    /// A full status arrived and our stream was looked up again; it may now
    /// be `None`.
    Resolved,
    /// Our stream's status or properties changed.
    Changed,
    /// Which stream is ours may have changed: send `Server.GetStatus`.
    Refetch,
}

pub struct Session {
    client: String,
    stream: Option<Stream>,
}

impl Session {
    pub fn new(client: &str) -> Session {
        Session {
            client: client.to_owned(),
            stream: None,
        }
    }

    /// Our client's stream, as of the last line. `None` until the first
    /// status, and while the client is missing or disconnected.
    pub fn stream(&self) -> Option<&Stream> {
        self.stream.as_ref()
    }

    /// Handles one line. An error means the line was malformed or an error
    /// response, and nothing changed.
    pub fn handle(&mut self, line: &str) -> Result<Action, String> {
        let msg: Value = serde_json::from_str(line).map_err(|e| format!("malformed line: {e}"))?;
        if let Some(error) = msg.get("error") {
            return Err(format!("error response: {error}"));
        }
        // The only request we send is Server.GetStatus.
        if let Some(result) = msg.get("result") {
            return self.resolve(&result["server"]);
        }

        let params = &msg["params"];
        let ours = |id: &Value| self.stream.as_ref().is_some_and(|s| id == s.id.as_str());
        match msg["method"].as_str().unwrap_or_default() {
            "Server.OnUpdate" => self.resolve(&params["server"]),
            "Stream.OnUpdate" if ours(&params["id"]) => {
                self.stream = Some(parse(&params["stream"])?);
                Ok(Action::Changed)
            }
            "Stream.OnProperties" if ours(&params["id"]) => {
                let properties = parse(&params["properties"])?;
                if let Some(stream) = &mut self.stream {
                    stream.properties = properties;
                }
                Ok(Action::Changed)
            }
            "Group.OnStreamChanged" | "Client.OnConnect" | "Client.OnDisconnect" => {
                Ok(Action::Refetch)
            }
            _ => Ok(Action::Ignored),
        }
    }

    fn resolve(&mut self, server: &Value) -> Result<Action, String> {
        let server: Server = parse(server)?;
        self.stream = server.stream_for(&self.client).cloned();
        Ok(Action::Resolved)
    }
}

fn parse<T: DeserializeOwned>(value: &Value) -> Result<T, String> {
    T::deserialize(value).map_err(|e| format!("unexpected message: {e}"))
}
