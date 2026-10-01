use crate::{
    define_wasi_module,
    host::{
        store::GlobalStore,
        translation::{
            FromGuest, GuestPointer, GuestSlice, WasmAdress, WasmUsize, borrow_memory, get_memory,
        },
        wasi::{
            Error,
            error::{WasiResult, wrap_function},
        },
    },
};
use alloc::{string::String, vec::Vec};
use wasmi::Caller;

fn count_arguments_sizes(arguments: &[String]) -> Result<(WasmUsize, WasmUsize), Error> {
    let count = WasmUsize::try_from(arguments.len()).map_err(|_| Error::Overflow)?;
    let size = arguments
        .iter()
        .try_fold(0usize, |sum, arg| sum.checked_add(arg.len() + 1))
        .ok_or(Error::Overflow)?;
    Ok((
        count,
        WasmUsize::try_from(size).map_err(|_| Error::Overflow)?,
    ))
}

define_wasi_module! {
    module: "wasi_snapshot_preview1";

    fn args_get(caller: Caller<GlobalStore>, argv: WasmAdress, argv_buf: WasmAdress) -> Result<WasiResult, wasmi::Error> {
        wrap_function!({
            let mut caller = caller;
            let args = caller.data().wasi.arguments.clone();
            let (_, total) = count_arguments_sizes(&args)?;
            let memory = get_memory(&mut caller).ok_or(Error::Fault)?;
            let (pointer_table, output) = {
                let bytes = borrow_memory(&memory, &mut caller);
                let table: *mut [WasmAdress] = GuestSlice::new(argv, args.len() as WasmUsize).from_guest(bytes).ok_or(Error::Fault)?;
                let out: *mut [u8] = GuestSlice::new(argv_buf, total).from_guest(bytes).ok_or(Error::Fault)?;
                (table, out)
            };
            let mut offset = 0usize;
            for (index, arg) in args.iter().enumerate() {
                let len = arg.len();
                let target = unsafe { &mut (&mut *output)[offset..offset + len + 1] };
                target[..len].copy_from_slice(arg.as_bytes());
                target[len] = 0;
                let guest_offset = argv_buf
                    .checked_add(WasmUsize::try_from(offset).map_err(|_| Error::Overflow)?)
                    .ok_or(Error::Overflow)?;
                unsafe { (&mut *pointer_table)[index] = guest_offset; }
                offset += len + 1;
            }
            Ok(())
        })
    }

    fn args_sizes_get(caller: Caller<GlobalStore>, argc_ptr: WasmAdress, argv_buf_size_ptr: WasmAdress) -> Result<WasiResult, wasmi::Error> {
        wrap_function!({
            let mut caller = caller;
            let sizes = count_arguments_sizes(&caller.data().wasi.arguments)?;
            let memory = get_memory(&mut caller).ok_or(Error::Fault)?;
            let bytes = borrow_memory(&memory, &mut caller);
            let argc: *mut WasmUsize = GuestPointer::new(argc_ptr).from_guest(bytes).ok_or(Error::Fault)?;
            let size: *mut WasmUsize = GuestPointer::new(argv_buf_size_ptr).from_guest(bytes).ok_or(Error::Fault)?;
            if argc.is_null() || size.is_null() { return Err(Error::Fault.into()); }
            unsafe { *argc = sizes.0; *size = sizes.1; }
            Ok(())
        })
    }

    fn environ_get(caller: Caller<GlobalStore>, environ: WasmAdress, environ_buf: WasmAdress) -> Result<WasiResult, wasmi::Error> {
        wrap_function!({
            let mut caller = caller;
            let vars = caller.data().wasi.environment.clone();
            let mut encoded = Vec::new();
            for (name, value) in &vars {
                let mut bytes = Vec::new();
                bytes.extend_from_slice(name.as_bytes());
                bytes.push(b'=');
                bytes.extend_from_slice(value.as_bytes());
                bytes.push(0);
                encoded.push(bytes);
            }
            let buf_len = encoded.iter().try_fold(0usize, |n, v| n.checked_add(v.len())).ok_or(Error::Overflow)?;
            let buf_len = WasmUsize::try_from(buf_len).map_err(|_| Error::Overflow)?;
            let memory = get_memory(&mut caller).ok_or(Error::Fault)?;
            let (table, out) = {
                let bytes = borrow_memory(&memory, &mut caller);
                let table: *mut [WasmAdress] = GuestSlice::new(environ, encoded.len() as WasmUsize).from_guest(bytes).ok_or(Error::Fault)?;
                let out: *mut [u8] = GuestSlice::new(environ_buf, buf_len).from_guest(bytes).ok_or(Error::Fault)?;
                (table, out)
            };
            let mut offset = 0usize;
            for (index, entry) in encoded.iter().enumerate() {
                let end = offset.checked_add(entry.len()).ok_or(Error::Overflow)?;
                unsafe { (&mut *out)[offset..end].copy_from_slice(entry); }
                let guest_offset = environ_buf
                    .checked_add(WasmUsize::try_from(offset).map_err(|_| Error::Overflow)?)
                    .ok_or(Error::Overflow)?;
                unsafe { (&mut *table)[index] = guest_offset; }
                offset = end;
            }
            Ok(())
        })
    }

    fn environ_sizes_get(caller: Caller<GlobalStore>, count_ptr: WasmAdress, size_ptr: WasmAdress) -> Result<WasiResult, wasmi::Error> {
        wrap_function!({
            let mut caller = caller;
            let vars = caller.data().wasi.environment.clone();
            let count = WasmUsize::try_from(vars.len()).map_err(|_| Error::Overflow)?;
            let size = vars.iter().try_fold(0usize, |n, (name, value)| n.checked_add(name.len() + value.len() + 2)).ok_or(Error::Overflow)?;
            let size = WasmUsize::try_from(size).map_err(|_| Error::Overflow)?;
            let memory = get_memory(&mut caller).ok_or(Error::Fault)?;
            let bytes = borrow_memory(&memory, &mut caller);
            let count_out: *mut WasmUsize = GuestPointer::new(count_ptr).from_guest(bytes).ok_or(Error::Fault)?;
            let size_out: *mut WasmUsize = GuestPointer::new(size_ptr).from_guest(bytes).ok_or(Error::Fault)?;
            if count_out.is_null() || size_out.is_null() { return Err(Error::Fault.into()); }
            unsafe { *count_out = count; *size_out = size; }
            Ok(())
        })
    }
}
