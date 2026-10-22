mod model;
mod now_playing;

pub use model::{State, Track};
pub use now_playing::now_playing;

#[cfg(test)]
mod tests;
