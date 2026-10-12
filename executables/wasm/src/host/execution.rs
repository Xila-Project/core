//! Wasmi-specific cooperative execution. No guest borrow crosses an await.
use super::{
    error::{Error, Result},
    store::GlobalStore,
    translation::GuestMemory,
    wasi::poll_memory::ClockPoll,
};
use alloc::string::ToString;
use core::{fmt, num::NonZeroU32, time::Duration};
use wasmi::{Store, TypedFunc, TypedResumableCall, Val};

pub enum PendingCall {
    Yield,
    ClockPoll {
        poll: ClockPoll,
        memory: wasmi::Memory,
    },
}

#[derive(Debug)]
pub(crate) struct Suspend;
impl fmt::Display for Suspend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("cooperative VM suspension")
    }
}
impl wasmi::errors::HostError for Suspend {}

pub(crate) fn clock_now(
    context: &xila::executable::ExecutableContext,
) -> core::result::Result<[u64; 2], wasmi::Error> {
    let realtime = context
        .time_manager
        .get_current_time()
        .map_err(|_| wasmi::Error::new("clock error"))?;
    let monotonic = context
        .time_manager
        .get_current_time_since_startup()
        .map_err(|_| wasmi::Error::new("clock error"))?;
    Ok([
        u64::try_from(realtime.as_nanos()).map_err(|_| wasmi::Error::new("clock overflow"))?,
        u64::try_from(monotonic.as_nanos()).map_err(|_| wasmi::Error::new("clock overflow"))?,
    ])
}

pub async fn run(
    store: &mut Store<GlobalStore>,
    start: TypedFunc<(), ()>,
    instruction_limit: NonZeroU32,
) -> Result<()> {
    store.set_fuel(instruction_limit.get() as u64)?;
    let mut call = start.call_resumable(&mut *store, ())?;
    loop {
        call = match call {
            TypedResumableCall::Finished(()) => return Ok(()),
            TypedResumableCall::OutOfFuel(call) => {
                xila::task::yield_now().await;
                // A single bulk operation may require more than one normal quantum.
                let fuel = u64::from(instruction_limit.get()).max(call.required_fuel());
                store.set_fuel(fuel)?;
                call.resume(&mut *store)?
            }
            TypedResumableCall::HostTrap(call) => {
                if call.host_error().downcast_ref::<Suspend>().is_some() {
                    let pending = store
                        .data_mut()
                        .pending
                        .take()
                        .ok_or_else(|| wasmi::Error::new("missing pending call"))?;
                    let context = store.data().context;
                    let errno = match pending {
                        PendingCall::Yield => {
                            xila::task::yield_now().await;
                            0
                        }
                        PendingCall::ClockPoll { poll, memory } => {
                            loop {
                                let delay = poll.delay(clock_now(context)?);
                                if !delay.is_zero() {
                                    // Bound timer arithmetic and periodically recheck realtime.
                                    xila::task::Manager::sleep(delay.min(Duration::from_secs(60)))
                                        .await;
                                } else {
                                    xila::task::yield_now().await;
                                }
                                let now = clock_now(context)?;
                                match poll.complete(
                                    &mut GuestMemory::new(memory.data_mut(&mut *store)),
                                    now,
                                ) {
                                    Ok(0) => continue,
                                    Ok(_) => break 0,
                                    Err(error) => break error as i32,
                                }
                            }
                        }
                    };
                    call.resume(&mut *store, &[Val::I32(errno)])?
                } else if let Some(code) = store.data().wasi.exit_code {
                    return if code == 0 {
                        Ok(())
                    } else {
                        Err(Error::Runtime(code))
                    };
                } else {
                    return Err(Error::Wasm(wasmi::Error::new(
                        call.host_error().to_string(),
                    )));
                }
            }
        };
    }
}
