use core::{fmt::Debug, mem::forget};

use crate::{Error, ItemStatic, Result, VirtualFileSystem};
use alloc::{vec, vec::Vec};
use exported_file_system::StateFlags;
use exported_file_system::{ControlCommand, Permissions};
use file_system::{
    AccessFlags, AttributeFlags, Attributes, Context, Flags, Path, Position, Size, Statistics,
};
use shared::AnyByLayout;
use task::TaskIdentifier;
use task::block_on;
use users::{GroupIdentifier, UserIdentifier};

pub struct SynchronousFile<'a> {
    virtual_file_system: &'a VirtualFileSystem,
    pub(crate) item: ItemStatic,
    pub(crate) position: Size,
    pub(crate) flags: Flags,
    pub(crate) context: Context,
}

impl Debug for SynchronousFile<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SynchronousFile")
            .field("item", &self.item)
            .field("position", &self.position)
            .field("flags", &self.flags)
            .finish()
    }
}

impl<'a> SynchronousFile<'a> {
    pub(crate) const fn new(
        virtual_file_system: &'a VirtualFileSystem,
        item: ItemStatic,
        position: Size,
        flags: Flags,
        context: Context,
    ) -> Self {
        Self {
            virtual_file_system,
            item,
            position,
            flags,
            context,
        }
    }

    pub fn open(
        virtual_file_system: &'a VirtualFileSystem,
        task: TaskIdentifier,
        path: impl AsRef<Path>,
        flags: Flags,
    ) -> Result<Self> {
        let file_identifier = block_on(virtual_file_system.open(&path, flags, task))?;

        Ok(file_identifier.into_synchronous_file())
    }

    pub fn set_position(&mut self, position: &Position) -> Result<Size> {
        self.position = self
            .item
            .as_base_operations()
            .ok_or(Error::UnsupportedOperation)?
            .set_position(&mut self.context, self.position, position)?;

        Ok(self.position)
    }

    // - Operations

    pub fn write(&mut self, buffer: &[u8]) -> Result<usize> {
        if !self.flags.get_access().contains(AccessFlags::Write) {
            return Err(Error::InvalidMode);
        }

        let size = self
            .item
            .as_base_operations()
            .ok_or(Error::UnsupportedOperation)?
            .write(&mut self.context, buffer, self.position)?;

        self.position += size as Size;

        Ok(size)
    }

    pub fn write_vectored(&mut self, buffers: &[&[u8]]) -> Result<usize> {
        if !self.flags.get_access().contains(AccessFlags::Write) {
            return Err(Error::InvalidMode);
        }

        let size = self
            .item
            .as_base_operations()
            .ok_or(Error::UnsupportedOperation)?
            .write_vectored(&mut self.context, buffers, self.position)?;

        self.position += size as Size;

        Ok(size)
    }

    pub fn write_line(&mut self, buffer: &[u8]) -> Result<usize> {
        let size = self.write(buffer)? + self.write(b"\n")?;

        Ok(size)
    }

    pub fn read(&mut self, buffer: &mut [u8]) -> Result<usize> {
        let size = self
            .item
            .as_base_operations()
            .ok_or(Error::UnsupportedOperation)?
            .read(&mut self.context, buffer, self.position)?;

        self.position += size as Size;

        Ok(size)
    }

    pub fn read_until(&mut self, buffer: &mut [u8], delimiter: &[u8]) -> Result<usize> {
        let size = self
            .item
            .as_base_operations()
            .ok_or(Error::UnsupportedOperation)?
            .read_until(&mut self.context, buffer, self.position, delimiter)?;

        self.position += size as Size;

        Ok(size)
    }

    pub fn read_to_end(&mut self, buffer: &mut Vec<u8>, chunk_size: usize) -> Result<usize> {
        let mut total_read_size = 0;

        let mut chunk = vec![0u8; chunk_size];
        loop {
            let read_size = self
                .item
                .as_base_operations()
                .ok_or(Error::UnsupportedOperation)?
                .read(&mut self.context, &mut chunk, self.position)?;

            if read_size == 0 {
                break;
            }

            self.position += read_size as Size;

            total_read_size += read_size;

            buffer.extend_from_slice(&chunk[..read_size]);

            // Yield to allow other tasks to run.
            //yield_now().await;
        }

        Ok(total_read_size)
    }

    pub fn flush(&mut self) -> Result<()> {
        self.item
            .as_base_operations()
            .ok_or(Error::UnsupportedOperation)?
            .flush(&mut self.context)?;

        Ok(())
    }

    pub fn duplicate(&self) -> Result<Self> {
        let context = self
            .item
            .as_base_operations()
            .ok_or(Error::UnsupportedOperation)?
            .clone_context(&self.context)?;

        Ok(Self {
            virtual_file_system: self.virtual_file_system,
            item: self.item.clone(),
            position: self.position,
            flags: self.flags,
            context,
        })
    }

    pub fn get_statistics(&mut self) -> Result<Statistics> {
        let mut attributes = Attributes::new().set_mask(AttributeFlags::All);

        self.item
            .as_attributes_operations()
            .ok_or(Error::UnsupportedOperation)?
            .get_attributes(&mut self.context, &mut attributes)?;

        Statistics::from_attributes(&attributes).ok_or(Error::MissingAttribute)
    }

    pub fn set_owner(
        &mut self,
        user: Option<UserIdentifier>,
        group: Option<GroupIdentifier>,
    ) -> Result<()> {
        let mut attributes = Attributes::new();

        if let Some(user) = user {
            attributes = attributes.set_user(user);
        }

        if let Some(group) = group {
            attributes = attributes.set_group(group);
        }

        self.item
            .as_attributes_operations()
            .ok_or(Error::UnsupportedOperation)?
            .set_attributes(&mut self.context, &attributes)?;

        Ok(())
    }

    pub fn set_permissions(&mut self, permissions: Permissions) -> Result<()> {
        let attributes = Attributes::new().set_permissions(permissions);

        self.item
            .as_attributes_operations()
            .ok_or(Error::UnsupportedOperation)?
            .set_attributes(&mut self.context, &attributes)?;

        Ok(())
    }

    pub fn control<C>(&mut self, _command: C, argument: &C::Input) -> Result<C::Output>
    where
        C: ControlCommand,
        C::Output: Default,
    {
        let mut output = C::Output::default();

        self.item
            .as_base_operations()
            .ok_or(Error::UnsupportedOperation)?
            .control(
                &mut self.context,
                C::IDENTIFIER,
                AnyByLayout::from(argument),
                AnyByLayout::from_mutable(&mut output),
            )?;

        Ok(output)
    }

    pub fn get_access(&self) -> Result<AccessFlags> {
        Ok(self.flags.get_access())
    }

    pub fn get_state(&self) -> Result<StateFlags> {
        Ok(self.flags.get_state())
    }

    pub fn close_internal(&mut self, virtual_file_system: &VirtualFileSystem) -> crate::Result<()> {
        block_on(virtual_file_system.close(&self.item, &mut self.context))
    }

    pub fn close(mut self, virtual_file_system: &VirtualFileSystem) -> crate::Result<()> {
        let result = self.close_internal(virtual_file_system);
        forget(self);

        result
    }
}

impl Drop for SynchronousFile<'_> {
    fn drop(&mut self) {
        // Note: We cannot use async in Drop, so we just ignore errors here.
        let _ = self.close_internal(self.virtual_file_system).map_err(|e| {
            log::warning!("Failed to close SynchronousFile in Drop: {e}");
        });
    }
}
