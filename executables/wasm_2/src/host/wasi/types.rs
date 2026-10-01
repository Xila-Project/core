use core::fmt::Debug;

use crate::host::translation::{WasmPod, types::sealed::Sealed};

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Fdstat {
    pub filetype: u8,
    pub flags: u16,
    pub rights_base: u64,
    pub rights_inheriting: u64,
}

impl Sealed for Fdstat {}
impl WasmPod for Fdstat {}

/// Describes the type of pre-opened resource.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preopentype {
    Dir = 0,
}

/// Metadata payload when `tag == Preopentype::Dir`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PrestatDir {
    /// The byte length of the pre-opened directory's path name.
    pub pr_name_len: u32,
}

/// Information about a pre-opened resource.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Prestat {
    /// Discriminator identifying the payload variant.
    pub tag: u8,
    /// Union payload containing variant-specific data.
    pub u: PrestatUnion,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union PrestatUnion {
    pub dir: PrestatDir,
}

impl Debug for PrestatUnion {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        unsafe { write!(f, "PrestatUnion {{ dir: {:?} }}", self.dir) }
    }
}

impl Sealed for Prestat {}
impl WasmPod for Prestat {}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Filestat {
    pub dev: u64,
    pub ino: u64,
    pub filetype: u8,
    pub nlink: u64,
    pub size: u64,
    pub atime: u64,
    pub mtime: u64,
    pub ctime: u64,
}

impl Sealed for Filestat {}
impl WasmPod for Filestat {}
