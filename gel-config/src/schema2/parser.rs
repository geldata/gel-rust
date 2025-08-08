use indexmap::IndexMap;

use crate::schema2::{
    ops::{self, SchemaValue},
    structure::{ConfigDomain, ConfigDomainName, ConfigDomains, ConfigPropertyType, ConfigTable},
};

pub fn parse_toml(domains: &ConfigDomains, toml: &toml::Table) -> ops::SchemaOps {
    let mut ops = ops::SchemaOps::default();

    for (key, value) in toml {
        let config_domain = match key.as_str() {
            "branch" => ConfigDomainName::DatabaseBranch,
            "instance" => ConfigDomainName::Instance,
            _ => panic!("Unexpected domain: {}", key),
        };

        let schema = domains.domains.get(&config_domain).unwrap();

        for (key, value) in value.as_table().unwrap() {
            if key != "config" {
                panic!("Unexpected key: {}", key);
            }

            for (key, value) in value.as_table().unwrap() {
                let tables = schema.get_tables_by_path_or_name(key);
                if (value.is_array() || value.is_table()) && !tables.is_empty() {
                    if tables.is_empty() {
                        panic!("No tables found for path or name: {}", key);
                    }

                    if value.is_array() && !tables[0].multi {
                        panic!("Expected a table, got an array: {}", key);
                    }

                    if value.is_table() && tables[0].multi {
                        panic!("Expected an array, got a table: {}", key);
                    }

                    let toml_tables = if let Some(tables) = value.as_array() {
                        tables.iter().map(|v| v.as_table().unwrap()).collect()
                    } else if let Some(table) = value.as_table() {
                        vec![table]
                    } else {
                        panic!("Expected a table or array, got a value: {}", key);
                    };

                    for toml_table in toml_tables {
                        let tname = toml_table.get("_tname");

                        let type_name = if tables.len() > 1 {
                            let mut iter = tables.iter();
                            loop {
                                let Some(table) = iter.next() else {
                                    panic!("No table found for path or name: {}", key);
                                };
                                if &table.name == key {
                                    break table.name.clone();
                                }
                            }
                        } else {
                            if let Some(tname) = tname {
                                if tname.as_str().unwrap() != tables[0].name {
                                    panic!(
                                        "Expected _tname to be {}, got {}",
                                        tables[0].name,
                                        tname.as_str().unwrap()
                                    );
                                }
                            }
                            tables[0].name.clone()
                        };

                        let mut properties = IndexMap::new();
                        for (key, value) in toml_table {
                            properties
                                .insert(key.to_string(), parse_property(tables[0], key, value));
                        }

                        ops.insert
                            .entry(tables[0].path.clone().unwrap())
                            .or_default()
                            .push(ops::SchemaInsert {
                                type_name,
                                properties,
                            });
                    }
                } else {
                    let table = schema.get_root_table();
                    ops.set.push(parse_property(table, key, value));
                }
            }
        }
    }

    return ops;
}

pub fn parse_property(
    table: &ConfigTable,
    key: &str,
    value: &toml::Value,
) -> ops::SchemaNamedValue {
    let property = table
        .get_property(key)
        .expect(&format!("No property found for key: {}", key));
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
                    .map(|v| v.as_str().unwrap().to_owned())
                    .collect(),
            ),
        ),
        _ => {
            panic!(
                "Expected a string, got a value: {:#?} for key: {} and property type: {:?}",
                value, key, property.property_type
            );
        }
    };

    ops::SchemaNamedValue {
        name: key.to_string(),
        property_type,
        value,
    }
}
