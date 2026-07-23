//! Router-level coverage for the authoritative Sumeragi v2 leader endpoint.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(feature = "telemetry")]

use std::sync::Mutex;

use axum::{Router, body::Body, http::Request, routing::get};
use http_body_util::BodyExt as _;
use iroha_core::sumeragi::status;
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::block::consensus_v2::{
    ConsensusMode, DualQuorum, HeightContext, HeightContextId, PROTOCOL_VERSION,
    SumeragiV2BodyState, SumeragiV2HeightContextStatus, SumeragiV2Status, SumeragiV2StatusPhase,
};
use tower::ServiceExt as _;

static LEADER_ENDPOINT_TEST_LOCK: Mutex<()> = Mutex::new(());

#[tokio::test]
async fn sumeragi_leader_endpoint_uses_authoritative_v2_round() {
    let _guard = LEADER_ENDPOINT_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let published = SumeragiV2Status {
        protocol_version: PROTOCOL_VERSION,
        node_fingerprint: Hash::new(b"node"),
        build_fingerprint: Hash::new(b"build"),
        config_fingerprint: Hash::new(b"config"),
        restart_required: false,
        height_context_id: HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(
            Hash::new(b"height-context"),
        )),
        height: 123,
        view: 4,
        phase: SumeragiV2StatusPhase::AwaitingProposal,
        leader: 2,
        locked_prepare_qc: None,
        highest_prepare_qc: None,
        last_timeout_certificate: None,
        body_state: SumeragiV2BodyState::Missing,
        pending_persistence_id: None,
        last_committed_height: 0,
        last_committed_subject: None,
        height_context: SumeragiV2HeightContextStatus {
            epoch: 1,
            epoch_end_height: 200,
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
    };
    published.validate().expect("valid leader status fixture");
    status::set_v2_status(published);

    let app = Router::new().route(
        "/v1/sumeragi/leader",
        get(|| async move {
            iroha_torii::handle_v1_sumeragi_leader(Some(axum::http::HeaderValue::from_static(
                "application/json",
            )))
            .await
        }),
    );
    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/sumeragi/leader")
                .body(Body::empty())
                .expect("leader request"),
        )
        .await
        .expect("leader response");
    status::clear_v2_status();

    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = response
        .into_body()
        .collect()
        .await
        .expect("collect leader response")
        .to_bytes();
    let value: norito::json::Value = norito::json::from_slice(&body).expect("decode leader JSON");
    assert_eq!(value.get("leader_index").and_then(|x| x.as_u64()), Some(2));
    let round = value
        .get("prf")
        .and_then(norito::json::Value::as_object)
        .expect("round context");
    assert_eq!(round.get("height").and_then(|x| x.as_u64()), Some(123));
    assert_eq!(round.get("view").and_then(|x| x.as_u64()), Some(4));
    assert!(round.get("epoch_seed").is_none());
}
