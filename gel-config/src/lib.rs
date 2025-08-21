use std::str::FromStr;

use serde::{Deserialize, Serialize};

pub mod ops;
pub mod parser;
pub mod raw;
pub mod structure;

<<<<<<< HEAD
use derive_more::{Display, Error};
use indexmap::IndexMap;
use std::{borrow::Cow, fmt::Debug, str::FromStr};
use toml::Value as TomlValue;
=======
/// Retrieve the hard-coded current configuration schema without consulting the
/// database.
#[cfg(feature = "precomputed")]
pub fn current_schema() -> crate::raw::ConfigSchema {
    use std::io::Read;
    const SCHEMA: &[u8] = include_bytes!("schema.json.gz");
>>>>>>> 3c11a7c (gel-config evolution)

    let mut decoder = flate2::read::GzDecoder::new(SCHEMA);
    let mut data = Vec::new();
    decoder.read_to_end(&mut data).unwrap();
    serde_json::from_slice(&data).unwrap()
}

/// Retrieve the hard-coded current configuration.
#[cfg(feature = "precomputed")]
pub fn current_config() -> crate::structure::ConfigDomains {
    let schema = current_schema();

    crate::structure::from_raw(schema).unwrap()
}

/// The query to retrieve the current configuration schema.
pub fn schema_query() -> &'static str {
    include_str!("schema.edgeql")
}

#[derive(
    Clone, Serialize, Deserialize, PartialEq, Eq, Hash, derive_more::Display, derive_more::Debug,
)]
/// Represents the primitive data types supported in Gel configuration schemas.
pub enum ConfigSchemaPrimitiveType {
    #[display("std::str")]
    #[debug("std::str")]
    Str,
    #[display("std::bool")]
    #[debug("std::bool")]
    Bool,
    #[display("std::int16")]
    #[debug("std::int16")]
    Int16,
    #[display("std::int32")]
    #[debug("std::int32")]
    Int32,
    #[display("std::int64")]
    #[debug("std::int64")]
    Int64,
    #[display("std::float32")]
    #[debug("std::float32")]
    Float32,
    #[display("std::float64")]
    #[debug("std::float64")]
    Float64,
    #[display("std::bigint")]
    #[debug("std::bigint")]
    BigInt,
    #[display("std::decimal")]
    #[debug("std::decimal")]
    Decimal,
    #[display("std::uuid")]
    #[debug("std::uuid")]
    Uuid,
    #[display("std::duration")]
    #[debug("std::duration")]
    Duration,
    #[display("std::bytes")]
    #[debug("std::bytes")]
    Bytes,
    #[display("std::sequence")]
    #[debug("std::sequence")]
    Sequence,
    #[display("cfg::memory")]
    #[debug("cfg::memory")]
    Memory,
}

impl ConfigSchemaPrimitiveType {
    pub fn is_literal(&self) -> bool {
        matches!(
            self,
            Self::Str | Self::Bool | Self::Int16 | Self::Int32 | Self::Int64
        )
    }
}

<<<<<<< HEAD
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
=======
    pub fn is_str(&self) -> bool {
        matches!(self, Self::Str)
    }

    pub fn is_int(&self) -> bool {
        matches!(self, Self::Int16 | Self::Int32 | Self::Int64)
    }

    pub fn is_float(&self) -> bool {
        matches!(self, Self::Float32 | Self::Float64)
    }

    pub fn is_bool(&self) -> bool {
        matches!(self, Self::Bool)
    }
}

impl FromStr for ConfigSchemaPrimitiveType {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "std::str" => Ok(Self::Str),
            "std::bool" => Ok(Self::Bool),
            "std::int16" => Ok(Self::Int16),
            "std::int32" => Ok(Self::Int32),
            "std::int64" => Ok(Self::Int64),
            "std::float32" => Ok(Self::Float32),
            "std::float64" => Ok(Self::Float64),
            "std::bigint" => Ok(Self::BigInt),
            "std::decimal" => Ok(Self::Decimal),
            "std::uuid" => Ok(Self::Uuid),
            "std::duration" => Ok(Self::Duration),
            "std::bytes" => Ok(Self::Bytes),
            "std::sequence" => Ok(Self::Sequence),
            "cfg::memory" => Ok(Self::Memory),
>>>>>>> 3c11a7c (gel-config evolution)
            _ => Err(()),
        }
    }
}

#[cfg(test)]
#[cfg(feature = "precomputed")]
mod tests {
    use crate::{
        parser::parse_toml,
        raw::{
            ConfigSchema, ConfigSchemaLinkBuilder, ConfigSchemaObjectBuilder,
            ConfigSchemaPropertyBuilder, ConfigSchemaTypeReference,
        },
        structure::from_raw,
    };
    use pretty_assertions::assert_eq;

    use super::*;

    pub fn test_schema() -> ConfigSchema {
        let mut schema = ConfigSchema::new();
        schema.types.push(
            ConfigSchemaObjectBuilder::new()
                .with_name("TestType".to_string())
                .with_properties(vec![
                    ConfigSchemaPropertyBuilder::new()
                        .with_name("int".to_string())
                        .with_target(ConfigSchemaPrimitiveType::Int32)
                        .build(),
                ])
                .build(),
        );

        schema.types.push(
            ConfigSchemaObjectBuilder::new()
                .with_name("cfg::DatabaseConfig".to_string())
                .with_properties(vec![
                    ConfigSchemaPropertyBuilder::new()
                        .with_name("test_property_root".to_string())
                        .with_target(ConfigSchemaPrimitiveType::Str)
                        .build(),
                ])
                .with_links(vec![
                    ConfigSchemaLinkBuilder::new()
                        .with_name("root_link".to_string())
                        .with_target(ConfigSchemaTypeReference::new("MyRootType"))
                        .with_multi(true)
                        .build(),
                ])
                .build(),
        );

        schema.types.push(
            ConfigSchemaObjectBuilder::new()
                .with_name("MyRootType")
                .with_links(vec![
                    ConfigSchemaLinkBuilder::new()
                        .with_name("test_property".to_string())
                        .with_target(ConfigSchemaTypeReference::new("TestType"))
                        .build(),
                ])
                .build(),
        );

        schema
            .types
            .push(ConfigSchemaObjectBuilder::new().with_name("type1").build());

        schema.roots.push(ConfigSchemaTypeReference {
            name: "cfg::DatabaseConfig".to_string(),
        });

        schema
    }

    pub fn test_toml() -> toml::Table {
        r#"
[branch.config]
test_property_root = 'hello'

[[branch.config.MyRootType]]
test_property = { _tname = "TestType", int = 1 }
        "#
        .parse()
        .unwrap()
    }

    #[test]
    fn test_fully() {
        let schema = test_schema();
        let domains = from_raw(schema).unwrap();
        eprintln!("{domains:#?}");
        let toml = test_toml();
        eprintln!("{toml:#?}");
        let (ops, warnings) = parse_toml(&domains, &toml).unwrap();
        eprintln!("{}", ops.to_ddl());
        eprintln!("{warnings:#?}");
        assert_eq!(
            ops.to_ddl().trim(),
            r#"
configure current database set test_property_root := <std::str>'hello';
configure current database reset root_link;
configure current database insert MyRootType {
    test_property := (insert TestType {
        int := <std::int32>1
    })
};"#
            .trim()
        );
    }
}
<<<<<<< HEAD

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
=======
>>>>>>> 3c11a7c (gel-config evolution)
