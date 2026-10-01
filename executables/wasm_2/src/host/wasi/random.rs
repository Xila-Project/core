use wasmi::Caller;

use crate::{
    define_wasi_module,
    host::{
        store::GlobalStore,
        translation::{FromGuest, GuestSlice, WasmUsize, borrow_memory, get_memory},
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
            let memory = get_memory(&mut caller).ok_or(Error::Fault)?;
            let _: *mut [u8] = GuestSlice::<u8>::new(buf_ptr, buf_len)
                .from_guest(borrow_memory(&memory, &mut caller)).ok_or(Error::Fault)?;
            let mut random_bytes = alloc::vec![0; usize::try_from(buf_len).map_err(|_| Error::Fault)?];
            let random = xila::virtual_file_system::SynchronousFile::open(
                xila::virtual_file_system::get_instance(),
                caller.data().wasi.task,
                xila::file_system::Path::from_str("/devices/random"),
                xila::file_system::AccessFlags::Read.into(),
            ).map_err(|_| Error::Io)?;
            let mut random = random;
            let read_result = random.read(&mut random_bytes);
            random.close(xila::virtual_file_system::get_instance()).map_err(|_| Error::Io)?;
            let read = read_result.map_err(|_| Error::Io)?;
            if read != random_bytes.len() { return Err(Error::Io.into()); }
            let memory_slice: *mut [u8] = GuestSlice::<u8>::new(buf_ptr, buf_len)
                .from_guest(borrow_memory(&memory, &mut caller)).ok_or(Error::Fault)?;
            unsafe { (&mut *memory_slice).copy_from_slice(&random_bytes); }

            Ok(())
        })
    }
}
