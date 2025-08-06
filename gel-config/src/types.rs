use std::{collections::HashMap, time::Duration};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConfigInput {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    DateTime(String),
    Duration(Duration),
    Date(String),
    Time(String),
    Array(Vec<ConfigInput>),
    Object {
        tname: String,
        values: HashMap<String, ConfigInput>,
    },
    Null,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigSchemaType {
    String,
    Integer,
    Float,
    Boolean,
    DateTime,
    Duration,
    Date,
    Time,
    Array(Box<ConfigSchemaType>),
    Object(HashMap<String, ConfigSchemaType>),
    Enum(Vec<ConfigObject>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigObject {
    pub name: String,
    pub schema: HashMap<String, ConfigSchemaType>,
}
