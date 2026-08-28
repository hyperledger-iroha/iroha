//! Router-level coverage for the authoritative Sumeragi v2 status endpoint.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(feature = "telemetry")]
use axum::{body::Body, http::Request};
use http::{StatusCode, header};
use http_body_util::BodyExt as _;
use iroha_config::parameters::actual::TelemetryProfile;
use iroha_core::{
    kiso::KisoHandle,
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
    sumeragi::status,
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::block::consensus_v2::{
    ConsensusMode, DualQuorum, HeightContext, HeightContextId, PROTOCOL_VERSION,
    SumeragiV2BodyState, SumeragiV2HeightContextStatus, SumeragiV2Status, SumeragiV2StatusPhase,
};
use iroha_torii::{MaybeTelemetry, OnlinePeersProvider, Torii};
use std::{
    net::SocketAddr,
    sync::{Arc, Mutex, MutexGuard},
};
use tower::ServiceExt as _;
const NORITO_MIME_TYPE: &str = "application/x-norito";
static STATUS_TEST_LOCK: Mutex<()> = Mutex::new(());
struct PublishedStatus {
    _guard: MutexGuard<'static, ()>,
}
impl PublishedStatus {
    fn install(value: SumeragiV2Status) -> Self {
        let guard = STATUS_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        status::set_v2_status(value);
        Self { _guard: guard }
    }
}
impl Drop for PublishedStatus {
    fn drop(&mut self) {
        status::clear_v2_status();
    }
}
fn status_fixture() -> SumeragiV2Status {
    SumeragiV2Status {
        protocol_version: PROTOCOL_VERSION,
        node_fingerprint: Hash::new(b"node"),
        build_fingerprint: Hash::new(b"build"),
        config_fingerprint: Hash::new(b"config"),
        restart_required: false,
        height_context_id: HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(
            Hash::new(b"height-context"),
        )),
        height: 42,
        view: 3,
        phase: SumeragiV2StatusPhase::Prepare,
        leader: 2,
        locked_prepare_qc: None,
        highest_prepare_qc: None,
        last_timeout_certificate: None,
        body_state: SumeragiV2BodyState::Validated,
        pending_persistence_id: Some(17),
        last_committed_height: 41,
        last_committed_subject: None,
        height_context: SumeragiV2HeightContextStatus {
            epoch: 1,
            epoch_end_height: 100,
            mode: ConsensusMode::Permissioned,
            epoch_seed: [0xA5; 32],
            validator_count: 4,
            quorum: DualQuorum {
                min_signers: 3,
                total_power: 4,
            },
        },
        last_commit_qc: None,
        liveness: Default::default(),
    }
}
fn build_status_router() -> axum::Router {
    let cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let state = Arc::new(State::new_for_testing(
        World::default(),
        kura.clone(),
        LiveQueryStore::start_test(),
    ));
    let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(iroha_core::queue::Queue::from_config(
        iroha_config::parameters::actual::Queue::default(),
        events_sender,
    ));
    let (_peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let telemetry = MaybeTelemetry::for_tests().with_profile(TelemetryProfile::Full);
    let torii = Torii::new_with_handle(
        cfg.common.chain.clone(),
        iroha_torii::test_utils::signed_query_network_id(),
        kiso,
        cfg.torii,
        queue,
        tokio::sync::broadcast::channel(1).0,
        LiveQueryStore::start_test(),
        kura,
        state,
        cfg.common.key_pair,
        OnlinePeersProvider::new(peers_rx),
        None,
        telemetry,
    );
    torii.api_router_for_tests()
}
async fn status_response(accept: &str) -> axum::response::Response {
    build_status_router()
        .oneshot(
            Request::builder()
                .uri("/v1/sumeragi/status")
                .header(header::ACCEPT, accept)
                .extension(axum::extract::ConnectInfo(SocketAddr::from((
                    [127, 0, 0, 1],
                    0,
                ))))
                .body(Body::empty())
                .expect("status request"),
        )
        .await
        .expect("status response")
}
#[tokio::test]
async fn json_status_is_exact_authoritative_v2_schema() {
    let expected = status_fixture();
    let _published = PublishedStatus::install(expected.clone());
    let response = status_response("application/json").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response
        .into_body()
        .collect()
        .await
        .expect("collect JSON status")
        .to_bytes();
    let decoded: SumeragiV2Status =
        norito::json::from_slice(&body).expect("decode authoritative v2 JSON");
    assert_eq!(decoded, expected);
    let value: norito::json::Value =
        norito::json::from_slice(&body).expect("decode status JSON object");
    assert!(
        value.get("liveness").is_some(),
        "authoritative liveness snapshot is required"
    );
    for retired in [
        "canonical",
        "rbc_status",
        "missing_qc_total",
        "consensus_missing_qc_reacquire_attempt_total",
        "lane_settlement_commitments",
        "lane_relay_envelopes",
    ] {
        assert!(
            value.get(retired).is_none(),
            "retired field {retired} leaked"
        );
    }
}
#[tokio::test]
async fn norito_status_decodes_as_exact_authoritative_v2_type() {
    let expected = status_fixture();
    let _published = PublishedStatus::install(expected.clone());
    let response = status_response(NORITO_MIME_TYPE).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE),
        Some(&http::HeaderValue::from_static(NORITO_MIME_TYPE))
    );
    let body = response
        .into_body()
        .collect()
        .await
        .expect("collect Norito status")
        .to_bytes();
    let decoded: SumeragiV2Status =
        norito::decode_from_bytes(&body).expect("decode authoritative v2 Norito");
    assert_eq!(decoded, expected);
}
