#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Router-level test for GET /v1/sumeragi/rbc/sessions
#![cfg(feature = "telemetry")]

#[tokio::test]
async fn rbc_sessions_endpoint_shape() {
    use std::time::SystemTime;

    use axum::{Router, routing::get};
    use tower::ServiceExt;

    let _guard = crate::rbc_status_test_guard();

    // Seed one session into the global snapshot
    let bh = iroha_crypto::HashOf::<iroha_data_model::block::BlockHeader>::from_untyped_unchecked(
        iroha_crypto::Hash::prehashed([7u8; 32]),
    );
    let handle = iroha_core::sumeragi::rbc_status::register_handle();
    iroha_core::sumeragi::rbc_status::set_active(&handle);
    let summary = iroha_core::sumeragi::rbc_status::Summary {
        block_hash: bh,
        height: 5,
        view: 1,
        total_chunks: 2,
        encoding: iroha_data_model::block::consensus::RbcEncoding::Plain,
        data_shards: 0,
        parity_shards: 0,
        received_chunks: 1,
        ready_count: 0,
        delivered: false,
        payload_hash: None,
        recovered_from_disk: false,
        invalid: false,
        reconstructed_stripes: 0,
        reconstructable_stripes: 0,
        lane_backlog: Vec::new(),
        dataspace_backlog: Vec::new(),
    };
    handle.update(summary, SystemTime::now());

    let app = Router::new().route(
        "/v1/sumeragi/rbc/sessions",
        get(|| async move { iroha_torii::handle_v1_sumeragi_rbc_sessions().await }),
    );

    let resp = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/sumeragi/rbc/sessions")
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
    let v: norito::json::Value = norito::json::from_str(&s).unwrap();
    assert!(v.get("sessions_active").is_some());
    let items = v.get("items").and_then(|x| x.as_array()).unwrap();
    assert!(!items.is_empty());
    let first = items[0].as_object().unwrap();
    assert!(first.get("block_hash").is_some());
    assert!(first.get("height").is_some());
    assert!(first.get("view").is_some());
    assert!(first.get("total_chunks").is_some());
    assert!(first.get("received_chunks").is_some());
    assert!(first.get("ready_count").is_some());
    assert!(first.get("delivered").is_some());
    assert!(first.get("complete_delivery").is_some());
    assert!(first.get("recovered").is_some());
    assert!(first.get("invalid").is_some());
    let lane_backlog = first
        .get("lane_backlog")
        .and_then(|v| v.as_array())
        .expect("lane backlog array present");
    assert!(
        lane_backlog.is_empty(),
        "lane backlog should be empty for seeded stub"
    );
    let dataspace_backlog = first
        .get("dataspace_backlog")
        .and_then(|v| v.as_array())
        .expect("dataspace backlog array present");
    assert!(
        dataspace_backlog.is_empty(),
        "dataspace backlog should be empty for seeded stub"
    );
}

#[tokio::test]
async fn rbc_sessions_endpoint_exposes_complete_delivery_separately_from_raw_delivered() {
    use std::time::SystemTime;

    use axum::{Router, routing::get};
    use iroha_core::sumeragi::rbc_status;
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::block::BlockHeader;
    use tower::ServiceExt;

    let _guard = crate::rbc_status_test_guard();

    let handle = rbc_status::register_handle();
    rbc_status::set_active(&handle);
    let summary = |hash_byte: u8,
                   height: u64,
                   total_chunks: u32,
                   received_chunks: u32,
                   delivered: bool,
                   invalid: bool| {
        rbc_status::Summary {
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                [hash_byte; 32],
            )),
            height,
            view: 1,
            total_chunks,
            encoding: iroha_data_model::block::consensus::RbcEncoding::Plain,
            data_shards: 0,
            parity_shards: 0,
            received_chunks,
            ready_count: 5,
            delivered,
            payload_hash: None,
            recovered_from_disk: false,
            invalid,
            reconstructed_stripes: 0,
            reconstructable_stripes: 0,
            lane_backlog: Vec::new(),
            dataspace_backlog: Vec::new(),
        }
    };
    handle.update(summary(8, 6, 2, 2, true, false), SystemTime::now());
    handle.update(summary(9, 7, 2, 1, true, false), SystemTime::now());
    handle.update(summary(10, 8, 2, 2, true, true), SystemTime::now());
    handle.update(summary(11, 9, 2, 3, true, false), SystemTime::now());

    let app = Router::new().route(
        "/v1/sumeragi/rbc/sessions",
        get(|| async move { iroha_torii::handle_v1_sumeragi_rbc_sessions().await }),
    );

    let resp = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/sumeragi/rbc/sessions")
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
    let v: norito::json::Value = norito::json::from_str(&s).unwrap();
    let items = v.get("items").and_then(|x| x.as_array()).unwrap();

    let complete = items
        .iter()
        .find(|item| item["height"].as_u64() == Some(6))
        .expect("complete delivered summary present");
    assert!(complete["delivered"].as_bool().unwrap());
    assert!(complete["complete_delivery"].as_bool().unwrap());

    let incomplete = items
        .iter()
        .find(|item| item["height"].as_u64() == Some(7))
        .expect("raw delivered incomplete summary present");
    assert!(incomplete["delivered"].as_bool().unwrap());
    assert!(
        !incomplete["complete_delivery"].as_bool().unwrap(),
        "raw DELIVER without complete chunks must remain distinguishable"
    );

    let invalid = items
        .iter()
        .find(|item| item["height"].as_u64() == Some(8))
        .expect("raw delivered invalid summary present");
    assert!(invalid["delivered"].as_bool().unwrap());
    assert!(invalid["invalid"].as_bool().unwrap());
    assert!(
        !invalid["complete_delivery"].as_bool().unwrap(),
        "invalid sessions must remain diagnostic-only even with complete chunks"
    );

    let overcounted = items
        .iter()
        .find(|item| item["height"].as_u64() == Some(9))
        .expect("raw delivered over-counted summary present");
    assert!(overcounted["delivered"].as_bool().unwrap());
    assert_eq!(overcounted["received_chunks"].as_u64(), Some(3));
    assert_eq!(overcounted["total_chunks"].as_u64(), Some(2));
    assert!(
        !overcounted["complete_delivery"].as_bool().unwrap(),
        "over-counted sessions must remain diagnostic-only"
    );
}
