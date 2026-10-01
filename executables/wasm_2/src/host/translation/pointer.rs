use core::{marker::PhantomData, ptr::null_mut};

use crate::host::translation::{
    FromGuest, GuestSlice, IntoGuest, WasmPod, WasmUsize, sealed::Sealed, validate_and_slice,
    validate_offset,
};

/// Safe, typed handle to a WASM guest pointer.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuestPointer<T: ?Sized> {
    pub offset: WasmUsize,
    pub _marker: PhantomData<T>,
}

impl<T: ?Sized> GuestPointer<T> {
    pub const fn new(offset: WasmUsize) -> Self {
        Self {
            offset,
            _marker: PhantomData,
        }
    }

    pub const fn as_slice<U>(&self, size: WasmUsize) -> GuestSlice<U> {
        GuestSlice::new(self.offset, size)
    }
}

impl<T: Sealed> Sealed for GuestPointer<T> {}
impl<T: WasmPod> WasmPod for GuestPointer<T> {}

impl<T: WasmPod> FromGuest<*const T> for GuestPointer<T> {
    fn from_guest(&self, memory: &mut [u8]) -> Option<*const T> {
        if self.offset == 0 {
            return Some(null_mut()); // Null pointer is valid as an offset
        }

        let offset = usize::try_from(self.offset).ok()?;
        let pointer = validate_and_slice::<T>(offset, memory)?;

        Some(pointer.as_mut_ptr() as *mut T)
    }
}

impl<T: WasmPod> FromGuest<*mut T> for GuestPointer<T> {
    fn from_guest(&self, memory: &mut [u8]) -> Option<*mut T> {
        if self.offset == 0 {
            return Some(null_mut()); // Null pointer is valid as an offset
        }

        let offset = usize::try_from(self.offset).ok()?;
        let pointer = validate_and_slice::<T>(offset, memory)?;

        Some(pointer.as_mut_ptr() as *mut T)
    }
}

impl<T: WasmPod> IntoGuest<GuestPointer<T>> for *const T {
    fn into_guest(self, memory: &mut [u8]) -> Option<GuestPointer<T>> {
        let offset = validate_offset::<T>(self, memory)?;

        Some(GuestPointer::new(offset))
    }
}

impl<T: WasmPod> IntoGuest<GuestPointer<T>> for *mut T {
    fn into_guest(self, memory: &mut [u8]) -> Option<GuestPointer<T>> {
        let offset = validate_offset::<T>(self, memory)?;

        Some(GuestPointer::new(offset))
    }
}

impl<T: WasmPod> IntoGuest<GuestPointer<T>> for &T {
    fn into_guest(self, memory: &mut [u8]) -> Option<GuestPointer<T>> {
        let offset = validate_offset::<T>(self, memory)?;

        Some(GuestPointer::new(offset))
    }
}

impl<T: WasmPod> IntoGuest<GuestPointer<T>> for &mut T {
    fn into_guest(self, memory: &mut [u8]) -> Option<GuestPointer<T>> {
        let offset = validate_offset::<T>(self, memory)?;

        Some(GuestPointer::new(offset))
    }
}

impl<T: WasmPod> IntoGuest<GuestPointer<T>> for *mut [T] {
    fn into_guest(self, memory: &mut [u8]) -> Option<GuestPointer<T>> {
        let offset = validate_offset::<T>(self as *const T, memory)?;

        Some(GuestPointer::new(offset))
    }
}

impl<T: WasmPod> IntoGuest<GuestPointer<T>> for *const [T] {
    fn into_guest(self, memory: &mut [u8]) -> Option<GuestPointer<T>> {
        let offset = validate_offset::<T>(self as *const T, memory)?;

        Some(GuestPointer::new(offset))
    }
}

impl<T: WasmPod> IntoGuest<GuestPointer<T>> for &[T] {
    fn into_guest(self, memory: &mut [u8]) -> Option<GuestPointer<T>> {
        let offset = validate_offset::<T>(self.as_ptr(), memory)?;

        Some(GuestPointer::new(offset))
    }
}

impl<T: WasmPod> IntoGuest<GuestPointer<T>> for &mut [T] {
    fn into_guest(self, memory: &mut [u8]) -> Option<GuestPointer<T>> {
        let offset = validate_offset::<T>(self.as_mut_ptr(), memory)?;

        Some(GuestPointer::new(offset))
    }
}
