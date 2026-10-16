use core::time::Duration;
use getargs_derive::GetArgs;
use xila::file_system::Path;

use super::{CommandContext, UserCommand};
use crate::{Error, Result};

pub struct SleepCommand;

#[derive(GetArgs)]
#[command(
    name = "sleep",
    about = "Pause for a nonnegative integer duration. Suffixes: ms, s (default), m, h. Fractions and multiple durations are not supported."
)]
pub(super) struct SleepArguments<'a> {
    duration: &'a str,
}

impl UserCommand for SleepCommand {
    async fn execute<'a, I, C>(
        &self,
        _context: &mut C,
        options: &mut getargs::Options<&'a str, I>,
        _paths: &[&Path],
    ) -> Result<(), Error<'a>>
    where
        I: Iterator<Item = &'a str>,
        C: CommandContext,
    {
        let arguments = SleepArguments::parse(options)?;
        let duration = parse_duration(arguments.duration).ok_or(Error::InvalidSleepDuration)?;
        xila::task::sleep(duration).await;
        Ok(())
    }
}

fn parse_duration(value: &str) -> Option<Duration> {
    let (number, nanos_per_unit) = if let Some(number) = value.strip_suffix("ms") {
        (number, 1_000_000u64)
    } else if let Some(number) = value.strip_suffix('s') {
        (number, 1_000_000_000)
    } else if let Some(number) = value.strip_suffix('m') {
        (number, 60_000_000_000)
    } else if let Some(number) = value.strip_suffix('h') {
        (number, 3_600_000_000_000)
    } else {
        (value, 1_000_000_000)
    };
    if number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    // task::sleep converts nanoseconds to u64; reject values that would wrap.
    let nanos = number.parse::<u64>().ok()?.checked_mul(nanos_per_unit)?;
    Some(Duration::from_nanos(nanos))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_documented_integer_units() {
        for (text, millis) in [
            ("0", 0),
            ("12", 12_000),
            ("2s", 2_000),
            ("3ms", 3),
            ("2m", 120_000),
            ("1h", 3_600_000),
        ] {
            assert_eq!(
                parse_duration(text),
                Some(Duration::from_millis(millis)),
                "{text}"
            );
        }
    }

    #[test]
    fn rejects_invalid_and_overflowing_durations() {
        for text in [
            "",
            "s",
            "-1",
            "+1",
            "1.5",
            "NaN",
            "inf",
            "1d",
            "1MS",
            " 1",
            "1 ",
            "1s2",
            "18446744073709551616ms",
            "18446744074",
            "5124096h",
        ] {
            assert_eq!(parse_duration(text), None, "{text}");
        }
        assert!(parse_duration("18446744073s").is_some());
        assert!(parse_duration("18446744073709ms").is_some());
        assert!(parse_duration("18446744073710ms").is_none());
    }

    #[test]
    fn requires_exactly_one_duration_and_rejects_unknown_options() {
        for arguments in [
            alloc::vec![],
            alloc::vec!["1", "2"],
            alloc::vec!["--unknown"],
            alloc::vec!["-1"],
        ] {
            assert!(
                SleepArguments::parse(&mut getargs::Options::new(arguments.into_iter())).is_err()
            );
        }
    }
}
