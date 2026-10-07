use alloc::borrow::ToOwned;
use getargs::Options;
use xila::{
    file_system::{AccessFlags, Path},
    virtual_file_system::{self, File},
};

use crate::{Error, Result, commands::check_no_more_options};

use super::{CommandContext, UserCommand};

pub struct ConcatenateCommand;

impl UserCommand for ConcatenateCommand {
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
        execute_concatenate(context, options).await
    }
}

fn resolve_path<C: CommandContext>(
    context: &C,
    argument: &str,
) -> Result<xila::file_system::PathOwned> {
    let path = Path::from_str(argument);

    if path.is_absolute() {
        Ok(path.to_owned())
    } else {
        context
            .current_directory_owned()
            .join(path)
            .ok_or(Error::FailedToJoinPath)
    }
}

async fn read_file_and_write<C: CommandContext>(context: &mut C, path: &Path) -> Result<()> {
    let virtual_file_system = &context.executable_context().virtual_file_system;

    let mut file = File::open(
        virtual_file_system,
        context.task_id(),
        path,
        AccessFlags::Read.into(),
    )
    .await
    .map_err(Error::FailedToOpenFile)?;

    let _ = file.display_content(context.standard().out(), 256).await;

    Ok(())
}

async fn execute_concatenate<'a, I, C>(
    context: &mut C,
    options: &mut Options<&'a str, I>,
) -> Result<()>
where
    I: Iterator<Item = &'a str>,
    C: CommandContext,
{
    check_no_more_options(options)?;

    while let Some(argument) = options.next_positional() {
        let path = resolve_path(context, argument)?;
        read_file_and_write(context, &path).await?;
    }

    context.write_out_fmt(format_args!("\n"))?;

    Ok(())
}
