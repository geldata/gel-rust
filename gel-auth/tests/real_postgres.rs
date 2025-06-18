#![cfg(unix)]

use std::collections::HashMap;

use gel_auth::postgres::client::{
    ConnectionDrive, ConnectionState, ConnectionStateSend, ConnectionStateType,
    ConnectionStateUpdate,
};
use gel_auth::postgres::{ConnectionSslRequirement, Credentials};
use gel_auth::*;
use gel_pg_captive::*;
use gel_pg_protocol::errors::PgServerError;
use gel_pg_protocol::prelude::StructBuffer;
use gel_pg_protocol::protocol::{IntoFrontendBuilder, IntoInitialBuilder, Message, SSLResponse};
use gel_stream::{Connector, RawStream, StreamUpgrade, Target, TlsParameters};
use rstest::rstest;
use tokio::io::AsyncWriteExt;
use tracing::{trace, Level};

#[derive(Debug, derive_more::Display, derive_more::From, derive_more::Error)]
pub enum ConnectError {
    ServerError(gel_auth::postgres::ConnectionError),
    ConnectionError(gel_stream::ConnectionError),
    SslError(gel_stream::SslError),
    ParseError(gel_pg_protocol::prelude::ParseError),
    IoError(std::io::Error),
}

// Note: these tests will probably move to gel-pg-protocol

#[derive(Clone, Default, Debug)]
pub struct ConnectionParams {
    pub ssl: bool,
    pub params: HashMap<String, String>,
    pub cancellation_key: (i32, i32),
    pub auth: AuthType,
}

pub struct ConnectionDriver {
    send_buffer: Vec<u8>,
    upgrade: bool,
    params: ConnectionParams,
}

impl ConnectionStateSend for ConnectionDriver {
    fn send_initial<'a, M>(
        &mut self,
        message: impl IntoInitialBuilder<'a, M>,
    ) -> Result<(), std::io::Error> {
        let message = message.into_builder();
        self.send_buffer.extend(message.to_vec());
        Ok(())
    }
    fn send<'a, M>(
        &mut self,
        message: impl IntoFrontendBuilder<'a, M>,
    ) -> Result<(), std::io::Error> {
        let message = message.into_builder();
        self.send_buffer.extend(message.to_vec());
        Ok(())
    }
    fn upgrade(&mut self) -> Result<(), std::io::Error> {
        self.upgrade = true;
        self.params.ssl = true;
        Ok(())
    }
}

impl ConnectionStateUpdate for ConnectionDriver {
    fn state_changed(&mut self, state: ConnectionStateType) {
        trace!("State: {state:?}");
    }
    fn cancellation_key(&mut self, pid: i32, key: i32) {
        self.params.cancellation_key = (pid, key);
    }
    fn parameter(&mut self, name: &str, value: &str) {
        self.params.params.insert(name.to_owned(), value.to_owned());
    }
    fn auth(&mut self, auth: AuthType) {
        trace!("Auth: {auth:?}");
        self.params.auth = auth;
    }
}

impl Default for ConnectionDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnectionDriver {
    pub fn new() -> Self {
        Self {
            send_buffer: Vec::new(),
            upgrade: false,
            params: ConnectionParams::default(),
        }
    }

    async fn drive_bytes(
        &mut self,
        state: &mut ConnectionState,
        drive: &[u8],
        message_buffer: &mut StructBuffer<Message<'static>>,
        mut stream: RawStream,
    ) -> Result<RawStream, ConnectError> {
        message_buffer.push_fallible(drive, |msg| {
            state.drive(ConnectionDrive::Message(msg), self)
        })?;
        loop {
            if !self.send_buffer.is_empty() {
                if tracing::enabled!(Level::TRACE) {
                    trace!("Write:");
                    for s in hexdump::hexdump_iter(&self.send_buffer) {
                        trace!("{}", s);
                    }
                }
                stream.write_all(&self.send_buffer).await?;
                self.send_buffer.clear();
            }
            if self.upgrade {
                self.upgrade = false;
                stream = stream.secure_upgrade().await?;
                state.drive(ConnectionDrive::SslReady, self)?;
            } else {
                break;
            }
        }
        Ok(stream)
    }

    async fn drive(
        &mut self,
        state: &mut ConnectionState,
        drive: ConnectionDrive<'_>,
        mut stream: RawStream,
    ) -> Result<RawStream, ConnectError> {
        state.drive(drive, self)?;
        loop {
            if !self.send_buffer.is_empty() {
                if tracing::enabled!(Level::TRACE) {
                    trace!("Write:");
                    for s in hexdump::hexdump_iter(&self.send_buffer) {
                        trace!("{}", s);
                    }
                }
                stream.write_all(&self.send_buffer).await?;
                self.send_buffer.clear();
            }
            if self.upgrade {
                self.upgrade = false;
                stream = stream.secure_upgrade().await?;
                state.drive(ConnectionDrive::SslReady, self)?;
            } else {
                break;
            }
        }
        Ok(stream)
    }
}

pub async fn connect_raw_ssl(
    credentials: Credentials,
    ssl_mode: ConnectionSslRequirement,
    target: Target,
) -> Result<(gel_stream::RawStream, ConnectionParams), ConnectError> {
    let mut state = ConnectionState::new(credentials, ssl_mode);
    let stream = Connector::new(target)?.connect().await?;

    let mut update = ConnectionDriver::new();
    let mut stream = update
        .drive(&mut state, ConnectionDrive::Initial, stream)
        .await?;

    let mut struct_buffer: StructBuffer<Message> = StructBuffer::<Message>::default();

    while !state.is_ready() {
        let mut buffer = [0; 1024];
        let n = tokio::io::AsyncReadExt::read(&mut stream, &mut buffer).await?;
        if n == 0 {
            return Err(ConnectError::ConnectionError(
                std::io::Error::from(std::io::ErrorKind::UnexpectedEof).into(),
            ));
        }
        if tracing::enabled!(Level::TRACE) {
            trace!("Read:");
            let bytes: &[u8] = &buffer[..n];
            for s in hexdump::hexdump_iter(bytes) {
                trace!("{}", s);
            }
        }
        if state.read_ssl_response() {
            let ssl_response = SSLResponse::new(&buffer)?;
            stream = update
                .drive(
                    &mut state,
                    ConnectionDrive::SslResponse(ssl_response),
                    stream,
                )
                .await?;
            continue;
        }

        stream = update
            .drive_bytes(&mut state, &buffer[..n], &mut struct_buffer, stream)
            .await?;
    }
    Ok((stream, update.params))
}

pub fn get_target(postgres_process: &PostgresProcess) -> Target {
    if postgres_process.socket_address.is_tcp() {
        Target::new_resolved_starttls(
            postgres_process.socket_address.clone(),
            TlsParameters::insecure(),
        )
    } else {
        Target::new_resolved(postgres_process.socket_address.clone())
    }
}

#[tokio::test]
async fn test_smoke() -> Result<(), Box<dyn std::error::Error>> {
    let Some(postgres_process) = setup_postgres(AuthType::Trust, Mode::TcpSsl)? else {
        return Ok(());
    };

    let credentials = Credentials {
        username: DEFAULT_USERNAME.to_string(),
        password: DEFAULT_PASSWORD.to_string(),
        database: DEFAULT_DATABASE.to_string(),
        server_settings: Default::default(),
    };

    let target = if postgres_process.socket_address.is_tcp() {
        Target::new_resolved_starttls(
            postgres_process.socket_address.clone(),
            TlsParameters::insecure(),
        )
    } else {
        Target::new_resolved(postgres_process.socket_address.clone())
    };

    let ssl_requirement = ConnectionSslRequirement::Optional;

    let (_socket, params) = connect_raw_ssl(credentials, ssl_requirement, target).await?;

    assert_eq!(params.auth, AuthType::Trust);
    assert!(params.ssl);

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_auth_real(
    #[values(AuthType::Trust, AuthType::Plain, AuthType::Md5, AuthType::ScramSha256)]
    auth: AuthType,
    #[values(Mode::Tcp, Mode::TcpSsl, Mode::Unix)] mode: Mode,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(postgres_process) = setup_postgres(auth, mode)? else {
        return Ok(());
    };

    let credentials = Credentials {
        username: DEFAULT_USERNAME.to_string(),
        password: DEFAULT_PASSWORD.to_string(),
        database: DEFAULT_DATABASE.to_string(),
        server_settings: Default::default(),
    };

    let target = get_target(&postgres_process);

    let ssl_requirement = match mode {
        Mode::TcpSsl => ConnectionSslRequirement::Required,
        _ => ConnectionSslRequirement::Optional,
    };

    let (_socket, params) = connect_raw_ssl(credentials, ssl_requirement, target).await?;

    assert_eq!(matches!(mode, Mode::TcpSsl), params.ssl);
    assert_eq!(auth, params.auth);

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_bad_password(
    #[values(AuthType::Plain, AuthType::Md5, AuthType::ScramSha256)] auth: AuthType,
    #[values(Mode::Tcp, Mode::TcpSsl, Mode::Unix)] mode: Mode,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(postgres_process) = setup_postgres(auth, mode)? else {
        return Ok(());
    };

    let credentials = Credentials {
        username: DEFAULT_USERNAME.to_string(),
        password: "badpassword".to_string(),
        database: DEFAULT_DATABASE.to_string(),
        server_settings: Default::default(),
    };

    let target = get_target(&postgres_process);

    let ssl_requirement = match mode {
        Mode::TcpSsl => ConnectionSslRequirement::Required,
        _ => ConnectionSslRequirement::Optional,
    };

    let res = connect_raw_ssl(credentials, ssl_requirement, target).await;
    let Err(ConnectError::ServerError(gel_auth::postgres::ConnectionError::ServerError(
        PgServerError { code, .. },
    ))) = res
    else {
        panic!("Expected server error");
    };
    assert_eq!(&code.to_code(), b"28P01");

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_bad_username(
    #[values(AuthType::Plain, AuthType::Md5, AuthType::ScramSha256)] auth: AuthType,
    #[values(Mode::Tcp, Mode::TcpSsl, Mode::Unix)] mode: Mode,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(postgres_process) = setup_postgres(auth, mode)? else {
        return Ok(());
    };

    let credentials = Credentials {
        username: "badusername".to_string(),
        password: DEFAULT_PASSWORD.to_string(),
        database: DEFAULT_DATABASE.to_string(),
        server_settings: Default::default(),
    };

    let target = get_target(&postgres_process);

    let ssl_requirement = match mode {
        Mode::TcpSsl => ConnectionSslRequirement::Required,
        _ => ConnectionSslRequirement::Optional,
    };

    let res = connect_raw_ssl(credentials, ssl_requirement, target).await;
    let Err(ConnectError::ServerError(gel_auth::postgres::ConnectionError::ServerError(
        PgServerError { code, .. },
    ))) = res
    else {
        panic!("Expected server error");
    };
    assert_eq!(&code.to_code(), b"28P01");

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_bad_database(
    #[values(AuthType::Plain, AuthType::Md5, AuthType::ScramSha256)] auth: AuthType,
    #[values(Mode::Tcp, Mode::TcpSsl, Mode::Unix)] mode: Mode,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(postgres_process) = setup_postgres(auth, mode)? else {
        return Ok(());
    };

    let credentials = Credentials {
        username: DEFAULT_USERNAME.to_string(),
        password: DEFAULT_PASSWORD.to_string(),
        database: "baddatabase".to_string(),
        server_settings: Default::default(),
    };

    let target = get_target(&postgres_process);

    let ssl_requirement = match mode {
        Mode::TcpSsl => ConnectionSslRequirement::Required,
        _ => ConnectionSslRequirement::Optional,
    };

    let res = connect_raw_ssl(credentials, ssl_requirement, target).await;
    let Err(ConnectError::ServerError(gel_auth::postgres::ConnectionError::ServerError(
        PgServerError { code, .. },
    ))) = res
    else {
        panic!("Expected server error");
    };
    assert_eq!(&code.to_code(), b"3D000");

    Ok(())
}
