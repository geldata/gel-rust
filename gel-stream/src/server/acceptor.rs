use crate::{
    common::tokio_stream::TokioListenerStream, ConnectionError, LocalAddress, Preview,
    PreviewConfiguration, ResolvedTarget, RewindStream, Ssl, StreamUpgrade, TlsDriver,
    TlsServerParameterProvider, UpgradableStream, DEFAULT_TLS_BACKLOG,
};
use futures::{stream::FuturesUnordered, StreamExt};
use std::{
    future::Future,
    pin::Pin,
    task::{ready, Poll},
};
use std::{net::SocketAddr, path::Path};
use tokio::io::AsyncReadExt;

type Connection<D = Ssl> = UpgradableStream<crate::BaseStream, D>;

pub struct Acceptor<const PREVIEW: bool = false> {
    resolved_target: ResolvedTarget,
    tls_provider: Option<TlsServerParameterProvider>,
    should_upgrade: bool,
    options: StreamOptions<PREVIEW>,
}

#[derive(Debug, Clone, Copy)]
struct StreamOptions<const PREVIEW: bool> {
    ignore_missing_tls_close_notify: bool,
    preview_configuration: Option<PreviewConfiguration>,
    tcp_backlog: Option<u32>,
    tls_backlog: Option<u32>,
}

impl<const PREVIEW: bool> Default for StreamOptions<PREVIEW> {
    fn default() -> Self {
        Self {
            ignore_missing_tls_close_notify: false,
            preview_configuration: None,
            tcp_backlog: None,
            tls_backlog: None,
        }
    }
}

impl Acceptor<false> {
    pub fn new(target: ResolvedTarget) -> Self {
        Self {
            resolved_target: target,
            tls_provider: None,
            should_upgrade: false,
            options: Default::default(),
        }
    }

    pub fn new_tls(target: ResolvedTarget, provider: TlsServerParameterProvider) -> Self {
        Self {
            resolved_target: target,
            tls_provider: Some(provider),
            should_upgrade: true,
            options: Default::default(),
        }
    }

    pub fn new_starttls(target: ResolvedTarget, provider: TlsServerParameterProvider) -> Self {
        Self {
            resolved_target: target,
            tls_provider: Some(provider),
            should_upgrade: false,
            options: Default::default(),
        }
    }

    pub fn new_tcp(addr: SocketAddr) -> Self {
        Self {
            resolved_target: ResolvedTarget::SocketAddr(addr),
            tls_provider: None,
            should_upgrade: false,
            options: Default::default(),
        }
    }

    pub fn new_tcp_tls(addr: SocketAddr, provider: TlsServerParameterProvider) -> Self {
        Self {
            resolved_target: ResolvedTarget::SocketAddr(addr),
            tls_provider: Some(provider),
            should_upgrade: true,
            options: Default::default(),
        }
    }

    pub fn new_tcp_starttls(addr: SocketAddr, provider: TlsServerParameterProvider) -> Self {
        Self {
            resolved_target: ResolvedTarget::SocketAddr(addr),
            tls_provider: Some(provider),
            should_upgrade: false,
            options: Default::default(),
        }
    }

    pub fn new_unix_path(path: impl AsRef<Path>) -> Result<Self, std::io::Error> {
        #[cfg(unix)]
        {
            Ok(Self {
                resolved_target: ResolvedTarget::from(
                    std::os::unix::net::SocketAddr::from_pathname(path)?,
                ),
                tls_provider: None,
                should_upgrade: false,
                options: Default::default(),
            })
        }
        #[cfg(not(unix))]
        {
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Unix domain sockets are not supported on this platform",
            ))
        }
    }

    pub fn new_unix_domain(domain: impl AsRef<[u8]>) -> Result<Self, std::io::Error> {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            use std::os::linux::net::SocketAddrExt;
            Ok(Self {
                resolved_target: ResolvedTarget::from(
                    std::os::unix::net::SocketAddr::from_abstract_name(domain)?,
                ),
                tls_provider: None,
                should_upgrade: false,
                options: Default::default(),
            })
        }
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        {
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Unix domain sockets are not supported on this platform",
            ))
        }
    }

    pub async fn bind(
        self,
    ) -> Result<
        impl ::futures::Stream<Item = Result<Connection, ConnectionError>> + LocalAddress,
        ConnectionError,
    > {
        let stream = self
            .resolved_target
            .listen_raw(self.options.tcp_backlog)
            .await?;
        Ok(AcceptedStream::<Connection<Ssl>> {
            stream,
            should_upgrade: self.should_upgrade,
            ignore_missing_tls_close_notify: self.options.ignore_missing_tls_close_notify,
            tls_provider: self.tls_provider,
            tls_backlog: TlsAcceptBacklog::new(
                self.options.tls_backlog.unwrap_or(DEFAULT_TLS_BACKLOG) as _,
            ),
            preview_configuration: None,
            _phantom: None,
        })
    }

    #[allow(private_bounds)]
    pub async fn bind_explicit<D: TlsDriver>(
        self,
    ) -> Result<
        impl ::futures::Stream<Item = Result<Connection<D>, ConnectionError>> + LocalAddress,
        ConnectionError,
    > {
        let stream = self
            .resolved_target
            .listen_raw(self.options.tcp_backlog)
            .await?;
        Ok(AcceptedStream::<Connection<D>, D> {
            stream,
            ignore_missing_tls_close_notify: self.options.ignore_missing_tls_close_notify,
            should_upgrade: self.should_upgrade,
            tls_provider: self.tls_provider,
            tls_backlog: TlsAcceptBacklog::new(
                self.options.tls_backlog.unwrap_or(DEFAULT_TLS_BACKLOG) as _,
            ),
            preview_configuration: None,
            _phantom: None,
        })
    }

    /// Listen, and then accept one and only one connection from the listener.
    pub async fn accept_one(self) -> Result<Connection, ConnectionError> {
        let Some(conn) = self.bind().await?.next().await else {
            return Err(ConnectionError::Io(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "No connection received",
            )));
        };
        conn
    }
}

impl Acceptor<true> {
    /// Create a new TCP/TLS acceptor that will preview the first
    /// [`PreviewConfiguration::max_preview_bytes`] bytes of the connection.
    pub fn new_tcp_tls_previewing(
        addr: SocketAddr,
        preview_configuration: PreviewConfiguration,
        provider: TlsServerParameterProvider,
    ) -> Self {
        Self {
            resolved_target: ResolvedTarget::SocketAddr(addr),
            tls_provider: Some(provider),
            should_upgrade: false,
            options: StreamOptions {
                preview_configuration: Some(preview_configuration),
                ..Default::default()
            },
        }
    }

    /// Create a new acceptor that will preview the first
    /// [`PreviewConfiguration::max_preview_bytes`] bytes of the connection.
    pub fn new_tls_previewing(
        addr: ResolvedTarget,
        preview_configuration: PreviewConfiguration,
        provider: TlsServerParameterProvider,
    ) -> Self {
        Self {
            resolved_target: addr,
            tls_provider: Some(provider),
            should_upgrade: false,
            options: StreamOptions {
                preview_configuration: Some(preview_configuration),
                ..Default::default()
            },
        }
    }

    /// Create a new acceptor that will preview the first
    /// [`PreviewConfiguration::max_preview_bytes`] bytes of the connection.
    pub fn new_previewing(
        addr: ResolvedTarget,
        preview_configuration: PreviewConfiguration,
    ) -> Self {
        Self {
            resolved_target: addr,
            tls_provider: None,
            should_upgrade: false,
            options: StreamOptions {
                preview_configuration: Some(preview_configuration),
                ..Default::default()
            },
        }
    }

    pub async fn bind(
        self,
    ) -> Result<
        impl ::futures::Stream<Item = Result<(Preview, Connection), ConnectionError>> + LocalAddress,
        ConnectionError,
    > {
        let stream = self
            .resolved_target
            .listen_raw(self.options.tcp_backlog)
            .await?;
        Ok(AcceptedStream::<(Preview, Connection<Ssl>)> {
            stream,
            should_upgrade: self.should_upgrade,
            ignore_missing_tls_close_notify: self.options.ignore_missing_tls_close_notify,
            tls_provider: self.tls_provider,
            tls_backlog: TlsAcceptBacklog::new(self.options.tls_backlog.unwrap_or(128) as _),
            preview_configuration: self.options.preview_configuration,
            _phantom: None,
        })
    }

    #[allow(private_bounds)]
    pub async fn bind_explicit<D: TlsDriver>(
        self,
    ) -> Result<
        impl ::futures::Stream<Item = Result<(Preview, Connection<D>), ConnectionError>> + LocalAddress,
        ConnectionError,
    > {
        let stream = self
            .resolved_target
            .listen_raw(self.options.tcp_backlog)
            .await?;
        Ok(AcceptedStream::<(Preview, Connection<D>), D> {
            stream,
            should_upgrade: self.should_upgrade,
            ignore_missing_tls_close_notify: self.options.ignore_missing_tls_close_notify,
            tls_provider: self.tls_provider,
            tls_backlog: TlsAcceptBacklog::new(
                self.options.tls_backlog.unwrap_or(DEFAULT_TLS_BACKLOG) as _,
            ),
            preview_configuration: self.options.preview_configuration,
            _phantom: None,
        })
    }

    /// Listen, and then accept one and only one connection from the listener.
    pub async fn accept_one(self) -> Result<(Preview, Connection), ConnectionError> {
        let Some(conn) = self.bind().await?.next().await else {
            return Err(ConnectionError::Io(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "No connection received",
            )));
        };
        conn
    }
}

struct AcceptedStream<S, D: TlsDriver = Ssl> {
    stream: TokioListenerStream,
    should_upgrade: bool,
    ignore_missing_tls_close_notify: bool,
    tls_provider: Option<TlsServerParameterProvider>,
    tls_backlog: TlsAcceptBacklog<S>,
    preview_configuration: Option<PreviewConfiguration>,
    // Avoid using PhantomData because it fails to implement certain auto-traits
    _phantom: Option<&'static D>,
}

impl<S, D: TlsDriver> LocalAddress for AcceptedStream<S, D> {
    fn local_address(&self) -> std::io::Result<ResolvedTarget> {
        self.stream.local_address()
    }
}

impl<D: TlsDriver> futures::Stream for AcceptedStream<Connection<D>, D> {
    type Item = Result<Connection<D>, ConnectionError>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let ignore_missing_tls_close_notify = self.ignore_missing_tls_close_notify;
        let make_stream = move |tls_provider: Option<TlsServerParameterProvider>, stream| {
            let mut stream = UpgradableStream::<_, D>::new_server(stream, tls_provider);
            if ignore_missing_tls_close_notify {
                stream.ignore_missing_close_notify();
            }
            stream
        };

        // If we're not upgrading, we can just return the stream as is and skip
        // the second-level backlog.
        if !self.should_upgrade {
            return self.as_mut().stream.poll_next_unpin(cx).map(|c| {
                c.map(|c| Ok(c.map(|(c, _t)| make_stream(self.tls_provider.clone(), c))?))
            });
        }

        // Fill the backlog to capacity as log as we have connections to accept.
        while !self.tls_backlog.is_full() {
            let Poll::Ready(r) = self.stream.poll_next_unpin(cx) else {
                if self.tls_backlog.is_empty() {
                    return Poll::Pending;
                }
                break;
            };

            let Some((stream, _t)) = r.transpose()? else {
                if self.tls_backlog.is_empty() {
                    return Poll::Ready(None);
                }
                break;
            };

            let tls_provider = self.tls_provider.clone();
            self.tls_backlog.push(async move {
                let stream = make_stream(tls_provider, stream);
                let stream = stream.secure_upgrade().await?;
                Ok(stream)
            })
        }

        // We've got at least one pending connection here
        debug_assert!(!self.tls_backlog.is_empty());
        let r = ready!(Pin::new(&mut self.tls_backlog).poll_next(cx))?;
        Poll::Ready(Some(Ok(r)))
    }
}

impl<D: TlsDriver> futures::Stream for AcceptedStream<(Preview, Connection<D>), D> {
    type Item = Result<(Preview, Connection<D>), ConnectionError>;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        // Fill the backlog to capacity as log as we have connections to accept.
        while !self.tls_backlog.is_full() {
            let Poll::Ready(r) = self.stream.poll_next_unpin(cx) else {
                if self.tls_backlog.is_empty() {
                    return Poll::Pending;
                }
                break;
            };

            let Some((mut stream, _t)) = r.transpose()? else {
                if self.tls_backlog.is_empty() {
                    return Poll::Ready(None);
                }
                break;
            };

            let tls_provider = self.tls_provider.clone();
            let preview_configuration = self.preview_configuration.unwrap();
            let ignore_missing_tls_close_notify = self.ignore_missing_tls_close_notify;
            self.tls_backlog.push(async move {
                let mut buf = smallvec::SmallVec::with_capacity(
                    preview_configuration.max_preview_bytes.get(),
                );
                buf.resize(preview_configuration.max_preview_bytes.get(), 0);
                stream.read_exact(&mut buf).await?;
                let mut stream = RewindStream::new(stream);
                stream.rewind(&buf);
                let preview = Preview::new(buf);
                let mut stream = UpgradableStream::<_, D>::new_server_preview(stream, tls_provider);
                if ignore_missing_tls_close_notify {
                    stream.ignore_missing_close_notify();
                }

                Ok((preview, stream))
            })
        }

        // We've got at least one pending connection here
        debug_assert!(!self.tls_backlog.is_empty());
        let r = ready!(Pin::new(&mut self.tls_backlog).poll_next(cx))?;
        Poll::Ready(Some(Ok(r)))
    }
}

struct TlsAcceptBacklog<C> {
    capacity: usize,
    #[allow(clippy::type_complexity)]
    futures: FuturesUnordered<
        Pin<Box<dyn Future<Output = Result<C, ConnectionError>> + Send + 'static>>,
    >,
}

impl<C> TlsAcceptBacklog<C> {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            futures: FuturesUnordered::new(),
        }
    }

    fn is_full(&self) -> bool {
        self.futures.len() >= self.capacity
    }

    fn is_empty(&self) -> bool {
        self.futures.len() == 0
    }

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<C, ConnectionError>> {
        debug_assert!(!self.is_empty());
        self.futures.poll_next_unpin(cx).map(|r| r.unwrap())
    }

    fn push(&mut self, future: impl Future<Output = Result<C, ConnectionError>> + Send + 'static) {
        self.futures.push(Box::pin(future));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Connector, OpensslDriver, RustlsDriver, Target, TlsParameters, TlsServerParameters,
    };
    use std::net::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn test_acceptor_new_tcp_previewing<D: TlsDriver>() -> Result<(), ConnectionError> {
        let acceptor = Acceptor::new_tcp_tls_previewing(
            SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
            PreviewConfiguration::default(),
            TlsServerParameterProvider::new(TlsServerParameters::new_with_certificate(
                crate::test_keys::SERVER_KEY.clone_key(),
            )),
        );

        let mut conns = acceptor.bind().await?;

        let addr = conns.local_address()?;
        tokio::task::spawn(async move {
            let mut conn = Connector::new_resolved(addr).connect().await?;
            conn.write_all(b"HELLO WORLD").await
        });

        let (preview, mut conn) = conns.next().await.unwrap()?;
        assert_eq!(preview.len(), 8);
        assert_eq!(preview, b"HELLO WO");
        let mut string = String::new();
        conn.read_to_string(&mut string).await?;
        assert_eq!(string, "HELLO WORLD");

        let addr = conns.local_address()?;
        tokio::task::spawn(async move {
            let target = Target::new_resolved_tls(addr, TlsParameters::insecure());
            let mut conn = Connector::new(target)?.connect().await?;
            conn.write_all(b"HELLO WORLD").await
        });

        let (preview, conn) = conns.next().await.unwrap()?;
        assert_eq!(preview.len(), 8);
        assert!(matches!(preview.as_ref(), [0x16, 3, 1, ..]));
        let (preview, mut conn) = conn
            .secure_upgrade_preview(PreviewConfiguration::default())
            .await?;
        assert_eq!(preview.len(), 8);
        assert_eq!(preview, b"HELLO WO");

        let mut string = String::new();
        conn.read_to_string(&mut string).await?;
        assert_eq!(string, "HELLO WORLD");

        Ok(())
    }

    #[tokio::test]
    async fn test_acceptor_new_tcp_previewing_openssl() -> Result<(), ConnectionError> {
        test_acceptor_new_tcp_previewing::<OpensslDriver>().await
    }

    #[tokio::test]
    async fn test_acceptor_new_tcp_previewing_rustls() -> Result<(), ConnectionError> {
        test_acceptor_new_tcp_previewing::<RustlsDriver>().await
    }
}
