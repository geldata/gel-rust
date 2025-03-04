//! Parses DSNs for Gel database connections.

pub(crate) mod builder;
mod env;
pub mod error;
mod instance_name;
mod params;

use std::{
    collections::HashMap,
    fmt,
    num::NonZeroU16,
    path::{Path, PathBuf},
    str::FromStr,
};

pub use builder::{Authentication, Config, DatabaseBranch};
pub use instance_name::InstanceName;
use params::{CloudCredentialsFile, CredentialsFile};
pub use params::{Param, Params};
use serde::{Deserialize, Serialize};
use url::Url;

use error::*;

/// Parse a set of parameters into a configuration.
///
/// No project directory is provided, so only the explicit parameters,
/// environment and filesystem will be used.
pub fn parse(
    params: impl TryInto<Params, Error = ParseError>,
) -> (Result<Config, ParseError>, Warnings) {
    let params = match params.try_into() {
        Ok(params) => params,
        Err(e) => return (Err(e), Warnings::default()),
    };
    let mut context = BuildContextImpl::new();
    let res = builder::parse(params, &mut context, None);
    (res, context.warnings)
}

/// Parse a set of parameters and a project directory into a configuration.
///
/// A project directory is provided which will only be used if the explicit
/// parameters, environment and filesystem are insufficient.
pub fn parse_project(
    params: impl TryInto<Params, Error = ParseError>,
    project_dir: impl AsRef<Path>,
    config_dir: Option<&Path>,
) -> (Result<Config, ParseError>, Warnings) {
    let params = match params.try_into() {
        Ok(params) => params,
        Err(e) => return (Err(e), Warnings::default()),
    };
    let mut context = BuildContextImpl::new();
    context.config_dir = config_dir.map(|p| p.to_path_buf());
    let res = builder::parse(params, &mut context, Some(project_dir.as_ref()));
    (res, context.warnings)
}

/// Parse a set of parameters, an environment/filesystem implements and a
/// project directory into a configuration.
///
/// A project directory is provided which will only be used if the explicit
/// parameters, environment and filesystem are insufficient.
pub fn parse_from(
    params: impl TryInto<Params, Error = ParseError>,
    project_dir: Option<&Path>,
    config_dir: Option<&Path>,
    env: impl EnvVar,
    files: impl FileAccess,
) -> (Result<Config, ParseError>, Warnings, Traces) {
    let params = match params.try_into() {
        Ok(params) => params,
        Err(e) => return (Err(e), Warnings::default(), Traces::default()),
    };
    let mut context = BuildContextImpl::new_with(env, files);
    context.traces = Some(Traces::default());
    context.config_dir = config_dir.map(|p| p.to_path_buf());
    let res = builder::parse(params, &mut context, project_dir);
    (res, context.warnings, context.traces.unwrap())
}

use crate::{
    env::SystemEnvVars, file::SystemFileAccess, host::HostType, EnvVar, FileAccess, Traces,
    Warnings,
};

pub(crate) trait FromParamStr: Sized {
    type Err;
    fn from_param_str(s: &str, context: &mut impl BuildContext) -> Result<Self, Self::Err>;
}

macro_rules! impl_from_param_str {
    ($($t:ty),*) => {
        $(
            impl FromParamStr for $t {
                type Err = <$t as FromStr>::Err;
                fn from_param_str(s: &str, _context: &mut impl BuildContext) -> Result<Self, Self::Err> {
                    FromStr::from_str(s)
                }
            }
        )*
    };
}

impl_from_param_str!(
    InstanceName,
    HostType,
    NonZeroU16,
    PathBuf,
    String,
    CredentialsFile,
    TlsSecurity,
    ClientSecurity,
    CloudCredentialsFile,
    CloudCerts
);

impl FromParamStr for std::time::Duration {
    type Err = ParseError;
    fn from_param_str(s: &str, _context: &mut impl BuildContext) -> Result<Self, Self::Err> {
        gel_protocol::model::Duration::from_str(s)
            .map_err(|_| ParseError::EnvNotFound)
            .map(|d| std::time::Duration::from_micros(d.to_micros() as u64))
    }
}

impl FromParamStr for Url {
    type Err = ParseError;
    fn from_param_str(s: &str, context: &mut impl BuildContext) -> Result<Self, Self::Err> {
        // Ensure the URL contains `://`
        if !s.starts_with("edgedb://") && !s.starts_with("gel://") {
            return Err(ParseError::InvalidDsn);
        }

        let res = Url::parse(s);
        match res {
            Ok(url) => Ok(url),
            Err(e) => {
                // Because the url crate refuses to add scope identifiers, we need to
                // strip them for now.
                if e == url::ParseError::InvalidIpv6Address && s.contains("%25") {
                    // Try to re-parse "s" without the scope identifier. It's possible that
                    // the URL has a username/password and we're trying to parse out
                    // scheme://username:password@[<ipv6>%25<scope>] and replace it with
                    // scheme://username:password@[<ipv6>].

                    let original_url = s;

                    // First, trim off the scheme.
                    let Some(scheme_end) = s.find("://") else {
                        return Err(ParseError::InvalidDsn);
                    };
                    let s = &s[scheme_end + 3..];

                    // Next, find the end of the authority.
                    let authority_end = if let Some(authority_end) = s.find('/') {
                        authority_end
                    } else {
                        s.len()
                    };

                    let s = &s[..authority_end];

                    let Some(scope_start) = s.rfind("%25") else {
                        return Err(ParseError::InvalidDsn);
                    };
                    let Some(addr_end) = s.rfind(']') else {
                        return Err(ParseError::InvalidDsn);
                    };

                    // Now we can do the math to remove the scope chunk of original_url. We
                    // start from the %25 and go until the ].
                    let scope_len = addr_end - scope_start;
                    let scope_start = scheme_end + 3 + scope_start;

                    let new_url = original_url[..scope_start].to_string()
                        + &original_url[scope_start + scope_len..];

                    context.trace(&format!(
                        "Ignored scope identifier in IPv6 URL: {}, use an explicit host parameter instead",
                        &original_url[scope_start..scope_start + scope_len]
                    ));

                    // YOLO parse the new URL.
                    Url::parse(&new_url).map_err(|_| ParseError::InvalidDsn)
                } else {
                    Err(ParseError::InvalidDsn)
                }
            }
        }
    }
}

struct BuildContextImpl<E: EnvVar = SystemEnvVars, F: FileAccess = SystemFileAccess> {
    env: E,
    files: F,
    pub config_dir: Option<PathBuf>,
    pub(crate) warnings: Warnings,
    pub(crate) traces: Option<Traces>,
}

impl Default for BuildContextImpl<SystemEnvVars, SystemFileAccess> {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildContextImpl<SystemEnvVars, SystemFileAccess> {
    /// Create a new build context with default values.
    pub fn new() -> Self {
        Self {
            env: SystemEnvVars,
            files: SystemFileAccess,
            config_dir: None,
            warnings: Warnings::default(),
            traces: None,
        }
    }
}

impl<E: EnvVar, F: FileAccess> BuildContextImpl<E, F> {
    /// Create a new build context with default values.
    pub fn new_with(env: E, files: F) -> Self {
        Self {
            env,
            files,
            config_dir: None,
            warnings: Warnings::default(),
            traces: None,
        }
    }
}

pub(crate) trait BuildContext {
    type EnvVar: EnvVar;
    fn env(&self) -> &impl EnvVar;
    fn files(&self) -> &impl FileAccess;
    fn warn(&mut self, message: String);
    fn ok<T>(&self, value: T) -> Result<T, ParseError>;
    fn read_config_file<T: FromParamStr>(
        &mut self,
        path: impl AsRef<Path>,
    ) -> Result<Option<T>, T::Err>;
    fn read_env<'a, 'b, 'c, T: FromParamStr>(
        &'c mut self,
        env: impl Fn(&'b mut Self) -> Result<Option<T>, ParseError>,
    ) -> Result<Option<T>, ParseError>
    where
        Self::EnvVar: 'a,
        'c: 'a,
        'c: 'b;
    fn trace(&mut self, message: &str);
}

impl<E: EnvVar, F: FileAccess> BuildContext for BuildContextImpl<E, F> {
    type EnvVar = E;
    fn env(&self) -> &impl EnvVar {
        &self.env
    }

    fn files(&self) -> &impl FileAccess {
        &self.files
    }

    fn warn(&mut self, message: String) {
        self.warnings.warnings.push(message);
    }

    fn ok<T>(&self, value: T) -> Result<T, ParseError> {
        Ok(value)
    }

    fn read_config_file<T: FromParamStr>(
        &mut self,
        path: impl AsRef<Path>,
    ) -> Result<Option<T>, T::Err> {
        let Some(config_dir) = &self.config_dir else {
            return Ok(None);
        };
        let path = config_dir.join(path.as_ref());
        self.trace(&format!("Reading config file: {}", path.display()));
        if let Ok(file) = self.files.read(&path) {
            // TODO?
            let res = T::from_param_str(&file, self);
            self.trace(&format!(
                "File content: {:?}",
                res.as_ref().map(|_| ()).map_err(|_| ())
            ));
            match res {
                Ok(value) => Ok(Some(value)),
                Err(e) => Err(e),
            }
        } else {
            Ok(None)
        }
    }

    fn read_env<'a, 'b, 'c, T: FromParamStr>(
        &'c mut self,
        env: impl Fn(&'b mut Self) -> Result<Option<T>, ParseError>,
    ) -> Result<Option<T>, ParseError>
    where
        Self::EnvVar: 'a,
        'c: 'a,
        'c: 'b,
    {
        let res = env(self);
        match res {
            Ok(Some(value)) => Ok(Some(value)),
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn trace(&mut self, message: &str) {
        if let Some(traces) = &mut self.traces {
            traces.traces.push(message.to_string());
        }
    }
}

/// Client security mode.
#[derive(Default, Debug, Clone, Copy, Eq, PartialEq)]
pub enum ClientSecurity {
    /// Disable security checks
    InsecureDevMode,
    /// Always verify domain an certificate
    Strict,
    /// Verify domain only if no specific certificate is configured
    #[default]
    Default,
}

impl FromStr for ClientSecurity {
    type Err = ParseError;
    fn from_str(s: &str) -> Result<ClientSecurity, Self::Err> {
        use ClientSecurity::*;

        match s {
            "default" => Ok(Default),
            "strict" => Ok(Strict),
            "insecure_dev_mode" => Ok(InsecureDevMode),
            // TODO: this should have its own error?
            mode => Err(ParseError::InvalidTlsSecurity(
                TlsSecurityError::InvalidValue,
            )),
        }
    }
}

/// The type of cloud certificate to use.
#[derive(Debug, Clone, Copy)]
pub enum CloudCerts {
    Staging,
    Local,
}

impl FromStr for CloudCerts {
    type Err = ParseError;
    fn from_str(s: &str) -> Result<CloudCerts, Self::Err> {
        use CloudCerts::*;

        match s {
            "staging" => Ok(Staging),
            "local" => Ok(Local),
            // TODO: incorrect error
            option => Err(ParseError::FileNotFound),
        }
    }
}

/// TLS Client Security Mode
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum TlsSecurity {
    /// Allow any certificate for TLS connection
    Insecure,
    /// Verify certificate against trusted chain but allow any host name
    ///
    /// This is useful for localhost (you can't make trusted chain certificate
    /// for localhost). And when certificate of specific server is stored in
    /// credentials file so it's secure regardless of which host name was used
    /// to expose the server to the network.
    NoHostVerification,
    /// Normal TLS certificate check (checks trusted chain and hostname)
    Strict,
    /// If there is a specific certificate in credentials, do not check
    /// the host name, otherwise use `Strict` mode
    #[default]
    Default,
}

impl FromStr for TlsSecurity {
    type Err = ParseError;
    fn from_str(val: &str) -> Result<Self, Self::Err> {
        match val {
            "default" => Ok(TlsSecurity::Default),
            "insecure" => Ok(TlsSecurity::Insecure),
            "no_host_verification" => Ok(TlsSecurity::NoHostVerification),
            "strict" => Ok(TlsSecurity::Strict),
            val => Err(ParseError::InvalidTlsSecurity(
                TlsSecurityError::InvalidValue,
            )),
        }
    }
}

impl fmt::Display for TlsSecurity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Insecure => write!(f, "insecure"),
            Self::NoHostVerification => write!(f, "no_host_verification"),
            Self::Strict => write!(f, "strict"),
            Self::Default => write!(f, "default"),
        }
    }
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[serde(default)]
#[serde(deny_unknown_fields)]
pub struct ConnectionOptions {
    pub dsn: Option<String>,
    pub user: Option<String>,
    pub password: Option<String>,
    pub instance: Option<String>,
    pub database: Option<String>,
    pub host: Option<String>,
    #[serde(deserialize_with = "deserialize_string_or_number")]
    pub port: Option<String>,
    pub branch: Option<String>,
    #[serde(rename = "tlsSecurity")]
    pub tls_security: Option<String>,
    #[serde(rename = "tlsCA")]
    pub tls_ca: Option<String>,
    #[serde(rename = "tlsCAFile")]
    pub tls_ca_file: Option<String>,
    #[serde(rename = "tlsServerName")]
    pub tls_server_name: Option<String>,
    #[serde(rename = "waitUntilAvailable")]
    pub wait_until_available: Option<String>,
    #[serde(rename = "serverSettings")]
    pub server_settings: Option<HashMap<String, String>>,
    #[serde(rename = "credentialsFile")]
    pub credentials_file: Option<String>,
    pub credentials: Option<String>,
    #[serde(rename = "secretKey")]
    pub secret_key: Option<String>,
}

#[cfg(feature = "serde")]
fn deserialize_string_or_number<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = serde_json::Value::deserialize(deserializer)?;
    if let Some(s) = s.as_str() {
        Ok(Some(s.to_string()))
    } else {
        Ok(Some(s.to_string()))
    }
}

impl TryInto<Params> for ConnectionOptions {
    type Error = ParseError;

    fn try_into(self) -> Result<Params, Self::Error> {
        if self.credentials.is_some() && self.credentials_file.is_some() {
            return Err(ParseError::MultipleCompoundOpts(vec![
                CompoundSource::CredentialsFile,
                CompoundSource::CredentialsFile,
            ]));
        }

        if self.tls_ca.is_some() && self.tls_ca_file.is_some() {
            return Err(ParseError::ExclusiveOptions);
        }

        if self.branch.is_some() && self.database.is_some() {
            return Err(ParseError::ExclusiveOptions);
        }

        let mut credentials = Param::from_file(self.credentials_file.clone());
        if credentials.is_none() {
            credentials = Param::from_unparsed(self.credentials.clone());
        }

        let mut tls_ca = Param::from_unparsed(self.tls_ca.clone());
        if tls_ca.is_none() {
            tls_ca = Param::from_file(self.tls_ca_file.clone());
        }

        let explicit = Params {
            dsn: Param::from_unparsed(self.dsn.clone()),
            credentials,
            user: Param::from_unparsed(self.user.clone()),
            password: Param::from_unparsed(self.password.clone()),
            instance: Param::from_unparsed(self.instance.clone()),
            database: Param::from_unparsed(self.database.clone()),
            host: Param::from_unparsed(self.host.clone()),
            port: Param::from_unparsed(self.port.as_ref().map(|n| n.to_string())),
            branch: Param::from_unparsed(self.branch.clone()),
            secret_key: Param::from_unparsed(self.secret_key.clone()),
            tls_security: Param::from_unparsed(self.tls_security.clone()),
            tls_ca,
            tls_server_name: Param::from_unparsed(self.tls_server_name.clone()),
            server_settings: self.server_settings.unwrap_or_default(),
            wait_until_available: Param::from_unparsed(self.wait_until_available.clone()),
            ..Default::default()
        };

        Ok(explicit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dsn_with_scope() {
        for dsn in [
            "edgedb://[::1%25lo0]:5656",
            "edgedb://[::1%25lo0]:5656/",
            "edgedb://username%25@password%25:[::1%25lo0]:5656/db",
            "edgedb://username%25@password%25:[::1%25lo0]:5656/db/",
            "edgedb://user3@[fe80::1ff:fe23:4567:890a%25lo0]:3000/ab",
        ] {
            let result = <Url as FromParamStr>::from_param_str(dsn, &mut BuildContextImpl::new());
            let dsn2 = dsn.replace("%25lo0", "");
            let result2 =
                <Url as FromParamStr>::from_param_str(&dsn2, &mut BuildContextImpl::new());
            eprintln!("{dsn} = {result:?}, {dsn2} = {result2:?}");
            assert_eq!(
                result, result2,
                "Expected {} to parse the same as {}",
                dsn, dsn2
            );
        }
    }
}
