use wasmi::Caller;

use crate::{
    define_wasi_module,
    host::{
        store::GlobalStore,
        translation::{FromGuest, GuestPointer, GuestSlice, WasmUsize, borrow_memory, get_memory},
        wasi::{
            Error,
            error::{WasiResult, wrap_function},
        },
    },
};

define_wasi_module! {
    module: "wasi_snapshot_preview1";

    fn random_get(
        caller: Caller<GlobalStore>,
        buf_ptr: WasmUsize,
        buf_len: WasmUsize,
    ) -> Result<WasiResult, wasmi::Error> {
        wrap_function!({
            let mut caller = caller;
            let old_state = caller.data().wasi.random_state;

            let memory = get_memory(&mut caller).ok_or(Error::Fault)?;

            let buffer : *mut [u8] = GuestSlice::new(buf_ptr, buf_len).from_guest(borrow_memory(&memory, &mut caller)).ok_or(Error::Fault)?;

            let mut state = old_state;

            for byte in (unsafe { &mut *buffer }).iter_mut() {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                *byte = (state >> 32) as u8;
            }

            caller.data_mut().wasi.random_state = state;

            Ok(())
        })
    }
}
