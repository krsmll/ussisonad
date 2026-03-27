mod eval;
mod lex;
mod parse;

pub use crate::eval::evaluator::Evaluator;
pub use crate::eval::model::{
    ArgSchema, ArgSchemaBuilder, CommandDefinition, CommandDefinitionBuilder, CommandError,
    CommandHandler, CommandInput, ConfigError, EvalError, FieldSchema, FieldSchemaBuilder,
    ObjectSchema, ObjectSchemaBuilder, OptionSchema, OptionSchemaBuilder, Registry,
    RegistryBuilder, Value, ValueType,
};
pub use crate::lex::{LexError, LexErrorType, Token};
pub use crate::parse::ast::PipelineNode;
pub use crate::parse::error::ParserError;

use crate::parse::parser::Parser;

pub fn parse(input: &str) -> Result<PipelineNode, ParserError> {
    let tokenizer = lex::make_tokenizer(input);
    Parser::parse(tokenizer)
}
