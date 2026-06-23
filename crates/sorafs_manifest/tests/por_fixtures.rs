#![allow(unexpected_cfgs)]

use std::fs;

use sorafs_manifest::{
    PotrReceiptV1, ProofStreamTier, RepairTaskRecordV1, RepairTaskStateV1,
    governance::{GovernanceLogNodeV1, GovernanceLogPayloadV1},
    por::{AuditOutcomeV1, AuditVerdictV1, PorChallengeV1, PorProofV1},
};

const FIXTURES_ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/sorafs_manifest"
);

fn read_fixture(path: &str) -> Vec<u8> {
    fs::read(path).unwrap_or_else(|err| panic!("failed to read {path}: {err}"))
}

#[test]
fn por_challenge_fixture_decodes_and_validates() {
    let bytes = read_fixture(&format!("{FIXTURES_ROOT}/por/challenge_v1.to"));
    let challenge: PorChallengeV1 =
        norito::decode_from_bytes(&bytes).expect("challenge fixture should decode");
    challenge.validate().expect("challenge must validate");
    assert_eq!(
        challenge.sample_indices.len(),
        usize::from(challenge.sample_count)
    );
}

#[test]
fn por_proof_fixture_decodes_and_validates() {
    let bytes = read_fixture(&format!("{FIXTURES_ROOT}/por/proof_v1.to"));
    let proof: PorProofV1 = norito::decode_from_bytes(&bytes).expect("proof fixture should decode");
    let digest = proof.proof_digest();
    proof.validate().expect("proof must validate");
    assert!(!digest.iter().all(|&b| b == 0), "digest must be non-zero");
}

#[test]
fn audit_verdict_fixture_decodes_and_validates() {
    let bytes = read_fixture(&format!("{FIXTURES_ROOT}/por/verdict_v1.to"));
    let verdict: AuditVerdictV1 =
        norito::decode_from_bytes(&bytes).expect("verdict fixture should decode");
    verdict.validate().expect("verdict must validate");
    assert_eq!(verdict.outcome, AuditOutcomeV1::Success);
}

#[test]
fn potr_receipt_fixture_decodes_and_validates() {
    let bytes = read_fixture(&format!("{FIXTURES_ROOT}/potr/receipt_v1.to"));
    let receipt: PotrReceiptV1 =
        norito::decode_from_bytes(&bytes).expect("PoTR receipt fixture should decode");
    receipt.validate().expect("PoTR receipt must validate");
    assert_eq!(receipt.tier, ProofStreamTier::Hot);
    assert_eq!(receipt.manifest_digest, [0x42; 32]);
    assert_eq!(receipt.provider_id, [0x10; 32]);
}

#[test]
fn repair_task_fixture_decodes_and_validates() {
    let bytes = read_fixture(&format!("{FIXTURES_ROOT}/repair/task_v1.to"));
    let task: RepairTaskRecordV1 =
        norito::decode_from_bytes(&bytes).expect("repair task fixture should decode");
    task.validate().expect("repair task must validate");
    assert!(matches!(task.state, RepairTaskStateV1::Queued(_)));
    assert_eq!(task.manifest_digest, [0x42; 32]);
    assert_eq!(task.provider_id, [0x10; 32]);
}

#[test]
fn governance_node_fixture_wraps_por_proof() {
    let bytes = read_fixture(&format!("{FIXTURES_ROOT}/governance/node_v1.to"));
    let node: GovernanceLogNodeV1 =
        norito::decode_from_bytes(&bytes).expect("governance node should decode");
    node.validate().expect("governance node must validate");
    node.verify_publisher_signature()
        .expect("governance node signature must verify");
    match node.payload {
        GovernanceLogPayloadV1::PorProof(ref proof) => {
            proof.validate().expect("embedded proof must validate");
        }
        other => panic!("expected PorProof payload, got {other:?}"),
    }
}
