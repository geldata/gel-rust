use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    net::IpAddr,
    num::NonZeroU16,
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use serde::{Deserialize, Serialize};
use url::Url;

use super::{
    builder::{CompoundSource, InvalidCredentialsFileError, TlsSecurityError},
    env::Env,
    BuildContext, ClientSecurity, Config, DatabaseBranch, FromParamStr, InstanceName, ParseError,
    TlsSecurity,
};
use crate::{
    env::EnvVar,
    gel::Authentication,
    host::{Host, HostType},
    FileAccess,
};

#[derive(Default, Debug, Clone)]
pub enum Param<T: Clone> {
    #[default]
    None,
    Unparsed(String),
    Env(String),
    File(PathBuf),
    EnvFile(String),
    Parsed(T),
}

impl<T: Clone> Param<T>
where
    T: FromParamStr,
    <T as FromParamStr>::Err: Into<ParseError>,
{
    pub fn from_unparsed(value: Option<String>) -> Self {
        if let Some(value) = value {
            Self::Unparsed(value)
        } else {
            Self::None
        }
    }

    pub fn from_file(value: Option<impl AsRef<Path>>) -> Self {
        if let Some(value) = value {
            Self::File(value.as_ref().to_path_buf())
        } else {
            Self::None
        }
    }

    pub fn from_parsed(value: Option<T>) -> Self {
        if let Some(value) = value {
            Self::Parsed(value)
        } else {
            Self::None
        }
    }

    fn cast<U: Clone>(self) -> Result<Param<U>, Self> {
        match self {
            Self::None => Ok(Param::None),
            Self::Unparsed(value) => Ok(Param::Unparsed(value)),
            Self::Env(value) => Ok(Param::Env(value)),
            Self::File(value) => Ok(Param::File(value)),
            Self::EnvFile(value) => Ok(Param::EnvFile(value)),
            Self::Parsed(value) => Err(Self::Parsed(value)),
        }
    }

    pub fn get(&self, context: &mut impl BuildContext) -> Result<Option<T>, ParseError> {
        let value = match self {
            Self::None => {
                return Ok(None);
            }
            Self::Unparsed(value) => value.clone(),
            Self::Env(key) => {
                context.trace(&format!("Reading env: {key}"));
                context
                    .env()
                    .read(key)
                    .map(|s| s.to_string())
                    .map_err(|e| ParseError::EnvNotFound)?
            }
            Self::File(path) => {
                context.trace(&format!("Reading file: {path:?}"));
                let res = context
                    .files()
                    .read(path)
                    .map(|s| s.to_string())
                    .map_err(|_| ParseError::FileNotFound);
                context.trace(&format!("File content: {res:?}"));
                res?
            }
            Self::EnvFile(key) => {
                context.trace(&format!("Reading env for file: {key}"));
                let env = context
                    .env()
                    .read(key)
                    .map_err(|_| ParseError::EnvNotFound)?
                    .to_string();
                context.trace(&format!("Reading file: {env}"));
                let res = context
                    .files()
                    .read(&PathBuf::from(env))
                    .map_err(|_| ParseError::FileNotFound);
                context.trace(&format!("File content: {res:?}"));
                res?
            }
            Self::Parsed(value) => return Ok(Some(value.clone())),
        };

        let value = T::from_param_str(&value, context).map_err(|e| e.into())?;
        Ok(Some(value))
    }
}

impl<T: Clone> Param<T> {
    pub fn is_some(&self) -> bool {
        !matches!(self, Self::None)
    }

    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum BuildPhase {
    Options,
    Environment,
    Project,
}

#[derive(Clone, Default)]
pub struct Explicit {
    pub dsn: Param<Url>,
    pub instance: Param<InstanceName>,
    pub credentials: Param<CredentialsFile>,

    pub host: Param<HostType>,
    pub port: Param<NonZeroU16>,

    pub unix_path: Param<PathBuf>,
    pub database: Param<String>,
    pub branch: Param<String>,
    pub user: Param<String>,
    pub password: Param<String>,
    pub client_security: Param<ClientSecurity>,
    pub tls_ca: Param<String>,
    pub tls_security: Param<TlsSecurity>,
    pub tls_server_name: Param<String>,
    pub secret_key: Param<String>,
    pub cloud_profile: Param<String>,
    pub wait_until_available: Param<Duration>,

    pub server_settings: HashMap<String, String>,
}

impl Debug for Explicit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("Explicit");
        if self.dsn.is_some() {
            s.field("dsn", &self.dsn);
        }
        if self.instance.is_some() {
            s.field("instance", &self.instance);
        }
        if self.credentials.is_some() {
            s.field("credentials", &self.credentials);
        }
        if self.host.is_some() {
            s.field("host", &self.host);
        }
        if self.port.is_some() {
            s.field("port", &self.port);
        }
        if self.unix_path.is_some() {
            s.field("unix_path", &self.unix_path);
        }
        if self.database.is_some() {
            s.field("database", &self.database);
        }
        if self.branch.is_some() {
            s.field("branch", &self.branch);
        }
        if self.user.is_some() {
            s.field("user", &self.user);
        }
        if self.password.is_some() {
            s.field("password", &self.password);
        }
        if self.client_security.is_some() {
            s.field("client_security", &self.client_security);
        }
        if self.tls_ca.is_some() {
            s.field("tls_ca", &self.tls_ca);
        }
        if self.tls_security.is_some() {
            s.field("tls_security", &self.tls_security);
        }
        if self.tls_server_name.is_some() {
            s.field("tls_server_name", &self.tls_server_name);
        }
        if self.secret_key.is_some() {
            s.field("secret_key", &self.secret_key);
        }
        if self.cloud_profile.is_some() {
            s.field("cloud_profile", &self.cloud_profile);
        }
        if self.wait_until_available.is_some() {
            s.field("wait_until_available", &self.wait_until_available);
        }
        if !self.server_settings.is_empty() {
            s.field("server_settings", &self.server_settings);
        }
        s.finish()
    }
}

impl Explicit {
    pub fn merge(&mut self, other: Self) {
        if self.dsn.is_none() {
            self.dsn = other.dsn;
        }
        if self.instance.is_none() {
            self.instance = other.instance;
        }
        if self.credentials.is_none() {
            self.credentials = other.credentials;
        }
        if self.host.is_none() {
            self.host = other.host;
        }
        if self.port.is_none() {
            self.port = other.port;
        }
        if self.unix_path.is_none() {
            self.unix_path = other.unix_path;
        }
        if self.database.is_none() && self.branch.is_none() {
            self.database = other.database;
            self.branch = other.branch;
        }
        if self.user.is_none() {
            self.user = other.user;
        }
        if self.password.is_none() {
            self.password = other.password;
        }
        if self.client_security.is_none() {
            self.client_security = other.client_security;
        }
        if self.tls_ca.is_none() {
            self.tls_ca = other.tls_ca;
        }
        if self.tls_security.is_none() {
            self.tls_security = other.tls_security;
        }
        if self.tls_server_name.is_none() {
            self.tls_server_name = other.tls_server_name;
        }
        if self.secret_key.is_none() {
            self.secret_key = other.secret_key;
        }
        if self.cloud_profile.is_none() {
            self.cloud_profile = other.cloud_profile;
        }
        if self.wait_until_available.is_none() {
            self.wait_until_available = other.wait_until_available;
        }
        for (key, value) in other.server_settings {
            self.server_settings.entry(key).or_insert(value);
        }
    }

    fn check_overlap(&self) -> Vec<CompoundSource> {
        let mut sources = Vec::new();
        if self.dsn.is_some() {
            sources.push(CompoundSource::Dsn);
        }
        if self.instance.is_some() {
            sources.push(CompoundSource::Instance);
        }
        if self.host.is_some() || self.port.is_some() {
            sources.push(CompoundSource::HostPort);
        }
        if self.credentials.is_some() {
            sources.push(CompoundSource::CredentialsFile);
        }
        sources
    }

    /// Try to build the config. Returns `None` if the config is incomplete.
    pub(crate) fn try_build(
        &self,
        context: &mut impl BuildContext,
        phase: BuildPhase,
    ) -> Result<Option<Config>, ParseError> {
        // Step 0: Check for compound option overlap. If there is, return an error.
        let compound_sources = self.check_overlap();
        if compound_sources.len() > 1 {
            if phase == BuildPhase::Options {
                return Err(ParseError::MultipleCompoundOpts(compound_sources));
            } else {
                return Err(ParseError::MultipleCompoundEnv(compound_sources));
            }
        }

        // Step 1: Resolve DSN, credentials file, instance if available
        let mut explicit = self.clone();

        context.trace(&format!("Start: {:?}", explicit));

        if let Some(dsn) = self.dsn.get(context)? {
            let dsn = parse_dsn(&dsn, context)?;
            context.trace(&format!("DSN: {:?}", dsn));
            explicit.merge(dsn);
        }
        if let Some(file) = self.credentials.get(context).map_err(|e| {
            // Special case: map FileNotFound to InvalidCredentialsFile
            if e == ParseError::FileNotFound {
                ParseError::InvalidCredentialsFile(InvalidCredentialsFileError::FileNotFound)
            } else {
                e
            }
        })? {
            let file = parse_credentials(&file, context)?;
            context.trace(&format!("Credentials: {:?}", file));
            explicit.merge(file);
        }
        if let Some(instance) = self.instance.get(context)? {
            match instance {
                InstanceName::Local(local) => {
                    let instance = parse_instance(&local, context)?;
                    context.trace(&format!("Instance: {:?}", instance));
                    explicit.merge(instance);
                }
                InstanceName::Cloud { .. } => {
                    let profile = explicit
                        .cloud_profile
                        .get(context)?
                        .unwrap_or("default".to_string());
                    let cloud = parse_cloud(&profile, context)?;
                    context.trace(&format!("Cloud: {:?}", cloud));
                    explicit.merge(cloud);

                    if let Some(secret_key) = explicit.secret_key.get(context)? {
                        match instance.cloud_address(&secret_key) {
                            Ok(Some(address)) => {
                                explicit.host = Param::Unparsed(address);
                            }
                            Ok(None) => {
                                unreachable!();
                            }
                            Err(e) => {
                                // Special case: we ignore the secret key error until the final phase
                                if phase == BuildPhase::Project {
                                    return Err(e);
                                }
                            }
                        }
                    } else {
                        return Err(ParseError::SecretKeyNotFound);
                    }
                }
            }
        }

        context.trace(&format!("Merged: {:?}", explicit));

        // Step 2: Resolve host. If we have no host yet, exit.
        let host = match (explicit.host.get(context)?, explicit.port.get(context)?) {
            (Some(host), Some(port)) => Host(host, port.into()),
            (Some(host), None) => Host(host, 5656),
            (None, Some(port)) => Host(HostType::Hostname("localhost".to_string()), port.into()),
            (None, None) => {
                return Ok(None);
            }
        };

        if host.is_unix() {
            return Err(ParseError::UnixSocketUnsupported);
        }

        let authentication = if let Some(password) = explicit.password.get(context)? {
            Authentication::Password(password)
        } else if let Some(secret_key) = explicit.secret_key.get(context)? {
            Authentication::SecretKey(secret_key)
        } else {
            Authentication::None
        };

        let user = explicit.user.get(context)?;
        let database = explicit.database.get(context)?;
        let branch = explicit.branch.get(context)?;

        for (param, error) in [
            (&user, ParseError::InvalidUser),
            (&database, ParseError::InvalidDatabase),
            (&branch, ParseError::InvalidDatabase),
        ] {
            if let Some(param) = param {
                if param.trim() != param || param.is_empty() {
                    return Err(error);
                }
            }
        }

        let db = match (database, branch) {
            (Some(db), Some(branch)) if db != branch => {
                return Err(ParseError::InvalidDatabase);
            }
            (Some(_), Some(branch)) => DatabaseBranch::Branch(branch),
            (Some(db), None) => DatabaseBranch::Database(db),
            (None, Some(branch)) => DatabaseBranch::Branch(branch),
            (None, None) => DatabaseBranch::Default,
        };

        let tls_ca = explicit.tls_ca.get(context)?;
        let client_security = explicit.client_security.get(context)?.unwrap_or_default();
        let tls_security = explicit.tls_security.get(context)?.unwrap_or_default();
        let tls_server_name = explicit.tls_server_name.get(context)?;
        let wait_until_available = explicit.wait_until_available.get(context)?;
        let server_settings = explicit.server_settings;

        // If we have a client security option, we need to check if it's compatible with the TLS security option.
        let tls_security = match (client_security, tls_security) {
            (ClientSecurity::Strict, TlsSecurity::Insecure | TlsSecurity::NoHostVerification) => {
                return Err(ParseError::InvalidTlsSecurity(
                    TlsSecurityError::IncompatibleSecurityOptions,
                ));
            }
            (ClientSecurity::Strict, _) => TlsSecurity::Strict,
            (ClientSecurity::InsecureDevMode, TlsSecurity::Default) => TlsSecurity::Insecure,
            (ClientSecurity::Default, TlsSecurity::Insecure) => TlsSecurity::Insecure,
            (_, TlsSecurity::Default) if tls_ca.is_none() => TlsSecurity::Strict,
            (_, TlsSecurity::Default) => TlsSecurity::NoHostVerification,
            (_, TlsSecurity::NoHostVerification) => TlsSecurity::NoHostVerification,
            (_, TlsSecurity::Strict) => TlsSecurity::Strict,
            (_, TlsSecurity::Insecure) => TlsSecurity::Insecure,
        };

        context.ok(Some(Config {
            host,
            db,
            user,
            authentication,
            client_security,
            tls_security,
            tls_ca,
            tls_server_name,
            wait_until_available: wait_until_available.unwrap_or(Duration::from_secs(30)),
            server_settings,
        }))
    }
}

pub fn parse_dsn(dsn: &Url, context: &mut impl BuildContext) -> Result<Explicit, ParseError> {
    let mut explicit = Explicit::default();

    context.trace(&format!("Parsing DSN: {:?}", dsn));

    if !(dsn.scheme() == "edgedb" || dsn.scheme() == "gel") {
        return Err(ParseError::InvalidDsn);
    }

    let mut set = HashSet::new();
    if let Some(host) = dsn.host() {
        set.insert("host".to_string());
        match host {
            url::Host::Domain(domain) => {
                explicit.host = Param::Unparsed(domain.to_string());
            }
            url::Host::Ipv4(address) => {
                explicit.host = Param::Parsed(HostType::IP(IpAddr::V4(address), None));
            }
            url::Host::Ipv6(address) => {
                explicit.host = Param::Parsed(HostType::IP(IpAddr::V6(address), None));
            }
        }
    } else {
        explicit.host = Param::Unparsed("localhost".to_string());
    }
    if let Some(port) = dsn.port() {
        if let Some(port) = NonZeroU16::new(port) {
            set.insert("port".to_string());
            explicit.port = Param::Parsed(port);
        } else {
            return Err(ParseError::InvalidPort);
        }
    } else {
        explicit.port = Param::Parsed(NonZeroU16::new(5656).unwrap());
    }

    let path = dsn.path().strip_prefix('/').unwrap_or(dsn.path());
    if !path.is_empty() {
        set.insert("branch".to_string());
        explicit.branch = Param::Unparsed(path.to_string());
    }

    if !dsn.username().is_empty() {
        set.insert("user".to_string());
        explicit.user = Param::Unparsed(dsn.username().to_string());
    }

    if let Some(password) = dsn.password() {
        if !password.is_empty() {
            set.insert("password".to_string());
            explicit.password = Param::Unparsed(password.to_string());
        }
    }

    explicit.server_settings = HashMap::new();

    for (key, value) in dsn.query_pairs() {
        // Weird case: database and branch are stripped of the leading '/'
        let value = if key == "database" || key == "branch" {
            value.strip_prefix('/').unwrap_or(value.as_ref()).into()
        } else {
            value
        };

        let (key, param) = if let Some(key) = key.strip_suffix("_file_env") {
            (key, Param::EnvFile(value.to_string()))
        } else if let Some(key) = key.strip_suffix("_env") {
            (key, Param::<String>::Env(value.to_string()))
        } else if let Some(key) = key.strip_suffix("_file") {
            (key, Param::File(PathBuf::from(value.to_string())))
        } else {
            (key.as_ref(), Param::Unparsed(value.to_string()))
        };
        if !set.insert(key.to_string()) {
            return Err(ParseError::InvalidDsn);
        }
        match key {
            "host" => explicit.host = param.cast().unwrap(),
            "user" => explicit.user = param,
            "password" => explicit.password = param,
            "secret_key" => explicit.secret_key = param,
            "tls_ca" => explicit.tls_ca = param,
            "tls_server_name" => explicit.tls_server_name = param,
            "database" => explicit.database = param,
            "branch" => explicit.branch = param,
            "port" => explicit.port = param.cast().unwrap(),
            "tls_security" => explicit.tls_security = param.cast().unwrap(),
            "wait_until_available" => explicit.wait_until_available = param.cast().unwrap(),
            key => {
                if explicit
                    .server_settings
                    .insert(key.to_string(), value.to_string())
                    .is_some()
                {
                    return Err(ParseError::InvalidDsn);
                }
            }
        }
    }

    if explicit.database.is_some() && explicit.branch.is_some() {
        return Err(ParseError::InvalidDsn);
    }

    context.ok(explicit)
}

pub fn parse_credentials(
    credentials: &CredentialsFile,
    context: &mut impl BuildContext,
) -> Result<Explicit, ParseError> {
    let mut explicit = Explicit::default();

    explicit.host = Param::from_unparsed(credentials.host.clone());
    explicit.port = Param::Parsed(credentials.port);
    explicit.user = Param::Unparsed(credentials.user.clone());
    explicit.password = Param::from_unparsed(credentials.password.clone());
    explicit.database = Param::from_unparsed(credentials.database.clone());
    explicit.branch = Param::from_unparsed(credentials.branch.clone());
    explicit.tls_ca = Param::from_unparsed(credentials.tls_ca.clone());
    explicit.tls_security = Param::Unparsed(credentials.tls_security.to_string());
    explicit.tls_server_name = Param::from_unparsed(credentials.tls_server_name.clone());

    context.ok(explicit)
}

pub fn parse_env(context: &mut impl BuildContext) -> Result<Explicit, ParseError> {
    let mut explicit = Explicit {
        dsn: Param::from_parsed(context.read_env(Env::dsn)?),
        instance: Param::from_parsed(context.read_env(Env::instance)?),
        credentials: Param::from_file(context.read_env(Env::credentials_file)?),
        host: Param::from_parsed(context.read_env(Env::host)?),
        port: Param::from_parsed(context.read_env(Env::port)?),
        database: Param::from_parsed(context.read_env(Env::database)?),
        branch: Param::from_parsed(context.read_env(Env::branch)?),
        user: Param::from_parsed(context.read_env(Env::user)?),
        password: Param::from_parsed(context.read_env(Env::password)?),
        tls_security: Param::from_parsed(context.read_env(Env::client_tls_security)?),
        tls_ca: Param::from_parsed(context.read_env(Env::tls_ca)?),
        tls_server_name: Param::from_parsed(context.read_env(Env::tls_server_name)?),
        client_security: Param::from_parsed(context.read_env(Env::client_security)?),
        secret_key: Param::from_parsed(context.read_env(Env::secret_key)?),
        cloud_profile: Param::from_parsed(context.read_env(Env::cloud_profile)?),
        wait_until_available: Param::from_parsed(context.read_env(Env::wait_until_available)?),
        ..Default::default()
    };

    if explicit.branch.is_some() && explicit.database.is_some() {
        return Err(ParseError::ExclusiveOptions);
    }

    let ca_file = Param::from_file(context.read_env(Env::tls_ca_file)?);
    if explicit.tls_ca.is_none() {
        explicit.tls_ca = ca_file;
    } else if ca_file.is_some() {
        return Err(ParseError::ExclusiveOptions);
    }

    context.ok(explicit)
}

pub fn parse_instance(
    local: &str,
    context: &mut impl BuildContext,
) -> Result<Explicit, ParseError> {
    let Some(credentials) = (match context.read_config_file(format!("credentials/{local}.json"))? {
        Some(credentials) => Some(credentials),
        None => {
            return Err(ParseError::CredentialsFileNotFound);
            None
        }
    }) else {
        return context.ok(Explicit::default());
    };
    parse_credentials(&credentials, context)
}

pub fn parse_cloud(profile: &str, context: &mut impl BuildContext) -> Result<Explicit, ParseError> {
    let mut explicit = Explicit::default();

    let Some(cloud_credentials): Option<CloudCredentialsFile> =
        context.read_config_file(format!("cloud-credentials/{profile}.json"))?
    else {
        return context.ok(Explicit::default());
    };
    explicit.secret_key = Param::Unparsed(cloud_credentials.secret_key);

    context.ok(explicit)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialsFile {
    pub host: Option<String>,
    pub port: NonZeroU16,
    pub user: String,
    pub password: Option<String>,
    pub database: Option<String>,
    pub branch: Option<String>,
    pub tls_ca: Option<String>,
    #[serde(default)]
    pub tls_security: TlsSecurity,
    pub tls_server_name: Option<String>,
}

impl FromStr for CredentialsFile {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(res) = serde_json::from_str::<CredentialsFile>(s) {
            // Special case: don't allow database and branch to be set at the same time
            if let (Some(database), Some(branch)) = (&res.database, &res.branch) {
                if database != branch {
                    return Err(ParseError::InvalidCredentialsFile(
                        InvalidCredentialsFileError::ConflictingSettings(
                            ("database".to_string(), database.clone()),
                            ("branch".to_string(), branch.clone()),
                        ),
                    ));
                }
            }

            return Ok(res);
        }

        let res = serde_json::from_str::<CredentialsFileCompat>(s).map_err(|e| {
            ParseError::InvalidCredentialsFile(InvalidCredentialsFileError::SerializationError(
                e.to_string(),
            ))
        })?;

        res.try_into()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CredentialsFileCompat {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    port: Option<NonZeroU16>,
    user: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    password: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    database: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tls_cert_data: Option<String>, // deprecated
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tls_ca: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tls_server_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tls_verify_hostname: Option<bool>, // deprecated
    tls_security: Option<TlsSecurity>,
}

impl TryInto<CredentialsFile> for CredentialsFileCompat {
    type Error = ParseError;

    fn try_into(self) -> Result<CredentialsFile, Self::Error> {
        let expected_verify = match self.tls_security {
            Some(TlsSecurity::Strict) => Some(true),
            Some(TlsSecurity::NoHostVerification) => Some(false),
            Some(TlsSecurity::Insecure) => Some(false),
            _ => None,
        };
        if self.tls_verify_hostname.is_some()
            && self.tls_security.is_some()
            && expected_verify
                .zip(self.tls_verify_hostname)
                .map(|(actual, expected)| actual != expected)
                .unwrap_or(false)
        {
            Err(ParseError::InvalidCredentialsFile(
                InvalidCredentialsFileError::ConflictingSettings(
                    (
                        "tls_security".to_string(),
                        self.tls_security.unwrap().to_string(),
                    ),
                    (
                        "tls_verify_hostname".to_string(),
                        self.tls_verify_hostname.unwrap().to_string(),
                    ),
                ),
            ))
        } else if self.tls_ca.is_some()
            && self.tls_cert_data.is_some()
            && self.tls_ca != self.tls_cert_data
        {
            return Err(ParseError::InvalidCredentialsFile(
                InvalidCredentialsFileError::ConflictingSettings(
                    ("tls_ca".to_string(), self.tls_ca.unwrap().to_string()),
                    (
                        "tls_cert_data".to_string(),
                        self.tls_cert_data.unwrap().to_string(),
                    ),
                ),
            ));
        } else {
            // Special case: don't allow database and branch to be set at the same time
            if self.database.is_some() && self.branch.is_some() && self.database != self.branch {
                return Err(ParseError::InvalidCredentialsFile(
                    InvalidCredentialsFileError::ConflictingSettings(
                        ("database".to_string(), self.database.unwrap().to_string()),
                        ("branch".to_string(), self.branch.unwrap().to_string()),
                    ),
                ));
            }

            Ok(CredentialsFile {
                host: self.host,
                port: self.port.unwrap_or(NonZeroU16::new(5656).unwrap()),
                user: self.user,
                password: self.password,
                database: self.database,
                branch: self.branch,
                tls_ca: self.tls_ca.or(self.tls_cert_data.clone()),
                tls_server_name: self.tls_server_name,
                tls_security: self.tls_security.unwrap_or(match self.tls_verify_hostname {
                    None => TlsSecurity::Default,
                    Some(true) => TlsSecurity::Strict,
                    Some(false) => TlsSecurity::NoHostVerification,
                }),
            })
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CloudCredentialsFile {
    pub secret_key: String,
}

impl FromStr for CloudCredentialsFile {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s).map_err(|e| {
            ParseError::InvalidCredentialsFile(InvalidCredentialsFileError::SerializationError(
                e.to_string(),
            ))
        })
    }
}

#[derive(Debug, Clone)]
pub struct Project {
    pub cloud_profile: Option<String>,
    pub instance_name: InstanceName,
    pub project_path: Option<PathBuf>,
    pub branch: Option<String>,
    pub database: Option<String>,
}

impl Project {
    pub fn load(path: &Path, context: &mut impl BuildContext) -> Result<Self, ParseError> {
        let cloud_profile = context
            .read_config_file::<String>(path.join("cloud-profile"))
            .unwrap_or_default();
        let instance_name = context
            .read_config_file::<InstanceName>(path.join("instance-name"))
            .unwrap_or_default();
        let project_path = context
            .read_config_file::<PathBuf>(path.join("project-path"))
            .unwrap_or_default();
        let branch = context
            .read_config_file::<String>(path.join("branch"))
            .unwrap_or_default();
        let database = context
            .read_config_file::<String>(path.join("database"))
            .unwrap_or_default();
        let Some(instance_name) = instance_name else {
            return Err(ParseError::ProjectNotInitialised);
        };
        Ok(Self {
            cloud_profile,
            instance_name,
            project_path,
            branch,
            database,
        })
    }
}
