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

pub use arrays::{Array, ArrayExt, ArrayIter, RestArray, ZTArray};
pub use buffer::StructBuffer;
pub use datatypes::{Encoded, LString, Length, Rest, RestString, Uuid, ZTString};
pub use writer::BufWriter;

#[doc(inline)]
pub use gen::protocol;
#[doc(inline)]
pub use message_group::{match_message, message_group};

/// Re-export for the `protocol!` macro.
#[doc(hidden)]
pub use paste::paste;
#[doc(hidden)]
pub use type_mapper;

/// Ensures we can use the `gel-protogen-proc-macros` crate in this crate.
#[doc(hidden)]
extern crate self as gel_protogen;

pub mod prelude {
    pub use super::declare_meta;
    pub use super::declare_type;
    pub use super::make_static;
    pub use super::strip_lifetime;

    pub use super::encoding::BuilderFor;
    pub use super::encoding::DataType;
    pub use super::encoding::DataTypeFixedSize;
    pub use super::encoding::DecoderFor;
    pub use super::encoding::EncoderFor;
    pub use super::encoding::EncoderForExt;
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

    pub use super::arrays::*;
    pub use super::buffer::StructBuffer;
    pub use super::datatypes::*;

    pub use super::match_message;
    pub use super::message_group;
    pub use super::protocol;

    pub use gel_protogen_proc_macros::Protocol;
    pub use uuid::Uuid;
}

/// Compilation tests for proc macros.
#[cfg(test)]
mod tests {
    use super::prelude::Protocol;

    super::protocol!(
        struct MA<'a> {
            a: u8,
        }

        struct MB<'a> {
            b: u8,
        }

        struct MC<'a> {
            c: u8,
        }
    );

    #[derive(Copy, Clone, Protocol)]
    #[repr(u8)]
    enum TestEnum {
        A = 1,
    }

    #[derive(Copy, Clone, Protocol)]
    enum TestEnumChoice<'a> {
        A(MA<'a>),
        B(MB<'a>),
        C(MC<'a>),
    }
}
