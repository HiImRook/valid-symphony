#![no_std]

pub mod background;
pub mod clutch;
pub mod geometry;

pub use background::Background;
pub use clutch::{Clutch, Event, Phase, Tuning};
pub use geometry::{Vec3, Zone};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
