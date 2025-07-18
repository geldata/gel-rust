use std::error::Error;
use std::str;

use snafu::{Backtrace, IntoError, Snafu};

use crate::value::Value;

#[derive(Snafu, Debug)]
#[snafu(visibility(pub), context(suffix(false)))]
#[non_exhaustive]
pub enum DecodeError {
    #[snafu(display("unexpected end of frame"))]
    Underflow { backtrace: Backtrace },
    #[snafu(display("frame contains extra data after decoding"))]
    ExtraData { backtrace: Backtrace },
    #[snafu(display("invalid utf8 when decoding string: {}", source))]
    InvalidUtf8 {
        backtrace: Backtrace,
        source: str::Utf8Error,
    },
    #[snafu(display("invalid auth status: {:x}", auth_status))]
    AuthStatusInvalid {
        backtrace: Backtrace,
        auth_status: u32,
    },
    #[snafu(display("unsupported transaction state: {:x}", transaction_state))]
    InvalidTransactionState {
        backtrace: Backtrace,
        transaction_state: u8,
    },
    #[snafu(display("unsupported io format: {:x}", io_format))]
    InvalidIoFormat { backtrace: Backtrace, io_format: u8 },
    #[snafu(display("unsupported cardinality: {:x}", cardinality))]
    InvalidCardinality {
        backtrace: Backtrace,
        cardinality: u8,
    },
    #[snafu(display("unsupported input language: {:x}", input_language))]
    InvalidInputLanguage {
        backtrace: Backtrace,
        input_language: u8,
    },
    #[snafu(display("unsupported capability: {:b}", capabilities))]
    InvalidCapabilities {
        backtrace: Backtrace,
        capabilities: u64,
    },
    #[snafu(display("unsupported compilation flags: {:b}", compilation_flags))]
    InvalidCompilationFlags {
        backtrace: Backtrace,
        compilation_flags: u64,
    },
    #[snafu(display("unsupported dump flags: {:b}", dump_flags))]
    InvalidDumpFlags {
        backtrace: Backtrace,
        dump_flags: u64,
    },
    #[snafu(display("unsupported describe aspect: {:x}", aspect))]
    InvalidAspect { backtrace: Backtrace, aspect: u8 },
    #[snafu(display("unsupported type descriptor: {:x}", descriptor))]
    InvalidTypeDescriptor {
        backtrace: Backtrace,
        descriptor: u8,
    },
    #[snafu(display("invalid uuid: {}", source))]
    InvalidUuid {
        backtrace: Backtrace,
        source: uuid::Error,
    },
    #[snafu(display("non-zero reserved bytes received in data"))]
    NonZeroReservedBytes { backtrace: Backtrace },
    #[snafu(display("object data size does not match its shape"))]
    ObjectSizeMismatch { backtrace: Backtrace },
    #[snafu(display("tuple size does not match its shape"))]
    TupleSizeMismatch { backtrace: Backtrace },
    #[snafu(display("unknown negative length marker"))]
    InvalidMarker { backtrace: Backtrace },
    #[snafu(display("array shape for the Set codec is invalid"))]
    InvalidSetShape { backtrace: Backtrace },
    #[snafu(display("array shape is invalid"))]
    InvalidArrayShape { backtrace: Backtrace },
    #[snafu(display("array or set shape is invalid"))]
    InvalidArrayOrSetShape { backtrace: Backtrace },
    #[snafu(display("decimal or bigint sign bytes have invalid value"))]
    BadSign { backtrace: Backtrace },
    #[snafu(display("invalid boolean value: {val:?}"))]
    InvalidBool { backtrace: Backtrace, val: u8 },
    #[snafu(display("invalid optional u32 value"))]
    InvalidOptionU32 { backtrace: Backtrace },
    #[snafu(display("datetime is out of range"))]
    InvalidDate { backtrace: Backtrace },
    #[snafu(display("json format is invalid"))]
    InvalidJsonFormat { backtrace: Backtrace },
    #[snafu(display("enum value returned is not in type descriptor"))]
    ExtraEnumValue { backtrace: Backtrace },
    #[snafu(display("too may descriptors ({})", index))]
    TooManyDescriptors { backtrace: Backtrace, index: usize },
    #[snafu(display("invalid index in input shape ({})", index))]
    InvalidIndex { backtrace: Backtrace, index: usize },
    #[snafu(display("uuid {} not found", uuid))]
    UuidNotFound {
        backtrace: Backtrace,
        uuid: uuid::Uuid,
    },
    #[snafu(display("error decoding value"))]
    DecodeValue {
        backtrace: Backtrace,
        source: Box<dyn Error + Send + Sync>,
    },
    #[snafu(display("missing required link or property"))]
    MissingRequiredElement { backtrace: Backtrace },
    #[snafu(display("invalid format of {annotation} annotation"))]
    InvalidAnnotationFormat {
        backtrace: Backtrace,
        annotation: &'static str,
    },
    #[snafu(display("invalid type operation value"))]
    InvalidTypeOperation { backtrace: Backtrace },
}

#[derive(Snafu, Debug)]
#[snafu(visibility(pub(crate)), context(suffix(false)))]
#[non_exhaustive]
pub enum EncodeError {
    #[snafu(display("message doesn't fit 4GiB"))]
    MessageTooLong { backtrace: Backtrace },
    #[snafu(display("string is larger than 64KiB"))]
    StringTooLong { backtrace: Backtrace },
    #[snafu(display("more than 64Ki extensions"))]
    TooManyExtensions { backtrace: Backtrace },
    #[snafu(display("more than 64Ki headers"))]
    TooManyHeaders { backtrace: Backtrace },
    #[snafu(display("more than 64Ki params"))]
    TooManyParams { backtrace: Backtrace },
    #[snafu(display("more than 64Ki attributes"))]
    TooManyAttributes { backtrace: Backtrace },
    #[snafu(display("more than 64Ki authentication methods"))]
    TooManyMethods { backtrace: Backtrace },
    #[snafu(display("more than 4Gi elements in the object"))]
    TooManyElements { backtrace: Backtrace },
    #[snafu(display("single element larger than 4Gi"))]
    ElementTooLong { backtrace: Backtrace },
    #[snafu(display("array or set has more than 4Gi elements"))]
    ArrayTooLong { backtrace: Backtrace },
    #[snafu(display("bigint has more than 256Ki digits"))]
    BigIntTooLong { backtrace: Backtrace },
    #[snafu(display("decimal has more than 256Ki digits"))]
    DecimalTooLong { backtrace: Backtrace },
    #[snafu(display("unknown message types cannot be encoded"))]
    UnknownMessageCantBeEncoded { backtrace: Backtrace },
    #[snafu(display(
        "trying to encode invalid value type {} with codec {}",
        value_type,
        codec
    ))]
    InvalidValue {
        backtrace: Backtrace,
        value_type: &'static str,
        codec: &'static str,
    },
    #[snafu(display("shape of data does not match shape of encoder"))]
    ObjectShapeMismatch { backtrace: Backtrace },
    #[snafu(display("datetime value is out of range"))]
    DatetimeRange { backtrace: Backtrace },
    #[snafu(display("tuple size doesn't match encoder"))]
    TupleShapeMismatch { backtrace: Backtrace },
    #[snafu(display("enum value is not in type descriptor"))]
    MissingEnumValue { backtrace: Backtrace },
}

impl From<crate::new_protocol::prelude::ParseError> for DecodeError {
    fn from(e: crate::new_protocol::prelude::ParseError) -> Self {
        match e {
            crate::new_protocol::prelude::ParseError::TooShort(_) => DecodeError::Underflow {
                backtrace: Backtrace::capture(),
            },
            e => DecodeError::DecodeValue {
                backtrace: Backtrace::capture(),
                source: Box::new(e),
            },
        }
    }
}

#[derive(Snafu, Debug)]
#[snafu(visibility(pub(crate)), context(suffix(false)))]
#[non_exhaustive]
pub enum CodecError {
    #[snafu(display("type position {} is absent", position))]
    UnexpectedTypePos { backtrace: Backtrace, position: u16 },
    #[snafu(display("base scalar with uuid {} not found", uuid))]
    UndefinedBaseScalar {
        backtrace: Backtrace,
        uuid: uuid::Uuid,
    },
}

pub fn invalid_value(codec: &'static str, value: &Value) -> EncodeError {
    InvalidValue {
        codec,
        value_type: value.kind(),
    }
    .build()
}

pub fn decode_error<E: Error + Send + Sync + 'static>(e: E) -> DecodeError {
    DecodeValue.into_error(Box::new(e))
}
