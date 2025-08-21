use std::{collections::BTreeMap, str::FromStr, vec};

use crate::{
    ConfigSchemaPrimitiveType,
    raw::{ConfigSchema, ConfigSchemaObject},
};

#[derive(Debug, Clone, derive_more::Error, derive_more::Display)]
/// Errors that can occur when processing raw configuration schema into structured domains.
pub enum StructureError {
    #[display("Invalid primitive type: {_0} ({_1})")]
    InvalidPrimitiveType(String, String),
    #[display("No linked types found for {_0}::{_1} -> {_2}")]
    NoLinkedTypesFound(String, String, String),
    #[display("Protected type {_0}::{_1} missing default value")]
    ProtectedTypeMissingDefault(String, String),
}

#[derive(Debug, Clone, Default)]
/// Collection of configuration domains (instance, database, session) with their associated tables.
pub struct ConfigDomains {
    pub domains: BTreeMap<ConfigDomainName, ConfigDomain>,
}

#[derive(Debug, Clone)]
/// A configuration domain containing tables for a specific scope (instance, database, or session).
pub struct ConfigDomain {
    pub name: ConfigDomainName,
    pub tables: BTreeMap<String, ConfigTable>,
}

impl ConfigDomain {
    pub fn new(name: ConfigDomainName, tables: impl IntoIterator<Item = ConfigTable>) -> Self {
        Self {
            name,
            tables: tables
                .into_iter()
                .map(|table| (table.name.clone(), table))
                .collect(),
        }
    }

    pub fn get_root_table(&self) -> &ConfigTable {
        self.tables.get("cfg::Config").unwrap()
    }

    pub fn get_table(&self, name: &str) -> Option<&ConfigTable> {
        self.tables.get(name)
    }

    /// Multiple tables can have the same path.
    pub fn get_tables_by_path_or_name(&self, name: &str) -> Vec<&ConfigTable> {
        if let Some(table) = self.tables.get(name) {
            return vec![table];
        }

        let mut tables = vec![];
        for table in self.tables.values() {
            if table.path.as_deref() == Some(name) {
                tables.push(table);
            }
        }
        tables
    }
}

#[derive(Clone)]
pub struct ConfigTable {
    /// The fully-qualified schema name of the table
    pub name: String,
    /// The path to the table in the global namespace. This can be used to reset
    /// the table, though not all tables are resettable even if they have a path.
    pub path: Option<String>,
    /// Whether the path requires a _tname to be set
    pub path_requires_tname: bool,
    /// The properties of the table
    pub properties: Vec<ConfigProperty>,
    /// Whether the table is multi-instance
    pub multi: bool,
    /// Whether the table is a singleton and allows set operations
    pub singleton: bool,
}

impl ConfigTable {
    pub fn get_property(&self, key: &str) -> Option<&ConfigProperty> {
        self.properties.iter().find(|p| p.name == key)
    }
}

impl std::fmt::Debug for ConfigTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if f.alternate() {
            if let Some(path) = &self.path {
                writeln!(f, "# path={}, multi={}", path, self.multi)?;
            } else if self.multi {
                writeln!(f, "# multi")?;
            }
            writeln!(f, "{{")?;
        } else {
            write!(
                f,
                "ConfigTable {{ name: {}, path: {:?}, multi={}, properties: [",
                self.name, self.path, self.multi
            )?;
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
                    writeln!(f)?;
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
/// A configuration property with its type, requirements, and metadata.
pub struct ConfigProperty {
    pub name: String,
    pub property_type: ConfigPropertyType,
    pub requirement: ConfigRequirement,
    pub default: Option<String>,
    pub description: Option<String>,
    pub is_internal: bool,
}

#[derive(Debug, Clone, derive_more::Display, PartialEq, Eq)]
/// Defines the requirement level of a configuration property (ie, a requirement
/// computed from its required, readonly and protected flags).
pub enum ConfigRequirement {
    #[display("required")]
    /// A required property must be set at insertion time.
    Required,
    /// A required readonly property must be set at insertion time, but cannot
    /// be changed after.
    #[display("required readonly")]
    RequiredReadOnly,
    /// An optional property is one that is not required to be set.
    #[display("optional")]
    Optional,
    /// A readonly property is one that is not writable by the user after it is
    /// created. It can be set at insertion time, but not after.
    #[display("readonly")]
    ReadOnly,
    /// A protected property is one that is not writable by the user, but is
    /// set to its default value.
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
    #[debug("<{_0:#?}>")]
    Object(BTreeMap<String, ConfigTable>),
}

impl ConfigPropertyType {
    pub fn name(&self) -> Option<String> {
        match self {
            ConfigPropertyType::Primitive(primitive_type) => {
                Some(primitive_type.to_schema_type().name)
            }
            ConfigPropertyType::Enum(name, _) => Some(name.clone()),
            ConfigPropertyType::Array(_) => None,
            ConfigPropertyType::Object(_) => None,
        }
    }

    pub fn describe(&self) -> String {
        match self {
            ConfigPropertyType::Primitive(primitive_type) => primitive_type.to_schema_type().name,
            ConfigPropertyType::Enum(name, _) => name.clone(),
            ConfigPropertyType::Array(inner) => format!("array<{}>", inner.describe()),
            ConfigPropertyType::Object(map) => {
                let mut names = map.keys().collect::<Vec<_>>();
                names.sort();
                format!(
                    "choice<{}>",
                    names
                        .iter()
                        .map(|n| n.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// Identifies the scope of a configuration domain (instance-wide, database-specific, or session-specific).
pub enum ConfigDomainName {
    Instance,
    DatabaseBranch,
    Session,
}

impl std::fmt::Display for ConfigDomainName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigDomainName::Instance => write!(f, "instance"),
            ConfigDomainName::DatabaseBranch => write!(f, "current database"),
            ConfigDomainName::Session => write!(f, "session"),
        }
    }
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
        };

        // Walk through all linked objects
        walk_object(
            &mut domain,
            &schema,
            root,
            Locatable::Root,
            false,
            Some("cfg::Config".into()),
        )?;

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
        let mut is_internal = false;

        for annotation in &property.annotations {
            if annotation.name == "cfg::internal" {
                is_internal = true;
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
        } else if property.required {
            ConfigRequirement::Required
        } else {
            ConfigRequirement::Optional
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
            is_internal,
        });
    }

    if include_links {
        'links: for link in &object.links {
            let target_types = schema.find_types_by_subclass(&link.target.name);
            if target_types.is_empty() {
                if link.target.name == "cfg::ExtensionConfig" {
                    continue 'links;
                }
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
            } else if link.required {
                ConfigRequirement::Required
            } else {
                ConfigRequirement::Optional
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
                    is_internal: false,
                });
            } else {
                properties.push(ConfigProperty {
                    name: link.name.clone(),
                    property_type: ConfigPropertyType::Object(object_map),
                    requirement,
                    default: None,
                    description,
                    is_internal: false,
                });
            }
        }
    }

    Ok(ConfigTable {
        name: object.name.clone(),
        path: None,
        path_requires_tname: false,
        properties,
        multi: false,
        singleton: false,
    })
}

fn walk_object(
    domain: &mut ConfigDomain,
    schema: &ConfigSchema,
    object: &ConfigSchemaObject,
    mut locatable: Locatable,
    mut polymorphic: bool,
    path: Option<String>,
) -> Result<(), StructureError> {
    // Check if this object is an ExtensionConfig
    let is_extension_config = object
        .ancestors
        .iter()
        .any(|ancestor| ancestor.name == "cfg::ExtensionConfig");

    // Extensions are only available on database/session branches
    if is_extension_config {
        locatable = Locatable::Single;
        polymorphic = false;
        if domain.name == ConfigDomainName::Instance {
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
    table.path = path;
    table.path_requires_tname = polymorphic;
    if locatable == Locatable::Root {
        table.name = "cfg::Config".to_string();
    }

    table.singleton = locatable == Locatable::Root || is_extension_config;

    if locatable == Locatable::Multi {
        table.multi = true;
        domain.tables.insert(table.name.clone(), table);
    } else {
        domain.tables.insert(table.name.clone(), table);

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

            let polymorphic =
                !(target_types.len() == 1 && target_types[0].name == link.target.name);

            if locatable != Locatable::No {
                for target_type in target_types {
                    // TODO: this logic might break if we have more nested singletons
                    let path = if locatable == Locatable::Single {
                        Some(format!("{}::{}", object.name, link.name))
                    } else if locatable == Locatable::Root
                        && link.target.name != "cfg::ExtensionConfig"
                    {
                        Some(link.name.clone())
                    } else {
                        Some(target_type.name.clone())
                    };

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
                        polymorphic,
                        path,
                    )?;
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::current_config;

    #[test]
    #[cfg(feature = "precomputed")]
    fn test_config_schema() {
        let config = current_config();
        println!("{config:#?}");
    }
}
