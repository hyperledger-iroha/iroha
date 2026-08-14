//! Adds support for sending/receiving custom Iroha messages over the WebSocket
use axum::extract::ws::{CloseFrame, Message, Utf8Bytes, WebSocket};
use core::{result::Result, time::Duration};
use futures::{SinkExt, StreamExt};
use norito::prelude::*;
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
    /// Unexpected WebSocket frame `{actual}`; expected {expected}
    UnexpectedFrame {
        /// Expected frame kind for the current protocol state.
        expected: &'static str,
        /// Received frame kind.
        actual: &'static str,
    },
    /// Connection is closed
    Closed,
}
/// RFC 6455 close code for invalid frame payload data.
pub const CLOSE_INVALID_PAYLOAD: u16 = 1007;
/// RFC 6455 close code for a protocol policy violation.
pub const CLOSE_POLICY_VIOLATION: u16 = 1008;
/// RFC 6455 close code for an unexpected server failure.
pub const CLOSE_INTERNAL_ERROR: u16 = 1011;
/// RFC 6455 close code asking the client to retry later.
pub const CLOSE_TRY_AGAIN_LATER: u16 = 1013;
fn decode_subscription_request<M>(bytes: &[u8]) -> Result<M, norito::Error>
where
    M: NoritoSerialize,
    for<'de> M: NoritoDeserialize<'de>,
{
    norito::decode_canonical(bytes)
}
/// Wrapper to send/receive Norito encoded messages
#[derive(Debug)]
pub struct WebSocketNorito {
    ws: WebSocket,
    timeout: Duration,
}
impl WebSocketNorito {
    /// Create a new Norito WebSocket wrapper with a fixed message timeout.
    #[must_use]
    pub fn new(ws: WebSocket, timeout: Duration) -> Self {
        Self { ws, timeout }
    }
    /// Send message encoded in Norito
    pub async fn send<M: NoritoSerialize + Send>(&mut self, message: M) -> Result<(), Error> {
        // Use Norito framing (header + checksum) so clients can validate payloads.
        let buf = norito::to_bytes(&message).map_err(Error::Encode)?;
        tokio::time::timeout(
            self.timeout,
            self.ws.send(Message::Binary(axum::body::Bytes::from(buf))),
        )
        .await
        .map_err(|_err| Error::SendTimeout)?
        .map_err(extract_ws_closed)
    }
    /// Send a JSON string as a Text WebSocket frame (used for convenience event streams).
    pub async fn send_json_text(&mut self, json: &str) -> Result<(), Error> {
        tokio::time::timeout(
            self.timeout,
            self.ws
                .send(Message::Text(Utf8Bytes::from(json.to_string()))),
        )
        .await
        .map_err(|_err| Error::SendTimeout)?
        .map_err(extract_ws_closed)
    }
    /// Send an empty WebSocket ping frame as a transport heartbeat.
    pub async fn ping(&mut self) -> Result<(), Error> {
        tokio::time::timeout(
            self.timeout,
            self.ws.send(Message::Ping(axum::body::Bytes::new())),
        )
        .await
        .map_err(|_err| Error::SendTimeout)?
        .map_err(extract_ws_closed)
    }
    /// Receive and decode one canonical, uncompressed Norito request.
    pub async fn recv<M>(&mut self) -> Result<M, Error>
    where
        M: NoritoSerialize,
        for<'a> M: NoritoDeserialize<'a>,
        M: Send,
    {
        // Control frames remain valid while text data frames are a protocol error.
        loop {
            let message = tokio::time::timeout(self.timeout, self.ws.next())
                .await
                .map_err(|_err| Error::ReadTimeout)?
                // NOTE: `None` is the same as `ConnectionClosed` or `AlreadyClosed`
                .ok_or(Error::Closed)?
                .map_err(extract_ws_closed)?;
            match message {
                Message::Binary(binary) => {
                    return decode_subscription_request::<M>(binary.as_ref())
                        .map_err(Error::Decode);
                }
                Message::Text(_) => {
                    return Err(Error::UnexpectedFrame {
                        expected: "a binary Norito subscription request",
                        actual: "text",
                    });
                }
                Message::Ping(_) | Message::Pong(_) => {
                    iroha_logger::trace!(?message, "Unexpected message received");
                }
                Message::Close(_) => {
                    iroha_logger::trace!(?message, "Close message received");
                    return Err(Error::Closed);
                }
            }
        }
    }
    /// Receive one canonical request with a custom timeout.
    ///
    /// Returns [`Error::ReadTimeout`] when `dur` expires.
    pub async fn recv_with_timeout<M>(&mut self, dur: Duration) -> Result<M, Error>
    where
        M: NoritoSerialize,
        for<'a> M: NoritoDeserialize<'a>,
        M: Send,
    {
        loop {
            let message = tokio::time::timeout(dur, self.ws.next())
                .await
                .map_err(|_err| Error::ReadTimeout)?
                .ok_or(Error::Closed)?
                .map_err(extract_ws_closed)?;
            match message {
                Message::Binary(binary) => {
                    return decode_subscription_request::<M>(binary.as_ref())
                        .map_err(Error::Decode);
                }
                Message::Text(_) => {
                    return Err(Error::UnexpectedFrame {
                        expected: "a binary Norito subscription request",
                        actual: "text",
                    });
                }
                Message::Ping(_) | Message::Pong(_) => {}
                Message::Close(_) => return Err(Error::Closed),
            }
        }
    }
    /// Wait for the peer to close while rejecting post-subscription data frames.
    ///
    /// Canonical Torii event and block streams are server-to-client after their
    /// single binary subscription request. Silently accepting further data
    /// frames would make protocol mistakes indistinguishable from supported
    /// control messages.
    pub async fn closed(&mut self) -> Result<(), Error> {
        loop {
            match self.ws.next().await {
                // NOTE: `None` is the same as `ConnectionClosed` or `AlreadyClosed`
                None => return Ok(()),
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                Some(Ok(Message::Close(_))) => return Ok(()),
                Some(Ok(Message::Binary(_))) => {
                    return Err(Error::UnexpectedFrame {
                        expected: "ping, pong, or close",
                        actual: "binary",
                    });
                }
                Some(Ok(Message::Text(_))) => {
                    return Err(Error::UnexpectedFrame {
                        expected: "ping, pong, or close",
                        actual: "text",
                    });
                }
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
        match <_ as SinkExt<_>>::close(&mut self.ws)
            .await
            .map_err(extract_ws_closed)
        {
            Err(Error::Closed) | Ok(()) => Ok(()),
            Err(error) => Err(error),
        }
    }
    /// Close the WebSocket with a stable protocol status and reason.
    pub async fn close_with(self, code: u16, reason: impl Into<String>) -> Result<(), Error> {
        let mut this = self;
        let reason = reason.into();
        let frame = CloseFrame {
            code,
            reason: Utf8Bytes::from(reason),
        };
        match tokio::time::timeout(this.timeout, this.ws.send(Message::Close(Some(frame)))).await {
            Err(_elapsed) => return Err(Error::SendTimeout),
            Ok(Err(error)) => match extract_ws_closed(error) {
                Error::Closed => return Ok(()),
                error => return Err(error),
            },
            Ok(Ok(())) => {}
        }
        match <_ as SinkExt<_>>::close(&mut this.ws)
            .await
            .map_err(extract_ws_closed)
        {
            Err(Error::Closed) | Ok(()) => Ok(()),
            Err(error) => Err(error),
        }
    }
}
#[cfg(test)]
mod subscription_decode_tests {
    use super::*;
    use iroha_data_model::{
        block::stream::BlockSubscriptionRequest, events::stream::EventSubscriptionRequest,
    };
    use std::num::NonZeroU64;
    fn assert_common_noncanonical_frames_rejected<T>(value: &T)
    where
        T: NoritoSerialize,
        for<'de> T: NoritoDeserialize<'de>,
    {
        let canonical = norito::encode_canonical(value).expect("encode canonical subscription");
        decode_subscription_request::<T>(&canonical).expect("canonical subscription must decode");
        let compressed =
            norito::to_compressed_bytes(value, Some(norito::CompressionConfig::default()))
                .expect("encode compressed subscription");
        assert!(matches!(
            decode_subscription_request::<T>(&compressed),
            Err(norito::Error::NonCanonicalEncoding)
        ));
        let mut trailing = canonical.clone();
        trailing.push(0);
        assert!(decode_subscription_request::<T>(&trailing).is_err());
        let length_offset = norito::core::Header::SIZE
            - 2 * core::mem::size_of::<u64>()
            - core::mem::size_of::<u8>();
        let mut oversized = canonical;
        oversized[length_offset..length_offset + core::mem::size_of::<u64>()]
            .copy_from_slice(&(1024_u64 * 1024).to_le_bytes());
        assert!(decode_subscription_request::<T>(&oversized).is_err());
    }
    #[test]
    fn event_subscription_requires_one_exact_canonical_frame() {
        let request = EventSubscriptionRequest::new(Vec::new());
        assert_common_noncanonical_frames_rejected(&request);
        let canonical = norito::encode_canonical(&request).expect("encode canonical event request");
        let alternate_flags =
            norito::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let alternate = {
            let _flags = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            norito::core::to_bytes(&request).expect("encode alternate-layout event request")
        };
        assert_ne!(
            alternate, canonical,
            "fixture must exercise alternate flags"
        );
        assert!(matches!(
            decode_subscription_request::<EventSubscriptionRequest>(&alternate),
            Err(norito::Error::NonCanonicalEncoding)
        ));
    }
    #[test]
    fn block_subscription_requires_one_exact_canonical_frame() {
        let request = BlockSubscriptionRequest(
            NonZeroU64::new(1).expect("subscription height must be non-zero"),
        );
        assert_common_noncanonical_frames_rejected(&request);
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
    use super::*;
    use futures::stream::{SplitSink, SplitStream};
    use futures::{Sink, Stream};
    use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
    /// Read half adapter over a WebSocket stream that yields bytes from Binary frames.
    pub struct WsReadHalf {
        inner: SplitStream<WebSocket>,
        buf: axum::body::Bytes,
    }
    /// Write half adapter over a WebSocket sink that sends bytes as Binary frames on flush.
    pub struct WsWriteHalf {
        inner: SplitSink<WebSocket, Message>,
        buf: Vec<u8>,
    }
    impl WsWriteHalf {
        fn poll_send_buffered(
            &mut self,
            cx: &mut core::task::Context<'_>,
        ) -> core::task::Poll<std::io::Result<()>> {
            if self.buf.is_empty() {
                return core::task::Poll::Ready(Ok(()));
            }
            {
                let mut sink = core::pin::Pin::new(&mut self.inner);
                futures::ready!(sink.as_mut().poll_ready(cx).map_err(|e| {
                    std::io::Error::new(std::io::ErrorKind::Other, format!("ws ready error: {e}"))
                }))?;
            }
            let data = core::mem::take(&mut self.buf);
            debug_assert!(data.len() <= iroha_p2p::transport::ws::WEBSOCKET_CHUNK_BYTES);
            let mut sink = core::pin::Pin::new(&mut self.inner);
            sink.as_mut()
                .start_send(Message::Binary(axum::body::Bytes::from(data)))
                .map_err(|e| {
                    std::io::Error::new(std::io::ErrorKind::Other, format!("ws send error: {e}"))
                })?;
            core::task::Poll::Ready(Ok(()))
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
            let next = futures::ready!(core::pin::Pin::new(&mut self.inner).poll_next(cx));
            match next {
                Some(Ok(Message::Binary(b))) if b.is_empty() => {
                    // Empty WebSocket data messages contribute no bytes and do
                    // not terminate the adapted byte stream. Yield after one
                    // ignored message to keep each poll bounded.
                    cx.waker().wake_by_ref();
                    core::task::Poll::Pending
                }
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
            cx: &mut core::task::Context<'_>,
            data: &[u8],
        ) -> core::task::Poll<std::io::Result<usize>> {
            if self.buf.len() == iroha_p2p::transport::ws::WEBSOCKET_CHUNK_BYTES {
                futures::ready!(self.poll_send_buffered(cx))?;
            }
            let accepted = data.len().min(
                iroha_p2p::transport::ws::WEBSOCKET_CHUNK_BYTES.saturating_sub(self.buf.len()),
            );
            self.buf.extend_from_slice(&data[..accepted]);
            core::task::Poll::Ready(Ok(accepted))
        }
        fn poll_flush(
            mut self: core::pin::Pin<&mut Self>,
            cx: &mut core::task::Context<'_>,
        ) -> core::task::Poll<std::io::Result<()>> {
            futures::ready!(self.poll_send_buffered(cx))?;
            let mut sink = core::pin::Pin::new(&mut self.inner);
            futures::ready!(sink.as_mut().poll_flush(cx).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, format!("ws flush error: {e}"))
            }))?;
            core::task::Poll::Ready(Ok(()))
        }
        fn poll_shutdown(
            mut self: core::pin::Pin<&mut Self>,
            cx: &mut core::task::Context<'_>,
        ) -> core::task::Poll<std::io::Result<()>> {
            futures::ready!(self.as_mut().poll_flush(cx))?;
            let mut sink = core::pin::Pin::new(&mut self.inner);
            futures::ready!(sink.as_mut().poll_close(cx).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, format!("ws close error: {e}"))
            }))?;
            core::task::Poll::Ready(Ok(()))
        }
    }
    pub fn split(ws: WebSocket) -> (WsReadHalf, WsWriteHalf) {
        let (sink, stream) = ws.split();
        (
            WsReadHalf {
                inner: stream,
                buf: axum::body::Bytes::new(),
            },
            WsWriteHalf {
                inner: sink,
                buf: Vec::new(),
            },
        )
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        use axum::{Router, extract::ws::WebSocketUpgrade, routing::get};
        use std::sync::{Arc, Mutex};
        use tokio::{
            io::{AsyncReadExt as _, AsyncWriteExt as _},
            net::TcpListener,
            sync::oneshot,
            task::spawn_blocking,
            time::{Duration, timeout},
        };
        use tungstenite::{
            Message as TungsteniteMessage, client::connect_with_config, protocol::WebSocketConfig,
            stream::MaybeTlsStream,
        };
        const TEST_TIMEOUT: Duration = Duration::from_secs(30);
        fn websocket_config() -> WebSocketConfig {
            let chunk_bytes = iroha_p2p::transport::ws::WEBSOCKET_CHUNK_BYTES;
            WebSocketConfig::default()
                .read_buffer_size(chunk_bytes)
                .write_buffer_size(chunk_bytes)
                .max_write_buffer_size(chunk_bytes * 4)
                .max_message_size(Some(chunk_bytes))
                .max_frame_size(Some(chunk_bytes))
        }
        async fn assert_chunked_stream(byte_len: usize) {
            let expected = Arc::new(
                (0..byte_len)
                    .map(|index| {
                        u8::try_from(index % 251).expect("fixture byte is bounded below 251")
                    })
                    .collect::<Vec<_>>(),
            );
            let server_payload = Arc::clone(&expected);
            let (write_done_tx, write_done_rx) = oneshot::channel();
            let write_done_tx = Arc::new(Mutex::new(Some(write_done_tx)));
            let server_write_done = Arc::clone(&write_done_tx);
            let chunk_bytes = iroha_p2p::transport::ws::WEBSOCKET_CHUNK_BYTES;
            let app = Router::new().route(
                "/p2p",
                get(move |ws: WebSocketUpgrade| {
                    let payload = Arc::clone(&server_payload);
                    let write_done = Arc::clone(&server_write_done);
                    async move {
                        ws.read_buffer_size(chunk_bytes)
                            .write_buffer_size(chunk_bytes)
                            .max_write_buffer_size(chunk_bytes * 4)
                            .max_message_size(chunk_bytes)
                            .max_frame_size(chunk_bytes)
                            .on_upgrade(move |socket| async move {
                                let (_read, mut write) = split(socket);
                                let result = async {
                                    write.write_all(payload.as_ref()).await?;
                                    write.flush().await
                                }
                                .await
                                .map_err(|error| error.to_string());
                                let sender = write_done
                                    .lock()
                                    .expect("write completion mutex must not be poisoned")
                                    .take()
                                    .expect("test route must be upgraded exactly once");
                                let _ = sender.send(result);
                            })
                    }
                }),
            );
            let listener = TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind loopback WebSocket server");
            let address = listener.local_addr().expect("read loopback server address");
            let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
            let server = axum::serve(listener, app).with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            });
            let server_task = tokio::spawn(async move { server.await });
            let client_task = spawn_blocking(move || {
                let (mut client, _response) =
                    connect_with_config(format!("ws://{address}/p2p"), Some(websocket_config()), 0)
                        .expect("WebSocket handshake must succeed");
                if let MaybeTlsStream::Plain(stream) = client.get_mut() {
                    stream
                        .set_read_timeout(Some(TEST_TIMEOUT))
                        .expect("set loopback WebSocket read timeout");
                }
                let mut received = Vec::with_capacity(byte_len);
                let mut binary_messages = 0usize;
                while received.len() < byte_len {
                    match client.read().expect("WebSocket message must be valid") {
                        TungsteniteMessage::Binary(chunk) => {
                            assert!(
                                !chunk.is_empty(),
                                "writer must not emit empty data messages"
                            );
                            assert!(
                                chunk.len() <= chunk_bytes,
                                "binary message length {} exceeds the {chunk_bytes}-byte transport cap",
                                chunk.len()
                            );
                            received.extend_from_slice(&chunk);
                            binary_messages = binary_messages
                                .checked_add(1)
                                .expect("test message count must remain bounded");
                        }
                        other => panic!("expected a bounded binary message, got {other:?}"),
                    }
                }
                (received, binary_messages)
            });
            let (received, binary_messages) = timeout(TEST_TIMEOUT, client_task)
                .await
                .expect("WebSocket observer must not time out")
                .expect("WebSocket observer must not panic");
            assert_eq!(
                received.as_slice(),
                expected.as_slice(),
                "WebSocket chunking must preserve bytes"
            );
            assert_eq!(
                binary_messages,
                byte_len.div_ceil(chunk_bytes),
                "writer must emit the minimum number of bounded messages"
            );
            timeout(TEST_TIMEOUT, write_done_rx)
                .await
                .expect("server writer completion must not time out")
                .expect("server writer completion channel must remain open")
                .expect("server writer must flush successfully");
            let _ = shutdown_tx.send(());
            timeout(TEST_TIMEOUT, server_task)
                .await
                .expect("loopback server shutdown must not time out")
                .expect("loopback server task must not panic")
                .expect("loopback server must shut down cleanly");
        }
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn write_half_chunks_boundaries_and_default_maximum_p2p_frame() {
            let chunk_bytes = iroha_p2p::transport::ws::WEBSOCKET_CHUNK_BYTES;
            for byte_len in [
                chunk_bytes - 1,
                chunk_bytes,
                chunk_bytes + 1,
                iroha_config::parameters::defaults::network::MAX_FRAME_BYTES.get()
                    + core::mem::size_of::<u32>(),
            ] {
                assert_chunked_stream(byte_len).await;
            }
        }
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn read_half_ignores_empty_binary_without_reporting_stream_eof() {
            let (read_done_tx, read_done_rx) = oneshot::channel();
            let read_done_tx = Arc::new(Mutex::new(Some(read_done_tx)));
            let server_read_done = Arc::clone(&read_done_tx);
            let chunk_bytes = iroha_p2p::transport::ws::WEBSOCKET_CHUNK_BYTES;
            let app = Router::new().route(
                "/p2p",
                get(move |ws: WebSocketUpgrade| {
                    let read_done = Arc::clone(&server_read_done);
                    async move {
                        ws.read_buffer_size(chunk_bytes)
                            .write_buffer_size(chunk_bytes)
                            .max_write_buffer_size(chunk_bytes * 4)
                            .max_message_size(chunk_bytes)
                            .max_frame_size(chunk_bytes)
                            .on_upgrade(move |socket| async move {
                                let (mut read, _write) = split(socket);
                                let mut received = [0_u8; 4];
                                let result = read
                                    .read_exact(&mut received)
                                    .await
                                    .map(|_| received)
                                    .map_err(|error| error.to_string());
                                let sender = read_done
                                    .lock()
                                    .expect("read completion mutex must not be poisoned")
                                    .take()
                                    .expect("test route must be upgraded exactly once");
                                let _ = sender.send(result);
                            })
                    }
                }),
            );
            let listener = TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind loopback WebSocket server");
            let address = listener.local_addr().expect("read loopback server address");
            let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
            let server = axum::serve(listener, app).with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            });
            let server_task = tokio::spawn(async move { server.await });
            let expected = [0xA5, 0x5A, 0x11, 0x22];
            let client_task = spawn_blocking(move || {
                let (mut client, _response) =
                    connect_with_config(format!("ws://{address}/p2p"), Some(websocket_config()), 0)
                        .expect("WebSocket handshake must succeed");
                client
                    .send(TungsteniteMessage::Binary(Vec::new().into()))
                    .expect("send legal empty WebSocket data message");
                client
                    .send(TungsteniteMessage::Binary(expected.to_vec().into()))
                    .expect("send following non-empty WebSocket data message");
            });
            timeout(TEST_TIMEOUT, client_task)
                .await
                .expect("WebSocket writer must not time out")
                .expect("WebSocket writer must not panic");
            let received = timeout(TEST_TIMEOUT, read_done_rx)
                .await
                .expect("server reader completion must not time out")
                .expect("server reader completion channel must remain open")
                .expect("empty Binary message must not terminate the byte stream");
            assert_eq!(received, expected);
            let _ = shutdown_tx.send(());
            timeout(TEST_TIMEOUT, server_task)
                .await
                .expect("loopback server shutdown must not time out")
                .expect("loopback server task must not panic")
                .expect("loopback server must shut down cleanly");
        }
    }
}
#[cfg(feature = "p2p_ws")]
pub use ws_io::{WsReadHalf, WsWriteHalf, split as ws_split};
