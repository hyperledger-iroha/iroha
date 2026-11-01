//! Smoke test that Torii does not expose retired Sumeragi telemetry routes.
#![cfg(feature = "telemetry")]
use axum::{extract::ConnectInfo, http::Request};
use http::StatusCode;
use iroha_config::parameters::actual::TelemetryProfile;
use iroha_core::{kiso::KisoHandle, query::store::LiveQueryStore, state::World};
use iroha_torii::{MaybeTelemetry, Torii};
use std::sync::Arc;
use tower::ServiceExt as _;
mod fixtures;
fn signed_loopback_get(
    runtime: &fixtures::CommittedToriiRouterRuntime,
    cfg: &iroha_config::parameters::actual::Root,
    uri: &'static str,
) -> Request<axum::body::Body> {
    let request = Request::builder()
        .uri(uri)
        .body(axum::body::Body::empty())
        .unwrap();
    let mut request = runtime.operator_signed_request(&cfg.common.key_pair, request, &[]);
    request
        .extensions_mut()
        .insert(ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 0))));
    request
}
#[tokio::test]
async fn sumeragi_tel_subrouter_rejects_retired_endpoints() {
    // Minimal Torii setup with telemetry enabled
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
    let telemetry_handle = MaybeTelemetry::for_tests().with_profile(TelemetryProfile::Full);
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
                telemetry_handle,
            ),
            &iroha_torii::test_utils::private_settlement_test_node_key(),
        ),
    )
    .expect("valid Torii Sumeragi-telemetry fixture");
    let runtime = fixtures::CommittedToriiRouterRuntime::new(
        torii
            .api_router_for_tests()
            .expect("test Torii router initializes"),
        native_chain,
    );
    let app = runtime.router();
    for retired in [
        "/v1/sumeragi/pacemaker",
        "/v1/sumeragi/rbc",
        "/v1/sumeragi/phases",
        "/v1/sumeragi/telemetry",
        "/v1/sumeragi/rbc/delivered/0/0",
        "/v1/sumeragi/rbc/sessions",
        "/v1/sumeragi/rbc/sample",
        "/v1/sumeragi/collectors",
    ] {
        let response = app
            .clone()
            .oneshot(signed_loopback_get(&runtime, &cfg, retired))
            .await
            .expect("retired route response");
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{retired}");
    }
    runtime.shutdown().await;
}

#[path = "../src/build_identity_test_fixture.rs"]
mod build_identity_test_fixture;
