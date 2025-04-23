use std::marker::PhantomData;

use crate::datatypes::*;
use crate::declare_type;
use crate::prelude::*;
use uuid::Uuid;

/// All data types must implement this trait. This allows for encoding and
/// decoding of the data type to byte buffers.
pub trait DataType
where
    Self: Sized,
{
    const META: StructFieldMeta;
    /// Always a reference
    type BuilderForEncode;
    type BuilderForStruct<'unused>;
    type DecodeLifetime<'a>;

    fn decode<'a>(buf: &mut &'a [u8]) -> Result<Self::DecodeLifetime<'a>, ParseError>;
    fn encode<'a, 'b>(buf: &mut BufWriter<'a>, value: &'b Self::BuilderForEncode);
    #[allow(unused)]
    fn encode_usize<'a>(buf: &mut BufWriter<'a>, value: usize) {
        unreachable!("encode usize")
    }
    #[allow(unused)]
    fn decode_usize<'a>(buf: &mut &'a [u8]) -> Result<usize, ParseError> {
        unreachable!("decode usize")
    }
}

pub trait DataTypeFixedSize {
    const SIZE: usize;
}

#[derive(thiserror::Error, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    #[error("Buffer is too short")]
    TooShort,
    #[error("Invalid data")]
    InvalidData,
}

pub struct EncodeTarget<'a> {
    _phantom: PhantomData<&'a ()>,
}

impl<'a, L: DataType, T: DataType> DataType for Array<'a, L, T>
where
    T::BuilderForEncode: 'a,
    T::BuilderForStruct<'a>: 'a,
{
    const META: StructFieldMeta = declare_meta!(
        type = Array,
        constant_size = None,
        flags = []
    );
    type BuilderForEncode = &'a [T::BuilderForEncode];
    type BuilderForStruct<'unused> = &'a [T::BuilderForStruct<'a>];
    type DecodeLifetime<'__next_lifetime> =
        Array<'__next_lifetime, L, T::DecodeLifetime<'__next_lifetime>>;

    fn decode<'__next_lifetime>(
        buf: &mut &'__next_lifetime [u8],
    ) -> Result<Self::DecodeLifetime<'__next_lifetime>, ParseError> {
        let len = L::decode_usize(buf)?;
        let orig_buf = *buf;
        for _ in 0..len {
            T::decode(buf)?;
        }
        Ok(Array::new(orig_buf, len as _))
    }

    fn encode<'__buffer_lifetime, '__value_lifetime>(
        buf: &mut BufWriter<'__buffer_lifetime>,
        value: &'__value_lifetime Self::BuilderForEncode,
    ) {
        L::encode_usize(buf, value.len());
        for elem in value.iter() {
            T::encode(buf, elem);
        }
    }
}

impl<'a, T: DataType> DataType for ZTArray<'a, T>
where
    T::BuilderForEncode: 'a,
    T::BuilderForStruct<'a>: 'a,
{
    const META: StructFieldMeta = declare_meta!(
        type = ZTArray,
        constant_size = None,
        flags = []
    );
    type BuilderForEncode = &'a [T::BuilderForEncode];
    type BuilderForStruct<'unused> = &'a [T::BuilderForStruct<'a>];
    type DecodeLifetime<'__next_lifetime> =
        ZTArray<'__next_lifetime, T::DecodeLifetime<'__next_lifetime>>;

    fn decode<'__next_lifetime>(
        buf: &mut &'__next_lifetime [u8],
    ) -> Result<Self::DecodeLifetime<'__next_lifetime>, ParseError> {
        let mut orig_buf = *buf;
        let mut len = 0;
        loop {
            if buf.is_empty() {
                return Err(crate::prelude::ParseError::TooShort);
            }
            if buf[0] == 0 {
                orig_buf = &orig_buf[0..orig_buf.len() - buf.len() + 1];
                *buf = &buf[1..];
                break;
            }
            T::decode(buf)?;
            len += 1;
        }
        Ok(ZTArray::new(orig_buf, len))
    }

    fn encode<'__buffer_lifetime, '__value_lifetime>(
        buf: &mut BufWriter<'__buffer_lifetime>,
        value: &'__value_lifetime Self::BuilderForEncode,
    ) {
        for elem in value.iter() {
            T::encode(buf, elem);
        }
        buf.write(&[0]);
    }
}

impl<const N: usize, T: DataType> DataType for [T; N]
where
    for<'a> T::DecodeLifetime<'a>: Default + Copy,
{
    const META: StructFieldMeta = declare_meta!(
        type = FixedArray,
        constant_size = Some(std::mem::size_of::<T>() * N),
        flags = []
    );
    type BuilderForStruct<'unused> = [T::BuilderForStruct<'unused>; N];
    type BuilderForEncode = [T::BuilderForEncode; N];
    type DecodeLifetime<'__next_lifetime> = [T::DecodeLifetime<'__next_lifetime>; N];
    fn decode<'__next_lifetime>(
        buf: &mut &'__next_lifetime [u8],
    ) -> Result<Self::DecodeLifetime<'__next_lifetime>, crate::prelude::ParseError> {
        let mut res = [T::DecodeLifetime::<'__next_lifetime>::default(); N];
        for i in 0..N {
            res[i] = T::decode(buf)?;
        }
        Ok(res)
    }
    fn encode<'__buffer_lifetime, '__value_lifetime>(
        buf: &mut crate::prelude::BufWriter<'__buffer_lifetime>,
        value: &'__value_lifetime Self::BuilderForEncode,
    ) {
        for elem in value {
            T::encode(buf, elem);
        }
    }
}

declare_type!(DataType, Rest<'a>, builder: &'a [u8],
{
    fn decode(buf: &mut &[u8]) -> Result<Self, ParseError> {
        let res = Rest::new(buf);
        *buf = &[];
        Ok(res)
    }

    fn encode(buf: &mut BufWriter, value: &[u8]) {
        buf.write(value)
    }
}
);

declare_type!(DataType, LString<'a>, builder: &'a str, {
    fn decode(buf: &mut &[u8]) -> Result<Self, ParseError> {
        let arr = Array::<u32, u8>::decode(buf)?;
        Ok(LString::new(arr.into_slice()))
    }
    fn encode(buf: &mut BufWriter, value: &str) {
        Array::<u32, u8>::encode(buf, &value.as_bytes());
    }
});
declare_type!(DataType, ZTString<'a>, builder: &'a str, {
    fn decode(buf: &mut &[u8]) -> Result<Self, ParseError> {
        let arr = ZTArray::<u8>::decode(buf)?;
        let slice = arr.into_slice();
        Ok(ZTString::new(&slice[0..slice.len() - 1]))
    }
    fn encode(buf: &mut BufWriter, value: &str) {
        ZTArray::<u8>::encode(buf, &value.as_bytes());
    }
});

declare_type!(DataType, Encoded<'a>, builder: Encoded<'a>, {
    fn decode(buf: &mut &[u8]) -> Result<Self, ParseError> {
        if let Some((len, array)) = buf.split_first_chunk::<4>() {
            let len = i32::from_be_bytes(*len);
            if len == -1 {
                *buf = array;
                Ok(Encoded::Null)
            } else if len < 0 {
                Err(ParseError::InvalidData)
            } else if array.len() < len as _ {
                Err(ParseError::TooShort)
            } else {
                *buf = &array[len as usize..];
                Ok(Encoded::Value(&array[..len as usize]))
            }
        } else {
            Err(ParseError::TooShort)
        }
    }
    fn encode(buf: &mut BufWriter, value: &Encoded<'a>) {
        match value {
            Encoded::Null => buf.write(&(-1_i32).to_be_bytes()),
            Encoded::Value(value) => {
                let len: i32 = value.len() as _;
                buf.write(&len.to_be_bytes());
                buf.write(value);
            }
        }
    }
});

declare_type!(DataType, Length, flags = [length], {
    fn decode(buf: [u8; 4]) -> Result<Self, ParseError> {
        Ok(Self(i32::from_be_bytes(buf)))
    }
    fn encode(value: u32) -> [u8; 4] {
        value.0.to_be_bytes()
    }
    fn to_usize(value: usize) -> Length {
        Length(value as _)
    }
    fn from_usize(value: Length) -> usize {
        value.0 as usize
    }
});

declare_type!(DataType, Uuid, {
    fn decode(buf: [u8; 16]) -> Result<Self, ParseError> {
        Ok(Uuid::from_bytes(buf))
    }
    fn encode(value: Uuid) -> [u8; 16] {
        value.into_bytes()
    }
});

declare_type!(u8);
declare_type!(u16);
declare_type!(u32);
declare_type!(u64);
declare_type!(i8);
declare_type!(i16);
declare_type!(i32);
declare_type!(i64);

declare_type!(f32);
declare_type!(f64);
