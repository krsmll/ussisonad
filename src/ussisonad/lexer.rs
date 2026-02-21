use crate::ussisonad::error::{LexError, LexErrorType, SrcSpan};
use crate::ussisonad::token::Token;

pub type Spanned = (Token, usize, usize);
pub type LexResult = Result<Spanned, LexError>;

pub fn make_tokenizer(source: &str) -> impl Iterator<Item = LexResult> + '_ {
    let chars = source.char_indices().map(|(i, c)| (c, i));
    let char_iter = CharIterator::new(chars);
    Lexer::new(char_iter)
}

pub struct CharIterator<T: Iterator<Item = (char, usize)>> {
    source: T,
    ch0: Option<(char, usize)>,
    ch1: Option<(char, usize)>,
}

impl<T> CharIterator<T>
where
    T: Iterator<Item = (char, usize)>,
{
    pub fn new(source: T) -> Self {
        let mut nlh = CharIterator {
            source,
            ch0: None,
            ch1: None,
        };
        let _ = nlh.shift();
        let _ = nlh.shift();
        nlh
    }

    fn shift(&mut self) -> Option<(char, usize)> {
        let result = self.ch0;
        self.ch0 = self.ch1;
        self.ch1 = self.source.next();
        result
    }
}

impl<T> Iterator for CharIterator<T>
where
    T: Iterator<Item = (char, usize)>,
{
    type Item = (char, usize);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(('\r', i)) = self.ch0 {
            if let Some(('\n', _)) = self.ch1 {
                let _ = self.shift();
                self.ch0 = Some(('\n', i))
            } else {
                self.ch0 = Some(('\n', i))
            }
        }
        self.shift()
    }
}

#[derive(Debug)]
pub struct Lexer<T: Iterator<Item = (char, usize)>> {
    chars: T,
    pending: Vec<Spanned>,
    ch0: Option<char>,
    ch1: Option<char>,
    loc0: usize,
    loc1: usize,
}

impl<T> Lexer<T>
where
    T: Iterator<Item = (char, usize)>,
{
    pub fn new(source: T) -> Self {
        let mut lexer = Self {
            chars: source,
            pending: Vec::new(),
            ch0: None,
            ch1: None,
            loc0: 0,
            loc1: 0,
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
        let mut check_for_minus = false;
        if let Some(current) = self.current_char() {
            if current.is_alphabetic(){
                let s = self.lex_word()?;
                self.emit(s)
            } else if self.is_number_start(current, self.peek()) {
                check_for_minus = true;
                let s = self.lex_number()?;
                self.emit(s);
            } else {
                self.consume_char(current)?
            }

            if check_for_minus {
                if Some('-') == self.ch0 && self.is_number_start('-', self.ch1) {
                    self.emit_single_char(Token::Minus)?;
                }
            }
        } else {
            let eof_pos = self.pos();
            self.emit((Token::EOF, eof_pos, eof_pos))
        }
        Ok(())
    }

    fn consume_char(&mut self, c: char) -> Result<(), LexError> {
        match c {
            '=' => self.emit_single_char(Token::Eq)?,
            '+' => self.emit_single_char(Token::Plus)?,
            '*' => self.emit_single_char(Token::Asterisk)?,
            '/' => self.emit_single_char(Token::Slash)?,
            '%' => self.emit_single_char(Token::Percent)?,
            '(' => self.emit_single_char(Token::LeftParen)?,
            ')' => self.emit_single_char(Token::RightParen)?,
            '{' => self.emit_single_char(Token::LeftBrace)?,
            '}' => self.emit_single_char(Token::RightBrace)?,
            ',' => self.emit_single_char(Token::Comma)?,
            '.' => self.emit_single_char(Token::Dot)?,
            ';' => self.emit_single_char(Token::Semicolon)?,
            '"' => {
                self.next_char();
                let s = self.lex_string()?;
                self.emit(s)
            }
            '-' => {
                let tok_start = self.pos();
                self.next_char();
                match self.current_char() {
                    Some('-') => {
                        self.next_char();
                        let tok_end = self.pos();
                        self.emit((Token::MinusMinus, tok_start, tok_end));
                    }
                    _ => {
                        let tok_end = self.pos();
                        self.emit((Token::Minus, tok_start, tok_end));
                    }
                }
            }
            '!' => {
                let tok_start = self.pos();
                self.next_char();
                if let Some('=') = self.current_char() {
                    let _ = self.next_char();
                    let tok_end = self.pos();
                    self.emit((Token::Ne, tok_start, tok_end));
                } else {
                    return Err(LexError {
                        error: LexErrorType::UnrecognizedToken('!'),
                        location: SrcSpan {
                            start: tok_start,
                            end: tok_start,
                        },
                    });
                }
            }

            '<' => {
                let tok_start = self.pos();
                self.next_char();
                match self.current_char() {
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
                match self.current_char() {
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
                let location = self.pos();
                return Err(LexError {
                    error: LexErrorType::UnrecognizedToken(c),
                    location: SrcSpan {
                        start: location,
                        end: location,
                    },
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
                location: SrcSpan {
                    start: tok_start,
                    end: self.pos(),
                },
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
            match self.current_char() {
                Some(c) if self.is_word_boundary(c) => break,
                Some(c) => content.push(c),
                None => break,
            }
            self.next_char();
        }

        match Token::keyword_from_str(content.as_str()) {
            Some(token) => Ok((token, start_pos, self.pos())),
            None => Ok((Token::Word(content), start_pos, self.pos())),
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
                    if let Some(c) = self.current_char()
                        && matches!(c, 'f' | 'n' | 't' | 'r' | '"' | '\\')
                    {
                        self.next_char();
                        content.push('\\');
                        content.push(c);
                    } else {
                        return Err(LexError {
                            error: LexErrorType::BadStringEscape,
                            location: SrcSpan {
                                start: slash_pos,
                                end: self.pos(),
                            },
                        });
                    }
                }
                Some(c) => content.push(c),
                None => {
                    return Err(LexError {
                        error: LexErrorType::UnexpectedStringEnd,
                        location: SrcSpan {
                            start: start_pos,
                            end: start_pos,
                        },
                    });
                }
            }
        }

        Ok((Token::String(content), start_pos, self.pos()))
    }

    fn lex_number(&mut self) -> LexResult {
        let start_pos = self.pos();
        let mut content = String::new();
        let mut is_decimal = false;
        let mut is_string = false;

        if self.current_char() == Some('-') {
            content.push('-');
            self.next_char();
        }

        loop {
            match self.current_char() {
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
            Ok((Token::String(content), start_pos, self.pos()))
        } else if is_decimal {
            Ok((Token::Float(content), start_pos, self.pos()))
        } else {
            Ok((Token::Integer(content), start_pos, self.pos()))
        }
    }

    fn next_char(&mut self) -> Option<char> {
        let c = self.ch0;
        let nxt = match self.chars.next() {
            Some((c, loc)) => {
                self.loc0 = self.loc1;
                self.loc1 = loc;
                Some(c)
            }
            None => {
                // EOF needs a single advance
                self.loc0 = self.loc1;
                self.loc1 += 1;
                None
            }
        };
        self.ch0 = self.ch1;
        self.ch1 = nxt;
        c
    }

    fn current_char(&self) -> Option<char> {
        self.ch0
    }

    fn peek(&self) -> Option<char> {
        self.ch1
    }

    fn pos(&self) -> usize {
        self.loc0
    }

    fn emit(&mut self, spanned: Spanned) {
        self.pending.push(spanned);
    }
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
