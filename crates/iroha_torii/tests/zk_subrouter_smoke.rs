#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Smoke test that ZK endpoints (verify, attachments) are exposed via the merged sub-router.
#![cfg(feature = "app_api")]

use std::sync::Arc;

use axum::http::Request;
use http::StatusCode;
use iroha_core::{
    kiso::KisoHandle,
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};
use iroha_data_model::peer::PeerId;
#[cfg(feature = "telemetry")]
use iroha_primitives::time::TimeSource;
use iroha_torii::Torii;
use tower::ServiceExt as _;

#[path = "fixtures.rs"]
mod fixtures;

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn zk_verify_and_attachments_endpoints_exposed() {
    // Minimal Torii setup (no telemetry requirement for these endpoints)
    let cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());
    let mut world = World::default();
    fixtures::seed_peer(&mut world, local_peer_id.clone());
    let state = Arc::new(State::new_for_testing(world, kura.clone(), query));
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(iroha_core::queue::Queue::from_config(
        queue_cfg,
        events_sender,
    ));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx;

    // Optional telemetry used elsewhere; not needed here, but keep setup consistent
    #[cfg(feature = "telemetry")]
    let telemetry = {
        use iroha_core::telemetry as core_telemetry;
        let metrics = fixtures::shared_metrics();
        let (_mh, ts) = TimeSource::new_mock(core::time::Duration::default());
        core_telemetry::start(
            metrics,
            state.clone(),
            kura.clone(),
            queue.clone(),
            peers_rx.clone(),
            local_peer_id.clone(),
            ts,
            false,
        )
        .0
    };

    let da_receipt_signer = cfg.common.key_pair.clone();
    let torii = {
        #[cfg(feature = "telemetry")]
        {
            Torii::new(
                iroha_data_model::ChainId::from("test-chain"),
                kiso,
                cfg.torii.clone(),
                queue,
                tokio::sync::broadcast::channel(1).0,
                LiveQueryStore::start_test(),
                kura,
                state,
                da_receipt_signer.clone(),
                iroha_torii::OnlinePeersProvider::new(peers_rx),
                telemetry,
                true,
            )
        }
        #[cfg(not(feature = "telemetry"))]
        {
            Torii::new(
                iroha_data_model::ChainId::from("test-chain"),
                kiso,
                cfg.torii.clone(),
                queue,
                tokio::sync::broadcast::channel(1).0,
                LiveQueryStore::start_test(),
                kura,
                state,
                da_receipt_signer,
                iroha_torii::OnlinePeersProvider::new(peers_rx),
            )
        }
    };

    let app = torii.api_router_for_tests();

    // POST /v1/zk/verify with minimal JSON; accept OK or 429
    let resp = app
        .clone()
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/zk/verify")
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

    // GET /v1/zk/attachments (empty list by default); accept OK or 429
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/zk/attachments")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(matches!(
        resp.status(),
        StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
    ));

    // GET /v1/zk/attachments/{id} with a placeholder id; accept 200, 404, or 429
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/zk/attachments/placeholder-id")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(matches!(
        resp.status(),
        StatusCode::OK
            | StatusCode::NOT_FOUND
            | StatusCode::BAD_REQUEST
            | StatusCode::TOO_MANY_REQUESTS
    ));
}
