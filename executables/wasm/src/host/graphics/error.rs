//! Graphics ABI status codes.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum GraphicsError {
    InvalidArgumentsCount = 1,
    InvalidMemory = 2,
    Unavailable = 3,
    Unsupported = 4,
    InvalidHandle = 5,
    ResourceLimit = 6,
    InvalidValue = 7,
    NativeFailure = 8,
}

pub type Result<T> = core::result::Result<T, GraphicsError>;
