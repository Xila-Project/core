use std::{env, error::Error, path::PathBuf, process};

use drivers_std::drive_file::FileDriveDevice;
use file_system::{
    DirectBaseOperations, XILA_DISK_SIGNATURE,
    mbr::{Mbr, PartitionKind, create_partition_device},
};

const DEFAULT_SIZE: u64 = 16 * 1024 * 1024;
const CACHE_SIZE: usize = 256;
const BLOCK_SIZE: u64 = 512;

fn main() {
    drivers_std::memory::instantiate_global_allocator!();
    // Pull the supported ABI memory exports into the host executable. LittleFS
    // calls these C ABI symbols from its allocator hooks.
    std::hint::black_box(abi_definitions::xila_memory_allocate as *const ());
    std::hint::black_box(abi_definitions::xila_memory_deallocate as *const ());
    std::hint::black_box(&abi_definitions::XILA_MEMORY_CAPABILITIES_NONE as *const _ as usize);
    if let Err(error) = run() {
        eprintln!("fs_image: {error}");
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut arguments = env::args_os().skip(1);
    let output = arguments
        .next()
        .map(PathBuf::from)
        .ok_or("usage: fs_image <output-image> [size-bytes] (size defaults to 16777216)")?;
    let size = match arguments.next() {
        Some(value) => value.to_string_lossy().parse::<u64>()?,
        None => DEFAULT_SIZE,
    };
    if arguments.next().is_some() {
        return Err("too many arguments".into());
    }
    if size % BLOCK_SIZE != 0 || size / BLOCK_SIZE < 2048 {
        return Err("disk size must be a multiple of 512 bytes and at least 1048576 bytes".into());
    }
    let output = output.canonicalize_parent()?;
    let output_string = output
        .to_str()
        .ok_or("output image path must be valid UTF-8")?;

    create_output_file(&output, size)?;

    {
        let drive = Box::leak(Box::new(FileDriveDevice::new(&output_string, size)));
        let partition = Mbr::format_disk_with_signature_and_partition(
            drive,
            XILA_DISK_SIGNATURE,
            PartitionKind::Xila,
        )?;
        let partition = Box::leak(Box::new(partition));
        let file_system = little_fs::FileSystem::new_format(partition, CACHE_SIZE)?;
        drop(file_system);
        drive.flush()?;
    }

    {
        let drive = Box::leak(Box::new(FileDriveDevice::new(&output_string, size)));
        let mbr = Mbr::read_from_device(drive)?;
        mbr.validate()?;
        if mbr.get_disk_signature() != XILA_DISK_SIGNATURE {
            return Err("MBR disk signature does not match Xila".into());
        }
        let entry = mbr
            .partitions
            .iter()
            .find(|entry| entry.is_valid() && entry.kind == PartitionKind::Xila)
            .ok_or("MBR has no valid Xila partition")?;
        let partition = create_partition_device(drive, entry)?;
        let partition = Box::leak(Box::new(partition));
        let file_system = little_fs::FileSystem::new(partition, CACHE_SIZE)?;
        drop(file_system);
        drive.flush()?;
    }

    println!(
        "Validated Xila MBR and mounted LittleFS: {} ({} bytes)",
        output.display(),
        size
    );
    Ok(())
}

fn create_output_file(path: &std::path::Path, size: u64) -> std::io::Result<()> {
    if path.to_str().is_none() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "output image path must be valid UTF-8",
        ));
    }
    // Exclusive creation prevents accidental replacement of an existing image.
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?
        .set_len(size)
}

trait CanonicalizeParent {
    fn canonicalize_parent(&self) -> Result<PathBuf, Box<dyn Error>>;
}

impl CanonicalizeParent for PathBuf {
    fn canonicalize_parent(&self) -> Result<PathBuf, Box<dyn Error>> {
        let parent = self
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(std::path::Path::new("."));
        let parent = parent.canonicalize()?;
        Ok(parent.join(self.file_name().ok_or("output path must name a file")?))
    }
}

#[cfg(test)]
mod tests {
    use super::create_output_file;
    use std::{fs, io::Write};

    #[cfg(unix)]
    #[test]
    fn rejects_non_utf8_output_before_creating_file() {
        use std::os::unix::ffi::OsStringExt;

        let mut name = format!(
            "fs-image-invalid-{}-{}.img",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )
        .into_bytes();
        name.push(0xff);
        let path = std::env::temp_dir().join(std::ffi::OsString::from_vec(name));

        let result = create_output_file(&path, 16 * 1024 * 1024);

        let error = result.unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("valid UTF-8"));
        assert!(!path.exists());
    }

    #[test]
    fn refuses_existing_output_without_modifying_it() {
        let path = std::env::temp_dir().join(format!(
            "fs-image-existing-{}-{}.img",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut existing = fs::File::create(&path).unwrap();
        existing.write_all(b"keep this installation").unwrap();
        drop(existing);

        let result = create_output_file(&path, 16 * 1024 * 1024);

        assert!(result.is_err());
        assert_eq!(fs::read(&path).unwrap(), b"keep this installation");
        fs::remove_file(path).unwrap();
    }
}
