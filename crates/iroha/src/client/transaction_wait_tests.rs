//! Blocking and async waits share one deadline and survive read backpressure and transient
//! transport failures without replaying writes.

use std::{
    collections::VecDeque,
    io::ErrorKind,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use url::Url;

use super::{
    Client, Hash, HashOf, Response, SignedTransaction, StatusCode, TransactionFinalityFailure,
    TransactionWaitOptions,
    evidence_http_tests::{
        assert_status_scope, base_url, client_with_base_url, json_response, wait_status_case,
    },
};
use crate::{
    http::Method,
    http_default::{DefaultHttpTransport, RequestSnapshot},
};

fn hash() -> HashOf<SignedTransaction> {
    HashOf::from_untyped_unchecked(Hash::prehashed([0x71; Hash::LENGTH]))
}

fn status(kind: &str, resolved_from: &str) -> Response<Vec<u8>> {
    scoped_status(kind, resolved_from, "global")
}

fn scoped_status(kind: &str, resolved_from: &str, scope: &str) -> Response<Vec<u8>> {
    let status = if kind == "Applied" {
        norito::json!({"kind": kind, "block_height": 7})
    } else {
        norito::json!({"kind": kind})
    };
    let transaction_hash = hash().to_string();
    let payload = norito::json!({
        "hash": transaction_hash,
        "status": status,
        "scope": scope,
        "resolved_from": resolved_from
    });
    json_response(
        StatusCode::OK,
        &norito::json::to_string(&payload).expect("status JSON"),
    )
}

fn backpressure(retry_after: Option<&str>) -> Response<Vec<u8>> {
    let mut response = json_response(
        StatusCode::TOO_MANY_REQUESTS,
        r#"{"code":"proxy_capacity_exceeded","message":"Torii proxy memory capacity is exhausted"}"#,
    );
    if let Some(value) = retry_after {
        response
            .headers_mut()
            .insert(http::header::RETRY_AFTER, value.parse().expect("header"));
    }
    response
}

/// Every connection-level failure kind that a finality wait repeats.
const TRANSIENT_TRANSPORT_FAILURES: [ErrorKind; 7] = [
    ErrorKind::ConnectionRefused,
    ErrorKind::ConnectionReset,
    ErrorKind::ConnectionAborted,
    ErrorKind::NotConnected,
    ErrorKind::BrokenPipe,
    ErrorKind::UnexpectedEof,
    ErrorKind::TimedOut,
];

/// One scripted status read: a response, or a failure before any response arrived.
#[derive(Clone)]
enum Read {
    Response(Response<Vec<u8>>),
    /// The default transport's dispatch context over one connection-level failure.
    Transport(ErrorKind),
    /// Any other read failure, built afresh for every dispatch.
    Fail(fn() -> eyre::Report),
}

impl Read {
    fn into_result(self) -> eyre::Result<Response<Vec<u8>>> {
        match self {
            Self::Response(response) => Ok(response),
            Self::Transport(kind) => Err(dispatch_failure(std::io::Error::new(
                kind,
                "scripted connection failure",
            ))),
            Self::Fail(failure) => Err(failure()),
        }
    }
}

fn dispatch_failure(cause: std::io::Error) -> eyre::Report {
    eyre::Report::new(cause).wrap_err(
        "Failed to send http GET request to http://mock.local/v1/pipeline/transactions/status",
    )
}

fn scripted_client(
    responses: Vec<Response<Vec<u8>>>,
) -> (Client, Arc<Mutex<Vec<RequestSnapshot>>>) {
    scripted_reads(responses.into_iter().map(Read::Response).collect())
}

fn scripted_reads(reads: Vec<Read>) -> (Client, Arc<Mutex<Vec<RequestSnapshot>>>) {
    let reads = Arc::new(Mutex::new(VecDeque::from(reads)));
    let snapshots = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&snapshots);
    let transport = DefaultHttpTransport::mock(Arc::new(move |snapshot| {
        observed.lock().expect("snapshots").push(snapshot);
        let mut pending = reads.lock().expect("reads");
        // Repeat the last read so timeout tests cannot pass by exhausting a mock.
        if pending.len() > 1 {
            pending.pop_front().expect("next read")
        } else {
            pending.front().expect("nonempty read script").clone()
        }
        .into_result()
    }));
    (
        client_with_base_url(base_url()).with_test_http_transport(transport),
        snapshots,
    )
}

fn carries_transport_failure(error: &eyre::Report, kind: ErrorKind) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .is_some_and(|cause| cause.kind() == kind)
    })
}

/// Read the attempt count from an unresolved deadline report.
fn reported_attempts(error: &eyre::Report) -> u64 {
    let report = format!("{error:#}");
    let (_, tail) = report.split_once("attempts=").expect("attempt count");
    tail.chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .expect("attempt count")
}

fn wait(
    client: &Client,
    asynchronous: bool,
    timeout: Duration,
) -> eyre::Result<super::TransactionWaitOutcome> {
    wait_with_options(
        client,
        asynchronous,
        TransactionWaitOptions {
            timeout,
            poll_interval: Duration::from_millis(1),
        },
    )
}

fn wait_with_options(
    client: &Client,
    asynchronous: bool,
    options: TransactionWaitOptions,
) -> eyre::Result<super::TransactionWaitOutcome> {
    if asynchronous {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(client.wait_until_transaction_applied(hash(), options))
    } else {
        client.wait_for_transaction_applied(hash(), options)
    }
}

fn assert_only_exact_status_reads(snapshots: &[RequestSnapshot]) {
    assert_exact_scoped_reads(snapshots, "global");
}

fn assert_exact_scoped_reads(snapshots: &[RequestSnapshot], scope: &str) {
    for request in snapshots {
        assert_eq!(request.method, Method::GET);
        assert_eq!(request.url.path(), "/v1/pipeline/transactions/status");
        let query = request.url.query_pairs().collect::<Vec<_>>();
        assert_eq!(query.len(), 2);
        assert!(
            query
                .iter()
                .any(|(key, value)| key == "hash" && value == &hash().to_string())
        );
        assert!(
            query
                .iter()
                .any(|(key, value)| key == "scope" && value == scope)
        );
    }
}

#[test]
fn transaction_wait_backpressure_retries_only_reads_and_requires_state_applied() {
    for asynchronous in [false, true] {
        let (client, snapshots) = scripted_client(vec![
            backpressure(Some("0")),
            status("Applied", "cache"),
            status("Applied", "state"),
        ]);
        let outcome = wait(&client, asynchronous, Duration::from_secs(1)).expect("state finality");
        assert_eq!(outcome.attempts, 3);
        assert_eq!(outcome.resolved_from, "state");
        let snapshots = snapshots.lock().expect("snapshots");
        assert_eq!(snapshots.len(), 3);
        assert_only_exact_status_reads(&snapshots);
    }
}

#[test]
fn transaction_wait_backpressure_honors_retry_after_without_extending_deadline() {
    for asynchronous in [false, true] {
        let (client, snapshots) = scripted_client(vec![backpressure(Some("3600"))]);
        let error = wait(&client, asynchronous, Duration::from_millis(20)).expect_err("deadline");
        let report = format!("{error:#}");
        assert!(report.contains("did not reach state-resolved Applied within 20 ms"));
        assert!(report.contains(&format!("transaction {} did not reach", hash())));
        assert!(report.contains("attempts=1"));
        assert!(report.contains("429 Too Many Requests"));
        assert!(report.contains("proxy_capacity_exceeded"));
        let snapshots = snapshots.lock().expect("snapshots");
        assert_eq!(snapshots.len(), 1, "no read after the deadline");
        assert_only_exact_status_reads(&snapshots);
    }
}

#[test]
fn transaction_wait_timeout_identifies_the_exact_pending_transaction() {
    for asynchronous in [false, true] {
        for (response, expected_status) in [
            (
                super::evidence_http_tests::scoped_pipeline_absence(&hash()),
                "not_observed",
            ),
            (status("Queued", "queue"), "Queued"),
        ] {
            let (client, snapshots) = scripted_client(vec![response]);
            let error = wait_with_options(
                &client,
                asynchronous,
                TransactionWaitOptions {
                    timeout: Duration::from_millis(50),
                    poll_interval: Duration::from_millis(50),
                },
            )
            .expect_err("unresolved deadline");
            let report = format!("{error:#}");
            assert!(report.contains(&format!("transaction {} did not reach", hash())));
            assert!(report.contains(&format!("last_status={expected_status}; attempts=1")));
            let snapshots = snapshots.lock().expect("snapshots");
            assert_eq!(snapshots.len(), 1, "no read after the original deadline");
            assert_only_exact_status_reads(&snapshots);
        }
    }
}

#[test]
fn transaction_wait_backpressure_without_retry_after_uses_poll_interval() {
    for asynchronous in [false, true] {
        let (client, snapshots) =
            scripted_client(vec![backpressure(None), status("Applied", "state")]);
        assert_eq!(
            wait(&client, asynchronous, Duration::from_secs(1))
                .expect("finality")
                .attempts,
            2
        );
        assert_eq!(snapshots.lock().expect("snapshots").len(), 2);
    }
}

#[test]
fn transaction_wait_backpressure_is_still_an_error_for_one_shot_reads() {
    let (client, snapshots) = scripted_client(vec![backpressure(None)]);
    let error = client
        .get_transaction_status_response(hash())
        .expect_err("one-shot error");
    assert!(error.to_string().contains("429 Too Many Requests"));
    assert_eq!(snapshots.lock().expect("snapshots").len(), 1);
}

#[test]
fn transaction_wait_backpressure_does_not_retry_malformed_instructions_or_other_errors() {
    let mut duplicate = backpressure(Some("1"));
    duplicate
        .headers_mut()
        .append(http::header::RETRY_AFTER, "2".parse().expect("header"));
    let cases = [
        (
            backpressure(Some("-1")),
            "invalid Retry-After delta seconds",
        ),
        (
            backpressure(Some("18446744073709551616")),
            "invalid Retry-After delta seconds",
        ),
        (duplicate, "multiple Retry-After values"),
        (
            json_response(StatusCode::FORBIDDEN, "permission denied"),
            "403 Forbidden",
        ),
        (
            json_response(StatusCode::SERVICE_UNAVAILABLE, "route unavailable"),
            "503 Service Unavailable",
        ),
        (
            json_response(StatusCode::OK, "{}"),
            "Failed to get pipeline transaction status",
        ),
    ];
    for asynchronous in [false, true] {
        for (response, expected_error) in &cases {
            let (client, snapshots) =
                scripted_client(vec![response.clone(), status("Applied", "state")]);
            let error = wait(&client, asynchronous, Duration::from_secs(1))
                .expect_err("non-retryable error");
            assert!(format!("{error:#}").contains(*expected_error), "{error:#}");
            assert_eq!(snapshots.lock().expect("snapshots").len(), 1);
        }
    }
}

#[test]
fn transaction_wait_backpressure_preserves_fixed_failure_and_hash_binding() {
    for asynchronous in [false, true] {
        for kind in ["Rejected", "Expired"] {
            let (client, snapshots) =
                scripted_client(vec![backpressure(None), status(kind, "state")]);
            let error =
                wait(&client, asynchronous, Duration::from_secs(1)).expect_err("fixed failure");
            assert!(error.to_string().contains("fixed terminal failure"));
            assert_eq!(snapshots.lock().expect("snapshots").len(), 2);
        }
        let mut wrong = status("Applied", "state");
        let body = String::from_utf8(wrong.body().clone()).expect("JSON text");
        *wrong.body_mut() = body
            .replace(
                &hash().to_string(),
                &Hash::prehashed([0x73; Hash::LENGTH]).to_string(),
            )
            .into_bytes();
        let (client, snapshots) = scripted_client(vec![backpressure(None), wrong]);
        let error =
            wait(&client, asynchronous, Duration::from_secs(1)).expect_err("wrong hash must fail");
        assert!(
            matches!(
                error.downcast_ref::<crate::Error>(),
                Some(crate::Error::ResponseBinding {
                    operation: "pipeline.transaction_status",
                    field: "hash",
                })
            ),
            "{error:#}"
        );
        assert_eq!(snapshots.lock().expect("snapshots").len(), 2);
    }
}

#[test]
fn wait_for_transaction_applied_rejects_fixed_failures() {
    for (seed, kind) in [(0x33, "Rejected"), (0x35, "Expired")] {
        let (result, expected_hash, snapshots) =
            wait_status_case(seed, &norito::json!({ "kind": kind }), "state");
        let err = result.expect_err("terminal failure must fail the wait");
        assert!(err.to_string().contains("fixed terminal failure status"));
        let proof = err
            .chain()
            .find_map(|cause| cause.downcast_ref::<TransactionFinalityFailure>())
            .expect("fixed terminal failure remains typed through the SDK error chain");
        assert_eq!(proof.response().hash, expected_hash);
        assert_eq!(proof.response().status.kind, kind);
        proof
            .validate_for_hash(expected_hash.parse().expect("exact fixture hash"))
            .expect("canonical failure proof");
        let encoded = norito::json::to_vec(proof).expect("serialize exact failure evidence");
        let retained: TransactionFinalityFailure =
            norito::json::from_slice(&encoded).expect("decode exact failure evidence");
        assert_eq!(&retained, proof);
        let mut cached = proof.response().clone();
        cached.resolved_from = "cache".to_owned();
        assert!(
            TransactionFinalityFailure::from_response(expected_hash.parse().unwrap(), cached)
                .unwrap()
                .is_none()
        );
        assert_eq!(snapshots.len(), 1);
        assert_status_scope(&snapshots[0], "global");
    }
}

fn assert_unresolved(error: &eyre::Report, attempts: u64) {
    let final_error = error
        .downcast_ref::<super::TxConfirmationFinalError>()
        .expect("typed confirmation failure");
    assert_eq!(
        final_error.resolution,
        super::TxConfirmationErrorResolution::Unresolved
    );
    let report = format!("{error:#}");
    assert!(report.contains(&format!("transaction {} did not reach", hash())));
    assert!(report.contains(&format!("attempts={attempts}")), "{report}");
}

#[test]
fn transaction_wait_zero_timeout_never_dispatches_an_initial_read() {
    for asynchronous in [false, true] {
        for response in [
            super::evidence_http_tests::scoped_pipeline_absence(&hash()),
            status("Queued", "queue"),
            status("Applied", "state"),
        ] {
            let (client, snapshots) = scripted_client(vec![response]);
            let error = wait(&client, asynchronous, Duration::ZERO)
                .expect_err("zero budget cannot admit an observation");
            assert_unresolved(&error, 0);
            assert!(format!("{error:#}").contains("last_status=not_observed"));
            assert!(snapshots.lock().expect("snapshots").is_empty());
        }
    }
}

#[test]
fn transaction_wait_unrepresentable_deadline_fails_before_dispatch() {
    for asynchronous in [false, true] {
        let (client, snapshots) = scripted_client(vec![status("Applied", "state")]);
        let error = wait(&client, asynchronous, Duration::MAX).expect_err("invalid deadline");
        assert!(
            error
                .to_string()
                .contains("timeout cannot be represented as a monotonic deadline")
        );
        assert!(snapshots.lock().expect("snapshots").is_empty());
    }
}

#[test]
fn transaction_wait_expired_context_deadline_cannot_be_extended() {
    for asynchronous in [false, true] {
        let (client, snapshots) = scripted_client(vec![status("Applied", "state")]);
        let expired = client
            .with_request_deadline(Instant::now())
            .with_request_deadline(Instant::now() + Duration::from_secs(1));
        let error = wait(&expired, asynchronous, Duration::from_secs(1))
            .expect_err("an inherited deadline is an upper bound");
        assert_unresolved(&error, 0);
        assert!(snapshots.lock().expect("snapshots").is_empty());
        // The bounded clone retains pools without modifying the original context.
        assert!(
            expired
                .http_transport
                .shares_pools_with(&client.http_transport)
        );
        assert!(client.http_transport.deadline().is_none());
    }
}

#[test]
fn transaction_wait_late_http_status_is_unresolved_in_both_transports() {
    for asynchronous in [false, true] {
        for response in [
            status("Applied", "state"),
            status("Queued", "queue"),
            status("Rejected", "state"),
            status("Expired", "state"),
        ] {
            let snapshots = Arc::new(Mutex::new(Vec::new()));
            let observed = Arc::clone(&snapshots);
            let budget = Duration::from_millis(50);
            let transport = DefaultHttpTransport::mock(Arc::new(move |snapshot| {
                observed.lock().expect("snapshots").push(snapshot);
                // This synchronous responder returns a ready async future after expiry.
                // The blocking transport rejects it at completion; PollState must also
                // reject every late-ready async result, including terminal evidence.
                std::thread::sleep(budget + Duration::from_millis(1));
                Ok(response.clone())
            }));
            let client = client_with_base_url(base_url()).with_test_http_transport(transport);
            let error = wait(&client, asynchronous, budget).expect_err("late response");
            assert_unresolved(&error, 1);
            assert!(
                error
                    .chain()
                    .all(|cause| cause.downcast_ref::<TransactionFinalityFailure>().is_none()),
                "late terminal statuses must not produce finality evidence"
            );
            let snapshots = snapshots.lock().expect("snapshots");
            assert_eq!(snapshots.len(), 1);
            assert_only_exact_status_reads(&snapshots);
            assert!(snapshots[0].timeout.expect("remaining HTTP budget") <= budget);
        }
    }
}

#[test]
fn transaction_wait_retries_spend_one_remaining_http_budget() {
    for asynchronous in [false, true] {
        for inherited in [false, true] {
            let snapshots = Arc::new(Mutex::new(Vec::new()));
            let observed = Arc::clone(&snapshots);
            let transport = DefaultHttpTransport::mock(Arc::new(move |snapshot| {
                let attempt = {
                    let mut observed = observed.lock().expect("snapshots");
                    observed.push(snapshot);
                    observed.len()
                };
                std::thread::sleep(Duration::from_millis(5));
                Ok(match attempt {
                    1 => backpressure(Some("0")),
                    2 => status("Applied", "cache"),
                    3 => status("Applied", "state"),
                    _ => panic!("unexpected read"),
                })
            }));
            let client = client_with_base_url(base_url()).with_test_http_transport(transport);
            let options_budget = Duration::from_secs(2);
            let effective_budget = if inherited {
                Duration::from_secs(1)
            } else {
                options_budget
            };
            let client = if inherited {
                client.with_request_deadline(Instant::now() + effective_budget)
            } else {
                client
            };
            let outcome = wait(&client, asynchronous, options_budget).expect("in-budget finality");
            assert_eq!(outcome.attempts, 3);
            assert_eq!(outcome.resolved_from, "state");
            let snapshots = snapshots.lock().expect("snapshots");
            assert_eq!(snapshots.len(), 3);
            assert_only_exact_status_reads(&snapshots);
            let budgets = snapshots
                .iter()
                .map(|snapshot| snapshot.timeout.expect("remaining budget"))
                .collect::<Vec<_>>();
            assert!(budgets.iter().all(|budget| *budget <= effective_budget));
            assert!(budgets.windows(2).all(|pair| pair[1] < pair[0]));
        }
    }
}

#[test]
fn transaction_wait_outcome_admission_rechecks_deadline_after_decoding() {
    let response: super::PipelineTransactionStatusResponse =
        norito::json::from_slice(status("Applied", "state").body()).expect("typed status");
    let mut state = super::transaction_wait::PollState::new(
        hash(),
        TransactionWaitOptions {
            timeout: Duration::from_millis(50),
            poll_interval: Duration::from_millis(1),
        },
        None,
        super::transaction_wait::Scope::Global,
    )
    .expect("wait");
    state.begin_poll().expect("in-budget dispatch");
    std::thread::sleep(state.deadline().saturating_duration_since(Instant::now()));
    let error = state
        .observe(Ok(Some(response.clone())))
        .expect_err("predecoded Applied cannot bypass outcome deadline");
    assert_unresolved(&error, 1);
    assert!(format!("{error:#}").contains("last_status=Applied"));

    let mut wrong = response;
    wrong.hash = Hash::prehashed([0x73; Hash::LENGTH]).to_string();
    let error = state
        .observe(Ok(Some(wrong)))
        .expect_err("binding remains mandatory");
    assert!(matches!(
        error.downcast_ref::<crate::Error>(),
        Some(crate::Error::ResponseBinding {
            operation: "pipeline.transaction_status",
            field: "hash",
        })
    ));
}

#[tokio::test]
async fn transaction_wait_async_deadline_retires_the_pending_status_future() {
    use std::sync::atomic::{AtomicBool, Ordering};

    use crate::http::{HttpTransport, TransportFuture, TransportRequest};

    struct RetainedRead(Arc<AtomicBool>);
    impl Drop for RetainedRead {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    #[derive(Debug)]
    struct PendingStatus {
        dropped: Arc<AtomicBool>,
        snapshots: Arc<Mutex<Vec<RequestSnapshot>>>,
    }
    impl HttpTransport for PendingStatus {
        fn send_blocking(&self, _: TransportRequest) -> eyre::Result<Response<Vec<u8>>> {
            panic!("async-only fixture")
        }
        fn send(&self, request: TransportRequest) -> TransportFuture<'_> {
            self.snapshots
                .lock()
                .expect("snapshots")
                .push((&request).into());
            let retained = RetainedRead(Arc::clone(&self.dropped));
            Box::pin(async move {
                let _retained = retained;
                std::future::pending().await
            })
        }
    }
    let dropped = Arc::new(AtomicBool::new(false));
    let snapshots = Arc::new(Mutex::new(Vec::new()));
    let transport = DefaultHttpTransport::from_shared(Arc::new(PendingStatus {
        dropped: Arc::clone(&dropped),
        snapshots: Arc::clone(&snapshots),
    }));
    let client = client_with_base_url(base_url()).with_test_http_transport(transport);
    let error = tokio::time::timeout(
        Duration::from_secs(2),
        client.wait_until_transaction_applied(
            hash(),
            TransactionWaitOptions {
                timeout: Duration::from_millis(30),
                poll_interval: Duration::from_millis(1),
            },
        ),
    )
    .await
    .expect("the operation deadline must retire the pending read")
    .expect_err("unresolved deadline");
    assert_unresolved(&error, 1);
    assert!(dropped.load(Ordering::SeqCst));
    let snapshots = snapshots.lock().expect("snapshots");
    assert_eq!(snapshots.len(), 1);
    assert_only_exact_status_reads(&snapshots);
    assert!(snapshots[0].timeout.expect("deadline") <= Duration::from_millis(30));
}

#[derive(Clone, Copy, Debug)]
enum LocalWaitTransport {
    Blocking,
    Async,
    Facade,
}
const LOCAL_WAIT_TRANSPORTS: [LocalWaitTransport; 3] = [
    LocalWaitTransport::Blocking,
    LocalWaitTransport::Async,
    LocalWaitTransport::Facade,
];

fn wait_local(
    client: &Client,
    transport: LocalWaitTransport,
    timeout: Duration,
) -> eyre::Result<super::TransactionWaitOutcome> {
    let options = TransactionWaitOptions {
        timeout,
        poll_interval: Duration::from_millis(1),
    };
    match transport {
        LocalWaitTransport::Blocking => client.wait_for_transaction_applied_local(hash(), options),
        LocalWaitTransport::Async => tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(client.wait_until_transaction_applied_local(hash(), options)),
        LocalWaitTransport::Facade => crate::blocking::Client::from_client(client.clone())?
            .wait_for_transaction_applied_local(hash(), options),
    }
}

fn assert_not_global_finality(error: &eyre::Report) {
    assert!(
        error
            .chain()
            .all(|cause| cause.downcast_ref::<TransactionFinalityFailure>().is_none())
    );
    assert!(
        error
            .downcast_ref::<super::TxConfirmationFinalError>()
            .is_none()
    );
}

#[test]
fn transaction_wait_local_requires_exact_state_applied_and_requests_local_in_every_transport() {
    for transport in LOCAL_WAIT_TRANSPORTS {
        let (client, snapshots) = scripted_client(vec![
            backpressure(Some("0")),
            scoped_status("Applied", "cache", "local"),
            scoped_status("Applied", "state", "local"),
        ]);
        let client = client.with_request_deadline(Instant::now() + Duration::from_secs(1));
        let outcome = wait_local(&client, transport, Duration::from_secs(2)).expect("peer Applied");
        assert_eq!(outcome.scope, "local");
        assert_eq!(outcome.resolved_from, "state");
        assert_eq!(outcome.attempts, 3);
        let snapshots = snapshots.lock().expect("snapshots");
        assert_eq!(snapshots.len(), 3);
        assert_exact_scoped_reads(&snapshots, "local");
        let budgets = snapshots
            .iter()
            .map(|request| request.timeout.expect("remaining budget"))
            .collect::<Vec<_>>();
        assert!(
            budgets
                .iter()
                .all(|budget| *budget <= Duration::from_secs(1))
        );
        assert!(budgets.windows(2).all(|pair| pair[1] < pair[0]));
    }
}

#[test]
fn transaction_wait_local_rejects_wrong_hash_or_global_authority() {
    for transport in LOCAL_WAIT_TRANSPORTS {
        let mut wrong_hash = scoped_status("Applied", "state", "local");
        let body = String::from_utf8(wrong_hash.body().clone()).expect("JSON");
        *wrong_hash.body_mut() = body
            .replace(
                &hash().to_string(),
                &Hash::prehashed([0x73; Hash::LENGTH]).to_string(),
            )
            .into_bytes();
        for (response, expected) in [(wrong_hash, "hash"), (status("Applied", "state"), "scope")] {
            let (client, snapshots) = scripted_client(vec![response]);
            let error = wait_local(&client, transport, Duration::from_secs(1))
                .expect_err("wrong authority");
            if expected == "hash" {
                assert!(matches!(
                    error.downcast_ref::<crate::Error>(),
                    Some(crate::Error::ResponseBinding {
                        operation: "pipeline.transaction_status",
                        field: "hash",
                    })
                ));
            } else {
                assert!(format!("{error:#}").contains("scope must be exactly `local`"));
            }
            assert_not_global_finality(&error);
            let snapshots = snapshots.lock().expect("snapshots");
            assert_eq!(snapshots.len(), 1);
            assert_exact_scoped_reads(&snapshots, "local");
        }
    }
}

#[test]
fn transaction_wait_local_terminal_failure_is_not_global_finality_evidence() {
    for transport in LOCAL_WAIT_TRANSPORTS {
        for kind in ["Rejected", "Expired"] {
            let (client, snapshots) = scripted_client(vec![scoped_status(kind, "state", "local")]);
            let error =
                wait_local(&client, transport, Duration::from_secs(1)).expect_err("local failure");
            assert!(format!("{error:#}").contains("not a global finality failure"));
            assert_not_global_finality(&error);
            let snapshots = snapshots.lock().expect("snapshots");
            assert_eq!(snapshots.len(), 1);
            assert_exact_scoped_reads(&snapshots, "local");
        }
    }
}

#[test]
fn transaction_wait_local_cache_and_backpressure_cannot_outlive_original_deadline() {
    for transport in LOCAL_WAIT_TRANSPORTS {
        for response in [
            scoped_status("Applied", "cache", "local"),
            backpressure(Some("3600")),
        ] {
            let backpressured = response.status() == StatusCode::TOO_MANY_REQUESTS;
            let (client, snapshots) = scripted_client(vec![response]);
            let error = wait_local(&client, transport, Duration::from_millis(20))
                .expect_err("local deadline");
            assert!(
                format!("{error:#}").contains("peer-local Applied observation remains unresolved")
            );
            assert_not_global_finality(&error);
            let snapshots = snapshots.lock().expect("snapshots");
            assert!(!snapshots.is_empty());
            if backpressured {
                assert_eq!(snapshots.len(), 1, "Retry-After cannot extend budget");
            }
            assert_exact_scoped_reads(&snapshots, "local");
            assert!(
                snapshots
                    .iter()
                    .all(|request| request.timeout.expect("remaining budget")
                        <= Duration::from_millis(20))
            );
        }
    }
}

#[test]
fn transaction_wait_local_zero_or_inherited_expired_deadline_dispatches_nothing() {
    for transport in LOCAL_WAIT_TRANSPORTS {
        for inherited in [false, true] {
            let (client, snapshots) =
                scripted_client(vec![scoped_status("Applied", "state", "local")]);
            let (client, timeout) = if inherited {
                (
                    client
                        .with_request_deadline(Instant::now())
                        .with_request_deadline(Instant::now() + Duration::from_secs(1)),
                    Duration::from_secs(1),
                )
            } else {
                (client, Duration::ZERO)
            };
            let error = wait_local(&client, transport, timeout).expect_err("expired budget");
            assert_not_global_finality(&error);
            assert!(snapshots.lock().expect("snapshots").is_empty());
        }
    }
}

#[test]
fn transaction_wait_local_late_applied_never_confirms_the_peer() {
    for transport in LOCAL_WAIT_TRANSPORTS {
        let snapshots = Arc::new(Mutex::new(Vec::new()));
        let observed = Arc::clone(&snapshots);
        let budget = Duration::from_millis(30);
        let http = DefaultHttpTransport::mock(Arc::new(move |snapshot| {
            observed.lock().expect("snapshots").push(snapshot);
            std::thread::sleep(budget + Duration::from_millis(1));
            Ok(scoped_status("Applied", "state", "local"))
        }));
        let client = client_with_base_url(base_url()).with_test_http_transport(http);
        let error = wait_local(&client, transport, budget).expect_err("late local observation");
        assert_not_global_finality(&error);
        assert!(format!("{error:#}").contains("peer-local Applied observation remains unresolved"));
        let snapshots = snapshots.lock().expect("snapshots");
        assert_eq!(snapshots.len(), 1);
        assert_exact_scoped_reads(&snapshots, "local");
    }
}

#[test]
fn transaction_wait_repeats_transient_transport_failures_until_state_applied() {
    for asynchronous in [false, true] {
        for kind in TRANSIENT_TRANSPORT_FAILURES {
            let (client, snapshots) = scripted_reads(vec![
                Read::Transport(kind),
                Read::Response(status("Applied", "state")),
            ]);
            let outcome = wait(&client, asynchronous, Duration::from_secs(30))
                .unwrap_or_else(|error| panic!("{kind:?} must repeat the read: {error:#}"));
            assert_eq!(outcome.attempts, 2);
            assert_eq!(outcome.resolved_from, "state");
            let snapshots = snapshots.lock().expect("snapshots");
            assert_eq!(snapshots.len(), 2);
            assert_only_exact_status_reads(&snapshots);
        }
        // Transport failures share backpressure's cadence and never end the wait early.
        let (client, snapshots) = scripted_reads(vec![
            Read::Transport(ErrorKind::ConnectionReset),
            Read::Response(backpressure(Some("0"))),
            Read::Transport(ErrorKind::UnexpectedEof),
            Read::Response(status("Applied", "cache")),
            Read::Transport(ErrorKind::TimedOut),
            Read::Response(status("Applied", "state")),
        ]);
        let outcome = wait(&client, asynchronous, Duration::from_secs(30)).expect("state finality");
        assert_eq!(outcome.attempts, 6);
        let snapshots = snapshots.lock().expect("snapshots");
        assert_eq!(snapshots.len(), 6);
        assert_only_exact_status_reads(&snapshots);
    }
}

#[test]
fn transaction_wait_persistent_transport_failure_ends_unresolved_with_the_last_read_error() {
    // Coarse polls keep the final read clear of the deadline, so the report keeps the
    // scripted failure instead of the transport's own refusal at the deadline.
    let options = TransactionWaitOptions {
        timeout: Duration::from_millis(500),
        poll_interval: Duration::from_millis(100),
    };
    for asynchronous in [false, true] {
        let (client, snapshots) =
            scripted_reads(vec![Read::Transport(ErrorKind::ConnectionRefused)]);
        let error =
            wait_with_options(&client, asynchronous, options).expect_err("persistent failure");
        let attempts = reported_attempts(&error);
        assert_unresolved(&error, attempts);
        let report = format!("{error:#}");
        assert!(
            report.contains("within 500 ms; last_status=not_observed"),
            "{report}"
        );
        assert!(report.contains("scripted connection failure"), "{report}");
        assert!(
            carries_transport_failure(&error, ErrorKind::ConnectionRefused),
            "{report}"
        );
        let snapshots = snapshots.lock().expect("snapshots");
        assert!(snapshots.len() >= 2, "the read was not repeated: {report}");
        assert_eq!(attempts, u64::try_from(snapshots.len()).expect("count"));
        assert_only_exact_status_reads(&snapshots);
        assert!(
            snapshots
                .iter()
                .all(|request| request.timeout.expect("remaining budget") <= options.timeout)
        );

        // A later transport failure keeps the last observed status beside its own cause.
        let (client, _) = scripted_reads(vec![
            Read::Response(status("Queued", "queue")),
            Read::Transport(ErrorKind::ConnectionReset),
        ]);
        let error = wait_with_options(&client, asynchronous, options).expect_err("unresolved");
        let report = format!("{error:#}");
        assert!(report.contains("last_status=Queued"), "{report}");
        assert!(
            carries_transport_failure(&error, ErrorKind::ConnectionReset),
            "{report}"
        );
    }
}

#[test]
fn transaction_wait_terminal_read_failures_end_the_wait_at_once() {
    fn certificate() -> eyre::Report {
        dispatch_failure(std::io::Error::new(
            ErrorKind::InvalidData,
            "invalid peer certificate: UnknownIssuer",
        ))
    }
    fn name_resolution() -> eyre::Report {
        dispatch_failure(std::io::Error::other(
            "dns error: failed to lookup address information",
        ))
    }
    fn oversized() -> eyre::Report {
        crate::Error::ResponseTooLarge {
            maximum: 8,
            actual: Some(9),
        }
        .into()
    }
    fn untyped() -> eyre::Report {
        eyre::eyre!("connection reset by peer, in display text only")
    }
    let failures: [(fn() -> eyre::Report, &str); 4] = [
        (certificate, "invalid peer certificate"),
        (name_resolution, "dns error"),
        (oversized, "response exceeds the 8 byte limit"),
        (untyped, "in display text only"),
    ];
    let mut wrong_hash = status("Applied", "state");
    let body = String::from_utf8(wrong_hash.body().clone()).expect("JSON text");
    *wrong_hash.body_mut() = body
        .replace(
            &hash().to_string(),
            &Hash::prehashed([0x73; Hash::LENGTH]).to_string(),
        )
        .into_bytes();
    for asynchronous in [false, true] {
        for (failure, expected) in failures {
            let (client, snapshots) = scripted_reads(vec![
                Read::Fail(failure),
                Read::Response(status("Applied", "state")),
            ]);
            let error = wait(&client, asynchronous, Duration::from_secs(30))
                .expect_err("terminal read failure");
            assert!(format!("{error:#}").contains(expected), "{error:#}");
            assert!(
                error
                    .downcast_ref::<super::TxConfirmationFinalError>()
                    .is_none(),
                "a terminal read failure is not an unresolved deadline: {error:#}"
            );
            assert_eq!(snapshots.lock().expect("snapshots").len(), 1);
        }

        // A repeated read still ends at the next terminal outcome.
        let (client, snapshots) = scripted_reads(vec![
            Read::Transport(ErrorKind::ConnectionReset),
            Read::Response(status("Rejected", "state")),
        ]);
        let error = wait(&client, asynchronous, Duration::from_secs(30)).expect_err("rejection");
        assert!(
            error
                .chain()
                .any(|cause| cause.downcast_ref::<TransactionFinalityFailure>().is_some()),
            "{error:#}"
        );
        assert_eq!(snapshots.lock().expect("snapshots").len(), 2);

        let (client, snapshots) = scripted_reads(vec![
            Read::Transport(ErrorKind::ConnectionReset),
            Read::Response(wrong_hash.clone()),
        ]);
        let error = wait(&client, asynchronous, Duration::from_secs(30)).expect_err("binding");
        assert!(
            matches!(
                error.downcast_ref::<crate::Error>(),
                Some(crate::Error::ResponseBinding {
                    operation: "pipeline.transaction_status",
                    field: "hash",
                })
            ),
            "{error:#}"
        );
        assert_eq!(snapshots.lock().expect("snapshots").len(), 2);

        let (client, snapshots) = scripted_reads(vec![
            Read::Transport(ErrorKind::ConnectionReset),
            Read::Fail(certificate),
        ]);
        let error = wait(&client, asynchronous, Duration::from_secs(30)).expect_err("TLS");
        assert!(
            format!("{error:#}").contains("invalid peer certificate"),
            "{error:#}"
        );
        assert_eq!(snapshots.lock().expect("snapshots").len(), 2);
    }
}

#[test]
fn transaction_wait_local_repeats_transient_transport_failures_in_every_transport() {
    for transport in LOCAL_WAIT_TRANSPORTS {
        let (client, snapshots) = scripted_reads(vec![
            Read::Transport(ErrorKind::ConnectionReset),
            Read::Response(scoped_status("Applied", "state", "local")),
        ]);
        let outcome = wait_local(&client, transport, Duration::from_secs(30))
            .unwrap_or_else(|error| panic!("{transport:?}: {error:#}"));
        assert_eq!(outcome.attempts, 2);
        assert_eq!(outcome.scope, "local");
        assert_eq!(outcome.resolved_from, "state");
        let snapshots = snapshots.lock().expect("snapshots");
        assert_eq!(snapshots.len(), 2);
        assert_exact_scoped_reads(&snapshots, "local");
    }
}

#[test]
fn transaction_wait_transient_transport_failures_still_fail_one_shot_status_reads() {
    type OneShot = fn(&Client) -> eyre::Result<Option<super::PipelineTransactionStatusResponse>>;
    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(future)
    }
    let reads: [OneShot; 5] = [
        |client| client.get_transaction_status_response(hash()),
        |client| client.get_transaction_status_response_global(hash()),
        |client| client.get_transaction_status_response_local(hash()),
        |client| block_on(client.fetch_transaction_status_response_global(hash())),
        |client| block_on(client.fetch_transaction_status_response_local(hash())),
    ];
    for read in reads {
        let (client, snapshots) = scripted_reads(vec![
            Read::Transport(ErrorKind::ConnectionReset),
            Read::Response(status("Applied", "state")),
        ]);
        let error = read(&client).expect_err("one-shot reads fail fast");
        assert!(
            carries_transport_failure(&error, ErrorKind::ConnectionReset),
            "{error:#}"
        );
        assert_eq!(snapshots.lock().expect("snapshots").len(), 1);
    }
}

/// One loopback connection served by a real socket, in script order.
enum Connection {
    /// Read three request bytes and close: the unread rest makes the kernel reset.
    Reset,
    /// Read the whole request head, then close before any response.
    CloseBeforeResponse,
    /// Answer with one complete response, then close.
    Respond(Response<Vec<u8>>),
}

/// Serve `connections` on a fresh loopback port; the handle reports how many were served.
fn serve_loopback(connections: Vec<Connection>) -> (Url, std::thread::JoinHandle<usize>) {
    use std::io::{Read as _, Write as _};

    fn read_request_head(stream: &mut std::net::TcpStream) {
        let mut head = Vec::new();
        let mut buffer = [0_u8; 1024];
        while !head.windows(4).any(|window| window == b"\r\n\r\n") {
            let read = stream.read(&mut buffer).expect("request head");
            assert!(read > 0, "connection closed inside the request head");
            head.extend_from_slice(&buffer[..read]);
        }
        assert!(
            head.starts_with(b"GET /v1/pipeline/transactions/status?"),
            "{}",
            String::from_utf8_lossy(&head)
        );
    }

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("loopback listener");
    let address = listener.local_addr().expect("loopback address");
    listener
        .set_nonblocking(true)
        .expect("nonblocking listener");
    let server = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(60);
        let mut served = 0;
        for connection in connections {
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error)
                        if error.kind() == ErrorKind::WouldBlock && Instant::now() < deadline =>
                    {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => return served,
                }
            };
            // Accepted sockets can inherit the listener's nonblocking mode.
            stream.set_nonblocking(false).expect("blocking stream");
            stream
                .set_read_timeout(Some(Duration::from_secs(30)))
                .expect("read timeout");
            match connection {
                Connection::Reset => {
                    let mut method = [0_u8; 3];
                    stream.read_exact(&mut method).expect("request method");
                    assert_eq!(&method, b"GET");
                }
                Connection::CloseBeforeResponse => read_request_head(&mut stream),
                Connection::Respond(response) => {
                    read_request_head(&mut stream);
                    let mut raw = format!("HTTP/1.1 {}\r\n", response.status()).into_bytes();
                    for (name, value) in response.headers() {
                        raw.extend_from_slice(name.as_str().as_bytes());
                        raw.extend_from_slice(b": ");
                        raw.extend_from_slice(value.as_bytes());
                        raw.extend_from_slice(b"\r\n");
                    }
                    raw.extend_from_slice(
                        format!(
                            "Content-Length: {}\r\nConnection: close\r\n\r\n",
                            response.body().len()
                        )
                        .as_bytes(),
                    );
                    raw.extend_from_slice(response.body());
                    stream.write_all(&raw).expect("write response");
                }
            }
            served += 1;
        }
        served
    });
    let url = Url::parse(&format!("http://{address}/")).expect("loopback URL");
    (url, server)
}

/// A client on real reqwest pools that ignore any proxy environment for the loopback peer.
fn loopback_client(url: Url) -> Client {
    let mut client = client_with_base_url(url);
    client.http_transport =
        DefaultHttpTransport::new().with_policy(crate::config::HttpTransportPolicy::DirectLoopback);
    client
}

#[test]
fn transaction_wait_survives_real_connection_reset_and_premature_close() {
    for asynchronous in [false, true] {
        let (url, server) = serve_loopback(vec![
            Connection::Reset,
            Connection::CloseBeforeResponse,
            Connection::Respond(status("Applied", "state")),
        ]);
        let outcome = wait(&loopback_client(url), asynchronous, Duration::from_secs(30))
            .unwrap_or_else(|error| {
                panic!("real connection failures must repeat the read: {error:#}")
            });
        assert_eq!(outcome.attempts, 3);
        assert_eq!(outcome.resolved_from, "state");
        assert_eq!(server.join().expect("loopback server"), 3);
    }
}

#[test]
fn transaction_wait_refused_connections_end_unresolved_at_the_original_deadline() {
    let options = TransactionWaitOptions {
        timeout: Duration::from_millis(500),
        poll_interval: Duration::from_millis(100),
    };
    for asynchronous in [false, true] {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("loopback listener");
        let url = Url::parse(&format!(
            "http://{}/",
            listener.local_addr().expect("loopback address")
        ))
        .expect("loopback URL");
        drop(listener);
        let started = Instant::now();
        let error = wait_with_options(&loopback_client(url), asynchronous, options)
            .expect_err("nothing listens");
        assert!(
            started.elapsed() >= options.timeout,
            "a refused read cannot end the wait before its deadline"
        );
        let attempts = reported_attempts(&error);
        assert!(attempts >= 2, "the read was not repeated: {error:#}");
        assert_unresolved(&error, attempts);
        assert!(
            carries_transport_failure(&error, ErrorKind::ConnectionRefused),
            "{error:#}"
        );
    }
}

#[test]
fn rejection_summary_includes_the_underlying_cause_once() {
    use iroha_data_model::{ValidationFail, transaction::error::TransactionRejectionReason};
    let reason = TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
        "authority cannot modify this metadata".to_owned(),
    ));
    let summary = crate::client::rejection_summary(&reason);
    assert!(
        summary.contains("authority cannot modify this metadata"),
        "{summary}"
    );
    assert_eq!(
        summary
            .matches("authority cannot modify this metadata")
            .count(),
        1,
        "{summary}"
    );
}
