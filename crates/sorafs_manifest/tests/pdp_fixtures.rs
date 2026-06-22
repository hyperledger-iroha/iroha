//! Round-trip coverage for committed SoraFS PDP fixtures.

#![allow(unexpected_cfgs)]

use std::fs;

use sorafs_manifest::{
    PdpChallengeV1, PdpCommitmentV1, PdpProofV1, validate_pdp_challenge_bytes,
    validate_pdp_challenge_proof_bytes, validate_pdp_commitment_bytes,
    validate_pdp_commitment_challenge_bytes, validate_pdp_commitment_challenge_proof_bytes,
    validate_pdp_proof_bytes,
};

const FIXTURES_ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/sorafs_manifest/pdp"
);

fn read_fixture_bytes(path: &str) -> Vec<u8> {
    let path = format!("{FIXTURES_ROOT}/{path}");
    fs::read(&path).unwrap_or_else(|err| panic!("failed to read {path}: {err}"))
}

fn assert_json_hex_matches(name: &str, bytes: &[u8]) {
    let path = format!("{FIXTURES_ROOT}/{name}.json");
    let json_text =
        fs::read_to_string(&path).unwrap_or_else(|err| panic!("failed to read {path}: {err}"));
    let json_value =
        norito::json::parse_value(&json_text).expect("fixture commentary must be valid JSON");
    let norito_hex = json_value
        .get("norito_bytes_hex")
        .and_then(|value| value.as_str())
        .expect("fixture commentary must contain `norito_bytes_hex` string");
    let norito_bytes =
        hex::decode(norito_hex).expect("fixture commentary must contain valid hex payload");
    assert_eq!(norito_bytes, bytes, "`norito_bytes_hex` drifted");
}

#[test]
fn pdp_commitment_fixture_decodes_validates_and_roundtrips() {
    let bytes = read_fixture_bytes("commitment_v1.to");
    let commitment: PdpCommitmentV1 =
        norito::decode_from_bytes(&bytes).expect("commitment fixture should decode");
    commitment.validate().expect("commitment must validate");
    assert_eq!(commitment.manifest_digest, [0x42; 32]);
    assert_eq!(
        norito::to_bytes(&commitment).expect("fixture should re-encode"),
        bytes
    );
    assert_json_hex_matches("commitment_v1", &bytes);

    let outcome = validate_pdp_commitment_bytes(&bytes, "commitment_v1.to", 123);
    assert!(outcome.is_ok(), "{outcome:?}");
}

#[test]
fn pdp_challenge_fixture_decodes_validates_and_roundtrips() {
    let bytes = read_fixture_bytes("challenge_v1.to");
    let challenge: PdpChallengeV1 =
        norito::decode_from_bytes(&bytes).expect("challenge fixture should decode");
    challenge.validate().expect("challenge must validate");
    assert_eq!(challenge.manifest_digest, [0x42; 32]);
    assert_eq!(challenge.provider_id, [0x10; 32]);
    assert_eq!(
        norito::to_bytes(&challenge).expect("fixture should re-encode"),
        bytes
    );
    assert_json_hex_matches("challenge_v1", &bytes);

    let outcome = validate_pdp_challenge_bytes(&bytes, "challenge_v1.to", 123);
    assert!(outcome.is_ok(), "{outcome:?}");
}

#[test]
fn pdp_proof_fixture_decodes_validates_and_roundtrips() {
    let bytes = read_fixture_bytes("proof_v1.to");
    let proof: PdpProofV1 = norito::decode_from_bytes(&bytes).expect("proof fixture should decode");
    proof.validate().expect("proof must validate");
    assert_eq!(proof.manifest_digest, [0x42; 32]);
    assert_eq!(proof.provider_id, [0x10; 32]);
    assert_eq!(proof.signature.len(), 64);
    assert_eq!(
        norito::to_bytes(&proof).expect("fixture should re-encode"),
        bytes
    );
    assert_json_hex_matches("proof_v1", &bytes);

    let outcome = validate_pdp_proof_bytes(&bytes, "proof_v1.to", 123);
    assert!(outcome.is_ok(), "{outcome:?}");
}

#[test]
fn pdp_commitment_challenge_and_challenge_proof_fixtures_cross_link() {
    let commitment_bytes = read_fixture_bytes("commitment_v1.to");
    let challenge_bytes = read_fixture_bytes("challenge_v1.to");
    let proof_bytes = read_fixture_bytes("proof_v1.to");

    let commitment_outcome = validate_pdp_commitment_challenge_bytes(
        &commitment_bytes,
        &challenge_bytes,
        "commitment_v1.to",
        "challenge_v1.to",
        123,
    );
    assert!(commitment_outcome.is_ok(), "{commitment_outcome:?}");

    let proof_outcome = validate_pdp_challenge_proof_bytes(
        &challenge_bytes,
        &proof_bytes,
        "challenge_v1.to",
        "proof_v1.to",
        123,
    );
    assert!(proof_outcome.is_ok(), "{proof_outcome:?}");

    let combined_outcome = validate_pdp_commitment_challenge_proof_bytes(
        &commitment_bytes,
        &challenge_bytes,
        &proof_bytes,
        "commitment_v1.to",
        "challenge_v1.to",
        "proof_v1.to",
        123,
    );
    assert!(combined_outcome.is_ok(), "{combined_outcome:?}");
}

#[test]
fn pdp_negative_challenge_fixture_is_rejected() {
    let bytes = read_fixture_bytes("negative/duplicate_hot_leaf_challenge_v1.to");
    let outcome = validate_pdp_challenge_bytes(&bytes, "duplicate_hot_leaf_challenge_v1.to", 123);
    assert!(!outcome.is_ok(), "{outcome:?}");
    assert_eq!(outcome.code, "SFS-PDP-001");
}

#[test]
fn pdp_negative_proof_fixture_is_rejected() {
    let bytes = read_fixture_bytes("negative/missing_signature_proof_v1.to");
    let outcome = validate_pdp_proof_bytes(&bytes, "missing_signature_proof_v1.to", 123);
    assert!(!outcome.is_ok(), "{outcome:?}");
    assert_eq!(outcome.code, "SFS-SIG-008");
}
