use core::mem::ManuallyDrop;

use alloc::boxed::Box;
use alloc::string::String;
use xila::abi_declarations::{XilaFileSystemDirectory, XilaFileSystemFile};
use xila::file_system::{Path, PathOwned};
use xila::log;
use xila::shared::generate_shadow_type;
use xila::virtual_file_system::{SynchronousDirectory, SynchronousFile};

#[derive(PartialEq, Eq)]
pub enum FileVariantKind {
    Regular,
    StandardInput,
    StandardOutput,
    StandardError,
}

pub struct FileVariant {
    pub file: ManuallyDrop<SynchronousFile<'static>>,
    pub kind: FileVariantKind,
}

impl AsMut<XilaFileSystemFile> for FileVariant {
    #[inline]
    fn as_mut(&mut self) -> &mut XilaFileSystemFile {
        unsafe {
            &mut *(&mut *self.file as *mut SynchronousFile<'static> as *mut XilaFileSystemFile)
        }
    }
}

pub struct DirectoryVariant {
    pub path: PathOwned,
    pub directory: ManuallyDrop<SynchronousDirectory<'static>>,
}

impl AsMut<XilaFileSystemDirectory> for DirectoryVariant {
    #[inline]
    fn as_mut(&mut self) -> &mut XilaFileSystemDirectory {
        unsafe {
            &mut *(&mut *self.directory as *mut SynchronousDirectory<'static>
                as *mut XilaFileSystemDirectory)
        }
    }
}

pub enum FileSystemItem {
    File(FileVariant),
    Directory(DirectoryVariant),
}

impl FileSystemItem {
    pub fn new_file(file: SynchronousFile<'static>, kind: FileVariantKind) -> *mut Self {
        Box::into_raw(Box::new(FileSystemItem::File(FileVariant {
            file: ManuallyDrop::new(file),
            kind,
        })))
    }

    pub fn new_directory(directory: SynchronousDirectory<'static>, path: PathOwned) -> *mut Self {
        let item = FileSystemItem::Directory(DirectoryVariant {
            path,
            directory: ManuallyDrop::new(directory),
        });

        Box::into_raw(Box::new(item))
    }

    /// # Safety
    /// The caller must ensure that the provided pointer is valid and points to a properly initialized `FileSystemItem`.
    pub unsafe fn borrow_from_raw<'a>(ptr: *mut Self) -> &'a mut Self {
        unsafe { &mut *ptr }
    }

    /// # Safety
    /// The caller must ensure that the provided pointer is valid and points to a properly initialized `
    pub unsafe fn own_from_raw(ptr: *mut Self) -> Box<Self> {
        unsafe { Box::from_raw(ptr) }
    }

    pub fn as_file(&self) -> Option<&FileVariant> {
        match self {
            FileSystemItem::File(file) => Some(file),
            _ => None,
        }
    }

    pub fn as_directory(&self) -> Option<&DirectoryVariant> {
        match self {
            FileSystemItem::Directory(directory) => Some(directory),
            _ => None,
        }
    }

    pub fn resolve(&self, path: impl AsRef<Path>) -> Option<PathOwned> {
        let parent_path = self.as_directory()?.path.as_str();

        let capacity = parent_path.len() + path.as_ref().as_str().len() + 2; // +1 for the separator, +1 for potential null terminator
        let mut resolved_path = String::with_capacity(capacity);

        resolved_path.push_str(parent_path);
        if !parent_path.ends_with('/') {
            resolved_path.push('/');
        }
        resolved_path.push_str(path.as_ref().as_str());
        resolved_path.push('\0'); // Null terminator for C compatibility
        resolved_path.truncate(resolved_path.len() - 1); // Remove the null terminator for Rust usage

        log::information!(
            "Computed capacity {}, real size {}, and resolved path {:?} from parent path {:?} and input path {:?}",
            capacity,
            resolved_path.capacity(),
            resolved_path,
            parent_path,
            path.as_ref().as_str()
        );

        PathOwned::new(resolved_path)
    }
}

generate_shadow_type!(XilaFileSystemItem, FileSystemItem);
