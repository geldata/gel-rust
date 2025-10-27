use std::fmt::Write;
use std::rc::Rc;
use std::str::FromStr;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Class, Name};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Value {
    Object(Object),
    ObjectDict(Rc<ObjectDict>),
    ObjectList(Rc<ObjectList>),
    ObjectSet(Rc<ObjectSet>),
    ObjectIndex(Rc<ObjectIndex>),
    Name(Rc<Name>),
    Expression(Rc<Expression>),
    ExpressionList(Rc<Vec<Expression>>),
    ExpressionDict(Rc<IndexMap<String, Expression>>),
    Uuid(Uuid),
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(Rc<String>),
    Container(Rc<Container>),
    Enum(EnumTy, String),
    Version(Version),
    Span(Rc<Span>),
    None,
}

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Object {
    pub(crate) class: Class,
    pub(crate) id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Expression {
    pub text: String,
    pub refs: Vec<Uuid>,
    pub origin: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version {
    pub major: u8,
    pub minor: u8,
    pub stage: VersionStage,
    pub stage_no: u16,
    pub local: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectDict {
    pub keys: Vec<String>,
    pub values: Vec<Uuid>,
    pub value_ty: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectList {
    pub ty: ObjectListTy,
    pub value_ty: Option<String>,
    pub values: Vec<Uuid>,
}

#[derive(Debug, Clone, strum::EnumString, strum::AsRefStr, Serialize, Deserialize)]
pub enum ObjectListTy {
    ObjectList,
    FuncParameterList,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectSet {
    pub value_ty: Option<String>,
    pub values: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectIndex {
    pub ty: ObjectIndexTy,
    // keys are (sometimes) derived from target object's name
    pub keys: Option<Vec<Name>>,
    pub values: Vec<Uuid>,
    pub value_ty: String,
}

#[derive(Debug, Clone, strum::EnumString, strum::AsRefStr, Serialize, Deserialize)]
pub enum ObjectIndexTy {
    ObjectIndexByFullname,
    ObjectIndexByShortname,
    ObjectIndexByUnqualifiedName,
    ObjectIndexByConstraintName,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Container {
    pub ty: ContainerTy,
    pub values: Vec<Value>,
    pub value_ty: String,
}

#[derive(Debug, Clone, strum::EnumString, strum::AsRefStr, Serialize, Deserialize)]
pub enum ContainerTy {
    FrozenCheckedList,
    FrozenCheckedSet,
    ExpressionList,
    CheckedList,
    MultiPropSet,
}

#[derive(Debug, Clone, strum::EnumString, strum::AsRefStr, Serialize, Deserialize)]
#[strum(ascii_case_insensitive)]
pub enum VersionStage {
    DEV,
    ALPHA,
    BETA,
    RC,
    FINAL,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    pub filename: Option<String>,
    pub buffer: String,
    pub start: u32,
    pub end: u32,
}

impl Value {
    pub fn ref_ids<'a>(&'a self) -> Box<dyn Iterator<Item = Uuid> + 'a> {
        match self {
            Value::Object(object) => Box::new(Some(object.id).into_iter()),

            Value::ObjectDict(o) => Box::new(o.values.iter().cloned()),
            Value::ObjectList(o) => Box::new(o.values.iter().cloned()),
            Value::ObjectSet(o) => Box::new(o.values.iter().cloned()),
            Value::ObjectIndex(o) => Box::new(o.values.iter().cloned()),

            Value::Expression(expression) => Box::new(expression.refs.iter().cloned()),
            Value::ExpressionList(exprs) => {
                Box::new(exprs.iter().flat_map(|e| e.refs.iter().cloned()))
            }
            Value::ExpressionDict(exprs) => {
                Box::new(exprs.values().flat_map(|e| e.refs.iter().cloned()))
            }

            Value::Container(c) => Box::new(c.values.iter().flat_map(|i| i.ref_ids())),

            Value::Name(_)
            | Value::Uuid(_)
            | Value::Bool(_)
            | Value::Int(_)
            | Value::Float(_)
            | Value::Str(_)
            | Value::Enum(..)
            | Value::Version(_)
            | Value::Span(_)
            | Value::None => Box::new(std::iter::empty()),
        }
    }

    pub fn is_none(&self) -> bool {
        matches!(self, Value::None)
    }

    pub fn get_container_ty_name(name: &str) -> ContainerTy {
        ContainerTy::from_str(name).unwrap_or_else(|_| panic!("unknown ContainerTy: {name}"))
    }

    pub fn get_object_index_ty_name(name: &str) -> ObjectIndexTy {
        ObjectIndexTy::from_str(name).unwrap_or_else(|_| panic!("unknown ObjectIndexTy: {name}"))
    }
}

#[derive(Debug, Clone, strum::EnumString, strum::AsRefStr, Serialize, Deserialize)]
pub enum EnumTy {
    Language,
    ExprType,
    SchemaCardinality,
    LinkTargetDeleteAction,
    LinkSourceDeleteAction,
    OperatorKind,
    Volatility,
    ParameterKind,
    TypeModifier,
    AccessPolicyAction,
    AccessKind,
    TriggerTiming,
    TriggerKind,
    TriggerScope,
    RewriteKind,
    MigrationGeneratedBy,
    IndexDeferrability,
    SplatStrategy,
}

macro_rules! parse_enum {
    ($val: expr, $enum_name: expr, [$($variants: literal,)+]) => {
        match $val {
            $(
                $variants => ($enum_name, $variants),
            )+
            v => panic!("invalid value for `{:?}`: {v}", $enum_name),
        }
    };
}

impl Value {
    pub fn parse_enum(enum_name: &str, val: &str) -> Option<Value> {
        let (enum_ty, variant_name) = match enum_name {
            "Language" => parse_enum!(val, EnumTy::Language, ["SQL", "EdgeQL",]),
            "ExprType" => parse_enum!(
                val,
                EnumTy::ExprType,
                ["Select", "Insert", "Update", "Delete", "Group",]
            ),

            "Cardinality" | "SchemaCardinality" => {
                parse_enum!(val, EnumTy::SchemaCardinality, ["One", "Many", "Unknown",])
            }

            "LinkTargetDeleteAction" | "TargetDeleteAction" => parse_enum!(
                val,
                EnumTy::LinkTargetDeleteAction,
                ["Restrict", "DeleteSource", "Allow", "DeferredRestrict",]
            ),

            "LinkSourceDeleteAction" | "SourceDeleteAction" => parse_enum!(
                val,
                EnumTy::LinkSourceDeleteAction,
                ["DeleteTarget", "Allow", "DeleteTargetIfOrphan",]
            ),

            "OperatorKind" => parse_enum!(
                val,
                EnumTy::OperatorKind,
                ["Infix", "Postfix", "Prefix", "Ternary",]
            ),

            "Volatility" => parse_enum!(
                val,
                EnumTy::Volatility,
                ["Immutable", "Stable", "Volatile", "Modifying",]
            ),

            "ParameterKind" => parse_enum!(
                val,
                EnumTy::ParameterKind,
                ["VariadicParam", "NamedOnlyParam", "PositionalParam",]
            ),

            "TypeModifier" => parse_enum!(
                val,
                EnumTy::TypeModifier,
                ["SetOfType", "OptionalType", "SingletonType",]
            ),

            "AccessPolicyAction" => {
                parse_enum!(val, EnumTy::AccessPolicyAction, ["Allow", "Deny",])
            }

            "AccessKind" => parse_enum!(
                val,
                EnumTy::AccessKind,
                ["Select", "UpdateRead", "UpdateWrite", "Delete", "Insert",]
            ),

            "TriggerTiming" => {
                parse_enum!(val, EnumTy::TriggerTiming, ["After", "AfterCommitOf",])
            }

            "TriggerKind" => {
                parse_enum!(val, EnumTy::TriggerKind, ["Update", "Delete", "Insert",])
            }

            "TriggerScope" => parse_enum!(val, EnumTy::TriggerScope, ["All", "Each",]),

            "RewriteKind" => parse_enum!(val, EnumTy::RewriteKind, ["Update", "Insert",]),

            "MigrationGeneratedBy" => {
                parse_enum!(
                    val,
                    EnumTy::MigrationGeneratedBy,
                    ["DevMode", "DDLStatement",]
                )
            }

            "IndexDeferrability" => parse_enum!(
                val,
                EnumTy::IndexDeferrability,
                ["Prohibited", "Permitted", "Required",]
            ),

            "SplatStrategy" => {
                parse_enum!(
                    val,
                    EnumTy::SplatStrategy,
                    ["Default", "Explicit", "Implicit",]
                )
            }

            _ => return None,
        };
        Some(Value::Enum(enum_ty, variant_name.to_string()))
    }
}

impl Object {
    pub fn new(class: Class, id: Uuid) -> Self {
        Self { class, id }
    }

    pub fn class(&self) -> &Class {
        &self.class
    }

    pub fn id(&self) -> &Uuid {
        &self.id
    }
}

impl std::fmt::Debug for Object {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_char('<')?;
        self.class.fmt(f)?;
        f.write_char(' ')?;
        self.id.fmt(f)?;
        f.write_char('>')
    }
}
