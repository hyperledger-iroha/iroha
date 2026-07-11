//! Integration coverage for the single, first-release Torii API surface.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(feature = "app_api")]

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};

use axum::http::Request;
use http::{
    HeaderMap, Method, StatusCode,
    header::{ACCEPT, ALLOW, CONTENT_TYPE, VARY},
};
use http_body_util::BodyExt as _;
use iroha_core::{
    kiso::KisoHandle,
    kura::Kura,
    query::store::LiveQueryStore,
    queue::Queue,
    state::{State, World},
};
use iroha_torii::{OnlinePeersProvider, Torii, test_utils};
use iroha_torii_shared::{ErrorEnvelope, uri};
use tower::ServiceExt as _;

const RETIRED_API_VERSION_HEADER: &str = "x-iroha-api-version";

fn local_connect_info() -> axum::extract::connect_info::ConnectInfo<SocketAddr> {
    axum::extract::connect_info::ConnectInfo(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
}

fn header_contains_token(headers: &HeaderMap, name: http::header::HeaderName, token: &str) -> bool {
    headers
        .get_all(name)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .any(|value| value.trim().eq_ignore_ascii_case(token))
}

async fn decode_error_response(
    response: axum::response::Response,
) -> (StatusCode, HeaderMap, ErrorEnvelope) {
    let status = response.status();
    let headers = response.headers().clone();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("collect error response")
        .to_bytes();
    let content_type = headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .expect("typed fallback content type");
    let envelope = if content_type.starts_with("application/json") {
        norito::json::from_slice(&body).expect("decode JSON error envelope")
    } else {
        assert_eq!(content_type, "application/x-norito");
        norito::decode_from_bytes(&body).expect("decode Norito error envelope")
    };
    (status, headers, envelope)
}

#[tokio::test]
async fn unknown_routes_use_negotiated_typed_error_envelopes() {
    let router = build_router();
    for accept in ["application/json", "application/x-norito"] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/route-that-does-not-exist")
                    .extension(local_connect_info())
                    .header(ACCEPT, accept)
                    .body(axum::body::Body::empty())
                    .expect("unknown-route request"),
            )
            .await
            .expect("unknown-route response");
        let (status, headers, envelope) = decode_error_response(response).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "Accept: {accept}");
        assert_eq!(envelope.code(), "route_not_found", "Accept: {accept}");
        assert!(header_contains_token(&headers, VARY, "Accept"));
    }
}

#[tokio::test]
async fn wrong_methods_use_negotiated_typed_errors_and_retain_allow() {
    let router = build_router();
    for accept in ["application/json", "application/x-norito"] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(uri::HEALTH)
                    .extension(local_connect_info())
                    .header(ACCEPT, accept)
                    .body(axum::body::Body::empty())
                    .expect("wrong-method request"),
            )
            .await
            .expect("wrong-method response");
        let (status, headers, envelope) = decode_error_response(response).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "Accept: {accept}");
        assert_eq!(envelope.code(), "method_not_allowed", "Accept: {accept}");
        assert!(header_contains_token(&headers, ALLOW, "GET"));
        assert!(header_contains_token(&headers, ALLOW, "HEAD"));
        assert!(header_contains_token(&headers, VARY, "Accept"));
    }
}

#[tokio::test]
async fn retired_api_version_header_is_ignored_and_not_advertised() {
    let response = build_router()
        .oneshot(
            Request::builder()
                .uri(uri::HEALTH)
                .extension(local_connect_info())
                .header(RETIRED_API_VERSION_HEADER, "not-a-version")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let headers = response.headers().clone();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        status,
        StatusCode::OK,
        "unexpected status {status} with body {}",
        String::from_utf8_lossy(&body)
    );
    for retired in [
        RETIRED_API_VERSION_HEADER,
        "x-iroha-api-supported",
        "x-iroha-api-min-proof-version",
        "x-iroha-api-sunset-unix",
    ] {
        assert!(!headers.contains_key(retired), "retired header {retired}");
    }
}

#[tokio::test]
async fn retired_api_versions_endpoint_is_unmounted() {
    let response = build_router()
        .oneshot(
            Request::builder()
                .uri("/v1/api/versions")
                .extension(local_connect_info())
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn proof_access_is_not_gated_by_a_version_header() {
    let response = build_router()
        .oneshot(
            Request::builder()
                .uri("/v1/zk/proofs?limit=1")
                .extension(local_connect_info())
                .header(RETIRED_API_VERSION_HEADER, "0.0")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_ne!(response.status(), StatusCode::UPGRADE_REQUIRED);
}

fn build_router() -> axum::Router {
    let cfg = test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(
        World::default(),
        kura.clone(),
        query,
    ));
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(Queue::from_config(queue_cfg, events_sender));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx;

    let torii = Torii::new_with_handle(
        cfg.common.chain.clone(),
        kiso,
        cfg.torii.clone(),
        queue,
        tokio::sync::broadcast::channel(1).0,
        LiveQueryStore::start_test(),
        kura,
        state,
        cfg.common.key_pair.clone(),
        OnlinePeersProvider::new(peers_rx),
        None,
        iroha_torii::MaybeTelemetry::disabled(),
    );

    torii.api_router_for_tests()
}
