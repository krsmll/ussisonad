use crate::{LexError, ParserError};
use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::sync::Arc;

#[derive(Debug)]
pub enum CommandError {
    External(Box<dyn Error + Send + Sync>),
    MissingArgument(&'static str),
    InvalidArgument(String),
    FlagConflict(Vec<&'static str>),
    TypeMismatch {
        expected: Vec<ValueType>,
        got: &'static str,
    },
}

impl CommandError {
    pub fn new_external<E: Error + Send + Sync + 'static>(error: E) -> Self {
        CommandError::External(Box::new(error))
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandError::External(e) => write!(f, "{e}"),
            CommandError::MissingArgument(name) => write!(f, "missing required argument: {name}"),
            CommandError::InvalidArgument(msg) => write!(f, "invalid argument: {msg}"),
            CommandError::FlagConflict(conflicting) => {
                let conflicting = conflicting
                    .iter()
                    .map(|s| format!("'{s}'"))
                    .collect::<Vec<_>>()
                    .join(", ");

                write!(f, "incompatible flags: {conflicting}")
            }
            CommandError::TypeMismatch { expected, got } => {
                let expected = expected
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "type mismatch: expected {expected}, got {got}")
            }
        }
    }
}

impl Error for CommandError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            CommandError::External(e) => Some(e.as_ref()),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum EvalError {
    UnknownCommand(String),
    UnknownField(String),
    TypeMismatch {
        expected: &'static str,
        got: &'static str,
    },
    NumberTypeMismatch(Vec<&'static str>),
    UnexpectedInputType {
        command: String,
        expected: Vec<ValueType>,
        got: &'static str,
    },
    UnexpectedArgumentType {
        command: String,
        expected: Vec<ValueType>,
        got: &'static str,
    },
    UnexpectedReturnType {
        command: String,
        expected: ValueType,
        got: &'static str,
    },
    NotComparable(&'static str, &'static str),
    LexerStage(Vec<LexError>),
    ParsingStage(ParserError),
    Handler(CommandError),
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvalError::UnknownCommand(name) => write!(f, "unknown command: {name}"),
            EvalError::UnknownField(name) => write!(f, "unknown field: {name}"),
            EvalError::TypeMismatch { expected, got } => {
                write!(f, "type mismatch: expected {expected}, got {got}")
            }
            EvalError::NumberTypeMismatch(types) => {
                write!(
                    f,
                    "cannot apply numeric operation to types: {}",
                    types.join(", ")
                )
            }
            EvalError::UnexpectedInputType {
                command,
                expected,
                got,
            } => {
                let expected: Vec<String> = expected.iter().map(ToString::to_string).collect();
                write!(
                    f,
                    "command '{command}' received unexpected input type: expected one of [{}], got {got}",
                    expected.join(", ")
                )
            }
            EvalError::UnexpectedArgumentType {
                command,
                expected,
                got,
            } => {
                let expected: Vec<String> = expected.iter().map(ToString::to_string).collect();
                write!(
                    f,
                    "command '{command}' received unexpected argument type: expected one of [{}], got {got}",
                    expected.join(", ")
                )
            }
            EvalError::UnexpectedReturnType {
                command,
                expected,
                got,
            } => {
                write!(
                    f,
                    "command '{command}' returned unexpected type: expected {expected}, got {got}"
                )
            }
            EvalError::NotComparable(a, b) => {
                write!(f, "values of type '{a}' and '{b}' are not comparable")
            }
            EvalError::LexerStage(reasons) => {
                let reasons: Vec<String> = reasons.iter().map(ToString::to_string).collect();
                write!(f, "errors occurred during lexing: [{}]", reasons.join(", "))
            }
            EvalError::ParsingStage(reason) => {
                write!(f, "errors occurred during parsing: {reason}")
            }
            EvalError::Handler(e) => write!(f, "command handler error: {e}"),
        }
    }
}

impl Error for EvalError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            EvalError::Handler(e) => Some(e),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum ConfigError {
    MissingCommandName,
    MissingCommandReturnType(String),
    EmptyCommandName,
    DuplicateCommandName(String),
    MissingCommandHandler,

    MissingObjectSchemaName,
    EmptyObjectSchemaName,
    DuplicateObjectSchemaName(String),
    EmptyObjectSchemaFields,

    MissingFieldSchemaName,
    EmptyFieldSchemaName,
    MissingFieldSchemaValueType(String),

    MissingArgSchemaName,
    EmptyArgSchemaName,
    MissingArgSchemaValueType(String),

    MissingOptionSchemaName,
    MissingOptionSchemaShort,
    EmptyOptionSchemaName,
    EmptyOptionSchemaShort,
    MissingOptionSchemaValueType(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::MissingCommandName => write!(f, "command is missing a name"),
            ConfigError::MissingCommandReturnType(name) => {
                write!(f, "command '{name}' is missing a return type")
            }
            ConfigError::EmptyCommandName => write!(f, "command name must not be empty"),
            ConfigError::DuplicateCommandName(name) => {
                write!(f, "duplicate command name: '{name}'")
            }
            ConfigError::MissingCommandHandler => write!(f, "command is missing a handler"),

            ConfigError::MissingObjectSchemaName => write!(f, "object schema is missing a name"),
            ConfigError::EmptyObjectSchemaName => {
                write!(f, "object schema name must not be empty")
            }
            ConfigError::DuplicateObjectSchemaName(name) => {
                write!(f, "duplicate object schema name: '{name}'")
            }
            ConfigError::EmptyObjectSchemaFields => {
                write!(f, "object schema must have at least one field")
            }

            ConfigError::MissingFieldSchemaName => write!(f, "field schema is missing a name"),
            ConfigError::EmptyFieldSchemaName => {
                write!(f, "field schema name must not be empty")
            }
            ConfigError::MissingFieldSchemaValueType(name) => {
                write!(f, "field '{name}' is missing a value type")
            }

            ConfigError::MissingArgSchemaName => write!(f, "arg schema is missing a name"),
            ConfigError::EmptyArgSchemaName => write!(f, "arg schema name must not be empty"),
            ConfigError::MissingArgSchemaValueType(name) => {
                write!(f, "arg '{name}' must accept at least one value type")
            }

            ConfigError::MissingOptionSchemaName => write!(f, "option schema is missing a name"),
            ConfigError::MissingOptionSchemaShort => {
                write!(f, "option schema is missing a short name")
            }
            ConfigError::EmptyOptionSchemaName => {
                write!(f, "option schema name must not be empty")
            }
            ConfigError::EmptyOptionSchemaShort => {
                write!(f, "option schema short name must not be empty")
            }
            ConfigError::MissingOptionSchemaValueType(name) => {
                write!(f, "option '{name}' is missing a value type")
            }
        }
    }
}

impl Error for ConfigError {}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    None,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Vector(Vec<Value>),
    Object(HashMap<String, Value>),
}

impl Value {
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::None => "null",
            Value::Bool(_) => "boolean",
            Value::Int(_) => "integer",
            Value::Float(_) => "decimal",
            Value::Str(_) => "string",
            Value::Vector(_) => "vec",
            Value::Object(_) => "obj",
        }
    }

    pub fn into_vector(self) -> Result<Vec<Value>, EvalError> {
        match self {
            Value::Vector(items) => Ok(items),
            other => Err(EvalError::TypeMismatch {
                expected: "list",
                got: other.type_name(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ValueType {
    None,
    Bool,
    Int,
    Float,
    Str,
    Vector(Box<ValueType>),
    Object(Box<ObjectSchema>),
}

impl ValueType {
    #[must_use]
    pub fn matches(&self, value: &Value) -> bool {
        match (self, value) {
            (ValueType::Bool, Value::Bool(_))
            | (ValueType::Int, Value::Int(_))
            | (ValueType::Float, Value::Float(_))
            | (ValueType::Str, Value::Str(_)) => true,
            (ValueType::Vector(t), Value::Vector(items)) => {
                items.iter().all(|item| t.matches(item))
            }
            (ValueType::Object(schema), Value::Object(map)) => schema.fields.iter().all(|field| {
                map.get(&field.name)
                    .is_some_and(|v| field.value_type.matches(v))
            }),
            _ => false,
        }
    }
}

impl fmt::Display for ValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValueType::None => write!(f, "none"),
            ValueType::Bool => write!(f, "boolean"),
            ValueType::Int => write!(f, "integer"),
            ValueType::Float => write!(f, "decimal"),
            ValueType::Str => write!(f, "string"),
            ValueType::Vector(inner) => write!(f, "list<{inner}>"),
            ValueType::Object(schema) => write!(f, "object<{}>", schema.name),
        }
    }
}

pub struct Registry {
    commands: HashMap<String, Arc<CommandDefinition>>,
    schemas: HashMap<String, Arc<ObjectSchema>>,
}

impl Registry {
    fn new(
        commands: HashMap<String, Arc<CommandDefinition>>,
        schemas: HashMap<String, Arc<ObjectSchema>>,
    ) -> Self {
        Self { commands, schemas }
    }

    #[must_use]
    pub fn builder() -> RegistryBuilder {
        RegistryBuilder::new()
    }

    #[must_use]
    pub fn get_command(&self, name: &str) -> Option<&Arc<CommandDefinition>> {
        self.commands.get(name)
    }

    #[must_use]
    pub fn commands_that_accept(
        &self,
        def: &Arc<CommandDefinition>,
    ) -> Vec<&Arc<CommandDefinition>> {
        self.commands
            .values()
            .filter(|c| c.depends_on.contains(&def.returns))
            .collect::<Vec<_>>()
    }

    #[must_use]
    pub fn commands_that_return(
        &self,
        def: &Arc<CommandDefinition>,
    ) -> Vec<&Arc<CommandDefinition>> {
        self.commands
            .values()
            .filter(|c| match &c.arg {
                Some(arg) => arg.accepts.contains(&def.returns),
                _ => false,
            })
            .collect::<Vec<_>>()
    }

    #[must_use]
    pub fn get_schema(&self, name: &str) -> Option<&Arc<ObjectSchema>> {
        self.schemas.get(name)
    }
}

pub struct RegistryBuilder {
    commands: Vec<CommandDefinitionBuilder>,
}

impl RegistryBuilder {
    fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    #[must_use]
    pub fn register(mut self, def: CommandDefinitionBuilder) -> Self {
        self.commands.push(def);
        self
    }

    pub fn build(self) -> Result<Registry, ConfigError> {
        let mut commands: HashMap<String, Arc<CommandDefinition>> = HashMap::new();
        let mut schemas: HashMap<String, Arc<ObjectSchema>> = HashMap::new();

        for command in self.commands {
            let def = command.build()?;

            Self::populate_schema(&mut schemas, &def.returns)?;
            Self::populate_schemas(&mut schemas, &def.depends_on)?;

            if let Some(arg) = &def.arg {
                Self::populate_schemas(&mut schemas, &arg.accepts)?;
            }

            let arc = Arc::new(def);
            if commands.insert(arc.name.clone(), arc.clone()).is_some() {
                return Err(ConfigError::DuplicateCommandName(arc.name.clone()));
            }

            for alias in &arc.aliases {
                if commands.insert(alias.clone(), arc.clone()).is_some() {
                    return Err(ConfigError::DuplicateCommandName(alias.clone()));
                }
            }
        }

        Ok(Registry::new(commands, schemas))
    }

    fn populate_schemas(
        schemas: &mut HashMap<String, Arc<ObjectSchema>>,
        value_types: &[ValueType],
    ) -> Result<(), ConfigError> {
        for t in value_types {
            Self::populate_schema(schemas, t)?;
        }
        Ok(())
    }

    fn populate_schema(
        schemas: &mut HashMap<String, Arc<ObjectSchema>>,
        value_type: &ValueType,
    ) -> Result<(), ConfigError> {
        if let Some(schema) = Self::extract_object_schema(value_type)? {
            schemas.insert(schema.name.clone(), Arc::new(schema));
        }
        Ok(())
    }

    fn extract_object_schema(value_type: &ValueType) -> Result<Option<ObjectSchema>, ConfigError> {
        match value_type {
            ValueType::Vector(item_type) => Self::extract_object_schema(item_type),
            ValueType::Object(schema) => {
                if schema.name.trim().is_empty() {
                    Err(ConfigError::EmptyObjectSchemaName)
                } else if schema.fields.is_empty() {
                    Err(ConfigError::EmptyObjectSchemaFields)
                } else {
                    Ok(Some(*schema.clone()))
                }
            }
            _ => Ok(None),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CommandInput {
    pub arg: Value,
    pub flags: HashSet<String>,
    pub options: HashMap<String, Value>,
}

impl CommandInput {
    pub fn has_flag(&self, flag: &str) -> bool {
        self.flags.contains(flag)
    }

    pub fn get_option(&self, option_name: &str) -> Option<&Value> {
        self.options.get(option_name)
    }
}

#[async_trait]
pub trait CommandHandler: Send + Sync {
    async fn execute(&self, context: Value, input: CommandInput) -> Result<Value, CommandError>;
}

pub struct CommandDefinition {
    pub name: String,
    pub aliases: Vec<String>,
    pub arg: Option<ArgSchema>,
    pub flags: Vec<String>,
    pub options: Vec<OptionSchema>,
    pub description: Option<String>,
    pub usage: Option<String>,
    pub depends_on: Vec<ValueType>,
    pub returns: ValueType,
    pub(crate) handler: Box<dyn CommandHandler>,
}

impl CommandDefinition {
    #[must_use]
    pub fn builder() -> CommandDefinitionBuilder {
        CommandDefinitionBuilder::default()
    }
}

#[derive(Default)]
pub struct CommandDefinitionBuilder {
    name: Option<String>,
    aliases: Vec<String>,
    arg: Option<ArgSchemaBuilder>,
    flags: Vec<String>,
    options: Vec<OptionSchemaBuilder>,
    description: Option<String>,
    usage: Option<String>,
    depends_on: Vec<ValueType>,
    returns: Option<ValueType>,
    handler: Option<Box<dyn CommandHandler>>,
}

impl CommandDefinitionBuilder {
    #[must_use]
    pub fn name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    #[must_use]
    pub fn alias(mut self, alias: &str) -> Self {
        self.aliases.push(alias.to_string());
        self
    }

    #[must_use]
    pub fn arg(mut self, arg: ArgSchemaBuilder) -> Self {
        self.arg = Some(arg);
        self
    }

    #[must_use]
    pub fn flag(mut self, flag: &str) -> Self {
        self.flags.push(flag.to_string());
        self
    }

    #[must_use]
    pub fn option(mut self, option: OptionSchemaBuilder) -> Self {
        self.options.push(option);
        self
    }

    #[must_use]
    pub fn description(mut self, description: &str) -> Self {
        self.description = Some(description.to_string());
        self
    }

    #[must_use]
    pub fn usage(mut self, usage: &str) -> Self {
        self.usage = Some(usage.to_string());
        self
    }

    #[must_use]
    pub fn depends_on(mut self, value_type: ValueType) -> Self {
        self.depends_on.push(value_type);
        self
    }

    #[must_use]
    pub fn returns(mut self, value_type: ValueType) -> Self {
        self.returns = Some(value_type);
        self
    }

    #[must_use]
    pub fn handler(mut self, handler: impl CommandHandler + 'static) -> Self {
        self.handler = Some(Box::new(handler));
        self
    }

    pub fn build(self) -> Result<CommandDefinition, ConfigError> {
        let name = require_string(
            self.name,
            ConfigError::MissingCommandName,
            ConfigError::EmptyCommandName,
        )?;
        let aliases = self.aliases;
        let arg = self.arg.map(ArgSchemaBuilder::build).transpose()?;
        let flags = self.flags;
        let description = self.description;
        let usage = self.usage;
        let handler = self.handler.ok_or(ConfigError::MissingCommandHandler)?;
        let depends_on = self.depends_on;
        let returns = self
            .returns
            .ok_or(ConfigError::MissingCommandReturnType(name.clone()))?;

        let options = self
            .options
            .into_iter()
            .map(OptionSchemaBuilder::build)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(CommandDefinition {
            name,
            aliases,
            arg,
            flags,
            options,
            description,
            usage,
            depends_on,
            returns,
            handler,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectSchema {
    pub name: String,
    pub fields: Vec<FieldSchema>,
}

impl ObjectSchema {
    #[must_use]
    pub fn builder() -> ObjectSchemaBuilder {
        ObjectSchemaBuilder::default()
    }
}

#[derive(Default)]
pub struct ObjectSchemaBuilder {
    name: Option<String>,
    fields: Vec<FieldSchemaBuilder>,
}

impl ObjectSchemaBuilder {
    #[must_use]
    pub fn name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    #[must_use]
    pub fn field(mut self, field: FieldSchemaBuilder) -> Self {
        self.fields.push(field);
        self
    }

    pub fn build(self) -> Result<ObjectSchema, ConfigError> {
        if self.fields.is_empty() {
            return Err(ConfigError::EmptyObjectSchemaFields);
        }

        let name = require_string(
            self.name,
            ConfigError::MissingObjectSchemaName,
            ConfigError::EmptyObjectSchemaName,
        )?;

        let fields = self
            .fields
            .into_iter()
            .map(FieldSchemaBuilder::build)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ObjectSchema { name, fields })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldSchema {
    pub name: String,
    pub aliases: Vec<String>,
    pub value_type: ValueType,
}

impl FieldSchema {
    #[must_use]
    pub fn builder() -> FieldSchemaBuilder {
        FieldSchemaBuilder::default()
    }
}

#[derive(Default)]
pub struct FieldSchemaBuilder {
    name: Option<String>,
    aliases: Vec<String>,
    value_type: Option<ValueType>,
}

impl FieldSchemaBuilder {
    #[must_use]
    pub fn name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    #[must_use]
    pub fn alias(mut self, alias: &str) -> Self {
        self.aliases.push(alias.to_string());
        self
    }

    #[must_use]
    pub fn value_type(mut self, value_type: ValueType) -> Self {
        self.value_type = Some(value_type);
        self
    }

    fn build(self) -> Result<FieldSchema, ConfigError> {
        let name = require_string(
            self.name,
            ConfigError::MissingFieldSchemaName,
            ConfigError::EmptyFieldSchemaName,
        )?;
        let aliases = self.aliases;
        let value_type = self
            .value_type
            .ok_or(ConfigError::MissingFieldSchemaValueType(name.clone()))?;

        Ok(FieldSchema {
            name,
            aliases,
            value_type,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArgSchema {
    pub name: String,
    pub accepts: Vec<ValueType>,
    pub required: bool,
}

impl ArgSchema {
    #[must_use]
    pub fn builder() -> ArgSchemaBuilder {
        ArgSchemaBuilder::default()
    }

    pub fn accepts(&self, value: &Value) -> bool {
        self.accepts.iter().any(|accept| accept.matches(value))
    }
}

#[derive(Default)]
pub struct ArgSchemaBuilder {
    name: Option<String>,
    accepts: Vec<ValueType>,
    required: bool,
}

impl ArgSchemaBuilder {
    #[must_use]
    pub fn name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    #[must_use]
    pub fn accepts(mut self, value_type: ValueType) -> Self {
        self.accepts.push(value_type);
        self
    }

    #[must_use]
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    fn build(self) -> Result<ArgSchema, ConfigError> {
        let required = self.required;
        let name = require_string(
            self.name,
            ConfigError::MissingArgSchemaName,
            ConfigError::EmptyArgSchemaName,
        )?;

        let accepts = if self.accepts.is_empty() {
            return Err(ConfigError::MissingArgSchemaValueType(name));
        } else {
            self.accepts
        };

        Ok(ArgSchema {
            name,
            accepts,
            required,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OptionSchema {
    pub name: String,
    pub short: String,
    pub value_type: ValueType,
    pub default: Option<Value>,
}

impl OptionSchema {
    #[must_use]
    pub fn builder() -> OptionSchemaBuilder {
        OptionSchemaBuilder::default()
    }
}

#[derive(Default)]
pub struct OptionSchemaBuilder {
    name: Option<String>,
    short: Option<String>,
    value_type: Option<ValueType>,
    default: Option<Value>,
}

impl OptionSchemaBuilder {
    #[must_use]
    pub fn name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    #[must_use]
    pub fn short(mut self, short: &str) -> Self {
        self.short = Some(short.to_string());
        self
    }

    #[must_use]
    pub fn value_type(mut self, value_type: ValueType) -> Self {
        self.value_type = Some(value_type);
        self
    }

    #[must_use]
    pub fn default_value(mut self, default: Value) -> Self {
        self.default = Some(default);
        self
    }

    fn build(self) -> Result<OptionSchema, ConfigError> {
        let name = require_string(
            self.name,
            ConfigError::MissingOptionSchemaName,
            ConfigError::EmptyOptionSchemaName,
        )?;
        let short = match self.short {
            None => return Err(ConfigError::MissingOptionSchemaShort),
            Some(s) if s.trim().is_empty() => return Err(ConfigError::EmptyOptionSchemaShort),
            Some(s) => s,
        };
        let value_type = self
            .value_type
            .ok_or(ConfigError::MissingOptionSchemaValueType(name.clone()))?;
        let default = self.default;

        Ok(OptionSchema {
            name,
            short,
            value_type,
            default,
        })
    }
}

fn require_string(
    value: Option<String>,
    missing_err: ConfigError,
    empty_err: ConfigError,
) -> Result<String, ConfigError> {
    match value {
        None => Err(missing_err),
        Some(s) if s.trim().is_empty() => Err(empty_err),
        Some(s) => Ok(s),
    }
}
