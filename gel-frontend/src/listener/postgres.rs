use crate::{
    config::ListenerConfig,
    listener::handle_connection_inner,
    service::{AuthTarget, BabelfishService, ConnectionIdentityBuilder, StreamLanguage},
    stream::{ListenerStream, StreamPropertiesBuilder, TransportType},
    stream_type::{PostgresInitialMessage, StreamState, StreamType},
};
use bytes::BytesMut;
use gel_auth::postgres::ConnectionSslRequirement;
use gel_auth::postgres::server::{ConnectionDrive, ConnectionEvent, ServerState};
use gel_pg_protocol::errors::{PgError, PgErrorInvalidAuthorizationSpecification};
use std::collections::HashMap;
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{error, trace};

use super::IsBoundConfig;

pub async fn handle_stream_postgres_ssl(
    mut socket: ListenerStream,
    identity: ConnectionIdentityBuilder,
    bound_config: impl IsBoundConfig,
) -> Result<(), std::io::Error> {
    // Postgres checks to see if the socket is readable and fails here
    let peek = [0; 1];
    // let len = socket.peek(&mut peek).await?;
    // if len != 0 {
    //     return Err(std::io::Error::new(
    //         std::io::ErrorKind::InvalidData,
    //         "Invalid SSL handshake",
    //     ));
    // }

    if !bound_config.config().is_supported_final(
        StreamType::PostgresInitial(PostgresInitialMessage::StartupMessage),
        TransportType::Ssl,
        socket.props(),
    ) {
        socket.write_all(b"N").await?;
        return Box::pin(handle_connection_inner(
            StreamState::PgSslUpgrade,
            socket,
            identity,
            bound_config,
        ))
        .await;
    }

    eprintln!("Booting postgres SSL");
    socket.write_all(b"S").await?;

    let ssl_socket = socket.start_tls().await?;
    Box::pin(handle_connection_inner(
        StreamState::PgSslUpgrade,
        ssl_socket,
        identity,
        bound_config,
    ))
    .await
}

pub async fn handle_stream_postgres_initial(
    mut socket: ListenerStream,
    identity: ConnectionIdentityBuilder,
    bound_config: impl IsBoundConfig,
) -> Result<(), std::io::Error> {
    let mut resolved_identity = None;
    let mut server_state = ServerState::new(ConnectionSslRequirement::Disable);
    let mut startup_params = HashMap::with_capacity(16);
    let send_buf = Mutex::new(BytesMut::new());
    let auth_ready = AtomicBool::new(false);
    let params_ready = AtomicBool::new(false);
    let mut update = |update: ConnectionEvent<'_>| {
        use ConnectionEvent::*;
        trace!("UPDATE: {update:?}");
        match update {
            Auth(user, database) => {
                identity.set_pg_database(database);
                identity.set_user(user);
                auth_ready.store(true, Ordering::SeqCst);
            }
            Parameter(name, value) => {
                startup_params.insert(name.to_owned(), value.to_owned());
            }
            Params => params_ready.store(true, Ordering::SeqCst),
            Send(bytes) => {
                // TODO: Reduce copies and allocations here
                send_buf.lock().unwrap().extend_from_slice(&bytes.to_vec());
            }
            SendSSL(..) => unreachable!(),
            ServerError(e) => {
                trace!("ERROR {e:?}");
            }
            StateChanged(..) => {}
            Upgrade => unreachable!(),
        }
        Ok(())
    };

    while !server_state.is_done() || !send_buf.lock().unwrap().is_empty() {
        let send_buf = std::mem::take(&mut *send_buf.lock().unwrap());
        if !send_buf.is_empty() {
            eprintln!("Sending {send_buf:?}");
            socket.write_all(&send_buf).await?;
        } else if auth_ready.swap(false, Ordering::SeqCst) {
            let built = match identity.clone().build() {
                Ok(built) => built,
                Err(e) => {
                    server_state.drive(ConnectionDrive::Fail(PgError::InvalidAuthorizationSpecification(PgErrorInvalidAuthorizationSpecification::InvalidAuthorizationSpecification), "Missing database or user"), &mut update).unwrap();
                    return Ok(());
                }
            };
            resolved_identity = Some(built);
            let auth = bound_config
                .service()
                .lookup_auth(
                    resolved_identity.clone().unwrap(),
                    AuthTarget::Stream(StreamLanguage::Postgres),
                )
                .await?;
            server_state
                .drive(
                    ConnectionDrive::AuthInfo(auth.auth_type(), auth),
                    &mut update,
                )
                .unwrap();
        } else if params_ready.swap(false, Ordering::SeqCst) {
            server_state
                .drive(ConnectionDrive::Ready(1, 2), &mut update)
                .unwrap();
        } else {
            let mut b = [0; 512];
            let n = socket.read(&mut b).await?;
            if n == 0 {
                // EOF
                return Ok(());
            }
            let res = server_state.drive(ConnectionDrive::RawMessage(&b[..n]), &mut update);
            if res.is_err() {
                // TODO?
                error!("{res:?}");
                return Ok(());
            }
        }
    }

    let socket = socket.upgrade(StreamPropertiesBuilder {
        stream_params: Some(startup_params),
        ..Default::default()
    });
    bound_config
        .service()
        .accept_stream(resolved_identity.unwrap(), StreamLanguage::Postgres, socket)
        .await?;

    Ok(())
}
