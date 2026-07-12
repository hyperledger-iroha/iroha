#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Router-level test for GET /v1/sumeragi/rbc/sessions.
#![cfg(feature = "telemetry")]

#[tokio::test]
async fn rbc_sessions_endpoint_reports_retired_snapshot_as_empty() {
    use axum::{Router, routing::get};
    use tower::ServiceExt;

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
    let value: norito::json::Value =
        norito::json::from_str(&String::from_utf8(body.to_vec()).unwrap()).unwrap();

    assert_eq!(value["sessions_active"].as_u64(), Some(0));
    assert!(
        value["items"]
            .as_array()
            .is_some_and(|items| items.is_empty())
    );
}
