use std::borrow::Cow;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::{future::Future, str::FromStr, task::Poll};

use crate::{MaybeResolvedTarget, ResolvedTarget, TargetName, TcpResolve};

/// An async resolver for hostnames to IP addresses.
#[derive(Clone)]
pub struct Resolver {
    #[cfg(feature = "hickory")]
    resolver: std::sync::Arc<hickory_resolver::TokioResolver>,
}

#[cfg(feature = "tokio")]
#[allow(unused)]
async fn resolve_host_to_socket_addrs(host: String) -> std::io::Result<ResolvedTarget> {
    let res = tokio::task::spawn_blocking(move || format!("{}:0", host).to_socket_addrs())
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Interrupted, e.to_string()))??;
    res.into_iter()
        .next()
        .ok_or(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "No address found",
        ))
        .map(|addr| ResolvedTarget::SocketAddr(addr))
}

impl Resolver {
    /// Create a new resolver.
    pub fn new() -> Result<Self, std::io::Error> {
        Ok(Self {
            #[cfg(feature = "hickory")]
            resolver: hickory_resolver::Resolver::builder_tokio()?.build().into(),
        })
    }

    pub(crate) fn resolve_remote(
        &self,
        host: &MaybeResolvedTarget,
    ) -> ResolveResult<ResolvedTarget> {
        match host {
            MaybeResolvedTarget::Resolved(resolved) => {
                ResolveResult::new_sync(Ok(resolved.clone()))
            }
            MaybeResolvedTarget::Unresolved(host, port, _) => {
                if let Ok(ip) = IpAddr::from_str(&host) {
                    ResolveResult::new_sync(Ok(ResolvedTarget::SocketAddr(SocketAddr::from((
                        ip, *port,
                    )))))
                } else {
                    #[cfg(feature = "hickory")]
                    {
                        let resolver = self.resolver.clone();
                        let host = host.to_string();
                        let port = *port;
                        ResolveResult::new_async(async move {
                            let f = resolver.lookup_ip(host);
                            let Some(addr) = f.await?.iter().next() else {
                                return Err(std::io::Error::new(
                                    std::io::ErrorKind::NotFound,
                                    "No address found",
                                ));
                            };
                            Ok(ResolvedTarget::SocketAddr(SocketAddr::new(addr, port)))
                        })
                    }
                    #[cfg(all(feature = "tokio", not(feature = "hickory")))]
                    {
                        ResolveResult::new_async(resolve_host_to_socket_addrs(host.to_string()))
                    }
                    #[cfg(not(any(feature = "tokio", feature = "hickory")))]
                    {
                        ResolveResult::new_sync(Err(std::io::Error::new(
                            std::io::ErrorKind::Unsupported,
                            "No resolver available",
                        )))
                    }
                }
            }
        }
    }
}

/// The result of a resolution. It may be synchronous or asynchronous, but you
/// can always call `.await` on it.
pub struct ResolveResult<T> {
    inner: ResolveResultInner<T>,
}

impl<T> ResolveResult<T> {
    fn new_sync(result: Result<T, std::io::Error>) -> Self {
        Self {
            inner: ResolveResultInner::Sync(result),
        }
    }

    fn new_async(future: impl Future<Output = std::io::Result<T>> + Send + 'static) -> Self {
        Self {
            inner: ResolveResultInner::Async(Box::pin(future)),
        }
    }

    pub fn sync(&mut self) -> Result<Option<T>, std::io::Error> {
        if let ResolveResultInner::Sync(_) = &mut self.inner {
            let this = std::mem::replace(&mut self.inner, ResolveResultInner::Fused);
            let ResolveResultInner::Sync(result) = this else {
                unreachable!()
            };
            result.map(Some)
        } else {
            Ok(None)
        }
    }

    pub fn map<U>(self, f: impl (FnOnce(T) -> U) + Send + 'static) -> ResolveResult<U>
    where
        T: 'static,
    {
        match self.inner {
            ResolveResultInner::Sync(Ok(t)) => ResolveResult::new_sync(Ok(f(t))),
            ResolveResultInner::Sync(Err(e)) => ResolveResult::new_sync(Err(e)),
            ResolveResultInner::Async(future) => {
                ResolveResult::new_async(async move { Ok(f(future.await?)) })
            }
            ResolveResultInner::Fused => ResolveResult::new_sync(Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Polled a previously awaited result",
            ))),
        }
    }
}

enum ResolveResultInner<T> {
    Sync(Result<T, std::io::Error>),
    Async(std::pin::Pin<Box<dyn Future<Output = std::io::Result<T>> + Send>>),
    Fused,
}

impl<T> Future for ResolveResult<T>
where
    Self: Unpin,
{
    type Output = std::io::Result<T>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.get_mut();
        match &mut this.inner {
            ResolveResultInner::Sync(_) => {
                let this = std::mem::replace(&mut this.inner, ResolveResultInner::Fused);
                let ResolveResultInner::Sync(result) = this else {
                    unreachable!()
                };
                Poll::Ready(result)
            }
            ResolveResultInner::Async(future) => future.as_mut().poll(cx),
            ResolveResultInner::Fused => {
                panic!("Polled a previously awaited result")
            }
        }
    }
}

/// A trait for types that can be resolved to a target.
pub trait Resolvable {
    type Target;

    fn resolve(&self, resolver: &Resolver) -> ResolveResult<Self::Target>;
}

impl Resolvable for String {
    type Target = IpAddr;

    fn resolve(&self, resolver: &Resolver) -> ResolveResult<Self::Target> {
        resolver
            .resolve_remote(&MaybeResolvedTarget::Unresolved(
                Cow::Owned(self.clone()),
                0,
                None,
            ))
            .map(|target| match target {
                ResolvedTarget::SocketAddr(addr) => addr.ip(),
                _ => unreachable!(),
            })
    }
}

impl<T: TcpResolve + Clone> Resolvable for T {
    type Target = ResolvedTarget;

    fn resolve(&self, resolver: &Resolver) -> ResolveResult<Self::Target> {
        resolver.resolve_remote(&self.clone().into())
    }
}

impl Resolvable for TargetName {
    type Target = ResolvedTarget;

    fn resolve(&self, resolver: &Resolver) -> ResolveResult<Self::Target> {
        resolver.resolve_remote(self.maybe_resolved())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::*;

    #[tokio::test]
    async fn test_resolve_remote() {
        let resolver = Resolver::new().unwrap();
        let target = TargetName::new_tcp(("localhost", 8080));
        let result = target.resolve(&resolver).await.unwrap();
        assert_eq!(
            result,
            ResolvedTarget::SocketAddr(SocketAddr::new(
                IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                8080
            ))
        );
    }

    #[cfg(feature = "__manual_tests")]
    #[tokio::test]
    async fn test_resolve_real_domain() {
        let resolver = Resolver::new().unwrap();
        let target = TargetName::new_tcp(("www.google.com", 443));
        let result = target.resolve(&resolver).await.unwrap();
        println!("{:?}", result);
    }
}
