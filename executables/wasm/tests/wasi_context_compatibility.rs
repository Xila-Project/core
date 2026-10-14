#![cfg(all(
    feature = "host",
    any(target_os = "linux", target_os = "macos", target_os = "windows")
))]

use wasm::WasiContext;

#[test]
fn legacy_borrowed_accessors_remain_callable_through_the_public_context() {
    let mut context = WasiContext {
        files: Default::default(),
        next_fd: 3,
        arguments: Default::default(),
        environment: Default::default(),
        task: xila::task::Manager::ROOT_TASK_IDENTIFIER,
        prestats: Default::default(),
        exit_code: None,
    };

    assert!(
        context
            .get_file_system_item(0)
            .and_then(|item| item.into_synchronous_file())
            .is_none()
    );
    assert!(
        context
            .get_file_system_item(0)
            .and_then(|item| item.into_synchronous_directory())
            .is_none()
    );
}
