#![doc = include_str!("../README.md")]
// We don't want to warn about unused code when 1) either client or server is not
// enabled, or 2) no crypto backend is enabled.
#![cfg_attr(
    not(all(
        all(feature = "client", feature = "server"),
        any(feature = "rustls", feature = "openssl")
    )),
    allow(unused)
)]

#[cfg(feature = "client")]
mod client;
#[cfg(feature = "server")]
mod server;

#[cfg(feature = "client")]
pub use client::Connector;

#[cfg(feature = "server")]
pub use server::Acceptor;

mod common;
#[cfg(feature = "openssl")]
pub use common::openssl::OpensslDriver;
#[cfg(feature = "rustls")]
pub use common::rustls::RustlsDriver;
pub use common::{resolver::*, stream::*, target::*, tls::*, BaseStream};
pub use rustls_pki_types as pki_types;

pub type RawStream = UpgradableStream<BaseStream>;

/// The default TCP backlog for the server.
pub const DEFAULT_TCP_BACKLOG: u32 = 1024;
/// The default TLS backlog for the server.
pub const DEFAULT_TLS_BACKLOG: u32 = 128;
/// The default preview buffer size for the server.
pub const DEFAULT_PREVIEW_BUFFER_SIZE: u32 = 8;

#[derive(Debug, derive_more::Error, derive_more::Display, derive_more::From)]
pub enum ConnectionError {
    /// I/O error encountered during connection operations.
    #[display("I/O error: {_0}")]
    Io(#[from] std::io::Error),

    /// UTF-8 decoding error.
    #[display("UTF8 error: {_0}")]
    Utf8Error(#[from] std::str::Utf8Error),

    /// SSL-related error.
    #[display("SSL error: {_0}")]
    SslError(#[from] SslError),
}

impl From<ConnectionError> for std::io::Error {
    fn from(err: ConnectionError) -> Self {
        match err {
            ConnectionError::Io(e) => e,
            ConnectionError::Utf8Error(e) => std::io::Error::other(e),
            ConnectionError::SslError(e) => e.into(),
        }
    }
}

#[derive(Debug, derive_more::Error, derive_more::Display, derive_more::From)]
pub enum SslError {
    #[display("SSL is not supported by this transport")]
    SslUnsupported,
    #[display("SSL is already upgraded or is in the process of upgrading")]
    SslAlreadyUpgraded,

    #[cfg(feature = "openssl")]
    #[display("OpenSSL error: {_0}")]
    OpenSslError(#[from] ::openssl::ssl::Error),
    #[cfg(feature = "openssl")]
    #[display("OpenSSL error: {_0}")]
    OpenSslErrorStack(#[from] ::openssl::error::ErrorStack),
    #[cfg(feature = "openssl")]
    #[display("OpenSSL certificate verification error: {_0}")]
    OpenSslErrorVerify(#[from] ::openssl::x509::X509VerifyResult),

    #[cfg(feature = "rustls")]
    #[display("Rustls error: {_0}")]
    RustlsError(#[from] ::rustls::Error),

    #[cfg(feature = "rustls")]
    #[display("Webpki error: {_0}")]
    WebpkiError(
        #[from]
        #[error(not(source))]
        ::webpki::Error,
    ),

    #[cfg(feature = "rustls")]
    #[display("Verifier builder error: {_0}")]
    VerifierBuilderError(#[from] ::rustls::server::VerifierBuilderError),

    #[display("Invalid DNS name: {_0}")]
    InvalidDnsNameError(#[from] ::rustls_pki_types::InvalidDnsNameError),

    #[display("SSL I/O error: {_0}")]
    SslIoError(#[from] std::io::Error),
}

impl From<SslError> for std::io::Error {
    fn from(err: SslError) -> Self {
        match err {
            SslError::SslIoError(e) => e,
            other => std::io::Error::other(other),
        }
    }
}

impl SslError {
    /// Returns a common error for any time of crypto backend.
    pub fn common_error(&self) -> Option<CommonError> {
        match self {
            #[cfg(feature = "rustls")]
            SslError::RustlsError(::rustls::Error::InvalidCertificate(cert_err)) => {
                match cert_err {
                    ::rustls::CertificateError::NotValidForName
                    | ::rustls::CertificateError::NotValidForNameContext { .. } => {
                        Some(CommonError::InvalidCertificateForName)
                    }
                    ::rustls::CertificateError::Revoked => Some(CommonError::CertificateRevoked),
                    ::rustls::CertificateError::Expired => Some(CommonError::CertificateExpired),
                    ::rustls::CertificateError::UnknownIssuer => Some(CommonError::InvalidIssuer),
                    _ => None,
                }
            }
            #[cfg(feature = "rustls")]
            SslError::RustlsError(::rustls::Error::InvalidMessage(_)) => {
                Some(CommonError::InvalidTlsProtocolData)
            }
            #[cfg(feature = "openssl")]
            SslError::OpenSslErrorVerify(e) => match e.as_raw() {
                openssl_sys::X509_V_ERR_HOSTNAME_MISMATCH => {
                    Some(CommonError::InvalidCertificateForName)
                }
                openssl_sys::X509_V_ERR_IP_ADDRESS_MISMATCH => {
                    Some(CommonError::InvalidCertificateForName)
                }
                openssl_sys::X509_V_ERR_CERT_REVOKED => Some(CommonError::CertificateRevoked),
                openssl_sys::X509_V_ERR_CERT_HAS_EXPIRED => Some(CommonError::CertificateExpired),
                openssl_sys::X509_V_ERR_UNABLE_TO_GET_ISSUER_CERT
                | openssl_sys::X509_V_ERR_UNABLE_TO_GET_ISSUER_CERT_LOCALLY => {
                    Some(CommonError::InvalidIssuer)
                }
                _ => None,
            },
            #[cfg(feature = "openssl")]
            SslError::OpenSslErrorStack(e) => match e.errors().first().map(|err| err.code()) {
                // SSL_R_WRONG_VERSION_NUMBER
                Some(0xa00010b) => Some(CommonError::InvalidTlsProtocolData),
                // SSL_R_PACKET_LENGTH_TOO_LONG
                Some(0xa0000c6) => Some(CommonError::InvalidTlsProtocolData),
                _ => None,
            },
            #[cfg(feature = "openssl")]
            SslError::OpenSslError(e) => match e.code().as_raw() {
                // TODO: We should probably wrap up handshake errors differently.
                openssl_sys::SSL_ERROR_SSL => {
                    match e
                        .ssl_error()
                        .and_then(|e| e.errors().first())
                        .map(|err| err.code())
                    {
                        // SSL_R_WRONG_VERSION_NUMBER
                        Some(0xa00010b) => Some(CommonError::InvalidTlsProtocolData),
                        // SSL_R_PACKET_LENGTH_TOO_LONG
                        Some(0xa0000c6) => Some(CommonError::InvalidTlsProtocolData),
                        _ => None,
                    }
                }
                _ => None,
            },
            _ => None,
        }
    }
}

#[derive(
    Debug,
    derive_more::Error,
    derive_more::Display,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Clone,
    Copy,
    Hash,
)]
pub enum CommonError {
    #[display("The certificate's subject name(s) do not match the name of the host")]
    InvalidCertificateForName,
    #[display("The certificate has been revoked")]
    CertificateRevoked,
    #[display("The certificate has expired")]
    CertificateExpired,
    #[display("The certificate was issued by an untrusted authority")]
    InvalidIssuer,
    #[display("TLS protocol error")]
    InvalidTlsProtocolData,
}

#[cfg(feature = "__test_keys")]
pub mod test_keys {
    macro_rules! include_files {
        ($($name:ident : $type:path => $path:literal),*) => {
            /// Raw PEM text from the test files.
            pub mod raw {
                $(
                    #[doc = concat!("Test key: ", $path)]
                    pub static $name: &str = include_str!(concat!("../tests/", $path));
                )*
            }

            #[cfg(feature = "pem")]
            pub mod binary {
                use std::sync::LazyLock;
                $(
                    #[doc = concat!("Test key: ", $path)]
                    pub static $name: LazyLock<$type> = LazyLock::new(
                        || rustls_pki_types::pem::PemObject::from_pem_slice($crate::test_keys::raw::$name.as_bytes()).unwrap()
                    );
                )*
            }
        }
    }

    include_files!(
        SERVER_KEY: rustls_pki_types::PrivateKeyDer => "certs/server.key.pem",
        SERVER_CERT: rustls_pki_types::CertificateDer => "certs/server.cert.pem",
        SERVER_ALT_KEY: rustls_pki_types::PrivateKeyDer => "certs/server-alt.key.pem",
        SERVER_ALT_CERT: rustls_pki_types::CertificateDer => "certs/server-alt.cert.pem",
        CLIENT_KEY_PROTECTED: rustls_pki_types::PrivateKeyDer => "certs/client.key.protected.pem",
        CLIENT_KEY: rustls_pki_types::PrivateKeyDer => "certs/client.key.pem",
        CLIENT_CERT: rustls_pki_types::CertificateDer => "certs/client.cert.pem",
        CLIENT_CA_KEY: rustls_pki_types::PrivateKeyDer => "certs/client_ca.key.pem",
        CLIENT_CA_CERT: rustls_pki_types::CertificateDer => "certs/client_ca.cert.pem",
        CA_KEY: rustls_pki_types::PrivateKeyDer => "certs/ca.key.pem",
        CA_CRL: rustls_pki_types::CertificateRevocationListDer => "certs/ca.crl.pem",
        CA_CERT: rustls_pki_types::CertificateDer => "certs/ca.cert.pem"
    );

    use std::sync::LazyLock;

    #[cfg(feature = "pem")]
    pub static SERVER_KEY: LazyLock<crate::TlsKey> = LazyLock::new(|| {
        crate::TlsKey::new(binary::SERVER_KEY.clone_key(), binary::SERVER_CERT.clone())
    });

    #[cfg(feature = "pem")]
    pub static SERVER_ALT_KEY: LazyLock<crate::TlsKey> = LazyLock::new(|| {
        crate::TlsKey::new(
            binary::SERVER_ALT_KEY.clone_key(),
            binary::SERVER_ALT_CERT.clone(),
        )
    });

    #[cfg(feature = "pem")]
    pub static CLIENT_KEY: LazyLock<crate::TlsKey> = LazyLock::new(|| {
        crate::TlsKey::new(binary::CLIENT_KEY.clone_key(), binary::CLIENT_CERT.clone())
    });
}
