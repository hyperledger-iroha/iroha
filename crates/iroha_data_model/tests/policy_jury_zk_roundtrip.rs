//! Norito binary and JSON roundtrips for policy-jury ZK ballot types.

#![cfg(feature = "zk-ballot")]

use iroha_data_model::ministry::{PolicyJuryBallotMode, PolicyJuryZkEnvelope};

fn sample_envelope() -> PolicyJuryZkEnvelope {
    PolicyJuryZkEnvelope {
        proof_uri: "sorafs://proofs/pj-2026-02/juror-1".to_owned(),
        attachments: vec![
            "sorafs://proofs/pj-2026-02/juror-1/transcript".to_owned(),
            "sorafs://proofs/pj-2026-02/juror-1/public-inputs".to_owned(),
        ],
    }
}

#[test]
fn policy_jury_zk_envelope_norito_roundtrip() {
    let original = sample_envelope();
    let encoded = norito::to_bytes(&original).expect("encode policy-jury ZK envelope");
    let decoded: PolicyJuryZkEnvelope =
        norito::decode_from_bytes(&encoded).expect("decode policy-jury ZK envelope");

    assert_eq!(decoded, original);
}

#[cfg(feature = "json")]
#[test]
fn policy_jury_zk_envelope_json_roundtrip() {
    let original = sample_envelope();
    let encoded = norito::json::to_json(&original).expect("encode policy-jury ZK envelope JSON");
    let decoded: PolicyJuryZkEnvelope =
        norito::json::from_str(&encoded).expect("decode policy-jury ZK envelope JSON");

    assert_eq!(decoded, original);
}

#[test]
fn policy_jury_zk_ballot_mode_tuple_variant_norito_roundtrip() {
    let original = PolicyJuryBallotMode::ZkEnvelope(sample_envelope());
    let encoded = norito::to_bytes(&original).expect("encode policy-jury ZK ballot mode");
    let decoded: PolicyJuryBallotMode =
        norito::decode_from_bytes(&encoded).expect("decode policy-jury ZK ballot mode");

    assert_eq!(decoded, original);
}

#[cfg(feature = "json")]
#[test]
fn policy_jury_zk_ballot_mode_tuple_variant_json_roundtrip() {
    let original = PolicyJuryBallotMode::ZkEnvelope(sample_envelope());
    let encoded = norito::json::to_json(&original).expect("encode policy-jury ZK ballot mode JSON");
    let decoded: PolicyJuryBallotMode =
        norito::json::from_str(&encoded).expect("decode policy-jury ZK ballot mode JSON");

    assert_eq!(decoded, original);
}
