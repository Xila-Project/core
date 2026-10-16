use getargs::Options;
use getargs_derive::GetArgs;
use xila::file_system::Path;

use crate::Result;

use super::{CommandContext, UserCommand};

pub struct ExitCommand;

impl UserCommand for ExitCommand {
    async fn execute<'a, I, C>(
        &self,
        context: &mut C,
        options: &mut Options<&'a str, I>,
        _paths: &[&Path],
    ) -> Result<(), crate::Error<'a>>
    where
        I: Iterator<Item = &'a str>,
        C: CommandContext,
    {
        execute_exit(context, options)
    }
}

#[derive(GetArgs)]
#[command(name = "exit", about = "Exit the shell.")]
pub(super) struct ExitArguments {}

fn execute_exit<'a, I, C>(
    context: &mut C,
    options: &mut Options<&'a str, I>,
) -> Result<(), crate::Error<'a>>
where
    I: Iterator<Item = &'a str>,
    C: CommandContext,
{
    let _ = ExitArguments::parse(options)?;

    context.stop();

    Ok(())
}
