mod arrays;
mod buffer;
mod datatypes;
mod encoding;
mod gen;
mod macros;
mod message_group;
mod structs;
mod writer;

#[doc(hidden)]
pub mod test_protocol;

pub use arrays::{Array, ArrayIter, ZTArray, ZTArrayIter};
pub use buffer::StructBuffer;
pub use datatypes::{Encoded, LString, Length, Rest, Uuid, ZTString};
pub use writer::BufWriter;

#[doc(inline)]
pub use gen::protocol;
#[doc(inline)]
pub use message_group::{match_message, message_group};

/// Re-export for the `protocol!` macro.
#[doc(hidden)]
pub use paste::paste;

pub mod prelude {
    pub use super::encoding::DataType;
    pub use super::encoding::DataTypeFixedSize;
    pub use super::encoding::EncodeTarget;
    pub use super::encoding::ParseError;
    pub use super::writer::BufWriter;

    pub use super::structs::EnumMeta;

    pub use super::structs::StructAttributeFieldCount;
    pub use super::structs::StructAttributeFixedSize;
    pub use super::structs::StructAttributeHasLengthField;
    pub use super::structs::StructField;
    pub use super::structs::StructFieldComputed;
    pub use super::structs::StructFieldMeta;
    pub use super::structs::StructFields;
    pub use super::structs::StructLength;
    pub use super::structs::StructMeta;

    pub use super::declare_meta;

    pub use super::arrays::*;
    pub use super::buffer::StructBuffer;
    pub use super::datatypes::*;

    pub use super::match_message;
}
