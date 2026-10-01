#[cfg(all(feature = "memory_32", not(feature = "memory_64")))]
pub type WasiIsize = i32;
#[cfg(feature = "memory_64")]
pub type WasiIsize = i64;

#[cfg(all(feature = "memory_32", not(feature = "memory_64")))]
pub type WasmUsize = u32;
#[cfg(feature = "memory_64")]
pub type WasmUsize = u64;

#[cfg(all(feature = "memory_32", not(feature = "memory_64")))]
pub type WasmAdress = u32;
#[cfg(feature = "memory_64")]
pub type WasmAdress = u64;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct WasiVector {
    pub buffer: WasmAdress,
    pub length: WasmUsize,
}

impl WasiVector {
    pub fn new(buffer: WasmAdress, length: WasmUsize) -> Self {
        Self { buffer, length }
    }
}

pub(crate) mod sealed {
    /// Marker trait to prevent external implementations of `WasmPod` and `WasmPointee`.
    pub trait Sealed {}
}

/// Types where *every* bit pattern is a valid value.
pub trait WasmPod: sealed::Sealed + Copy {}

/// Marker for types safe to construct a WASM guest pointer to.
pub trait WasmPointee {}

impl<T: WasmPod> WasmPointee for T {}
impl WasmPointee for core::ffi::c_void {}

impl sealed::Sealed for WasiVector {}
impl WasmPod for WasiVector {}

// Macro for primitive types
macro_rules! primitive_wasm_pod {
    ($($t:ty),* $(,)?) => {
        $(
            impl sealed::Sealed for $t {}
            impl WasmPod for $t {}
        )*
    };
}

primitive_wasm_pod!(u8, u16, u32, usize, i8, i16, i32, isize, f32, u64, i64, f64);
