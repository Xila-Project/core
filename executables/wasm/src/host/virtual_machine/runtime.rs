//! WASM Runtime management and configuration.
//!
//! This module provides a wrapper around the WAMR runtime with a builder pattern
//! for configuring and creating runtime instances with registered host functions.

use crate::host::virtual_machine::{
    Environment, Error, Function, Instance, Module, Registrable, Result,
};
use alloc::{format, vec::Vec};
use core::{num::NonZero, task::Poll, time::Duration};
use wamr_rust_sdk::value::WasmValue;
use wasm_abi_bindings::{EnvironmentContext, EnvironmentState, GlobalContext, InstanceContext};
use xila::{
    log,
    task::{self, TaskIdentifier, yield_now},
    virtual_file_system::File,
};

pub struct Runtime(wamr_rust_sdk::runtime::Runtime);

unsafe impl Send for Runtime {}
unsafe impl Sync for Runtime {}

impl Runtime {
    pub fn new<'a>(registrables: impl IntoIterator<Item = &'a dyn Registrable>) -> Result<Self> {
        let mut runtime_builder = wamr_rust_sdk::runtime::Runtime::builder()
            .use_system_allocator()
            .run_as_interpreter();

        for registrable in registrables {
            for function_descriptor in registrable.get_functions() {
                runtime_builder = runtime_builder
                    .register_host_function(function_descriptor.name, function_descriptor.pointer);
            }
        }

        let runtime = runtime_builder.build()?;

        Ok(Self(runtime))
    }

    pub fn get_inner_reference(&self) -> &wamr_rust_sdk::runtime::Runtime {
        &self.0
    }

    /// Execute a WASM module with the specified I/O configuration.
    ///
    /// This is the main entry point for executing WASM modules. It creates a new
    /// module instance, sets up the execution environment with proper I/O redirection,
    /// and calls the module's main function.
    #[allow(clippy::too_many_arguments)]
    pub async fn execute(
        &'static self,
        name: &str,
        buffer: Vec<u8>,
        stack_size: usize,
        (standard_in, standard_out, standard_error): (File<'static>, File<'static>, File<'static>),
        function_name: &str,
        arguments: Vec<WasmValue>,
        task: TaskIdentifier,
        instruction_limit: NonZero<u32>,
        task_manager: &'static task::Manager,
    ) -> Result<Vec<WasmValue>> {
        log::information!(
            "Starting execution of WASM module '{name}' with function '{function_name}'"
        );
        let mut instance_context = InstanceContext::new();
        let raw_instance_context =
            &mut instance_context as *const InstanceContext as *mut InstanceContext;

        let mut environment_context = EnvironmentContext::new(task);
        let raw_environment_context =
            &mut environment_context as *const EnvironmentContext as *mut EnvironmentContext;

        GlobalContext::set(raw_instance_context, raw_environment_context).await;
        let module = Module::from_buffer(
            self,
            buffer,
            name,
            standard_in,
            standard_out,
            standard_error,
            task_manager,
        )
        .await?;
        log::information!("SUCE MOI LE MODULE '{name}'");
        let instance = Instance::new(self, &module, &mut instance_context, stack_size as _)?;

        GlobalContext::clear().await;

        log::information!("CACA '{name}'");

        let mut function = Function::find_exported(&instance, function_name)?;

        log::information!("PROUT '{name}'");

        let mut environment =
            Environment::new(&instance, &mut environment_context, stack_size as _)?;

        log::information!(
            "Successfully created execution environment for module '{name}' with task {:?} and stack size {}",
            task,
            stack_size
        );

        environment.set_instruction_count_limit(Some(instruction_limit.get()));

        log::information!(
            "Set instruction count limit for module '{name}' to {} instructions",
            instruction_limit
        );

        let result = loop {
            GlobalContext::set(raw_instance_context, raw_environment_context).await;
            let result = function.call(&instance, &mut environment, &arguments);
            GlobalContext::clear().await;

            if let Poll::Ready(result) = result {
                return result;
            }

            match environment.get_context().take_state() {
                EnvironmentState::Running => {
                    yield_now().await;
                }
                EnvironmentState::Sleeping(duration) => match duration {
                    Duration::MAX => {
                        log::information!("WASM module '{name}' is suspending execution");
                        task::suspend(|_| {}).await
                    }
                    Duration::ZERO => {
                        log::information!("WASM module '{name}' is yielding execution");

                        task::yield_now().await
                    }
                    duration => {
                        log::information!("WASM module '{name}' is sleeping for {:?}", duration);
                        task::sleep(duration).await
                    }
                },
                EnvironmentState::Exited => {
                    break Err(Error::Execution(format!(
                        "WASM execution failed: task {} exited",
                        task.into_inner()
                    )));
                }
            }
        };

        log::information!("Finished execution of WASM module '{name}' with result: {result:?}");
        result
    }
}
