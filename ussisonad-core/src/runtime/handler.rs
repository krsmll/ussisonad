use crate::runtime::value::Value;
use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum CommandError {
    External(Box<dyn Error + Send + Sync>),
    MissingArgument(&'static str),
    InvalidArgument(String),
    FlagConflict(Vec<&'static str>),
    TypeMismatch {
        expected: Vec<crate::runtime::value::ValueType>,
        got: &'static str,
    },
}

impl CommandError {
    pub fn from_external<E: Error + Send + Sync + 'static>(error: E) -> Self {
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
