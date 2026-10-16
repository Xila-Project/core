//! Borrowed shell tokens; preserve quotes for execution-time expansion.
use alloc::vec::Vec;
pub const MAX_INPUT_BYTES: usize = 4096;
pub const MAX_TOKENS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntaxError {
    pub position: usize,
    pub message: &'static str,
}
impl SyntaxError {
    pub const fn new(position: usize, message: &'static str) -> Self {
        Self { position, message }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind<'a> {
    Word(&'a str),
    Pipe,
    And,
    Or,
    Separator,
    Background,
    Input,
    Output,
    Append,
    ErrorOutput,
    ErrorAppend,
}
impl Kind<'_> {
    pub fn is_redirect(self) -> bool {
        matches!(
            self,
            Self::Input | Self::Output | Self::Append | Self::ErrorOutput | Self::ErrorAppend
        )
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token<'a> {
    pub kind: Kind<'a>,
    pub position: usize,
}

pub fn tokenize(input: &str) -> Result<Vec<Token<'_>>, SyntaxError> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(SyntaxError::new(
            MAX_INPUT_BYTES,
            "command line is too long",
        ));
    }
    let bytes = input.as_bytes();
    let mut tokens = Vec::new();
    let mut position = 0;
    while position < bytes.len() {
        if bytes[position].is_ascii_whitespace() {
            position += 1;
            continue;
        }
        if bytes[position] == b'#' {
            break;
        }
        let start = position;
        let (kind, length) = match &bytes[position..] {
            [b'&', b'&', ..] => (Kind::And, 2),
            [b'|', b'|', ..] => (Kind::Or, 2),
            [b'>', b'>', ..] => (Kind::Append, 2),
            [b'2', b'>', b'>', ..] => (Kind::ErrorAppend, 3),
            [b'2', b'>', ..] => (Kind::ErrorOutput, 2),
            [b'|', ..] => (Kind::Pipe, 1),
            [b'&', ..] => (Kind::Background, 1),
            [b';', ..] => (Kind::Separator, 1),
            [b'<', ..] => (Kind::Input, 1),
            [b'>', ..] => (Kind::Output, 1),
            _ => {
                let mut quote = None;
                while position < bytes.len() {
                    let byte = bytes[position];
                    if byte == 0 {
                        return Err(SyntaxError::new(position, "NUL is not allowed"));
                    }
                    if byte == b'\\' && quote != Some(b'\'') {
                        position += 1;
                        if position == bytes.len() {
                            return Err(SyntaxError::new(position - 1, "unfinished escape"));
                        }
                        if bytes[position] == 0 {
                            return Err(SyntaxError::new(position, "NUL is not allowed"));
                        }
                        position += 1;
                        continue;
                    }
                    if matches!(byte, b'\'' | b'"') {
                        if quote == Some(byte) {
                            quote = None;
                        } else if quote.is_none() {
                            quote = Some(byte);
                        }
                    } else if quote.is_none()
                        && (byte.is_ascii_whitespace() || b"|&;<>".contains(&byte))
                    {
                        break;
                    }
                    position += 1;
                }
                if quote.is_some() {
                    return Err(SyntaxError::new(start, "unterminated quote"));
                }
                (Kind::Word(&input[start..position]), 0)
            }
        };
        if tokens.len() == MAX_TOKENS {
            return Err(SyntaxError::new(start, "too many tokens"));
        }
        tokens.push(Token {
            kind,
            position: start,
        });
        position += length;
    }
    Ok(tokens)
}

#[derive(Debug, PartialEq, Eq)]
pub enum Segment<'a> {
    Literal(&'a str),
    Variable(&'a str),
    Status,
}
/// Iterate word parts without allocating or discarding the quote distinction.
pub struct Segments<'a> {
    source: &'a str,
    position: usize,
    quote: Option<u8>,
}
impl<'a> Segments<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            position: 0,
            quote: None,
        }
    }
}
fn name_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}
fn name_continue(byte: u8) -> bool {
    name_start(byte) || byte.is_ascii_digit()
}
impl<'a> Iterator for Segments<'a> {
    type Item = Result<Segment<'a>, SyntaxError>;
    fn next(&mut self) -> Option<Self::Item> {
        let bytes = self.source.as_bytes();
        while self.position < bytes.len() {
            let start = self.position;
            let byte = bytes[start];
            self.position += 1;
            if Some(byte) == self.quote {
                self.quote = None;
                continue;
            }
            if self.quote.is_none() && matches!(byte, b'\'' | b'"') {
                self.quote = Some(byte);
                continue;
            }
            if self.quote != Some(b'\'') {
                if byte == b'`' || (byte == b'$' && bytes.get(self.position) == Some(&b'(')) {
                    self.position = bytes.len();
                    return Some(Err(SyntaxError::new(
                        start,
                        "command substitution is not supported",
                    )));
                }
                if byte == b'\\' {
                    let Some(character) = self.source[self.position..].chars().next() else {
                        return Some(Err(SyntaxError::new(start, "unfinished escape")));
                    };
                    if self.quote == Some(b'"') && !matches!(character, '$' | '"' | '\\') {
                        return Some(Ok(Segment::Literal(&self.source[start..self.position])));
                    }
                    let escaped = self.position;
                    self.position += character.len_utf8();
                    return Some(Ok(Segment::Literal(&self.source[escaped..self.position])));
                }
                if byte == b'$' {
                    if bytes.get(self.position) == Some(&b'?') {
                        self.position += 1;
                        return Some(Ok(Segment::Status));
                    }
                    let braced = bytes.get(self.position) == Some(&b'{');
                    let name = self.position + usize::from(braced);
                    if braced || bytes.get(name).is_some_and(|byte| name_start(*byte)) {
                        let mut end = name;
                        while bytes.get(end).is_some_and(|byte| name_continue(*byte)) {
                            end += 1;
                        }
                        if !bytes.get(name).is_some_and(|byte| name_start(*byte))
                            || (braced && bytes.get(end) != Some(&b'}'))
                        {
                            self.position = bytes.len();
                            return Some(Err(SyntaxError::new(
                                start,
                                "invalid variable expansion",
                            )));
                        }
                        self.position = end + usize::from(braced);
                        return Some(Ok(Segment::Variable(&self.source[name..end])));
                    }
                    return Some(Ok(Segment::Literal(&self.source[start..self.position])));
                }
            }
            while self.position < bytes.len() {
                let next = bytes[self.position];
                if Some(next) == self.quote
                    || (self.quote.is_none() && matches!(next, b'\'' | b'"'))
                    || (self.quote != Some(b'\'') && matches!(next, b'$' | b'\\' | b'`'))
                {
                    break;
                }
                self.position += 1;
            }
            return Some(Ok(Segment::Literal(&self.source[start..self.position])));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{string::String, vec};
    #[test]
    fn quoted_spaces_and_adjacent_operators() {
        let tokens = tokenize("echo 'hello world'&&echo a\\|b>>out").unwrap();
        assert_eq!(
            tokens.iter().map(|token| token.kind).collect::<Vec<_>>(),
            vec![
                Kind::Word("echo"),
                Kind::Word("'hello world'"),
                Kind::And,
                Kind::Word("echo"),
                Kind::Word("a\\|b"),
                Kind::Append,
                Kind::Word("out")
            ]
        );
    }
    #[test]
    fn expansion_preserves_quotes_and_escaped_unicode() {
        let parts = Segments::new("pre\"$NAME\"'$OTHER'\\é${SUFFIX}$?")
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            parts,
            vec![
                Segment::Literal("pre"),
                Segment::Variable("NAME"),
                Segment::Literal("$OTHER"),
                Segment::Literal("é"),
                Segment::Variable("SUFFIX"),
                Segment::Status
            ]
        );
    }
    #[test]
    fn double_quotes_only_escape_shell_specials() {
        let parts = Segments::new(r#""\q\$x\"\\""#)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let value: String = parts
            .into_iter()
            .map(|part| match part {
                Segment::Literal(value) => value,
                _ => panic!(),
            })
            .collect();
        assert_eq!(value, "\\q$x\"\\");
    }
    #[test]
    fn comments_start_at_word_boundaries() {
        assert_eq!(tokenize("echo a#b '#' \\# # rest && bad").unwrap().len(), 4);
    }
    #[test]
    fn rejects_unfinished_quotes_escapes_and_oversized_input() {
        for input in [
            "echo 'bad",
            "echo \\",
            "echo \0",
            &"x".repeat(MAX_INPUT_BYTES + 1),
        ] {
            assert!(tokenize(input).is_err(), "{input:?}");
        }
    }
    #[test]
    fn unsupported_or_malformed_expansion_is_rejected() {
        for input in [
            "${}",
            "${1}",
            "${NAME",
            "${NAME:-fallback}",
            "$(echo)",
            "`echo`",
        ] {
            assert!(
                Segments::new(input).any(|segment| segment.is_err()),
                "{input:?}"
            );
        }
    }
}
