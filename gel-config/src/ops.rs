use std::collections::BTreeMap;

use indexmap::IndexMap;

use crate::structure::ConfigDomainName;

#[derive(Default, Debug)]
/// Collection of database configuration operations organized by domain (instance, database, session).
pub struct AllSchemaOps {
    pub by_domain: BTreeMap<ConfigDomainName, SchemaOps>,
}

impl AllSchemaOps {
    pub fn to_ddl(&self) -> String {
        let mut result = String::new();
        for (domain, ops) in &self.by_domain {
            result.push_str(&ops.to_ddl(domain));
        }
        result
    }
}

#[derive(Default, Debug)]
pub struct SchemaOps {
    /// Use set for root tables only
    pub set: Vec<SchemaNamedValue>,
    /// Use insert to insert all other tables at the given path
    pub insert: IndexMap<String, Vec<SchemaInsert>>,
}

impl SchemaOps {
    pub fn to_ddl(&self, domain: &ConfigDomainName) -> String {
        let mut result = String::new();

        // Handle set operations
        for value in &self.set {
            result.push_str(&format!(
                "configure {} set {} := {};\n",
                domain,
                value.name,
                value.value.to_ddl_with_type(&value.property_type)
            ));
        }

        // Handle insert operations
        for (path, inserts) in &self.insert {
            // First reset the path
            result.push_str(&format!("configure {domain} reset {path};\n"));

            // Then do all the inserts
            for insert in inserts {
                result.push_str(&format!(
                    "configure {} insert {} {{\n",
                    domain, insert.type_name
                ));
                let mut first = true;
                for (name, value) in &insert.properties {
                    if !first {
                        result.push_str(",\n");
                    }
                    result.push_str(&format!(
                        "    {} := {}",
                        name,
                        value.value.to_ddl_with_type(&value.property_type)
                    ));
                    first = false;
                }
                result.push_str("\n};\n");
            }
        }

        result
    }
}

#[derive(Debug, Clone)]
/// A named configuration value with its type information for database operations.
pub struct SchemaNamedValue {
    pub name: String,
    pub property_type: String,
    pub value: SchemaValue,
}

#[derive(Debug, Clone)]
/// Represents a configuration value that can be a primitive, array, or complex object.
pub enum SchemaValue {
    Unitary(SchemaPrimitive),
    Array(Vec<SchemaPrimitive>),
    Object(String, IndexMap<String, SchemaNamedValue>),
}

#[derive(Debug, Clone)]
/// Basic data types that can be stored in configuration properties.
pub enum SchemaPrimitive {
    String(String),
    Bool(bool),
    Integer(isize),
}

impl SchemaPrimitive {
    pub fn to_ddl(&self, property_type: &str) -> String {
        match self {
            SchemaPrimitive::String(s) => format!("<{property_type}>{}", quote_string(s)),
            SchemaPrimitive::Bool(b) => format!("<{property_type}>{b}"),
            SchemaPrimitive::Integer(i) => format!("<{property_type}>{i}"),
        }
    }
}

impl SchemaValue {
    pub fn to_ddl(&self, property_type: &str) -> String {
        self.to_ddl_with_type(property_type)
    }

    pub fn to_ddl_with_type(&self, property_type: &str) -> String {
        match self {
            SchemaValue::Unitary(val) => val.to_ddl(property_type),
            SchemaValue::Array(vals) => {
                let mut result = String::from("{");
                for (i, v) in vals.iter().enumerate() {
                    if i > 0 {
                        result.push_str(", ");
                    }
                    result.push_str(&v.to_ddl(property_type));
                }
                result.push('}');
                result
            }
            SchemaValue::Object(type_name, props) => {
                let mut result = format!("(insert {type_name} {{\n");

                for (i, (name, value)) in props.iter().enumerate() {
                    if i > 0 {
                        result.push_str(",\n");
                    }
                    result.push_str(&format!(
                        "        {} := {}",
                        name,
                        value.value.to_ddl_with_type(&value.property_type)
                    ));

                    // Add comma if this is not the last property
                    if i < props.len() - 1 {
                        result.push(',');
                    }
                }
                result.push_str("\n    })");
                result
            }
        }
    }
}

#[derive(Debug, Clone)]
/// Represents an insert operation for a configuration object with its properties.
pub struct SchemaInsert {
    pub type_name: String,
    pub properties: IndexMap<String, SchemaNamedValue>,
}

fn quote_string(s: &str) -> String {
    use std::fmt::Write;

    let mut buf = String::with_capacity(s.len() + 2);
    buf.push('\'');
    for c in s.chars() {
        match c {
            '\'' => {
                buf.push('\\');
                buf.push('\'');
            }
            '\\' => {
                buf.push('\\');
                buf.push('\\');
            }
            '\x00'..='\x08'
            | '\x0B'
            | '\x0C'
            | '\x0E'..='\x1F'
            | '\u{007F}'
            | '\u{0080}'..='\u{009F}' => {
                write!(buf, "\\x{:02x}", c as u32).unwrap();
            }
            c => buf.push(c),
        }
    }
    buf.push('\'');
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_ddl() {
        let mut ops = AllSchemaOps::default();
        ops.by_domain.insert(
            ConfigDomainName::Instance,
            SchemaOps {
                set: vec![SchemaNamedValue {
                    name: "test_property".to_string(),
                    property_type: "settype".to_string(),
                    value: SchemaValue::Unitary(SchemaPrimitive::String("string".to_string())),
                }],
                insert: IndexMap::from_iter([(
                    "test".to_string(),
                    vec![SchemaInsert {
                        type_name: "test".to_string(),
                        properties: IndexMap::from_iter([
                            (
                                "test".to_string(),
                                SchemaNamedValue {
                                    name: "test1".to_string(),
                                    property_type: "type1".to_string(),
                                    value: SchemaValue::Unitary(SchemaPrimitive::String(
                                        "test".to_string(),
                                    )),
                                },
                            ),
                            (
                                "test2".to_string(),
                                SchemaNamedValue {
                                    name: "test2".to_string(),
                                    property_type: "type2".to_string(),
                                    value: SchemaValue::Unitary(SchemaPrimitive::String(
                                        "test2".to_string(),
                                    )),
                                },
                            ),
                        ]),
                    }],
                )]),
            },
        );

        assert_eq!(
            ops.to_ddl().trim(),
            r#"
configure instance set test_property := <settype>'string';
configure instance reset test;
configure instance insert test {
    test := <type1>'test',
    test2 := <type2>'test2'
};
"#
            .trim()
        );
    }
}
