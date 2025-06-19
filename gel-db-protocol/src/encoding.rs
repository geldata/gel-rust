use std::mem::MaybeUninit;

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

    #[allow(unused)]
    fn encode_usize(buf: &mut BufWriter<'_>, value: usize) {
        unreachable!("encode usize")
    }
    #[allow(unused)]
    fn decode_usize(buf: &mut &[u8]) -> Result<usize, ParseError> {
        unreachable!("decode usize")
    }
}

/// Implemented for all data types that have a fixed size.
pub trait DataTypeFixedSize {
    const SIZE: usize;
}

/// Marks a type as a builder for a given message.
pub trait BuilderFor: EncoderFor<Self::Message> + Sized {
    type Message: 'static;
}

/// Marks a type as a decoder for itself.
pub trait DecoderFor<'a, F: 'a>: DataType + 'a {
    fn decode_for(buf: &mut &'a [u8]) -> Result<F, ParseError>;
}

/// Marks a type as an encoder for a given type.
pub trait EncoderFor<F: 'static> {
    fn encode_for(&self, buf: &mut BufWriter<'_>);
}

/// Helper trait for encodable objects.
pub trait EncoderForExt {
    /// Convert this builder into a vector of bytes. This is generally
    /// not the most efficient way to perform serialization.
    #[allow(unused)]
    fn to_vec<F: 'static>(&self) -> Vec<u8>
    where
        Self: EncoderFor<F>,
    {
        let mut vec = Vec::with_capacity(256);
        let mut buf = BufWriter::new(&mut vec);
        EncoderFor::<F>::encode_for(self, &mut buf);
        match buf.finish() {
            Ok(size) => {
                vec.truncate(size);
                vec
            }
            Err(size) => {
                vec.resize(size, 0);
                let mut buf = BufWriter::new(&mut vec);
                EncoderFor::<F>::encode_for(self, &mut buf);
                // Will not fail this second time
                let size = buf.finish().unwrap();
                vec.truncate(size);
                vec
            }
        }
    }

    /// Encode this builder into a given buffer. If the buffer is
    /// too small, the function will return the number of bytes
    /// required to encode the builder.
    #[allow(unused)]
    fn encode_buffer<F: 'static>(&self, buf: &mut [u8]) -> Result<usize, usize>
    where
        Self: EncoderFor<F>,
    {
        let mut writer = BufWriter::new(buf);
        EncoderFor::<F>::encode_for(self, &mut writer);
        writer.finish()
    }

    /// Encode this builder into a given buffer. If the buffer is
    /// too small, the function will return the number of bytes
    /// required to encode the builder.
    #[allow(unused)]
    fn encode_buffer_uninit<'a, F: 'static>(
        &self,
        buf: &'a mut [MaybeUninit<u8>],
    ) -> Result<&'a mut [u8], usize>
    where
        Self: EncoderFor<F>,
    {
        let mut writer = BufWriter::new_uninit(buf);
        EncoderFor::<F>::encode_for(self, &mut writer);
        writer.finish_buf()
    }

    #[allow(unused)]
    fn measure<F: 'static>(&self) -> usize
    where
        Self: EncoderFor<F>,
    {
        let mut buf = Vec::new();
        let mut writer = BufWriter::new(&mut buf);
        EncoderFor::<F>::encode_for(self, &mut writer);
        writer.finish().unwrap_err()
    }
}

impl<T> EncoderForExt for T where T: ?Sized {}

#[derive(derive_more::Error, derive_more::Display, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    #[display("Buffer is too short")]
    TooShort,
    #[display("Buffer is too long ({_0} extra bytes)")]
    TooLong(#[error(not(source))] usize),
    #[display("Invalid data for {_0}: {_1}")]
    InvalidData(
        #[error(not(source))] &'static str,
        #[error(not(source))] usize,
    ),
    #[display("Invalid data for field {_0}: {_1}")]
    InvalidFieldData(
        #[error(not(source))] &'static str,
        #[error(not(source))] &'static str,
    ),
}

impl<'a, L: DataType, T: DataType> DataType for Array<'a, L, T>
where
    T: DecoderFor<'a, T>,
{
    const META: StructFieldMeta = declare_meta!(
        type = Array,
        constant_size = None,
        flags = [array]
    );
}

impl<'a, L: DataType, T: DataType> DecoderFor<'a, Array<'a, L, T>> for Array<'a, L, T>
where
    L: 'a,
    T: DecoderFor<'a, T>,
{
    fn decode_for(buf: &mut &'a [u8]) -> Result<Self, ParseError> {
        let len = L::decode_usize(buf)?;
        let orig_buf = *buf;
        for _ in 0..len {
            T::decode_for(buf)?;
        }
        Ok(Array::new(orig_buf, len as _))
    }
}

impl<L: DataType + 'static, T: DataType + 'static, IT> EncoderFor<Array<'static, L, T>>
    for &'_ [IT]
where
    IT: EncoderFor<T>,
    T: DecoderFor<'static, T>,
{
    fn encode_for(&self, buf: &mut BufWriter<'_>) {
        let iter = self.into_iter();
        L::encode_usize(buf, self.len());
        for elem in iter {
            IT::encode_for(&elem, buf);
        }
    }
}

impl<L: DataType + 'static, T: DataType + 'static, const N: usize, IT>
    EncoderFor<Array<'static, L, T>> for [IT; N]
where
    IT: EncoderFor<T>,
    T: DecoderFor<'static, T>,
{
    fn encode_for(&self, buf: &mut BufWriter<'_>) {
        let iter = self.into_iter();
        L::encode_usize(buf, self.len());
        for elem in iter {
            IT::encode_for(&elem, buf);
        }
    }
}

impl<L: DataType + 'static, T: DataType + 'static, const N: usize, IT>
    EncoderFor<Array<'static, L, T>> for &'_ [IT; N]
where
    IT: EncoderFor<T>,
    T: DecoderFor<'static, T>,
{
    fn encode_for(&self, buf: &mut BufWriter<'_>) {
        let iter = self.into_iter();
        L::encode_usize(buf, self.len());
        for elem in iter {
            IT::encode_for(&elem, buf);
        }
    }
}

impl<'a, L, T, U> EncoderFor<Array<'static, L, T>> for Array<'a, L, U>
where
    L: DataType + 'static,
    U: EncoderFor<T> + DecoderFor<'a, U>,
    T: DecoderFor<'static, T>,
{
    fn encode_for(&self, buf: &mut BufWriter<'_>) {
        L::encode_usize(buf, self.len());
        for elem in self.into_iter() {
            U::encode_for(&elem, buf);
        }
    }
}

impl<L: DataType + 'static, T: DataType + 'static, F, I, II, IT> EncoderFor<Array<'static, L, T>>
    for F
where
    F: Fn() -> I,
    I: IntoIterator<Item = IT, IntoIter = II>,
    IT: EncoderFor<T>,
    II: ExactSizeIterator<Item = IT>,
    T: DecoderFor<'static, T>,
{
    fn encode_for(&self, buf: &mut BufWriter<'_>) {
        let iter = self().into_iter();
        L::encode_usize(buf, II::len(&iter));
        for elem in iter {
            IT::encode_for(&elem, buf);
        }
    }
}

impl<'a, T: DataType> DataType for ZTArray<'a, T>
where
    T: DecoderFor<'a, T>,
{
    const META: StructFieldMeta = declare_meta!(
        type = ZTArray,
        constant_size = None,
        flags = [array]
    );
}

impl<'a, T: DataType> DecoderFor<'a, ZTArray<'a, T>> for ZTArray<'a, T>
where
    T: DecoderFor<'a, T>,
{
    fn decode_for(buf: &mut &'a [u8]) -> Result<Self, ParseError> {
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
            T::decode_for(buf)?;
            len += 1;
        }
        Ok(ZTArray::new(orig_buf, len))
    }
}

impl<T: DataType + 'static, IT> EncoderFor<ZTArray<'static, T>> for &'_ [IT]
where
    IT: EncoderFor<T>,
    T: DecoderFor<'static, T>,
{
    fn encode_for(&self, buf: &mut BufWriter<'_>) {
        let iter = self.into_iter();
        for elem in iter {
            IT::encode_for(&elem, buf);
        }
        buf.write(&[0]);
    }
}

impl<T: DataType + 'static, const N: usize, IT> EncoderFor<ZTArray<'static, T>> for [IT; N]
where
    IT: EncoderFor<T>,
    T: DecoderFor<'static, T>,
{
    fn encode_for(&self, buf: &mut BufWriter<'_>) {
        let iter = self.into_iter();
        for elem in iter {
            IT::encode_for(&elem, buf);
        }
        buf.write(&[0]);
    }
}

impl<T: DataType + 'static, const N: usize, IT> EncoderFor<ZTArray<'static, T>> for &'_ [IT; N]
where
    IT: EncoderFor<T>,
    T: DecoderFor<'static, T>,
{
    fn encode_for(&self, buf: &mut BufWriter<'_>) {
        let iter = self.into_iter();
        for elem in iter {
            IT::encode_for(&elem, buf);
        }
        buf.write(&[0]);
    }
}

impl<'a, T, U> EncoderFor<ZTArray<'static, T>> for ZTArray<'a, U>
where
    T: DataType + 'static,
    T: DecoderFor<'static, T>,
    U: DataType,
    U: EncoderFor<T>,
    U: DecoderFor<'a, U>,
{
    fn encode_for(&self, buf: &mut BufWriter<'_>) {
        for elem in self.into_iter() {
            U::encode_for(&elem, buf);
        }
    }
}

impl<T: DataType + 'static, F, I, II, IT> EncoderFor<ZTArray<'static, T>> for F
where
    F: Fn() -> I,
    I: IntoIterator<Item = IT, IntoIter = II>,
    IT: EncoderFor<T>,
    II: Iterator<Item = IT>,
    T: DecoderFor<'static, T>,
{
    fn encode_for(&self, buf: &mut BufWriter<'_>) {
        let iter = self().into_iter();
        for elem in iter {
            IT::encode_for(&elem, buf);
        }
        buf.write(&[0]);
    }
}

impl<const N: usize, T: DataType> DataType for [T; N]
where
    for<'a> T: Default + Copy,
{
    const META: StructFieldMeta = declare_meta!(
        type = FixedArray,
        constant_size = Some(std::mem::size_of::<T>() * N),
        flags = [array]
    );
}

impl<'a, T: DataType, const N: usize> DecoderFor<'a, [T; N]> for [T; N]
where
    T: DecoderFor<'a, T> + Default + Copy,
{
    fn decode_for(buf: &mut &'a [u8]) -> Result<Self, ParseError> {
        let mut res = [T::default(); N];
        for res in res.iter_mut().take(N) {
            *res = T::decode_for(buf)?;
        }
        Ok(res)
    }
}

impl<const N: usize, T: DataType> DataTypeFixedSize for [T; N] {
    const SIZE: usize = std::mem::size_of::<T>() * N;
}

impl<const N: usize, T: DataType + 'static, U: EncoderFor<T>> EncoderFor<[T; N]> for [U; N] {
    fn encode_for(&self, buf: &mut BufWriter<'_>) {
        for elem in self {
            U::encode_for(elem, buf);
        }
    }
}

/// Implements [`DataType`] and [`DataTypeFixedSize`] for tuples.
macro_rules! tuple_type {
    () => {};
    ($head:ident $(, $tail:ident)*) => {
        impl <$head: DataType, $($tail: DataType),*> DataType for ($head, $($tail),*) {
            const META: StructFieldMeta = declare_meta!(type = Tuple, constant_size = None, flags = []);
        }

        impl <$head: DataType, $($tail: DataType),*> DataTypeFixedSize for ($head, $($tail),*) where $head: DataTypeFixedSize, $($tail: DataTypeFixedSize),* {
            const SIZE: usize = $head::SIZE $(+ $tail::SIZE)*;
        }

        $crate::paste!(
            /// Homomorphic mapping: If A: DecoderFor<A_X>, B: DecoderFor<B_X>, then (A, B): DecoderFor<(A_X, B_X)>
            impl <'a,$head: DataType, $($tail: DataType),*> DecoderFor<'a, ($head, $($tail),*)> for ($head, $($tail),*) where $head: DecoderFor<'a, $head>, $($tail: DecoderFor<'a, $tail>),* {
                fn decode_for(buf: &mut &'a [u8]) -> Result<Self, ParseError> {
                    Ok((
                        $head::decode_for(buf)?,
                        $($tail::decode_for(buf)?),*
                    ))
                }
            }

            /// Homomorphic mapping: If A: EncoderFor<A_X>, B: EncoderFor<B_X>, then (A, B): EncoderFor<(A_X, B_X)>
            impl <$head, [<$head X>]: 'static, $($tail, [<$tail X>]: 'static),*>
                EncoderFor<([<$head X>], $([<$tail X>]),*)> for ($head, $($tail),*)

                where $head: EncoderFor<[<$head X>]>, $($tail: EncoderFor<[<$tail X>]>),* {

                fn encode_for(&self, buf: &mut BufWriter<'_>) {
                    #[allow(non_snake_case)]
                    let ($head, $($tail),*) = self;
                    EncoderFor::<[<$head X>]>::encode_for($head, buf);
                    $(
                        EncoderFor::<[<$tail X>]>::encode_for($tail, buf);
                    )*
                }
            }
        );

        // recurse
        tuple_type!($($tail),*);
    };
}

// Up to 52 fields seems reasonable.
tuple_type!(
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, A1, B1, C1, D1,
    E1, F1, G1, H1, I1, J1, K1, L1, M1, N1, O1, P1, Q1, R1, S1, T1, U1, V1, W1, X1, Y1, Z1
);

declare_type!(DataType, Rest<'a>, builder: &'a [u8],
{}
);

impl<'a> DecoderFor<'a, Rest<'a>> for Rest<'a> {
    fn decode_for(buf: &mut &'a [u8]) -> Result<Self, ParseError> {
        let res = Rest::new(buf);
        *buf = &[];
        Ok(res)
    }
}

impl<T> EncoderFor<Rest<'static>> for T
where
    T: AsRef<[u8]>,
{
    fn encode_for(&self, buf: &mut BufWriter<'_>) {
        buf.write(self.as_ref());
    }
}

declare_type!(DataType, LString<'a>, builder: &'a str, {});

impl<'a> DecoderFor<'a, LString<'a>> for LString<'a> {
    fn decode_for(buf: &mut &'a [u8]) -> Result<Self, ParseError> {
        let arr = Array::<u32, u8>::decode_for(buf)?;
        Ok(LString::new(arr.into_slice()))
    }
}

impl<T> EncoderFor<LString<'static>> for T
where
    for<'any> &'any T: AsRef<str>,
{
    fn encode_for(&self, buf: &mut BufWriter<'_>) {
        let bytes = self.as_ref().as_bytes();
        EncoderFor::<u32>::encode_for(&(bytes.len() as u32), buf);
        buf.write(self.as_ref().as_bytes());
    }
}

impl<'a> EncoderFor<LString<'static>> for LString<'a> {
    fn encode_for(&self, buf: &mut BufWriter<'_>) {
        let bytes = self.to_bytes();
        EncoderFor::<u32>::encode_for(&(bytes.len() as u32), buf);
        buf.write(bytes);
    }
}

declare_type!(DataType, ZTString<'a>, builder: &'a str, {
});

impl<'a> DecoderFor<'a, ZTString<'a>> for ZTString<'a> {
    fn decode_for(buf: &mut &'a [u8]) -> Result<Self, ParseError> {
        let arr = ZTArray::<u8>::decode_for(buf)?;
        let slice = arr.into_slice();
        Ok(ZTString::new(&slice[0..slice.len() - 1]))
    }
}

impl<T> EncoderFor<ZTString<'static>> for T
where
    for<'any> &'any T: AsRef<str>,
{
    fn encode_for(&self, buf: &mut BufWriter<'_>) {
        buf.write(self.as_ref().as_bytes());
        buf.write(&[0]);
    }
}

impl<'a> EncoderFor<ZTString<'static>> for ZTString<'a> {
    fn encode_for(&self, buf: &mut BufWriter<'_>) {
        buf.write(self.to_bytes());
        buf.write(&[0]);
    }
}

declare_type!(DataType, Encoded<'a>, builder: Encoded<'a>, {});

impl<'a> DecoderFor<'a, Encoded<'a>> for Encoded<'a> {
    fn decode_for(buf: &mut &'a [u8]) -> Result<Self, ParseError> {
        if let Some((len, array)) = buf.split_first_chunk::<{ std::mem::size_of::<i32>() }>() {
            let len = i32::from_be_bytes(*len);
            if len == -1 {
                *buf = array;
                Ok(Encoded::Null)
            } else if len < 0 {
                Err(ParseError::InvalidData("Encoded", len as usize))
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
}

impl<T> EncoderFor<Encoded<'static>> for Option<T>
where
    T: AsRef<[u8]>,
{
    fn encode_for(&self, buf: &mut BufWriter<'_>) {
        match self {
            Some(value) => buf.write(value.as_ref()),
            None => buf.write(&(-1_i32).to_be_bytes()),
        }
    }
}

impl EncoderFor<Encoded<'static>> for Encoded<'_> {
    fn encode_for(&self, buf: &mut BufWriter<'_>) {
        match self {
            Encoded::Null => buf.write(&(-1_i32).to_be_bytes()),
            Encoded::Value(value) => {
                let len: i32 = value.len() as _;
                buf.write(&len.to_be_bytes());
                buf.write(value);
            }
        }
    }
}

impl EncoderFor<Encoded<'static>> for &'_ Encoded<'_> {
    fn encode_for(&self, buf: &mut BufWriter<'_>) {
        match self {
            Encoded::Null => buf.write(&(-1_i32).to_be_bytes()),
            Encoded::Value(value) => {
                let len: i32 = value.len() as _;
                buf.write(&len.to_be_bytes());
                buf.write(value);
            }
        }
    }
}

declare_type!(DataType, Length, flags = [length], {
    fn to_usize(value: usize) -> Length {
        Length(value as _)
    }
    fn from_usize(value: Length) -> usize {
        value.0 as usize
    }
});

impl<'a> DecoderFor<'a, Length> for Length {
    fn decode_for(buf: &mut &'a [u8]) -> Result<Self, ParseError> {
        i32::decode_for(buf).map(Length)
    }
}

impl EncoderFor<Length> for u32 {
    fn encode_for(&self, buf: &mut BufWriter<'_>) {
        buf.write(&self.to_be_bytes());
    }
}

impl EncoderFor<Length> for Length {
    fn encode_for(&self, buf: &mut BufWriter<'_>) {
        buf.write(&self.0.to_be_bytes());
    }
}

declare_type!(DataType, Uuid, {});

impl<'a> DecoderFor<'a, Uuid> for Uuid {
    fn decode_for(buf: &mut &'a [u8]) -> Result<Self, ParseError> {
        <[u8; 16] as DecoderFor<'a, [u8; 16]>>::decode_for(buf).map(Uuid::from_bytes)
    }
}

impl EncoderFor<Uuid> for &'_ Uuid {
    fn encode_for(&self, buf: &mut BufWriter<'_>) {
        buf.write(&self.into_bytes());
    }
}

impl EncoderFor<Uuid> for Uuid {
    fn encode_for(&self, buf: &mut BufWriter<'_>) {
        buf.write(&self.into_bytes());
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    static_assertions::assert_impl_all!(u8: DataType, DataTypeFixedSize);
    static_assertions::assert_impl_all!([u8; 4]: DataType, DataTypeFixedSize, DecoderFor<'static, [u8; 4]>);
    static_assertions::assert_impl_all!((u8, u8): DataType, DataTypeFixedSize, EncoderFor<(u8, u8)>);

    static_assertions::assert_impl_all!(&'static str: EncoderFor<LString<'static>>);
    static_assertions::assert_impl_all!(String: EncoderFor<LString<'static>>);
    static_assertions::assert_impl_all!(&'static String: EncoderFor<LString<'static>>);
}
