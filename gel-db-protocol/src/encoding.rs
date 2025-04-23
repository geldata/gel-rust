use std::marker::PhantomData;

use uuid::Uuid;
use crate::declare_type;
use crate::prelude::*;
use crate::datatypes::*;

/// All data types must implement this trait. This allows for encoding and
/// decoding of the data type to byte buffers.
pub trait DataType where Self: Sized {
    const META: StructFieldMeta;
    /// Always a reference
    type BuilderForEncode: ?Sized;
    type BuilderForStruct<'unused>;
    type DecodeLifetime<'a>;
    
    fn decode<'a>(buf: &mut &'a [u8]) -> Result<Self::DecodeLifetime<'a>, ParseError>;
    fn encode<'a, 'b>(buf: &mut BufWriter<'a>, value: &'b Self::BuilderForEncode);
    #[allow(unused)] 
    fn encode_usize<'a>(buf: &mut BufWriter<'a>, value: usize) { unreachable!("encode usize") }
    #[allow(unused)] 
    fn decode_usize<'a>(buf: &mut &'a [u8]) -> Result<usize, ParseError> { unreachable!("decode usize") }
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

// declare_type!(ZTArray<'a, T>, builder: [T], flags: [array], 

//     fn decode(buf: &mut &[u8]) -> Result<ZTArray<'a, T>, ParseError> {
//         let orig_buf = *buf;
//         let mut len = 0;
//         loop {
//             if buf.is_empty() {
//                 return Err(ParseError::TooShort);
//             }
//             if buf[0] == 0 {
//                 break;
//             }
//             T::decode(buf)?;
//             len += 1;
//         }
//         Ok(ZTArray::new(orig_buf, len))
//     }

//     const fn encode(mut buf: EncodeTarget, value: &[T]) -> BufWriter {
//         for elem in *value {
//             buf = T::encode(buf, elem);
//         }
//         buf = buf.write_u8(0);
//         buf
//     }

// );

// declare_type!(Array<'a, L, T>, builder: [T], flags: [array],

//     fn decode(buf: &mut &[u8]) -> Result<Array<'a, L, T>, ParseError> {
//         let orig_buf = *buf;
//         let len = L::decode(buf)?;
//         let len = len.try_into()?;
//         for _ in 0..len {
//             T::decode(buf)?;
//         }
//         Ok(Array::new(orig_buf, len))
//     }

//     const fn encode(mut buf: EncodeTarget, value: &[T]) -> BufWriter {
//         buf = L::encode(buf, &value.len());
//         for elem in value {
//             buf = T::encode(buf, elem);
//         }
//         buf
//     }

// );

// declare_type!([T; L], flags: [array], 

//     fn decode(buf: &mut &[u8]) -> Result<Array<'a, L, T>, ParseError> {
//         let orig_buf = *buf;
//         let len = L::decode(buf)?;
//         let len = len.try_into()?;
//         for _ in 0..len {
//             T::decode(buf)?;
//         }
//         Ok(Array::new(orig_buf, len))
//     }

//     const fn encode(mut buf: EncodeTarget, value: &[T]) -> BufWriter {
//         buf = L::encode(buf, &value.len());
//         for elem in value {
//             buf = T::encode(buf, elem);
//         }
//         buf
//     }

// );

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
        Ok(ZTString::new(arr.into_slice()))
    }
    fn encode(buf: &mut BufWriter, value: &str) {
        ZTArray::<u8>::encode(buf, &value.as_bytes());
    }
});

declare_type!(DataType, Encoded<'a>, builder: Encoded<'a>, {
    fn decode(buf: &mut &[u8]) -> Result<Self, ParseError> {
        if let Some((len, array)) = buf.split_first_chunk::<4>() {
            let len = i32::from_be_bytes(*len);
            if len == -1 && array.is_empty() {
                Ok(Encoded::Null)
            } else if len < 0 {
                Err(ParseError::InvalidData)
            } else if array.len() < len as _ {
                Err(ParseError::TooShort)
            } else {
                Ok(Encoded::Value(array))
            }
        } else {
            Err(ParseError::TooShort)
        }
    }
    fn encode(buf: &mut BufWriter, value: &Encoded<'a>) {
        match value {
            Encoded::Null => buf.write(&[0xff, 0xff, 0xff, 0xff]),
            Encoded::Value(value) => {
                let len: i32 = value.len() as _;
                buf.write(&len.to_be_bytes());
                buf.write(value);
            }
        }
    }
});

    // Meta = EncodedMeta,
    // Inflated = Encoded<'a>,
    // Measure = Encoded<'a>,
    // Builder = Encoded<'a>,

    // pub const fn size_of_field_at(buf: &[u8]) -> Result<usize, ParseError> {
    //     const N: usize = std::mem::size_of::<i32>();
    //     if let Some(len) = buf.first_chunk::<N>() {
    //         let len = i32::from_be_bytes(*len);
    //         if len == -1 {
    //             Ok(N)
    //         } else if len < 0 {
    //             Err(ParseError::InvalidData)
    //         } else if buf.len() < len as usize + N {
    //             Err(ParseError::TooShort)
    //         } else {
    //             Ok(len as usize + N)
    //         }
    //     } else {
    //         Err(ParseError::TooShort)
    //     }
    // }

    // pub const fn extract(buf: &[u8]) -> Result<Encoded<'_>, ParseError> {
    //     const N: usize = std::mem::size_of::<i32>();
    //     if let Some((len, array)) = buf.split_first_chunk::<N>() {
    //         let len = i32::from_be_bytes(*len);
    //         if len == -1 && array.is_empty() {
    //             Ok(Encoded::Null)
    //         } else if len < 0 {
    //             Err(ParseError::InvalidData)
    //         } else if array.len() < len as _ {
    //             Err(ParseError::TooShort)
    //         } else {
    //             Ok(Encoded::Value(array))
    //         }
    //     } else {
    //         Err(ParseError::TooShort)
    //     }
    // }

    // pub const fn measure(value: &Encoded) -> usize {
    //     match value {
    //         Encoded::Null => std::mem::size_of::<i32>(),
    //         Encoded::Value(value) => value.len() + std::mem::size_of::<i32>(),
    //     }
    // }

    // pub fn copy_to_buf(buf: &mut BufWriter, value: &Encoded) {
    //     match value {
    //         Encoded::Null => buf.write(&[0xff, 0xff, 0xff, 0xff]),
    //         Encoded::Value(value) => {
    //             let len: i32 = value.len() as _;
    //             buf.write(&len.to_be_bytes());
    //             buf.write(value);
    //         }
    //     }
    // }

    // pub const fn constant(_constant: usize) -> Encoded<'static> {
    //     panic!("Constants unsupported for this data type")
    // }


declare_type!(DataType, Length, flags=[length], {
    fn decode(buf: [u8; 4]) -> Result<Self, ParseError> {
        Ok(Self(i32::from_be_bytes(buf)))
    }
    fn encode(value: u32) -> [u8; 4] {
        value.0.to_be_bytes()
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
