#![no_std]

extern crate alloc;

#[cfg(feature = "host")]
mod host;

#[cfg(feature = "host")]
pub use host::*;

#[cfg(feature = "guest")]
mod guest;

#[cfg(feature = "guest")]
pub use guest::*;
