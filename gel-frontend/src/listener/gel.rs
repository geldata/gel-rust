use crate::{
    service::{AuthTarget, BabelfishService, ConnectionIdentityBuilder, StreamLanguage},
    stream::{ListenerStream, StreamPropertiesBuilder},
};
use bytes::BytesMut;
use gel_auth::gel::EdbError;
use gel_auth::gel::server::{ConnectionDrive, ConnectionEvent, ServerState};
use std::collections::HashMap;
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{error, trace};

use super::IsBoundConfig;

pub async fn handle_stream_gel_binary(
    mut socket: ListenerStream,
    identity: ConnectionIdentityBuilder,
    bound_config: impl IsBoundConfig,
) -> Result<(), std::io::Error> {
    let mut resolved_identity = None;
    let mut server_state = ServerState::default();
    let mut startup_params = HashMap::with_capacity(16);
    let send_buf = Mutex::new(BytesMut::new());
    let auth_ready = AtomicBool::new(false);
    let params_ready = AtomicBool::new(false);
    let mut update = |update: ConnectionEvent<'_>| {
        use ConnectionEvent::*;
        trace!("UPDATE: {update:?}");
        match update {
            Auth(user, database, branch) => {
                identity.set_branch(branch);
                identity.set_database(database);
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
            ServerError(e) => {
                trace!("ERROR {e:?}");
            }
            StateChanged(..) => {}
        }
        Ok(())
    };

    while !server_state.is_done() || !send_buf.lock().unwrap().is_empty() {
        let send_buf = std::mem::take(&mut *send_buf.lock().unwrap());
        if !send_buf.is_empty() {
            eprintln!("Sending {send_buf:?}");
            socket.write_all(&send_buf).await?;
        } else if auth_ready.swap(false, Ordering::SeqCst) {
            eprintln!("auth ready");
            let built = match identity.clone().build() {
                Ok(built) => built,
                Err(e) => {
                    server_state
                        .drive(
                            ConnectionDrive::Fail(
                                EdbError::AuthenticationError,
                                "Missing database or user",
                            ),
                            &mut update,
                        )
                        .unwrap();
                    return Ok(());
                }
            };
            resolved_identity = Some(built);
            let auth = bound_config
                .service()
                .lookup_auth(
                    resolved_identity.clone().unwrap(),
                    AuthTarget::Stream(StreamLanguage::EdgeDB),
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
                .drive(ConnectionDrive::Ready(Default::default()), &mut update)
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

    eprintln!("ready");
    let socket = socket.upgrade(StreamPropertiesBuilder {
        stream_params: Some(startup_params),
        ..Default::default()
    });
    bound_config
        .service()
        .accept_stream(resolved_identity.unwrap(), StreamLanguage::EdgeDB, socket)
        .await?;

    Ok(())
}
