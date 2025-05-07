#![allow(private_bounds)]

use crate::prelude::*;

pub use std::marker::PhantomData;

/// Inflated version of a zero-terminated array with zero-copy iterator access.
#[derive(Copy, Clone, Default)]
pub struct ZTArray<'a, T> {
    _phantom: PhantomData<T>,
    buf: &'a [u8],
    len: usize,
}

impl<'a, T> ZTArray<'a, T> {
    #[inline(always)]
    pub const fn new(buf: &'a [u8], len: usize) -> Self {
        Self {
            buf,
            len,
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
}

/// [`ZTArray`] [`Iterator`] for values of type `T`.
pub struct ZTArrayIter<'a, T> {
    _phantom: PhantomData<T>,
    buf: &'a [u8],
}

impl<'a, T> std::fmt::Debug for ZTArray<'a, T>
where
    T: DataType,
    T::DecodeLifetime<'a>: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self).finish()
    }
}

// ZTArrays of type [`u8`] are special-cased to return a slice of bytes.
impl<'a> ZTArray<'a, u8> {
    #[inline(always)]
    pub fn into_slice(self) -> &'a [u8] {
        self.buf
    }
}

impl AsRef<[u8]> for ZTArray<'_, u8> {
    #[inline(always)]
    fn as_ref(&self) -> &[u8] {
        &self.buf[..self.len as _]
    }
}

impl<'a, T: DataType> IntoIterator for ZTArray<'a, T> {
    type Item = T::DecodeLifetime<'a>;
    type IntoIter = ZTArrayIter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        ZTArrayIter {
            _phantom: PhantomData,
            buf: self.buf,
        }
    }
}

impl<'a, T: DataType> IntoIterator for &ZTArray<'a, T> {
    type Item = T::DecodeLifetime<'a>;
    type IntoIter = ZTArrayIter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        ZTArrayIter {
            _phantom: PhantomData,
            buf: self.buf,
        }
    }
}

impl<'a, T: DataType> Iterator for ZTArrayIter<'a, T> {
    type Item = T::DecodeLifetime<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.buf[0] == 0 {
            return None;
        }
        let value = T::decode(&mut self.buf).ok()?;
        Some(value)
    }
}

/// Inflated version of a length-specified array with zero-copy iterator access.
#[derive(Copy, Clone, Default)]
pub struct Array<'a, L, T> {
    _phantom: PhantomData<(L, T)>,
    buf: &'a [u8],
    len: u32,
}

impl<'a, L, T> Array<'a, L, T> {
    pub const fn new(buf: &'a [u8], len: u32) -> Self {
        Self {
            buf,
            len,
            _phantom: PhantomData,
        }
    }

    #[inline(always)]
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    #[inline(always)]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

// Arrays of type [`u8`] are special-cased to return a slice of bytes.
impl<'a, L> Array<'a, L, u8> {
    #[inline(always)]
    pub fn into_slice(self) -> &'a [u8] {
        &self.buf[..self.len as _]
    }
}

impl<T> AsRef<[u8]> for Array<'_, T, u8> {
    #[inline(always)]
    fn as_ref(&self) -> &[u8] {
        &self.buf[..self.len as _]
    }
}

impl<'a, L, T> std::fmt::Debug for Array<'a, L, T>
where
    for<'b> &'b Self: IntoIterator,
    for<'b> <&'b Self as IntoIterator>::Item: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self).finish()
    }
}

/// [`Array`] [`Iterator`] for values of type `T`.
pub struct ArrayIter<'a, T> {
    _phantom: PhantomData<T>,
    buf: &'a [u8],
    len: u32,
}

impl<'a, L, T: DataType> IntoIterator for Array<'a, L, T> {
    type Item = T::DecodeLifetime<'a>;
    type IntoIter = ArrayIter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        ArrayIter {
            _phantom: PhantomData,
            buf: self.buf,
            len: self.len,
        }
    }
}

impl<'a, L, T: DataType> IntoIterator for &Array<'a, L, T> {
    type Item = T::DecodeLifetime<'a>;
    type IntoIter = ArrayIter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        ArrayIter {
            _phantom: PhantomData,
            buf: self.buf,
            len: self.len,
        }
    }
}

impl<'a, T: DataType> Iterator for ArrayIter<'a, T> {
    type Item = T::DecodeLifetime<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        let value = T::decode(&mut self.buf).ok()?;
        Some(value)
    }
}

// Arrays of fixed-size elements can extract elements in O(1).
impl<'a, L: TryInto<usize>, T: DataTypeFixedSize + DataType> Array<'a, L, T> {
    pub fn get(&self, index: L) -> Option<T::DecodeLifetime<'a>> {
        let Ok(index) = index.try_into() else {
            return None;
        };
        let index: usize = index;
        if index >= self.len as _ {
            None
        } else {
            let mut segment = &self.buf[T::SIZE * index..T::SIZE * (index + 1)];
            // As we've normally pre-scanned all items, this will not panic
            Some(T::decode(&mut segment).unwrap())
        }
    }
}
