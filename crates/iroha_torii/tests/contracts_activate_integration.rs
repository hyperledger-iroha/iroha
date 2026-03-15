#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! End-to-end integration: deploy + activate instance, then query instances.
#![cfg(all(feature = "app_api", feature = "ws_integration_tests"))]
#![allow(unexpected_cfgs, clippy::too_many_lines)]

use std::sync::Arc;

use axum::{
    Router,
    routing::{get, post},
};
use base64::Engine as _;
use http_body_util::BodyExt as _;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    queue::Queue,
    state::{State, World},
};
use tower::ServiceExt as _; // for Router::oneshot

#[tokio::test]
async fn contracts_deploy_then_activate_and_list_instance() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: contracts activate integration test gated. Set IROHA_RUN_IGNORED=1 to run."
        );
        return;
    }
    // Minimal in-memory state and queue
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

    // Wire routes
    let app = Router::new()
        .route(
            "/v1/contracts/deploy",
            post({
                let chain_id = Arc::new(chain_id.clone());
                let queue = queue.clone();
                let state = state.clone();
                let telemetry = telemetry.clone();
                move |iroha_torii::NoritoJson(req): iroha_torii::NoritoJson<
                    iroha_torii::DeployContractDto,
                >| async move {
                    iroha_torii::handle_post_contract_deploy(
                        chain_id.clone(),
                        queue.clone(),
                        state.clone(),
                        telemetry.clone(),
                        iroha_torii::NoritoJson(req),
                    )
                    .await
                }
            }),
        )
        .route(
            "/v1/contracts/instance/activate",
            post({
                let chain_id = Arc::new(chain_id.clone());
                let queue = queue.clone();
                let state = state.clone();
                let telemetry = telemetry.clone();
                move |iroha_torii::NoritoJson(req): iroha_torii::NoritoJson<
                    iroha_torii::ActivateInstanceDto,
                >| async move {
                    iroha_torii::handle_post_contract_instance_activate(
                        chain_id.clone(),
                        queue.clone(),
                        state.clone(),
                        telemetry.clone(),
                        iroha_torii::NoritoJson(req),
                    )
                    .await
                }
            }),
        )
        .route(
            "/v1/gov/instances/{ns}",
            get({
                let state = state.clone();
                move |axum::extract::Path(ns): axum::extract::Path<String>,
                      iroha_torii::NoritoQuery(q): iroha_torii::NoritoQuery<
                    iroha_torii::InstancesQuery,
                >| async move {
                    iroha_torii::handle_gov_instances_by_ns(
                        state,
                        axum::extract::Path(ns),
                        iroha_torii::NoritoQuery(q),
                    )
                    .await
                }
            }),
        );

    // Build minimal program and deploy
    let prog = iroha_torii::test_utils::minimal_ivm_program(1);
    let code_hash_hex = iroha_torii::test_utils::body_code_hash_hex(&prog);
    let code_b64 = base64::engine::general_purpose::STANDARD.encode(&prog);
    // Authority + private key
    let creds = iroha_torii::test_utils::random_authority();
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
    assert_eq!(ch, code_hash_hex);

    // Apply queued (deploy) transactions
    let applied_deploy =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 1);
    assert!(applied_deploy > 0);

    // Activate an instance
    let body2 = iroha_torii::test_utils::activate_instance_request_json(
        &creds.account,
        &creds.private_key,
        "apps",
        "calc.v1",
        ch,
    );
    let req2 = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/instance/activate")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body2))
        .unwrap();
    let resp2 = app.clone().oneshot(req2).await.unwrap();
    assert_eq!(resp2.status(), http::StatusCode::OK);

    // Apply queued (activate) transactions
    let applied_activate =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 2);
    assert!(applied_activate > 0);

    // List instances by namespace and assert our instance is present
    let req3 = http::Request::builder()
        .method("GET")
        .uri("/v1/gov/instances/apps")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp3 = app.clone().oneshot(req3).await.unwrap();
    assert_eq!(resp3.status(), http::StatusCode::OK);
    let b3 = resp3.into_body().collect().await.unwrap().to_bytes();
    let v3: norito::json::Value = norito::json::from_slice(&b3).unwrap();
    let arr = v3.get("instances").and_then(|x| x.as_array()).unwrap();
    assert!(arr.iter().any(|o| {
        o.get("contract_id").and_then(|x| x.as_str()) == Some("calc.v1")
            && o.get("code_hash_hex").and_then(|x| x.as_str()) == Some(ch)
    }));
}

#[tokio::test]
async fn contracts_deploy_and_activate_via_single_endpoint() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: contracts deploy+activate integration test gated. Set IROHA_RUN_IGNORED=1 to run."
        );
        return;
    }
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

    let app = Router::new()
        .route(
            "/v1/contracts/instance",
            post({
                let chain_id = Arc::new(chain_id.clone());
                let queue = queue.clone();
                let state = state.clone();
                let telemetry = telemetry.clone();
                move |iroha_torii::NoritoJson(req): iroha_torii::NoritoJson<
                    iroha_torii::DeployAndActivateInstanceDto,
                >| async move {
                    iroha_torii::handle_post_contract_instance(
                        chain_id.clone(),
                        queue.clone(),
                        state.clone(),
                        telemetry.clone(),
                        iroha_torii::NoritoJson(req),
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
                    iroha_torii::handle_get_contract_code_bytes(
                        state.clone(),
                        axum::extract::Path(ch),
                    )
                    .await
                }
            }),
        )
        .route(
            "/v1/gov/instances/{ns}",
            get({
                let state = state.clone();
                move |axum::extract::Path(ns): axum::extract::Path<String>,
                      iroha_torii::NoritoQuery(q): iroha_torii::NoritoQuery<
                    iroha_torii::InstancesQuery,
                >| async move {
                    iroha_torii::handle_gov_instances_by_ns(
                        state,
                        axum::extract::Path(ns),
                        iroha_torii::NoritoQuery(q),
                    )
                    .await
                }
            }),
        );

    let prog = iroha_torii::test_utils::minimal_ivm_program(1);
    let code_hash_hex = iroha_torii::test_utils::body_code_hash_hex(&prog);
    let code_b64 = base64::engine::general_purpose::STANDARD.encode(&prog);
    let creds = iroha_torii::test_utils::random_authority();
    let body = iroha_torii::test_utils::deploy_and_activate_request_json(
        &creds.account,
        &creds.private_key,
        "apps",
        "calc.v2",
        &code_b64,
    );
    let req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/instance")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    assert_eq!(
        v.get("code_hash_hex").and_then(|x| x.as_str()),
        Some(code_hash_hex.as_str())
    );
    assert_eq!(v.get("namespace").and_then(|x| x.as_str()), Some("apps"));
    assert_eq!(
        v.get("contract_id").and_then(|x| x.as_str()),
        Some("calc.v2")
    );

    let applied = iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 1);
    assert!(applied > 0);

    let uri = format!("/v1/contracts/code-bytes/{code_hash_hex}");
    let req2 = http::Request::builder()
        .method("GET")
        .uri(uri)
        .body(axum::body::Body::empty())
        .unwrap();
    let resp2 = app.clone().oneshot(req2).await.unwrap();
    assert_eq!(resp2.status(), http::StatusCode::OK);
    let b2 = resp2.into_body().collect().await.unwrap().to_bytes();
    let v2: norito::json::Value = norito::json::from_slice(&b2).unwrap();
    let returned_b64 = v2.get("code_b64").and_then(|x| x.as_str()).unwrap();
    let roundtrip = base64::engine::general_purpose::STANDARD
        .decode(returned_b64.as_bytes())
        .unwrap();
    assert_eq!(roundtrip, prog);

    let req3 = http::Request::builder()
        .method("GET")
        .uri("/v1/gov/instances/apps")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp3 = app.oneshot(req3).await.unwrap();
    assert_eq!(resp3.status(), http::StatusCode::OK);
    let b3 = resp3.into_body().collect().await.unwrap().to_bytes();
    let v3: norito::json::Value = norito::json::from_slice(&b3).unwrap();
    let arr = v3.get("instances").and_then(|x| x.as_array()).unwrap();
    assert!(arr.iter().any(|o| {
        o.get("contract_id").and_then(|x| x.as_str()) == Some("calc.v2")
            && o.get("code_hash_hex").and_then(|x| x.as_str()) == Some(code_hash_hex.as_str())
    }));
}
