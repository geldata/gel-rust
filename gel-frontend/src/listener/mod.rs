use crate::service::{BabelfishService, ConnectionIdentityBuilder};
use crate::stream::{ListenerStream, StreamProperties, TransportType};
use crate::stream_type::{
    ALPN_EDGEDB_BINARY, ALPN_GEL_BINARY, ALPN_HTTP1_1, ALPN_HTTP2, ALPN_POSTGRESQL,
    PostgresInitialMessage, StreamState, StreamType, identify_stream,
};
use futures::StreamExt;
use gel_stream::{
    Acceptor, LocalAddress, PeerCred, PreviewConfiguration, RemoteAddress, ResolvedTarget, TlsAlpn,
    TlsServerParameterProvider, Transport,
};
use scopeguard::defer;
use std::{
    collections::{HashMap, HashSet},
    future::Future,
    iter::FromIterator,
    pin::pin,
    sync::{Arc, Mutex},
};
use tokio::{io::AsyncWriteExt, task::JoinHandle};
use tracing::{error, info, trace, warn};

use crate::config::ListenerConfig;

mod gel;
mod http;
mod postgres;

/// Handles a connection from the listener. This method will not return until the connection is closed.
pub async fn handle_connection_inner(
    state: StreamState,
    mut socket: ListenerStream,
    identity: ConnectionIdentityBuilder,
    bound_config: impl IsBoundConfig,
) -> Result<(), std::io::Error> {
    trace!("handle_connection_inner state={state:?} {socket:?}");
    let res = identify_stream(state, &mut socket).await;
    let stream_type = match res {
        Ok(stream_type) => stream_type,
        Err(unknown_type) => {
            warn!("Unknown stream type: {unknown_type:?}");
            handle_stream_shutdown(unknown_type.go_away_message(), socket).await?;
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid protocol ({unknown_type:?})"),
            ));
        }
    };

    let transport = socket.transport_type();
    if !bound_config.config().is_supported_final(
        stream_type,
        socket.transport_type(),
        socket.props(),
    ) {
        warn!("{stream_type:?} on {transport:?} disabled");
        handle_stream_shutdown(stream_type.go_away_message(), socket).await?;
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Disabled stream type: {stream_type:?}"),
        ));
    }

    match stream_type {
        StreamType::GelBinary => {
            gel::handle_stream_gel_binary(socket, identity, bound_config).await
        }
        StreamType::HTTP1x => http::handle_stream_http1x(socket, identity, bound_config).await,
        StreamType::HTTP2 => http::handle_stream_http2(socket, identity, bound_config).await,
        StreamType::SSLTLS => handle_stream_ssltls(socket, identity, bound_config).await,
        StreamType::PostgresInitial(PostgresInitialMessage::SSLRequest) => {
            postgres::handle_stream_postgres_ssl(socket, identity, bound_config).await
        }
        StreamType::PostgresInitial(..) => {
            postgres::handle_stream_postgres_initial(socket, identity, bound_config).await
        }
    }
}

async fn handle_stream_shutdown(
    message: &[u8],
    mut socket: ListenerStream,
) -> Result<(), std::io::Error> {
    // Send the go away message to the peer and then an SSL shutdown, if appropriate.
    _ = socket.write_all(message).await;

    Ok(())
}

#[derive(Debug)]
pub struct BoundConfig<C: ListenerConfig, S: BabelfishService> {
    config: Arc<C>,
    service: Arc<S>,
}

impl<C: ListenerConfig, S: BabelfishService> BoundConfig<C, S> {
    pub fn new(config: C, service: S) -> std::io::Result<Self> {
        let config = Arc::new(config);
        Ok(Self {
            config,
            service: service.into(),
        })
    }
}

impl<C: ListenerConfig, S: BabelfishService> Clone for BoundConfig<C, S> {
    fn clone(&self) -> Self {
        Self {
            config: Arc::clone(&self.config),
            service: Arc::clone(&self.service),
        }
    }
}

pub trait IsBoundConfig: Clone + Send + Sync + 'static {
    type Config: ListenerConfig;
    type Service: BabelfishService;

    fn config(&self) -> &Arc<Self::Config>;
    fn service(&self) -> &Arc<Self::Service>;
}

impl<C: ListenerConfig, S: BabelfishService> IsBoundConfig for BoundConfig<C, S> {
    type Config = C;
    type Service = S;

    #[inline(always)]
    fn config(&self) -> &Arc<Self::Config> {
        &self.config
    }

    #[inline(always)]
    fn service(&self) -> &Arc<Self::Service> {
        &self.service
    }
}

pub struct BoundServer {
    task: tokio::task::JoinHandle<std::io::Result<()>>,
    addresses: tokio::sync::Mutex<tokio::sync::watch::Receiver<Option<Vec<ResolvedTarget>>>>,
}

impl BoundServer {
    pub fn bind(
        config: impl ListenerConfig,
        service: impl BabelfishService,
    ) -> std::io::Result<Self> {
        let config = BoundConfig::new(config, service)?;

        trace!("Booting bound server with {config:#?}");

        let (tx, rx) = tokio::sync::watch::channel(None);
        let task = tokio::task::spawn(bind_task(tx, config.config().clone(), move |stm| {
            let config = config.clone();
            tokio::task::spawn(async move {
                let identity = ConnectionIdentityBuilder::new();
                if config
                    .config
                    .is_supported(None, stm.transport_type(), stm.props())
                    .is_no()
                {
                    return;
                }
                if let Err(e) =
                    handle_connection_inner(StreamState::Raw, stm, identity, config).await
                {
                    error!("Connection error: {e:?}");
                }
            });
        }));
        Ok(Self {
            task,
            addresses: rx.into(),
        })
    }

    pub async fn addresses(&self) -> Vec<ResolvedTarget> {
        let mut lock = self.addresses.lock().await;
        let Ok(res) = lock.wait_for(|t| t.is_some()).await else {
            return vec![];
        };
        res.clone().unwrap_or_default()
    }

    pub fn shutdown(self) -> impl Future<Output = ()> {
        self.task.abort();
        async {
            _ = self.task.await;
        }
    }
}

fn compute_alpn(config: Arc<impl ListenerConfig>, stream_props: &StreamProperties) -> TlsAlpn {
    let mut alpn = Vec::default();
    if config
        .is_supported(
            Some(StreamType::GelBinary),
            TransportType::Tcp,
            stream_props,
        )
        .is_yes_or_maybe()
    {
        alpn.push(ALPN_EDGEDB_BINARY);
        alpn.push(ALPN_GEL_BINARY);
    }
    if config
        .is_supported(
            Some(StreamType::PostgresInitial(
                PostgresInitialMessage::StartupMessage,
            )),
            TransportType::Tcp,
            stream_props,
        )
        .is_yes_or_maybe()
    {
        alpn.push(ALPN_POSTGRESQL);
    }
    if config
        .is_supported(Some(StreamType::HTTP2), TransportType::Tcp, stream_props)
        .is_yes_or_maybe()
    {
        alpn.push(ALPN_HTTP2);
    }
    if config
        .is_supported(Some(StreamType::HTTP1x), TransportType::Tcp, stream_props)
        .is_yes_or_maybe()
    {
        alpn.push(ALPN_HTTP1_1);
    }
    alpn.into()
}

/// Bind on the stream of addresses provided by this listener.
fn bind_task<C: ListenerConfig>(
    tx: tokio::sync::watch::Sender<Option<Vec<ResolvedTarget>>>,
    config: Arc<C>,
    callback: impl FnMut(ListenerStream) + Send + Sync + 'static,
) -> impl Future<Output = std::io::Result<()>> {
    let callback = Arc::new(Mutex::new(callback));
    async move {
        let mut stm = pin!(config.listen_address());
        let listeners = Mutex::new(HashMap::<
            _,
            (ResolvedTarget, tokio::task::JoinHandle<std::io::Result<()>>),
        >::new());
        defer!({
            _ = tx.send(Some(vec![]));
            for (_, (_, listener)) in listeners.lock().unwrap().drain() {
                listener.abort()
            }
        });
        while let Some(entry) = stm.next().await.transpose()? {
            info!(
                "Listen addresses: {addresses:?}",
                addresses = entry.addresses
            );
            let tls_lookup = entry.tls_lookup();
            info!("TLS lookup: {tls_lookup:?}");
            let mut new_listeners = HashSet::<_>::from_iter(entry.addresses);
            listeners.lock().unwrap().retain(|k, (_, v)| {
                // Remove any crashed tasks
                if v.is_finished() {
                    return false;
                }
                let res = new_listeners.contains(k);
                if !res {
                    v.abort();
                }
                res
            });

            for addr in new_listeners.drain() {
                if listeners.lock().unwrap().contains_key(&addr) {
                    continue;
                }

                let (listen_addr, task) =
                    match bind(addr.clone(), tls_lookup.clone(), callback.clone()).await {
                        Ok(task) => task,
                        Err(e) => {
                            error!("Failed to bind {addr:?}: {e:?}");
                            continue;
                        }
                    };

                listeners
                    .lock()
                    .unwrap()
                    .insert(addr.clone(), (listen_addr.clone(), task));
                let addresses: Vec<ResolvedTarget> = listeners
                    .lock()
                    .unwrap()
                    .values()
                    .map(|(addr, _)| addr.clone())
                    .collect();
                _ = tx.send(Some(addresses));
            }
        }
        Ok(())
    }
}

async fn bind(
    addr: ResolvedTarget,
    tls_lookup: Option<TlsServerParameterProvider>,
    callback: Arc<Mutex<impl FnMut(ListenerStream) + Send + Sync + 'static>>,
) -> Result<(ResolvedTarget, JoinHandle<std::io::Result<()>>), std::io::Error> {
    let acceptor = if let Some(tls_lookup) = tls_lookup {
        Acceptor::new_tls_previewing(addr.clone(), PreviewConfiguration::default(), tls_lookup)
    } else {
        Acceptor::new_previewing(addr.clone(), PreviewConfiguration::default())
    };

    let mut acceptor = acceptor.bind().await?;
    let local_addr = acceptor.local_address()?;
    info!("Listening on {local_addr:?}");

    let task = match addr.transport() {
        Transport::Tcp => tokio::task::spawn(async move {
            defer!({
                warn!("Closing TCP listener");
            });
            while let Some(res) = acceptor.next().await {
                let Ok((preview, stream)) = res else {
                    continue;
                };
                (callback.lock().unwrap())(ListenerStream::new_tcp(stream, preview));
            }
            #[allow(unreachable_code)]
            Ok::<_, std::io::Error>(())
        }),
        Transport::Unix => {
            let local_addr = local_addr.clone();

            tokio::task::spawn(async move {
                defer!({
                    warn!("Closing Unix listener");
                });
                #[cfg(unix)]
                while let Some(res) = acceptor.next().await {
                    let Ok((preview, stream)) = res else {
                        continue;
                    };
                    let peer_addr = stream.remote_address().ok();
                    let peer_cred = stream.peer_cred().ok();
                    (callback.lock().unwrap())(ListenerStream::new_unix(
                        stream,
                        preview,
                        Some(local_addr.clone()),
                        peer_addr,
                        peer_cred,
                    ));
                }
                #[allow(unreachable_code)]
                Ok::<_, std::io::Error>(())
            })
        }
    };

    Ok((local_addr, task))
}

pub async fn handle_stream_ssltls(
    socket: ListenerStream,
    identity: ConnectionIdentityBuilder,
    bound_config: impl IsBoundConfig,
) -> Result<(), std::io::Error> {
    let ssl_socket = socket.start_tls().await?;
    Box::pin(handle_connection_inner(
        StreamState::Ssl,
        ssl_socket,
        identity,
        bound_config,
    ))
    .await
}

#[cfg(test)]
mod tests {
    use gel_auth::CredentialData;
    use gel_pg_protocol::prelude::StructBuffer;
    use gel_stream::{Connector, RawStream, Target, TlsParameters};
    use hyper::Uri;
    use hyper_util::rt::TokioIo;
    use ntest_timeout::timeout;
    use rstest::rstest;
    use std::time::Duration;
    use tokio::io::AsyncReadExt;

    use crate::{
        config::TestListenerConfig,
        service::{AuthTarget, ConnectionIdentity, StreamLanguage},
    };

    use super::*;
    use std::sync::{Arc, Mutex};

    /// Captured from PostgreSQL 7.x
    const LEGACY_POSTGRES: &[u8] = &[
        0x00, 0x00, 0x01, 0x28, 0x00, 0x02, 0x00, 0x00, 0x6d, 0x61, 0x74, 0x74, 0x00, 0x00, 0x00,
        0x00,
    ];
    /// Captured from OpenSSL 1.0.2k (using -ssl2)
    const LEGACY_SSL2: &[u8] = &[
        0x80, 0x25, 0x01, 0x00, 0x02, 0x00, 0x0c, 0x00, 0x00, 0x00, 0x10, 0x05, 0x00, 0x80, 0x03,
        0x00,
    ];
    /// Captured from a modern PostgreSQL client (version 16+)
    const MODERN_POSTGRES: &[u8] = &[
        0x00, 0x00, 0x00, 0x4c, 0x00, 0x03, 0x00, 0x00, 0x75, 0x73, 0x65, 0x72, 0x00, 0x6d, 0x61,
        0x74,
    ];
    /// Captured from a modern EdgeDB client
    const MODERN_EDGEDB: &[u8] = &[
        0x56, 0x00, 0x00, 0x00, 0x4d, 0x00, 0x01, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x08,
        0x64,
    ];

    #[derive(Clone, Debug, Default)]
    struct TestService {
        log: Arc<Mutex<Vec<String>>>,
    }

    impl TestService {
        fn log(&self, msg: String) {
            eprintln!("{msg:?}");
            self.log.lock().unwrap().push(msg);
        }
    }

    enum TestMode {
        Tcp,
        Ssl,
        SslAlpn(&'static str),
    }

    impl BabelfishService for TestService {
        fn lookup_auth(
            &self,
            identity: ConnectionIdentity,
            target: AuthTarget,
        ) -> impl Future<Output = Result<CredentialData, std::io::Error>> {
            self.log(format!("lookup_auth: {:?}", identity));
            async { Ok(CredentialData::Trust) }
        }

        fn accept_stream(
            &self,
            identity: ConnectionIdentity,
            language: StreamLanguage,
            stream: ListenerStream,
        ) -> impl Future<Output = Result<(), std::io::Error>> {
            self.log(format!(
                "accept_stream: {:?}, {:?}, {:?}",
                identity, language, stream
            ));
            async { Ok(()) }
        }

        fn accept_http(
            &self,
            identity: ConnectionIdentity,
            req: hyper::http::Request<hyper::body::Incoming>,
        ) -> impl Future<Output = Result<hyper::http::Response<String>, std::io::Error>> {
            self.log(format!("accept_http: {:?}, {:?}", identity, req));
            async { Ok(Default::default()) }
        }

        fn accept_http_unauthenticated(
            &self,
            req: hyper::http::Request<hyper::body::Incoming>,
        ) -> impl Future<Output = Result<hyper::http::Response<String>, std::io::Error>> {
            self.log(format!("accept_http_unauthenticated: {:?}", req));
            async { Ok(Default::default()) }
        }
    }

    /// Run a test server and connect to it.
    fn run_test_service<F: Future<Output = Result<(), std::io::Error>> + Send + 'static>(
        mode: TestMode,
        f: impl Fn(RawStream) -> F + Send + Sync + 'static,
    ) {
        let svc = TestService::default();
        let config = TestListenerConfig::new("localhost:0");

        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async move {
                let server = BoundServer::bind(config, svc).unwrap();
                let addr = server.addresses().await.first().cloned().unwrap();

                let t2 = tokio::spawn(async move {
                    let target = match mode {
                        TestMode::Tcp => Target::new_resolved(addr),
                        TestMode::Ssl => {
                            let mut params = TlsParameters::insecure();
                            params.sni_override = Some("localhost".into());
                            Target::new_resolved_tls(addr, params)
                        }
                        TestMode::SslAlpn(alpn) => {
                            let mut params = TlsParameters::insecure();
                            params.sni_override = Some("localhost".into());
                            params.alpn = TlsAlpn::new_str(&[alpn]);
                            Target::new_resolved_tls(addr, params)
                        }
                    };
                    let stm = Connector::new(target)
                        .expect("failed to create connector")
                        .connect()
                        .await
                        .expect("failed to connect");
                    f(stm).await.expect("test failed!")
                });

                info!("Waiting for task to finish");
                t2.await.expect("task failed");
                info!("Shutting down server");
                server.shutdown().await;
                info!("Server shut down");
            });
    }

    /// Closes the connection with an error starting with "E" and ending in NUL.
    #[rstest]
    #[test_log::test]
    #[timeout(Duration::from_secs(30))]

    fn test_legacy_postgres(#[values(TestMode::Tcp, TestMode::Ssl)] mode: TestMode) {
        run_test_service(mode, |mut stm| async move {
            stm.write_all(LEGACY_POSTGRES).await.unwrap();
            stm.flush().await.unwrap();
            let mut buf = vec![];
            info!("Reading from stream");
            stm.read_to_end(&mut buf).await.unwrap();
            info!("Read from stream: {:?}", buf);
            assert_eq!(buf[0], b'E');
            assert_eq!(buf[buf.len() - 1], 0);
            Ok(())
        });
    }

    /// Closes the connection with an SSLv2 error.
    #[test]
    #[test_log::test]
    #[timeout(10_000)]

    fn test_legacy_ssl() {
        run_test_service(TestMode::Tcp, |mut stm| async move {
            stm.write_all(LEGACY_SSL2).await.unwrap();
            stm.flush().await.unwrap();
            let mut buf = vec![];
            stm.read_to_end(&mut buf).await.unwrap();
            assert_eq!(buf, vec![0x80, 3, 0, 0, 1]);
            Ok(())
        });
    }

    #[test]
    #[test_log::test]
    #[timeout(10_000)]

    fn test_raw_postgres() {
        use gel_pg_protocol::protocol::{StartupMessageBuilder, StartupNameValueBuilder};
        run_test_service(TestMode::Tcp, |mut stm| async move {
            let msg = StartupMessageBuilder {
                params: &[
                    StartupNameValueBuilder {
                        name: "database",
                        value: "name",
                    },
                    StartupNameValueBuilder {
                        name: "user",
                        value: "me",
                    },
                ],
            }
            .to_vec();
            stm.write_all(&msg).await.unwrap();
            assert_eq!(stm.read_u8().await.unwrap(), b'R'); // AuthenticationOk
            Ok(())
        });
    }

    #[rstest]
    #[test_log::test]
    #[timeout(Duration::from_secs(30))]

    fn test_http_manual(
        #[values(TestMode::Tcp, TestMode::Ssl, TestMode::SslAlpn("http/1.1"))] mode: TestMode,
    ) {
        run_test_service(mode, |mut stm| async move {
            stm.write_all(b"GET /\r\n\r\n").await.unwrap();
            stm.flush().await.unwrap();
            let mut buf = vec![];
            stm.read_to_end(&mut buf).await.unwrap();
            let result = String::from_utf8(buf).unwrap();
            assert_eq!(&result[..12], "HTTP/1.1 400");
            Ok(())
        });
    }

    #[rstest]
    #[test_log::test]
    #[timeout(Duration::from_secs(30))]

    fn test_http_1(
        #[values(TestMode::Tcp, TestMode::Ssl, TestMode::SslAlpn("http/1.1"))] mode: TestMode,
    ) {
        run_test_service(mode, |stm| async move {
            let http1 = hyper::client::conn::http1::Builder::new();
            let (mut send, conn) = http1
                .handshake::<_, String>(TokioIo::new(stm))
                .await
                .unwrap();
            tokio::task::spawn(conn);
            let req = hyper::Request::new("x".to_string());
            let resp = send.send_request(req).await.unwrap();
            eprintln!("{resp:?}");
            Ok(())
        });
    }

    #[rstest]
    #[test_log::test]
    #[timeout(Duration::from_secs(30))]

    fn test_http_2(
        #[values(TestMode::Tcp, TestMode::Ssl, TestMode::SslAlpn("h2"))] mode: TestMode,
    ) {
        run_test_service(mode, |stm| {
            async move {
                let http2 =
                    hyper::client::conn::http2::Builder::new(hyper_util::rt::TokioExecutor::new());
                let (mut send, conn) = http2
                    .handshake::<_, String>(TokioIo::new(stm))
                    .await
                    .unwrap();
                tokio::task::spawn(conn);
                let req = hyper::Request::new("x".to_string());
                let resp = send.send_request(req).await.unwrap();
                eprintln!("{resp:?}");

                // assert_eq!(stm.read_u8().await.unwrap(), b'S');
                Ok(())
            }
        });
    }

    #[rstest]
    #[test_log::test]
    #[timeout(Duration::from_secs(30))]
    fn test_tunneled_edgedb(
        #[values(TestMode::Tcp, TestMode::Ssl, TestMode::SslAlpn("h2"))] mode: TestMode,
    ) {
        run_test_service(mode, |stm| {
            async move {
                let http2 =
                    hyper::client::conn::http2::Builder::new(hyper_util::rt::TokioExecutor::new());
                let (mut send, conn) = http2.handshake::<_, _>(TokioIo::new(stm)).await.unwrap();
                tokio::task::spawn(conn);
                let mut body = vec![];

                // body.extend_from_slice(&gel_protocol::new_protocol::ExecuteBuilder {
                //     annotations: &[],
                //     allowed_capabilities: 18446744073709551577,
                //     compilation_flags: 0,
                //     implicit_limit: 0,
                //     output_format: 98,
                //     expected_cardinality: 111,
                //     command_text: "select {\n        instanceName := sys::get_instance_name(),\n        databases := sys::Database.name,\n        roles := sys::Role.name,\n      }",
                //     state_typedesc_id: Uuid::from_u128(0xffffffffffff_ffff_ffff_ffffffff),
                //     state_data: &[],
                //     input_typedesc_id: Uuid::from_u128(0x00000000_0000_0000_0000_000000000000),
                //     output_typedesc_id: Uuid::from_u128(0x00000000_0000_0000_0000_000000000000),
                //     arguments: &[]
                // }.to_vec());

                // body.extend_from_slice(b"O\x00\x00\x00\xef\x00\x00\xff\xff\xff\xff\xff\xff\xff\xd9\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00bo\x00\x00\x00\x93");
                // body.extend_from_slice(b"\n      select {\n        instanceName := sys::get_instance_name(),\n        databases := sys::Database.name,\n        s,\n      }");
                // body.extend_from_slice(b"\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00");
                // body.extend_from_slice(b"\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00S\x00\x00\x00\x04");

                let mut buf = StructBuffer::<gel_protocol::new_protocol::Message>::default();
                buf.push(&body, |msg| {
                    let msg = msg.unwrap();
                    match msg.mtype() {
                        b'S' => {
                            let status =
                                gel_protocol::new_protocol::Sync::new(msg.as_ref()).unwrap();
                            eprintln!("{status:?}");
                        }
                        b'O' => {
                            let execute =
                                gel_protocol::new_protocol::Execute::new(msg.as_ref()).unwrap();
                            eprintln!("{execute:?}");
                        }
                        _ => {
                            eprintln!("{msg:?} {}", msg.mtype() as char);
                        }
                    }
                });

                let mut req =
                    hyper::Request::new(http_body_util::Full::new(hyper::body::Bytes::from(body)));
                *req.uri_mut() = Uri::from_static("/db/./mydb");
                let resp = send.send_request(req).await.unwrap();
                eprintln!("{resp:?}");

                // assert_eq!(stm.read_u8().await.unwrap(), b'S');
                Ok(())
            }
        });
    }
}
