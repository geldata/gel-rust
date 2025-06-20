#![allow(private_bounds)]

use crate::prelude::*;

pub use std::marker::PhantomData;

/// Shared implementation for all array types.
macro_rules! array_impl {
    (#[$doc:meta] impl <$lt:lifetime, $generic:ident $(, $length_generic:ident)?> $ty:ident) => {
        #[$doc]
        #[derive(Copy, Clone, Default)]
        pub struct $ty<$lt, $($length_generic,)? $generic>
        where
            $generic: DecoderFor<$lt, $generic>,
        {
            _phantom: PhantomData<( $generic , $( $length_generic)? )>,
            buf: &'a [u8],
            len: usize,
        }

        impl<$lt, $generic, $($length_generic)?> $ty<$lt, $($length_generic,)? $generic>
        where
            $generic: DecoderFor<$lt, $generic>,
        {
            #[inline(always)]
            pub const fn new(buf: &$lt [u8], len: usize) -> Self {
                Self {
                    buf,
                    len,
                    _phantom: PhantomData,
                }
            }

            #[inline(always)]
            pub const fn empty() -> Self {
                Self {
                    buf: &[],
                    len: 0,
                    _phantom: PhantomData,
                }
            }

            #[inline(always)]
            pub const fn len(&self) -> usize {
                self.len
            }

            #[inline(always)]
            pub const fn is_empty(&self) -> bool {
                self.len == 0
            }

            #[inline(always)]
            pub const fn into_slice(self) -> &'a [u8] {
                self.buf
            }
        }

        impl<$lt, $generic, $($length_generic)?> std::fmt::Debug for $ty<$lt, $($length_generic,)? $generic>
        where
            $generic: DecoderFor<$lt, $generic>,
            $generic: std::fmt::Debug,
        {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_list().entries(self).finish()
            }
        }

        // ZTArrays of type [`u8`] are special-cased to return a slice of bytes.
        impl<$lt, $generic, $($length_generic)?> ArrayExt<$lt> for $ty<$lt, $($length_generic,)? $generic>
        where
            $generic: DecoderFor<$lt, $generic>,
            $( $length_generic: $lt )?
        {
            #[inline(always)]
            fn into_slice(self) -> &'a [u8] {
                self.buf
            }
        }

        impl<$lt, $generic, $($length_generic)?> AsRef<[u8]> for $ty<$lt, $($length_generic,)? $generic>
        where
            $generic: DecoderFor<$lt, $generic>,
        {
            #[inline(always)]
            fn as_ref(&self) -> &[u8] {
                self.buf
            }
        }

        impl<$lt, $generic, $($length_generic)?> IntoIterator for $ty<$lt, $($length_generic,)? $generic>
        where
            $generic: DecoderFor<$lt, $generic>,
        {
            type Item = $generic;
            type IntoIter = ArrayIter<'a, $generic>;
            fn into_iter(self) -> Self::IntoIter {
                Self::IntoIter {
                    _phantom: PhantomData,
                    buf: self.buf,
                    len: self.len,
                }
            }
        }

        impl<$lt, $generic, $($length_generic)?> IntoIterator for &$ty<$lt, $($length_generic,)? $generic>
        where
            $generic: DecoderFor<$lt, $generic>,
        {
            type Item = $generic;
            type IntoIter = ArrayIter<'a, $generic>;
            fn into_iter(self) -> Self::IntoIter {
                Self::IntoIter {
                    _phantom: PhantomData,
                    buf: self.buf,
                    len: self.len,
                }
            }
        }

        // Arrays of fixed-size elements can extract elements in O(1).
        impl<$lt, $generic, $($length_generic)?> $ty<$lt, $($length_generic,)? $generic>
        where
            $generic: DataTypeFixedSize + DecoderFor<$lt, $generic>,
        {
            #[inline]
            pub fn get(&self, index: impl TryInto<usize>) -> Option<$generic> {
                let Ok(index) = index.try_into() else {
                    return None;
                };
                let index: usize = index;
                if index >= self.len as _ {
                    None
                } else {
                    let mut segment = &self.buf[T::SIZE * index..T::SIZE * (index + 1)];
                    // As we've normally pre-scanned all items, this will not panic
                    Some(T::decode_for(&mut segment).unwrap())
                }
            }
        }

        /// Arrays of `u8` can be indexed.
        impl<$lt, $($length_generic)?> std::ops::Index<usize> for $ty<$lt, $($length_generic,)? u8> {
            type Output = u8;
            #[inline(always)]
            fn index(&self, index: usize) -> &Self::Output {
                &self.as_ref()[index]
            }
        }

        /// Arrays of `u8` can be compared to slices.
        impl<$lt, $($length_generic)?> PartialEq<&[u8]> for $ty<$lt, $($length_generic,)? u8>
        {
            fn eq(&self, other: &&[u8]) -> bool {
                self.as_ref() == *other
            }
        }

        /// Arrays of `u8` can be compared to fixed-size slices.
        impl<$lt, $($length_generic, )? const N: usize> PartialEq<&[u8; N]> for $ty<$lt, $($length_generic,)? u8>
        {
            fn eq(&self, other: &&[u8; N]) -> bool {
                self.as_ref() == *other
            }
        }
    };
}

/// Shared trait for all array types.
pub trait ArrayExt<'a>: 'a {
    /// Convert the array into a slice of bytes.
    fn into_slice(self) -> &'a [u8];
}

array_impl!(
    /// A zero-terminated array.
    impl <'a, T> ZTArray
);
array_impl!(
    /// A count-prefixed array.
    impl <'a, T, L> Array
);
array_impl!(
    /// A rest array: consumes the remainder of the buffer.
    impl <'a, T> RestArray
);

/// [`ZTArray`], [`Array`], and [`RestArray`] [`Iterator`] for values of type `T`.
#[derive(Copy, Clone, Default)]
pub struct ArrayIter<'a, T> {
    _phantom: PhantomData<T>,
    buf: &'a [u8],
    len: usize,
}

impl<'a, T> Iterator for ArrayIter<'a, T>
where
    T: DecoderFor<'a, T>,
{
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        let value = T::decode_for(&mut self.buf).ok()?;
        Some(value)
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<'a, T> ExactSizeIterator for ArrayIter<'a, T>
where
    T: DecoderFor<'a, T>,
{
    #[inline(always)]
    fn len(&self) -> usize {
        self.len as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rest_array_u8() {
        // Test with u8 values
        let data = vec![1, 2, 3, 4, 5];
        let mut buf = &data[..];
        let rest_array = RestArray::<u8>::decode_for(&mut buf).unwrap();

        assert_eq!(rest_array.len(), 5);
        assert!(!rest_array.is_empty());
        assert_eq!(buf.len(), 0); // Buffer should be consumed entirely

        let collected: Vec<u8> = rest_array.into_iter().collect();
        assert_eq!(collected, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_rest_array_u32() {
        let data = vec![
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x03,
        ];

        let mut buf = &data[..];
        let rest_array = RestArray::<u32>::decode_for(&mut buf).unwrap();

        assert_eq!(rest_array.len(), 3);
        assert!(!rest_array.is_empty());
        assert_eq!(buf.len(), 0); // Buffer should be consumed entirely

        let collected: Vec<u32> = rest_array.into_iter().collect();
        assert_eq!(collected, vec![1, 2, 3]);
    }

    #[test]
    fn test_rest_array_empty() {
        // Test with empty buffer
        let data: Vec<u8> = vec![];
        let mut buf = &data[..];
        let rest_array = RestArray::<u8>::decode_for(&mut buf).unwrap();

        assert_eq!(rest_array.len(), 0);
        assert!(rest_array.is_empty());
        assert_eq!(buf.len(), 0);

        let collected: Vec<u8> = rest_array.into_iter().collect();
        assert_eq!(collected, vec![]);
    }

    #[test]
    fn test_rest_array_get() {
        // Test get method for fixed-size elements
        let data = vec![1u8, 2, 3, 4, 5];
        let mut buf = &data[..];
        let rest_array = RestArray::<u8>::decode_for(&mut buf).unwrap();

        assert_eq!(rest_array.get(0), Some(1));
        assert_eq!(rest_array.get(2), Some(3));
        assert_eq!(rest_array.get(4), Some(5));
        assert_eq!(rest_array.get(5), None); // Out of bounds
    }
}
