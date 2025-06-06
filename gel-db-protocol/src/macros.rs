/// This macro is used to declare serialization traits for a type.
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
            fn decode(buf: &mut &[u8]) -> Result<Self, $crate::prelude::ParseError> {
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
            fn encode(buf: &mut $crate::prelude::BufWriter<'_>, value: &Self::BuilderForEncode) {
                let $evalue = *value;
                let bytes = $encode;
                buf.write(&bytes);
            }
            $(
                fn encode_usize<'__value_lifetime>(buf: &mut $crate::prelude::BufWriter<'_>, value: usize) {
                    let $eusize = value;
                    let value = $to_usize;
                    Self::encode(buf, &value);
                }
                fn decode_usize(buf: &mut &[u8]) -> Result<usize, $crate::prelude::ParseError> {
                    let $eusize2 = Self::decode(buf)?;
                    Ok($from_usize)
                }
            )?
        }

        impl $crate::prelude::DataTypeFixedSize for $ty {
            const SIZE: usize = std::mem::size_of::<$ty>();
        }

    };

    // Lifetime type, non-fixed size
    ($datatype:path, $ty:ident<$lt:lifetime>, builder: $builder:ty, $( flags=[$($flag:ident),*], )?
    {
        fn decode($dbuf:ident: &mut &[u8]) -> Result<Self, ParseError> $decode:block
        fn encode($ebuf:ident: &mut BufWriter, $evalue:ident: $encode_type:ty) $encode:block
    }) => {
        impl <$lt> $datatype
            for $ty<$lt> {
            const META: $crate::prelude::StructFieldMeta = $crate::prelude::declare_meta!(
                type = $ty,
                constant_size = None,
                flags = [$($($flag),*)?]
            );
            type BuilderForStruct<'unused> = $builder;
            type BuilderForEncode = $builder;
            type DecodeLifetime<'__next_lifetime> = $ty<'__next_lifetime>;
            fn decode<'__next_lifetime>(buf: &mut &'__next_lifetime [u8]) -> Result<Self::DecodeLifetime<'__next_lifetime>, $crate::prelude::ParseError> {
                let $dbuf = buf;
                $decode
            }
            fn encode(buf: &mut $crate::prelude::BufWriter<'_>, value: &Self::BuilderForEncode) {
                let $ebuf = buf;
                let $evalue = value;
                $encode
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
