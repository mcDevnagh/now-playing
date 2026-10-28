//! The parts of snapserver's `Server.GetStatus` result that the app reads.
//!
//! Every field the app doesn't strictly need is optional or defaulted, so a
//! source that reports less still parses.

use serde::Deserialize;

/// The `server` object of `Server.GetStatus` and `Server.OnUpdate`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Server {
    #[serde(default)]
    pub groups: Vec<Group>,
    #[serde(default)]
    pub streams: Vec<Stream>,
}

impl Server {
    /// The stream `client` is playing: its group's stream.
    ///
    /// `client` is matched against client ids, first exactly, then ignoring
    /// case, then against host names ignoring case. `None` if the client is
    /// missing or disconnected, or its group's stream doesn't exist.
    pub fn stream_for(&self, client: &str) -> Option<&Stream> {
        let clients = || {
            self.groups
                .iter()
                .flat_map(|g| g.clients.iter().map(move |c| (g, c)))
        };
        let matches: [&dyn Fn(&Client) -> bool; 3] = [
            &|c| c.id == client,
            &|c| c.id.eq_ignore_ascii_case(client),
            &|c| c.host.name.eq_ignore_ascii_case(client),
        ];
        let (group, found) = matches.iter().find_map(|m| clients().find(|(_, c)| m(c)))?;
        if !found.connected {
            return None;
        }
        self.streams.iter().find(|s| s.id == group.stream_id)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Group {
    pub id: String,
    pub stream_id: String,
    #[serde(default)]
    pub clients: Vec<Client>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Client {
    pub id: String,
    #[serde(default)]
    pub connected: bool,
    #[serde(default)]
    pub host: Host,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Host {
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Stream {
    pub id: String,
    /// `idle`, `playing`, `disabled` or `unknown`.
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub properties: Properties,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Properties {
    /// `playing`, `paused` or `stopped`, when the source reports it.
    pub playback_status: Option<String>,
    /// Seconds, as of the last event.
    pub position: Option<f64>,
    #[serde(default)]
    pub metadata: Metadata,
    /// Whether the stream takes `Stream.Control` commands at all, and
    /// which. A source that doesn't say can't.
    #[serde(default)]
    pub can_control: bool,
    #[serde(default)]
    pub can_play: bool,
    #[serde(default)]
    pub can_pause: bool,
    #[serde(default)]
    pub can_go_next: bool,
    #[serde(default)]
    pub can_go_previous: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub title: Option<String>,
    #[serde(default)]
    pub artist: Vec<String>,
    pub album: Option<String>,
    pub art_url: Option<String>,
    /// Seconds.
    pub duration: Option<f64>,
}
