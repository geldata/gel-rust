use std::fmt::Debug;
use std::num::NonZeroU16;
use std::path::PathBuf;
use std::time::Duration;

use url::Url;

use super::{BuildContext, ClientSecurity, CloudCerts, InstanceName, ParseError, TlsSecurity};
use crate::env::define_env;
use crate::host::HostType;

define_env!(
    type Error = ParseError;

    /// The host to connect to.
    #[env(GEL_HOST, EDGEDB_HOST)]
    host: HostType,

    /// The port to connect to.
    #[env(GEL_PORT, EDGEDB_PORT)]
    #[preprocess=ignore_docker_tcp_port]
    port: NonZeroU16,

    /// The database name to connect to.
    #[env(GEL_DATABASE, EDGEDB_DATABASE)]
    database: String,

    /// The branch name to connect to.
    #[env(GEL_BRANCH, EDGEDB_BRANCH)]
    branch: String,

    /// The username to connect as.
    #[env(GEL_USER, EDGEDB_USER)]
    user: String,

    /// The password to use for authentication.
    #[env(GEL_PASSWORD, EDGEDB_PASSWORD)]
    password: String,

    /// TLS server name to verify.
    #[env(GEL_TLS_SERVER_NAME, EDGEDB_TLS_SERVER_NAME)]
    tls_server_name: String,

    /// Path to credentials file.
    #[env(GEL_CREDENTIALS_FILE, EDGEDB_CREDENTIALS_FILE)]
    credentials_file: PathBuf,

    /// Instance name to connect to.
    #[env(GEL_INSTANCE, EDGEDB_INSTANCE)]
    instance: InstanceName,

    /// Connection DSN string.
    #[env(GEL_DSN, EDGEDB_DSN)]
    dsn: Url,

    /// Secret key for authentication.
    #[env(GEL_SECRET_KEY, EDGEDB_SECRET_KEY)]
    secret_key: String,

    /// Client security mode.
    #[env(GEL_CLIENT_SECURITY, EDGEDB_CLIENT_SECURITY)]
    client_security: ClientSecurity,

    /// TLS security mode.
    #[env(GEL_CLIENT_TLS_SECURITY, EDGEDB_CLIENT_TLS_SECURITY)]
    client_tls_security: TlsSecurity,

    /// Path to TLS CA certificate file.
    #[env(GEL_TLS_CA, EDGEDB_TLS_CA)]
    tls_ca: String,

    /// Path to TLS CA certificate file.
    #[env(GEL_TLS_CA_FILE, EDGEDB_TLS_CA_FILE)]
    tls_ca_file: PathBuf,

    /// Cloud profile name.
    #[env(GEL_CLOUD_PROFILE, EDGEDB_CLOUD_PROFILE)]
    cloud_profile: String,

    /// Cloud certificates mode.
    #[env(_GEL_CLOUD_CERTS, _EDGEDB_CLOUD_CERTS)]
    _cloud_certs: CloudCerts,

    /// How long to wait for server to become available.
    #[env(GEL_WAIT_UNTIL_AVAILABLE, EDGEDB_WAIT_UNTIL_AVAILABLE)]
    wait_until_available: Duration,
);

fn ignore_docker_tcp_port(
    s: &str,
    context: &mut impl BuildContext,
) -> Result<Option<String>, ParseError> {
    if s.starts_with("tcp://") {
        context.warn("GEL_PORT/EDGEDB_PORT is ignored when using Docker TCP port".to_string());
        Ok(None)
    } else {
        Ok(Some(s.to_string()))
    }
}
