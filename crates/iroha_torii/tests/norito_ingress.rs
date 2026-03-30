#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Tests covering Norito-RPC ingress policies and Content-Type enforcement.

#[path = "common/norito_rpc_harness.rs"]
mod norito_rpc_harness;

use axum::http::{StatusCode, header::RETRY_AFTER};
use iroha_config::parameters::actual::NoritoRpcStage;
use norito_rpc_harness::NoritoRpcHarness;

const ERROR_HEADER: &str = "x-iroha-error-code";

fn default_alias_policy() -> sorafs_manifest::alias_cache::AliasCachePolicy {
    sorafs_manifest::alias_cache::AliasCachePolicy::new(
        std::time::Duration::from_secs(
            iroha_config::parameters::defaults::torii::SORAFS_ALIAS_POSITIVE_TTL_SECS,
        ),
        std::time::Duration::from_secs(
            iroha_config::parameters::defaults::torii::SORAFS_ALIAS_REFRESH_WINDOW_SECS,
        ),
        std::time::Duration::from_secs(
            iroha_config::parameters::defaults::torii::SORAFS_ALIAS_HARD_EXPIRY_SECS,
        ),
        std::time::Duration::from_secs(
            iroha_config::parameters::defaults::torii::SORAFS_ALIAS_NEGATIVE_TTL_SECS,
        ),
        std::time::Duration::from_secs(
            iroha_config::parameters::defaults::torii::SORAFS_ALIAS_REVOCATION_TTL_SECS,
        ),
        std::time::Duration::from_secs(
            iroha_config::parameters::defaults::torii::SORAFS_ALIAS_ROTATION_MAX_AGE_SECS,
        ),
        std::time::Duration::from_secs(
            iroha_config::parameters::defaults::torii::SORAFS_ALIAS_SUCCESSOR_GRACE_SECS,
        ),
        std::time::Duration::from_secs(
            iroha_config::parameters::defaults::torii::SORAFS_ALIAS_GOVERNANCE_GRACE_SECS,
        ),
    )
}

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

#[tokio::test]
async fn public_transaction_route_rejects_internal_entrypoint_payload() {
    use axum::body::Body;
    use axum::http::{Request, header::CONTENT_TYPE};
    use http_body_util::BodyExt;
    use iroha_torii_shared::uri;
    use tower::ServiceExt as _;

    let harness = NoritoRpcHarness::new(|cfg| {
        cfg.torii.transport.norito_rpc.stage = NoritoRpcStage::Ga;
    });

    let resp = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri::TRANSACTION)
                .header(CONTENT_TYPE, "application/x-norito")
                .body(Body::from(
                    norito_rpc_harness::sample_transaction_entrypoint_bytes(),
                ))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = BodyExt::collect(resp.into_body())
        .await
        .expect("collect body")
        .to_bytes();
    let text = String::from_utf8(body.to_vec()).expect("response text");
    assert!(
        text.contains("Could not decode request"),
        "unexpected error body: {text}"
    );
}

#[tokio::test]
async fn iroha_client_submit_transaction_succeeds_against_torii_public_signed_transaction_ingress()
{
    use iroha::{client::Client, config::Config};
    use iroha_data_model::{
        ChainId, account::AccountId, isi::Log, transaction::TransactionBuilder,
    };
    use iroha_logger::Level;
    use tokio::net::TcpListener;

    let harness = NoritoRpcHarness::new(|cfg| {
        cfg.torii.transport.norito_rpc.stage = NoritoRpcStage::Ga;
    });

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test listener");
    let addr = listener.local_addr().expect("listener addr");
    let app = harness.app.clone();
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve test Torii");
    });

    let chain: ChainId = harness.cfg.common.chain.clone();
    let key_pair = iroha_crypto::KeyPair::random();
    let account = AccountId::of(key_pair.public_key().clone());
    let client = Client::new(Config {
        chain: chain.clone(),
        account: account.clone(),
        key_pair: key_pair.clone(),
        basic_auth: None,
        torii_api_url: format!("http://{addr}/").parse().expect("torii url"),
        torii_api_version: iroha::config::default_torii_api_version(),
        torii_api_min_proof_version: iroha::config::DEFAULT_TORII_API_MIN_PROOF_VERSION.to_string(),
        torii_request_timeout: iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT,
        transaction_ttl: std::time::Duration::from_secs(5),
        transaction_status_timeout: std::time::Duration::from_secs(10),
        transaction_add_nonce: false,
        connect_queue_root: iroha::config::default_connect_queue_root(),
        soracloud_http_witness_file: None,
        sorafs_alias_cache: default_alias_policy(),
        sorafs_anonymity_policy: iroha::config::AnonymityPolicy::GuardPq,
        sorafs_rollout_phase: iroha_config::parameters::actual::SorafsRolloutPhase::Canary,
    });

    let tx = TransactionBuilder::new(chain, account)
        .with_instructions([Log::new(Level::INFO, "client submit e2e".to_owned())])
        .sign(key_pair.private_key());
    let expected_hash = tx.hash();

    let actual_hash = tokio::task::spawn_blocking(move || client.submit_transaction(&tx))
        .await
        .expect("join client submit")
        .expect("submit transaction");
    assert_eq!(actual_hash, expected_hash);
}
