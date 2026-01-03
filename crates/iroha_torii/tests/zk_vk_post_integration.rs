//! Integration tests for POST VK registry endpoints (`app_api`).
#![cfg(feature = "app_api")]
#![allow(clippy::too_many_lines)]

use std::sync::Arc;

use axum::{Router, routing::post};
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    queue::Queue,
    state::{State, World},
};
use iroha_data_model::{account::AccountId, prelude::ChainId};
use iroha_torii::NoritoJson;
use nonzero_ext::nonzero;
use norito::json;
use tower::ServiceExt as _;

#[tokio::test]
async fn vk_register_update_return_202() {
    // Minimal state and queue
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let state = State::new(
        World::new(),
        kura.clone(),
        query,
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let state = State::new(World::new(), kura.clone(), query);
    let state = Arc::new(state);
    let chain_id = Arc::new(ChainId::from("test-chain"));
    let queue_cfg = iroha_config::parameters::actual::Queue {
        capacity: nonzero!(8usize),
        capacity_per_user: nonzero!(8usize),
        transaction_time_to_live: core::time::Duration::from_mins(1),
    };
    let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(4).0;
    let queue = Arc::new(Queue::from_config(queue_cfg, events_sender));
    #[cfg(feature = "telemetry")]
    let telemetry = iroha_torii::MaybeTelemetry::for_tests();
    #[cfg(not(feature = "telemetry"))]
    let telemetry = iroha_torii::MaybeTelemetry::disabled();

    // Build routes that capture the chain_id/queue/state
    let app = Router::new()
        .route(
            "/v1/zk/vk/register",
            post({
                let chain_id = chain_id.clone();
                let queue = queue.clone();
                let state = state.clone();
                let telemetry = telemetry.clone();
                move |req: NoritoJson<iroha_torii::ZkVkRegisterDto>| {
                    let chain_id = chain_id.clone();
                    let queue = queue.clone();
                    let state = state.clone();
                    let telemetry = telemetry.clone();
                    async move {
                        iroha_torii::handle_post_vk_register(chain_id, queue, state, telemetry, req)
                            .await
                    }
                }
            }),
        )
        .route(
            "/v1/zk/vk/update",
            post({
                let chain_id = chain_id.clone();
                let queue = queue.clone();
                let state = state.clone();
                let telemetry = telemetry.clone();
                move |req: NoritoJson<iroha_torii::ZkVkUpdateDto>| {
                    let chain_id = chain_id.clone();
                    let queue = queue.clone();
                    let state = state.clone();
                    let telemetry = telemetry.clone();
                    async move {
                        iroha_torii::handle_post_vk_update(chain_id, queue, state, telemetry, req)
                            .await
                    }
                }
            }),
        );

    // Helper: build headers
    let _json_ct = {
        let mut h = axum::http::HeaderMap::new();
        h.insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/json"),
        );
        h
    };

    // Prepare a keypair whose public key matches the authority account id.
    let kp = iroha_crypto::KeyPair::random();
    let exposed = iroha_crypto::ExposedPrivateKey(kp.private_key().clone()).to_string();
    let authority: AccountId = format!("{}@wonderland", kp.public_key())
        .parse()
        .expect("valid account id");

    // 1) Register (vk_bytes omitted; provide commitment_hex only)
    let body_reg_value = iroha_torii::json_object(vec![
        iroha_torii::json_entry("authority", authority.clone().to_string()),
        iroha_torii::json_entry("private_key", exposed.clone()),
        iroha_torii::json_entry("backend", "halo2/ipa"),
        iroha_torii::json_entry("name", "vk_add"),
        iroha_torii::json_entry("version", 1u64),
        iroha_torii::json_entry("circuit_id", "circuit_alpha"),
        iroha_torii::json_entry(
            "public_inputs_schema_hex",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ),
        iroha_torii::json_entry("gas_schedule_id", "halo2_default"),
        iroha_torii::json_entry("vk_len", 1024u64),
        iroha_torii::json_entry(
            "commitment_hex",
            "0000000000000000000000000000000000000000000000000000000000000000",
        ),
    ]);
    let body_reg = json::to_json(&body_reg_value).unwrap();
    let req_reg = http::Request::builder()
        .method("POST")
        .uri("/v1/zk/vk/register")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body_reg))
        .unwrap();
    let resp_reg = app.clone().oneshot(req_reg).await.unwrap();
    assert_eq!(resp_reg.status(), http::StatusCode::ACCEPTED);

    // 2) Update (version increments)
    let body_upd_value = iroha_torii::json_object(vec![
        iroha_torii::json_entry("authority", authority.clone().to_string()),
        iroha_torii::json_entry("private_key", exposed.clone()),
        iroha_torii::json_entry("backend", "halo2/ipa"),
        iroha_torii::json_entry("name", "vk_add"),
        iroha_torii::json_entry("version", 2u64),
        iroha_torii::json_entry("circuit_id", "circuit_alpha"),
        iroha_torii::json_entry(
            "public_inputs_schema_hex",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        ),
        iroha_torii::json_entry("gas_schedule_id", "halo2_default"),
        iroha_torii::json_entry("vk_len", 1024u64),
        iroha_torii::json_entry(
            "commitment_hex",
            "1111111111111111111111111111111111111111111111111111111111111111",
        ),
    ]);
    let body_upd = json::to_json(&body_upd_value).unwrap();
    let req_upd = http::Request::builder()
        .method("POST")
        .uri("/v1/zk/vk/update")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body_upd))
        .unwrap();
    let resp_upd = app.clone().oneshot(req_upd).await.unwrap();
    assert_eq!(resp_upd.status(), http::StatusCode::ACCEPTED);

}
