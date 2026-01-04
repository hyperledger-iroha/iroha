//! Adds support for sending/receiving custom Iroha messages over the WebSocket

use core::{result::Result, time::Duration};

use axum::extract::ws::{Message, Utf8Bytes, WebSocket};
use futures::{SinkExt, StreamExt};
use norito::prelude::*;

#[cfg(test)]
const TIMEOUT: Duration = Duration::from_millis(10_000);
#[cfg(not(test))]
const TIMEOUT: Duration = Duration::from_millis(1000);

/// Error type with generic for actual Stream/Sink error type
#[derive(Debug, displaydoc::Display, thiserror::Error)]
#[ignore_extra_doc_attributes]
pub enum Error {
    /// Read message timeout
    ReadTimeout,
    /// Send message timeout
    SendTimeout,
    /// WebSocket error: {_0}
    WebSocket(#[source] axum::Error),
    /// Error during Norito message decoding
    Decode(#[from] norito::Error),
    /// Error during Norito message encoding
    Encode(norito::Error),
    /// Connection is closed
    Closed,
}

/// Wrapper to send/receive Norito encoded messages
#[derive(Debug)]
pub struct WebSocketNorito(pub(crate) WebSocket);

impl WebSocketNorito {
    /// Send message encoded in Norito
    pub async fn send<M: NoritoSerialize + Send>(&mut self, message: M) -> Result<(), Error> {
        // Use Norito framing (header + checksum) so clients can validate payloads.
        let buf = norito::to_bytes(&message).map_err(Error::Encode)?;
        tokio::time::timeout(
            TIMEOUT,
            self.0.send(Message::Binary(axum::body::Bytes::from(buf))),
        )
        .await
        .map_err(|_err| Error::SendTimeout)?
        .map_err(extract_ws_closed)
    }

    /// Send a JSON string as a Text WebSocket frame (used for convenience event streams).
    pub async fn send_json_text(&mut self, json: &str) -> Result<(), Error> {
        tokio::time::timeout(
            TIMEOUT,
            self.0
                .send(Message::Text(Utf8Bytes::from(json.to_string()))),
        )
        .await
        .map_err(|_err| Error::SendTimeout)?
        .map_err(extract_ws_closed)
    }

    /// Recv message and try to decode it
    pub async fn recv<M: for<'a> NoritoDeserialize<'a> + Send>(&mut self) -> Result<M, Error> {
        // NOTE: ignore non binary messages
        loop {
            let message = tokio::time::timeout(TIMEOUT, self.0.next())
                .await
                .map_err(|_err| Error::ReadTimeout)?
                // NOTE: `None` is the same as `ConnectionClosed` or `AlreadyClosed`
                .ok_or(Error::Closed)?
                .map_err(extract_ws_closed)?;

            match message {
                Message::Binary(binary) => {
                    // Decode using Norito framing (header + checksum).
                    return norito::decode_from_bytes::<M>(binary.as_ref()).map_err(Error::Decode);
                }
                Message::Text(_) | Message::Ping(_) | Message::Pong(_) => {
                    iroha_logger::trace!(?message, "Unexpected message received");
                }
                Message::Close(_) => {
                    iroha_logger::trace!(?message, "Close message received");
                }
            }
        }
    }

    /// Recv with a custom timeout duration, returning Err(ReadTimeout) on expiry.
    pub async fn recv_with_timeout<M: for<'a> NoritoDeserialize<'a> + Send>(
        &mut self,
        dur: Duration,
    ) -> Result<M, Error> {
        loop {
            let message = tokio::time::timeout(dur, self.0.next())
                .await
                .map_err(|_err| Error::ReadTimeout)?
                .ok_or(Error::Closed)?
                .map_err(extract_ws_closed)?;
            match message {
                Message::Binary(binary) => {
                    return norito::decode_from_bytes::<M>(binary.as_ref()).map_err(Error::Decode);
                }
                Message::Text(_) | Message::Ping(_) | Message::Pong(_) => {}
                Message::Close(_) => return Err(Error::Closed),
            }
        }
    }

    /// Discard messages and wait for close message
    pub async fn closed(&mut self) -> Result<(), Error> {
        loop {
            match self.0.next().await {
                // NOTE: `None` is the same as `ConnectionClosed` or `AlreadyClosed`
                None => return Ok(()),
                Some(Ok(_)) => {}
                // NOTE: technically `ConnectionClosed` or `AlreadyClosed` never returned
                // from `Stream` impl of `tokio_tungstenite` but left `ConnectionClosed` extraction to protect from potential change
                Some(Err(error)) => match extract_ws_closed(error) {
                    Error::Closed => return Ok(()),
                    error => return Err(error),
                },
            }
        }
    }

    /// Close websocket
    pub async fn close(mut self) -> Result<(), Error> {
        // NOTE: use `SinkExt::close` because it's not trying to write to closed socket
        match <_ as SinkExt<_>>::close(&mut self.0)
            .await
            .map_err(extract_ws_closed)
        {
            Err(Error::Closed) | Ok(()) => Ok(()),
            Err(error) => Err(error),
        }
    }
}

/// Check if websocket was closed normally
pub fn extract_ws_closed(error: axum::Error) -> Error {
    let error = error.into_inner();
    // NOTE: for this downcast to work versions of `tungstenite` here and in axum should match
    if let Some(tungstenite::Error::ConnectionClosed) = error.downcast_ref::<tungstenite::Error>() {
        return Error::Closed;
    }
    if let Some(tungstenite::Error::AlreadyClosed) = error.downcast_ref::<tungstenite::Error>() {
        return Error::Closed;
    }

    Error::WebSocket(axum::Error::new(error))
}

#[cfg(feature = "p2p_ws")]
mod ws_io {
    use futures::stream::{SplitSink, SplitStream};
    use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

    use super::*;

    /// Read half adapter over a WebSocket stream that yields bytes from Binary frames.
    pub struct WsReadHalf {
        inner: SplitStream<WebSocket>,
        buf: bytes::Bytes,
    }

    /// Write half adapter over a WebSocket sink that sends bytes as Binary frames on flush.
    pub struct WsWriteHalf {
        inner: SplitSink<WebSocket, Message>,
        buf: Vec<u8>,
    }

    impl From<WebSocket> for (WsReadHalf, WsWriteHalf) {
        fn from(ws: WebSocket) -> Self {
            let (sink, stream) = ws.split();
            (
                WsReadHalf {
                    inner: stream,
                    buf: bytes::Bytes::new(),
                },
                WsWriteHalf {
                    inner: sink,
                    buf: Vec::new(),
                },
            )
        }
    }

    impl AsyncRead for WsReadHalf {
        fn poll_read(
            mut self: core::pin::Pin<&mut Self>,
            cx: &mut core::task::Context<'_>,
            dst: &mut ReadBuf<'_>,
        ) -> core::task::Poll<std::io::Result<()>> {
            if !self.buf.is_empty() {
                let n = core::cmp::min(self.buf.len(), dst.remaining());
                dst.put_slice(&self.buf.split_to(n));
                return core::task::Poll::Ready(Ok(()));
            }
            match futures::ready!(core::pin::Pin::new(&mut self.inner).poll_next(cx)) {
                Some(Ok(Message::Binary(b))) => {
                    self.buf = b;
                    let n = core::cmp::min(self.buf.len(), dst.remaining());
                    dst.put_slice(&self.buf.split_to(n));
                    core::task::Poll::Ready(Ok(()))
                }
                Some(Ok(Message::Text(_)))
                | Some(Ok(Message::Ping(_)))
                | Some(Ok(Message::Pong(_))) => {
                    cx.waker().wake_by_ref();
                    core::task::Poll::Pending
                }
                Some(Ok(Message::Close(_))) | None => core::task::Poll::Ready(Ok(())),
                Some(Err(e)) => core::task::Poll::Ready(Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("ws read error: {e}"),
                ))),
            }
        }
    }

    impl AsyncWrite for WsWriteHalf {
        fn poll_write(
            mut self: core::pin::Pin<&mut Self>,
            _cx: &mut core::task::Context<'_>,
            data: &[u8],
        ) -> core::task::Poll<std::io::Result<usize>> {
            self.buf.extend_from_slice(data);
            core::task::Poll::Ready(Ok(data.len()))
        }

        fn poll_flush(
            mut self: core::pin::Pin<&mut Self>,
            cx: &mut core::task::Context<'_>,
        ) -> core::task::Poll<std::io::Result<()>> {
            if self.buf.is_empty() {
                return core::task::Poll::Ready(Ok(()));
            }
            let data = core::mem::take(&mut self.buf);
            let mut sink = core::pin::Pin::new(&mut self.inner);
            match futures::ready!(
                sink.as_mut()
                    .start_send(Message::Binary(axum::body::Bytes::from(data)))
                    .map_err(|e| {
                        std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("ws send error: {e}"),
                        )
                    })
            ) {
                () => {}
            }
            match futures::ready!(sink.as_mut().poll_flush(cx).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, format!("ws flush error: {e}"))
            })) {
                () => core::task::Poll::Ready(Ok(())),
            }
        }

        fn poll_shutdown(
            mut self: core::pin::Pin<&mut Self>,
            cx: &mut core::task::Context<'_>,
        ) -> core::task::Poll<std::io::Result<()>> {
            let mut sink = core::pin::Pin::new(&mut self.inner);
            match futures::ready!(sink.as_mut().start_send(Message::Close(None)).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, format!("ws close error: {e}"))
            })) {
                () => {}
            }
            match futures::ready!(sink.as_mut().poll_flush(cx).map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("ws close flush error: {e}"),
                )
            })) {
                () => core::task::Poll::Ready(Ok(())),
            }
        }
    }

    pub fn split(ws: WebSocket) -> (WsReadHalf, WsWriteHalf) {
        ws.into()
    }
}

#[cfg(feature = "p2p_ws")]
pub use ws_io::{WsReadHalf, WsWriteHalf, split as ws_split};
