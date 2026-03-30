#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Smoke test for the maintained offline cash app routes.

use std::sync::Arc;

use axum::extract::connect_info::ConnectInfo;
use axum::http::{Request, StatusCode, Uri, header::CONTENT_TYPE};
use iroha_core::{
    kiso::KisoHandle, kura::Kura, prelude::World, query::store::LiveQueryStore, state::State,
};
use iroha_data_model::{ChainId, peer::PeerId};
use tower::ServiceExt as _;

#[path = "fixtures.rs"]
mod fixtures;

fn mk_minimal_root_cfg() -> iroha_config::parameters::actual::Root {
    iroha_torii::test_utils::mk_minimal_root_cfg()
}

#[tokio::test]
async fn offline_cash_routes_are_mounted_and_legacy_allowances_are_absent() {
    let _data_dir = iroha_torii::test_utils::TestDataDirGuard::new();
    let cfg = mk_minimal_root_cfg();
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
    let da_receipt_signer = cfg.common.key_pair.clone();

    let torii = {
        #[cfg(feature = "telemetry")]
        {
            use iroha_core::telemetry as core_telemetry;
            use iroha_primitives::time::TimeSource;

            let metrics = fixtures::shared_metrics();
            let (_mh, ts) = TimeSource::new_mock(core::time::Duration::default());
            let telemetry = core_telemetry::start(
                metrics,
                state.clone(),
                kura.clone(),
                queue.clone(),
                peers_rx.clone(),
                local_peer_id,
                ts,
                false,
            )
            .0;
            iroha_torii::Torii::new(
                ChainId::from("test-chain"),
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
            iroha_torii::Torii::new(
                ChainId::from("test-chain"),
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
    let connect_info = ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 0)));

    let readiness = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(Uri::from_static("/v1/offline/cash/readiness"))
                .extension(connect_info)
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(readiness.status(), StatusCode::OK);

    for path in [
        "/v1/offline/cash/setup",
        "/v1/offline/cash/load",
        "/v1/offline/cash/refresh",
        "/v1/offline/cash/sync",
        "/v1/offline/cash/redeem",
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path)
                    .header(CONTENT_TYPE, "application/json")
                    .extension(ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 0))))
                    .body(axum::body::Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_ne!(
            response.status(),
            StatusCode::NOT_FOUND,
            "offline cash route should be mounted: {path}"
        );
    }

    for retired_path in ["/v1/offline/allowances", "/v1/offline/allowances/query"] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(retired_path)
                    .extension(ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 0))))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "legacy offline route should be absent: {retired_path}"
        );
    }
}
