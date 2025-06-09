use std::marker::PhantomData;

use crate::common::resolver::Resolver;
use crate::common::tokio_stream::TokioStream;
use crate::Target;
use crate::{ConnectionError, ResolvedTarget, Ssl, StreamUpgrade, TlsDriver, UpgradableStream};

type Connection<S, D> = UpgradableStream<S, D>;

#[derive(derive_more::Debug, Clone)]
enum ConnectorInner {
    #[debug("{:?}", _0)]
    Unresolved(Target, Resolver),
    #[debug("{:?}", _0)]
    Resolved(ResolvedTarget),
}

/// A connector can be used to connect multiple times to the same target.
#[derive(derive_more::Debug, Clone)]
#[allow(private_bounds)]
pub struct Connector<D: TlsDriver = Ssl> {
    target: ConnectorInner,
    #[debug(skip)]
    driver: PhantomData<D>,
    ignore_missing_close_notify: bool,
    #[cfg(feature = "keepalive")]
    keepalive: Option<std::time::Duration>,
}

impl Connector<Ssl> {
    /// Create a new connector with the given target and default resolver.
    pub fn new(target: Target) -> Result<Self, std::io::Error> {
        Self::new_explicit(target)
    }

    /// Create a new connector with the given resolved target.
    pub fn new_resolved(target: ResolvedTarget) -> Self {
        Self::new_explicit_resolved(target)
    }

    /// Create a new connector with the given target and resolver.
    pub fn new_with_resolver(target: Target, resolver: Resolver) -> Self {
        Self::new_explicit_with_resolver(target, resolver)
    }
}

#[allow(private_bounds)]
impl<D: TlsDriver> Connector<D> {
    /// Create a new connector with the given TLS driver and default resolver.
    pub fn new_explicit(target: Target) -> Result<Self, std::io::Error> {
        Ok(Self {
            target: ConnectorInner::Unresolved(target, Resolver::new()?),
            driver: PhantomData,
            ignore_missing_close_notify: false,
            #[cfg(feature = "keepalive")]
            keepalive: None,
        })
    }

    /// Create a new connector with the given TLS driver and resolved target.
    pub fn new_explicit_resolved(target: ResolvedTarget) -> Self {
        Self {
            target: ConnectorInner::Resolved(target),
            driver: PhantomData,
            ignore_missing_close_notify: false,
            #[cfg(feature = "keepalive")]
            keepalive: None,
        }
    }

    /// Create a new connector with the given TLS driver and resolver.
    pub fn new_explicit_with_resolver(target: Target, resolver: Resolver) -> Self {
        Self {
            target: ConnectorInner::Unresolved(target, resolver),
            driver: PhantomData,
            ignore_missing_close_notify: false,
            #[cfg(feature = "keepalive")]
            keepalive: None,
        }
    }

    /// Set a keepalive for the connection. This is only supported for TCP
    /// connections and will be ignored for unix sockets.
    #[cfg(feature = "keepalive")]
    pub fn set_keepalive(&mut self, keepalive: Option<std::time::Duration>) {
        self.keepalive = keepalive;
    }

    /// For TLS connections, ignore a hard close where the socket was closed
    /// before receiving CLOSE_NOTIFY.
    ///
    /// This may result in vulnerability to truncation attacks for protocols
    /// that do not include an implicit length, but may also result in spurious
    /// failures on Windows where sockets may be closed before the CLOSE_NOTIFY
    /// is received.
    pub fn ignore_missing_tls_close_notify(&mut self) {
        self.ignore_missing_close_notify = true;
    }

    /// Connect to the target.
    pub async fn connect(&self) -> Result<Connection<TokioStream, D>, ConnectionError> {
        let target = match &self.target {
            ConnectorInner::Unresolved(target, resolver) => {
                resolver.resolve_remote(target.maybe_resolved()).await?
            }
            ConnectorInner::Resolved(target) => target.clone(),
        };
        let stream = target.connect().await?;

        #[cfg(feature = "keepalive")]
        if let Some(keepalive) = self.keepalive {
            if target.is_tcp() {
                stream.set_keepalive(Some(keepalive))?;
            }
        }

        if let ConnectorInner::Unresolved(target, _) = &self.target {
            if let Some(ssl) = target.maybe_ssl() {
                let ssl = D::init_client(ssl, target.name())?;
                let mut stm = UpgradableStream::new_client(stream, Some(ssl));
                if self.ignore_missing_close_notify {
                    stm.ignore_missing_close_notify();
                }
                if !target.is_starttls() {
                    stm = stm.secure_upgrade().await?;
                }
                Ok(stm)
            } else {
                Ok(UpgradableStream::new_client(stream, None))
            }
        } else {
            Ok(UpgradableStream::new_client(stream, None))
        }
    }
}
