use alloc::{string::String, vec::Vec};
use wasmi::{AsContextMut, Caller};
use xila::task::{self, block_on};

use crate::{
    define_wasi_module,
    host::{
        store::GlobalStore,
        translation::{
            FromGuest, GuestPointer, GuestSlice, IntoGuest, WasmAdress, WasmUsize, borrow_memory,
            get_memory,
        },
        wasi::{
            Error,
            error::{WasiResult, wrap_function},
        },
    },
};

fn count_arguments_sizes(caller: &Vec<String>) -> (WasmUsize, WasmUsize) {
    let argc = caller.len() as WasmUsize;
    let argv_buf_size = caller.iter().map(|arg| arg.len() + 1).sum::<usize>() as WasmUsize;
    (argc, argv_buf_size)
}

define_wasi_module! {
    module: "wasi_snapshot_preview1";

  fn args_get(
        caller: Caller<GlobalStore>,
        argv: WasmAdress,
        argv_buf: WasmAdress,
    ) -> Result<WasiResult, wasmi::Error> {
        wrap_function!({
            let mut caller = caller;

            let (argc, argv_buf_size) = count_arguments_sizes(&caller.data().wasi.arguments);

            let memory = get_memory(&mut caller).ok_or(Error::Fault)?;

            // 1. Mutably borrow caller strictly to resolve guest slices, then drop the borrow immediately
            let (argv_slice, argv_buf_slice) = {
                let memory = get_memory(&mut caller).ok_or(Error::Fault)?;
                let argv_ptr: *mut [GuestPointer<u8>] = GuestSlice::new(argv, argc)
                    .from_guest(borrow_memory(&memory, &mut caller))
                    .ok_or(Error::Fault)?;
                let argv_buf_ptr: *mut [u8] = GuestSlice::new(argv_buf, argv_buf_size)
                    .from_guest(borrow_memory(&memory, &mut caller))
                    .ok_or(Error::Fault)?;
                (argv_ptr, argv_buf_ptr)
            };
            // `memory` and `&mut caller` are dropped here!

            let mut argv_buf_offset = 0;

            // 2. Iterate safely: caller can be immutably accessed without borrow conflicts
            for (arg_index, arg) in unsafe { (&mut *argv_slice).iter_mut() }.enumerate() {
                let arg_str = &caller.data().wasi.arguments.get(arg_index).ok_or(Error::Fault)?.as_str();
                let arg_bytes = arg_str.as_bytes();
                let arg_len = arg_bytes.len();

                if argv_buf_offset + arg_len + 1 > argv_buf_size as usize {
                    return Err(Error::Fault.into());
                }

                // Copy bytes and null-terminate
                let target = &mut unsafe { &mut *argv_buf_slice }
                    [argv_buf_offset..argv_buf_offset + arg_len + 1];
                target[..arg_len].copy_from_slice(arg_bytes);
                target[arg_len] = 0;

                // Calculate absolute WASM address by incrementing base offset
                *arg = target.as_mut_ptr().into_guest(borrow_memory(&memory, &mut caller)).ok_or(Error::Fault)?;

                argv_buf_offset += arg_len + 1;
            }

            Ok(())
        })
    }

    fn args_sizes_get(
        caller: Caller<GlobalStore>,
        argc_ptr: WasmAdress,
        argv_buf_size_ptr: WasmAdress,
    ) -> Result<WasiResult, wasmi::Error> {
        wrap_function!({
            let mut caller = caller;
            let memory = get_memory(&mut caller).ok_or(Error::Fault)?;
            let memory = borrow_memory(&memory, &mut caller);

            let argc : *mut WasmUsize = GuestPointer::new(argc_ptr).from_guest(memory).ok_or(Error::Fault)?;
            let argv_buf_size : *mut WasmUsize = GuestPointer::new(argv_buf_size_ptr).from_guest(memory).ok_or(Error::Fault)?;

            let result = count_arguments_sizes(&caller.data().wasi.arguments);

            unsafe {
                *argc = result.0;
                *argv_buf_size = result.1;
            }

            Ok(())
        })
    }

  fn environ_get(
        caller: Caller<GlobalStore>,
        environ: WasmAdress,
        environ_buf: WasmAdress,
    ) -> Result<WasiResult, wasmi::Error> {
        wrap_function!({
            let mut caller = caller;

            let task = caller.data().wasi.task;
            let environment_variables = block_on(task::get_instance().get_environment_variables(task))
                .map_err(|_| Error::Fault)?;

            let env_count = environment_variables.len() as WasmUsize;
            let env_buf_size: usize = environment_variables
                .iter()
                .map(|var| var.get_name().len() + 1 + var.get_value().len() + 1)
                .sum();

            let memory = get_memory(&mut caller).ok_or(Error::Fault)?;

            let environ_slice: *mut [GuestPointer<u8>] = GuestSlice::new(environ, env_count)
                .from_guest(borrow_memory(&memory, &mut caller))
                .ok_or(Error::Fault)?;
            let environ_buf_slice: *mut [u8] = GuestSlice::new(environ_buf, env_buf_size as WasmUsize)
                .from_guest(borrow_memory(&memory, &mut caller))
                .ok_or(Error::Fault)?;

            let mut environ_buf_offset = 0;

            for (env_ptr, env_var) in unsafe { &mut *environ_slice }.iter_mut().zip(environment_variables.iter()) {
                let name = env_var.get_name().as_bytes();
                let value = env_var.get_value().as_bytes();
                let var_total_len = name.len() + 1 + value.len() + 1; // "KEY=VALUE\0"

                if environ_buf_offset + var_total_len > env_buf_size {
                    return Err(Error::Fault.into());
                }

                unsafe {
                    let target = &mut (*environ_buf_slice)[environ_buf_offset..environ_buf_offset + var_total_len];

                    // Copy "KEY=VALUE\0" directly without format!() allocation
                    target[..name.len()].copy_from_slice(name);
                    target[name.len()] = b'=';
                    target[name.len() + 1..name.len() + 1 + value.len()].copy_from_slice(value);
                    target[var_total_len - 1] = 0;

                    // Convert host pointer to GuestPointer<u8> using IntoGuest trait
                    *env_ptr = target.as_mut_ptr().into_guest(borrow_memory(&memory, &mut caller)).ok_or(Error::Fault)?;
                }

                environ_buf_offset += var_total_len;
            }

            Ok(())
        })
    }

    fn environ_sizes_get(
        caller: Caller<GlobalStore>,
        environ_count_ptr: WasmAdress,
        environ_buf_size_ptr: WasmAdress,
    ) -> Result<WasiResult, wasmi::Error> {
        wrap_function!({
            let mut caller = caller;
            let task = caller.data().wasi.task;

            let environment_variables = block_on(task::get_instance().get_environment_variables(task))
                .map_err(|_| Error::Fault)?;

            let memory = get_memory(&mut caller).ok_or(Error::Fault)?;
            let memory = borrow_memory(&memory, &mut caller);

            let environ_count: *mut WasmUsize = GuestPointer::new(environ_count_ptr)
                .from_guest(memory)
                .ok_or(Error::Fault)?;
            let environ_buf_size: *mut WasmUsize = GuestPointer::new(environ_buf_size_ptr)
                .from_guest(memory)
                .ok_or(Error::Fault)?;

            let count = environment_variables.len() as WasmUsize;
            let total_buf_size: usize = environment_variables
                .iter()
                .map(|var| var.get_name().len() + 1 + var.get_value().len() + 1)
                .sum();

            unsafe {
                *environ_count = count;
                *environ_buf_size = total_buf_size as WasmUsize;
            }

            Ok(())
        })
    }
}
