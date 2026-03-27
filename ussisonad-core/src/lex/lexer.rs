use crate::lex::error::{LexError, LexErrorType};
use crate::lex::token::Token;
use std::collections::VecDeque;

pub type Spanned = (Token, usize, usize);
pub type LexResult = Result<Spanned, LexError>;

pub fn make_tokenizer(source: &str) -> impl Iterator<Item = LexResult> + '_ {
    let chars = source.char_indices().map(|(i, c)| (c, i));
    Lexer::new(chars)
}

#[derive(Debug)]
pub struct Lexer<T: Iterator<Item = (char, usize)>> {
    chars: T,
    pending: VecDeque<Spanned>,
    current: Option<char>,
    peek: Option<char>,
    current_pos: usize,
    peek_pos: usize,
}

impl<T> Iterator for Lexer<T>
where
    T: Iterator<Item = (char, usize)>,
{
    type Item = LexResult;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner_next() {
            Ok((Token::Eof, _, _)) => None,
            other => Some(other),
        }
    }
}

impl<T> Lexer<T>
where
    T: Iterator<Item = (char, usize)>,
{
    pub fn new(source: T) -> Self {
        let mut lexer = Self {
            chars: source,
            pending: VecDeque::new(),
            current: None,
            peek: None,
            current_pos: 0,
            peek_pos: 0,
        };
        let _ = lexer.next_char();
        let _ = lexer.next_char();
        lexer
    }

    fn inner_next(&mut self) -> LexResult {
        while self.pending.is_empty() {
            self.consume()?;
        }

        Ok(self.pending.pop_front().unwrap())
    }

    fn consume(&mut self) -> Result<(), LexError> {
        match self.current {
            Some(c) if c.is_alphabetic() => {
                let s = self.lex_word();
                self.emit(s);
            }
            Some(c) if Self::is_number_start(c, self.peek()) => {
                let s = self.lex_number();
                self.emit(s);

                if Some('-') == self.current && Self::is_number_start('-', self.peek) {
                    self.emit_single_char(Token::Sub)?;
                }
            }
            Some(c) => self.consume_char(c)?,
            None => {
                let pos = self.pos();
                self.emit((Token::Eof, pos, pos));
                return Ok(());
            }
        }

        Ok(())
    }

    fn consume_char(&mut self, c: char) -> Result<(), LexError> {
        match c {
            '=' => self.emit_single_char(Token::Eq)?,
            '*' => self.emit_single_char(Token::Mul)?,
            '%' => self.emit_single_char(Token::Mod)?,
            '(' => self.emit_single_char(Token::LeftParen)?,
            ')' => self.emit_single_char(Token::RightParen)?,
            '{' => self.emit_single_char(Token::LeftBrace)?,
            '}' => self.emit_single_char(Token::RightBrace)?,
            ',' => self.emit_single_char(Token::Comma)?,
            ';' => self.emit_single_char(Token::Semicolon)?,
            '/' => {
                let tok_start = self.current_pos;
                self.next_char();
                if self.current == Some('/') {
                    self.next_char();
                    let tok_end = self.pos();
                    self.emit((Token::DivDiv, tok_start, tok_end));
                } else {
                    self.emit_single_char(Token::Div)?;
                }
            }
            '.' => {
                if let Some(next) = self.peek()
                    && !next.is_whitespace()
                {
                    self.emit_single_char(Token::Dot)?;
                } else {
                    let pos = self.pos();
                    self.next_char();
                    return Err(LexError {
                        kind: LexErrorType::UnfinishedDotAccess,
                        location: (pos, pos),
                    });
                }
            }
            '"' => {
                self.next_char();
                let s = self.lex_string()?;
                self.emit(s);
            }
            '+' => {
                let tok_start = self.pos();
                self.next_char();
                match self.current {
                    Some('+') => {
                        self.next_char();
                        let tok_end = self.pos();
                        self.emit((Token::AddAdd, tok_start, tok_end));
                    }
                    _ => {
                        self.emit_single_char(Token::Add)?;
                    }
                }
            }
            '-' => {
                let tok_start = self.pos();
                self.next_char();

                if let Some('-') = self.current {
                    self.next_char();
                    let tok_end = self.pos();
                    self.emit((Token::SubSub, tok_start, tok_end));
                } else {
                    let tok_end = self.pos();
                    self.emit((Token::Sub, tok_start, tok_end));
                }
            }
            '!' => {
                let tok_start = self.pos();
                self.next_char();
                if let Some('=') = self.current {
                    let _ = self.next_char();
                    let tok_end = self.pos();
                    self.emit((Token::Ne, tok_start, tok_end));
                } else {
                    self.emit((Token::Not, tok_start, tok_start));
                }
            }

            '<' => {
                let tok_start = self.pos();
                self.next_char();

                if let Some('=') = self.current {
                    self.next_char();
                    let tok_end = self.pos();
                    self.emit((Token::Le, tok_start, tok_end));
                } else {
                    let tok_end = self.pos();
                    self.emit((Token::Lt, tok_start, tok_end));
                }
            }
            '>' => {
                let tok_start = self.pos();
                self.next_char();
                match self.current {
                    Some('>') => {
                        self.next_char();
                        let tok_end = self.pos();
                        self.emit((Token::GtGt, tok_start, tok_end));
                    }
                    Some('=') => {
                        self.next_char();
                        let tok_end = self.pos();
                        self.emit((Token::Ge, tok_start, tok_end));
                    }
                    _ => {
                        let tok_end = self.pos();
                        self.emit((Token::Gt, tok_start, tok_end));
                    }
                }
            }
            c if c.is_whitespace() => {
                self.next_char();
                return Ok(());
            }
            c => {
                let pos = self.pos();
                self.next_char();
                return Err(LexError {
                    kind: LexErrorType::UnrecognizedToken(c),
                    location: (pos, pos),
                });
            }
        }

        Ok(())
    }

    fn emit_single_char(&mut self, token: Token) -> Result<(), LexError> {
        let tok_start = self.pos();

        match self.next_char() {
            Some(_) => {
                self.emit((token, tok_start, self.pos()));
                Ok(())
            }
            None => Err(LexError {
                kind: LexErrorType::UnexpectedEof,
                location: (tok_start, self.pos()),
            }),
        }
    }

    fn is_number_start(c: char, c1: Option<char>) -> bool {
        match c {
            '0'..='9' => true,
            '-' => matches!(c1, Some('0'..='9')),
            _ => false,
        }
    }

    fn is_word_boundary(c: char) -> bool {
        match c {
            '_' => false,
            _ => {
                c.is_whitespace()
                    | matches!(c, '!'..='/')
                    | matches!(c, ':'..='@')
                    | matches!(c, '['..='`')
                    | matches!(c, '{'..='~')
            }
        }
    }

    fn lex_word(&mut self) -> Spanned {
        let start_pos = self.pos();
        let mut content = String::new();

        loop {
            match self.current {
                Some(c) if Self::is_word_boundary(c) => break,
                Some(c) => content.push(c),
                None => break,
            }
            self.next_char();
        }

        match Token::str_to_keyword(content.as_str()) {
            Some(token) => (token, start_pos, self.pos()),
            None => (Token::Str(content), start_pos, self.pos()),
        }
    }

    fn lex_string(&mut self) -> LexResult {
        let start_pos = self.pos();
        let mut content = String::new();

        loop {
            match self.next_char() {
                Some('"') => break,
                Some('\\') => {
                    let slash_pos = self.pos() - 1;
                    if let Some(c) = self.current
                        && matches!(c, 'f' | 'n' | 't' | 'r' | '"' | '\\')
                    {
                        self.next_char();
                        content.push('\\');
                        content.push(c);
                    } else {
                        // skip to closing quote or eof in case of bad string escape
                        loop {
                            match self.next_char() {
                                Some('"') | None => break,
                                _ => {}
                            }
                        }
                        return Err(LexError {
                            kind: LexErrorType::BadStringEscape,
                            location: (slash_pos, self.pos()),
                        });
                    }
                }
                Some(c) => content.push(c),
                None => {
                    return Err(LexError {
                        kind: LexErrorType::UnexpectedStringEnd,
                        location: (start_pos, start_pos),
                    });
                }
            }
        }

        Ok((Token::Str(content), start_pos, self.pos()))
    }

    fn lex_number(&mut self) -> Spanned {
        let start_pos = self.pos();
        let mut content = String::new();
        let mut is_decimal = false;
        let mut is_string = false;

        if self.current == Some('-') {
            content.push('-');
            self.next_char();
        }

        loop {
            match self.current {
                Some('_') => {}
                Some('.') => {
                    is_decimal = true;
                    content.push('.');
                }
                Some(c) if c.is_alphabetic() => {
                    is_string = true;
                    content.push(c);
                }
                Some(c) if c.is_numeric() => content.push(c),
                _ => break,
            }
            self.next_char();
        }

        if is_string {
            (Token::Str(content), start_pos, self.pos())
        } else if is_decimal {
            (Token::Float(content), start_pos, self.pos())
        } else {
            (Token::Int(content), start_pos, self.pos())
        }
    }

    fn next_char(&mut self) -> Option<char> {
        let c = self.current;
        let nxt = if let Some((c, loc)) = self.chars.next() {
            self.current_pos = self.peek_pos;
            self.peek_pos = loc;
            Some(c)
        } else {
            self.current_pos = self.peek_pos;
            self.peek_pos += 1;
            None
        };
        self.current = self.peek;
        self.peek = nxt;
        c
    }

    fn peek(&self) -> Option<char> {
        self.peek
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
    use crate::lex::error::{LexError, LexErrorType};
    use crate::lex::lexer::make_tokenizer;
    use crate::lex::token::Token;

    macro_rules! assert_tokens {
   ($src:expr, $($expected:expr),* $(,)?) => {
        let tokens = tokenize_sequence($src);
        let expected = vec![$($expected),*];
        assert_eq!(
            tokens.len(),
            expected.len(),
            "Token count mismatch for input: {}",
            $src
        );
        for (i, (token, exp)) in tokens.iter().zip(expected.iter()).enumerate() {
            assert_eq!(token, exp, "Token mismatch at index {}: expected {:?}, got {:?}", i, exp, token);
        }
       ()
    };
}

    macro_rules! assert_error {
    ($src:expr, $($expected:expr),* $(,)?) => {
        let errors = tokenize_error_sequence($src);
        let expected = vec![$($expected),*];
        assert_eq!(
            errors.len(),
            expected.len(),
            "Error count mismatch for input: '{}'",
            $src
        );
        for (i, (error, exp)) in errors.iter().zip(expected.iter()).enumerate() {
            assert_eq!(error, exp, "Error mismatch at index {}: expected {:?}, got {:?}", i, exp, error);
        }
        ()
    };
}

    macro_rules! assert_error_type {
    ($src:expr, $($expected:expr),* $(,)?) => {
        let errors = tokenize_error_sequence($src);
        let expected = vec![$($expected),*];
        assert_eq!(
            errors.len(),
            expected.len(),
            "Error count mismatch for input: '{}'",
            $src
        );
        for (i, (error, exp)) in errors.iter().zip(expected.iter()).enumerate() {
            assert_eq!(error.kind, *exp, "Error mismatch at index {}: expected {:?}, got {:?}", i, exp, error);
        }
        ()
    };
}

    fn tokenize_sequence(source: &str) -> Vec<Token> {
        make_tokenizer(source)
            .map(|x| x.unwrap())
            .map(|x| x.0)
            .collect()
    }

    fn tokenize_error_sequence(source: &str) -> Vec<LexError> {
        make_tokenizer(source)
            .filter(|x| x.is_err())
            .map(|x| x.unwrap_err())
            .collect()
    }

    #[test]
    fn test_flag() {
        assert_tokens!(";top", Token::Semicolon, Token::Str("top".to_string()));
    }

    #[test]
    fn test_value_identifier() {
        assert_tokens!(
            ";top Slay",
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::Str("Slay".to_string())
        );
    }

    #[test]
    fn test_value_string() {
        assert_tokens!(
            ";top \"Tiger Claw\"",
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::Str("Tiger Claw".to_string())
        );
    }

    #[test]
    fn test_value_array_with_commas() {
        assert_tokens!(
            ";top (Slay, \"Tiger Claw\")",
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::LeftParen,
            Token::Str("Slay".to_string()),
            Token::Comma,
            Token::Str("Tiger Claw".to_string()),
            Token::RightParen,
        );
    }

    #[test]
    fn test_value_array_no_commas() {
        assert_tokens!(
            ";top (Slay Lotragon blourgh \"Tiger Claw\")",
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::LeftParen,
            Token::Str("Slay".to_string()),
            Token::Str("Lotragon".to_string()),
            Token::Str("blourgh".to_string()),
            Token::Str("Tiger Claw".to_string()),
            Token::RightParen,
        );
    }

    #[test]
    fn test_value_integer() {
        assert_tokens!(
            ";square 67",
            Token::Semicolon,
            Token::Str("square".to_string()),
            Token::Int("67".to_string()),
        );
    }

    #[test]
    fn test_value_negative_integer() {
        assert_tokens!(
            ";square -69",
            Token::Semicolon,
            Token::Str("square".to_string()),
            Token::Int("-69".to_string()),
        );
    }

    #[test]
    fn test_value_expr() {
        assert_tokens!(
            ";square 67 + 7.27",
            Token::Semicolon,
            Token::Str("square".to_string()),
            Token::Int("67".to_string()),
            Token::Add,
            Token::Float("7.27".to_string()),
        );
    }

    #[test]
    fn test_value_with_underscore() {
        assert_tokens!(
            ";top CreeperBro_2015",
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::Str("CreeperBro_2015".to_string()),
        );
    }

    #[test]
    fn test_option_all_types() {
        assert_tokens!(
            ";top --limit 5 -mode standard -fc",
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::SubSub,
            Token::Str("limit".to_string()),
            Token::Int("5".to_string()),
            Token::Sub,
            Token::Str("mode".to_string()),
            Token::Str("standard".to_string()),
            Token::Sub,
            Token::Str("fc".to_string()),
        );
    }

    #[test]
    fn test_options_with_value() {
        assert_tokens!(
            ";top Slay --limit 5 --mode standard",
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::Str("Slay".to_string()),
            Token::SubSub,
            Token::Str("limit".to_string()),
            Token::Int("5".to_string()),
            Token::SubSub,
            Token::Str("mode".to_string()),
            Token::Str("standard".to_string())
        );
    }

    #[test]
    fn test_error_unclosed_string_value() {
        let s = ";top \"Tiger Claw";
        let last_quote_pos = s.rfind('\"').unwrap() + 1;
        assert_error!(
            s,
            LexError {
                kind: LexErrorType::UnexpectedStringEnd,
                location: (last_quote_pos, last_quote_pos),
            }
        );
    }

    #[test]
    fn test_dot_access() {
        assert_tokens!(
            ";top .some.value",
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::Dot,
            Token::Str("some".to_string()),
            Token::Dot,
            Token::Str("value".to_string()),
        );
    }

    #[test]
    fn test_error_unfinished_dot_access() {
        let s = ";top . --limit 5";
        let err_idx = s.rfind('.').unwrap();
        assert_error!(
            s,
            LexError {
                kind: LexErrorType::UnfinishedDotAccess,
                location: (err_idx, err_idx)
            }
        );
    }

    #[test]
    fn test_error_unfinished_dot_access_at_eof() {
        let s = ";top .";
        let err_idx = s.len() - 1;
        assert_error!(
            s,
            LexError {
                kind: LexErrorType::UnfinishedDotAccess,
                location: (err_idx, err_idx)
            }
        );
    }

    #[test]
    fn test_with_one_pipe_no_expr() {
        assert_tokens!(
            ";top >> count",
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::GtGt,
            Token::Count,
        );
    }

    #[test]
    fn test_one_pipe_with_expr() {
        assert_tokens!(
            ";top >> filter .bpm >= 250",
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::GtGt,
            Token::Filter,
            Token::Dot,
            Token::Str("bpm".to_string()),
            Token::Ge,
            Token::Int("250".to_string()),
        );
    }

    #[test]
    fn test_multiple_pipes_with_expr() {
        assert_tokens!(
            ";top chocomint >> filter .bpm >= 250 >> sort .acc --ascending",
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::Str("chocomint".to_string()),
            Token::GtGt,
            Token::Filter,
            Token::Dot,
            Token::Str("bpm".to_string()),
            Token::Ge,
            Token::Int("250".to_string()),
            Token::GtGt,
            Token::Sort,
            Token::Dot,
            Token::Str("acc".to_string()),
            Token::SubSub,
            Token::Str("ascending".to_string()),
        );
    }

    #[test]
    fn test_subcommand() {
        assert_tokens!(
            ";tops (Slay, Lotragon) ++ { top mrekk --server akatsuki } >> sort .bpm",
            Token::Semicolon,
            Token::Str("tops".to_string()),
            Token::LeftParen,
            Token::Str("Slay".to_string()),
            Token::Comma,
            Token::Str("Lotragon".to_string()),
            Token::RightParen,
            Token::AddAdd,
            Token::LeftBrace,
            Token::Str("top".to_string()),
            Token::Str("mrekk".to_string()),
            Token::SubSub,
            Token::Str("server".to_string()),
            Token::Str("akatsuki".to_string()),
            Token::RightBrace,
            Token::GtGt,
            Token::Sort,
            Token::Dot,
            Token::Str("bpm".to_string()),
        );
    }

    #[test]
    fn test_logic() {
        assert_tokens!(
            ";top >> filter (.bpm >= 230 and HD in .mods) or .bpm >= 250",
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::GtGt,
            Token::Filter,
            Token::LeftParen,
            Token::Dot,
            Token::Str("bpm".to_string()),
            Token::Ge,
            Token::Int("230".to_string()),
            Token::And,
            Token::Str("HD".to_string()),
            Token::In,
            Token::Dot,
            Token::Str("mods".to_string()),
            Token::RightParen,
            Token::Or,
            Token::Dot,
            Token::Str("bpm".to_string()),
            Token::Ge,
            Token::Int("250".to_string()),
        );
    }

    #[test]
    fn test_keyword_it() {
        assert_tokens!(
            ";top >> filter it > 0",
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::GtGt,
            Token::Filter,
            Token::It,
            Token::Gt,
            Token::Int("0".to_string()),
        );
    }

    #[test]
    fn test_keyword_self_alias() {
        assert_tokens!(
            ";top >> filter self > 0",
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::GtGt,
            Token::Filter,
            Token::It,
            Token::Gt,
            Token::Int("0".to_string()),
        );
    }

    #[test]
    fn test_keyword_booleans() {
        assert_tokens!(
            ";top true false",
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::Bool(true),
            Token::Bool(false),
        );
    }

    #[test]
    fn test_keyword_not() {
        assert_tokens!(
            ";top >> filter not .fc",
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::GtGt,
            Token::Filter,
            Token::Not,
            Token::Dot,
            Token::Str("fc".to_string()),
        );
    }

    #[test]
    fn test_operator_bang_not() {
        assert_tokens!(
            ";top >> filter !.fc",
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::GtGt,
            Token::Filter,
            Token::Not,
            Token::Dot,
            Token::Str("fc".to_string()),
        );
    }

    #[test]
    fn test_keyword_contains() {
        assert_tokens!(
            ";top >> filter .title contains loved",
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::GtGt,
            Token::Filter,
            Token::Dot,
            Token::Str("title".to_string()),
            Token::Contains,
            Token::Str("loved".to_string()),
        );
    }

    #[test]
    fn test_keyword_take() {
        assert_tokens!(
            ";top >> take 5",
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::GtGt,
            Token::Take,
            Token::Int("5".to_string()),
        );
    }

    #[test]
    fn test_keyword_with_alias() {
        assert_tokens!(
            ";top with recent",
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::AddAdd,
            Token::Str("recent".to_string()),
        );
    }

    #[test]
    fn test_keyword_is_alias() {
        assert_tokens!(
            ";top >> filter .status is active",
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::GtGt,
            Token::Filter,
            Token::Dot,
            Token::Str("status".to_string()),
            Token::Eq,
            Token::Str("active".to_string()),
        );
    }

    #[test]
    fn test_keyword_above_alias() {
        assert_tokens!(
            ";top >> filter .bpm above 200",
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::GtGt,
            Token::Filter,
            Token::Dot,
            Token::Str("bpm".to_string()),
            Token::Gt,
            Token::Int("200".to_string()),
        );
    }

    #[test]
    fn test_keyword_atleast_alias() {
        assert_tokens!(
            ";top >> filter .bpm atleast 200",
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::GtGt,
            Token::Filter,
            Token::Dot,
            Token::Str("bpm".to_string()),
            Token::Ge,
            Token::Int("200".to_string()),
        );
    }

    #[test]
    fn test_keyword_below_alias() {
        assert_tokens!(
            ";top >> filter .bpm below 300",
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::GtGt,
            Token::Filter,
            Token::Dot,
            Token::Str("bpm".to_string()),
            Token::Lt,
            Token::Int("300".to_string()),
        );
    }

    #[test]
    fn test_keyword_atmost_alias() {
        assert_tokens!(
            ";top >> filter .bpm atmost 300",
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::GtGt,
            Token::Filter,
            Token::Dot,
            Token::Str("bpm".to_string()),
            Token::Le,
            Token::Int("300".to_string()),
        );
    }

    #[test]
    fn test_operator_mul_mod() {
        assert_tokens!(
            ";top 3 * 4 % 2",
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::Int("3".to_string()),
            Token::Mul,
            Token::Int("4".to_string()),
            Token::Mod,
            Token::Int("2".to_string()),
        );
    }

    #[test]
    fn test_operator_div() {
        assert_tokens!(
            ";top 10 / 2",
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::Int("10".to_string()),
            Token::Div,
            Token::Int("2".to_string()),
        );
    }

    #[test]
    fn test_operator_divdiv() {
        assert_tokens!(
            ";top 10 // 3",
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::Int("10".to_string()),
            Token::DivDiv,
            Token::Int("3".to_string()),
        );
    }

    #[test]
    fn test_operator_ne() {
        assert_tokens!(
            ";top >> filter .rank != 1",
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::GtGt,
            Token::Filter,
            Token::Dot,
            Token::Str("rank".to_string()),
            Token::Ne,
            Token::Int("1".to_string()),
        );
    }

    #[test]
    fn test_operator_lt_le() {
        assert_tokens!(
            ";top >> filter .score < 100",
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::GtGt,
            Token::Filter,
            Token::Dot,
            Token::Str("score".to_string()),
            Token::Lt,
            Token::Int("100".to_string()),
        );
        assert_tokens!(
            ";top >> filter .score <= 100",
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::GtGt,
            Token::Filter,
            Token::Dot,
            Token::Str("score".to_string()),
            Token::Le,
            Token::Int("100".to_string()),
        );
    }

    #[test]
    fn test_value_negative_float() {
        assert_tokens!(
            ";top -3.14",
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::Float("-3.14".to_string()),
        );
    }

    #[test]
    fn test_value_quoted_string_with_escape() {
        assert_tokens!(
            r#";top "say \"hi\"""#,
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::Str(r#"say \"hi\""#.to_string()),
        );
    }

    #[test]
    fn test_error_bad_string_escape() {
        assert_error_type!(r#";top "\z""#, LexErrorType::BadStringEscape);
    }

    #[test]
    fn test_error_unrecognized_token() {
        assert_error_type!(";top @value", LexErrorType::UnrecognizedToken('@'));
    }

    #[test]
    fn test_empty_string_literal() {
        assert_tokens!(
            r#";top """#,
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::Str("".to_string()),
        );
    }

    #[test]
    fn test_string_escape_newline() {
        assert_tokens!(
            r#";top "\n""#,
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::Str("\\n".to_string()),
        );
    }

    #[test]
    fn test_string_escape_tab() {
        assert_tokens!(
            r#";top "\t""#,
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::Str("\\t".to_string()),
        );
    }

    #[test]
    fn test_string_escape_backslash() {
        assert_tokens!(
            r#";top "\\""#,
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::Str("\\\\".to_string()),
        );
    }

    #[test]
    fn test_number_zero() {
        assert_tokens!(
            ";top 0",
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::Int("0".to_string()),
        );
    }

    #[test]
    fn test_number_with_underscore() {
        assert_tokens!(
            ";top 1_000",
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::Int("1000".to_string()),
        );
    }

    #[test]
    fn test_operator_ge_symbol() {
        assert_tokens!(
            ";top >= 5",
            Token::Semicolon,
            Token::Str("top".to_string()),
            Token::Ge,
            Token::Int("5".to_string()),
        );
    }

    #[test]
    fn test_error_multiple_unrecognized_tokens() {
        assert_error_type!(
            ";top @ # value",
            LexErrorType::UnrecognizedToken('@'),
            LexErrorType::UnrecognizedToken('#'),
        );
    }
}
