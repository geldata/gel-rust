use hyper::{
    body::{Body, Buf, Bytes},
    upgrade::Upgraded,
};
use hyper_util::rt::TokioIo;
use std::{
    pin::Pin,
    task::{Context, Poll, ready},
};

/// A stream that wraps a [hyper::upgrade::Upgraded].
#[derive(derive_io::AsyncRead, derive_io::AsyncWrite)]
pub struct HyperUpgradedStream {
    #[read]
    #[write]
    inner: TokioIo<Upgraded>,
}

impl HyperUpgradedStream {
    pub fn new(upgraded: Upgraded) -> Self {
        HyperUpgradedStream {
            inner: TokioIo::new(upgraded),
        }
    }
}

/// A stream that wraps a `hyper::body::Incoming` for reads, and provides
/// an mpsc channel of frames (bounded) for writes for a response body.
///
/// Note that an HTTP/1.x and HTTP/2 request/response pair _might_ be
/// technically duplex but we explicitly convert them to simplex here
/// because we cannot guarantee that a middleware box hasn't tampered with
/// the state.
pub struct HyperStream {
    state: StreamState,
    response_body_rx: tokio::sync::mpsc::Receiver<Bytes>,
}

enum StreamState {
    Reading {
        incoming: hyper::body::Incoming,
        partial_frame: hyper::body::Bytes,
        response_body_tx: tokio::sync::mpsc::Sender<Bytes>,
    },
    Writing(tokio_util::sync::PollSender<Bytes>),
    StaticResponse {
        buffer: hyper::body::Bytes,
    },
    Shutdown,
}

impl HyperStream {
    pub fn new(incoming: hyper::body::Incoming) -> Self {
        let (response_body_tx, response_body_rx) = tokio::sync::mpsc::channel(10); // Adjust buffer size as needed
        HyperStream {
            state: StreamState::Reading {
                incoming,
                partial_frame: Bytes::new(),
                response_body_tx,
            },
            response_body_rx,
        }
    }

    pub fn static_response(s: impl AsRef<str>) -> Self {
        let (_, response_body_rx) = tokio::sync::mpsc::channel(10); // Adjust buffer size as needed
        HyperStream {
            state: StreamState::StaticResponse {
                buffer: s.as_ref().as_bytes().to_vec().into(),
            },
            response_body_rx,
        }
    }
}

impl From<String> for HyperStream {
    fn from(s: String) -> Self {
        let (_, response_body_rx) = tokio::sync::mpsc::channel(10); // Adjust buffer size as needed
        Self {
            state: StreamState::StaticResponse {
                buffer: s.as_bytes().to_vec().into(),
            },
            response_body_rx,
        }
    }
}

impl tokio::io::AsyncRead for HyperStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();

        match &mut this.state {
            StreamState::StaticResponse { .. } => {
                Poll::Ready(Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "Stream is in static response state",
                )))
            }
            StreamState::Reading {
                incoming,
                partial_frame,
                ..
            } => {
                // If there are any partial bytes, copy them to the buffer first
                if !partial_frame.is_empty() {
                    let len = std::cmp::min(partial_frame.len(), buf.remaining());
                    buf.put_slice(&partial_frame[..len]);
                    partial_frame.advance(len);
                    if partial_frame.is_empty() {
                        *partial_frame = Bytes::new();
                    }
                    return Poll::Ready(Ok(()));
                }

                loop {
                    // Read from the incoming stream
                    break match Pin::new(&mut *incoming).poll_frame(cx) {
                        Poll::Ready(Some(Ok(mut data))) => {
                            // Ignore trailers
                            let Some(data) = data.data_mut() else {
                                continue;
                            };
                            let len = std::cmp::min(data.len(), buf.remaining());
                            buf.put_slice(&data[..len]);
                            if len < data.len() {
                                *partial_frame = data.slice(len..);
                            }
                            Poll::Ready(Ok(()))
                        }
                        Poll::Ready(Some(Err(e))) => Poll::Ready(Err(std::io::Error::other(e))),
                        Poll::Ready(None) => Poll::Ready(Ok(())),
                        Poll::Pending => Poll::Pending,
                    };
                }
            }
            StreamState::Writing(_) | StreamState::Shutdown => {
                Poll::Ready(Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "Stream is in writing or shutdown state",
                )))
            }
        }
    }
}

impl tokio::io::AsyncWrite for HyperStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let this = self.get_mut();
        loop {
            break match &mut this.state {
                StreamState::StaticResponse { .. } => {
                    return Poll::Ready(Err(std::io::Error::new(
                        std::io::ErrorKind::BrokenPipe,
                        "Stream is in static response state",
                    )));
                }
                StreamState::Reading {
                    response_body_tx, ..
                } => {
                    // Transition to Writing state
                    let tx = response_body_tx.clone();
                    this.state = StreamState::Writing(tokio_util::sync::PollSender::new(tx));
                    // Fall through to Writing case
                    continue;
                }
                StreamState::Writing(outgoing) => {
                    match ready!(Pin::new(&mut *outgoing).poll_reserve(cx)) {
                        Ok(_) => match outgoing.send_item(Bytes::copy_from_slice(buf)) {
                            Ok(_) => Poll::Ready(Ok(buf.len())),
                            Err(e) => Poll::Ready(Err(std::io::Error::new(
                                std::io::ErrorKind::BrokenPipe,
                                format!("Stream has been shut down: {e}"),
                            ))),
                        },
                        Err(e) => Poll::Ready(Err(std::io::Error::new(
                            std::io::ErrorKind::BrokenPipe,
                            format!("Stream has been shut down: {e}"),
                        ))),
                    }
                }
                StreamState::Shutdown => Poll::Ready(Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "Stream has been shut down",
                ))),
            };
        }
    }

    /// If the stream is in the writing state, we flush enough so that there's
    /// at least one send slot available, otherwise just return `Ok(())`.
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match &mut self.get_mut().state {
            StreamState::Writing(outgoing) => {
                let Ok(_) = ready!(Pin::new(&mut *outgoing).poll_reserve(cx)) else {
                    return Poll::Ready(Ok(()));
                };
                outgoing.abort_send();
                Poll::Ready(Ok(()))
            }
            StreamState::StaticResponse { .. }
            | StreamState::Reading { .. }
            | StreamState::Shutdown => Poll::Ready(Ok(())),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        this.state = StreamState::Shutdown;
        Poll::Ready(Ok(()))
    }
}

impl hyper::body::Body for HyperStream {
    type Data = hyper::body::Bytes;
    type Error = std::io::Error;
    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<hyper::body::Frame<Self::Data>, Self::Error>>> {
        let this = self.get_mut();
        if let StreamState::StaticResponse { buffer } = &mut this.state {
            return if buffer.is_empty() {
                Poll::Ready(None)
            } else {
                Poll::Ready(Some(Ok(hyper::body::Frame::data(buffer.split_off(0)))))
            };
        }
        this.response_body_rx
            .poll_recv(cx)
            .map(|option| option.map(|bytes| Ok(hyper::body::Frame::data(bytes))))
    }

    fn is_end_stream(&self) -> bool {
        self.response_body_rx.is_closed()
    }

    fn size_hint(&self) -> hyper::body::SizeHint {
        hyper::body::SizeHint::default()
    }
}
