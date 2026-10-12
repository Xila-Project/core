use core::marker::PhantomData;

use super::{GuestSlice, GuestValue, WasmUsize, sealed::Sealed};

/// A compact guest offset, not a host pointer or a borrow of guest memory.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuestPointer<T> {
    pub offset: WasmUsize,
    pub _marker: PhantomData<T>,
}

impl<T> GuestPointer<T> {
    pub const fn new(offset: WasmUsize) -> Self {
        Self {
            offset,
            _marker: PhantomData,
        }
    }

    pub const fn as_slice(&self, size: WasmUsize) -> GuestSlice<T> {
        GuestSlice::new(self.offset, size)
    }
}

impl<T: GuestValue> Sealed for GuestPointer<T> {}
impl<T: GuestValue> GuestValue for GuestPointer<T> {
    const SIZE: usize = WasmUsize::SIZE;

    fn decode(bytes: &[u8]) -> Self {
        Self::new(WasmUsize::decode(bytes))
    }

    fn encode(self, bytes: &mut [u8]) {
        self.offset.encode(bytes);
    }
}
