//! A pipeline DSL engine for building custom query interfaces.
//!
//! `ussisonad` lets you define commands with typed arguments, register handlers for them,
//! and evaluate pipeline expressions at runtime. It includes a lexer, a Pratt parser,
//! and an async evaluator with built-in support for `filter`, `sort`, `map`, `unique`,
//! `take`, and `count` over vectors.
//!
//! # Quick start
//!
//! ```no_run
//! use ussisonad::{
//!     parse, Evaluator, Registry,
//!     CommandDefinition, CommandHandler, CommandInput, CommandError,
//!     Value, ValueType,
//! };
//! use async_trait::async_trait;
//! use std::sync::Arc;
//!
//! struct GreetHandler;
//!
//! #[async_trait]
//! impl CommandHandler for GreetHandler {
//!     async fn execute(&self, _ctx: &Value, _input: CommandInput) -> Result<Value, CommandError> {
//!         Ok(Value::Str("hello".to_string()))
//!     }
//! }
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let registry = Registry::builder()
//!     .register(
//!         CommandDefinition::builder()
//!             .name("greet")
//!             .returns(ValueType::Str)
//!             .handler(GreetHandler),
//!     )
//!     .build()?;
//!
//! let evaluator = Evaluator::new(Arc::new(registry));
//! let result = evaluator.execute(";greet").await?;
//! assert_eq!(result, Value::Str("hello".to_string()));
//! # Ok(())
//! # }
//! ```
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

/// Parses a DSL input string into a pipeline node ready for evaluation.
pub fn parse(input: &str) -> Result<PipelineNode, ParserError> {
    let tokenizer = ussisonad::lex::make_tokenizer(input);
    ussisonad::parse::parser::Parser::parse(tokenizer)
}
