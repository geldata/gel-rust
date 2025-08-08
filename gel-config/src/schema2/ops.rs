use indexmap::IndexMap;

use crate::schema2::ConfigSchemaPrimitiveType;

#[derive(Default, Debug)]
pub struct SchemaOps {
    /// Use set for root tables only
    pub set: Vec<SchemaNamedValue>,
    /// Use insert to insert all other tables at the given path
    pub insert: IndexMap<String, Vec<SchemaInsert>>,
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
}

#[derive(Debug, Clone)]
pub struct SchemaInsert {
    pub type_name: String,
    pub properties: IndexMap<String, SchemaNamedValue>,
}
