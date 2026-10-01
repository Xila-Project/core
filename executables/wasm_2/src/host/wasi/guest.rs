//! Checked access to guest memory for the WASI bindings.
//!
//! Every read or write of guest memory made by a binding goes through these helpers. They
//! translate with the `translation` module (bounds, null and alignment checks) and report
//! `EFAULT` instead of trapping, and they are the only place a translated guest range is turned
//! into a host reference.
//!
//! A translated range borrows the guest memory exclusively for as long as it is alive, so two
//! ranges (which a guest may make overlap) are never alive at once, and nothing translated is
//! kept across another use of the memory.

use core::str::from_utf8;

use wasmi::Caller;

use crate::host::{
    store::GlobalStore,
    translation::{
        FromGuest, GuestPointer, GuestSlice, WasmPod, WasmUsize, borrow_memory, get_memory,
    },
    wasi::{Error, SharedWasiContext},
};

/// Runs `operation` with the WASI context and the guest memory of `caller`.
///
/// The context handle is cloned out of the store first, so the context and the guest memory
/// (which borrows `caller` exclusively) can be used together without `Caller::data_mut()`.
pub fn with_guest<R>(
    caller: &mut Caller<GlobalStore>,
    operation: impl FnOnce(&SharedWasiContext, &mut [u8]) -> Result<R, Error>,
) -> Result<R, Error> {
    let wasi = caller.data().wasi.clone();
    let memory = get_memory(caller).ok_or(Error::Fault)?;

    operation(&wasi, borrow_memory(&memory, caller))
}

/// A host length as a guest size (`EOVERFLOW` if it does not fit).
pub fn guest_size(value: usize) -> Result<WasmUsize, Error> {
    WasmUsize::try_from(value).map_err(|_| Error::Overflow)
}

/// Translates `count` elements of `T` at guest `offset` to a host slice.
///
/// Fails with `EFAULT` for a non-empty range at the null offset, a range outside memory, or a
/// misaligned address.
pub fn guest_slice<T: WasmPod>(
    memory: &mut [u8],
    offset: WasmUsize,
    count: WasmUsize,
) -> Result<&mut [T], Error> {
    let pointer: *mut [T] = GuestSlice::<T>::new(offset, count)
        .from_guest(memory)
        .ok_or(Error::Fault)?;

    // SAFETY: `from_guest` checked that the range lies inside `memory` and is aligned for `T`.
    // `memory` is exclusively borrowed for the returned lifetime, and every bit pattern is a
    // valid `T` because `T: WasmPod`.
    Ok(unsafe { &mut *pointer })
}

/// Translates the guest address of a single `T`, which must not be null.
fn out_pointer<T: WasmPod>(memory: &mut [u8], offset: WasmUsize) -> Result<*mut T, Error> {
    let pointer: *mut T = GuestPointer::<T>::new(offset)
        .from_guest(memory)
        .ok_or(Error::Fault)?;

    // The translation maps the null offset to a null pointer, which is never a valid output.
    if pointer.is_null() {
        return Err(Error::Fault);
    }

    Ok(pointer)
}

/// Checks that a `T` can be stored at guest `offset` (non-null, in bounds, aligned), so that a
/// binding can reject a bad output pointer before it has any side effect.
pub fn check_out<T: WasmPod>(memory: &mut [u8], offset: WasmUsize) -> Result<(), Error> {
    out_pointer::<T>(memory, offset).map(|_| ())
}

/// Stores `value` at guest `offset`. Fails like [`check_out`].
pub fn write_out<T: WasmPod>(memory: &mut [u8], offset: WasmUsize, value: T) -> Result<(), Error> {
    let pointer = out_pointer::<T>(memory, offset)?;

    // SAFETY: `pointer` was translated just now from `memory`: it is non-null, in bounds and
    // aligned. Overwriting drops nothing because `T: Copy`.
    unsafe { pointer.write(value) };

    Ok(())
}

/// Borrows a UTF-8 string from guest memory: `EFAULT` for a bad range, `EILSEQ` if the bytes
/// are not UTF-8.
pub fn guest_str(memory: &mut [u8], offset: WasmUsize, length: WasmUsize) -> Result<&str, Error> {
    from_utf8(guest_slice::<u8>(memory, offset, length)?).map_err(|_| Error::Ilseq)
}

#[cfg(test)]
pub(crate) mod testing {
    /// Guest memory for tests: a fixed buffer aligned for any WASI type.
    #[repr(align(8))]
    pub struct Memory(pub [u8; 512]);

    impl Memory {
        pub fn new() -> Self {
            Self([0; 512])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{testing::Memory, *};

    #[test]
    fn slices_cover_elements_not_bytes_and_alias_guest_memory() {
        let mut backing = Memory::new();
        let memory = &mut backing.0[..];
        let base = memory.as_ptr() as usize;

        let words = guest_slice::<u32>(memory, 8, 3).unwrap();
        assert_eq!(words.len(), 3);
        assert_eq!(words.as_ptr() as usize, base + 8);

        let bytes = guest_slice::<u8>(memory, 5, 7).unwrap();
        assert_eq!(bytes.len(), 7);
        assert_eq!(bytes.as_ptr() as usize, base + 5);

        // The whole memory, exactly.
        assert_eq!(guest_slice::<u8>(memory, 1, 511).unwrap().len(), 511);
    }

    #[test]
    fn slices_reject_null_out_of_range_and_misaligned_ranges() {
        let mut backing = Memory::new();
        let memory = &mut backing.0[..];

        assert_eq!(guest_slice::<u8>(memory, 0, 4).err(), Some(Error::Fault));
        assert_eq!(guest_slice::<u8>(memory, 510, 3).err(), Some(Error::Fault));
        assert_eq!(guest_slice::<u8>(memory, 600, 1).err(), Some(Error::Fault));
        assert_eq!(
            guest_slice::<u8>(memory, 8, WasmUsize::MAX).err(),
            Some(Error::Fault)
        );
        assert_eq!(guest_slice::<u32>(memory, 2, 1).err(), Some(Error::Fault));
        // Four u32 elements do not fit in the last twelve bytes, although four bytes would.
        assert_eq!(guest_slice::<u32>(memory, 500, 4).err(), Some(Error::Fault));
    }

    #[test]
    fn empty_slices_only_need_an_offset_inside_memory() {
        let mut backing = Memory::new();
        let memory = &mut backing.0[..];

        assert_eq!(guest_slice::<u8>(memory, 0, 0).unwrap().len(), 0);
        assert_eq!(guest_slice::<u8>(memory, 512, 0).unwrap().len(), 0);
        assert_eq!(guest_slice::<u8>(memory, 513, 0).err(), Some(Error::Fault));
    }

    #[test]
    fn write_out_stores_little_endian_values_and_nothing_else() {
        let mut backing = Memory::new();
        let memory = &mut backing.0[..];

        write_out(memory, 8, 0x1122_3344_u32).unwrap();
        write_out(memory, 16, 0x0102_0304_0506_0708_u64).unwrap();

        assert_eq!(&memory[8..12], [0x44, 0x33, 0x22, 0x11]);
        assert_eq!(&memory[16..24], [8, 7, 6, 5, 4, 3, 2, 1]);
        assert_eq!(memory.iter().filter(|&&byte| byte != 0).count(), 12);
    }

    #[test]
    fn bad_output_pointers_are_rejected_and_nothing_is_written() {
        let mut backing = Memory::new();
        let memory = &mut backing.0[..];

        // Null, straddling the end of memory, past the end, misaligned.
        for offset in [0, 510, 600, 6] {
            assert_eq!(
                check_out::<u32>(memory, offset),
                Err(Error::Fault),
                "{offset}"
            );
            assert_eq!(
                write_out(memory, offset, u32::MAX),
                Err(Error::Fault),
                "{offset}"
            );
        }

        assert!(memory.iter().all(|&byte| byte == 0));
        assert_eq!(check_out::<u32>(memory, 8), Ok(()));
    }

    #[test]
    fn strings_are_borrowed_from_guest_memory() {
        let mut backing = Memory::new();
        let memory = &mut backing.0[..];
        let text = "h\u{e9}llo/w\u{f6}rld";
        memory[20..20 + text.len()].copy_from_slice(text.as_bytes());

        assert_eq!(guest_str(memory, 20, text.len() as WasmUsize), Ok(text));
        assert_eq!(guest_str(memory, 20, 0), Ok(""));
    }

    #[test]
    fn strings_must_be_utf8_and_inside_memory() {
        let mut backing = Memory::new();
        let memory = &mut backing.0[..];
        memory[20..22].copy_from_slice(&[b'a', 0xFF]);

        assert_eq!(guest_str(memory, 20, 2), Err(Error::Ilseq));
        assert_eq!(guest_str(memory, 511, 2), Err(Error::Fault));
        assert_eq!(guest_str(memory, 0, 1), Err(Error::Fault));
    }
}
