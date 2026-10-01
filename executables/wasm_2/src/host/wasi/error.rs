use crate::host::wasi;

/// Preview 1 errno results are always a 32-bit ABI value for both guest pointer widths.
pub type WasiResult = i32;

pub enum CombinedError {
    Wasi(wasi::Error),
    Wasmi(wasmi::Error),
}

impl From<wasmi::Error> for CombinedError {
    fn from(e: wasmi::Error) -> Self {
        CombinedError::Wasmi(e)
    }
}

impl From<Error> for CombinedError {
    fn from(e: Error) -> Self {
        CombinedError::Wasi(e)
    }
}

macro_rules! wrap_function {
    ($body:expr) => {{
        // Evaluates $body directly inside a try-block pattern
        let result: Result<(), crate::host::wasi::error::CombinedError> = (|| $body)();

        match result {
            Ok(()) => Ok(0),
            Err(crate::host::wasi::error::CombinedError::Wasi(e)) => Ok(i32::from(e)),
            Err(crate::host::wasi::error::CombinedError::Wasmi(e)) => Err(e),
        }
    }};
}

pub(crate) use wrap_function;

pub type CombinedResult<T> = Result<T, CombinedError>;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// Argument list too long (`E2BIG`).
    Toobig = 1,
    /// Permission denied (`EACCES`).
    Access = 2,
    /// Address already in use (`EADDRINUSE`).
    Addrinuse = 3,
    /// Address not available (`EADDRNOTAVAIL`).
    Addrnotavail = 4,
    /// Address family not supported by protocol (`EAFNOSUPPORT`).
    Afnosupport = 5,
    /// Resource temporarily unavailable / Try again (`EAGAIN`).
    Again = 6,
    /// Operation already in progress (`EALREADY`).
    Already = 7,
    /// Bad file descriptor (`EBADF`).
    Badf = 8,
    /// Bad message (`EBADMSG`).
    Badmsg = 9,
    /// Device or resource busy (`EBUSY`).
    Busy = 10,
    /// Operation canceled (`ECANCELED`).
    Canceled = 11,
    /// No child processes (`ECHILD`).
    Child = 12,
    /// Software caused connection abort (`ECONNABORTED`).
    Connaborted = 13,
    /// Connection refused (`ECONNREFUSED`).
    Connrefused = 14,
    /// Connection reset by peer (`ECONNRESET`).
    Connreset = 15,
    /// Resource deadlock avoided (`EDEADLK`).
    Deadlk = 16,
    /// Destination address required (`EDESTADDRREQ`).
    Destaddrreq = 17,
    /// Mathematics argument out of domain of function (`EDOM`).
    Dom = 18,
    /// Disk quota exceeded (`EDQUOT`).
    Dquot = 19,
    /// File exists (`EEXIST`).
    Exist = 20,
    /// Bad address (`EFAULT`).
    Fault = 21,
    /// File too large (`EFBIG`).
    Fbig = 22,
    /// No route to host (`EHOSTUNREACH`).
    Hostunreach = 23,
    /// Identifier removed (`EIDRM`).
    Idrm = 24,
    /// Illegal byte sequence (`EILSEQ`).
    Ilseq = 25,
    /// Operation now in progress (`EINPROGRESS`).
    Inprogress = 26,
    /// Interrupted system call (`EINTR`).
    Intr = 27,
    /// Invalid argument (`EINVAL`).
    Inval = 28,
    /// I/O error (`EIO`).
    Io = 29,
    /// Transport endpoint is already connected (`EISCONN`).
    Isconn = 30,
    /// Is a directory (`EISDIR`).
    Isdir = 31,
    /// Too many levels of symbolic links (`ELOOP`).
    Loop = 32,
    /// Too many open files for this process (`EMFILE`).
    Mfile = 33,
    /// Too many links (`EMLINK`).
    Mlink = 34,
    /// Message too long (`EMSGSIZE`).
    Msgsize = 35,
    /// Multihop attempted (`EMULTIHOP`).
    Multihop = 36,
    /// File name too long (`ENAMETOOLONG`).
    Nametoolong = 37,
    /// Network is down (`ENETDOWN`).
    Netdown = 38,
    /// Network dropped connection on reset (`ENETRESET`).
    Netreset = 39,
    /// Network is unreachable (`ENETUNREACH`).
    Netunreach = 40,
    /// Too many open files in system (`ENFILE`).
    Nfile = 41,
    /// No buffer space available (`ENOBUFS`).
    Nobufs = 42,
    /// No such device (`ENODEV`).
    Nodev = 43,
    /// No such file or directory (`ENOENT`).
    Noent = 44,
    /// Exec format error (`ENOEXEC`).
    Noexec = 45,
    /// No locks available (`ENOLCK`).
    Nolck = 46,
    /// Link has been severed (`ENOLINK`).
    Nolink = 47,
    /// Out of memory (`ENOMEM`).
    Nomem = 48,
    /// No message of desired type (`ENOMSG`).
    Nomsg = 49,
    /// Protocol not available (`ENOPROTOOPT`).
    Noprotoopt = 50,
    /// No space left on device (`ENOSPC`).
    Nospc = 51,
    /// Function not implemented (`ENOSYS`).
    Nosys = 52,
    /// Transport endpoint is not connected (`ENOTCONN`).
    Notconn = 53,
    /// Not a directory (`ENOTDIR`).
    Notdir = 54,
    /// Directory not empty (`ENOTEMPTY`).
    Notempty = 55,
    /// State not recoverable (`ENOTRECOVERABLE`).
    Notrecoverable = 56,
    /// Socket operation on non-socket (`ENOTSOCK`).
    Notsock = 57,
    /// Operation not supported (`ENOTSUP`).
    Notsup = 58,
    /// Inappropriate ioctl for device (`ENOTTY`).
    Notty = 59,
    /// No such device or address (`ENXIO`).
    Nxio = 60,
    /// Value too large for defined data type (`EOVERFLOW`).
    Overflow = 61,
    /// Owner died (`EOWNERDEAD`).
    Ownerdead = 62,
    /// Operation not permitted (`EPERM`).
    Perm = 63,
    /// Broken pipe (`EPIPE`).
    Pipe = 64,
    /// Protocol error (`EPROTO`).
    Proto = 65,
    /// Protocol not supported (`EPROTONOSUPPORT`).
    Protonosupport = 66,
    /// Protocol wrong type for socket (`EPROTOTYPE`).
    Prototype = 67,
    /// Result too large or numerical result out of range (`ERANGE`).
    Range = 68,
    /// Read-only file system (`EROFS`).
    Rofs = 69,
    /// Invalid seek (`ESPIPE`).
    Spipe = 70,
    /// No such process (`ESRCH`).
    Srch = 71,
    /// Stale file handle (`ESTALE`).
    Stale = 72,
    /// Connection timed out (`ETIMEDOUT`).
    Timedout = 73,
    /// Text file busy (`ETXTBSY`).
    Txtbsy = 74,
    /// Cross-device link (`EXDEV`).
    Xdev = 75,
    /// Capabilities insufficient (`ENOTCAPABLE`).
    Notcapable = 76,
    /// Cannot send after socket shutdown (`ESHUTDOWN`).
    Shutdown = 77,
    /// Memory violation or illegal memory access.
    Memviolation = 78,
    /// Unknown or unspecified error.
    Unknown = 79,
}

impl From<Error> for i32 {
    fn from(err: Error) -> Self {
        err as i32
    }
}

pub fn vfs_error(err: xila::virtual_file_system::Error) -> Error {
    use xila::virtual_file_system::Error as Vfs;
    match err {
        Vfs::PermissionDenied => Error::Access,
        Vfs::NotADirectory => Error::Notdir,
        Vfs::InvalidPath => Error::Inval,
        Vfs::AlreadyExists => Error::Exist,
        Vfs::RessourceBusy => Error::Again,
        Vfs::TooManyOpenFiles => Error::Nfile,
        Vfs::InvalidIdentifier => Error::Badf,
        _ => Error::Io,
    }
}
