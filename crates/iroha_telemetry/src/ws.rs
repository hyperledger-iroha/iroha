//! Telemetry sent to a server.
use crate::integrity::ChainState;
use crate::retry_period::RetryPeriod;
use chrono::Utc;
use eyre::{Result, eyre};
use futures::{Sink, SinkExt, StreamExt, stream::SplitSink};
use iroha_config::parameters::actual::{Telemetry as Config, TelemetryIntegrity};
use iroha_logger::telemetry::Event as Telemetry;
use norito::json::{Map, Value};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::{
    net::TcpStream,
    sync::{broadcast, mpsc},
    task::JoinHandle,
};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream,
    tungstenite::{Error, Message, protocol::WebSocketConfig},
};
use url::Url;
type WebSocketSplitSink = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;
const RECONNECT_CHANNEL_CAPACITY: usize = 4;
const INITIAL_CONNECTION_ID: u64 = 0;
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const SEND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
const INBOUND_BUFFER_BYTES: usize = 4 * 1024;
const INBOUND_MESSAGE_MAX_BYTES: usize = 1024;

#[derive(Debug)]
enum InternalMessage {
    Reconnect,
    RetryCheckpoint,
    Disconnected(u64),
}

struct WebsocketSink {
    write: WebSocketSplitSink,
    read_task: JoinHandle<()>,
}

impl Drop for WebsocketSink {
    fn drop(&mut self) {
        self.read_task.abort();
    }
}

impl Sink<Message> for WebsocketSink {
    type Error = Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.get_mut().write).poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
        Pin::new(&mut self.get_mut().write).start_send(item)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.get_mut().write).poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.get_mut().write).poll_close(cx)
    }
}

async fn connect_websocket(
    url: &Url,
    internal_sender: mpsc::Sender<InternalMessage>,
    connection_id: u64,
) -> Result<WebsocketSink> {
    let websocket_config = WebSocketConfig::default()
        .read_buffer_size(INBOUND_BUFFER_BYTES)
        .max_message_size(Some(INBOUND_MESSAGE_MAX_BYTES))
        .max_frame_size(Some(INBOUND_MESSAGE_MAX_BYTES));
    let (ws, _) =
        tokio_tungstenite::connect_async_with_config(url.as_str(), Some(websocket_config), false)
            .await?;
    let (write, mut read) = ws.split();
    let read_task = tokio::task::spawn(async move {
        while let Some(message) = read.next().await {
            match message {
                Ok(Message::Ping(_) | Message::Pong(_)) => {}
                Ok(Message::Close(_)) => break,
                Ok(Message::Text(_) | Message::Binary(_) | Message::Frame(_)) => {
                    iroha_logger::warn!(
                        "telemetry collector sent an unsupported WebSocket data frame"
                    );
                    break;
                }
                Err(error) => {
                    iroha_logger::debug!(%error, "telemetry WebSocket reader stopped");
                    break;
                }
            }
        }
        if internal_sender
            .send(InternalMessage::Disconnected(connection_id))
            .await
            .is_err()
        {
            iroha_logger::debug!("telemetry client stopped before disconnect notification");
        }
    });
    Ok(WebsocketSink { write, read_task })
}

/// Starts telemetry sending data to a server
/// # Errors
/// Fails if the integrity checkpoint cannot be loaded. Collector connections
/// are attempted by the background task and do not block node startup.
pub fn start(
    Config {
        url,
        max_retry_delay_exponent,
        min_retry_period,
        ..
    }: Config,
    integrity: TelemetryIntegrity,
    telemetry: broadcast::Receiver<Telemetry>,
) -> Result<JoinHandle<()>> {
    iroha_logger::info!("Starting telemetry exporter");
    let chain = ChainState::new_with_kind(integrity, "ws", url.as_str().as_bytes())?;
    let (internal_sender, internal_receiver) = mpsc::channel(RECONNECT_CHANNEL_CAPACITY);
    let client = Client::<WebsocketSink, _>::new(
        None,
        WebsocketSinkFactory::new(url, internal_sender.clone()),
        RetryPeriod::new(min_retry_period, max_retry_delay_exponent),
        internal_sender,
        chain,
        INITIAL_CONNECTION_ID,
    );
    let handle = tokio::task::spawn(async move {
        client.run(telemetry, internal_receiver).await;
    });
    Ok(handle)
}
struct Client<S, F> {
    sink_factory: F,
    retry_period: RetryPeriod,
    internal_sender: mpsc::Sender<InternalMessage>,
    sink: Option<S>,
    integrity: ChainState,
    connection_id: u64,
    reconnect_scheduled: bool,
    checkpoint_retry_scheduled: bool,
}
impl<S, F> Client<S, F>
where
    S: Sink<Message, Error = Error> + Send + Unpin,
    F: SinkFactory<Sink = S> + Send,
{
    fn new(
        sink: Option<S>,
        sink_factory: F,
        retry_period: RetryPeriod,
        internal_sender: mpsc::Sender<InternalMessage>,
        integrity: ChainState,
        connection_id: u64,
    ) -> Self {
        Self {
            sink_factory,
            retry_period,
            internal_sender,
            sink,
            integrity,
            connection_id,
            reconnect_scheduled: false,
            checkpoint_retry_scheduled: false,
        }
    }
    pub async fn run(
        mut self,
        mut receiver: broadcast::Receiver<Telemetry>,
        mut internal_receiver: mpsc::Receiver<InternalMessage>,
    ) {
        if self.sink.is_none() {
            self.on_reconnect().await;
        } else if self.integrity.pending_record().is_some() {
            self.send_pending().await;
        }
        loop {
            tokio::select! {
                msg = receiver.recv() => {
                    match msg {
                        Ok(msg) => {
                            self.on_telemetry(msg).await;
                        }
                        Err(broadcast::error::RecvError::Lagged(skipped)) => {
                            iroha_logger::warn!(
                                %skipped,
                                "telemetry channel lagged; dropped events"
                            );
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
                msg = internal_receiver.recv() => {
                    match msg {
                        Some(InternalMessage::Reconnect) => {
                            self.reconnect_scheduled = false;
                            if self.sink.is_none() {
                                self.on_reconnect().await;
                            }
                        }
                        Some(InternalMessage::RetryCheckpoint) => {
                            self.checkpoint_retry_scheduled = false;
                            self.on_checkpoint_retry().await;
                        }
                        Some(InternalMessage::Disconnected(connection_id)) => {
                            self.on_disconnected(connection_id);
                        }
                        None => break,
                    }
                }
            }
        }
    }
    async fn on_telemetry(&mut self, telemetry: Telemetry) {
        if self.sink.is_none() {
            return;
        }
        if self.integrity.pending_record().is_some() {
            return;
        }
        let map = build_message_map(telemetry);
        if let Err(error) = self.integrity.stage_record(map, false).await {
            iroha_logger::error!(%error, "failed to stage telemetry record");
            if self.integrity.pending_record().is_some() {
                self.schedule_checkpoint_retry();
            }
            return;
        }
        self.send_pending().await;
    }
    async fn on_reconnect(&mut self) {
        let Some(connection_id) = self.connection_id.checked_add(1) else {
            iroha_logger::error!("telemetry WebSocket connection identifier exhausted");
            return;
        };
        match self.sink_factory.create(connection_id).await {
            Ok(sink) => {
                iroha_logger::debug!("Reconnected telemetry");
                self.sink = Some(sink);
                self.connection_id = connection_id;
                if self.integrity.pending_record().is_some() {
                    if !self.integrity.pending_is_durable()
                        || self.integrity.pending_output_is_confirmed()
                    {
                        self.schedule_checkpoint_retry();
                    } else {
                        self.send_pending().await;
                    }
                } else {
                    self.retry_period.reset();
                }
            }
            Err(error) => {
                iroha_logger::warn!(%error, "failed to reconnect telemetry");
                self.schedule_reconnect();
            }
        }
    }
    fn on_disconnected(&mut self, connection_id: u64) {
        if self.connection_id != connection_id || self.sink.is_none() {
            return;
        }
        iroha_logger::debug!("telemetry WebSocket closed by peer");
        self.sink = None;
        self.schedule_reconnect();
    }
    async fn send_pending(&mut self) {
        if !self.integrity.pending_is_durable() || self.integrity.pending_output_is_confirmed() {
            self.schedule_checkpoint_retry();
            return;
        }
        let Some(bytes) = self.integrity.pending_record().map(<[u8]>::to_vec) else {
            return;
        };
        let Some(sink) = self.sink.as_mut() else {
            return;
        };
        match tokio::time::timeout(SEND_TIMEOUT, sink.send(Message::Binary(bytes.into()))).await {
            Ok(Ok(())) => {
                if let Err(error) = self.integrity.confirm_pending_output() {
                    iroha_logger::error!(%error, "failed to confirm telemetry output");
                    self.schedule_checkpoint_retry();
                    return;
                }
                if let Err(error) = self.integrity.commit_pending().await {
                    iroha_logger::error!(
                        %error,
                        "telemetry was sent but its integrity checkpoint could not be committed"
                    );
                    self.schedule_checkpoint_retry();
                } else {
                    self.retry_period.reset();
                }
            }
            Ok(Err(error)) => {
                if matches!(&error, Error::AlreadyClosed | Error::ConnectionClosed) {
                    iroha_logger::debug!("Closed connection to telemetry");
                } else {
                    iroha_logger::error!(%error, "send failed");
                }
                self.sink = None;
                self.schedule_reconnect();
            }
            Err(_) => {
                iroha_logger::warn!("telemetry send timed out");
                self.sink = None;
                self.schedule_reconnect();
            }
        }
    }
    async fn on_checkpoint_retry(&mut self) {
        if self.integrity.pending_record().is_none() {
            return;
        }
        if !self.integrity.pending_is_durable() {
            if let Err(error) = self.integrity.persist_pending().await {
                iroha_logger::error!(%error, "failed to persist staged telemetry record");
                self.schedule_checkpoint_retry();
                return;
            }
        }
        if self.integrity.pending_output_is_confirmed() {
            if let Err(error) = self.integrity.commit_pending().await {
                iroha_logger::error!(%error, "failed to commit telemetry integrity checkpoint");
                self.schedule_checkpoint_retry();
            } else {
                self.retry_period.reset();
            }
        } else if self.sink.is_some() {
            self.send_pending().await;
        }
    }
    fn schedule_reconnect(&mut self) {
        if self.reconnect_scheduled {
            return;
        }
        self.reconnect_scheduled = true;
        let period = self.retry_period.next_period();
        iroha_logger::debug!(
            "Scheduled reconnecting to telemetry in {} seconds",
            period.as_secs()
        );
        let sender = self.internal_sender.clone();
        tokio::task::spawn(async move {
            tokio::time::sleep(period).await;
            if sender.send(InternalMessage::Reconnect).await.is_err() {
                iroha_logger::debug!("telemetry reconnect task dropped; channel closed");
            }
        });
    }
    fn schedule_checkpoint_retry(&mut self) {
        if self.checkpoint_retry_scheduled {
            return;
        }
        self.checkpoint_retry_scheduled = true;
        let period = self.retry_period.next_period();
        let sender = self.internal_sender.clone();
        tokio::task::spawn(async move {
            tokio::time::sleep(period).await;
            if sender.send(InternalMessage::RetryCheckpoint).await.is_err() {
                iroha_logger::debug!("telemetry checkpoint retry task dropped; channel closed");
            }
        });
    }
}
fn build_message_map(telemetry: Telemetry) -> Map {
    let Telemetry { target, fields } = telemetry;
    let payload = fields
        .0
        .into_iter()
        .map(|(field, value)| (field.to_owned(), value))
        .collect();
    let now = Utc::now();
    let mut map = Map::new();
    map.insert("ts".into(), now.to_rfc3339().into());
    map.insert("target".into(), target.into());
    map.insert("payload".into(), Value::Object(payload));
    map
}
trait SinkFactory: Send {
    type Sink: Sink<Message, Error = Error> + Send + Unpin;
    fn create(&mut self, connection_id: u64) -> impl Future<Output = Result<Self::Sink>> + Send;
}
struct WebsocketSinkFactory {
    url: Url,
    internal_sender: mpsc::Sender<InternalMessage>,
}
impl WebsocketSinkFactory {
    #[inline]
    const fn new(url: Url, internal_sender: mpsc::Sender<InternalMessage>) -> Self {
        Self {
            url,
            internal_sender,
        }
    }
}
impl SinkFactory for WebsocketSinkFactory {
    type Sink = WebsocketSink;
    fn create(&mut self, connection_id: u64) -> impl Future<Output = Result<Self::Sink>> + Send {
        async move {
            tokio::time::timeout(
                CONNECT_TIMEOUT,
                connect_websocket(&self.url, self.internal_sender.clone(), connection_id),
            )
            .await
            .map_err(|_| eyre!("telemetry WebSocket connection timed out"))?
        }
    }
}
#[cfg(test)]
mod tests {
    use crate::{
        integrity::ChainState,
        ws::{
            Client, INITIAL_CONNECTION_ID, InternalMessage, RetryPeriod, SinkFactory,
            connect_websocket,
        },
    };
    use eyre::{Result, eyre};
    use futures::{Sink, SinkExt, StreamExt};
    use iroha_config::parameters::actual::{Telemetry as TelemetryConfig, TelemetryIntegrity};
    use iroha_logger::telemetry::{Event, Fields};
    use norito::json::{Map, Value};
    use std::{
        future::Future,
        pin::Pin,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        task::{Context, Poll},
        time::Duration,
    };
    use tokio::task::JoinHandle;
    use tokio_tungstenite::tungstenite::{Error, Message};
    use url::Url;
    #[test]
    fn message_map_preserves_events_without_msg_field() {
        let telemetry = Event {
            target: "test",
            fields: Fields(vec![
                ("event", Value::String("proof.completed".to_owned())),
                ("duration_ms", Value::Number(7_u64.into())),
            ]),
        };
        let map = super::build_message_map(telemetry);
        assert_eq!(map.get("target").and_then(Value::as_str), Some("test"));
        let payload = map
            .get("payload")
            .and_then(Value::as_object)
            .expect("payload");
        assert_eq!(
            payload.get("event").and_then(Value::as_str),
            Some("proof.completed")
        );
        assert_eq!(payload.get("duration_ms").and_then(Value::as_u64), Some(7));
    }

    #[tokio::test]
    async fn websocket_reader_handles_ping_and_reports_close() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let address = listener.local_addr().expect("listener address");
        let server = tokio::task::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept client");
            let mut websocket = tokio_tungstenite::accept_async(stream)
                .await
                .expect("accept WebSocket");
            websocket
                .send(Message::Ping(vec![1, 2, 3].into()))
                .await
                .expect("send ping");
            let response = tokio::time::timeout(Duration::from_secs(1), websocket.next())
                .await
                .expect("pong timeout")
                .expect("pong frame")
                .expect("read pong");
            assert_eq!(response, Message::Pong(vec![1, 2, 3].into()));
            websocket
                .send(Message::Close(None))
                .await
                .expect("send close");
        });
        let (internal_sender, mut internal_receiver) =
            tokio::sync::mpsc::channel(super::RECONNECT_CHANNEL_CAPACITY);
        let url = Url::parse(&format!("ws://{address}")).expect("WebSocket URL");
        let _sink = connect_websocket(&url, internal_sender, 42)
            .await
            .expect("connect client");
        server.await.expect("server task");
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(1), internal_receiver.recv())
                .await
                .expect("disconnect timeout"),
            Some(InternalMessage::Disconnected(42))
        ));
    }

    #[tokio::test]
    async fn websocket_reader_rejects_collector_data_frames() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let address = listener.local_addr().expect("listener address");
        let server = tokio::task::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept client");
            let mut websocket = tokio_tungstenite::accept_async(stream)
                .await
                .expect("accept WebSocket");
            websocket
                .send(Message::Text("collector payloads are unsupported".into()))
                .await
                .expect("send data frame");
        });
        let (internal_sender, mut internal_receiver) =
            tokio::sync::mpsc::channel(super::RECONNECT_CHANNEL_CAPACITY);
        let url = Url::parse(&format!("ws://{address}")).expect("WebSocket URL");
        let _sink = connect_websocket(&url, internal_sender, 43)
            .await
            .expect("connect client");
        server.await.expect("server task");
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(1), internal_receiver.recv())
                .await
                .expect("disconnect timeout"),
            Some(InternalMessage::Disconnected(43))
        ));
    }

    #[tokio::test]
    async fn websocket_reader_bounds_collector_frames() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let address = listener.local_addr().expect("listener address");
        let server = tokio::task::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept client");
            let mut websocket = tokio_tungstenite::accept_async(stream)
                .await
                .expect("accept WebSocket");
            websocket
                .send(Message::Binary(
                    vec![0_u8; super::INBOUND_MESSAGE_MAX_BYTES + 1].into(),
                ))
                .await
                .expect("send oversized frame");
        });
        let (internal_sender, mut internal_receiver) =
            tokio::sync::mpsc::channel(super::RECONNECT_CHANNEL_CAPACITY);
        let url = Url::parse(&format!("ws://{address}")).expect("WebSocket URL");
        let _sink = connect_websocket(&url, internal_sender, 44)
            .await
            .expect("connect client");
        server.await.expect("server task");
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(1), internal_receiver.recv())
                .await
                .expect("disconnect timeout"),
            Some(InternalMessage::Disconnected(44))
        ));
    }

    #[tokio::test]
    async fn unavailable_collector_does_not_block_exporter_startup() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("reserve address");
        let address = listener.local_addr().expect("listener address");
        drop(listener);
        let config = TelemetryConfig {
            url: Url::parse(&format!("ws://{address}")).expect("WebSocket URL"),
            min_retry_period: Duration::from_millis(10),
            max_retry_delay_exponent: 1,
            telegram_bot_key: None,
            telegram_chat_id: None,
            telegram_min_level: None,
            telegram_targets: None,
            telegram_rate_per_minute: None,
            telegram_include_metrics: false,
            telegram_allow_kinds: None,
            telegram_deny_kinds: None,
        };
        let (sender, receiver) = tokio::sync::broadcast::channel(1);
        let started_at = tokio::time::Instant::now();
        let handle = super::start(
            config,
            TelemetryIntegrity {
                enabled: false,
                state_dir: None,
                signing_key: None,
                signing_key_id: None,
            },
            receiver,
        )
        .expect("start exporter");
        assert!(
            started_at.elapsed() < Duration::from_millis(100),
            "start must not wait for collector"
        );
        drop(sender);
        tokio::time::timeout(Duration::from_secs(1), handle)
            .await
            .expect("exporter shutdown timeout")
            .expect("exporter task");
    }
    #[derive(Clone)]
    struct FallibleSender<T, F> {
        sender: futures::channel::mpsc::Sender<T>,
        before_send: F,
        fail_flush_once: Arc<AtomicBool>,
    }
    impl<T, F> FallibleSender<T, F> {
        fn new(sender: futures::channel::mpsc::Sender<T>, before_send: F) -> Self {
            Self {
                sender,
                before_send,
                fail_flush_once: Arc::new(AtomicBool::new(false)),
            }
        }

        fn with_flush_failure(mut self, fail_flush_once: Arc<AtomicBool>) -> Self {
            self.fail_flush_once = fail_flush_once;
            self
        }
    }
    impl<T, F> Sink<T> for FallibleSender<T, F>
    where
        F: FnMut() -> Result<(), Error> + Unpin,
    {
        type Error = Error;
        fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            let this = Pin::into_inner(self);
            match this.sender.poll_ready(cx) {
                Poll::Ready(r) => {
                    let result = (this.before_send)().map(|()| r.expect("failed to send"));
                    Poll::Ready(result)
                }
                Poll::Pending => Poll::Pending,
            }
        }
        fn start_send(self: Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
            let this = Pin::into_inner(self);
            // In this harness we surface failures via `before_send` to produce `E`.
            // The inner sink is a channel and should not fail here; if it does, panic with context.
            this.sender
                .start_send(item)
                .expect("unexpected inner sink error in start_send during telemetry test harness");
            Ok(())
        }
        fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            let this = Pin::into_inner(self);
            match Pin::new(&mut this.sender).poll_flush(cx) {
                Poll::Ready(Ok(())) if this.fail_flush_once.swap(false, Ordering::SeqCst) => {
                    Poll::Ready(Err(Error::ConnectionClosed))
                }
                Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
                Poll::Ready(Err(err)) => panic!(
                    "unexpected inner sink error in poll_flush during telemetry test harness: {err}"
                ),
                Poll::Pending => Poll::Pending,
            }
        }
        fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            let this = Pin::into_inner(self);
            match Pin::new(&mut this.sender).poll_close(cx) {
                Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
                Poll::Ready(Err(err)) => panic!(
                    "unexpected inner sink error in poll_close during telemetry test harness: {err}"
                ),
                Poll::Pending => Poll::Pending,
            }
        }
    }
    struct MockSinkFactory<F> {
        fail: Arc<AtomicBool>,
        sender: FallibleSender<Message, F>,
    }
    impl<F> SinkFactory for MockSinkFactory<F>
    where
        F: FnMut() -> Result<(), Error> + Clone + Send + Unpin,
    {
        type Sink = FallibleSender<Message, F>;
        fn create(
            &mut self,
            _connection_id: u64,
        ) -> impl Future<Output = Result<Self::Sink>> + Send {
            async move {
                if self.fail.load(Ordering::SeqCst) {
                    Err(eyre!("failed to create"))
                } else {
                    Ok(self.sender.clone())
                }
            }
        }
    }
    struct Suite {
        fail_send: Arc<AtomicBool>,
        fail_flush_once: Arc<AtomicBool>,
        fail_factory_create: Arc<AtomicBool>,
        telemetry_sender: tokio::sync::broadcast::Sender<Event>,
        message_receiver: futures::channel::mpsc::Receiver<Message>,
    }
    impl Suite {
        fn new() -> (Self, JoinHandle<()>) {
            Self::new_with_capacity(100)
        }
        fn new_with_capacity(channel_capacity: usize) -> (Self, JoinHandle<()>) {
            assert!(channel_capacity > 0, "channel capacity must be positive");
            let (telemetry_sender, telemetry_receiver) =
                tokio::sync::broadcast::channel(channel_capacity);
            let (message_sender, message_receiver) = futures::channel::mpsc::channel(100);
            let fail_send = Arc::new(AtomicBool::new(false));
            let fail_flush_once = Arc::new(AtomicBool::new(false));
            let message_sender = {
                let fail = Arc::clone(&fail_send);
                FallibleSender::new(message_sender, move || {
                    if fail.load(Ordering::SeqCst) {
                        Err(Error::ConnectionClosed)
                    } else {
                        Ok(())
                    }
                })
                .with_flush_failure(Arc::clone(&fail_flush_once))
            };
            let fail_factory_create = Arc::new(AtomicBool::new(false));
            let (internal_sender, internal_receiver) =
                tokio::sync::mpsc::channel(super::RECONNECT_CHANNEL_CAPACITY);
            let run_handle = {
                let client = Client::new(
                    Some(message_sender.clone()),
                    MockSinkFactory {
                        fail: Arc::clone(&fail_factory_create),
                        sender: message_sender,
                    },
                    RetryPeriod::new(Duration::from_secs(1), 0),
                    internal_sender,
                    ChainState::new_with_state_path(
                        TelemetryIntegrity {
                            enabled: true,
                            state_dir: None,
                            signing_key: None,
                            signing_key_id: None,
                        },
                        None,
                        "ws-test",
                        b"mock-sink",
                    )
                    .expect("initialize telemetry integrity chain"),
                    INITIAL_CONNECTION_ID,
                );
                tokio::task::spawn(async move {
                    client.run(telemetry_receiver, internal_receiver).await;
                })
            };
            let me = Self {
                fail_send,
                fail_flush_once,
                fail_factory_create,
                telemetry_sender,
                message_receiver,
            };
            (me, run_handle)
        }
    }
    fn system_interval_telemetry(peers: u64) -> Event {
        Event {
            target: "telemetry::test",
            fields: Fields(vec![
                ("msg", Value::String("system.interval".to_owned())),
                ("peers", Value::Number(peers.into())),
            ]),
        }
    }
    async fn send_succeeds_with_suite(suite: Suite) {
        let Suite {
            telemetry_sender,
            mut message_receiver,
            ..
        } = suite;
        telemetry_sender.send(system_interval_telemetry(1)).unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        let first_hash = {
            let msg = message_receiver.next().await.unwrap();
            let Message::Binary(bytes) = msg else {
                panic!("expected binary telemetry frame, got {msg:?}")
            };
            let map: Map = norito::json::from_slice(&bytes).unwrap();
            assert!(!map.contains_key("id"));
            assert!(map.contains_key("ts"));
            let chain = map.get("chain").and_then(Value::as_object).unwrap();
            assert_eq!(chain.get("seq").and_then(Value::as_u64), Some(1));
            assert_eq!(
                chain.get("prev_hash").and_then(Value::as_str),
                Some("0000000000000000000000000000000000000000000000000000000000000000")
            );
            let first_hash = chain
                .get("hash")
                .and_then(Value::as_str)
                .unwrap()
                .to_string();
            let payload = map.get("payload").unwrap().as_object().unwrap();
            assert_eq!(
                payload.get("msg"),
                Some(&Value::String("system.interval".to_owned()))
            );
            assert_eq!(payload.get("peers"), Some(&Value::Number(1_u64.into())));
            first_hash
        };
        // The second message is `update`
        telemetry_sender.send(system_interval_telemetry(2)).unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        {
            let msg = message_receiver.next().await.unwrap();
            let Message::Binary(bytes) = msg else {
                panic!("expected binary telemetry frame, got {msg:?}")
            };
            let map: Map = norito::json::from_slice(&bytes).unwrap();
            assert!(!map.contains_key("id"));
            assert!(map.contains_key("ts"));
            let chain = map.get("chain").and_then(Value::as_object).unwrap();
            assert_eq!(chain.get("seq").and_then(Value::as_u64), Some(2));
            assert_eq!(
                chain.get("prev_hash").and_then(Value::as_str),
                Some(first_hash.as_str())
            );
            assert!(map.contains_key("payload"));
            let payload = map.get("payload").unwrap().as_object().unwrap();
            assert_eq!(
                payload.get("msg"),
                Some(&Value::String("system.interval".to_owned()))
            );
            assert_eq!(payload.get("peers"), Some(&Value::Number(2_u64.into())));
        }
    }
    async fn reconnect_fails_with_suite(suite: Suite) {
        let Suite {
            fail_send,
            fail_factory_create,
            telemetry_sender,
            mut message_receiver,
            ..
        } = suite;
        // Fail sending the first message
        fail_send.store(true, Ordering::SeqCst);
        telemetry_sender.send(system_interval_telemetry(1)).unwrap();
        message_receiver.try_recv().unwrap_err();
        tokio::time::sleep(Duration::from_millis(100)).await;
        // The second message is not sent because the sink is reset
        fail_send.store(false, Ordering::SeqCst);
        telemetry_sender.send(system_interval_telemetry(1)).unwrap();
        message_receiver.try_recv().unwrap_err();
        tokio::time::sleep(Duration::from_millis(100)).await;
        // Fail the reconnection
        fail_factory_create.store(true, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_secs(1)).await;
        // The third message is not sent because the sink is not created yet
        telemetry_sender.send(system_interval_telemetry(1)).unwrap();
        message_receiver.try_recv().unwrap_err();
    }
    async fn send_after_reconnect_fails_with_suite(suite: Suite) {
        let Suite {
            fail_send,
            telemetry_sender,
            mut message_receiver,
            ..
        } = suite;
        // Fail sending the first message
        fail_send.store(true, Ordering::SeqCst);
        telemetry_sender.send(system_interval_telemetry(1)).unwrap();
        message_receiver.try_recv().unwrap_err();
        tokio::time::sleep(Duration::from_millis(100)).await;
        // The second message is not sent because the sink is reset
        fail_send.store(false, Ordering::SeqCst);
        telemetry_sender.send(system_interval_telemetry(1)).unwrap();
        message_receiver.try_recv().unwrap_err();
        tokio::time::sleep(Duration::from_millis(100)).await;
        // Fail sending the first message after reconnect
        fail_send.store(true, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_secs(1)).await;
        message_receiver.try_recv().unwrap_err();
        // The message is sent
        fail_send.store(false, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_secs(1)).await;
        let message = message_receiver.try_recv().unwrap();
        let Message::Binary(bytes) = message else {
            panic!("expected binary telemetry frame, got {message:?}")
        };
        let map: Map = norito::json::from_slice(&bytes).unwrap();
        assert_eq!(
            map.get("chain")
                .and_then(Value::as_object)
                .and_then(|chain| chain.get("seq"))
                .and_then(Value::as_u64),
            Some(1),
            "failed sends must not advance the integrity chain"
        );
    }
    async fn ambiguous_flush_failure_retries_identical_record_with_suite(suite: Suite) {
        let Suite {
            fail_flush_once,
            telemetry_sender,
            mut message_receiver,
            ..
        } = suite;
        fail_flush_once.store(true, Ordering::SeqCst);
        telemetry_sender.send(system_interval_telemetry(1)).unwrap();
        let first = tokio::time::timeout(Duration::from_secs(1), message_receiver.next())
            .await
            .expect("first frame timeout")
            .expect("first frame");
        let retry = tokio::time::timeout(Duration::from_secs(2), message_receiver.next())
            .await
            .expect("retry frame timeout")
            .expect("retry frame");
        assert_eq!(first, retry, "ambiguous sends must retry identical bytes");

        telemetry_sender.send(system_interval_telemetry(2)).unwrap();
        let next = tokio::time::timeout(Duration::from_secs(1), message_receiver.next())
            .await
            .expect("next frame timeout")
            .expect("next frame");
        let Message::Binary(bytes) = next else {
            panic!("expected binary telemetry frame")
        };
        let map: Map = norito::json::from_slice(&bytes).expect("parse next frame");
        assert_eq!(
            map.get("chain")
                .and_then(Value::as_object)
                .and_then(|chain| chain.get("seq"))
                .and_then(Value::as_u64),
            Some(2)
        );
    }
    async fn broadcast_lag_does_not_stop_client_with_suite(suite: Suite) {
        let Suite {
            telemetry_sender,
            mut message_receiver,
            ..
        } = suite;
        telemetry_sender.send(system_interval_telemetry(1)).unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        // Drain the first message so subsequent assertions focus on the lag burst.
        let _ = message_receiver.next().await.unwrap();
        // Flood the channel faster than the client can drain it to trigger lag handling.
        for peers in 0..200_u64 {
            telemetry_sender
                .send(system_interval_telemetry(peers))
                .unwrap();
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
        telemetry_sender
            .send(system_interval_telemetry(777))
            .unwrap();
        // Ensure the latest update still arrives even after the lag burst.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
        let mut received_latest = false;
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_millis(100), message_receiver.next()).await {
                Ok(Some(Message::Binary(bytes))) => {
                    let map: Map = norito::json::from_slice(&bytes).unwrap();
                    let Some(Value::Object(payload)) = map.get("payload") else {
                        continue;
                    };
                    if payload.get("msg").and_then(Value::as_str) == Some("system.interval")
                        && payload.get("peers").and_then(Value::as_u64) == Some(777)
                    {
                        received_latest = true;
                        break;
                    }
                }
                Ok(Some(_)) => {}
                Ok(None) | Err(_) => break,
            }
        }
        assert!(
            received_latest,
            "expected telemetry to continue after broadcast lag"
        );
    }
    macro_rules! test_with_suite {
        ($ident:ident, $future:ident) => {
            #[tokio::test]
            async fn $ident() {
                let (suite, run_handle) = Suite::new();
                $future(suite).await;
                run_handle.await.unwrap();
            }
        };
    }
    test_with_suite!(send_succeeds, send_succeeds_with_suite);
    test_with_suite!(reconnect_fails, reconnect_fails_with_suite);
    test_with_suite!(
        send_after_reconnect_fails,
        send_after_reconnect_fails_with_suite
    );
    test_with_suite!(
        ambiguous_flush_failure_retries_identical_record,
        ambiguous_flush_failure_retries_identical_record_with_suite
    );
    #[tokio::test]
    async fn broadcast_lag_does_not_stop_client() {
        let (suite, run_handle) = Suite::new_with_capacity(1);
        broadcast_lag_does_not_stop_client_with_suite(suite).await;
        run_handle.await.unwrap();
    }
}
