//! Router-level coverage for the authoritative Sumeragi v2 QC endpoint.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(feature = "telemetry")]
use axum::{Router, body::Body, http::Request, routing::get};
use http_body_util::BodyExt as _;
use iroha_core::sumeragi::status;
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::block::{
    BlockHeader,
    consensus_v2::{
        BlockSubject, ConsensusMode, ConsensusRound, DualQuorum, ExecutionCommitment, GlobalPhase,
        HeightContext, HeightContextId, PROTOCOL_VERSION, QuorumCertificateRef,
        SumeragiV2BodyState, SumeragiV2HeightContextStatus, SumeragiV2Status,
        SumeragiV2StatusPhase,
    },
};
use iroha_torii::SumeragiV2QcResponse;
use std::sync::{Mutex, MutexGuard};
use tower::ServiceExt as _;
static QC_ENDPOINT_TEST_LOCK: Mutex<()> = Mutex::new(());
struct PublishedStatus {
    _guard: MutexGuard<'static, ()>,
}
impl PublishedStatus {
    fn install(value: SumeragiV2Status) -> Self {
        let guard = QC_ENDPOINT_TEST_LOCK
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
fn status_fixture() -> (SumeragiV2Status, QuorumCertificateRef) {
    let context_id = HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(Hash::new(
        b"height-context",
    )));
    let subject = BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"block")),
        payload_hash: Hash::new(b"payload"),
    };
    let round = ConsensusRound {
        context_id,
        height: 42,
        view: 3,
    };
    let certificate = QuorumCertificateRef {
        round,
        proposal_round: round,
        phase: GlobalPhase::Prepare,
        subject,
        execution_commitment: ExecutionCommitment::without_offline_cash_top_ups_or_merge_carrier(
            Hash::new(b"parent-state"),
            Hash::new(b"post-state"),
            Hash::new(b"writes"),
            1,
            Hash::new(b"executed-block-wire"),
        ),
    };
    (
        SumeragiV2Status {
            protocol_version: PROTOCOL_VERSION,
            node_fingerprint: Hash::new(b"node"),
            build_fingerprint: Hash::new(b"build"),
            config_fingerprint: Hash::new(b"config"),
            restart_required: false,
            height_context_id: context_id,
            height: 42,
            view: 3,
            phase: SumeragiV2StatusPhase::Commit,
            leader: 2,
            locked_prepare_qc: Some(certificate),
            highest_prepare_qc: Some(certificate),
            last_timeout_certificate: None,
            body_state: SumeragiV2BodyState::Validated,
            pending_persistence_id: None,
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
        },
        certificate,
    )
}
fn qc_endpoint_app() -> Router {
    Router::new().route(
        "/v1/sumeragi/qc",
        get(|headers: axum::http::HeaderMap| async move {
            iroha_torii::handle_v1_sumeragi_qc(headers.get(axum::http::header::ACCEPT).cloned())
                .await
        }),
    )
}
#[tokio::test]
async fn sumeragi_qc_json_uses_v2_certificate_references() {
    let (status, certificate) = status_fixture();
    let _published = PublishedStatus::install(status);
    let response = qc_endpoint_app()
        .oneshot(
            Request::builder()
                .uri("/v1/sumeragi/qc")
                .body(Body::empty())
                .expect("QC request"),
        )
        .await
        .expect("QC response");
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = response
        .into_body()
        .collect()
        .await
        .expect("collect QC JSON")
        .to_bytes();
    let decoded: SumeragiV2QcResponse = norito::json::from_slice(&body).expect("decode v2 QC JSON");
    assert_eq!(decoded.highest_prepare_qc, Some(certificate));
    assert_eq!(decoded.locked_prepare_qc, Some(certificate));
}
#[tokio::test]
async fn sumeragi_qc_norito_uses_v2_certificate_references() {
    let (status, certificate) = status_fixture();
    let _published = PublishedStatus::install(status);
    let response = qc_endpoint_app()
        .oneshot(
            Request::builder()
                .uri("/v1/sumeragi/qc")
                .header(axum::http::header::ACCEPT, "application/x-norito")
                .body(Body::empty())
                .expect("QC request"),
        )
        .await
        .expect("QC response");
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = response
        .into_body()
        .collect()
        .await
        .expect("collect QC Norito")
        .to_bytes();
    let decoded: SumeragiV2QcResponse =
        norito::decode_from_bytes(&body).expect("decode v2 QC Norito");
    assert_eq!(decoded.highest_prepare_qc, Some(certificate));
    assert_eq!(decoded.locked_prepare_qc, Some(certificate));
}
