//! Exact operator-authentication client boundary tests.
use std::sync::{Arc, Mutex};
use crate::http::{Method as HttpMethod, Response, StatusCode};
use super::evidence_http_tests::{
    SnapshotStore, base_url, client_with_base_url, respond_with, with_mock_http,
};
use super::{
    HEADER_OPERATOR_NONCE, HEADER_OPERATOR_PUBLIC_KEY, HEADER_OPERATOR_SIGNATURE,
    HEADER_OPERATOR_TIMESTAMP_MS, checked_random_keypair,
};
#[test]
fn operator_endpoint_requires_a_signing_key_before_dispatch() {
    let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
    let client = client_with_base_url(base_url());
    let error = with_mock_http(
        respond_with(
            &snapshots,
            Response::builder()
                .status(StatusCode::OK)
                .body(Vec::new())
                .expect("response build"),
        ),
        || client.get_config(),
    )
    .expect_err("operator endpoint must reject a missing local signer");
    assert!(
        error
            .to_string()
            .contains("operator signing key is required before request dispatch")
    );
    assert!(
        snapshots.lock().expect("lock snapshots").is_empty(),
        "missing operator credentials must fail before transport"
    );
}
#[test]
fn proof_retention_requires_a_signing_key_before_dispatch() {
    let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
    let client = client_with_base_url(base_url());
    let error = with_mock_http(
        respond_with(
            &snapshots,
            Response::builder()
                .status(StatusCode::OK)
                .body(Vec::new())
                .expect("response build"),
        ),
        || client.get_proof_retention_status(),
    )
    .expect_err("proof-retention read must reject a missing local signer");
    assert!(
        error
            .to_string()
            .contains("operator signing key is required")
    );
    assert!(
        snapshots.lock().expect("lock snapshots").is_empty(),
        "missing operator credentials must fail before transport"
    );
}
#[test]
fn proof_retention_uses_one_exact_signed_empty_body_get() {
    let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
    let mut client = client_with_base_url(base_url());
    client.set_operator_key_pair(checked_random_keypair());
    let error = with_mock_http(
        respond_with(
            &snapshots,
            Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(Vec::new())
                .expect("response build"),
        ),
        || client.get_proof_retention_status(),
    )
    .expect_err("mocked unauthorized response must fail");
    assert!(error.to_string().contains("proof retention"));
    let snapshots = snapshots.lock().expect("lock snapshots");
    assert_eq!(snapshots.len(), 1, "operator reads are one-shot");
    let snapshot = &snapshots[0];
    assert_eq!(snapshot.method, HttpMethod::GET);
    assert_eq!(snapshot.url.path(), "/v1/proofs/retention");
    assert!(snapshot.url.query().is_none());
    assert!(snapshot.body.is_empty());
    for header in [
        HEADER_OPERATOR_PUBLIC_KEY,
        HEADER_OPERATOR_TIMESTAMP_MS,
        HEADER_OPERATOR_NONCE,
        HEADER_OPERATOR_SIGNATURE,
    ] {
        assert!(
            snapshot
                .headers
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case(header)),
            "missing {header}"
        );
    }
}
