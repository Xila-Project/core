use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::ptr;
use core::sync::atomic::{AtomicU64, Ordering};

use embassy_executor::{Spawner, raw};
use file_system::DirectBaseOperations;
use js_sys::Promise;
use task::{ExecutorStatisticsSnapshot, ExecutorWithStatistics};
use wasm_bindgen::prelude::*;
extern crate alloc;
use alloc::boxed::Box;

use crate::devices::TimeDevice;

#[unsafe(export_name = "__pender")]
fn __pender(context: *mut ()) {
    let signaler: &'static WasmContext = unsafe { core::mem::transmute(context) };
    let _ = signaler.promise.then(unsafe { signaler.closure.as_mut() });
}

struct UninitCell<T>(MaybeUninit<core::cell::UnsafeCell<T>>);

impl<T> UninitCell<T> {
    const fn uninit() -> Self {
        Self(core::mem::MaybeUninit::uninit())
    }

    unsafe fn as_mut_ptr(&self) -> *mut T {
        unsafe { (*self.0.as_ptr()).get() }
    }

    unsafe fn write_in_place(&self, func: impl FnOnce() -> T) {
        unsafe { ptr::write(self.as_mut_ptr(), func()) };
    }

    unsafe fn as_mut(&self) -> &mut T {
        unsafe { &mut *self.as_mut_ptr() }
    }
}

unsafe impl<T> Sync for UninitCell<T> {}

struct WasmContext {
    promise: Promise,
    closure: UninitCell<Closure<dyn FnMut(JsValue)>>,
}

impl WasmContext {
    fn new() -> Self {
        Self {
            promise: Promise::resolve(&JsValue::undefined()),
            closure: UninitCell::uninit(),
        }
    }
}

pub struct Executor {
    inner: raw::Executor,
    ctx: &'static WasmContext,
    statistics: &'static ExecutorStatistics,
    not_send: PhantomData<*mut ()>,
}

pub struct ExecutorStatistics {
    busy_ticks: AtomicU64,
    idle_ticks: AtomicU64,
    last_poll_end_ticks: AtomicU64,
}

impl ExecutorStatistics {
    const fn new() -> Self {
        Self {
            busy_ticks: AtomicU64::new(0),
            idle_ticks: AtomicU64::new(0),
            last_poll_end_ticks: AtomicU64::new(0),
        }
    }

    fn record_poll(&self, poll_start_ticks: u64, poll_end_ticks: u64) {
        let previous_poll_end = self
            .last_poll_end_ticks
            .swap(poll_end_ticks, Ordering::Relaxed);

        if previous_poll_end > 0 && poll_start_ticks > previous_poll_end {
            self.idle_ticks
                .fetch_add(poll_start_ticks - previous_poll_end, Ordering::Relaxed);
        }

        if poll_end_ticks > poll_start_ticks {
            self.busy_ticks
                .fetch_add(poll_end_ticks - poll_start_ticks, Ordering::Relaxed);
        }
    }
}

impl ExecutorStatistics {
    fn snapshot(&self) -> ExecutorStatisticsSnapshot {
        ExecutorStatisticsSnapshot {
            busy_ticks: self.busy_ticks.load(Ordering::Relaxed),
            idle_ticks: self.idle_ticks.load(Ordering::Relaxed),
        }
    }
}

impl Executor {
    pub fn new() -> Self {
        let ctx = Box::leak(Box::new(WasmContext::new()));
        let statistics = Box::leak(Box::new(ExecutorStatistics::new()));

        Self {
            inner: raw::Executor::new(ctx as *mut WasmContext as *mut ()),
            ctx,
            statistics,
            not_send: PhantomData,
        }
    }

    pub fn start(&'static self, init: impl FnOnce(Spawner)) {
        unsafe {
            let executor = &self.inner;
            let statistics = self.statistics;
            let future = Closure::new(move |_| {
                let poll_start = get_current_ticks_from_time_device();
                executor.poll();
                let poll_end = get_current_ticks_from_time_device();
                statistics.record_poll(poll_start, poll_end);
            });
            self.ctx.closure.write_in_place(|| future);
            init(self.inner.spawner());
        }
    }

    pub fn spawner(&'static self) -> Spawner {
        self.inner.spawner()
    }
}

impl ExecutorWithStatistics for Executor {
    fn spawner(&'static self) -> Spawner {
        Executor::spawner(self)
    }

    fn statistics_snapshot(&self) -> Option<ExecutorStatisticsSnapshot> {
        Some(self.statistics.snapshot())
    }
}

fn get_current_ticks_from_time_device() -> u64 {
    let mut current_time = core::time::Duration::default();

    let current_time_raw = unsafe {
        core::slice::from_raw_parts_mut(
            &mut current_time as *mut core::time::Duration as *mut u8,
            core::mem::size_of::<core::time::Duration>(),
        )
    };

    if TimeDevice.read(current_time_raw, 0).is_err() {
        return 0;
    }

    current_time.as_nanos() as u64
}

#[macro_export]
macro_rules! instantiate_static_executor {
    () => {{
        static mut __EXECUTOR: Option<$crate::executor::Executor> = None;

        unsafe {
            if __EXECUTOR.is_none() {
                __EXECUTOR = Some($crate::executor::Executor::new());
            }
            __EXECUTOR.as_mut().expect("Executor is not initialized")
        }
    }};
}

pub use instantiate_static_executor;
