extern crate alloc;

use alloc::boxed::Box;
use drivers_wasm::devices::DriveDevice;
use xila::{
    authentication,
    file_system::{
        self, DirectBlockDevice, XILA_DISK_SIGNATURE,
        mbr::{Mbr, PartitionKind, create_partition_device},
    },
    little_fs, users,
};

pub fn mount(drive: &'static DriveDevice) -> file_system::Result<little_fs::FileSystem> {
    let partition = if drive.is_new() {
        Mbr::format_disk_with_signature_and_partition(
            drive,
            XILA_DISK_SIGNATURE,
            PartitionKind::Xila,
        )
        .map_err(|_| file_system::Error::InvalidFile)?
    } else {
        let mbr = Mbr::read_from_device(drive).map_err(|_| file_system::Error::InvalidFile)?;
        let count = drive.get_block_count()?;
        // Check extents before MBR's overlap arithmetic; malformed entries
        // must neither overflow nor address bytes outside the disk.
        for entry in mbr.partitions.iter().filter(|entry| entry.is_valid()) {
            if entry.start_block == 0 || entry.start_block as u64 + entry.block_count as u64 > count
            {
                return Err(file_system::Error::InvalidFile);
            }
        }
        mbr.validate()
            .map_err(|_| file_system::Error::InvalidFile)?;
        if mbr.get_disk_signature() != XILA_DISK_SIGNATURE {
            return Err(file_system::Error::InvalidFile);
        }
        let entry = mbr
            .partitions
            .iter()
            .find(|entry| entry.is_valid() && entry.kind == PartitionKind::Xila)
            .ok_or(file_system::Error::InvalidFile)?;
        create_partition_device(drive, entry).map_err(|_| file_system::Error::InvalidFile)?
    };
    let partition = Box::leak(Box::new(partition));
    if drive.is_new() {
        little_fs::FileSystem::new_format(partition, 256)
    } else {
        // Never interpret a mount error as permission to erase saved data.
        little_fs::FileSystem::new(partition, 256)
    }
}

pub async fn initialize_users(
    context: &authentication::Context<'_>,
    new_disk: bool,
) -> authentication::Result<()> {
    if !new_disk {
        return authentication::load_all_users_and_groups(context).await;
    }
    let group = users::GroupIdentifier::new(1000);
    authentication::create_group(context, "administrator", Some(group)).await?;
    authentication::create_user(context, "administrator", "", group, None).await?;
    Ok(())
}
