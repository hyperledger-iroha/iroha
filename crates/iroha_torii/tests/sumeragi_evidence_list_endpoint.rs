//! Telemetry-enabled tests for the sumeragi evidence list endpoint.
#![cfg(feature = "telemetry")]

use std::sync::Arc;

use axum::extract::State;
use http_body_util::BodyExt as _;
use iroha_core::{
    kura::Kura,
    query::{insert_evidence_record_for_test, store::LiveQueryStore},
    state::{State as CoreState, World},
    sumeragi::consensus::{Evidence, EvidenceKind, EvidencePayload, Phase, Qc, QcAggregate, Vote},
    telemetry::StateTelemetry,
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    block::{BlockHeader, consensus::EvidenceRecord},
    consensus::VALIDATOR_SET_HASH_VERSION_V1,
};
use iroha_torii::{EvidenceListQuery, NoritoQuery, handle_v1_sumeragi_evidence_list};

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

fn make_double_prevote_evidence(height: u64, seed: u8) -> Evidence {
    fn vote(height: u64, signer: u32, seed: u8, phase: Phase) -> Vote {
        let hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([seed; 32]));
        Vote {
            phase,
            block_hash: hash,
            parent_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
            post_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
            height,
            view: 0,
            epoch: 0,
            highest_qc: None,
            signer,
            bls_sig: Vec::new(),
        }
    }

    Evidence {
        kind: EvidenceKind::DoublePrepare,
        payload: EvidencePayload::DoubleVote {
            v1: vote(height, 1, seed, Phase::Prepare),
            v2: vote(height, 1, seed.wrapping_add(1), Phase::Prepare),
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
            evidence: make_invalid_commit_qc_evidence(10, 0xA1),
            recorded_at_height: 1,
            recorded_at_view: 0,
            recorded_at_ms: 10,
            penalty_applied: false,
            penalty_applied_at_height: None,
        },
        EvidenceRecord {
            evidence: make_double_prevote_evidence(20, 0xB2),
            recorded_at_height: 2,
            recorded_at_view: 0,
            recorded_at_ms: 20,
            penalty_applied: false,
            penalty_applied_at_height: None,
        },
        EvidenceRecord {
            evidence: make_invalid_commit_qc_evidence(30, 0xC3),
            recorded_at_height: 3,
            recorded_at_view: 0,
            recorded_at_ms: 30,
            penalty_applied: false,
            penalty_applied_at_height: None,
        },
    ];
    for record in records {
        insert_evidence_record_for_test(&mut state, record);
    }

    let state = Arc::new(state);

    let query_all = EvidenceListQuery {
        limit: Some(2),
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
    assert_eq!(items.len(), 2);
    assert_eq!(
        items[0].get("kind").and_then(norito::json::Value::as_str),
        Some("InvalidQc")
    );
    assert_eq!(
        items[1].get("kind").and_then(norito::json::Value::as_str),
        Some("DoublePrepare")
    );

    let query_filtered = EvidenceListQuery {
        limit: Some(1),
        offset: Some(1),
        kind: Some("InvalidQc".to_string()),
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
        Some(2)
    );
    let filtered_items = json_filtered
        .get("items")
        .and_then(norito::json::Value::as_array)
        .cloned()
        .expect("items array");
    assert_eq!(filtered_items.len(), 1);
    assert_eq!(
        filtered_items[0]
            .get("subject_block_hash")
            .and_then(norito::json::Value::as_str)
            .map(str::len),
        Some(64),
    );
}
