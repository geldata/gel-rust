use futures::{stream, Stream, StreamExt};
use gel_jwt::{Key, KeyRegistry};
use gel_stream::{ResolvedTarget, TlsKey, TlsServerParameterProvider, TlsServerParameters};
use std::{
    net::{SocketAddr, ToSocketAddrs},
    path::PathBuf,
    sync::{Arc, Mutex},
};

use crate::{
    stream::{StreamProperties, TransportType},
    stream_type::StreamType,
};

/// Implemented by the embedder to configure the live state of the listener.
pub trait ListenerConfig: std::fmt::Debug + Send + Sync + 'static {
    /// Returns a stream of [`ListenerAddress`]es, allowing the server to
    /// reconfigure the listening port at any time.
    fn listen_address(&self) -> impl Stream<Item = std::io::Result<Vec<ResolvedTarget>>> + Send;

    /// Return the SSL configuration, optionally with a lookup function.
    fn ssl_config(&self) -> Result<TlsServerParameterProvider, ()>;

    /// Returns the JWT key registry.
    fn jwt_key(&self) -> Result<gel_jwt::KeyRegistry<Key>, ()>;

    /// Returns true if the given [`StreamType`] is supported at this time.
    fn is_supported(
        &self,
        stream_type: Option<StreamType>,
        transport_type: TransportType,
        stream_props: &StreamProperties,
    ) -> bool;
}

pub struct SslConfig {
    inner: Arc<Mutex<SslConfigInner>>,
}

impl SslConfig {
    pub fn new(cert: PathBuf, key: PathBuf) -> Self {
        Self {
            inner: Arc::new(Mutex::new(SslConfigInner::Unconfigured { cert, key })),
        }
    }

    pub(crate) fn maybe_configure(
        &self,
        f: impl FnOnce(&mut TlsServerParameters),
    ) -> Arc<TlsServerParameters> {
        let mut inner = self.inner.lock().unwrap();
        match &mut *inner {
            SslConfigInner::Unconfigured { cert, key } => {
                let cert_file = std::fs::read(cert).unwrap();
                let key_file = std::fs::read(key).unwrap();

                let key = TlsKey::new_pem(&cert_file, &key_file).unwrap();
                let mut ctx_builder = TlsServerParameters::new_with_certificate(key);

                // Apply any additional configuration
                f(&mut ctx_builder);

                Arc::new(ctx_builder)
            }
            SslConfigInner::Configured { context } => context.clone(),
        }
    }
}

enum SslConfigInner {
    Unconfigured { cert: PathBuf, key: PathBuf },
    Configured { context: Arc<TlsServerParameters> },
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
    ) -> bool {
        eprintln!("is_supported? stream_type={stream_type:?} transport_type={transport_type:?} stream_props={stream_props:?}");
        true
    }

    fn ssl_config(&self) -> Result<TlsServerParameterProvider, ()> {
        Ok(TlsServerParameterProvider::new(
            TlsServerParameters::new_with_certificate(
                gel_stream::test_keys::SERVER_KEY.clone_key(),
            ),
        ))
    }

    fn jwt_key(&self) -> Result<KeyRegistry<Key>, ()> {
        let mut key = KeyRegistry::new();
        key.generate_key(None, gel_jwt::KeyType::ES256)
            .map_err(|_| ())?;
        Ok(key)
    }

    fn listen_address(&self) -> impl Stream<Item = std::io::Result<Vec<ResolvedTarget>>> {
        let addrs = self
            .addrs
            .iter()
            .map(|addr| ResolvedTarget::from(*addr))
            .collect();
        stream::select_all(vec![
            stream::once(async { Ok(addrs) }).boxed(),
            stream::pending().boxed(),
        ])
    }
}
