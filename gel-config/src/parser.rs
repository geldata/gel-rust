use std::collections::BTreeMap;

use indexmap::IndexMap;

use crate::{
    ConfigSchemaPrimitiveType,
    ops::{self, AllSchemaOps, SchemaOps, SchemaValue},
    structure::{
        ConfigDomainName, ConfigDomains, ConfigPropertyType, ConfigRequirement, ConfigTable,
    },
};

#[derive(Debug, Clone, derive_more::Display, Eq, PartialEq)]
/// Errors that can occur when parsing TOML configuration files into database operations.
pub enum ParserError {
    /// The schema must provide domains if those are used in the toml file.
    #[display("Domain {_0} not found in schema")]
    DomainNotFound(String),
    #[display("Unexpected key: {_0}")]
    UnexpectedKey(String),
    #[display("No tables found for path or name: {_0}")]
    NoTablesFound(String),
    #[display("Expected a table, got an array: {_0}")]
    ExpectedTableGotArray(String),
    #[display("Expected an array, got a table: {_0} for {_1}")]
    ExpectedArrayGotTable(String, String),
    #[display("Expected a table or array, got a value: {_0}")]
    ExpectedTableOrArray(String),
    #[display("Expected _tname to be {_0}, got {_1}")]
    UnexpectedTypeName(String, String),
    #[display("No property found for key: {_0} in table {_1}")]
    PropertyNotFound(String, String),
    #[display("Invalid value type for key {_0} with property type {_1:?}")]
    InvalidValueType(String, String),
    #[display("Invalid enum value {_1} for key {_0} with property type {_2:?}")]
    InvalidEnumValue(String, String, String),
    #[display("Invalid _tname for key {_0}: {_1:?}")]
    InvalidTname(String, String),
    #[display("Protected property {_0} cannot be set")]
    ProtectedProperty(String),
}

impl std::error::Error for ParserError {}

#[derive(Debug, Clone, derive_more::Display, Eq, PartialEq, Ord, PartialOrd)]
/// Non-fatal warnings that can occur during TOML configuration parsing.
pub enum ParserWarning {
    #[display("Coercing value for {_0} to {_1}")]
    CoercingValue(String, String),

    #[display("Unexpected key: {_0}")]
    UnexpectedKey(String),

    #[display("Ignoring internal property {_0}")]
    InternalProperty(String),
}

pub fn parse_toml(
    domains: &ConfigDomains,
    toml: &toml::Table,
) -> Result<(AllSchemaOps, Vec<ParserWarning>), ParserError> {
    let mut by_domain = BTreeMap::new();
    let mut warnings = Vec::new();

    for (key, value) in toml {
        let config_domain = match key.as_str() {
            "branch" => ConfigDomainName::DatabaseBranch,
            "instance" => ConfigDomainName::Instance,
            _ => {
                warnings.push(ParserWarning::UnexpectedKey(key.clone()));
                continue;
            }
        };

        let Some(schema) = domains.domains.get(&config_domain) else {
            return Err(ParserError::DomainNotFound(key.clone()));
        };

        let table = value
            .as_table()
            .ok_or_else(|| ParserError::ExpectedTableOrArray(key.clone()))?;

        by_domain.insert(
            config_domain,
            apply(table, schema, &mut warnings, key.clone())?,
        );
    }

    warnings.sort();

    Ok((AllSchemaOps { by_domain }, warnings))
}

fn apply(
    table: &toml::Table,
    schema: &super::structure::ConfigDomain,
    warnings: &mut Vec<ParserWarning>,
    path: String,
) -> Result<SchemaOps, ParserError> {
    let mut ops = SchemaOps::default();
    for (key, value) in table {
        let path = format!("{path}.{key}");
        if key != "config" {
            warnings.push(ParserWarning::UnexpectedKey(path));
            continue;
        }

        let Some(table) = value.as_table() else {
            return Err(ParserError::ExpectedTableOrArray(path.clone()));
        };
        ops = apply_config(schema, warnings, path, table)?;
    }
    Ok(ops)
}

fn apply_config(
    schema: &super::structure::ConfigDomain,
    warnings: &mut Vec<ParserWarning>,
    path: String,
    value: &toml::Table,
) -> Result<SchemaOps, ParserError> {
    let mut ops = SchemaOps::default();

    // First, apply all the set operations and queue the insert operations by
    // path for a second phase. Compute the effective tname for each table during the first phase as well.

    let mut by_path = BTreeMap::<String, Vec<(String, String, &toml::Table)>>::new();

    for (key, value) in value {
        let path = format!("{path}.{key}");

        if key == "cfg::Config" {
            let table = schema.get_root_table();
            let Some(toml_table) = value.as_table() else {
                return Err(ParserError::ExpectedTableOrArray(key.clone()));
            };
            let properties = parse_properties(warnings, path.clone(), toml_table, table)?;
            for (_, property) in properties {
                ops.set.push(property);
            }
            continue;
        }
        let tables = schema.get_tables_by_path_or_name(key);

        if (value.is_array() || value.is_table()) && !tables.is_empty() {
            if value.is_array() && !tables[0].multi {
                return Err(ParserError::ExpectedTableGotArray(path.clone()));
            }

            if value.is_table() && tables[0].multi {
                return Err(ParserError::ExpectedArrayGotTable(
                    path.clone(),
                    tables[0].name.clone(),
                ));
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
                                ConfigSchemaPrimitiveType::Str.to_string(),
                            )
                        })?;
                        if tname_str != tables[0].name {
                            return Err(ParserError::UnexpectedTypeName(
                                tables[0].name.clone(),
                                tname_str.to_string(),
                            ));
                        }
                    } else if tables[0].path_requires_tname && key != &tables[0].name {
                        return Err(ParserError::InvalidTname(path.clone(), "".to_string()));
                    }
                    tables[0].name.clone()
                };

                if tables[0].singleton {
                    for (key, value) in toml_table {
                        if key != "_tname" {
                            if let Some((mut property, _)) = parse_property(
                                warnings,
                                tables[0],
                                format!("{path}.{key}"),
                                key,
                                value,
                            )? {
                                property.name = format!("{}::{}", tables[0].name, property.name);
                                ops.set.push(property);
                            }
                        }
                    }
                } else {
                    by_path
                        .entry(tables[0].path.clone().expect("Table path should exist"))
                        .or_default()
                        .push((type_name.clone(), path.clone(), toml_table));
                }
            }
        } else {
            let table = schema.get_root_table();
            if let Some((property, _)) = parse_property(warnings, table, path.clone(), key, value)?
            {
                ops.set.push(property);
            }
        }
    }

    for (path, tables) in by_path {
        let mut entries = vec![];
        for (type_name, orig_path, toml_table) in tables {
            let table = schema
                .get_table(&type_name)
                .expect("Table should exist in schema");

            let properties = parse_properties(warnings, orig_path.clone(), toml_table, table)?;
            entries.push(ops::SchemaInsert {
                type_name,
                properties,
            });
        }
        ops.insert.insert(path, entries);
    }

    Ok(ops)
}

fn parse_properties(
    warnings: &mut Vec<ParserWarning>,
    path: String,
    toml_table: &toml::map::Map<String, toml::Value>,
    table: &ConfigTable,
) -> Result<IndexMap<String, ops::SchemaNamedValue>, ParserError> {
    let mut properties_with_index = Vec::new();
    for (key, value) in toml_table {
        if key != "_tname" {
            if let Some((property, index)) =
                parse_property(warnings, table, format!("{path}.{key}"), key, value)?
            {
                properties_with_index.push((key.to_string(), property, index));
            }
        }
    }

    // Sort by the property index from the ConfigTable
    properties_with_index.sort_by_key(|(_, _, index)| *index);

    // Convert back to IndexMap
    let mut properties = IndexMap::new();
    for (key, property, _) in properties_with_index {
        properties.insert(key, property);
    }
    Ok(properties)
}

pub fn parse_property(
    warnings: &mut Vec<ParserWarning>,
    table: &ConfigTable,
    path: String,
    key: &str,
    value: &toml::Value,
) -> Result<Option<(ops::SchemaNamedValue, usize)>, ParserError> {
    let property = table
        .get_property(key)
        .ok_or_else(|| ParserError::PropertyNotFound(key.to_string(), table.name.clone()))?;

    // Find the index of this property in the table's properties list
    let property_index = table
        .properties
        .iter()
        .position(|p| p.name == key)
        .unwrap_or(usize::MAX);

    let (property_type, value) = match (value, &property.property_type) {
        (toml::Value::String(value), ConfigPropertyType::Primitive(primitive_type)) => {
            if primitive_type.is_bool() || primitive_type.is_int() || primitive_type.is_float() {
                warnings.push(ParserWarning::CoercingValue(
                    path.clone(),
                    primitive_type.to_schema_type().name,
                ));
            }
            (
                primitive_type.to_schema_type().name,
                SchemaValue::Unitary(ops::SchemaPrimitive::String(value.clone())),
            )
        }
        (toml::Value::String(value), ConfigPropertyType::Enum(name, values)) => {
            if !values.contains(value) {
                return Err(ParserError::InvalidEnumValue(
                    path.clone(),
                    value.clone(),
                    property.property_type.describe(),
                ));
            }
            (
                name.clone(),
                SchemaValue::Unitary(ops::SchemaPrimitive::String(value.clone())),
            )
        }
        (toml::Value::Integer(value), ConfigPropertyType::Primitive(primitive_type)) => {
            if !primitive_type.is_int() && !primitive_type.is_float() {
                if !primitive_type.is_str() {
                    return Err(ParserError::InvalidValueType(
                        path.clone(),
                        property.property_type.describe(),
                    ));
                }
                warnings.push(ParserWarning::CoercingValue(
                    path.clone(),
                    primitive_type.to_schema_type().name,
                ));
            }
            (
                primitive_type.to_schema_type().name,
                SchemaValue::Unitary(ops::SchemaPrimitive::Integer(*value as isize)),
            )
        }
        (toml::Value::Float(value), ConfigPropertyType::Primitive(primitive_type)) => (
            primitive_type.to_schema_type().name,
            SchemaValue::Unitary(ops::SchemaPrimitive::String(value.to_string())),
        ),
        (toml::Value::Boolean(value), ConfigPropertyType::Primitive(primitive_type)) => (
            primitive_type.to_schema_type().name,
            SchemaValue::Unitary(ops::SchemaPrimitive::Bool(*value)),
        ),
        (toml::Value::Array(value), ConfigPropertyType::Array(array_type)) => (
            array_type.name().unwrap(),
            SchemaValue::Array(
                value
                    .iter()
                    .map(|v| {
                        v.as_str()
                            .ok_or_else(|| {
                                ParserError::InvalidValueType(
                                    key.to_string(),
                                    array_type.describe(),
                                )
                            })
                            .map(|s| ops::SchemaPrimitive::String(s.to_owned()))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            ),
        ),
        (toml::Value::Table(value), ConfigPropertyType::Object(object_type)) => {
            let tname = value.get("_tname");
            let type_name = if let Some(tname) = tname {
                tname
                    .as_str()
                    .ok_or_else(|| ParserError::InvalidTname(key.to_string(), tname.to_string()))?
            } else {
                return Err(ParserError::InvalidTname(key.to_string(), "".to_string()));
            };

            let actual_type = object_type
                .get(type_name)
                .ok_or_else(|| ParserError::InvalidTname(key.to_string(), type_name.to_string()))?;
            let properties = parse_properties(warnings, path.clone(), value, actual_type)?;
            (
                type_name.to_string(),
                SchemaValue::Object(type_name.to_string(), properties),
            )
        }
        _ => {
            return Err(ParserError::InvalidValueType(
                path.to_string(),
                property.property_type.describe(),
            ));
        }
    };

    if property.is_internal {
        warnings.push(ParserWarning::InternalProperty(path.to_string()));
        return Ok(None);
    }

    if property.requirement == ConfigRequirement::Protected {
        return Err(ParserError::ProtectedProperty(path.to_string()));
    }

    Ok(Some((
        ops::SchemaNamedValue {
            name: key.to_string(),
            property_type,
            value,
        },
        property_index,
    )))
}
