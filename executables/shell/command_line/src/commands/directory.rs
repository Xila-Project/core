use crate::{Error, Result};
use alloc::borrow::ToOwned;
use getargs::Options;
use getargs_derive::GetArgs;
use xila::{
    file_system::{Kind, Path},
    virtual_file_system::Directory,
};

use super::{CommandContext, UserCommand};

pub struct CreateDirectoryCommand;
pub struct RemoveFileCommand;
pub struct RemoveDirectoryCommand;

impl UserCommand for CreateDirectoryCommand {
    async fn execute<'a, I, C>(
        &self,
        context: &mut C,
        options: &mut Options<&'a str, I>,
        _paths: &[&Path],
    ) -> Result<()>
    where
        I: Iterator<Item = &'a str>,
        C: CommandContext,
    {
        execute_create_directory(context, options).await
    }
}

impl UserCommand for RemoveFileCommand {
    async fn execute<'a, I, C>(
        &self,
        context: &mut C,
        options: &mut Options<&'a str, I>,
        _paths: &[&Path],
    ) -> Result<()>
    where
        I: Iterator<Item = &'a str>,
        C: CommandContext,
    {
        execute_remove_file(context, options).await
    }
}

impl UserCommand for RemoveDirectoryCommand {
    async fn execute<'a, I, C>(
        &self,
        context: &mut C,
        options: &mut Options<&'a str, I>,
        _paths: &[&Path],
    ) -> Result<()>
    where
        I: Iterator<Item = &'a str>,
        C: CommandContext,
    {
        execute_remove_directory(context, options).await
    }
}

#[derive(GetArgs)]
struct DirectoryCreateArguments<'a> {
    path: &'a str,
}

#[derive(GetArgs)]
struct DirectoryRemoveArguments<'a> {
    path: &'a str,
}

fn resolve_path<C: CommandContext>(
    context: &C,
    path: &str,
) -> Result<xila::file_system::PathOwned> {
    let path = Path::from_str(path);

    if !path.is_valid() {
        return Err(Error::InvalidPath);
    }

    if path.is_absolute() {
        Ok(path.to_owned())
    } else {
        context
            .current_directory_owned()
            .join(path)
            .map(|path| path.canonicalize())
            .ok_or(Error::FailedToJoinPath)
    }
}

fn can_remove_with_rm(kind: Kind) -> bool {
    kind != Kind::Directory
}

fn can_remove_with_rmdir(kind: Kind) -> bool {
    kind == Kind::Directory
}

async fn execute_create_directory<'a, I, C>(
    context: &mut C,
    options: &mut Options<&'a str, I>,
) -> Result<()>
where
    I: Iterator<Item = &'a str>,
    C: CommandContext,
{
    let DirectoryCreateArguments { path } = DirectoryCreateArguments::parse(options)?;

    let path = resolve_path(context, path)?;

    Directory::create(
        context.executable_context().virtual_file_system,
        context.task_id(),
        &path,
    )
    .await
    .map_err(Error::FailedToCreateDirectory)
}

async fn execute_remove_file<'a, I, C>(
    context: &mut C,
    options: &mut Options<&'a str, I>,
) -> Result<()>
where
    I: Iterator<Item = &'a str>,
    C: CommandContext,
{
    let DirectoryRemoveArguments { path } = DirectoryRemoveArguments::parse(options)?;
    let path = resolve_path(context, path)?;

    let statistics = context
        .executable_context()
        .virtual_file_system
        .get_statistics(&path)
        .await
        .map_err(Error::FailedToGetMetadata)?;

    if !can_remove_with_rm(statistics.kind) {
        return Err(Error::InvalidArgument);
    }

    context
        .executable_context()
        .virtual_file_system
        .remove(context.task_id(), &path)
        .await
        .map_err(Error::FailedToRemoveDirectory)
}

async fn execute_remove_directory<'a, I, C>(
    context: &mut C,
    options: &mut Options<&'a str, I>,
) -> Result<()>
where
    I: Iterator<Item = &'a str>,
    C: CommandContext,
{
    let DirectoryRemoveArguments { path } = DirectoryRemoveArguments::parse(options)?;
    let path = resolve_path(context, path)?;

    let statistics = context
        .executable_context()
        .virtual_file_system
        .get_statistics(&path)
        .await
        .map_err(Error::FailedToGetMetadata)?;

    if !can_remove_with_rmdir(statistics.kind) {
        return Err(Error::InvalidArgument);
    }

    context
        .executable_context()
        .virtual_file_system
        .remove(context.task_id(), &path)
        .await
        .map_err(Error::FailedToRemoveDirectory)
}

#[cfg(test)]
mod tests {
    use super::{can_remove_with_rm, can_remove_with_rmdir, resolve_path};
    use crate::{Error, Result};
    use alloc::borrow::ToOwned;
    use core::fmt;
    use xila::{
        executable::Standard,
        file_system::{Kind, Path, PathOwned},
        task::TaskIdentifier,
    };

    struct FakeContext {
        current_directory: PathOwned,
    }

    impl super::CommandContext for FakeContext {
        fn task_id(&self) -> TaskIdentifier {
            TaskIdentifier::new(1)
        }

        fn current_directory(&self) -> &Path {
            &self.current_directory
        }

        fn set_current_directory(&mut self, directory: PathOwned) {
            self.current_directory = directory;
        }

        fn stop(&mut self) {}

        fn write_out_fmt(&mut self, _arguments: fmt::Arguments<'_>) -> Result<()> {
            Ok(())
        }

        async fn write_out(&mut self, _buffer: &[u8]) {}

        async fn write_out_line(&mut self, _buffer: &[u8]) {}

        fn executable_context(&self) -> &'static xila::executable::ExecutableContext {
            panic!("executable context not needed in this test")
        }

        fn standard(&mut self) -> &mut Standard<'static> {
            panic!("standard not needed in this test")
        }
    }

    #[test]
    fn resolve_path_keeps_absolute_path() {
        let context = FakeContext {
            current_directory: Path::from_str("/base").to_owned(),
        };

        let path = resolve_path(&context, "/tmp").unwrap();

        assert_eq!(path.as_str(), "/tmp");
    }

    #[test]
    fn resolve_path_resolves_relative_path() {
        let context = FakeContext {
            current_directory: Path::from_str("/base").to_owned(),
        };

        let path = resolve_path(&context, "folder").unwrap();

        assert_eq!(path.as_str(), "/base/folder");
    }

    #[test]
    fn resolve_path_rejects_path_with_invalid_character() {
        let context = FakeContext {
            current_directory: Path::from_str("/base").to_owned(),
        };

        let result = resolve_path(&context, "bad path");

        assert!(matches!(result, Err(Error::InvalidPath)));
    }

    #[test]
    fn rm_rejects_directories() {
        assert!(!can_remove_with_rm(Kind::Directory));
    }

    #[test]
    fn rm_accepts_files() {
        assert!(can_remove_with_rm(Kind::File));
    }

    #[test]
    fn rmdir_accepts_directories_only() {
        assert!(can_remove_with_rmdir(Kind::Directory));
        assert!(!can_remove_with_rmdir(Kind::File));
    }
}
