mod additionnal;
mod error;
pub mod prelude;
mod translate;

use crate::host::virtual_machine::{
    EnvironmentReference, FunctionDescriptor, Registrable, Translator,
};
use wamr_rust_sdk::sys::WASMExecEnv;
use wasm_abi_bindings::{WasmPointer, WasmUsize};
use xila::{graphics, log, task::block_on};

pub use error::{Error, Result};

mod generated_bindings {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

pub struct GraphicsBindings;

impl Registrable for GraphicsBindings {
    fn get_functions(&self) -> &[FunctionDescriptor] {
        &GRAPHICS_BINDINGS_FUNCTIONS
    }
}

/// Call to graphics API
///
/// # Safety
///
/// This function is unsafe because it may dereference raw pointers (e.g. `Environment`, `Result` or `Arguments`).
/// The pointer must be valid and properly aligned (ensured by the virtual machine).
#[allow(clippy::too_many_arguments)]
unsafe fn call_inner(
    environment: *mut WASMExecEnv,
    function: generated_bindings::FunctionCall,
    argument_0: WasmUsize,
    argument_1: WasmUsize,
    argument_2: WasmUsize,
    argument_3: WasmUsize,
    argument_4: WasmUsize,
    argument_5: WasmUsize,
    argument_6: WasmUsize,
    arguments_count: u8,
    result_pointer: WasmPointer,
) -> Result<()> {
    unsafe {
        let instance = graphics::ffi_manager();

        let _lock = block_on(instance.lock());

        let environment = EnvironmentReference::from_raw_pointer(environment);

        let mut translation_map = Translator::from_environment(environment)
            .map_err(|_| Error::EnvironmentRetrievalFailed)?;

        generated_bindings::call_function(
            &mut translation_map,
            function,
            argument_0,
            argument_1,
            argument_2,
            argument_3,
            argument_4,
            argument_5,
            argument_6,
            arguments_count,
            result_pointer,
        )?;

        Ok(())

        // Lock is automatically released here.
    }
}

#[allow(clippy::too_many_arguments)]
pub unsafe fn call(
    environment: *mut WASMExecEnv,
    function: generated_bindings::FunctionCall,
    argument_0: WasmUsize,
    argument_1: WasmUsize,
    argument_2: WasmUsize,
    argument_3: WasmUsize,
    argument_4: WasmUsize,
    argument_5: WasmUsize,
    argument_6: WasmUsize,
    arguments_count: u8,
    result_pointer: WasmPointer,
) -> i32 {
    let result = unsafe {
        call_inner(
            environment,
            function,
            argument_0,
            argument_1,
            argument_2,
            argument_3,
            argument_4,
            argument_5,
            argument_6,
            arguments_count,
            result_pointer,
        )
    };

    match result {
        Ok(_) => 0,
        Err(error) => {
            log::error!(
                "Error {error:?} durring graphics call: {function:?} with arguments: {argument_0:x}, {argument_1:x}, {argument_2:x}, {argument_3:x}, {argument_4:x}, {argument_5:x}, {argument_6:x}, count: {arguments_count}, result pointer: {result_pointer:x}"
            );
            error as i32
        }
    }
}

const GRAPHICS_BINDINGS_FUNCTIONS: [FunctionDescriptor; 1] = [FunctionDescriptor {
    name: "xila_graphics_call",
    pointer: call as *mut _,
}];
