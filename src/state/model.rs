/// What the screen shows.
#[derive(Debug, Clone, PartialEq)]
pub enum State {
    /// Nothing to show: black.
    Idle,
    Playing(Track),
    /// Shown dimmed.
    Paused(Track),
}

/// The track on screen. Everything but the title may be missing.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub title: String,
    /// Empty when the source reports none.
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub art_url: Option<String>,
    /// Seconds, as of the last update. Only set together with `duration`.
    pub position: Option<f64>,
    /// Seconds. Only set together with `position`.
    pub duration: Option<f64>,
}
