use std::mem::MaybeUninit;

use crate::datatypes::LengthMeta;
use crate::{FieldAccess, FieldAccessArray, FixedSize, ParseError, StructLength};

pub trait EnumMeta {}

/// A trait for structs that describes their fields, containing some associated
/// constants.
pub trait StructMeta {
    const FIELDS: StructFields;
    const FIELD_COUNT: usize = Self::FIELDS.fields.len();
    const IS_FIXED_SIZE: bool = Self::FIELDS.constant_size().is_some();
    const FIXED_SIZE: Option<usize> = Self::FIELDS.constant_size();
    const LENGTH_FIELD_INDEX: Option<usize> = Self::FIELDS.length_field_fixed_offset();
    const HAS_LENGTH_FIELD: bool = Self::LENGTH_FIELD_INDEX.is_some();
}

/// A trait with generic constant parameters that can be used to optionally
/// implement further traits.
pub trait StructAttributeFixedSize<const IS_FIXED_SIZE: bool>: StructMeta {}

pub trait StructAttributeHasLengthField<const HAS_LENGTH_FIELD: bool>: StructMeta {}

#[derive(Clone, Copy, Debug)]
pub struct StructField {
    pub name: &'static str,
    pub meta: &'static StructFieldMeta,
    pub size_of_field_at: fn(&[u8]) -> Result<usize, ParseError>,
}

pub trait HasStructFieldMeta {
    const META: StructFieldMeta;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct StructFieldMeta {
    pub type_name: &'static str,
    pub constant_size: Option<usize>,
    pub is_length: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct StructFieldComputed {
    pub field: StructField,
    pub fixed_offset: Option<usize>,
}

impl StructFieldComputed {
    pub const fn new<const N: usize>(fields: [StructField; N]) -> [Self; N] {
        let mut out: [MaybeUninit<Self>; N] = [const { MaybeUninit::uninit() }; N];
        let mut i = 0;
        while i < N {
            out[i] = MaybeUninit::new(Self {
                field: fields[i],
                fixed_offset: None,
            });
            i += 1;
        }
        // SAFETY: This whole array is initialized
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

    pub fn compute_size(&self, mut buf: &[u8]) -> Result<usize, ParseError> {
        if let Some(constant_size) = self.constant_size {
            if buf.len() < constant_size {
                return Err(ParseError::TooShort);
            }
            return Ok(constant_size);
        }
        let mut size = 0;
        for field in self.fields {
            let field_size = (field.field.size_of_field_at)(buf)?;
            buf = buf.split_at(field_size).1;
            size += field_size;
        }
        Ok(size)
    }
}

impl<T: StructAttributeFixedSize<true> + FieldAccessArray> FixedSize for T {
    const SIZE: usize = T::FIXED_SIZE.unwrap();
    #[inline(always)]
    fn extract_infallible(buf: &[u8]) -> T::WithLifetime<'_> {
        T::extract(buf).unwrap()
    }
}

impl<T: StructAttributeHasLengthField<true> + crate::StructMeta> StructLength for T {
    fn length_of_buf(buf: &[u8]) -> Option<usize> {
        let index = T::LENGTH_FIELD_INDEX.unwrap();
        if buf.len() < index + std::mem::size_of::<u32>() {
            None
        } else {
            let len =
                FieldAccess::<LengthMeta>::extract(&buf[index..index + std::mem::size_of::<u32>()])
                    .ok()?;
            Some(index + len)
        }
    }
}
