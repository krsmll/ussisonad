use std::fmt;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum LexErrorType {
    BadStringEscape,
    UnfinishedDotAccess,
    UnexpectedStringEnd,
    UnexpectedEof,
    UnrecognizedToken(char),
}

impl fmt::Display for LexErrorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LexErrorType::BadStringEscape => write!(f, "bad string escape sequence"),
            LexErrorType::UnfinishedDotAccess => write!(f, "unfinished field access"),
            LexErrorType::UnexpectedStringEnd => write!(f, "unexpected end of string literal"),
            LexErrorType::UnexpectedEof => write!(f, "unexpected end of input"),
            LexErrorType::UnrecognizedToken(c) => write!(f, "unrecognized character: {c:?}"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct LexError {
    pub kind: LexErrorType,
    pub location: (usize, usize),
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at {}:{}",
            self.kind, self.location.0, self.location.1
        )
    }
}

impl std::error::Error for LexError {}
