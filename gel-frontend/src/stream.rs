use crate::{
    hyper::{HyperStream, HyperUpgradedStream},
    stream_type::{PREFACE_SIZE, StreamType, known_protocol},
};
use gel_stream::{
    Preview, PreviewConfiguration, RawStream, RemoteAddress, ResolvedTarget, StreamUpgrade,
};
use hyper::{HeaderMap, Version};
use std::{collections::HashMap, sync::Arc};
use strum::IntoDiscriminant;
use tokio::net::unix::UCred;
use tracing::trace;

macro_rules! stream_properties {
    (
        $(#[doc=$doc:literal] pub $name:ident: $type:ty),+ $(,)?
    ) => {
        #[derive(Clone, Default, Debug)]
        pub struct StreamPropertiesBuilder {
            $(
                #[doc=$doc]
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
                pub $name: $type,
            )+
        }

        impl StreamProperties {
            pub fn new(transport: TransportType) -> Self {
                StreamProperties {
                    parent: None,
                    transport,
                    $($name: None),+
                }
            }

            pub fn upgrade(self: Arc<Self>, props: StreamPropertiesBuilder) -> Arc<Self> {
                let transport = self.transport;
                Arc::new(StreamProperties {
                    parent: Some(self),
                    transport,
                    $($name: props.$name),+
                })
            }

            $(
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
    pub peer_creds: Option<UCred>,
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
    #[read]
    #[write]
    inner: ListenerStreamInner,
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

impl ListenerStream {
    pub fn new_tcp(stream: RawStream, preview: Preview) -> Self {
        let stream_properties = StreamProperties {
            peer_addr: stream.remote_address().ok().map(|s| s.into()),
            local_addr: stream.remote_address().ok().map(|s| s.into()),
            ..StreamProperties::new(TransportType::Tcp)
        }
        .into();
        ListenerStream {
            stream_properties,
            preview: Some(preview),
            inner: ListenerStreamInner::Tcp(stream),
        }
    }

    #[cfg(unix)]
    pub fn new_unix(
        stream: RawStream,
        preview: Preview,
        local_addr: Option<ResolvedTarget>,
        peer_addr: Option<ResolvedTarget>,
        peer_creds: Option<UCred>,
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
            inner: ListenerStreamInner::Unix(stream),
        }
    }

    pub fn new_websocket(stream_props: StreamProperties, stream: HyperUpgradedStream) -> Self {
        ListenerStream {
            stream_properties: stream_props.into(),
            preview: None,
            inner: ListenerStreamInner::WebSocket(stream),
        }
    }

    pub async fn start_tls(self) -> Result<Self, std::io::Error> {
        trace!("Starting TLS on {self:?}");
        match self.inner {
            ListenerStreamInner::Tcp(stream) => {
                let parent_stream_properties = self.stream_properties.clone();

                let (preview, ssl_stream) = stream
                    .secure_upgrade_preview(PreviewConfiguration::default())
                    .await
                    .map_err(|e| std::io::Error::from(e))?;

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
                    inner: ListenerStreamInner::Ssl(ssl_stream),
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
        self.stream_properties.local_addr.as_ref()
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
