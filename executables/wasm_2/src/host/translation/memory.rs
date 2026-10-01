use wasmi::{AsContextMut, Caller};

use crate::host::translation::{WasmAdress, WasmUsize};

pub fn get_memory<'a, T>(caller: &'a mut Caller<T>) -> Option<wasmi::Memory> {
    caller.get_export("memory").and_then(|e| e.into_memory())
}

pub fn borrow_memory<'a, T>(memory: &'a wasmi::Memory, caller: &'a mut Caller<T>) -> &'a mut [u8] {
    memory.data_mut(caller.as_context_mut())
}

/// Bounds-check + align-check `start..start + count * size_of::<T>()`
/// against `memory`, returning the validated sub-slice. Every translation
/// below (single pointer, slice, and the array iterator) goes through this
/// one function, so they can't drift apart the way the mut/const pointer
/// paths previously did.
pub fn validate_and_slice_n<T>(start: usize, count: usize, memory: &mut [u8]) -> Option<&mut [u8]> {
    let byte_len = count.checked_mul(core::mem::size_of::<T>())?;

    let end = start.checked_add(byte_len)?;

    let slice = memory.get_mut(start..end)?;

    if byte_len != 0 && slice.as_mut_ptr() as usize % core::mem::align_of::<T>() != 0 {
        return None;
    }

    Some(slice)
}

pub fn validate_and_slice<T>(start: usize, memory: &mut [u8]) -> Option<&mut [u8]> {
    validate_and_slice_n::<T>(start, 1, memory)
}

/// Computes the wasm-side offset of a host pointer, checked for overflow
/// and bounds. This can only check that the arithmetic lands within
/// `memory` — it cannot *prove* `pointer` actually originated from
/// `memory`. Only pass in pointers this module produced via a matching
/// `TranslateFrom` call against the same memory.
pub fn validate_offset<T>(pointer: *const T, memory: &[u8]) -> Option<WasmAdress> {
    let offset = (pointer as usize).checked_sub(memory.as_ptr() as usize)?;

    let end = offset.checked_add(core::mem::size_of::<T>())?;
    if end > memory.len() {
        return None;
    }

    if (pointer as usize) % core::mem::align_of::<T>() != 0 {
        return None;
    }

    WasmUsize::try_from(offset).ok()
}
