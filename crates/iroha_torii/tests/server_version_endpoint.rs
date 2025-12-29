//! Smoke test for the compat `/server_version` endpoint.

use std::sync::Arc;

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
use tower::ServiceExt as _;

#[tokio::test]
async fn server_version_endpoint_returns_payload() {
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
        kura.clone(),
        state.clone(),
        cfg.common.key_pair.clone(),
        OnlinePeersProvider::new(peers_rx.clone()),
        None,
        iroha_torii::MaybeTelemetry::disabled(),
    );

    let app = torii.api_router_for_tests();
    let response = app
        .oneshot(
            Request::builder()
                .uri(iroha_torii_shared::uri::SERVER_VERSION)
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let version: iroha_torii_shared::Version = norito::json::from_slice(&body).unwrap();

    assert_eq!(version.version, env!("CARGO_PKG_VERSION"));
    assert!(!version.git_sha.is_empty());
}
