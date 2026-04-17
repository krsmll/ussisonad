mod eval;
mod lexer;
mod parser;
pub mod runtime;

pub use crate::eval::evaluator::{EvalError, Evaluator};
pub use crate::lexer::{LexError, LexResult, Lexer, Loc, Token};
pub use crate::parser::ast::PipelineNode;
pub use crate::runtime::{
    ArgSchema, ArgSchemaBuilder, CommandDefinition, CommandDefinitionBuilder, CommandError,
    CommandHandler, CommandInput, ConfigError, FieldSchema, FieldSchemaBuilder, ObjectSchema,
    ObjectSchemaBuilder, OptionSchema, OptionSchemaBuilder, Registry, RegistryBuilder, Value,
    ValueType,
};

use crate::parser::expr::{Parser, ParserError};

/// # Errors
///
/// Returns `ParserError` if lexing or parsing fails.
pub fn parse(input: &str) -> Result<PipelineNode, ParserError> {
    let tokenizer = Lexer::new_from_str(input);
    Parser::parse(tokenizer)
}
