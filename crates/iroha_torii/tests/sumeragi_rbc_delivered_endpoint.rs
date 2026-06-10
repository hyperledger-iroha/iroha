#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Router-level test for GET /v1/sumeragi/rbc/delivered/{height}/{view}
#![cfg(feature = "telemetry")]

#[tokio::test]
async fn sumeragi_rbc_delivered_endpoint_shape() {
    use std::time::SystemTime;

    use axum::{Router, routing::get};
    use iroha_core::sumeragi::rbc_status;
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::block::BlockHeader;
    use tower::ServiceExt;

    let _guard = crate::rbc_status_test_guard();

    // Seed a delivered RBC session summary for (height=10, view=2)
    let bh = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([1u8; 32]));
    let handle = rbc_status::register_handle();
    rbc_status::set_active(&handle);
    let summary = rbc_status::Summary {
        block_hash: bh,
        height: 10,
        view: 2,
        total_chunks: 3,
        encoding: iroha_data_model::block::consensus::RbcEncoding::Plain,
        data_shards: 0,
        parity_shards: 0,
        received_chunks: 3,
        ready_count: 3,
        delivered: true,
        payload_hash: None,
        recovered_from_disk: false,
        invalid: false,
        reconstructed_stripes: 0,
        reconstructable_stripes: 0,
        lane_backlog: Vec::new(),
        dataspace_backlog: Vec::new(),
    };
    handle.update(summary, SystemTime::now());

    // Build a tiny router with the delivered endpoint handler
    let app = Router::new().route(
        "/v1/sumeragi/rbc/delivered/{height}/{view}",
        get(|path: axum::extract::Path<(u64, u64)>| async move {
            iroha_torii::handle_v1_sumeragi_rbc_delivered_height_view(path).await
        }),
    );

    let resp = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/sumeragi/rbc/delivered/10/2")
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

#[tokio::test]
async fn sumeragi_rbc_delivered_endpoint_requires_complete_chunk_evidence() {
    use std::time::SystemTime;

    use axum::{Router, routing::get};
    use iroha_core::sumeragi::rbc_status;
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::block::BlockHeader;
    use tower::ServiceExt;

    let _guard = crate::rbc_status_test_guard();

    let handle = rbc_status::register_handle();
    rbc_status::set_active(&handle);
    let incomplete_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([2u8; 32]));
    handle.update(
        rbc_status::Summary {
            block_hash: incomplete_hash,
            height: 11,
            view: 3,
            total_chunks: 4,
            encoding: iroha_data_model::block::consensus::RbcEncoding::Plain,
            data_shards: 0,
            parity_shards: 0,
            received_chunks: 3,
            ready_count: 5,
            delivered: true,
            payload_hash: None,
            recovered_from_disk: false,
            invalid: false,
            reconstructed_stripes: 0,
            reconstructable_stripes: 0,
            lane_backlog: Vec::new(),
            dataspace_backlog: Vec::new(),
        },
        SystemTime::now(),
    );

    let app = Router::new().route(
        "/v1/sumeragi/rbc/delivered/{height}/{view}",
        get(|path: axum::extract::Path<(u64, u64)>| async move {
            iroha_torii::handle_v1_sumeragi_rbc_delivered_height_view(path).await
        }),
    );

    let resp = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/sumeragi/rbc/delivered/11/3")
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
    let v: norito::json::Value =
        norito::json::from_str(&String::from_utf8(body.to_vec()).unwrap()).unwrap();
    assert!(v["present"].as_bool().unwrap());
    assert!(
        !v["delivered"].as_bool().unwrap(),
        "missing local chunks must not satisfy delivered endpoint evidence"
    );
    assert_eq!(
        v["received_chunks"].as_u64().unwrap(),
        3,
        "malformed delivered rows should remain visible as diagnostics"
    );

    let complete_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([3u8; 32]));
    handle.update(
        rbc_status::Summary {
            block_hash: complete_hash,
            height: 11,
            view: 3,
            total_chunks: 4,
            encoding: iroha_data_model::block::consensus::RbcEncoding::Plain,
            data_shards: 0,
            parity_shards: 0,
            received_chunks: 4,
            ready_count: 3,
            delivered: true,
            payload_hash: None,
            recovered_from_disk: false,
            invalid: false,
            reconstructed_stripes: 0,
            reconstructable_stripes: 0,
            lane_backlog: Vec::new(),
            dataspace_backlog: Vec::new(),
        },
        SystemTime::now(),
    );

    let resp = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/sumeragi/rbc/delivered/11/3")
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
    let v: norito::json::Value =
        norito::json::from_str(&String::from_utf8(body.to_vec()).unwrap()).unwrap();
    assert!(
        v["delivered"].as_bool().unwrap(),
        "complete delivered evidence should still satisfy the endpoint"
    );
    assert_eq!(v["received_chunks"].as_u64().unwrap(), 4);
    assert_eq!(v["total_chunks"].as_u64().unwrap(), 4);
}

#[tokio::test]
async fn sumeragi_rbc_delivered_endpoint_rejects_invalid_zero_and_overcount_chunk_evidence() {
    use std::time::SystemTime;

    use axum::{Router, routing::get};
    use iroha_core::sumeragi::rbc_status;
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::block::BlockHeader;
    use tower::ServiceExt;

    let _guard = crate::rbc_status_test_guard();

    let handle = rbc_status::register_handle();
    rbc_status::set_active(&handle);
    let invalid_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([4u8; 32]));
    handle.update(
        rbc_status::Summary {
            block_hash: invalid_hash,
            height: 12,
            view: 4,
            total_chunks: 2,
            encoding: iroha_data_model::block::consensus::RbcEncoding::Plain,
            data_shards: 0,
            parity_shards: 0,
            received_chunks: 2,
            ready_count: 5,
            delivered: true,
            payload_hash: None,
            recovered_from_disk: false,
            invalid: true,
            reconstructed_stripes: 0,
            reconstructable_stripes: 0,
            lane_backlog: Vec::new(),
            dataspace_backlog: Vec::new(),
        },
        SystemTime::now(),
    );
    let zero_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([5u8; 32]));
    handle.update(
        rbc_status::Summary {
            block_hash: zero_hash,
            height: 13,
            view: 5,
            total_chunks: 0,
            encoding: iroha_data_model::block::consensus::RbcEncoding::Plain,
            data_shards: 0,
            parity_shards: 0,
            received_chunks: 0,
            ready_count: 5,
            delivered: true,
            payload_hash: None,
            recovered_from_disk: false,
            invalid: false,
            reconstructed_stripes: 0,
            reconstructable_stripes: 0,
            lane_backlog: Vec::new(),
            dataspace_backlog: Vec::new(),
        },
        SystemTime::now(),
    );
    let overcount_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([6u8; 32]));
    handle.update(
        rbc_status::Summary {
            block_hash: overcount_hash,
            height: 14,
            view: 6,
            total_chunks: 2,
            encoding: iroha_data_model::block::consensus::RbcEncoding::Plain,
            data_shards: 0,
            parity_shards: 0,
            received_chunks: 3,
            ready_count: 5,
            delivered: true,
            payload_hash: None,
            recovered_from_disk: false,
            invalid: false,
            reconstructed_stripes: 0,
            reconstructable_stripes: 0,
            lane_backlog: Vec::new(),
            dataspace_backlog: Vec::new(),
        },
        SystemTime::now(),
    );

    let app = Router::new().route(
        "/v1/sumeragi/rbc/delivered/{height}/{view}",
        get(|path: axum::extract::Path<(u64, u64)>| async move {
            iroha_torii::handle_v1_sumeragi_rbc_delivered_height_view(path).await
        }),
    );

    let resp = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/sumeragi/rbc/delivered/12/4")
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
    let v: norito::json::Value =
        norito::json::from_str(&String::from_utf8(body.to_vec()).unwrap()).unwrap();
    assert!(v["present"].as_bool().unwrap());
    assert!(
        !v["delivered"].as_bool().unwrap(),
        "invalid sessions must remain diagnostic-only even with complete chunks"
    );
    assert_eq!(v["received_chunks"].as_u64().unwrap(), 2);
    assert_eq!(v["total_chunks"].as_u64().unwrap(), 2);

    let resp = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/sumeragi/rbc/delivered/13/5")
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
    let v: norito::json::Value =
        norito::json::from_str(&String::from_utf8(body.to_vec()).unwrap()).unwrap();
    assert!(v["present"].as_bool().unwrap());
    assert!(
        !v["delivered"].as_bool().unwrap(),
        "zero-chunk summaries must not satisfy delivered endpoint evidence"
    );
    assert_eq!(v["received_chunks"].as_u64().unwrap(), 0);
    assert_eq!(v["total_chunks"].as_u64().unwrap(), 0);

    let resp = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/sumeragi/rbc/delivered/14/6")
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
    let v: norito::json::Value =
        norito::json::from_str(&String::from_utf8(body.to_vec()).unwrap()).unwrap();
    assert!(v["present"].as_bool().unwrap());
    assert!(
        !v["delivered"].as_bool().unwrap(),
        "over-counted summaries must not satisfy delivered endpoint evidence"
    );
    assert_eq!(v["received_chunks"].as_u64().unwrap(), 3);
    assert_eq!(v["total_chunks"].as_u64().unwrap(), 2);
}
