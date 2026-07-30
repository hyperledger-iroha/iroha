//! Norito binary and JSON roundtrips for policy-jury ZK ballot types.

use iroha_data_model::ministry::{
    POLICY_JURY_BALLOT_REVEAL_VERSION_V1, PolicyJuryBallotMode, PolicyJuryBallotRevealV1,
    PolicyJuryVoteChoice, PolicyJuryZkEnvelope,
};

fn sample_envelope() -> PolicyJuryZkEnvelope {
    PolicyJuryZkEnvelope {
        proof_uri: "sorafs://proofs/pj-2026-02/juror-1".to_owned(),
        attachments: vec![
            "sorafs://proofs/pj-2026-02/juror-1/transcript".to_owned(),
            "sorafs://proofs/pj-2026-02/juror-1/public-inputs".to_owned(),
        ],
    }
}

fn sample_reveal() -> PolicyJuryBallotRevealV1 {
    PolicyJuryBallotRevealV1 {
        version: POLICY_JURY_BALLOT_REVEAL_VERSION_V1,
        proposal_id: "AC-2026-042".to_owned(),
        round_id: "PJ-2026-02".to_owned(),
        juror_id: "juror#1".to_owned(),
        choice: PolicyJuryVoteChoice::Approve,
        nonce: vec![0x42; 32],
        revealed_at_unix_ms: 1_738_000_000_000,
        zk_proof_uris: vec!["sorafs://proofs/pj-2026-02/juror-1".to_owned()],
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

#[test]
fn policy_jury_reveal_with_zk_proof_uris_norito_roundtrip() {
    let original = sample_reveal();
    let encoded = norito::to_bytes(&original).expect("encode policy-jury ballot reveal");
    let decoded: PolicyJuryBallotRevealV1 =
        norito::decode_from_bytes(&encoded).expect("decode policy-jury ballot reveal");

    assert_eq!(decoded, original);
}

#[cfg(feature = "json")]
#[test]
fn policy_jury_reveal_json_contains_canonical_zk_proof_uris_field() {
    let original = sample_reveal();
    let encoded = norito::json::to_json(&original).expect("encode policy-jury ballot reveal JSON");
    let value: norito::json::Value =
        norito::json::from_str(&encoded).expect("parse policy-jury ballot reveal JSON");
    let proof_uris = value
        .get("zk_proof_uris")
        .and_then(norito::json::Value::as_array)
        .expect("canonical reveal JSON must contain zk_proof_uris");
    assert_eq!(proof_uris.len(), 1);
    assert_eq!(
        proof_uris[0].as_str(),
        Some("sorafs://proofs/pj-2026-02/juror-1")
    );

    let decoded: PolicyJuryBallotRevealV1 =
        norito::json::from_str(&encoded).expect("decode policy-jury ballot reveal JSON");
    assert_eq!(decoded, original);
}
