use super::{IsBoundConfig, handle_connection_inner};
use crate::hyper::HyperUpgradedStream;
use crate::{
    service::ConnectionIdentityBuilder,
    stream::{ListenerStream, StreamProperties, TransportType},
    stream_type::{StreamState, negotiate_ws_protocol},
};
use hyper::{Request, StatusCode, upgrade::OnUpgrade};
use hyper::{Response, server::conn::http2};
use hyper_util::rt::TokioIo;
use std::io::ErrorKind;
use std::{future::Future, pin::Pin, sync::Arc};
use tracing::{error, trace};

use hyper::server::conn::http1;

pub async fn handle_stream_http1x(
    socket: ListenerStream,
    identity: ConnectionIdentityBuilder,
    bound_config: impl IsBoundConfig,
) -> Result<(), std::io::Error> {
    let http1 = http1::Builder::new();
    let props = socket.props_clone();
    let conn = http1.serve_connection(
        TokioIo::new(socket),
        HttpService::new(bound_config, props, identity),
    );
    conn.with_upgrades()
        .await
        .map_err(|e| std::io::Error::new(ErrorKind::InvalidData, e))
}

pub async fn handle_stream_http2(
    socket: ListenerStream,
    identity: ConnectionIdentityBuilder,
    bound_config: impl IsBoundConfig,
) -> Result<(), std::io::Error> {
    let mut http2 = http2::Builder::new(hyper_util::rt::TokioExecutor::new());
    http2.enable_connect_protocol();
    let props = socket.props_clone();
    let conn = http2.serve_connection(
        TokioIo::new(socket),
        HttpService::new(bound_config, props, identity),
    );
    tokio::task::spawn(conn)
        .await?
        .map_err(|e| std::io::Error::new(ErrorKind::InvalidData, e))
}

async fn handle_ws_upgrade(
    stream: ListenerStream,
    identity: ConnectionIdentityBuilder,
    bound_config: impl IsBoundConfig,
) -> Result<(), std::io::Error> {
    handle_connection_inner(StreamState::Encapsulated, stream, identity, bound_config).await
}

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

impl<T: IsBoundConfig> hyper::service::Service<Request<hyper::body::Incoming>> for HttpService<T> {
    type Error = std::io::Error;
    type Future =
        Pin<Box<dyn Future<Output = Result<Response<String>, std::io::Error>> + Send + Sync>>;
    type Response = Response<String>;

    fn call(&self, req: Request<hyper::body::Incoming>) -> Self::Future {
        let bound_config = self.bound_config.clone();
        let stream_props = self.stream_props.clone();
        let identity = self.identity.new_builder();
        Box::pin(async move {
            // First, check for invalid URI segments. The server will require fully-normalized paths.
            let uri = req.uri();
            if uri.path()[1..]
                .split('/')
                .any(|segment| segment == "." || segment == ".." || segment.is_empty())
            {
                return Ok(Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body("Invalid request: URI contains invalid segments".to_string())
                    .unwrap());
            }

            req.headers().get("x-edgedb-user");

            if req.extensions().get::<OnUpgrade>().is_some() {
                match req.version() {
                    hyper::Version::HTTP_11 => {
                        return handle_ws_upgrade_http1(stream_props, identity, req, bound_config)
                            .await;
                    }
                    hyper::Version::HTTP_2 => {
                        return handle_ws_upgrade_http2(stream_props, identity, req, bound_config)
                            .await;
                    }
                    _ => {
                        return Ok(Response::builder()
                            .status(StatusCode::HTTP_VERSION_NOT_SUPPORTED)
                            .body("Unsupported HTTP version".to_string())
                            .unwrap());
                    }
                }
            }

            if uri.path().starts_with("/db/") || uri.path().starts_with("/branch/") {
                let mut split = uri.path().split('/');
                split.next();
                if let Some(branch_or_db) = split.next() {
                    if split.next().is_none() {}
                }
            }

            Ok::<_, std::io::Error>(Response::new("Hello!".to_owned()))
        })
    }
}

async fn handle_ws_upgrade_http1(
    stream_props: Arc<StreamProperties>,
    identity: ConnectionIdentityBuilder,
    mut req: Request<hyper::body::Incoming>,
    bound_config: impl IsBoundConfig,
) -> Result<Response<String>, std::io::Error> {
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
                .body("Invalid WebSocket upgrade request".to_string())
                .unwrap());
        }

        tokio::task::spawn(async move {
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
            .body("Switching to WebSocket".to_string())
            .unwrap())
    } else {
        Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Invalid WebSocket upgrade request".to_string())
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
) -> Result<Response<String>, std::io::Error> {
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
                    .body("Invalid WebSocket upgrade request".to_string())
                    .unwrap());
            }

            tokio::task::spawn(async move {
                if let Ok(upgraded) = hyper::upgrade::on(req).await {
                    let stream = ListenerStream::new_websocket(
                        stream_props,
                        HyperUpgradedStream::new(upgraded),
                    );
                    handle_ws_upgrade(stream, identity, bound_config).await;
                }
            });

            Ok(Response::builder()
                .status(StatusCode::OK)
                .body("Switching to WebSocket".to_string())
                .unwrap())
        } else {
            Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body("Invalid WebSocket protocol".to_string())
                .unwrap())
        }
    } else {
        Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Missing protocol extension".to_string())
            .unwrap())
    }
}
