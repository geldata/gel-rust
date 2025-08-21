//! Raw schema representation.

use serde::{Deserialize, Serialize};
use serde_json;
use std::ops::Bound;
use typesafe_builder::*;

use crate::ConfigSchemaPrimitiveType;

/// Represents a configuration schema object (type definition)
#[derive(Debug, Clone, Serialize, Deserialize, Builder)]
pub struct ConfigSchemaObject {
    /// Name of the type (e.g., "cfg::Config", "cfg::Auth")
    #[builder(required, into)]
    pub name: String,
    /// List of ancestor type IDs
    #[builder(default = Vec::new())]
    pub ancestors: Vec<ConfigSchemaTypeReference>,
    /// Properties of this type
    #[builder(default = Vec::new())]
    pub properties: Vec<ConfigSchemaProperty>,
    /// Links (relationships) to other types
    #[builder(default = Vec::new())]
    pub links: Vec<ConfigSchemaLink>,
}

/// Represents a property of a configuration schema object
#[derive(Debug, Clone, Serialize, Deserialize, Builder)]
pub struct ConfigSchemaProperty {
    /// Name of the property
    #[builder(required, into)]
    pub name: String,
    /// Default value (can be null)
    #[builder(optional)]
    pub default: Option<String>,
    /// Target type information
    #[builder(required, into)]
    pub target: ConfigSchemaType,
    /// Whether the property is required
    #[builder(default = false)]
    pub required: bool,
    /// Whether the property is readonly
    #[builder(default = false)]
    pub readonly: bool,
    /// Whether the property is protected
    #[builder(default = false)]
    #[serde(default)]
    pub protected: bool,
    /// Whether the property is multi-valued
    #[builder(default = false)]
    pub multi: bool,
    /// Constraints for the property
    #[builder(default = ConfigSchemaConstraints::default())]
    pub constraints: ConfigSchemaConstraints,
    /// Annotations for the property
    #[builder(default = Vec::new())]
    pub annotations: Vec<ConfigSchemaAnnotation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Builder)]
/// Represents metadata annotations attached to configuration schema properties or links.
pub struct ConfigSchemaAnnotation {
    /// Name of the annotation
    #[builder(required)]
    pub name: String,
    /// Value of the annotation
    #[builder(required)]
    pub value: String,
}

#[derive(Debug, Clone, Builder)]
/// Represents validation constraints for configuration properties (ranges, exclusivity).
pub struct ConfigSchemaConstraints {
    /// Whether the property value is exclusive
    #[builder(default = false)]
    pub exclusive: bool,
    /// Range of values if this type has a valid range
    #[builder(default = (Bound::Unbounded, Bound::Unbounded))]
    pub range: (Bound<String>, Bound<String>),
}

impl Default for ConfigSchemaConstraints {
    fn default() -> Self {
        Self {
            exclusive: false,
            range: (Bound::Unbounded, Bound::Unbounded),
        }
    }
}

impl<'de> serde::Deserialize<'de> for ConfigSchemaConstraints {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Serialize, Deserialize)]
        struct Constraint {
            name: String,
            params: Vec<NameValue>,
        }

        #[derive(Serialize, Deserialize)]
        struct NameValue {
            name: String,
            value: String,
        }

        let mut range = (Bound::Unbounded, Bound::Unbounded);
        let mut exclusive = false;
        let constraints: Vec<Constraint> = serde::Deserialize::deserialize(deserializer)?;
        for constraint in constraints {
            if constraint.name == "std::min_value" {
                range.0 = Bound::Included(constraint.params[0].value.clone());
            } else if constraint.name == "std::max_value" {
                range.1 = Bound::Included(constraint.params[0].value.clone());
            } else if constraint.name == "std::exclusive" {
                exclusive = true;
            }
        }
        Ok(ConfigSchemaConstraints { exclusive, range })
    }
}

impl serde::Serialize for ConfigSchemaConstraints {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut constraints = Vec::new();

        // Add min_value constraint if range start is bounded
        if let Bound::Included(value) = &self.range.0 {
            constraints.push(serde_json::json!({
                "name": "std::min_value",
                "params": [{"name": "value", "value": value}]
            }));
        }

        // Add max_value constraint if range end is bounded
        if let Bound::Included(value) = &self.range.1 {
            constraints.push(serde_json::json!({
                "name": "std::max_value",
                "params": [{"name": "value", "value": value}]
            }));
        }

        // Add exclusive constraint if set
        if self.exclusive {
            constraints.push(serde_json::json!({
                "name": "std::exclusive",
                "params": []
            }));
        }

        constraints.serialize(serializer)
    }
}

/// Represents a link/relationship to another type
#[derive(Debug, Clone, Serialize, Deserialize, Builder)]
pub struct ConfigSchemaLink {
    /// Name of the link
    #[builder(required, into)]
    pub name: String,
    /// Whether the link is multi-valued
    #[builder(default = false)]
    pub multi: bool,
    /// Target type information
    #[builder(required, into)]
    pub target: ConfigSchemaTypeReference,
    /// Whether the property is required
    #[builder(default = false)]
    pub required: bool,
    /// Whether the property is readonly
    #[builder(default = false)]
    pub readonly: bool,
    /// Constraints for the link
    #[builder(default = ConfigSchemaConstraints::default())]
    pub constraints: ConfigSchemaConstraints,
    /// Annotations for the link
    #[builder(default = Vec::new())]
    pub annotations: Vec<ConfigSchemaAnnotation>,
}

/// Represents type information for properties and links
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSchemaType {
    /// Name of the type (e.g., "std::str", "cfg::ConnectionTransport")
    pub name: String,
    /// Enum values if this is an enum type
    pub enum_values: Option<Vec<String>>,
}

impl ConfigSchemaType {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            enum_values: None,
        }
    }
}

impl From<ConfigSchemaPrimitiveType> for ConfigSchemaType {
    fn from(primitive: ConfigSchemaPrimitiveType) -> Self {
        primitive.to_schema_type()
    }
}

impl ConfigSchemaPrimitiveType {
    /// Convert to ConfigSchemaType
    pub fn to_schema_type(&self) -> ConfigSchemaType {
        ConfigSchemaType {
            name: format!("{self}"),
            enum_values: None,
        }
    }
}

/// Represents a type reference with ID and name
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSchemaTypeReference {
    /// Name of the referenced type
    pub name: String,
}

impl ConfigSchemaTypeReference {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

/// Represents the complete configuration schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSchema {
    /// Root type IDs
    pub roots: Vec<ConfigSchemaTypeReference>,
    /// All type definitions
    pub types: Vec<ConfigSchemaObject>,
}

impl ConfigSchema {
    /// Create a new empty schema
    pub fn new() -> Self {
        Self {
            roots: Vec::new(),
            types: Vec::new(),
        }
    }

    /// Find a type by its name
    pub fn find_type_by_name(&self, name: &str) -> Option<&ConfigSchemaObject> {
        self.types.iter().find(|t| t.name == name)
    }

    /// Find all types that are subclasses of the given name
    pub fn find_types_by_subclass(&self, name: &str) -> Vec<&ConfigSchemaObject> {
        self.types
            .iter()
            .filter(|t| t.name == name || t.ancestors.iter().any(|a| a.name == name))
            .collect()
    }

    /// Get all root types
    pub fn get_root_types(&self) -> Vec<&ConfigSchemaObject> {
        self.roots
            .iter()
            .filter_map(|id| self.find_type_by_name(&id.name))
            .collect()
    }
}

impl Default for ConfigSchema {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::current_schema;

    #[test]
    #[cfg(feature = "precomputed")]
    fn test_config_schema() {
        let schema = current_schema();
        for typ in schema.types {
            println!("{}", typ.name);
            for property in typ.properties {
                println!("  {}: {}", property.name, property.target.name);
            }
        }
    }
}
