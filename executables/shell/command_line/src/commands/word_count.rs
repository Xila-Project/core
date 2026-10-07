use crate::{Error, Result};
use alloc::borrow::ToOwned;
use getargs_derive::GetArgs;
use xila::{
    file_system::{AccessFlags, Path},
    virtual_file_system::{self, File},
};

use super::{CommandContext, UserCommand};

pub struct WordCountCommand;

impl UserCommand for WordCountCommand {
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
        execute_word_count(context, options).await
    }
}

#[derive(GetArgs)]
struct WordCountArguments<'a> {
    #[arg(positional, default = "")]
    path: &'a str,
    #[arg(flag, short = 'c', long = "characters", default = false)]
    characters: bool,
    #[arg(flag, short = 'w', long = "words", default = false)]
    words: bool,
    #[arg(flag, short = 'l', long = "lines", default = false)]
    lines: bool,
    #[arg(flag, short = 'L', long = "longest-line", default = false)]
    longest_line: bool,
}

struct Counts {
    characters: usize,
    words: usize,
    lines: usize,
    longest_line: usize,
}

struct CountState {
    current_line_length: usize,
    in_word: bool,
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

fn update_counts(counts: &mut Counts, state: &mut CountState, byte: u8) {
    counts.characters += 1;

    if byte == b'\n' {
        counts.lines += 1;
        counts.longest_line = counts.longest_line.max(state.current_line_length);
        state.current_line_length = 0;
    } else {
        state.current_line_length += 1;
    }

    if byte.is_ascii_whitespace() {
        if state.in_word {
            counts.words += 1;
            state.in_word = false;
        }
    } else {
        state.in_word = true;
    }
}

fn finalize_counts(counts: &mut Counts, state: &CountState) {
    if state.in_word {
        counts.words += 1;
    }

    counts.longest_line = counts.longest_line.max(state.current_line_length);
}

async fn count_file(mut file: File<'_>) -> Result<Counts> {
    let mut counts = Counts {
        characters: 0,
        words: 0,
        lines: 0,
        longest_line: 0,
    };
    let mut state = CountState {
        current_line_length: 0,
        in_word: false,
    };

    let mut buffer = [0u8; 256];
    loop {
        let bytes_read = file
            .read(&mut buffer)
            .await
            .map_err(Error::FailedToReadFile)?;

        if bytes_read == 0 {
            break;
        }

        for &byte in &buffer[..bytes_read] {
            update_counts(&mut counts, &mut state, byte);
        }
    }

    finalize_counts(&mut counts, &state);

    Ok(counts)
}

fn write_counts<C: CommandContext>(
    context: &mut C,
    counts: &Counts,
    arguments: &WordCountArguments<'_>,
) -> Result<()> {
    if arguments.characters {
        context.write_out_fmt(format_args!("{} ", counts.characters))?;
    }
    if arguments.words {
        context.write_out_fmt(format_args!("{} ", counts.words))?;
    }
    if arguments.lines {
        context.write_out_fmt(format_args!("{} ", counts.lines))?;
    }
    if arguments.longest_line {
        context.write_out_fmt(format_args!("{}", counts.longest_line))?;
    }
    context.write_out_fmt(format_args!("\n"))?;

    Ok(())
}

async fn execute_word_count<'a, I, C>(
    context: &mut C,
    options: &mut getargs::Options<&'a str, I>,
) -> Result<()>
where
    I: Iterator<Item = &'a str>,
    C: CommandContext,
{
    let mut arguments = WordCountArguments::parse(options)?;
    let path = resolve_path(context, arguments.path)?;

    if arguments.characters || arguments.words || arguments.lines || arguments.longest_line {
        // At least one specific count requested, do nothing
    } else {
        // No specific count requested, default to all counts
        arguments.characters = true;
        arguments.words = true;
        arguments.lines = true;
        arguments.longest_line = true;
    }

    let file = File::open(
        &context.executable_context().virtual_file_system,
        context.task_id(),
        &path,
        AccessFlags::Read.into(),
    )
    .await
    .map_err(Error::FailedToOpenFile)?;

    let counts = count_file(file).await?;
    write_counts(context, &counts, &arguments)
}

#[cfg(test)]
mod tests {
    use super::{CountState, Counts, finalize_counts, resolve_path, update_counts};
    use crate::{Result, commands::CommandContext};
    use alloc::borrow::ToOwned;
    use core::fmt;
    use xila::{
        executable::Standard,
        file_system::{Path, PathOwned},
        task::TaskIdentifier,
    };

    struct FakeContext {
        current_directory: PathOwned,
    }

    impl CommandContext for FakeContext {
        fn task_id(&self) -> TaskIdentifier {
            TaskIdentifier::new(1)
        }

        fn current_directory(&self) -> &Path {
            &self.current_directory
        }

        fn set_current_directory(&mut self, directory: PathOwned) {
            self.current_directory = directory;
        }

        fn stop(&mut self) {}

        fn write_out_fmt(&mut self, _arguments: fmt::Arguments<'_>) -> Result<()> {
            Ok(())
        }

        async fn write_out(&mut self, _buffer: &[u8]) {}

        async fn write_out_line(&mut self, _buffer: &[u8]) {}

        fn executable_context(&self) -> &'static xila::executable::ExecutableContext {
            panic!("executable context not needed in this test")
        }

        fn standard(&mut self) -> &mut Standard<'static> {
            panic!("standard not needed in this test")
        }
    }

    #[test]
    fn resolve_path_keeps_absolute_path() {
        let context = FakeContext {
            current_directory: Path::from_str("/base").to_owned(),
        };

        let path = resolve_path(&context, "/tmp/file.txt").unwrap();

        assert_eq!(path.as_str(), "/tmp/file.txt");
    }

    #[test]
    fn resolve_path_expands_relative_path_from_context() {
        let context = FakeContext {
            current_directory: Path::from_str("/base").to_owned(),
        };

        let path = resolve_path(&context, "file.txt").unwrap();

        assert_eq!(path.as_str(), "/base/file.txt");
    }

    #[test]
    fn update_counts_tracks_words_lines_and_characters() {
        let mut counts = Counts {
            characters: 0,
            words: 0,
            lines: 0,
            longest_line: 0,
        };
        let mut state = CountState {
            current_line_length: 0,
            in_word: false,
        };

        for byte in b"hello world\n" {
            update_counts(&mut counts, &mut state, *byte);
        }

        assert_eq!(counts.characters, 12);
        assert_eq!(counts.words, 2);
        assert_eq!(counts.lines, 1);
        assert_eq!(counts.longest_line, 11);
        assert_eq!(state.current_line_length, 0);
    }

    #[test]
    fn finalize_counts_counts_trailing_word_without_whitespace() {
        let mut counts = Counts {
            characters: 5,
            words: 0,
            lines: 0,
            longest_line: 0,
        };
        let state = CountState {
            current_line_length: 5,
            in_word: true,
        };

        finalize_counts(&mut counts, &state);

        assert_eq!(counts.words, 1);
        assert_eq!(counts.longest_line, 5);
    }

    #[test]
    fn finalize_counts_preserves_word_count_when_not_in_word() {
        let mut counts = Counts {
            characters: 6,
            words: 2,
            lines: 1,
            longest_line: 3,
        };
        let state = CountState {
            current_line_length: 1,
            in_word: false,
        };

        finalize_counts(&mut counts, &state);

        assert_eq!(counts.words, 2);
        assert_eq!(counts.longest_line, 3);
    }
}
