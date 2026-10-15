#![no_std]

extern crate alloc;

#[cfg(feature = "graphics")]
mod graphics_event;

#[cfg(feature = "graphics")]
pub use graphics_event::{EVENT_CLOSE_REQUESTED, EventCode};

#[cfg(feature = "host")]
mod host;

#[cfg(feature = "host")]
pub use host::*;

#[cfg(feature = "guest")]
mod guest;

#[cfg(feature = "guest")]
pub use guest::*;
