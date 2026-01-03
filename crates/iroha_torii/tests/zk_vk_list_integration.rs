//! Integration tests for GET /v1/zk/vk (list with filters).
#![cfg(feature = "app_api")]

use std::sync::Arc;

use axum::{Router, routing::get};
use http_body_util::BodyExt as _;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};
use iroha_data_model::{
    confidential::ConfidentialStatus,
    proof::{VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord},
    zk::BackendTag,
};
use tower::ServiceExt as _;

#[tokio::test]
async fn vk_list_filters_by_backend_and_status() {
    // Build minimal state
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let state = State::new(
        World::new(),
        kura,
        query,
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let state = State::new(World::new(), kura, query);
    let mut state = state;

    // Insert 3 records: 2 active + 1 proposed
    // Insert via test helper
    let backend = "halo2/ipa";
    for (i, status) in [
        ConfidentialStatus::Active,
        ConfidentialStatus::Active,
        ConfidentialStatus::Proposed,
    ]
    .into_iter()
    .enumerate()
    {
        let name = format!("vk_{i}");
        let vk = VerifyingKeyBox::new(backend.into(), vec![u8::try_from(i).unwrap()]);
        let commitment = iroha_core::zk::hash_vk(&vk);
        let mut rec = VerifyingKeyRecord::new(
            1,
            format!("{backend}:{name}"),
            BackendTag::Halo2IpaPasta,
            "pallas",
            [u8::try_from(i + 1).unwrap_or(1); 32],
            commitment,
        );
        rec.vk_len = 1;
        rec.key = Some(vk);
        rec.status = status;
        rec.gas_schedule_id = Some("test_schedule".into());
        let id = VerifyingKeyId::new(backend, name);
        iroha_core::query::insert_verifying_key_record_for_test(&mut state, id, rec);
    }

    let state = Arc::new(state);
    let app = Router::new().route(
        "/v1/zk/vk",
        get({
            let state = state.clone();
            move |q: iroha_torii::NoritoQuery<iroha_torii::VkListQuery>| async move {
                iroha_torii::handle_list_vk(state, q).await
            }
        }),
    );

    // List all
    let req_all = http::Request::builder()
        .method("GET")
        .uri("/v1/zk/vk")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp_all = app.clone().oneshot(req_all).await.unwrap();
    assert_eq!(resp_all.status(), http::StatusCode::OK);
    let v_all: norito::json::Value =
        norito::json::from_slice(&resp_all.into_body().collect().await.unwrap().to_bytes())
            .unwrap();
    let arr_all = v_all.as_array().unwrap();
    assert_eq!(arr_all.len(), 3);

    // Filter Proposed
    let req_prop = http::Request::builder()
        .method("GET")
        .uri("/v1/zk/vk?status=Proposed")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp_prop = app.clone().oneshot(req_prop).await.unwrap();
    assert_eq!(resp_prop.status(), http::StatusCode::OK);
    let v_prop: norito::json::Value =
        norito::json::from_slice(&resp_prop.into_body().collect().await.unwrap().to_bytes())
            .unwrap();
    let arr_prop = v_prop.as_array().unwrap();
    assert_eq!(arr_prop.len(), 1);
}
