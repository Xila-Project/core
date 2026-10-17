#![no_std]

//! PCM character-device commands, bounded playback, and a fixed-capacity mixer.
//!
//! Backends own their hardware stream and a [`Renderer`]; callers own the
//! [`PlaybackBuffer`] or [`AudioSystem`] storage. Codecs and resampling belong to applications.

mod command;
mod configuration;
mod playback;
mod system;

pub use command::*;
pub use configuration::*;
pub use playback::*;
pub use system::*;
