//! Adversarial transport and lifecycle tests for canonical account streams.

use super::*;
use crate::stream::{StreamConnectFuture, StreamConnection, StreamTransport};
use futures_util::{Sink, StreamExt};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

#[derive(Debug, Default)]
struct Observed {
    requests: Mutex<Vec<StreamRequest>>,
    sent: Mutex<Vec<Vec<u8>>>,
    drops: AtomicUsize,
    closes: AtomicUsize,
}

#[derive(Debug)]
struct TestTransport {
    observed: Arc<Observed>,
    frames: Mutex<VecDeque<Result<StreamFrame>>>,
    pending_connect: bool,
    pending_send: bool,
    pending_close: bool,
    read_termination: ReadTermination,
    status: u16,
    protocols: Vec<&'static str>,
    pending_read: Mutex<Option<PendingRead>>,
}

#[derive(Clone, Copy, Debug)]
enum ReadTermination {
    Pending,
    EndOfStream,
}

#[derive(Debug)]
struct PendingRead {
    started: Option<std::sync::mpsc::Sender<()>>,
    finish: tokio::sync::oneshot::Receiver<()>,
}

impl TestTransport {
    fn new(frames: impl IntoIterator<Item = Result<StreamFrame>>) -> Self {
        Self {
            observed: Arc::default(),
            frames: Mutex::new(frames.into_iter().collect()),
            pending_connect: false,
            pending_send: false,
            pending_close: false,
            read_termination: ReadTermination::Pending,
            status: 101,
            protocols: vec![NORITO_V1_WEBSOCKET_SUBPROTOCOL],
            pending_read: Mutex::new(None),
        }
    }
}

struct TestSocket {
    observed: Arc<Observed>,
    frames: VecDeque<Result<StreamFrame>>,
    pending_send: bool,
    pending_close: bool,
    read_termination: ReadTermination,
    pending_read: Option<PendingRead>,
}

impl Drop for TestSocket {
    fn drop(&mut self) {
        self.observed.drops.fetch_add(1, Ordering::SeqCst);
    }
}

impl Stream for TestSocket {
    type Item = Result<StreamFrame>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(frame) = self.frames.pop_front() {
            Poll::Ready(Some(frame))
        } else if matches!(self.read_termination, ReadTermination::EndOfStream) {
            Poll::Ready(None)
        } else if let Some(control) = &mut self.pending_read {
            if let Some(started) = control.started.take() {
                started.send(()).unwrap();
            }
            futures_util::ready!(std::future::Future::poll(Pin::new(&mut control.finish), cx))
                .unwrap();
            Poll::Ready(Some(Ok(StreamFrame::Close {
                code: Some(1000),
                reason: String::new(),
            })))
        } else {
            Poll::Pending
        }
    }
}

impl Sink<Vec<u8>> for TestSocket {
    type Error = Error;
    fn poll_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<()>> {
        if self.pending_send {
            Poll::Pending
        } else {
            Poll::Ready(Ok(()))
        }
    }
    fn start_send(self: Pin<&mut Self>, item: Vec<u8>) -> Result<()> {
        self.observed.sent.lock().unwrap().push(item);
        Ok(())
    }
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<()>> {
        Poll::Ready(Ok(()))
    }
    fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<()>> {
        self.observed.closes.fetch_add(1, Ordering::SeqCst);
        if self.pending_close {
            Poll::Pending
        } else {
            Poll::Ready(Ok(()))
        }
    }
}

impl StreamTransport for TestTransport {
    fn connect(&self, request: StreamRequest) -> StreamConnectFuture<'_> {
        Box::pin(async move {
            self.observed.requests.lock().unwrap().push(request);
            if self.pending_connect {
                std::future::pending::<()>().await;
            }
            let mut response = http::Response::builder().status(self.status);
            for protocol in &self.protocols {
                response = response.header(http::header::SEC_WEBSOCKET_PROTOCOL, *protocol);
            }
            Ok(StreamConnection {
                response: response.body(Vec::new()).unwrap(),
                socket: Box::new(TestSocket {
                    observed: Arc::clone(&self.observed),
                    frames: std::mem::take(&mut *self.frames.lock().unwrap()),
                    pending_send: self.pending_send,
                    pending_close: self.pending_close,
                    read_termination: self.read_termination,
                    pending_read: self.pending_read.lock().unwrap().take(),
                }),
            })
        })
    }
}

fn account(transport: Arc<TestTransport>) -> AccountClient {
    let mut builder = super::super::evidence_http_tests::client_with_base_url(
        "http://127.0.0.1:8080/".parse().unwrap(),
    )
    .to_builder()
    .stream_transport(transport);
    builder.torii_request_timeout = Duration::from_millis(30);
    builder.build().unwrap().account_client().unwrap()
}

fn filter() -> EventFilterBox {
    crate::data_model::events::pipeline::TransactionEventFilter::default().into()
}

fn event() -> EventBox {
    use crate::data_model::{
        block::BlockHeader,
        events::pipeline::{PipelineEventBox, PipelineWarning},
    };
    EventBox::Pipeline(PipelineEventBox::Warning(PipelineWarning {
        header: BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0),
        kind: "test".to_owned(),
        details: "canonical event".to_owned(),
    }))
}

fn encoded_event() -> Vec<u8> {
    norito::to_bytes(&crate::data_model::events::stream::EventMessage(event())).unwrap()
}

#[test]
fn selected_subprotocol_must_match_exactly() {
    for protocols in [
        vec![NORITO_V1_WEBSOCKET_SUBPROTOCOL],
        vec![],
        vec!["IROHA-NORITO-V1"],
        vec!["other-protocol"],
        vec![
            NORITO_V1_WEBSOCKET_SUBPROTOCOL,
            NORITO_V1_WEBSOCKET_SUBPROTOCOL,
        ],
    ] {
        let mut response = http::Response::builder().status(101);
        for protocol in &protocols {
            response = response.header(http::header::SEC_WEBSOCKET_PROTOCOL, *protocol);
        }
        let result = validate_upgrade(EVENTS_OPERATION, &response.body(Vec::new()).unwrap());
        if protocols == [NORITO_V1_WEBSOCKET_SUBPROTOCOL] {
            result.expect("canonical protocol");
        } else {
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("did not select required subprotocol")
            );
        }
    }
}

#[tokio::test]
async fn injected_event_stream_signs_exact_upgrade_and_subscribes_once() {
    let transport = Arc::new(TestTransport::new([
        Ok(StreamFrame::Binary(encoded_event())),
        Ok(StreamFrame::Close {
            code: Some(1000),
            reason: String::new(),
        }),
    ]));
    let account = account(Arc::clone(&transport));
    let mut stream = account.events().subscribe([filter()]).await.unwrap();
    assert_eq!(stream.next().await.unwrap().unwrap(), event());
    assert!(stream.next().await.is_none());
    assert!(stream.next().await.is_none());
    {
        let requests = transport.observed.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert_eq!(request.operation, EVENTS_OPERATION);
        assert_eq!(
            request.request.uri().path(),
            iroha_torii_shared::uri::SUBSCRIPTION
        );
        let snapshot = crate::http_default::RequestSnapshot {
            method: request.request.method().clone(),
            url: request.request.uri().to_string().parse().unwrap(),
            headers: request
                .request
                .headers()
                .iter()
                .map(|(n, v)| (n.to_string(), v.to_str().unwrap().to_owned()))
                .collect(),
            body: Vec::new(),
            timeout: Some(request.timeout),
            max_response_bytes: request.max_message_bytes,
            direct_loopback: false,
        };
        super::super::tests::assert_canonical_account_signed_request(&account.context, &snapshot);
        assert_eq!(request.max_message_bytes, MESSAGE_MAX_BYTES);
    }
    let sent = transport.observed.sent.lock().unwrap();
    assert_eq!(sent.len(), 1);
    let request: crate::data_model::events::stream::EventSubscriptionRequest =
        norito::decode_from_bytes(&sent[0]).unwrap();
    assert_eq!(request.filters, vec![filter()]);
    assert!(request.proof_backend.is_none());
    drop(sent);
    drop(stream);
    assert_eq!(transport.observed.drops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn block_stream_preserves_start_height_and_exact_authority() {
    let transport = Arc::new(TestTransport::new([]));
    let account = account(Arc::clone(&transport));
    let stream = account
        .blocks()
        .subscribe(NonZeroU64::new(37).unwrap())
        .await
        .unwrap();
    {
        let sent = transport.observed.sent.lock().unwrap();
        let request: crate::data_model::block::stream::BlockSubscriptionRequest =
            norito::decode_from_bytes(&sent[0]).unwrap();
        assert_eq!(request.0.get(), 37);
    }
    assert_eq!(
        transport.observed.requests.lock().unwrap()[0].operation,
        BLOCKS_OPERATION
    );
    stream.close().await.unwrap();
    assert_eq!(transport.observed.closes.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn filters_and_upgrade_rejection_never_send_a_subscription() {
    let empty = Arc::new(TestTransport::new([]));
    assert!(matches!(
        account(Arc::clone(&empty))
            .events()
            .subscribe(Vec::<EventFilterBox>::new())
            .await,
        Err(Error::InvalidRequest { .. })
    ));
    assert!(empty.observed.requests.lock().unwrap().is_empty());
    for status in [302, 401, 403, 429] {
        let mut transport = TestTransport::new([]);
        transport.status = status;
        let transport = Arc::new(transport);
        let result = account(Arc::clone(&transport))
            .events()
            .subscribe([filter()])
            .await;
        assert!(matches!(result,Err(Error::Http{status:actual,..}) if actual==status));
        assert_eq!(transport.observed.requests.lock().unwrap().len(), 1);
        assert!(transport.observed.sent.lock().unwrap().is_empty());
        assert_eq!(transport.observed.drops.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn connect_and_initial_send_deadlines_cancel_without_replay() {
    for connect in [true, false] {
        let mut transport = TestTransport::new([]);
        transport.pending_connect = connect;
        transport.pending_send = !connect;
        let transport = Arc::new(transport);
        let account = account(Arc::clone(&transport));
        let timer = async {
            tokio::time::sleep(Duration::from_millis(5)).await;
            42
        };
        let (result, responsive) = tokio::join!(account.events().subscribe([filter()]), timer);
        assert_eq!(responsive, 42);
        assert!(matches!(
            result,
            Err(Error::Timeout {
                operation: EVENTS_OPERATION
            })
        ));
        assert_eq!(transport.observed.requests.lock().unwrap().len(), 1);
        assert!(transport.observed.sent.lock().unwrap().is_empty());
        assert_eq!(
            transport.observed.drops.load(Ordering::SeqCst),
            usize::from(!connect)
        );
    }
}

#[tokio::test]
async fn malformed_oversized_and_transport_errors_are_terminal() {
    for (frame, kind) in [
        (Ok(StreamFrame::Binary(vec![0; 9])), 0),
        (Ok(StreamFrame::Binary(vec![0])), 1),
        (
            Err(Error::Transport {
                operation: EVENTS_OPERATION,
                kind: crate::TransportErrorKind::Io(std::io::ErrorKind::ConnectionReset),
                details: "reset".to_owned(),
            }),
            2,
        ),
    ] {
        let transport = Arc::new(TestTransport::new([
            frame,
            Ok(StreamFrame::Binary(encoded_event())),
        ]));
        let mut stream = account(Arc::clone(&transport))
            .events()
            .subscribe([filter()])
            .await
            .unwrap();
        stream.maximum = 8;
        let error = stream.next().await.unwrap().unwrap_err();
        match kind {
            0 => assert!(matches!(
                error,
                Error::ResponseTooLarge {
                    maximum: 8,
                    actual: Some(9)
                }
            )),
            1 => assert!(matches!(error, Error::Decode { .. })),
            _ => assert!(matches!(error, Error::Transport { .. })),
        }
        assert!(stream.next().await.is_none());
        assert_eq!(transport.observed.requests.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn policy_backpressure_and_missing_close_preserve_disposition() {
    for code in [Some(1008), Some(1013), None] {
        let transport = Arc::new(TestTransport::new([Ok(StreamFrame::Close {
            code,
            reason: "peer diagnostic".to_owned(),
        })]));
        let mut stream = account(transport)
            .events()
            .subscribe([filter()])
            .await
            .unwrap();
        assert_eq!(
            stream.next().await.unwrap().unwrap_err(),
            Error::StreamClosed {
                operation: EVENTS_OPERATION,
                code,
                reason: "peer diagnostic".to_owned()
            }
        );
        assert!(stream.next().await.is_none());
    }
    let mut transport = TestTransport::new([]);
    transport.read_termination = ReadTermination::EndOfStream;
    let mut stream = account(Arc::new(transport))
        .events()
        .subscribe([filter()])
        .await
        .unwrap();
    assert!(matches!(
        stream.next().await,
        Some(Err(Error::StreamClosed { code: None, .. }))
    ));
}

#[tokio::test]
async fn close_timeout_drops_the_owned_connection() {
    let mut transport = TestTransport::new([]);
    transport.pending_close = true;
    let transport = Arc::new(transport);
    let stream = account(Arc::clone(&transport))
        .events()
        .subscribe([filter()])
        .await
        .unwrap();
    assert_eq!(
        stream.close().await,
        Err(Error::Timeout {
            operation: EVENTS_OPERATION
        })
    );
    assert_eq!(transport.observed.drops.load(Ordering::SeqCst), 1);
}

#[test]
fn blocking_stream_reuses_runtime_and_distinguishes_wait_timeout_from_eof() {
    let transport = Arc::new(TestTransport::new([Ok(StreamFrame::Binary(
        encoded_event(),
    ))]));
    let facade =
        crate::blocking::AccountClient::from_client(account(Arc::clone(&transport))).unwrap();
    let mut stream = facade.events().subscribe([filter()]).unwrap();
    assert_eq!(
        stream.recv(Some(Duration::from_millis(5))).unwrap(),
        Some(event())
    );
    assert_eq!(
        stream.recv(Some(Duration::from_millis(5))),
        Err(Error::Timeout {
            operation: "stream.receive"
        })
    );
    assert_eq!(
        stream.recv(Some(Duration::from_millis(5))),
        Err(Error::Timeout {
            operation: "stream.receive"
        })
    );
    stream.close().unwrap();
    assert_eq!(transport.observed.requests.lock().unwrap().len(), 1);
    assert_eq!(transport.observed.closes.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn blocking_stream_subscribe_rejects_async_runtime_before_dispatch() {
    let transport = Arc::new(TestTransport::new([]));
    let facade =
        crate::blocking::AccountClient::from_client(account(Arc::clone(&transport))).unwrap();
    assert!(matches!(
        facade.events().subscribe([filter()]),
        Err(Error::Blocking(_))
    ));
    assert!(matches!(
        facade.blocks().subscribe(NonZeroU64::new(1).unwrap()),
        Err(Error::Blocking(_))
    ));
    assert!(transport.observed.requests.lock().unwrap().is_empty());
    drop(facade);
}

#[test]
fn pending_blocking_stream_does_not_serialize_sibling_subscription_receive_or_close() {
    let transport = Arc::new(TestTransport::new([]));
    let (started, waiting) = std::sync::mpsc::channel();
    let (finish, closed) = tokio::sync::oneshot::channel();
    *transport.pending_read.lock().unwrap() = Some(PendingRead {
        started: Some(started),
        finish: closed,
    });
    let facade =
        crate::blocking::AccountClient::from_client(account(Arc::clone(&transport))).unwrap();
    let mut pending = facade.events().subscribe([filter()]).unwrap();
    let first = std::thread::spawn(move || pending.recv(None));
    let entered = waiting.recv_timeout(Duration::from_secs(5));

    transport
        .frames
        .lock()
        .unwrap()
        .push_back(Ok(StreamFrame::Binary(encoded_event())));
    let (completed, completion) = std::sync::mpsc::channel();
    let sibling = std::thread::spawn(move || {
        let result = (|| -> Result<_> {
            let mut stream = facade.events().subscribe([filter()])?;
            let value = stream.recv(Some(Duration::from_secs(1)))?;
            stream.close()?;
            Ok(value)
        })();
        completed.send(result).unwrap();
    });
    let independent = completion.recv_timeout(Duration::from_secs(2));
    // Always release and join both callers, including when a runtime lock
    // regression prevented the sibling from completing before this signal.
    finish.send(()).unwrap();
    let first_result = first.join().unwrap();
    sibling.join().unwrap();

    entered.expect("the first receiver was polled before starting its sibling");
    assert_eq!(first_result.unwrap(), None);
    assert_eq!(
        independent
            .expect("a pending receiver must not monopolize the shared runtime")
            .unwrap(),
        Some(event())
    );
    assert_eq!(transport.observed.requests.lock().unwrap().len(), 2);
    assert_eq!(transport.observed.closes.load(Ordering::SeqCst), 1);
    assert_eq!(transport.observed.drops.load(Ordering::SeqCst), 2);
}

#[test]
fn upgrade_body_bound_applies_to_success_and_errors() {
    for status in [101, 403] {
        let response = http::Response::builder()
            .status(status)
            .header(
                http::header::SEC_WEBSOCKET_PROTOCOL,
                NORITO_V1_WEBSOCKET_SUBPROTOCOL,
            )
            .body(vec![0; UPGRADE_MAX_BYTES + 1])
            .unwrap();
        assert!(matches!(
            validate_upgrade(EVENTS_OPERATION, &response),
            Err(Error::ResponseTooLarge { .. })
        ));
    }
}

#[tokio::test]
async fn cloned_and_rebuilt_contexts_sign_their_own_stream_targets() {
    use crate::{
        crypto::{Hash, HashOf, KeyPair},
        data_model::{NetworkId, account::AccountId},
    };
    let transport = Arc::new(TestTransport::new([]));
    let original = account(Arc::clone(&transport));
    let cloned = original.clone();
    let mut builder = original.context.to_builder();
    builder.torii_url = "https://other.example/api/".parse().unwrap();
    builder.network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"other stream network",
    )));
    builder.key_pair = KeyPair::try_random().expect("stream test key pair");
    builder.account = AccountId::new(builder.key_pair.public_key().clone());
    let rebuilt = builder.build().unwrap().account_client().unwrap();
    for account in [&original, &cloned, &rebuilt] {
        let stream = account.events().subscribe([filter()]).await.unwrap();
        let requests = transport.observed.requests.lock().unwrap();
        let request = requests.last().unwrap();
        let snapshot = crate::http_default::RequestSnapshot {
            method: request.request.method().clone(),
            url: request.request.uri().to_string().parse().unwrap(),
            headers: request
                .request
                .headers()
                .iter()
                .map(|(n, v)| (n.to_string(), v.to_str().unwrap().to_owned()))
                .collect(),
            body: Vec::new(),
            timeout: Some(request.timeout),
            max_response_bytes: request.max_message_bytes,
            direct_loopback: false,
        };
        super::super::tests::assert_canonical_account_signed_request(&account.context, &snapshot);
        drop(requests);
        drop(stream);
    }
    let requests = transport.observed.requests.lock().unwrap();
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[0].request.uri(), requests[1].request.uri());
    assert_ne!(requests[0].request.uri(), requests[2].request.uri());
    assert!(Arc::ptr_eq(
        &original.context.stream_transport,
        &cloned.context.stream_transport
    ));
    let fresh = super::super::evidence_http_tests::client_with_base_url(
        super::super::evidence_http_tests::base_url(),
    );
    let other = super::super::evidence_http_tests::client_with_base_url(
        super::super::evidence_http_tests::base_url(),
    );
    assert!(!Arc::ptr_eq(
        &fresh.stream_transport,
        &other.stream_transport
    ));
}

#[tokio::test]
async fn protocol_header_validation_is_not_reported_as_signing_failure() {
    let transport = Arc::new(TestTransport::new([]));
    let mut builder = account(Arc::clone(&transport)).context.to_builder();
    builder
        .headers
        .insert("Last-Event-ID".to_owned(), "unsupported".to_owned());
    let account = builder.build().unwrap().account_client().unwrap();
    assert!(matches!(
        account.events().subscribe([filter()]).await,
        Err(Error::InvalidRequest { .. })
    ));
    assert!(matches!(
        account
            .blocks()
            .subscribe(NonZeroU64::new(1).unwrap())
            .await,
        Err(Error::InvalidRequest { .. })
    ));
    assert!(transport.observed.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn oversized_subscription_is_rejected_before_connecting() {
    let transport = Arc::new(TestTransport::new([]));
    let account = account(Arc::clone(&transport));
    let filters = vec![filter(); 100_000];
    let encoded = norito::to_bytes(
        &crate::data_model::events::stream::EventSubscriptionRequest::new(filters.clone()),
    )
    .unwrap();
    assert!(encoded.len() > SUBSCRIPTION_MAX_BYTES);
    assert!(matches!(
        account.events().subscribe(filters).await,
        Err(Error::InvalidRequest { .. })
    ));
    assert!(transport.observed.requests.lock().unwrap().is_empty());
}

fn rejected_instruction_filter(trigger_depth: usize) -> EventFilterBox {
    use crate::data_model::{
        Level,
        events::{
            data::DataEventFilter,
            pipeline::{TransactionEventFilter, TransactionStatus},
        },
        isi::{InstructionBox, Log, Register},
        transaction::error::{InstructionExecutionFail, TransactionRejectionReason},
        trigger::{
            Trigger,
            action::{Action, Repeats},
        },
    };

    let mut instruction: InstructionBox =
        Log::new(Level::INFO, "subscription encoding depth".to_owned()).into();
    for depth in 0..trigger_depth {
        let id = format!("stream_depth_{depth}").parse().unwrap();
        let action = Action::new(
            vec![instruction],
            Repeats::Exactly(1),
            iroha_test_samples::ALICE_ID.clone(),
            DataEventFilter::Any,
        )
        .expect("each constructed trigger action satisfies its public invariants");
        instruction = Register::trigger(Trigger::new(id, action)).into();
    }
    TransactionEventFilter::default()
        .for_status(TransactionStatus::Rejected(Box::new(
            TransactionRejectionReason::InstructionExecution(InstructionExecutionFail {
                instruction,
                reason: "caller-provided rejected instruction selector".to_owned(),
            }),
        )))
        .into()
}

#[tokio::test]
async fn shallow_rejected_instruction_subscription_preserves_exact_encoding() {
    let transport = Arc::new(TestTransport::new([]));
    let account = account(Arc::clone(&transport));
    let filter = rejected_instruction_filter(1);
    let expected = norito::to_bytes(
        &crate::data_model::events::stream::EventSubscriptionRequest::new(vec![filter.clone()]),
    )
    .expect("shallow rejected-instruction subscription is canonically encodable");
    assert!(expected.len() <= SUBSCRIPTION_MAX_BYTES);

    let stream = account.events().subscribe([filter]).await.unwrap();
    assert_eq!(transport.observed.requests.lock().unwrap().len(), 1);
    assert_eq!(*transport.observed.sent.lock().unwrap(), vec![expected]);
    stream.close().await.unwrap();
    assert_eq!(transport.observed.closes.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn subscription_encoding_depth_failure_is_typed_before_transport_dispatch() {
    let transport = Arc::new(TestTransport::new([]));
    let account = account(Arc::clone(&transport));
    let filter = rejected_instruction_filter(norito::core::MAX_VALUE_NESTING_DEPTH);
    let request =
        crate::data_model::events::stream::EventSubscriptionRequest::new(vec![filter.clone()]);
    assert!(matches!(
        norito::to_bytes(&request),
        Err(norito::core::Error::NestingDepthExceeded {
            context: "encode budget",
            ..
        })
    ));
    assert!(matches!(
        norito::core::to_bytes_bounded(&request, SUBSCRIPTION_MAX_BYTES),
        Err(norito::core::BoundedEncodeError::Serialization(
            norito::core::Error::NestingDepthExceeded {
                context: "encode budget",
                ..
            }
        ))
    ));

    assert!(matches!(
        account.events().subscribe([filter]).await,
        Err(Error::InvalidRequest { operation: EVENTS_OPERATION, details })
            if details.contains("nesting depth")
    ));
    assert!(transport.observed.requests.lock().unwrap().is_empty());
    assert!(transport.observed.sent.lock().unwrap().is_empty());
    assert_eq!(transport.observed.closes.load(Ordering::SeqCst), 0);

    // A rejected encode must release its nesting and layout scopes so the same
    // context can immediately encode and send an ordinary valid subscription.
    let recovery =
        crate::data_model::events::stream::EventSubscriptionRequest::new(vec![self::filter()]);
    let expected = norito::core::to_bytes_bounded(&recovery, SUBSCRIPTION_MAX_BYTES)
        .expect("bounded encoder recovers after the depth failure");
    assert_eq!(norito::to_bytes(&recovery).unwrap(), expected);
    let stream = account.events().subscribe(recovery.filters).await.unwrap();
    assert_eq!(transport.observed.requests.lock().unwrap().len(), 1);
    assert_eq!(*transport.observed.sent.lock().unwrap(), vec![expected]);
    stream.close().await.unwrap();
    assert_eq!(transport.observed.closes.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn message_bytes_are_per_stream_and_survive_pending_and_normal_close() {
    let bytes = encoded_event();
    let expected_len = bytes.len();
    let (finish, closed) = tokio::sync::oneshot::channel();
    let transport = TestTransport::new([Ok(StreamFrame::Binary(bytes))]);
    *transport.pending_read.lock().unwrap() = Some(PendingRead {
        started: None,
        finish: closed,
    });
    let transport = Arc::new(transport);
    let account = account(Arc::clone(&transport));
    let mut first = account.events().subscribe([filter()]).await.unwrap();
    transport
        .frames
        .lock()
        .unwrap()
        .push_back(Ok(StreamFrame::Binary(vec![0; 7])));
    let mut second = account.events().subscribe([filter()]).await.unwrap();
    assert_eq!(first.last_message_bytes(), None);
    assert_eq!(second.last_message_bytes(), None);

    assert_eq!(first.next().await.unwrap().unwrap(), event());
    assert_eq!(first.last_message_bytes(), Some(expected_len));
    assert_eq!(second.last_message_bytes(), None);
    assert!(futures_util::poll!(first.next()).is_pending());
    assert_eq!(first.last_message_bytes(), Some(expected_len));

    assert!(matches!(
        second.next().await,
        Some(Err(Error::Decode { .. }))
    ));
    assert_eq!(second.last_message_bytes(), Some(7));
    assert_eq!(first.last_message_bytes(), Some(expected_len));
    assert!(second.next().await.is_none());
    assert_eq!(second.last_message_bytes(), Some(7));

    finish.send(()).unwrap();
    assert!(first.next().await.is_none());
    assert_eq!(first.last_message_bytes(), Some(expected_len));
    assert!(first.next().await.is_none());
    assert_eq!(first.last_message_bytes(), Some(expected_len));
    first.close().await.unwrap();
    second.close().await.unwrap();
    assert_eq!(transport.observed.requests.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn binary_decode_and_bounds_failures_keep_exact_message_bytes() {
    for bytes in [Vec::new(), vec![0], vec![0; 9]] {
        let actual = bytes.len();
        let transport = Arc::new(TestTransport::new([Ok(StreamFrame::Binary(bytes))]));
        let mut stream = account(transport)
            .events()
            .subscribe([filter()])
            .await
            .unwrap();
        stream.maximum = 8;
        assert_eq!(stream.last_message_bytes(), None);
        let error = stream.next().await.unwrap().unwrap_err();
        if actual > 8 {
            assert_eq!(
                error,
                Error::ResponseTooLarge {
                    maximum: 8,
                    actual: Some(actual),
                }
            );
        } else {
            assert!(matches!(
                error,
                Error::Decode {
                    operation: EVENTS_OPERATION,
                    ..
                }
            ));
        }
        assert_eq!(stream.last_message_bytes(), Some(actual));
        assert!(stream.next().await.is_none());
        assert_eq!(stream.last_message_bytes(), Some(actual));
    }
}

#[tokio::test]
async fn yielded_transport_and_close_failures_clear_message_bytes() {
    for terminal in [
        Some(Err(Error::Transport {
            operation: EVENTS_OPERATION,
            kind: crate::TransportErrorKind::Io(std::io::ErrorKind::ConnectionReset),
            details: "reset after a binary message".to_owned(),
        })),
        Some(Err(Error::ResponseTooLarge {
            maximum: MESSAGE_MAX_BYTES,
            actual: Some(MESSAGE_MAX_BYTES + 1),
        })),
        Some(Ok(StreamFrame::Close {
            code: Some(1008),
            reason: "permission revoked".to_owned(),
        })),
        Some(Ok(StreamFrame::Close {
            code: Some(1013),
            reason: "slow reader".to_owned(),
        })),
        Some(Ok(StreamFrame::Close {
            code: None,
            reason: "missing disposition".to_owned(),
        })),
        None,
    ] {
        let bytes = encoded_event();
        let actual = bytes.len();
        let mut transport = TestTransport::new([Ok(StreamFrame::Binary(bytes))]);
        if let Some(terminal) = terminal {
            transport.frames.lock().unwrap().push_back(terminal);
        } else {
            transport.read_termination = ReadTermination::EndOfStream;
        }
        let mut stream = account(Arc::new(transport))
            .events()
            .subscribe([filter()])
            .await
            .unwrap();
        assert_eq!(stream.last_message_bytes(), None);
        assert_eq!(stream.next().await.unwrap().unwrap(), event());
        assert_eq!(stream.last_message_bytes(), Some(actual));
        assert!(stream.next().await.unwrap().is_err());
        // An adapter error is not a delivered binary message, even when its
        // diagnostic carries a size. Do not substitute that size for receipt.
        assert_eq!(stream.last_message_bytes(), None);
        assert!(stream.next().await.is_none());
        assert_eq!(stream.last_message_bytes(), None);
    }
}

#[test]
fn blocking_message_bytes_preserve_receive_timeouts_and_clear_transport_errors() {
    let bytes = encoded_event();
    let actual = bytes.len();
    let transport = Arc::new(TestTransport::new([Ok(StreamFrame::Binary(bytes))]));
    let facade =
        crate::blocking::AccountClient::from_client(account(Arc::clone(&transport))).unwrap();
    let mut first = facade.events().subscribe([filter()]).unwrap();
    assert_eq!(first.last_message_bytes(), None);
    assert_eq!(first.recv(None).unwrap(), Some(event()));
    assert_eq!(first.last_message_bytes(), Some(actual));
    assert_eq!(
        first.recv(Some(Duration::from_millis(5))),
        Err(Error::Timeout {
            operation: "stream.receive"
        })
    );
    assert_eq!(first.last_message_bytes(), Some(actual));

    transport.frames.lock().unwrap().extend([
        Ok(StreamFrame::Binary(encoded_event())),
        Err(Error::Transport {
            operation: EVENTS_OPERATION,
            kind: crate::TransportErrorKind::Io(std::io::ErrorKind::ConnectionReset),
            details: "reset after a binary message".to_owned(),
        }),
    ]);
    let mut second = facade.events().subscribe([filter()]).unwrap();
    assert_eq!(second.last_message_bytes(), None);
    assert_eq!(second.recv(None).unwrap(), Some(event()));
    assert_eq!(second.last_message_bytes(), Some(actual));
    assert!(matches!(second.recv(None), Err(Error::Transport { .. })));
    assert_eq!(second.last_message_bytes(), None);
    assert_eq!(first.last_message_bytes(), Some(actual));
    first.close().unwrap();
    second.close().unwrap();
    assert_eq!(transport.observed.requests.lock().unwrap().len(), 2);
}
