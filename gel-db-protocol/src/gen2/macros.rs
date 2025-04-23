
#[macro_export]
macro_rules! declare_type {
    ($ty:ident , $( flags=[$($flag:ident),*], )? $( alias=$alias:ty, )? {$($rest:tt)*}) => {
        impl $crate::prelude::DataType for $ty {
            const META: $crate::prelude::StructFieldMeta = $crate::prelude::declare_meta!(
                type = $ty,
                constant_size = Some(std::mem::size_of::<$ty>()),
                flags = [$($($flag),*)?]
            );
            type BuilderForStruct<'unused> = $ty;
            type BuilderForEncode = $ty;
            fn decode<'__next_lifetime>(_buf: &mut &'__next_lifetime [u8]) -> Result<Self, $crate::prelude::ParseError> {
                unimplemented!()
            }
            fn encode<'__buffer_lifetime, '__value_lifetime>(_buf: $crate::prelude::EncodeTarget<'__buffer_lifetime>, _value: &'__value_lifetime Self::BuilderForEncode) -> $crate::prelude::EncodeTarget<'__buffer_lifetime> {
                unimplemented!()
            }
        }

        impl <'a, L: $crate::prelude::DataType> $crate::prelude::DataType 
            for $crate::prelude::Array<'a, L, $ty> {
                const META: $crate::prelude::StructFieldMeta = $crate::prelude::declare_meta!(
                    type = $ty,
                    constant_size = None,
                    flags = [$($($flag),*)?]
                );
                type BuilderForStruct<'unused> = &'a [$ty];
                type BuilderForEncode = [$ty];
                fn decode<'__next_lifetime>(_buf: &mut &'__next_lifetime [u8]) -> Result<Self, $crate::prelude::ParseError> {
                    unimplemented!()
                }
                fn encode<'__buffer_lifetime, '__value_lifetime>(_buf: $crate::prelude::EncodeTarget<'__buffer_lifetime>, _value: &'__value_lifetime Self::BuilderForEncode) -> $crate::prelude::EncodeTarget<'__buffer_lifetime> {
                    unimplemented!()
                }
        }

        impl <const N: usize> $crate::prelude::DataType 
            for [$ty; N] {
                const META: $crate::prelude::StructFieldMeta = $crate::prelude::declare_meta!(
                    type = $ty,
                    constant_size = Some(std::mem::size_of::<$ty>() * N),
                    flags = [$($($flag),*)?]
                );
                type BuilderForStruct<'unused> = [$ty; N];
                type BuilderForEncode = [$ty; N];
                fn decode<'__next_lifetime>(_buf: &mut &'__next_lifetime [u8]) -> Result<Self, $crate::prelude::ParseError> {
                    unimplemented!()
                }
                fn encode<'__buffer_lifetime, '__value_lifetime>(_buf: $crate::prelude::EncodeTarget<'__buffer_lifetime>, _value: &'__value_lifetime Self::BuilderForEncode) -> $crate::prelude::EncodeTarget<'__buffer_lifetime> {
                    unimplemented!()
                }
        }

        impl $crate::prelude::DataTypeFixedSize for $ty {
            const SIZE: usize = std::mem::size_of::<$ty>();
        }

        impl <const N: usize> $crate::prelude::DataTypeFixedSize for [$ty; N] {
            const SIZE: usize = std::mem::size_of::<$ty>() * N;
        }
    };

    ($ty:ident<$lt:lifetime $(, $generics:ident)*>, builder: $builder:ty, $( flags=[$($flag:ident),*], )? $( alias=$alias:ty, )? {$($rest:tt)*}) => {
        impl <$lt $(,$generics)*> $crate::prelude::DataType 
            for $ty<$lt $(,$generics)*> where $($generics: $crate::prelude::DataType + 'a),* {
            const META: $crate::prelude::StructFieldMeta = $crate::prelude::declare_meta!(
                type = $ty,
                constant_size = None,
                flags = [$($($flag),*)?]
            );
            type BuilderForStruct<'unused> = $builder;
            type BuilderForEncode = $builder;
            fn decode<'__next_lifetime>(_buf: &mut &'__next_lifetime [u8]) -> Result<Self, $crate::prelude::ParseError> {
                unimplemented!()
            }
            fn encode<'__buffer_lifetime, '__value_lifetime>(_buf: $crate::prelude::EncodeTarget<'__buffer_lifetime>, _value: &'__value_lifetime Self::BuilderForEncode) -> $crate::prelude::EncodeTarget<'__buffer_lifetime> {
                unimplemented!()
            }
        }

        impl <$lt, L: $crate::prelude::DataType> $crate::prelude::DataType 
            for $crate::prelude::Array<$lt, L, $ty<$lt>> {
                const META: $crate::prelude::StructFieldMeta = $crate::prelude::declare_meta!(
                    type = $ty,
                    constant_size = None,
                    flags = [$($($flag),*)?]
                );
                type BuilderForStruct<'unused> = &$lt [$builder];
                type BuilderForEncode = [$builder];
                fn decode<'__next_lifetime>(_buf: &mut &'__next_lifetime [u8]) -> Result<Self, $crate::prelude::ParseError> {
                    unimplemented!()
                }
                fn encode<'__buffer_lifetime, '__value_lifetime>(_buf: $crate::prelude::EncodeTarget<'__buffer_lifetime>, _value: &'__value_lifetime Self::BuilderForEncode) -> $crate::prelude::EncodeTarget<'__buffer_lifetime> {
                    unimplemented!()
                }
        }

        impl <$lt, const N: usize> $crate::prelude::DataType 
            for [$ty<$lt>; N] {
                const META: $crate::prelude::StructFieldMeta = $crate::prelude::declare_meta!(
                    type = $ty,
                    constant_size = None,
                    flags = [$($($flag),*)?]
                );
                type BuilderForStruct<'unused> = [$builder; N];
                type BuilderForEncode = [$builder; N];
                fn decode<'__next_lifetime>(_buf: &mut &'__next_lifetime [u8]) -> Result<Self, $crate::prelude::ParseError> {
                    unimplemented!()
                }
                fn encode<'__buffer_lifetime, '__value_lifetime>(_buf: $crate::prelude::EncodeTarget<'__buffer_lifetime>, _value: &'__value_lifetime Self::BuilderForEncode) -> $crate::prelude::EncodeTarget<'__buffer_lifetime> {
                    unimplemented!()
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
