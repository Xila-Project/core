#![no_std]

extern crate alloc;
#[cfg(target_arch = "wasm32")]
extern crate std;

pub mod api;
#[cfg(test)]
mod docs;
pub mod format;
pub mod model;
pub mod net;
pub mod state;
pub mod trigger;
