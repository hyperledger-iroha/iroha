#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration test for the contract call endpoint.
#![cfg(all(feature = "app_api", feature = "ws_integration_tests"))]
#![allow(unexpected_cfgs, clippy::too_many_lines)]

use std::sync::Arc;

use axum::{Router, routing::post};
use base64::Engine as _;
use http_body_util::BodyExt as _;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    queue::Queue,
    state::{State, World},
};
use iroha_crypto::Signature;
use norito::json;
use tower::ServiceExt as _;

#[tokio::test]
async fn contracts_call_enqueues_transaction() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: contract call integration test gated. Set IROHA_RUN_IGNORED=1 to run."
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
            "/v1/contracts/call",
            post({
                let chain_id = Arc::new(chain_id.clone());
                let queue = queue.clone();
                let state = state.clone();
                let telemetry = telemetry.clone();
                move |iroha_torii::NoritoJson(req): iroha_torii::NoritoJson<
                    iroha_torii::ContractCallDto,
                >| async move {
                    iroha_torii::handle_post_contract_call(
                        chain_id.clone(),
                        queue.clone(),
                        state.clone(),
                        telemetry.clone(),
                        iroha_torii::NoritoJson(req),
                    )
                    .await
                }
            }),
        );

    let program = iroha_torii::test_utils::minimal_ivm_program(1);
    let code_hash_hex = iroha_torii::test_utils::body_code_hash_hex(&program);
    let code_b64 = base64::engine::general_purpose::STANDARD.encode(&program);
    let creds = iroha_torii::test_utils::random_authority();

    let deploy_body =
        iroha_torii::test_utils::deploy_request_json(&creds.account, &creds.private_key, &code_b64);
    let deploy_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/deploy")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(deploy_body))
        .unwrap();
    let deploy_resp = app.clone().oneshot(deploy_req).await.unwrap();
    assert_eq!(deploy_resp.status(), http::StatusCode::OK);
    let deploy_bytes = deploy_resp.into_body().collect().await.unwrap().to_bytes();
    let deploy_json: json::Value = json::from_slice(&deploy_bytes).unwrap();
    let abi_hash_hex = deploy_json
        .get("abi_hash_hex")
        .and_then(json::Value::as_str)
        .expect("abi_hash_hex present")
        .to_owned();

    let applied_deploy =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 1);
    assert_eq!(applied_deploy, 1);

    let activate_body = iroha_torii::test_utils::activate_instance_request_json(
        &creds.account,
        &creds.private_key,
        "apps",
        "calc.v1",
        &code_hash_hex,
    );
    let activate_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/instance/activate")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(activate_body))
        .unwrap();
    let activate_resp = app.clone().oneshot(activate_req).await.unwrap();
    assert_eq!(activate_resp.status(), http::StatusCode::OK);

    let applied_activate =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 2);
    assert_eq!(applied_activate, 1);

    let missing_limit_payload = iroha_torii::json_object(vec![
        iroha_torii::json_entry("authority", creds.account.clone()),
        iroha_torii::json_entry("private_key", creds.private_key.to_string()),
        iroha_torii::json_entry("namespace", "apps"),
        iroha_torii::json_entry("contract_id", "calc.v1"),
    ]);
    let missing_limit_body = json::to_json(&missing_limit_payload).expect("serialize call request");
    let missing_limit_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/call")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(missing_limit_body))
        .unwrap();
    let missing_limit_resp = app.clone().oneshot(missing_limit_req).await.unwrap();
    assert_eq!(missing_limit_resp.status(), http::StatusCode::BAD_REQUEST);

    let zero_limit_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        "apps",
        "calc.v1",
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: Some("main"),
            payload: None,
            gas_asset_id: None,
            gas_limit: 0,
        },
    );
    let zero_limit_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/call")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(zero_limit_body))
        .unwrap();
    let zero_limit_resp = app.clone().oneshot(zero_limit_req).await.unwrap();
    assert_eq!(zero_limit_resp.status(), http::StatusCode::BAD_REQUEST);

    let payload = norito::json!({ "note": "integration-test" });
    let call_body = iroha_torii::test_utils::contract_call_request_json(
        &creds.account,
        &creds.private_key,
        "apps",
        "calc.v1",
        iroha_torii::test_utils::ContractCallOptions {
            entrypoint: Some("main"),
            payload: Some(&payload),
            gas_asset_id: None,
            gas_limit: 5_000,
        },
    );
    let call_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/call")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(call_body))
        .unwrap();
    let call_resp = app.clone().oneshot(call_req).await.unwrap();
    assert_eq!(call_resp.status(), http::StatusCode::OK);
    let call_bytes = call_resp.into_body().collect().await.unwrap().to_bytes();
    let call_json: json::Value = json::from_slice(&call_bytes).unwrap();
    assert!(
        call_json
            .get("ok")
            .and_then(json::Value::as_bool)
            .unwrap_or(false)
    );
    assert_eq!(
        call_json
            .get("namespace")
            .and_then(json::Value::as_str)
            .unwrap(),
        "apps"
    );
    assert_eq!(
        call_json
            .get("contract_id")
            .and_then(json::Value::as_str)
            .unwrap(),
        "calc.v1"
    );
    assert_eq!(
        call_json
            .get("code_hash_hex")
            .and_then(json::Value::as_str)
            .unwrap(),
        code_hash_hex
    );
    assert_eq!(
        call_json
            .get("abi_hash_hex")
            .and_then(json::Value::as_str)
            .unwrap(),
        abi_hash_hex
    );
    let tx_hash_hex = call_json
        .get("tx_hash_hex")
        .and_then(json::Value::as_str)
        .expect("tx_hash_hex present");
    assert_eq!(tx_hash_hex.len(), 64);
    assert_eq!(
        call_json
            .get("submitted")
            .and_then(json::Value::as_bool)
            .unwrap_or(false),
        true
    );

    let applied_call =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 3);
    assert_eq!(applied_call, 1);

    let draft_payload = norito::json!({ "note": "client-signed" });
    let draft_body = iroha_torii::json_object(vec![
        iroha_torii::json_entry("authority", creds.account.clone()),
        iroha_torii::json_entry("namespace", "apps"),
        iroha_torii::json_entry("contract_id", "calc.v1"),
        iroha_torii::json_entry("entrypoint", "main"),
        iroha_torii::json_entry("payload", draft_payload),
        iroha_torii::json_entry("gas_limit", 5_000u64),
    ]);
    let draft_body = json::to_json(&draft_body).expect("serialize draft call request");
    let draft_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/call")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(draft_body))
        .unwrap();
    let draft_resp = app.clone().oneshot(draft_req).await.unwrap();
    assert_eq!(draft_resp.status(), http::StatusCode::OK);
    let draft_bytes = draft_resp.into_body().collect().await.unwrap().to_bytes();
    let draft_json: json::Value = json::from_slice(&draft_bytes).unwrap();
    assert_eq!(
        draft_json
            .get("submitted")
            .and_then(json::Value::as_bool)
            .unwrap_or(true),
        false
    );
    assert!(
        draft_json
            .get("signed_transaction_b64")
            .and_then(json::Value::as_str)
            .is_some(),
        "expected contract call draft scaffold when private_key is omitted"
    );
    let signing_message_b64 = draft_json
        .get("signing_message_b64")
        .and_then(json::Value::as_str)
        .expect("signing_message_b64 present");
    let creation_time_ms = draft_json
        .get("creation_time_ms")
        .and_then(json::Value::as_u64)
        .expect("creation_time_ms present");
    let signing_message = base64::engine::general_purpose::STANDARD
        .decode(signing_message_b64)
        .expect("decode signing message");
    let detached_signature = Signature::new(&creds.private_key.0, &signing_message);
    let detached_submit_body = iroha_torii::json_object(vec![
        iroha_torii::json_entry("authority", creds.account.clone()),
        iroha_torii::json_entry(
            "public_key_hex",
            hex::encode_upper(
                iroha_crypto::PublicKey::from(creds.private_key.0.clone())
                    .to_bytes()
                    .1,
            ),
        ),
        iroha_torii::json_entry(
            "signature_b64",
            base64::engine::general_purpose::STANDARD.encode(detached_signature.payload()),
        ),
        iroha_torii::json_entry("namespace", "apps"),
        iroha_torii::json_entry("contract_id", "calc.v1"),
        iroha_torii::json_entry("entrypoint", "main"),
        iroha_torii::json_entry("payload", draft_payload),
        iroha_torii::json_entry("creation_time_ms", creation_time_ms),
        iroha_torii::json_entry("gas_limit", 5_000u64),
    ]);
    let detached_submit_body =
        json::to_json(&detached_submit_body).expect("serialize detached submit request");
    let detached_submit_req = http::Request::builder()
        .method("POST")
        .uri("/v1/contracts/call")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(detached_submit_body))
        .unwrap();
    let detached_submit_resp = app.clone().oneshot(detached_submit_req).await.unwrap();
    assert_eq!(detached_submit_resp.status(), http::StatusCode::OK);
    let detached_submit_bytes = detached_submit_resp
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let detached_submit_json: json::Value = json::from_slice(&detached_submit_bytes).unwrap();
    assert_eq!(
        detached_submit_json
            .get("submitted")
            .and_then(json::Value::as_bool)
            .unwrap_or(false),
        true
    );
    let detached_submit_hash = detached_submit_json
        .get("tx_hash_hex")
        .and_then(json::Value::as_str)
        .expect("detached submit tx hash present");
    assert_eq!(detached_submit_hash.len(), 64);
    assert!(
        draft_json.get("tx_hash_hex").is_none()
            || draft_json
                .get("tx_hash_hex")
                .is_some_and(json::Value::is_null)
    );

    let applied_detached_submit =
        iroha_torii::test_utils::apply_queued_in_one_block(&state, &queue, &chain_id, 4);
    assert_eq!(applied_detached_submit, 1);
}
