#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Router-level integration test for /v1/contracts/deploy and /v1/contracts/code-bytes/{hash}
#![cfg(all(feature = "app_api", feature = "ws_integration_tests"))]
#![allow(unexpected_cfgs)]

use std::sync::Arc;

use axum::{
    Router,
    routing::{get, post},
};
use base64::Engine as _;
use http_body_util::BodyExt as _;
use iroha_core::{kura::Kura, query::store::LiveQueryStore, queue::Queue, state::State};
use tower::ServiceExt as _; // for Router::oneshot

#[tokio::test]
async fn contracts_deploy_and_fetch_code_bytes() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: contracts deploy integration test gated. Set IROHA_RUN_IGNORED=1 to run."
        );
        return;
    }
    let creds = iroha_torii::test_utils::random_authority();
    let world = iroha_torii::test_utils::world_with_authority(&creds.account);

    // Minimal in-memory state and queue
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(world, kura, query));
    iroha_torii::test_utils::grant_contract_operator_permissions(&state, &creds.account);
    let events: iroha_core::EventsSender = tokio::sync::broadcast::channel(8).0;
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let queue = Arc::new(Queue::from_config(queue_cfg, events));
    let chain_id: iroha_data_model::ChainId = "chain".parse().unwrap();
    #[cfg(feature = "telemetry")]
    let telemetry = iroha_torii::MaybeTelemetry::for_tests();
    #[cfg(not(feature = "telemetry"))]
    let telemetry = iroha_torii::MaybeTelemetry::disabled();

    // Wire routes
    let app = Router::new()
        .route(
            "/v1/contracts/deploy",
            post({
                let chain_id = Arc::new(chain_id.clone());
                let queue = queue.clone();
                let state = state.clone();
                let telemetry = telemetry.clone();
                move |req: iroha_torii::NoritoJson<iroha_torii::DeployContractDto>| async move {
                    iroha_torii::handle_post_contract_deploy(
                        chain_id.clone(),
                        queue.clone(),
                        state.clone(),
                        telemetry.clone(),
                        req,
                    )
                    .await
                }
            }),
        )
        .route(
            "/v1/contracts/code-bytes/{code_hash}",
            get({
                let state = state.clone();
                move |axum::extract::Path(ch): axum::extract::Path<String>| async move {
                    iroha_torii::handle_get_contract_code_bytes(state, axum::extract::Path(ch))
                        .await
                }
            }),
        );

    // Build minimal program and request
    let prog = iroha_torii::test_utils::minimal_ivm_program(1);
    let code_b64 = base64::engine::general_purpose::STANDARD.encode(&prog);
    let body =
        iroha_torii::test_utils::deploy_request_json(&creds.account, &creds.private_key, &code_b64);
    let req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/deploy")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    let ch = v
        .get("code_hash_hex")
        .and_then(|x| x.as_str())
        .expect("code_hash_hex");

    // Process queued transaction(s) using the shared test helper
    let applied = iroha_torii::test_utils::drain_queue_and_apply_all(&state, &queue, &chain_id, 1);
    assert!(applied > 0, "no transactions applied from queue");

    // Fetch back code bytes via GET (now stored on-chain via RegisterSmartContractBytes)
    let uri = format!("/v1/contracts/code-bytes/{ch}");
    let req2 = http::Request::builder()
        .method("GET")
        .uri(&uri)
        .body(axum::body::Body::empty())
        .unwrap();
    let resp2 = app.clone().oneshot(req2).await.unwrap();
    assert_eq!(resp2.status(), http::StatusCode::OK);
    let bytes2 = resp2.into_body().collect().await.unwrap().to_bytes();
    let v2: norito::json::Value = norito::json::from_slice(&bytes2).unwrap();
    let code_b64_back = v2.get("code_b64").and_then(|x| x.as_str()).unwrap_or("");
    let roundtrip = base64::engine::general_purpose::STANDARD
        .decode(code_b64_back.as_bytes())
        .unwrap();
    assert_eq!(roundtrip, prog);
}
