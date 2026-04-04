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
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    queue::Queue,
    state::{State, WorldReadOnly},
};
use mv::storage::StorageReadOnly;
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

#[tokio::test]
async fn contracts_redeploy_same_alias_rotates_address_and_deactivates_previous() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: contracts deploy integration test gated. Set IROHA_RUN_IGNORED=1 to run."
        );
        return;
    }
    let creds = iroha_torii::test_utils::random_authority();
    let world = iroha_torii::test_utils::world_with_authority(&creds.account);

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

    let app = Router::new().route(
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
    );

    let alias = iroha_data_model::smart_contract::ContractAlias::from_components(
        "upgradeprobe",
        None,
        "universal",
    )
    .expect("contract alias");
    let program = iroha_torii::test_utils::minimal_ivm_program(1);
    let code_b64 = base64::engine::general_purpose::STANDARD.encode(&program);
    let build_request = || {
        let body = iroha_torii::json_object(vec![
            iroha_torii::json_entry("authority", creds.account.clone()),
            iroha_torii::json_entry("private_key", creds.private_key.to_string()),
            iroha_torii::json_entry("code_b64", code_b64.clone()),
            iroha_torii::json_entry("contract_alias", alias.clone()),
        ]);
        http::Request::builder()
            .method("POST")
            .uri("/v1/contracts/deploy")
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(
                norito::json::to_json(&body).expect("serialize deploy request"),
            ))
            .expect("deploy request")
    };

    let first_resp = app
        .clone()
        .oneshot(build_request())
        .await
        .expect("first deploy response");
    assert_eq!(first_resp.status(), http::StatusCode::OK);
    let first_body = first_resp.into_body().collect().await.unwrap().to_bytes();
    let first_json: norito::json::Value = norito::json::from_slice(&first_body).unwrap();
    let first_address: iroha_data_model::smart_contract::ContractAddress = first_json
        .get("contract_address")
        .and_then(norito::json::Value::as_str)
        .expect("first contract address")
        .parse()
        .expect("parse first contract address");
    assert_eq!(
        first_json
            .get("contract_alias")
            .and_then(norito::json::Value::as_str),
        Some(alias.as_ref())
    );
    assert_eq!(
        first_json
            .get("previous_contract_address")
            .and_then(norito::json::Value::as_str),
        None
    );
    assert_eq!(
        first_json
            .get("upgraded")
            .and_then(norito::json::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        first_json
            .get("deploy_nonce")
            .and_then(norito::json::Value::as_u64),
        Some(0)
    );

    let applied_first =
        iroha_torii::test_utils::drain_queue_and_apply_all(&state, &queue, &chain_id, 1);
    assert!(
        applied_first > 0,
        "first deploy should enqueue transactions"
    );

    {
        let view = state.view();
        assert_eq!(
            view.world().contract_address_by_alias_at(&alias, 0),
            Some(first_address.clone())
        );
        assert!(
            view.world()
                .contract_instances()
                .get(&first_address)
                .is_some()
        );
    }

    let second_resp = app
        .clone()
        .oneshot(build_request())
        .await
        .expect("second deploy response");
    assert_eq!(second_resp.status(), http::StatusCode::OK);
    let second_body = second_resp.into_body().collect().await.unwrap().to_bytes();
    let second_json: norito::json::Value = norito::json::from_slice(&second_body).unwrap();
    let second_address: iroha_data_model::smart_contract::ContractAddress = second_json
        .get("contract_address")
        .and_then(norito::json::Value::as_str)
        .expect("second contract address")
        .parse()
        .expect("parse second contract address");
    assert_ne!(
        second_address, first_address,
        "redeploy must mint a new immutable address"
    );
    assert_eq!(
        second_json
            .get("previous_contract_address")
            .and_then(norito::json::Value::as_str),
        Some(first_address.as_ref())
    );
    assert_eq!(
        second_json
            .get("upgraded")
            .and_then(norito::json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        second_json
            .get("deploy_nonce")
            .and_then(norito::json::Value::as_u64),
        Some(1)
    );

    let applied_second =
        iroha_torii::test_utils::drain_queue_and_apply_all(&state, &queue, &chain_id, 2);
    assert!(
        applied_second > 0,
        "second deploy should enqueue transactions"
    );

    let view = state.view();
    assert_eq!(
        view.world().contract_address_by_alias_at(&alias, 0),
        Some(second_address.clone())
    );
    assert!(
        view.world()
            .contract_instances()
            .get(&first_address)
            .is_none()
    );
    assert!(
        view.world()
            .contract_instances()
            .get(&second_address)
            .is_some()
    );
    assert!(
        view.world()
            .contract_alias_bindings()
            .get(&first_address)
            .is_none()
    );
    assert_eq!(
        view.world()
            .contract_alias_bindings()
            .get(&second_address)
            .map(|binding| binding.alias.as_ref()),
        Some(alias.as_ref())
    );
}
