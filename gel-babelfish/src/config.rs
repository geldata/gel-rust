use futures::{Stream, StreamExt, stream};
use gel_jwt::{Key, KeyRegistry};
use gel_stream::{ResolvedTarget, TlsServerParameterProvider, TlsServerParameters};
use std::{
    borrow::Cow,
    net::{SocketAddr, ToSocketAddrs},
    sync::Arc,
};
use tracing::trace;

use crate::{
    stream::{StreamProperties, TransportType},
    stream_type::StreamType,
};

/// A function to lookup the tenant configuration for a given tenant name.
pub type TenantLookup = Box<dyn Fn(&str) -> Option<Arc<TenantConfig>> + Send + Sync + 'static>;

/// The configuration for a tenant.
pub struct TenantConfig {
    /// The TLS configuration for the tenant, if configured.
    pub tls: Option<TlsServerParameterProvider>,
    /// The JWT key registry for the tenant, if configured.
    pub jwt: Option<KeyRegistry<Key>>,
    /// The tenant name, if configured.
    pub name: Option<String>,
}

/// A listener entry.
pub struct ListenerEntry {
    pub addresses: Vec<ResolvedTarget>,
    /// The default tenant configuration.
    pub default_tenant: TenantConfig,
    /// A function to lookup the tenant configuration for a given tenant name.
    pub tenant_lookup: TenantLookup,
}

impl ListenerEntry {
    pub fn tls_lookup(&self) -> Option<TlsServerParameterProvider> {
        self.default_tenant.tls.clone()
    }
}

/// Whether a stream type is supported.
pub enum StreamSupported {
    /// The stream type is supported, regardless of further specialization.
    Yes,
    /// The stream type is not supported, no matter what further specialization
    /// is provided.
    No(Cow<'static, str>),
    /// The stream type may be supported, depending on further specialization.
    Maybe,
}

impl StreamSupported {
    pub fn is_yes(&self) -> bool {
        matches!(self, StreamSupported::Yes)
    }

    pub fn is_no(&self) -> bool {
        matches!(self, StreamSupported::No(_))
    }

    pub fn is_maybe(&self) -> bool {
        matches!(self, StreamSupported::Maybe)
    }

    pub fn is_yes_or_maybe(&self) -> bool {
        matches!(self, StreamSupported::Yes | StreamSupported::Maybe)
    }

    pub fn is_no_or_maybe(&self) -> bool {
        matches!(self, StreamSupported::No(_) | StreamSupported::Maybe)
    }
}

/// Implemented by the embedder to configure the live state of the listener.
pub trait ListenerConfig: std::fmt::Debug + Send + Sync + 'static {
    /// Returns a stream of [`ListenerAddress`]es, allowing the server to
    /// reconfigure the listening port at any time.
    fn listen_address(&self) -> impl Stream<Item = std::io::Result<ListenerEntry>> + Send;

    /// Returns [`StreamSupported::Yes`] if the given [`StreamType`] is supported at this time.
    fn is_supported(
        &self,
        stream_type: Option<StreamType>,
        transport_type: TransportType,
        stream_props: &StreamProperties,
    ) -> StreamSupported;

    /// Returns `true` if the given [`StreamType`] is supported at this time. No
    /// further specifialization will be available.
    fn is_supported_final(
        &self,
        stream_type: StreamType,
        transport_type: TransportType,
        stream_props: &StreamProperties,
    ) -> bool;
}

#[derive(Debug)]
pub struct TestListenerConfig {
    addrs: Vec<SocketAddr>,
}

impl TestListenerConfig {
    pub fn new(s: impl ToSocketAddrs) -> Self {
        let addrs = s.to_socket_addrs().unwrap().collect();
        Self { addrs }
    }
}

impl ListenerConfig for TestListenerConfig {
    fn is_supported(
        &self,
        stream_type: Option<StreamType>,
        transport_type: TransportType,
        stream_props: &StreamProperties,
    ) -> StreamSupported {
        trace!(
            "is_supported? stream_type={stream_type:?} transport_type={transport_type:?} stream_props={stream_props:?}"
        );
        StreamSupported::Yes
    }

    fn is_supported_final(
        &self,
        stream_type: StreamType,
        transport_type: TransportType,
        stream_props: &StreamProperties,
    ) -> bool {
        trace!(
            "is_supported_final? stream_type={stream_type:?} transport_type={transport_type:?} stream_props={stream_props:?}"
        );
        true
    }

    fn listen_address(&self) -> impl Stream<Item = std::io::Result<ListenerEntry>> {
        let addrs = self
            .addrs
            .iter()
            .map(|addr| ResolvedTarget::from(*addr))
            .collect();
        stream::select_all(vec![
            stream::once(async {
                Ok(ListenerEntry {
                    addresses: addrs,
                    default_tenant: TenantConfig {
                        tls: Some(default_tls_config()),
                        jwt: Some(default_jwt_key()),
                        name: None,
                    },
                    tenant_lookup: Box::new(|_| None),
                })
            })
            .boxed(),
            stream::pending().boxed(),
        ])
    }
}

fn default_tls_config() -> TlsServerParameterProvider {
    TlsServerParameterProvider::new(TlsServerParameters::new_with_certificate(
        gel_stream::test_keys::SERVER_KEY.clone_key(),
    ))
}

fn default_jwt_key() -> KeyRegistry<Key> {
    let mut key = KeyRegistry::new();
    key.generate_key(None, gel_jwt::KeyType::ES256).unwrap();
    key
}
