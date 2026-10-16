use alloc::collections::VecDeque;
use file_system::{AccessFlags, BaseOperations, Context, Error, MountOperations, Result, Size};
use synchronization::{blocking_mutex::raw::CriticalSectionRawMutex, rwlock::RwLock};
use task::block_on;

#[derive(Debug)]
struct State {
    buffer: VecDeque<u8>,
    readers: usize,
    writers: usize,
}

/// Bounded FIFO. The final writer closing makes an empty read return EOF.
#[derive(Debug)]
pub struct Pipe {
    state: RwLock<CriticalSectionRawMutex, State>,
    capacity: usize,
}

impl Pipe {
    pub fn new(capacity: usize) -> Self {
        Self {
            state: RwLock::new(State {
                buffer: VecDeque::with_capacity(capacity),
                readers: 0,
                writers: 0,
            }),
            capacity,
        }
    }

    pub(crate) fn open_context(&self, access: AccessFlags) -> Context {
        let mut state = block_on(self.state.write());
        state.readers += usize::from(access.contains(AccessFlags::Read));
        state.writers += usize::from(access.contains(AccessFlags::Write));
        Context::new(Some(access))
    }

    pub async fn write(&self, data: &[u8]) -> Result<usize> {
        if data.is_empty() {
            return Ok(0);
        }
        let mut state = self.state.write().await;
        if state.readers == 0 {
            return Err(Error::InputOutput);
        }
        let length = data.len().min(self.capacity - state.buffer.len());
        if length == 0 {
            return Err(Error::RessourceBusy);
        }
        state.buffer.extend(&data[..length]);
        Ok(length)
    }

    pub async fn read(&self, data: &mut [u8]) -> Result<usize> {
        if data.is_empty() {
            return Ok(0);
        }
        let mut state = self.state.write().await;
        let length = data.len().min(state.buffer.len());
        if length == 0 && state.writers > 0 {
            return Err(Error::RessourceBusy);
        }
        for byte in &mut data[..length] {
            *byte = state.buffer.pop_front().unwrap();
        }
        Ok(length)
    }

    fn check_access(context: &Context, required: AccessFlags) -> Result<()> {
        if context
            .get_private_data_of_type::<AccessFlags>()
            .is_some_and(|access| access.contains(required))
        {
            Ok(())
        } else {
            Err(Error::InvalidMode)
        }
    }
}

impl BaseOperations for Pipe {
    fn close(&self, context: &mut Context) -> Result<()> {
        let access = context
            .take_private_data_of_type::<AccessFlags>()
            .ok_or(Error::InvalidContext)?;
        let mut state = block_on(self.state.write());
        state.readers -= usize::from(access.contains(AccessFlags::Read));
        state.writers -= usize::from(access.contains(AccessFlags::Write));
        Ok(())
    }
    fn clone_context(&self, context: &Context) -> Result<Context> {
        let access = *context
            .get_private_data_of_type::<AccessFlags>()
            .ok_or(Error::InvalidContext)?;
        Ok(self.open_context(access))
    }
    fn read(&self, context: &mut Context, data: &mut [u8], _: Size) -> Result<usize> {
        Self::check_access(context, AccessFlags::Read)?;
        block_on(self.read(data))
    }
    fn read_until(
        &self,
        context: &mut Context,
        data: &mut [u8],
        _: Size,
        delimiter: &[u8],
    ) -> Result<usize> {
        Self::check_access(context, AccessFlags::Read)?;
        if delimiter.is_empty() {
            return Err(Error::InvalidParameter);
        }
        let mut state = block_on(self.state.write());
        if !data.is_empty() && state.buffer.is_empty() && state.writers > 0 {
            return Err(Error::RessourceBusy);
        }
        let mut length = 0;
        while length < data.len() {
            let Some(byte) = state.buffer.pop_front() else {
                break;
            };
            data[length] = byte;
            length += 1;
            if data[..length].ends_with(delimiter) {
                break;
            }
        }
        Ok(length)
    }
    fn write(&self, context: &mut Context, data: &[u8], _: Size) -> Result<usize> {
        Self::check_access(context, AccessFlags::Write)?;
        block_on(self.write(data))
    }
}
impl MountOperations for Pipe {}

#[cfg(test)]
mod regression_tests {
    extern crate std;
    use super::*;
    #[task::test]
    async fn short_read_does_not_pop_beyond_available_bytes() {
        let pipe = Pipe::new(4);
        let _reader = pipe.open_context(AccessFlags::Read);
        assert_eq!(pipe.write(b"hi").await.unwrap(), 2);
        let mut output = [0; 8];
        assert_eq!(pipe.read(&mut output).await.unwrap(), 2);
        assert_eq!(&output[..2], b"hi");
    }
    #[task::test]
    async fn writes_are_bounded_by_the_configured_capacity() {
        let pipe = Pipe::new(4);
        let _reader = pipe.open_context(AccessFlags::Read);
        assert_eq!(pipe.write(b"abcdef").await.unwrap(), 4);
        assert!(matches!(pipe.write(b"x").await, Err(Error::RessourceBusy)));
    }
    #[task::test]
    async fn eof_waits_for_every_writer_and_preserves_buffered_data() {
        let pipe = Pipe::new(4);
        let mut reader = pipe.open_context(AccessFlags::Read);
        let mut writer = pipe.open_context(AccessFlags::Write);
        let mut duplicate = pipe.clone_context(&writer).unwrap();
        assert!(matches!(
            pipe.read(&mut [0; 1]).await,
            Err(Error::RessourceBusy)
        ));
        pipe.write(b"hi").await.unwrap();
        pipe.close(&mut writer).unwrap();
        let mut output = [0; 8];
        assert_eq!(pipe.read(&mut output).await.unwrap(), 2);
        assert!(matches!(
            pipe.read(&mut output).await,
            Err(Error::RessourceBusy)
        ));
        pipe.close(&mut duplicate).unwrap();
        assert_eq!(pipe.read(&mut output).await.unwrap(), 0);
        pipe.close(&mut reader).unwrap();
        assert!(matches!(pipe.write(b"x").await, Err(Error::InputOutput)));
    }
}
