mod error;
mod lexer;
mod token;

pub(crate) use self::lexer::{LexResult, Spanned, make_tokenizer};
pub use self::{
    error::{LexError, LexErrorType},
    token::Token,
};
