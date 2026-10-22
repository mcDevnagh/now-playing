//! Cover art: fetched and decoded off the main thread.

mod decode;
mod fetch;

pub use decode::{Cover, Image, decode};
pub use fetch::{Fetcher, with_host};

#[cfg(test)]
mod tests;
