#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Telemetry JSON endpoints smoke tests (RBC/Pacemaker).

#![cfg(feature = "telemetry")]

use std::sync::Arc;

use axum::http::Request;
use iroha_config::parameters::actual::TelemetryProfile;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};
use iroha_data_model::ChainId;
use iroha_torii::MaybeTelemetry;
use tower::util::ServiceExt as _;

fn mk_minimal_root_cfg() -> iroha_config::parameters::actual::Root {
    iroha_torii::test_utils::mk_minimal_root_cfg()
}

fn build_torii(cfg: &iroha_config::parameters::actual::Root) -> iroha_torii::Torii {
    let (kiso, _child) = iroha_core::kiso::KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(
        World::default(),
        kura.clone(),
        query,
    ));
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(iroha_core::queue::Queue::from_config(
        queue_cfg,
        events_sender,
    ));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx;
    let telemetry = MaybeTelemetry::for_tests().map_gate(TelemetryProfile::Full);
    iroha_torii::Torii::new_with_handle(
        ChainId::from("test-chain"),
        kiso,
        cfg.torii.clone(),
        queue,
        tokio::sync::broadcast::channel(1).0,
        LiveQueryStore::start_test(),
        kura,
        state,
        cfg.common.key_pair.clone(),
        iroha_torii::OnlinePeersProvider::new(peers_rx),
        None,
        telemetry,
    )
}

#[tokio::test]
async fn rbc_status_json_shape() {
    let cfg = mk_minimal_root_cfg();
    let torii = build_torii(&cfg);
    let app = torii.api_router_for_tests();
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v2/sumeragi/rbc")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(resp.status().is_success());
    let body = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&body).unwrap();
    assert!(v.get("sessions_active").is_some());
    assert!(v.get("ready_broadcasts_total").is_some());
    assert!(v.get("ready_rebroadcasts_skipped_total").is_some());
    assert!(v.get("payload_rebroadcasts_skipped_total").is_some());
}

#[tokio::test]
async fn pacemaker_status_json_shape() {
    let cfg = mk_minimal_root_cfg();
    let torii = build_torii(&cfg);
    let app = torii.api_router_for_tests();
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v2/sumeragi/pacemaker")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(resp.status().is_success());
    let body = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&body).unwrap();
    // Expect at least one of the pacemaker keys
    assert!(v.get("backoff_ms").is_some() || v.get("pacemaker_backoff_ms").is_some());
}
