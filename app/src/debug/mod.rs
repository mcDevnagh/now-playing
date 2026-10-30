//! Tools for working on the app without the TV or a server. Debug builds
//! only.

mod print;
mod replay;
mod weather;

pub use print::printer;
pub use replay::replay;
pub use weather::{FakeWeather, parse_fake_weather};
