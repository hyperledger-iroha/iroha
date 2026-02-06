#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Tests covering Norito-RPC ingress policies and Content-Type enforcement.

#[path = "common/norito_rpc_harness.rs"]
mod norito_rpc_harness;

use axum::http::{StatusCode, header::RETRY_AFTER};
use iroha_config::parameters::actual::NoritoRpcStage;
use norito_rpc_harness::NoritoRpcHarness;

const ERROR_HEADER: &str = "x-iroha-error-code";

#[tokio::test]
async fn missing_content_type_is_rejected() {
    let harness = NoritoRpcHarness::new(|cfg| {
        cfg.torii.transport.norito_rpc.stage = NoritoRpcStage::Ga;
    });

    let resp = harness.post_transaction(false, &[]).await;
    assert_eq!(resp.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
}

#[tokio::test]
async fn disabled_stage_blocks_norito_requests() {
    let harness = NoritoRpcHarness::new(|cfg| {
        cfg.torii.transport.norito_rpc.stage = NoritoRpcStage::Disabled;
    });

    let resp = harness.post_transaction(true, &[]).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        resp.headers()
            .get(ERROR_HEADER)
            .map(|v| v.to_str().unwrap()),
        Some("norito_rpc_disabled")
    );
    assert_eq!(
        resp.headers().get(RETRY_AFTER).map(|v| v.to_str().unwrap()),
        Some("300")
    );
}

#[tokio::test]
async fn canary_stage_enforces_allowlist() {
    let allowlist_token = "norito-canary";
    let harness = NoritoRpcHarness::new(|cfg| {
        cfg.torii.transport.norito_rpc.stage = NoritoRpcStage::Canary;
        cfg.torii.transport.norito_rpc.allowed_clients = vec![allowlist_token.to_string()];
    });

    let denied = harness.post_transaction(true, &[]).await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        denied
            .headers()
            .get(ERROR_HEADER)
            .map(|v| v.to_str().unwrap()),
        Some("norito_rpc_canary_denied")
    );
    assert_eq!(
        denied
            .headers()
            .get(RETRY_AFTER)
            .map(|v| v.to_str().unwrap()),
        Some("300")
    );

    let allowed = harness
        .post_transaction(true, &[("x-api-token", allowlist_token)])
        .await;
    assert_ne!(allowed.status(), StatusCode::FORBIDDEN);
    assert!(allowed.headers().get(ERROR_HEADER).is_none());
}

#[tokio::test]
async fn norito_transaction_returns_submission_receipt() {
    use axum::body::Body;
    use axum::http::{Request, header::CONTENT_TYPE};
    use http_body_util::BodyExt;
    use iroha_data_model::transaction::{SignedTransaction, TransactionSubmissionReceipt};
    use iroha_torii_shared::uri;
    use iroha_version::codec::DecodeVersioned as _;
    use tower::ServiceExt as _;

    let harness = NoritoRpcHarness::new(|cfg| {
        cfg.torii.transport.norito_rpc.stage = NoritoRpcStage::Ga;
    });

    let tx_bytes = norito_rpc_harness::sample_transaction_bytes();
    let tx = SignedTransaction::decode_all_versioned(&tx_bytes).expect("decode transaction");
    let expected_hash = tx.hash();

    let resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri::TRANSACTION)
                .header(CONTENT_TYPE, "application/x-norito")
                .body(Body::from(tx_bytes))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let body = BodyExt::collect(resp.into_body())
        .await
        .expect("collect body")
        .to_bytes();
    let receipt: TransactionSubmissionReceipt =
        norito::decode_from_bytes(&body).expect("decode receipt");
    assert!(receipt.verify().is_ok());
    assert_eq!(receipt.payload.tx_hash, expected_hash);
    assert_eq!(
        receipt.payload.signer,
        harness.cfg.common.key_pair.public_key().clone()
    );
}
