use std::fmt::Debug;

use indexmap::IndexMap;

use toml::Value as TomlValue;

use super::schema::{ObjectType, Schema, Type};
use crate::{quote_string as ql, ConfigError, HintExt, PrimitiveType, Value};

pub fn validate(value: TomlValue, schema: &Schema) -> Result<Commands, ConfigError> {
    let mut validator = Validator {
        commands: Commands::default(),
        schema,
        path: Vec::new(),
    };

    validator.validate_top_level(value)?;

    Ok(validator.commands)
}

pub struct ConfigureSet {
    pub object_name: String,
    pub extension_name: Option<String>,
    pub property_name: String,
    pub value: Value,
}

impl Debug for ConfigureSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(extension_name) = &self.extension_name {
            write!(f, "configure {extension_name}")?;
        } else {
            write!(f, "configure")?;
        }
        write!(
            f,
            " {} set {} = {:?}",
            self.object_name, self.property_name, self.value
        )
    }
}

#[derive(Debug)]
pub struct ConfigureInsert {
    pub extension_name: Option<String>,
    pub values: Vec<IndexMap<String, Value>>,
}

#[derive(Default)]
pub struct Commands {
    pub set: Vec<ConfigureSet>,
    pub insert: IndexMap<String, ConfigureInsert>,
}

impl Debug for Commands {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for set in &self.set {
            writeln!(f, "{set:?};")?;
        }
        for (object_name, insert) in &self.insert {
            for value in &insert.values {
                if let Some(extension_name) = &insert.extension_name {
                    write!(f, "insert {extension_name}")?;
                } else {
                    write!(f, "insert")?;
                }
                writeln!(f, " {object_name} {{")?;
                for (key, value) in value {
                    let mut value = format!("{value:?}");
                    value = value.replace("\n", "\n  ");
                    writeln!(f, "  {key}: {value},")?;
                }
                writeln!(f, "}};")?;
            }
        }
        Ok(())
    }
}

impl Commands {
    pub fn set(
        &mut self,
        config_object: &str,
        extension_name: Option<&str>,
        values: IndexMap<String, Value>,
    ) {
        for (property_name, value) in values {
            let object_name = config_object.to_string();
            let extension_name = extension_name.map(|name| name.to_string());
            self.set.push(ConfigureSet {
                object_name,
                extension_name,
                property_name,
                value,
            });
        }
    }
    pub fn insert(
        &mut self,
        config_object: String,
        extension_name: Option<&str>,
        values: IndexMap<String, Value>,
    ) {
        let inserts = self.insert.entry(config_object).or_insert_with(|| {
            let extension_name = extension_name.map(|name| name.to_string());
            ConfigureInsert {
                extension_name,
                values: Vec::new(),
            }
        });
        inserts.values.push(values);
    }
    pub fn is_empty(&self) -> bool {
        self.set.is_empty() && self.insert.is_empty()
    }
}

struct Validator<'s> {
    commands: Commands,
    schema: &'s Schema,
    path: Vec<String>,
}

impl Validator<'_> {
    /// Entry point
    fn validate_top_level(&mut self, value: TomlValue) -> Result<(), ConfigError> {
        let TomlValue::Table(entries) = value else {
            return Err(ConfigError::expected_table_for_config());
        };

        // validate entries like `cfg::Config` and `ext::auth::AuthConfig```
        let mut not_found = toml::map::Map::new();
        for (cfg_object, value) in entries {
            let Some((ext_name, object_type)) = self.schema.find_object(&cfg_object) else {
                not_found.insert(cfg_object, value);
                continue;
            };

            let toml_values = if object_type.is_multi {
                let TomlValue::Array(values) = value else {
                    return Err(self.err_expected("an array", &value));
                };
                values
            } else {
                vec![value]
            };

            self.path.push(cfg_object.clone());
            for v in toml_values {
                let values = self.validate_object_type(v, object_type)?;
                if object_type.is_top_level {
                    self.commands.set(&cfg_object, ext_name, values);
                } else {
                    self.commands.insert(cfg_object.clone(), ext_name, values);
                }
            }

            self.path.pop();
        }

        // validate entries like `allow_bare_ddl`, which we implicitly assume are on cfg::Config
        let (ext_name, cfg_config) = self.schema.find_object("cfg::Config").unwrap();
        let values = self.validate_object_type(TomlValue::Table(not_found), cfg_config)?;
        self.commands.set("cfg::Config", ext_name, values);

        Ok(())
    }

    fn validate_object_type(
        &mut self,
        value: TomlValue,
        obj: &ObjectType,
    ) -> Result<IndexMap<String, Value>, ConfigError> {
        let TomlValue::Table(entries) = value else {
            return Err(self.err_expected("a table", &value));
        };

        let mut properties = IndexMap::new();

        for (key, value) in entries {
            self.path.push(key.clone());

            let Some(ptr) = obj.pointers.get(&key) else {
                return Err(ConfigError::unknown_configuration_option(
                    self.path.join("."),
                ));
            };

            if ptr.target.is_scalar() {
                // properties

                if !ptr.is_multi {
                    let value = self.validate_property(value, &ptr.target)?;
                    properties.insert(key, value);
                } else {
                    let TomlValue::Array(array) = value else {
                        return Err(self.err_expected("an array", &value));
                    };
                    let values = array
                        .into_iter()
                        .map(|v| self.validate_property(v, &ptr.target))
                        .collect::<Result<Vec<_>, _>>()?;
                    properties.insert(key, Value::Set(values));
                }
            } else {
                // links

                if !ptr.is_multi {
                    let value = self.validate_link(value, &ptr.target)?;
                    if let Some(value) = value {
                        properties.insert(key, value);
                    }
                } else {
                    let TomlValue::Array(array) = value else {
                        return Err(self.err_expected("an array", &value));
                    };
                    let mut set = Vec::new();
                    for v in array {
                        if let Some(v) = self.validate_link(v, &ptr.target)? {
                            set.push(v);
                        }
                    }
                    if !set.is_empty() {
                        properties.insert(key, Value::Set(set));
                    }
                }
            }

            self.path.pop();
        }

        Ok(properties)
    }

    fn validate_property(&mut self, value: TomlValue, typ: &Type) -> Result<Value, ConfigError> {
        use TomlValue as Toml;
        use Type::*;

        let as_injected = value
            .as_str()
            .and_then(|v| v.strip_prefix("{{"))
            .and_then(|v| v.strip_suffix("}}"));
        if let Some(injected) = as_injected {
            return Ok(Value::Injected(injected.to_string()));
        }

        Ok(match (typ, value) {
            (Primitive(prim), Toml::Array(v)) => {
                return Err(self.err_expected(prim, &Toml::Array(v)));
            }
            (Primitive(prim), Toml::Table(v)) => {
                return Err(self.err_expected(prim, &Toml::Table(v)));
            }
            (Enum { name, .. }, Toml::Array(v)) => {
                return Err(self.err_expected(name, &Toml::Array(v)));
            }
            (Enum { name, .. }, Toml::Table(v)) => {
                return Err(self.err_expected(name, &Toml::Table(v)));
            }
            (Primitive(prim), value) => match (prim, value) {
                (PrimitiveType::String, Toml::String(value)) => Value::Injected(ql(&value)),
                (PrimitiveType::Int64, Toml::Integer(value)) => Value::Injected(value.to_string()),
                (PrimitiveType::Int32, Toml::Integer(value)) => Value::Injected(value.to_string()),
                (PrimitiveType::Int16, Toml::Integer(value)) => Value::Injected(value.to_string()),
                (PrimitiveType::Float64, Toml::Float(value)) => Value::Injected(value.to_string()),
                (PrimitiveType::Float32, Toml::Float(value)) => Value::Injected(value.to_string()),
                (PrimitiveType::Boolean, Toml::Boolean(value)) => {
                    Value::Injected(value.to_string())
                }
                (PrimitiveType::Duration, Toml::String(value)) => {
                    Value::Injected(format!("<duration>{}", ql(&value)))
                }
                (prim, value) => {
                    return Err(self.err_expected(prim, &value));
                }
            },
            (Enum { name, choices }, Toml::String(value)) => {
                if !choices.contains(&value) {
                    return Err(
                        self.err_expected(format!("one of {choices:?}"), &Toml::String(value))
                    );
                }
                Value::Injected(format!("<{}>{}", name, ql(&value)))
            }
            (typ, value) => {
                return Err(self.err_expected(typ, &value));
            }
        })
    }

    fn validate_link(
        &mut self,
        value: TomlValue,
        typ: &Type,
    ) -> Result<Option<Value>, ConfigError> {
        use TomlValue as Toml;
        use Type::*;

        match (typ, value) {
            (ObjectRef(target_ref), Toml::Table(value)) => {
                let Some((ext_name, target)) = self.schema.find_object(target_ref) else {
                    return Err(ConfigError::unknown_config_object(
                        self.path.join("."),
                        target_ref.clone(),
                    ));
                };

                let values = self.validate_object_type(Toml::Table(value), target)?;
                Ok(if target.is_non_locatable {
                    Some(Value::Insert {
                        typ: target_ref.clone(),
                        values,
                    })
                } else {
                    self.commands.insert(target_ref.clone(), ext_name, values);
                    None
                })
            }
            (Union(obj_type_refs), Toml::Table(mut value)) => {
                let Some(Toml::String(t_name)) = value.remove("_tname") else {
                    return Err(
                        ConfigError::missing_tname_field(self.path.join("."))
                            .with_hint(|| format!(
                                "{} can be any of the following types: {}. Use _tname to differenciate between them.",
                                self.path.last().unwrap(),
                                obj_type_refs.join(", ")
                            ))
                    );
                };
                let Some(obj_type_ref) = obj_type_refs.iter().find(|r| r == &&t_name) else {
                    return Err(ConfigError::unknown_type(
                        self.path.join("."),
                        t_name.clone(),
                    ));
                };

                let Some((ext_name, target)) = self.schema.find_object(obj_type_ref) else {
                    return Err(ConfigError::unknown_config_object(
                        self.path.join("."),
                        obj_type_ref.clone(),
                    ));
                };
                let values = self.validate_object_type(Toml::Table(value), target)?;
                Ok(if target.is_non_locatable {
                    Some(Value::Insert {
                        typ: obj_type_ref.clone(),
                        values,
                    })
                } else {
                    self.commands.insert(obj_type_ref.clone(), ext_name, values);
                    None
                })
            }
            (typ, value) => Err(self.err_expected(typ, &value)),
        }
    }

    fn err_expected(&self, expected: impl std::fmt::Display, got: &TomlValue) -> ConfigError {
        ConfigError::err_expected(expected, got, &self.path)
    }
}
