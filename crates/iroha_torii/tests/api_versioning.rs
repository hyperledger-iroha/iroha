//! Integration coverage for Torii API version negotiation.

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};

use axum::http::Request;
use http::StatusCode;
use http_body_util::BodyExt as _;
use iroha_core::{
    kiso::KisoHandle,
    kura::Kura,
    query::store::LiveQueryStore,
    queue::Queue,
    state::{State, World},
};
use iroha_torii::{OnlinePeersProvider, Torii, test_utils};
use iroha_torii_shared::{ErrorEnvelope, HEADER_API_VERSION, uri};
use tower::ServiceExt as _;

#[tokio::test]
async fn default_version_is_advertised() {
    let app = build_router();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(uri::HEALTH)
                .extension(axum::extract::connect_info::ConnectInfo(SocketAddr::new(
                    IpAddr::V4(Ipv4Addr::LOCALHOST),
                    0,
                )))
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
    let header = headers
        .get(HEADER_API_VERSION)
        .and_then(|value| value.to_str().ok());
    assert_eq!(header, Some("1.1"));
    let supported = headers
        .get("x-iroha-api-supported")
        .and_then(|value| value.to_str().ok());
    assert_eq!(supported, Some("1.0, 1.1"));
    let min_proof = headers
        .get("x-iroha-api-min-proof-version")
        .and_then(|value| value.to_str().ok());
    assert_eq!(min_proof, Some("1.1"));
    let sunset = headers
        .get("x-iroha-api-sunset-unix")
        .and_then(|value| value.to_str().ok());
    assert_eq!(
        sunset,
        iroha_torii_shared::API_VERSION_SUNSET_UNIX
            .map(|ts| ts.to_string())
            .as_deref()
    );
}

#[tokio::test]
async fn rejecting_unsupported_api_version() {
    let app = build_router();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(uri::HEALTH)
                .extension(axum::extract::connect_info::ConnectInfo(SocketAddr::new(
                    IpAddr::V4(Ipv4Addr::LOCALHOST),
                    0,
                )))
                .header(HEADER_API_VERSION, "2")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let payload = norito::decode_from_bytes::<ErrorEnvelope>(&body).unwrap();
    assert_eq!(payload.code, "torii_api_version_unsupported");
    assert!(
        !payload.message.is_empty(),
        "expected error message for unsupported version"
    );
}

#[tokio::test]
async fn api_versions_endpoint_lists_supported_versions() {
    let app = build_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri(uri::API_VERSIONS)
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let info: iroha_torii_shared::ApiVersionInfo = norito::decode_from_bytes(&body).unwrap();
    assert_eq!(info.default, "1.1");
    assert_eq!(info.supported, vec!["1.0", "1.1"]);
    assert_eq!(info.min_proof_version, "1.1");
    assert_eq!(
        info.sunset_unix,
        iroha_torii_shared::API_VERSION_SUNSET_UNIX
    );
}

#[tokio::test]
async fn proof_endpoints_reject_old_api_version() {
    let app = build_router();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/zk/proofs?limit=1")
                .extension(axum::extract::connect_info::ConnectInfo(SocketAddr::new(
                    IpAddr::V4(Ipv4Addr::LOCALHOST),
                    0,
                )))
                .header(HEADER_API_VERSION, "1.0")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UPGRADE_REQUIRED);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let payload = norito::decode_from_bytes::<ErrorEnvelope>(&body).unwrap();
    assert_eq!(payload.code, "torii_api_version_too_old");
    assert!(
        payload.message.contains("minimum `1.1`"),
        "expected minimum version message, got {}",
        payload.message
    );
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
        queue.clone(),
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
