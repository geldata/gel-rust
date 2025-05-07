pub mod stream;
pub mod target;
pub mod tls;

#[cfg(feature = "openssl")]
pub mod openssl;
#[cfg(feature = "rustls")]
pub mod rustls;
#[cfg(feature = "tokio")]
pub mod tokio_stream;

#[doc(hidden)]
#[cfg(feature = "tokio")]
pub type BaseStream = tokio_stream::TokioStream;

#[doc(hidden)]
#[cfg(not(feature = "tokio"))]
pub type BaseStream = ();
