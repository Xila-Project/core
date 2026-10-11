use crate::{Error, Result};
use alloc::{borrow::ToOwned, vec::Vec};
use getargs::{Arg, Options};
use xila::{
    file_system::{AccessFlags, CreateFlags, Error as FileSystemError, Flags, Path},
    virtual_file_system::{self, File},
};

use super::{CommandContext, UserCommand};

pub struct TouchCommand;

impl UserCommand for TouchCommand {
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
        execute_touch(context, options).await
    }
}

struct TouchParameters<'a> {
    no_create: bool,
    access: bool,
    modification: bool,
    paths: Vec<&'a str>,
}

fn resolve_path<C: CommandContext>(
    context: &C,
    path: &str,
) -> Result<xila::file_system::PathOwned> {
    let path = Path::from_str(path);

    if path.is_absolute() {
        Ok(path.to_owned())
    } else {
        context
            .current_directory_owned()
            .join(path)
            .ok_or(Error::FailedToJoinPath)
    }
}

fn parse_touch_parameters<'a, I>(options: &mut Options<&'a str, I>) -> Result<TouchParameters<'a>>
where
    I: Iterator<Item = &'a str>,
{
    let mut no_create = false;
    let mut access = false;
    let mut modification = false;
    let mut paths = Vec::new();

    while let Some(argument) = options.next_arg()? {
        match argument {
            Arg::Long("no-create") | Arg::Short('c') => {
                no_create = true;
            }
            Arg::Long("access") | Arg::Short('a') => {
                access = true;
            }
            Arg::Long("modification") | Arg::Short('m') => {
                modification = true;
            }
            Arg::Positional(path) => {
                paths.push(path);
            }
            _ => {
                return Err(Error::InvalidOption);
            }
        }
    }

    if paths.is_empty() {
        return Err(Error::MissingPositionalArgument("path"));
    }

    Ok(TouchParameters {
        no_create,
        access,
        modification,
        paths,
    })
}

fn resolve_update_mask(access: bool, modification: bool) -> (bool, bool) {
    if !access && !modification {
        (true, true)
    } else {
        (access, modification)
    }
}

async fn touch_one_path<C: CommandContext>(
    context: &C,
    path: &Path,
    no_create: bool,
    update_access: bool,
    update_modification: bool,
) -> Result<()> {
    let virtual_file_system = virtual_file_system::get_instance();
    let exists = match virtual_file_system.get_statistics(&path).await {
        Ok(_) => true,
        Err(xila::virtual_file_system::Error::FileSystem(FileSystemError::NotFound)) => false,
        Err(error) => return Err(Error::FailedToGetMetadata(error)),
    };

    if !exists {
        if no_create {
            return Ok(());
        }

        let _file = File::open(
            virtual_file_system,
            context.task_id(),
            path,
            Flags::new(AccessFlags::Write, Some(CreateFlags::Create), None),
        )
        .await
        .map_err(Error::FailedToOpenFile)?;
    }

    virtual_file_system
        .set_times(context.task_id(), path, update_access, update_modification)
        .await
        .map_err(Error::FailedToSetMetadata)
}

async fn execute_touch<'a, I, C>(context: &mut C, options: &mut Options<&'a str, I>) -> Result<()>
where
    I: Iterator<Item = &'a str>,
    C: CommandContext,
{
    let parameters = parse_touch_parameters(options)?;
    let (update_access, update_modification) =
        resolve_update_mask(parameters.access, parameters.modification);

    let mut first_error: Option<Error> = None;

    for path in parameters.paths {
        let path = resolve_path(context, path)?;
        let result = touch_one_path(
            context,
            &path,
            parameters.no_create,
            update_access,
            update_modification,
        )
        .await;

        if let Err(error) = result {
            if first_error.is_none() {
                first_error = Some(error);
            }
        }
    }

    if let Some(error) = first_error {
        return Err(error);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use getargs::Options;

    use super::{parse_touch_parameters, resolve_update_mask};

    #[test]
    fn resolves_default_mask_to_access_and_modification() {
        assert_eq!(resolve_update_mask(false, false), (true, true));
    }

    #[test]
    fn resolves_access_only_mask() {
        assert_eq!(resolve_update_mask(true, false), (true, false));
    }

    #[test]
    fn resolves_modification_only_mask() {
        assert_eq!(resolve_update_mask(false, true), (false, true));
    }

    #[test]
    fn parses_flags_and_multiple_paths() {
        let input = ["-c", "-a", "first.txt", "second.txt"];
        let mut options = Options::new(input.into_iter());

        let parsed = parse_touch_parameters(&mut options).unwrap();

        assert!(parsed.no_create);
        assert!(parsed.access);
        assert!(!parsed.modification);
        assert_eq!(parsed.paths, ["first.txt", "second.txt"]);
    }

    #[test]
    fn fails_when_no_path_is_provided() {
        let input = ["-m"];
        let mut options = Options::new(input.into_iter());

        let parsed = parse_touch_parameters(&mut options);

        assert!(parsed.is_err());
    }
}
