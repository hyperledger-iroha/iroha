#![allow(unexpected_cfgs)]

use std::{fs, path::Path};

use assert_cmd::cargo::cargo_bin_cmd;
use sorafs_manifest::{
    PotrReceiptV1, ProofStreamTier, RepairTaskRecordV1, RepairTaskStateV1,
    governance::{
        GovernanceLogNodeV1, GovernanceLogPayloadV1, SoraFsModerationBallotGovernanceEventKindV1,
    },
    por::{AuditOutcomeV1, AuditVerdictV1, PorChallengeV1, PorProofV1},
};
use tempfile::tempdir;

const FIXTURES_ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/sorafs_manifest"
);

fn read_fixture(path: &str) -> Vec<u8> {
    fs::read(path).unwrap_or_else(|err| panic!("failed to read {path}: {err}"))
}

fn regenerate_fixtures(root: &Path) {
    for directory in [
        "appeal_finance",
        "orderbook",
        "pdp",
        "provider_admission",
        "replication_order",
    ] {
        copy_fixture_tree(
            Path::new(FIXTURES_ROOT).join(directory).as_path(),
            root.join("fixtures/sorafs_manifest")
                .join(directory)
                .as_path(),
        );
    }
    let output = cargo_bin_cmd!("generate_por_fixtures")
        .current_dir(root)
        .output()
        .expect("run deterministic SoraFS fixture generator");
    assert!(
        output.status.success(),
        "fixture generator failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

fn copy_fixture_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap_or_else(|error| {
        panic!(
            "create fixture seed directory `{}`: {error}",
            destination.display()
        )
    });
    for entry in fs::read_dir(source)
        .unwrap_or_else(|error| panic!("read fixture seed `{}`: {error}", source.display()))
    {
        let entry = entry.expect("read fixture seed entry");
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if entry
            .file_type()
            .expect("read fixture seed entry type")
            .is_dir()
        {
            copy_fixture_tree(&source_path, &destination_path);
        } else {
            fs::copy(&source_path, &destination_path).unwrap_or_else(|error| {
                panic!(
                    "copy fixture seed `{}` to `{}`: {error}",
                    source_path.display(),
                    destination_path.display()
                )
            });
        }
    }
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
    proof
        .verify_signature()
        .expect("provider proof signature must verify");
    assert!(!digest.iter().all(|&b| b == 0), "digest must be non-zero");
}

#[test]
fn audit_verdict_fixture_decodes_and_validates() {
    let bytes = read_fixture(&format!("{FIXTURES_ROOT}/por/verdict_v1.to"));
    let verdict: AuditVerdictV1 =
        norito::decode_from_bytes(&bytes).expect("verdict fixture should decode");
    verdict.validate().expect("verdict must validate");
    verdict
        .verify_signatures()
        .expect("auditor verdict signature must verify");
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
fn repair_link_negative_fixtures_are_structurally_valid() {
    let manifest_bytes = read_fixture(&format!(
        "{FIXTURES_ROOT}/repair/negative/task_manifest_mismatch_v1.to"
    ));
    let manifest_task: RepairTaskRecordV1 = norito::decode_from_bytes(&manifest_bytes)
        .expect("manifest-mismatch repair task should decode");
    manifest_task
        .validate()
        .expect("manifest-mismatch repair task must remain structurally valid");
    assert_eq!(manifest_task.manifest_digest, [0x99; 32]);
    assert_eq!(manifest_task.provider_id, [0x10; 32]);

    let provider_bytes = read_fixture(&format!(
        "{FIXTURES_ROOT}/repair/negative/task_provider_unassigned_v1.to"
    ));
    let provider_task: RepairTaskRecordV1 = norito::decode_from_bytes(&provider_bytes)
        .expect("unassigned-provider repair task should decode");
    provider_task
        .validate()
        .expect("unassigned-provider repair task must remain structurally valid");
    assert_eq!(provider_task.manifest_digest, [0x42; 32]);
    assert_eq!(provider_task.provider_id, [0x99; 32]);
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
            proof
                .verify_signature()
                .expect("embedded proof signature must verify");
        }
        other => panic!("expected PorProof payload, got {other:?}"),
    }
}

#[test]
fn moderation_governance_node_fixture_is_typed_and_signed() {
    let bytes = read_fixture(&format!("{FIXTURES_ROOT}/moderation/governance_node_v1.to"));
    let node: GovernanceLogNodeV1 =
        norito::decode_from_bytes(&bytes).expect("moderation governance node should decode");
    node.validate()
        .expect("moderation governance node must validate");
    node.verify_publisher_signature()
        .expect("moderation governance node signature must verify");
    match node.payload {
        GovernanceLogPayloadV1::ModerationBallotEvent(ref event) => {
            event
                .validate()
                .expect("typed moderation event must validate");
            assert_eq!(
                event.kind,
                SoraFsModerationBallotGovernanceEventKindV1::BallotTallied
            );
            assert_eq!(event.case_id, "case-reference-sdk-v1");
            assert_eq!(event.round_id, "round-reference-sdk-v1");
            assert_eq!(event.committed_count, 3);
            assert_eq!(event.revealed_count, 3);
        }
        other => panic!("expected ModerationBallotEvent payload, got {other:?}"),
    }
}

#[test]
fn governance_sdk_fixture_regeneration_is_byte_identical() {
    const INVENTORIED_FILES: [&str; 26] = [
        "dag_block_0_v1.json",
        "dag_block_0_v1.to",
        "dag_block_1_bad_predecessor_v1.json",
        "dag_block_1_bad_predecessor_v1.to",
        "dag_block_1_v1.json",
        "dag_block_1_v1.to",
        "dag_block_bad_signature_v1.json",
        "dag_block_bad_signature_v1.to",
        "dag_block_trailing_bytes_v1.to",
        "dag_head_bad_predecessor_v1.json",
        "dag_head_bad_predecessor_v1.to",
        "dag_head_bad_signature_v1.json",
        "dag_head_bad_signature_v1.to",
        "dag_head_v1.json",
        "dag_head_v1.to",
        "node_v1.json",
        "node_v1.to",
        "dag_block_bad_signature_validation_outcome_v1.json",
        "dag_block_cid_mismatch_validation_outcome_v1.json",
        "dag_block_trailing_bytes_validation_outcome_v1.json",
        "dag_block_validation_outcome_v1.json",
        "dag_head_bad_predecessor_validation_outcome_v1.json",
        "dag_head_bad_signature_validation_outcome_v1.json",
        "dag_head_reordered_validation_outcome_v1.json",
        "dag_head_validation_outcome_v1.json",
        "sdk_validation_inventory_v1.json",
    ];

    let first = tempdir().expect("create first fixture generation directory");
    let second = tempdir().expect("create second fixture generation directory");
    regenerate_fixtures(first.path());
    regenerate_fixtures(second.path());

    for name in INVENTORIED_FILES {
        let relative = Path::new("fixtures/sorafs_manifest/governance").join(name);
        let first_bytes = fs::read(first.path().join(&relative))
            .unwrap_or_else(|error| panic!("read first regenerated `{name}`: {error}"));
        let second_bytes = fs::read(second.path().join(&relative))
            .unwrap_or_else(|error| panic!("read second regenerated `{name}`: {error}"));
        let checked_in = read_fixture(&format!("{FIXTURES_ROOT}/governance/{name}"));

        assert_eq!(
            first_bytes, second_bytes,
            "two regenerations diverged for `{name}`"
        );
        assert_eq!(
            first_bytes, checked_in,
            "regenerated bytes differ from checked-in `{name}`"
        );
    }
}

#[test]
fn release_wide_reference_sdk_fixture_regeneration_is_byte_identical() {
    const INVENTORIED_FILES: [&str; 25] = [
        "appeal_finance/cancel_asset_lock_v1.json",
        "appeal_finance/cancel_asset_lock_v1.to",
        "appeal_finance/negative/cancel_asset_lock_legacy_missing_expected_v1.json",
        "appeal_finance/negative/cancel_asset_lock_legacy_missing_expected_v1.to",
        "appeal_finance/negative/cancel_asset_lock_nested_escrow_id_v1.to",
        "appeal_finance/negative/cancel_asset_lock_noncanonical_quantity_v1.json",
        "appeal_finance/negative/cancel_asset_lock_zero_expected_v1.json",
        "appeal_finance/negative/cancel_asset_lock_zero_expected_v1.to",
        "moderation/governance_node_v1.json",
        "moderation/governance_node_v1.to",
        "moderation/governance_node_validation_outcome_v1.json",
        "repair/negative/task_manifest_mismatch_v1.json",
        "repair/negative/task_manifest_mismatch_v1.to",
        "repair/negative/task_provider_unassigned_v1.json",
        "repair/negative/task_provider_unassigned_v1.to",
        "reference_sdk/bundle_heterogeneous_positive_validation_outcome_v1.json",
        "reference_sdk/bundle_orderbook_bad_signature_negative_validation_outcome_v1.json",
        "reference_sdk/bundle_orderbook_trailing_bytes_negative_validation_outcome_v1.json",
        "reference_sdk/bundle_pdp_duplicate_hot_leaf_negative_validation_outcome_v1.json",
        "reference_sdk/bundle_pdp_missing_signature_negative_validation_outcome_v1.json",
        "reference_sdk/bundle_pdp_wrong_provider_negative_validation_outcome_v1.json",
        "reference_sdk/bundle_repair_manifest_mismatch_negative_validation_outcome_v1.json",
        "reference_sdk/bundle_repair_provider_unassigned_negative_validation_outcome_v1.json",
        "reference_sdk/bundle_routing_admission_positive_validation_outcome_v1.json",
        "reference_sdk_validation_inventory_v1.json",
    ];

    let first = tempdir().expect("create first release-wide generation directory");
    let second = tempdir().expect("create second release-wide generation directory");
    regenerate_fixtures(first.path());
    regenerate_fixtures(second.path());

    for name in INVENTORIED_FILES {
        let relative = Path::new("fixtures/sorafs_manifest").join(name);
        let first_bytes = fs::read(first.path().join(&relative))
            .unwrap_or_else(|error| panic!("read first regenerated `{name}`: {error}"));
        let second_bytes = fs::read(second.path().join(&relative))
            .unwrap_or_else(|error| panic!("read second regenerated `{name}`: {error}"));
        let checked_in = read_fixture(&format!("{FIXTURES_ROOT}/{name}"));

        assert_eq!(
            first_bytes, second_bytes,
            "two release-wide regenerations diverged for `{name}`"
        );
        assert_eq!(
            first_bytes, checked_in,
            "release-wide regenerated bytes differ from checked-in `{name}`"
        );
    }
}
