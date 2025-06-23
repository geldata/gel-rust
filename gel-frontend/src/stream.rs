use crate::{
    hyper::{HyperStream, HyperUpgradedStream},
    stream_type::{PREFACE_SIZE, StreamType, known_protocol},
};
use base64::write;
use consume_on_drop::{Consume, ConsumeOnDrop};
use gel_stream::{
    Preview, PreviewConfiguration, RawStream, RemoteAddress, ResolvedTarget, StreamUpgrade,
};
use hyper::{HeaderMap, Version};
use std::{collections::HashMap, mem::MaybeUninit, sync::Arc, time::Duration};
use strum::IntoDiscriminant;
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadBuf};
use tracing::{trace, warn};
use unflatter::UnwrapAllOrDefaultExt;

macro_rules! stream_properties {
    (
        $(#[doc=$doc:literal] $(#[$attr:meta])* pub $name:ident: $type:ty),+ $(,)?
    ) => {
        #[derive(Clone, Default, Debug)]
        pub struct StreamPropertiesBuilder {
            $(
                #[doc=$doc]
                $(#[$attr])*
                pub $name: $type,
            )+
        }

        pub struct StreamProperties {
            /// A parent transport, if one exists
            pub parent: Option<Arc<StreamProperties>>,
            /// The underlying transport type
            pub transport: TransportType,
            $(
                #[doc=$doc]
                $(#[$attr])*
                pub $name: $type,
            )+
        }

        impl StreamProperties {
            pub fn new(transport: TransportType) -> Self {
                StreamProperties {
                    parent: None,
                    transport,
                    $($(#[$attr])* $name: None),+
                }
            }

            pub fn upgrade(self: Arc<Self>, props: StreamPropertiesBuilder) -> Arc<Self> {
                let transport = self.transport;
                Arc::new(StreamProperties {
                    parent: Some(self),
                    transport,
                    $($(#[$attr])* $name: props.$name),+
                })
            }

            $(
                $(#[$attr])*
                pub fn $name(&self) -> &$type {
                    &self.$name
                }
            )+
        }

        impl std::fmt::Debug for StreamProperties {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let mut debug_struct = f.debug_struct("StreamProperties");

                debug_struct.field("transport", &self.transport);
                $(
                    $(#[$attr])*
                    if let Some($name) = &self.$name {
                        debug_struct.field(stringify!($name), $name);
                    }
                )+
                if let Some(parent) = &self.parent {
                    debug_struct.field("parent", parent);
                }

                debug_struct.finish()
            }
        }
    };
}

stream_properties! {
    /// The language or protocol of the stream
    pub language: Option<StreamType>,
    /// The local address of the connection
    pub local_addr: Option<ResolvedTarget>,
    /// The peer address of the connection
    pub peer_addr: Option<ResolvedTarget>,
    /// The peer credentials (for Unix domain sockets)
    #[cfg(unix)]
    pub peer_creds: Option<tokio::net::unix::UCred>,
    /// The HTTP version used (for HTTP connections)
    pub http_version: Option<Version>,
    /// The HTTP request headers (for HTTP connections)
    pub request_headers: Option<HeaderMap>,
    /// The stream parameters (for PG/EDB connections)
    pub stream_params: Option<HashMap<String, String>>,
    /// The peer's SSL certificate (for SSL connections)
    pub peer_certificate: Option<x509_parser::prelude::X509Certificate<'static>>,
    /// The SSL/TLS version.
    pub ssl_version: Option<gel_stream::SslVersion>,
    /// The SSL/TLS version.
    pub ssl_cipher_name: Option<&'static str>,
    /// The Server Name Indication (SNI) provided by the client (for SSL connections)
    pub server_name_indication: Option<String>,
    /// The negotiated protocol (e.g., for ALPN in SSL connections, protocol for WebSocket)
    pub protocol: Option<&'static str>,
}

/// As we may be dealing with multiple types of streams, we have one top-level stream type that
/// dispatches to the appropriate underlying stream type.
#[derive(derive_io::AsyncRead, derive_io::AsyncWrite)]
pub struct ListenerStream {
    preview: Option<Preview>,
    stream_properties: Arc<StreamProperties>,
    #[read(deref)]
    #[write(deref)]
    inner: ConsumeOnDrop<ListenerStreamInner>,
}

#[derive(derive_io::AsyncRead, derive_io::AsyncWrite, strum::EnumDiscriminants)]
#[strum_discriminants(name(TransportType))]
pub enum ListenerStreamInner {
    /// Raw TCP.
    Tcp(
        #[read]
        #[write]
        gel_stream::RawStream,
    ),
    /// Unix
    Unix(
        #[read]
        #[write]
        gel_stream::RawStream,
    ),
    /// SSL
    Ssl(
        #[read]
        #[write]
        gel_stream::RawStream,
    ),
    /// Stream tunneled through HTTP request/response.
    Http(
        #[read]
        #[write]
        HyperStream,
    ),
    /// Upgraded stream (WebSocket, CONNECT, etc).
    WebSocket(
        #[read]
        #[write]
        HyperUpgradedStream,
    ),
}

const DRAIN_IDLE_TIMEOUT: Duration = Duration::from_millis(100);
const DRAIN_TIMEOUT: Duration = Duration::from_secs(10);
const DRAIN_MAX_SIZE: usize = 1024 * 1024;

/// Read any remaining incoming data on the socket before shutting down, up to 1
/// MB total. This draining process is necessary to handle TCP connection
/// teardown gracefully.
///
/// When we close a socket while there is still unread data in the receive
/// buffer, the peer may receive a connection reset error (ECONNRESET) instead
/// of a clean close. To avoid this, we read any pending data until the stream
/// is idle, we've read [`DRAIN_MAX_SIZE`] or [`DRAIN_TIMEOUT`] has elapsed (the
/// latter is checked outside of this function).
///
/// This gives the peer a chance to finish sending any in-flight data before we
/// terminate the connection. The timeout prevents us from waiting indefinitely
/// if the peer keeps sending data and prevents resource exhaustion attacks.
///
/// We don't care about the data, we just want to read it.
async fn drain_stream(mut socket: RawStream) {
    let mut buf = [MaybeUninit::uninit(); 1024];
    let mut read = 0;
    while read < DRAIN_MAX_SIZE {
        let mut buf = ReadBuf::uninit(&mut buf);
        // Consider a stream idle if no data arrives within `DRAIN_IDLE_TIMEOUT`.
        let n = tokio::time::timeout(DRAIN_IDLE_TIMEOUT, socket.read_buf(&mut buf))
            .await
            .unwrap_all_or_default();
        if n > 0 {
            read += n;
        } else {
            return;
        }
    }
}

/// Consume the stream, draining it to avoid `ECONNRESET`.
impl Consume for ListenerStreamInner {
    fn consume(self) {
        match self {
            ListenerStreamInner::Tcp(mut stream)
            | ListenerStreamInner::Unix(mut stream)
            | ListenerStreamInner::Ssl(mut stream) => {
                // Don't close the stream, create a task to drain it to avoid `ECONNRESET`.
                tokio::task::spawn(async move {
                    _ = stream.shutdown().await;
                    // Drain the stream, up to DRAIN_TIMEOUT.
                    tokio::time::timeout(DRAIN_TIMEOUT, drain_stream(stream))
                        .await
                        .unwrap_or_else(|_| {
                            warn!(
                                "Draining stream took too long (> {} ms)",
                                DRAIN_TIMEOUT.as_millis()
                            );
                        });
                });
            }
            ListenerStreamInner::Http(_) => {}
            ListenerStreamInner::WebSocket(_) => {}
        }
    }
}

impl ListenerStream {
    pub fn new_tcp(stream: RawStream, preview: Preview) -> Self {
        let stream_properties = StreamProperties {
            peer_addr: stream.remote_address().ok(),
            local_addr: stream.remote_address().ok(),
            ..StreamProperties::new(TransportType::Tcp)
        }
        .into();
        ListenerStream {
            stream_properties,
            preview: Some(preview),
            inner: ConsumeOnDrop::new(ListenerStreamInner::Tcp(stream)),
        }
    }

    #[cfg(unix)]
    pub fn new_unix(
        stream: RawStream,
        preview: Preview,
        local_addr: Option<ResolvedTarget>,
        peer_addr: Option<ResolvedTarget>,
        peer_creds: Option<tokio::net::unix::UCred>,
    ) -> Self {
        let stream_properties = StreamProperties {
            peer_addr,
            local_addr,
            peer_creds,
            ..StreamProperties::new(TransportType::Unix)
        }
        .into();
        ListenerStream {
            stream_properties,
            preview: Some(preview),
            inner: ConsumeOnDrop::new(ListenerStreamInner::Unix(stream)),
        }
    }

    pub fn new_websocket(stream_props: StreamProperties, stream: HyperUpgradedStream) -> Self {
        ListenerStream {
            stream_properties: stream_props.into(),
            preview: None,
            inner: ConsumeOnDrop::new(ListenerStreamInner::WebSocket(stream)),
        }
    }

    pub async fn start_tls(self) -> Result<Self, std::io::Error> {
        trace!("Starting TLS on {self:?}");
        match ConsumeOnDrop::into_inner(self.inner) {
            ListenerStreamInner::Tcp(stream) => {
                let parent_stream_properties = self.stream_properties.clone();

                let (preview, ssl_stream) = stream
                    .secure_upgrade_preview(PreviewConfiguration::default())
                    .await
                    .map_err(std::io::Error::from)?;

                let mut stream_properties = StreamProperties {
                    parent: Some(parent_stream_properties),
                    ..StreamProperties::new(TransportType::Ssl)
                };

                if let Some(handshake) = ssl_stream.handshake() {
                    stream_properties.server_name_indication =
                        handshake.sni.as_ref().map(|s| s.as_ref().to_string());
                    stream_properties.protocol =
                        handshake.alpn.as_ref().and_then(|s| known_protocol(s));
                    // TODO
                    // stream_properties.peer_certificate = handshake.cert.map(|c| c.into());
                    stream_properties.ssl_version = handshake.version;
                    // TODO
                    // stream_properties.ssl_cipher_name = handshake.cipher_name;
                }

                Ok(ListenerStream {
                    stream_properties: stream_properties.into(),
                    preview: Some(preview),
                    inner: ConsumeOnDrop::new(ListenerStreamInner::Ssl(ssl_stream)),
                })
            }
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "SSL connection cannot be establed on this transport",
            )),
        }
    }

    /// If the underlying stream is SSL or a WebSocket, retrieves the negotiated
    /// protocol.
    pub fn selected_protocol(&self) -> Option<&'static str> {
        self.stream_properties.protocol
    }

    /// Returns the transport type of the underlying stream.
    pub fn transport_type(&self) -> TransportType {
        self.inner.discriminant()
    }

    /// Returns the peer address of the underlying stream.
    #[inline(always)]
    pub fn props(&self) -> &StreamProperties {
        &self.stream_properties
    }

    /// Returns the peer address of the underlying stream.
    #[inline(always)]
    pub fn props_clone(&self) -> Arc<StreamProperties> {
        self.stream_properties.clone()
    }

    /// Returns the local address of the underlying stream.
    pub fn local_addr(&self) -> Option<&ResolvedTarget> {
        self.stream_properties.local_addr.as_ref()
    }

    /// Returns the peer address of the underlying stream.
    pub fn peer_addr(&self) -> Option<&ResolvedTarget> {
        self.stream_properties.peer_addr.as_ref()
    }

    pub fn upgrade(self, props: StreamPropertiesBuilder) -> Self {
        Self {
            inner: self.inner,
            preview: None,
            stream_properties: self.stream_properties.upgrade(props),
        }
    }

    pub fn preface(&self) -> Option<[u8; PREFACE_SIZE]> {
        self.preview
            .as_ref()
            .map(|p| p.as_ref().try_into().unwrap())
    }
}

impl std::fmt::Debug for ListenerStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}({:?})", self.transport_type(), self.props())
    }
}
