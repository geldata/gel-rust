# gel-stream

This crate provides a runtime and TLS agnostic client and server stream API for
services requiring TCP/Unix socket, plaintext, TLS, and STARTTLS connections.

The crate may be used with either an OpenSSL or Rustls TLS implementation
without changing the API.

## Features

- `full`: Enable all features (not recommended).
- `openssl`: Enable OpenSSL support.
- `rustls`: Enable Rustls support.
- `tokio`: Enable Tokio support (default).
- `hickory`: Enable Hickory support.
- `keepalive`: Enable keepalive support.
- `serde`: Enable serde serialization support for most types.
- `pem`: Enable PEM support for TLS parameters.

## TLS

TLS is supported via the `openssl` or `rustls` features. Regardless of which TLS
library is used, the API is the same.

## Usage

The crate provides a `Target` and `Connector` for clients and a `Acceptor` for
servers.

### Examples

Creating and connecting to a TCP server:

```rust
use gel_stream::*;
use std::net::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use futures::TryStreamExt;

#[tokio::main]
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Create a server that listens on all interfaces on a random port.
    let acceptor = Acceptor::new_tcp(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0));
    let mut server = acceptor.bind().await?;
    let addr = server.local_address()?;

    /// When creating servers, clients and servers should be run in separate tasks.
    let task1 = tokio::spawn(async move {
        let mut server_conn = server.try_next().await?.expect("Didn't get a connection");
        server_conn.write_all(b"Hello, world!").await?;
        std::io::Result::Ok(())
    });

    let task2 = tokio::spawn(async move {
        let mut client_conn = Connector::new(Target::new_resolved(addr))?.connect().await?;
        let mut buffer = String::new();
        client_conn.read_to_string(&mut buffer).await?;
        assert_eq!(buffer, "Hello, world!");
        std::io::Result::Ok(())
    });

    task1.await??;
    task2.await??;

    Ok(())
}

# run().expect("failed to run example!");
```

Creating a TLS server with a given key and certificate, and connecting to it:

```rust
use gel_stream::*;
use std::net::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use futures::TryStreamExt;

#[tokio::main]
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Create a server that listens on all interfaces on a random port.
    let tls_params = TlsServerParameters::new_with_certificate(TlsKey::new_pem(
        include_bytes!("../tests/certs/server.key.pem"),
        include_bytes!("../tests/certs/server.cert.pem"),
    )?);
    let acceptor = Acceptor::new_tcp_tls(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
        TlsServerParameterProvider::new(tls_params),
    );
    let mut server = acceptor.bind().await?;
    let addr = server.local_address()?;

    /// When creating servers, clients and servers should be run in separate tasks.
    let task1 = tokio::spawn(async move {
        let mut server_conn = server.try_next().await?.expect("Didn't get a connection");
        server_conn.write_all(b"Hello, world!").await?;
        std::io::Result::Ok(())
    });

    let task2 = tokio::spawn(async move {
        let mut client_conn = Connector::new(Target::new_resolved_tls(addr, TlsParameters::insecure()))?.connect().await?;
        let mut buffer = String::new();
        client_conn.read_to_string(&mut buffer).await?;
        assert_eq!(buffer, "Hello, world!");
        std::io::Result::Ok(())
    });

    task1.await??;
    task2.await??;

    Ok(())
}

# run().expect("failed to run example!");
```
