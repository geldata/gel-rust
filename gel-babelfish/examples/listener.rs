use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{future::Future, time::Duration};

use gel_auth::AuthType;
use gel_auth::CredentialData;
use gel_babelfish::config::TestListenerConfig;
use gel_babelfish::hyper::HyperStreamBody;
use gel_babelfish::listener::BoundServer;
use gel_babelfish::service::{AuthTarget, BabelfishService, ConnectionIdentity, StreamLanguage};
use gel_babelfish::stream::ListenerStream;
use gel_pg_protocol::prelude::*;
use hyper::Response;
use tokio::io::ReadBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::trace;
use uuid::Uuid;

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
                StreamLanguage::Gel(_, _) => {
                    use gel_db_protocol::protocol::{
                        Annotation, CommandCompleteBuilder, CommandDataDescriptionBuilder,
                        DataBuilder, DataElementBuilder, Execute, Execute2, Message, Parse, Parse2,
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
                                    if msg.command_text() == "SELECT 1" {
                                        send_queue.extend(DataBuilder {
                                            data: &[&DataElementBuilder { data: b"1" }],
                                        }.to_vec());
                                    } else if msg.command_text() == "SELECT sys::get_version_as_str()" {
                                        send_queue.extend(DataBuilder {
                                            data: &[&DataElementBuilder { data: b"7.0+fffffff" }],
                                        }.to_vec());
                                    } else {
                                        // todo
                                    }
                                    send_queue.extend(CommandCompleteBuilder {
                                        status: "SELECT",
                                        annotations: Array::<_, Annotation>::empty(),
                                        capabilities: 0,
                                        state_typedesc_id: Uuid::default(),
                                        state_data: Array::<_, u8>::empty(),
                                    }.to_vec());
                                },
                                (Parse as msg) => {
                                    eprintln!("{msg:?}");
                                    send_queue.extend(CommandDataDescriptionBuilder {
                                        annotations: Array::<_, Annotation>::empty(),
                                        capabilities: 0,
                                        result_cardinality: msg.expected_cardinality(),
                                        input_typedesc_id: Uuid::default(),
                                        input_typedesc: Array::<_, u8>::empty(),
                                        output_typedesc_id: Uuid::from_u128(0x101),
                                        output_typedesc: b"\0\0\0 \x03\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x01\x01\0\0\0\x08std::str\x01\0\0"
                                    }.to_vec());
                                },
                                (Sync as msg) => {
                                    eprintln!("{msg:?}");
                                    send_queue.extend(ReadyForCommandBuilder {
                                        annotations: Array::<_, Annotation>::empty(),
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
    ) -> impl Future<Output = Result<hyper::http::Response<HyperStreamBody>, std::io::Error>> {
        trace!("accept_http: {:?}, {:?}", identity, req);
        self.http_count.fetch_add(1, Ordering::Relaxed);
        async {
            tokio::time::sleep(Duration::from_secs(1)).await;
            Ok(Response::new("Hello!\n".into()))
        }
    }

    fn accept_http_unauthenticated(
        &self,
        req: hyper::http::Request<hyper::body::Incoming>,
    ) -> impl Future<Output = Result<hyper::http::Response<HyperStreamBody>, std::io::Error>> {
        trace!("accept_http_unauthenticated: {:?}", req);
        self.http_count.fetch_add(1, Ordering::Relaxed);
        async {
            tokio::time::sleep(Duration::from_millis(100)).await;
            Ok(Response::new("Hello (no user)!\n".into()))
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
