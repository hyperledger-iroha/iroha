//! Telemetry sent to a server

use chrono::Utc;
use eyre::{Result, eyre};
use futures::{Sink, SinkExt, StreamExt, stream::SplitSink};
use iroha_config::parameters::actual::{Telemetry as Config, TelemetryIntegrity};
use iroha_logger::telemetry::Event as Telemetry;
use norito::json::Map;
use tokio::{
    net::TcpStream,
    sync::{broadcast, mpsc},
    task::JoinHandle,
};
use tokio_stream::wrappers::{BroadcastStream, ReceiverStream, errors::BroadcastStreamRecvError};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream,
    tungstenite::{Error, Message},
};
use url::Url;

use crate::integrity::ChainState;
use crate::retry_period::RetryPeriod;

type WebSocketSplitSink = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;

const INTERNAL_CHANNEL_CAPACITY: usize = 10;

/// Starts telemetry sending data to a server
/// # Errors
/// Fails if unable to connect to the server
pub async fn start(
    Config {
        name,
        url,
        max_retry_delay_exponent,
        min_retry_period,
        ..
    }: Config,
    integrity: TelemetryIntegrity,
    telemetry: broadcast::Receiver<Telemetry>,
) -> Result<JoinHandle<()>> {
    iroha_logger::info!(%url, "Starting telemetry");
    let (ws, _) = tokio_tungstenite::connect_async(url.as_str()).await?;
    let (write, _read) = ws.split();
    let (internal_sender, internal_receiver) = mpsc::channel(INTERNAL_CHANNEL_CAPACITY);
    let client = Client::new(
        name,
        write,
        WebsocketSinkFactory::new(url),
        RetryPeriod::new(min_retry_period, max_retry_delay_exponent),
        internal_sender,
        ChainState::new_with_kind(integrity, "ws"),
    );
    let handle = tokio::task::spawn(async move {
        client.run(telemetry, internal_receiver).await;
    });

    Ok(handle)
}

struct Client<S, F> {
    name: String,
    sink_factory: F,
    retry_period: RetryPeriod,
    internal_sender: mpsc::Sender<InternalMessage>,
    sink: Option<S>,
    init_payload: Option<Map>,
    integrity: ChainState,
}

impl<S, F> Client<S, F>
where
    S: SinkExt<Message> + Sink<Message, Error = Error> + Send + Unpin,
    F: SinkFactory<Sink = S> + Send,
{
    pub fn new(
        name: String,
        sink: S,
        sink_factory: F,
        retry_period: RetryPeriod,
        internal_sender: mpsc::Sender<InternalMessage>,
        integrity: ChainState,
    ) -> Self {
        Self {
            name,
            sink_factory,
            retry_period,
            internal_sender,
            sink: Some(sink),
            init_payload: None,
            integrity,
        }
    }

    pub async fn run(
        mut self,
        receiver: broadcast::Receiver<Telemetry>,
        internal_receiver: mpsc::Receiver<InternalMessage>,
    ) {
        let mut stream = BroadcastStream::new(receiver).fuse();
        let mut internal_stream = ReceiverStream::new(internal_receiver).fuse();
        loop {
            tokio::select! {
                msg = stream.next() => {
                    match msg {
                        Some(Ok(msg)) => self.on_telemetry(msg).await,
                        Some(Err(error)) => match error {
                            BroadcastStreamRecvError::Lagged(skipped) => {
                                iroha_logger::warn!(
                                    %skipped,
                                    "telemetry channel lagged; dropped events"
                                );
                            }
                        },
                        None => break,
                    }
                }
                msg = internal_stream.next() => {
                    if matches!(msg, Some(InternalMessage::Reconnect)) {
                        self.on_reconnect().await;
                    }
                }
            }
        }
    }

    async fn on_telemetry(&mut self, telemetry: Telemetry) {
        match prepare_message(&self.name, telemetry, &mut self.integrity) {
            Ok((msg, msg_kind, init_payload)) => {
                if matches!(msg_kind, Some(MessageKind::Initialization)) {
                    self.init_payload = init_payload;
                }
                self.send_message(msg).await;
            }
            Err(error) => {
                iroha_logger::error!(%error, "prepare_message failed");
            }
        }
    }

    async fn on_reconnect(&mut self) {
        if let Ok(sink) = self.sink_factory.create().await {
            if let Some(payload) = self.init_payload.clone() {
                iroha_logger::debug!("Reconnected telemetry");
                self.sink = Some(sink);
                match build_message(&payload, &mut self.integrity) {
                    Ok(msg) => self.send_message(msg).await,
                    Err(error) => {
                        iroha_logger::error!(%error, "Failed to rebuild telemetry init payload");
                    }
                }
            } else {
                // The reconnect is required if sending a message fails.
                // The first message to be sent is initialization.
                // The path is assumed to be unreachable.
                iroha_logger::error!(
                    "Cannot reconnect telemetry because there is no initialization message"
                );
            }
        } else {
            self.schedule_reconnect();
        }
    }

    async fn send_message(&mut self, msg: Message) {
        if let Some(sink) = self.sink.as_mut() {
            match sink.send(msg).await {
                Ok(()) => {}
                Err(Error::AlreadyClosed | Error::ConnectionClosed) => {
                    iroha_logger::debug!("Closed connection to telemetry");
                    self.sink = None;
                    self.schedule_reconnect();
                }
                Err(error) => {
                    iroha_logger::error!(%error, "send failed");
                }
            }
        }
    }

    fn schedule_reconnect(&mut self) {
        self.retry_period.increase_exponent();
        let period = self.retry_period.period();
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
}

#[derive(Debug)]
enum InternalMessage {
    Reconnect,
}

fn prepare_message(
    name: &str,
    telemetry: Telemetry,
    integrity: &mut ChainState,
) -> Result<(Message, Option<MessageKind>, Option<Map>)> {
    let (payload, msg_kind) = build_payload(name, telemetry)?;
    let msg = build_message(&payload, integrity)?;
    let init_payload = if matches!(msg_kind, Some(MessageKind::Initialization)) {
        Some(payload)
    } else {
        None
    };
    Ok((msg, msg_kind, init_payload))
}

fn build_payload(name: &str, telemetry: Telemetry) -> Result<(Map, Option<MessageKind>)> {
    let mut msg_kind: Option<MessageKind> = None;
    let mut msg_field_present = false;
    let mut payload = Map::new();

    for (field, value) in telemetry.fields.0 {
        if field == "msg" {
            msg_field_present = true;
            let kind = value
                .as_str()
                .ok_or_else(|| eyre!("Failed to read 'msg'"))?
                .trim();
            msg_kind = match kind {
                "system.connected" => Some(MessageKind::Initialization),
                _ => None,
            };
        }

        let processed = match field {
            "genesis_hash" | "best" | "finalized_hash" => {
                let hash = value
                    .as_str()
                    .ok_or_else(|| eyre!("invalid or missing hash string for `{field}`"))?;
                let mut prefixed = String::with_capacity(hash.len().saturating_add(2));
                prefixed.push_str("0x");
                prefixed.push_str(hash);
                prefixed.into()
            }
            _ => value,
        };

        payload.insert(field.to_owned(), processed);
    }

    if !msg_field_present {
        return Err(eyre!("Failed to read 'msg'"));
    }

    if matches!(msg_kind, Some(MessageKind::Initialization)) {
        let now = Utc::now();
        payload.insert("name".into(), name.into());
        payload.insert("chain".into(), "Iroha".into());
        payload.insert("implementation".into(), "".into());
        let vergen_git_sha = option_env!("VERGEN_GIT_SHA").unwrap_or("unknown");
        let vergen_target = option_env!("VERGEN_CARGO_TARGET_TRIPLE").unwrap_or("unknown");
        payload.insert(
            "version".into(),
            format!(
                "{}-{vergen_git_sha}-{vergen_target}",
                env!("CARGO_PKG_VERSION")
            )
            .into(),
        );
        payload.insert("config".into(), "".into());
        payload.insert("authority".into(), false.into());
        payload.insert(
            "startup_time".into(),
            now.timestamp_millis().to_string().into(),
        );
        payload.insert("network_id".into(), "".into());
    }

    Ok((payload, msg_kind))
}

fn build_message(payload: &Map, integrity: &mut ChainState) -> Result<Message> {
    let now = Utc::now();
    let mut map = Map::new();
    map.insert("id".into(), 0_i32.into());
    map.insert("ts".into(), now.to_rfc3339().into());
    map.insert("payload".into(), payload.clone().into());
    integrity.attach_chain(&mut map)?;
    let msg = Message::Binary(norito::json::to_vec(&map)?.into());
    Ok(msg)
}

#[derive(Debug, Clone, Copy)]
enum MessageKind {
    Initialization,
}

#[async_trait::async_trait]
trait SinkFactory {
    type Sink: SinkExt<Message> + Sink<Message, Error = Error> + Send + Unpin;

    async fn create(&mut self) -> Result<Self::Sink>;
}

struct WebsocketSinkFactory {
    url: Url,
}

impl WebsocketSinkFactory {
    #[inline]
    pub const fn new(url: Url) -> Self {
        Self { url }
    }
}

#[async_trait::async_trait]
impl SinkFactory for WebsocketSinkFactory {
    type Sink = WebSocketSplitSink;

    async fn create(&mut self) -> Result<Self::Sink> {
        let (ws, _) = tokio_tungstenite::connect_async(self.url.as_str()).await?;
        let (write, _) = ws.split();
        Ok(write)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        pin::Pin,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        task::{Context, Poll},
        time::Duration,
    };

    use eyre::{Result, eyre};
    use futures::{Sink, StreamExt};
    use iroha_config::parameters::actual::TelemetryIntegrity;
    use iroha_logger::telemetry::{Event, Fields};
    use norito::json::{Map, Value};
    use tokio::task::JoinHandle;
    use tokio_tungstenite::tungstenite::{Error, Message};

    use crate::{
        integrity::ChainState,
        ws::{Client, RetryPeriod, SinkFactory},
    };

    #[test]
    fn prepare_message_fails_on_invalid_hash_fields() {
        for field in ["genesis_hash", "best", "finalized_hash"] {
            let telemetry = Event {
                target: "telemetry::test",
                fields: Fields(vec![
                    ("msg", Value::String("system.connected".to_owned())),
                    (field, Value::Bool(true)),
                ]),
            };
            let mut integrity = ChainState::new_with_state_path(
                TelemetryIntegrity {
                    enabled: true,
                    state_dir: None,
                    signing_key: None,
                    signing_key_id: None,
                },
                None,
            );
            assert!(
                super::prepare_message("node", telemetry, &mut integrity).is_err(),
                "expected error for field {field}",
            );
        }
    }

    #[derive(Clone)]
    pub struct FallibleSender<T, F> {
        sender: futures::channel::mpsc::Sender<T>,
        before_send: F,
    }

    impl<T, F> FallibleSender<T, F> {
        pub fn new(sender: futures::channel::mpsc::Sender<T>, before_send: F) -> Self {
            Self {
                sender,
                before_send,
            }
        }
    }

    impl<T, E, F> Sink<T> for FallibleSender<T, F>
    where
        F: FnMut() -> Result<(), E> + Unpin,
    {
        type Error = E;

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

    #[async_trait::async_trait]
    impl<F> SinkFactory for MockSinkFactory<F>
    where
        F: FnMut() -> Result<(), Error> + Clone + Send + Unpin,
    {
        type Sink = FallibleSender<Message, F>;

        async fn create(&mut self) -> Result<Self::Sink> {
            if self.fail.load(Ordering::SeqCst) {
                Err(eyre!("failed to create"))
            } else {
                Ok(self.sender.clone())
            }
        }
    }

    struct Suite {
        fail_send: Arc<AtomicBool>,
        fail_factory_create: Arc<AtomicBool>,
        telemetry_sender: tokio::sync::broadcast::Sender<Event>,
        message_receiver: futures::channel::mpsc::Receiver<Message>,
    }

    impl Suite {
        pub fn new() -> (Self, JoinHandle<()>) {
            Self::new_with_capacity(100)
        }

        pub fn new_with_capacity(channel_capacity: usize) -> (Self, JoinHandle<()>) {
            assert!(channel_capacity > 0, "channel capacity must be positive");
            let (telemetry_sender, telemetry_receiver) =
                tokio::sync::broadcast::channel(channel_capacity);
            let (message_sender, message_receiver) = futures::channel::mpsc::channel(100);
            let fail_send = Arc::new(AtomicBool::new(false));
            let message_sender = {
                let fail = Arc::clone(&fail_send);
                FallibleSender::new(message_sender, move || {
                    if fail.load(Ordering::SeqCst) {
                        Err(Error::ConnectionClosed)
                    } else {
                        Ok(())
                    }
                })
            };
            let fail_factory_create = Arc::new(AtomicBool::new(false));
            let (internal_sender, internal_receiver) = tokio::sync::mpsc::channel(10);
            let run_handle = {
                let client = Client::new(
                    "node".to_owned(),
                    message_sender.clone(),
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
                    ),
                );
                tokio::task::spawn(async move {
                    client.run(telemetry_receiver, internal_receiver).await;
                })
            };
            let me = Self {
                fail_send,
                fail_factory_create,
                telemetry_sender,
                message_receiver,
            };
            (me, run_handle)
        }
    }

    fn system_connected_telemetry() -> Event {
        Event {
            target: "telemetry::test",
            fields: Fields(vec![
                ("msg", Value::String("system.connected".to_owned())),
                (
                    "genesis_hash",
                    Value::String("00000000000000000000000000000000".to_owned()),
                ),
            ]),
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

        // The first message is `initialization`
        telemetry_sender.send(system_connected_telemetry()).unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        let first_hash = {
            let msg = message_receiver.next().await.unwrap();
            let Message::Binary(bytes) = msg else {
                panic!("expected binary telemetry frame, got {msg:?}")
            };
            let map: Map = norito::json::from_slice(&bytes).unwrap();
            assert_eq!(map.get("id"), Some(&Value::Number(0_u64.into())));
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
                Some(&Value::String("system.connected".to_owned()))
            );
            assert_eq!(
                payload.get("genesis_hash"),
                Some(&Value::String(
                    "0x00000000000000000000000000000000".to_owned()
                ))
            );
            assert!(payload.contains_key("chain"));
            assert!(payload.contains_key("implementation"));
            assert!(payload.contains_key("version"));
            assert!(payload.contains_key("config"));
            assert!(payload.contains_key("authority"));
            assert!(payload.contains_key("startup_time"));
            assert!(payload.contains_key("network_id"));
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
            assert_eq!(map.get("id"), Some(&Value::Number(0_u64.into())));
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
        } = suite;

        // Fail sending the first message
        fail_send.store(true, Ordering::SeqCst);
        telemetry_sender.send(system_connected_telemetry()).unwrap();
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
        telemetry_sender.send(system_connected_telemetry()).unwrap();
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
        message_receiver.try_recv().unwrap();
    }

    async fn broadcast_lag_does_not_stop_client_with_suite(suite: Suite) {
        let Suite {
            telemetry_sender,
            mut message_receiver,
            ..
        } = suite;

        telemetry_sender.send(system_connected_telemetry()).unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        // Drain the initialization message so subsequent assertions focus on interval telemetry.
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

    #[tokio::test]
    async fn broadcast_lag_does_not_stop_client() {
        let (suite, run_handle) = Suite::new_with_capacity(1);
        broadcast_lag_does_not_stop_client_with_suite(suite).await;
        run_handle.await.unwrap();
    }
}
