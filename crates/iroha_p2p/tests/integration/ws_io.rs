//! Bounded server-side `WebSocket` byte stream used by P2P integration tests.
use bytes::Bytes;
use futures::{Sink as _, Stream as _};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf, ReadHalf, WriteHalf};
use tokio_tungstenite::{
    WebSocketStream, accept_async_with_config,
    tungstenite::{Message, protocol::WebSocketConfig},
};
const CHUNK_BYTES: usize = iroha_p2p::transport::ws::WEBSOCKET_CHUNK_BYTES;
fn websocket_config() -> WebSocketConfig {
    WebSocketConfig::default()
        .read_buffer_size(CHUNK_BYTES)
        .write_buffer_size(CHUNK_BYTES)
        .max_write_buffer_size(CHUNK_BYTES * 4)
        .max_message_size(Some(CHUNK_BYTES))
        .max_frame_size(Some(CHUNK_BYTES))
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ShutdownState {
    Open,
    FlushingForClose,
    Closing,
    Closed,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReadState {
    Open,
    FlushingCloseAcknowledgement,
    Eof,
}
/// Stateful adapter that preserves stream semantics across bounded `Binary` messages.
pub(super) struct WsByteStream<S> {
    inner: WebSocketStream<S>,
    read_buffer: Bytes,
    read_state: ReadState,
    write_buffer: Vec<u8>,
    shutdown: ShutdownState,
}
impl<S> WsByteStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn new(inner: WebSocketStream<S>) -> Self {
        Self {
            inner,
            read_buffer: Bytes::new(),
            read_state: ReadState::Open,
            write_buffer: Vec::new(),
            shutdown: ShutdownState::Open,
        }
    }
    fn poll_send_buffered(
        &mut self,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<std::io::Result<()>> {
        if self.write_buffer.is_empty() {
            return core::task::Poll::Ready(Ok(()));
        }
        let mut sink = core::pin::Pin::new(&mut self.inner);
        futures::ready!(sink.as_mut().poll_ready(cx))
            .map_err(|err| std::io::Error::other(format!("ws ready error: {err}")))?;
        let payload = core::mem::take(&mut self.write_buffer);
        debug_assert!(payload.len() <= CHUNK_BYTES);
        sink.as_mut()
            .start_send(Message::Binary(payload.into()))
            .map_err(|err| std::io::Error::other(format!("ws send error: {err}")))?;
        core::task::Poll::Ready(Ok(()))
    }
    fn poll_flush_buffered(
        &mut self,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<std::io::Result<()>> {
        futures::ready!(self.poll_send_buffered(cx))?;
        futures::ready!(core::pin::Pin::new(&mut self.inner).poll_flush(cx))
            .map_err(|err| std::io::Error::other(format!("ws flush error: {err}")))?;
        core::task::Poll::Ready(Ok(()))
    }
    fn poll_flush_close_acknowledgement(
        &mut self,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<std::io::Result<()>> {
        futures::ready!(core::pin::Pin::new(&mut self.inner).poll_flush(cx)).map_err(|err| {
            std::io::Error::other(format!("ws close acknowledgement error: {err}"))
        })?;
        self.read_state = ReadState::Eof;
        core::task::Poll::Ready(Ok(()))
    }
}
impl<S> AsyncRead for WsByteStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_read(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> core::task::Poll<std::io::Result<()>> {
        if buf.remaining() == 0 {
            return core::task::Poll::Ready(Ok(()));
        }
        if !self.read_buffer.is_empty() {
            let len = core::cmp::min(self.read_buffer.len(), buf.remaining());
            buf.put_slice(&self.read_buffer.split_to(len));
            return core::task::Poll::Ready(Ok(()));
        }
        match self.read_state {
            ReadState::Eof => return core::task::Poll::Ready(Ok(())),
            ReadState::FlushingCloseAcknowledgement => {
                return self.poll_flush_close_acknowledgement(cx);
            }
            ReadState::Open => {}
        }
        match futures::ready!(core::pin::Pin::new(&mut self.inner).poll_next(cx)) {
            Some(Ok(Message::Binary(frame))) if frame.is_empty() => {
                // Empty messages carry no stream bytes and are not EOF. Yield
                // after one so a hostile peer cannot monopolize this poll.
                cx.waker().wake_by_ref();
                core::task::Poll::Pending
            }
            Some(Ok(Message::Binary(frame))) => {
                self.read_buffer = frame;
                let len = core::cmp::min(self.read_buffer.len(), buf.remaining());
                buf.put_slice(&self.read_buffer.split_to(len));
                core::task::Poll::Ready(Ok(()))
            }
            Some(Ok(
                Message::Text(_) | Message::Ping(_) | Message::Pong(_) | Message::Frame(_),
            )) => {
                cx.waker().wake_by_ref();
                core::task::Poll::Pending
            }
            Some(Ok(Message::Close(_))) => {
                self.write_buffer.clear();
                self.shutdown = ShutdownState::Closed;
                self.read_state = ReadState::FlushingCloseAcknowledgement;
                self.poll_flush_close_acknowledgement(cx)
            }
            None => {
                self.write_buffer.clear();
                self.shutdown = ShutdownState::Closed;
                self.read_state = ReadState::Eof;
                core::task::Poll::Ready(Ok(()))
            }
            Some(Err(err)) => {
                core::task::Poll::Ready(Err(std::io::Error::other(format!("ws read error: {err}"))))
            }
        }
    }
}
impl<S> AsyncWrite for WsByteStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
        data: &[u8],
    ) -> core::task::Poll<std::io::Result<usize>> {
        if self.shutdown != ShutdownState::Open {
            return core::task::Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "websocket stream is shutting down",
            )));
        }
        if self.write_buffer.len() == CHUNK_BYTES {
            futures::ready!(self.poll_send_buffered(cx))?;
        }
        let accepted = data
            .len()
            .min(CHUNK_BYTES.saturating_sub(self.write_buffer.len()));
        self.write_buffer.extend_from_slice(&data[..accepted]);
        core::task::Poll::Ready(Ok(accepted))
    }
    fn poll_flush(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<std::io::Result<()>> {
        match self.shutdown {
            ShutdownState::Open | ShutdownState::FlushingForClose => self.poll_flush_buffered(cx),
            ShutdownState::Closing => core::task::Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "websocket stream is closing",
            ))),
            ShutdownState::Closed if self.read_state == ReadState::FlushingCloseAcknowledgement => {
                self.poll_flush_close_acknowledgement(cx)
            }
            ShutdownState::Closed => core::task::Poll::Ready(Ok(())),
        }
    }
    fn poll_shutdown(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<std::io::Result<()>> {
        if self.read_state == ReadState::FlushingCloseAcknowledgement {
            return self.poll_flush_close_acknowledgement(cx);
        }
        if self.shutdown == ShutdownState::Open {
            // Transition before draining so a cancelled shutdown cannot permit
            // additional writes on a stream already committed to closing.
            self.shutdown = ShutdownState::FlushingForClose;
        }
        if self.shutdown == ShutdownState::FlushingForClose {
            futures::ready!(self.poll_flush_buffered(cx))?;
            self.shutdown = ShutdownState::Closing;
        }
        if self.shutdown == ShutdownState::Closing {
            futures::ready!(core::pin::Pin::new(&mut self.inner).poll_close(cx))
                .map_err(|err| std::io::Error::other(format!("ws close error: {err}")))?;
            self.shutdown = ShutdownState::Closed;
        }
        core::task::Poll::Ready(Ok(()))
    }
}
/// Accept a `WebSocket` with production-equivalent allocation bounds and split
/// it into byte-oriented halves suitable for `NetworkHandle::accept_stream`.
///
/// # Errors
///
/// Returns an I/O error if the `WebSocket` server handshake fails.
pub(super) async fn accept_bounded<S>(
    stream: S,
) -> std::io::Result<(ReadHalf<WsByteStream<S>>, WriteHalf<WsByteStream<S>>)>
where
    S: AsyncRead + AsyncWrite + Send + Unpin,
{
    let websocket = accept_async_with_config(stream, Some(websocket_config()))
        .await
        .map_err(|err| std::io::Error::other(format!("ws accept error: {err}")))?;
    Ok(tokio::io::split(WsByteStream::new(websocket)))
}
#[cfg(test)]
mod tests {
    use core::{
        pin::Pin,
        task::{Context, Poll},
    };
    use futures::{SinkExt as _, StreamExt as _, task::noop_waker_ref};
    use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _, ReadBuf};
    use tokio_tungstenite::{WebSocketStream, client_async};
    use super::WsByteStream;
    async fn websocket_pair(
        capacity: usize,
    ) -> (
        WsByteStream<tokio::io::DuplexStream>,
        WebSocketStream<tokio::io::DuplexStream>,
    ) {
        let (client_io, server_io) = tokio::io::duplex(capacity);
        let client = tokio::spawn(async move {
            client_async("ws://localhost/p2p", client_io)
                .await
                .expect("client websocket handshake")
                .0
        });
        let server =
            tokio_tungstenite::accept_async_with_config(server_io, Some(super::websocket_config()))
                .await
                .expect("server websocket handshake");
        (
            WsByteStream::new(server),
            client.await.expect("client handshake task"),
        )
    }
    #[tokio::test]
    async fn empty_binary_message_is_not_stream_eof() {
        let (mut server, mut client) = websocket_pair(4_096).await;
        client
            .send(tokio_tungstenite::tungstenite::Message::Binary(
                Vec::new().into(),
            ))
            .await
            .expect("send empty binary message");
        client
            .send(tokio_tungstenite::tungstenite::Message::Binary(
                b"payload".to_vec().into(),
            ))
            .await
            .expect("send payload");
        let mut received = [0_u8; 7];
        server
            .read_exact(&mut received)
            .await
            .expect("read after empty binary message");
        assert_eq!(&received, b"payload");
    }
    #[tokio::test]
    async fn zero_capacity_read_is_immediately_ready() {
        let (mut server, _client) = websocket_pair(4_096).await;
        let mut empty = [];
        let mut read_buffer = ReadBuf::new(&mut empty);
        let mut cx = Context::from_waker(noop_waker_ref());
        assert!(matches!(
            Pin::new(&mut server).poll_read(&mut cx, &mut read_buffer),
            Poll::Ready(Ok(()))
        ));
        assert!(read_buffer.filled().is_empty());
    }
    #[tokio::test]
    async fn peer_close_is_acknowledged_before_sticky_eof() {
        let (mut server, mut client) = websocket_pair(4_096).await;
        client
            .send(tokio_tungstenite::tungstenite::Message::Close(None))
            .await
            .expect("send close frame");
        let mut byte = [0_u8; 1];
        let first_read =
            tokio::time::timeout(std::time::Duration::from_secs(1), server.read(&mut byte))
                .await
                .expect("server should flush the close acknowledgement");
        assert_eq!(first_read.expect("read peer close"), 0);
        let acknowledgement =
            tokio::time::timeout(std::time::Duration::from_secs(1), client.next())
                .await
                .expect("client should receive the close acknowledgement")
                .expect("close acknowledgement message")
                .expect("valid close acknowledgement");
        assert!(acknowledgement.is_close());
        let mut cx = Context::from_waker(noop_waker_ref());
        let mut read_buffer = ReadBuf::new(&mut byte);
        assert!(matches!(
            Pin::new(&mut server).poll_read(&mut cx, &mut read_buffer),
            Poll::Ready(Ok(()))
        ));
        assert!(read_buffer.filled().is_empty());
    }
    #[tokio::test]
    async fn pending_flush_retains_the_buffered_payload() {
        let (mut server, mut client) = websocket_pair(64).await;
        let payload = vec![0xA5; 1_024];
        server
            .write_all(&payload)
            .await
            .expect("buffer websocket payload");
        {
            let mut cx = Context::from_waker(noop_waker_ref());
            assert!(matches!(
                Pin::new(&mut server).poll_flush(&mut cx),
                Poll::Pending
            ));
        }
        let receiver = tokio::spawn(async move {
            client
                .next()
                .await
                .expect("websocket message")
                .expect("valid websocket message")
        });
        server.flush().await.expect("complete pending flush");
        let message = receiver.await.expect("receiver task");
        assert_eq!(message.into_data().as_ref(), payload.as_slice());
    }
    #[tokio::test]
    async fn cancelled_shutdown_rejects_subsequent_writes() {
        let (mut server, _client) = websocket_pair(64).await;
        server
            .write_all(&[0x5A; 1_024])
            .await
            .expect("buffer websocket payload");
        let mut cx = Context::from_waker(noop_waker_ref());
        assert!(matches!(
            Pin::new(&mut server).poll_shutdown(&mut cx),
            Poll::Pending
        ));
        let Poll::Ready(Err(error)) = Pin::new(&mut server).poll_write(&mut cx, b"late write")
        else {
            panic!("write after a cancelled shutdown must fail");
        };
        assert_eq!(error.kind(), std::io::ErrorKind::BrokenPipe);
    }
}
