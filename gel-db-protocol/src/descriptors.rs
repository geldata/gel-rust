use crate::protocol::Cardinality;
use gel_protogen::prelude::*;

protocol!(
    /// BaseDescriptor - represents the shape of all descriptors
    struct BaseDescriptor<'a> {
        /// Descriptor tag.
        tag: u8,
        /// Descriptor data.
        data: Rest<'a>,
    }

    /// Set Descriptor - represents a set of values
    struct SetDescriptor<'a>: BaseDescriptor {
        /// Indicates that this is a Set value descriptor.
        tag: u8 = 0,
        /// Descriptor ID.
        id: Uuid,
        /// Set element type descriptor index.
        type_pos: u16,
    }

    /// Object Shape Descriptor - represents the shape of an object
    struct ObjectShapeDescriptor<'a>: BaseDescriptor {
        /// Indicates that this is an Object Shape descriptor.
        tag: u8 = 1,
        /// Descriptor ID.
        id: Uuid,
        /// Whether is is an ephemeral free shape, if true, then `type_pos` would always be 0 and should not be interpreted.
        ephemeral_free_shape: bool,
        /// Object type descriptor index.
        type_pos: u16,
        /// Array of shape elements.
        elements: Array<'a, u16, ObjectShapeElement<'a>>,
    }

    /// Shape Element - represents a field in an object shape
    struct ObjectShapeElement<'a> {
        /// Field flags: 1 << 0: the field is implicit, 1 << 1: the field is a link property, 1 << 2: the field is a link
        flags: u32,
        /// The cardinality of the shape element.
        cardinality: Cardinality,
        /// Element name.
        name: LString<'a>,
        /// Element type descriptor index.
        type_pos: u16,
        /// Source schema type descriptor index (useful for polymorphic queries).
        source_type_pos: u16,
    }

    /// Base Scalar Type Descriptor - represents a base scalar type
    struct BaseScalarTypeDescriptor<'a>: BaseDescriptor  {
        /// Indicates that this is a Base Scalar Type descriptor.
        tag: u8 = 2,
        /// Schema type ID.
        id: Uuid,
    }

    /// Scalar Type Descriptor - represents a scalar type
    struct ScalarTypeDescriptor<'a>: BaseDescriptor  {
        /// Indicates that this is a Scalar Type descriptor.
        tag: u8 = 3,
        /// Schema type ID.
        id: Uuid,
        /// Schema type name.
        name: LString<'a>,
        /// Whether the type is defined in the schema or is ephemeral.
        schema_defined: bool,
        /// Indexes of ancestor scalar type descriptors in ancestor resolution order (C3).
        ancestors: Array<'a, u16, u16>,
    }

    /// Tuple Type Descriptor - represents a tuple type
    struct TupleTypeDescriptor<'a>: BaseDescriptor  {
        /// Indicates that this is a Tuple Type descriptor.
        tag: u8 = 4,
        /// Schema type ID.
        id: Uuid,
        /// Schema type name.
        name: LString<'a>,
        /// Whether the type is defined in the schema or is ephemeral.
        schema_defined: bool,
        /// Indexes of ancestor scalar type descriptors in ancestor resolution order (C3).
        ancestors: Array<'a, u16, u16>,
        /// The number of elements in tuple.
        element_types: Array<'a, u16, u16>,
    }

    /// Named Tuple Type Descriptor - represents a named tuple type
    struct NamedTupleTypeDescriptor<'a>: BaseDescriptor  {
        /// Indicates that this is a Named Tuple Type descriptor.
        tag: u8 = 5,
        /// Schema type ID.
        id: Uuid,
        /// Schema type name.
        name: LString<'a>,
        /// Whether the type is defined in the schema or is ephemeral.
        schema_defined: bool,
        /// Indexes of ancestor scalar type descriptors in ancestor resolution order (C3).
        ancestors: Array<'a, u16, u16>,
        /// The number of elements in tuple.
        elements: Array<'a, u16, NamedTupleElement<'a>>,
    }

    /// Tuple Element - represents an element in a named tuple
    struct NamedTupleElement<'a> {
        /// Field name.
        name: LString<'a>,
        /// Field type descriptor index.
        type_pos: u16,
    }

    /// Array Type Descriptor - represents an array type
    struct ArrayTypeDescriptor<'a>: BaseDescriptor  {
        /// Indicates that this is an Array Type descriptor.
        tag: u8 = 6,
        /// Schema type ID.
        id: Uuid,
        /// Schema type name.
        name: LString<'a>,
        /// Whether the type is defined in the schema or is ephemeral.
        schema_defined: bool,
        /// Indexes of ancestor scalar type descriptors in ancestor resolution order (C3).
        ancestors: Array<'a, u16, u16>,
        /// Array element type.
        type_pos: u16,
        /// Sizes of array dimensions, -1 indicates unbound dimension.
        dimensions: Array<'a, u16, i32>,
    }

    /// Enumeration Type Descriptor - represents an enumeration type
    struct EnumerationTypeDescriptor<'a>: BaseDescriptor  {
        /// Indicates that this is an Enumeration Type descriptor.
        tag: u8 = 7,
        /// Schema type ID.
        id: Uuid,
        /// Schema type name.
        name: LString<'a>,
        /// Whether the type is defined in the schema or is ephemeral.
        schema_defined: bool,
        /// Indexes of ancestor scalar type descriptors in ancestor resolution order (C3).
        ancestors: Array<'a, u16, u16>,
        /// The number of enumeration members.
        members: Array<'a, u16, LString<'a>>,
    }

    /// Input Shape Descriptor - represents the shape of input data
    struct InputShapeDescriptor<'a>: BaseDescriptor  {
        /// Indicates that this is an Input Shape descriptor.
        tag: u8 = 8,
        /// Descriptor ID.
        id: Uuid,
        /// Shape elements.
        elements: Array<'a, u16, InputShapeElement<'a>>,
    }

    /// Input Shape Element - represents a field in an input shape
    struct InputShapeElement<'a> {
        /// Field flags, currently always zero.
        flags: u32,
        /// The cardinality of the shape element.
        cardinality: Cardinality,
        /// Element name.
        name: LString<'a>,
        /// Element type descriptor index.
        type_pos: u16,
    }

    /// Range Type Descriptor - represents a range type
    struct RangeTypeDescriptor<'a>: BaseDescriptor  {
        /// Indicates that this is a Range Type descriptor.
        tag: u8 = 9,
        /// Schema type ID.
        id: Uuid,
        /// Schema type name.
        name: LString<'a>,
        /// Whether the type is defined in the schema or is ephemeral.
        schema_defined: bool,
        /// Indexes of ancestor scalar type descriptors in ancestor resolution order (C3).
        ancestors: Array<'a, u16, u16>,
        /// Range type descriptor index.
        type_pos: u16,
    }

    /// Object Type Descriptor - represents an object type
    struct ObjectTypeDescriptor<'a>: BaseDescriptor  {
        /// Indicates that this is an object type descriptor.
        tag: u8 = 10,
        /// Schema type ID.
        id: Uuid,
        /// Schema type name (can be empty for ephemeral free object types).
        name: LString<'a>,
        /// Whether the type is defined in the schema or is ephemeral.
        schema_defined: bool,
    }

    /// Compound Type Descriptor - represents a compound type
    struct CompoundTypeDescriptor<'a>: BaseDescriptor  {
        /// Indicates that this is a compound type descriptor.
        tag: u8 = 11,
        /// Schema type ID.
        id: Uuid,
        /// Schema type name.
        name: LString<'a>,
        /// Whether the type is defined in the schema or is ephemeral.
        schema_defined: bool,
        /// Compound type operation, see TypeOperation below.
        op: TypeOperation,
        /// Compound type component type descriptor indexes.
        components: Array<'a, u16, u16>,
    }

    /// Multi-Range Type Descriptor - represents a multi-range type
    struct MultiRangeTypeDescriptor<'a>: BaseDescriptor  {
        /// Indicates that this is a Multi-Range Type descriptor.
        tag: u8 = 12,
        /// Schema type ID.
        id: Uuid,
        /// Schema type name.
        name: LString<'a>,
        /// Whether the type is defined in the schema or is ephemeral.
        schema_defined: bool,
        /// Indexes of ancestor scalar type descriptors in ancestor resolution order (C3).
        ancestors: Array<'a, u16, u16>,
        /// Multi-range type descriptor index.
        type_pos: u16,
    }

    /// SQL Record Descriptor - represents a SQL record type
    struct SQLRecordDescriptor<'a>: BaseDescriptor  {
        /// Indicates that this is a SQL Record descriptor.
        tag: u8 = 13,
        /// Descriptor ID.
        id: Uuid,
        /// Array of shape elements.
        elements: Array<'a, u16, SQLRecordElement<'a>>,
    }

    /// SQL Record Element - represents an element in a SQL record
    struct SQLRecordElement<'a> {
        /// Element name.
        name: LString<'a>,
        /// Element type descriptor index.
        type_pos: u16,
    }

    /// Type Annotation Descriptor - represents type annotations
    struct TypeAnnotationDescriptor<'a>: BaseDescriptor  {
        /// Indicates that this is a Type Annotation descriptor.
        tag: Ranged<u8, 127, 255>,
        /// Index of the descriptor the annotation is for.
        descriptor: u16,
        /// Annotation value.
        data: Rest<'a>,
    }

    struct TypeNameAnnotation<'a>: TypeAnnotationDescriptor {
        /// Indicates that this is a Type Name annotation.
        tag: u8 = 255,
        /// Index of the descriptor the annotation is for.
        descriptor: u16,
        /// Type name.
        name: LString<'a>,
    }
);

/// Generates the boilerplate for each descriptor type. There's a lot of repetition.
macro_rules! impl_descriptor {
    ($($type:ident),* $(,)?) => {
        gel_protogen::paste!( impl_descriptor!(__inner__ $(
            ($type,
            [< $type Descriptor >],
            [< Parsed $type Descriptor >])
        )* ); );
    };

    (__inner__ $(
        ($name:ident,
        $descriptor:ident,
        $parsed_descriptor:ident)
    )*) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Protocol)]
        pub enum Descriptor<'a> {
            $(
                $descriptor($descriptor<'a>),
            )*
            TypeAnnotationDescriptor(TypeAnnotationDescriptor<'a>),
            TypeNameAnnotation(TypeNameAnnotation<'a>),
        }

        impl<'a> Descriptor<'a> {
            /// Returns the [`Uuid`] of the descriptor.
            pub fn id(&self) -> Uuid {
                match self {
                    $( Descriptor::$descriptor(descriptor) => descriptor.id, )*
                    _ => Uuid::nil(),
                }
            }
        }

        #[derive(derive_more::Debug, Clone, Copy, derive_more::From)]
        pub enum ParsedDescriptor<'a, 'b> {
            $(
                #[debug("{_0:?}")]
                $name($parsed_descriptor<'a, 'b>),
            )*
        }

        impl<'a, 'b> ParsedDescriptor<'a, 'b> {
            fn new(descriptors: &'b ParsedDescriptors<'a>, index: u16) -> ParsedDescriptor<'a, 'b> {
                match descriptors.descriptors[index as usize] {
                    $(
                        Descriptor::$descriptor(descriptor) => {
                            ParsedDescriptor::$name($parsed_descriptor::new(descriptors, descriptor))
                        }
                    )*
                    Descriptor::TypeAnnotationDescriptor(..) | Descriptor::TypeNameAnnotation(..) => unimplemented!(),
                }
            }
        }
    };
}

impl_descriptor!(
    Set,
    ObjectShape,
    BaseScalarType,
    ScalarType,
    TupleType,
    NamedTupleType,
    ArrayType,
    EnumerationType,
    InputShape,
    RangeType,
    ObjectType,
    CompoundType,
    MultiRangeType,
    SQLRecord,
);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Protocol)]
/// Type Operation enum for compound types
#[repr(u8)]
pub enum TypeOperation {
    #[default]
    /// Foo | Bar
    Union = 1,
    /// Foo & Bar
    Intersection = 2,
}

#[derive(derive_more::Debug, Clone, derive_more::From)]
#[debug("{:?}", self.root())]
pub struct ParsedDescriptors<'a> {
    root: usize,
    descriptors: Vec<Descriptor<'a>>,
}

impl<'a> ParsedDescriptors<'a> {
    pub fn new(
        root: Uuid,
        raw_descriptors: RestArray<'a, LengthPrefixed<Descriptor<'a>>>,
    ) -> Result<Self, ParseError> {
        let mut descriptors = Vec::with_capacity(raw_descriptors.len());
        let mut root_offset = None;

        for (i, descriptor) in raw_descriptors.into_iter().enumerate() {
            descriptors.push(descriptor.0);
            if descriptor.0.id() == root {
                root_offset = Some(i);
            }
        }

        let Some(root_offset) = root_offset else {
            return Err(ParseError::InvalidData("ParsedDescriptor", 0));
        };

        Ok(Self {
            root: root_offset,
            descriptors,
        })
    }

    pub fn root(&self) -> ParsedDescriptor<'a, '_> {
        self.get_descriptor(self.root as u16)
    }

    fn get_descriptor(&self, index: u16) -> ParsedDescriptor<'a, '_> {
        ParsedDescriptor::new(self, index)
    }
}

#[derive(derive_more::Debug, Clone, Copy, derive_more::Deref, derive_more::Constructor)]
#[debug("Set(id={})", self.id())]
pub struct ParsedSetDescriptor<'a, 'b> {
    descriptors: &'b ParsedDescriptors<'a>,
    #[deref]
    descriptor: SetDescriptor<'a>,
}

impl<'a, 'b> ParsedSetDescriptor<'a, 'b> {
    pub fn set_type(&self) -> ParsedDescriptor<'a, 'b> {
        self.descriptors.get_descriptor(self.descriptor.type_pos)
    }
}

#[derive(derive_more::Debug, Clone, Copy, derive_more::Deref, derive_more::Constructor)]
#[debug("ObjectShape(id={}, ephemeral_free_shape={}, elements={:#?})", self.id(), self.ephemeral_free_shape(), self.elements().collect::<Vec<_>>())]
pub struct ParsedObjectShapeDescriptor<'a, 'b> {
    descriptors: &'b ParsedDescriptors<'a>,
    #[deref]
    descriptor: ObjectShapeDescriptor<'a>,
}

impl<'a, 'b> ParsedObjectShapeDescriptor<'a, 'b> {
    pub fn elements(&self) -> impl Iterator<Item = ParsedObjectShapeElement<'a, 'b>> {
        self.descriptor
            .elements
            .into_iter()
            .map(|index| ParsedObjectShapeElement::new(self.descriptors, index))
    }
}

#[derive(derive_more::Debug, Clone, Copy, derive_more::Deref, derive_more::Constructor)]
#[debug("ObjectShapeElement(name={:?}, cardinality={:?}, element_type={:?}, source_type={:?})", self.name(), self.cardinality(), self.element_type(), self.source_type())]
pub struct ParsedObjectShapeElement<'a, 'b> {
    descriptors: &'b ParsedDescriptors<'a>,
    #[deref]
    descriptor: ObjectShapeElement<'a>,
}

impl<'a, 'b> ParsedObjectShapeElement<'a, 'b> {
    pub fn element_type(&self) -> ParsedDescriptor<'a, 'b> {
        self.descriptors.get_descriptor(self.descriptor.type_pos)
    }

    pub fn source_type(&self) -> ParsedDescriptor<'a, 'b> {
        self.descriptors
            .get_descriptor(self.descriptor.source_type_pos)
    }
}

#[derive(derive_more::Debug, Clone, Copy, derive_more::Deref, derive_more::Constructor)]
#[debug("Set(id={})", self.id())]
pub struct ParsedBaseScalarTypeDescriptor<'a, 'b> {
    #[expect(unused)]
    descriptors: &'b ParsedDescriptors<'a>,
    #[deref]
    descriptor: BaseScalarTypeDescriptor<'a>,
}

#[derive(derive_more::Debug, Clone, Copy, derive_more::Deref, derive_more::Constructor)]
#[debug("ScalarType(id={}, name={:?}, schema_defined={})", self.id(), self.name(), self.schema_defined())]
pub struct ParsedScalarTypeDescriptor<'a, 'b> {
    descriptors: &'b ParsedDescriptors<'a>,
    #[deref]
    descriptor: ScalarTypeDescriptor<'a>,
}

impl<'a, 'b> ParsedScalarTypeDescriptor<'a, 'b> {
    pub fn ancestors(&self) -> impl Iterator<Item = ParsedDescriptor<'a, 'b>> {
        self.descriptor
            .ancestors
            .into_iter()
            .map(|index| self.descriptors.get_descriptor(index))
    }
}

#[derive(derive_more::Debug, Clone, Copy, derive_more::Deref, derive_more::Constructor)]
#[debug("TupleType(id={}, name={:?}, schema_defined={}, elements={:#?})", self.id(), self.name(), self.schema_defined(), self.elements().collect::<Vec<_>>())]
pub struct ParsedTupleTypeDescriptor<'a, 'b> {
    descriptors: &'b ParsedDescriptors<'a>,
    #[deref]
    descriptor: TupleTypeDescriptor<'a>,
}

impl<'a, 'b> ParsedTupleTypeDescriptor<'a, 'b> {
    pub fn ancestors(&self) -> impl Iterator<Item = ParsedDescriptor<'a, 'b>> {
        self.descriptor
            .ancestors
            .into_iter()
            .map(|index| self.descriptors.get_descriptor(index))
    }

    pub fn elements(&self) -> impl Iterator<Item = ParsedDescriptor<'a, 'b>> {
        self.descriptor
            .element_types
            .into_iter()
            .map(|index| self.descriptors.get_descriptor(index))
    }
}

#[derive(derive_more::Debug, Clone, Copy, derive_more::Deref, derive_more::Constructor)]
#[debug("NamedTupleType(id={}, name={:?}, schema_defined={}, elements={:#?})", self.id(), self.name(), self.schema_defined(), self.elements().collect::<Vec<_>>())]
pub struct ParsedNamedTupleTypeDescriptor<'a, 'b> {
    descriptors: &'b ParsedDescriptors<'a>,
    #[deref]
    descriptor: NamedTupleTypeDescriptor<'a>,
}

impl<'a, 'b> ParsedNamedTupleTypeDescriptor<'a, 'b> {
    pub fn ancestors(&self) -> impl Iterator<Item = ParsedDescriptor<'a, 'b>> {
        self.descriptor
            .ancestors
            .into_iter()
            .map(|index| self.descriptors.get_descriptor(index))
    }

    pub fn elements(&self) -> impl Iterator<Item = (&'a str, ParsedDescriptor<'a, 'b>)> {
        self.descriptor.elements.into_iter().map(|index| {
            (
                index.name.to_str().unwrap_or_default(),
                self.descriptors.get_descriptor(index.type_pos),
            )
        })
    }
}

#[derive(derive_more::Debug, Clone, Copy, derive_more::Deref, derive_more::Constructor)]
#[debug("ArrayType(id={}, name={:?}, schema_defined={}, element_type={:?}, dimensions={:?})", self.id(), self.name(), self.schema_defined(), self.element_type(), self.dimensions())]
pub struct ParsedArrayTypeDescriptor<'a, 'b> {
    descriptors: &'b ParsedDescriptors<'a>,
    #[deref]
    descriptor: ArrayTypeDescriptor<'a>,
}

impl<'a, 'b> ParsedArrayTypeDescriptor<'a, 'b> {
    pub fn element_type(&self) -> ParsedDescriptor<'a, 'b> {
        self.descriptors.get_descriptor(self.descriptor.type_pos)
    }

    pub fn dimensions(&self) -> Vec<i32> {
        self.descriptor.dimensions.to_vec()
    }

    pub fn ancestors(&self) -> impl Iterator<Item = ParsedDescriptor<'a, 'b>> {
        self.descriptor
            .ancestors
            .into_iter()
            .map(|index| self.descriptors.get_descriptor(index))
    }
}

#[derive(derive_more::Debug, Clone, Copy, derive_more::Deref, derive_more::Constructor)]
#[debug("EnumerationType(id={}, name={:?}, schema_defined={}, members={:#?})", self.id(), self.name(), self.schema_defined(), self.members().collect::<Vec<_>>())]
pub struct ParsedEnumerationTypeDescriptor<'a, 'b> {
    descriptors: &'b ParsedDescriptors<'a>,
    #[deref]
    descriptor: EnumerationTypeDescriptor<'a>,
}

impl<'a, 'b> ParsedEnumerationTypeDescriptor<'a, 'b> {
    pub fn members(&self) -> impl Iterator<Item = &'a str> {
        self.descriptor
            .members
            .into_iter()
            .map(|s| s.to_str().unwrap_or_default())
    }

    pub fn ancestors(&self) -> impl Iterator<Item = ParsedDescriptor<'a, 'b>> {
        self.descriptor
            .ancestors
            .into_iter()
            .map(|index| self.descriptors.get_descriptor(index))
    }
}

#[derive(derive_more::Debug, Clone, Copy, derive_more::Deref, derive_more::Constructor)]
#[debug("InputShape(id={}, elements={:#?})", self.id(), self.elements().collect::<Vec<_>>())]
pub struct ParsedInputShapeDescriptor<'a, 'b> {
    descriptors: &'b ParsedDescriptors<'a>,
    #[deref]
    descriptor: InputShapeDescriptor<'a>,
}

impl<'a, 'b> ParsedInputShapeDescriptor<'a, 'b> {
    pub fn elements(&self) -> impl Iterator<Item = ParsedInputShapeElement<'a, 'b>> {
        self.descriptor
            .elements
            .into_iter()
            .map(|index| ParsedInputShapeElement::new(self.descriptors, index))
    }
}

#[derive(derive_more::Debug, Clone, Copy, derive_more::Deref, derive_more::Constructor)]
#[debug("InputShapeElement(name={:?}, cardinality={:?}, element_type={:?})", self.name(), self.cardinality(), self.element_type())]
pub struct ParsedInputShapeElement<'a, 'b> {
    descriptors: &'b ParsedDescriptors<'a>,
    #[deref]
    descriptor: InputShapeElement<'a>,
}

impl<'a, 'b> ParsedInputShapeElement<'a, 'b> {
    pub fn element_type(&self) -> ParsedDescriptor<'a, 'b> {
        self.descriptors.get_descriptor(self.descriptor.type_pos)
    }
}

#[derive(derive_more::Debug, Clone, Copy, derive_more::Deref, derive_more::Constructor)]
#[debug("RangeType(id={}, name={:?}, schema_defined={}, element_type={:?})", self.id(), self.name(), self.schema_defined(), self.element_type())]
pub struct ParsedRangeTypeDescriptor<'a, 'b> {
    descriptors: &'b ParsedDescriptors<'a>,
    #[deref]
    descriptor: RangeTypeDescriptor<'a>,
}

impl<'a, 'b> ParsedRangeTypeDescriptor<'a, 'b> {
    pub fn element_type(&self) -> ParsedDescriptor<'a, 'b> {
        self.descriptors.get_descriptor(self.descriptor.type_pos)
    }

    pub fn ancestors(&self) -> impl Iterator<Item = ParsedDescriptor<'a, 'b>> {
        self.descriptor
            .ancestors
            .into_iter()
            .map(|index| self.descriptors.get_descriptor(index))
    }
}

#[derive(derive_more::Debug, Clone, Copy, derive_more::Deref, derive_more::Constructor)]
#[debug("ObjectType(id={}, name={:?}, schema_defined={})", self.id(), self.name(), self.schema_defined())]
pub struct ParsedObjectTypeDescriptor<'a, 'b> {
    #[expect(unused)]
    descriptors: &'b ParsedDescriptors<'a>,
    #[deref]
    descriptor: ObjectTypeDescriptor<'a>,
}

#[derive(derive_more::Debug, Clone, Copy, derive_more::Deref, derive_more::Constructor)]
#[debug("CompoundType(id={}, name={:?}, schema_defined={}, elements={:#?})", self.id(), self.name(), self.schema_defined(), self.components().collect::<Vec<_>>())]
pub struct ParsedCompoundTypeDescriptor<'a, 'b> {
    descriptors: &'b ParsedDescriptors<'a>,
    #[deref]
    descriptor: CompoundTypeDescriptor<'a>,
}

impl<'a, 'b> ParsedCompoundTypeDescriptor<'a, 'b> {
    pub fn op(&self) -> TypeOperation {
        self.descriptor.op
    }

    pub fn components(&self) -> impl Iterator<Item = ParsedDescriptor<'a, 'b>> {
        self.descriptor
            .components
            .into_iter()
            .map(|index| self.descriptors.get_descriptor(index))
    }
}

#[derive(derive_more::Debug, Clone, Copy, derive_more::Deref, derive_more::Constructor)]
#[debug("MultiRangeType(id={}, name={:?}, schema_defined={}, range_type={:?})", self.id(), self.name(), self.schema_defined(), self.range_type())]
pub struct ParsedMultiRangeTypeDescriptor<'a, 'b> {
    descriptors: &'b ParsedDescriptors<'a>,
    #[deref]
    descriptor: MultiRangeTypeDescriptor<'a>,
}

impl<'a, 'b> ParsedMultiRangeTypeDescriptor<'a, 'b> {
    pub fn range_type(&self) -> ParsedDescriptor<'a, 'b> {
        self.descriptors.get_descriptor(self.descriptor.type_pos)
    }

    pub fn ancestors(&self) -> impl Iterator<Item = ParsedDescriptor<'a, 'b>> {
        self.descriptor
            .ancestors
            .into_iter()
            .map(|index| self.descriptors.get_descriptor(index))
    }
}

#[derive(derive_more::Debug, Clone, Copy, derive_more::Deref, derive_more::Constructor)]
#[debug("SQLRecord(id={}, elements={:#?})", self.id(), self.elements().collect::<Vec<_>>())]
pub struct ParsedSQLRecordDescriptor<'a, 'b> {
    descriptors: &'b ParsedDescriptors<'a>,
    #[deref]
    descriptor: SQLRecordDescriptor<'a>,
}

impl<'a, 'b> ParsedSQLRecordDescriptor<'a, 'b> {
    pub fn elements(&self) -> impl Iterator<Item = (&'a str, ParsedDescriptor<'a, 'b>)> {
        self.descriptor.elements.into_iter().map(|index| {
            (
                index.name.to_str().unwrap_or_default(),
                self.descriptors.get_descriptor(index.type_pos),
            )
        })
    }
}

pub fn parse_descriptor<'a>(
    root: Uuid,
    mut data: &'a [u8],
) -> Result<ParsedDescriptors<'a>, ParseError> {
    let array: RestArray<'a, LengthPrefixed<Descriptor<'a>>> = RestArray::decode_for(&mut data)?;
    ParsedDescriptors::new(root, array)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn concat_bytes(bytes: &[&[u8]]) -> Vec<u8> {
        let mut result = Vec::new();
        for b in bytes {
            result.extend_from_slice(b);
        }
        result
    }

    #[test]
    fn test_parse_descriptor() {
        let buf = concat_bytes(&[
            b"\0\0\0 \x03\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x01\x01\0\0\0\x08",
            b"std::str\x01\0\0\0\0\0!\x03\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x01",
            b"\0\0\0\0\tstd::uuid\x01\0\0\0\0\0\"\n\xc3\xcc\xa7R\x95\xb7",
            b"\x11\xef\xb4\x87\x1d\x1b\x9f\xa20\x03\0\0\0\x0cdefault::Foo",
            b"\x01\0\0\0\"\n\r\xdc\xd7\x1e\x95\xb8\x11\xef\x82M!7\x80\\^4",
            b"\0\0\0\x0cdefault::Bar\x01\0\0\0^\x01\x1dMg\xe7{\xdd]9\x90\x97",
            b"O\x82\xfa\xd8\xaf7\0\0\x02\0\x04\0\0\0\x01A\0\0\0\t__tname__",
            b"\0\0\0\x02\0\0\0\0A\0\0\0\x02id\0\x01\0\x02\0\0\0\0o\0\0\0\x05",
            b"title\0\0\0\x02\0\0\0\0o\0\0\0\x04body\0\0\0\x03",
        ]);

        let root = Uuid::parse_str("1d4d67e7-7bdd-5d39-9097-4f82fad8af37").unwrap();
        let desc = parse_descriptor(root, &buf).unwrap();
        eprintln!("desc: {:#?}", desc);
    }

    #[test]
    fn test_parse_link_descriptor() {
        let buf = concat_bytes(&[
            b"\0\0\0!\x03\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x01\0\0\0\0\tstd::uuid\x01\0\0\0\0\0%\n\x85\xee\x02\xebQ\xee\x11\xf0\x8a\xa8\x83I\xab\xd9 ",
            b"\x92\0\0\0\x0fdefault::MyType\x01\0\0\0%\x01\0y\x18Hi4ZI\xa0\x89\xbch\xfc\x98A$\0\0\x01\0\x01\0\0\0\0A\0\0\0\x02id\0\0\0\x01"
        ]);
        let root = Uuid::parse_str("00791848-6934-5a49-a089-bc68fc984124").unwrap();
        let desc = parse_descriptor(root, &buf).unwrap();
        eprintln!("desc: {:#?}", desc);
    }

    #[test]
    fn test_parse_abstract_query() {
        let buf = concat_bytes(&[
            b"\0\0\0!\x03\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x01\0\0\0\0\tstd::uuid\x01\0\0\0\0\0$\n\xd4\x07\xba\xf6Q)\x11\xf0\xb8\x91\xe9q\x85@\x83\xd5\0\0\0\x0edefault::Media\x01\0\0\0%\x01\xdd/\x035c\rR\x92\x8e\xe7a\x06\xdc?<#\0\0\x01\0\x01\0\0\0\x01A\0\0\0\x02id\0\0\0\x01"
        ]);

        let root = Uuid::parse_str("dd2f0335-630d-5292-8ee7-6106dc3f3c23").unwrap();
        let desc = parse_descriptor(root, &buf).unwrap();
        eprintln!("desc: {:#?}", desc);
    }

    #[test]
    fn test_parse_scalar_descriptor() {
        let buf = concat_bytes(&[
            b"\0\0\0 \x03\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x01\x01\0\0\0\x08std::str\x01\0\0",
        ]);

        let root = Uuid::parse_str("00000000-0000-0000-0000-000000000101").unwrap();
        let desc = parse_descriptor(root, &buf).unwrap();
        eprintln!("desc: {:#?}", desc);
    }

    #[test]
    fn test_parse_array_tuple() {
        // <array<tuple<int16, str>>>
        let buf = concat_bytes(&[
            b"\0\0\0\"\x03\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x01\x03\0\0\0\nstd::int16\x01\0\0\0\0\0 \x03\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x01\x01\0\0\0\x08",
            b"std::str\x01\0\0\0\0\0\x37\x04\xee\x8d\xb7.\x13\xb0Z\xf1\xaa\x96T\xf6>\x96q\xe8\0\0\0\x19",
            b"tuple<std|int16, std|str>\0\0\0\0\x02\0\0\0\x01\0\0\0B\x06\x17\x83\xb0(F\xd0X\x98\xb7\x0c\x1cu\xcd\xa5\x1b\xef\0\0\0\"",
            b"array<tuple<std||int16, std||str>>\0\0\0\0\x02\0\x01\xff\xff\xff\xff",
        ]);

        let root = Uuid::parse_str("1783b028-46d0-5898-b70c-1c75cda51bef").unwrap();
        let desc = parse_descriptor(root, &buf).unwrap();
        eprintln!("desc: {:#?}", desc);
    }

    #[test]
    fn test_free_object() {
        let buf = concat_bytes(&[
            b"\0\0\0 \x03\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x01\x01\0\0\0\x08std::str\x01\0\0\0\0\0,\x01\xb5",
            b"\0\x7f\x19mPR\xcc\xa1\0\xcaN,B\xa4\x7f\x01\0\0\0\x01\0\0\0\0A\0\0\0\tmy_string\0\0\0\0"
        ]);

        let root = Uuid::parse_str("b5007f19-6d50-52cc-a100-ca4e2c42a47f").unwrap();
        let desc = parse_descriptor(root, &buf).unwrap();
        eprintln!("desc: {:#?}", desc);
    }

    #[test]
    fn test_ranges() {
        let buf = concat_bytes(&[
            b"\0\0\0\"\x03\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x01\x05\0\0\0\nstd::int64\x01\0\0\0\0\0/\x0c\x8bM1S",
            b"\xae\xe2\\\xc4\xae\xc1\xf1^ 3\x99B\0\0\0\x15multirange<std|int64>\x01\0\0\0\0",
        ]);

        let root = Uuid::parse_str("8b4d3153-aee2-5cc4-aec1-f15e20339942").unwrap();
        let desc = parse_descriptor(root, &buf).unwrap();
        eprintln!("desc: {:#?}", desc);
    }

    #[test]
    fn test_input_shape() {
        let buf = concat_bytes(&[
            b"\0\0\0\"\x03\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x01\x03\0\0\0\nstd::int16\x01\0\0\0\0\0,\x04\x8b\xbb",
            b"<\xa0Z\x91X?\x92\xc9\x86\x1f#\xfcX\xff\0\0\0\x10tuple<std|int16>\0\0\0\0\x01\0\0\0\0\08\x06\x8e",
            b"\x10Vcy\xd8Ts\x94$\x0c\x0c\x13\x07R\"\0\0\0\x18array<tuple<std||int16>>\0\0\0\0\x01\0\x01\xff\xff\xff\xff",
            b"\0\0\0$\x01g+\xd2\xbf\x05#W*\x9fz8\xf6\xe4:\x07\xa6\x01\0\0\0\x01\0\0\0\0A\0\0\0\x010\0\x02\0\0",
        ]);

        let root = Uuid::parse_str("672bd2bf-0523-572a-9f7a-38f6e43a07a6").unwrap();
        let desc = parse_descriptor(root, &buf).unwrap();
        eprintln!("desc: {:#?}", desc);
    }
}
