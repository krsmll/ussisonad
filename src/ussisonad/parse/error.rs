use crate::ussisonad::lex::{LexError, Spanned, Token};
use std::fmt;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ParserError {
    LexerStage(Vec<LexError>),
    UnexpectedToken(Spanned),
    UnexpectedTokenWithContext(Token, Spanned),
    ExpectedString(Spanned),
    InvalidInt(Spanned),
    InvalidUnsignedInt(Spanned),
    InvalidFloat(Spanned),
    EmptyVector((usize, usize)),
    UnexpectedTrailingToken(Spanned),
    UnexpectedEOF,
}

impl fmt::Display for ParserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParserError::LexerStage(lex_errors) => {
                let msgs: Vec<String> = lex_errors.iter().map(ToString::to_string).collect();
                write!(f, "lex errors: {}", msgs.join("; "))
            }
            ParserError::UnexpectedToken((tok, start, _)) => {
                write!(f, "unexpected token '{tok}' at position {start}")
            }
            ParserError::UnexpectedTokenWithContext(expected, (got, start, _)) => {
                write!(f, "expected '{expected}', got '{got}' at position {start}")
            }
            ParserError::ExpectedString((tok, start, _)) => {
                write!(f, "expected string, got '{tok}' at position {start}")
            }
            ParserError::InvalidInt((tok, start, _)) => {
                write!(f, "invalid integer literal '{tok}' at position {start}")
            }
            ParserError::InvalidUnsignedInt((tok, start, _)) => {
                write!(
                    f,
                    "invalid non-negative integer literal '{tok}' at position {start}"
                )
            }
            ParserError::InvalidFloat((tok, start, _)) => {
                write!(f, "invalid float literal '{tok}' at position {start}")
            }
            ParserError::EmptyVector((start, _)) => {
                write!(f, "empty vector literal at position {start}")
            }
            ParserError::UnexpectedTrailingToken((tok, start, _)) => {
                write!(f, "unexpected trailing token '{tok}' at position {start}")
            }
            ParserError::UnexpectedEOF => write!(f, "unexpected end of input"),
        }
    }
}

impl std::error::Error for ParserError {}
