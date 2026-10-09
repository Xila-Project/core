use crate::{
    define_wasi_module,
    host::{
        store::GlobalStore,
        translation::{GuestPointer, GuestSlice, WasmAdress, WasmUsize, borrow_memory, get_memory},
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
            let mut guest = borrow_memory(&memory, &mut caller);
            let pointer_table = GuestSlice::<WasmAdress>::new(argv, args.len() as WasmUsize);
            let output = GuestSlice::new(argv_buf, total);
            guest.validate_slice(pointer_table).ok_or(Error::Fault)?;
            guest.validate_slice(output).ok_or(Error::Fault)?;
            let mut offset = 0usize;
            for (index, arg) in args.iter().enumerate() {
                let len = arg.len();
                let target = &mut guest.bytes_mut(output).ok_or(Error::Fault)?[offset..offset + len + 1];
                target[..len].copy_from_slice(arg.as_bytes());
                target[len] = 0;
                let guest_offset = argv_buf
                    .checked_add(WasmUsize::try_from(offset).map_err(|_| Error::Overflow)?)
                    .ok_or(Error::Overflow)?;
                let entry = pointer_table.pointer_at(index as WasmUsize).ok_or(Error::Fault)?;
                guest.write(entry, guest_offset).ok_or(Error::Fault)?;
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
            let mut guest = borrow_memory(&memory, &mut caller);
            let argc = GuestPointer::<WasmUsize>::new(argc_ptr);
            let size = GuestPointer::<WasmUsize>::new(argv_buf_size_ptr);
            guest.validate_pointer(argc).ok_or(Error::Fault)?;
            guest.validate_pointer(size).ok_or(Error::Fault)?;
            guest.write(argc, sizes.0).ok_or(Error::Fault)?;
            guest.write(size, sizes.1).ok_or(Error::Fault)?;
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
            let mut guest = borrow_memory(&memory, &mut caller);
            let table = GuestSlice::<WasmAdress>::new(environ, encoded.len() as WasmUsize);
            let out = GuestSlice::new(environ_buf, buf_len);
            guest.validate_slice(table).ok_or(Error::Fault)?;
            guest.validate_slice(out).ok_or(Error::Fault)?;
            let mut offset = 0usize;
            for (index, entry) in encoded.iter().enumerate() {
                let end = offset.checked_add(entry.len()).ok_or(Error::Overflow)?;
                guest.bytes_mut(out).ok_or(Error::Fault)?[offset..end].copy_from_slice(entry);
                let guest_offset = environ_buf
                    .checked_add(WasmUsize::try_from(offset).map_err(|_| Error::Overflow)?)
                    .ok_or(Error::Overflow)?;
                let entry = table.pointer_at(index as WasmUsize).ok_or(Error::Fault)?;
                guest.write(entry, guest_offset).ok_or(Error::Fault)?;
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
            let mut guest = borrow_memory(&memory, &mut caller);
            let count_out = GuestPointer::<WasmUsize>::new(count_ptr);
            let size_out = GuestPointer::<WasmUsize>::new(size_ptr);
            guest.validate_pointer(count_out).ok_or(Error::Fault)?;
            guest.validate_pointer(size_out).ok_or(Error::Fault)?;
            guest.write(count_out, count).ok_or(Error::Fault)?;
            guest.write(size_out, size).ok_or(Error::Fault)?;
            Ok(())
        })
    }
}
