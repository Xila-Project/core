use crate::Result;
use getargs::Options;
use getargs_derive::GetArgs;
use xila::file_system::Path;

use super::{CommandContext, UserCommand};

pub struct ClearCommand;

impl UserCommand for ClearCommand {
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
        execute_clear(context, options).await
    }
}

#[derive(GetArgs)]
#[command(name = "clear", about = "Clear the terminal.")]
struct ClearArguments {}

async fn execute_clear<'a, I, C>(
    context: &mut C,
    options: &mut Options<&'a str, I>,
) -> Result<(), crate::Error<'a>>
where
    I: Iterator<Item = &'a str>,
    C: CommandContext,
{
    let _ = ClearArguments::parse(options)?;

    context.write_out_fmt(format_args!("\x1B[2J")).await?;
    context.write_out_fmt(format_args!("\x1B[H")).await?;

    Ok(())
}
