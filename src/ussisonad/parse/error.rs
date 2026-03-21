use crate::ussisonad::lex::{LexError, Spanned, Token};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ParserError {
    Lex(Option<Box<ParserError>>, Vec<LexError>),
    UnexpectedToken(Spanned),
    UnexpectedTokenWithContext(Token, Spanned),
    ExpectedString(Spanned),
    IntParseError(Spanned),
    FloatParseError(Spanned),
    EmptyVector((usize, usize)),
    UnexpectedTrailingToken(Spanned),
    UnexpectedEOF,
}
