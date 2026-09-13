//! Blocking and async waits share one deadline and survive read backpressure without replaying writes.

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

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
    let status = if kind == "Applied" {
        norito::json!({"kind": kind, "block_height": 7})
    } else {
        norito::json!({"kind": kind})
    };
    let transaction_hash = hash().to_string();
    let payload = norito::json!({
        "hash": transaction_hash,
        "status": status,
        "scope": "global",
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

fn scripted_client(
    responses: Vec<Response<Vec<u8>>>,
) -> (Client, Arc<Mutex<Vec<RequestSnapshot>>>) {
    let responses = Arc::new(Mutex::new(VecDeque::from(responses)));
    let snapshots = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&snapshots);
    let transport = DefaultHttpTransport::mock(Arc::new(move |snapshot| {
        observed.lock().expect("snapshots").push(snapshot);
        let mut pending = responses.lock().expect("responses");
        // Repeat the last response so timeout tests cannot pass by exhausting a mock.
        Ok(if pending.len() > 1 {
            pending.pop_front().expect("next response")
        } else {
            pending.front().expect("nonempty response script").clone()
        })
    }));
    (
        client_with_base_url(base_url()).with_test_http_transport(transport),
        snapshots,
    )
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
                .any(|(key, value)| key == "scope" && value == "global")
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
                json_response(StatusCode::NOT_FOUND, "transaction not observed"),
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
            json_response(StatusCode::NOT_FOUND, "transaction not observed"),
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
