use wasmi::Caller;

use crate::{
    define_wasi_module,
    host::{store::GlobalStore, wasi::error::WasiResult},
};

define_wasi_module! {
    module: "wasi_snapshot_preview1";

    fn sched_yield(
        caller: Caller<GlobalStore>,
    ) -> Result<WasiResult, wasmi::Error> {
        let mut caller=caller;
        caller.data_mut().pending=Some(crate::host::execution::PendingCall::Yield);
        Err(wasmi::Error::host(crate::host::execution::Suspend))
    }
}
