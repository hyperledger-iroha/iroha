//! Router-level coverage for the authoritative Sumeragi v2 status endpoint.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Router-level tests for the authoritative Sumeragi v2 status endpoint.
#![cfg(feature = "telemetry")]

use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};

use axum::{body::Body, http::Request};
use http::{StatusCode, header};
use http_body_util::BodyExt as _;
use iroha_config::parameters::actual::TelemetryProfile;
use iroha_core::{
    kiso::KisoHandle,
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
    sumeragi,
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    ChainId,
    block::{
        consensus::{SumeragiLaneBlockSessionStatus, SumeragiLanePayloadOwnership},
        consensus_v2::{
            self as v2, SumeragiV2BodyState, SumeragiV2Status, SumeragiV2StatusPhase,
            SumeragiV2StatusResponse,
        },
    },
    nexus::{DataSpaceId, LaneId},
    peer::PeerId,
};
use iroha_torii::{MaybeTelemetry, OnlinePeersProvider, Torii};
use tower::ServiceExt as _;

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
    }
}

fn build_status_router() -> axum::Router {
    let cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());
    let mut world = World::default();
    fixtures::seed_peer(&mut world, local_peer_id.clone());
    let state = Arc::new(State::new_for_testing(world, kura.clone(), query.clone()));
    let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(iroha_core::queue::Queue::from_config(
        Queue::default(),
        events_sender,
    ));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx;

    let telemetry = {
        let metrics = fixtures::shared_metrics();
        let (_mock_handle, time_source) = TimeSource::new_mock(core::time::Duration::default());
        iroha_core::telemetry::start(
            metrics,
            state.clone(),
            kura.clone(),
            queue.clone(),
            peers_rx.clone(),
            local_peer_id,
            time_source,
            true,
        )
        .0
    };

    let torii = iroha_torii::Torii::new(
        ChainId::from("test-chain"),
        kiso,
        cfg.torii,
        queue,
        tokio::sync::broadcast::channel(1).0,
        LiveQueryStore::start_test(),
        kura,
        state,
        cfg.common.key_pair.clone(),
        iroha_torii::OnlinePeersProvider::new(peers_rx),
        telemetry,
    );
    torii.api_router_for_tests()
}

fn status_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct StatusFixtureGuard {
    _lock: MutexGuard<'static, ()>,
}

impl Drop for StatusFixtureGuard {
    fn drop(&mut self) {
        reset_status_state();
    }
}

fn status_test_guard() -> StatusFixtureGuard {
    let lock = status_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    reset_status_state();
    StatusFixtureGuard { _lock: lock }
}

fn reset_status_state() {
    sumeragi::status::clear_v2_status();
    sumeragi::status::clear_v2_operator_status();
    sumeragi::status::set_v2_tx_queue_status(v2::SumeragiV2TxQueueStatus {
        capacity: 64,
        max_retained_bytes: 8_192,
        ..v2::SumeragiV2TxQueueStatus::default()
    });
    sumeragi::status::clear_lane_payload_ownerships();
    sumeragi::status::set_lane_settlement_commitments(Vec::new());
    sumeragi::status::set_lane_relay_envelopes(Vec::new());
    sumeragi::status::set_committed_lane_blocks(Vec::new());
    sumeragi::status::set_lane_block_sessions(Vec::new());
    sumeragi::status::set_local_removed_from_world(false);
}

fn authoritative_status_fixture() -> SumeragiV2Status {
    SumeragiV2Status {
        protocol_version: v2::PROTOCOL_VERSION,
        node_fingerprint: Hash::new(b"torii-v2-status-node"),
        build_fingerprint: Hash::new(b"torii-v2-status-build"),
        config_fingerprint: Hash::new(b"torii-v2-status-config"),
        height_context_id: v2::HeightContextId(
            HashOf::<v2::HeightContext>::from_untyped_unchecked(Hash::new(
                b"torii-v2-status-context",
            )),
        ),
        height: 7,
        view: 2,
        phase: SumeragiV2StatusPhase::Prepare,
        leader: 1,
        locked_prepare_qc: None,
        highest_prepare_qc: None,
        last_timeout_certificate: None,
        body_state: SumeragiV2BodyState::Validated,
        pending_persistence_id: None,
        last_committed_height: 0,
        last_committed_subject: None,
        height_context: v2::SumeragiV2HeightContextStatus {
            epoch: 1,
            epoch_end_height: 10,
            mode: v2::ConsensusMode::Permissioned,
            epoch_seed: [0xA5; 32],
            validator_count: 4,
            quorum: v2::DualQuorum {
                min_signers: 3,
                total_power: 4,
            },
        },
        last_commit_qc: None,
    }
}

fn lane_payload_ownership_fixture() -> SumeragiLanePayloadOwnership {
    SumeragiLanePayloadOwnership {
        proposal_height: 12,
        proposal_view: 3,
        lane_id: LaneId::new(7),
        dataspace_id: DataSpaceId::new(42),
        lane_incarnation: Hash::new(b"torii-status-endpoint-lane-incarnation"),
        lane_block_height: 2,
        lane_block_view: 1,
        subject_hash: Hash::prehashed([0x41; Hash::LENGTH]),
        qc_mode_tag: "test-lane-qc-mode".to_owned(),
        accepted_candidate_indices: vec![0, 2],
        accepted_transaction_hashes: vec![
            Hash::prehashed([0x44; Hash::LENGTH]),
            Hash::prehashed([0x45; Hash::LENGTH]),
        ],
        previous_lane_block_height: 1,
        previous_lane_block_descriptor_hash: Some(Hash::prehashed([0x47; Hash::LENGTH])),
        lane_block_descriptor_hash: Some(Hash::prehashed([0x46; Hash::LENGTH])),
        lane_block_descriptor_validator_set: Vec::new(),
        lane_block_descriptor_validator_count: 0,
        lane_block_descriptor_min_quorum: 0,
        payload_ownership_hash: Hash::prehashed([0x42; Hash::LENGTH]),
        rbc_instance_hash: Hash::prehashed([0x43; Hash::LENGTH]),
    }
}

fn lane_block_session_fixture() -> SumeragiLaneBlockSessionStatus {
    SumeragiLaneBlockSessionStatus {
        lane_id: LaneId::new(7),
        dataspace_id: DataSpaceId::new(42),
        lane_incarnation: Hash::new(b"torii-status-endpoint-lane-incarnation"),
        lane_block_height: 2,
        lane_block_view: 1,
        proposal_hash: Hash::prehashed([0x51; Hash::LENGTH]),
        has_proposal: true,
        prepare_vote_count: 3,
        commit_vote_count: 2,
        has_prepare_qc: true,
        has_commit_qc: false,
        pending_commit_vote_request: true,
        pending_committed_session_drain: false,
        committed_session_drained: false,
        validator_count: 4,
        min_quorum: 3,
    }
}

async fn request_status(app: Router, accept: &str) -> axum::response::Response {
    app.oneshot(
        Request::builder()
            .uri("/v1/sumeragi/status")
            .header("Accept", accept)
            .body(Body::empty())
            .expect("build status request"),
    )
    .await
    .expect("serve status request")
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn sumeragi_status_endpoint_fails_closed_before_v2_replay() {
    let _guard = status_test_guard();
    let app = build_status_router();

    for accept in ["application/json", "application/x-norito"] {
        let response = request_status(app.clone(), accept).await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn sumeragi_status_endpoint_rejects_impossible_operator_occupancy() {
    let _guard = status_test_guard();
    iroha_core::sumeragi::status::set_v2_status(authoritative_status_fixture());
    iroha_core::sumeragi::status::set_v2_adapter_queue_status(v2::SumeragiV2AdapterQueueStatus {
        ingress_keys: 2,
        ingress_capacity: 1,
        ..v2::SumeragiV2AdapterQueueStatus::default()
    });

    let response = request_status(build_status_router(), "application/json").await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn sumeragi_status_endpoint_exposes_complete_v2_json_shape() {
    let _guard = status_test_guard();
    let authoritative = authoritative_status_fixture();
    iroha_core::sumeragi::status::set_v2_status(authoritative.clone());
    iroha_core::sumeragi::status::set_lane_payload_ownerships(vec![
        lane_payload_ownership_fixture(),
    ]);
    iroha_core::sumeragi::status::set_lane_block_sessions(vec![lane_block_session_fixture()]);
    let response = request_status(build_status_router(), "application/json").await;

    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("application/json"))
    );
    let bytes = BodyExt::collect(response.into_body())
        .await
        .expect("collect JSON status response")
        .to_bytes();
    let value: norito::json::Value =
        norito::json::from_slice(&bytes).expect("decode JSON status response");
    let root = value.as_object().expect("status response object");

    assert!(root.get("authoritative").is_none(), "JSON stays flattened");
    assert_eq!(
        root.get("protocol_version")
            .and_then(norito::json::Value::as_u64),
        Some(u64::from(v2::PROTOCOL_VERSION))
    );
    assert_eq!(
        root.get("height").and_then(norito::json::Value::as_u64),
        Some(authoritative.height)
    );
    assert_eq!(
        root.get("view").and_then(norito::json::Value::as_u64),
        Some(authoritative.view)
    );
    assert_eq!(
        root.get("leader").and_then(norito::json::Value::as_u64),
        Some(u64::from(authoritative.leader))
    );
    for (field, expected_len) in [
        ("lane_settlement_commitments", 0),
        ("lane_relay_envelopes", 0),
        ("lane_payload_ownerships", 1),
        ("committed_lane_blocks", 0),
        ("lane_block_sessions", 1),
    ] {
        assert_eq!(
            root.get(field)
                .and_then(norito::json::Value::as_array)
                .map(Vec::len),
            Some(expected_len),
            "unexpected {field} shape"
        );
    }
    assert_eq!(
        root.get("local_peer_removed")
            .and_then(norito::json::Value::as_bool),
        Some(false)
    );
    assert!(root.get("height_context").is_some());
    assert!(root.get("operator").is_some());
    let phase = root
        .get("phase")
        .and_then(norito::json::Value::as_object)
        .expect("tagged v2 phase");
    assert_eq!(
        phase.get("phase").and_then(norito::json::Value::as_str),
        Some("prepare")
    );
    assert_eq!(phase.get("details"), Some(&norito::json::Value::Null));
    let context = root
        .get("height_context")
        .and_then(norito::json::Value::as_object)
        .expect("v2 height context");
    let mode = context
        .get("mode")
        .and_then(norito::json::Value::as_object)
        .expect("tagged consensus mode");
    assert_eq!(
        mode.get("mode").and_then(norito::json::Value::as_str),
        Some("permissioned")
    );
    assert_eq!(mode.get("details"), Some(&norito::json::Value::Null));
    assert_eq!(
        context
            .get("epoch_seed")
            .and_then(norito::json::Value::as_str),
        Some("A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5")
    );
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn sumeragi_status_endpoint_json_and_norito_payloads_match_semantics() {
    let _guard = status_test_guard();
    let authoritative = authoritative_status_fixture();
    let ownership = lane_payload_ownership_fixture();
    let session = lane_block_session_fixture();
    iroha_core::sumeragi::status::set_v2_status(authoritative.clone());
    iroha_core::sumeragi::status::set_lane_payload_ownerships(vec![ownership.clone()]);
    iroha_core::sumeragi::status::set_lane_block_sessions(vec![session.clone()]);
    iroha_core::sumeragi::status::set_local_removed_from_world(true);
    let app = build_status_router();

    let json_response = request_status(app.clone(), "application/json").await;
    assert_eq!(json_response.status(), StatusCode::OK);
    let json_bytes = BodyExt::collect(json_response.into_body())
        .await
        .expect("collect JSON status response")
        .to_bytes();
    let json_value: norito::json::Value =
        norito::json::from_slice(&json_bytes).expect("decode JSON status response");
    let json_root = json_value.as_object().expect("JSON status object");

    let norito_response = request_status(app, "application/x-norito").await;
    assert_eq!(norito_response.status(), StatusCode::OK);
    assert_eq!(
        norito_response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("application/x-norito")
    );
    let norito_bytes = BodyExt::collect(norito_response.into_body())
        .await
        .expect("collect Norito status response")
        .to_bytes();
    let norito_wire: SumeragiV2StatusResponse =
        norito::decode_from_bytes(&norito_bytes).expect("decode complete v2 status response");

    assert_eq!(norito_wire.authoritative, authoritative);
    assert_eq!(norito_wire.lane_payload_ownerships, vec![ownership]);
    assert_eq!(norito_wire.lane_block_sessions, vec![session]);
    assert!(norito_wire.local_peer_removed);
    norito_wire
        .validate()
        .expect("valid typed v2 status response");
    assert_eq!(
        norito_wire.operator,
        iroha_core::sumeragi::status::v2_operator_status()
    );
    assert_eq!(
        json_root
            .get("protocol_version")
            .and_then(norito::json::Value::as_u64),
        Some(u64::from(norito_wire.authoritative.protocol_version))
    );
    let expected_operator =
        norito::json::to_value(&norito_wire.operator).expect("encode operator status");
    assert_eq!(json_root.get("operator"), Some(&expected_operator));
    assert_eq!(
        json_root
            .get("height")
            .and_then(norito::json::Value::as_u64),
        Some(norito_wire.authoritative.height)
    );
    assert_eq!(
        json_root.get("view").and_then(norito::json::Value::as_u64),
        Some(norito_wire.authoritative.view)
    );
    assert_eq!(
        json_root
            .get("lane_payload_ownerships")
            .and_then(norito::json::Value::as_array)
            .map(Vec::len),
        Some(norito_wire.lane_payload_ownerships.len())
    );
    assert_eq!(
        json_root
            .get("lane_block_sessions")
            .and_then(norito::json::Value::as_array)
            .map(Vec::len),
        Some(norito_wire.lane_block_sessions.len())
    );
    assert_eq!(
        json_root
            .get("local_peer_removed")
            .and_then(norito::json::Value::as_bool),
        Some(norito_wire.local_peer_removed)
    );
}
