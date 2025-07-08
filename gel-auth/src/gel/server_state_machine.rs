use gel_db_protocol::errors::EdbError;
use gel_db_protocol::prelude::*;
use gel_db_protocol::protocol::{
    Annotation, AuthenticationOkBuilder, AuthenticationRequiredSASLMessageBuilder,
    AuthenticationSASLContinueBuilder, AuthenticationSASLFinalBuilder,
    AuthenticationSASLInitialResponse, AuthenticationSASLResponse, ClientHandshake,
    EdgeDBBackendBuilder, ErrorResponseBuilder, IntoEdgeDBBackendBuilder, KeyValue, Message,
    ParameterStatusBuilder, ProtocolExtension, ReadyForCommandBuilder, ServerHandshakeBuilder,
    ServerKeyDataBuilder, TransactionState,
};

use crate::handshake::{ServerAuth, ServerAuthDrive, ServerAuthError, ServerAuthResponse};
use crate::{AuthType, CredentialData};
use std::str::Utf8Error;
use tracing::{error, trace, warn};

use super::ConnectionError;

#[derive(Clone, Copy, Debug)]
pub enum ConnectionStateType {
    Connecting,
    Authenticating,
    Synchronizing,
    Ready,
}

#[derive(Debug)]
pub enum ConnectionDrive<'a> {
    RawMessage(&'a [u8]),
    Message(Result<Message<'a>, ParseError>),
    AuthInfo(AuthType, CredentialData),
    Parameter(String, String),
    Ready([u8; 32]),
    Fail(EdbError, &'a str),
}

pub trait ConnectionStateSend {
    fn send<'a, M>(
        &mut self,
        message: impl IntoEdgeDBBackendBuilder<'a, M>,
    ) -> Result<(), std::io::Error>;
    fn auth(
        &mut self,
        user: String,
        database: String,
        branch: String,
    ) -> Result<(), std::io::Error>;
    fn params(&mut self) -> Result<(), std::io::Error>;
}

#[allow(unused)]
pub trait ConnectionStateUpdate: ConnectionStateSend {
    fn parameter(&mut self, name: &str, value: &str) {}
    fn state_changed(&mut self, state: ConnectionStateType) {}
    fn server_error(&mut self, error: &EdbError) {}
    fn protocol_version(&mut self, major: u8, minor: u8) {}
}

#[derive(derive_more::Debug)]
pub enum ConnectionEvent<'a> {
    #[debug("Send(...)")]
    Send(EdgeDBBackendBuilder<'a>),
    Auth(String, String, String),
    Params,
    Parameter(&'a str, &'a str),
    ProtocolVersion(u8, u8),
    StateChanged(ConnectionStateType),
    ServerError(EdbError),
}

impl<F> ConnectionStateSend for F
where
    F: for<'a> FnMut(ConnectionEvent<'a>) -> Result<(), std::io::Error>,
{
    fn send<'a, M>(
        &mut self,
        message: impl IntoEdgeDBBackendBuilder<'a, M>,
    ) -> Result<(), std::io::Error> {
        self(ConnectionEvent::Send(message.into_builder()))
    }

    fn auth(
        &mut self,
        user: String,
        database: String,
        branch: String,
    ) -> Result<(), std::io::Error> {
        self(ConnectionEvent::Auth(user, database, branch))
    }

    fn params(&mut self) -> Result<(), std::io::Error> {
        self(ConnectionEvent::Params)
    }
}

impl<F> ConnectionStateUpdate for F
where
    F: FnMut(ConnectionEvent) -> Result<(), std::io::Error>,
{
    fn parameter(&mut self, name: &str, value: &str) {
        let _ = self(ConnectionEvent::Parameter(name, value));
    }

    fn state_changed(&mut self, state: ConnectionStateType) {
        let _ = self(ConnectionEvent::StateChanged(state));
    }

    fn server_error(&mut self, error: &EdbError) {
        let _ = self(ConnectionEvent::ServerError(*error));
    }
}

#[derive(Debug, derive_more::Display, derive_more::Error, derive_more::From)]
enum ServerError {
    IO(#[from] std::io::Error),
    Protocol(#[from] EdbError),
    Utf8Error(#[from] Utf8Error),
}

impl From<ServerAuthError> for ServerError {
    fn from(value: ServerAuthError) -> Self {
        match value {
            ServerAuthError::InvalidAuthorizationSpecification => {
                ServerError::Protocol(EdbError::AuthenticationError)
            }
            ServerAuthError::InvalidPassword => {
                ServerError::Protocol(EdbError::AuthenticationError)
            }
            ServerAuthError::InvalidSaslMessage(_) => {
                ServerError::Protocol(EdbError::ProtocolError)
            }
            ServerAuthError::UnsupportedAuthType => {
                ServerError::Protocol(EdbError::UnsupportedFeatureError)
            }
            ServerAuthError::InvalidMessageType => ServerError::Protocol(EdbError::ProtocolError),
        }
    }
}

const PROTOCOL_ERROR: ServerError = ServerError::Protocol(EdbError::ProtocolError);
const AUTH_ERROR: ServerError = ServerError::Protocol(EdbError::AuthenticationError);
const PROTOCOL_VERSION_ERROR: ServerError =
    ServerError::Protocol(EdbError::UnsupportedProtocolVersionError);

#[derive(Debug, Default)]
#[allow(clippy::large_enum_variant)] // Auth is much larger
enum ServerStateImpl {
    #[default]
    Initial,
    AuthInfo(String),
    Authenticating(ServerAuth),
    Synchronizing,
    Ready,
    Error,
}

#[derive(Debug, Default)]
pub struct ServerState {
    state: ServerStateImpl,
    buffer: StructBuffer<Message<'static>>,
}

impl ServerState {
    pub fn is_ready(&self) -> bool {
        matches!(self.state, ServerStateImpl::Ready)
    }

    pub fn is_error(&self) -> bool {
        matches!(self.state, ServerStateImpl::Error)
    }

    pub fn is_done(&self) -> bool {
        self.is_ready() || self.is_error()
    }

    pub fn drive(
        &mut self,
        drive: ConnectionDrive,
        update: &mut impl ConnectionStateUpdate,
    ) -> Result<(), ConnectionError> {
        trace!("SERVER DRIVE: {:?} {:?}", self.state, drive);
        let res = match drive {
            ConnectionDrive::RawMessage(raw) => self.buffer.push_fallible(raw, |message| {
                trace!("Parsed message: {message:?}");
                self.state
                    .drive_inner(ConnectionDrive::Message(message), update)
            }),
            drive => self.state.drive_inner(drive, update),
        };

        match res {
            Ok(_) => Ok(()),
            Err(ServerError::IO(e)) => Err(e.into()),
            Err(ServerError::Utf8Error(e)) => Err(e.into()),
            Err(ServerError::Protocol(code)) => {
                self.state = ServerStateImpl::Error;
                send_error(update, code, "Connection error")?;
                Err(code.into())
            }
        }
    }
}

impl ServerStateImpl {
    fn drive_inner(
        &mut self,
        drive: ConnectionDrive,
        update: &mut impl ConnectionStateUpdate,
    ) -> Result<(), ServerError> {
        use ServerStateImpl::*;

        match (&mut *self, drive) {
            (Initial, ConnectionDrive::Message(message)) => {
                match_message!(message, Message {
                    (ClientHandshake as handshake) => {
                        trace!("ClientHandshake: {handshake:?}");

                        // The handshake should generate an event rather than hardcoding the min/max protocol versions.

                        // We support 1.x and 2.0
                        let major_ver = handshake.major_ver();
                        let minor_ver = handshake.minor_ver();
                        match (major_ver, minor_ver) {
                            (..=0, _) => {
                                update.protocol_version(major_ver as u8, minor_ver as u8);
                                update.send(&ServerHandshakeBuilder { major_ver: 1, minor_ver: 0, extensions: Array::<_, ProtocolExtension>::default() })?;
                            }
                            (1, 1..) => {
                                // 1.(1+) never existed
                                return Err(PROTOCOL_VERSION_ERROR);
                            }
                            (2, 1..) | (3.., _) => {
                                update.protocol_version(major_ver as u8, minor_ver as u8);
                                update.send(&ServerHandshakeBuilder { major_ver, minor_ver, extensions: Array::<_, ProtocolExtension>::default() })?;
                            }
                            _ => {}
                        }

                        let mut user = String::new();
                        let mut database = String::new();
                        let mut branch = String::new();
                        for param in handshake.params() {
                            match param.name().to_str()? {
                                "user" => user = param.value().to_owned()?,
                                "database" => database = param.value().to_owned()?,
                                "branch" => branch = param.value().to_owned()?,
                                _ => {}
                            }
                            update.parameter(param.name().to_str()?, param.value().to_str()?);
                        }
                        if user.is_empty() {
                            return Err(AUTH_ERROR);
                        }
                        if database.is_empty() {
                            database = user.clone();
                        }
                        *self = AuthInfo(user.clone());
                        update.auth(user, database, branch)?;
                    },
                    unknown => {
                        log_unknown_message(unknown, "Initial")?;
                    }
                });
            }
            (AuthInfo(username), ConnectionDrive::AuthInfo(auth_type, credential_data)) => {
                let mut auth = ServerAuth::new(username.clone(), auth_type, credential_data);
                match auth.drive(ServerAuthDrive::Initial) {
                    ServerAuthResponse::Initial(AuthType::ScramSha256, _) => {
                        update.send(&AuthenticationRequiredSASLMessageBuilder {
                            methods: &["SCRAM-SHA-256"],
                        })?;
                    }
                    ServerAuthResponse::Complete(..) => {
                        update.send(&AuthenticationOkBuilder {})?;
                        *self = Synchronizing;
                        update.params()?;
                        return Ok(());
                    }
                    ServerAuthResponse::Error(e) => return Err(e.into()),
                    _ => return Err(PROTOCOL_ERROR),
                }
                *self = Authenticating(auth);
            }
            (Authenticating(auth), ConnectionDrive::Message(message)) => {
                match_message!(message, Message {
                    (AuthenticationSASLInitialResponse as sasl) if auth.is_initial_message() => {
                        match auth.drive(ServerAuthDrive::Message(AuthType::ScramSha256, sasl.sasl_data().as_ref())) {
                            ServerAuthResponse::Continue(final_message) => {
                                update.send(&AuthenticationSASLContinueBuilder {
                                    sasl_data: final_message.as_slice(),
                                })?;
                            }
                            ServerAuthResponse::Error(e) => return Err(e.into()),
                            _ => return Err(PROTOCOL_ERROR),
                        }
                    },
                    (AuthenticationSASLResponse as sasl) if !auth.is_initial_message() => {
                        match auth.drive(ServerAuthDrive::Message(AuthType::ScramSha256, sasl.sasl_data().as_ref())) {
                            ServerAuthResponse::Complete(data) => {
                                update.send(&AuthenticationSASLFinalBuilder {
                                    sasl_data: data.as_slice(),
                                })?;
                                update.send(&AuthenticationOkBuilder::default())?;
                                *self = Synchronizing;
                                update.params()?;
                            }
                            ServerAuthResponse::Error(e) => return Err(e.into()),
                            _ => return Err(PROTOCOL_ERROR),
                        }
                    },
                    unknown => {
                        log_unknown_message(unknown, "Authenticating")?;
                    }
                });
            }
            (Synchronizing, ConnectionDrive::Parameter(name, value)) => {
                update.send(&ParameterStatusBuilder {
                    name: name.as_bytes(),
                    value: value.as_bytes(),
                })?;
            }
            (Synchronizing, ConnectionDrive::Ready(key_data)) => {
                update.send(&ServerKeyDataBuilder { data: key_data })?;
                update.send(&ReadyForCommandBuilder {
                    annotations: Array::<_, Annotation>::default(),
                    transaction_state: TransactionState::NotInTransaction,
                })?;
                *self = Ready;
            }
            (_, ConnectionDrive::Fail(error, _)) => {
                return Err(ServerError::Protocol(error));
            }
            _ => {
                error!("Unexpected drive in state {:?}", self);
                return Err(PROTOCOL_ERROR);
            }
        }

        Ok(())
    }
}

fn log_unknown_message(
    message: Result<Message, ParseError>,
    state: &str,
) -> Result<(), ServerError> {
    match message {
        Ok(message) => {
            warn!(
                "Unexpected message {:?} (length {}) received in {} state",
                message.mtype(),
                message.mlen(),
                state
            );
            Ok(())
        }
        Err(e) => {
            error!("Corrupted message received in {} state {:?}", state, e);
            Err(PROTOCOL_ERROR)
        }
    }
}

fn send_error(
    update: &mut impl ConnectionStateUpdate,
    code: EdbError,
    message: &str,
) -> std::io::Result<()> {
    update.server_error(&code);
    update.send(&ErrorResponseBuilder {
        severity: ErrorSeverity::Error as u8,
        error_code: code as u32,
        message,
        attributes: Array::<_, KeyValue>::default(),
    })
}

#[allow(unused)]
enum ErrorSeverity {
    Error = 0x78,
    Fatal = 0xc8,
    Panic = 0xff,
}
