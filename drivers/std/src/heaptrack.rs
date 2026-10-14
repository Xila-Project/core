//! Heaptrack's optional custom allocator API; compiled only on Linux with heaptrack.
use core::{cell::Cell, ffi::c_void};
use std::sync::OnceLock;

type Allocate = unsafe extern "C" fn(*mut c_void, usize);
type Free = unsafe extern "C" fn(*mut c_void);
struct Hooks {
    allocate: Option<Allocate>,
    free: Option<Free>,
}
static HOOKS: OnceLock<Hooks> = OnceLock::new();
std::thread_local! { static REPORTING: Cell<bool> = const { Cell::new(false) }; }

struct Guard;
impl Guard {
    fn enter() -> Option<Self> {
        REPORTING.with(|reporting| {
            if reporting.replace(true) {
                None
            } else {
                Some(Guard)
            }
        })
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        REPORTING.with(|reporting| reporting.set(false));
    }
}

fn hooks() -> &'static Hooks {
    HOOKS.get_or_init(|| unsafe {
        let allocate = libc::dlsym(libc::RTLD_DEFAULT, c"heaptrack_malloc".as_ptr());
        let free = libc::dlsym(libc::RTLD_DEFAULT, c"heaptrack_free".as_ptr());
        Hooks {
            allocate: if allocate.is_null() {
                None
            } else {
                Some(core::mem::transmute::<*mut c_void, Allocate>(allocate))
            },
            free: if free.is_null() {
                None
            } else {
                Some(core::mem::transmute::<*mut c_void, Free>(free))
            },
        }
    })
}

pub fn allocated(pointer: *mut u8, size: usize) {
    let Some(_guard) = Guard::enter() else {
        return;
    };
    if let Some(report) = hooks().allocate {
        unsafe { report(pointer.cast(), size) };
    }
}

pub fn freed(pointer: *mut u8) {
    let Some(_guard) = Guard::enter() else {
        return;
    };
    if let Some(report) = hooks().free {
        unsafe { report(pointer.cast()) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recursion_is_suppressed_until_outer_callback_returns() {
        let guard = Guard::enter().unwrap();
        assert!(Guard::enter().is_none());
        assert!(Guard::enter().is_none());
        drop(guard);
        assert!(Guard::enter().is_some());
    }
}
