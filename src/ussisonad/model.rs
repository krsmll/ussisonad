use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::sync::Arc;

#[derive(Debug)]
pub enum CommandError {
    External(Box<dyn Error + Send + Sync>),
    MissingArgument(&'static str),
    InvalidArgument(String),
    TypeMismatch {
        expected: &'static str,
        got: &'static str,
    },
}

#[derive(Debug)]
pub enum EvalError {
    UnknownCommand(String),
    UnknownField(String),
    UnexpectedNull,
    TypeMismatch {
        expected: &'static str,
        got: &'static str,
    },
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
    NotIterable,
    Handler(CommandError),
}

#[derive(Debug)]
pub enum ConfigError {
    MissingCommandName,
    MissingCommandReturnType,
    EmptyCommandName,
    DuplicateCommandName(String),
    MissingCommandHandler,

    MissingObjectSchemaName,
    EmptyObjectSchemaName,
    DuplicateObjectSchemaName(String),
    EmptyObjectSchemaFields,

    MissingFieldSchemaName,
    EmptyFieldSchemaName,
    MissingFieldSchemaValueType,

    MissingArgSchemaName,
    EmptyArgSchemaName,

    MissingOptionSchemaName,
    MissingOptionSchemaShort,
    EmptyOptionSchemaName,
    EmptyOptionSchemaShort,
    MissingOptionSchemaValueType,
}

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
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::None => "null",
            Value::Bool(_) => "bool",
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::Str(_) => "str",
            Value::Vector(_) => "vec",
            Value::Object(_) => "obj",
        }
    }

    pub fn into_vector(self) -> Result<Vec<Value>, EvalError> {
        match self {
            Value::Vector(items) => Ok(items),
            other => Err(EvalError::TypeMismatch {
                expected: "vector",
                got: other.type_name(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ValueType {
    Bool,
    Int,
    Float,
    Str,
    Vector(Box<ValueType>),
    Object(Box<ObjectSchema>),
}

impl ValueType {
    pub fn matches(&self, value: &Value) -> bool {
        match (self, value) {
            (ValueType::Bool, Value::Bool(_)) => true,
            (ValueType::Int, Value::Int(_)) => true,
            (ValueType::Float, Value::Float(_)) => true,
            (ValueType::Str, Value::Str(_)) => true,
            (ValueType::Vector(t), Value::Vector(items)) => {
                items.iter().all(|item| t.matches(item))
            }
            (ValueType::Object(schema), Value::Object(map)) => schema.fields.iter().all(|field| {
                map.get(&field.name)
                    .map(|v| field.value_type.matches(v))
                    .unwrap_or(false)
            }),
            _ => false,
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

    pub fn builder() -> RegistryBuilder {
        RegistryBuilder::new()
    }

    pub fn get_command(&self, name: &str) -> Option<&Arc<CommandDefinition>> {
        self.commands.get(name)
    }

    pub fn commands_depending_on(
        &self,
        def: &Arc<CommandDefinition>,
    ) -> Vec<&Arc<CommandDefinition>> {
        self.commands
            .values()
            .filter(|c| c.depends_on.contains(&def.returns))
            .collect::<Vec<_>>()
    }

    pub fn commands_depends_on(
        &self,
        def: &Arc<CommandDefinition>,
    ) -> Vec<&Arc<CommandDefinition>> {
        self.commands
            .values()
            .filter_map(|c| match &c.arg {
                Some(arg) if arg.accepts.contains(&def.returns) => Some(c),
                _ => None,
            })
            .collect::<Vec<_>>()
    }

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
        value_types: &Vec<ValueType>,
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

#[async_trait]
pub trait CommandHandler: Send + Sync {
    async fn execute(&self, context: &Value, input: CommandInput) -> Result<Value, CommandError>;
}

pub struct CommandDefinition {
    pub(crate) name: String,
    pub(crate) aliases: Vec<String>,
    pub(crate) arg: Option<ArgSchema>,
    pub(crate) flags: Vec<String>,
    pub(crate) options: Vec<OptionSchema>,
    pub(crate) description: Option<String>,
    pub(crate) usage: Option<String>,
    pub(crate) depends_on: Vec<ValueType>,
    pub(crate) returns: ValueType,
    pub(crate) handler: Box<dyn CommandHandler>,
}

impl CommandDefinition {
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
    pub fn name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    pub fn alias(mut self, alias: &str) -> Self {
        self.aliases.push(alias.to_string());
        self
    }

    pub fn arg(mut self, arg: ArgSchemaBuilder) -> Self {
        self.arg = Some(arg);
        self
    }

    pub fn flag(mut self, flag: &str) -> Self {
        self.flags.push(flag.to_string());
        self
    }

    pub fn option(mut self, option: OptionSchemaBuilder) -> Self {
        self.options.push(option);
        self
    }

    pub fn description(mut self, description: &str) -> Self {
        self.description = Some(description.to_string());
        self
    }

    pub fn usage(mut self, usage: &str) -> Self {
        self.usage = Some(usage.to_string());
        self
    }

    pub fn depends_on(mut self, value_type: ValueType) -> Self {
        self.depends_on.push(value_type);
        self
    }

    pub fn returns(mut self, value_type: ValueType) -> Self {
        self.returns = Some(value_type);
        self
    }

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
        let arg = self.arg.map(|arg| arg.build()).transpose()?;
        let flags = self.flags;
        let description = self.description;
        let usage = self.usage;
        let handler = self.handler.ok_or(ConfigError::MissingCommandHandler)?;
        let depends_on = self.depends_on;

        let returns = if let Some(t) = self.returns {
            t
        } else {
            return Err(ConfigError::MissingCommandReturnType);
        };

        let options = self
            .options
            .into_iter()
            .map(|a| a.build())
            .collect::<Result<Vec<_>, _>>()?;

        Ok(CommandDefinition {
            name,
            aliases,
            arg,
            flags,
            description,
            usage,
            depends_on,
            returns,
            handler,
            options,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectSchema {
    name: String,
    fields: Vec<FieldSchema>,
}

impl ObjectSchema {
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
    pub fn name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

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
            .map(|f| f.build())
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ObjectSchema { name, fields })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldSchema {
    name: String,
    aliases: Vec<String>,
    value_type: ValueType,
}

impl FieldSchema {
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
    pub fn name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    pub fn alias(mut self, alias: &str) -> Self {
        self.aliases.push(alias.to_string());
        self
    }

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
            .ok_or(ConfigError::MissingFieldSchemaValueType)?;

        Ok(FieldSchema {
            name,
            aliases,
            value_type,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArgSchema {
    pub(crate) name: String,
    pub(crate) accepts: Vec<ValueType>,
    pub(crate) required: bool,
}

impl ArgSchema {
    pub fn builder() -> ArgSchemaBuilder {
        ArgSchemaBuilder::default()
    }
}

#[derive(Default)]
pub struct ArgSchemaBuilder {
    name: Option<String>,
    accepts: Vec<ValueType>,
    required: bool,
}

impl ArgSchemaBuilder {
    pub fn name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    pub fn accepts(mut self, value_type: ValueType) -> Self {
        self.accepts.push(value_type);
        self
    }

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
            return Err(ConfigError::MissingFieldSchemaValueType);
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
    name: String,
    short: String,
    value_type: ValueType,
    default: Option<Value>,
}

impl OptionSchema {
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
    pub fn name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    pub fn short(mut self, short: &str) -> Self {
        self.short = Some(short.to_string());
        self
    }

    pub fn value_type(mut self, value_type: ValueType) -> Self {
        self.value_type = Some(value_type);
        self
    }

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
            .ok_or(ConfigError::MissingOptionSchemaValueType)?;
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
