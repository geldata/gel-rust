use futures::StreamExt;
use gel_stream::{
    Acceptor, BulkStreamDirection, ConnectionError, Connector, LocalAddress, StreamOptimization,
    StreamOptimizationExt, Target,
};
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[cfg(unix)]
#[tokio::test]
#[ntest::timeout(30_000)]
async fn test_low_level_target_unix() -> Result<(), ConnectionError> {
    let tempdir = tempfile::tempdir().unwrap();
    let path = tempdir.path().join("gel-stream-test");

    // Create a unix socket and connect to it
    let mut acceptor = Acceptor::new_unix_path(&path)?.bind().await?;

    let accept_task = tokio::spawn(async move {
        let mut connection = acceptor.next().await.unwrap().unwrap();
        connection
            .optimize_for(StreamOptimization::BulkStreaming(BulkStreamDirection::Both))
            .expect("Failed to optimize for bulk streaming");

        let mut buf = String::new();
        connection.read_to_string(&mut buf).await.unwrap();
        assert_eq!(buf, "Hello, world!");
    });

    let connect_task = tokio::spawn(async {
        let target = Target::new_unix_path(path)?;
        let mut stm = Connector::new(target).unwrap().connect().await.unwrap();
        stm.optimize_for(StreamOptimization::BulkStreaming(BulkStreamDirection::Both))
            .expect("Failed to optimize for bulk streaming");
        stm.write_all(b"Hello, world!").await?;
        Ok::<_, std::io::Error>(())
    });

    accept_task.await.unwrap();
    connect_task.await.unwrap().unwrap();

    Ok(())
}

#[tokio::test]
#[ntest::timeout(30_000)]
async fn test_low_level_target_tcp() -> Result<(), ConnectionError> {
    // Create a TCP listener on a random port
    let mut acceptor = Acceptor::new_tcp(SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0))
        .bind()
        .await?;
    let addr = acceptor.local_address()?;

    let accept_task = tokio::spawn(async move {
        let mut connection = acceptor.next().await.unwrap().unwrap();
        connection
            .optimize_for(StreamOptimization::BulkStreaming(BulkStreamDirection::Both))
            .expect("Failed to optimize for bulk streaming");

        let mut buf = String::new();
        connection.read_to_string(&mut buf).await.unwrap();
        assert_eq!(buf, "Hello, world!");
    });

    let connect_task = tokio::spawn(async move {
        let target = Target::new_resolved(addr);
        let mut stm = Connector::new(target).unwrap().connect().await.unwrap();
        stm.optimize_for(StreamOptimization::BulkStreaming(BulkStreamDirection::Both))
            .expect("Failed to optimize for bulk streaming");
        stm.write_all(b"Hello, world!").await?;
        Ok::<_, std::io::Error>(())
    });

    accept_task.await.unwrap();
    connect_task.await.unwrap().unwrap();

    Ok(())
}
