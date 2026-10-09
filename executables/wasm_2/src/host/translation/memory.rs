use core::{marker::PhantomData, ops::Range};
use wasmi::Caller;

use super::{GuestPointer, GuestSlice, GuestValue, WasmUsize};

pub fn get_memory<T>(caller: &Caller<'_, T>) -> Option<wasmi::Memory> {
    caller
        .get_export("memory")
        .and_then(|export| export.into_memory())
}

pub fn borrow_memory<'a, T>(
    memory: &wasmi::Memory,
    caller: &'a mut Caller<'_, T>,
) -> GuestMemory<'a> {
    GuestMemory::new(memory.data_mut(caller))
}

/// An exclusive, allocation-free borrow of the current guest memory.
///
/// Byte ranges borrow this view; ABI values are decoded by value. Neither can yield a
/// dangling typed reference after memory growth, or bypass Rust's aliasing rules.
pub struct GuestMemory<'a> {
    bytes: &'a mut [u8],
}

impl<'a> GuestMemory<'a> {
    pub fn new(bytes: &'a mut [u8]) -> Self {
        Self { bytes }
    }

    fn range<T: GuestValue>(&self, slice: GuestSlice<T>) -> Option<Range<usize>> {
        if T::SIZE == 0 || (slice.offset == 0 && !slice.is_empty()) {
            return None;
        }
        let start = usize::try_from(slice.offset).ok()?;
        let count = usize::try_from(slice.size).ok()?;
        let end = start.checked_add(count.checked_mul(T::SIZE)?)?;
        self.bytes.get(start..end)?;
        Some(start..end)
    }

    pub fn validate_pointer<T: GuestValue>(&self, pointer: GuestPointer<T>) -> Option<()> {
        self.validate_slice(pointer.as_slice(1))
    }

    pub fn validate_slice<T: GuestValue>(&self, slice: GuestSlice<T>) -> Option<()> {
        self.range(slice).map(|_| ())
    }

    /// Borrows guest bytes without copying them. Nonempty null ranges are rejected.
    pub fn bytes(&self, slice: GuestSlice<u8>) -> Option<&[u8]> {
        Some(&self.bytes[self.range(slice)?])
    }

    /// Exclusively borrows guest bytes; overlapping live mutable ranges cannot be obtained.
    pub fn bytes_mut(&mut self, slice: GuestSlice<u8>) -> Option<&mut [u8]> {
        let range = self.range(slice)?;
        Some(&mut self.bytes[range])
    }

    /// Consumes the view to return a range borrowing the original memory buffer.
    pub fn into_bytes_mut(self, slice: GuestSlice<u8>) -> Option<&'a mut [u8]> {
        let range = self.range(slice)?;
        Some(&mut self.bytes[range])
    }

    /// Decodes a little-endian ABI value, including at unaligned addresses.
    pub fn read<T: GuestValue>(&self, pointer: GuestPointer<T>) -> Option<T> {
        Some(T::decode(&self.bytes[self.range(pointer.as_slice(1))?]))
    }

    /// Encodes an ABI value without native layout, alignment, or padding assumptions.
    pub fn write<T: GuestValue>(&mut self, pointer: GuestPointer<T>, value: T) -> Option<()> {
        let range = self.range(pointer.as_slice(1))?;
        value.encode(&mut self.bytes[range]);
        Some(())
    }

    /// Lazily decodes a table of ABI values without allocating a host array.
    pub fn values<T: GuestValue>(&self, slice: GuestSlice<T>) -> Option<GuestValues<'_, T>> {
        let range = self.range(slice)?;
        Some(GuestValues {
            bytes: &self.bytes[range],
            marker: PhantomData,
        })
    }

    /// Recovers the offset AND length of a borrowed byte range.
    ///
    /// This only compares numeric addresses; it does not establish pointer provenance.
    /// Prefer retaining the original `GuestSlice`, especially for mutable ranges.
    pub fn offset_of(&self, bytes: &[u8]) -> Option<GuestSlice<u8>> {
        let start = (bytes.as_ptr() as usize).checked_sub(self.bytes.as_ptr() as usize)?;
        let end = start.checked_add(bytes.len())?;
        self.bytes.get(start..end)?;
        let slice = GuestSlice::new(
            WasmUsize::try_from(start).ok()?,
            WasmUsize::try_from(bytes.len()).ok()?,
        );
        self.validate_slice(slice)?;
        Some(slice)
    }
}

/// A borrowed table iterator. Each item is decoded into a small host value on demand.
pub struct GuestValues<'a, T> {
    bytes: &'a [u8],
    marker: PhantomData<T>,
}

impl<T: GuestValue> Iterator for GuestValues<'_, T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        if self.bytes.is_empty() {
            return None;
        }
        let (value, remaining) = self.bytes.split_at(T::SIZE);
        self.bytes = remaining;
        Some(T::decode(value))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.bytes.len() / T::SIZE;
        (remaining, Some(remaining))
    }
}

impl<T: GuestValue> ExactSizeIterator for GuestValues<'_, T> {}
impl<T: GuestValue> core::iter::FusedIterator for GuestValues<'_, T> {}
