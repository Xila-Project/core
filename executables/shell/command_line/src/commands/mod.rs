mod authentication;
mod change_directory;
mod clear;
mod concatenate;
mod directory;
mod dns;
mod echo;
mod environment_variables;
mod execute;
mod exit;
mod head;
mod ip;
mod list;
mod ping;
mod print_working_directory;
mod statistics;
mod tail;
mod touch;
mod web_request;
mod which;
mod word_count;

use crate::Result;
use alloc::borrow::ToOwned;
use core::fmt;
use xila::file_system::Path;
use xila::{executable::Standard, file_system::PathOwned, task::TaskIdentifier};

use self::{
    change_directory::ChangeDirectoryCommand,
    clear::ClearCommand,
    concatenate::ConcatenateCommand,
    directory::{CreateDirectoryCommand, RemoveDirectoryCommand, RemoveFileCommand},
    dns::DnsResolveCommand,
    echo::EchoCommand,
    environment_variables::{
        PrintEnvironmentVariableCommand, RemoveEnvironmentVariableCommand,
        SetEnvironmentVariableCommand,
    },
    exit::ExitCommand,
    head::HeadCommand,
    ip::IpCommand,
    list::ListCommand,
    ping::PingCommand,
    print_working_directory::PrintWorkingDirectoryCommand,
    statistics::StatisticsCommand,
    tail::TailCommand,
    touch::TouchCommand,
    web_request::WebRequestCommand,
    which::WhichCommand,
    word_count::WordCountCommand,
};

pub trait CommandContext {
    fn task_id(&self) -> TaskIdentifier;
    fn current_directory(&self) -> &Path;
    fn set_current_directory(&mut self, directory: PathOwned);
    fn stop(&mut self);
    fn write_out_fmt(&mut self, arguments: fmt::Arguments<'_>) -> Result<()>;
    async fn write_out(&mut self, buffer: &[u8]);
    async fn write_out_line(&mut self, buffer: &[u8]);
    fn standard(&mut self) -> &mut Standard;
    fn current_directory_owned(&self) -> PathOwned {
        self.current_directory().to_owned()
    }
}

pub trait UserCommand {
    async fn execute<'a, I, C>(
        &self,
        context: &mut C,
        options: &mut getargs::Options<&'a str, I>,
        paths: &[&Path],
    ) -> Result<()>
    where
        I: Iterator<Item = &'a str>,
        C: CommandContext;
}

#[derive(Clone, Copy)]
pub enum UserCommandKind {
    Exit,
    ChangeDirectory,
    Echo,
    List,
    Clear,
    Concatenate,
    Statistics,
    CreateDirectory,
    SetEnvironmentVariable,
    RemoveEnvironmentVariable,
    RemoveFile,
    RemoveDirectory,
    WebRequest,
    DnsResolve,
    Ping,
    Ip,
    PrintWorkingDirectory,
    PrintEnvironmentVariable,
    Which,
    WordCount,
    Head,
    Tail,
    Touch,
}

pub fn resolve_user_command(name: &str) -> Option<UserCommandKind> {
    match name {
        "exit" => Some(UserCommandKind::Exit),
        "cd" => Some(UserCommandKind::ChangeDirectory),
        "echo" => Some(UserCommandKind::Echo),
        "ls" => Some(UserCommandKind::List),
        "clear" => Some(UserCommandKind::Clear),
        "cat" => Some(UserCommandKind::Concatenate),
        "stat" => Some(UserCommandKind::Statistics),
        "mkdir" => Some(UserCommandKind::CreateDirectory),
        "export" => Some(UserCommandKind::SetEnvironmentVariable),
        "unset" => Some(UserCommandKind::RemoveEnvironmentVariable),
        "rm" => Some(UserCommandKind::RemoveFile),
        "rmdir" => Some(UserCommandKind::RemoveDirectory),
        "web_request" => Some(UserCommandKind::WebRequest),
        "dns_resolve" => Some(UserCommandKind::DnsResolve),
        "ping" => Some(UserCommandKind::Ping),
        "ip" => Some(UserCommandKind::Ip),
        "pwd" => Some(UserCommandKind::PrintWorkingDirectory),
        "printenv" => Some(UserCommandKind::PrintEnvironmentVariable),
        "which" => Some(UserCommandKind::Which),
        "wc" => Some(UserCommandKind::WordCount),
        "head" => Some(UserCommandKind::Head),
        "tail" => Some(UserCommandKind::Tail),
        "touch" => Some(UserCommandKind::Touch),
        _ => None,
    }
}

pub async fn dispatch_user_command<'a, I, C>(
    kind: UserCommandKind,
    context: &mut C,
    options: &mut getargs::Options<&'a str, I>,
    paths: &[&Path],
) -> Result<()>
where
    I: Iterator<Item = &'a str>,
    C: CommandContext,
{
    match kind {
        UserCommandKind::Exit => ExitCommand.execute(context, options, paths).await,
        UserCommandKind::ChangeDirectory => {
            ChangeDirectoryCommand
                .execute(context, options, paths)
                .await
        }
        UserCommandKind::Echo => EchoCommand.execute(context, options, paths).await,
        UserCommandKind::List => ListCommand.execute(context, options, paths).await,
        UserCommandKind::Clear => ClearCommand.execute(context, options, paths).await,
        UserCommandKind::Concatenate => ConcatenateCommand.execute(context, options, paths).await,
        UserCommandKind::Statistics => StatisticsCommand.execute(context, options, paths).await,
        UserCommandKind::CreateDirectory => {
            CreateDirectoryCommand
                .execute(context, options, paths)
                .await
        }
        UserCommandKind::SetEnvironmentVariable => {
            SetEnvironmentVariableCommand
                .execute(context, options, paths)
                .await
        }
        UserCommandKind::RemoveEnvironmentVariable => {
            RemoveEnvironmentVariableCommand
                .execute(context, options, paths)
                .await
        }
        UserCommandKind::RemoveFile => RemoveFileCommand.execute(context, options, paths).await,
        UserCommandKind::RemoveDirectory => {
            RemoveDirectoryCommand
                .execute(context, options, paths)
                .await
        }
        UserCommandKind::WebRequest => WebRequestCommand.execute(context, options, paths).await,
        UserCommandKind::DnsResolve => DnsResolveCommand.execute(context, options, paths).await,
        UserCommandKind::Ping => PingCommand.execute(context, options, paths).await,
        UserCommandKind::Ip => IpCommand.execute(context, options, paths).await,
        UserCommandKind::PrintWorkingDirectory => {
            PrintWorkingDirectoryCommand
                .execute(context, options, paths)
                .await
        }
        UserCommandKind::PrintEnvironmentVariable => {
            PrintEnvironmentVariableCommand
                .execute(context, options, paths)
                .await
        }
        UserCommandKind::Which => WhichCommand.execute(context, options, paths).await,
        UserCommandKind::WordCount => WordCountCommand.execute(context, options, paths).await,
        UserCommandKind::Head => HeadCommand.execute(context, options, paths).await,
        UserCommandKind::Tail => TailCommand.execute(context, options, paths).await,
        UserCommandKind::Touch => TouchCommand.execute(context, options, paths).await,
    }
}

fn check_no_more_options<'a, I>(options: &mut getargs::Options<&'a str, I>) -> crate::Result<()>
where
    I: Iterator<Item = &'a str>,
{
    if Ok(None) != options.next_opt() {
        return Err(crate::Error::InvalidOption);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{UserCommandKind, resolve_user_command};

    #[test]
    fn resolves_all_supported_user_commands() {
        assert!(matches!(
            resolve_user_command("exit"),
            Some(UserCommandKind::Exit)
        ));
        assert!(matches!(
            resolve_user_command("cd"),
            Some(UserCommandKind::ChangeDirectory)
        ));
        assert!(matches!(
            resolve_user_command("echo"),
            Some(UserCommandKind::Echo)
        ));
        assert!(matches!(
            resolve_user_command("ls"),
            Some(UserCommandKind::List)
        ));
        assert!(matches!(
            resolve_user_command("clear"),
            Some(UserCommandKind::Clear)
        ));
        assert!(matches!(
            resolve_user_command("cat"),
            Some(UserCommandKind::Concatenate)
        ));
        assert!(matches!(
            resolve_user_command("stat"),
            Some(UserCommandKind::Statistics)
        ));
        assert!(matches!(
            resolve_user_command("mkdir"),
            Some(UserCommandKind::CreateDirectory)
        ));
        assert!(matches!(
            resolve_user_command("export"),
            Some(UserCommandKind::SetEnvironmentVariable)
        ));
        assert!(matches!(
            resolve_user_command("unset"),
            Some(UserCommandKind::RemoveEnvironmentVariable)
        ));
        assert!(matches!(
            resolve_user_command("rm"),
            Some(UserCommandKind::RemoveFile)
        ));
        assert!(matches!(
            resolve_user_command("rmdir"),
            Some(UserCommandKind::RemoveDirectory)
        ));
        assert!(matches!(
            resolve_user_command("web_request"),
            Some(UserCommandKind::WebRequest)
        ));
        assert!(matches!(
            resolve_user_command("dns_resolve"),
            Some(UserCommandKind::DnsResolve)
        ));
        assert!(matches!(
            resolve_user_command("ping"),
            Some(UserCommandKind::Ping)
        ));
        assert!(matches!(
            resolve_user_command("ip"),
            Some(UserCommandKind::Ip)
        ));
        assert!(matches!(
            resolve_user_command("pwd"),
            Some(UserCommandKind::PrintWorkingDirectory)
        ));
        assert!(matches!(
            resolve_user_command("printenv"),
            Some(UserCommandKind::PrintEnvironmentVariable)
        ));
        assert!(matches!(
            resolve_user_command("which"),
            Some(UserCommandKind::Which)
        ));
        assert!(matches!(
            resolve_user_command("wc"),
            Some(UserCommandKind::WordCount)
        ));
        assert!(matches!(
            resolve_user_command("head"),
            Some(UserCommandKind::Head)
        ));
        assert!(matches!(
            resolve_user_command("tail"),
            Some(UserCommandKind::Tail)
        ));
        assert!(matches!(
            resolve_user_command("touch"),
            Some(UserCommandKind::Touch)
        ));
        assert!(resolve_user_command("unknown").is_none());
    }
}
