use super::{CommandContext, UserCommand};
use crate::Result;
use getargs::Options;
use xila::file_system::Path;
pub struct EchoCommand;
impl UserCommand for EchoCommand {
    async fn execute<'a, I, C>(
        &self,
        context: &mut C,
        arguments: &mut Options<&'a str, I>,
        _paths: &[&Path],
    ) -> Result<(), crate::Error<'a>>
    where
        I: Iterator<Item = &'a str>,
        C: CommandContext,
    {
        let mut first = true;
        while let Some(argument) = arguments.next_positional() {
            if !first {
                context.write_out(b" ").await?;
            }
            context.write_out(argument.as_bytes()).await?;
            first = false;
        }
        context.write_out(b"\n").await
    }
}
