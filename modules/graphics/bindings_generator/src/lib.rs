//! Shared, build-only LVGL guest ABI discovery.

pub mod additional;
mod catalog;
pub mod context;
pub mod enumeration;
pub mod function;
pub mod naming;

pub use catalog::{Call, CallCatalog, GeneratorError};
pub const MAX_ARGUMENTS: usize = 7;
/// Last standard event number in the existing LVGL 9.3 guest contract.
pub const GUEST_EVENT_LAST: u32 = 67;
