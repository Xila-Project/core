use crate::{Error, Result};
use getargs_derive::GetArgs;
use xila::file_system::{Path, Permissions};

use super::{CommandContext, UserCommand, directory::resolve_path};

pub struct ChmodCommand;

#[derive(GetArgs)]
#[command(
    name = "chmod",
    about = "Change file or directory permissions. MODE is octal 000-777 (an optional leading zero is accepted). Symbolic modes, special bits and recursion are unsupported."
)]
struct ChmodArguments<'a> {
    mode: &'a str,
    path: &'a str,
}

fn parse_mode(mode: &str) -> Result<Permissions> {
    // Check syntax before parsing: from_str_radix also accepts a leading '+'.
    if mode.is_empty() || mode.len() > 4 || !mode.bytes().all(|byte| matches!(byte, b'0'..=b'7')) {
        return Err(Error::InvalidArgument);
    }
    u16::from_str_radix(mode, 8)
        .ok()
        .and_then(Permissions::from_octal)
        .ok_or(Error::InvalidArgument)
}

impl UserCommand for ChmodCommand {
    async fn execute<'a, I, C>(
        &self,
        context: &mut C,
        options: &mut getargs::Options<&'a str, I>,
        _paths: &[&Path],
    ) -> Result<(), crate::Error<'a>>
    where
        I: Iterator<Item = &'a str>,
        C: CommandContext,
    {
        let ChmodArguments { mode, path } = ChmodArguments::parse(options)?;
        let permissions = parse_mode(mode)?;
        let path = resolve_path(context, path)?;
        context
            .executable_context()
            .virtual_file_system
            .set_permissions(context.task_id(), &path, permissions)
            .await
            .map_err(Error::FailedToSetPermissions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn octal_modes_validate_syntax_and_supported_bits() {
        for (mode, expected) in [
            ("0", 0),
            ("000", 0),
            ("644", 0o644),
            ("0755", 0o755),
            ("777", 0o777),
        ] {
            assert_eq!(parse_mode(mode).unwrap().as_u16(), expected);
        }
        for mode in [
            "", "888", "-1", "+644", "u+x", "0o755", "1000", "4755", "7777", "00000", " 644",
        ] {
            assert!(parse_mode(mode).is_err(), "accepted {mode}");
        }
    }

    #[test]
    fn requires_mode_and_exactly_one_path() {
        for args in [
            alloc::vec![],
            alloc::vec!["644"],
            alloc::vec!["644", "a", "b"],
            alloc::vec!["-R", "644", "a"],
        ] {
            assert!(ChmodArguments::parse(&mut getargs::Options::new(args.into_iter())).is_err());
        }
    }
}

#[cfg(all(
    test,
    any(target_os = "linux", target_os = "macos", target_os = "windows")
))]
mod filesystem_tests {
    use super::*;
    use alloc::borrow::ToOwned;
    use xila::{
        executable::{ExecutableContext, Standard},
        file_system::{AccessFlags, CreateFlags, Flags, PathOwned},
        task::TaskIdentifier,
        users::{GroupIdentifier, UserIdentifier},
        virtual_file_system::{Directory, File},
    };

    struct Context {
        standard: Standard<'static>,
        task: TaskIdentifier,
    }

    impl CommandContext for Context {
        fn executable_context(&self) -> &'static ExecutableContext {
            self.standard.context
        }
        fn task_id(&self) -> TaskIdentifier {
            self.task
        }
        fn current_directory(&self) -> &Path {
            Path::from_str("/chmod_test")
        }
        fn set_current_directory(&mut self, _: PathOwned) {
            panic!("unexpected directory change")
        }
        fn stop(&mut self) {
            panic!("unexpected exit")
        }
        fn write_out_fmt(&mut self, _: core::fmt::Arguments<'_>) -> Result<()> {
            Ok(())
        }
        async fn write_out(&mut self, _: &[u8]) {}
        async fn write_out_line(&mut self, _: &[u8]) {}
        fn standard(&mut self) -> &mut Standard<'static> {
            &mut self.standard
        }
    }

    async fn chmod<'a>(
        context: &mut Context,
        mode: &'a str,
        path: &'a str,
    ) -> Result<(), Error<'a>> {
        ChmodCommand
            .execute(
                context,
                &mut getargs::Options::new([mode, path].into_iter()),
                &[],
            )
            .await
    }

    #[xila::task::test(task_path = xila::task)]
    async fn changes_file_and_directory_modes_and_enforces_ownership() {
        let standard = testing::initialize(false, false).await;
        let services = standard.context;
        let task = services.task_manager.get_current_task_identifier().await;
        let vfs = services.virtual_file_system;
        Directory::create(vfs, task, "/chmod_test").await.unwrap();
        Directory::create(vfs, task, "/chmod_test/directory")
            .await
            .unwrap();
        File::open(
            vfs,
            task,
            "/chmod_test/file",
            Flags::new(AccessFlags::Write, Some(CreateFlags::Create), None),
        )
        .await
        .unwrap()
        .close(vfs)
        .await
        .unwrap();
        let mut context = Context { standard, task };
        chmod(&mut context, "0640", "file").await.unwrap();
        chmod(&mut context, "700", "/chmod_test/directory")
            .await
            .unwrap();
        assert_eq!(
            vfs.get_statistics(&"/chmod_test/file")
                .await
                .unwrap()
                .permissions
                .as_u16(),
            0o640
        );
        assert_eq!(
            vfs.get_statistics(&"/chmod_test/directory")
                .await
                .unwrap()
                .permissions
                .as_u16(),
            0o700
        );
        assert!(chmod(&mut context, "888", "file").await.is_err());
        assert!(matches!(
            chmod(&mut context, "644", "missing").await,
            Err(Error::FailedToSetPermissions(_))
        ));
        let owner = UserIdentifier::new(263);
        let other = UserIdentifier::new(264);
        for user in [owner, other] {
            services
                .users_manager
                .add_user(
                    user,
                    if user == owner {
                        "chmod_owner"
                    } else {
                        "chmod_other"
                    },
                    GroupIdentifier::ROOT,
                )
                .await
                .unwrap();
        }
        vfs.set_ownership(task, &"/chmod_test/file", Some(owner), None)
            .await
            .unwrap();
        vfs.set_ownership(task, &"/chmod_test/directory", Some(owner), None)
            .await
            .unwrap();
        services.task_manager.set_user(task, owner).await.unwrap();
        chmod(&mut context, "600", "file").await.unwrap();
        chmod(&mut context, "750", "directory").await.unwrap();
        services.task_manager.set_user(task, other).await.unwrap();
        assert!(matches!(
            chmod(&mut context, "777", "file").await,
            Err(Error::FailedToSetPermissions(
                xila::virtual_file_system::Error::PermissionDenied
            ))
        ));
        assert!(matches!(
            chmod(&mut context, "777", "directory").await,
            Err(Error::FailedToSetPermissions(
                xila::virtual_file_system::Error::PermissionDenied
            ))
        ));
        assert_eq!(
            vfs.get_statistics(&"/chmod_test/file")
                .await
                .unwrap()
                .permissions
                .as_u16(),
            0o600
        );
        services
            .task_manager
            .set_user(task, UserIdentifier::ROOT)
            .await
            .unwrap();
        chmod(&mut context, "000", "file").await.unwrap();
        assert_eq!(
            vfs.get_statistics(&"/chmod_test/file")
                .await
                .unwrap()
                .permissions,
            Permissions::NONE
        );
        assert_eq!(
            resolve_path(&context, "./directory").unwrap(),
            Path::from_str("/chmod_test/directory").to_owned()
        );
    }
}
