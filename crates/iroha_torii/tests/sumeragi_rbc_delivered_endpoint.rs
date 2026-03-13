#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Router-level test for GET /v2/sumeragi/rbc/delivered/{height}/{view}
#![cfg(feature = "telemetry")]

#[tokio::test]
async fn sumeragi_rbc_delivered_endpoint_shape() {
    use std::time::SystemTime;

    use axum::{Router, routing::get};
    use iroha_core::sumeragi::rbc_status;
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::block::BlockHeader;
    use tower::ServiceExt;

    // Seed a delivered RBC session summary for (height=10, view=2)
    let bh = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([1u8; 32]));
    let handle = rbc_status::register_handle();
    rbc_status::set_active(&handle);
    let summary = rbc_status::Summary {
        block_hash: bh,
        height: 10,
        view: 2,
        total_chunks: 3,
        received_chunks: 3,
        ready_count: 3,
        delivered: true,
        payload_hash: None,
        recovered_from_disk: false,
        invalid: false,
        lane_backlog: Vec::new(),
        dataspace_backlog: Vec::new(),
    };
    handle.update(summary, SystemTime::now());

    // Build a tiny router with the delivered endpoint handler
    let app = Router::new().route(
        "/v2/sumeragi/rbc/delivered/{height}/{view}",
        get(|path: axum::extract::Path<(u64, u64)>| async move {
            iroha_torii::handle_v1_sumeragi_rbc_delivered_height_view(path).await
        }),
    );

    let resp = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/v2/sumeragi/rbc/delivered/10/2")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);

    let body = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    let s = String::from_utf8(body.to_vec()).unwrap();

    // Parse JSON and check expected keys
    let v: norito::json::Value = norito::json::from_str(&s).unwrap();
    assert_eq!(v["height"].as_u64().unwrap(), 10);
    assert_eq!(v["view"].as_u64().unwrap(), 2);
    assert!(v["present"].as_bool().unwrap());
    assert!(v["delivered"].as_bool().unwrap());
}
