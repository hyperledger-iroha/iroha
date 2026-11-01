//! Smoke tests for Torii domains endpoints.
#![cfg(feature = "app_api")]
use axum::http::Request;
use http::StatusCode;
use iroha_core::{kiso::KisoHandle, query::store::LiveQueryStore, state::World};
use iroha_torii::Torii;
use std::sync::Arc;
use tower::ServiceExt as _;
#[path = "fixtures.rs"]
mod fixtures;
#[tokio::test]
async fn domains_endpoints_exist() {
    // Minimal Torii setup
    let _data_dir = iroha_torii::test_utils::TestDataDirGuard::new();
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let native_chain = fixtures::commit_signed_genesis(&mut cfg, World::default(), Vec::new());
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(iroha_core::queue::Queue::from_config(
        queue_cfg,
        events_sender,
    ));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx;
    let torii = Torii::new_with_handle(
        cfg.common.chain.clone(),
        native_chain.network_id(),
        kiso,
        cfg.torii.clone(),
        queue,
        tokio::sync::broadcast::channel(1).0,
        LiveQueryStore::start_test(),
        native_chain.kura().clone(),
        native_chain.state().clone(),
        cfg.common.key_pair.clone(),
        iroha_torii::OnlinePeersProvider::new(peers_rx),
        None,
        iroha_torii::test_utils::with_private_settlement_test_signers(
            iroha_torii::ToriiRuntimeDeps::new(
                build_identity_test_fixture::build_identity(),
                iroha_torii::MaybeTelemetry::disabled(),
            ),
            &iroha_torii::test_utils::private_settlement_test_node_key(),
        ),
    )
    .expect("valid Torii domain fixture")
    .with_local_peer_id(native_chain.validators()[0].0.clone());
    let runtime = fixtures::CommittedToriiRouterRuntime::new(
        torii
            .api_router_for_tests()
            .expect("test Torii router initializes"),
        native_chain,
    );
    let app = runtime.router();
    // GET /v1/domains
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/domains?limit=10")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(matches!(
        resp.status(),
        StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
    ));
    // POST /v1/domains/query
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/domains/query")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(matches!(
        resp.status(),
        StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
    ));
    runtime.shutdown().await;
}

#[path = "../src/build_identity_test_fixture.rs"]
mod build_identity_test_fixture;
