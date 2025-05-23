//! This module provides functionality to connect to Tokio TCP and Unix sockets.

use std::pin::Pin;
use std::task::{ready, Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream};

use crate::PeekableStream;

use super::target::{LocalAddress, ResolvedTarget};

impl ResolvedTarget {
    #[cfg(feature = "client")]
    /// Connects to the socket address and returns a [`TokioStream`].
    pub async fn connect(&self) -> std::io::Result<TokioStream> {
        match self {
            ResolvedTarget::SocketAddr(addr) => {
                let stream = TcpStream::connect(addr).await?;
                Ok(TokioStream::Tcp(stream))
            }
            #[cfg(unix)]
            ResolvedTarget::UnixSocketAddr(path) => {
                let stm = std::os::unix::net::UnixStream::connect_addr(path)?;
                stm.set_nonblocking(true)?;
                let stream = UnixStream::from_std(stm)?;
                Ok(TokioStream::Unix(stream))
            }
        }
    }

    #[cfg(feature = "server")]
    pub async fn listen(
        &self,
    ) -> std::io::Result<
        impl futures::Stream<Item = std::io::Result<(TokioStream, ResolvedTarget)>> + LocalAddress,
    > {
        self.listen_raw(None).await
    }

    #[cfg(feature = "server")]
    pub async fn listen_backlog(
        &self,
        backlog: usize,
    ) -> std::io::Result<
        impl futures::Stream<Item = std::io::Result<(TokioStream, ResolvedTarget)>> + LocalAddress,
    > {
        if !self.is_tcp() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Unix sockets do not support a connectionbacklog",
            ));
        }
        let backlog = u32::try_from(backlog)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
        self.listen_raw(Some(backlog)).await
    }

    /// Listens for incoming connections on the socket address and returns a
    /// [`futures::Stream`] of [`TokioStream`]s and the incoming address.
    #[cfg(feature = "server")]
    pub(crate) async fn listen_raw(
        &self,
        backlog: Option<u32>,
    ) -> std::io::Result<TokioListenerStream> {
        use std::net::SocketAddr;

        use tokio::net::TcpSocket;

        use crate::DEFAULT_TCP_BACKLOG;

        match self {
            ResolvedTarget::SocketAddr(addr) => {
                let backlog = backlog.unwrap_or(DEFAULT_TCP_BACKLOG);
                let socket = match addr {
                    SocketAddr::V4(..) => TcpSocket::new_v4()?,
                    SocketAddr::V6(..) => TcpSocket::new_v6()?,
                };
                socket.bind(*addr)?;
                let listener = socket.listen(backlog)?;

                Ok(TokioListenerStream::Tcp(listener))
            }
            #[cfg(unix)]
            ResolvedTarget::UnixSocketAddr(path) => {
                let listener = std::os::unix::net::UnixListener::bind_addr(path)?;
                listener.set_nonblocking(true)?;
                let listener = tokio::net::UnixListener::from_std(listener)?;
                Ok(TokioListenerStream::Unix(listener))
            }
        }
    }
}

pub(crate) enum TokioListenerStream {
    Tcp(TcpListener),
    #[cfg(unix)]
    Unix(UnixListener),
}

impl LocalAddress for TokioListenerStream {
    fn local_address(&self) -> std::io::Result<ResolvedTarget> {
        match self {
            TokioListenerStream::Tcp(listener) => {
                listener.local_addr().map(ResolvedTarget::SocketAddr)
            }
            #[cfg(unix)]
            TokioListenerStream::Unix(listener) => listener
                .local_addr()
                .map(|addr| ResolvedTarget::UnixSocketAddr(addr.into())),
        }
    }
}

impl futures::Stream for TokioListenerStream {
    type Item = std::io::Result<(TokioStream, ResolvedTarget)>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.get_mut() {
            TokioListenerStream::Tcp(listener) => {
                let (stream, addr) = ready!(listener.poll_accept(cx))?;
                let stream = TokioStream::Tcp(stream);
                let target = ResolvedTarget::SocketAddr(addr);
                Poll::Ready(Some(Ok((stream, target))))
            }
            #[cfg(unix)]
            TokioListenerStream::Unix(listener) => {
                let (stream, addr) = ready!(listener.poll_accept(cx))?;
                let stream = TokioStream::Unix(stream);
                let target = ResolvedTarget::UnixSocketAddr(addr.into());
                Poll::Ready(Some(Ok((stream, target))))
            }
        }
    }
}

/// Represents a connected Tokio stream, either TCP or Unix
pub enum TokioStream {
    /// TCP stream
    Tcp(TcpStream),
    /// Unix stream (only available on Unix systems)
    #[cfg(unix)]
    Unix(UnixStream),
}

impl TokioStream {
    #[cfg(feature = "keepalive")]
    pub fn set_keepalive(&self, keepalive: Option<std::time::Duration>) -> std::io::Result<()> {
        use socket2::*;
        match self {
            TokioStream::Tcp(stream) => {
                let sock = socket2::SockRef::from(&stream);
                if let Some(keepalive) = keepalive {
                    sock.set_tcp_keepalive(
                        &TcpKeepalive::new()
                            .with_interval(keepalive)
                            .with_time(keepalive),
                    )
                } else {
                    sock.set_keepalive(false)
                }
            }
            #[cfg(unix)]
            TokioStream::Unix(_) => Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Unix sockets do not support keepalive",
            )),
        }
    }
}

impl AsyncRead for TokioStream {
    #[inline(always)]
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            TokioStream::Tcp(stream) => Pin::new(stream).poll_read(cx, buf),
            #[cfg(unix)]
            TokioStream::Unix(stream) => Pin::new(stream).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for TokioStream {
    #[inline(always)]
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        match self.get_mut() {
            TokioStream::Tcp(stream) => Pin::new(stream).poll_write(cx, buf),
            #[cfg(unix)]
            TokioStream::Unix(stream) => Pin::new(stream).poll_write(cx, buf),
        }
    }

    #[inline(always)]
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        match self.get_mut() {
            TokioStream::Tcp(stream) => Pin::new(stream).poll_flush(cx),
            #[cfg(unix)]
            TokioStream::Unix(stream) => Pin::new(stream).poll_flush(cx),
        }
    }

    #[inline(always)]
    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        match self.get_mut() {
            TokioStream::Tcp(stream) => Pin::new(stream).poll_shutdown(cx),
            #[cfg(unix)]
            TokioStream::Unix(stream) => Pin::new(stream).poll_shutdown(cx),
        }
    }

    #[inline(always)]
    fn is_write_vectored(&self) -> bool {
        match self {
            TokioStream::Tcp(stream) => stream.is_write_vectored(),
            #[cfg(unix)]
            TokioStream::Unix(stream) => stream.is_write_vectored(),
        }
    }

    #[inline(always)]
    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> Poll<Result<usize, std::io::Error>> {
        match self.get_mut() {
            TokioStream::Tcp(stream) => Pin::new(stream).poll_write_vectored(cx, bufs),
            #[cfg(unix)]
            TokioStream::Unix(stream) => Pin::new(stream).poll_write_vectored(cx, bufs),
        }
    }
}

impl PeekableStream for TokioStream {
    fn poll_peek(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<usize>> {
        match self.get_mut() {
            TokioStream::Tcp(stream) => Pin::new(stream).poll_peek(cx, buf),
            #[cfg(unix)]
            TokioStream::Unix(stream) => loop {
                ready!(stream.poll_read_ready(cx))?;
                let sock = socket2::SockRef::from(&*stream);
                break match sock.recv_with_flags(unsafe { buf.unfilled_mut() }, libc::MSG_PEEK) {
                    Ok(n) => Poll::Ready(Ok(n)),
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        continue;
                    }
                    Err(e) => Poll::Ready(Err(e)),
                };
            },
        }
    }
}
