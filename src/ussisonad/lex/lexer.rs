use crate::ussisonad::lex::error::{LexError, LexErrorType};
use crate::ussisonad::lex::token::Token;

pub type Spanned = (Token, usize, usize);
pub type LexResult = Result<Spanned, LexError>;

pub fn make_tokenizer(source: &str) -> impl Iterator<Item = LexResult> + '_ {
    let chars = source.char_indices().map(|(i, c)| (c, i));
    Lexer::new(chars)
}

#[derive(Debug)]
pub struct Lexer<T: Iterator<Item = (char, usize)>> {
    chars: T,
    pending: Vec<Spanned>,
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
            Ok((Token::EOF, _, _)) => None,
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
            pending: Vec::new(),
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

        Ok(self.pending.remove(0))
    }

    fn consume(&mut self) -> Result<(), LexError> {
        match self.current {
            Some(c) if c.is_alphabetic() => {
                let s = self.lex_word()?;
                self.emit(s)
            }
            Some(c) if self.is_number_start(c, self.peek()) => {
                let s = self.lex_number()?;
                self.emit(s);

                if Some('-') == self.current && self.is_number_start('-', self.peek) {
                    self.emit_single_char(Token::Sub)?;
                }
            }
            Some(c) => self.consume_char(c)?,
            None => {
                let pos = self.pos();
                self.emit((Token::EOF, pos, pos));
                return Ok(());
            }
        }

        Ok(())
    }

    fn consume_char(&mut self, c: char) -> Result<(), LexError> {
        match c {
            '=' => self.emit_single_char(Token::Eq)?,
            '*' => self.emit_single_char(Token::Mul)?,
            '/' => self.emit_single_char(Token::Div)?,
            '%' => self.emit_single_char(Token::Mod)?,
            '(' => self.emit_single_char(Token::LeftParen)?,
            ')' => self.emit_single_char(Token::RightParen)?,
            '{' => self.emit_single_char(Token::LeftBrace)?,
            '}' => self.emit_single_char(Token::RightBrace)?,
            ',' => self.emit_single_char(Token::Comma)?,
            ';' => self.emit_single_char(Token::Semicolon)?,
            '.' => {
                if let Some(next) = self.peek()
                    && !next.is_whitespace()
                {
                    self.emit_single_char(Token::Dot)?
                } else {
                    let pos = self.pos();
                    self.next_char();
                    return Err(LexError {
                        error: LexErrorType::UnfinishedDotAccess,
                        location: (pos, pos),
                    });
                }
            }
            '"' => {
                self.next_char();
                let s = self.lex_string()?;
                self.emit(s)
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
                match self.current {
                    Some('-') => {
                        self.next_char();
                        let tok_end = self.pos();
                        self.emit((Token::SubSub, tok_start, tok_end));
                    }
                    _ => {
                        let tok_end = self.pos();
                        self.emit((Token::Sub, tok_start, tok_end));
                    }
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
                match self.current {
                    Some('=') => {
                        self.next_char();
                        let tok_end = self.pos();
                        self.emit((Token::Le, tok_start, tok_end));
                    }
                    _ => {
                        let tok_end = self.pos();
                        self.emit((Token::Lt, tok_start, tok_end));
                    }
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
                return Err(LexError {
                    error: LexErrorType::UnrecognizedToken(c),
                    location: (pos, pos),
                });
            }
        }

        Ok(())
    }

    fn emit_single_char(&mut self, token: Token) -> Result<(), LexError> {
        let tok_start = self.pos();

        match self.next_char() {
            Some(_) => Ok(self.emit((token, tok_start, self.pos()))),
            None => Err(LexError {
                error: LexErrorType::UnexpectedEof,
                location: (tok_start, self.pos()),
            }),
        }
    }

    fn is_number_start(&self, c: char, c1: Option<char>) -> bool {
        match c {
            '0'..='9' => true,
            '-' => matches!(c1, Some('0'..='9')),
            _ => false,
        }
    }

    fn is_word_boundary(&self, c: char) -> bool {
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

    fn lex_word(&mut self) -> LexResult {
        let start_pos = self.pos();
        let mut content = String::new();

        loop {
            match self.current {
                Some(c) if self.is_word_boundary(c) => break,
                Some(c) => content.push(c),
                None => break,
            }
            self.next_char();
        }

        match Token::keyword_from_str(content.as_str()) {
            Some(token) => Ok((token, start_pos, self.pos())),
            None => Ok((Token::Ident(content), start_pos, self.pos())),
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
                        return Err(LexError {
                            error: LexErrorType::BadStringEscape,
                            location: (slash_pos, self.pos()),
                        });
                    }
                }
                Some(c) => content.push(c),
                None => {
                    return Err(LexError {
                        error: LexErrorType::UnexpectedStringEnd,
                        location: (start_pos, start_pos),
                    });
                }
            }
        }

        Ok((Token::Str(content), start_pos, self.pos()))
    }

    fn lex_number(&mut self) -> LexResult {
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
                Some('_') => continue,
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
            Ok((Token::Str(content), start_pos, self.pos()))
        } else if is_decimal {
            Ok((Token::Float(content), start_pos, self.pos()))
        } else {
            Ok((Token::Int(content), start_pos, self.pos()))
        }
    }

    fn next_char(&mut self) -> Option<char> {
        let c = self.current;
        let nxt = match self.chars.next() {
            Some((c, loc)) => {
                self.current_pos = self.peek_pos;
                self.peek_pos = loc;
                Some(c)
            }
            None => {
                // EOF needs a single advance
                self.current_pos = self.peek_pos;
                self.peek_pos += 1;
                None
            }
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
        self.pending.push(spanned);
    }
}

#[cfg(test)]
mod tests {
    use crate::ussisonad::lex::error::{LexError, LexErrorType};
    use crate::ussisonad::lex::lexer::make_tokenizer;
    use crate::ussisonad::lex::token::Token;

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

    #[allow(dead_code)]
    fn tokenize_sequence(source: &str) -> Vec<Token> {
        make_tokenizer(source)
            .map(|x| x.unwrap())
            .map(|x| x.0)
            .collect()
    }

    #[allow(dead_code)]
    fn tokenize_error_sequence(source: &str) -> Vec<LexError> {
        make_tokenizer(source)
            .filter(|x| x.is_err())
            .map(|x| x.unwrap_err())
            .collect()
    }

    #[test]
    fn flag() {
        assert_tokens!(";top", Token::Semicolon, Token::Ident("top".to_string()));
    }

    #[test]
    fn value_identifier() {
        assert_tokens!(
            ";top Slay",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::Ident("Slay".to_string())
        );
    }

    #[test]
    fn value_string() {
        assert_tokens!(
            ";top \"Tiger Claw\"",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::Str("Tiger Claw".to_string())
        );
    }

    #[test]
    fn value_array_with_commas() {
        assert_tokens!(
            ";top (Slay, \"Tiger Claw\")",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::LeftParen,
            Token::Ident("Slay".to_string()),
            Token::Comma,
            Token::Str("Tiger Claw".to_string()),
            Token::RightParen,
        );
    }

    #[test]
    fn value_array_no_commas() {
        assert_tokens!(
            ";top (Slay Lotragon blourgh \"Tiger Claw\")",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::LeftParen,
            Token::Ident("Slay".to_string()),
            Token::Ident("Lotragon".to_string()),
            Token::Ident("blourgh".to_string()),
            Token::Str("Tiger Claw".to_string()),
            Token::RightParen,
        );
    }

    #[test]
    fn value_integer() {
        assert_tokens!(
            ";square 67",
            Token::Semicolon,
            Token::Ident("square".to_string()),
            Token::Int("67".to_string()),
        );
    }

    #[test]
    fn value_negative_integer() {
        assert_tokens!(
            ";square -69",
            Token::Semicolon,
            Token::Ident("square".to_string()),
            Token::Int("-69".to_string()),
        );
    }

    #[test]
    fn value_expr() {
        assert_tokens!(
            ";square 67 + 7.27",
            Token::Semicolon,
            Token::Ident("square".to_string()),
            Token::Int("67".to_string()),
            Token::Add,
            Token::Float("7.27".to_string()),
        );
    }

    #[test]
    fn value_with_underscore() {
        assert_tokens!(
            ";top CreeperBro_2015",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::Ident("CreeperBro_2015".to_string()),
        );
    }

    #[test]
    fn option_all_types() {
        assert_tokens!(
            ";top --limit 5 -mode standard -fc",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::SubSub,
            Token::Ident("limit".to_string()),
            Token::Int("5".to_string()),
            Token::Sub,
            Token::Ident("mode".to_string()),
            Token::Ident("standard".to_string()),
            Token::Sub,
            Token::Ident("fc".to_string()),
        );
    }

    #[test]
    fn options_with_value() {
        assert_tokens!(
            ";top Slay --limit 5 --mode standard",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::Ident("Slay".to_string()),
            Token::SubSub,
            Token::Ident("limit".to_string()),
            Token::Int("5".to_string()),
            Token::SubSub,
            Token::Ident("mode".to_string()),
            Token::Ident("standard".to_string())
        );
    }

    #[test]
    fn error_unclosed_string_value() {
        let s = ";top \"Tiger Claw";
        let last_quote_pos = s.rfind('\"').unwrap() + 1;
        assert_error!(
            s,
            LexError {
                error: LexErrorType::UnexpectedStringEnd,
                location: (last_quote_pos, last_quote_pos),
            }
        );
    }

    #[test]
    fn dot_access() {
        assert_tokens!(
            ";top .some.value",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::Dot,
            Token::Ident("some".to_string()),
            Token::Dot,
            Token::Ident("value".to_string()),
        );
    }

    #[test]
    fn error_unfinished_dot_access() {
        let s = ";top . --limit 5";
        let err_idx = s.rfind('.').unwrap();
        assert_error!(
            s,
            LexError {
                error: LexErrorType::UnfinishedDotAccess,
                location: (err_idx, err_idx)
            }
        );
    }

    #[test]
    fn error_unfinished_dot_access_at_eof() {
        let s = ";top .";
        let err_idx = s.len() - 1;
        assert_error!(
            s,
            LexError {
                error: LexErrorType::UnfinishedDotAccess,
                location: (err_idx, err_idx)
            }
        );
    }

    #[test]
    fn with_one_pipe_no_expr() {
        assert_tokens!(
            ";top >> count",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::GtGt,
            Token::Count,
        );
    }

    #[test]
    fn one_pipe_with_expr() {
        assert_tokens!(
            ";top >> filter .bpm >= 250",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::GtGt,
            Token::Filter,
            Token::Dot,
            Token::Ident("bpm".to_string()),
            Token::Ge,
            Token::Int("250".to_string()),
        );
    }

    #[test]
    fn multiple_pipes_with_expr() {
        assert_tokens!(
            ";top chocomint >> filter .bpm >= 250 >> sort .acc --ascending",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::Ident("chocomint".to_string()),
            Token::GtGt,
            Token::Filter,
            Token::Dot,
            Token::Ident("bpm".to_string()),
            Token::Ge,
            Token::Int("250".to_string()),
            Token::GtGt,
            Token::Sort,
            Token::Dot,
            Token::Ident("acc".to_string()),
            Token::SubSub,
            Token::Ident("ascending".to_string()),
        );
    }

    #[test]
    fn subcommand() {
        assert_tokens!(
            ";tops (Slay, Lotragon) ++ { top mrekk --server akatsuki } >> sort .bpm",
            Token::Semicolon,
            Token::Ident("tops".to_string()),
            Token::LeftParen,
            Token::Ident("Slay".to_string()),
            Token::Comma,
            Token::Ident("Lotragon".to_string()),
            Token::RightParen,
            Token::AddAdd,
            Token::LeftBrace,
            Token::Ident("top".to_string()),
            Token::Ident("mrekk".to_string()),
            Token::SubSub,
            Token::Ident("server".to_string()),
            Token::Ident("akatsuki".to_string()),
            Token::RightBrace,
            Token::GtGt,
            Token::Sort,
            Token::Dot,
            Token::Ident("bpm".to_string()),
        );
    }

    #[test]
    fn logic() {
        assert_tokens!(
            ";top >> filter (.bpm >= 230 and HD in .mods) or .bpm >= 250",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::GtGt,
            Token::Filter,
            Token::LeftParen,
            Token::Dot,
            Token::Ident("bpm".to_string()),
            Token::Ge,
            Token::Int("230".to_string()),
            Token::And,
            Token::Ident("HD".to_string()),
            Token::In,
            Token::Dot,
            Token::Ident("mods".to_string()),
            Token::RightParen,
            Token::Or,
            Token::Dot,
            Token::Ident("bpm".to_string()),
            Token::Ge,
            Token::Int("250".to_string()),
        );
    }

    #[test]
    fn logic_with_subcommand() {
        assert_tokens!(
            ";top >> filter .title in { top blourgh >> map .title } or .acc > 98.5",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::GtGt,
            Token::Filter,
            Token::Dot,
            Token::Ident("title".to_string()),
            Token::In,
            Token::LeftBrace,
            Token::Ident("top".to_string()),
            Token::Ident("blourgh".to_string()),
            Token::GtGt,
            Token::Map,
            Token::Dot,
            Token::Ident("title".to_string()),
            Token::RightBrace,
            Token::Or,
            Token::Dot,
            Token::Ident("acc".to_string()),
            Token::Gt,
            Token::Float("98.5".to_string()),
        );
    }
}
