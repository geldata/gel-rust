use std::{collections::BTreeMap, str::FromStr};

use crate::schema2::{
    raw::{ConfigSchema, ConfigSchemaObject},
    ConfigSchemaPrimitiveType,
};

#[derive(Debug, Clone, derive_more::Error, derive_more::Display)]
pub enum StructureError {
    #[display("Invalid primitive type: {_0} ({_1})")]
    InvalidPrimitiveType(String, String),
    #[display("No linked types found for {_0}::{_1} -> {_2}")]
    NoLinkedTypesFound(String, String, String),
    #[display("Protected type {_0}::{_1} missing default value")]
    ProtectedTypeMissingDefault(String, String),
}

#[derive(Debug, Clone)]
pub struct ConfigDomains {
    pub domains: BTreeMap<ConfigDomainName, ConfigDomain>,
}

#[derive(Debug, Clone)]
pub struct ConfigDomain {
    pub name: ConfigDomainName,
    /// Single-instance tables
    pub tables: BTreeMap<String, ConfigTable>,
    pub array_tables: BTreeMap<String, ConfigTable>,
}

#[derive(Clone)]
pub struct ConfigTable {
    pub name: String,
    pub properties: Vec<ConfigProperty>,
}

impl std::fmt::Debug for ConfigTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if f.alternate() {
            write!(f, "{{\n")?;
        } else {
            write!(f, "ConfigTable {{ name: {}, properties: [", self.name)?;
        }
        for (i, property) in self.properties.iter().enumerate() {
            let property_str = if f.alternate() {
                format!("{:?}: {:#?}", property.name, property.property_type)
            } else {
                format!("{:?}: {:?}", property.name, property.property_type)
            };
            let default_str = if let Some(default) = property.default.as_ref() {
                format!(" = {default:?}")
            } else {
                "".to_string()
            };
            if f.alternate() {
                if let Some(description) = property.description.as_ref() {
                    write!(
                        f,
                        "  # {}\n  {} {}{}",
                        description, property.requirement, property_str, default_str
                    )?;
                } else {
                    write!(
                        f,
                        "  {} {}{}",
                        property.requirement, property_str, default_str
                    )?;
                }
            } else {
                write!(
                    f,
                    " {} {}{}",
                    property.requirement, property_str, default_str
                )?;
            }
            if i < self.properties.len() - 1 {
                if f.alternate() {
                    write!(f, "\n")?;
                } else {
                    write!(f, ", ")?;
                }
            }
        }
        if f.alternate() {
            write!(f, "\n}}")?;
        } else {
            write!(f, "] }}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ConfigProperty {
    pub name: String,
    pub property_type: ConfigPropertyType,
    pub requirement: ConfigRequirement,
    pub default: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, derive_more::Display, PartialEq, Eq)]
pub enum ConfigRequirement {
    #[display("required")]
    Required,
    #[display("required readonly")]
    RequiredReadOnly,
    #[display("optional")]
    Optional,
    #[display("readonly")]
    ReadOnly,
    #[display("protected")]
    Protected,
}

#[derive(derive_more::Debug, Clone)]
pub enum ConfigPropertyType {
    #[debug("{_0:#?}")]
    /// A primitive type (string, int, float, etc.)
    Primitive(ConfigSchemaPrimitiveType),
    /// An enum type (string, with possible values)
    #[debug("{_0:?}: enum<{}>", _1.join(", "))]
    Enum(String, Vec<String>),
    /// An array of types
    #[debug("array<{_0:?}>")]
    Array(Box<ConfigPropertyType>),
    /// A set of possible object structures
    Object(BTreeMap<String, ConfigTable>),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConfigDomainName {
    Instance,
    DatabaseBranch,
    Session,
}

pub fn from_raw(schema: ConfigSchema) -> Result<ConfigDomains, StructureError> {
    let mut domains = BTreeMap::new();

    // Walk the list of properties starting at the root. If an object is linked
    // from an AbstractConfig or an ExtensionConfig subclass, we add it either
    // to tables or array_tables depending on the cardinality of the link. If the
    // object is an ExtensionConfig, we add it to the non-array tables, since
    // only one of each ExtensionConfig can exist. Add the root object itself to
    // the tables as well as "cfg::Config".

    // Process each root configuration type
    for root in schema.get_root_types() {
        let domain_name = match root.name.as_str() {
            "cfg::InstanceConfig" => ConfigDomainName::Instance,
            "cfg::DatabaseConfig" => ConfigDomainName::DatabaseBranch,
            "cfg::Config" => ConfigDomainName::Session,
            _ => continue, // Skip other root types
        };

        let mut domain = ConfigDomain {
            name: domain_name,
            tables: BTreeMap::new(),
            array_tables: BTreeMap::new(),
        };

        // Walk through all linked objects
        walk_object(&mut domain, &schema, root, Locatable::Root)?;

        domains.insert(domain_name, domain);
    }

    Ok(ConfigDomains { domains })
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Locatable {
    /// The object is not locatable, so we don't need to create a table for it.
    No,
    /// The object is a root object, so we need to create a table named cfg::Config for it.
    Root,
    /// The object is locatable and a singleton (ie: an extension config OR a
    /// singleton on an extension config), so we need to create a
    /// single-instance table for it.
    Single,
    /// The object is locatable and multi-instance, so we need to create a
    /// multi-instance table for it.
    Multi,
}

fn create_table(
    domain: &ConfigDomain,
    schema: &ConfigSchema,
    object: &ConfigSchemaObject,
    include_links: bool,
) -> Result<ConfigTable, StructureError> {
    let mut properties = vec![];
    'properties: for property in &object.properties {
        // Skip system properties in domains other than the instance domain
        if domain.name != ConfigDomainName::Instance {
            for annotation in &property.annotations {
                if annotation.name == "cfg::system" {
                    continue 'properties;
                }
            }
        }

        let default = property.default.clone();
        let mut description = None;

        for annotation in &property.annotations {
            if annotation.name == "cfg::internal" {
                continue 'properties;
            }
            if annotation.name == "std::description" {
                description = Some(annotation.value.clone());
            }
        }

        let mut property_type = if let Some(enum_values) = property.target.enum_values.clone() {
            ConfigPropertyType::Enum(property.target.name.clone(), enum_values)
        } else {
            ConfigPropertyType::Primitive(
                ConfigSchemaPrimitiveType::from_str(&property.target.name).map_err(|_| {
                    StructureError::InvalidPrimitiveType(
                        property.name.clone(),
                        property.target.name.clone(),
                    )
                })?,
            )
        };
        if property.multi {
            property_type = ConfigPropertyType::Array(Box::new(property_type));
        }
        let requirement = if property.protected {
            ConfigRequirement::Protected
        } else if property.readonly {
            if property.required {
                ConfigRequirement::RequiredReadOnly
            } else {
                ConfigRequirement::ReadOnly
            }
        } else {
            if property.required {
                ConfigRequirement::Required
            } else {
                ConfigRequirement::Optional
            }
        };
        if requirement == ConfigRequirement::Protected && default.is_none() {
            return Err(StructureError::ProtectedTypeMissingDefault(
                object.name.clone(),
                property.name.clone(),
            ));
        }
        properties.push(ConfigProperty {
            name: property.name.clone(),
            property_type,
            default,
            requirement,
            description,
        });
    }

    if include_links {
        'links: for link in &object.links {
            let target_types = schema.find_types_by_subclass(&link.target.name);
            if target_types.is_empty() {
                return Err(StructureError::NoLinkedTypesFound(
                    object.name.clone(),
                    link.name.clone(),
                    link.target.name.clone(),
                ));
            }
            let mut object_map = BTreeMap::new();
            for target_type in target_types {
                let target_object = create_table(domain, schema, target_type, false)?;
                object_map.insert(target_type.name.clone(), target_object);
            }

            let mut description = None;

            for annotation in &link.annotations {
                if annotation.name == "cfg::internal" {
                    continue 'links;
                }
                if annotation.name == "std::description" {
                    description = Some(annotation.value.clone());
                }
            }

            let requirement = if link.readonly {
                if link.required {
                    ConfigRequirement::RequiredReadOnly
                } else {
                    ConfigRequirement::ReadOnly
                }
            } else {
                if link.required {
                    ConfigRequirement::Required
                } else {
                    ConfigRequirement::Optional
                }
            };

            if link.multi {
                properties.push(ConfigProperty {
                    name: link.name.clone(),
                    property_type: ConfigPropertyType::Array(Box::new(ConfigPropertyType::Object(
                        object_map,
                    ))),
                    requirement,
                    default: None,
                    description,
                });
            } else {
                properties.push(ConfigProperty {
                    name: link.name.clone(),
                    property_type: ConfigPropertyType::Object(object_map),
                    requirement,
                    default: None,
                    description,
                });
            }
        }
    }

    Ok(ConfigTable {
        name: object.name.clone(),
        properties,
    })
}

fn walk_object(
    domain: &mut ConfigDomain,
    schema: &ConfigSchema,
    object: &ConfigSchemaObject,
    mut locatable: Locatable,
) -> Result<(), StructureError> {
    // Check if this object is an ExtensionConfig
    let is_extension_config = object
        .ancestors
        .iter()
        .any(|ancestor| ancestor.name == "cfg::ExtensionConfig");

    // Extensions are only available on database branches
    if is_extension_config {
        locatable = Locatable::Single;
        if domain.name != ConfigDomainName::DatabaseBranch {
            return Ok(());
        }
    }

    if locatable == Locatable::No {
        return Ok(());
    }

    // Session domain cannot insert
    if locatable == Locatable::Multi && domain.name == ConfigDomainName::Session {
        return Ok(());
    }

    // Create table for the target object
    let mut table = create_table(domain, schema, object, locatable == Locatable::Multi)?;
    if locatable == Locatable::Root {
        table.name = "cfg::Config".to_string();
    }

    if locatable == Locatable::Multi {
        domain.array_tables.insert(object.name.clone(), table);
    } else {
        domain.tables.insert(object.name.clone(), table);

        // Process all links from this object
        for link in &object.links {
            // Find the target object type
            let target_types = schema.find_types_by_subclass(&link.target.name);
            if target_types.is_empty() {
                return Err(StructureError::NoLinkedTypesFound(
                    object.name.clone(),
                    link.name.clone(),
                    link.target.name.clone(),
                ));
            }

            if locatable != Locatable::No {
                for target_type in target_types {
                    // Recursively walk the target object to find nested configurations
                    walk_object(
                        domain,
                        schema,
                        target_type,
                        if link.multi {
                            Locatable::Multi
                        } else {
                            Locatable::Single
                        },
                    )?;
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema2::raw::*;

    #[test]
    fn test_config_schema() {
        let schema = include_str!("schema-6.json");
        let schema: ConfigSchema = serde_json::from_str(schema).unwrap();
        let domains = from_raw(schema);
        println!("{:#?}", domains);
    }
}
