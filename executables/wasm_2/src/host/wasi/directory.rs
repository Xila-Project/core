use alloc::vec::Vec;
use wasmi::Caller;

use crate::{
    define_wasi_module,
    host::{
        store::GlobalStore,
        translation::{FromGuest, GuestPointer, GuestSlice, WasmUsize, borrow_memory, get_memory},
        wasi::{
            Error,
            error::{WasiResult, vfs_error, wrap_function},
        },
    },
};

fn kind_to_filetype(kind: &xila::file_system::Kind) -> u8 {
    use xila::file_system::Kind;
    match kind {
        Kind::Directory => 3,
        Kind::CharacterDevice => 2,
        Kind::BlockDevice => 1,
        Kind::Pipe => 10,
        Kind::Socket => 11,
        Kind::SymbolicLink => 7,
        _ => 4,
    }
}

define_wasi_module! {
    module: "wasi_snapshot_preview1";

    fn fd_readdir(caller: Caller<GlobalStore>, fd: i32, buf_ptr: WasmUsize, buf_len: WasmUsize, cookie: u64, nread_ptr: WasmUsize) -> Result<WasiResult, wasmi::Error> {
        wrap_function!({
            let mut caller = caller;
            let memory = get_memory(&mut caller).ok_or(Error::Fault)?;
            {
                let bytes = borrow_memory(&memory, &mut caller);
                let _: *mut [u8] = GuestSlice::<u8>::new(buf_ptr, buf_len).from_guest(bytes).ok_or(Error::Fault)?;
                let count: *mut WasmUsize = GuestPointer::<WasmUsize>::new(nread_ptr).from_guest(bytes).ok_or(Error::Fault)?;
                if count.is_null() { return Err(Error::Fault.into()); }
            }
            if cookie == u64::MAX {
                let bytes = borrow_memory(&memory, &mut caller);
                let count: *mut WasmUsize = GuestPointer::<WasmUsize>::new(nread_ptr).from_guest(bytes).ok_or(Error::Fault)?;
                unsafe { *count = 0; }
                return Ok(());
            }
            if cookie == u64::MAX {
                let bytes = borrow_memory(&memory, &mut caller);
                let count: *mut WasmUsize = GuestPointer::<WasmUsize>::new(nread_ptr)
                    .from_guest(bytes)
                    .ok_or(Error::Fault)?;
                unsafe { *count = 0; }
                return Ok(());
            }
            let start = usize::try_from(cookie).map_err(|_| Error::Inval)?;
            let entries = {
                let dir = caller.data_mut().wasi.get_synchronous_directory(fd as u32).ok_or(Error::Badf)?;
                dir.rewind().map_err(vfs_error)?;
                let mut entries = Vec::new();
                while let Some(entry) = dir.read().map_err(vfs_error)? { entries.push(entry); }
                entries
            };
            if start > entries.len() { return Err(Error::Inval.into()); }
            let mut encoded = Vec::new();
            for (index, entry) in entries.iter().enumerate().skip(start) {
                let mut record = Vec::with_capacity(24 + entry.name.len());
                let next = u64::try_from(index + 1).unwrap_or(u64::MAX);
                record.extend_from_slice(&next.to_le_bytes());
                record.extend_from_slice(&entry.inode.to_le_bytes());
                record.extend_from_slice(&u32::try_from(entry.name.len()).map_err(|_| Error::Overflow)?.to_le_bytes());
                record.push(kind_to_filetype(&entry.kind));
                record.extend_from_slice(&[0; 3]);
                record.extend_from_slice(entry.name.as_bytes());
                if encoded.len().checked_add(record.len()).ok_or(Error::Overflow)? > buf_len as usize { break; }
                encoded.extend_from_slice(&record);
            }
            let bytes = borrow_memory(&memory, &mut caller);
            let output: *mut [u8] = GuestSlice::<u8>::new(buf_ptr, encoded.len() as WasmUsize).from_guest(bytes).ok_or(Error::Fault)?;
            unsafe { (&mut *output).copy_from_slice(&encoded); }
            let count: *mut WasmUsize = GuestPointer::<WasmUsize>::new(nread_ptr).from_guest(bytes).ok_or(Error::Fault)?;
            unsafe { *count = encoded.len() as WasmUsize; }
            Ok(())
        })
    }
}
