#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum LexErrorType {
    BadStringEscape,
    UnfinishedDotAccess,
    UnexpectedStringEnd,
    UnexpectedEof,
    UnrecognizedToken(char)
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct LexError {
    pub error: LexErrorType,
    pub location: (usize, usize),
}