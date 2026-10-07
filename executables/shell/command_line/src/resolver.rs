use crate::error::{Error, Result};
use xila::{
    file_system::{Path, PathOwned},
    task,
    virtual_file_system::{self, Directory},
};

pub async fn resolve(
    executable_context: &'static xila::executable::ExecutableContext,
    command: &str,
    paths: &[&Path],
) -> Result<PathOwned> {
    let virtual_file_system = &executable_context.virtual_file_system;
    let task = executable_context
        .task_manager
        .get_current_task_identifier()
        .await;

    for path in paths {
        if let Ok(mut directory) = Directory::open(virtual_file_system, task, path).await {
            while let Ok(Some(entry)) = directory.read().await {
                if entry.name == command {
                    return path.append(command).ok_or(Error::InvalidPath);
                }
            }
        }
    }

    Err(Error::CommandNotFound)
}

#[cfg(test)]
mod tests {
    use super::resolve;
    use crate::Error;
    use core::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;

    drivers_std::memory::instantiate_global_allocator!();

    use xila::{
        file_system::{AccessFlags, CreateFlags, Flags, Path},
        task,
        virtual_file_system::{self, Directory, File},
    };

    static TESTING_INITIALIZED: AtomicBool = AtomicBool::new(false);
    static TESTING_INITIALIZATION_LOCK: Mutex<()> = Mutex::new(());

    async fn initialize_testing() {
        if TESTING_INITIALIZED.load(Ordering::Acquire) {
            return;
        }

        let _guard = TESTING_INITIALIZATION_LOCK
            .lock()
            .expect("testing initialization lock poisoned");

        if !TESTING_INITIALIZED.load(Ordering::Acquire) {
            let _ = testing::initialize(false, true).await;
            TESTING_INITIALIZED.store(true, Ordering::Release);
        }
    }

    async fn create_file(path: &str) {
        let standard = testing::initialize(false, true).await;
        let context = standard.context;
        let virtual_file_system = &context.virtual_file_system;
        let task = context.task_manager.get_current_task_identifier().await;

        let file = File::open(
            virtual_file_system,
            task,
            Path::from_str(path),
            Flags::new(AccessFlags::Write, Some(CreateFlags::Create), None),
        )
        .await
        .unwrap();

        file.close(virtual_file_system).await.unwrap();
    }

    async fn create_directory(path: &str) {
        let standard = testing::initialize(false, true).await;
        let context = standard.context;
        let virtual_file_system = &context.virtual_file_system;
        let task = context.task_manager.get_current_task_identifier().await;

        Directory::create(virtual_file_system, task, Path::from_str(path))
            .await
            .unwrap();
    }

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    #[xila::task::test(task_path = xila::task)]
    async fn resolve_returns_path_from_later_search_directory() {
        initialize_testing().await;

        create_directory("/resolver_test_a").await;
        create_directory("/resolver_test_b").await;
        create_file("/resolver_test_b/hello").await;

        let standard = testing::initialize(false, true).await;
        let executable_context = standard.context;
        let result = resolve(
            executable_context,
            "hello",
            &[
                Path::from_str("/resolver_test_a"),
                Path::from_str("/resolver_test_b"),
            ],
        )
        .await
        .unwrap();

        assert_eq!(result.as_str(), "/resolver_test_b/hello");
    }

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    #[xila::task::test(task_path = xila::task)]
    async fn resolve_prefers_first_matching_directory() {
        initialize_testing().await;

        create_directory("/resolver_test_first").await;
        create_directory("/resolver_test_second").await;
        create_file("/resolver_test_first/tool").await;
        create_file("/resolver_test_second/tool").await;

        let standard = testing::initialize(false, true).await;
        let executable_context = standard.context;
        let result = resolve(
            executable_context,
            "tool",
            &[
                Path::from_str("/resolver_test_first"),
                Path::from_str("/resolver_test_second"),
            ],
        )
        .await
        .unwrap();

        assert_eq!(result.as_str(), "/resolver_test_first/tool");
    }

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    #[xila::task::test(task_path = xila::task)]
    async fn resolve_returns_command_not_found_for_unknown_command() {
        initialize_testing().await;

        create_directory("/resolver_test_empty").await;

        let standard = testing::initialize(false, true).await;
        let result = resolve(
            standard.context,
            "missing_command",
            &[Path::from_str("/resolver_test_empty")],
        )
        .await;

        assert!(matches!(result, Err(Error::CommandNotFound)));
    }
}
