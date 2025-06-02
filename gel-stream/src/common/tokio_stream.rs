//! This module provides functionality to connect to Tokio TCP and Unix sockets.

use std::pin::Pin;
use std::task::{ready, Context, Poll};
use tokio::net::{TcpListener, TcpStream};
#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream};

use crate::{AsHandle, PeekableStream, PeerCred, RemoteAddress, StreamMetadata, Transport};

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
#[derive(derive_io::AsyncRead, derive_io::AsyncWrite, derive_io::AsSocketDescriptor)]
pub enum TokioStream {
    /// TCP stream
    Tcp(
        #[read]
        #[write]
        #[descriptor]
        TcpStream,
    ),
    /// Unix stream (only available on Unix systems)
    #[cfg(unix)]
    Unix(
        #[read]
        #[write]
        #[descriptor]
        UnixStream,
    ),
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

impl AsHandle for TokioStream {
    #[cfg(windows)]
    fn as_handle(&self) -> std::os::windows::io::BorrowedSocket {
        <Self as std::os::windows::io::AsSocket>::as_handle(self)
    }

    #[cfg(unix)]
    fn as_fd(&self) -> std::os::fd::BorrowedFd {
        <Self as std::os::fd::AsFd>::as_fd(self)
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

impl LocalAddress for TokioStream {
    fn local_address(&self) -> std::io::Result<ResolvedTarget> {
        match self {
            TokioStream::Tcp(stream) => <TcpStream as LocalAddress>::local_address(stream),
            #[cfg(unix)]
            TokioStream::Unix(stream) => <UnixStream as LocalAddress>::local_address(stream),
        }
    }
}

impl RemoteAddress for TokioStream {
    fn remote_address(&self) -> std::io::Result<ResolvedTarget> {
        match self {
            TokioStream::Tcp(stream) => <TcpStream as RemoteAddress>::remote_address(stream),
            #[cfg(unix)]
            TokioStream::Unix(stream) => <UnixStream as RemoteAddress>::remote_address(stream),
        }
    }
}

impl PeerCred for TokioStream {
    #[cfg(all(unix, feature = "tokio"))]
    fn peer_cred(&self) -> std::io::Result<tokio::net::unix::UCred> {
        match self {
            TokioStream::Unix(unix) => unix.peer_cred(),
            TokioStream::Tcp(_) => Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "TCP sockets do not support peer credentials",
            )),
        }
    }
}

impl StreamMetadata for TokioStream {
    fn transport(&self) -> Transport {
        match self {
            TokioStream::Tcp(_) => Transport::Tcp,
            #[cfg(unix)]
            TokioStream::Unix(_) => Transport::Unix,
        }
    }
}

impl LocalAddress for TcpStream {
    fn local_address(&self) -> std::io::Result<ResolvedTarget> {
        self.local_addr().map(ResolvedTarget::SocketAddr)
    }
}

impl RemoteAddress for TcpStream {
    fn remote_address(&self) -> std::io::Result<ResolvedTarget> {
        self.peer_addr().map(ResolvedTarget::SocketAddr)
    }
}

impl PeerCred for TcpStream {
    #[cfg(all(unix, feature = "tokio"))]
    fn peer_cred(&self) -> std::io::Result<tokio::net::unix::UCred> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "TCP sockets do not support peer credentials",
        ))
    }
}

impl StreamMetadata for TcpStream {
    fn transport(&self) -> Transport {
        Transport::Tcp
    }
}

impl AsHandle for TcpStream {
    #[cfg(windows)]
    fn as_handle(&self) -> std::os::windows::io::BorrowedSocket {
        <Self as std::os::windows::io::AsSocket>::as_handle(self)
    }

    #[cfg(unix)]
    fn as_fd(&self) -> std::os::fd::BorrowedFd {
        <Self as std::os::fd::AsFd>::as_fd(self)
    }
}

#[cfg(unix)]
impl LocalAddress for UnixStream {
    fn local_address(&self) -> std::io::Result<ResolvedTarget> {
        self.local_addr()
            .map(|addr| ResolvedTarget::UnixSocketAddr(addr.into()))
    }
}

#[cfg(unix)]
impl RemoteAddress for UnixStream {
    fn remote_address(&self) -> std::io::Result<ResolvedTarget> {
        self.peer_addr()
            .map(|addr| ResolvedTarget::UnixSocketAddr(addr.into()))
    }
}

#[cfg(unix)]
impl PeerCred for UnixStream {
    fn peer_cred(&self) -> std::io::Result<tokio::net::unix::UCred> {
        self.peer_cred()
    }
}

#[cfg(unix)]
impl StreamMetadata for UnixStream {
    fn transport(&self) -> Transport {
        Transport::Unix
    }
}

#[cfg(unix)]
impl AsHandle for UnixStream {
    fn as_fd(&self) -> std::os::fd::BorrowedFd {
        <Self as std::os::fd::AsFd>::as_fd(self)
    }
}
