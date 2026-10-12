#![no_std]

#[cfg(target_arch = "wasm32")]
mod storage;

#[cfg(target_arch = "wasm32")]
static TASK_MANAGER: xila::task::Manager = xila::task::Manager::new();

#[cfg(target_arch = "wasm32")]
#[xila::task::run(
    task_path = xila::task,
    executor = drivers_wasm::executor::instantiate_static_executor!(),
    manager = &TASK_MANAGER
)]
async fn main(task_manager: &'static xila::task::Manager) {
    drivers_wasm::memory::instantiate_global_allocator!();

    extern crate alloc;

    use alloc::boxed::Box;
    use alloc::string::ToString;
    use alloc::vec;
    use drivers_wasm::devices::graphics::GraphicsDevices;
    use xila::bootsplash::Bootsplash;
    use xila::executable::{self, Standard, mount_executables};
    use xila::file_system::mbr::Mbr;
    use xila::file_system::{MountOperations, Path};
    use xila::log;
    use xila::task;
    use xila::time::{self, Duration};
    use xila::virtual_file_system::mount_static;
    use xila::{authentication, graphics, users, virtual_file_system};

    console_error_panic_hook::set_once();

    // - Initialize the system
    log::initialize(&drivers_wasm::log::Logger).unwrap();

    // Initialize the task manager
    let task = task_manager.get_current_task_identifier().await;
    let users_manager: &'static users::Manager = Box::leak(Box::new(users::Manager::new()));
    let time_manager: &'static time::Manager<'static> = Box::leak(Box::new(
        time::Manager::new(&drivers_wasm::devices::TimeDevice).unwrap(),
    ));

    // - Initialize the graphics manager
    // - - Initialize the graphics driver
    let GraphicsDevices {
        screen_device,
        mouse_device,
        keyboard_device,
        canvas: _canvas,
    } = drivers_wasm::devices::graphics::new().await.unwrap();

    let resolution = drivers_wasm::devices::graphics::get_resolution().unwrap();

    // - - Initialize the graphics manager
    let graphics_manager = graphics::initialize(
        time_manager,
        Box::leak(Box::new(screen_device)),
        Box::leak(Box::new(mouse_device)),
        graphics::InputKind::Pointer,
        graphics::get_recommended_buffer_size(&resolution),
        //graphics::get_minimal_buffer_size(&resolution),
        true,
    )
    .await;
    let graphics_manager: &'static graphics::Manager = Box::leak(Box::new(graphics_manager));
    graphics::set_ffi_manager(graphics_manager);

    graphics_manager
        .add_input_device(
            Box::leak(Box::new(keyboard_device)),
            graphics::InputKind::Keypad,
        )
        .await
        .unwrap();

    let bootsplash = Bootsplash::new(graphics_manager).await.unwrap();

    task_manager
        .spawn(task, "Graphics", None, move |_| {
            graphics_manager.r#loop(task::Manager::sleep)
        })
        .await
        .unwrap();

    // - Initialize the file system
    // Open the origin's persistent disk before synchronous MBR/LittleFS setup.
    let drive: &'static drivers_wasm::devices::DriveDevice = Box::leak(Box::new(
        drivers_wasm::devices::DriveDevice::new(Path::new("xila_drive.img"), 16 * 1024 * 1024)
            .await
            .expect("Failed to open OPFS disk (use HTTPS/localhost and close other Xila tabs)"),
    ));

    // Format only an empty disk; mount errors on saved images are fatal.
    let file_system = storage::mount(drive)
        .expect("Failed to mount OPFS disk; existing data was not reformatted");

    // Print MBR information
    let mbr = Mbr::read_from_device(drive).unwrap();
    log::information!("MBR Information: {mbr}");

    // Initialize the virtual file system
    let virtual_file_system: &'static virtual_file_system::VirtualFileSystem = Box::leak(Box::new(
        virtual_file_system::initialize(task_manager, users_manager, time_manager, file_system)
            .unwrap(),
    ));
    xila::abi_definitions::initialize(xila::abi_definitions::RuntimeContext {
        virtual_file_system,
        time_manager,
    });

    // - - Mount the devices

    // - - Create the default system hierarchy
    let _ = virtual_file_system::create_default_hierarchy(virtual_file_system, task).await;
    let _ = virtual_file_system
        .create_directory(task, &"/devices/cpu")
        .await;

    // - - Mount the devices
    virtual_file_system::clean_devices(virtual_file_system, task)
        .await
        .unwrap();

    mount_static!(
        virtual_file_system,
        task,
        &[
            (
                &"/devices/standard_in",
                CharacterDevice,
                drivers_core::NullDevice
            ),
            (
                &"/devices/standard_out",
                CharacterDevice,
                drivers_core::NullDevice
            ),
            (
                &"/devices/standard_error",
                CharacterDevice,
                drivers_core::NullDevice
            ),
            (
                &"/devices/time",
                CharacterDevice,
                drivers_wasm::devices::TimeDevice
            ),
            (
                &"/devices/cpu/informations",
                CharacterDevice,
                drivers_wasm::devices::CpuInformationsDevice
            ),
            (
                &"/devices/random",
                CharacterDevice,
                drivers_shared::devices::RandomDevice
            ),
            (&"/devices/null", CharacterDevice, drivers_core::NullDevice),
            (
                &"/devices/http_client",
                CharacterDevice,
                drivers_wasm::devices::HttpClientDevice
            ),
            (
                &"/devices/https_client",
                CharacterDevice,
                drivers_wasm::devices::HttpClientDevice
            ),
            (
                &"/devices/hasher",
                CharacterDevice,
                drivers_shared::devices::HashDevice
            ),
        ]
    )
    .await
    .unwrap();

    // Mount static executables

    let executable_context: &'static executable::ExecutableContext =
        Box::leak(Box::new(executable::ExecutableContext {
            task_manager,
            users_manager,
            virtual_file_system,
            graphics_manager: Some(graphics_manager),
            network_manager: None,
            time_manager,
        }));

    mount_executables!(
        executable_context,
        virtual_file_system,
        task,
        &[
            (&"/binaries/wasm", wasm::WasmExecutable),
            (
                &"/binaries/graphical_shell",
                graphical_shell::ShellExecutable
            ),
            (
                &"/binaries/command_line_shell",
                command_line_shell::ShellExecutable
            ),
            (
                &"/binaries/settings",
                settings::SettingsExecutable::new(virtual_file_system, task)
                    .await
                    .unwrap()
            ),
            (
                &"/binaries/file_manager",
                file_manager::FileManagerExecutable::new(virtual_file_system, task)
                    .await
                    .unwrap()
            ),
            (
                &"/binaries/terminal",
                terminal::TerminalExecutable::new(virtual_file_system, task)
                    .await
                    .unwrap()
            ),
        ]
    )
    .await
    .unwrap();

    // - Execute the shell
    // - - Open the standard input, output and error
    let standard = Standard::open(
        &"/devices/standard_in",
        &"/devices/standard_out",
        &"/devices/standard_error",
        task,
        virtual_file_system,
        executable_context,
    )
    .await
    .unwrap();

    // - - Load saved accounts, or create defaults only on a fresh disk.
    let authentication_context = authentication::Context {
        virtual_file_system,
        task_manager,
        users_manager,
        task,
    };
    storage::initialize_users(&authentication_context, drive.is_new())
        .await
        .expect("Failed to initialize users and groups");

    drive
        .sync()
        .await
        .expect("Failed to save OPFS disk at boot");
    log::information!("OPFS disk saved at boot");

    // Main-thread OPFS commits are asynchronous. Keep one autosave running and
    // explicitly join it before the final shutdown save.
    use core::sync::atomic::{AtomicBool, Ordering};
    let autosave_running: &'static AtomicBool = Box::leak(Box::new(AtomicBool::new(true)));
    let (autosave, _) = task_manager
        .spawn(task, "OPFS autosave", None, move |_| async move {
            loop {
                task::Manager::sleep(Duration::from_secs(1)).await;
                if !autosave_running.load(Ordering::Relaxed) {
                    break;
                }
                if let Err(error) = drive.sync().await {
                    log::error!("Failed to save OPFS disk: {error}");
                }
            }
        })
        .await
        .unwrap();

    // - - Set the environment variables
    task_manager
        .set_environment_variable(task, "Paths", "/")
        .await
        .unwrap();

    task_manager
        .set_environment_variable(task, "Host", "xila")
        .await
        .unwrap();

    // wait some time to show the bootsplash
    task::Manager::sleep(Duration::from_secs(2)).await;

    bootsplash.stop(graphics_manager).await.unwrap();

    let has_touch_screen = drivers_wasm::devices::graphics::has_touch_screen().unwrap_or(false);

    let arguments = if has_touch_screen {
        log::information!("Touch screen detected.");
        vec!["--show-keyboard".to_string()]
    } else {
        vec![]
    };

    // - - Execute the shell
    let _ = executable::execute(
        executable_context,
        "/binaries/graphical_shell",
        arguments,
        standard,
        None,
    )
    .await
    .unwrap()
    .join()
    .await;

    autosave_running.store(false, Ordering::Relaxed);
    autosave.join().await;
    virtual_file_system.uninitialize().await;
    drive
        .sync()
        .await
        .expect("Failed to save OPFS disk at shutdown");
    drive.unmount().expect("Failed to release OPFS disk");
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    extern crate std;
    panic!("This executable is only for the WebAssembly target.");
}
