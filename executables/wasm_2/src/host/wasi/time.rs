use wasmi::Caller;

use crate::{
    define_wasi_module,
    host::{
        store::GlobalStore,
        translation::{GuestPointer, WasmUsize, borrow_memory, get_memory},
        wasi::error::WasiResult,
    },
};

define_wasi_module! {
    module: "wasi_snapshot_preview1";

    fn clock_res_get(
        caller: Caller<GlobalStore>,
        _clock_id: i32,
        resolution_ptr: WasmUsize,
    ) -> Result<WasiResult, wasmi::Error> {
        let mut caller = caller;
        let memory = get_memory(&mut caller).ok_or_else(|| wasmi::Error::new("missing memory"))?;
        let resolution = caller.data().context.time_manager
            .get_current_time_since_startup()
            .map_err(|_| wasmi::Error::new("time error"))?;
        borrow_memory(&memory, &mut caller)
            .write(GuestPointer::new(resolution_ptr), resolution.as_nanos() as u64)
            .ok_or_else(|| wasmi::Error::new("invalid output pointer"))?;
        Ok(0)
    }

    fn clock_time_get(
        caller: Caller<GlobalStore>,
        _clock_id: i32,
        _precision: i64,
        time_ptr: WasmUsize,
    ) -> Result<WasiResult, wasmi::Error> {
        let mut caller = caller;
        let memory = get_memory(&mut caller).ok_or_else(|| wasmi::Error::new("missing memory"))?;
        let now = caller.data().context.time_manager
            .get_current_time()
            .map_err(|_| wasmi::Error::new("time error"))?;
        borrow_memory(&memory, &mut caller)
            .write(GuestPointer::new(time_ptr), now.as_nanos() as u64)
            .ok_or_else(|| wasmi::Error::new("invalid output pointer"))?;
        Ok(0)
    }
}
