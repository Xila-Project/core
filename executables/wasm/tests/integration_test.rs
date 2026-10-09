#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[cfg(all(feature = "host", feature = "graphics"))]
#[xila::task::test(task_path = xila::task)]
#[ignore = "This test is meant to be run interactively"]
async fn main() {
    drivers_std::memory::instantiate_global_allocator!();

    extern crate alloc;
    extern crate abi_definitions;

    use command_line_shell::ShellExecutable;
    use drivers_std::loader::load_to_virtual_file_system;
    use wasm::WasmExecutable;
    use xila::executable::{build_crate, mount_executables};

    let standard = testing::initialize(true, false).await;
    let context = standard.context;
    let virtual_file_system = &context.virtual_file_system;
    let task = context.task_manager.get_current_task_identifier().await;

    let binary_path = build_crate(&"wasm_wasm_test").unwrap();
    load_to_virtual_file_system(
        context.task_manager,
        virtual_file_system,
        binary_path,
        "/test_wasm.wasm",
    )
    .await
    .unwrap();

    mount_executables!(
        context,
        virtual_file_system,
        task,
        &[
            ("/binaries/command_line_shell", ShellExecutable),
            ("/binaries/wasm", WasmExecutable)
        ]
    )
    .await
    .unwrap();

    let result = executable::execute(
        context,
        "/binaries/wasm",
        vec!["/test_wasm.wasm".to_string()],
        standard,
        None,
    )
    .await
    .unwrap()
    .join()
    .await;

    //    let result = executable::execute("/binaries/command_line_shell", vec![], standard, None)
    //        .await
    //        .unwrap()
    //        .join()
    //        .await;

    assert!(result == 0);
}
