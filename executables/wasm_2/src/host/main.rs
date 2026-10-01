use core::num::{NonZeroU32, NonZeroUsize};

use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use core::fmt::Write;
use getargs_derive::GetArgs;
use wasmi::{Caller, Config, Engine, Linker, Module, Store, TypedResumableCall};
use xila::{
    executable::Standard,
    file_system::{Kind, Path},
    log, task,
    virtual_file_system::{self, File},
};

use crate::host::{
    error::{Error, Result},
    store::GlobalStore,
    wasi::{self, DirectoryVariant, FileSystemItem, FileVariant, Prestat, WasiContext},
};

const DEFAULT_STACK_SIZE: usize = 4096;
const START_FUNCTION_NAME: &str = "_start";
const INSTALL_FUNCTION_NAME: &str = "__install";

#[derive(GetArgs)]
struct WasmArguments<'a> {
    path: &'a str,
    #[arg(flag)]
    install: bool,
    #[arg(default = DEFAULT_STACK_SIZE)]
    stack_size: usize,
    #[arg(default = NonZeroU32::new(100_000).unwrap())]
    instruction_limit: NonZeroU32,
}

pub async fn main(
    mut standard: Standard,
    arguments: Vec<String>,
) -> core::result::Result<(), NonZeroUsize> {
    let arguments = match WasmArguments::from_args(arguments.iter().map(|s| s.as_str())) {
        Ok(args) => args,
        Err(e) => {
            writeln!(standard.error(), "Error parsing arguments: {}", e).unwrap();
            return Err(NonZeroUsize::new(1).unwrap());
        }
    };

    main_inner(standard.duplicate().await.unwrap(), arguments)
        .await
        .map_err(|e| {
            writeln!(standard.error(), "Error: {:?}", e).unwrap();
            NonZeroUsize::new(1).unwrap()
        })?;

    Ok(())
}

async fn read_file(path: &str) -> Result<Vec<u8>> {
    let virtual_file_system = virtual_file_system::get_instance();

    let task = task::get_instance().get_current_task_identifier().await;

    let path = Path::new(path);

    let statistics = virtual_file_system.get_statistics(&path).await?;

    if statistics.kind != Kind::File {
        return Err(Error::NotAWasmFile);
    }

    let mut buffer = Vec::with_capacity(statistics.size as usize);

    File::read_from_path(virtual_file_system, task, &path, &mut buffer).await?;

    Ok(buffer)
}

pub async fn main_inner(standard: Standard, arguments: WasmArguments<'_>) -> Result<()> {
    let WasmArguments {
        path,
        install: _,
        stack_size: _,
        instruction_limit,
    } = arguments;

    let mut configuration = Config::default();
    configuration.consume_fuel(true);

    let engine = Engine::new(&configuration);

    let buffer = read_file(path).await?;

    let module = Module::new(&engine, &buffer)?;

    let task = xila::task::get_instance()
        .get_current_task_identifier()
        .await;
    let (_standard_in, standard_out, standard_error) = standard.split();
    let root_dir = xila::virtual_file_system::SynchronousDirectory::open(
        xila::virtual_file_system::get_instance(),
        task,
        xila::file_system::Path::from_str("/"),
    );

    let mut store = Store::new(
        &engine,
        GlobalStore {
            wasi: WasiContext {
                files: alloc::collections::BTreeMap::new(),
                next_fd: 3,
                arguments: alloc::vec![path.to_string()],
                environment: task::block_on(task::get_instance().get_environment_variables(task))
                    .map_err(|_| wasmi::Error::new("environment variables unavailable"))?
                    .into_iter()
                    .map(|variable| {
                        (
                            variable.get_name().to_string(),
                            variable.get_value().to_string(),
                        )
                    })
                    .collect(),
                task,
                random_state: 0,
                prestats: Vec::new(),
                exit_code: None,
            },
        },
    );

    {
        let data = store.data_mut();
        data.wasi.files.insert(
            0,
            FileSystemItem::StandardInput(FileVariant {
                file: _standard_in.into_synchronous_file(),
            }),
        );
        data.wasi.files.insert(
            1,
            FileSystemItem::StandardOutput(FileVariant {
                file: standard_out.into_synchronous_file(),
            }),
        );
        data.wasi.files.insert(
            2,
            FileSystemItem::StandardError(FileVariant {
                file: standard_error.into_synchronous_file(),
            }),
        );
        if let Ok(dir) = root_dir {
            data.wasi.files.insert(
                3,
                FileSystemItem::Directory(DirectoryVariant {
                    directory: dir,
                    path: xila::file_system::PathOwned::root(),
                }),
            );
            data.wasi.next_fd = 4;
            data.wasi.prestats.push(Prestat {
                name: b"/".to_vec(),
            });
        }
    }

    let mut linker = Linker::<GlobalStore>::new(&engine);

    wasi::register::add_wasi_to_linker(&mut linker)?;

    linker.func_wrap(
        "host",
        "hello",
        |caller: Caller<'_, GlobalStore>, param: i32| {
            log::information!(
                "Hello from WebAssembly! Got {} from WebAssembly and my host state is: {}",
                param,
                caller.data()
            );
        },
    )?;

    match linker.instantiate_and_start(&mut store, &module) {
        Ok(instance) => {
            let start = instance.get_typed_func::<(), ()>(&store, START_FUNCTION_NAME)?;
            store.set_fuel(instruction_limit.get() as u64)?;

            let mut call = start.call_resumable(&mut store, ())?;
            loop {
                call = match call {
                    TypedResumableCall::Finished(()) => return Ok(()),
                    TypedResumableCall::OutOfFuel(call) => {
                        store.set_fuel(instruction_limit.get() as u64)?;
                        call.resume(&mut store)?
                    }
                    TypedResumableCall::HostTrap(_) => {
                        let code = store
                            .data()
                            .wasi
                            .exit_code
                            .ok_or_else(|| wasmi::Error::new("unhandled WASI host trap"))?;
                        return if code == 0 {
                            Ok(())
                        } else {
                            Err(Error::Runtime(code))
                        };
                    }
                };
            }
        }
        Err(e) => {
            if let Some(code) = store.data().wasi.exit_code {
                if code == 0 {
                    Ok(())
                } else {
                    Err(Error::Runtime(code))
                }
            } else {
                Err(Error::Wasm(e))
            }
        }
    }
}
