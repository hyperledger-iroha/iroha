//! Profile gate test for `/v1/sumeragi/status/sse`.
#![cfg(feature = "telemetry")]
use crate::fixtures;
use axum::http::Request;
use http::StatusCode;
use iroha_config::parameters::actual::TelemetryProfile;
use iroha_core::{kiso::KisoHandle, query::store::LiveQueryStore, state::World};
use iroha_torii::{MaybeTelemetry, Torii};
use std::sync::Arc;
use tower::ServiceExt as _;
fn mk_minimal_root_cfg() -> iroha_config::parameters::actual::Root {
    iroha_torii::test_utils::mk_minimal_root_cfg()
}
fn build_runtime(
    profile: TelemetryProfile,
) -> (fixtures::CommittedToriiRouterRuntime, iroha_crypto::KeyPair) {
    let mut cfg = mk_minimal_root_cfg();
    let native_chain = fixtures::commit_signed_genesis(&mut cfg, World::default(), Vec::new());
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(iroha_core::queue::Queue::from_config(
        queue_cfg,
        events_sender,
    ));
    let (_peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let telemetry = MaybeTelemetry::for_tests().with_profile(profile);
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
                telemetry,
            ),
            &iroha_torii::test_utils::private_settlement_test_node_key(),
        ),
    )
    .expect("valid Torii Sumeragi-SSE fixture");
    (
        fixtures::CommittedToriiRouterRuntime::new(
            torii
                .api_router_for_tests()
                .expect("test Torii router initializes"),
            native_chain,
        ),
        cfg.common.key_pair,
    )
}
/// The stream is an authenticated operator route, so the profile gate is reached signed.
fn signed_status_sse_request(
    runtime: &fixtures::CommittedToriiRouterRuntime,
    operator: &iroha_crypto::KeyPair,
) -> Request<axum::body::Body> {
    runtime.operator_signed_request(
        operator,
        Request::builder()
            .uri("/v1/sumeragi/status/sse")
            .body(axum::body::Body::empty())
            .unwrap(),
        &[],
    )
}
#[tokio::test]
async fn status_sse_allowed_under_extended_profile() {
    let _data_dir = iroha_torii::test_utils::TestDataDirGuard::new();
    let (runtime, operator) = build_runtime(TelemetryProfile::Extended);
    let resp = runtime
        .router()
        .oneshot(signed_status_sse_request(&runtime, &operator))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    assert!(ct.contains("text/event-stream"));
    runtime.shutdown().await;
}
#[tokio::test]
async fn status_sse_restricted_under_operator_profile() {
    let _data_dir = iroha_torii::test_utils::TestDataDirGuard::new();
    let (runtime, operator) = build_runtime(TelemetryProfile::Operator);
    let resp = runtime
        .router()
        .oneshot(signed_status_sse_request(&runtime, &operator))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    runtime.shutdown().await;
}

use crate::build_identity_test_fixture;
