#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration test for /v1/zk/submit-proof minimal handler.
#![allow(clippy::redundant_closure_for_method_calls)]
#![cfg(feature = "app_api")]

use axum::{Router, routing::post};
use http_body_util::BodyExt as _;
use norito::json;
use tower::ServiceExt as _;

#[tokio::test]
async fn zk_submit_proof_endpoint_accepts_json_and_norito_and_returns_id() {
    // Router with submit-proof handler
    let app = Router::new().route(
        "/v1/zk/submit-proof",
        post(
            |headers: axum::http::HeaderMap, body: axum::body::Bytes| async move {
                iroha_torii::handle_v1_zk_submit_proof(headers, body).await
            },
        ),
    );

    // JSON path
    let body_json_value = iroha_torii::json_object(vec![
        iroha_torii::json_entry("backend", "demo"),
        iroha_torii::json_entry(
            "proof",
            iroha_torii::json_object(vec![iroha_torii::json_entry(
                "bytes",
                iroha_torii::json_array(vec![1u8, 2u8, 3u8]),
            )]),
        ),
        iroha_torii::json_entry(
            "vk",
            iroha_torii::json_object(vec![iroha_torii::json_entry(
                "bytes",
                iroha_torii::json_array(vec![4u8, 5u8, 6u8]),
            )]),
        ),
    ]);
    let body_json = json::to_string(&body_json_value).expect("serialize json body");
    let req_json = http::Request::builder()
        .method("POST")
        .uri("/v1/zk/submit-proof")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body_json.clone()))
        .unwrap();
    let expected_id_json = {
        let h = iroha_crypto::Hash::new(body_json.as_bytes());
        hex::encode::<[u8; 32]>(h.into())
    };
    let resp_json = app.clone().oneshot(req_json).await.unwrap();
    assert_eq!(resp_json.status(), http::StatusCode::OK);
    let bytes = resp_json.into_body().collect().await.unwrap().to_bytes();
    let v: norito::json::Value = json::from_slice(&bytes).unwrap();
    assert_eq!(v.get("ok").and_then(|x| x.as_bool()), Some(true));
    assert_eq!(
        v.get("id").and_then(|x| x.as_str()),
        Some(expected_id_json.as_str())
    );

    // Norito path: any non-empty bytes accepted by minimal handler
    let norito_bytes = vec![0x01, 0x02, 0x03, 0x04];
    let expected_id_norito = {
        let h = iroha_crypto::Hash::new(&norito_bytes);
        hex::encode::<[u8; 32]>(h.into())
    };
    let req_norito = http::Request::builder()
        .method("POST")
        .uri("/v1/zk/submit-proof")
        .header(http::header::CONTENT_TYPE, "application/x-norito")
        .body(axum::body::Body::from(norito_bytes))
        .unwrap();
    let resp_norito = app.clone().oneshot(req_norito).await.unwrap();
    assert_eq!(resp_norito.status(), http::StatusCode::OK);
    let bytes2 = resp_norito.into_body().collect().await.unwrap().to_bytes();
    let v2: norito::json::Value = json::from_slice(&bytes2).unwrap();
    assert_eq!(v2.get("ok").and_then(|x| x.as_bool()), Some(true));
    assert_eq!(
        v2.get("id").and_then(|x| x.as_str()),
        Some(expected_id_norito.as_str())
    );

    // Norito path negative: empty body should return ok=false; id still present
    let req_empty = http::Request::builder()
        .method("POST")
        .uri("/v1/zk/submit-proof")
        .header(http::header::CONTENT_TYPE, "application/x-norito")
        .body(axum::body::Body::from(Vec::<u8>::new()))
        .unwrap();
    let resp_empty = app.clone().oneshot(req_empty).await.unwrap();
    assert_eq!(resp_empty.status(), http::StatusCode::OK);
    let bytes3 = resp_empty.into_body().collect().await.unwrap().to_bytes();
    let v3: norito::json::Value = json::from_slice(&bytes3).unwrap();
    assert_eq!(v3.get("ok").and_then(|x| x.as_bool()), Some(false));
    assert!(v3.get("id").and_then(|x| x.as_str()).is_some());
}
