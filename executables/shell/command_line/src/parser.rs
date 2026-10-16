//! Validate the whole line before execution. One borrowed token buffer, no tree.
use crate::tokenizer::{Kind, Segments, SyntaxError, Token, tokenize};
use alloc::vec::Vec;
pub const MAX_COMMANDS: usize = 32;
pub const MAX_PIPELINE_STAGES: usize = 8;

pub fn parse(source: &str) -> Result<Vec<Token<'_>>, SyntaxError> {
    let tokens = tokenize(source)?;
    let mut words = 0;
    let mut commands = 0;
    let mut stages = 1;
    let mut target = false;
    let mut required = false;
    for token in &tokens {
        if let Kind::Word(raw) = token.kind {
            for part in Segments::new(raw) {
                part.map_err(|error| {
                    SyntaxError::new(token.position + error.position, error.message)
                })?;
            }
            if target {
                target = false;
            } else {
                words += 1;
                required = false;
            }
        } else {
            if target {
                return Err(SyntaxError::new(
                    token.position,
                    "redirection requires a path",
                ));
            }
            if token.kind.is_redirect() {
                target = true;
                continue;
            }
            if words == 0 {
                return Err(SyntaxError::new(token.position, "expected a command"));
            }
            commands += 1;
            if commands > MAX_COMMANDS {
                return Err(SyntaxError::new(token.position, "too many commands"));
            }
            words = 0;
            required = matches!(token.kind, Kind::Pipe | Kind::And | Kind::Or);
            if token.kind == Kind::Pipe {
                stages += 1;
                if stages > MAX_PIPELINE_STAGES {
                    return Err(SyntaxError::new(token.position, "too many pipeline stages"));
                }
            } else {
                stages = 1;
            }
        }
    }
    if target || required {
        return Err(SyntaxError::new(
            source.len(),
            "unfinished command or redirection",
        ));
    }
    if words == 0
        && !tokens.is_empty()
        && !matches!(
            tokens.last().unwrap().kind,
            Kind::Separator | Kind::Background
        )
    {
        return Err(SyntaxError::new(source.len(), "expected a command"));
    }
    if commands + usize::from(words > 0) > MAX_COMMANDS {
        return Err(SyntaxError::new(source.len(), "too many commands"));
    }
    Ok(tokens)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_composition_and_quotes() {
        for source in [
            "",
            " #comment",
            "echo ''",
            "echo a|cat&&pwd||echo failed;pwd&",
            "cat<in>>out 2>errors",
            "echo 'a|b' \"$VALUE\"",
            ">out echo a",
        ] {
            assert!(parse(source).is_ok(), "{source:?}");
        }
    }
    #[test]
    fn rejects_entire_line_before_any_execution() {
        for source in [
            ";echo",
            "echo a||",
            "echo >",
            "echo ><in",
            "echo a||;pwd",
            "echo a| |pwd",
            "echo a;;pwd",
            "<in",
            "echo 2>&1",
            "echo <<EOF",
            "echo x; echo ${BAD",
            "echo x; echo $(pwd)",
        ] {
            assert!(parse(source).is_err(), "{source:?}");
        }
    }
    #[test]
    fn pipeline_and_command_counts_are_bounded() {
        assert!(parse(&alloc::vec!["echo"; MAX_PIPELINE_STAGES].join("|")).is_ok());
        assert!(parse(&alloc::vec!["echo"; MAX_PIPELINE_STAGES + 1].join("|")).is_err());
        assert!(parse(&alloc::vec!["echo"; MAX_COMMANDS + 1].join(";")).is_err());
    }
}
