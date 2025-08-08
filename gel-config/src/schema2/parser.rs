use std::collections::BTreeMap;

use indexmap::IndexMap;

use crate::schema2::{
    ops::{self, SchemaOps, SchemaValue},
    structure::{ConfigDomainName, ConfigDomains, ConfigPropertyType, ConfigTable},
};

#[derive(Debug, Clone, derive_more::Display)]
pub enum ParserError {
    #[display("Unexpected domain: {_0}")]
    UnexpectedDomain(String),
    #[display("Unexpected key: {_0}")]
    UnexpectedKey(String),
    #[display("No tables found for path or name: {_0}")]
    NoTablesFound(String),
    #[display("Expected a table, got an array: {_0}")]
    ExpectedTableGotArray(String),
    #[display("Expected an array, got a table: {_0}")]
    ExpectedArrayGotTable(String),
    #[display("Expected a table or array, got a value: {_0}")]
    ExpectedTableOrArray(String),
    #[display("Expected _tname to be {_0}, got {_1}")]
    UnexpectedTypeName(String, String),
    #[display("No property found for key: {_0}")]
    PropertyNotFound(String),
    #[display("Invalid value type for key {_0} with property type {_1:?}")]
    InvalidValueType(String, ConfigPropertyType),
    #[display("Invalid _tname for key {_0}")]
    InvalidTname(String),
}

impl std::error::Error for ParserError {}

pub fn parse_toml(
    domains: &ConfigDomains,
    toml: &toml::Table,
) -> Result<BTreeMap<ConfigDomainName, ops::SchemaOps>, ParserError> {
    let mut ops = BTreeMap::new();

    for (key, value) in toml {
        let config_domain = match key.as_str() {
            "branch" => ConfigDomainName::DatabaseBranch,
            "instance" => ConfigDomainName::Instance,
            _ => return Err(ParserError::UnexpectedDomain(key.clone())),
        };

        let schema = domains
            .domains
            .get(&config_domain)
            .expect("Domain should exist in schema"); // This is a schema invariant

        let table = value.as_table().ok_or_else(|| {
            ParserError::InvalidValueType(
                key.clone(),
                ConfigPropertyType::Object(Default::default()),
            )
        })?;

        ops.insert(config_domain, apply(table, schema)?);
    }

    Ok(ops)
}

fn apply(
    table: &toml::Table,
    schema: &super::structure::ConfigDomain,
) -> Result<SchemaOps, ParserError> {
    let mut ops = SchemaOps::default();
    for (key, value) in table {
        if key != "config" {
            return Err(ParserError::UnexpectedKey(key.clone()));
        }

        ops = apply_config(schema, key, value)?;
    }
    Ok(ops)
}

fn apply_config(schema: &super::structure::ConfigDomain, key: &String, value: &toml::Value) -> Result<SchemaOps, ParserError> {
    let mut ops = SchemaOps::default();

    // First, apply all the set operations and queue the insert operations by
    // path for a second phase. Compute the effective tname for each table during the first phase as well.

    let mut by_path = BTreeMap::<String, Vec<(String, &toml::Table)>>::new();

    for (key, value) in value.as_table().ok_or_else(|| {
        ParserError::InvalidValueType(
            key.clone(),
            ConfigPropertyType::Object(Default::default()),
        )
    })? {
        let tables = schema.get_tables_by_path_or_name(key);
        if (value.is_array() || value.is_table()) && !tables.is_empty() {
            if value.is_array() && !tables[0].multi {
                return Err(ParserError::ExpectedTableGotArray(key.clone()));
            }

            if value.is_table() && tables[0].multi {
                return Err(ParserError::ExpectedArrayGotTable(key.clone()));
            }

            let toml_tables = if let Some(tables) = value.as_array() {
                tables
                    .iter()
                    .map(|v| {
                        v.as_table()
                            .ok_or_else(|| ParserError::ExpectedTableOrArray(key.clone()))
                    })
                    .collect::<Result<Vec<_>, _>>()?
            } else if let Some(table) = value.as_table() {
                vec![table]
            } else {
                unreachable!();
            };

            for toml_table in toml_tables {
                let tname = toml_table.get("_tname");

                let type_name = if tables.len() > 1 {
                    let mut iter = tables.iter();
                    loop {
                        let Some(table) = iter.next() else {
                            return Err(ParserError::NoTablesFound(key.clone()));
                        };
                        if &table.name == key {
                            break table.name.clone();
                        }
                    }
                } else {
                    if let Some(tname) = tname {
                        let tname_str = tname.as_str().ok_or_else(|| {
                            ParserError::InvalidValueType(
                                "_tname".to_string(),
                                ConfigPropertyType::Primitive(
                                    tables[0]
                                        .get_property("_tname")
                                        .map(|p| match &p.property_type {
                                            ConfigPropertyType::Primitive(t) => t.clone(),
                                            _ => unreachable!(),
                                        })
                                        .unwrap_or_else(|| unreachable!()),
                                ),
                            )
                        })?;
                        if tname_str != tables[0].name {
                            return Err(ParserError::UnexpectedTypeName(
                                tables[0].name.clone(),
                                tname_str.to_string(),
                            ));
                        }
                    }
                    tables[0].name.clone()
                };

                by_path.entry(tables[0].path.clone().expect("Table path should exist")).or_default().push((type_name.clone(), toml_table));
            }
        } else {
            let table = schema.get_root_table();
            ops.set.push(parse_property(table, key, value)?);
        }
    }

    for (path, tables) in by_path {
        let mut entries = vec![];
        for (type_name, toml_table) in tables {
            let mut properties = IndexMap::new();
            let table = schema.get_table(&type_name).expect("Table should exist in schema");
            for (key, value) in toml_table {
                if key != "_tname" {
                    properties.insert(key.to_string(), parse_property(table, key, value)?);
                }
            }
            entries.push(ops::SchemaInsert {
                type_name,
                properties,
            });
        }
        ops.insert.insert(path, entries);
    }

    Ok(ops)
}

pub fn parse_property(
    table: &ConfigTable,
    key: &str,
    value: &toml::Value,
) -> Result<ops::SchemaNamedValue, ParserError> {
    let property = table
        .get_property(key)
        .ok_or_else(|| ParserError::PropertyNotFound(key.to_string()))?;

    let (property_type, value) = match (value, &property.property_type) {
        (toml::Value::String(value), ConfigPropertyType::Primitive(primitive_type)) => (
            primitive_type.to_schema_type().name,
            SchemaValue::Unitary(value.clone()),
        ),
        (toml::Value::String(value), ConfigPropertyType::Enum(name, _)) => {
            // TODO: check enum values
            (name.clone(), SchemaValue::Unitary(value.clone()))
        }
        (toml::Value::Integer(value), ConfigPropertyType::Primitive(primitive_type)) => (
            primitive_type.to_schema_type().name,
            SchemaValue::Unitary(value.to_string()),
        ),
        (toml::Value::Float(value), ConfigPropertyType::Primitive(primitive_type)) => (
            primitive_type.to_schema_type().name,
            SchemaValue::Unitary(value.to_string()),
        ),
        (toml::Value::Boolean(value), ConfigPropertyType::Primitive(primitive_type)) => (
            primitive_type.to_schema_type().name,
            SchemaValue::Unitary(value.to_string()),
        ),
        (toml::Value::Array(value), ConfigPropertyType::Array(array_type)) => (
            array_type.name().unwrap(),
            SchemaValue::Array(
                value
                    .iter()
                    .map(|v| {
                        v.as_str()
                            .ok_or_else(|| {
                                ParserError::InvalidValueType(key.to_string(), *array_type.clone())
                            })
                            .map(|s| s.to_owned())
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            ),
        ),
        (toml::Value::Table(value), ConfigPropertyType::Object(object_type)) => {
            let tname = value.get("_tname");
            let type_name = if let Some(tname) = tname {
                tname.as_str().ok_or_else(|| ParserError::InvalidTname(key.to_string()))?
            } else {
                return Err(ParserError::InvalidTname(key.to_string()));
            };

            let actual_type = object_type.get(type_name).ok_or_else(|| ParserError::InvalidTname(key.to_string()))?;
            let mut properties = IndexMap::new();
            for (key, value) in value {
                if key != "_tname" {
                    properties.insert(key.to_string(), parse_property(actual_type, key, value)?);
                }
            }
            (type_name.to_string(), SchemaValue::Object(properties))
        }
        _ => {
            return Err(ParserError::InvalidValueType(
                key.to_string(),
                property.property_type.clone(),
            ));
        }
    };

    Ok(ops::SchemaNamedValue {
        name: key.to_string(),
        property_type,
        value,
    })
}
