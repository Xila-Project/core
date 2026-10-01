use alloc::{collections::btree_map::BTreeMap, string::String, vec::Vec};
use xila::{
    file_system::PathOwned,
    task::TaskIdentifier,
    virtual_file_system::{SynchronousDirectory, SynchronousFile},
};

pub struct WasiContext {
    pub files: BTreeMap<u32, FileSystemItem>,
    pub next_fd: u32,
    pub arguments: Vec<String>,
    pub environment: Vec<(String, String)>,
    pub task: TaskIdentifier,
    pub random_state: u64,
    pub prestats: Vec<Prestat>,
    pub exit_code: Option<i32>,
}

impl WasiContext {
    /// Allocate and insert the next available descriptor.
    pub fn insert_file_system_item(&mut self, item: FileSystemItem) -> Option<u32> {
        let fd = allocate_descriptor_id(&mut self.next_fd)?;
        self.files.insert(fd, item);
        Some(fd)
    }

    pub fn get_file_system_item(&mut self, fd: u32) -> Option<&mut FileSystemItem> {
        self.files.get_mut(&fd)
    }

    pub fn get_synchronous_file(&mut self, fd: u32) -> Option<&mut SynchronousFile> {
        self.files
            .get_mut(&fd)
            .and_then(|item| item.into_synchronous_file())
    }

    pub fn is_directory(&self, fd: u32) -> bool {
        matches!(self.files.get(&fd), Some(FileSystemItem::Directory(_)))
    }

    pub fn get_synchronous_directory(&mut self, fd: u32) -> Option<&mut SynchronousDirectory> {
        self.files
            .get_mut(&fd)
            .and_then(|item| item.into_synchronous_directory())
    }

    pub fn pop_file_system_item(&mut self, fd: u32) -> Option<FileSystemItem> {
        self.files.remove(&fd)
    }
}

pub struct Prestat {
    pub name: Vec<u8>,
}

pub struct FileVariant {
    pub file: SynchronousFile,
}

pub struct DirectoryVariant {
    pub path: PathOwned,
    pub directory: SynchronousDirectory,
}

pub enum FileSystemItem {
    StandardInput(FileVariant),
    StandardOutput(FileVariant),
    StandardError(FileVariant),
    File(FileVariant),
    Directory(DirectoryVariant),
}

impl FileSystemItem {
    pub fn file_type(&self) -> u8 {
        match self {
            FileSystemItem::StandardInput(_)
            | FileSystemItem::StandardOutput(_)
            | FileSystemItem::StandardError(_) => descriptor_type(DescriptorKind::CharacterDevice),
            FileSystemItem::File(_) => descriptor_type(DescriptorKind::RegularFile),
            FileSystemItem::Directory(_) => descriptor_type(DescriptorKind::Directory),
        }
    }

    pub fn into_synchronous_file(&mut self) -> Option<&mut SynchronousFile> {
        match self {
            FileSystemItem::StandardInput(file) => Some(&mut file.file),
            FileSystemItem::StandardOutput(file) => Some(&mut file.file),
            FileSystemItem::StandardError(file) => Some(&mut file.file),
            FileSystemItem::File(file) => Some(&mut file.file),
            FileSystemItem::Directory(_) => None,
        }
    }

    pub fn into_synchronous_directory(&mut self) -> Option<&mut SynchronousDirectory> {
        match self {
            FileSystemItem::Directory(dir) => Some(&mut dir.directory),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
enum DescriptorKind {
    CharacterDevice,
    RegularFile,
    Directory,
}

fn descriptor_type(kind: DescriptorKind) -> u8 {
    match kind {
        DescriptorKind::CharacterDevice => 2,
        DescriptorKind::Directory => 3,
        DescriptorKind::RegularFile => 4,
    }
}

fn allocate_descriptor_id(next_fd: &mut u32) -> Option<u32> {
    let fd = *next_fd;
    *next_fd = next_fd.checked_add(1)?;
    Some(fd)
}

#[cfg(test)]
mod tests {
    #[test]
    fn descriptor_ids_increase_without_wrapping() {
        let mut next_fd = 3u32;
        assert_eq!(super::allocate_descriptor_id(&mut next_fd), Some(3));
        assert_eq!(super::allocate_descriptor_id(&mut next_fd), Some(4));

        next_fd = u32::MAX;
        assert_eq!(super::allocate_descriptor_id(&mut next_fd), None);
    }

    #[test]
    fn descriptor_kinds_match_wasi_filetype_values() {
        assert_eq!(
            super::descriptor_type(super::DescriptorKind::RegularFile),
            4
        );
        assert_eq!(super::descriptor_type(super::DescriptorKind::Directory), 3);
        assert_eq!(
            super::descriptor_type(super::DescriptorKind::CharacterDevice),
            2
        );
    }
}
