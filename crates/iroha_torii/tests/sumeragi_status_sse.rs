#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Basic shape test for `/v2/sumeragi/status/sse`
#![cfg(feature = "telemetry")]
#![allow(unexpected_cfgs)]

#[tokio::test]
async fn sumeragi_status_sse_content_type() {
    use axum::{Router, routing::get};
    use iroha_core::sumeragi::status;
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::block::BlockHeader;
    use tower::ServiceExt;

    // Seed minimal snapshot values so the SSE stream emits meaningful payloads
    status::set_leader_index(3);
    status::set_highest_qc(12, 1);
    status::set_highest_qc_hash(HashOf::from_untyped_unchecked(Hash::prehashed(
        [0xAA; Hash::LENGTH],
    )));
    status::set_locked_qc(
        11,
        0,
        Some(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([0xBB; Hash::LENGTH]),
        )),
    );

    let app = Router::new().route(
        "/v2/sumeragi/status/sse",
        get(|| async move { iroha_torii::handle_v1_sumeragi_status_sse(200, true) }),
    );

    let resp = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/v2/sumeragi/status/sse")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
    let ct = resp
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    assert!(ct.contains("text/event-stream"));
}
