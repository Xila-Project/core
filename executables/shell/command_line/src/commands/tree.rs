use crate::{Error, Result};
use alloc::{borrow::ToOwned, vec::Vec};
use getargs_derive::GetArgs;
use xila::{
    file_system::{Kind, Path, PathOwned},
    task::TaskIdentifier,
    virtual_file_system::{Directory, VirtualFileSystem},
};

use super::{CommandContext, UserCommand};

const MAX_DEPTH: usize = 32;
const MAX_PATH_BYTES: usize = 1024;

pub struct TreeCommand;

#[derive(GetArgs)]
#[command(
    name = "tree",
    about = "Print a directory tree, including hidden entries. Maximum depth is 32 (the default); -L 0 prints only the root. Paths are limited to 1024 bytes."
)]
pub(super) struct TreeArguments<'a> {
    #[arg(default_value = "")]
    path: &'a str,
    #[arg(short = 'L', long = "level", default_value_t = 32)]
    depth: usize,
}

impl UserCommand for TreeCommand {
    async fn execute<'a, I, C>(
        &self,
        context: &mut C,
        options: &mut getargs::Options<&'a str, I>,
        _paths: &[&Path],
    ) -> Result<(), crate::Error<'a>>
    where
        I: Iterator<Item = &'a str>,
        C: CommandContext,
    {
        let TreeArguments { path, depth } = TreeArguments::parse(options)?;
        if depth > MAX_DEPTH {
            return Err(Error::InvalidArgument);
        }
        let path = resolve_path(context.current_directory(), path)?;
        let vfs = context.executable_context().virtual_file_system;
        let task = context.task_id();
        // Open before printing so nonexistent or inaccessible roots fail without a false listing.
        walk(vfs, task, path, depth, |level, name| {
            if level == 0 {
                return context.write_out_fmt(format_args!("{name}\n"));
            }
            for _ in 1..level {
                context.write_out_fmt(format_args!("|   "))?;
            }
            context.write_out_fmt(format_args!("|-- {name}\n"))
        })
        .await
    }
}

fn resolve_path(base: &Path, path: &str) -> Result<PathOwned> {
    if !Path::from_str(path).is_valid() {
        return Err(Error::InvalidPath);
    }
    if base.as_str().len() + path.len() > MAX_PATH_BYTES && !Path::from_str(path).is_absolute() {
        return Err(Error::InvalidPath);
    }
    if path.len() > MAX_PATH_BYTES {
        return Err(Error::InvalidPath);
    }
    let path = Path::from_str(path);
    let path = if path.is_absolute() {
        path.to_owned()
    } else {
        base.to_owned().join(path).ok_or(Error::FailedToJoinPath)?
    }
    .canonicalize();
    if path.as_str().len() > MAX_PATH_BYTES {
        return Err(Error::InvalidPath);
    }
    Ok(path)
}

/// Stream entries in filesystem order. Each level retains one directory handle;
/// one shared path buffer replaces recursive futures and collections of siblings.
/// Directory's RAII drop closes all remaining handles on any early error.
async fn walk(
    vfs: &VirtualFileSystem,
    task: TaskIdentifier,
    mut path: PathOwned,
    max_depth: usize,
    mut write_entry: impl FnMut(usize, &str) -> Result<()>,
) -> Result<()> {
    if max_depth > MAX_DEPTH {
        return Err(Error::InvalidArgument);
    }
    if path.as_str().len() > MAX_PATH_BYTES {
        return Err(Error::InvalidPath);
    }
    let root = Directory::open(vfs, task, &path)
        .await
        .map_err(Error::FailedToOpenDirectory)?;
    write_entry(0, path.as_str())?;
    if max_depth == 0 {
        return root.close(vfs).await.map_err(Error::FailedToOpenDirectory);
    }
    let mut stack = Vec::with_capacity(max_depth);
    stack.push((root, path.as_str().len()));
    while let Some((directory, _)) = stack.last_mut() {
        let entry = directory
            .read()
            .await
            .map_err(Error::FailedToReadDirectoryEntry)?;
        let Some(entry) = entry else {
            let (directory, parent_length) = stack.pop().unwrap();
            directory
                .close(vfs)
                .await
                .map_err(Error::FailedToOpenDirectory)?;
            path.truncate(parent_length);
            continue;
        };
        if entry.name == "." || entry.name == ".." {
            continue;
        }
        let depth = stack.len();
        write_entry(depth, &entry.name)?;
        if entry.kind == Kind::Directory && depth < max_depth {
            let parent_length = path.as_str().len();
            if parent_length + entry.name.len() + 1 > MAX_PATH_BYTES {
                return Err(Error::InvalidPath);
            }
            path = path.append(&entry.name).ok_or(Error::FailedToJoinPath)?;
            let child = Directory::open(vfs, task, &path)
                .await
                .map_err(Error::FailedToOpenDirectory)?;
            stack.push((child, parent_length));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;
    use core::fmt::Write;

    #[test]
    fn parses_default_explicit_and_invalid_depth() {
        assert_eq!(TreeArguments::from_args([].into_iter()).unwrap().depth, 32);
        let args = TreeArguments::from_args(["-L", "2", "nested"].into_iter()).unwrap();
        assert_eq!(args.depth, 2);
        assert_eq!(args.path, "nested");
        assert!(TreeArguments::from_args(["-L", "invalid"].into_iter()).is_err());
    }

    #[test]
    fn resolves_relative_and_default_paths_against_current_directory() {
        let base = Path::from_str("/work");
        assert_eq!(resolve_path(base, "").unwrap().as_str(), "/work");
        assert_eq!(
            resolve_path(base, "nested").unwrap().as_str(),
            "/work/nested"
        );
        assert_eq!(resolve_path(base, "/other").unwrap().as_str(), "/other");
        assert_eq!(resolve_path(base, "../other").unwrap().as_str(), "/other");
        assert!(resolve_path(base, &"x".repeat(1025)).is_err());
        assert!(resolve_path(base, "bad?name").is_err());
    }

    #[xila::task::test(task_path = xila::task)]
    async fn walks_nested_directories_and_limits_depth() {
        let standard = testing::initialize(false, false).await;
        let vfs = standard.context.virtual_file_system;
        let task = standard
            .context
            .task_manager
            .get_current_task_identifier()
            .await;
        for path in [
            "/tree_test",
            "/tree_test/child",
            "/tree_test/child/grandchild",
        ] {
            Directory::create(vfs, task, Path::from_str(path))
                .await
                .unwrap();
        }
        for (depth, expected) in [(0, ""), (1, "1:child\n"), (2, "1:child\n2:grandchild\n")] {
            let mut output = String::new();
            walk(
                vfs,
                task,
                Path::from_str("/tree_test").to_owned(),
                depth,
                |depth, name| {
                    if depth > 0 {
                        writeln!(output, "{depth}:{name}").unwrap();
                    }
                    Ok(())
                },
            )
            .await
            .unwrap();
            assert_eq!(output, expected);
        }
        let mut hidden = alloc::vec::Vec::new();
        Directory::create(vfs, task, Path::from_str("/tree_test/.hidden"))
            .await
            .unwrap();
        walk(
            vfs,
            task,
            Path::from_str("/tree_test").to_owned(),
            1,
            |_, name| {
                hidden.push(alloc::string::String::from(name));
                Ok(())
            },
        )
        .await
        .unwrap();
        assert!(hidden.iter().any(|name| name == ".hidden"));
        assert!(!hidden.iter().any(|name| name == "." || name == ".."));
        let file = xila::virtual_file_system::File::open(
            vfs,
            task,
            Path::from_str("/tree_test/file"),
            xila::file_system::Flags::new(
                xila::file_system::AccessFlags::Write,
                Some(xila::file_system::CreateFlags::Create),
                None,
            ),
        )
        .await
        .unwrap();
        file.close(vfs).await.unwrap();
        assert!(matches!(
            walk(
                vfs,
                task,
                Path::from_str("/tree_test/file").to_owned(),
                1,
                |_, _| Ok(())
            )
            .await,
            Err(Error::FailedToOpenDirectory(_))
        ));
        assert!(matches!(
            walk(
                vfs,
                task,
                Path::from_str("/tree_test").to_owned(),
                33,
                |_, _| Ok(())
            )
            .await,
            Err(Error::InvalidArgument)
        ));
        assert!(matches!(
            walk(
                vfs,
                task,
                Path::from_str("/tree_missing").to_owned(),
                1,
                |_, _| Ok(())
            )
            .await,
            Err(Error::FailedToOpenDirectory(_))
        ));
        vfs.set_permissions(
            task,
            Path::from_str("/tree_test"),
            xila::file_system::Permissions::NONE,
        )
        .await
        .unwrap();
        standard
            .context
            .task_manager
            .set_user(task, xila::users::UserIdentifier::MINIMUM)
            .await
            .unwrap();
        let denied = walk(
            vfs,
            task,
            Path::from_str("/tree_test").to_owned(),
            1,
            |_, _| Ok(()),
        )
        .await;
        standard
            .context
            .task_manager
            .set_user(task, xila::users::UserIdentifier::ROOT)
            .await
            .unwrap();
        assert!(matches!(denied, Err(Error::FailedToOpenDirectory(_))));
    }
}
