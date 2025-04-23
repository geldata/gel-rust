/// This macro is used to declare a new type.
/// 
/// Note that we use a "new" trait type for arrays to work around orphan rules.
#[macro_export]
macro_rules! declare_type {
    // Primitive types (no lifetime, fixed size)
    ($ty:ident) =>
    {
        $crate::declare_type!($crate::prelude::DataType, $ty, {
            fn decode(buf: [u8; N]) -> Result<Self, ParseError> {
                Ok($ty::from_be_bytes(buf))
            }
            fn encode(value: $ty) -> [u8; N] {
                value.to_be_bytes()
            }
            fn to_usize(value: usize) -> $ty {
                value as $ty
            }
            fn from_usize(value: $ty) -> usize {
                value as usize
            }
        });
    };
    ($datatype:path, $ty:ident , $( flags=[$($flag:ident),*], )?
    {
        fn decode($ebuf:ident: [u8; $dsize:expr]) -> Result<Self, ParseError> $decode:block
        fn encode($evalue:ident: $encode_type:ty) -> [u8; $esize:expr] $encode:block
        $( fn to_usize($eusize:ident: usize) -> $eusize_self:ty $to_usize:block )?
        $( fn from_usize($eusize2:ident: $eusize_self2:ty) -> usize $from_usize:block )?
    }
        ) => {
        impl $datatype for $ty {
            const META: $crate::prelude::StructFieldMeta = $crate::prelude::declare_meta!(
                type = $ty,
                constant_size = Some(std::mem::size_of::<$ty>()),
                flags = [$($($flag),*)?]
            );
            type BuilderForStruct<'unused> = $ty;
            type BuilderForEncode = $ty;
            type DecodeLifetime<'a> = $ty;
            fn decode<'__next_lifetime>(buf: &mut &'__next_lifetime [u8]) -> Result<Self, $crate::prelude::ParseError> {
                if let Some((chunk, next)) = buf.split_first_chunk::<{std::mem::size_of::<$ty>()}>() {
                    let res = {
                        let $ebuf = *chunk;
                        $decode
                    };
                    *buf = next;
                    res
                } else {
                    Err($crate::prelude::ParseError::TooShort)
                }
            }
            fn encode<'__buffer_lifetime, '__value_lifetime>(buf: &mut $crate::prelude::BufWriter<'__buffer_lifetime>, value: &'__value_lifetime Self::BuilderForEncode) {
                let $evalue = *value;
                let bytes = $encode;
                buf.write(&bytes);
            }
            $(
                fn encode_usize<'__buffer_lifetime, '__value_lifetime>(buf: &mut $crate::prelude::BufWriter<'__buffer_lifetime>, value: usize) {
                    let $eusize = value;
                    let value = $to_usize;
                    Self::encode(buf, &value);
                }
                fn decode_usize<'__next_lifetime>(buf: &mut &'__next_lifetime [u8]) -> Result<usize, $crate::prelude::ParseError> {
                    let $eusize2 = Self::decode(buf)?;
                    Ok($from_usize)
                }
            )?
        }

        impl <'a, L: $crate::prelude::DataType> $datatype 
            for $crate::prelude::Array<'a, L, $ty> {
                const META: $crate::prelude::StructFieldMeta = $crate::prelude::declare_meta!(
                    type = $ty,
                    constant_size = None,
                    flags = [$($($flag),*)?]
                );
                type BuilderForStruct<'unused> = &'a [$ty];
                type BuilderForEncode = [$ty];
                type DecodeLifetime<'__next_lifetime> = Array<'__next_lifetime, L, $ty>;
                fn decode<'__next_lifetime>(buf: &mut &'__next_lifetime [u8]) -> Result<Self::DecodeLifetime<'__next_lifetime>, $crate::prelude::ParseError> {
                    let len = L::decode_usize(buf)?;
                    let orig_buf = *buf;
                    for _ in 0..len {
                        <$ty as $datatype>::decode(buf)?;
                    }
                    Ok(Array::new(orig_buf, len as _))
                }
                fn encode<'__buffer_lifetime, '__value_lifetime>(buf: &mut $crate::prelude::BufWriter<'__buffer_lifetime>, value: &'__value_lifetime Self::BuilderForEncode) {
                    let len = value.len();
                    L::encode_usize(buf, len);
                    for elem in value {
                        <$ty as $datatype>::encode(buf, elem);
                    }
                }
        }

        impl <'a> $datatype 
            for $crate::prelude::ZTArray<'a, $ty> {
                const META: $crate::prelude::StructFieldMeta = $crate::prelude::declare_meta!(
                    type = $ty,
                    constant_size = None,
                    flags = [$($($flag),*)?]
                );
                type BuilderForStruct<'unused> = &'a [$ty];
                type BuilderForEncode = [$ty];
                type DecodeLifetime<'__next_lifetime> = ZTArray<'__next_lifetime, $ty>;
                fn decode<'__next_lifetime>(buf: &mut &'__next_lifetime [u8]) -> Result<Self::DecodeLifetime<'__next_lifetime>, $crate::prelude::ParseError> {
                    let mut orig_buf = *buf;
                    let mut len = 0;
                    loop {
                        if buf.is_empty() {
                            return Err($crate::prelude::ParseError::TooShort);
                        }
                        if buf[0] == 0 {
                            orig_buf = &orig_buf[0..orig_buf.len() - buf.len()];
                            *buf = &buf[1..];
                            break;
                        }
                        <$ty as $datatype>::decode(buf)?;
                        len += 1;
                    }
                    Ok(ZTArray::new(orig_buf, len))
                }
                fn encode<'__buffer_lifetime, '__value_lifetime>(buf: &mut $crate::prelude::BufWriter<'__buffer_lifetime>, value: &'__value_lifetime Self::BuilderForEncode) {
                    for elem in value {
                        <$ty as $datatype>::encode(buf, elem);
                    }
                    buf.write(&[0]);
                }
        }

        impl <const N: usize> $datatype 
            for [$ty; N] {
                const META: $crate::prelude::StructFieldMeta = $crate::prelude::declare_meta!(
                    type = $ty,
                    constant_size = Some(std::mem::size_of::<$ty>() * N),
                    flags = [$($($flag),*)?]
                );
                type BuilderForStruct<'unused> = [$ty; N];
                type BuilderForEncode = [$ty; N];
                type DecodeLifetime<'__next_lifetime> = [$ty; N];
                fn decode<'__next_lifetime>(buf: &mut &'__next_lifetime [u8]) -> Result<Self, $crate::prelude::ParseError> {
                    let mut res = [$ty::default(); N];
                    for i in 0..N {
                        res[i] = <$ty as $datatype>::decode(buf)?;
                    }
                    Ok(res)
                }
                fn encode<'__buffer_lifetime, '__value_lifetime>(buf: &mut $crate::prelude::BufWriter<'__buffer_lifetime>, value: &'__value_lifetime Self::BuilderForEncode) {
                    for elem in value {
                        <$ty as $datatype>::encode(buf, elem);
                    }
                }
        }

        impl $crate::prelude::DataTypeFixedSize for $ty {
            const SIZE: usize = std::mem::size_of::<$ty>();
        }

    };

    ($datatype:path, $ty:ident<$lt:lifetime $(, $generics:ident)*>, builder: $builder:ty, $( flags=[$($flag:ident),*], )?
    {
        fn decode($dbuf:ident: &mut &[u8]) -> Result<Self, ParseError> $decode:block
        fn encode($ebuf:ident: &mut BufWriter, $evalue:ident: $encode_type:ty) $encode:block
    }) => {
        impl <$lt $(,$generics)*> $datatype 
            for $ty<$lt $(,$generics)*> where $($generics: $datatype + 'a),* {
            const META: $crate::prelude::StructFieldMeta = $crate::prelude::declare_meta!(
                type = $ty,
                constant_size = None,
                flags = [$($($flag),*)?]
            );
            type BuilderForStruct<'unused> = $builder;
            type BuilderForEncode = $builder;
            type DecodeLifetime<'__next_lifetime> = $ty<'__next_lifetime $(,$generics)*>;
            fn decode<'__next_lifetime>(buf: &mut &'__next_lifetime [u8]) -> Result<Self::DecodeLifetime<'__next_lifetime>, $crate::prelude::ParseError> {
                let $dbuf = buf;
                $decode
            }
            fn encode<'__buffer_lifetime, '__value_lifetime>(buf: &mut $crate::prelude::BufWriter<'__buffer_lifetime>, value: &'__value_lifetime Self::BuilderForEncode) {
                let $ebuf = buf;
                let $evalue = value;
                $encode
            }
        }

        impl <$lt, L: $crate::prelude::DataType> $datatype 
            for $crate::prelude::Array<$lt, L, $ty<$lt>> {
                const META: $crate::prelude::StructFieldMeta = $crate::prelude::declare_meta!(
                    type = $ty,
                    constant_size = None,
                    flags = [$($($flag),*)?]
                );
                type BuilderForStruct<'unused> = &$lt [$builder];
                type BuilderForEncode = [$builder];
                type DecodeLifetime<'__next_lifetime> = Array<'__next_lifetime, L, $ty<'__next_lifetime>>;
                fn decode<'__next_lifetime>(buf: &mut &'__next_lifetime [u8]) -> Result<Self::DecodeLifetime<'__next_lifetime>, $crate::prelude::ParseError> {
                    let len = L::decode_usize(buf)?;
                    let orig_buf = *buf;
                    for _ in 0..len {
                        <$ty as $datatype>::decode(buf)?;
                    }
                    Ok(Array::new(orig_buf, len as _))
                }
                fn encode<'__buffer_lifetime, '__value_lifetime>(buf: &mut $crate::prelude::BufWriter<'__buffer_lifetime>, value: &'__value_lifetime Self::BuilderForEncode) {
                    let len = value.len();
                    L::encode_usize(buf, len);
                    for elem in value {
                        <$ty as $datatype>::encode(buf, elem);
                    }
                }
        }

        impl <$lt> $datatype 
            for $crate::prelude::ZTArray<$lt, $ty<$lt>> {
                const META: $crate::prelude::StructFieldMeta = $crate::prelude::declare_meta!(
                    type = $ty,
                    constant_size = None,
                    flags = [$($($flag),*)?]
                );
                type BuilderForStruct<'unused> = &$lt [$builder];
                type BuilderForEncode = [$builder];
                type DecodeLifetime<'__next_lifetime> = ZTArray<'__next_lifetime, $ty<'__next_lifetime>>;
                fn decode<'__next_lifetime>(_buf: &mut &'__next_lifetime [u8]) -> Result<Self::DecodeLifetime<'__next_lifetime>, $crate::prelude::ParseError> {
                    unimplemented!("7")
                }
                fn encode<'__buffer_lifetime, '__value_lifetime>(buf: &mut $crate::prelude::BufWriter<'__buffer_lifetime>, value: &'__value_lifetime Self::BuilderForEncode) {
                    unimplemented!("8")
                }
        }

        impl <$lt, const N: usize> $datatype 
            for [$ty<$lt>; N] {
                const META: $crate::prelude::StructFieldMeta = $crate::prelude::declare_meta!(
                    type = $ty,
                    constant_size = None,
                    flags = [$($($flag),*)?]
                );
                type BuilderForStruct<'unused> = [$builder; N];
                type BuilderForEncode = [$builder; N];
                type DecodeLifetime<'__next_lifetime> = [$ty<'__next_lifetime>; N];
                fn decode<'__next_lifetime>(buf: &mut &'__next_lifetime [u8]) -> Result<Self::DecodeLifetime<'__next_lifetime>, $crate::prelude::ParseError> {
                    let mut res = [$ty::<'__next_lifetime>::default(); N];
                    for i in 0..N {
                        res[i] = <$ty as $datatype>::decode(buf)?;
                    }
                    Ok(res)
                }
                fn encode<'__buffer_lifetime, '__value_lifetime>(buf: &mut $crate::prelude::BufWriter<'__buffer_lifetime>, value: &'__value_lifetime Self::BuilderForEncode) {
                    for elem in value {
                        <$ty as $datatype>::encode(buf, elem);
                    }
                }
        }

    };
}

#[macro_export]
macro_rules! declare_meta {
    (type = $ty:ident, constant_size = $constant_size:expr, flags = [$($flag:ident),*]) => {
        $crate::paste!($crate::prelude::StructFieldMeta::new(stringify!($ty), $constant_size)
            $(
                .[< set_ $flag >]()
            )*)
        
    };
}

#[macro_export]
macro_rules! copy_datatype {
    ($from:path, $to:path, $($ty:ident $(<$lt:lifetime>)?),*) => {
        $(
            impl $(<$lt>)? $to for $ty $(<$lt>)? {
                const META: StructFieldMeta = <$ty $(<$lt>)? as $from>::META;
                type BuilderForEncode = <$ty $(<$lt>)? as $from>::BuilderForEncode;
                type BuilderForStruct<'unused> = <$ty $(<$lt>)? as $from>::BuilderForStruct<'unused>;
                type DecodeLifetime<'__next_lifetime> = <$ty $(<$lt>)? as $from>::DecodeLifetime<'__next_lifetime>;

                fn decode<'__next_lifetime>(buf: &mut &'__next_lifetime [u8]) -> Result<Self::DecodeLifetime<'__next_lifetime>, ParseError> { 
                    <$ty as $from>::decode(buf)
                }

                fn encode<'__buffer_lifetime, '__value_lifetime>(buf: &mut $crate::prelude::BufWriter<'__buffer_lifetime>, value: &'__value_lifetime Self::BuilderForEncode) {
                    <$ty as $from>::encode(buf, value)
                }

                fn encode_usize<'__buffer_lifetime, '__value_lifetime>(buf: &mut $crate::prelude::BufWriter<'__buffer_lifetime>, value: usize) {
                    <$ty as $from>::encode_usize(buf, value)
                }

                fn decode_usize<'__next_lifetime>(buf: &mut &'__next_lifetime [u8]) -> Result<usize, ParseError> {
                    <$ty as $from>::decode_usize(buf)
                }
            }

        )*
    }
}
