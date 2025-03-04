mod builder;
mod env;
mod instance_name;
mod params;

use std::{
    collections::HashMap,
    fmt,
    num::NonZeroU16,
    path::{Path, PathBuf},
    str::FromStr,
};

pub use builder::{parse, Authentication, Config, DatabaseBranch, ParseError};
use builder::{CompoundSource, TlsSecurityError};
pub use instance_name::InstanceName;
use params::{CloudCredentialsFile, CredentialsFile};
pub use params::{Explicit, Param};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    env::SystemEnvVars, file::SystemFileAccess, host::HostType, EnvVar, FileAccess, Warnings,
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
                    Ok(FromStr::from_str(s)?)
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

pub struct BuildContextImpl<E: EnvVar = SystemEnvVars, F: FileAccess = SystemFileAccess> {
    env: E,
    files: F,
    pub config_dir: Option<PathBuf>,
    pub(crate) warnings: Warnings,
    errors: Vec<ParseError>,
    pub trace: Option<Vec<String>>,
}

impl BuildContextImpl<SystemEnvVars, SystemFileAccess> {
    /// Create a new build context with default values.
    pub fn new() -> Self {
        Self {
            env: SystemEnvVars,
            files: SystemFileAccess,
            config_dir: None,
            warnings: Warnings::default(),
            errors: Vec::new(),
            trace: None,
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
            errors: Vec::new(),
            trace: None,
        }
    }
}

pub trait BuildContext {
    type EnvVar: EnvVar;
    fn env(&self) -> &impl EnvVar;
    fn files(&self) -> &impl FileAccess;
    fn warnings(&mut self) -> &mut Warnings;
    fn warn(&mut self, message: String);
    fn ok<T>(&self, value: T) -> Result<T, ParseError>;
    fn error(&mut self, error: ParseError) -> Result<(), ParseError>;
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

    fn warnings(&mut self) -> &mut Warnings {
        &mut self.warnings
    }

    fn warn(&mut self, message: String) {
        self.warnings.warnings.push(message);
    }

    fn ok<T>(&self, value: T) -> Result<T, ParseError> {
        Ok(value)
    }

    fn error(&mut self, error: ParseError) -> Result<(), ParseError> {
        // Short-circuiting errors
        if error.is_fatal() {
            return Err(error);
        }
        self.errors.push(error);
        Ok(())
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
        if let Some(trace) = &mut self.trace {
            trace.push(message.to_string());
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

/// Client security mode.
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
#[serde(deny_unknown_fields)]
pub struct ConnectionOptions {
    #[serde(default)]
    pub dsn: Option<String>,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub instance: Option<String>,
    #[serde(default)]
    pub database: Option<String>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(deserialize_with = "deserialize_string_or_number")]
    #[serde(default)]
    pub port: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    #[serde(rename = "tlsSecurity")]
    pub tls_security: Option<String>,
    #[serde(default)]
    #[serde(rename = "tlsCA")]
    pub tls_ca: Option<String>,
    #[serde(default)]
    #[serde(rename = "tlsCAFile")]
    pub tls_ca_file: Option<String>,
    #[serde(default)]
    #[serde(rename = "tlsServerName")]
    pub tls_server_name: Option<String>,
    #[serde(default)]
    #[serde(rename = "waitUntilAvailable")]
    pub wait_until_available: Option<String>,
    #[serde(default)]
    #[serde(rename = "serverSettings")]
    pub server_settings: Option<HashMap<String, String>>,
    #[serde(default)]
    #[serde(rename = "credentialsFile")]
    pub credentials_file: Option<String>,
    #[serde(default)]
    #[serde(rename = "credentials")]
    pub credentials: Option<String>,
    #[serde(default)]
    #[serde(rename = "secretKey")]
    pub secret_key: Option<String>,
}

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

impl TryInto<Explicit> for ConnectionOptions {
    type Error = ParseError;

    fn try_into(self) -> Result<Explicit, Self::Error> {
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

        let explicit = Explicit {
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
            let result = <Url as FromParamStr>::from_param_str(&dsn, &mut BuildContextImpl::new());
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
