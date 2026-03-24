mod ussisonad;

pub use ussisonad::evaluator::Evaluator;

pub use ussisonad::model::{
    ArgSchema, ArgSchemaBuilder, CommandDefinition, CommandDefinitionBuilder, CommandError,
    CommandHandler, CommandInput, ConfigError, EvalError, FieldSchema, FieldSchemaBuilder,
    ObjectSchema, ObjectSchemaBuilder, OptionSchema, OptionSchemaBuilder, Registry,
    RegistryBuilder, Value, ValueType,
};

pub use ussisonad::lex::{LexError, LexErrorType, Token};
pub use ussisonad::parse::ast::PipelineNode;
pub use ussisonad::parse::error::ParserError;

pub fn parse(input: &str) -> Result<PipelineNode, ParserError> {
    let tokenizer = ussisonad::lex::make_tokenizer(input);
    ussisonad::parse::parser::Parser::parse(tokenizer)
}
