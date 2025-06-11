use std::str::Utf8Error;
pub use uuid::Uuid;

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
#[derive(Copy, Clone, Default)]
pub struct ZTString<'a> {
    buf: &'a [u8],
}

impl std::fmt::Debug for ZTString<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        String::from_utf8_lossy(self.buf).fmt(f)
    }
}

impl<'a> ZTString<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf }
    }
}

impl ZTString<'_> {
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

impl PartialEq for ZTString<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.buf == other.buf
    }
}
impl Eq for ZTString<'_> {}

impl PartialEq<str> for ZTString<'_> {
    fn eq(&self, other: &str) -> bool {
        self.buf == other.as_bytes()
    }
}

impl PartialEq<&str> for ZTString<'_> {
    fn eq(&self, other: &&str) -> bool {
        self.buf == other.as_bytes()
    }
}

impl<'a> TryInto<&'a str> for ZTString<'a> {
    type Error = Utf8Error;
    fn try_into(self) -> Result<&'a str, Self::Error> {
        std::str::from_utf8(self.buf)
    }
}

/// A length-prefixed string.
#[derive(Copy, Clone, Default)]
pub struct LString<'a> {
    buf: &'a [u8],
}

impl std::fmt::Debug for LString<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        String::from_utf8_lossy(self.buf).fmt(f)
    }
}

impl<'a> LString<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf }
    }
}

impl LString<'_> {
    pub fn to_owned(&self) -> Result<String, std::str::Utf8Error> {
        std::str::from_utf8(self.buf).map(|s| s.to_owned())
    }

    pub fn to_str(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(self.buf)
    }

    pub fn to_string_lossy(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(&self.buf)
    }

    pub fn to_bytes(&self) -> &[u8] {
        self.buf
    }
}

impl PartialEq for LString<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.buf == other.buf
    }
}
impl Eq for LString<'_> {}

impl PartialEq<str> for LString<'_> {
    fn eq(&self, other: &str) -> bool {
        self.buf == other.as_bytes()
    }
}

impl PartialEq<&str> for LString<'_> {
    fn eq(&self, other: &&str) -> bool {
        self.buf == other.as_bytes()
    }
}

impl<'a> TryInto<&'a str> for LString<'a> {
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
