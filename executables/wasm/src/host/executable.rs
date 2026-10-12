use alloc::boxed::Box;
use xila::executable::ExecutableTrait;

use crate::host::main::main;

pub struct WasmExecutable;

impl ExecutableTrait for WasmExecutable {
    fn main(
        standard: xila::executable::Standard<'static>,
        arguments: alloc::vec::Vec<alloc::string::String>,
    ) -> xila::executable::MainFuture {
        Box::pin(async move { main(standard, arguments).await })
    }
}
