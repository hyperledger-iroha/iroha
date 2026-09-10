//! Blocking and async finality waits must survive read backpressure without replaying writes.

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

use super::{
    Client, Hash, HashOf, Response, SignedTransaction, StatusCode, TransactionWaitOptions,
    evidence_http_tests::{base_url, client_with_base_url, json_response},
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
    let options = TransactionWaitOptions {
        timeout,
        poll_interval: Duration::from_millis(1),
    };
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
            let error =
                wait(&client, asynchronous, Duration::ZERO).expect_err("unresolved deadline");
            let report = format!("{error:#}");
            assert!(report.contains(&format!("transaction {} did not reach", hash())));
            assert!(report.contains(&format!("last_status={expected_status}; attempts=1")));
            let snapshots = snapshots.lock().expect("snapshots");
            assert_eq!(snapshots.len(), 1, "zero timeout still observes once");
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
