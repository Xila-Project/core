use alloc::borrow::ToOwned;
use getargs::Options;
use getargs_derive::GetArgs;
use xila::{file_system::Path, virtual_file_system};

use crate::{Result, error::Error, resolver::resolve};

use super::{CommandContext, UserCommand};

pub struct WhichCommand;

impl UserCommand for WhichCommand {
    async fn execute<'a, I, C>(
        &self,
        context: &mut C,
        options: &mut Options<&'a str, I>,
        paths: &[&Path],
    ) -> Result<()>
    where
        I: Iterator<Item = &'a str>,
        C: CommandContext,
    {
        execute_which(context, options, paths).await
    }
}

#[derive(GetArgs)]
struct WhichArguments<'a> {
    command: &'a str,
}

async fn validate_and_print_path<C: CommandContext>(
    context: &mut C,
    resolved_path: &Path,
) -> Result<()> {
    let _ = context
        .executable_context()
        .virtual_file_system
        .get_statistics(&resolved_path)
        .await
        .map_err(Error::FailedToGetMetadata)?;

    context.write_out_fmt(format_args!("{}\n", resolved_path.as_str()))?;

    Ok(())
}

fn resolve_as_path<C: CommandContext>(
    context: &C,
    command: &str,
) -> Result<Option<xila::file_system::PathOwned>> {
    let path = Path::from_str(command);

    if !path.is_valid() {
        return Ok(None);
    }

    if path.is_absolute() {
        Ok(Some(path.to_owned()))
    } else {
        Ok(Some(
            context
                .current_directory_owned()
                .join(path)
                .ok_or(Error::FailedToJoinPath)?,
        ))
    }
}

async fn execute_which<'a, I, C>(
    context: &mut C,
    options: &mut Options<&'a str, I>,
    paths: &[&Path],
) -> Result<()>
where
    I: Iterator<Item = &'a str>,
    C: CommandContext,
{
    let WhichArguments { command } = WhichArguments::parse(options)?;

    if let Some(path) = resolve_as_path(context, command)? {
        validate_and_print_path(context, &path).await?;
    } else {
        let path = resolve(context.executable_context(), command, paths).await?;
        validate_and_print_path(context, &path).await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::resolve_as_path;
    use crate::{Result, commands::CommandContext};
    use alloc::borrow::ToOwned;
    use core::fmt;
    use xila::{
        executable::Standard,
        file_system::{Path, PathOwned},
        task::TaskIdentifier,
    };

    struct FakeContext {
        current_directory: PathOwned,
    }

    impl CommandContext for FakeContext {
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
    fn resolve_as_path_returns_none_for_command_name_without_path_separator() {
        let context = FakeContext {
            current_directory: Path::from_str("/base").to_owned(),
        };

        let resolved = resolve_as_path(&context, "ls").unwrap();

        assert_eq!(resolved.unwrap().as_str(), "/base/ls");
    }

    #[test]
    fn resolve_as_path_returns_none_for_invalid_command_token() {
        let context = FakeContext {
            current_directory: Path::from_str("/base").to_owned(),
        };

        let resolved = resolve_as_path(&context, "bad command").unwrap();

        assert!(resolved.is_none());
    }

    #[test]
    fn resolve_as_path_keeps_absolute_path() {
        let context = FakeContext {
            current_directory: Path::from_str("/base").to_owned(),
        };

        let resolved = resolve_as_path(&context, "/bin/ls").unwrap().unwrap();

        assert_eq!(resolved.as_str(), "/bin/ls");
    }

    #[test]
    fn resolve_as_path_expands_relative_path() {
        let context = FakeContext {
            current_directory: Path::from_str("/base").to_owned(),
        };

        let resolved = resolve_as_path(&context, "tool").unwrap().unwrap();

        assert_eq!(resolved.as_str(), "/base/tool");
    }
}
