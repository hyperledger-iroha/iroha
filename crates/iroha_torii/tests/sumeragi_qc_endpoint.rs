#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Router-level test for GET /v1/sumeragi/qc
#![allow(clippy::redundant_closure_for_method_calls)]
#![cfg(feature = "telemetry")]

#[tokio::test]
async fn sumeragi_qc_endpoint_shape() {
    use axum::{Router, routing::get};
    use iroha_core::sumeragi::status;
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::block::BlockHeader;
    use tower::ServiceExt;

    // Seed status
    status::set_highest_qc(11, 3);
    let locked_hash =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([2; Hash::LENGTH]));
    status::set_locked_qc(10, 2, Some(locked_hash));
    // Seed subject hash
    let h = Hash::prehashed([0xAB; 32]);
    let typed =
        iroha_crypto::HashOf::<iroha_data_model::block::BlockHeader>::from_untyped_unchecked(h);
    status::set_highest_qc_hash(typed);

    let app = Router::new().route(
        "/v1/sumeragi/qc",
        get(|| async move { iroha_torii::handle_v1_sumeragi_qc(None).await }),
    );

    let resp = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/sumeragi/qc")
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
    let v: norito::json::Value = norito::json::from_slice(&body).unwrap();
    assert!(v.get("highest_qc").is_some());
    assert!(v.get("locked_qc").is_some());
    let hq = v.get("highest_qc").unwrap();
    assert_eq!(hq.get("height").and_then(|x| x.as_u64()), Some(11));
    assert_eq!(hq.get("view").and_then(|x| x.as_u64()), Some(3));
    assert!(hq.get("subject_block_hash").is_some());
    let lq = v.get("locked_qc").unwrap();
    assert_eq!(
        lq.get("subject_block_hash")
            .and_then(|x| x.as_str())
            .map(ToString::to_string),
        Some(format!("{locked_hash}"))
    );
}

#[tokio::test]
async fn sumeragi_qc_endpoint_supports_norito_payload() {
    use axum::{Router, routing::get};
    use iroha_core::sumeragi::status;
    use iroha_data_model::block::consensus::SumeragiQcSnapshot;
    use tower::ServiceExt;

    status::set_highest_qc(6, 2);
    status::set_locked_qc(5, 1, None);

    let app = Router::new().route(
        "/v1/sumeragi/qc",
        get(|| async move { iroha_torii::handle_v1_sumeragi_qc(None).await }),
    );

    let resp = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/sumeragi/qc")
                .header("Accept", "application/x-norito")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .and_then(|h| h.to_str().ok()),
        Some("application/x-norito")
    );

    let bytes = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    let snapshot: SumeragiQcSnapshot =
        norito::decode_from_bytes(&bytes).expect("QC Norito payload decodes");
    assert_eq!(snapshot.highest_qc.height, 6);
    assert_eq!(snapshot.highest_qc.view, 2);
    assert_eq!(snapshot.locked_qc.height, 5);
    assert_eq!(snapshot.locked_qc.view, 1);
}
