use std::{
    collections::HashMap, convert::Infallible, num::ParseIntError, path::Path, str::FromStr,
    time::Duration,
};

use super::{
    params::{parse_env, BuildPhase, Explicit, Project},
    BuildContext, ClientSecurity, Param, TlsSecurity,
};
use crate::host::{Host, HostParseError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub host: Host,
    pub db: DatabaseBranch,
    pub user: Option<String>,

    pub authentication: Authentication,

    pub client_security: ClientSecurity,
    pub tls_security: TlsSecurity,

    pub tls_ca: Option<String>,
    pub tls_server_name: Option<String>,
    pub wait_until_available: Duration,

    pub server_settings: HashMap<String, String>,
}

impl Config {
    #[cfg(feature = "serde")]
    pub fn to_json(&self) -> impl serde::Serialize {
        use serde::Serialize;
        use std::collections::BTreeMap;

        #[derive(Serialize)]
        #[allow(non_snake_case)]
        struct ConfigJson {
            address: (String, usize),
            branch: String,
            database: String,
            password: Option<String>,
            secretKey: Option<String>,
            serverSettings: BTreeMap<String, String>,
            tlsCAData: Option<String>,
            tlsSecurity: String,
            tlsServerName: Option<String>,
            user: String,
            waitUntilAvailable: String,
        }

        ConfigJson {
            address: (self.host.0.to_string(), self.host.1 as usize),
            branch: self.db.branch().to_string(),
            database: self.db.database().to_string(),
            password: self.authentication.password().map(|s| s.to_string()),
            secretKey: self.authentication.secret_key().map(|s| s.to_string()),
            serverSettings: BTreeMap::from_iter(self.server_settings.clone()),
            tlsCAData: self.tls_ca.clone(),
            tlsSecurity: self.tls_security.to_string(),
            tlsServerName: self.tls_server_name.clone(),
            user: self.user.clone().unwrap_or("edgedb".to_string()),
            waitUntilAvailable: gel_protocol::model::RelativeDuration::try_from_micros(
                self.wait_until_available.as_micros() as i64,
            )
            .map(|d| d.to_string())
            .unwrap_or("PT30S".to_string()),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Authentication {
    #[default]
    None,
    Password(String),
    SecretKey(String),
}

impl Authentication {
    pub fn password(&self) -> Option<&str> {
        match self {
            Self::Password(password) => Some(password),
            _ => None,
        }
    }

    pub fn secret_key(&self) -> Option<&str> {
        match self {
            Self::SecretKey(secret_key) => Some(secret_key),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum DatabaseBranch {
    #[default]
    Default,
    Database(String),
    Branch(String),
    Ambiguous(String),
}

impl DatabaseBranch {
    pub fn database(&self) -> &str {
        match self {
            Self::Database(database) => database,
            Self::Branch(branch) => branch,
            Self::Ambiguous(ambiguous) => ambiguous,
            _ => "edgedb",
        }
    }

    pub fn branch(&self) -> &str {
        match self {
            Self::Branch(branch) => branch,
            Self::Database(database) => database,
            Self::Ambiguous(ambiguous) => ambiguous,
            _ => "__default__",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display, PartialOrd, Ord)]
pub enum CompoundSource {
    Dsn,
    Instance,
    CredentialsFile,
    HostPort,
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

    pub fn is_fatal(&self) -> bool {
        matches!(
            self,
            Self::InvalidInstanceName(_)
                | Self::MultipleCompoundEnv(_)
                | Self::MultipleCompoundOpts(_)
                | Self::SecretKeyNotFound
                | Self::InvalidUser
                | Self::InvalidDsn
                | Self::InvalidDatabase
                | Self::CredentialsFileNotFound
                | Self::UnixSocketUnsupported
        )
    }
}

impl From<url::ParseError> for ParseError {
    fn from(error: url::ParseError) -> Self {
        ParseError::InvalidDsn
    }
}

impl From<ParseIntError> for ParseError {
    fn from(error: ParseIntError) -> Self {
        ParseError::InvalidPort
    }
}

impl From<HostParseError> for ParseError {
    fn from(error: HostParseError) -> Self {
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
    fn from(error: Infallible) -> Self {
        unreachable!()
    }
}

/// Parse the connection from the given sources given the following precedence:
///
/// 1. Explicit options
/// 2. Environment variables (GEL_DSN / GEL_INSTANCE / GEL_CREDENTIALS_FILE / GEL_HOST+GEL_PORT)
///
/// If no explicit options or environment variables were provided, the project-linked credentials will be used.
///
pub fn parse(
    mut explicit: Explicit,
    context: &mut impl BuildContext,
    project: Option<&Path>,
) -> Result<Config, ParseError> {
    if let Some(config) = explicit.try_build(context, BuildPhase::Options)? {
        return Ok(config);
    }

    let env_params = parse_env(context)?;
    explicit.merge(env_params);

    if let Some(config) = explicit.try_build(context, BuildPhase::Environment)? {
        return Ok(config);
    }

    if let Some(project) = project {
        let project = Project::load(&project, context)?;
        explicit.merge(Explicit {
            cloud_profile: Param::from_unparsed(project.cloud_profile),
            instance: Param::from_parsed(Some(project.instance_name)),
            database: Param::from_unparsed(project.database),
            branch: Param::from_unparsed(project.branch),
            ..Default::default()
        });
    }

    if let Some(config) = explicit.try_build(context, BuildPhase::Project)? {
        return Ok(config);
    }

    return Err(ParseError::NoOptionsOrToml);
}

#[cfg(test)]
mod tests {
    use crate::{
        gel::{BuildContextImpl, Param},
        host::HostType,
    };

    use super::*;

    #[test]
    fn test_parse() {
        let explicit = Explicit {
            dsn: Param::Unparsed("edgedb://localhost:5656".to_string()),
            ..Default::default()
        };
        let mut build_context = BuildContextImpl::new_with((), ());
        let result = parse(explicit, &mut build_context, None);
        assert_eq!(
            result,
            Ok(Config {
                host: Host(HostType::Hostname("localhost".to_string()), 5656),
                db: DatabaseBranch::Default,
                user: None,
                authentication: Authentication::None,
                client_security: ClientSecurity::Default,
                tls_security: TlsSecurity::Strict,
                tls_ca: None,
                tls_server_name: None,
                wait_until_available: Duration::from_secs(30),
                server_settings: HashMap::new(),
            })
        );
    }
}
