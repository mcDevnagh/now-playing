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
