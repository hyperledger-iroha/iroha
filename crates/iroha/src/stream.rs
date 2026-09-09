//! Injectable, client-owned transport for canonical binary WebSocket streams.

use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{Sink, Stream};

use crate::{Error, Result, TransportErrorKind};

/// Exact authenticated upgrade and resource limits supplied to a stream transport.
pub struct StreamRequest {
    /// Complete one-shot WebSocket upgrade, including canonical authentication.
    pub request: http::Request<()>,
    /// Canonical operation owning this connection.
    pub operation: &'static str,
    /// Maximum bytes in one complete binary message, including fragmented messages.
    pub max_message_bytes: usize,
    /// Deadline for connecting and sending the subscription; zero disables it.
    pub timeout: Duration,
}

impl std::fmt::Debug for StreamRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamRequest")
            .field("operation", &self.operation)
            .field("max_message_bytes", &self.max_message_bytes)
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

/// Application data or the peer's terminal close disposition.
#[derive(Debug)]
pub enum StreamFrame {
    /// One complete binary message.
    Binary(Vec<u8>),
    /// The peer closed the stream. Only code 1000 is normal completion.
    Close {
        /// WebSocket status, when supplied by the peer.
        code: Option<u16>,
        /// Peer-supplied diagnostic, bounded by the WebSocket control-frame limit.
        reason: String,
    },
}

/// Bidirectional binary channel owned by one subscription.
///
/// Implementations handle ping/pong internally, reject text/raw frames, bound
/// fragmented messages before allocation, and release I/O when dropped. Dropping
/// a pending connect/send/close future must not start background retries.
pub trait StreamSocket:
    Stream<Item = Result<StreamFrame>> + Sink<Vec<u8>, Error = Error> + Send + Unpin
{
}

impl<T> StreamSocket for T where
    T: Stream<Item = Result<StreamFrame>> + Sink<Vec<u8>, Error = Error> + Send + Unpin
{
}

/// An established channel and its exact bounded HTTP upgrade response.
pub struct StreamConnection {
    /// Channel consumed by the SDK subscription state machine.
    pub socket: Box<dyn StreamSocket>,
    /// HTTP response; the SDK validates status and subprotocol before subscribing.
    pub response: http::Response<Vec<u8>>,
}

/// Cancellable connection future returned by an injected transport.
pub type StreamConnectFuture<'a> =
    Pin<Box<dyn Future<Output = Result<StreamConnection>> + Send + 'a>>;

/// Client-owned WebSocket connection boundary.
///
/// Dispatch the supplied authenticated upgrade exactly once, without redirects
/// or retries. Clones of a client share this transport. Separately built clients
/// receive independent defaults unless the caller explicitly shares an instance.
pub trait StreamTransport: std::fmt::Debug + Send + Sync {
    /// Connect to the exact upgrade target; cancellation releases pending I/O.
    fn connect(&self, request: StreamRequest) -> StreamConnectFuture<'_>;
}

#[derive(Debug)]
pub(crate) struct DefaultStreamTransport;

type NativeSocket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

struct Socket {
    inner: NativeSocket,
    operation: &'static str,
    maximum: usize,
}

fn socket_error(operation: &'static str, error: tungstenite::Error) -> Error {
    match error {
        tungstenite::Error::Capacity(tungstenite::error::CapacityError::MessageTooLong {
            size,
            max_size,
        }) => Error::ResponseTooLarge {
            maximum: max_size,
            actual: Some(size),
        },
        tungstenite::Error::Http(response) => Error::Http {
            operation,
            status: response.status().as_u16(),
            retry_after: crate::error::retry_after(response.headers()),
            body: response.into_body().unwrap_or_default(),
        },
        tungstenite::Error::Io(error) if error.kind() == std::io::ErrorKind::TimedOut => {
            Error::Timeout { operation }
        }
        tungstenite::Error::Io(error) => Error::Transport {
            operation,
            kind: TransportErrorKind::Io(error.kind()),
            details: error.to_string(),
        },
        error => Error::StreamProtocol {
            operation,
            details: error.to_string(),
        },
    }
}

impl StreamTransport for DefaultStreamTransport {
    fn connect(&self, request: StreamRequest) -> StreamConnectFuture<'_> {
        Box::pin(async move {
            let maximum = request.max_message_bytes;
            if maximum == 0 {
                return Err(Error::InvalidRequest {
                    operation: request.operation,
                    details: "stream message limit must be positive".to_owned(),
                });
            }
            let config = tungstenite::protocol::WebSocketConfig::default()
                .read_buffer_size(16 * 1024)
                .write_buffer_size(16 * 1024)
                .max_write_buffer_size(maximum.saturating_add(16 * 1024 + 1024))
                .max_message_size(Some(maximum))
                .max_frame_size(Some(maximum));
            // This API makes one connection and performs one HTTP upgrade. It
            // does not follow Location responses or retry failed handshakes.
            let (inner, response) =
                tokio_tungstenite::connect_async_with_config(request.request, Some(config), false)
                    .await
                    .map_err(|error| socket_error(request.operation, error))?;
            let (parts, body) = response.into_parts();
            Ok(StreamConnection {
                socket: Box::new(Socket {
                    inner,
                    operation: request.operation,
                    maximum,
                }),
                response: http::Response::from_parts(parts, body.unwrap_or_default()),
            })
        })
    }
}

impl Stream for Socket {
    type Item = Result<StreamFrame>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        use tungstenite::Message;
        // Bound control-frame work in one poll so a ready peer cannot starve
        // the executor with ping/pong traffic.
        for _ in 0..32 {
            match futures_util::ready!(Pin::new(&mut self.inner).poll_next(cx)) {
                Some(Ok(Message::Binary(bytes))) => {
                    return Poll::Ready(Some(Ok(StreamFrame::Binary(bytes.to_vec()))));
                }
                Some(Ok(Message::Close(frame))) => {
                    return Poll::Ready(Some(Ok(StreamFrame::Close {
                        code: frame.as_ref().map(|frame| u16::from(frame.code)),
                        reason: frame.map_or_else(String::new, |frame| frame.reason.to_string()),
                    })));
                }
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                Some(Ok(Message::Text(_) | Message::Frame(_))) => {
                    return Poll::Ready(Some(Err(Error::StreamProtocol {
                        operation: self.operation,
                        details: "Torii WebSocket sent a non-binary data frame".to_owned(),
                    })));
                }
                Some(Err(error)) => {
                    return Poll::Ready(Some(Err(socket_error(self.operation, error))));
                }
                None => return Poll::Ready(None),
            }
        }
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

impl Sink<Vec<u8>> for Socket {
    type Error = Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        let operation = self.operation;
        Pin::new(&mut self.inner)
            .poll_ready(cx)
            .map_err(|error| socket_error(operation, error))
    }

    fn start_send(mut self: Pin<&mut Self>, item: Vec<u8>) -> Result<()> {
        if item.len() > self.maximum {
            return Err(Error::ResponseTooLarge {
                maximum: self.maximum,
                actual: Some(item.len()),
            });
        }
        let operation = self.operation;
        Pin::new(&mut self.inner)
            .start_send(tungstenite::Message::Binary(item.into()))
            .map_err(|error| socket_error(operation, error))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        let operation = self.operation;
        Pin::new(&mut self.inner)
            .poll_flush(cx)
            .map_err(|error| socket_error(operation, error))
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        let operation = self.operation;
        Pin::new(&mut self.inner)
            .poll_close(cx)
            .map_err(|error| socket_error(operation, error))
    }
}

#[cfg(test)]
mod tests;
