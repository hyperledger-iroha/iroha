//! Router-level test for GET /v1/sumeragi/status
#![cfg(feature = "telemetry")]

use std::sync::{Arc, Mutex, OnceLock};

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use iroha_config::parameters::actual::Queue;
use iroha_core::{
    kiso::KisoHandle,
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
    sumeragi::da::GateReason,
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    ChainId,
    block::{
        BlockHeader,
        consensus::{SumeragiDaGateReason, SumeragiDaGateSatisfaction, SumeragiStatusWire},
    },
    peer::PeerId,
};
use iroha_primitives::time::TimeSource;
use tower::ServiceExt as _;

#[path = "fixtures.rs"]
mod fixtures;

fn build_status_router() -> Router {
    let cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());
    let mut world = World::default();
    world.peers.mutate_vec(|peers| {
        let _ = peers.push(local_peer_id.clone());
    });
    let state = Arc::new(State::new_for_testing(world, kura.clone(), query.clone()));
    let queue_cfg = Queue::default();
    let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(iroha_core::queue::Queue::from_config(
        queue_cfg,
        events_sender,
    ));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx;

    let telemetry = {
        let metrics = fixtures::shared_metrics();
        let (_mh, ts) = TimeSource::new_mock(core::time::Duration::default());
        iroha_core::telemetry::start(
            metrics,
            state.clone(),
            kura.clone(),
            queue.clone(),
            peers_rx.clone(),
            local_peer_id,
            ts,
            true,
        )
        .0
    };

    let da_receipt_signer = cfg.common.key_pair.clone();
    let torii = iroha_torii::Torii::new(
        ChainId::from("test-chain"),
        kiso,
        cfg.torii.clone(),
        queue,
        tokio::sync::broadcast::channel(1).0,
        query,
        kura,
        state,
        da_receipt_signer,
        iroha_torii::OnlinePeersProvider::new(peers_rx),
        telemetry,
        true,
    );

    torii.api_router_for_tests()
}

fn status_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[allow(clippy::await_holding_lock, clippy::too_many_lines)]
#[tokio::test]
async fn sumeragi_status_endpoint_shape() {
    let _guard = status_lock().lock().unwrap();
    let app = build_status_router();

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/sumeragi/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = BodyExt::collect(resp.into_body()).await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&body).unwrap();
    assert!(v.get("leader_index").is_some());
    let hq = v.get("highest_qc").and_then(|x| x.as_object()).unwrap();
    assert!(hq.get("height").is_some());
    assert!(hq.get("view").is_some());
    assert!(hq.contains_key("subject_block_hash"));
    let lq = v.get("locked_qc").and_then(|x| x.as_object()).unwrap();
    assert!(lq.get("height").is_some());
    assert!(lq.get("view").is_some());
    assert!(lq.contains_key("subject_block_hash"));
    assert!(v.get("gossip_fallback_total").is_some());
    assert!(v.get("block_created_dropped_by_lock_total").is_some());
    assert!(v.get("block_created_hint_mismatch_total").is_some());
    assert!(v.get("block_created_proposal_mismatch_total").is_some());
    assert!(v.get("pacemaker_backpressure_deferrals_total").is_some());
    let view_change_causes = v
        .get("view_change_causes")
        .and_then(|x| x.as_object())
        .unwrap();
    assert!(view_change_causes.get("validation_reject_total").is_some());
    assert!(view_change_causes.get("missing_payload_total").is_some());
    assert!(view_change_causes.get("missing_qc_total").is_some());
    assert!(view_change_causes.get("quorum_timeout_total").is_some());
    assert!(view_change_causes.get("commit_failure_total").is_some());
    assert!(view_change_causes.get("da_gate_total").is_some());
    assert!(view_change_causes.get("last_cause").is_some());
    assert!(view_change_causes.get("last_cause_timestamp_ms").is_some());
    let validation_rejects = v
        .get("validation_rejects")
        .and_then(|x| x.as_object())
        .unwrap();
    assert!(validation_rejects.get("total").is_some());
    assert!(validation_rejects.get("stateless_total").is_some());
    assert!(validation_rejects.get("execution_total").is_some());
    assert!(validation_rejects.get("prev_hash_total").is_some());
    assert!(validation_rejects.get("prev_height_total").is_some());
    assert!(validation_rejects.get("topology_total").is_some());
    assert!(validation_rejects.get("last_reason").is_some());
    assert!(validation_rejects.get("last_height").is_some());
    assert!(validation_rejects.get("last_view").is_some());
    assert!(validation_rejects.get("last_block").is_some());
    assert!(validation_rejects.get("last_timestamp_ms").is_some());
    let peer_key_policy = v
        .get("peer_key_policy")
        .and_then(|x| x.as_object())
        .unwrap();
    assert!(peer_key_policy.get("total").is_some());
    assert!(peer_key_policy.get("missing_hsm_total").is_some());
    let txq = v.get("tx_queue").and_then(|x| x.as_object()).unwrap();
    assert!(txq.get("depth").is_some());
    assert!(txq.get("capacity").is_some());
    assert!(txq.get("saturated").is_some());
    let worker_loop = v.get("worker_loop").and_then(|x| x.as_object()).unwrap();
    assert!(worker_loop.get("queue_depths").is_some());
    assert!(worker_loop.get("queue_diagnostics").is_some());
    let commit_inflight = v
        .get("commit_inflight")
        .and_then(|x| x.as_object())
        .unwrap();
    assert!(commit_inflight.get("active").is_some());
    assert!(commit_inflight.get("pause_queue_depths").is_some());
    let prf = v.get("prf").and_then(|x| x.as_object()).unwrap();
    assert!(prf.get("height").is_some());
    assert!(prf.get("view").is_some());
    assert!(prf.contains_key("epoch_seed"));
    assert!(
        v.get("view_change_proof_accepted_total")
            .and_then(norito::json::Value::as_u64)
            .is_some()
    );
    assert!(
        v.get("view_change_proof_stale_total")
            .and_then(norito::json::Value::as_u64)
            .is_some()
    );
    assert!(
        v.get("view_change_proof_rejected_total")
            .and_then(norito::json::Value::as_u64)
            .is_some()
    );
    assert!(
        v.get("view_change_suggest_total")
            .and_then(norito::json::Value::as_u64)
            .is_some()
    );
    assert!(
        v.get("view_change_install_total")
            .and_then(norito::json::Value::as_u64)
            .is_some()
    );
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn sumeragi_status_endpoint_locked_qc_monotonic() {
    let _guard = status_lock().lock().unwrap();
    iroha_core::sumeragi::status::set_locked_qc(0, 0, None);
    let initial_hash =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x11; Hash::LENGTH]));
    let updated_hash =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x22; Hash::LENGTH]));

    let app = build_status_router();

    iroha_core::sumeragi::status::set_locked_qc(10, 2, Some(initial_hash));
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/sumeragi/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = BodyExt::collect(resp.into_body()).await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&body).unwrap();
    let lq = v.get("locked_qc").and_then(|x| x.as_object()).unwrap();
    assert_eq!(
        lq.get("height").and_then(norito::json::Value::as_u64),
        Some(10)
    );
    assert_eq!(
        lq.get("view").and_then(norito::json::Value::as_u64),
        Some(2)
    );
    assert_eq!(
        lq.get("subject_block_hash")
            .and_then(norito::json::Value::as_str)
            .map(ToString::to_string),
        Some(format!("{initial_hash}"))
    );

    iroha_core::sumeragi::status::set_locked_qc(9, 5, None);
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/sumeragi/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = BodyExt::collect(resp.into_body()).await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&body).unwrap();
    let lq = v.get("locked_qc").and_then(|x| x.as_object()).unwrap();
    assert_eq!(
        lq.get("height").and_then(norito::json::Value::as_u64),
        Some(10)
    );
    assert_eq!(
        lq.get("view").and_then(norito::json::Value::as_u64),
        Some(2)
    );

    iroha_core::sumeragi::status::set_locked_qc(12, 0, Some(updated_hash));
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/sumeragi/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = BodyExt::collect(resp.into_body()).await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&body).unwrap();
    let lq = v.get("locked_qc").and_then(|x| x.as_object()).unwrap();
    assert_eq!(
        lq.get("height").and_then(norito::json::Value::as_u64),
        Some(12)
    );
    assert_eq!(
        lq.get("view").and_then(norito::json::Value::as_u64),
        Some(0)
    );
    assert_eq!(
        lq.get("subject_block_hash")
            .and_then(norito::json::Value::as_str)
            .map(ToString::to_string),
        Some(format!("{updated_hash}"))
    );
}

#[allow(clippy::await_holding_lock, clippy::too_many_lines)]
#[tokio::test]
async fn sumeragi_status_endpoint_reflects_leader_and_highest_qc() {
    let _guard = status_lock().lock().unwrap();
    let app = build_status_router();

    iroha_core::sumeragi::status::set_leader_index(3);
    iroha_core::sumeragi::status::set_highest_qc(7, 4);
    let membership_hash = [0xCDu8; 32];
    iroha_core::sumeragi::status::set_membership_view_hash(membership_hash, 11, 5, 2);
    let expected_hash_hex = hex::encode(membership_hash);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/sumeragi/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = BodyExt::collect(resp.into_body()).await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&body).unwrap();
    assert_eq!(
        v.get("leader_index").and_then(norito::json::Value::as_u64),
        Some(3)
    );
    let hq = v.get("highest_qc").and_then(|x| x.as_object()).unwrap();
    assert_eq!(
        hq.get("height").and_then(norito::json::Value::as_u64),
        Some(7)
    );
    assert_eq!(
        hq.get("view").and_then(norito::json::Value::as_u64),
        Some(4)
    );
    let membership = v
        .get("membership")
        .and_then(|value| value.as_object())
        .expect("membership object present");
    assert_eq!(
        membership
            .get("height")
            .and_then(norito::json::Value::as_u64),
        Some(11)
    );
    assert_eq!(
        membership.get("view").and_then(norito::json::Value::as_u64),
        Some(5)
    );
    assert_eq!(
        membership
            .get("epoch")
            .and_then(norito::json::Value::as_u64),
        Some(2)
    );
    assert_eq!(
        membership
            .get("view_hash")
            .and_then(norito::json::Value::as_str),
        Some(expected_hash_hex.as_str())
    );
    let membership_mismatch = v
        .get("membership_mismatch")
        .and_then(|value| value.as_object())
        .expect("membership_mismatch object present");
    assert!(
        membership_mismatch
            .get("active_peers")
            .and_then(norito::json::Value::as_array)
            .is_some_and(Vec::is_empty)
    );
    assert!(
        membership_mismatch
            .get("last_peer")
            .is_some_and(norito::json::Value::is_null)
    );
    assert_eq!(
        membership_mismatch
            .get("last_height")
            .and_then(norito::json::Value::as_u64),
        Some(0)
    );
    assert_eq!(
        membership_mismatch
            .get("last_view")
            .and_then(norito::json::Value::as_u64),
        Some(0)
    );
    assert_eq!(
        membership_mismatch
            .get("last_epoch")
            .and_then(norito::json::Value::as_u64),
        Some(0)
    );
    assert!(
        membership_mismatch
            .get("last_local_hash")
            .is_some_and(norito::json::Value::is_null)
    );
    assert!(
        membership_mismatch
            .get("last_remote_hash")
            .is_some_and(norito::json::Value::is_null)
    );
    assert_eq!(
        membership_mismatch
            .get("last_timestamp_ms")
            .and_then(norito::json::Value::as_u64),
        Some(0)
    );
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn sumeragi_status_endpoint_supports_norito_payload() {
    let _guard = status_lock().lock().unwrap();
    let app = build_status_router();
    iroha_core::sumeragi::status::set_highest_qc(0, 0);
    iroha_core::sumeragi::status::set_locked_qc(0, 0, None);
    iroha_core::sumeragi::status::set_highest_qc(9, 4);
    iroha_core::sumeragi::status::set_locked_qc(8, 3, None);
    iroha_core::sumeragi::status::record_missing_block_fetch(3, 17);
    iroha_core::sumeragi::status::record_da_gate_transition(
        None,
        Some(GateReason::MissingLocalData),
    );
    iroha_core::sumeragi::status::record_da_gate_transition(
        Some(GateReason::MissingLocalData),
        None,
    );
    iroha_core::sumeragi::status::record_da_gate_transition(
        None,
        Some(GateReason::MissingLocalData),
    );
    let kura_hash =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xAB; Hash::LENGTH]));
    iroha_core::sumeragi::status::record_kura_store_failure(5, 2, kura_hash);
    iroha_core::sumeragi::status::record_kura_store_retry(7, 11);
    iroha_core::sumeragi::status::inc_kura_store_abort();

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/sumeragi/status")
                .header("Accept", "application/x-norito")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .and_then(|h| h.to_str().ok()),
        Some("application/x-norito")
    );

    let bytes = BodyExt::collect(resp.into_body()).await.unwrap().to_bytes();
    let wire: SumeragiStatusWire =
        norito::decode_from_bytes(&bytes).expect("status Norito payload must decode");
    assert!(wire.prf_epoch_seed.is_none());
    assert_eq!(wire.prf_height, 0);
    assert_eq!(wire.prf_view, 0);
    assert_eq!(wire.membership.height, 0);
    assert_eq!(wire.membership.view, 0);
    assert_eq!(wire.membership.epoch, 0);
    assert!(wire.membership.view_hash.is_none());
    assert!(wire.membership_mismatch.active_peers.is_empty());
    assert!(wire.membership_mismatch.last_peer.is_none());
    assert_eq!(wire.membership_mismatch.last_height, 0);
    assert_eq!(wire.membership_mismatch.last_view, 0);
    assert_eq!(wire.membership_mismatch.last_epoch, 0);
    assert!(wire.membership_mismatch.last_local_hash.is_none());
    assert!(wire.membership_mismatch.last_remote_hash.is_none());
    assert_eq!(wire.membership_mismatch.last_timestamp_ms, 0);
    assert!(wire.missing_block_fetch.total >= 1);
    assert_eq!(wire.missing_block_fetch.last_targets, 3);
    assert_eq!(wire.missing_block_fetch.last_dwell_ms, 17);
    assert_eq!(wire.view_change_causes.validation_reject_total, 0);
    assert_eq!(wire.validation_rejects.total, 0);
    assert!(wire.validation_rejects.last_reason.is_none());
    assert!(!wire.commit_inflight.active);
    assert_eq!(wire.commit_inflight.timeout_total, 0);
    assert_eq!(wire.worker_loop.queue_diagnostics.blocked_total.vote_rx, 0);
    assert_eq!(wire.da_gate.reason, SumeragiDaGateReason::MissingLocalData);
    assert_eq!(
        wire.da_gate.last_satisfied,
        SumeragiDaGateSatisfaction::MissingDataRecovered
    );
    assert!(wire.da_gate.missing_local_data_total >= 1);
    assert_eq!(wire.da_gate.manifest_guard_total, 0);
    assert!(wire.kura_store.failures_total >= 1);
    assert!(wire.kura_store.abort_total >= 1);
    assert_eq!(wire.kura_store.stage_total, 0);
    assert_eq!(wire.kura_store.rollback_total, 0);
    assert_eq!(wire.kura_store.lock_reset_total, 0);
    assert_eq!(wire.kura_store.last_height, 5);
    assert_eq!(wire.kura_store.last_view, 2);
    assert_eq!(wire.kura_store.last_hash, Some(kura_hash));
    assert_eq!(wire.kura_store.last_retry_attempt, 7);
    assert_eq!(wire.kura_store.last_retry_backoff_ms, 11);
}
