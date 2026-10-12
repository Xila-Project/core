use alloc::vec::Vec;
use wasmi::Caller;
use xila::file_system::Position;

use crate::{
    define_wasi_module,
    host::{
        store::GlobalStore,
        translation::{GuestPointer, GuestSlice, WasiVector, WasmUsize, borrow_memory, get_memory},
        wasi::{
            Error,
            context::{FileSystemItem, FileVariant},
            error::{WasiResult, vfs_error, wrap_function},
            types::{Fdstat, Filestat, Prestat as PrestatAbi, PrestatDir},
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
                let guest = borrow_memory(&memory, &mut caller);
                guest.validate_pointer(GuestPointer::<WasmUsize>::new(nread_ptr)).ok_or(Error::Fault)?;
                let vectors = guest.values(GuestSlice::<WasiVector>::new(iovs, iovs_len)).ok_or(Error::Fault)?;
                (vectors.collect::<Vec<_>>(), nread_ptr)
            };
            for vector in &vectors {
                borrow_memory(&memory, &mut caller)
                    .validate_slice(GuestSlice::<u8>::new(vector.buffer, vector.length)).ok_or(Error::Fault)?;
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
                let mut guest = borrow_memory(&memory, &mut caller);
                for (offset, buffer) in &buffers {
                    guest.bytes_mut(GuestSlice::new(*offset, buffer.len() as WasmUsize))
                        .ok_or(Error::Fault)?.copy_from_slice(buffer);
                }
                guest.write(GuestPointer::new(nread_offset), WasmUsize::try_from(total).map_err(|_| Error::Overflow)?)
                    .ok_or(Error::Fault)?;
            }
            Ok(())
        })
    }

    fn fd_write(caller: Caller<GlobalStore>, fd: i32, iovs: WasmUsize, iovs_len: WasmUsize, nwritten_ptr: WasmUsize) -> Result<WasiResult, wasmi::Error> {
        wrap_function!({
            let mut caller = caller;
            let memory = get_memory(&mut caller).ok_or(Error::Fault)?;
            let (buffers, nwritten_offset) = {
                let guest = borrow_memory(&memory, &mut caller);
                guest.validate_pointer(GuestPointer::<WasmUsize>::new(nwritten_ptr)).ok_or(Error::Fault)?;
                let vectors = guest.values(GuestSlice::<WasiVector>::new(iovs, iovs_len)).ok_or(Error::Fault)?;
                let mut buffers: Vec<Vec<u8>> = Vec::new();
                for vector in vectors {
                    let source = guest.bytes(GuestSlice::new(vector.buffer, vector.length)).ok_or(Error::Fault)?;
                    buffers.push(source.to_vec());
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
            borrow_memory(&memory, &mut caller)
                .write(GuestPointer::new(nwritten_offset), WasmUsize::try_from(total).map_err(|_| Error::Overflow)?)
                .ok_or(Error::Fault)?;
            Ok(())
        })
    }

    fn fd_close(caller: Caller<GlobalStore>, fd: i32) -> Result<WasiResult, wasmi::Error> {
        wrap_function!({
            let mut caller = caller;
            let item = caller.data_mut().wasi.pop_file_system_item(fd as u32).ok_or(Error::Badf)?;
            let vfs = caller.data().context.virtual_file_system;
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
            borrow_memory(&memory, &mut caller).write(GuestPointer::new(newoffset_ptr), new_position).ok_or(Error::Fault)?;
            Ok(())
        })
    }

    fn fd_fdstat_get(caller: Caller<GlobalStore>, fd: i32, stat_ptr: WasmUsize) -> Result<WasiResult, wasmi::Error> {
        wrap_function!({
            let mut caller = caller;
            let memory = get_memory(&mut caller).ok_or(Error::Fault)?;
            let filetype = caller.data_mut().wasi.get_file_system_item(fd as u32).ok_or(Error::Badf)?.file_type();
            borrow_memory(&memory, &mut caller).write(GuestPointer::new(stat_ptr),
                Fdstat { filetype, flags: 0, rights_base: u64::MAX, rights_inheriting: u64::MAX }).ok_or(Error::Fault)?;
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
            borrow_memory(&memory, &mut caller).write(GuestPointer::new(stat_ptr),
                Filestat { dev: 0, ino: metadata.inode, filetype, nlink: metadata.links, size: metadata.size, atime: metadata.access.as_u64().saturating_mul(1_000_000_000), mtime: metadata.modification.as_u64().saturating_mul(1_000_000_000), ctime: metadata.creation.as_u64().saturating_mul(1_000_000_000) }).ok_or(Error::Fault)?;
            Ok(())
        })
    }

    fn fd_prestat_get(caller: Caller<GlobalStore>, fd: i32, prestat_ptr: WasmUsize) -> Result<WasiResult, wasmi::Error> {
        wrap_function!({
            let mut caller = caller;
            let name_len = u32::try_from(caller.data().wasi.prestats.get(fd.saturating_sub(3) as usize).filter(|_| fd >= 3).ok_or(Error::Badf)?.name.len()).map_err(|_| Error::Overflow)?;
            let memory = get_memory(&mut caller).ok_or(Error::Fault)?;
            borrow_memory(&memory, &mut caller).write(GuestPointer::new(prestat_ptr),
                PrestatAbi { tag: 0, dir: PrestatDir { pr_name_len: name_len } }).ok_or(Error::Fault)?;
            Ok(())
        })
    }

    fn fd_prestat_dir_name(caller: Caller<GlobalStore>, fd: i32, path_ptr: WasmUsize, path_len: WasmUsize) -> Result<WasiResult, wasmi::Error> {
        wrap_function!({
            let name = caller.data().wasi.prestats.get(fd.saturating_sub(3) as usize).filter(|_| fd >= 3).ok_or(Error::Badf)?.name.clone();
            if name.len() > path_len as usize { return Err(Error::Nametoolong.into()); }
            let mut caller = caller;
            let memory = get_memory(&mut caller).ok_or(Error::Fault)?;
            borrow_memory(&memory, &mut caller).bytes_mut(GuestSlice::new(path_ptr, path_len))
                .ok_or(Error::Fault)?[..name.len()].copy_from_slice(&name);
            Ok(())
        })
    }
}
