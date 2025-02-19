use std::marker::PhantomData;
use std::net::SocketAddr;

use crate::common::tokio_stream::{Resolver, TokioStream};
use crate::{ConnectionError, Ssl, StreamUpgrade, TlsDriver, UpgradableStream};
use crate::{MaybeResolvedTarget, ResolvedTarget, Target};

type Connection<S, D> = UpgradableStream<S, D>;

/// A connector can be used to connect multiple times to the same target.
#[allow(private_bounds)]
pub struct Connector<D: TlsDriver = Ssl> {
    target: Target,
    resolver: Resolver,
    driver: PhantomData<D>,
    #[cfg(feature = "keepalive")]
    keepalive: Option<std::time::Duration>,
}

impl Connector<Ssl> {
    pub fn new(target: Target) -> Result<Self, std::io::Error> {
        Self::new_explicit(target)
    }
}

#[allow(private_bounds)]
impl<D: TlsDriver> Connector<D> {
    pub fn new_explicit(target: Target) -> Result<Self, std::io::Error> {
        Ok(Self {
            target,
            resolver: Resolver::new()?,
            driver: PhantomData,
            #[cfg(feature = "keepalive")]
            keepalive: None,
        })
    }

    /// Set a keepalive for the connection. This is only supported for TCP
    /// connections and will be ignored for unix sockets.
    pub fn set_keepalive(&mut self, keepalive: Option<std::time::Duration>) {
        self.keepalive = keepalive;
    }

    pub async fn connect(&self) -> Result<Connection<TokioStream, D>, ConnectionError> {
        let stream = match self.target.maybe_resolved() {
            MaybeResolvedTarget::Resolved(target) => target.connect().await?,
            MaybeResolvedTarget::Unresolved(host, port, _) => {
                let ip = self
                    .resolver
                    .resolve_remote(host.clone().into_owned())
                    .await?;
                ResolvedTarget::SocketAddr(SocketAddr::new(ip, *port))
                    .connect()
                    .await?
            }
        };

        if let Some(keepalive) = self.keepalive {
            if self.target.is_tcp() {
                stream.set_keepalive(Some(keepalive))?;
            }
        }

        if let Some(ssl) = self.target.maybe_ssl() {
            let ssl = D::init_client(ssl, self.target.name())?;
            let mut stm = UpgradableStream::new_client(stream, Some(ssl));
            if !self.target.is_starttls() {
                stm.secure_upgrade().await?;
            }
            Ok(stm)
        } else {
            Ok(UpgradableStream::new_client(stream, None))
        }
    }
}
