#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Telemetry-enabled tests for the sumeragi evidence list endpoint.
#![cfg(feature = "telemetry")]
use axum::extract::State;
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
        consensus::{
            Evidence, EvidencePenaltyStatus, EvidenceRecord, SumeragiV2EquivocationEvidence,
        },
        consensus_v2::{
            BlockSubject, ConsensusMode, ConsensusRound, DataAvailabilityLayout, DualQuorum,
            ExecutionCommitment, GlobalPhase, HeightContext, PROTOCOL_VERSION, PayloadEncoding,
            SumeragiV2Equivocation, ValidatorPower, Vote,
        },
    },
    peer::PeerId,
};
use iroha_torii::{Error, EvidenceListQuery, NoritoQuery, handle_v1_sumeragi_evidence_list};
use std::sync::Arc;
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
        nexus_amx_context_hash: Hash::new(b"evidence list nexus context"),
        execution_policy_hash: Hash::new(b"evidence list execution policy"),
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
        Hash::new(b"evidence list parent state"),
        Hash::new(b"evidence list post state"),
        Hash::new(b"evidence list ordinary writes"),
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
#[allow(clippy::too_many_lines)]
async fn evidence_list_endpoint_supports_filters_and_pagination() {
    let kura = Arc::new(Kura::blank_kura_for_testing());
    let query = LiveQueryStore::start_test();
    let state = CoreState::with_telemetry(
        World::default(),
        Arc::clone(&kura),
        query,
        StateTelemetry::default(),
    );
    let mut state = state;
    let records = [
        EvidenceRecord {
            evidence: make_phase_vote_evidence(10, 0xA1),
            recorded_at_height: 1,
            recorded_at_view: 0,
            recorded_at_ms: 10,
            penalty_status: EvidencePenaltyStatus::Pending,
        },
        EvidenceRecord {
            evidence: make_phase_vote_evidence(20, 0xB2),
            recorded_at_height: 2,
            recorded_at_view: 0,
            recorded_at_ms: 20,
            penalty_status: EvidencePenaltyStatus::Applied { height: 4 },
        },
        EvidenceRecord {
            evidence: make_phase_vote_evidence(30, 0xC3),
            recorded_at_height: 3,
            recorded_at_view: 0,
            recorded_at_ms: 30,
            penalty_status: EvidencePenaltyStatus::Cancelled { height: 5 },
        },
    ];
    for record in records {
        insert_evidence_record_for_test(&mut state, record);
    }
    let state = Arc::new(state);
    let query_all = EvidenceListQuery {
        limit: Some(3),
        offset: Some(0),
        kind: None,
    };
    let response =
        handle_v1_sumeragi_evidence_list(State(state.clone()), NoritoQuery(query_all), None)
            .await
            .expect("handler returns OK");
    let body = response
        .into_body()
        .collect()
        .await
        .expect("read body")
        .to_bytes();
    let json: norito::json::Value = norito::json::from_slice(&body).expect("parse json");
    assert_eq!(
        json.get("total").and_then(norito::json::Value::as_u64),
        Some(3)
    );
    let items = json
        .get("items")
        .and_then(norito::json::Value::as_array)
        .cloned()
        .expect("array of items");
    assert_eq!(items.len(), 3);
    let expected_statuses = [
        norito::json::json!({
            "status": "cancelled",
            "details": { "height": 5 }
        }),
        norito::json::json!({
            "status": "applied",
            "details": { "height": 4 }
        }),
        norito::json::json!({
            "status": "pending",
            "details": null
        }),
    ];
    for ((item, expected_status), expected_admission_height) in items
        .iter()
        .zip(expected_statuses.iter())
        .zip([3_u64, 2, 1])
    {
        assert_eq!(
            item.get("kind").and_then(norito::json::Value::as_str),
            Some("SumeragiV2Equivocation")
        );
        assert_eq!(item.get("penalty_status"), Some(expected_status));
        assert_eq!(
            item.get("consensus_admitted_height")
                .and_then(norito::json::Value::as_u64),
            Some(expected_admission_height)
        );
        let item = item.as_object().expect("evidence audit object");
        for retired in [
            "penalty_applied",
            "penalty_cancelled",
            "penalty_cancelled_at_height",
            "penalty_applied_at_height",
            "consensus_admitted_at_height",
        ] {
            assert!(
                !item.contains_key(retired),
                "retired evidence field `{retired}` must remain absent"
            );
        }
    }
    let query_filtered = EvidenceListQuery {
        limit: Some(1),
        offset: Some(1),
        kind: Some("SumeragiV2Equivocation".to_string()),
    };
    let response_filtered =
        handle_v1_sumeragi_evidence_list(State(state.clone()), NoritoQuery(query_filtered), None)
            .await
            .expect("handler returns OK");
    let body_filtered = response_filtered
        .into_body()
        .collect()
        .await
        .expect("read body")
        .to_bytes();
    let json_filtered: norito::json::Value =
        norito::json::from_slice(&body_filtered).expect("parse json");
    assert_eq!(
        json_filtered
            .get("total")
            .and_then(norito::json::Value::as_u64),
        Some(3)
    );
    let filtered_items = json_filtered
        .get("items")
        .and_then(norito::json::Value::as_array)
        .cloned()
        .expect("items array");
    assert_eq!(filtered_items.len(), 1);
    assert_eq!(
        filtered_items[0]
            .get("class")
            .and_then(norito::json::Value::as_str),
        Some("phase_vote"),
    );
    for kind in ["SumeragiV2Equivocation"] {
        let query = EvidenceListQuery {
            limit: None,
            offset: None,
            kind: Some(kind.to_owned()),
        };
        assert!(
            handle_v1_sumeragi_evidence_list(State(state.clone()), NoritoQuery(query), None)
                .await
                .is_ok(),
            "canonical evidence kind `{kind}` must be accepted"
        );
    }
    for kind in [
        "DoublePrevote",
        "DoublePrecommit",
        "DoublePrepare",
        "DoubleCommit",
        "InvalidQc",
        "InvalidProposal",
        "Censorship",
        "InvalidQC",
        "doubleprepare",
        " InvalidQc",
        "InvalidQc ",
        "null",
        "NULL",
        "",
        "Unknown",
    ] {
        let query = EvidenceListQuery {
            limit: None,
            offset: None,
            kind: Some(kind.to_owned()),
        };
        let result =
            handle_v1_sumeragi_evidence_list(State(state.clone()), NoritoQuery(query), None).await;
        let Err(Error::AppQueryValidation { code, message }) = result else {
            panic!("noncanonical evidence kind `{kind}` must fail closed");
        };
        assert_eq!(code, "sumeragi_evidence_kind_invalid");
        assert!(message.contains(kind));
    }
}
