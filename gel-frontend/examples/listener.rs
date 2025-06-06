use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{future::Future, time::Duration};

use gel_auth::AuthType;
use gel_auth::CredentialData;
use gel_frontend::config::TestListenerConfig;
use gel_frontend::listener::BoundServer;
use gel_frontend::service::{AuthTarget, BabelfishService, ConnectionIdentity, StreamLanguage};
use gel_frontend::stream::ListenerStream;
use gel_pg_protocol::prelude::{StructBuffer, match_message};
use gel_protocol::model::Uuid;
use hyper::Response;
use tokio::io::ReadBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::trace;

#[derive(Clone, Debug, Default)]
struct ExampleService {
    stream_count: Arc<AtomicUsize>,
    http_count: Arc<AtomicUsize>,
}

impl BabelfishService for ExampleService {
    fn lookup_auth(
        &self,
        identity: ConnectionIdentity,
        target: AuthTarget,
    ) -> impl Future<Output = Result<CredentialData, std::io::Error>> {
        eprintln!("lookup_auth: identity={identity:?} target={target:?}");
        async move {
            Ok(CredentialData::new(
                AuthType::Trust,
                "matt".to_owned(),
                "password".to_owned(),
            ))
        }
    }

    fn accept_stream(
        &self,
        identity: ConnectionIdentity,
        language: StreamLanguage,
        mut stream: ListenerStream,
    ) -> impl Future<Output = Result<(), std::io::Error>> {
        trace!(
            "accept_stream: {:?}, {:?}, {:?}",
            identity, language, stream
        );
        self.stream_count.fetch_add(1, Ordering::Relaxed);
        async move {
            match language {
                StreamLanguage::EdgeDB => {
                    use gel_protocol::new_protocol::{
                        CommandDataDescriptionBuilder, Execute, Message, Parse,
                        ReadyForCommandBuilder, Sync, TransactionState,
                    };
                    let mut buffer = StructBuffer::<Message>::default();
                    let mut send_queue = VecDeque::new();
                    loop {
                        if !send_queue.is_empty() {
                            let (a, b) = send_queue.as_slices();
                            eprintln!("Sending {a:?}{b:?}");
                            stream.write_all(a).await?;
                            stream.write_all(b).await?;
                            send_queue.clear();
                        }
                        let mut buf = [0; 1024];
                        let mut buf = ReadBuf::new(&mut buf);
                        let read = stream.read_buf(&mut buf).await?;
                        if read == 0 {
                            eprintln!("<eof>");
                            break;
                        }
                        buffer.push(buf.filled(), |msg| {
                            match_message!(msg, EdgeDBFrontend {
                                (Execute as msg) => {
                                    eprintln!("{msg:?}");
                                },
                                (Parse as msg) => {
                                    eprintln!("{msg:?}");
                                    send_queue.extend(CommandDataDescriptionBuilder {
                                        annotations: &[],
                                        capabilities: 0,
                                        result_cardinality: msg.expected_cardinality(),
                                        input_typedesc_id: Uuid::default(),
                                        input_typedesc: &[],
                                        output_typedesc_id: Uuid::default(),
                                        output_typedesc: &[]
                                    }.to_vec());
                                },
                                (Sync as msg) => {
                                    eprintln!("{msg:?}");
                                    send_queue.extend(ReadyForCommandBuilder {
                                        annotations: &[],
                                        transaction_state: TransactionState::NotInTransaction,
                                    }.to_vec());
                                },
                                unknown => {
                                    match unknown {
                                        Ok(msg) => eprintln!("unknown (mtype = {}): {msg:?}", msg.mtype() as char),
                                        Err(e) => eprintln!("error: {e:?}")
                                    }
                                }
                            })
                        });
                    }
                }
                StreamLanguage::Postgres => {
                    use gel_pg_protocol::protocol::*;
                    let mut buffer = StructBuffer::<Message>::default();
                    let mut send_queue = VecDeque::new();
                    loop {
                        if !send_queue.is_empty() {
                            let (a, b) = send_queue.as_slices();
                            eprintln!("Sending {a:?}{b:?}");
                            stream.write_all(a).await?;
                            stream.write_all(b).await?;
                            send_queue.clear();
                        }
                        let mut buf = [0; 1024];
                        let mut buf = ReadBuf::new(&mut buf);
                        let read = stream.read_buf(&mut buf).await?;
                        if read == 0 {
                            eprintln!("<eof>");
                            break;
                        }
                        buffer.push(buf.filled(), |msg| {
                            match_message!(msg, Frontend {
                                (Execute as msg) => {
                                    eprintln!("{msg:?}");
                                },
                                (Parse as msg) => {
                                    eprintln!("{msg:?}");
                                },
                                (Sync as msg) => {
                                    eprintln!("{msg:?}");
                                },
                                (Query as msg) => {
                                    eprintln!("{msg:?}");
                                },
                                unknown => {
                                    match unknown {
                                        Ok(msg) => eprintln!("unknown (mtype = {}): {msg:?}", msg.mtype() as char),
                                        Err(e) => eprintln!("error: {e:?}")
                                    }
                                }
                            })
                        });
                    }
                }
                _ => loop {
                    let mut buf = [0; 1024];
                    let mut buf = ReadBuf::new(&mut buf);
                    let read = stream.read_buf(&mut buf).await?;
                    if read == 0 {
                        eprintln!("<eof>");
                        break;
                    }
                    for line in hexdump::hexdump_iter(buf.filled()) {
                        eprintln!("{line}");
                    }
                },
            }
            Ok(())
        }
    }

    fn accept_http(
        &self,
        identity: ConnectionIdentity,
        req: hyper::http::Request<hyper::body::Incoming>,
    ) -> impl Future<Output = Result<hyper::http::Response<String>, std::io::Error>> {
        trace!("accept_http: {:?}, {:?}", identity, req);
        self.http_count.fetch_add(1, Ordering::Relaxed);
        async {
            tokio::time::sleep(Duration::from_secs(1)).await;
            Ok(Response::new("Hello!\n".to_string()))
        }
    }

    fn accept_http_unauthenticated(
        &self,
        req: hyper::http::Request<hyper::body::Incoming>,
    ) -> impl Future<Output = Result<hyper::http::Response<String>, std::io::Error>> {
        trace!("accept_http_unauthenticated: {:?}", req);
        self.http_count.fetch_add(1, Ordering::Relaxed);
        async {
            tokio::time::sleep(Duration::from_millis(100)).await;
            Ok(Response::new("Hello (no user)!\n".to_string()))
        }
    }
}

/// Run a test server and connect to it.
fn run_test_service() {
    let server = ExampleService::default();

    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async move {
            let stream_count = server.stream_count.clone();
            let http_count = server.http_count.clone();
            BoundServer::bind(TestListenerConfig::new("localhost:21340"), server).unwrap();
            let mut last_metrics = (0, 0);
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                let metrics = (
                    stream_count.load(Ordering::Relaxed),
                    http_count.load(Ordering::Relaxed),
                );
                if metrics != last_metrics {
                    eprintln!(
                        "http={} req/s, stream={} req/s",
                        metrics.1 - last_metrics.1,
                        metrics.0 - last_metrics.0
                    );
                    last_metrics = metrics;
                }
            }
        });
}

pub fn main() {
    tracing_subscriber::fmt::init();
    run_test_service();
}
