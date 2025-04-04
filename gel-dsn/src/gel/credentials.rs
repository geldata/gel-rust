use std::{num::NonZeroU16, str::FromStr};

use serde::{Deserialize, Serialize};

use super::{
    error::*, Param, Params, TlsSecurity, DEFAULT_BRANCH_NAME, DEFAULT_DATABASE_NAME, DEFAULT_HOST,
    DEFAULT_PORT,
};

/// An opaque type representing a credentials file.
///
/// Use [`std::str::FromStr`] to parse a credentials file from a string.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CredentialsFile {
    pub user: Option<String>,
    pub host: Option<String>,
    pub port: Option<NonZeroU16>,
    pub password: Option<String>,
    pub secret_key: Option<String>,
    pub database: Option<String>,
    pub branch: Option<String>,
    pub tls_ca: Option<String>,
    #[serde(default)]
    pub tls_security: TlsSecurity,
    pub tls_server_name: Option<String>,

    #[serde(skip)]
    pub(crate) warnings: Vec<Warning>,
}

impl From<&CredentialsFile> for Params {
    fn from(credentials: &CredentialsFile) -> Self {
        let host = if let Some(host) = credentials.host.clone() {
            Param::Unparsed(host)
        } else {
            Param::Parsed(DEFAULT_HOST.clone())
        };
        let port = if let Some(port) = credentials.port {
            Param::Parsed(port.into())
        } else {
            Param::Parsed(DEFAULT_PORT)
        };

        Params {
            host,
            port,
            user: Param::from_unparsed(credentials.user.clone()),
            password: Param::from_unparsed(credentials.password.clone()),
            secret_key: Param::from_unparsed(credentials.secret_key.clone()),
            database: Param::from_unparsed(credentials.database.clone()),
            branch: Param::from_unparsed(credentials.branch.clone()),
            tls_ca: Param::from_unparsed(credentials.tls_ca.clone()),
            tls_security: Param::Parsed(credentials.tls_security),
            tls_server_name: Param::from_unparsed(credentials.tls_server_name.clone()),
            ..Default::default()
        }
    }
}

impl From<CredentialsFile> for Params {
    fn from(credentials: CredentialsFile) -> Self {
        Self::from(&credentials)
    }
}

impl CredentialsFile {
    pub fn warnings(&self) -> &[Warning] {
        &self.warnings
    }
}

impl FromStr for CredentialsFile {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(mut res) = serde_json::from_str::<CredentialsFile>(s) {
            // Special case: treat database=__default__ and branch=edgedb as not set
            if (Some(DEFAULT_DATABASE_NAME), Some(DEFAULT_BRANCH_NAME))
                == (res.database.as_deref(), res.branch.as_deref())
            {
                res.database = None;
                res.branch = None;
            }

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    user: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    password: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    secret_key: Option<String>,
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

impl CredentialsFileCompat {
    fn validate(&self) -> Vec<Warning> {
        let mut warnings = Vec::new();
        if self.database.as_deref() == Some(DEFAULT_DATABASE_NAME)
            && self.branch.as_deref() == Some(DEFAULT_BRANCH_NAME)
        {
            warnings.push(Warning::DefaultDatabaseAndBranch);
        }
        if self.tls_verify_hostname.is_some() {
            warnings.push(Warning::DeprecatedCredentialProperty(
                "tls_verify_hostname".to_string(),
            ));
        }
        if self.tls_cert_data.is_some() {
            warnings.push(Warning::DeprecatedCredentialProperty(
                "tls_cert_data".to_string(),
            ));
        }
        warnings
    }
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
            let warnings = self.validate();

            let mut database = self.database;
            let mut branch = self.branch;

            // Special case: treat database=__default__ and branch=edgedb as not set
            if (Some(DEFAULT_DATABASE_NAME), Some(DEFAULT_BRANCH_NAME))
                == (database.as_deref(), branch.as_deref())
            {
                database = None;
                branch = None;
            }

            // Special case: don't allow database and branch to be set at the same time
            if database.is_some() && branch.is_some() && database != branch {
                return Err(ParseError::InvalidCredentialsFile(
                    InvalidCredentialsFileError::ConflictingSettings(
                        ("database".to_string(), database.unwrap().to_string()),
                        ("branch".to_string(), branch.unwrap().to_string()),
                    ),
                ));
            }

            Ok(CredentialsFile {
                host: self.host,
                port: self.port,
                user: self.user,
                password: self.password,
                secret_key: self.secret_key,
                database,
                branch,
                tls_ca: self.tls_ca.or(self.tls_cert_data.clone()),
                tls_server_name: self.tls_server_name,
                tls_security: self.tls_security.unwrap_or(match self.tls_verify_hostname {
                    None => TlsSecurity::Default,
                    Some(true) => TlsSecurity::Strict,
                    Some(false) => TlsSecurity::NoHostVerification,
                }),
                warnings,
            })
        }
    }
}

/// An opaque type representing a cloud credentials file.
///
/// Use [`std::str::FromStr`] to parse a cloud credentials file from a string.
#[derive(Debug, Clone, Deserialize)]
pub struct CloudCredentialsFile {
    pub(crate) secret_key: String,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credentials_file() {
        let credentials = CredentialsFile::from_str("{\"branch\": \"edgedb\"}").unwrap();
        assert_eq!(credentials.branch, Some("edgedb".to_string()));
        assert_eq!(credentials.database, None);
    }

    #[test]
    fn test_credentials_file_default_database_and_branch() {
        let credentials =
            CredentialsFile::from_str("{\"database\": \"edgedb\", \"branch\": \"__default__\"}")
                .unwrap();
        assert_eq!(credentials.database, None);
        assert_eq!(credentials.branch, None);
    }
}
