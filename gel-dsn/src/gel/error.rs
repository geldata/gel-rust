use super::branding::*;
use crate::{gel::BuildPhase, host::HostParseError};
use std::{convert::Infallible, num::ParseIntError};

use super::ParamSource;

#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display, PartialOrd, Ord)]
pub enum CompoundSource {
    #[display("DSN")]
    Dsn,
    #[display("Instance")]
    Instance,
    #[display("Credentials file")]
    CredentialsFile,
    #[display("Host and port")]
    HostPort,
    #[display("Unix socket")]
    UnixSocket,
}

#[derive(
    Debug, Clone, PartialEq, Eq, derive_more::Display, derive_more::Error, PartialOrd, Ord,
)]
pub enum TlsSecurityError {
    IncompatibleSecurityOptions,
    InvalidValue,
}

#[derive(
    Debug, Clone, PartialEq, Eq, derive_more::Display, derive_more::Error, PartialOrd, Ord,
)]
pub enum InstanceNameError {
    InvalidInstanceName,
    InvalidCloudOrgName,
    InvalidCloudInstanceName,
}

#[derive(
    Debug, Clone, PartialEq, Eq, derive_more::Display, derive_more::Error, PartialOrd, Ord,
)]
#[error(ignore)]
pub enum InvalidCredentialsFileError {
    FileNotFound,
    #[display("{}={}, {}={}", _0.0, _0.1, _1.0, _1.1)]
    ConflictingSettings((String, String), (String, String)),
    SerializationError(String),
}

#[derive(
    Debug, Clone, PartialEq, Eq, derive_more::Display, derive_more::Error, PartialOrd, Ord,
)]
pub enum InvalidSecretKeyError {
    InvalidJwt,
    MissingIssuer,
}

#[derive(
    Debug, Clone, PartialEq, Eq, derive_more::Display, derive_more::Error, PartialOrd, Ord,
)]
pub enum InvalidDsnError {
    InvalidScheme,
    ParseError,
    DuplicateOptions(#[error(not(source))] String),
    BranchAndDatabase,
}

/// DSN parsing errors.
///
/// This is the top-level error type for DSN parsing errors. It is used to
/// represent errors that may occur when parsing a DSN, accessing environment
/// variables, files that hold credentials, and any other part of the parsing
/// process.
#[derive(
    Debug,
    derive_more::Error,
    derive_more::Display,
    derive_more::From,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
)]
pub enum ParseError {
    #[display("Credentials file not found")]
    CredentialsFileNotFound,
    #[display("Environment variable was not set: {_1} (from {_0})")]
    EnvNotFound(ParamSource, String),
    #[display("{_0} and {_1} are mutually exclusive and cannot be used together")]
    ExclusiveOptions(String, String),
    #[display("File not found")]
    FileNotFound,
    #[display("Invalid credentials file: {_0}")]
    #[from]
    InvalidCredentialsFile(InvalidCredentialsFileError),
    #[display("Invalid database")]
    InvalidDatabase,
    #[display("Invalid DSN: {_0}")]
    #[from]
    InvalidDsn(InvalidDsnError),
    #[display("Invalid DSN or instance name")]
    InvalidDsnOrInstanceName,
    #[display("Invalid host")]
    InvalidHost,
    #[display("Invalid instance name: {_0}")]
    #[from]
    InvalidInstanceName(InstanceNameError),
    #[display("Invalid port")]
    InvalidPort,
    #[display("Invalid secret key")]
    #[from]
    InvalidSecretKey(InvalidSecretKeyError),
    #[display("Invalid TLS security")]
    #[from]
    InvalidTlsSecurity(TlsSecurityError),
    #[display("Invalid user")]
    InvalidUser,
    #[display("Invalid certificate")]
    InvalidCertificate,
    #[display("Invalid duration")]
    InvalidDuration,
    #[display("Multiple compound options were specified while parsing {_0}: {_1:#?}")]
    MultipleCompound(BuildPhase, #[error(not(source))] Vec<CompoundSource>),
    #[display("No connection options specified, and no project manifest file found ({MANIFEST_FILE_DISPLAY_NAME})")]
    NoOptionsOrToml,
    #[display("Project not initialized")]
    ProjectNotInitialised,
    #[display("Secret key not found")]
    SecretKeyNotFound,
    #[display("Unix socket unsupported")]
    UnixSocketUnsupported,
}

impl ParseError {
    pub fn error_type(&self) -> &str {
        match self {
            Self::EnvNotFound(..) => "env_not_found",
            Self::CredentialsFileNotFound => "credentials_file_not_found",
            Self::ExclusiveOptions(..) => "exclusive_options",
            Self::FileNotFound => "file_not_found",
            Self::InvalidCredentialsFile(_) => "invalid_credentials_file",
            Self::InvalidDatabase => "invalid_database",
            Self::InvalidDsn(_) => "invalid_dsn",
            Self::InvalidDsnOrInstanceName => "invalid_dsn_or_instance_name",
            Self::InvalidHost => "invalid_host",
            Self::InvalidInstanceName(_) => "invalid_instance_name",
            Self::InvalidPort => "invalid_port",
            Self::InvalidSecretKey(_) => "invalid_secret_key",
            Self::InvalidTlsSecurity(_) => "invalid_tls_security",
            Self::InvalidUser => "invalid_user",
            Self::InvalidCertificate => "invalid_certificate",
            Self::InvalidDuration => "invalid_duration",
            Self::MultipleCompound(BuildPhase::Environment, _) => "multiple_compound_env",
            Self::MultipleCompound(BuildPhase::Options, _) => "multiple_compound_opts",
            Self::MultipleCompound(BuildPhase::Project, _) => "multiple_compound_project",
            Self::NoOptionsOrToml => "no_options_or_toml",
            Self::ProjectNotInitialised => "project_not_initialised",
            Self::SecretKeyNotFound => "secret_key_not_found",
            Self::UnixSocketUnsupported => "unix_socket_unsupported",
        }
    }

    pub fn gel_error(self) -> gel_errors::Error {
        use gel_errors::ErrorKind;

        match self {
            Self::EnvNotFound(..)
            | Self::CredentialsFileNotFound
            | Self::FileNotFound
            | Self::InvalidCredentialsFile(_)
            | Self::InvalidDatabase
            | Self::InvalidDsn(_)
            | Self::InvalidDsnOrInstanceName
            | Self::InvalidHost
            | Self::InvalidInstanceName(_)
            | Self::InvalidPort
            | Self::InvalidSecretKey(_)
            | Self::InvalidTlsSecurity(_)
            | Self::InvalidUser
            | Self::InvalidCertificate
            | Self::InvalidDuration
            | Self::UnixSocketUnsupported => {
                // The argument is invalid
                gel_errors::InvalidArgumentError::with_source(self)
            }
            Self::MultipleCompound(..) | Self::ExclusiveOptions(..) => {
                // The argument is valid, but the use is invalid
                gel_errors::InterfaceError::with_source(self)
            }
            Self::NoOptionsOrToml | Self::ProjectNotInitialised => {
                // Credentials are missing
                gel_errors::ClientNoCredentialsError::with_source(self)
            }
            Self::SecretKeyNotFound => {
                // Required cloud configuration is missing
                gel_errors::NoCloudConfigFound::with_source(self)
            }
        }
    }
}

impl From<ParseError> for gel_errors::Error {
    fn from(val: ParseError) -> Self {
        val.gel_error()
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

impl From<Infallible> for ParseError {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display, PartialOrd, Ord)]
pub enum Warning {
    #[display("Deprecated credential property: {_0}")]
    DeprecatedCredentialProperty(String),
    #[display("Deprecated environment variable: {_0}")]
    DeprecatedEnvironmentVariable(String, String),
    #[display("Multiple environment variables set: {}", _0.join(", "))]
    MultipleEnvironmentVariables(Vec<String>),
    #[display("{_0} is ignored when using Docker TCP port")]
    DockerPortIgnored(String),
    #[display("Database and branch are set to default values")]
    DefaultDatabaseAndBranch,
    #[display("Updated out-of-date credentials file")]
    UpdatedOutdatedCredentials,
}

#[derive(Debug, Default)]
pub struct Warnings {
    warnings: Vec<Warning>,
}

impl Warnings {
    pub fn warn(&mut self, warning: Warning) {
        self.warnings.push(warning);
    }

    pub fn into_vec(self) -> Vec<Warning> {
        self.warnings
    }

    pub fn iter(&self) -> impl Iterator<Item = &Warning> {
        self.warnings.iter()
    }
}

impl<'a> IntoIterator for &'a Warnings {
    type Item = &'a Warning;

    type IntoIter = std::slice::Iter<'a, Warning>;

    fn into_iter(self) -> Self::IntoIter {
        self.warnings.iter()
    }
}
