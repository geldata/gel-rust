//! Parses DSNs for PostgreSQL database connections.
//!
//! There are some small differences with how `libpq` works:
//!
//!  - Unrecognized options are supported and collected in a map.
//!  - `database` is recognized as an alias for `dbname`
//!  - `[host1,host2]` is considered valid for psql
use gel_stream::SslVersionParseError;

mod host;
mod params;
mod passfile;
mod raw_params;
mod url;

pub use host::{Host, HostType, ToAddrsSyncVec};
pub use params::{ConnectionParameters, Ssl, SslParameters};
pub use passfile::{Password, PasswordWarning};
pub use raw_params::{RawConnectionParameters, SslMode};
pub use url::{parse_postgres_dsn, parse_postgres_dsn_env};

#[derive(Debug, PartialEq, Eq, derive_more::Display, derive_more::From, derive_more::Error)]
#[allow(clippy::enum_variant_names)]
// #[error(not(source))] because derive_more infers a source for a single-field error
pub enum ParseError {
    #[display(
        "Invalid DSN: scheme is expected to be either \"postgresql\" or \"postgres\", got {_0}"
    )]
    InvalidScheme(#[error(not(source))] String),

    #[display("Invalid value for parameter \"{_0}\": \"{_1}\"")]
    InvalidParameter(String, String),

    #[display("Invalid percent encoding")]
    InvalidPercentEncoding,

    #[display("Invalid port: \"{_0}\"")]
    InvalidPort(#[error(not(source))] String),

    #[display("Unexpected number of ports, must be either a single port or the same number as the host count: \"{_0}\"")]
    InvalidPortCount(#[error(not(source))] String),

    #[display("Invalid hostname: \"{_0}\"")]
    InvalidHostname(#[error(not(source))] String),

    #[display("Invalid query parameter: \"{_0}\"")]
    InvalidQueryParameter(#[error(not(source))] String),

    #[display("Invalid TLS version: \"{_0}\"")]
    #[from]
    InvalidTLSVersion(SslVersionParseError),

    #[display("Could not determine the connection {_0}")]
    MissingRequiredParameter(#[error(not(source))] String),

    #[display("URL parse error: {_0}")]
    #[from]
    UrlParseError(::url::ParseError),
}
