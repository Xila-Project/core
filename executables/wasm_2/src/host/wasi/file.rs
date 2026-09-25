use alloc::vec::Vec;
use wasmi::{AsContextMut, Caller};
use xila::{
    file_system::Position,
    virtual_file_system::{self, File},
};

use crate::{
    define_wasi_module,
    host::{
        store::GlobalStore,
        translation::{
            FromGuest, GuestPointer, GuestSlice, GuestSliceStream, IntoGuest, WasiVector,
            WasmPointer, WasmUsize, borrow_memory, get_memory,
        },
        wasi::{
            Error,
            context::FileSystemItem,
            error::{WasiResult, vfs_error, wrap_function},
            types::{Fdstat, Filestat, Prestat},
        },
    },
};

fn fd_type(ty: &FileSystemItem) -> u8 {
    match ty {
        FileSystemItem::File(_) => 4,
        FileSystemItem::Directory(_, _) => 3,
        FileSystemItem::CharacterDevice => 2,
        FileSystemItem::Stdout(_) => 2,
        FileSystemItem::Stderr(_) => 2,
    }
}

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

    fn fd_read(
        caller: Caller<GlobalStore>,
        fd: i32,
        iovs_ptr: WasmUsize,
        iovs_len: WasmUsize,
        nread_ptr: WasmUsize,
    ) -> Result<WasiResult, wasmi::Error> {

        wrap_function!({

            xila::log::information!(
                "fd_read: fd={}, iovs={:?}, iovs_len={}, nread={:?}",
                fd, iovs_ptr, iovs_len, nread_ptr
            );

            let mut caller = caller;
            let memory = get_memory(&mut caller).ok_or(Error::Fault)?;

            let nread : *mut WasmUsize = GuestPointer::<WasmUsize>::new(nread_ptr).from_guest(borrow_memory(&memory, &mut caller)).ok_or(Error::Fault)?;
            let iovs_stream : *mut [GuestSlice<u8>] = GuestSlice::<GuestSlice<u8>>::new(iovs_ptr, iovs_len).from_guest(borrow_memory(&memory, &mut caller)).ok_or(Error::Fault)?;


            let file = caller.data_mut().wasi.get_synchronous_file(fd as u32).ok_or(
                Error::Badf
            )?;

            let mut total_read = 0;

            for buf in unsafe { &mut *iovs_stream }.iter_mut() {
                let buf: *mut [u8] = buf.from_guest(borrow_memory(&memory, &mut caller)).ok_or(Error::Fault)?;

                let read = file.read(unsafe { &mut *buf }).map_err(vfs_error)?;

                total_read += read;
            }


            unsafe {
                *nread = total_read as u32;
            }

            Ok(())
        })



    }

    fn fd_write(
        caller: Caller<GlobalStore>,
        fd: i32,
        iovs_ptr: i32,
        iovs_len: i32,
        nwritten_ptr: WasmUsize,
    ) -> Result<i32, wasmi::Error> {
        wrap_function!({

            xila::log::information!(
                "fd_write: fd={}, iovs={:#x}, iovs_len={}, nwritten={:#x}",
                fd, iovs_ptr, iovs_len, nwritten_ptr
            );

            let mut caller = caller;
            let memory = get_memory(&mut caller).ok_or(Error::Fault)?;

            let nwritten : *mut u32 = unsafe { FromGuest::from_guest(nwritten_ptr as _, memory).ok_or(Error::Fault)? };
            let stream = WasiVector::new(iovs_ptr as _, iovs_len as _);
            let mut stream = unsafe { GuestSliceStream::from_guest(stream, memory).ok_or(Error::Fault)? };

            let file = caller.data_mut().wasi.get_synchronous_file(fd as u32).ok_or(
                Error::Badf
            )?;

            let mut total_written = 0;

            while let Some(buf) = stream.next() {
                let buf: &[u8] = buf.ok_or(Error::Fault)?;

                let written = file.write(buf).map_err(vfs_error)?;

                total_written += written;
            }

            unsafe {
                *nwritten = total_written as u32;
            }
            Ok(())
        })
    }

    fn fd_close(
        caller: Caller<GlobalStore>,
        fd: i32,
    ) -> Result<WasiResult, wasmi::Error> {
        wrap_function!({
            xila::log::information!("fd_close: fd={}", fd);

            let virtual_file_system = virtual_file_system::get_instance();


            let mut caller = caller;
            let store = caller.data_mut();
            let file = store.wasi.pop_file_system_item(fd as u32).ok_or(Error::Badf)?;

            match file {
                FileSystemItem::File(file) => {
                    file.file.close(virtual_file_system).map_err(vfs_error)?;
                }
                FileSystemItem::StandardInput(file) => {
                    file.file.close(virtual_file_system).map_err(vfs_error)?;
                }
                FileSystemItem::StandardOutput(file) => {
                    file.file.close(virtual_file_system).map_err(vfs_error)?;
                }
                FileSystemItem::StandardError(file) => {
                    file.file.close(virtual_file_system).map_err(vfs_error)?;
                }
                FileSystemItem::Directory(dir) => {
                    dir.directory.close(virtual_file_system).map_err(vfs_error)?;
                }
            }

            Ok(())
        })
    }

    fn fd_seek(
        caller: Caller<GlobalStore>,
        fd: i32,
        offset: i64,
        whence: i32,
        newoffset_ptr: WasmUsize,
    ) -> Result<WasiResult, wasmi::Error> {
        wrap_function!({
            xila::log::information!(
                "fd_seek: fd={}, offset={}, whence={}, newoffset={:#x}",
                fd, offset, whence, newoffset_ptr
            );

            let mut caller = caller;
            let memory = get_memory(&mut caller).ok_or(Error::Fault)?;

            let newoffset : *mut u64 = unsafe { FromGuest::from_guest(newoffset_ptr as _, memory).ok_or(Error::Fault)? };
            let file = caller.data_mut().wasi.get_synchronous_file(fd as u32).ok_or(Error::Badf)?;

            let position = match whence {
                0 => Position::Start(offset as u64),
                1 => Position::Current(offset),
                2 => Position::End(offset),
                _ => Err(Error::Inval)?,
            };

            let new_position = file.set_position(&position).map_err(vfs_error)?;

            unsafe {
                *newoffset = new_position as u64;
            }


            Ok(())
        })
    }

    fn fd_fdstat_get(
        caller: Caller<GlobalStore>,
        fd: i32,
        stat_ptr: WasmUsize,
    ) -> Result<i32, wasmi::Error> {
        wrap_function!({
            xila::log::information!(
                "fd_fdstat_get: fd={}, stat={:#x}",
                fd, stat_ptr
            );

            let mut caller = caller;
            let memory = get_memory(&mut caller).ok_or(Error::Fault)?;

            let stat : *mut Fdstat = unsafe { FromGuest::from_guest(stat_ptr as _, memory).ok_or(Error::Fault)? };

            let file = caller.data_mut().wasi.get_file_system_item(fd as u32).ok_or(Error::Badf)?;

            let filetype = fd_type(file);
            let flags = 0u16;
            let rights_base = u64::MAX;
            let rights_inheriting = u64::MAX;

            unsafe {
                *stat = Fdstat {
                    filetype,
                    flags,
                    rights_base,
                    rights_inheriting,
                };
            }

            Ok(())
        })
    }

    fn fd_prestat_get(
        caller: Caller<GlobalStore>,
        fd: i32,
        prestat_ptr: WasmUsize,
    ) -> Result<i32, wasmi::Error> {
        wrap_function!({
            xila::log::information!(
                "fd_prestat_get: fd={}, prestat={:#x}",
                fd, prestat_ptr
            );

            let mut caller = caller;
            let memory = get_memory(&mut caller).ok_or(Error::Fault)?;

            let prestat : *mut Prestat = unsafe { FromGuest::from_guest(prestat_ptr as _, memory).ok_or(Error::Fault)? };

            let prestats = &caller.data().wasi.prestats;
            if fd <= 3 || (fd as usize - 3) >= prestats.len() {
                return Err(Error::Badf.into());
            }

            let prestat_item = &prestats[fd as usize - 3];
            unsafe {
                *prestat = crate::host::wasi::types::Prestat {
                    tag: crate::host::wasi::types::Preopentype::Dir,
                    u: crate::host::wasi::types::PrestatUnion {
                        dir: crate::host::wasi::types::PrestatDir {
                            pr_name_len: prestat_item.name.len() as u32,
                        },
                    },
                };
            }

            Ok(())
        })
    }

    fn fd_prestat_dir_name(
        caller: Caller<GlobalStore>,
        fd: i32,
        path_ptr: WasmUsize,
        path_len: WasmUsize,
    ) -> Result<i32, wasmi::Error> {
        wrap_function!({
            xila::log::information!(
                "fd_prestat_dir_name: fd={}, path={:#x}, path_len={}",
                fd, path_ptr, path_len
            );

            let mut caller = caller;
            let memory = get_memory(&mut caller).ok_or(Error::Fault)?;

            let prestats = &caller.data().wasi.prestats;
            if fd <= 3 || (fd as usize - 3) >= prestats.len() {
                return Err(Error::Badf.into());
            }

            let prestat_item = &prestats[fd as usize - 3];
            if prestat_item.name.len() > path_len as usize {
                return Err(Error::Nametoolong.into());
            }

            let path_slice = WasiVector::new(path_ptr as _, path_len as _);
            let mut path_slice : &mut [u8] = unsafe { FromGuest::from_guest(path_slice, memory).ok_or(Error::Fault)? };
            unsafe {
                (*path_slice)[..prestat_item.name.len()].copy_from_slice(&prestat_item.name);
            }

            Ok(())
        })
    }

    fn fd_filestat_get(
        caller: Caller<GlobalStore>,
        fd: i32,
        stat_ptr: WasmUsize,
    ) -> Result<i32, wasmi::Error> {
        wrap_function!({
            xila::log::information!(
                "fd_filestat_get: fd={}, stat={:#x}",
                fd, stat_ptr
            );

            let mut caller = caller;
            let memory = get_memory(&mut caller).ok_or(Error::Fault)?;

            let stat : *mut Filestat = unsafe { FromGuest::from_guest(stat_ptr as _, memory).ok_or(Error::Fault)? };

            let file = caller.data_mut().wasi.get_file_system_item(fd as u32).ok_or(Error::Badf)?;

            let (size, atime, mtime, ctime) = match file {
                FileSystemItem::File(file) => {
                    let metadata = file.file.get_statistics().map_err(vfs_error)?;
                    (metadata.size(), metadata.accessed(), metadata.modified(), metadata.created())
                }
                FileSystemItem::StandardInput(file) => {
                    let metadata = file.file.get_statistics().map_err(vfs_error)?;
                    (metadata.size(), metadata.accessed(), metadata.modified(), metadata.created())
                }
                FileSystemItem::StandardOutput(file) => {
                    let metadata = file.file.metadata().map_err(vfs_error)?;
                    (metadata.size(), metadata.accessed(), metadata.modified(), metadata.created())
                }
                FileSystemItem::StandardError(file) => {
                    let metadata = file.file.metadata().map_err(vfs_error)?;
                    (metadata.size(), metadata.accessed(), metadata.modified(), metadata.created())
                }
                FileSystemItem::Directory(dir) => {
                    let metadata = dir.directory.metadata().map_err(vfs_error)?;
                    (metadata.size(), metadata.accessed(), metadata.modified(), metadata.created())
                }
            };

            unsafe {
                *stat = Filestat {

                    size,
                    atime,
                    mtime,
                    ctime,
                };
            }

            Ok(())
        })
    }
}
