mod arrays;
mod buffer;
mod datatypes;
mod encoding;
mod gen;
pub mod gen2;
mod message_group;
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
    
    pub use super::gen2::StructFieldMeta;
    pub use super::gen2::StructLength;

    pub use super::declare_meta;

    pub use super::datatypes::*;
    pub use super::arrays::*;
}
