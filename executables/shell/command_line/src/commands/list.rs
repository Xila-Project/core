use crate::{Error, Result};
use alloc::borrow::ToOwned;
use alloc::string::ToString;
use getargs_derive::GetArgs;
use xila::{
    file_system::{Kind, Path},
    log, users,
    virtual_file_system::{self, Directory},
};

use super::{CommandContext, UserCommand};

pub struct ListCommand;

impl UserCommand for ListCommand {
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
        execute_list(context, options).await
    }
}

#[derive(GetArgs)]
struct ListArguments<'a> {
    #[arg(positional, default = "")]
    path: &'a str,
    #[arg(flag, short = 'l', long = "long")]
    long: bool,
}

fn resolve_list_path<C: CommandContext>(context: &C, path: &str) -> xila::file_system::PathOwned {
    if path.is_empty() {
        context.current_directory_owned()
    } else {
        Path::from_str(path).to_owned()
    }
}

fn kind_as_str(kind: Kind) -> &'static str {
    match kind {
        Kind::File => "f",
        Kind::Directory => "d",
        Kind::CharacterDevice => "c",
        Kind::BlockDevice => "b",
        Kind::Pipe => "p",
        Kind::Socket => "s",
        Kind::SymbolicLink => "l",
    }
}

async fn write_short_entry<C: CommandContext>(context: &mut C, entry_name: &str) -> Result<()> {
    context.write_out_fmt(format_args!("{}\n", entry_name))
}

async fn write_long_entry<C: CommandContext>(
    context: &mut C,
    path: &Path,
    entry: &impl AsRef<str>,
) -> Result<()> {
    let entry_path = path.append(entry.as_ref()).ok_or(Error::FailedToJoinPath)?;
    let statistics = context
        .executable_context()
        .virtual_file_system
        .get_statistics(&entry_path)
        .await
        .map_err(Error::FailedToGetMetadata)?;

    let users_manager = &context.executable_context().users_manager;
    let user = users_manager
        .get_user_name(statistics.user)
        .await
        .unwrap_or_else(|_| statistics.user.as_u16().to_string());
    let group = users_manager
        .get_group_name(statistics.group)
        .await
        .unwrap_or_else(|_| statistics.group.as_u16().to_string());

    context.write_out_fmt(format_args!(
        "{} {} {} {} {} {} {} {}\n",
        kind_as_str(statistics.kind),
        statistics.permissions,
        statistics.links,
        user,
        group,
        statistics.size,
        statistics.modification,
        entry.as_ref(),
    ))
}

async fn execute_list<'a, I, C>(
    context: &mut C,
    options: &mut getargs::Options<&'a str, I>,
) -> Result<()>
where
    I: Iterator<Item = &'a str>,
    C: CommandContext,
{
    let ListArguments { path, long } = ListArguments::parse(options)?;
    let path = resolve_list_path(context, path);

    let virtual_file_system = &context.executable_context().virtual_file_system;
    let mut directory = Directory::open(virtual_file_system, context.task_id(), &path)
        .await
        .map_err(Error::FailedToOpenDirectory)?;

    while let Some(entry) = directory
        .read()
        .await
        .map_err(Error::FailedToReadDirectoryEntry)?
    {
        if long {
            write_long_entry(context, &path, &entry.name).await?;
        } else {
            write_short_entry(context, &entry.name).await?;
        }
    }

    directory.close(virtual_file_system).await.map_err(|e| {
        log::error!("Failed to close directory {}", path.as_str());
        Error::FailedToOpenDirectory(e)
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::kind_as_str;
    use xila::file_system::Kind;

    #[test]
    fn kind_as_str_maps_file() {
        assert_eq!(kind_as_str(Kind::File), "f");
    }

    #[test]
    fn kind_as_str_maps_directory() {
        assert_eq!(kind_as_str(Kind::Directory), "d");
    }

    #[test]
    fn kind_as_str_maps_character_device() {
        assert_eq!(kind_as_str(Kind::CharacterDevice), "c");
    }

    #[test]
    fn kind_as_str_maps_block_device() {
        assert_eq!(kind_as_str(Kind::BlockDevice), "b");
    }

    #[test]
    fn kind_as_str_maps_pipe() {
        assert_eq!(kind_as_str(Kind::Pipe), "p");
    }

    #[test]
    fn kind_as_str_maps_socket() {
        assert_eq!(kind_as_str(Kind::Socket), "s");
    }

    #[test]
    fn kind_as_str_maps_symbolic_link() {
        assert_eq!(kind_as_str(Kind::SymbolicLink), "l");
    }
}
