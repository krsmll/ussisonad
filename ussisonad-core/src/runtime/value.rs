use std::collections::HashMap;
use std::fmt;

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

    /// # Errors
    ///
    /// Returns `ConfigError` if required fields are missing, empty, or the field list is empty.
    pub fn build(self) -> Result<ObjectSchema, super::registry::ConfigError> {
        use super::registry::{ConfigError, require_string};

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

    pub(super) fn build(self) -> Result<FieldSchema, super::registry::ConfigError> {
        use super::registry::{ConfigError, require_string};

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
