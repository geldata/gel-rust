use serde::{Deserialize, Serialize};

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    strum::AsRefStr,
    strum::EnumString,
    Serialize,
    Deserialize,
)]
pub enum Class {
    Object,
    InternalObject,
    QualifiedObject,
    ObjectFragment,
    GlobalObject,
    ExternalObject,
    DerivableObject,
    SubclassableObject,
    InheritingObject,
    DerivableInheritingObject,
    ReferencedObject,
    ReferencedInheritingObject,
    NamedReferencedInheritingObject,
    AnnotationValue,
    AnnotationSubject,
    Annotation,
    Type,
    QualifiedType,
    InheritingType,
    Collection,
    CollectionExprAlias,
    Array,
    ArrayExprAlias,
    Tuple,
    TupleExprAlias,
    Range,
    RangeExprAlias,
    MultiRange,
    MultiRangeExprAlias,
    Alias,
    Global,
    Permission,
    Parameter,
    VolatilitySubject,
    CallableObject,
    Function,
    Cast,
    Migration,
    Module,
    Operator,
    PseudoType,
    BaseSchemaVersion,
    SchemaVersion,
    GlobalSchemaVersion,
    Constraint,
    ConsistencySubject,
    FutureBehavior,
    Rewrite,
    Pointer,
    ScalarType,
    Index,
    IndexableSubject,
    IndexMatch,
    Source,
    Property,
    Link,
    AccessPolicy,
    Trigger,
    ObjectTypeRefMixin,
    ObjectType,
    ExtensionPackage,
    ExtensionPackageMigration,
    Extension,
    Role,
    Branch,
}

impl Class {
    pub const fn bases(&self) -> &'static [Class] {
        use Class::*;

        /* Generated with python:

        from edb.schema import objects as so
        for mcls in so.ObjectMeta.get_schema_metaclasses():
            print(mcls.__name__, '=> &[', end='')
            for base in mcls.__bases__:
                if not issubclass(base, so.Object):
                    continue
                if base.__module__ == 'edb.schema.abc':
                    continue
                print(base.__name__, end=', ')
            print('],')
        */

        match self {
            Object => &[],
            InternalObject => &[Object],
            QualifiedObject => &[Object],
            ObjectFragment => &[QualifiedObject],
            GlobalObject => &[Object],
            ExternalObject => &[GlobalObject],
            DerivableObject => &[QualifiedObject],
            SubclassableObject => &[Object],
            InheritingObject => &[SubclassableObject],
            DerivableInheritingObject => &[DerivableObject, InheritingObject],
            ReferencedObject => &[DerivableObject],
            ReferencedInheritingObject => &[DerivableInheritingObject, ReferencedObject],
            NamedReferencedInheritingObject => &[ReferencedInheritingObject],
            AnnotationValue => &[ReferencedInheritingObject],
            AnnotationSubject => &[Object],
            Annotation => &[QualifiedObject, InheritingObject, AnnotationSubject],
            Type => &[SubclassableObject, AnnotationSubject],
            QualifiedType => &[QualifiedObject, Type],
            InheritingType => &[DerivableInheritingObject, QualifiedType],
            Collection => &[Type],
            CollectionExprAlias => &[QualifiedType, Collection],
            Array => &[Collection],
            ArrayExprAlias => &[CollectionExprAlias, Array],
            Tuple => &[Collection],
            TupleExprAlias => &[CollectionExprAlias, Tuple],
            Range => &[Collection],
            RangeExprAlias => &[CollectionExprAlias, Range],
            MultiRange => &[Collection],
            MultiRangeExprAlias => &[CollectionExprAlias, MultiRange],
            Alias => &[QualifiedObject, AnnotationSubject],
            Global => &[QualifiedObject, AnnotationSubject],
            Permission => &[QualifiedObject, AnnotationSubject],
            Parameter => &[ObjectFragment, Object],
            VolatilitySubject => &[Object],
            CallableObject => &[QualifiedObject, AnnotationSubject],
            Function => &[CallableObject, VolatilitySubject],
            Cast => &[QualifiedObject, AnnotationSubject, VolatilitySubject],
            Migration => &[Object],
            Module => &[AnnotationSubject, Object],
            Operator => &[CallableObject, VolatilitySubject],
            PseudoType => &[InheritingObject, Type],
            BaseSchemaVersion => &[Object],
            SchemaVersion => &[BaseSchemaVersion, InternalObject],
            GlobalSchemaVersion => &[BaseSchemaVersion, InternalObject, GlobalObject],
            Constraint => &[ReferencedInheritingObject, CallableObject],
            ConsistencySubject => &[QualifiedObject, InheritingObject, AnnotationSubject],
            FutureBehavior => &[Object],
            Rewrite => &[
                NamedReferencedInheritingObject,
                InheritingObject,
                AnnotationSubject,
            ],
            Pointer => &[
                NamedReferencedInheritingObject,
                ConsistencySubject,
                AnnotationSubject,
            ],
            ScalarType => &[InheritingType, ConsistencySubject],
            Index => &[
                ReferencedInheritingObject,
                InheritingObject,
                AnnotationSubject,
            ],
            IndexableSubject => &[InheritingObject],
            IndexMatch => &[QualifiedObject, AnnotationSubject],
            Source => &[QualifiedObject, IndexableSubject, Object],
            Property => &[Pointer],
            Link => &[Source, Pointer],
            AccessPolicy => &[
                NamedReferencedInheritingObject,
                InheritingObject,
                AnnotationSubject,
            ],
            Trigger => &[NamedReferencedInheritingObject, InheritingObject],
            ObjectTypeRefMixin => &[Object],
            ObjectType => &[
                Source,
                ConsistencySubject,
                InheritingType,
                InheritingObject,
                Type,
                AnnotationSubject,
                ObjectTypeRefMixin,
            ],
            ExtensionPackage => &[GlobalObject, AnnotationSubject],
            ExtensionPackageMigration => &[GlobalObject, AnnotationSubject],
            Extension => &[Object],
            Role => &[GlobalObject, InheritingObject, AnnotationSubject],
            Branch => &[ExternalObject, AnnotationSubject],
        }
    }

    pub fn is_subclass(&self, ancestor: &Class) -> bool {
        if self == ancestor {
            return true;
        }

        self.bases().iter().any(|b| b.is_subclass(ancestor))
    }

    pub fn is_qualified(&self) -> bool {
        self.is_subclass(&Class::QualifiedObject)
    }

    pub fn get_display_name(&self) -> &'static str {
        match self {
            Class::Module => "module",

            Class::ObjectType => "object type",
            Class::Property => "property",
            Class::Link => "link",

            Class::Index => "index",
            Class::IndexMatch => "index match",

            Class::Constraint => "constraint",
            Class::Trigger => "trigger",
            Class::Rewrite => "rewrite",
            Class::AccessPolicy => "access policy",

            Class::Type => "type",
            Class::Collection | Class::Array | Class::Tuple | Class::Range | Class::MultiRange => {
                "collection"
            }
            Class::CollectionExprAlias
            | Class::ArrayExprAlias
            | Class::TupleExprAlias
            | Class::RangeExprAlias
            | Class::MultiRangeExprAlias => "expression alias",
            Class::ScalarType => "scalar type",

            Class::Alias => "alias",
            Class::Global => "global",

            Class::Function => "function",
            Class::Parameter => "parameter",
            Class::Cast => "cast",
            Class::Operator => "operator",

            Class::Annotation | Class::AnnotationValue => "annotation",

            Class::Branch => "branch",
            Class::Migration => "migration",
            Class::FutureBehavior => "future behavior",
            Class::Extension => "extension",
            Class::ExtensionPackage => "extension package",
            Class::ExtensionPackageMigration => "extension package migration",

            Class::Permission => "permission",
            Class::Role => "role",

            _ => unimplemented!("get_display_name for abstract schema classes"),
        }
    }
}
