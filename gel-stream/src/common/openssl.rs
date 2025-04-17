use openssl::{
    ssl::{
        AlpnError, ClientHelloResponse, NameType, SniError, Ssl, SslAcceptor, SslContextBuilder,
        SslMethod, SslRef, SslVerifyMode,
    },
    x509::{verify::X509VerifyFlags, X509VerifyResult},
};
use rustls_pki_types::{CertificateDer, ServerName};
use std::{
    borrow::Cow,
    io::IoSlice,
    pin::Pin,
    sync::{Arc, Mutex, MutexGuard, OnceLock},
    task::{ready, Poll},
};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::TcpStream,
};

use crate::{
    RewindStream, SslError, SslVersion, Stream, TlsCert, TlsClientCertVerify, TlsDriver,
    TlsHandshake, TlsParameters, TlsServerCertVerify, TlsServerParameterProvider,
    TlsServerParameters,
};

use super::tokio_stream::TokioStream;

#[derive(Debug, Clone, Default)]
struct HandshakeData {
    server_alpn: Option<Vec<u8>>,
    handshake: TlsHandshake,
}

impl HandshakeData {
    fn from_ssl(ssl: &SslRef) -> Option<MutexGuard<Self>> {
        let mutex = ssl.ex_data(get_ssl_ex_data_index())?;
        mutex.lock().ok()
    }
}

static SSL_EX_DATA_INDEX: OnceLock<openssl::ex_data::Index<Ssl, Arc<Mutex<HandshakeData>>>> =
    OnceLock::new();

fn get_ssl_ex_data_index() -> openssl::ex_data::Index<Ssl, Arc<Mutex<HandshakeData>>> {
    *SSL_EX_DATA_INDEX
        .get_or_init(|| Ssl::new_ex_index().expect("Failed to create SSL ex_data index"))
}

#[derive(Default)]

pub struct OpensslDriver;

pub struct TlsStream(tokio_openssl::SslStream<TcpStream>);

impl Stream for TlsStream {
    fn with_socket2(
        &self,
        f: &mut dyn for<'a> FnMut(socket2::SockRef<'a>) -> Result<(), std::io::Error>,
    ) -> Result<(), std::io::Error> {
        f(socket2::SockRef::from(&self.0.get_ref()))
    }
}

impl AsyncRead for TlsStream {
    #[inline(always)]
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}

impl AsyncWrite for TlsStream {
    #[inline(always)]
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    #[inline(always)]
    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> std::task::Poll<std::io::Result<usize>> {
        Pin::new(&mut self.0).poll_write_vectored(cx, bufs)
    }

    #[inline(always)]
    fn is_write_vectored(&self) -> bool {
        self.0.is_write_vectored()
    }

    #[inline(always)]
    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    #[inline(always)]
    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let res = ready!(Pin::new(&mut self.0).poll_shutdown(cx));
        if let Err(e) = &res {
            // Swallow NotConnected errors here
            if e.kind() == std::io::ErrorKind::NotConnected {
                return Poll::Ready(Ok(()));
            }

            // Treat OpenSSL syscall errors during shutdown as graceful
            if let Some(ssl_err) = e
                .get_ref()
                .and_then(|e| e.downcast_ref::<openssl::ssl::Error>())
            {
                if ssl_err.code() == openssl::ssl::ErrorCode::SYSCALL {
                    return Poll::Ready(Ok(()));
                }
            }
        }
        Poll::Ready(res)
    }
}

/// Cache for the WebPKI roots
static WEBPKI_ROOTS: OnceLock<Vec<openssl::x509::X509>> = OnceLock::new();

impl TlsDriver for OpensslDriver {
    type Stream = TlsStream;
    type ClientParams = openssl::ssl::Ssl;
    type ServerParams = openssl::ssl::SslContext;

    fn init_client(
        params: &TlsParameters,
        name: Option<ServerName>,
    ) -> Result<Self::ClientParams, SslError> {
        let TlsParameters {
            server_cert_verify,
            root_cert,
            cert,
            key,
            crl,
            min_protocol_version,
            max_protocol_version,
            alpn,
            sni_override,
            enable_keylog,
        } = params;

        // let mut ssl = SslConnector::builder(SslMethod::tls_client())?;
        let mut ssl = SslContextBuilder::new(SslMethod::tls_client())?;

        // Load additional root certs
        match root_cert {
            TlsCert::Custom(root) | TlsCert::SystemPlus(root) | TlsCert::WebpkiPlus(root) => {
                for root in root {
                    let root = openssl::x509::X509::from_der(root.as_ref())?;
                    ssl.cert_store_mut().add_cert(root)?;
                }
            }
            _ => {}
        }

        match root_cert {
            TlsCert::Webpki | TlsCert::WebpkiPlus(_) => {
                let webpki_roots = WEBPKI_ROOTS.get_or_init(|| {
                    let webpki_roots = webpki_root_certs::TLS_SERVER_ROOT_CERTS;
                    let mut roots = Vec::new();
                    for root in webpki_roots {
                        // Don't expect the roots to fail to load
                        if let Ok(root) = openssl::x509::X509::from_der(root.as_ref()) {
                            roots.push(root);
                        }
                    }
                    roots
                });
                for root in webpki_roots {
                    ssl.cert_store_mut().add_cert(root.clone())?;
                }
            }
            _ => {}
        }

        // Load CA certificates from system for System/SystemPlus
        if matches!(root_cert, TlsCert::SystemPlus(_) | TlsCert::System) {
            // DANGER! Don't use the environment variable setter functions!
            let probe = openssl_probe::probe();
            ssl.load_verify_locations(probe.cert_file.as_deref(), probe.cert_dir.as_deref())?;
        }

        // Configure hostname verification
        match server_cert_verify {
            TlsServerCertVerify::Insecure => {
                ssl.set_verify(SslVerifyMode::NONE);
            }
            TlsServerCertVerify::IgnoreHostname => {
                ssl.set_verify(SslVerifyMode::PEER);
            }
            TlsServerCertVerify::VerifyFull => {
                ssl.set_verify(SslVerifyMode::PEER);
                if let Some(hostname) = sni_override {
                    ssl.verify_param_mut().set_host(hostname)?;
                } else if let Some(ServerName::DnsName(hostname)) = &name {
                    ssl.verify_param_mut().set_host(hostname.as_ref())?;
                } else if let Some(ServerName::IpAddress(ip)) = &name {
                    ssl.verify_param_mut().set_ip((*ip).into())?;
                }
            }
        }

        // Load CRL
        if !crl.is_empty() {
            // The openssl crate doesn't yet have add_crl, so we need to use the raw FFI
            use foreign_types::ForeignTypeRef;
            let ptr = ssl.cert_store_mut().as_ptr();

            extern "C" {
                pub fn X509_STORE_add_crl(
                    store: *mut openssl_sys::X509_STORE,
                    x: *mut openssl_sys::X509_CRL,
                ) -> openssl_sys::c_int;
            }

            for crl in crl {
                let crl = openssl::x509::X509Crl::from_der(crl.as_ref())?;
                let crl_ptr = crl.as_ptr();
                let res = unsafe { X509_STORE_add_crl(ptr, crl_ptr) };
                if res != 1 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "Failed to add CRL to store",
                    )
                    .into());
                }
            }

            ssl.verify_param_mut()
                .set_flags(X509VerifyFlags::CRL_CHECK | X509VerifyFlags::CRL_CHECK_ALL)?;
            ssl.cert_store_mut()
                .set_flags(X509VerifyFlags::CRL_CHECK | X509VerifyFlags::CRL_CHECK_ALL)?;
        }

        // Load certificate chain and private key
        if let (Some(cert), Some(key)) = (cert.as_ref(), key.as_ref()) {
            let builder = openssl::x509::X509::from_der(cert.as_ref())?;
            ssl.set_certificate(&builder)?;
            let builder = openssl::pkey::PKey::private_key_from_der(key.secret_der())?;
            ssl.set_private_key(&builder)?;
        }

        ssl.set_min_proto_version(min_protocol_version.map(|s| s.into()))?;
        ssl.set_max_proto_version(max_protocol_version.map(|s| s.into()))?;

        // Configure key log filename
        if *enable_keylog {
            if let Ok(path) = std::env::var("SSLKEYLOGFILE") {
                ssl.set_keylog_callback(move |_ssl, msg| {
                    let Ok(mut file) = std::fs::OpenOptions::new().append(true).open(&path) else {
                        return;
                    };
                    let _ = std::io::Write::write_all(&mut file, msg.as_bytes());
                });
            }
        }

        let mut ssl = openssl::ssl::Ssl::new(&ssl.build())?;
        ssl.set_connect_state();

        // Set hostname if it's not an IP address
        if let Some(hostname) = sni_override {
            ssl.set_hostname(hostname)?;
        } else if let Some(ServerName::DnsName(hostname)) = &name {
            ssl.set_hostname(hostname.as_ref())?;
        }

        if !alpn.is_empty() {
            ssl.set_alpn_protos(&alpn.as_bytes())?;
        }

        Ok(ssl)
    }

    fn init_server(params: &TlsServerParameters) -> Result<Self::ServerParams, SslError> {
        let TlsServerParameters {
            client_cert_verify,
            min_protocol_version,
            max_protocol_version,
            server_certificate,
            // Handled elsewhere
            alpn: _alpn,
        } = params;

        let mut ssl = SslAcceptor::mozilla_intermediate_v5(SslMethod::tls_server())?;
        let cert = openssl::x509::X509::from_der(server_certificate.cert.as_ref())?;
        let key = openssl::pkey::PKey::private_key_from_der(server_certificate.key.secret_der())?;
        ssl.set_certificate(&cert)?;
        ssl.set_private_key(&key)?;
        ssl.set_min_proto_version(min_protocol_version.map(|s| s.into()))?;
        ssl.set_max_proto_version(max_protocol_version.map(|s| s.into()))?;
        match client_cert_verify {
            TlsClientCertVerify::Ignore => ssl.set_verify(SslVerifyMode::NONE),
            TlsClientCertVerify::Optional(root) => {
                ssl.set_verify(SslVerifyMode::PEER);
                for root in root {
                    let root = openssl::x509::X509::from_der(root.as_ref())?;
                    ssl.cert_store_mut().add_cert(root)?;
                }
            }
            TlsClientCertVerify::Validate(root) => {
                ssl.set_verify(SslVerifyMode::PEER | SslVerifyMode::FAIL_IF_NO_PEER_CERT);
                for root in root {
                    let root = openssl::x509::X509::from_der(root.as_ref())?;
                    ssl.cert_store_mut().add_cert(root)?;
                }
            }
        }
        create_alpn_callback(&mut ssl);

        Ok(ssl.build().into_context())
    }

    async fn upgrade_client<S: Stream>(
        params: Self::ClientParams,
        stream: S,
    ) -> Result<(Self::Stream, TlsHandshake), SslError> {
        let stream = stream
            .downcast::<TokioStream>()
            .map_err(|_| crate::SslError::SslUnsupportedByClient)?;
        let TokioStream::Tcp(stream) = stream else {
            return Err(crate::SslError::SslUnsupportedByClient);
        };

        let mut stream = tokio_openssl::SslStream::new(params, stream)?;
        let res = Pin::new(&mut stream).do_handshake().await;
        if res.is_err() && stream.ssl().verify_result() != X509VerifyResult::OK {
            return Err(SslError::OpenSslErrorVerify(stream.ssl().verify_result()));
        }

        let alpn = stream
            .ssl()
            .selected_alpn_protocol()
            .map(|p| Cow::Owned(p.to_vec()));

        res.map_err(SslError::OpenSslError)?;
        let cert = stream
            .ssl()
            .peer_certificate()
            .map(|cert| cert.to_der())
            .transpose()?;
        let cert = cert.map(CertificateDer::from);
        Ok((
            TlsStream(stream),
            TlsHandshake {
                alpn,
                sni: None,
                cert,
            },
        ))
    }

    async fn upgrade_server<S: Stream>(
        params: TlsServerParameterProvider,
        stream: S,
    ) -> Result<(Self::Stream, TlsHandshake), SslError> {
        let stream = stream
            .downcast::<RewindStream<TokioStream>>()
            .map_err(|_| crate::SslError::SslUnsupportedByClient)?;
        let (stream, buffer) = stream.into_inner();
        if !buffer.is_empty() {
            // TODO: We should also be able to support rewinding
            return Err(crate::SslError::SslUnsupportedByClient);
        }
        let TokioStream::Tcp(stream) = stream else {
            return Err(crate::SslError::SslUnsupportedByClient);
        };

        let handshake = Arc::new(Mutex::new(HandshakeData::default()));

        let mut ssl = SslContextBuilder::new(SslMethod::tls_server())?;
        create_alpn_callback(&mut ssl);
        create_sni_callback(&mut ssl, params);
        ssl.set_client_hello_callback(move |ssl_ref, _alert| {
            // TODO: We need to check the clienthello for the SNI and determine
            // if we should verify the certificate or not. For now, just always
            // request a certificate. Note that if we return RETRY, we'll have
            // another chance to respond later (ie: when we implement async lookup
            // for TLS parameters).
            ssl_ref.set_verify(SslVerifyMode::PEER);
            Ok(ClientHelloResponse::SUCCESS)
        });

        let mut ssl = Ssl::new(&ssl.build())?;
        ssl.set_accept_state();
        ssl.set_ex_data(get_ssl_ex_data_index(), handshake.clone());

        let mut stream = tokio_openssl::SslStream::new(ssl, stream)?;
        let res = Pin::new(&mut stream).do_handshake().await;
        res.map_err(SslError::OpenSslError)?;

        let mut handshake = std::mem::take(&mut handshake.lock().unwrap().handshake);
        let cert = stream
            .ssl()
            .peer_certificate()
            .and_then(|c| c.to_der().ok());
        if let Some(cert) = cert {
            handshake.cert = Some(CertificateDer::from(cert));
        }

        Ok((TlsStream(stream), handshake))
    }
}

fn ssl_select_next_proto<'b>(server: &[u8], client: &'b [u8]) -> Option<&'b [u8]> {
    let mut server_packet = server;
    while !server_packet.is_empty() {
        let server_proto_len = *server_packet.first()? as usize;
        let server_proto = server_packet.get(1..1 + server_proto_len)?;
        let mut client_packet = client;
        while !client_packet.is_empty() {
            let client_proto_len = *client_packet.first()? as usize;
            let client_proto = client_packet.get(1..1 + client_proto_len)?;
            if client_proto == server_proto {
                return Some(client_proto);
            }
            client_packet = client_packet.get(1 + client_proto_len..)?;
        }
        server_packet = server_packet.get(1 + server_proto_len..)?;
    }
    None
}

/// Create an ALPN callback for the [`SslContextBuilder`].
fn create_alpn_callback(ssl: &mut SslContextBuilder) {
    ssl.set_alpn_select_callback(|ssl_ref, alpn| {
        let Some(mut handshake) = HandshakeData::from_ssl(ssl_ref) else {
            return Err(AlpnError::ALERT_FATAL);
        };

        if let Some(server) = handshake.server_alpn.take() {
            eprintln!("server: {:?} alpn: {:?}", server, alpn);
            let Some(selected) = ssl_select_next_proto(&server, alpn) else {
                return Err(AlpnError::NOACK);
            };
            handshake.handshake.alpn = Some(Cow::Owned(selected.to_vec()));

            Ok(selected)
        } else {
            Err(AlpnError::NOACK)
        }
    })
}

/// Create an SNI callback for the [`SslContextBuilder`].
fn create_sni_callback(ssl: &mut SslContextBuilder, params: TlsServerParameterProvider) {
    ssl.set_servername_callback(move |ssl_ref, _alert| {
        let Some(mut handshake) = HandshakeData::from_ssl(ssl_ref) else {
            return Ok(());
        };

        if let Some(servername) = ssl_ref.servername_raw(NameType::HOST_NAME) {
            handshake.handshake.sni =
                Some(Cow::Owned(String::from_utf8_lossy(servername).to_string()));
        }

        let params = params.lookup(None);
        if !params.alpn.is_empty() {
            handshake.server_alpn = Some(params.alpn.as_bytes().to_vec());
        }
        drop(handshake);

        let Ok(ssl) = OpensslDriver::init_server(&params) else {
            return Err(SniError::ALERT_FATAL);
        };
        let Ok(_) = ssl_ref.set_ssl_context(&ssl) else {
            return Err(SniError::ALERT_FATAL);
        };
        Ok(())
    });
}

impl From<SslVersion> for openssl::ssl::SslVersion {
    fn from(val: SslVersion) -> Self {
        match val {
            SslVersion::Tls1 => openssl::ssl::SslVersion::TLS1,
            SslVersion::Tls1_1 => openssl::ssl::SslVersion::TLS1_1,
            SslVersion::Tls1_2 => openssl::ssl::SslVersion::TLS1_2,
            SslVersion::Tls1_3 => openssl::ssl::SslVersion::TLS1_3,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ssl_select_next_proto() {
        let server = b"\x02h2\x08http/1.1";
        let client = b"\x08http/1.1";
        let selected = ssl_select_next_proto(server, client);
        assert_eq!(selected, Some(b"http/1.1".as_slice()));
    }

    #[test]
    fn test_ssl_select_next_proto_empty() {
        let server = b"";
        let client = b"";
        let selected = ssl_select_next_proto(server, client);
        assert_eq!(selected, None);
    }

    #[test]
    fn test_ssl_select_next_proto_invalid_length() {
        let server = b"\x08h2"; // Claims 8 bytes but only has 2
        let client = b"\x08http/1.1";
        let selected = ssl_select_next_proto(server, client);
        assert_eq!(selected, None);
    }

    #[test]
    fn test_ssl_select_next_proto_zero_length() {
        let server = b"\x00h2"; // Zero length but has data
        let client = b"\x08http/1.1";
        let selected = ssl_select_next_proto(server, client);
        assert_eq!(selected, None);
    }

    #[test]
    fn test_ssl_select_next_proto_truncated() {
        let server = b"\x02h2\x08http/1"; // Second protocol truncated
        let client = b"\x08http/1.1";
        let selected = ssl_select_next_proto(server, client);
        assert_eq!(selected, None);
    }

    #[test]
    fn test_ssl_select_next_proto_overflow() {
        let server = b"\xFFh2"; // Length that would overflow buffer
        let client = b"\x08http/1.1";
        let selected = ssl_select_next_proto(server, client);
        assert_eq!(selected, None);
    }

    #[test]
    fn test_ssl_select_next_proto_no_match() {
        let server = b"\x02h2";
        let client = b"\x08http/1.1";
        let selected = ssl_select_next_proto(server, client);
        assert_eq!(selected, None);
    }

    #[test]
    fn test_ssl_select_next_proto_multiple_server() {
        let server = b"\x02h2\x06spdy/2\x08http/1.1";
        let client = b"\x08http/1.1";
        let selected = ssl_select_next_proto(server, client);
        assert_eq!(selected, Some(b"http/1.1".as_slice()));
    }

    #[test]
    fn test_ssl_select_next_proto_multiple_client() {
        let server = b"\x08http/1.1";
        let client = b"\x02h2\x06spdy/2\x08http/1.1";
        let selected = ssl_select_next_proto(server, client);
        assert_eq!(selected, Some(b"http/1.1".as_slice()));
    }

    #[test]
    fn test_ssl_select_next_proto_first_match() {
        let server = b"\x02h2\x06spdy/2\x08http/1.1";
        let client = b"\x06spdy/2\x02h2\x08http/1.1";
        let selected = ssl_select_next_proto(server, client);
        assert_eq!(selected, Some(b"h2".as_slice()));
    }

    #[test]
    fn test_ssl_select_next_proto_first_match_2() {
        let server = b"\x06spdy/2\x02h2\x08http/1.1";
        let client = b"\x02h2\x06spdy/2\x08http/1.1";
        let selected = ssl_select_next_proto(server, client);
        assert_eq!(selected, Some(b"spdy/2".as_slice()));
    }
}
