#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration tests for GET /v1/zk/proof/{backend}/{hash} (`app_api`).
#![allow(clippy::similar_names)]

use std::sync::Arc;

use axum::{Router, routing::get};
use http_body_util::BodyExt as _;
use iroha_config::parameters::defaults;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};
use iroha_data_model::proof::{ProofId, ProofRecord, ProofStatus, VerifyingKeyId};
use tower::ServiceExt as _;

fn proof_app_with_record(backend: &str, proof_hash: [u8; 32]) -> (Router, String, String) {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::new(), kura, query);
    let mut state = state;

    let id = ProofId {
        backend: backend.into(),
        proof_hash,
    };
    let record = ProofRecord {
        id: id.clone(),
        vk_ref: Some(VerifyingKeyId::new("halo2/ipa", "vk_test")),
        vk_commitment: Some([0x11; 32]),
        status: ProofStatus::Verified,
        verified_at_height: Some(1),
        bridge: None,
    };
    iroha_core::query::insert_proof_record_for_test(&mut state, id.clone(), record);

    let state = Arc::new(state);
    let limits = iroha_torii::ProofApiLimits::default();
    let telemetry = iroha_torii::MaybeTelemetry::disabled();
    let app =
        Router::new().route(
            "/v1/zk/proof/{backend}/{hash}",
            get({
                let state = state.clone();
                let telemetry = telemetry.clone();
                move |headers: axum::http::HeaderMap,
                      path: axum::extract::Path<(String, String)>| async move {
                    iroha_torii::handle_get_proof(state, limits, &telemetry, Some(&headers), path)
                        .await
                }
            }),
        );
    let backend_enc = urlencoding::encode(backend);
    let hash_hex = hex::encode(proof_hash);
    let uri = format!("/v1/zk/proof/{backend_enc}/{hash_hex}");
    (app, uri, hash_hex)
}

#[tokio::test]
async fn zk_proof_get_returns_record() {
    let backend = "halo2/ipa";
    let proof_hash = [0xAB; 32];
    let (app, uri, hash_hex) = proof_app_with_record(backend, proof_hash);
    let req = http::Request::builder()
        .method("GET")
        .uri(uri.as_str())
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
    let (parts, body) = resp.into_parts();
    let content_type = parts
        .headers
        .get(http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(content_type.contains("application/json"));
    let bytes = body.collect().await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    assert_eq!(v.get("backend").and_then(|x| x.as_str()), Some(backend));
    assert_eq!(
        v.get("proof_hash").and_then(|x| x.as_str()),
        Some(hash_hex.as_str())
    );
    assert_eq!(v.get("status").and_then(|x| x.as_str()), Some("Verified"));
    assert_eq!(
        v.get("verified_at_height")
            .and_then(norito::json::Value::as_u64),
        Some(1)
    );
    assert_eq!(
        v.get("vk_commitment").and_then(|x| x.as_str()),
        Some(hex::encode([0x11u8; 32]).as_str())
    );
    let vk = v
        .get("vk_ref")
        .and_then(|x| x.as_object())
        .expect("vk_ref obj");
    assert_eq!(
        vk.get("backend").and_then(|x| x.as_str()),
        Some("halo2/ipa")
    );
    assert_eq!(vk.get("name").and_then(|x| x.as_str()), Some("vk_test"));
    let cache_control = parts.headers.get(http::header::CACHE_CONTROL);
    let expected_cache = format!(
        "public, max-age={}",
        defaults::torii::PROOF_CACHE_MAX_AGE_SECS
    );
    assert_eq!(
        cache_control.and_then(|h| h.to_str().ok()),
        Some(expected_cache.as_str()),
        "proof responses advertise cache lifetime"
    );
    let etag_header = parts.headers.get(http::header::ETAG).cloned();
    assert!(etag_header.is_some(), "proof responses include ETag");

    let conditional_req = http::Request::builder()
        .method("GET")
        .uri(uri.as_str())
        .header(http::header::IF_NONE_MATCH, etag_header.unwrap())
        .body(axum::body::Body::empty())
        .unwrap();
    let conditional_resp = app.clone().oneshot(conditional_req).await.unwrap();
    assert_eq!(
        conditional_resp.status(),
        http::StatusCode::NOT_MODIFIED,
        "matched ETag produces 304"
    );
    let conditional_body = conditional_resp
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    assert!(conditional_body.is_empty(), "304 responses have empty body");
}

#[tokio::test]
async fn zk_proof_get_not_found() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::new(), kura, query);
    let state = Arc::new(state);
    let limits = iroha_torii::ProofApiLimits::default();
    let telemetry = iroha_torii::MaybeTelemetry::disabled();
    let app =
        Router::new().route(
            "/v1/zk/proof/{backend}/{hash}",
            get({
                let state = state.clone();
                let telemetry = telemetry.clone();
                move |headers: axum::http::HeaderMap,
                      path: axum::extract::Path<(String, String)>| async move {
                    iroha_torii::handle_get_proof(state, limits, &telemetry, Some(&headers), path)
                        .await
                }
            }),
        );
    let backend_enc = urlencoding::encode("halo2/ipa");
    let uri = format!(
        "/v1/zk/proof/{backend_enc}/{hash}",
        hash = hex::encode([0u8; 32])
    );
    let req = http::Request::builder()
        .method("GET")
        .uri(uri)
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);
}
