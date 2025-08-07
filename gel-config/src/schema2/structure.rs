use std::{collections::HashMap, str::FromStr};

use crate::schema2::{
    raw::{ConfigSchema, ConfigSchemaObject},
    ConfigSchemaPrimitiveType,
};

#[derive(Debug, Clone)]
pub struct ConfigDomains {
    pub domains: HashMap<String, ConfigDomain>,
}

#[derive(Debug, Clone)]
pub struct ConfigDomain {
    pub name: String,
    /// Single-instance tables
    pub tables: HashMap<String, ConfigTable>,
    pub array_tables: HashMap<String, ConfigTable>,
}

#[derive(Debug, Clone)]
pub struct ConfigTable {
    pub name: String,
    pub properties: Vec<ConfigProperty>,
}

#[derive(Debug, Clone)]
pub struct ConfigProperty {
    pub name: String,
    pub property_type: ConfigPropertyType,
}

#[derive(derive_more::Debug, Clone)]
pub enum ConfigPropertyType {
    /// A primitive type (string, int, float, etc.)
    #[debug("{_0:?}")]
    Primitive(ConfigSchemaPrimitiveType),
    /// An enum type (string, with possible values)
    #[debug("{_0:?}: enum<{}>", _1.join(", "))]
    Enum(String, Vec<String>),
    /// An array of types
    #[debug("array<{_0:?}>")]
    Array(Box<ConfigPropertyType>),
    /// A set of possible object structures
    #[debug("{_0:?}")]
    Object(HashMap<String, ConfigTable>),
}

pub fn from_raw(schema: ConfigSchema) -> ConfigDomains {
    let mut domains = HashMap::new();

    // Walk the list of properties starting at the root. If an object is linked
    // from an AbstractConfig or an ExtensionConfig subclass, we add it either
    // to tables or array_tables depending on the cardinality of the link. If the
    // object is an ExtensionConfig, we add it to the non-array tables, since
    // only one of each ExtensionConfig can exist. Add the root object itself to
    // the tables as well as "cfg::Config".

    // Process each root configuration type
    for root in schema.get_root_types() {
        let domain_name = match root.name.as_str() {
            "cfg::InstanceConfig" => "instance",
            "cfg::DatabaseConfig" => "database",
            "cfg::Config" => "session",
            _ => continue, // Skip other root types
        };

        let mut domain = ConfigDomain {
            name: domain_name.to_string(),
            tables: HashMap::new(),
            array_tables: HashMap::new(),
        };

        // Walk through all linked objects
        walk_object(&mut domain, &schema, root, Locatable::Single);

        domains.insert(domain_name.to_string(), domain);
    }

    ConfigDomains { domains }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Locatable {
    /// The object is not locatable, so we don't need to create a table for it.
    No,
    /// The object is locatable and a singleton (ie: an extension config OR a
    /// singleton on an extension config), so we need to create a
    /// single-instance table for it.
    Single,
    /// The object is locatable and multi-instance, so we need to create a
    /// multi-instance table for it.
    Multi,
}

fn create_table(
    schema: &ConfigSchema,
    object: &ConfigSchemaObject,
    include_links: bool,
) -> ConfigTable {
    let mut properties = vec![];
    for property in &object.properties {
        let mut property_type = if let Some(enum_values) = property.target.enum_values.clone() {
            ConfigPropertyType::Enum(property.target.name.clone(), enum_values)
        } else {
            ConfigPropertyType::Primitive(
                ConfigSchemaPrimitiveType::from_str(&property.target.name).unwrap(),
            )
        };
        if property.multi {
            property_type = ConfigPropertyType::Array(Box::new(property_type));
        }
        properties.push(ConfigProperty {
            name: property.name.clone(),
            property_type,
        });
    }
    if include_links {
        for link in &object.links {
            let target_types = schema.find_types_by_subclass(&link.target.name);
            if target_types.is_empty() {
                eprintln!("{}: {:?} not found", object.name, link.target.name);
            }
            let mut object_map = HashMap::new();
            for target_type in target_types {
                let target_object = create_table(schema, target_type, false);
                object_map.insert(target_type.name.clone(), target_object);
            }
            if link.multi {
                properties.push(ConfigProperty {
                    name: link.name.clone(),
                    property_type: ConfigPropertyType::Array(Box::new(ConfigPropertyType::Object(
                        object_map,
                    ))),
                });
            } else {
                properties.push(ConfigProperty {
                    name: link.name.clone(),
                    property_type: ConfigPropertyType::Object(object_map),
                });
            }
        }
    }

    ConfigTable {
        name: object.name.clone(),
        properties,
    }
}

fn walk_object(
    domain: &mut ConfigDomain,
    schema: &ConfigSchema,
    object: &ConfigSchemaObject,
    mut locatable: Locatable,
) {
    // Check if this object is an ExtensionConfig
    let is_extension_config = object
        .ancestors
        .iter()
        .any(|ancestor| ancestor.name == "cfg::ExtensionConfig");

    if is_extension_config {
        locatable = Locatable::Single;
    }

    if locatable == Locatable::No {
        return;
    }

    // Create table for the target object
    let table = create_table(schema, object, locatable == Locatable::Multi);

    if locatable == Locatable::Multi {
        domain.array_tables.insert(object.name.clone(), table);
    } else {
        domain.tables.insert(object.name.clone(), table);

        // Process all links from this object
        for link in &object.links {
            // Find the target object type
            let target_types = schema.find_types_by_subclass(&link.target.name);
            if target_types.is_empty() {
                eprintln!("{}: {:?} not found", object.name, link.target.name);
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
                    );
                }
            }
        }
    }
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

    #[test]
    fn test_basic_config_structure() {
        // Create a simple test schema
        let mut schema = ConfigSchema::new();

        // Add root types
        schema.roots.push(ConfigSchemaTypeReference {
            name: "cfg::InstanceConfig".to_string(),
        });

        // Add type definitions
        schema.types.push(ConfigSchemaObject {
            name: "cfg::InstanceConfig".to_string(),
            ancestors: vec![ConfigSchemaTypeReference {
                name: "cfg::AbstractConfig".to_string(),
            }],
            properties: vec![ConfigSchemaPropertyBuilder::new()
                .with_name("listen_port".to_string())
                .with_target(ConfigSchemaType {
                    name: "std::int32".to_string(),
                    enum_values: None,
                })
                .build()],
            links: vec![ConfigSchemaLink {
                name: "auth".to_string(),
                multi: true,
                target: ConfigSchemaTypeReference {
                    name: "cfg::Auth".to_string(),
                },
            }],
        });

        schema.types.push(ConfigSchemaObject {
            name: "cfg::Auth".to_string(),
            ancestors: vec![ConfigSchemaTypeReference {
                name: "cfg::ConfigObject".to_string(),
            }],
            properties: vec![ConfigSchemaPropertyBuilder::new()
                .with_name("priority".to_string())
                .with_target(ConfigSchemaType {
                    name: "std::int64".to_string(),
                    enum_values: None,
                })
                .build()],
            links: vec![],
        });

        let domains = from_raw(schema);
        println!("{:#?}", domains);

        // Verify the structure
        assert!(domains.domains.contains_key("instance"));
        let instance_domain = &domains.domains["instance"];

        // Should have InstanceConfig in regular tables
        assert!(instance_domain.tables.contains_key("cfg::InstanceConfig"));

        // Should have Auth in array_tables (multi-valued link)
        assert!(instance_domain.array_tables.contains_key("cfg::Auth"));
    }
}
