#![allow(dead_code)]

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

const CLASS_STRUCTURE_JSON: &[u8] = include_bytes!("./class_structure.json");

static CLASS_STRUCTURE: OnceLock<Structures> = OnceLock::new();

pub fn get_structures() -> &'static Structures {
    CLASS_STRUCTURE.get_or_init(|| serde_json::from_slice(CLASS_STRUCTURE_JSON).unwrap())
}

#[derive(Debug, Deserialize)]
pub struct Structures {
    #[serde(flatten)]
    pub(crate) classes: HashMap<String, ClassStructure>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ClassStructure {
    pub layouts: HashMap<String, FieldLayout>,
    pub fields: HashMap<String, Field>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct FieldLayout {
    pub r#type: String,
    pub cardinality: Cardinality,
    pub properties: HashMap<String, FieldType>,
    pub fieldname: String,
    pub schema_fieldname: String,
    pub is_ordered: bool,
    pub reflection_proxy: Option<(String, String)>,
    pub storage: Option<FieldStorage>,
    pub is_refdict: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Field {
    pub r#type: String,
    pub hashable: Option<bool>,
    pub allow_ddl_set: Option<bool>,
    pub allow_interpolation: Option<bool>,
    pub index: usize,
}

#[derive(Debug, Deserialize)]
pub(crate) struct FieldStorage {
    /// Field type specifying special handling, if necessary.
    #[serde(rename = "fieldtype")]
    pub fieldtype: FieldType,
    /// Pointer kind (property or link) and cardinality (single or multi)
    #[serde(rename = "ptrkind")]
    pub ptrkind: String,
    /// Fully-qualified pointer target type.
    pub ptrtype: String,
    /// Shadow pointer kind, if any.
    pub shadow_ptrkind: Option<String>,
    /// Shadow pointer type, if any.
    pub shadow_ptrtype: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) enum Cardinality {
    One,
    Many,
    Unknown,
}

/// Field type tag for fields requiring special handling."""
#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum FieldType {
    /// Expression field
    Expr,
    /// ExpressionList field
    ExprList,
    /// ExpressionDict field
    ExprDict,
    /// ObjectDict field
    ObjDict,
    /// All other field types
    Other,
}

impl ClassStructure {
    pub fn get_object_reference_fields(&self) -> impl Iterator<Item = (&Field, &FieldLayout)> {
        self.layouts
            .values()
            .filter(|l| l.is_refdict)
            .map(|l| (self.fields.get(&l.fieldname).unwrap(), l))
    }
}
