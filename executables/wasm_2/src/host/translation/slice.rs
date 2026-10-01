use core::marker::PhantomData;

use crate::host::translation::{
    FromGuest, WasmPod, WasmUsize, sealed::Sealed, validate_and_slice_n,
};

/// Safe, typed handle to a WASM guest slice.
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

impl From<(WasmUsize, WasmUsize)> for GuestSlice<u8> {
    fn from((offset, len): (WasmUsize, WasmUsize)) -> Self {
        Self::new(offset, len)
    }
}

impl<T: WasmPod> FromGuest<*const [T]> for GuestSlice<T> {
    fn from_guest(&self, memory: &mut [u8]) -> Option<*const [T]> {
        if self.offset == 0 && self.size != 0 {
            return None;
        }
        let offset = usize::try_from(self.offset).ok()?;
        let length = usize::try_from(self.size).ok()?;
        let slice = validate_and_slice_n::<T>(offset, length, memory)?;

        if self.size == 0 {
            let pointer = core::ptr::NonNull::<T>::dangling().as_ptr();
            return Some(unsafe { core::slice::from_raw_parts(pointer, 0) });
        }

        Some(unsafe {
            core::slice::from_raw_parts(slice.as_mut_ptr() as *mut T, slice.len() as usize)
        })
    }
}

impl<T: WasmPod> FromGuest<*mut [T]> for GuestSlice<T> {
    fn from_guest(&self, memory: &mut [u8]) -> Option<*mut [T]> {
        if self.offset == 0 && self.size != 0 {
            return None;
        }
        let offset = usize::try_from(self.offset).ok()?;
        let length = usize::try_from(self.size).ok()?;
        let slice = validate_and_slice_n::<T>(offset, length, memory)?;

        if self.size == 0 {
            let pointer = core::ptr::NonNull::<T>::dangling().as_ptr();
            return Some(unsafe { core::slice::from_raw_parts_mut(pointer, 0) });
        }

        Some(unsafe {
            core::slice::from_raw_parts_mut(slice.as_mut_ptr() as *mut T, slice.len() as usize)
        })
    }
}

impl<T: Sealed> Sealed for GuestSlice<T> {}
impl<T: WasmPod> WasmPod for GuestSlice<T> {}
