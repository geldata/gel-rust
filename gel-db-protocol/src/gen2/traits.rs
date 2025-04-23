use std::mem::MaybeUninit;

use crate::prelude::*;

pub trait EnumMeta {}

/// A trait for structs that describes their fields, containing some associated
/// constants. Note that only `FIELDS` is provided. The remainder of the
/// associated constants are computed from `FIELDS`.
pub trait StructMeta: Copy + Clone + std::fmt::Debug {
    type Struct<'a>: Copy + Clone + std::fmt::Debug;
    const FIELDS: StructFields;
    const FIELD_COUNT: usize = Self::FIELDS.fields.len();
    const IS_FIXED_SIZE: bool = Self::FIELDS.constant_size().is_some();
    const FIXED_SIZE: Option<usize> = Self::FIELDS.constant_size();
    const LENGTH_FIELD_INDEX: Option<usize> = Self::FIELDS.length_field_fixed_offset();
    const HAS_LENGTH_FIELD: bool = Self::LENGTH_FIELD_INDEX.is_some();

    fn new<'a>(buf: &'a [u8]) -> Result<Self::Struct<'a>, ParseError>;
    fn to_vec(&self) -> Vec<u8>;
}

/// A trait implemented for all structs with a boolean determining whether they
/// are fixed size.
///
/// NOTE: A generic parameter is used here which allows for optional further
/// trait implementations.
pub trait StructAttributeFixedSize<const IS_FIXED_SIZE: bool>: StructMeta {}

/// A trait implemented for all structs with a boolean determining whether they
/// have a length field.
///
/// NOTE: A generic parameter is used here which allows for optional further
/// trait implementations.
pub trait StructAttributeHasLengthField<const HAS_LENGTH_FIELD: bool>: StructMeta {}

/// A trait implemented for all structs with a count of fields.
///
/// NOTE: A generic parameter is used here which allows for optional further
/// trait implementations.
pub trait StructAttributeFieldCount<const FIELD_COUNT: usize>: StructMeta {}

#[derive(Clone, Copy, Debug)]
pub struct StructField {
    /// The name of the field in the struct.
    pub name: &'static str,
    /// The metadata for the field.
    pub meta: &'static StructFieldMeta,
    /// The value of the field, if it is a constant.
    pub value: Option<usize>,
}

/// A struct that contains metadata about a field's type.
#[derive(Clone, Copy, Debug, Default)]
pub struct StructFieldMeta {
    pub type_name: &'static str,
    pub constant_size: Option<usize>,
    pub is_length: bool,
    pub is_enum: bool,
    pub is_struct: bool,
}

impl StructFieldMeta {
    pub const fn new(type_name: &'static str, constant_size: Option<usize>) -> Self {
        Self {
            type_name,
            constant_size,
            is_length: false,
            is_enum: false,
            is_struct: false,
        }
    }

    pub const fn set_length(self) -> Self {
        Self {
            is_length: true,
            ..self
        }
    }

    pub const fn set_enum(self) -> Self {
        Self {
            is_enum: true,
            ..self
        }
    }

    pub const fn set_struct(self) -> Self {
        Self {
            is_struct: true,
            ..self
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StructFieldComputed {
    pub field: StructField,
    pub fixed_offset: Option<usize>,
}

impl StructFieldComputed {
    pub const fn new<const N: usize>(fields: [StructField; N]) -> [Self; N] {
        // TODO: There's no way to prove to the compiler that the array is
        // fully initialized yet so we dip into `unsafe`.
        let mut out: [MaybeUninit<Self>; N] = [const { MaybeUninit::uninit() }; N];
        let mut i = 0;
        let mut fixed_offset = Some(0);
        while i < N {
            out[i] = MaybeUninit::new(Self {
                field: fields[i],
                fixed_offset,
            });
            if let Some(fixed_offset_value) = fixed_offset {
                if let Some(size) = fields[i].meta.constant_size {
                    fixed_offset = Some(fixed_offset_value + size);
                } else {
                    fixed_offset = None;
                }
            } else {
                fixed_offset = None;
            }
            i += 1;
        }
        // SAFETY: This whole array is initialized. This can be replaced with
        // https://doc.rust-lang.org/std/mem/union.MaybeUninit.html#method.array_assume_init
        // once stable.
        unsafe { out.as_ptr().cast::<[Self; N]>().read() }
    }
}

pub struct StructFields {
    pub fields: &'static [StructFieldComputed],
    pub constant_size: Option<usize>,
}

impl StructFields {
    pub const fn new(fields: &'static [StructFieldComputed]) -> Self {
        let constant_size = Self::compute_constant_size(fields);
        Self {
            fields,
            constant_size,
        }
    }

    const fn compute_constant_size(fields: &'static [StructFieldComputed]) -> Option<usize> {
        let mut i = 0;
        let mut size = 0;
        while i < fields.len() {
            if let Some(constant_size) = fields[i].field.meta.constant_size {
                size += constant_size;
            } else {
                return None;
            }
            i += 1;
        }
        Some(size)
    }

    pub const fn field_by_name(&self, name: &str) -> Option<usize> {
        let mut i = 0;
        while i < self.fields.len() {
            if const_str::equal!(self.fields[i].field.name, name) {
                return Some(i);
            }
            i += 1;
        }
        None
    }

    pub const fn field_by_index(&self, i: usize) -> &StructField {
        &self.fields[i].field
    }

    pub const fn field_fixed_offset(&self, field_index: usize) -> Option<usize> {
        self.fields[field_index].fixed_offset
    }

    pub const fn length_field_fixed_offset(&self) -> Option<usize> {
        let mut i = 0;
        while i < self.fields.len() {
            if self.fields[i].field.meta.is_length {
                return Some(i);
            }
            i += 1;
        }
        None
    }

    pub const fn constant_size(&self) -> Option<usize> {
        self.constant_size
    }

    pub const fn matches_field_constants(&self, buf: &[u8]) -> bool {
        let mut i = 0;
        while i < self.fields.len() {
            if let Some(value) = self.fields[i].field.value {
                if let Some(fixed_offset) = self.fields[i].fixed_offset {
                    if let Some(constant_size) = self.fields[i].field.meta.constant_size {
                        let buf = buf.split_at(fixed_offset).1;
                        if buf.len() < constant_size {
                            return false;
                        }
                        let buf = buf.split_at(constant_size).0;
                        match constant_size {
                            1 => {
                                if buf[0] != value as u8 {
                                    return false;
                                }
                            }
                            2 => {
                                if value
                                    != u16::from_be_bytes(*buf.split_first_chunk::<2>().unwrap().0)
                                        as _
                                {
                                    return false;
                                }
                            }
                            4 => {
                                if value
                                    != u32::from_be_bytes(*buf.split_first_chunk::<4>().unwrap().0)
                                        as _
                                {
                                    return false;
                                }
                            }
                            8 => {
                                if value
                                    != u64::from_be_bytes(*buf.split_first_chunk::<8>().unwrap().0)
                                        as _
                                {
                                    return false;
                                }
                            }
                            16 => {
                                if value
                                    != u128::from_be_bytes(
                                        *buf.split_first_chunk::<16>().unwrap().0,
                                    ) as _
                                {
                                    return false;
                                }
                            }
                            _ => panic!("Unsupported constant size for field"),
                        }
                    }
                }
            }
            i += 1;
        }
        true
    }
}

impl<T: StructAttributeFixedSize<true>> DataTypeFixedSize for T {
    const SIZE: usize = T::FIXED_SIZE.unwrap();
}

impl<T: StructAttributeHasLengthField<true> + StructMeta> StructLength for T {
    fn length_of_buf(buf: &[u8]) -> Option<usize> {
        let index = T::LENGTH_FIELD_INDEX.unwrap();
        if buf.len() < index + std::mem::size_of::<u32>() {
            None
        } else {
            let len =
                <u32 as DataType>::decode(&mut &buf[index..index + std::mem::size_of::<u32>()])
                    .ok()?;
            Some(index + len as usize)
        }
    }
}

/// Implemented for all generated structs that have a [`meta::Length`] field at a fixed offset.
pub trait StructLength: StructMeta {
    fn length_of_buf(buf: &[u8]) -> Option<usize>;
}
