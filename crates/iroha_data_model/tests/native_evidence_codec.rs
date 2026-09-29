//! Native evidence framing, canonical pair order and required historical attribution.

use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::block::consensus::{
    Evidence, EvidenceAttribution, EvidenceOffender, EvidencePenaltyStatus, EvidenceRecord,
};
use iroha_data_model::consensus::NposConsensusEffects;
use iroha_model_base::peer::PeerId;
use iroha_sumeragi::{
    message::{Evidence as NativeEvidence, MAX_EVIDENCE_FRAME_BYTES, Vote, VoteKind},
    types::{EpochId, Hash32, SIGNATURE_LEN, Signature},
};
use norito::codec::{DecodeAll, Encode};

fn native() -> NativeEvidence {
    let first = Vote {
        kind: VoteKind::Prepare,
        instance: Hash32([1; 32]),
        epoch: EpochId {
            epoch: 7,
            context: Hash32([2; 32]),
        },
        height: 12,
        view: 3,
        block_hash: Hash32([3; 32]),
        result: Hash32([4; 32]),
        attest: false,
        signer: 2,
        sig: Signature([5; SIGNATURE_LEN]),
        attestation: None,
    };
    let mut second = first.clone();
    second.block_hash = Hash32([6; 32]);
    NativeEvidence::VoteEquivocation(first, second)
}
fn record() -> EvidenceRecord {
    let key = KeyPair::try_from_seed(vec![7; 32], Algorithm::Ed25519).expect("fixture key");
    EvidenceRecord {
        evidence: Evidence::from_native(&native()).expect("bounded original frame"),
        attribution: EvidenceAttribution {
            instance: [1; 32],
            height: 12,
            epoch: 7,
            context_id: [2; 32],
            authority_generation: [8; 32],
            offenders: vec![EvidenceOffender {
                signer: 2,
                peer_id: PeerId::from(key.public_key().clone()),
            }],
            safety_violation: false,
        },
        recorded_at_height: 13,
        recorded_at_view: 1,
        recorded_at_ms: 1234,
        penalty_status: EvidencePenaltyStatus::Pending,
    }
}
#[test]
fn native_pair_constructor_is_order_independent_and_wire_is_strict() {
    let evidence = Evidence::from_native(&native()).unwrap();
    let NativeEvidence::VoteEquivocation(first, second) = evidence.decode_native().unwrap() else {
        panic!("vote pair")
    };
    let reversed = NativeEvidence::VoteEquivocation(second, first);
    assert_eq!(Evidence::from_native(&reversed).unwrap(), evidence);
    let invalid = Evidence {
        native: reversed.encode().unwrap(),
    };
    assert!(invalid.decode_native().is_err());
    assert!(Evidence::decode_all(&mut invalid.encode().as_slice()).is_err());
    assert!(norito::json::from_str::<Evidence>(&norito::json::to_json(&invalid).unwrap()).is_err());
    assert_eq!(evidence.native_frame(), evidence.native.as_slice());
}
#[test]
fn native_evidence_roundtrips_with_exact_attribution() {
    let record = record();
    assert_eq!(
        EvidenceRecord::decode_all(&mut record.encode().as_slice()).unwrap(),
        record
    );
    let json = norito::json::to_value(&record).unwrap();
    assert_eq!(
        norito::json::from_value::<EvidenceRecord>(json.clone()).unwrap(),
        record
    );
    for field in [
        "evidence",
        "attribution",
        "recorded_at_height",
        "recorded_at_view",
        "recorded_at_ms",
        "penalty_status",
    ] {
        let mut missing = json.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(
            norito::json::from_value::<EvidenceRecord>(missing).is_err(),
            "missing {field}"
        );
    }
    for field in [
        "instance",
        "height",
        "epoch",
        "context_id",
        "authority_generation",
        "offenders",
        "safety_violation",
    ] {
        let mut missing = json.clone();
        missing
            .as_object_mut()
            .unwrap()
            .get_mut("attribution")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(
            norito::json::from_value::<EvidenceRecord>(missing).is_err(),
            "missing attribution {field}"
        );
    }
}
#[test]
fn native_evidence_rejects_removed_authority_wrapper_and_malformed_frames() {
    let evidence = Evidence::from_native(&native()).unwrap();
    for value in [
        norito::json!({}),
        norito::json!({"equivocation": {}}),
        norito::json!({"native": [], "context": {}}),
    ] {
        assert!(norito::json::from_value::<Evidence>(value).is_err());
    }
    let mut suffixed = evidence.native.clone();
    suffixed.push(0);
    for frame in [
        Vec::new(),
        evidence.native[..evidence.native.len() - 1].to_vec(),
        suffixed,
        vec![0; MAX_EVIDENCE_FRAME_BYTES + 1],
    ] {
        let malformed = Evidence { native: frame };
        assert!(malformed.decode_native().is_err());
        assert!(Evidence::decode_all(&mut malformed.encode().as_slice()).is_err());
    }
}
#[test]
fn native_effects_require_both_vectors_and_reject_retired_slots() {
    let empty = NposConsensusEffects::default();
    assert!(empty.is_empty());
    let effects = NposConsensusEffects {
        evidence_admissions: vec![record().evidence],
        penalty_actions: Vec::new(),
    };
    assert!(!effects.is_empty());
    assert_eq!(
        NposConsensusEffects::decode_all(&mut effects.encode().as_slice()).unwrap(),
        effects
    );
    let json = norito::json::to_value(&effects).unwrap();
    assert_eq!(
        norito::json::from_value::<NposConsensusEffects>(json.clone()).unwrap(),
        effects
    );
    for field in ["evidence_admissions", "penalty_actions"] {
        let mut missing = json.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(
            norito::json::from_value::<NposConsensusEffects>(missing).is_err(),
            "missing {field}"
        );
    }
    for field in ["finalized_global_beacon_pulse", "v2_evidence_admissions"] {
        let mut retired = json.clone();
        retired
            .as_object_mut()
            .unwrap()
            .insert(field.into(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<NposConsensusEffects>(retired).is_err(),
            "retired {field}"
        );
    }
}
