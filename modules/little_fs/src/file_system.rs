use core::ptr::null_mut;

use super::{Configuration, Directory, convert_result, littlefs};
use crate::{
    File,
    attributes::InternalAttributes,
    configuration::{self},
};
use alloc::{boxed::Box, ffi::CString};
use file_system::{
    AttributeFlags, AttributeOperations, Attributes, BaseOperations, Context, DirectBlockDevice,
    DirectoryOperations, Entry, Error, FileOperations, FileSystemOperations, Flags, Kind,
    MountOperations, Path, Position, Result, Size, mount::MutexMountWrapper,
};
use synchronization::blocking_mutex::raw::CriticalSectionRawMutex;

pub struct FileSystem {
    file_system: MutexMountWrapper<CriticalSectionRawMutex, littlefs::lfs_t>,
}

impl FileSystem {
    pub fn new_format(device: &'static dyn DirectBlockDevice, cache_size: usize) -> Result<Self> {
        Self::format(device, cache_size)?;

        Self::new(device, cache_size)
    }

    pub fn get_or_format(
        device: &'static dyn DirectBlockDevice,
        cache_size: usize,
    ) -> Result<Self> {
        match Self::new(device, cache_size) {
            Ok(file_system) => Ok(file_system),
            Err(_) => {
                device.set_position(0, &Position::Start(0))?;

                Self::format(device, cache_size)?;

                Self::new(device, cache_size)
            }
        }
    }

    pub fn new(device: &'static dyn DirectBlockDevice, cache_size: usize) -> Result<Self> {
        let block_size = device.get_block_size().map_err(|_| Error::InputOutput)?;
        let block_count = device.get_block_count().map_err(|_| Error::InputOutput)?;

        let configuration: littlefs::lfs_config = Configuration::new(
            device,
            block_size as _,
            block_count as _,
            cache_size,
            cache_size,
        )
        .ok_or(Error::InvalidParameter)?
        .try_into()
        .map_err(|_| Error::InvalidParameter)?;

        let configuration = Box::new(configuration);

        let mut file_system = littlefs::lfs_t::default();

        unsafe {
            convert_result(littlefs::lfs_mount(
                &mut file_system,
                Box::leak(configuration),
            ))?;

            let result = convert_result(littlefs::lfs_getattr(
                &mut file_system,
                c"/".as_ptr(),
                InternalAttributes::IDENTIFIER,
                null_mut(),
                0,
            ));

            if let Err(Error::NoAttribute) = result {
                // Set root attributes if not present
                let mut internal_attributes = InternalAttributes::new_uninitialized().assume_init();
                internal_attributes.kind = Kind::Directory;

                convert_result(littlefs::lfs_setattr(
                    &mut file_system,
                    c"/".as_ptr(),
                    InternalAttributes::IDENTIFIER,
                    &internal_attributes as *const _ as *const _,
                    size_of::<InternalAttributes>() as u32,
                ))?;
            } else {
                result?;
            }
        }

        Ok(Self {
            file_system: MutexMountWrapper::new_mounted(file_system),
        })
    }

    pub fn format(device: &'static dyn DirectBlockDevice, cache_size: usize) -> Result<()> {
        let block_size = device.get_block_size().map_err(|_| Error::InputOutput)?;
        let block_count = device.get_block_count().map_err(|_| Error::InputOutput)?;

        let configuration: littlefs::lfs_config = Configuration::new(
            device,
            block_size as _,
            block_count as _,
            cache_size,
            cache_size,
        )
        .ok_or(Error::InvalidParameter)?
        .try_into()
        .map_err(|_| Error::InvalidParameter)?;

        let mut file_system = littlefs::lfs_t::default();

        convert_result(unsafe { littlefs::lfs_format(&mut file_system, &configuration) })?;

        Ok(())
    }

    pub fn operation<T>(
        &self,
        operation: impl FnOnce(&mut littlefs::lfs_t) -> Result<T>,
    ) -> Result<T> {
        let mut file_system = self.file_system.try_get()?;

        operation(&mut file_system)
    }

    pub fn operation_with_context<I: 'static, T>(
        &self,
        context: &mut Context,
        operation: impl FnOnce(&mut littlefs::lfs_t, &mut I) -> Result<T>,
    ) -> Result<T> {
        let mut file_system = self.file_system.try_get()?;

        let file = context
            .get_private_data_mutable_of_type::<I>()
            .ok_or(Error::InvalidParameter)?;

        operation(&mut file_system, file)
    }
}

unsafe impl Send for FileSystem {}
unsafe impl Sync for FileSystem {}

impl MountOperations for FileSystem {
    fn unmount(&self) -> Result<()> {
        self.file_system.unmount()
    }
}

impl FileSystemOperations for FileSystem {
    fn rename(&self, source: &Path, destination: &Path) -> Result<()> {
        self.operation(|file_system| {
            let source = CString::new(source.as_str()).map_err(|_| Error::InvalidParameter)?;

            let destination =
                CString::new(destination.as_str()).map_err(|_| Error::InvalidParameter)?;

            convert_result(unsafe {
                littlefs::lfs_rename(file_system, source.as_ptr(), destination.as_ptr())
            })?;

            Ok(())
        })
    }

    fn remove(&self, path: &Path) -> Result<()> {
        let path = CString::new(path.as_str()).map_err(|_| Error::InvalidParameter)?;

        self.operation(|file_system| {
            convert_result(unsafe { littlefs::lfs_remove(file_system, path.as_ptr()) })?;

            Ok(())
        })
    }

    fn create_directory(&self, path: &Path) -> Result<()> {
        self.operation(|file_system| {
            Directory::create(file_system, path)?;

            Ok(())
        })
    }

    fn lookup_directory(&self, context: &mut Context, path: &Path) -> Result<()> {
        self.operation(|file_system| {
            let directory = Directory::lookup(file_system, path)?;
            context.set_private_data(directory);
            Ok(())
        })
    }

    fn lookup_file(&self, context: &mut Context, path: &Path, flags: Flags) -> Result<()> {
        self.operation(|file_system| {
            let file = File::lookup(file_system, path, flags)?;
            context.set_private_data(file);
            Ok(())
        })
    }

    fn create_file(&self, path: &Path) -> Result<()> {
        self.operation(|file_system| File::create(file_system, path))
    }

    fn get_attributes(&self, path: &Path, attributes: &mut Attributes) -> Result<()> {
        self.operation(|file_system| {
            let path = CString::new(path.as_str()).map_err(|_| Error::InvalidParameter)?;

            let mut internal_attributes =
                unsafe { InternalAttributes::new_uninitialized().assume_init() };

            convert_result(unsafe {
                littlefs::lfs_getattr(
                    file_system,
                    path.as_ptr(),
                    InternalAttributes::IDENTIFIER,
                    &mut internal_attributes as *mut _ as *mut _,
                    size_of::<InternalAttributes>() as u32,
                )
            })?;

            internal_attributes.update_attributes(attributes)?;

            if let Some(size) = attributes.get_mutable_size() {
                let mut info = littlefs::lfs_info::default();

                convert_result(unsafe {
                    littlefs::lfs_stat(file_system, path.as_ptr(), &mut info)
                })?;

                *size = info.size as Size;
            }

            Ok(())
        })
    }

    fn set_attributes(&self, path: &Path, attributes: &Attributes) -> Result<()> {
        self.operation(|file_system| {
            let path = CString::new(path.as_str()).map_err(|_| Error::InvalidParameter)?;

            let mut internal_attributes =
                unsafe { InternalAttributes::new_uninitialized().assume_init() };

            if attributes.get_mask() != AttributeFlags::All {
                convert_result(unsafe {
                    littlefs::lfs_getattr(
                        file_system,
                        path.as_ptr(),
                        InternalAttributes::IDENTIFIER,
                        &mut internal_attributes as *mut _ as *mut _,
                        size_of::<InternalAttributes>() as u32,
                    )
                })?;
            }

            internal_attributes.update_with_attributes(attributes)?;

            convert_result(unsafe {
                littlefs::lfs_setattr(
                    file_system,
                    path.as_ptr(),
                    InternalAttributes::IDENTIFIER,
                    &internal_attributes as *const _ as *const _,
                    size_of::<InternalAttributes>() as u32,
                )
            })?;

            Ok(())
        })
    }
}

impl AttributeOperations for FileSystem {
    fn get_attributes(&self, context: &mut Context, attributes: &mut Attributes) -> Result<()> {
        if let Some(file) = context.get_private_data_mutable_of_type::<File>() {
            file.get_attributes(attributes)
        } else if let Some(directory) = context.get_private_data_mutable_of_type::<Directory>() {
            directory.get_attributes(attributes)
        } else {
            Err(Error::InvalidParameter)
        }
    }

    fn set_attributes(&self, context: &mut Context, attributes: &Attributes) -> Result<()> {
        if let Some(file) = context.get_private_data_mutable_of_type::<File>() {
            file.set_attributes(attributes)
        } else if let Some(directory) = context.get_private_data_mutable_of_type::<Directory>() {
            directory.set_attributes(attributes)
        } else {
            Err(Error::InvalidParameter)
        }
    }
}

impl BaseOperations for FileSystem {
    fn read(
        &self,
        context: &mut Context,
        buffer: &mut [u8],
        absolute_position: Size,
    ) -> Result<usize> {
        self.operation_with_context(context, |file_system, file: &mut File| {
            file.read(file_system, buffer, absolute_position)
        })
    }

    fn write(
        &self,
        context: &mut Context,
        buffer: &[u8],
        absolute_position: Size,
    ) -> Result<usize> {
        self.operation_with_context(context, |file_system, file: &mut File| {
            file.write(file_system, buffer, absolute_position)
        })
    }

    fn set_position(
        &self,
        context: &mut Context,
        current_position: Size,
        position: &Position,
    ) -> Result<Size> {
        self.operation_with_context(context, |file_system, file: &mut File| {
            file.set_position(file_system, current_position, position)
        })
    }

    fn flush(&self, context: &mut Context) -> Result<()> {
        self.operation_with_context(context, |file_system, file: &mut File| {
            file.flush(file_system)
        })
    }

    fn clone_context(&self, context: &Context) -> Result<Context> {
        if let Some(file) = context.get_private_data_of_type::<File>() {
            Ok(Context::new(Some(file.clone())))
        } else if let Some(directory) = context.get_private_data_of_type::<Directory>() {
            Ok(Context::new(Some(directory.clone())))
        } else {
            Err(Error::InvalidParameter)
        }
    }

    fn close(&self, context: &mut Context) -> Result<()> {
        self.operation(|file_system| {
            let mut file = context
                .take_private_data_of_type::<File>()
                .ok_or(Error::InvalidParameter)?;

            file.close(file_system)?;

            Ok(())
        })
    }
}

impl FileOperations for FileSystem {}

impl DirectoryOperations for FileSystem {
    fn read(&self, context: &mut Context) -> Result<Option<Entry>> {
        self.operation_with_context(context, |file_system, directory: &mut Directory| {
            directory.read(file_system)
        })
    }

    fn get_position(&self, context: &mut file_system::Context) -> Result<Size> {
        self.operation_with_context(context, |file_system, directory: &mut Directory| {
            directory.get_position(file_system)
        })
    }

    fn set_position(&self, context: &mut Context, position: Size) -> Result<()> {
        self.operation_with_context(context, |file_system, directory: &mut Directory| {
            directory.set_position(file_system, position)
        })
    }

    fn rewind(&self, context: &mut Context) -> Result<()> {
        self.operation_with_context(context, |file_system, directory: &mut Directory| {
            directory.rewind(file_system)
        })
    }

    fn close(&self, context: &mut Context) -> Result<()> {
        self.operation(|file_system| {
            let mut directory = context
                .take_private_data_of_type::<Directory>()
                .ok_or(Error::InvalidParameter)?;

            directory.close(file_system)?;

            Ok(())
        })
    }
}

impl Drop for FileSystem {
    fn drop(&mut self) {
        let _ = self.operation(|file_system| {
            unsafe {
                littlefs::lfs_unmount(file_system);
            }

            let mut configuration =
                unsafe { Box::from_raw(file_system.cfg as *mut littlefs::lfs_config) };

            unsafe {
                configuration::Context::take_from_configuration(&mut *configuration);
            }

            Ok(())
        });
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use drivers_std;
    use file_system::{MemoryDevice, file_system::tests::implement_file_system_tests};

    use super::*;

    const CACHE_SIZE: usize = 256;

    drivers_std::memory::instantiate_global_allocator!();

    fn initialize() -> FileSystem {
        if !log::is_initialized() {
            let _ = log::initialize(&drivers_std::log::Logger);
        }

        let device = Box::leak(Box::new(MemoryDevice::<512>::new(2048 * 512)));

        FileSystem::format(device, CACHE_SIZE).unwrap();

        FileSystem::new(device, CACHE_SIZE).unwrap()
    }

    implement_file_system_tests!(initialize());
}
