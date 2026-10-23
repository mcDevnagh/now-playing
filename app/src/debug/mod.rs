//! Tools for working on the app without the TV or a server. Debug builds
//! only.

mod print;
mod replay;

pub use print::printer;
pub use replay::replay;
