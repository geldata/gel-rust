#[cfg(feature = "tokio")]
use tokio::io::{AsyncRead, AsyncReadExt, ReadBuf};

#[cfg(feature = "tokio")]
use std::{
    any::Any,
    pin::Pin,
    task::{Context, Poll},
};
use std::{future::Future, num::NonZeroUsize, ops::Deref};

use crate::{
    LocalAddress, PeerCred, RemoteAddress, ResolvedTarget, Ssl, SslError, StreamMetadata,
    TlsDriver, TlsHandshake, TlsServerParameterProvider, Transport, DEFAULT_PREVIEW_BUFFER_SIZE,
};

/// A trait for streams that can be converted to a handle or file descriptor.
#[cfg(unix)]
pub trait AsHandle {
    fn as_fd(&self) -> std::os::fd::BorrowedFd;
}

/// A trait for streams that can be converted to a handle or file descriptor.
#[cfg(windows)]
pub trait AsHandle {
    fn as_handle(&self) -> std::os::windows::io::BorrowedSocket;
}

/// A convenience trait for streams from this crate.
#[cfg(feature = "tokio")]
pub trait Stream:
    tokio::io::AsyncRead + tokio::io::AsyncWrite + StreamMetadata + Send + Unpin + AsHandle + 'static
{
    /// Attempt to downcast a generic stream to a specific stream type.
    fn downcast<S: Stream + 'static>(self) -> Result<S, Self>
    where
        Self: Sized + 'static,
    {
        let mut holder = Some(self);
        let stream = &mut holder as &mut dyn Any;
        if let Some(stream) = stream.downcast_mut::<Option<S>>() {
            return Ok(stream.take().unwrap());
        }
        if let Some(stream) = stream.downcast_mut::<Option<Box<S>>>() {
            return Ok(*stream.take().unwrap());
        }
        Err(holder.take().unwrap())
    }

    /// Box the stream as a `Box<dyn Stream + Send>`.
    fn boxed(self) -> Box<dyn Stream + Send>
    where
        Self: Sized + 'static,
    {
        let mut holder = Some(self);
        let stream = &mut holder as &mut dyn Any;
        if let Some(stream) = stream.downcast_mut::<Option<Box<dyn Stream>>>() {
            stream.take().unwrap()
        } else {
            Box::new(holder.take().unwrap())
        }
    }
}

#[cfg(feature = "tokio")]
impl<T> Stream for T where
    T: tokio::io::AsyncRead
        + tokio::io::AsyncWrite
        + StreamMetadata
        + AsHandle
        + Unpin
        + Send
        + 'static
{
}

#[cfg(not(feature = "tokio"))]
impl<T> Stream for T where T: StreamMetadata + AsHandle + Unpin + Send + 'static {}

// NOTE: Once we're on Rust 1.87, we can use trait upcasting and get rid of this impl.
impl PeerCred for Box<dyn Stream + Send> {
    #[cfg(all(unix, feature = "tokio"))]
    fn peer_cred(&self) -> std::io::Result<tokio::net::unix::UCred> {
        self.as_ref().peer_cred()
    }
}

impl LocalAddress for Box<dyn Stream + Send> {
    fn local_address(&self) -> std::io::Result<ResolvedTarget> {
        self.as_ref().local_address()
    }
}

impl RemoteAddress for Box<dyn Stream + Send> {
    fn remote_address(&self) -> std::io::Result<ResolvedTarget> {
        self.as_ref().remote_address()
    }
}

impl StreamMetadata for Box<dyn Stream + Send> {
    fn transport(&self) -> Transport {
        self.as_ref().transport()
    }
}

#[cfg(not(feature = "tokio"))]
impl StreamMetadata for () {
    fn transport(&self) -> Transport {
        unreachable!()
    }
}

#[cfg(not(feature = "tokio"))]
impl LocalAddress for () {
    fn local_address(&self) -> std::io::Result<ResolvedTarget> {
        unreachable!()
    }
}

#[cfg(not(feature = "tokio"))]
impl RemoteAddress for () {
    fn remote_address(&self) -> std::io::Result<ResolvedTarget> {
        unreachable!()
    }
}

#[cfg(not(feature = "tokio"))]
impl PeerCred for () {}

#[cfg(not(feature = "tokio"))]
impl AsHandle for () {
    #[cfg(unix)]
    fn as_fd(&self) -> std::os::fd::BorrowedFd {
        unreachable!()
    }
    #[cfg(windows)]
    fn as_handle(&self) -> std::os::windows::io::BorrowedSocket {
        unreachable!()
    }
}

#[cfg(not(feature = "tokio"))]
pub trait Stream: StreamMetadata + Unpin + AsHandle + 'static {}

/// A trait for streams that can be upgraded to a TLS stream.
pub trait StreamUpgrade: Stream + Sized {
    /// Upgrade the stream to a TLS stream.
    fn secure_upgrade(self) -> impl Future<Output = Result<Self, SslError>> + Send;
    /// Upgrade the stream to a TLS stream, and preview the initial bytes.
    fn secure_upgrade_preview(
        self,
        options: PreviewConfiguration,
    ) -> impl Future<Output = Result<(Preview, Self), SslError>> + Send;
    /// Get the TLS handshake information, if the stream is upgraded.
    fn handshake(&self) -> Option<&TlsHandshake>;
}

#[cfg(feature = "optimization")]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StreamOptimization {
    #[default]
    /// Optimize for general use.
    General,
    /// Optimize for interactive use with low latency.
    Interactive,
    /// Optimize for bulk streaming.
    BulkStreaming(BulkStreamDirection),
}

#[cfg(feature = "optimization")]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BulkStreamDirection {
    Send,
    Receive,
    #[default]
    Both,
}

/// A trait for streams that can provide a `socket2::SockRef`.
#[cfg(any(feature = "optimization", feature = "keepalive"))]
fn with_socket2<S: AsHandle + Sized>(
    stream: &S,
    f: &mut dyn for<'a> FnMut(socket2::SockRef<'a>) -> Result<(), std::io::Error>,
) -> Result<(), std::io::Error> {
    #[cfg(unix)]
    let res = f(socket2::SockRef::from(&stream.as_fd()));
    #[cfg(windows)]
    let res = f(socket2::SockRef::from(&stream.as_handle()));
    res
}

#[cfg(feature = "optimization")]
pub trait StreamOptimizationExt: Stream + Sized {
    /// Optimize the stream for the given optimization.
    #[cfg(feature = "optimization")]
    fn optimize_for(&mut self, optimization: StreamOptimization) -> Result<(), std::io::Error> {
        macro_rules! try_optimize(
            ( $s:ident . $method:ident ( $($args:tt)* ) ) => {
                $s.$method($($args)*).map_err(|e: std::io::Error| std::io::Error::new(e.kind(), format!("{}: {}", stringify!($method), e)))
            };
        );

        #[cfg(unix)]
        if self.transport() == Transport::Unix {
            return Ok(());
        }

        let mut with_socket2_fn = move |s: socket2::SockRef<'_>| {
            #[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))]
            try_optimize!(s.set_tcp_cork(false))?;

            #[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))]
            try_optimize!(s.set_tcp_quickack(false))?;

            #[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))]
            try_optimize!(s.set_tcp_thin_linear_timeouts(false))?;

            try_optimize!(s.set_send_buffer_size(256 * 1024))?;
            try_optimize!(s.set_recv_buffer_size(256 * 1024))?;

            match optimization {
                StreamOptimization::General => {
                    try_optimize!(s.set_tcp_nodelay(false))?;
                    #[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))]
                    try_optimize!(s.set_tcp_thin_linear_timeouts(true))?;
                }
                StreamOptimization::Interactive => {
                    try_optimize!(s.set_tcp_nodelay(true))?;
                    #[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))]
                    try_optimize!(s.set_tcp_thin_linear_timeouts(true))?;
                }
                StreamOptimization::BulkStreaming(direction) => {
                    try_optimize!(s.set_tcp_nodelay(false))?;
                    // Handle send buffer size
                    match direction {
                        BulkStreamDirection::Send | BulkStreamDirection::Both => {
                            try_optimize!(s.set_send_buffer_size(16 * 1024 * 1024))?;
                            #[cfg(any(
                                target_os = "android",
                                target_os = "fuchsia",
                                target_os = "linux"
                            ))]
                            try_optimize!(s.set_tcp_cork(true))?;
                        }
                        BulkStreamDirection::Receive => {}
                    }

                    // Handle receive buffer size
                    match direction {
                        BulkStreamDirection::Receive | BulkStreamDirection::Both => {
                            try_optimize!(s.set_recv_buffer_size(16 * 1024 * 1024))?;
                            #[cfg(any(
                                target_os = "android",
                                target_os = "fuchsia",
                                target_os = "linux"
                            ))]
                            try_optimize!(s.set_tcp_quickack(true))?;
                        }
                        BulkStreamDirection::Send => {}
                    }
                }
            }
            Ok(())
        };

        with_socket2(self, &mut with_socket2_fn)
    }
}

#[cfg(feature = "optimization")]
impl<S: Stream + Sized> StreamOptimizationExt for S {}

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
#[cfg_attr(feature = "tokio", derive(derive_io::AsyncWrite))]
#[derive(derive_more::Debug, derive_io::AsSocketDescriptor)]
pub struct UpgradableStream<S: Stream, D: TlsDriver = Ssl> {
    #[write]
    #[descriptor]
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
        }
    }
}

impl<S: Stream, D: TlsDriver> StreamUpgrade for UpgradableStream<S, D> {
    async fn secure_upgrade(self) -> Result<Self, SslError> {
        let (upgraded, handshake) = match self.inner {
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
            _ => {
                return Err(SslError::SslAlreadyUpgraded);
            }
        };
        Ok(Self {
            inner: UpgradableStreamInner::Upgraded(upgraded, handshake),
            options: self.options,
        })
    }

    async fn secure_upgrade_preview(
        self,
        options: PreviewConfiguration,
    ) -> Result<(Preview, Self), SslError> {
        let (mut upgraded, handshake) = match self.inner {
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
            _ => {
                return Err(SslError::SslAlreadyUpgraded);
            }
        };
        let mut buffer = smallvec::SmallVec::with_capacity(options.max_preview_bytes.get());
        buffer.resize(options.max_preview_bytes.get(), 0);
        #[cfg(feature = "tokio")]
        upgraded.read_exact(&mut buffer).await?;
        #[cfg(not(feature = "tokio"))]
        unimplemented!("Preview is not supported without tokio");
        let mut rewind = RewindStream::new(upgraded);
        rewind.rewind(&buffer);
        Ok((
            Preview { buffer },
            Self {
                inner: UpgradableStreamInner::UpgradedPreview(rewind, handshake),
                options: self.options,
            },
        ))
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
        };
        if ignore_missing_close_notify
            && matches!(res, std::task::Poll::Ready(Err(ref e)) if e.kind() == std::io::ErrorKind::UnexpectedEof)
        {
            return std::task::Poll::Ready(Ok(()));
        }
        res
    }
}

impl<S: Stream, D: TlsDriver> LocalAddress for UpgradableStream<S, D> {
    fn local_address(&self) -> std::io::Result<ResolvedTarget> {
        self.inner
            .with_inner_metadata(|inner| inner.local_address())
    }
}

impl<S: Stream, D: TlsDriver> RemoteAddress for UpgradableStream<S, D> {
    fn remote_address(&self) -> std::io::Result<ResolvedTarget> {
        self.inner
            .with_inner_metadata(|inner| inner.remote_address())
    }
}

impl<S: Stream, D: TlsDriver> StreamMetadata for UpgradableStream<S, D> {
    fn transport(&self) -> Transport {
        self.inner.with_inner_metadata(|inner| inner.transport())
    }
}

#[cfg_attr(feature = "tokio", derive(derive_io::AsyncRead, derive_io::AsyncWrite))]
#[derive(derive_more::Debug, derive_io::AsSocketDescriptor)]
enum UpgradableStreamInner<S: Stream, D: TlsDriver> {
    #[debug("BaseClient(..)")]
    BaseClient(
        #[read]
        #[write]
        #[descriptor]
        S,
        Option<D::ClientParams>,
    ),
    #[debug("BaseServer(..)")]
    BaseServer(
        #[read]
        #[write]
        #[descriptor]
        S,
        Option<TlsServerParameterProvider>,
    ),
    #[debug("Preview(..)")]
    BaseServerPreview(
        #[read]
        #[write]
        #[descriptor]
        RewindStream<S>,
        Option<TlsServerParameterProvider>,
    ),
    #[debug("Upgraded(..)")]
    Upgraded(
        #[read]
        #[write]
        #[descriptor]
        D::Stream,
        TlsHandshake,
    ),
    #[debug("Upgraded(..)")]
    UpgradedPreview(
        #[read]
        #[write]
        #[descriptor]
        RewindStream<D::Stream>,
        TlsHandshake,
    ),
}

impl<S: Stream, D: TlsDriver> UpgradableStreamInner<S, D> {
    #[inline(always)]
    fn with_inner_metadata<T>(&self, f: impl FnOnce(&dyn StreamMetadata) -> T) -> T {
        match self {
            UpgradableStreamInner::BaseClient(base, _) => f(base),
            UpgradableStreamInner::BaseServer(base, _) => f(base),
            UpgradableStreamInner::BaseServerPreview(base, _) => f(base),
            UpgradableStreamInner::Upgraded(upgraded, _) => f(upgraded),
            UpgradableStreamInner::UpgradedPreview(upgraded, _) => f(upgraded),
        }
    }

    #[inline(always)]
    fn as_inner_handle(&self) -> &dyn AsHandle {
        match self {
            UpgradableStreamInner::BaseClient(base, _) => base,
            UpgradableStreamInner::BaseServer(base, _) => base,
            UpgradableStreamInner::BaseServerPreview(base, _) => base,
            UpgradableStreamInner::Upgraded(upgraded, _) => upgraded,
            UpgradableStreamInner::UpgradedPreview(upgraded, _) => upgraded,
        }
    }
}

impl<S: Stream, D: TlsDriver> AsHandle for UpgradableStream<S, D> {
    #[cfg(windows)]
    fn as_handle(&self) -> std::os::windows::io::BorrowedSocket {
        self.inner.as_inner_handle().as_handle()
    }

    #[cfg(unix)]
    fn as_fd(&self) -> std::os::fd::BorrowedFd {
        self.inner.as_inner_handle().as_fd()
    }
}

pub trait Rewindable {
    fn rewind(&mut self, bytes: &[u8]) -> std::io::Result<()>;
}

/// A stream that can be rewound.
#[cfg_attr(feature = "tokio", derive(derive_io::AsyncWrite))]
#[derive(derive_more::Debug, derive_io::AsSocketDescriptor)]
pub(crate) struct RewindStream<S> {
    buffer: Vec<u8>,
    #[write]
    #[descriptor]
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

impl<S: Stream> Rewindable for RewindStream<S> {
    fn rewind(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.rewind(bytes);
        Ok(())
    }
}

impl<S: LocalAddress> LocalAddress for RewindStream<S> {
    fn local_address(&self) -> std::io::Result<ResolvedTarget> {
        self.inner.local_address()
    }
}

impl<S: RemoteAddress> RemoteAddress for RewindStream<S> {
    fn remote_address(&self) -> std::io::Result<ResolvedTarget> {
        self.inner.remote_address()
    }
}

impl<S: PeerCred> PeerCred for RewindStream<S> {
    #[cfg(all(unix, feature = "tokio"))]
    fn peer_cred(&self) -> std::io::Result<tokio::net::unix::UCred> {
        self.inner.peer_cred()
    }
}

impl<S: StreamMetadata> StreamMetadata for RewindStream<S> {
    fn transport(&self) -> Transport {
        self.inner.transport()
    }
}

impl<S: PeekableStream> PeekableStream for RewindStream<S> {
    #[cfg(feature = "tokio")]
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

impl<S: Stream + AsHandle> AsHandle for RewindStream<S> {
    #[cfg(windows)]
    fn as_handle(&self) -> std::os::windows::io::BorrowedSocket {
        self.inner.as_handle()
    }

    #[cfg(unix)]
    fn as_fd(&self) -> std::os::fd::BorrowedFd {
        self.inner.as_fd()
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
            UpgradableStreamInner::BaseServerPreview(stm, _) => {
                stm.rewind(bytes);
                Ok(())
            }
            UpgradableStreamInner::Upgraded(stm, _) => stm.rewind(bytes),
            UpgradableStreamInner::UpgradedPreview(stm, _) => {
                stm.rewind(bytes);
                Ok(())
            }
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
        }
    }
}

impl<S: PeerCred + Stream, D: TlsDriver> PeerCred for UpgradableStream<S, D> {
    #[cfg(all(unix, feature = "tokio"))]
    fn peer_cred(&self) -> std::io::Result<tokio::net::unix::UCred> {
        self.inner.with_inner_metadata(|inner| inner.peer_cred())
    }
}
