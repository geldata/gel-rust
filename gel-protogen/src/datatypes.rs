use std::{marker::PhantomData, ops::Add, str::Utf8Error};
pub use uuid::Uuid;

use crate::{arrays::ArrayExt, Array, ZTArray};

/// Represents the remainder of data in a message.
#[derive(Copy, Debug, PartialEq, Eq, Default, Clone)]
pub struct Rest<'a> {
    buf: &'a [u8],
}

impl<'a> Rest<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf }
    }
}

impl Rest<'_> {}

impl AsRef<[u8]> for Rest<'_> {
    fn as_ref(&self) -> &[u8] {
        self.buf
    }
}

impl<'a> ArrayExt<'a> for Rest<'a> {
    fn into_slice(self) -> &'a [u8] {
        self.buf
    }
}

impl std::ops::Deref for Rest<'_> {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        self.buf
    }
}

impl PartialEq<[u8]> for Rest<'_> {
    fn eq(&self, other: &[u8]) -> bool {
        self.buf == other
    }
}

impl<const N: usize> PartialEq<&[u8; N]> for Rest<'_> {
    fn eq(&self, other: &&[u8; N]) -> bool {
        self.buf == *other
    }
}

impl PartialEq<&[u8]> for Rest<'_> {
    fn eq(&self, other: &&[u8]) -> bool {
        self.buf == *other
    }
}

/// A zero-terminated string.
pub type ZTString<'a> = ArrayString<'a, ZTArray<'a, u8>>;

/// A length-prefixed string.
pub type LString<'a> = ArrayString<'a, Array<'a, u32, u8>>;

/// A string which consumes the remainder of the buffer.
pub type RestString<'a> = ArrayString<'a, Rest<'a>>;

/// A string, which lives on top of a given array type.
#[derive(Copy, Clone, Default)]
pub struct ArrayString<'a, A> {
    buf: &'a [u8],
    _phantom: PhantomData<A>,
}

impl<A> std::fmt::Debug for ArrayString<'_, A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        String::from_utf8_lossy(self.buf).fmt(f)
    }
}

impl<'a, A> ArrayString<'a, A> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self {
            buf,
            _phantom: PhantomData,
        }
    }
}

impl<A> ArrayString<'_, A> {
    pub fn to_owned(&self) -> Result<String, std::str::Utf8Error> {
        std::str::from_utf8(self.buf).map(|s| s.to_owned())
    }

    pub fn to_str(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(self.buf)
    }

    pub fn to_string_lossy(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(self.buf)
    }

    pub fn to_bytes(&self) -> &[u8] {
        self.buf
    }
}

impl<A> PartialEq for ArrayString<'_, A> {
    fn eq(&self, other: &Self) -> bool {
        self.buf == other.buf
    }
}
impl<A> Eq for ArrayString<'_, A> {}

impl<A> PartialEq<str> for ArrayString<'_, A> {
    fn eq(&self, other: &str) -> bool {
        self.buf == other.as_bytes()
    }
}

impl<A> PartialEq<&str> for ArrayString<'_, A> {
    fn eq(&self, other: &&str) -> bool {
        self.buf == other.as_bytes()
    }
}

impl<'a, A> TryInto<&'a str> for ArrayString<'a, A> {
    type Error = Utf8Error;
    fn try_into(self) -> Result<&'a str, Self::Error> {
        std::str::from_utf8(self.buf)
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
/// An encoded row value.
pub enum Encoded<'a> {
    #[default]
    Null,
    Value(&'a [u8]),
}

impl Encoded<'_> {
    pub fn to_string_lossy(&self) -> std::borrow::Cow<'_, str> {
        match self {
            Encoded::Null => "".into(),
            Encoded::Value(value) => String::from_utf8_lossy(value),
        }
    }
}

impl<'a> AsRef<Encoded<'a>> for Encoded<'a> {
    fn as_ref(&self) -> &Encoded<'a> {
        self
    }
}

impl<'a> From<&'a [u8]> for Encoded<'a> {
    fn from(value: &'a [u8]) -> Self {
        Encoded::Value(value)
    }
}

impl<'a> Into<Option<&'a [u8]>> for Encoded<'a> {
    fn into(self) -> Option<&'a [u8]> {
        match self {
            Encoded::Null => None,
            Encoded::Value(value) => Some(value),
        }
    }
}

impl<'a, 'b> Into<Option<&'a [u8]>> for &'b Encoded<'a> {
    fn into(self) -> Option<&'a [u8]> {
        match self {
            Encoded::Null => None,
            Encoded::Value(value) => Some(value),
        }
    }
}

impl Encoded<'_> {}

impl PartialEq<str> for Encoded<'_> {
    fn eq(&self, other: &str) -> bool {
        self == &Encoded::Value(other.as_bytes())
    }
}

impl PartialEq<&str> for Encoded<'_> {
    fn eq(&self, other: &&str) -> bool {
        self == &Encoded::Value(other.as_bytes())
    }
}

impl PartialEq<[u8]> for Encoded<'_> {
    fn eq(&self, other: &[u8]) -> bool {
        self == &Encoded::Value(other)
    }
}

impl PartialEq<&[u8]> for Encoded<'_> {
    fn eq(&self, other: &&[u8]) -> bool {
        self == &Encoded::Value(other)
    }
}

#[derive(
    Copy,
    Clone,
    Default,
    derive_more::Debug,
    derive_more::Display,
    derive_more::Deref,
    derive_more::DerefMut,
    PartialEq,
    Eq,
)]
#[display("{_0}")]
#[debug("{_0}")]
pub struct Length(pub i32);

impl Add<usize> for Length {
    type Output = usize;
    fn add(self, other: usize) -> Self::Output {
        (self.0 as isize + other as isize) as usize
    }
}

/// A length-prefixed value.
#[derive(
    Copy,
    Clone,
    Default,
    derive_more::Debug,
    derive_more::Display,
    derive_more::Deref,
    derive_more::DerefMut,
    PartialEq,
    Eq,
)]
#[display("{_0}")]
#[debug("{_0:?}")]
pub struct LengthPrefixed<T>(pub T);
