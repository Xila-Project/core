use file_system::{AccessFlags, CreateFlags, Flags, Path};
use std::io;
use std::prelude::rust_2024::*;
use std::{fs::File, io::Read, path};
use virtual_file_system::VirtualFileSystem;

#[derive(Debug)]
pub enum Error {
    VirtualFileSystemError(virtual_file_system::Error),
    IoError(io::Error),
}

impl From<virtual_file_system::Error> for Error {
    fn from(error: virtual_file_system::Error) -> Self {
        Error::VirtualFileSystemError(error)
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Error::IoError(error)
    }
}

pub type Result<T> = core::result::Result<T, Error>;

pub async fn load_to_virtual_file_system(
    task_manager: &'static task::Manager,
    virtual_file_system: &VirtualFileSystem,
    source_path: impl AsRef<path::Path>,
    destination_path: impl AsRef<Path>,
) -> Result<()> {
    // Open file for reading on host
    let mut source_file = File::open(source_path.as_ref())?;

    let task = task_manager.get_current_task_identifier().await;

    let mut file = virtual_file_system::File::open(
        virtual_file_system,
        task,
        &destination_path.as_ref(),
        Flags::new(AccessFlags::Write, Some(CreateFlags::CREATE_TRUNCATE), None),
    )
    .await?;

    // Read and write file content block by block
    let mut buffer = [0; 512];

    loop {
        let bytes_read = source_file.read(&mut buffer)?;

        if bytes_read == 0 {
            break;
        }

        file.write(&buffer[..bytes_read]).await?;
    }

    file.close(virtual_file_system).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate alloc;
    use task::test;

    #[test(executor = crate::executor::Executor::new_static())]
    async fn test_loader() {
        // - Load the file in the file system
        let source_path = "Cargo.toml";
        let destination_path = "/Cargo.toml";

        let device = file_system::MemoryDevice::<512>::new_static(1024 * 1024 * 512);

        let task_instance = Box::leak(Box::new(task::Manager::new()));
        let users_manager = Box::leak(Box::new(users::Manager::new()));
        let time_manager = Box::leak(Box::new(
            time::Manager::new(&crate::devices::TimeDevice).unwrap(),
        ));

        let task = task_instance.get_current_task_identifier().await;

        little_fs::FileSystem::format(device, 256).unwrap();
        let file_system = little_fs::FileSystem::new(device, 256).unwrap();

        let virtual_file_system = virtual_file_system::initialize(
            task_instance,
            users_manager,
            time_manager,
            file_system,
        )
        .unwrap();

        load_to_virtual_file_system(
            task_instance,
            &virtual_file_system,
            source_path,
            destination_path,
        )
        .await
        .unwrap();

        // - Read the file and compare it with the original
        let test_file = std::fs::read_to_string(source_path).unwrap();

        let mut buffer = vec![0; test_file.len()];

        virtual_file_system::File::read_from_path(
            &virtual_file_system,
            task,
            &destination_path,
            &mut buffer,
        )
        .await
        .unwrap();

        assert_eq!(buffer, test_file.as_bytes());
    }
}
