#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration test for /v2/zk/verify minimal handler.
#![allow(clippy::redundant_closure_for_method_calls)]
#![cfg(feature = "app_api")]

use axum::{Router, routing::post};
use http_body_util::BodyExt as _;
use tower::ServiceExt as _;

#[tokio::test]
async fn zk_verify_endpoint_accepts_json_and_norito() {
    // Router with verify handler
    let app = Router::new().route(
        "/v2/zk/verify",
        post(
            |headers: axum::http::HeaderMap, body: axum::body::Bytes| async move {
                iroha_torii::handle_v1_zk_verify(headers, body).await
            },
        ),
    );

    // JSON path
    let body_json_value = iroha_torii::json_object(vec![
        iroha_torii::json_entry("backend", "demo"),
        iroha_torii::json_entry(
            "proof",
            iroha_torii::json_object(vec![iroha_torii::json_entry("bytes", vec![1u8, 2u8, 3u8])]),
        ),
        iroha_torii::json_entry(
            "vk",
            iroha_torii::json_object(vec![iroha_torii::json_entry("bytes", vec![4u8, 5u8, 6u8])]),
        ),
    ]);
    let body_json = norito::json::to_string(&body_json_value).expect("serialize body json");
    let req_json = http::Request::builder()
        .method("POST")
        .uri("/v2/zk/verify")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body_json))
        .unwrap();
    let resp_json = app.clone().oneshot(req_json).await.unwrap();
    assert_eq!(resp_json.status(), http::StatusCode::OK);
    let bytes = resp_json.into_body().collect().await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    assert_eq!(v.get("ok").and_then(|x| x.as_bool()), Some(true));

    // Norito path: any non-empty bytes accepted by minimal handler
    let req_norito = http::Request::builder()
        .method("POST")
        .uri("/v2/zk/verify")
        .header(http::header::CONTENT_TYPE, "application/x-norito")
        .body(axum::body::Body::from(vec![0x01, 0x02, 0x03]))
        .unwrap();
    let resp_norito = app.clone().oneshot(req_norito).await.unwrap();
    assert_eq!(resp_norito.status(), http::StatusCode::OK);
    let bytes2 = resp_norito.into_body().collect().await.unwrap().to_bytes();
    let v2: norito::json::Value = norito::json::from_slice(&bytes2).unwrap();
    assert_eq!(v2.get("ok").and_then(|x| x.as_bool()), Some(true));

    // Norito path negative: empty body should return ok=false
    let req_empty = http::Request::builder()
        .method("POST")
        .uri("/v2/zk/verify")
        .header(http::header::CONTENT_TYPE, "application/x-norito")
        .body(axum::body::Body::from(Vec::<u8>::new()))
        .unwrap();
    let resp_empty = app.clone().oneshot(req_empty).await.unwrap();
    assert_eq!(resp_empty.status(), http::StatusCode::OK);
    let bytes3 = resp_empty.into_body().collect().await.unwrap().to_bytes();
    let v3: norito::json::Value = norito::json::from_slice(&bytes3).unwrap();
    assert_eq!(v3.get("ok").and_then(|x| x.as_bool()), Some(false));
}
