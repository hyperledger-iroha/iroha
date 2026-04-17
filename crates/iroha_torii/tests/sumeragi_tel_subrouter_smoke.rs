#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Smoke test that Torii exposes telemetry-gated Sumeragi routes via the merged sub-router.
#![cfg(feature = "telemetry")]

use std::sync::Arc;

use axum::{extract::ConnectInfo, http::Request};
use http::StatusCode;
use iroha_config::parameters::actual::TelemetryProfile;
use iroha_core::{
    kiso::KisoHandle,
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};
use iroha_torii::{MaybeTelemetry, Torii};
use tower::ServiceExt as _;

mod fixtures;

fn signed_loopback_get(
    cfg: &iroha_config::parameters::actual::Root,
    uri: &'static str,
) -> Request<axum::body::Body> {
    let request = Request::builder()
        .uri(uri)
        .body(axum::body::Body::empty())
        .unwrap();
    let mut request = fixtures::operator_signed_request(&cfg.common.key_pair, request, &[]);
    request
        .extensions_mut()
        .insert(ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 0))));
    request
}

#[tokio::test]
async fn sumeragi_tel_subrouter_exposes_endpoints() {
    // Minimal Torii setup with telemetry enabled
    let cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
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
    let queue = Arc::new(iroha_core::queue::Queue::from_config(
        queue_cfg,
        events_sender,
    ));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx;
    let telemetry_handle = MaybeTelemetry::for_tests().map_gate(TelemetryProfile::Full);
    let torii = Torii::new_with_handle(
        iroha_data_model::ChainId::from("test-chain"),
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
        telemetry_handle,
    );
    let app = torii.api_router_for_tests();

    for uri in [
        "/v1/sumeragi/pacemaker",
        "/v1/sumeragi/rbc",
        "/v1/sumeragi/rbc/delivered/0/0",
        "/v1/sumeragi/phases",
    ] {
        let resp = app
            .clone()
            .oneshot(signed_loopback_get(&cfg, uri))
            .await
            .unwrap();
        assert!(matches!(
            resp.status(),
            StatusCode::OK | StatusCode::TOO_MANY_REQUESTS
        ));
    }
}
