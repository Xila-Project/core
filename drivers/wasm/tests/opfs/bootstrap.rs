use super::*;
use file_system::{
    XILA_DISK_SIGNATURE,
    mbr::{Mbr, PartitionKind},
};

#[wasm_bindgen_test]
async fn existing_invalid_mbr_is_not_reformatted() {
    let name = name();
    let drive = DriveDevice::new(Path::from_str(&name), 4 * 1024 * 1024)
        .await
        .unwrap();
    drive.write(&[0x42; 512], 0).unwrap();
    drive.sync().await.unwrap();
    drop(drive);
    let drive = Box::leak(Box::new(
        DriveDevice::new(Path::from_str(&name), 4 * 1024 * 1024)
            .await
            .unwrap(),
    ));
    assert!(
        storage::mount(drive).is_err(),
        "Existing invalid MBR must not trigger formatting"
    );
    drive.sync().await.unwrap();
    let mut bytes = [0; 512];
    drive.read(&mut bytes, 0).unwrap();
    assert_eq!(bytes, [0x42; 512]);
    drive.unmount().unwrap();
    remove(&name).await;
}

#[wasm_bindgen_test]
async fn existing_unmountable_filesystem_is_not_reformatted() {
    let name = name();
    let drive = DriveDevice::new(Path::from_str(&name), 4 * 1024 * 1024)
        .await
        .unwrap();
    let partition = Mbr::format_disk_with_signature_and_partition(
        &drive,
        XILA_DISK_SIGNATURE,
        PartitionKind::Xila,
    )
    .unwrap();
    partition.write(&[0x42; 1024], 0).unwrap();
    drive.sync().await.unwrap();
    drop(drive);
    let drive = Box::leak(Box::new(
        DriveDevice::new(Path::from_str(&name), 4 * 1024 * 1024)
            .await
            .unwrap(),
    ));
    assert!(
        storage::mount(drive).is_err(),
        "Existing invalid filesystem must not trigger formatting"
    );
    drive.sync().await.unwrap();
    let mut bytes = [0; 1024];
    drive.read(&mut bytes, 2048 * 512).unwrap();
    assert_eq!(bytes, [0x42; 1024]);
    drive.unmount().unwrap();
    remove(&name).await;
}

type Completion = synchronization::signal::Signal<
    synchronization::blocking_mutex::raw::CriticalSectionRawMutex,
    Result<(), String>,
>;

#[embassy_executor::task]
async fn check_saved_accounts(completion: &'static Completion) {
    completion.signal(saved_accounts_fixture().await);
}

async fn saved_accounts_fixture() -> Result<(), String> {
    use xila::{authentication, task, time, users, virtual_file_system};
    let name = name();
    let drive = Box::leak(Box::new(
        DriveDevice::new(Path::from_str(&name), 4 * 1024 * 1024)
            .await
            .map_err(|e| e.to_string())?,
    ));
    let task_manager = Box::leak(Box::new(task::Manager::new()));
    let task = task_manager.get_current_task_identifier().await;
    let users_manager = Box::leak(Box::new(users::Manager::new()));
    let time_manager = Box::leak(Box::new(
        time::Manager::new(&drivers_wasm::devices::TimeDevice).map_err(|e| e.to_string())?,
    ));
    let vfs = virtual_file_system::initialize(
        task_manager,
        users_manager,
        time_manager,
        storage::mount(drive).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    virtual_file_system::create_default_hierarchy(&vfs, task)
        .await
        .map_err(|e| e.to_string())?;
    for directory in ["/system/groups", "/system/users"] {
        vfs.create_directory(task, &directory)
            .await
            .map_err(|e| e.to_string())?;
    }
    let group = br#"{"identifier":1000,"name":"administrator","users":[42]}"#;
    let user = br#"{"identifier":42,"name":"bob","primary_group":1000,"hash":"saved-hash","salt":"saved-salt"}"#;
    for (path, bytes) in [
        ("/system/groups/administrator", group.as_slice()),
        ("/system/users/bob", user.as_slice()),
    ] {
        virtual_file_system::File::write_to_path(&vfs, task, &path, bytes)
            .await
            .map_err(|e| e.to_string())?;
    }
    let context = authentication::Context {
        virtual_file_system: &vfs,
        task_manager,
        users_manager,
        task,
    };
    storage::initialize_users(&context, false)
        .await
        .map_err(|e| e.to_string())?;
    let loaded = users_manager
        .get_user_name(users::UserIdentifier::new(42))
        .await
        .map_err(|e| e.to_string())?;
    if loaded != "bob" {
        return Err("Saved non-administrator user was not loaded".into());
    }
    let mut bytes = Vec::new();
    virtual_file_system::File::read_from_path(
        &vfs,
        task,
        &"/system/groups/administrator",
        &mut bytes,
    )
    .await
    .map_err(|e| e.to_string())?;
    if bytes != group {
        return Err("Saved administrator group was overwritten".into());
    }
    vfs.uninitialize().await;
    drop(vfs);
    drive.sync().await.map_err(|e| e.to_string())?;
    drive.unmount().map_err(|e| e.to_string())?;
    remove(&name).await;
    Ok(())
}

#[wasm_bindgen_test]
async fn saved_accounts_are_loaded_even_without_an_administrator_file() {
    let completion = Box::leak(Box::new(Completion::new()));
    let executor = Box::leak(Box::new(drivers_wasm::executor::Executor::new()));
    executor.start(|spawner| spawner.spawn(check_saved_accounts(completion).unwrap()));
    assert_eq!(completion.wait().await, Ok(()));
}
