#![no_std]

//! PCM character-device commands and bounded, allocation-free playback.
//!
//! Backends own their hardware stream and a [`Renderer`]; callers own the
//! [`PlaybackBuffer`] storage. Codecs and resampling belong to applications.

mod command;
mod configuration;
mod playback;

pub use command::*;
pub use configuration::*;
pub use playback::*;
