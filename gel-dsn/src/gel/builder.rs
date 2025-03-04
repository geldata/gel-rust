use std::{collections::HashMap, path::Path, time::Duration};

use super::{
    params::{parse_env, BuildPhase, Params, Project},
    BuildContext, ClientSecurity, Param, ParseError, TlsSecurity,
};
use crate::host::Host;

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

/// The authentication method to use for the connection.
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

/// The database or branch to use for the connection.
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

/// Parse the connection from the given sources given the following precedence:
///
/// 1. Explicit options
/// 2. Environment variables (GEL_DSN / GEL_INSTANCE / GEL_CREDENTIALS_FILE / GEL_HOST+GEL_PORT)
///
/// If no explicit options or environment variables were provided, the project-linked credentials will be used.
///
pub fn parse(
    mut explicit: Params,
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
        let project = Project::load(project, context)?;
        explicit.merge(Params {
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

    Err(ParseError::NoOptionsOrToml)
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
        let explicit = Params {
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
