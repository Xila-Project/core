use crate::Result;
use crate::error::Error;
use getargs::Options;
use xila::file_system::Path;

use super::{CommandContext, UserCommand};

pub struct EchoCommand;

impl UserCommand for EchoCommand {
    async fn execute<'a, I, C>(
        &self,
        context: &mut C,
        arguments: &mut Options<&'a str, I>,
        _paths: &[&Path],
    ) -> Result<()>
    where
        I: Iterator<Item = &'a str>,
        C: CommandContext,
    {
        execute_echo(context, arguments).await
    }
}

async fn write_echo_argument<C: CommandContext>(context: &mut C, argument: &str) -> Result<()> {
    if let Some(name) = argument.strip_prefix('$') {
        let environment_variable = context
            .executable_context()
            .task_manager
            .get_environment_variable(context.task_id(), name)
            .await
            .map_err(Error::FailedToReadEnvironmentVariable)?;

        context.write_out_fmt(format_args!("{} ", environment_variable.get_value()))?;
    } else {
        let argument = argument.trim_matches('"');
        context.write_out_fmt(format_args!("{} ", argument))?;
    }

    Ok(())
}

async fn execute_echo<'a, I, C>(context: &mut C, arguments: &mut Options<&'a str, I>) -> Result<()>
where
    I: Iterator<Item = &'a str>,
    C: CommandContext,
{
    while let Some(argument) = arguments.next_positional() {
        write_echo_argument(context, argument).await?;
    }

    context.write_out_fmt(format_args!("\n"))?;

    Ok(())
}
