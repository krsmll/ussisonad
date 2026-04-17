mod token;

pub use self::token::Token;

use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq)]
pub enum Loc {
    Point(usize),
    Slice(usize, usize),
}

impl std::fmt::Display for Loc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Loc::Point(p) => write!(f, "{p}"),
            Loc::Slice(s, p) => write!(f, "{s}:{p}"),
        }
    }
}

pub type Spanned = (Token, usize, usize);
pub type LexResult = Result<Spanned, LexError>;
#[derive(Debug, Clone, PartialEq)]
pub enum LexError {
    UnrecognizedToken(Loc, char),
    UnexpectedEof(Loc),
    UnfinishedDotAccess(Loc),
    BadStringEscape(Loc),
    UnexpectedStringEnd(Loc),
    MalformedNumber(Loc, String),
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LexError::UnrecognizedToken(span, c) => {
                write!(f, "unrecognized token '{c}' at {span}")
            }
            LexError::UnexpectedEof(span) => {
                write!(f, "unexpected end of input at {span}")
            }
            LexError::UnfinishedDotAccess(span) => {
                write!(f, "expected field name after '.' at {span}")
            }
            LexError::BadStringEscape(span) => {
                write!(f, "invalid escape sequence at {span}")
            }
            LexError::UnexpectedStringEnd(span) => {
                write!(f, "unterminated string literal at {span}")
            }
            LexError::MalformedNumber(span, s) => {
                write!(f, "malformed number '{s}' at {span}")
            }
        }
    }
}

impl std::error::Error for LexError {}

/// Lexer is generic over any iterator of (usize, char) pairs.
#[derive(Debug)]
pub struct Lexer<T: Iterator<Item = (usize, char)>> {
    chars: T,
    pending: VecDeque<Spanned>,
    current: Option<char>,
    peek: Option<char>,
    current_pos: usize,
    peek_pos: usize,
}

impl<T: Iterator<Item = (usize, char)>> Lexer<T> {
    /// Primary constructor. Accepts any `(usize, char)` iterator.
    pub fn new(source: T) -> Self {
        let mut lexer = Self {
            chars: source,
            pending: VecDeque::new(),
            current: None,
            peek: None,
            current_pos: 0,
            peek_pos: 0,
        };

        // Populate current and peek from the source iterator.
        lexer.advance();
        lexer.advance();
        lexer
    }
}

impl<'a> Lexer<std::str::CharIndices<'a>> {
    /// Convenience constructor for &str input.
    #[must_use]
    pub fn new_from_str(input: &'a str) -> Self {
        Self::new(input.char_indices())
    }
}

impl<T: Iterator<Item = (usize, char)>> Iterator for Lexer<T> {
    type Item = LexResult;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner_next() {
            Ok((Token::Eof, _, _)) => None,
            other => Some(other),
        }
    }
}

impl<T: Iterator<Item = (usize, char)>> Lexer<T> {
    fn inner_next(&mut self) -> LexResult {
        while self.pending.is_empty() {
            self.consume()?;
        }
        Ok(self.pending.pop_front().unwrap())
    }

    fn consume(&mut self) -> Result<(), LexError> {
        match self.current {
            Some(c) if c.is_alphabetic() || c == '_' => {
                let spanned = self.lex_word();
                self.emit(spanned);
            }
            Some(c) if Self::is_number_start(c, self.peek) => {
                let spanned = self.lex_number()?;
                self.emit(spanned);
            }
            Some(c) => self.consume_char(c)?,
            None => {
                let pos = self.pos();
                self.emit((Token::Eof, pos, pos));
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn consume_char(&mut self, c: char) -> Result<(), LexError> {
        match c {
            ' ' | '\t' | '\r' | '\n' => {
                self.advance();
            }
            '=' => self.emit_single_char(Token::Eq),
            '*' => self.emit_single_char(Token::Mul),
            '%' => self.emit_single_char(Token::Mod),
            '(' => self.emit_single_char(Token::LeftParen),
            ')' => self.emit_single_char(Token::RightParen),
            ',' => self.emit_single_char(Token::Comma),
            ';' => self.emit_single_char(Token::Semicolon),
            '/' => {
                let start = self.current_pos;
                self.advance();
                if self.current == Some('/') {
                    self.advance();
                    self.emit((Token::DivDiv, start, self.pos()));
                } else {
                    self.emit((Token::Div, start, self.pos()));
                }
            }
            '.' => {
                let pos = self.pos();
                match self.peek {
                    Some(next) if !next.is_whitespace() => {
                        self.emit_single_char(Token::Dot);
                    }
                    _ => {
                        self.advance();
                        return Err(LexError::UnfinishedDotAccess(Loc::Point(pos)));
                    }
                }
            }
            '"' => {
                // Consume the opening quote before entering lex_string.
                self.advance();
                let spanned = self.lex_string()?;
                self.emit(spanned);
            }
            '+' => {
                let start = self.pos();
                self.advance();
                if self.current == Some('+') {
                    self.advance();
                    self.emit((Token::AddAdd, start, self.pos()));
                } else {
                    self.emit((Token::Add, start, self.pos()));
                }
            }
            '-' => {
                let start = self.pos();
                self.advance();
                if self.current == Some('-') {
                    // '--' is the flag/option prefix — emit as two Sub tokens
                    // so the parser can recognise the '--ident' pattern.
                    self.advance();
                    self.emit((Token::SubSub, start, self.pos()));
                } else {
                    self.emit((Token::Sub, start, self.pos()));
                }
            }
            '!' => {
                let start = self.pos();
                self.advance();
                if self.current == Some('=') {
                    self.advance();
                    self.emit((Token::Ne, start, self.pos()));
                } else {
                    self.emit((Token::Not, start, start));
                }
            }
            '<' => {
                let start = self.pos();
                self.advance();
                if self.current == Some('=') {
                    self.advance();
                    self.emit((Token::Le, start, self.pos()));
                } else {
                    self.emit((Token::Lt, start, self.pos()));
                }
            }
            '>' => {
                let start = self.pos();
                self.advance();
                match self.current {
                    Some('>') => {
                        self.advance();
                        self.emit((Token::GtGt, start, self.pos()));
                    }
                    Some('=') => {
                        self.advance();
                        self.emit((Token::Ge, start, self.pos()));
                    }
                    _ => {
                        self.emit((Token::Gt, start, self.pos()));
                    }
                }
            }
            c => {
                let pos = self.pos();
                self.advance();
                return Err(LexError::UnrecognizedToken(Loc::Point(pos), c));
            }
        }
        Ok(())
    }

    fn emit_single_char(&mut self, token: Token) {
        let start = self.pos();
        self.advance();
        self.emit((token, start, self.pos()));
    }

    fn lex_word(&mut self) -> Spanned {
        let start = self.pos();
        let mut word = String::new();

        while let Some(c) = self.current {
            if Self::is_word_char(c) {
                word.push(c);
                self.advance();
            } else {
                break;
            }
        }

        let token = Token::from_word(&word.to_lowercase()).unwrap_or(Token::Ident(word));

        (token, start, self.pos())
    }

    /// A character is part of a word if it is alphanumeric or an underscore.
    fn is_word_char(c: char) -> bool {
        c.is_alphanumeric() || c == '_'
    }

    /// Lexes a quoted string. Assumes that the opening quote is already consumed.
    fn lex_string(&mut self) -> LexResult {
        let start = self.pos();
        let mut content = String::new();

        loop {
            match self.current {
                Some('"') => {
                    self.advance(); // consume closing quote
                    break;
                }
                Some('\\') => {
                    let slash_pos = self.pos();
                    self.advance(); // consume backslash
                    match self.current {
                        Some(c) if matches!(c, 'f' | 'n' | 't' | 'r' | '"' | '\\') => {
                            content.push('\\');
                            content.push(c);
                            self.advance();
                        }
                        _ => {
                            let err = LexError::BadStringEscape(Loc::Slice(slash_pos, self.pos()));

                            // Skip to the closing quote or EOF before returning
                            // the error so the lexer can continue afterwards.
                            loop {
                                match self.current {
                                    None => break,
                                    Some('"') => {
                                        self.advance();
                                        break;
                                    }
                                    _ => {
                                        self.advance();
                                    }
                                }
                            }

                            return Err(err);
                        }
                    }
                }
                Some(c) => {
                    content.push(c);
                    self.advance();
                }
                None => {
                    return Err(LexError::UnexpectedStringEnd(Loc::Slice(start, self.pos())));
                }
            }
        }

        Ok((Token::Str(content), start, self.pos()))
    }

    fn is_number_start(c: char, peek: Option<char>) -> bool {
        c.is_ascii_digit() || (c == '-' && matches!(peek, Some('0'..='9')))
    }

    fn lex_number(&mut self) -> LexResult {
        let start = self.pos();
        let mut content = String::new();
        let mut is_float = false;

        if self.current == Some('-') {
            content.push('-');
            self.advance();
        }

        loop {
            match self.current {
                Some('_') => {
                    self.advance();
                }
                Some('.') => {
                    is_float = true;
                    content.push('.');
                    self.advance();
                }
                Some(c) if c.is_ascii_digit() => {
                    content.push(c);
                    self.advance();
                }
                Some(c) if c.is_alphabetic() => {
                    // Consume the rest of the malformed token before erroring
                    // so the lexer can recover and continue.
                    while let Some(c) = self.current {
                        if c.is_alphanumeric() || c == '_' {
                            content.push(c);
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    return Err(LexError::MalformedNumber(
                        Loc::Slice(start, self.pos()),
                        content,
                    ));
                }
                _ => break,
            }
        }

        let token = if is_float {
            Token::Float(content)
        } else {
            Token::Int(content)
        };

        Ok((token, start, self.pos()))
    }

    fn advance(&mut self) -> Option<char> {
        let previous = self.current;
        let next = if let Some((byte_offset, ch)) = self.chars.next() {
            self.current_pos = self.peek_pos;
            self.peek_pos = byte_offset;
            Some(ch)
        } else {
            self.current_pos = self.peek_pos;
            self.peek_pos += 1;
            None
        };
        self.current = self.peek;
        self.peek = next;
        previous
    }

    fn pos(&self) -> usize {
        self.current_pos
    }

    fn emit(&mut self, spanned: Spanned) {
        self.pending.push_back(spanned);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex(input: &str) -> Vec<Token> {
        Lexer::new_from_str(input)
            .collect::<Result<Vec<_>, _>>()
            .map(|v| v.into_iter().map(|(t, _, _)| t).collect())
            .expect("lexer should not fail")
    }

    fn lex_err(input: &str) -> LexError {
        Lexer::new_from_str(input)
            .collect::<Result<Vec<_>, _>>()
            .expect_err("lexer should fail")
    }

    #[test]
    fn keywords_are_distinct() {
        assert_eq!(lex("filter"), vec![Token::Filter]);
        assert_eq!(lex("Slay"), vec![Token::Ident("Slay".to_string())]);
    }

    #[test]
    fn keywords_are_case_insensitive() {
        assert_eq!(lex("filter"), lex("FILTER"));
        assert_eq!(lex("filter"), lex("Filter"));
        assert_eq!(lex("sort"), lex("SORT"));
    }

    #[test]
    fn strings_and_idents_preserve_casing() {
        assert_eq!(
            lex(r#"Slay "sLAY""#),
            vec![
                Token::Ident("Slay".to_string()),
                Token::Str("sLAY".to_string()),
            ]
        );
    }

    #[test]
    fn alias_where_resolves_to_filter() {
        assert_eq!(lex("where"), vec![Token::Filter]);
    }

    #[test]
    fn pipe_and_concat_operators() {
        assert_eq!(lex(">>"), vec![Token::GtGt]);
        assert_eq!(lex("++"), vec![Token::AddAdd]);
    }

    #[test]
    fn comparison_operators() {
        assert_eq!(lex(">="), vec![Token::Ge]);
        assert_eq!(lex("<="), vec![Token::Le]);
        assert_eq!(lex("!="), vec![Token::Ne]);
        assert_eq!(lex(">"), vec![Token::Gt]);
        assert_eq!(lex("<"), vec![Token::Lt]);
        assert_eq!(lex("="), vec![Token::Eq]);
    }

    #[test]
    fn integer_and_float() {
        assert_eq!(lex("1000"), vec![Token::Int("1000".to_string())]);
        assert_eq!(lex("9.95"), vec![Token::Float("9.95".to_string())]);
        assert_eq!(lex("-42"), vec![Token::Int("-42".to_string())]);
        assert_eq!(lex("1_000"), vec![Token::Int("1000".to_string())]);
    }

    #[test]
    fn malformed_number_errors() {
        assert!(matches!(lex_err("123abc"), LexError::MalformedNumber(..)));
    }

    #[test]
    fn unterminated_string_errors() {
        assert!(matches!(
            lex_err(r#""unterminated"#),
            LexError::UnexpectedStringEnd(Loc::Slice(1, 13)),
        ));
    }

    #[test]
    fn bad_string_escape_errors() {
        assert!(matches!(
            lex_err(r#""\q""#),
            LexError::BadStringEscape(Loc::Slice(1, 2))
        ));
    }

    #[test]
    fn unfinished_dot_access_errors() {
        assert!(matches!(
            lex_err(". something"),
            LexError::UnfinishedDotAccess(Loc::Point(0))
        ));
    }

    #[test]
    fn unrecognized_token_errors() {
        assert!(matches!(
            lex_err("@bad"),
            LexError::UnrecognizedToken(Loc::Point(0), '@')
        ));
    }
}
