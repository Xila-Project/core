use xila::{
    file_system::{
        DirectBaseOperations, DirectCharacterDevice, Error, MountOperations, Position, Result, Size,
    },
    shared::utf8_chunks,
};

use crate::terminal::Terminal;

fn map_error(error: crate::Error) -> Error {
    match error {
        crate::Error::RessourceBusy => Error::RessourceBusy,
        _ => Error::InternalError,
    }
}

impl DirectBaseOperations for Terminal {
    fn read(&self, buffer: &mut [u8], _: Size) -> Result<usize> {
        let count = self.read_input(buffer).map_err(map_error)?;
        if count == 0 && !buffer.is_empty() {
            Err(Error::RessourceBusy)
        } else {
            Ok(count)
        }
    }

    fn write(&self, buffer: &[u8], _: Size) -> Result<usize> {
        let chunks = utf8_chunks(buffer);

        for string in chunks {
            self.print(string).map_err(map_error)?;
        }

        Ok(buffer.len())
    }

    fn set_position(&self, _: Size, _: &Position) -> Result<Size> {
        Err(Error::UnsupportedOperation)
    }
}

impl MountOperations for Terminal {}

impl DirectCharacterDevice for Terminal {}
