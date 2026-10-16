use crate::{Error, Result};
use core::cell::RefCell;
use file_system::Size;
use synchronization::{
    Arc,
    blocking_mutex::{Mutex, raw::CriticalSectionRawMutex},
    mutex::Mutex as AsyncMutex,
};

type SharedPosition = Arc<AsyncMutex<CriticalSectionRawMutex, Size>>;
#[derive(Clone)]
enum Position {
    Exclusive(Size),
    Shared(SharedPosition),
}

/// Allocate shared position storage only when a file is actually duplicated.
pub(crate) struct Cursor(Mutex<CriticalSectionRawMutex, RefCell<Position>>);
impl Cursor {
    pub const fn new(position: Size) -> Self {
        Self(Mutex::new(RefCell::new(Position::Exclusive(position))))
    }
    pub fn duplicate(&self) -> Self {
        self.0.lock(|position| {
            let mut position = position.borrow_mut();
            if let Position::Exclusive(value) = *position {
                *position = Position::Shared(Arc::new(AsyncMutex::new(value)));
            }
            Self(Mutex::new(RefCell::new(position.clone())))
        })
    }
    pub fn update<T>(&self, operation: impl FnOnce(Size) -> Result<(T, Size)>) -> Result<T> {
        // Never hold the global critical section across device I/O. Contended
        // duplicated streams return ResourceBusy for the caller's polling path.
        let position = self.0.lock(|position| position.borrow().clone());
        match position {
            Position::Exclusive(value) => {
                let (result, next) = operation(value)?;
                self.0
                    .lock(|position| *position.borrow_mut() = Position::Exclusive(next));
                Ok(result)
            }
            Position::Shared(shared) => {
                let mut value = shared.try_lock().map_err(|_| Error::RessourceBusy)?;
                let (result, next) = operation(*value)?;
                *value = next;
                Ok(result)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;

    #[test]
    fn io_callbacks_do_not_hold_the_global_critical_section() {
        let cursor = Cursor::new(0);
        let duplicate = cursor.duplicate();
        cursor
            .update(|position| {
                std::thread::scope(|scope| {
                    let (sender, receiver) = std::sync::mpsc::channel();
                    scope.spawn(move || {
                        let other = Cursor::new(0);
                        sender.send(other.update(|_| Ok(((), 1)))).unwrap();
                    });
                    assert!(
                        receiver
                            .recv_timeout(std::time::Duration::from_secs(1))
                            .unwrap()
                            .is_ok()
                    );
                });
                assert!(matches!(
                    duplicate.update(|_| Ok(((), 1))),
                    Err(Error::RessourceBusy)
                ));
                Ok(((), position + 1))
            })
            .unwrap();
        assert_eq!(
            duplicate
                .update(|position| Ok((position, position)))
                .unwrap(),
            1
        );
    }
}
