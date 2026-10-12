use crate::host::translation::{GuestValue, sealed::Sealed};

/// Logical host value; its guest encoding is always the 24-byte Preview 1 layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fdstat {
    pub filetype: u8,
    pub flags: u16,
    pub rights_base: u64,
    pub rights_inheriting: u64,
}

impl Sealed for Fdstat {}
impl GuestValue for Fdstat {
    const SIZE: usize = 24;

    fn decode(bytes: &[u8]) -> Self {
        Self {
            filetype: bytes[0],
            flags: u16::decode(&bytes[2..4]),
            rights_base: u64::decode(&bytes[8..16]),
            rights_inheriting: u64::decode(&bytes[16..24]),
        }
    }

    fn encode(self, bytes: &mut [u8]) {
        bytes.fill(0);
        bytes[0] = self.filetype;
        self.flags.encode(&mut bytes[2..4]);
        self.rights_base.encode(&mut bytes[8..16]);
        self.rights_inheriting.encode(&mut bytes[16..24]);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrestatDir {
    pub pr_name_len: u32,
}

/// The directory variant is represented directly; no native union is written to memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Prestat {
    pub tag: u8,
    pub dir: PrestatDir,
}

impl Sealed for Prestat {}
impl GuestValue for Prestat {
    const SIZE: usize = 8;

    fn decode(bytes: &[u8]) -> Self {
        Self {
            tag: bytes[0],
            dir: PrestatDir {
                pr_name_len: u32::decode(&bytes[4..8]),
            },
        }
    }

    fn encode(self, bytes: &mut [u8]) {
        bytes.fill(0);
        bytes[0] = self.tag;
        self.dir.pr_name_len.encode(&mut bytes[4..8]);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
impl GuestValue for Filestat {
    const SIZE: usize = 64;

    fn decode(bytes: &[u8]) -> Self {
        Self {
            dev: u64::decode(&bytes[0..8]),
            ino: u64::decode(&bytes[8..16]),
            filetype: bytes[16],
            nlink: u64::decode(&bytes[24..32]),
            size: u64::decode(&bytes[32..40]),
            atime: u64::decode(&bytes[40..48]),
            mtime: u64::decode(&bytes[48..56]),
            ctime: u64::decode(&bytes[56..64]),
        }
    }

    fn encode(self, bytes: &mut [u8]) {
        bytes.fill(0);
        self.dev.encode(&mut bytes[0..8]);
        self.ino.encode(&mut bytes[8..16]);
        bytes[16] = self.filetype;
        self.nlink.encode(&mut bytes[24..32]);
        self.size.encode(&mut bytes[32..40]);
        self.atime.encode(&mut bytes[40..48]);
        self.mtime.encode(&mut bytes[48..56]);
        self.ctime.encode(&mut bytes[56..64]);
    }
}
