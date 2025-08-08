use std::collections::BTreeMap;

use indexmap::IndexMap;

use crate::schema2::structure::ConfigDomainName;

#[derive(Default, Debug)]
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
                value.value.to_ddl(&value.property_type)
            ));
        }

        // Handle insert operations
        for (path, inserts) in &self.insert {
            // First reset the path
            result.push_str(&format!("configure {} reset {};\n", domain, path));

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
                        value.value.to_ddl(&value.property_type)
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
pub struct SchemaNamedValue {
    pub name: String,
    pub property_type: String,
    pub value: SchemaValue,
}

#[derive(Debug, Clone)]
pub enum SchemaValue {
    Unitary(String),
    Array(Vec<String>),
    Object(IndexMap<String, SchemaNamedValue>),
}

impl SchemaValue {
    pub fn to_ddl(&self, property_type: &str) -> String {
        match self {
            SchemaValue::Unitary(val) => {
                format!("<{}>'{}'\n", property_type, val).trim().to_string()
            }
            SchemaValue::Array(vals) => {
                let mut result = String::from("{");
                for (i, v) in vals.iter().enumerate() {
                    if i > 0 {
                        result.push_str(", ");
                    }
                    result.push_str(&format!("<{}>'{}'\n", property_type, v).trim());
                }
                result.push('}');
                result
            }
            SchemaValue::Object(props) => {
                let mut result = String::from("{\n");
                let mut first = true;
                for (name, value) in props {
                    if !first {
                        result.push_str(",\n");
                    }
                    result.push_str(&format!(
                        "    {} := {}",
                        name,
                        value.value.to_ddl(&value.property_type)
                    ));
                    first = false;
                }
                result.push('}');
                result
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct SchemaInsert {
    pub type_name: String,
    pub properties: IndexMap<String, SchemaNamedValue>,
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
                    value: SchemaValue::Unitary("string".to_string()),
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
                                    value: SchemaValue::Unitary("test".to_string()),
                                },
                            ),
                            (
                                "test2".to_string(),
                                SchemaNamedValue {
                                    name: "test2".to_string(),
                                    property_type: "type2".to_string(),
                                    value: SchemaValue::Unitary("test2".to_string()),
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
