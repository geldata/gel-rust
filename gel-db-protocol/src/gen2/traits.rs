use std::mem::MaybeUninit;

use crate::datatypes::LengthMeta;
use crate::{FieldAccess, FieldAccessArray, FixedSize, ParseError, StructLength};

pub trait EnumMeta {}

/// A trait for structs that describes their fields, containing some associated
/// constants. Note that only `FIELDS` is provided. The remainder of the
/// associated constants are computed from `FIELDS`.
pub trait StructMeta: Clone + Copy {
    const FIELDS: StructFields;
    const FIELD_COUNT: usize = Self::FIELDS.fields.len();
    const IS_FIXED_SIZE: bool = Self::FIELDS.constant_size().is_some();
    const FIXED_SIZE: Option<usize> = Self::FIELDS.constant_size();
    const LENGTH_FIELD_INDEX: Option<usize> = Self::FIELDS.length_field_fixed_offset();
    const HAS_LENGTH_FIELD: bool = Self::LENGTH_FIELD_INDEX.is_some();
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
    /// A function that returns the size of the field at a given offset in the buffer.
    pub size_of_field_at: fn(&[u8]) -> Result<usize, ParseError>,
}

/// Each type in the system implements this trait, providing a
/// [`StructFieldMeta`].
pub trait HasStructFieldMeta {
    const META: StructFieldMeta;
}

/// A struct that contains metadata about a field's type.
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

    pub fn compute_field_ends_from_buf<const FIELD_COUNT: usize>(
        &self,
        mut buf: &[u8],
    ) -> Result<[usize; FIELD_COUNT], ParseError> {
        let mut out = [0; FIELD_COUNT];
        debug_assert!(self.fields.len() <= FIELD_COUNT, "Field count mismatch");
        let mut offset = 0;
        let mut index = 0;
        for field in self.fields {
            let field_size = (field.field.size_of_field_at)(buf)?;
            offset += field_size;
            out[index] = offset;
            buf = buf.split_at(field_size).1;
            index += 1;
        }

        Ok(out)
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

#[derive(Debug, Copy, Clone)]
pub struct StructDemarcatedBuffer<'a, T: StructMeta, const FIELD_COUNT: usize> {
    buf: &'a [u8],
    field_ends: [usize; FIELD_COUNT],
    _marker: std::marker::PhantomData<T>,
}

impl<'a, T: StructMeta, const FIELD_COUNT: usize> StructDemarcatedBuffer<'a, T, FIELD_COUNT> {
    pub fn new(buf: &'a [u8]) -> Result<Self, ParseError> {
        let field_ends = <T as StructMeta>::FIELDS.compute_field_ends_from_buf(buf)?;
        Ok(Self {
            buf,
            field_ends,
            _marker: std::marker::PhantomData,
        })
    }
}

impl<'a, T: StructMeta, const FIELD_COUNT: usize> StructDemarcatedBuffer<'a, T, FIELD_COUNT> {
    pub fn eq(&self, other: &Self) -> bool {
        self.buf == other.buf
    }

    pub fn to_vec(self) -> Vec<u8> {
        self.buf.to_vec()
    }

    pub const fn as_ref(&self) -> &'a [u8] {
        self.buf
    }

    pub const fn extract_field(&self, field_index: usize) -> &[u8] {
        let field_start = if field_index == 0 {
            0
        } else {
            self.field_ends[field_index - 1]
        };
        let field_end = self.field_ends[field_index];
        let buf = self.buf.split_at(field_start).1;
        let buf = buf.split_at(field_end - field_start).0;
        buf
    }
}
