use alloc::vec::Vec;
use wasmi::Caller;
use xila::{file_system::Position, virtual_file_system};

use crate::{
    define_wasi_module,
    host::{
        store::GlobalStore,
        translation::{
            FromGuest, GuestPointer, GuestSlice, WasiVector, WasmUsize, borrow_memory, get_memory,
        },
        wasi::{
            Error,
            context::{FileSystemItem, FileVariant},
            error::{WasiResult, vfs_error, wrap_function},
            types::{Fdstat, Filestat, Prestat as PrestatAbi, PrestatDir, PrestatUnion},
        },
    },
};

define_wasi_module! {
    module: "wasi_snapshot_preview1";

    fn fd_read(caller: Caller<GlobalStore>, fd: i32, iovs: WasmUsize, iovs_len: WasmUsize, nread_ptr: WasmUsize) -> Result<WasiResult, wasmi::Error> {
        wrap_function!({
            let mut caller = caller;
            let memory = get_memory(&mut caller).ok_or(Error::Fault)?;
            let (vectors, nread_offset) = {
                let bytes = borrow_memory(&memory, &mut caller);
                let vectors: *mut [WasiVector] = GuestSlice::<WasiVector>::new(iovs, iovs_len).from_guest(bytes).ok_or(Error::Fault)?;
                let nread: *mut WasmUsize = GuestPointer::new(nread_ptr).from_guest(bytes).ok_or(Error::Fault)?;
                if nread.is_null() { return Err(Error::Fault.into()); }
                (unsafe { &*vectors }.iter().copied().collect::<Vec<_>>(), nread_ptr)
            };
            for vector in &vectors {
                let _: *mut [u8] = GuestSlice::<u8>::new(vector.buffer, vector.length)
                    .from_guest(borrow_memory(&memory, &mut caller)).ok_or(Error::Fault)?;
            }
            let mut buffers = Vec::with_capacity(vectors.len());
            let mut total = 0usize;
            for vector in &vectors {
                let length = usize::try_from(vector.length).map_err(|_| Error::Fault)?;
                let mut buffer = alloc::vec![0; length];
                let is_directory = caller.data().wasi.is_directory(fd as u32);
                let read = match caller.data_mut().wasi.get_synchronous_file(fd as u32) {
                    Some(file) => file.read(&mut buffer).map_err(vfs_error)?,
                    None if is_directory => return Err(Error::Isdir.into()),
                    None => return Err(Error::Badf.into()),
                };
                buffer.truncate(read);
                total = total.checked_add(read).ok_or(Error::Overflow)?;
                buffers.push((vector.buffer, buffer));
                if read < length { break; }
            }
            {
                let bytes = borrow_memory(&memory, &mut caller);
                for (offset, buffer) in &buffers {
                    let destination: *mut [u8] = GuestSlice::<u8>::new(*offset, buffer.len() as WasmUsize)
                        .from_guest(bytes).ok_or(Error::Fault)?;
                    unsafe { (&mut *destination).copy_from_slice(buffer); }
                }
                let nread: *mut WasmUsize = GuestPointer::new(nread_offset).from_guest(bytes).ok_or(Error::Fault)?;
                if nread.is_null() { return Err(Error::Fault.into()); }
                unsafe { *nread = total as WasmUsize; }
            }
            Ok(())
        })
    }

    fn fd_write(caller: Caller<GlobalStore>, fd: i32, iovs: WasmUsize, iovs_len: WasmUsize, nwritten_ptr: WasmUsize) -> Result<WasiResult, wasmi::Error> {
        wrap_function!({
            let mut caller = caller;
            let memory = get_memory(&mut caller).ok_or(Error::Fault)?;
            let (buffers, nwritten_offset) = {
                let bytes = borrow_memory(&memory, &mut caller);
                let vectors: *mut [WasiVector] = GuestSlice::<WasiVector>::new(iovs, iovs_len).from_guest(bytes).ok_or(Error::Fault)?;
                let nwritten: *mut WasmUsize = GuestPointer::new(nwritten_ptr).from_guest(bytes).ok_or(Error::Fault)?;
                if nwritten.is_null() { return Err(Error::Fault.into()); }
                let mut buffers: Vec<Vec<u8>> = Vec::new();
                for vector in unsafe { &*vectors }.iter() {
                    let source: *mut [u8] = GuestSlice::<u8>::new(vector.buffer, vector.length).from_guest(bytes).ok_or(Error::Fault)?;
                    buffers.push(unsafe { (&*source).to_vec() });
                }
                (buffers, nwritten_ptr)
            };
            let mut total = 0usize;
            for buffer in buffers {
                let is_directory = caller.data().wasi.is_directory(fd as u32);
                let written = match caller.data_mut().wasi.get_synchronous_file(fd as u32) {
                    Some(file) => file.write(&buffer).map_err(vfs_error)?,
                    None if is_directory => return Err(Error::Isdir.into()),
                    None => return Err(Error::Badf.into()),
                };
                total = total.checked_add(written).ok_or(Error::Overflow)?;
                if written < buffer.len() { break; }
            }
            let nwritten: *mut WasmUsize = GuestPointer::new(nwritten_offset)
                .from_guest(borrow_memory(&memory, &mut caller)).ok_or(Error::Fault)?;
            if nwritten.is_null() { return Err(Error::Fault.into()); }
            unsafe { *nwritten = total as WasmUsize; }
            Ok(())
        })
    }

    fn fd_close(caller: Caller<GlobalStore>, fd: i32) -> Result<WasiResult, wasmi::Error> {
        wrap_function!({
            let mut caller = caller;
            let item = caller.data_mut().wasi.pop_file_system_item(fd as u32).ok_or(Error::Badf)?;
            let vfs = virtual_file_system::get_instance();
            match item {
                FileSystemItem::File(file) | FileSystemItem::StandardInput(file) | FileSystemItem::StandardOutput(file) | FileSystemItem::StandardError(file) => file.file.close(vfs).map_err(vfs_error)?,
                FileSystemItem::Directory(dir) => dir.directory.close(vfs).map_err(vfs_error)?,
            }
            Ok(())
        })
    }

    fn fd_seek(caller: Caller<GlobalStore>, fd: i32, offset: i64, whence: i32, newoffset_ptr: WasmUsize) -> Result<WasiResult, wasmi::Error> {
        wrap_function!({
            let mut caller = caller;
            let memory = get_memory(&mut caller).ok_or(Error::Fault)?;
            let position = match whence { 0 if offset >= 0 => Position::Start(offset as u64), 0 => Err(Error::Inval)?, 1 => Position::Current(offset), 2 => Position::End(offset), _ => Err(Error::Inval)? };
            let is_directory = caller.data().wasi.is_directory(fd as u32);
            let new_position = match caller.data_mut().wasi.get_synchronous_file(fd as u32) {
                Some(file) => file.set_position(&position).map_err(vfs_error)?,
                None if is_directory => return Err(Error::Isdir.into()),
                None => return Err(Error::Badf.into()),
            };
            let newoffset: *mut u64 = GuestPointer::new(newoffset_ptr).from_guest(borrow_memory(&memory, &mut caller)).ok_or(Error::Fault)?;
            if newoffset.is_null() { return Err(Error::Fault.into()); }
            unsafe { *newoffset = new_position; }
            Ok(())
        })
    }

    fn fd_fdstat_get(caller: Caller<GlobalStore>, fd: i32, stat_ptr: WasmUsize) -> Result<WasiResult, wasmi::Error> {
        wrap_function!({
            let mut caller = caller;
            let memory = get_memory(&mut caller).ok_or(Error::Fault)?;
            let filetype = caller.data_mut().wasi.get_file_system_item(fd as u32).ok_or(Error::Badf)?.file_type();
            let stat: *mut Fdstat = GuestPointer::new(stat_ptr).from_guest(borrow_memory(&memory, &mut caller)).ok_or(Error::Fault)?;
            if stat.is_null() { return Err(Error::Fault.into()); }
            unsafe { *stat = Fdstat { filetype, flags: 0, rights_base: u64::MAX, rights_inheriting: u64::MAX }; }
            Ok(())
        })
    }

    fn fd_filestat_get(caller: Caller<GlobalStore>, fd: i32, stat_ptr: WasmUsize) -> Result<WasiResult, wasmi::Error> {
        wrap_function!({
            let mut caller = caller;
            let memory = get_memory(&mut caller).ok_or(Error::Fault)?;
            let (filetype, metadata) = {
                let item = caller.data_mut().wasi.get_file_system_item(fd as u32).ok_or(Error::Badf)?;
                let filetype = item.file_type();
                let metadata = match item {
                    FileSystemItem::File(FileVariant { file }) | FileSystemItem::StandardInput(FileVariant { file }) | FileSystemItem::StandardOutput(FileVariant { file }) | FileSystemItem::StandardError(FileVariant { file }) => file.get_statistics().map_err(vfs_error)?,
                    FileSystemItem::Directory(dir) => dir.directory.get_statistics().map_err(vfs_error)?,
                };
                (filetype, metadata)
            };
            let stat: *mut Filestat = GuestPointer::new(stat_ptr).from_guest(borrow_memory(&memory, &mut caller)).ok_or(Error::Fault)?;
            if stat.is_null() { return Err(Error::Fault.into()); }
            unsafe { *stat = Filestat { dev: 0, ino: metadata.inode, filetype, nlink: metadata.links, size: metadata.size, atime: metadata.access.as_u64().saturating_mul(1_000_000_000), mtime: metadata.modification.as_u64().saturating_mul(1_000_000_000), ctime: metadata.creation.as_u64().saturating_mul(1_000_000_000) }; }
            Ok(())
        })
    }

    fn fd_prestat_get(caller: Caller<GlobalStore>, fd: i32, prestat_ptr: WasmUsize) -> Result<WasiResult, wasmi::Error> {
        wrap_function!({
            let mut caller = caller;
            let name_len = u32::try_from(caller.data().wasi.prestats.get(fd.saturating_sub(3) as usize).filter(|_| fd >= 3).ok_or(Error::Badf)?.name.len()).map_err(|_| Error::Overflow)?;
            let memory = get_memory(&mut caller).ok_or(Error::Fault)?;
            let prestat: *mut PrestatAbi = GuestPointer::new(prestat_ptr).from_guest(borrow_memory(&memory, &mut caller)).ok_or(Error::Fault)?;
            if prestat.is_null() { return Err(Error::Fault.into()); }
            unsafe { *prestat = PrestatAbi { tag: 0, u: PrestatUnion { dir: PrestatDir { pr_name_len: name_len } } }; }
            Ok(())
        })
    }

    fn fd_prestat_dir_name(caller: Caller<GlobalStore>, fd: i32, path_ptr: WasmUsize, path_len: WasmUsize) -> Result<WasiResult, wasmi::Error> {
        wrap_function!({
            let name = caller.data().wasi.prestats.get(fd.saturating_sub(3) as usize).filter(|_| fd >= 3).ok_or(Error::Badf)?.name.clone();
            if name.len() > path_len as usize { return Err(Error::Nametoolong.into()); }
            let mut caller = caller;
            let memory = get_memory(&mut caller).ok_or(Error::Fault)?;
            let output: *mut [u8] = GuestSlice::<u8>::new(path_ptr, path_len).from_guest(borrow_memory(&memory, &mut caller)).ok_or(Error::Fault)?;
            unsafe { (&mut *output)[..name.len()].copy_from_slice(&name); }
            Ok(())
        })
    }
}
