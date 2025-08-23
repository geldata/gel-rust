#![cfg(unix)]

use futures::StreamExt;
use gel_stream::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn load_test_cert() -> rustls_pki_types::CertificateDer<'static> {
    gel_stream::test_keys::binary::SERVER_CERT.clone()
}

fn load_test_key() -> rustls_pki_types::PrivateKeyDer<'static> {
    gel_stream::test_keys::binary::SERVER_KEY.clone_key()
}

fn tls_server_parameters(alpn: TlsAlpn) -> TlsServerParameterProvider {
    TlsServerParameterProvider::new(TlsServerParameters {
        server_certificate: TlsKey::new(load_test_key(), load_test_cert()),
        client_cert_verify: TlsClientCertVerify::Ignore,
        min_protocol_version: None,
        max_protocol_version: None,
        alpn,
    })
}

async fn spawn_unix_tls_server<S: TlsDriver>(
    server_alpn: TlsAlpn,
    expected_alpn: Option<&str>,
) -> Result<
    (
        std::path::PathBuf,
        tokio::task::JoinHandle<Result<(), ConnectionError>>,
    ),
    ConnectionError,
> {
    let tempdir = tempfile::tempdir().unwrap();
    let path = tempdir.path().join("gel-stream-tls-test");

    let unix_addr = ResolvedTarget::from(std::os::unix::net::SocketAddr::from_pathname(&path)?);

    let mut acceptor = Acceptor::new_tls(unix_addr, tls_server_parameters(server_alpn))
        .bind_explicit::<S>()
        .await?;

    let expected_alpn = expected_alpn.map(|alpn| alpn.as_bytes().to_vec());
    let path_clone = path.clone();
    let accept_task = tokio::spawn(async move {
        // Keep tempdir alive for the duration of the test
        let _tempdir = tempdir;
        let mut connection = acceptor.next().await.unwrap()?;
        let handshake = connection
            .handshake()
            .unwrap_or_else(|| panic!("handshake was not available on {connection:?}"));
        assert!(handshake.version.is_some());
        assert_eq!(
            handshake.alpn.as_ref().map(|b| b.as_ref().to_vec()),
            expected_alpn
        );
        let mut buf = String::new();
        connection.read_to_string(&mut buf).await.unwrap();
        assert_eq!(buf, "Hello, Unix TLS!");
        connection.shutdown().await?;
        Ok::<_, ConnectionError>(())
    });
    Ok((path_clone, accept_task))
}

macro_rules! unix_tls_test (
    (
        $(
            $(#[ $attr:meta ])*
            async fn $name:ident<C: TlsDriver, S: TlsDriver>() -> Result<(), ConnectionError> $body:block
        )*
    ) => {
        mod rustls_openssl {
            use super::*;
            $(
                $(#[ $attr ])*
                async fn $name() -> Result<(), ConnectionError> {
                    async fn test_inner<C: TlsDriver, S: TlsDriver>() -> Result<(), ConnectionError> {
                        $body
                    }
                    test_inner::<RustlsDriver, OpensslDriver>().await
                }
            )*
        }

        mod openssl_rustls {
            use super::*;
            $(
                $(#[ $attr ])*
                async fn $name() -> Result<(), ConnectionError> {
                    async fn test_inner<C: TlsDriver, S: TlsDriver>() -> Result<(), ConnectionError> {
                        $body
                    }
                    test_inner::<OpensslDriver, RustlsDriver>().await
                }
            )*
        }
    }
);

unix_tls_test! {
    /// Basic Unix TLS test with ALPN - client connects to server over Unix socket with TLS
    #[tokio::test]
    #[ntest::timeout(30_000)]
    async fn test_unix_tls_basic<C: TlsDriver, S: TlsDriver>() -> Result<(), ConnectionError> {
        let (path, accept_task) = spawn_unix_tls_server::<S>(
            TlsAlpn::new_str(&["nope", "accepted"]),
            Some("accepted"),
        )
        .await?;

        let connect_task = tokio::spawn(async move {
            let name = TargetName::new_unix_path(path)?;
            let target = Target::new_tls(
                name,
                TlsParameters {
                    server_cert_verify: TlsServerCertVerify::Insecure,
                    alpn: TlsAlpn::new_str(&["accepted", "fake"]),
                    ..Default::default()
                },
            );
            let mut stm = Connector::<C>::new_explicit(target).unwrap().connect().await.unwrap();
            stm.write_all(b"Hello, Unix TLS!").await.unwrap();
            stm.shutdown().await?;
            Ok::<_, std::io::Error>(())
        });

        accept_task.await.unwrap().unwrap();
        connect_task.await.unwrap().unwrap();

        Ok(())
    }

    /// Unix TLS test with custom certificate verification
    #[tokio::test]
    #[ntest::timeout(30_000)]
    async fn test_unix_tls_custom_cert<C: TlsDriver, S: TlsDriver>() -> Result<(), ConnectionError> {
        let (path, accept_task) = spawn_unix_tls_server::<S>(
            TlsAlpn::new_str(&["unix-tls"]),
            Some("unix-tls"),
        )
        .await?;

        let connect_task = tokio::spawn(async move {
            let name = TargetName::new_unix_path(path)?;
            let target = Target::new_tls(
                name,
                TlsParameters {
                    server_cert_verify: TlsServerCertVerify::Insecure,
                    alpn: TlsAlpn::new_str(&["unix-tls"]),
                    ..Default::default()
                },
            );
            let mut stm = Connector::<C>::new_explicit(target).unwrap().connect().await.unwrap();
            stm.write_all(b"Hello, Unix TLS!").await.unwrap();
            stm.shutdown().await?;
            Ok::<_, std::io::Error>(())
        });

        accept_task.await.unwrap().unwrap();
        connect_task.await.unwrap().unwrap();

        Ok(())
    }
}
