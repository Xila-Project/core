use xila::file_system::Path;

use super::{CommandContext, UserCommand};

pub struct PrintWorkingDirectoryCommand;

impl UserCommand for PrintWorkingDirectoryCommand {
    async fn execute<'a, I, C>(
        &self,
        context: &mut C,
        _options: &mut getargs::Options<&'a str, I>,
        _paths: &[&Path],
    ) -> crate::Result<(), crate::Error<'a>>
    where
        I: Iterator<Item = &'a str>,
        C: CommandContext,
    {
        let current_directory = context.current_directory_owned();
        context
            .write_out_fmt(format_args!("{}\n", current_directory.as_str()))
            .await?;
        Ok(())
    }
}
