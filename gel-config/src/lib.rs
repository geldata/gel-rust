pub mod schema2;

use derive_more::{Display, Error};
use indexmap::IndexMap;
use std::{borrow::Cow, fmt::Debug};
use toml::Value as TomlValue;

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
