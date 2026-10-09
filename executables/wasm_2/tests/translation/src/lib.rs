#![no_std]

//! Borrowed byte ranges cannot coexist with writes to overlapping guest memory.
//!
//! ```compile_fail
//! use wasm_translation_tests::translation::{GuestMemory, GuestPointer, GuestSlice};
//! let mut buffer = [0; 8];
//! let mut memory = GuestMemory::new(&mut buffer);
//! let bytes = memory.bytes_mut(GuestSlice::new(1, 4)).unwrap();
//! memory.write(GuestPointer::new(2), 7_u8);
//! bytes[0] = 1;
//! ```
//!
//! A decoded table iterator also keeps memory borrowed until its last use.
//!
//! ```compile_fail
//! use wasm_translation_tests::translation::{GuestMemory, GuestPointer, GuestSlice};
//! let mut buffer = [0; 8];
//! let mut memory = GuestMemory::new(&mut buffer);
//! let mut values = memory.values(GuestSlice::<u32>::new(1, 1)).unwrap();
//! memory.write(GuestPointer::new(1), 42_u32);
//! assert_eq!(values.next(), Some(42));
//! ```
//!
//! Borrowed bytes cannot escape the lifetime of their original buffer.
//!
//! ```compile_fail
//! use wasm_translation_tests::translation::{GuestMemory, GuestSlice};
//! let bytes = {
//!     let mut buffer = [0; 8];
//!     GuestMemory::new(&mut buffer).into_bytes_mut(GuestSlice::new(1, 4)).unwrap()
//! };
//! assert_eq!(bytes.len(), 4);
//! ```

extern crate alloc;

#[path = "../../../src/host/translation/mod.rs"]
pub mod translation;

#[cfg(test)]
#[path = "../../../src/host/wasi/types.rs"]
pub(crate) mod abi;

// Preserve the production paths used by the included sources.
pub mod host {
    pub use crate::translation;

    #[cfg(test)]
    pub(crate) mod wasi {
        pub(crate) use crate::abi as types;
    }
}
