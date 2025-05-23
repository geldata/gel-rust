use tokio::io::AsyncReadExt;
#[cfg(feature = "tokio")]
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

#[cfg(feature = "tokio")]
use std::{
    any::Any,
    io::IoSlice,
    pin::Pin,
    task::{Context, Poll},
};
use std::{future::Future, num::NonZeroUsize, ops::Deref};

use crate::{
    Ssl, SslError, TlsDriver, TlsHandshake, TlsServerParameterProvider, DEFAULT_PREVIEW_BUFFER_SIZE,
};

/// A convenience trait for streams from this crate.
#[cfg(feature = "tokio")]
pub trait Stream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static {
    fn downcast<S: Stream + 'static>(self) -> Result<S, Self>
    where
        Self: Sized + 'static,
    {
        // Note that we only support Tokio TcpStream for rustls.
        let mut holder = Some(self);
        let stream = &mut holder as &mut dyn Any;
        let Some(stream) = stream.downcast_mut::<Option<S>>() else {
            return Err(holder.take().unwrap());
        };
        let stream = stream.take().unwrap();
        Ok(stream)
    }
}

#[cfg(feature = "tokio")]
impl<T> Stream for T where T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static {}

#[cfg(not(feature = "tokio"))]
pub trait Stream: 'static {}
#[cfg(not(feature = "tokio"))]
impl<S: Stream, D: TlsDriver> Stream for UpgradableStream<S, D> {}
#[cfg(not(feature = "tokio"))]
impl Stream for () {}

/// A trait for streams that can be upgraded to a TLS stream.
pub trait StreamUpgrade: Stream {
    /// Upgrade the stream to a TLS stream.
    fn secure_upgrade(&mut self) -> impl Future<Output = Result<(), SslError>> + Send;
    /// Upgrade the stream to a TLS stream, and preview the initial bytes.
    fn secure_upgrade_preview(
        &mut self,
        options: PreviewConfiguration,
    ) -> impl Future<Output = Result<Preview, SslError>> + Send;
    /// Get the TLS handshake information, if the stream is upgraded.
    fn handshake(&self) -> Option<&TlsHandshake>;
}

/// A trait for streams that can be peeked asynchronously.
pub trait PeekableStream: Stream {
    #[cfg(feature = "tokio")]
    fn poll_peek(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf,
    ) -> Poll<std::io::Result<usize>>;
    #[cfg(feature = "tokio")]
    fn peek(self: Pin<&mut Self>, buf: &mut [u8]) -> impl Future<Output = std::io::Result<usize>> {
        async {
            let mut this = self;
            std::future::poll_fn(move |cx| this.as_mut().poll_peek(cx, &mut ReadBuf::new(buf)))
                .await
        }
    }
}

/// A preview of the initial bytes of the stream.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct Preview {
    buffer: smallvec::SmallVec<[u8; DEFAULT_PREVIEW_BUFFER_SIZE as usize]>,
}

impl Preview {
    pub(crate) fn new(
        buffer: smallvec::SmallVec<[u8; DEFAULT_PREVIEW_BUFFER_SIZE as usize]>,
    ) -> Self {
        Self { buffer }
    }
}

impl Deref for Preview {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl AsRef<[u8]> for Preview {
    fn as_ref(&self) -> &[u8] {
        &self.buffer
    }
}

impl<const N: usize> PartialEq<[u8; N]> for Preview {
    fn eq(&self, other: &[u8; N]) -> bool {
        self.buffer.as_slice() == other
    }
}

impl<const N: usize> PartialEq<&[u8; N]> for Preview {
    fn eq(&self, other: &&[u8; N]) -> bool {
        self.buffer.as_slice() == *other
    }
}

impl PartialEq<[u8]> for Preview {
    fn eq(&self, other: &[u8]) -> bool {
        self.buffer.as_slice() == other
    }
}

/// Configuration for the initial preview of the client connection.
#[derive(Debug, Clone, Copy)]
pub struct PreviewConfiguration {
    /// The maximum number of bytes to preview. Recommended value is 8 bytes.
    pub max_preview_bytes: NonZeroUsize,
    /// The maximum duration to preview for. Recommended value is 10 seconds.
    pub max_preview_duration: std::time::Duration,
}

impl Default for PreviewConfiguration {
    fn default() -> Self {
        Self {
            max_preview_bytes: NonZeroUsize::new(DEFAULT_PREVIEW_BUFFER_SIZE as usize).unwrap(),
            max_preview_duration: std::time::Duration::from_secs(10),
        }
    }
}

#[derive(Default, Debug)]
struct UpgradableStreamOptions {
    ignore_missing_close_notify: bool,
}

#[allow(private_bounds)]
#[derive(derive_more::Debug)]
pub struct UpgradableStream<S: Stream, D: TlsDriver = Ssl> {
    inner: UpgradableStreamInner<S, D>,
    options: UpgradableStreamOptions,
}

#[allow(private_bounds)]
impl<S: Stream, D: TlsDriver> UpgradableStream<S, D> {
    #[inline(always)]
    pub(crate) fn new_client(base: S, config: Option<D::ClientParams>) -> Self {
        UpgradableStream {
            inner: UpgradableStreamInner::BaseClient(base, config),
            options: Default::default(),
        }
    }

    #[inline(always)]
    pub(crate) fn new_server(base: S, config: Option<TlsServerParameterProvider>) -> Self {
        UpgradableStream {
            inner: UpgradableStreamInner::BaseServer(base, config),
            options: Default::default(),
        }
    }

    #[inline(always)]
    pub(crate) fn new_server_preview(
        base: RewindStream<S>,
        config: Option<TlsServerParameterProvider>,
    ) -> Self {
        UpgradableStream {
            inner: UpgradableStreamInner::BaseServerPreview(base, config),
            options: Default::default(),
        }
    }

    /// Consume the `UpgradableStream` and return the underlying stream as a [`Box<dyn Stream>`].
    pub fn into_boxed(self) -> Result<Box<dyn Stream>, Self> {
        match self.inner {
            UpgradableStreamInner::BaseClient(base, _) => Ok(Box::new(base)),
            UpgradableStreamInner::BaseServer(base, _) => Ok(Box::new(base)),
            UpgradableStreamInner::BaseServerPreview(base, _) => Ok(Box::new(base)),
            UpgradableStreamInner::Upgraded(upgraded, _) => Ok(Box::new(upgraded)),
            UpgradableStreamInner::UpgradedPreview(upgraded, _) => Ok(Box::new(upgraded)),
            UpgradableStreamInner::Upgrading => Err(self),
        }
    }

    pub fn handshake(&self) -> Option<&TlsHandshake> {
        match &self.inner {
            UpgradableStreamInner::Upgraded(_, handshake) => Some(handshake),
            _ => None,
        }
    }

    pub fn ignore_missing_close_notify(&mut self) {
        self.options.ignore_missing_close_notify = true;
    }

    /// Uncleanly shut down the stream. This may cause errors on the peer side
    /// when using TLS.
    pub fn unclean_shutdown(self) -> Result<(), Self> {
        match self.inner {
            UpgradableStreamInner::BaseClient(..) => Ok(()),
            UpgradableStreamInner::BaseServer(..) => Ok(()),
            UpgradableStreamInner::BaseServerPreview(..) => Ok(()),
            UpgradableStreamInner::Upgraded(upgraded, cfg) => {
                if let Err(e) = D::unclean_shutdown(upgraded) {
                    Err(Self {
                        inner: UpgradableStreamInner::Upgraded(e, cfg),
                        options: self.options,
                    })
                } else {
                    Ok(())
                }
            }
            UpgradableStreamInner::UpgradedPreview(upgraded, cfg) => {
                let (stm, buf) = upgraded.into_inner();
                if let Err(e) = D::unclean_shutdown(stm) {
                    Err(Self {
                        inner: UpgradableStreamInner::UpgradedPreview(
                            RewindStream {
                                buffer: buf,
                                inner: e,
                            },
                            cfg,
                        ),
                        options: self.options,
                    })
                } else {
                    Ok(())
                }
            }
            UpgradableStreamInner::Upgrading => Err(self),
        }
    }
}

impl<S: Stream, D: TlsDriver> StreamUpgrade for UpgradableStream<S, D> {
    fn secure_upgrade(&mut self) -> impl Future<Output = Result<(), SslError>> + Send {
        async move {
            let (upgraded, handshake) =
                match std::mem::replace(&mut self.inner, UpgradableStreamInner::Upgrading) {
                    UpgradableStreamInner::BaseClient(base, config) => {
                        let Some(config) = config else {
                            return Err(SslError::SslUnsupported);
                        };
                        D::upgrade_client(config, base).await?
                    }
                    UpgradableStreamInner::BaseServer(base, config) => {
                        let Some(config) = config else {
                            return Err(SslError::SslUnsupported);
                        };
                        D::upgrade_server(config, base).await?
                    }
                    UpgradableStreamInner::BaseServerPreview(base, config) => {
                        let Some(config) = config else {
                            return Err(SslError::SslUnsupported);
                        };
                        D::upgrade_server(config, base).await?
                    }
                    other => {
                        self.inner = other;
                        return Err(SslError::SslAlreadyUpgraded);
                    }
                };
            self.inner = UpgradableStreamInner::Upgraded(upgraded, handshake);
            Ok(())
        }
    }

    fn secure_upgrade_preview(
        &mut self,
        options: PreviewConfiguration,
    ) -> impl Future<Output = Result<Preview, SslError>> + Send {
        async move {
            let (mut upgraded, handshake) =
                match std::mem::replace(&mut self.inner, UpgradableStreamInner::Upgrading) {
                    UpgradableStreamInner::BaseClient(base, config) => {
                        let Some(config) = config else {
                            return Err(SslError::SslUnsupported);
                        };
                        D::upgrade_client(config, base).await?
                    }
                    UpgradableStreamInner::BaseServer(base, config) => {
                        let Some(config) = config else {
                            return Err(SslError::SslUnsupported);
                        };
                        D::upgrade_server(config, base).await?
                    }
                    UpgradableStreamInner::BaseServerPreview(base, config) => {
                        let Some(config) = config else {
                            return Err(SslError::SslUnsupported);
                        };
                        D::upgrade_server(config, base).await?
                    }
                    other => {
                        self.inner = other;
                        return Err(SslError::SslAlreadyUpgraded);
                    }
                };
            let mut buffer = smallvec::SmallVec::with_capacity(options.max_preview_bytes.get());
            buffer.resize(options.max_preview_bytes.get(), 0);
            upgraded.read_exact(&mut buffer).await?;
            let mut rewind = RewindStream::new(upgraded);
            rewind.rewind(&buffer);
            self.inner = UpgradableStreamInner::UpgradedPreview(rewind, handshake);
            Ok(Preview { buffer })
        }
    }

    fn handshake(&self) -> Option<&TlsHandshake> {
        match &self.inner {
            UpgradableStreamInner::Upgraded(_, handshake) => Some(handshake),
            _ => None,
        }
    }
}

#[cfg(feature = "tokio")]
impl<S: Stream, D: TlsDriver> tokio::io::AsyncRead for UpgradableStream<S, D> {
    #[inline(always)]
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let ignore_missing_close_notify = self.options.ignore_missing_close_notify;
        let inner = &mut self.get_mut().inner;
        let res = match inner {
            UpgradableStreamInner::BaseClient(base, _) => Pin::new(base).poll_read(cx, buf),
            UpgradableStreamInner::BaseServer(base, _) => Pin::new(base).poll_read(cx, buf),
            UpgradableStreamInner::BaseServerPreview(base, _) => Pin::new(base).poll_read(cx, buf),
            UpgradableStreamInner::Upgraded(upgraded, _) => Pin::new(upgraded).poll_read(cx, buf),
            UpgradableStreamInner::UpgradedPreview(upgraded, _) => {
                Pin::new(upgraded).poll_read(cx, buf)
            }
            UpgradableStreamInner::Upgrading => std::task::Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Cannot read while upgrading",
            ))),
        };
        if ignore_missing_close_notify {
            if matches!(res, std::task::Poll::Ready(Err(ref e)) if e.kind() == std::io::ErrorKind::UnexpectedEof)
            {
                return std::task::Poll::Ready(Ok(()));
            }
        }
        res
    }
}

#[cfg(feature = "tokio")]
impl<S: Stream, D: TlsDriver> tokio::io::AsyncWrite for UpgradableStream<S, D> {
    #[inline(always)]
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        let inner = &mut self.get_mut().inner;
        match inner {
            UpgradableStreamInner::BaseClient(base, _) => Pin::new(base).poll_write(cx, buf),
            UpgradableStreamInner::BaseServer(base, _) => Pin::new(base).poll_write(cx, buf),
            UpgradableStreamInner::BaseServerPreview(base, _) => Pin::new(base).poll_write(cx, buf),
            UpgradableStreamInner::Upgraded(upgraded, _) => Pin::new(upgraded).poll_write(cx, buf),
            UpgradableStreamInner::UpgradedPreview(upgraded, _) => {
                Pin::new(upgraded).poll_write(cx, buf)
            }
            UpgradableStreamInner::Upgrading => std::task::Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Cannot write while upgrading",
            ))),
        }
    }

    #[inline(always)]
    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        let inner = &mut self.get_mut().inner;
        match inner {
            UpgradableStreamInner::BaseClient(base, _) => Pin::new(base).poll_flush(cx),
            UpgradableStreamInner::BaseServer(base, _) => Pin::new(base).poll_flush(cx),
            UpgradableStreamInner::BaseServerPreview(base, _) => Pin::new(base).poll_flush(cx),
            UpgradableStreamInner::Upgraded(upgraded, _) => Pin::new(upgraded).poll_flush(cx),
            UpgradableStreamInner::UpgradedPreview(upgraded, _) => {
                Pin::new(upgraded).poll_flush(cx)
            }
            UpgradableStreamInner::Upgrading => std::task::Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Cannot flush while upgrading",
            ))),
        }
    }

    #[inline(always)]
    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        let inner = &mut self.get_mut().inner;
        match inner {
            UpgradableStreamInner::BaseClient(base, _) => Pin::new(base).poll_shutdown(cx),
            UpgradableStreamInner::BaseServer(base, _) => Pin::new(base).poll_shutdown(cx),
            UpgradableStreamInner::BaseServerPreview(base, _) => Pin::new(base).poll_shutdown(cx),
            UpgradableStreamInner::Upgraded(upgraded, _) => Pin::new(upgraded).poll_shutdown(cx),
            UpgradableStreamInner::UpgradedPreview(upgraded, _) => {
                Pin::new(upgraded).poll_shutdown(cx)
            }
            UpgradableStreamInner::Upgrading => std::task::Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Cannot shutdown while upgrading",
            ))),
        }
    }

    #[inline(always)]
    fn is_write_vectored(&self) -> bool {
        match &self.inner {
            UpgradableStreamInner::BaseClient(base, _) => base.is_write_vectored(),
            UpgradableStreamInner::BaseServer(base, _) => base.is_write_vectored(),
            UpgradableStreamInner::BaseServerPreview(base, _) => base.is_write_vectored(),
            UpgradableStreamInner::Upgraded(upgraded, _) => upgraded.is_write_vectored(),
            UpgradableStreamInner::UpgradedPreview(upgraded, _) => upgraded.is_write_vectored(),
            UpgradableStreamInner::Upgrading => false,
        }
    }

    #[inline(always)]
    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        let inner = &mut self.get_mut().inner;
        match inner {
            UpgradableStreamInner::BaseClient(base, _) => {
                Pin::new(base).poll_write_vectored(cx, bufs)
            }
            UpgradableStreamInner::BaseServer(base, _) => {
                Pin::new(base).poll_write_vectored(cx, bufs)
            }
            UpgradableStreamInner::BaseServerPreview(base, _) => {
                Pin::new(base).poll_write_vectored(cx, bufs)
            }
            UpgradableStreamInner::Upgraded(upgraded, _) => {
                Pin::new(upgraded).poll_write_vectored(cx, bufs)
            }
            UpgradableStreamInner::UpgradedPreview(upgraded, _) => {
                Pin::new(upgraded).poll_write_vectored(cx, bufs)
            }
            UpgradableStreamInner::Upgrading => std::task::Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Cannot write vectored while upgrading",
            ))),
        }
    }
}

#[derive(derive_more::Debug)]
enum UpgradableStreamInner<S: Stream, D: TlsDriver> {
    #[debug("BaseClient(..)")]
    BaseClient(S, Option<D::ClientParams>),
    #[debug("BaseServer(..)")]
    BaseServer(S, Option<TlsServerParameterProvider>),
    #[debug("Preview(..)")]
    BaseServerPreview(RewindStream<S>, Option<TlsServerParameterProvider>),
    #[debug("Upgraded(..)")]
    Upgraded(D::Stream, TlsHandshake),
    #[debug("Upgraded(..)")]
    UpgradedPreview(RewindStream<D::Stream>, TlsHandshake),
    #[debug("Upgrading")]
    Upgrading,
}

pub trait Rewindable {
    fn rewind(&mut self, bytes: &[u8]) -> std::io::Result<()>;
}

/// A stream that can be rewound.
pub(crate) struct RewindStream<S> {
    buffer: Vec<u8>,
    inner: S,
}

impl<S> RewindStream<S> {
    pub fn new(inner: S) -> Self {
        RewindStream {
            buffer: Vec::new(),
            inner,
        }
    }

    pub fn rewind(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    pub fn into_inner(self) -> (S, Vec<u8>) {
        (self.inner, self.buffer)
    }
}

#[cfg(feature = "tokio")]
impl<S: AsyncRead + Unpin> AsyncRead for RewindStream<S> {
    #[inline(always)]
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        if !self.buffer.is_empty() {
            let to_read = std::cmp::min(buf.remaining(), self.buffer.len());
            let data = self.buffer.drain(..to_read).collect::<Vec<_>>();
            buf.put_slice(&data);
            Poll::Ready(Ok(()))
        } else {
            Pin::new(&mut self.inner).poll_read(cx, buf)
        }
    }
}

#[cfg(feature = "tokio")]
impl<S: AsyncWrite + Unpin> AsyncWrite for RewindStream<S> {
    #[inline(always)]
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    #[inline(always)]
    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    #[inline(always)]
    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }

    #[inline(always)]
    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }

    #[inline(always)]
    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.inner).poll_write_vectored(cx, bufs)
    }
}

impl<S: Stream> Rewindable for RewindStream<S> {
    fn rewind(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.rewind(bytes);
        Ok(())
    }
}

impl<S: PeekableStream> PeekableStream for RewindStream<S> {
    fn poll_peek(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<usize>> {
        if !self.buffer.is_empty() {
            let to_read = std::cmp::min(buf.remaining(), self.buffer.len());
            buf.put_slice(&self.buffer[..to_read]);
            Poll::Ready(Ok(to_read))
        } else {
            Pin::new(&mut self.inner).poll_peek(cx, buf)
        }
    }
}

impl<S: Stream + Rewindable, D: TlsDriver> Rewindable for UpgradableStream<S, D>
where
    D::Stream: Rewindable,
{
    fn rewind(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        match &mut self.inner {
            UpgradableStreamInner::BaseClient(stm, _) => stm.rewind(bytes),
            UpgradableStreamInner::BaseServer(stm, _) => stm.rewind(bytes),
            UpgradableStreamInner::BaseServerPreview(stm, _) => Ok(stm.rewind(bytes)),
            UpgradableStreamInner::Upgraded(stm, _) => stm.rewind(bytes),
            UpgradableStreamInner::UpgradedPreview(stm, _) => Ok(stm.rewind(bytes)),
            UpgradableStreamInner::Upgrading => Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Cannot rewind a stream that is upgrading",
            )),
        }
    }
}

impl<S: PeekableStream, D: TlsDriver> PeekableStream for UpgradableStream<S, D>
where
    D::Stream: PeekableStream,
{
    #[cfg(feature = "tokio")]
    fn poll_peek(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf,
    ) -> Poll<std::io::Result<usize>> {
        match &mut self.get_mut().inner {
            UpgradableStreamInner::BaseClient(base, _) => Pin::new(base).poll_peek(cx, buf),
            UpgradableStreamInner::BaseServer(base, _) => Pin::new(base).poll_peek(cx, buf),
            UpgradableStreamInner::BaseServerPreview(base, _) => Pin::new(base).poll_peek(cx, buf),
            UpgradableStreamInner::Upgraded(upgraded, _) => Pin::new(upgraded).poll_peek(cx, buf),
            UpgradableStreamInner::UpgradedPreview(upgraded, _) => {
                Pin::new(upgraded).poll_peek(cx, buf)
            }
            UpgradableStreamInner::Upgrading => Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Cannot peek a stream that is upgrading",
            ))),
        }
    }
}
