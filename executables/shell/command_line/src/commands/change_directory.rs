use crate::{Error, Result};
use alloc::borrow::ToOwned;
use getargs_derive::GetArgs;
use xila::{
    file_system::Path,
    virtual_file_system::{self, Directory},
};

use super::{CommandContext, UserCommand};

pub struct ChangeDirectoryCommand;

impl UserCommand for ChangeDirectoryCommand {
    async fn execute<'a, I, C>(
        &self,
        context: &mut C,
        options: &mut getargs::Options<&'a str, I>,
        _paths: &[&Path],
    ) -> Result<()>
    where
        I: Iterator<Item = &'a str>,
        C: CommandContext,
    {
        execute_change_directory(context, options).await
    }
}

#[derive(GetArgs)]
struct ChangeDirectoryArguments<'a> {
    path: &'a str,
}

async fn execute_change_directory<'a, I, C>(
    context: &mut C,
    options: &mut getargs::Options<&'a str, I>,
) -> Result<()>
where
    I: Iterator<Item = &'a str>,
    C: CommandContext,
{
    let ChangeDirectoryArguments { path } = ChangeDirectoryArguments::parse(options)?;

    let current_directory = resolve_target_directory(context.current_directory(), path)?;

    ensure_directory_exists(context, &current_directory).await?;

    context.set_current_directory(current_directory);

    Ok(())
}

fn resolve_target_directory(
    base_directory: &Path,
    path: &str,
) -> Result<xila::file_system::PathOwned> {
    let target_directory = Path::from_str(path).to_owned();

    let target_directory = if target_directory.is_absolute() {
        target_directory
    } else {
        match base_directory.to_owned().join(&target_directory) {
            Some(path) => path.canonicalize(),
            None => return Err(Error::FailedToJoinPath),
        }
    };

    Ok(target_directory)
}

async fn ensure_directory_exists<C: CommandContext>(context: &C, directory: &Path) -> Result<()> {
    let virtual_file_system = &context.executable_context().virtual_file_system;

    let _ = Directory::open(virtual_file_system, context.task_id(), directory)
        .await
        .map_err(Error::FailedToOpenDirectory)?
        .close(virtual_file_system)
        .await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::resolve_target_directory;
    use xila::file_system::Path;

    #[test]
    fn resolve_target_directory_keeps_absolute_path() {
        let base = Path::from_str("/home/user");

        let resolved = resolve_target_directory(base, "/tmp").unwrap();

        assert_eq!(resolved.as_str(), "/tmp");
    }

    #[test]
    fn resolve_target_directory_resolves_relative_path() {
        let base = Path::from_str("/home/user");

        let resolved = resolve_target_directory(base, "docs").unwrap();

        assert_eq!(resolved.as_str(), "/home/user/docs");
    }

    #[test]
    fn resolve_target_directory_accepts_empty_relative_path_as_base_directory() {
        let base = Path::from_str("/home/user");

        let result = resolve_target_directory(base, "").unwrap();

        assert_eq!(result.as_str(), "/home/user");
    }
}
