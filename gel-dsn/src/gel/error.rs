use crate::host::HostParseError;
use std::{convert::Infallible, num::ParseIntError};

#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display, PartialOrd, Ord)]
pub enum CompoundSource {
    Dsn,
    Instance,
    CredentialsFile,
    HostPort,
    UnixSocket,
}

#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display, PartialOrd, Ord)]

pub enum TlsSecurityError {
    IncompatibleSecurityOptions,
    InvalidValue,
}

#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display, PartialOrd, Ord)]
pub enum InstanceNameError {
    InvalidInstanceName,
    InvalidCloudOrgName,
    InvalidCloudInstanceName,
}

#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display, PartialOrd, Ord)]
pub enum InvalidCredentialsFileError {
    FileNotFound,
    #[display("{}={}, {}={}", _0.0, _0.1, _1.0, _1.1)]
    ConflictingSettings((String, String), (String, String)),
    SerializationError(String),
}

#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display, PartialOrd, Ord)]
pub enum InvalidSecretKeyError {
    InvalidJwt,
    MissingIssuer,
}

#[derive(Debug, derive_more::Error, derive_more::Display, PartialEq, Eq, PartialOrd, Ord)]
pub enum ParseError {
    CredentialsFileNotFound,
    EnvNotFound,
    ExclusiveOptions,
    FileNotFound,
    InvalidCredentialsFile(#[error(not(source))] InvalidCredentialsFileError),
    InvalidDatabase,
    InvalidDsn,
    InvalidDsnOrInstanceName,
    InvalidHost,
    InvalidInstanceName(#[error(not(source))] InstanceNameError),
    InvalidPort,
    InvalidSecretKey(#[error(not(source))] InvalidSecretKeyError),
    InvalidTlsSecurity(#[error(not(source))] TlsSecurityError),
    InvalidUser,
    #[display("{:?}", _0)]
    MultipleCompoundEnv(#[error(not(source))] Vec<CompoundSource>),
    #[display("{:?}", _0)]
    MultipleCompoundOpts(#[error(not(source))] Vec<CompoundSource>),
    NoOptionsOrToml,
    ProjectNotInitialised,
    SecretKeyNotFound,
    UnixSocketUnsupported,
}

impl ParseError {
    pub fn error_type(&self) -> &str {
        match self {
            Self::EnvNotFound => "env_not_found",
            Self::CredentialsFileNotFound => "credentials_file_not_found",
            Self::ExclusiveOptions => "exclusive_options",
            Self::FileNotFound => "file_not_found",
            Self::InvalidCredentialsFile(_) => "invalid_credentials_file",
            Self::InvalidDatabase => "invalid_database",
            Self::InvalidDsn => "invalid_dsn",
            Self::InvalidDsnOrInstanceName => "invalid_dsn_or_instance_name",
            Self::InvalidHost => "invalid_host",
            Self::InvalidInstanceName(_) => "invalid_instance_name",
            Self::InvalidPort => "invalid_port",
            Self::InvalidSecretKey(_) => "invalid_secret_key",
            Self::InvalidTlsSecurity(_) => "invalid_tls_security",
            Self::InvalidUser => "invalid_user",
            Self::MultipleCompoundEnv(_) => "multiple_compound_env",
            Self::MultipleCompoundOpts(_) => "multiple_compound_opts",
            Self::NoOptionsOrToml => "no_options_or_toml",
            Self::ProjectNotInitialised => "project_not_initialised",
            Self::SecretKeyNotFound => "secret_key_not_found",
            Self::UnixSocketUnsupported => "unix_socket_unsupported",
        }
    }
}

impl From<ParseIntError> for ParseError {
    fn from(_: ParseIntError) -> Self {
        ParseError::InvalidPort
    }
}

impl From<HostParseError> for ParseError {
    fn from(_: HostParseError) -> Self {
        ParseError::InvalidHost
    }
}

impl From<std::env::VarError> for ParseError {
    fn from(error: std::env::VarError) -> Self {
        match error {
            std::env::VarError::NotPresent => ParseError::EnvNotFound,
            std::env::VarError::NotUnicode(_) => ParseError::EnvNotFound,
        }
    }
}

impl From<Infallible> for ParseError {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}
