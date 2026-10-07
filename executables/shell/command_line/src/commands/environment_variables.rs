use crate::{Error, Result};
use getargs::Options;
use getargs_derive::GetArgs;
use xila::{file_system::Path, task};

use super::{CommandContext, UserCommand};

pub struct SetEnvironmentVariableCommand;
pub struct RemoveEnvironmentVariableCommand;
pub struct PrintEnvironmentVariableCommand;

impl UserCommand for SetEnvironmentVariableCommand {
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
        execute_set_environment_variable(context, options).await
    }
}

impl UserCommand for RemoveEnvironmentVariableCommand {
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
        execute_remove_environment_variable(context, options).await
    }
}

impl UserCommand for PrintEnvironmentVariableCommand {
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
        execute_print_environment_variable(context, options).await
    }
}

#[derive(GetArgs)]
struct SetEnvironmentVariableArguments<'a> {
    argument: &'a str,
}

#[derive(GetArgs)]
struct RemoveEnvironmentVariableArguments<'a> {
    name: &'a str,
}

#[derive(GetArgs)]
struct GetEnvironmentVariableArguments<'a> {
    #[arg(positional, default = "")]
    key: &'a str,
}

async fn execute_set_environment_variable<'a, I, C>(
    context: &mut C,
    options: &mut Options<&'a str, I>,
) -> Result<()>
where
    I: Iterator<Item = &'a str>,
    C: CommandContext,
{
    let SetEnvironmentVariableArguments { argument } =
        SetEnvironmentVariableArguments::parse(options)?;

    let (name, value) = argument.split_once('=').ok_or(Error::InvalidArgument)?;

    context
        .executable_context()
        .task_manager
        .set_environment_variable(context.task_id(), name, value)
        .await
        .map_err(Error::FailedToSetEnvironmentVariable)
}

async fn execute_remove_environment_variable<'a, I, C>(
    context: &mut C,
    options: &mut Options<&'a str, I>,
) -> Result<()>
where
    I: Iterator<Item = &'a str>,
    C: CommandContext,
{
    let RemoveEnvironmentVariableArguments { name } =
        RemoveEnvironmentVariableArguments::parse(options)?;

    context
        .executable_context()
        .task_manager
        .remove_environment_variable(context.task_id(), name)
        .await
        .map_err(Error::FailedToRemoveEnvironmentVariable)
}

async fn execute_print_environment_variable<'a, I, C>(
    context: &mut C,
    options: &mut getargs::Options<&'a str, I>,
) -> Result<()>
where
    I: Iterator<Item = &'a str>,
    C: CommandContext,
{
    let GetEnvironmentVariableArguments { key } = GetEnvironmentVariableArguments::parse(options)?;
    let task_manager = &context.executable_context().task_manager;

    if key.is_empty() {
        let environment_variables = task_manager
            .get_environment_variables(context.task_id())
            .await
            .map_err(|_| crate::Error::FailedToGetTaskIdentifier)?;

        for environment_variable in environment_variables {
            context.write_out_fmt(format_args!(
                "{}={}\n",
                environment_variable.get_name(),
                environment_variable.get_value()
            ))?;
        }
    } else {
        let environment_variable = task_manager
            .get_environment_variable(context.task_id(), key)
            .await
            .map_err(|_| crate::Error::FailedToGetTaskIdentifier)?;

        context.write_out_fmt(format_args!(
            "{}={}\n",
            environment_variable.get_name(),
            environment_variable.get_value()
        ))?;
    }

    Ok(())
}
