//! Bounded, allocation-free vectored I/O between a file and guest memory.
//!
//! Only translated guest ranges are involved, so nothing here depends on Wasmi or the VFS. The
//! iovec table is copied to a small stack snapshot (the guest may point a buffer at its own
//! table) and the data itself is handed straight to the caller-provided reader/writer, never
//! staged in an intermediate buffer.

use crate::host::{
    translation::{WasiVector, WasmUsize},
    wasi::{Error, guest::guest_slice},
};

/// Largest iovec table accepted from a guest. Keeps the snapshot on the stack.
pub const MAXIMUM_IOVECS: usize = 16;

/// Stack snapshot of a validated guest iovec table.
pub struct IoVectors {
    entries: [WasiVector; MAXIMUM_IOVECS],
    count: usize,
}

impl IoVectors {
    pub fn as_slice(&self) -> &[WasiVector] {
        &self.entries[..self.count]
    }
}

/// Copies the iovec table at `table` (`count` entries) out of guest memory.
///
/// Fails with `EINVAL` if `count` exceeds [`MAXIMUM_IOVECS`], and with `EFAULT` if the table or
/// any buffer it describes is not entirely inside guest memory, so that no byte is transferred
/// by a call that is going to fail.
pub fn decode_iovecs(
    memory: &mut [u8],
    table: WasmUsize,
    count: WasmUsize,
) -> Result<IoVectors, Error> {
    let count = usize::try_from(count).map_err(|_| Error::Inval)?;
    if count > MAXIMUM_IOVECS {
        return Err(Error::Inval);
    }

    let table = guest_slice::<WasiVector>(memory, table, count as WasmUsize)?;

    let mut vectors = IoVectors {
        entries: [WasiVector {
            buffer: 0,
            length: 0,
        }; MAXIMUM_IOVECS],
        count,
    };
    vectors.entries[..count].copy_from_slice(table);

    // Every buffer must be valid before any of them is used.
    for vector in vectors.as_slice() {
        guest_slice::<u8>(memory, vector.buffer, vector.length)?;
    }

    Ok(vectors)
}

/// Offers each non-empty buffer to `operation` in order and sums what it transferred.
///
/// Stops after the first short transfer. If `operation` fails after some bytes were transferred,
/// those bytes are reported and the failure is dropped (it resurfaces on the next call if it is
/// persistent); if nothing was transferred the error is returned.
fn transfer(
    memory: &mut [u8],
    vectors: &IoVectors,
    mut operation: impl FnMut(&mut [u8]) -> Result<usize, Error>,
) -> Result<usize, Error> {
    let mut total = 0usize;

    for vector in vectors.as_slice() {
        let buffer = guest_slice::<u8>(memory, vector.buffer, vector.length)?;
        if buffer.is_empty() {
            continue;
        }
        let requested = buffer.len();

        match operation(buffer) {
            Ok(count) => {
                total = total.saturating_add(count);
                if count < requested {
                    break;
                }
            }
            Err(error) if total == 0 => return Err(error),
            Err(_) => break,
        }
    }

    Ok(total)
}

/// Fills the described guest buffers in order; `read` is handed each buffer directly.
///
/// See [`transfer`] for how short transfers and errors are treated.
pub fn read_vectored(
    memory: &mut [u8],
    vectors: &IoVectors,
    read: impl FnMut(&mut [u8]) -> Result<usize, Error>,
) -> Result<usize, Error> {
    transfer(memory, vectors, read)
}

/// Hands the described guest buffers in order to `write`, which sees the guest bytes themselves.
///
/// See [`transfer`] for how short transfers and errors are treated.
pub fn write_vectored(
    memory: &mut [u8],
    vectors: &IoVectors,
    mut write: impl FnMut(&[u8]) -> Result<usize, Error>,
) -> Result<usize, Error> {
    transfer(memory, vectors, |buffer| write(buffer))
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;
    use core::mem::size_of;

    use super::{super::guest::testing::Memory, *};

    const WORD: usize = size_of::<WasmUsize>();

    /// Where tests put the iovec table: aligned, and away from the null page (a non-empty range
    /// at offset 0 is a null pointer and never valid).
    const TABLE: usize = 16;

    /// Writes an iovec table of `(buffer, length)` pairs at `table`.
    fn put_iovecs(memory: &mut [u8], table: usize, entries: &[(usize, usize)]) {
        for (index, (buffer, length)) in entries.iter().enumerate() {
            let at = table + index * 2 * WORD;
            memory[at..at + WORD].copy_from_slice(&(*buffer as WasmUsize).to_le_bytes());
            memory[at + WORD..at + 2 * WORD].copy_from_slice(&(*length as WasmUsize).to_le_bytes());
        }
    }

    fn decode(memory: &mut [u8], table: usize, count: usize) -> Result<IoVectors, Error> {
        decode_iovecs(memory, table as WasmUsize, count as WasmUsize)
    }

    /// Reader that hands out `source` and records the address of every buffer it was given.
    fn reader<'a>(
        source: &'a [u8],
        seen: &'a mut Vec<usize>,
    ) -> impl FnMut(&mut [u8]) -> Result<usize, Error> + 'a {
        let mut remaining = source;
        move |buffer: &mut [u8]| {
            assert!(
                !buffer.is_empty(),
                "empty buffers must not reach the reader"
            );
            seen.push(buffer.as_ptr() as usize);
            let count = buffer.len().min(remaining.len());
            buffer[..count].copy_from_slice(&remaining[..count]);
            remaining = &remaining[count..];
            Ok(count)
        }
    }

    #[test]
    fn decode_accepts_zero_and_sixteen_entries() {
        let mut backing = Memory::new();
        let memory = &mut backing.0[..];
        let entries: Vec<(usize, usize)> = (0..16).map(|index| (400 + index * 4, 4)).collect();
        put_iovecs(memory, TABLE, &entries);

        // An empty table needs no memory at all, not even a non-null pointer.
        assert_eq!(decode(memory, 0, 0).unwrap().as_slice().len(), 0);
        assert_eq!(decode(memory, TABLE, 0).unwrap().as_slice().len(), 0);

        let vectors = decode(memory, TABLE, 16).unwrap();
        assert_eq!(vectors.as_slice().len(), 16);
        assert_eq!(vectors.as_slice()[0].buffer as usize, 400);
        assert_eq!(vectors.as_slice()[15].buffer as usize, 400 + 15 * 4);
        assert_eq!(vectors.as_slice()[15].length as usize, 4);
    }

    #[test]
    fn decode_rejects_more_than_sixteen_entries_with_einval() {
        let mut backing = Memory::new();
        let memory = &mut backing.0[..];

        assert_eq!(decode(memory, TABLE, 17).err(), Some(Error::Inval));
        assert_eq!(
            decode(memory, TABLE, u32::MAX as usize).err(),
            Some(Error::Inval)
        );
    }

    #[test]
    fn decode_rejects_tables_outside_memory_with_efault() {
        let mut backing = Memory::new();
        let memory = &mut backing.0[..];

        // A non-empty table at the null offset, and one that runs past the end of memory.
        assert_eq!(decode(memory, 0, 1).err(), Some(Error::Fault));
        assert_eq!(decode(memory, 504, 2).err(), Some(Error::Fault));
    }

    #[test]
    fn decode_rejects_a_bad_buffer_anywhere_in_the_table_with_efault() {
        let mut backing = Memory::new();
        let memory = &mut backing.0[..];

        // Null with a length, starting outside memory, running past the end, overflowing. The
        // bad buffer is the second entry: the whole call must be known to be valid before any
        // byte is transferred.
        for bad in [(0, 4), (1000, 4), (508, 8), (8, u32::MAX as usize)] {
            put_iovecs(memory, TABLE, &[(100, 4), bad]);
            assert_eq!(
                decode(memory, TABLE, 2).err(),
                Some(Error::Fault),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn read_fills_buffers_in_order_and_reports_the_total() {
        let mut backing = Memory::new();
        let memory = &mut backing.0[..];
        put_iovecs(memory, TABLE, &[(100, 4), (200, 6)]);
        let vectors = decode(memory, TABLE, 2).unwrap();
        let mut seen = Vec::new();

        let total = read_vectored(memory, &vectors, reader(b"abcdefghij", &mut seen));

        assert_eq!(total, Ok(10));
        assert_eq!(&memory[100..104], b"abcd");
        assert_eq!(&memory[200..206], b"efghij");
    }

    #[test]
    fn read_hands_the_reader_the_guest_buffers_themselves() {
        let mut backing = Memory::new();
        let memory = &mut backing.0[..];
        put_iovecs(memory, TABLE, &[(100, 4), (200, 6)]);
        let vectors = decode(memory, TABLE, 2).unwrap();
        let base = memory.as_ptr() as usize;
        let mut seen = Vec::new();

        read_vectored(memory, &vectors, reader(b"abcdefghij", &mut seen)).unwrap();

        assert_eq!(seen, [base + 100, base + 200]);
    }

    #[test]
    fn read_stops_after_a_short_read() {
        let mut backing = Memory::new();
        let memory = &mut backing.0[..];
        put_iovecs(memory, TABLE, &[(100, 4), (200, 4), (300, 4)]);
        let vectors = decode(memory, TABLE, 3).unwrap();
        let mut seen = Vec::new();

        let total = read_vectored(memory, &vectors, reader(b"abcde", &mut seen));

        assert_eq!(total, Ok(5));
        assert_eq!(seen.len(), 2, "the third buffer must not be offered");
        assert_eq!(&memory[200..204], b"e\0\0\0");
        assert_eq!(&memory[300..304], [0; 4]);
    }

    #[test]
    fn read_skips_zero_length_buffers() {
        let mut backing = Memory::new();
        let memory = &mut backing.0[..];
        put_iovecs(memory, TABLE, &[(100, 0), (200, 3)]);
        let vectors = decode(memory, TABLE, 2).unwrap();
        let mut seen = Vec::new();

        let total = read_vectored(memory, &vectors, reader(b"abc", &mut seen));

        assert_eq!(total, Ok(3));
        assert_eq!(&memory[200..203], b"abc");
    }

    #[test]
    fn read_reports_bytes_already_read_when_a_later_read_fails() {
        let mut backing = Memory::new();
        let memory = &mut backing.0[..];
        put_iovecs(memory, TABLE, &[(100, 4), (200, 4)]);
        let vectors = decode(memory, TABLE, 2).unwrap();
        let mut calls = 0;

        let total = read_vectored(memory, &vectors, |buffer: &mut [u8]| {
            calls += 1;
            if calls == 1 {
                buffer.fill(b'x');
                Ok(buffer.len())
            } else {
                Err(Error::Io)
            }
        });

        assert_eq!(total, Ok(4));
    }

    #[test]
    fn read_returns_the_error_when_nothing_was_read() {
        let mut backing = Memory::new();
        let memory = &mut backing.0[..];
        put_iovecs(memory, TABLE, &[(100, 4)]);
        let vectors = decode(memory, TABLE, 1).unwrap();

        let total = read_vectored(memory, &vectors, |_: &mut [u8]| Err(Error::Io));

        assert_eq!(total, Err(Error::Io));
    }

    #[test]
    fn read_uses_the_table_snapshot_when_a_buffer_overwrites_the_table() {
        let mut backing = Memory::new();
        let memory = &mut backing.0[..];
        // The first buffer covers the whole table, including the entry describing the second one.
        let table_length = 2 * 2 * WORD;
        put_iovecs(memory, TABLE, &[(TABLE, table_length), (300, 4)]);
        let vectors = decode(memory, TABLE, 2).unwrap();
        let mut seen = Vec::new();

        let total = read_vectored(memory, &vectors, reader(&[0xAA; 64], &mut seen));

        assert_eq!(total, Ok(table_length + 4));
        assert!(
            memory[TABLE..TABLE + table_length]
                .iter()
                .all(|&byte| byte == 0xAA)
        );
        assert_eq!(&memory[300..304], [0xAA; 4]);
    }

    #[test]
    fn write_hands_the_writer_the_guest_buffers_themselves() {
        let mut backing = Memory::new();
        let memory = &mut backing.0[..];
        memory[100..104].copy_from_slice(b"abcd");
        memory[200..206].copy_from_slice(b"efghij");
        put_iovecs(memory, TABLE, &[(100, 4), (200, 6)]);
        let vectors = decode(memory, TABLE, 2).unwrap();
        let base = memory.as_ptr() as usize;
        let mut seen = Vec::new();
        let mut written = Vec::new();

        let total = write_vectored(memory, &vectors, |buffer: &[u8]| {
            seen.push((buffer.as_ptr() as usize, buffer.len()));
            written.extend_from_slice(buffer);
            Ok(buffer.len())
        });

        assert_eq!(total, Ok(10));
        assert_eq!(seen, [(base + 100, 4), (base + 200, 6)]);
        assert_eq!(written, b"abcdefghij");
    }

    #[test]
    fn write_stops_after_a_short_write_and_skips_empty_buffers() {
        let mut backing = Memory::new();
        let memory = &mut backing.0[..];
        put_iovecs(memory, TABLE, &[(100, 0), (200, 4), (300, 4)]);
        let vectors = decode(memory, TABLE, 3).unwrap();
        let mut calls = 0;

        let total = write_vectored(memory, &vectors, |buffer: &[u8]| {
            assert!(!buffer.is_empty());
            calls += 1;
            Ok(buffer.len() - 1)
        });

        assert_eq!(total, Ok(3));
        assert_eq!(calls, 1);
    }

    #[test]
    fn write_reports_progress_before_an_error_and_the_error_otherwise() {
        let mut backing = Memory::new();
        let memory = &mut backing.0[..];
        put_iovecs(memory, TABLE, &[(100, 4), (200, 4)]);
        let vectors = decode(memory, TABLE, 2).unwrap();
        let mut calls = 0;

        let after_progress = write_vectored(memory, &vectors, |buffer: &[u8]| {
            calls += 1;
            if calls == 1 {
                Ok(buffer.len())
            } else {
                Err(Error::Io)
            }
        });
        let immediately = write_vectored(memory, &vectors, |_: &[u8]| Err(Error::Pipe));

        assert_eq!(after_progress, Ok(4));
        assert_eq!(immediately, Err(Error::Pipe));
    }
}
