use super::{IsBoundConfig, handle_connection_inner};
use crate::hyper::{HyperStream, HyperStreamBody, HyperUpgradedStream};
use crate::service::{BabelfishService, GelVariant, GelVersion, IdentityError, StreamLanguage};
use crate::tower::build_tower;
use crate::{
    service::ConnectionIdentityBuilder,
    stream::{ListenerStream, StreamProperties, TransportType},
    stream_type::{StreamState, negotiate_ws_protocol},
};
use futures::FutureExt;
use hyper::{Request, StatusCode, upgrade::OnUpgrade};
use hyper::{Response, server::conn::http2};
use hyper_util::rt::TokioIo;
use std::io::ErrorKind;
use std::{future::Future, pin::Pin, sync::Arc};
use tokio::io::AsyncWriteExt;
use tracing::{error, trace};

use hyper::server::conn::http1;

const MASCOT: &str = r#"
                     ▄██▄        
   ▄▄▄▄▄      ▄▄▄    ████        
 ▄███████▄ ▄███████▄ ████        
 ▀███████▀ ▀███▀▀▀▀▀ ████        
   ▀▀▀▀▀      ▀▀▀     ▀▀         
  ▀▄▄▄▄▄▀                 
    ▀▀▀               
"#;

pub async fn handle_stream_http0x(
    mut socket: ListenerStream,
    _identity: ConnectionIdentityBuilder,
    _bound_config: impl IsBoundConfig,
) -> Result<(), std::io::Error> {
    socket.write_all(b"HTTP/1.0 200 OK\r\n\r\n").await?;
    socket.write_all(MASCOT.as_bytes()).await?;

    Ok(())
}

pub async fn handle_stream_http1x(
    socket: ListenerStream,
    identity: ConnectionIdentityBuilder,
    bound_config: impl IsBoundConfig,
) -> Result<(), std::io::Error> {
    let mut http1 = http1::Builder::new();
    // Allow client to close write side of the connection
    http1.half_close(true);
    let props = socket.props_clone();
    let service = build_tower(HttpService::new(bound_config, props, identity));
    let conn = http1.serve_connection(TokioIo::new(socket), service);
    match conn.without_shutdown().await {
        Ok(parts) => {
            let io = parts.io.into_inner();
            eprintln!("io: {io:?}");
            Ok(())
        }
        Err(e) if e.is_incomplete_message() => Ok(()),
        Err(e) => Err(std::io::Error::new(ErrorKind::InvalidData, e)),
    }
}

pub async fn handle_stream_http2(
    socket: ListenerStream,
    identity: ConnectionIdentityBuilder,
    bound_config: impl IsBoundConfig,
) -> Result<(), std::io::Error> {
    let mut http2 = http2::Builder::new(hyper_util::rt::TokioExecutor::new());
    http2.enable_connect_protocol();
    let props = socket.props_clone();
    let service = build_tower(HttpService::new(bound_config, props, identity));

    let conn = http2.serve_connection(TokioIo::new(socket), service);
    conn.await
        .map_err(|e| std::io::Error::new(ErrorKind::InvalidData, e))?;
    Ok(())
}

async fn handle_ws_upgrade(
    stream: ListenerStream,
    identity: ConnectionIdentityBuilder,
    bound_config: impl IsBoundConfig,
) -> Result<(), std::io::Error> {
    handle_connection_inner(StreamState::Encapsulated, stream, identity, bound_config).await
}

#[derive(Clone)]
struct HttpService<T: IsBoundConfig> {
    bound_config: T,
    stream_props: Arc<StreamProperties>,
    identity: ConnectionIdentityBuilder,
}

impl<T: IsBoundConfig> HttpService<T> {
    pub fn new(
        bound_config: T,
        stream_props: Arc<StreamProperties>,
        identity: ConnectionIdentityBuilder,
    ) -> Self {
        Self {
            bound_config,
            stream_props,
            identity,
        }
    }
}

impl<T: IsBoundConfig> tower::Service<Request<hyper::body::Incoming>> for HttpService<T> {
    type Error = std::io::Error;
    type Future =
        Pin<Box<dyn Future<Output = Result<Response<HyperStreamBody>, std::io::Error>> + Send>>;
    type Response = Response<HyperStreamBody>;

    fn poll_ready(
        &mut self,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, mut req: Request<hyper::body::Incoming>) -> Self::Future {
        let bound_config = self.bound_config.clone();
        let stream_props = self.stream_props.clone();
        let identity = self.identity.new_builder();

        tokio::task::spawn(async move {
            let content_type = req
                .headers()
                .get(hyper::header::CONTENT_TYPE)
                .map(|v| v.to_str().map(|s| s.to_string()))
                .transpose()
                .map_err(|e| std::io::Error::new(ErrorKind::InvalidData, e))?;

            if let Some(user) = req.headers().get("x-gel-user") {
                if let Ok(user) = user.to_str() {
                    identity.set_user(user.to_string());
                } else {
                    return Ok(Response::new(HyperStream::static_response("Invalid user")));
                }
            } else if let Some(user) = req.headers().get("x-edgedb-user") {
                if let Ok(user) = user.to_str() {
                    identity.set_user(user.to_string());
                } else {
                    return Ok(Response::new(HyperStream::static_response("Invalid user")));
                }
            };

            let stream_props = StreamProperties {
                parent: Some(stream_props),
                http_version: Some(req.version()),
                request_headers: Some(std::mem::take(req.headers_mut())),
                request_uri: Some(req.uri().clone()),
                ..StreamProperties::new(TransportType::Http)
            };

            // Special case for the root path.
            if req.uri().path() == "/" {
                let mut resp = Response::new(HyperStream::static_response(MASCOT));
                resp.headers_mut()
                    .insert("Content-Type", "text/plain; charset=utf-8".parse().unwrap());

                return Ok(resp);
            }

            // First, check for invalid URI segments. The server will require fully-normalized paths.
            let uri = req.uri();
            if uri.path()[1..]
                .split('/')
                .any(|segment| segment == "." || segment == ".." || segment.is_empty())
            {
                return Ok(Response::new(HyperStream::static_response(
                    "Invalid request: URI contains invalid segments",
                )));
            }

            if req.extensions().get::<OnUpgrade>().is_some() {
                match req.version() {
                    hyper::Version::HTTP_11 => {
                        return handle_ws_upgrade_http1(
                            stream_props.into(),
                            identity,
                            req,
                            bound_config,
                        )
                        .await;
                    }
                    hyper::Version::HTTP_2 => {
                        return handle_ws_upgrade_http2(
                            stream_props.into(),
                            identity,
                            req,
                            bound_config,
                        )
                        .await;
                    }
                    _ => {
                        return Ok(Response::new(HyperStream::static_response(
                            "Unsupported HTTP version",
                        )));
                    }
                }
            }

            if uri.path().starts_with("/db/") || uri.path().starts_with("/branch/") {
                let mut split = uri.path().split('/');
                assert_eq!(split.next(), Some(""));
                assert!(matches!(split.next(), Some("db") | Some("branch")));
                if let Some(branch_or_db) = split.next() {
                    if uri.path().starts_with("/db/") {
                        identity.set_database(branch_or_db.to_string());
                    } else {
                        identity.set_branch(branch_or_db.to_string());
                    }
                }

                let next = split.next();

                if next.is_none() || (next == Some("") && split.next().is_none()) {
                    // If this request is a POST, AND the content-type is application/x.edgedb.v_x_x.binary,
                    // then we need to convert the request body into a stream.
                    if req.method() == hyper::Method::POST {
                        // TODO: other versions
                        if content_type.as_deref() == Some("application/x.edgedb.v_3_0.binary") {
                            let identity = match identity.build() {
                                Ok(identity) => identity,
                                Err(IdentityError::NoUser) => {
                                    return Ok(Response::new(HyperStream::static_response(
                                        "Unauthorized",
                                    )));
                                }
                                Err(e) => {
                                    error!("Failed to build identity: {e:?}");
                                    return Ok(Response::new(HyperStream::static_response(
                                        "Unauthorized",
                                    )));
                                }
                            };

                            // Convert the request body into a stream
                            let (incoming, response) = HyperStream::new(req.into_body());
                            let stream = ListenerStream::new_http(stream_props, incoming);
                            tokio::task::spawn(async move {
                                bound_config
                                    .service()
                                    .accept_stream(
                                        identity,
                                        StreamLanguage::Gel(GelVersion::V3, GelVariant::Wire),
                                        stream,
                                    )
                                    .await
                            });
                            return Ok(Response::new(response));
                        }
                    } else {
                        return Ok(Response::new(HyperStream::static_response(
                            "Invalid request",
                        )));
                    }
                }
            }

            // This probably needs to handle more cases
            let identity = match identity.build() {
                Ok(identity) => Some(identity),
                Err(IdentityError::NoUser) => None,
                Err(e) => {
                    error!("Failed to build identity: {e:?}");
                    return Ok(Response::new(HyperStream::static_response("Unauthorized")));
                }
            };

            if let Some(identity) = identity {
                Ok(bound_config
                    .service()
                    .accept_http(identity, req)
                    .await?
                    .map(Into::into))
            } else {
                Ok(bound_config
                    .service()
                    .accept_http_unauthenticated(req)
                    .await?
                    .map(Into::into))
            }
        })
        .map(|r| r.unwrap())
        .boxed()
    }
}

async fn handle_ws_upgrade_http1(
    stream_props: Arc<StreamProperties>,
    identity: ConnectionIdentityBuilder,
    mut req: Request<hyper::body::Incoming>,
    bound_config: impl IsBoundConfig,
) -> Result<Response<HyperStreamBody>, std::io::Error> {
    let mut stream_props = StreamProperties {
        parent: Some(stream_props),
        http_version: Some(req.version()),
        ..StreamProperties::new(TransportType::WebSocket)
    };

    let mut ws_key = None;
    let mut ws_version = None;
    let mut ws_protocol = None;

    if let Some(upgrade) = req.headers().get(hyper::header::UPGRADE) {
        if upgrade.as_bytes().eq_ignore_ascii_case(b"websocket") {
            ws_key = req
                .headers()
                .get(hyper::header::SEC_WEBSOCKET_KEY)
                .map(|v| v.to_str().unwrap_or("").to_string());
            ws_version = req
                .headers()
                .get(hyper::header::SEC_WEBSOCKET_VERSION)
                .map(|v| v.to_str().unwrap_or("").to_string());
            ws_protocol = req
                .headers()
                .get(hyper::header::SEC_WEBSOCKET_PROTOCOL)
                .map(|v| v.to_str().unwrap_or("").to_string());
        }
    }

    stream_props.request_headers = Some(std::mem::take(req.headers_mut()));

    if let (Some(key), Some(version)) = (ws_key, ws_version) {
        trace!("WebSocket upgrade request detected:");
        trace!("  Key: {}", key);
        trace!("  Version: {}", version);
        if let Some(protocol) = &ws_protocol {
            trace!("  Protocol: {}", protocol);
            stream_props.protocol =
                negotiate_ws_protocol(bound_config.config().as_ref(), protocol, &stream_props);
        }

        if stream_props.protocol.is_none() {
            return Ok(Response::builder()
                .status(StatusCode::FORBIDDEN)
                .body(HyperStream::static_response(
                    "Invalid WebSocket upgrade request",
                ))
                .unwrap());
        }

        tokio::task::spawn_local(async move {
            if let Ok(upgraded) = hyper::upgrade::on(req).await {
                let stream =
                    ListenerStream::new_websocket(stream_props, HyperUpgradedStream::new(upgraded));
                if let Err(err) = handle_ws_upgrade(stream, identity, bound_config).await {
                    error!("WebSocket task failed {err:?}");
                }
            }
        });

        Ok(Response::builder()
            .status(StatusCode::SWITCHING_PROTOCOLS)
            .header(hyper::header::UPGRADE, "websocket")
            .header(hyper::header::CONNECTION, "Upgrade")
            .header(
                hyper::header::SEC_WEBSOCKET_ACCEPT,
                generate_ws_accept(&key),
            )
            .body(HyperStream::static_response("Switching to WebSocket"))
            .unwrap())
    } else {
        Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(HyperStream::static_response(
                "Invalid WebSocket upgrade request",
            ))
            .unwrap())
    }
}

fn generate_ws_accept(key: &str) -> String {
    use base64::{Engine as _, engine::general_purpose};
    use sha1::{Digest, Sha1};

    let mut sha1 = Sha1::new();
    sha1.update(key.as_bytes());
    sha1.update(b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
    let result = sha1.finalize();
    general_purpose::STANDARD.encode(result)
}

async fn handle_ws_upgrade_http2(
    stream_props: Arc<StreamProperties>,
    identity: ConnectionIdentityBuilder,
    mut req: Request<hyper::body::Incoming>,
    bound_config: impl IsBoundConfig,
) -> Result<Response<HyperStreamBody>, std::io::Error> {
    let mut stream_props = StreamProperties {
        parent: Some(stream_props),
        http_version: Some(req.version()),
        ..StreamProperties::new(TransportType::WebSocket)
    };
    if let Some(protocol) = req.extensions().get::<hyper::ext::Protocol>() {
        if protocol.as_str().eq_ignore_ascii_case("websocket") {
            let ws_version = req
                .headers()
                .get(hyper::header::SEC_WEBSOCKET_VERSION)
                .map(|v| v.to_str().unwrap_or("").to_string());
            let ws_protocol = req
                .headers()
                .get(hyper::header::SEC_WEBSOCKET_PROTOCOL)
                .map(|v| v.to_str().unwrap_or("").to_string());
            stream_props.request_headers = Some(std::mem::take(req.headers_mut()));

            if let Some(version) = ws_version {
                trace!("HTTP/2 WebSocket upgrade request detected:");
                trace!("  Version: {}", version);
                if let Some(protocol) = &ws_protocol {
                    trace!("  Protocol: {}", protocol);
                    stream_props.protocol = negotiate_ws_protocol(
                        bound_config.config().as_ref(),
                        protocol,
                        &stream_props,
                    );
                }
            }

            if stream_props.protocol.is_none() {
                return Ok(Response::builder()
                    .status(StatusCode::FORBIDDEN)
                    .body(HyperStream::static_response(
                        "Invalid WebSocket upgrade request",
                    ))
                    .unwrap());
            }

            tokio::task::spawn_local(async move {
                match hyper::upgrade::on(req).await {
                    Ok(upgraded) => {
                        let stream = ListenerStream::new_websocket(
                            stream_props,
                            HyperUpgradedStream::new(upgraded),
                        );
                        if let Err(err) = handle_ws_upgrade(stream, identity, bound_config).await {
                            error!("WebSocket task failed {err:?}");
                        }
                    }
                    Err(e) => {
                        error!("Failed to upgrade WebSocket: {e:?}");
                    }
                }
            });

            Ok(Response::builder()
                .status(StatusCode::OK)
                .body(HyperStream::static_response("Switching to WebSocket"))
                .unwrap())
        } else {
            Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(HyperStream::static_response("Invalid WebSocket protocol"))
                .unwrap())
        }
    } else {
        Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(HyperStream::static_response("Missing protocol extension"))
            .unwrap())
    }
}
