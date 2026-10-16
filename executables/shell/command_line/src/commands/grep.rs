use alloc::borrow::ToOwned;
use core::num::NonZeroUsize;
use getargs::Options;
use xila::{
    file_system::{AccessFlags, Path},
    virtual_file_system::File,
};

use crate::{Error, Result, commands::check_no_more_options};

use super::{CommandContext, UserCommand};

const LINE_BUFFER_SIZE: usize = 256;

pub struct GrepCommand;

impl UserCommand for GrepCommand {
    async fn execute<'a, I, C>(
        &self,
        context: &mut C,
        options: &mut Options<&'a str, I>,
        _paths: &[&Path],
    ) -> Result<(), Error<'a>>
    where
        I: Iterator<Item = &'a str>,
        C: CommandContext,
    {
        execute_grep(context, options).await
    }
}

fn resolve_path<C: CommandContext>(
    context: &C,
    argument: &str,
) -> Result<xila::file_system::PathOwned> {
    let path = Path::from_str(argument);

    if path.is_absolute() {
        Ok(path.to_owned())
    } else {
        context
            .current_directory_owned()
            .join(path)
            .ok_or(Error::FailedToJoinPath)
    }
}

fn contains(line: &[u8], pattern: &[u8]) -> bool {
    pattern.is_empty()
        || (pattern.len() <= line.len()
            && line.windows(pattern.len()).any(|window| window == pattern))
}

async fn search_reader<C: CommandContext>(
    context: &mut C,
    pattern: &[u8],
    input: &mut File<'_>,
    matched: &mut bool,
) -> Result<()> {
    let mut line = [0u8; LINE_BUFFER_SIZE];
    let mut line_length = 0;
    let mut chunk = [0u8; 256];

    loop {
        let bytes_read = input
            .read(&mut chunk)
            .await
            .map_err(Error::FailedToReadFile)?;
        if bytes_read == 0 {
            break;
        }

        for &byte in &chunk[..bytes_read] {
            if line_length == line.len() {
                return Err(Error::LineTooLong);
            }
            line[line_length] = byte;
            line_length += 1;

            if byte == b'\n' {
                let content_length = line_length - 1;
                if contains(&line[..content_length], pattern) {
                    context.write_out(&line[..line_length]).await?;
                    *matched = true;
                }
                line_length = 0;
            }
        }
    }

    if line_length != 0 && contains(&line[..line_length], pattern) {
        context.write_out(&line[..line_length]).await?;
        *matched = true;
    }

    Ok(())
}

async fn search_file<C: CommandContext>(
    context: &mut C,
    path: &Path,
    pattern: &[u8],
    matched: &mut bool,
) -> Result<()> {
    let mut file = File::open(
        context.executable_context().virtual_file_system,
        context.task_id(),
        path,
        AccessFlags::Read.into(),
    )
    .await
    .map_err(Error::FailedToOpenFile)?;

    search_reader(context, pattern, &mut file, matched).await
}

async fn execute_grep<'a, I, C>(
    context: &mut C,
    options: &mut Options<&'a str, I>,
) -> Result<(), Error<'a>>
where
    I: Iterator<Item = &'a str>,
    C: CommandContext,
{
    let pattern = options
        .next_positional()
        .ok_or(Error::MissingPositionalArgument("PATTERN"))?;
    check_no_more_options(options)?;

    let mut supplied_file = false;
    let mut matched = false;
    while let Some(argument) = options.next_positional() {
        supplied_file = true;
        if argument == "-" {
            let mut input = context
                .standard()
                .input()
                .duplicate()
                .await
                .map_err(Error::FailedToReadFile)?;
            search_reader(context, pattern.as_bytes(), &mut input, &mut matched).await?;
        } else {
            let path = resolve_path(context, argument)?;
            search_file(context, &path, pattern.as_bytes(), &mut matched).await?;
        }
    }
    if !supplied_file {
        let mut input = context
            .standard()
            .input()
            .duplicate()
            .await
            .map_err(Error::FailedToReadFile)?;
        search_reader(context, pattern.as_bytes(), &mut input, &mut matched).await?;
    }

    if matched {
        Ok(())
    } else {
        Err(Error::ExitStatus(NonZeroUsize::MIN))
    }
}

#[cfg(test)]
mod tests {
    use super::{LINE_BUFFER_SIZE, contains};

    #[test]
    fn matches_literal_substrings_and_empty_pattern() {
        assert!(contains(b"alpha beta", b"ha b"));
        assert!(!contains(b"alpha beta", b"ha.*beta"));
        assert!(contains(b"anything", b""));
    }

    #[test]
    fn line_buffer_has_a_fixed_capacity() {
        assert_eq!(LINE_BUFFER_SIZE, 256);
    }
}
