use wasmi::Caller;

use crate::{define_wasi_module, host::store::GlobalStore};

define_wasi_module! {
    module: "wasi_snapshot_preview1";

    fn proc_exit(
        caller: Caller<GlobalStore>,
        code: i32,
    ) -> Result<(), wasmi::Error> {
        let mut caller = caller;
        caller.data_mut().wasi.exit_code = Some(code);
        Err(wasmi::Error::new("exit"))
    }
}
