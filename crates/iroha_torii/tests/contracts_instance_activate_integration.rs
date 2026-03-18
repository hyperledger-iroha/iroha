#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Router-level test for /v1/contracts/instance/activate
#![allow(clippy::redundant_closure_for_method_calls)]
#![cfg(feature = "app_api")]

use std::sync::Arc;

use axum::{Router, routing::post};
use http_body_util::BodyExt as _;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    queue::Queue,
    state::{State, World},
};
use tower::ServiceExt as _; // for Router::oneshot

#[tokio::test]
async fn contracts_instance_activate_submits() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: contracts instance activate test gated. Set IROHA_RUN_IGNORED=1 to run."
        );
        return;
    }
    // Minimal state and queue
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(World::default(), kura, query));
    let events: iroha_core::EventsSender = tokio::sync::broadcast::channel(8).0;
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let queue = Arc::new(Queue::from_config(queue_cfg, events));
    let chain_id: iroha_data_model::ChainId = "chain".parse().unwrap();
    #[cfg(feature = "telemetry")]
    let telemetry = iroha_torii::MaybeTelemetry::for_tests();
    #[cfg(not(feature = "telemetry"))]
    let telemetry = iroha_torii::MaybeTelemetry::disabled();

    // Router
    let app = Router::new().route(
        "/v1/contracts/instance/activate",
        post({
            let chain_id = Arc::new(chain_id.clone());
            let queue = queue.clone();
            let state = state.clone();
            let telemetry = telemetry.clone();
            move |req: iroha_torii::NoritoJson<iroha_torii::ActivateInstanceDto>| async move {
                iroha_torii::handle_post_contract_instance_activate(
                    chain_id.clone(),
                    queue.clone(),
                    state.clone(),
                    telemetry.clone(),
                    req,
                )
                .await
            }
        }),
    );

    let creds = iroha_torii::test_utils::random_authority();
    let body = iroha_torii::test_utils::activate_instance_request_json(
        &creds.account,
        &creds.private_key,
        "apps",
        "calc.v1",
        &"aa".repeat(32),
    );
    let req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/instance/activate")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    assert_eq!(v.get("ok").and_then(|x| x.as_bool()), Some(true));
}
