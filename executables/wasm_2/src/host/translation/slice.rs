use core::marker::PhantomData;

use super::{GuestPointer, GuestValue, WasmUsize, sealed::Sealed};

/// A compact guest offset and element count, resolved through a `GuestMemory` borrow.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuestSlice<T> {
    pub offset: WasmUsize,
    pub size: WasmUsize,
    pub _marker: PhantomData<T>,
}

impl<T> GuestSlice<T> {
    pub const fn new(offset: WasmUsize, size: WasmUsize) -> Self {
        Self {
            offset,
            size,
            _marker: PhantomData,
        }
    }

    pub const fn offset(&self) -> WasmUsize {
        self.offset
    }

    pub const fn len(&self) -> WasmUsize {
        self.size
    }

    pub const fn is_empty(&self) -> bool {
        self.size == 0
    }
}

impl<T: GuestValue> GuestSlice<T> {
    /// Computes an element's guest offset without losing the original table bounds.
    pub fn pointer_at(&self, index: WasmUsize) -> Option<GuestPointer<T>> {
        if index >= self.size {
            return None;
        }
        let stride = WasmUsize::try_from(T::SIZE).ok()?;
        let offset = self.offset.checked_add(index.checked_mul(stride)?)?;
        Some(GuestPointer::new(offset))
    }
}

impl From<(WasmUsize, WasmUsize)> for GuestSlice<u8> {
    fn from((offset, size): (WasmUsize, WasmUsize)) -> Self {
        Self::new(offset, size)
    }
}

impl<T: GuestValue> Sealed for GuestSlice<T> {}
impl<T: GuestValue> GuestValue for GuestSlice<T> {
    const SIZE: usize = 2 * WasmUsize::SIZE;

    fn decode(bytes: &[u8]) -> Self {
        Self::new(
            WasmUsize::decode(&bytes[..WasmUsize::SIZE]),
            WasmUsize::decode(&bytes[WasmUsize::SIZE..]),
        )
    }

    fn encode(self, bytes: &mut [u8]) {
        self.offset.encode(&mut bytes[..WasmUsize::SIZE]);
        self.size.encode(&mut bytes[WasmUsize::SIZE..]);
    }
}
