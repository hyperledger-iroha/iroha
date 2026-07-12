#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Router-level test for GET /v1/sumeragi/rbc/delivered/{height}/{view}.
#![cfg(feature = "telemetry")]

#[tokio::test]
async fn sumeragi_rbc_delivered_endpoint_reports_retired_snapshot_as_absent() {
    use axum::{Router, routing::get};
    use tower::ServiceExt;

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
    let value: norito::json::Value =
        norito::json::from_str(&String::from_utf8(body.to_vec()).unwrap()).unwrap();

    assert_eq!(value["height"].as_u64(), Some(10));
    assert_eq!(value["view"].as_u64(), Some(2));
    assert_eq!(value["present"].as_bool(), Some(false));
    assert_eq!(value["delivered"].as_bool(), Some(false));
    assert!(value["block_hash"].is_null());
    assert_eq!(value["ready_count"].as_u64(), Some(0));
    assert_eq!(value["received_chunks"].as_u64(), Some(0));
    assert_eq!(value["total_chunks"].as_u64(), Some(0));
}
