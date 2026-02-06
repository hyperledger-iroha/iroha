#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Router-level test for GET /v1/sumeragi/leader
#![allow(clippy::redundant_closure_for_method_calls)]
#![cfg(feature = "telemetry")]

#[tokio::test]
async fn sumeragi_leader_endpoint_shape() {
    use axum::{Router, routing::get};
    use iroha_core::sumeragi::status;
    use tower::ServiceExt;

    // Seed status
    status::set_leader_index(2);
    status::set_prf_context([7u8; 32], 123, 4);

    let app = Router::new().route(
        "/v1/sumeragi/leader",
        get(|| async move { iroha_torii::handle_v1_sumeragi_leader(None).await }),
    );

    let resp = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/sumeragi/leader")
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
    assert_eq!(v.get("leader_index").and_then(|x| x.as_u64()), Some(2));
    assert!(v.get("prf").is_some());
    let prf = v.get("prf").unwrap();
    assert_eq!(prf.get("height").and_then(|x| x.as_u64()), Some(123));
    assert_eq!(prf.get("view").and_then(|x| x.as_u64()), Some(4));
}
