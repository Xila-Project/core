use wasmi::Linker;

use crate::host::{error::Result, store::GlobalStore};

pub fn add_wasi_to_linker(linker: &mut Linker<GlobalStore>) -> Result<()> {
    crate::host::wasi::environment::add_to_linker(linker)?;
    crate::host::wasi::file::add_to_linker(linker)?;
    crate::host::wasi::time::add_to_linker(linker)?;
    crate::host::wasi::random::add_to_linker(linker)?;
    crate::host::wasi::process::add_to_linker(linker)?;
    crate::host::wasi::scheduling::add_to_linker(linker)?;
    crate::host::wasi::directory::add_to_linker(linker)?;
    crate::host::wasi::path::add_to_linker(linker)?;
    Ok(())
}
