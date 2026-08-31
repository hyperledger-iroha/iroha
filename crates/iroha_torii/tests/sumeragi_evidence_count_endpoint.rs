#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Router-level test for GET /v1/sumeragi/evidence/count
#![cfg(feature = "telemetry")]
use axum::{Router, extract::State, routing::get};
use http_body_util::BodyExt as _;
use iroha_core::{
    kura::Kura,
    query::{insert_evidence_record_for_test, store::LiveQueryStore},
    state::{State as CoreState, World},
    telemetry::StateTelemetry,
};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    NetworkId,
    block::{
        BlockHeader,
        consensus::{Evidence, EvidenceRecord, SumeragiV2EquivocationEvidence},
        consensus_v2::{
            BlockSubject, ConsensusMode, ConsensusRound, DataAvailabilityLayout, DualQuorum,
            ExecutionCommitment, GlobalPhase, HeightContext, PROTOCOL_VERSION, PayloadEncoding,
            SumeragiV2Equivocation, ValidatorPower, Vote,
        },
    },
    peer::PeerId,
};
use iroha_torii::handle_v1_sumeragi_evidence_count;
use std::sync::Arc;
use tower::ServiceExt as _; // for Router::oneshot
fn make_phase_vote_evidence(height: u64, seed: u8) -> Evidence {
    let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
        .expect("derive evidence fixture key");
    let roster = vec![ValidatorPower {
        validator: PeerId::new(key_pair.public_key().clone()),
        power: 1,
    }];
    let context = HeightContext {
        network_id: NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([seed; Hash::LENGTH]),
        )),
        protocol_version: PROTOCOL_VERSION,
        height,
        epoch: 0,
        epoch_end_height: height,
        next_epoch_snapshot: None,
        mode: ConsensusMode::Permissioned,
        parent_commit_qc: None,
        snapshot_bootstrap: None,
        quorum: DualQuorum::from_roster(&roster).expect("fixture quorum"),
        roster,
        nexus_amx_context_hash: Hash::new(b"evidence count nexus context"),
        execution_policy_hash: Hash::new(b"evidence count execution policy"),
        da_layout: DataAvailabilityLayout {
            encoding: PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 4,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 1024,
            max_chunk_count: 512,
        },
        leader_seed: [seed; Hash::LENGTH],
    };
    let round = ConsensusRound {
        context_id: context.id(),
        height,
        view: 0,
    };
    let execution_commitment = ExecutionCommitment::without_topups_or_merge_carrier(
        Hash::new(b"evidence count parent state"),
        Hash::new(b"evidence count post state"),
        Hash::new(b"evidence count ordinary writes"),
        1,
        Hash::new([seed]),
    );
    let vote = |subject_seed: u8| Vote {
        round,
        proposal_round: round,
        phase: GlobalPhase::Prepare,
        subject: BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::prehashed(
                [subject_seed; Hash::LENGTH],
            )),
            payload_hash: Hash::new([subject_seed]),
        },
        execution_commitment,
        signer: 0,
        signature: vec![subject_seed; 96],
    };
    Evidence {
        equivocation: SumeragiV2EquivocationEvidence {
            context,
            proofs_of_possession: vec![vec![seed; 96]],
            conflict: SumeragiV2Equivocation::PhaseVote {
                first: vote(seed),
                second: vote(seed.wrapping_add(1)),
            },
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
                get(
                    |state: State<Arc<CoreState>>, headers: http::HeaderMap| async move {
                        let accept = headers.get(http::header::ACCEPT).cloned();
                        handle_v1_sumeragi_evidence_count(state, accept).await
                    },
                ),
            )
            .with_state(state.clone());
        let req0 = http::Request::builder()
            .method("GET")
            .uri("/v1/sumeragi/evidence/count")
            .header(http::header::ACCEPT, "application/json")
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
        let ev = make_phase_vote_evidence((idx + 1) as u64, *seed);
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
            get(
                |state: State<Arc<CoreState>>, headers: http::HeaderMap| async move {
                    let accept = headers.get(http::header::ACCEPT).cloned();
                    handle_v1_sumeragi_evidence_count(state, accept).await
                },
            ),
        )
        .with_state(state.clone());
    let req1 = http::Request::builder()
        .method("GET")
        .uri("/v1/sumeragi/evidence/count")
        .header(http::header::ACCEPT, "application/json")
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
