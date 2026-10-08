mod bindings;
mod virtual_machine;

use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::{borrow::ToOwned, string::String, vec, vec::Vec};
use core::fmt::Write;
use core::num::{NonZeroU32, NonZeroUsize};
use getargs_derive::GetArgs;
use xila::executable::ExecutableTrait;
use xila::executable::MainFuture;
use xila::executable::Standard;
use xila::file_system::{Kind, Path};
use xila::synchronization::once_lock::OnceLock;
use xila::task::{self};
use xila::virtual_file_system::{self, File};

#[cfg(feature = "graphics")]
use crate::host::bindings::graphics::GraphicsBindings;
use crate::host::virtual_machine::Registrable;

pub use crate::host::virtual_machine::Error;

pub struct WasmExecutable;

static RUNTIME: OnceLock<virtual_machine::Runtime> = OnceLock::new();
const REGISTRABLES: &[&dyn Registrable] = &[
    #[cfg(feature = "graphics")]
    &GraphicsBindings,
];
const START_FUNCTION_NAME: &str = "_start";
const INSTALL_FUNCTION_NAME: &str = "__install";
const DEFAULT_STACK_SIZE: usize = 4096;

#[derive(GetArgs)]
struct WasmArguments<'a> {
    path: &'a str,
    #[arg(flag)]
    install: bool,
    #[arg(default = DEFAULT_STACK_SIZE)]
    stack_size: usize,
    #[arg(default = NonZeroU32::new(200).unwrap())]
    instruction_limit: NonZeroU32,
}

impl ExecutableTrait for WasmExecutable {
    fn main(standard: Standard<'static>, arguments: Vec<String>) -> MainFuture {
        Box::pin(async move { main(standard, arguments).await })
    }
}

pub async fn inner_main(standard: Standard<'static>, arguments: Vec<String>) -> Result<(), Error> {
    let context = standard.context;
    let mut options = getargs::Options::new(arguments.iter().map(|argument| argument.as_str()));
    let WasmArguments {
        install,
        stack_size,
        path,
        instruction_limit,
    } = WasmArguments::parse(&mut options)?;
    let path = Path::new(path);

    let task_manager = context.task_manager;
    let task = task_manager.get_current_task_identifier().await;

    let path = if path.is_absolute() {
        path.to_owned()
    } else {
        let current_path = task_manager
            .get_environment_variable(task, "Current_directory")
            .await
            .map_err(|_| Error::FailedToGetCurrentDirectory)?;

        let current_path = current_path.get_value();

        let current_path = Path::new(current_path);

        current_path.join(path).ok_or(Error::InvalidPath)?
    };

    let virtual_file_system = &context.virtual_file_system;

    let statistics = virtual_file_system
        .get_statistics(&path)
        .await
        .map_err(|_| Error::InvalidPath)?;

    if statistics.kind != Kind::File {
        return Err(Error::NotAWasmFile);
    }

    let mut buffer = Vec::with_capacity(statistics.size as usize);

    File::read_from_path(virtual_file_system, task, &path, &mut buffer)
        .await
        .map_err(|_| Error::FailedToReadFile)?;

    let name = path.get_file_name().to_string();

    let function_name = if install {
        INSTALL_FUNCTION_NAME
    } else {
        START_FUNCTION_NAME
    };

    let runtime = RUNTIME
        .get_or_init(|| virtual_machine::Runtime::new(REGISTRABLES.iter().copied()).unwrap());

    let standard = standard.split();

    let task_identifier = task_manager.get_current_task_identifier().await;

    runtime
        .execute(
            &name,
            buffer,
            stack_size,
            standard,
            function_name,
            vec![],
            task_identifier,
            instruction_limit,
            task_manager,
        )
        .await?;

    Ok(())
}

pub async fn main(standard: Standard<'static>, arguments: Vec<String>) -> Result<(), NonZeroUsize> {
    let mut duplicated_standard = standard
        .duplicate()
        .await
        .map_err(Error::FailedToDuplicateStandard)?;

    match inner_main(standard, arguments).await {
        Ok(()) => Ok(()),
        Err(error) => {
            writeln!(
                duplicated_standard.standard_error,
                "WASM Executable Error: {}",
                error
            )
            .unwrap();
            Err(error.into())
        }
    }
}
