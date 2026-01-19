//! Router-level test for GET /v1/sumeragi/evidence/count
#![cfg(feature = "telemetry")]

use std::sync::Arc;

use axum::{Router, extract::State, routing::get};
use http_body_util::BodyExt as _;
use iroha_core::{
    kura::Kura,
    query::{insert_evidence_record_for_test, store::LiveQueryStore},
    state::{State as CoreState, World},
    sumeragi::consensus::{Evidence, EvidenceKind, EvidencePayload, Phase, Qc, QcAggregate},
    telemetry::StateTelemetry,
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    block::{BlockHeader, consensus::EvidenceRecord},
    consensus::VALIDATOR_SET_HASH_VERSION_V1,
};
use iroha_torii::handle_v1_sumeragi_evidence_count;
use tower::ServiceExt as _; // for Router::oneshot

fn make_invalid_commit_qc_evidence(height: u64, seed: u8) -> Evidence {
    let subject = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([seed; 32]));
    let certificate = Qc {
        phase: Phase::Prepare,
        subject_block_hash: subject,
        parent_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
        post_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
        height,
        view: 0,
        epoch: 0,
        mode_tag: "test-mode".to_string(),
        highest_qc: None,
        validator_set_hash: HashOf::from_untyped_unchecked(Hash::prehashed([0x11; 32])),
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set: Vec::new(),
        aggregate: QcAggregate {
            signers_bitmap: vec![0x01],
            bls_aggregate_signature: Vec::new(),
        },
    };
    Evidence {
        kind: EvidenceKind::InvalidQc,
        payload: EvidencePayload::InvalidQc {
            certificate,
            reason: "test".to_string(),
        },
    }
}

#[tokio::test]
async fn evidence_count_endpoint_reports_increase() {
    let kura = Arc::new(Kura::blank_kura_for_testing());
    let query = LiveQueryStore::start_test();
    let mut state = Arc::new(CoreState::with_telemetry(
        World::default(),
        Arc::clone(&kura),
        query,
        StateTelemetry::default(),
    ));

    {
        let app = Router::new()
            .route(
                "/v1/sumeragi/evidence/count",
                get(|state: State<Arc<CoreState>>| async move {
                    handle_v1_sumeragi_evidence_count(state, None).await
                }),
            )
            .with_state(state.clone());

        let req0 = http::Request::builder()
            .method("GET")
            .uri("/v1/sumeragi/evidence/count")
            .body(axum::body::Body::empty())
            .unwrap();
        let resp0 = app.clone().oneshot(req0).await.unwrap();
        assert_eq!(resp0.status(), http::StatusCode::OK);
        let body0 = resp0.into_body().collect().await.unwrap().to_bytes();
        let v0: norito::json::Value = norito::json::from_slice(&body0).unwrap();
        let c0 = v0
            .get("count")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        assert_eq!(c0, 0);
    }

    let state_mut = Arc::get_mut(&mut state).expect("state Arc should be uniquely owned here");

    // Insert two WSV-backed evidence records
    for (idx, seed) in [0x11u8, 0x22].iter().enumerate() {
        let ev = make_invalid_commit_qc_evidence((idx + 1) as u64, *seed);
        let record = EvidenceRecord {
            evidence: ev,
            recorded_at_height: (idx + 1) as u64,
            recorded_at_view: 0,
            recorded_at_ms: 0,
            penalty_applied: false,
            penalty_cancelled: false,
            penalty_cancelled_at_height: None,
            penalty_applied_at_height: None,
        };
        insert_evidence_record_for_test(state_mut, record);
    }

    let app = Router::new()
        .route(
            "/v1/sumeragi/evidence/count",
            get(|state: State<Arc<CoreState>>| async move {
                handle_v1_sumeragi_evidence_count(state, None).await
            }),
        )
        .with_state(state.clone());

    let req1 = http::Request::builder()
        .method("GET")
        .uri("/v1/sumeragi/evidence/count")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp1 = app.clone().oneshot(req1).await.unwrap();
    assert_eq!(resp1.status(), http::StatusCode::OK);
    let body1 = resp1.into_body().collect().await.unwrap().to_bytes();
    let v1j: norito::json::Value = norito::json::from_slice(&body1).unwrap();
    let c1 = v1j
        .get("count")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);

    assert_eq!(c1, 2);
}
