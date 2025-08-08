pub mod current;
pub mod parser;
pub mod schema;
pub mod types;
pub mod validation;

use derive_more::{Display, Error};
use indexmap::IndexMap;
use std::{borrow::Cow, fmt::Debug, str::FromStr};
use toml::Value as TomlValue;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PrimitiveType {
    String,
    Int64,
    Int32,
    Int16,
    Float64,
    Float32,
    Boolean,
    Duration,
}

impl PrimitiveType {
    pub fn as_str(&self) -> &'static str {
        match self {
            PrimitiveType::String => "str",
            PrimitiveType::Int64 => "int64",
            PrimitiveType::Int32 => "int32",
            PrimitiveType::Int16 => "int16",
            PrimitiveType::Float64 => "float64",
            PrimitiveType::Float32 => "float32",
            PrimitiveType::Boolean => "bool",
            PrimitiveType::Duration => "duration",
        }
    }
}

impl FromStr for PrimitiveType {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "str" => Ok(PrimitiveType::String),
            "int64" => Ok(PrimitiveType::Int64),
            "int32" => Ok(PrimitiveType::Int32),
            "int16" => Ok(PrimitiveType::Int16),
            "float64" => Ok(PrimitiveType::Float64),
            "float32" => Ok(PrimitiveType::Float32),
            "bool" => Ok(PrimitiveType::Boolean),
            "duration" => Ok(PrimitiveType::Duration),
            _ => Err(()),
        }
    }
}

impl std::fmt::Display for PrimitiveType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Clone)]
pub enum Value {
    Injected(String),
    Set(Vec<Value>),
    Array(Vec<Value>),
    Insert {
        typ: String,
        values: IndexMap<String, Value>,
    },
}

impl Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Injected(s) => write!(f, "{s}"),
            Value::Set(values) => {
                write!(f, "[")?;
                for (i, value) in values.iter().enumerate() {
                    write!(f, "{value:?}")?;
                    if i < values.len() - 1 {
                        write!(f, ", ")?;
                    }
                }
                write!(f, "]")
            }
            Value::Array(values) => {
                write!(f, "[")?;
                for (i, value) in values.iter().enumerate() {
                    write!(f, "{value:?}")?;
                    if i < values.len() - 1 {
                        write!(f, ", ")?;
                    }
                }
                write!(f, "]")
            }
            Value::Insert { typ, values } => {
                writeln!(f, "insert {typ} = {{")?;
                for (key, value) in values {
                    writeln!(f, "  {key}: {value:?},")?;
                }
                write!(f, "}}")
            }
        }
    }
}

pub trait HintExt {
    fn with_hint<F>(self, hint: F) -> Self
    where
        F: FnOnce() -> String;
}

impl HintExt for ConfigError {
    fn with_hint<F>(self, _hint: F) -> Self
    where
        F: FnOnce() -> String,
    {
        // For now, we'll ignore hints since we don't have a way to store them
        // This could be extended later if needed
        self
    }
}

#[derive(Debug, Display, Error)]
pub enum ConfigError {
    #[display("expected a table for [local.config]")]
    ExpectedTableForConfig,

    #[display("unknown configuration option: {path}")]
    UnknownConfigurationOption { path: String },

    #[display("{path}: unknown config object: {object_ref}")]
    UnknownConfigObject { path: String, object_ref: String },

    #[display("{path} is missing _tname field")]
    MissingTnameField { path: String },

    #[display("{path}: unknown type {type_name}")]
    UnknownType { path: String, type_name: String },

    #[display("{path} expected {expected}, got {got}")]
    TypeMismatch {
        path: String,
        expected: String,
        got: String,
    },

    #[display("expected {expected}, got {got}")]
    ExpectedGot { expected: String, got: String },
}

impl ConfigError {
    pub fn expected_table_for_config() -> Self {
        Self::ExpectedTableForConfig
    }

    pub fn unknown_configuration_option(path: String) -> Self {
        Self::UnknownConfigurationOption { path }
    }

    pub fn unknown_config_object(path: String, object_ref: String) -> Self {
        Self::UnknownConfigObject { path, object_ref }
    }

    pub fn missing_tname_field(path: String) -> Self {
        Self::MissingTnameField { path }
    }

    pub fn unknown_type(path: String, type_name: String) -> Self {
        Self::UnknownType { path, type_name }
    }

    pub fn type_mismatch(path: String, expected: String, got: String) -> Self {
        Self::TypeMismatch {
            path,
            expected,
            got,
        }
    }

    pub fn expected_got(expected: String, got: String) -> Self {
        Self::ExpectedGot { expected, got }
    }

    pub fn err_expected(
        expected: impl std::fmt::Display,
        got: &TomlValue,
        path: &[String],
    ) -> Self {
        let got_str = match got {
            TomlValue::String(s) => Cow::Owned(format!("\"{s}\"")),
            TomlValue::Integer(_) => "an integer".into(),
            TomlValue::Float(_) => "a float".into(),
            TomlValue::Boolean(_) => "a boolean".into(),
            TomlValue::Datetime(_) => "a datetime".into(),
            TomlValue::Array(_) => "an array".into(),
            TomlValue::Table(_) => "a table".into(),
        };
        Self::type_mismatch(path.join("."), expected.to_string(), got_str.to_string())
    }
}

/// Copied from edgeql-parser
fn quote_string(s: &str) -> String {
    use std::fmt::Write;

    let mut buf = String::with_capacity(s.len() + 2);
    buf.push('"');
    for c in s.chars() {
        match c {
            '"' => {
                buf.push('\\');
                buf.push('"');
            }
            '\\' => {
                buf.push('\\');
                buf.push('\\');
            }
            '\x00'..='\x08'
            | '\x0B'
            | '\x0C'
            | '\x0E'..='\x1F'
            | '\u{007F}'
            | '\u{0080}'..='\u{009F}' => {
                write!(buf, "\\x{:02x}", c as u32).unwrap();
            }
            c => buf.push(c),
        }
    }
    buf.push('"');
    buf
}
