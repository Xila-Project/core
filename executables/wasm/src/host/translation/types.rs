use super::GuestValue;

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
    /// Keeps guest ABI codecs under this crate's control.
    pub trait Sealed {}
}

impl sealed::Sealed for WasiVector {}
impl GuestValue for WasiVector {
    const SIZE: usize = 2 * WasmUsize::SIZE;

    fn decode(bytes: &[u8]) -> Self {
        Self::new(
            WasmAdress::decode(&bytes[..WasmUsize::SIZE]),
            WasmUsize::decode(&bytes[WasmUsize::SIZE..]),
        )
    }

    fn encode(self, bytes: &mut [u8]) {
        self.buffer.encode(&mut bytes[..WasmUsize::SIZE]);
        self.length.encode(&mut bytes[WasmUsize::SIZE..]);
    }
}
