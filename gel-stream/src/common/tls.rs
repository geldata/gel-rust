use crate::{SslError, Stream};
use rustls_pki_types::{CertificateDer, CertificateRevocationListDer, PrivateKeyDer, ServerName};
use std::{borrow::Cow, future::Future, sync::Arc};

use super::BaseStream;

// Note that we choose rustls when both openssl and rustls are enabled.

/// The default TLS driver.
#[cfg(all(feature = "openssl", not(feature = "rustls")))]
pub type Ssl = crate::common::openssl::OpensslDriver;
#[cfg(feature = "rustls")]
pub type Ssl = crate::common::rustls::RustlsDriver;
#[cfg(not(any(feature = "openssl", feature = "rustls")))]
pub type Ssl = NullTlsDriver;

/// A trait for TLS drivers.
#[doc(hidden)]
pub trait TlsDriver: Default + Send + Sync + Unpin + 'static {
    type Stream: Stream + Send;
    type ClientParams: Unpin + Send;
    type ServerParams: Unpin + Send;
    const DRIVER_NAME: &'static str;

    #[allow(unused)]
    fn init_client(
        params: &TlsParameters,
        name: Option<ServerName>,
    ) -> Result<Self::ClientParams, SslError>;
    #[allow(unused)]
    fn init_server(params: &TlsServerParameters) -> Result<Self::ServerParams, SslError>;

    fn upgrade_client<S: Stream>(
        params: Self::ClientParams,
        stream: S,
    ) -> impl Future<Output = Result<(Self::Stream, TlsHandshake), SslError>> + Send;
    fn upgrade_server<S: Stream>(
        params: TlsServerParameterProvider,
        stream: S,
    ) -> impl Future<Output = Result<(Self::Stream, TlsHandshake), SslError>> + Send;
    fn unclean_shutdown(this: Self::Stream) -> Result<(), Self::Stream>;

    fn is<D: TlsDriver>() -> bool {
        D::DRIVER_NAME == Self::DRIVER_NAME
    }
}

/// A TLS driver that fails when TLS is requested.
#[derive(Default)]
pub struct NullTlsDriver;

#[allow(unused)]
impl TlsDriver for NullTlsDriver {
    type Stream = BaseStream;
    type ClientParams = ();
    type ServerParams = ();
    const DRIVER_NAME: &'static str = "null";

    fn init_client(
        params: &TlsParameters,
        name: Option<ServerName>,
    ) -> Result<Self::ClientParams, SslError> {
        Err(SslError::SslUnsupported)
    }

    fn init_server(params: &TlsServerParameters) -> Result<Self::ServerParams, SslError> {
        Err(SslError::SslUnsupported)
    }

    fn upgrade_client<S: Stream>(
        params: Self::ClientParams,
        stream: S,
    ) -> impl Future<Output = Result<(Self::Stream, TlsHandshake), SslError>> + Send {
        async { Err(SslError::SslUnsupported) }
    }

    fn upgrade_server<S: Stream>(
        params: TlsServerParameterProvider,
        stream: S,
    ) -> impl Future<Output = Result<(Self::Stream, TlsHandshake), SslError>> + Send {
        async { Err(SslError::SslUnsupported) }
    }

    fn unclean_shutdown(_this: Self::Stream) -> Result<(), Self::Stream> {
        // Do nothing
        Ok(())
    }
}

/// Verification modes for TLS that are a superset of both PostgreSQL and EdgeDB/Gel.
///
/// Postgres offers six levels: `disable`, `allow`, `prefer`, `require`, `verify-ca` and `verify-full`.
///
/// EdgeDB/Gel offers three levels: `insecure`, `no_host_verification' and 'strict'.
///
/// This table maps the various levels:
///
/// | Postgres | EdgeDB/Gel | `TlsServerCertVerify` enum |
/// | -------- | ----------- | ----------------- |
/// | require  | insecure    | `Insecure`        |
/// | verify-ca | no_host_verification | `IgnoreHostname`        |
/// | verify-full | strict | `VerifyFull`      |
///
/// Note that both EdgeDB/Gel and Postgres may alter certificate validation levels
/// when custom root certificates are provided. This must be done in the
/// `TlsParameters` struct by the caller.
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq)]
pub enum TlsServerCertVerify {
    /// Do not verify the server's certificate. Only confirm that the server is
    /// using TLS.
    Insecure,
    /// Verify the server's certificate using the CA (ignore hostname).
    IgnoreHostname,
    /// Verify the server's certificate using the CA and hostname.
    #[default]
    VerifyFull,
}

#[derive(Clone, derive_more::Debug, Default, PartialEq, Eq)]
pub enum TlsCert {
    /// Use the system's default certificate.
    #[default]
    System,
    /// Use the system's default certificate and a set of custom root
    /// certificates.
    #[debug("SystemPlus([{} cert(s)])", _0.len())]
    SystemPlus(Vec<CertificateDer<'static>>),
    /// Use the webpki-roots default certificate.
    Webpki,
    /// Use the webpki-roots default certificate and a set of custom root
    /// certificates.
    #[debug("WebpkiPlus([{} cert(s)])", _0.len())]
    WebpkiPlus(Vec<CertificateDer<'static>>),
    /// Use a custom root certificate only.
    #[debug("Custom([{} cert(s)])", _0.len())]
    Custom(Vec<CertificateDer<'static>>),
}

#[derive(Default, derive_more::Debug, PartialEq, Eq)]
pub struct TlsParameters {
    pub server_cert_verify: TlsServerCertVerify,
    #[debug("{}", cert.as_ref().map(|_| "Some(...)").unwrap_or("None"))]
    pub cert: Option<CertificateDer<'static>>,
    #[debug("{}", key.as_ref().map(|_| "Some(...)").unwrap_or("None"))]
    pub key: Option<PrivateKeyDer<'static>>,
    pub root_cert: TlsCert,
    #[debug("{}", if crl.is_empty() { "[]".to_string() } else { format!("[{} item(s)]", crl.len()) })]
    pub crl: Vec<CertificateRevocationListDer<'static>>,
    pub min_protocol_version: Option<SslVersion>,
    pub max_protocol_version: Option<SslVersion>,
    pub enable_keylog: bool,
    pub sni_override: Option<Cow<'static, str>>,
    pub alpn: TlsAlpn,
}

impl TlsParameters {
    pub fn insecure() -> Self {
        Self {
            server_cert_verify: TlsServerCertVerify::Insecure,
            ..Default::default()
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SslVersion {
    Tls1,
    Tls1_1,
    Tls1_2,
    Tls1_3,
}

impl std::fmt::Display for SslVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            SslVersion::Tls1 => "TLSv1",
            SslVersion::Tls1_1 => "TLSv1.1",
            SslVersion::Tls1_2 => "TLSv1.2",
            SslVersion::Tls1_3 => "TLSv1.3",
        };
        f.write_str(s)
    }
}

#[derive(Debug, Clone, derive_more::Error, derive_more::Display, Eq, PartialEq)]
pub struct SslVersionParseError(#[error(not(source))] pub String);

#[cfg(feature = "serde")]
impl serde::Serialize for SslVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(match self {
            SslVersion::Tls1 => "TLSv1",
            SslVersion::Tls1_1 => "TLSv1.1",
            SslVersion::Tls1_2 => "TLSv1.2",
            SslVersion::Tls1_3 => "TLSv1.3",
        })
    }
}

impl TryFrom<Cow<'_, str>> for SslVersion {
    type Error = SslVersionParseError;
    fn try_from(value: Cow<str>) -> Result<SslVersion, Self::Error> {
        Ok(match value.to_lowercase().as_ref() {
            "tls_1" | "tlsv1" => SslVersion::Tls1,
            "tls_1.1" | "tlsv1.1" => SslVersion::Tls1_1,
            "tls_1.2" | "tlsv1.2" => SslVersion::Tls1_2,
            "tls_1.3" | "tlsv1.3" => SslVersion::Tls1_3,
            _ => return Err(SslVersionParseError(value.to_string())),
        })
    }
}

#[derive(Default, Debug, PartialEq, Eq)]
pub enum TlsClientCertVerify {
    /// Do not verify the client's certificate, just ignore it.
    #[default]
    Ignore,
    /// If a client certificate is provided, validate it.
    Optional(Vec<CertificateDer<'static>>),
    /// Validate that a client certificate exists and is valid. This configuration
    /// may not be ideal, because it does not fail the client-side handshake.
    Validate(Vec<CertificateDer<'static>>),
}

#[derive(derive_more::Debug, derive_more::Constructor)]
pub struct TlsKey {
    #[debug("key(...)")]
    pub(crate) key: PrivateKeyDer<'static>,
    #[debug("cert(...)")]
    pub(crate) cert: CertificateDer<'static>,
}

impl TlsKey {
    /// Create a new `TlsKey` from a PEM-encoded certificate and key.
    #[cfg(feature = "pem")]
    pub fn new_pem(mut key: &[u8], mut cert: &[u8]) -> Result<Self, std::io::Error> {
        let cert = rustls_pemfile::certs(&mut cert)
            .next()
            .ok_or(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "No certificate found",
            ))??;
        let key = rustls_pemfile::private_key(&mut key)?.ok_or(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "No key found",
        ))?;
        Ok(Self { cert, key })
    }

    /// Create a new `TlsKey` from the test certificate and key.
    #[cfg(test)]
    pub fn test_key() -> Self {
        fn load_test_cert() -> rustls_pki_types::CertificateDer<'static> {
            rustls_pemfile::certs(&mut include_str!("../../tests/certs/server.cert.pem").as_bytes())
                .next()
                .expect("no cert")
                .expect("cert is bad")
        }

        fn load_test_key() -> rustls_pki_types::PrivateKeyDer<'static> {
            rustls_pemfile::private_key(
                &mut include_str!("../../tests/certs/server.key.pem").as_bytes(),
            )
            .expect("no server key")
            .expect("server key is bad")
        }

        Self {
            key: load_test_key(),
            cert: load_test_cert(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TlsServerParameterProvider {
    inner: TlsServerParameterProviderInner,
}

impl TlsServerParameterProvider {
    pub fn new(params: TlsServerParameters) -> Self {
        Self {
            inner: TlsServerParameterProviderInner::Static(Arc::new(params)),
        }
    }

    pub fn with_lookup(
        lookup: impl Fn(Option<ServerName>) -> Arc<TlsServerParameters> + Send + Sync + 'static,
    ) -> Self {
        Self {
            inner: TlsServerParameterProviderInner::Lookup(Arc::new(lookup)),
        }
    }

    pub fn lookup(&self, name: Option<ServerName>) -> Arc<TlsServerParameters> {
        match &self.inner {
            TlsServerParameterProviderInner::Static(params) => params.clone(),
            TlsServerParameterProviderInner::Lookup(lookup) => lookup(name),
        }
    }
}

#[derive(derive_more::Debug, Clone)]
enum TlsServerParameterProviderInner {
    Static(Arc<TlsServerParameters>),
    #[debug("Lookup(...)")]
    #[allow(clippy::type_complexity)]
    Lookup(Arc<dyn Fn(Option<ServerName>) -> Arc<TlsServerParameters> + Send + Sync + 'static>),
}

#[derive(Debug)]
pub struct TlsServerParameters {
    pub client_cert_verify: TlsClientCertVerify,
    pub min_protocol_version: Option<SslVersion>,
    pub max_protocol_version: Option<SslVersion>,
    pub server_certificate: TlsKey,
    pub alpn: TlsAlpn,
}

impl TlsServerParameters {
    pub fn new_with_certificate(server_certificate: TlsKey) -> Self {
        Self {
            client_cert_verify: TlsClientCertVerify::default(),
            min_protocol_version: None,
            max_protocol_version: None,
            server_certificate,
            alpn: TlsAlpn::default(),
        }
    }
}

#[derive(Default, Eq, PartialEq)]
pub struct TlsAlpn {
    /// The split form (ie: ["AB", "ABCD"])
    alpn_parts: Cow<'static, [Cow<'static, [u8]>]>,
}

impl std::fmt::Debug for TlsAlpn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.alpn_parts.is_empty() {
            write!(f, "[]")
        } else {
            for (i, part) in self.alpn_parts.iter().enumerate() {
                if i == 0 {
                    write!(f, "[")?;
                } else {
                    write!(f, ", ")?;
                }
                // Print as binary literal with appropriate escaping
                let mut s = String::new();
                s.push_str("b\"");
                for &b in part.iter() {
                    for c in b.escape_ascii() {
                        s.push(c as char);
                    }
                }
                s.push('"');
                write!(f, "{}", s)?;
            }
            write!(f, "]")?;
            Ok(())
        }
    }
}

impl TlsAlpn {
    pub fn new(alpn: &'static [&'static [u8]]) -> Self {
        let alpn = alpn.iter().map(|s| Cow::Borrowed(*s)).collect::<Vec<_>>();
        Self {
            alpn_parts: Cow::Owned(alpn),
        }
    }

    pub fn new_str(alpn: &'static [&'static str]) -> Self {
        let alpn = alpn
            .iter()
            .map(|s| Cow::Borrowed(s.as_bytes()))
            .collect::<Vec<_>>();
        Self {
            alpn_parts: Cow::Owned(alpn),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.alpn_parts.is_empty()
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.alpn_parts.len() * 2);
        for part in self.alpn_parts.iter() {
            bytes.push(part.len() as u8);
            bytes.extend_from_slice(part.as_ref());
        }
        bytes
    }

    pub fn as_vec_vec(&self) -> Vec<Vec<u8>> {
        let mut vec = Vec::with_capacity(self.alpn_parts.len());
        for part in self.alpn_parts.iter() {
            vec.push(part.to_vec());
        }
        vec
    }
}

/// Negotiated TLS handshake information.
#[derive(Debug, Clone, Default)]
pub struct TlsHandshake {
    /// The negotiated ALPN protocol.
    pub alpn: Option<Cow<'static, [u8]>>,
    /// The SNI hostname if provided.
    pub sni: Option<Cow<'static, str>>,
    /// The client certificate, if any.
    pub cert: Option<CertificateDer<'static>>,
    /// The negotiated TLS version.
    pub version: Option<SslVersion>,
}

#[cfg(test)]
mod tests {
    use rustls_pki_types::PrivatePkcs1KeyDer;

    use super::*;

    #[test]
    fn test_tls_parameters_debug() {
        let params = TlsParameters::default();
        assert_eq!(
            format!("{:?}", params),
            "TlsParameters { server_cert_verify: VerifyFull, cert: None, key: None, \
            root_cert: System, crl: [], min_protocol_version: None, max_protocol_version: None, \
            enable_keylog: false, sni_override: None, alpn: [] }"
        );
        let params = TlsParameters {
            server_cert_verify: TlsServerCertVerify::Insecure,
            cert: Some(CertificateDer::from_slice(&[1, 2, 3])),
            key: Some(PrivateKeyDer::Pkcs1(PrivatePkcs1KeyDer::from(vec![
                1, 2, 3,
            ]))),
            root_cert: TlsCert::SystemPlus(vec![CertificateDer::from_slice(&[1, 2, 3])]),
            crl: vec![CertificateRevocationListDer::from(vec![1, 2, 3])],
            min_protocol_version: None,
            max_protocol_version: None,
            enable_keylog: false,
            sni_override: None,
            alpn: TlsAlpn::new_str(&["h2", "http/1.1"]),
        };
        assert_eq!(
            format!("{:?}", params),
            "TlsParameters { server_cert_verify: Insecure, cert: Some(...), key: Some(...), \
            root_cert: SystemPlus([1 cert(s)]), crl: [1 item(s)], min_protocol_version: None, \
            max_protocol_version: None, enable_keylog: false, sni_override: None, \
            alpn: [b\"h2\", b\"http/1.1\"] }"
        );
    }

    #[test]
    fn test_tls_alpn() {
        let alpn = TlsAlpn::new_str(&["h2", "http/1.1"]);
        assert_eq!(
            alpn.as_bytes(),
            vec![2, b'h', b'2', 8, b'h', b't', b't', b'p', b'/', b'1', b'.', b'1']
        );
        assert_eq!(
            alpn.as_vec_vec(),
            vec![b"h2".to_vec(), b"http/1.1".to_vec()]
        );
        assert!(!alpn.is_empty());
        assert_eq!(format!("{:?}", alpn), "[b\"h2\", b\"http/1.1\"]");

        let empty_alpn = TlsAlpn::default();
        assert!(empty_alpn.is_empty());
        assert_eq!(empty_alpn.as_bytes(), Vec::<u8>::new());
        assert_eq!(empty_alpn.as_vec_vec(), Vec::<Vec<u8>>::new());
        assert_eq!(format!("{:?}", empty_alpn), "[]");
    }
}
