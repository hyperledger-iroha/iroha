//! CLI coverage for the SoraFS reference validator.

use std::{fs, path::PathBuf};

use assert_cmd::cargo::cargo_bin_cmd;
use ed25519_dalek::{Signer, SigningKey};
use norito::json::Value;
use sorafs_manifest::repair::QueuedRepairStateV1;
use sorafs_manifest::{
    GOVERNANCE_DAG_BLOCK_VERSION_V1, GOVERNANCE_DAG_HEAD_VERSION_V1, GovernanceDagBlockV1,
    GovernanceDagHeadV1, GovernanceLogNodeV1, GovernanceLogSignatureV1,
    GovernanceSignatureAlgorithm, OrderRequestV1, POTR_RECEIPT_VERSION_V1, PotrReceiptV1,
    PotrStatus, ProofStreamTier, REPAIR_TASK_VERSION_V1, RepairTaskRecordV1, RepairTaskStateV1,
    RepairTicketId, SignatureAlgorithm, SignedReplicationOrderV1, governance_dag_block_cid_v1,
    verify_order_request_signature_v1,
};
use tempfile::tempdir;

fn workspace_fixture(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn potr_receipt() -> PotrReceiptV1 {
    PotrReceiptV1 {
        version: POTR_RECEIPT_VERSION_V1,
        manifest_digest: [0x11; 32],
        provider_id: [0x22; 32],
        tier: ProofStreamTier::Hot,
        deadline_ms: 90_000,
        latency_ms: 42_000,
        status: PotrStatus::Success,
        requested_at_ms: 1_700_000_000_000,
        responded_at_ms: 1_700_000_042_000,
        recorded_at_ms: 1_700_000_042_100,
        range_start: 0,
        range_end: 1_048_575,
        request_id: Some([0x44; 16]),
        trace_id: Some([0x33; 16]),
        note: Some("ok".to_owned()),
        gateway_signature: None,
        provider_signature: None,
    }
}

fn repair_task_record() -> RepairTaskRecordV1 {
    RepairTaskRecordV1 {
        version: REPAIR_TASK_VERSION_V1,
        ticket_id: RepairTicketId("REP-900".to_owned()),
        manifest_digest: [0x31; 32],
        provider_id: [0x32; 32],
        auditor_account: "auditor@sora".to_owned(),
        state: RepairTaskStateV1::Queued(QueuedRepairStateV1 {
            queued_at_unix: 1_700_000_060,
            sla_deadline_unix: Some(1_700_086_400),
        }),
        por_history_id: Some(7),
        sla_deadline_unix: Some(1_700_086_400),
        scheduler_notes: Some("waiting for worker claim".to_owned()),
        slash_proposal_digest: None,
    }
}

fn governance_node_fixture() -> GovernanceLogNodeV1 {
    let fixture = workspace_fixture("fixtures/sorafs_manifest/governance/node_v1.to");
    norito::decode_from_bytes(&fs::read(fixture).expect("read governance node fixture"))
        .expect("decode governance node fixture")
}

fn empty_governance_ed25519_signature() -> GovernanceLogSignatureV1 {
    GovernanceLogSignatureV1 {
        algorithm: GovernanceSignatureAlgorithm::Ed25519,
        public_key: Vec::new(),
        signature: Vec::new(),
    }
}

fn sign_governance_dag_block(block: &mut GovernanceDagBlockV1, seed: &[u8; 32]) {
    let signing_key = SigningKey::from_bytes(seed);
    let payload_bytes = block
        .signature_payload_bytes()
        .expect("encode governance DAG block signing payload");
    let signature = signing_key.sign(&payload_bytes);
    block.block_signature = GovernanceLogSignatureV1 {
        algorithm: GovernanceSignatureAlgorithm::Ed25519,
        public_key: signing_key.verifying_key().to_bytes().to_vec(),
        signature: signature.to_bytes().to_vec(),
    };
}

fn sign_governance_dag_head(head: &mut GovernanceDagHeadV1, seed: &[u8; 32]) {
    let signing_key = SigningKey::from_bytes(seed);
    let payload_bytes = head
        .signature_payload_bytes()
        .expect("encode governance DAG head signing payload");
    let signature = signing_key.sign(&payload_bytes);
    head.head_signature = GovernanceLogSignatureV1 {
        algorithm: GovernanceSignatureAlgorithm::Ed25519,
        public_key: signing_key.verifying_key().to_bytes().to_vec(),
        signature: signature.to_bytes().to_vec(),
    };
}

fn governance_dag_block(
    prev_block_cid: Option<Vec<u8>>,
    sequence: u64,
    timestamp: u64,
) -> GovernanceDagBlockV1 {
    let node = governance_node_fixture();
    let publisher_peer_id = b"12D3KooWGovernanceDagPublisher".to_vec();
    let block_cid = governance_dag_block_cid_v1(
        prev_block_cid.as_deref(),
        sequence,
        timestamp,
        &publisher_peer_id,
        &node,
    )
    .expect("derive governance DAG block CID");
    let mut block = GovernanceDagBlockV1 {
        version: GOVERNANCE_DAG_BLOCK_VERSION_V1,
        block_cid,
        prev_block_cid,
        sequence,
        timestamp,
        publisher_peer_id,
        node,
        block_signature: empty_governance_ed25519_signature(),
    };
    sign_governance_dag_block(&mut block, &[0xC7; 32]);
    block
}

fn governance_dag_head(blocks: &[GovernanceDagBlockV1]) -> GovernanceDagHeadV1 {
    let mut head = GovernanceDagHeadV1 {
        version: GOVERNANCE_DAG_HEAD_VERSION_V1,
        head_block_cid: blocks
            .last()
            .expect("governance DAG chain has a head")
            .block_cid
            .clone(),
        block_count: blocks.len() as u64,
        generated_at: 1_700_001_000,
        publisher_peer_id: b"12D3KooWGovernanceDagPublisher".to_vec(),
        checkpoint_cid: None,
        head_signature: empty_governance_ed25519_signature(),
    };
    sign_governance_dag_head(&mut head, &[0xD9; 32]);
    head
}

#[test]
fn sorafs_validate_advert_accepts_committed_fixture() {
    let fixture = workspace_fixture("fixtures/sorafs_manifest/provider_admission/advert_v1.to");
    let output = cargo_bin_cmd!("sorafs-validate")
        .args([
            "advert",
            "--input",
            fixture.to_str().expect("fixture path is utf-8"),
            "--now",
            "120",
            "--generated-at",
            "123",
            "--format",
            "json",
        ])
        .output()
        .expect("run sorafs-validate");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let outcome: Value = norito::json::from_slice(&output.stdout).expect("parse outcome json");
    assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Ok"));
    assert_eq!(
        outcome.get("code").and_then(Value::as_str),
        Some("SFS-OK-000")
    );
    assert_eq!(
        outcome.get("generated_at").and_then(Value::as_u64),
        Some(123)
    );
}

#[test]
fn sorafs_validate_advert_rejects_malformed_norito() {
    let temp = tempdir().expect("tempdir");
    let input = temp.path().join("bad.to");
    fs::write(&input, b"not norito").expect("write malformed payload");

    let output = cargo_bin_cmd!("sorafs-validate")
        .args([
            "advert",
            "--input",
            input.to_str().expect("temp path is utf-8"),
            "--now",
            "120",
            "--generated-at",
            "123",
            "--format",
            "json",
        ])
        .output()
        .expect("run sorafs-validate");
    assert_eq!(output.status.code(), Some(2));

    let outcome: Value = norito::json::from_slice(&output.stdout).expect("parse outcome json");
    assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Error"));
    assert_eq!(
        outcome.get("code").and_then(Value::as_str),
        Some("SFS-NORITO-001")
    );
    assert_eq!(
        outcome.get("category").and_then(Value::as_str),
        Some("norito")
    );
}

#[test]
fn sorafs_validate_admission_accepts_committed_fixture() {
    let fixture = workspace_fixture("fixtures/sorafs_manifest/provider_admission/envelope_v1.to");
    let output = cargo_bin_cmd!("sorafs-validate")
        .args([
            "admission",
            "--input",
            fixture.to_str().expect("fixture path is utf-8"),
            "--generated-at",
            "123",
            "--format",
            "json",
        ])
        .output()
        .expect("run sorafs-validate");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let outcome: Value = norito::json::from_slice(&output.stdout).expect("parse outcome json");
    assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Ok"));
    assert_eq!(
        outcome.get("code").and_then(Value::as_str),
        Some("SFS-OK-000")
    );
    assert_eq!(
        outcome.get("generated_at").and_then(Value::as_u64),
        Some(123)
    );
}

#[test]
fn sorafs_validate_admission_accepts_committed_renewal_fixture() {
    let envelope = workspace_fixture("fixtures/sorafs_manifest/provider_admission/envelope_v1.to");
    let renewal = workspace_fixture("fixtures/sorafs_manifest/provider_admission/renewal_v1.to");
    let output = cargo_bin_cmd!("sorafs-validate")
        .args([
            "admission",
            "--envelope",
            envelope.to_str().expect("fixture path is utf-8"),
            "--renewal",
            renewal.to_str().expect("fixture path is utf-8"),
            "--generated-at",
            "123",
            "--format",
            "json",
        ])
        .output()
        .expect("run sorafs-validate");
    assert!(
        output.status.success(),
        "stderr: {}\nstdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );

    let outcome: Value = norito::json::from_slice(&output.stdout).expect("parse outcome json");
    assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Ok"));
    assert_eq!(
        outcome.get("code").and_then(Value::as_str),
        Some("SFS-OK-000")
    );
    assert_eq!(
        outcome.get("generated_at").and_then(Value::as_u64),
        Some(123)
    );
}

#[test]
fn sorafs_validate_admission_accepts_committed_revocation_fixture() {
    let envelope = workspace_fixture("fixtures/sorafs_manifest/provider_admission/envelope_v1.to");
    let revocation =
        workspace_fixture("fixtures/sorafs_manifest/provider_admission/revocation_v1.to");
    let output = cargo_bin_cmd!("sorafs-validate")
        .args([
            "admission",
            "--envelope",
            envelope.to_str().expect("fixture path is utf-8"),
            "--revocation",
            revocation.to_str().expect("fixture path is utf-8"),
            "--generated-at",
            "123",
            "--format",
            "json",
        ])
        .output()
        .expect("run sorafs-validate");
    assert!(
        output.status.success(),
        "stderr: {}\nstdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );

    let outcome: Value = norito::json::from_slice(&output.stdout).expect("parse outcome json");
    assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Ok"));
    assert_eq!(
        outcome.get("code").and_then(Value::as_str),
        Some("SFS-OK-000")
    );
    assert_eq!(
        outcome.get("generated_at").and_then(Value::as_u64),
        Some(123)
    );
}

#[test]
fn sorafs_validate_admission_rejects_malformed_norito() {
    let temp = tempdir().expect("tempdir");
    let input = temp.path().join("bad-envelope.to");
    fs::write(&input, b"not norito").expect("write malformed payload");

    let output = cargo_bin_cmd!("sorafs-validate")
        .args([
            "admission",
            "--input",
            input.to_str().expect("temp path is utf-8"),
            "--generated-at",
            "123",
            "--format",
            "json",
        ])
        .output()
        .expect("run sorafs-validate");
    assert_eq!(output.status.code(), Some(2));

    let outcome: Value = norito::json::from_slice(&output.stdout).expect("parse outcome json");
    assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Error"));
    assert_eq!(
        outcome.get("code").and_then(Value::as_str),
        Some("SFS-NORITO-001")
    );
    assert_eq!(
        outcome.get("category").and_then(Value::as_str),
        Some("norito")
    );
}

#[test]
fn sorafs_validate_order_accepts_committed_fixture() {
    let fixture = workspace_fixture("fixtures/sorafs_manifest/replication_order/order_v1.to");
    let output = cargo_bin_cmd!("sorafs-validate")
        .args([
            "order",
            "--order",
            fixture.to_str().expect("fixture path is utf-8"),
            "--generated-at",
            "123",
            "--format",
            "json",
        ])
        .output()
        .expect("run sorafs-validate");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let outcome: Value = norito::json::from_slice(&output.stdout).expect("parse outcome json");
    assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Ok"));
    assert_eq!(
        outcome.get("code").and_then(Value::as_str),
        Some("SFS-OK-000")
    );
    assert_eq!(
        outcome.get("generated_at").and_then(Value::as_u64),
        Some(123)
    );
}

#[test]
fn sorafs_validate_order_rejects_malformed_norito() {
    let temp = tempdir().expect("tempdir");
    let input = temp.path().join("bad-order.to");
    fs::write(&input, b"not norito").expect("write malformed payload");

    let output = cargo_bin_cmd!("sorafs-validate")
        .args([
            "order",
            "--order",
            input.to_str().expect("temp path is utf-8"),
            "--generated-at",
            "123",
            "--format",
            "json",
        ])
        .output()
        .expect("run sorafs-validate");
    assert_eq!(output.status.code(), Some(2));

    let outcome: Value = norito::json::from_slice(&output.stdout).expect("parse outcome json");
    assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Error"));
    assert_eq!(
        outcome.get("code").and_then(Value::as_str),
        Some("SFS-NORITO-001")
    );
    assert_eq!(
        outcome.get("category").and_then(Value::as_str),
        Some("norito")
    );
}

#[test]
fn sorafs_validate_orderbook_accepts_committed_receipt_fixture() {
    let fixture = workspace_fixture("fixtures/sorafs_manifest/orderbook/settlement_receipt_v1.to");
    let output = cargo_bin_cmd!("sorafs-validate")
        .args([
            "orderbook",
            "--receipt",
            fixture.to_str().expect("fixture path is utf-8"),
            "--generated-at",
            "123",
            "--format",
            "json",
        ])
        .output()
        .expect("run sorafs-validate");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let outcome: Value = norito::json::from_slice(&output.stdout).expect("parse outcome json");
    assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Ok"));
    assert_eq!(
        outcome.get("code").and_then(Value::as_str),
        Some("SFS-OK-000")
    );
    assert_eq!(
        outcome.get("generated_at").and_then(Value::as_u64),
        Some(123)
    );
}

#[test]
fn sorafs_validate_sign_orderbook_writes_verified_order_payload() {
    let temp = tempdir().expect("tempdir");
    let fixture = workspace_fixture("fixtures/sorafs_manifest/orderbook/order_request_v1.to");
    let output_path = temp.path().join("signed-orderbook-order.to");
    let seed_hex = "b7".repeat(32);
    let expected_key = SigningKey::from_bytes(&[0xB7; 32])
        .verifying_key()
        .to_bytes()
        .to_vec();

    let output = cargo_bin_cmd!("sorafs-validate")
        .args([
            "sign",
            "--kind",
            "orderbook",
            "--payload-kind",
            "order-request",
            "--input",
            fixture.to_str().expect("fixture path is utf-8"),
            "--out",
            output_path.to_str().expect("output path is utf-8"),
            "--key-hex",
            &seed_hex,
            "--generated-at",
            "123",
            "--format",
            "json",
        ])
        .output()
        .expect("run sorafs-validate sign orderbook");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let outcome: Value = norito::json::from_slice(&output.stdout).expect("parse outcome json");
    assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Ok"));
    assert!(
        outcome
            .get("telemetry_tags")
            .and_then(Value::as_array)
            .is_some_and(|tags| tags
                .iter()
                .any(|tag| { tag.as_str() == Some("sorafs.reference.sign.orderbook") }))
    );
    assert!(
        outcome
            .get("context")
            .and_then(Value::as_array)
            .is_some_and(|fields| fields.iter().any(|field| {
                field.get("key").and_then(Value::as_str) == Some("payload_kind")
                    && field.get("value").and_then(Value::as_str) == Some("order-request")
            }))
    );

    let signed_bytes = fs::read(output_path).expect("read signed orderbook order");
    let signed_order: OrderRequestV1 =
        norito::decode_from_bytes(&signed_bytes).expect("decode signed orderbook order");
    assert_eq!(signed_order.signature.public_key, expected_key);
    verify_order_request_signature_v1(&signed_order).expect("signed order verifies");
}

#[test]
fn sorafs_validate_pdp_accepts_committed_fixtures() {
    let commitment = workspace_fixture("fixtures/sorafs_manifest/pdp/commitment_v1.to");
    let challenge = workspace_fixture("fixtures/sorafs_manifest/pdp/challenge_v1.to");
    let proof = workspace_fixture("fixtures/sorafs_manifest/pdp/proof_v1.to");
    let output = cargo_bin_cmd!("sorafs-validate")
        .args([
            "pdp",
            "--commitment",
            commitment.to_str().expect("fixture path is utf-8"),
            "--challenge",
            challenge.to_str().expect("fixture path is utf-8"),
            "--proof",
            proof.to_str().expect("fixture path is utf-8"),
            "--generated-at",
            "123",
            "--format",
            "json",
        ])
        .output()
        .expect("run sorafs-validate");
    assert!(
        output.status.success(),
        "stderr: {}\nstdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );

    let outcome: Value = norito::json::from_slice(&output.stdout).expect("parse outcome json");
    assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Ok"));
    assert_eq!(
        outcome.get("code").and_then(Value::as_str),
        Some("SFS-OK-000")
    );
    let inputs = outcome
        .get("inputs")
        .and_then(Value::as_array)
        .expect("PDP outcome should include inputs");
    assert!(
        inputs
            .iter()
            .any(|input| input.get("kind").and_then(Value::as_str) == Some("pdp_commitment")),
        "{outcome:?}"
    );
}

#[test]
fn sorafs_validate_pdp_rejects_negative_proof_fixture() {
    let challenge = workspace_fixture("fixtures/sorafs_manifest/pdp/challenge_v1.to");
    let proof =
        workspace_fixture("fixtures/sorafs_manifest/pdp/negative/missing_signature_proof_v1.to");
    let output = cargo_bin_cmd!("sorafs-validate")
        .args([
            "pdp",
            "--challenge",
            challenge.to_str().expect("fixture path is utf-8"),
            "--proof",
            proof.to_str().expect("fixture path is utf-8"),
            "--generated-at",
            "123",
            "--format",
            "json",
        ])
        .output()
        .expect("run sorafs-validate");
    assert_eq!(output.status.code(), Some(2));

    let outcome: Value = norito::json::from_slice(&output.stdout).expect("parse outcome json");
    assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Error"));
    assert_eq!(
        outcome.get("code").and_then(Value::as_str),
        Some("SFS-SIG-008")
    );
    assert_eq!(
        outcome.get("category").and_then(Value::as_str),
        Some("signature")
    );
}

#[test]
fn sorafs_validate_por_accepts_committed_fixtures() {
    let challenge = workspace_fixture("fixtures/sorafs_manifest/por/challenge_v1.to");
    let proof = workspace_fixture("fixtures/sorafs_manifest/por/proof_v1.to");
    let output = cargo_bin_cmd!("sorafs-validate")
        .args([
            "por",
            "--challenge",
            challenge.to_str().expect("fixture path is utf-8"),
            "--proof",
            proof.to_str().expect("fixture path is utf-8"),
            "--generated-at",
            "123",
            "--format",
            "json",
        ])
        .output()
        .expect("run sorafs-validate");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let outcome: Value = norito::json::from_slice(&output.stdout).expect("parse outcome json");
    assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Ok"));
    assert_eq!(
        outcome.get("code").and_then(Value::as_str),
        Some("SFS-OK-000")
    );
    assert_eq!(
        outcome.get("generated_at").and_then(Value::as_u64),
        Some(123)
    );
}

#[test]
fn sorafs_validate_por_rejects_malformed_challenge() {
    let temp = tempdir().expect("tempdir");
    let challenge = temp.path().join("bad-challenge.to");
    fs::write(&challenge, b"not norito").expect("write malformed payload");
    let proof = workspace_fixture("fixtures/sorafs_manifest/por/proof_v1.to");

    let output = cargo_bin_cmd!("sorafs-validate")
        .args([
            "por",
            "--challenge",
            challenge.to_str().expect("temp path is utf-8"),
            "--proof",
            proof.to_str().expect("fixture path is utf-8"),
            "--generated-at",
            "123",
            "--format",
            "json",
        ])
        .output()
        .expect("run sorafs-validate");
    assert_eq!(output.status.code(), Some(2));

    let outcome: Value = norito::json::from_slice(&output.stdout).expect("parse outcome json");
    assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Error"));
    assert_eq!(
        outcome.get("code").and_then(Value::as_str),
        Some("SFS-NORITO-001")
    );
    assert_eq!(
        outcome.get("category").and_then(Value::as_str),
        Some("norito")
    );
}

#[test]
fn sorafs_validate_potr_accepts_generated_receipt() {
    let temp = tempdir().expect("tempdir");
    let input = temp.path().join("receipt.to");
    fs::write(
        &input,
        norito::to_bytes(&potr_receipt()).expect("encode receipt"),
    )
    .expect("write receipt");

    let output = cargo_bin_cmd!("sorafs-validate")
        .args([
            "potr",
            "--receipt",
            input.to_str().expect("temp path is utf-8"),
            "--profile",
            "hot",
            "--generated-at",
            "123",
            "--format",
            "json",
        ])
        .output()
        .expect("run sorafs-validate");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let outcome: Value = norito::json::from_slice(&output.stdout).expect("parse outcome json");
    assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Ok"));
    assert_eq!(
        outcome.get("code").and_then(Value::as_str),
        Some("SFS-OK-000")
    );
    assert_eq!(
        outcome.get("generated_at").and_then(Value::as_u64),
        Some(123)
    );
}

#[test]
fn sorafs_validate_potr_accepts_committed_fixture() {
    let fixture = workspace_fixture("fixtures/sorafs_manifest/potr/receipt_v1.to");
    let output = cargo_bin_cmd!("sorafs-validate")
        .args([
            "potr",
            "--receipt",
            fixture.to_str().expect("fixture path is utf-8"),
            "--profile",
            "hot",
            "--generated-at",
            "123",
            "--format",
            "json",
        ])
        .output()
        .expect("run sorafs-validate");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let outcome: Value = norito::json::from_slice(&output.stdout).expect("parse outcome json");
    assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Ok"));
    assert_eq!(
        outcome.get("code").and_then(Value::as_str),
        Some("SFS-OK-000")
    );
    assert_eq!(
        outcome.get("generated_at").and_then(Value::as_u64),
        Some(123)
    );
}

#[test]
fn sorafs_validate_potr_rejects_profile_mismatch() {
    let temp = tempdir().expect("tempdir");
    let input = temp.path().join("receipt.to");
    fs::write(
        &input,
        norito::to_bytes(&potr_receipt()).expect("encode receipt"),
    )
    .expect("write receipt");

    let output = cargo_bin_cmd!("sorafs-validate")
        .args([
            "potr",
            "--receipt",
            input.to_str().expect("temp path is utf-8"),
            "--profile",
            "warm",
            "--generated-at",
            "123",
            "--format",
            "json",
        ])
        .output()
        .expect("run sorafs-validate");
    assert_eq!(output.status.code(), Some(2));

    let outcome: Value = norito::json::from_slice(&output.stdout).expect("parse outcome json");
    assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Error"));
    assert_eq!(
        outcome.get("code").and_then(Value::as_str),
        Some("SFS-POTR-002")
    );
    assert_eq!(
        outcome.get("category").and_then(Value::as_str),
        Some("validation")
    );
}

#[test]
fn sorafs_validate_repair_accepts_generated_task_record() {
    let temp = tempdir().expect("tempdir");
    let input = temp.path().join("repair-task.to");
    fs::write(
        &input,
        norito::to_bytes(&repair_task_record()).expect("encode repair task"),
    )
    .expect("write repair task");

    let output = cargo_bin_cmd!("sorafs-validate")
        .args([
            "repair",
            "--task",
            input.to_str().expect("temp path is utf-8"),
            "--generated-at",
            "123",
            "--format",
            "json",
        ])
        .output()
        .expect("run sorafs-validate");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let outcome: Value = norito::json::from_slice(&output.stdout).expect("parse outcome json");
    assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Ok"));
    assert_eq!(
        outcome.get("code").and_then(Value::as_str),
        Some("SFS-OK-000")
    );
    assert_eq!(
        outcome.get("generated_at").and_then(Value::as_u64),
        Some(123)
    );
}

#[test]
fn sorafs_validate_repair_accepts_committed_task_fixture() {
    let fixture = workspace_fixture("fixtures/sorafs_manifest/repair/task_v1.to");
    let output = cargo_bin_cmd!("sorafs-validate")
        .args([
            "repair",
            "--task",
            fixture.to_str().expect("fixture path is utf-8"),
            "--generated-at",
            "123",
            "--format",
            "json",
        ])
        .output()
        .expect("run sorafs-validate");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let outcome: Value = norito::json::from_slice(&output.stdout).expect("parse outcome json");
    assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Ok"));
    assert_eq!(
        outcome.get("code").and_then(Value::as_str),
        Some("SFS-OK-000")
    );
    assert_eq!(
        outcome.get("generated_at").and_then(Value::as_u64),
        Some(123)
    );
}

#[test]
fn sorafs_validate_repair_rejects_malformed_norito() {
    let temp = tempdir().expect("tempdir");
    let input = temp.path().join("bad-repair-task.to");
    fs::write(&input, b"not norito").expect("write malformed payload");

    let output = cargo_bin_cmd!("sorafs-validate")
        .args([
            "repair",
            "--kind",
            "task",
            "--input",
            input.to_str().expect("temp path is utf-8"),
            "--generated-at",
            "123",
            "--format",
            "json",
        ])
        .output()
        .expect("run sorafs-validate");
    assert_eq!(output.status.code(), Some(2));

    let outcome: Value = norito::json::from_slice(&output.stdout).expect("parse outcome json");
    assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Error"));
    assert_eq!(
        outcome.get("code").and_then(Value::as_str),
        Some("SFS-NORITO-001")
    );
    assert_eq!(
        outcome.get("category").and_then(Value::as_str),
        Some("norito")
    );
}

#[test]
fn sorafs_validate_bundle_accepts_committed_fixture_root() {
    let bundle = workspace_fixture("fixtures/sorafs_manifest");
    let output = cargo_bin_cmd!("sorafs-validate")
        .args([
            "bundle",
            "--bundle",
            bundle.to_str().expect("fixture path is utf-8"),
            "--now",
            "120",
            "--generated-at",
            "123",
            "--format",
            "json",
        ])
        .output()
        .expect("run sorafs-validate");
    assert!(
        output.status.success(),
        "stderr: {}\nstdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );

    let outcome: Value = norito::json::from_slice(&output.stdout).expect("parse outcome json");
    assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Ok"));
    assert_eq!(
        outcome.get("code").and_then(Value::as_str),
        Some("SFS-OK-000")
    );
    assert_eq!(
        outcome.get("generated_at").and_then(Value::as_u64),
        Some(123)
    );
    let inputs = outcome
        .get("inputs")
        .and_then(Value::as_array)
        .expect("bundle outcome should include inputs");
    assert!(
        inputs.iter().any(|input| {
            input.get("kind").and_then(Value::as_str) == Some("orderbook_order_request")
                && input
                    .get("path")
                    .and_then(Value::as_str)
                    .is_some_and(|path| path.ends_with("orderbook/order_request_v1.to"))
        }),
        "{outcome:?}"
    );
    assert!(
        inputs.iter().any(|input| {
            input.get("kind").and_then(Value::as_str) == Some("settlement_receipt")
                && input
                    .get("path")
                    .and_then(Value::as_str)
                    .is_some_and(|path| path.ends_with("orderbook/settlement_receipt_v1.to"))
        }),
        "{outcome:?}"
    );
    assert!(
        inputs.iter().any(|input| {
            input.get("kind").and_then(Value::as_str) == Some("orderbook_runtime_snapshot")
                && input
                    .get("path")
                    .and_then(Value::as_str)
                    .is_some_and(|path| path.ends_with("orderbook/runtime_snapshot_v1.to"))
        }),
        "{outcome:?}"
    );
    assert!(
        inputs.iter().any(|input| {
            input.get("kind").and_then(Value::as_str) == Some("pdp_commitment")
                && input
                    .get("path")
                    .and_then(Value::as_str)
                    .is_some_and(|path| path.ends_with("pdp/commitment_v1.to"))
        }),
        "{outcome:?}"
    );
    assert!(
        inputs.iter().any(|input| {
            input.get("kind").and_then(Value::as_str) == Some("pdp_proof")
                && input
                    .get("path")
                    .and_then(Value::as_str)
                    .is_some_and(|path| path.ends_with("pdp/proof_v1.to"))
        }),
        "{outcome:?}"
    );
}

#[test]
fn sorafs_validate_bundle_rejects_manifest_mismatch() {
    let temp = tempdir().expect("tempdir");
    let order_dir = temp.path().join("replication_order");
    let receipt_dir = temp.path().join("potr");
    fs::create_dir_all(&order_dir).expect("create order dir");
    fs::create_dir_all(&receipt_dir).expect("create receipt dir");
    fs::copy(
        workspace_fixture("fixtures/sorafs_manifest/replication_order/order_v1.to"),
        order_dir.join("order_v1.to"),
    )
    .expect("copy order fixture");
    let mut receipt = potr_receipt();
    receipt.manifest_digest = [0x99; 32];
    receipt.provider_id = [0x10; 32];
    fs::write(
        receipt_dir.join("receipt_v1.to"),
        norito::to_bytes(&receipt).expect("encode receipt"),
    )
    .expect("write receipt");

    let output = cargo_bin_cmd!("sorafs-validate")
        .args([
            "bundle",
            "--bundle",
            temp.path().to_str().expect("temp path is utf-8"),
            "--now",
            "120",
            "--generated-at",
            "123",
            "--format",
            "json",
        ])
        .output()
        .expect("run sorafs-validate");
    assert_eq!(output.status.code(), Some(2));

    let outcome: Value = norito::json::from_slice(&output.stdout).expect("parse outcome json");
    assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Error"));
    assert_eq!(
        outcome.get("code").and_then(Value::as_str),
        Some("SFS-BND-002")
    );
    assert_eq!(
        outcome.get("category").and_then(Value::as_str),
        Some("validation")
    );
}

#[test]
fn sorafs_validate_governance_accepts_committed_fixture() {
    let fixture = workspace_fixture("fixtures/sorafs_manifest/governance/node_v1.to");
    let output = cargo_bin_cmd!("sorafs-validate")
        .args([
            "governance",
            "--node",
            fixture.to_str().expect("fixture path is utf-8"),
            "--cid",
            "bafygovernancelognode",
            "--generated-at",
            "123",
            "--format",
            "json",
        ])
        .output()
        .expect("run sorafs-validate");
    assert!(
        output.status.success(),
        "stderr: {}\nstdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );

    let outcome: Value = norito::json::from_slice(&output.stdout).expect("parse outcome json");
    assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Ok"));
    assert_eq!(
        outcome.get("code").and_then(Value::as_str),
        Some("SFS-OK-000")
    );
    assert_eq!(
        outcome.get("generated_at").and_then(Value::as_u64),
        Some(123)
    );
}

#[test]
fn sorafs_validate_governance_rejects_cid_mismatch() {
    let fixture = workspace_fixture("fixtures/sorafs_manifest/governance/node_v1.to");
    let output = cargo_bin_cmd!("sorafs-validate")
        .args([
            "governance",
            "--node",
            fixture.to_str().expect("fixture path is utf-8"),
            "--cid",
            "bafywronggovernancenode",
            "--generated-at",
            "123",
            "--format",
            "json",
        ])
        .output()
        .expect("run sorafs-validate");
    assert_eq!(output.status.code(), Some(2));

    let outcome: Value = norito::json::from_slice(&output.stdout).expect("parse outcome json");
    assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Error"));
    assert_eq!(
        outcome.get("code").and_then(Value::as_str),
        Some("SFS-GOV-003")
    );
    assert_eq!(
        outcome.get("category").and_then(Value::as_str),
        Some("validation")
    );
}

#[test]
fn sorafs_validate_governance_dag_block_accepts_signed_block() {
    let temp = tempdir().expect("tempdir");
    let block = governance_dag_block(None, 0, 1_700_000_800);
    let block_path = temp.path().join("governance-block.to");
    fs::write(
        &block_path,
        norito::to_bytes(&block).expect("encode governance DAG block"),
    )
    .expect("write governance DAG block");
    let expected_cid = format!("hex:{}", hex::encode(&block.block_cid));

    let output = cargo_bin_cmd!("sorafs-validate")
        .args([
            "governance",
            "--block",
            block_path.to_str().expect("block path is utf-8"),
            "--cid",
            &expected_cid,
            "--generated-at",
            "123",
            "--format",
            "json",
        ])
        .output()
        .expect("run sorafs-validate governance block");
    assert!(
        output.status.success(),
        "stderr: {}\nstdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );

    let outcome: Value = norito::json::from_slice(&output.stdout).expect("parse outcome json");
    assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Ok"));
    assert_eq!(
        outcome.get("code").and_then(Value::as_str),
        Some("SFS-OK-000")
    );
}

#[test]
fn sorafs_validate_governance_dag_block_rejects_cid_mismatch() {
    let temp = tempdir().expect("tempdir");
    let block = governance_dag_block(None, 0, 1_700_000_800);
    let block_path = temp.path().join("governance-block.to");
    fs::write(
        &block_path,
        norito::to_bytes(&block).expect("encode governance DAG block"),
    )
    .expect("write governance DAG block");

    let output = cargo_bin_cmd!("sorafs-validate")
        .args([
            "governance",
            "--block",
            block_path.to_str().expect("block path is utf-8"),
            "--cid",
            "hex:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            "--generated-at",
            "123",
            "--format",
            "json",
        ])
        .output()
        .expect("run sorafs-validate governance block");
    assert_eq!(output.status.code(), Some(2));

    let outcome: Value = norito::json::from_slice(&output.stdout).expect("parse outcome json");
    assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Error"));
    assert_eq!(
        outcome.get("code").and_then(Value::as_str),
        Some("SFS-GOV-004")
    );
    assert_eq!(
        outcome.get("category").and_then(Value::as_str),
        Some("validation")
    );
}

#[test]
fn sorafs_validate_governance_dag_head_accepts_signed_chain() {
    let temp = tempdir().expect("tempdir");
    let first = governance_dag_block(None, 0, 1_700_000_800);
    let second = governance_dag_block(Some(first.block_cid.clone()), 1, 1_700_000_860);
    let blocks = vec![first, second];
    let head = governance_dag_head(&blocks);
    let head_path = temp.path().join("governance-head.to");
    let block_0_path = temp.path().join("governance-block-0.to");
    let block_1_path = temp.path().join("governance-block-1.to");
    fs::write(
        &head_path,
        norito::to_bytes(&head).expect("encode governance DAG head"),
    )
    .expect("write governance DAG head");
    fs::write(
        &block_0_path,
        norito::to_bytes(&blocks[0]).expect("encode governance DAG block"),
    )
    .expect("write governance DAG block 0");
    fs::write(
        &block_1_path,
        norito::to_bytes(&blocks[1]).expect("encode governance DAG block"),
    )
    .expect("write governance DAG block 1");

    let output = cargo_bin_cmd!("sorafs-validate")
        .args([
            "governance",
            "--head",
            head_path.to_str().expect("head path is utf-8"),
            "--block",
            block_0_path.to_str().expect("block 0 path is utf-8"),
            "--block",
            block_1_path.to_str().expect("block 1 path is utf-8"),
            "--generated-at",
            "123",
            "--format",
            "json",
        ])
        .output()
        .expect("run sorafs-validate governance head");
    assert!(
        output.status.success(),
        "stderr: {}\nstdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );

    let outcome: Value = norito::json::from_slice(&output.stdout).expect("parse outcome json");
    assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Ok"));
    assert_eq!(
        outcome.get("code").and_then(Value::as_str),
        Some("SFS-OK-000")
    );
}

#[test]
fn sorafs_validate_sign_advert_writes_valid_signed_norito() {
    let fixture = workspace_fixture("fixtures/sorafs_manifest/provider_admission/advert_v1.to");
    let temp = tempdir().expect("tempdir");
    let output_path = temp.path().join("signed-advert.to");
    let key_hex = "a5".repeat(32);

    let output = cargo_bin_cmd!("sorafs-validate")
        .args([
            "sign",
            "--kind",
            "advert",
            "--input",
            fixture.to_str().expect("fixture path is utf-8"),
            "--out",
            output_path.to_str().expect("output path is utf-8"),
            "--key-hex",
            &key_hex,
            "--now",
            "120",
            "--generated-at",
            "123",
            "--format",
            "json",
        ])
        .output()
        .expect("run sorafs-validate sign");
    assert!(
        output.status.success(),
        "stderr: {}\nstdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(output_path.is_file());

    let outcome: Value = norito::json::from_slice(&output.stdout).expect("parse outcome json");
    assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Ok"));
    assert_eq!(
        outcome.get("code").and_then(Value::as_str),
        Some("SFS-OK-000")
    );

    let validate_output = cargo_bin_cmd!("sorafs-validate")
        .args([
            "advert",
            "--input",
            output_path.to_str().expect("output path is utf-8"),
            "--now",
            "120",
            "--generated-at",
            "124",
            "--format",
            "json",
        ])
        .output()
        .expect("validate signed advert");
    assert!(
        validate_output.status.success(),
        "stderr: {}\nstdout: {}",
        String::from_utf8_lossy(&validate_output.stderr),
        String::from_utf8_lossy(&validate_output.stdout)
    );
    let validate_outcome: Value =
        norito::json::from_slice(&validate_output.stdout).expect("parse validate outcome json");
    assert_eq!(
        validate_outcome.get("status").and_then(Value::as_str),
        Some("Ok")
    );
}

#[test]
fn sorafs_validate_sign_order_writes_valid_signed_norito() {
    let fixture = workspace_fixture("fixtures/sorafs_manifest/replication_order/order_v1.to");
    let temp = tempdir().expect("tempdir");
    let output_path = temp.path().join("signed-order.to");
    let key_hex = "a7".repeat(32);

    let output = cargo_bin_cmd!("sorafs-validate")
        .args([
            "sign",
            "--kind",
            "order",
            "--input",
            fixture.to_str().expect("fixture path is utf-8"),
            "--out",
            output_path.to_str().expect("output path is utf-8"),
            "--key-hex",
            &key_hex,
            "--generated-at",
            "123",
            "--format",
            "json",
        ])
        .output()
        .expect("run sorafs-validate sign order");
    assert!(
        output.status.success(),
        "stderr: {}\nstdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(output_path.is_file());

    let outcome: Value = norito::json::from_slice(&output.stdout).expect("parse outcome json");
    assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Ok"));
    assert_eq!(
        outcome.get("code").and_then(Value::as_str),
        Some("SFS-OK-000")
    );

    let signed_order: SignedReplicationOrderV1 =
        norito::decode_from_bytes(&fs::read(&output_path).expect("read signed order"))
            .expect("decode signed order");
    assert_eq!(
        signed_order.signature.algorithm,
        SignatureAlgorithm::Ed25519
    );
    signed_order
        .verify_signature()
        .expect("signed replication order verifies");

    let validate_output = cargo_bin_cmd!("sorafs-validate")
        .args([
            "order",
            "--signed-order",
            output_path.to_str().expect("output path is utf-8"),
            "--generated-at",
            "124",
            "--format",
            "json",
        ])
        .output()
        .expect("validate signed order");
    assert!(
        validate_output.status.success(),
        "stderr: {}\nstdout: {}",
        String::from_utf8_lossy(&validate_output.stderr),
        String::from_utf8_lossy(&validate_output.stdout)
    );
    let validate_outcome: Value =
        norito::json::from_slice(&validate_output.stdout).expect("parse validate outcome json");
    assert_eq!(
        validate_outcome.get("status").and_then(Value::as_str),
        Some("Ok")
    );
}

#[test]
fn sorafs_validate_sign_governance_writes_valid_signed_norito() {
    let fixture = workspace_fixture("fixtures/sorafs_manifest/governance/node_v1.to");
    let temp = tempdir().expect("tempdir");
    let output_path = temp.path().join("signed-governance-node.to");
    let key_hex = "a6".repeat(32);

    let output = cargo_bin_cmd!("sorafs-validate")
        .args([
            "sign",
            "--kind",
            "governance",
            "--input",
            fixture.to_str().expect("fixture path is utf-8"),
            "--out",
            output_path.to_str().expect("output path is utf-8"),
            "--key-hex",
            &key_hex,
            "--generated-at",
            "123",
            "--format",
            "json",
        ])
        .output()
        .expect("run sorafs-validate sign governance");
    assert!(
        output.status.success(),
        "stderr: {}\nstdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(output_path.is_file());

    let outcome: Value = norito::json::from_slice(&output.stdout).expect("parse outcome json");
    assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Ok"));
    assert_eq!(
        outcome.get("code").and_then(Value::as_str),
        Some("SFS-OK-000")
    );

    let signed_node: GovernanceLogNodeV1 =
        norito::decode_from_bytes(&fs::read(&output_path).expect("read signed governance node"))
            .expect("decode signed governance node");
    assert_eq!(
        signed_node.publisher_signature.algorithm,
        GovernanceSignatureAlgorithm::Ed25519
    );
    signed_node
        .verify_publisher_signature()
        .expect("signed governance node verifies");

    let validate_output = cargo_bin_cmd!("sorafs-validate")
        .args([
            "governance",
            "--node",
            output_path.to_str().expect("output path is utf-8"),
            "--cid",
            "bafygovernancelognode",
            "--generated-at",
            "124",
            "--format",
            "json",
        ])
        .output()
        .expect("validate signed governance node");
    assert!(
        validate_output.status.success(),
        "stderr: {}\nstdout: {}",
        String::from_utf8_lossy(&validate_output.stderr),
        String::from_utf8_lossy(&validate_output.stdout)
    );
    let validate_outcome: Value =
        norito::json::from_slice(&validate_output.stdout).expect("parse validate outcome json");
    assert_eq!(
        validate_outcome.get("status").and_then(Value::as_str),
        Some("Ok")
    );
}
