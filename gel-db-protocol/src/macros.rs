/// This macro is used to declare serialization traits for a type.
#[macro_export]
macro_rules! declare_type {
    // Primitive types (no lifetime, fixed size)
    ($ty:ident) =>
    {
        $crate::declare_type!($crate::prelude::DataType, $ty, {
            fn to_usize(value: usize) -> $ty {
                value as $ty
            }
            fn from_usize(value: $ty) -> usize {
                value as usize
            }
        });

        impl $crate::prelude::EncoderFor<$ty> for $ty {
            fn encode_for(&self, buf: &mut $crate::BufWriter<'_>) {
                buf.write(&self.to_be_bytes());
            }
        }

        impl <'a> $crate::prelude::DecoderFor<'a, $ty> for $ty {
            fn decode_for(buf: &mut &'a [u8]) -> Result<Self, $crate::prelude::ParseError> {
                if let Some((chunk, next)) = buf.split_first_chunk::<{std::mem::size_of::<$ty>()}>() {
                    let res = {
                        let buf = *chunk;
                        Ok($ty::from_be_bytes(buf))
                    };
                    *buf = next;
                    res
                } else {
                    Err($crate::prelude::ParseError::TooShort)
                }
            }
        }
    };
    ($datatype:path, $ty:ident , $( flags=[$($flag:ident),*], )?
    {
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

            $(
                fn encode_usize<'__value_lifetime>(buf: &mut $crate::prelude::BufWriter<'_>, value: usize) {
                    let $eusize = value;
                    let value = $to_usize;
                    $crate::prelude::EncoderFor::<$ty>::encode_for(&value, buf);
                }
                fn decode_usize(buf: &mut &[u8]) -> Result<usize, $crate::prelude::ParseError> {
                    let $eusize2 = <$ty as $crate::prelude::DecoderFor<$ty>>::decode_for(buf)?;
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
    }) => {
        impl <$lt> $datatype
            for $ty<$lt> {
            const META: $crate::prelude::StructFieldMeta = $crate::prelude::declare_meta!(
                type = $ty,
                constant_size = None,
                flags = [$($($flag),*)?]
            );
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

/// Generate encoders for array types from slices, arrays, and
/// iterator-generating functions. Note that because we need to consume the
/// iterator and we may need to iterate multiple times for measurement before
/// serialization, we implement encoding for a function returning an iterator.
///
/// NOTE: For types that have a length generic, the length generic must appear
/// second.
#[macro_export]
macro_rules! encoder_for_array {
    (impl <$generic:ident $(, $length_generic:ident)?> for $ty:ty {
        fn encode_for(&self, $buf:ident: &mut BufWriter<'_>, $it:ident: impl $iter:ident) $block:block
    }) => {
        /// Self encoder
        impl<$generic : DataType, $($length_generic : DataType + 'static)?> EncoderFor<$ty> for $ty
        where
            $generic: EncoderFor<$generic>,
            $generic: DecoderFor<'static, $generic>,
        {
            fn encode_for(&self, $buf: &mut $crate::BufWriter<'_>) {
                let $it = self.into_iter();
                $block
            }
        }

        /// Slice encoder
        impl<$generic : DataType, $($length_generic : DataType + 'static, )? IT> EncoderFor<$ty>
            for &'_ [IT]
        where
            IT: EncoderFor<$generic>,
            $generic: DecoderFor<'static, $generic>,
        {
            fn encode_for(&self, $buf: &mut $crate::BufWriter<'_>) {
                let $it = self.into_iter();
                $block
            }
        }

        /// Array encoder
        impl<$generic : DataType, $($length_generic : DataType + 'static, )? const N: usize, IT>
            EncoderFor<$ty> for [IT; N]
        where
            IT: EncoderFor<$generic>,
            $generic: DecoderFor<'static, $generic>,
        {
            fn encode_for(&self, $buf: &mut $crate::BufWriter<'_>) {
                let $it = self.into_iter();
                $block
            }
        }

        /// Array reference encoder
        impl<$generic : DataType, $($length_generic : DataType + 'static, )? const N: usize, IT>
            EncoderFor<$ty> for &'_ [IT; N]
        where
            IT: EncoderFor<$generic>,
            $generic: DecoderFor<'static, $generic>,
        {
            fn encode_for(&self, $buf: &mut $crate::BufWriter<'_>) {
                let $it = self.into_iter();
                $block
            }
        }

        /// Function encoder: see the note about about non-restartable iterators.
        impl<$generic : DataType, $($length_generic : DataType + 'static, )? F, I, II, IT> EncoderFor<$ty>
            for F
        where
            F: Fn() -> I,
            I: IntoIterator<Item = IT, IntoIter = II>,
            IT: EncoderFor<$generic>,
            II: $iter<Item = IT>,
            $generic: DecoderFor<'static, $generic>,
        {
            fn encode_for(&self, $buf: &mut $crate::BufWriter<'_>) {
                let $it = self().into_iter();
                $block
            }
        }

        /// Direct Vec<T> encoder
        impl<$generic : DataType, $($length_generic : DataType + 'static, )? IT> EncoderFor<$ty>
            for Vec<IT>
        where
            IT: EncoderFor<$generic>,
            $generic: DecoderFor<'static, $generic>,
        {
            fn encode_for(&self, $buf: &mut $crate::BufWriter<'_>) {
                let $it = self.iter();
                $block
            }
        }

        /// Direct &Vec<T> encoder
        impl<$generic : DataType, $($length_generic : DataType + 'static, )? IT> EncoderFor<$ty>
            for &'_ Vec<IT>
        where
            IT: EncoderFor<$generic>,
            $generic: DecoderFor<'static, $generic>,
        {
            fn encode_for(&self, $buf: &mut $crate::BufWriter<'_>) {
                let $it = self.iter();
                $block
            }
        }
    };
}
