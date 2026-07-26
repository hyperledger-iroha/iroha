//! Generates PoR, PoTR, repair, and governance DAG fixtures.

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use ed25519_dalek::{Signer as _, SigningKey};
use hex::encode;
use iroha_crypto::{Algorithm, KeyPair, Signature, sha256};
use norito::{
    core::NoritoSerialize,
    json::{Map, Value, parse_value, to_string, to_string_pretty},
};
use sorafs_manifest::{
    CapacityMetadataEntry, FixtureBundlePayloadKindV1, FixtureBundlePayloadV1,
    GOVERNANCE_DAG_BLOCK_VERSION_V1, GOVERNANCE_DAG_HEAD_VERSION_V1, GovernanceDagBlockV1,
    GovernanceDagHeadV1, POTR_RECEIPT_VERSION_V1, PotrReceiptV1, PotrSignatureAlgorithm,
    PotrSignatureV1, PotrStatus, ProofStreamTier, REPAIR_TASK_VERSION_V1, RepairTaskRecordV1,
    RepairTaskStateV1, RepairTicketId,
    governance::{
        GOVERNANCE_LOG_VERSION_V1, GovernanceLogNodeV1, GovernanceLogPayloadV1,
        GovernanceLogSignatureV1, GovernanceSignatureAlgorithm,
        SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1,
        SoraFsModerationBallotGovernanceEventKindV1, SoraFsModerationBallotGovernanceEventV1,
        SoraFsModerationBallotGovernanceTallyV1, SoraFsModerationVoteChoiceV1,
        SoraFsModerationVoteCountsV1,
    },
    governance_dag_block_cid_v1, governance_log_node_cid_v1,
    por::{
        AUDIT_VERDICT_VERSION_V1, AuditOutcomeV1, AuditVerdictV1, POR_CHALLENGE_VERSION_V1,
        POR_PROOF_VERSION_V1, PorChallengeV1, PorProofSampleV1, PorProofV1, derive_challenge_id,
        derive_challenge_seed,
    },
    provider_advert::{AdvertSignature, SignatureAlgorithm},
    repair::QueuedRepairStateV1,
    validate_fixture_bundle_payloads, validate_governance_dag_head_chain_bytes,
    validate_governance_log_node_bytes,
};
use soranet_pq::{HedgedRngSeed, MlDsaSuite, deterministic_chacha20_rng, sign_mldsa};

const GOVERNANCE_FIXTURE_SIGNING_SEED: [u8; 32] = [0xC7; 32];
const GOVERNANCE_SDK_INVENTORY_SCHEMA: &str =
    "sorafs.reference_sdk.governance_fixture_inventory.v1";
const GOVERNANCE_SDK_INVENTORY_SCOPE: &str = "governance_sdk_subset";
const GOVERNANCE_FIXTURE_PUBLIC_KEY_HEX: &str =
    "d5af25e204ad03d0a26e236996404f1be51a60948bcc026cd084a83690b756d3";
const GOVERNANCE_FIXTURE_PUBLIC_KEY_FINGERPRINT_SHA256: &str =
    "1a09a6a1b85cec77787ba6ce26f18500a2434865cee04d79c69a481888f52fff";
const REFERENCE_SDK_INVENTORY_SCHEMA: &str = "sorafs.reference_sdk.validation_fixture_inventory.v1";
const REFERENCE_SDK_INVENTORY_SCOPE: &str = "sorafs_v1_release";

#[derive(Clone, norito::JsonSerialize)]
struct GovernanceSdkPayloadInventoryEntryV1 {
    path: String,
    kind: String,
    encoding: String,
    signature_expectation: String,
    byte_length: u64,
    sha256: String,
}

#[derive(Clone, norito::JsonSerialize)]
struct GovernanceSdkOutcomeInventoryEntryV1 {
    path: String,
    scenario: String,
    status: String,
    code: String,
    byte_length: u64,
    sha256: String,
}

#[derive(Clone, norito::JsonSerialize)]
struct GovernanceSdkUnsignedInventoryV1 {
    schema: String,
    scope: String,
    signing_domain: String,
    payloads: Vec<GovernanceSdkPayloadInventoryEntryV1>,
    outcomes: Vec<GovernanceSdkOutcomeInventoryEntryV1>,
}

#[derive(norito::JsonSerialize)]
struct GovernanceSdkInventorySignatureV1 {
    algorithm: String,
    key_usage: String,
    public_key_hex: String,
    public_key_fingerprint_sha256: String,
    signature_hex: String,
}

#[derive(norito::JsonSerialize)]
struct GovernanceSdkFixtureInventoryV1 {
    schema: String,
    scope: String,
    signing_domain: String,
    payloads: Vec<GovernanceSdkPayloadInventoryEntryV1>,
    outcomes: Vec<GovernanceSdkOutcomeInventoryEntryV1>,
    signature: GovernanceSdkInventorySignatureV1,
}

#[derive(Clone, norito::JsonSerialize)]
struct ReferenceSdkPayloadInventoryEntryV1 {
    path: String,
    domain: String,
    kind: String,
    encoding: String,
    expectation: String,
    byte_length: u64,
    sha256: String,
}

#[derive(Clone, norito::JsonSerialize)]
struct ReferenceSdkOutcomeInventoryEntryV1 {
    path: String,
    domain: String,
    scenario: String,
    status: String,
    code: String,
    byte_length: u64,
    sha256: String,
}

#[derive(Clone, norito::JsonSerialize)]
struct ReferenceSdkUnsignedInventoryV1 {
    schema: String,
    scope: String,
    signing_domain: String,
    payloads: Vec<ReferenceSdkPayloadInventoryEntryV1>,
    outcomes: Vec<ReferenceSdkOutcomeInventoryEntryV1>,
}

#[derive(norito::JsonSerialize)]
struct ReferenceSdkFixtureInventoryV1 {
    schema: String,
    scope: String,
    signing_domain: String,
    payloads: Vec<ReferenceSdkPayloadInventoryEntryV1>,
    outcomes: Vec<ReferenceSdkOutcomeInventoryEntryV1>,
    signature: GovernanceSdkInventorySignatureV1,
}

fn main() -> Result<(), Box<dyn Error>> {
    let fixtures_root = PathBuf::from("fixtures/sorafs_manifest");
    let por_dir = fixtures_root.join("por");
    let potr_dir = fixtures_root.join("potr");
    let repair_dir = fixtures_root.join("repair");
    let gov_dir = fixtures_root.join("governance");
    let moderation_dir = fixtures_root.join("moderation");
    fs::create_dir_all(&por_dir)?;
    fs::create_dir_all(&potr_dir)?;
    fs::create_dir_all(&repair_dir)?;
    fs::create_dir_all(repair_dir.join("negative"))?;
    fs::create_dir_all(&gov_dir)?;
    fs::create_dir_all(&moderation_dir)?;

    let manifest_digest = [0x42; 32];
    let provider_id = [0x10; 32];
    let epoch_id = 1_700_000;
    let drand_round = 5_432_101;
    let drand_randomness = [0x21; 32];
    let vrf_output = [0x23; 32];
    let seed = derive_challenge_seed(
        &drand_randomness,
        Some(&vrf_output),
        &manifest_digest,
        epoch_id,
    );
    let challenge_id =
        derive_challenge_id(&seed, &manifest_digest, &provider_id, epoch_id, drand_round);

    let challenge = PorChallengeV1 {
        version: POR_CHALLENGE_VERSION_V1,
        challenge_id,
        manifest_digest,
        provider_id,
        epoch_id,
        drand_round,
        drand_randomness,
        drand_signature: [0x24; 48],
        vrf_output: Some(vrf_output),
        vrf_proof: Some(iroha_crypto::vrf::VrfProof::SigInG1([0x25; 48])),
        forced: false,
        chunking_profile: "sorafs.sf1@1.0.0".to_string(),
        seed,
        sample_tier: 2,
        sample_count: 3,
        sample_indices: vec![1, 2_048, 65_535],
        issued_at: 1_700_000_000,
        deadline_at: 1_700_000_900,
    };
    challenge.validate()?;

    let proof_samples = vec![
        PorProofSampleV1 {
            sample_index: 1,
            chunk_offset: 0,
            chunk_size: 65_536,
            chunk_digest: [0xAA; 32],
            leaf_digest: [0xBB; 32],
        },
        PorProofSampleV1 {
            sample_index: 2_048,
            chunk_offset: 134_217_728,
            chunk_size: 65_536,
            chunk_digest: [0xAC; 32],
            leaf_digest: [0xBC; 32],
        },
        PorProofSampleV1 {
            sample_index: 65_535,
            chunk_offset: 4_294_836_224,
            chunk_size: 65_536,
            chunk_digest: [0xAD; 32],
            leaf_digest: [0xBD; 32],
        },
    ];

    let provider_signing_key = SigningKey::from_bytes(&[0x31; 32]);
    let mut proof = PorProofV1 {
        version: POR_PROOF_VERSION_V1,
        challenge_id: challenge.challenge_id,
        manifest_digest: challenge.manifest_digest,
        provider_id: challenge.provider_id,
        samples: proof_samples,
        auth_path: vec![[0x11; 32], [0x22; 32], [0x33; 32]],
        signature: AdvertSignature {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: provider_signing_key.verifying_key().to_bytes().to_vec(),
            signature: vec![0; 64],
        },
        submitted_at: 1_700_000_540,
    };
    let proof_payload = proof.signature_payload_bytes()?;
    proof.signature.signature = provider_signing_key
        .sign(&proof_payload)
        .to_bytes()
        .to_vec();
    proof.validate()?;
    proof.verify_signature()?;
    let proof_digest = proof.proof_digest();

    let auditor_signing_key = SigningKey::from_bytes(&[0x32; 32]);
    let mut verdict = AuditVerdictV1 {
        version: AUDIT_VERDICT_VERSION_V1,
        manifest_digest: challenge.manifest_digest,
        provider_id: challenge.provider_id,
        challenge_id: challenge.challenge_id,
        proof_digest: Some(proof_digest),
        outcome: AuditOutcomeV1::Success,
        failure_reason: None,
        decided_at: 1_700_000_600,
        auditor_signatures: vec![AdvertSignature {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: auditor_signing_key.verifying_key().to_bytes().to_vec(),
            signature: vec![0; 64],
        }],
        metadata: vec![CapacityMetadataEntry {
            key: "auditor.note".to_string(),
            value: "PoR verified successfully".to_string(),
        }],
    };
    let verdict_payload = verdict.signature_payload_bytes()?;
    verdict.auditor_signatures[0].signature = auditor_signing_key
        .sign(&verdict_payload)
        .to_bytes()
        .to_vec();
    verdict.validate()?;
    verdict.verify_signatures()?;

    write_norito_pair(
        &por_dir.join("challenge_v1"),
        &challenge,
        challenge_json(&challenge),
    )?;
    write_norito_pair(
        &por_dir.join("proof_v1"),
        &proof,
        proof_json(&proof, proof_digest),
    )?;
    write_norito_pair(
        &por_dir.join("verdict_v1"),
        &verdict,
        verdict_json(&verdict),
    )?;

    let potr_receipt = PotrReceiptV1 {
        version: POTR_RECEIPT_VERSION_V1,
        manifest_digest,
        provider_id,
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
        note: Some("fixture retrieval completed".to_string()),
        gateway_signature: None,
        provider_signature: None,
    };
    let gateway_key = KeyPair::try_from_seed(vec![0x11; 32], Algorithm::Ed25519)?;
    let provider_key = KeyPair::try_from_seed(vec![0x31; 32], Algorithm::MlDsa)?;
    let potr_receipt = sign_potr_receipt_fixture_v1(potr_receipt, &gateway_key, &provider_key)?;
    write_norito_pair(
        &potr_dir.join("receipt_v1"),
        &potr_receipt,
        potr_receipt_json(&potr_receipt),
    )?;

    let repair_task = RepairTaskRecordV1 {
        version: REPAIR_TASK_VERSION_V1,
        ticket_id: RepairTicketId("REP-900".to_owned()),
        manifest_digest,
        provider_id,
        auditor_account: "auditor@sora".to_owned(),
        state: RepairTaskStateV1::Queued(QueuedRepairStateV1 {
            queued_at_unix: 1_700_000_060,
            sla_deadline_unix: Some(1_700_086_400),
        }),
        por_history_id: Some(drand_round),
        sla_deadline_unix: Some(1_700_086_400),
        scheduler_notes: Some("waiting for worker claim".to_owned()),
        slash_proposal_digest: None,
    };
    repair_task.validate()?;
    write_norito_pair(
        &repair_dir.join("task_v1"),
        &repair_task,
        repair_task_json(&repair_task),
    )?;
    let mut manifest_mismatch_task = repair_task.clone();
    manifest_mismatch_task.manifest_digest = [0x99; 32];
    manifest_mismatch_task.validate()?;
    write_norito_pair(
        &repair_dir.join("negative/task_manifest_mismatch_v1"),
        &manifest_mismatch_task,
        repair_task_json(&manifest_mismatch_task),
    )?;
    let mut provider_unassigned_task = repair_task.clone();
    provider_unassigned_task.provider_id = [0x99; 32];
    provider_unassigned_task.validate()?;
    write_norito_pair(
        &repair_dir.join("negative/task_provider_unassigned_v1"),
        &provider_unassigned_task,
        repair_task_json(&provider_unassigned_task),
    )?;

    // Governance node sample (wrap proof).
    let mut node = GovernanceLogNodeV1 {
        version: GOVERNANCE_LOG_VERSION_V1,
        node_cid: Vec::new(),
        prev_cid: Some([0xA4; 32].to_vec()),
        timestamp: 1_700_000_700,
        publisher_peer_id: b"12D3KooWGovernancePublisher".to_vec(),
        payload: GovernanceLogPayloadV1::PorProof(proof.clone()),
        publisher_signature: GovernanceLogSignatureV1 {
            algorithm: GovernanceSignatureAlgorithm::Dilithium3,
            public_key: Vec::new(),
            signature: Vec::new(),
        },
    };
    node.node_cid = governance_log_node_cid_v1(
        node.prev_cid.as_deref(),
        node.timestamp,
        &node.publisher_peer_id,
        &node.payload,
    )?;
    sign_governance_log_node_mldsa(&mut node, b"sorafs-fixture-governance-mldsa-v1")?;
    node.validate()?;
    node.verify_publisher_signature()?;

    write_norito_pair(
        &gov_dir.join("node_v1"),
        &node,
        governance_node_json(&node, proof_digest),
    )?;

    let moderation_event = SoraFsModerationBallotGovernanceEventV1 {
        version: SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1,
        sequence: 6,
        kind: SoraFsModerationBallotGovernanceEventKindV1::BallotTallied,
        generated_at_unix_ms: 1_700_000_750_000,
        case_id: "case-reference-sdk-v1".to_owned(),
        round_id: "round-reference-sdk-v1".to_owned(),
        juror_id: None,
        committed_count: 3,
        revealed_count: 3,
        challenge_count: 0,
        tally: Some(SoraFsModerationBallotGovernanceTallyV1 {
            case_id: "case-reference-sdk-v1".to_owned(),
            round_id: "round-reference-sdk-v1".to_owned(),
            counts: SoraFsModerationVoteCountsV1 {
                uphold: 2,
                overturn: 1,
                modify: 0,
                escalate: 0,
            },
            votes_total: 3,
            quorum: 2,
            winning_choice: Some(SoraFsModerationVoteChoiceV1::Uphold),
            contested: false,
            tallied_at_unix_ms: 1_700_000_750_000,
        }),
        challenge: None,
    };
    let moderation_node = signed_governance_node(
        GovernanceLogPayloadV1::ModerationBallotEvent(moderation_event),
        Some(node.node_cid.clone()),
        1_700_000_750,
        b"12D3KooWModerationFixturePublisher",
    )?;
    write_norito_pair(
        &moderation_dir.join("governance_node_v1"),
        &moderation_node,
        moderation_governance_node_json(&moderation_node)?,
    )?;
    let moderation_node_bytes = norito::to_bytes(&moderation_node)?;
    let moderation_outcome = validate_governance_log_node_bytes(
        &moderation_node_bytes,
        "moderation/governance_node_v1.to",
        Some(moderation_node.node_cid.as_slice()),
        1_700_001_234,
    );
    write_expected_success_validation_outcome(
        &moderation_dir.join("governance_node_validation_outcome_v1.json"),
        &moderation_outcome,
    )?;

    let first_dag_node = governance_dag_node(proof.clone(), None, 1_700_000_790)?;
    let second_dag_node =
        governance_dag_node(proof, Some(first_dag_node.node_cid.clone()), 1_700_000_850)?;
    let first_block = governance_dag_block(first_dag_node, None, 0, 1_700_000_800)?;
    let second_block = governance_dag_block(
        second_dag_node.clone(),
        Some(first_block.block_cid.clone()),
        1,
        1_700_000_860,
    )?;
    let blocks = [first_block, second_block];
    let head = governance_dag_head(&blocks)?;
    for (index, block) in blocks.iter().enumerate() {
        write_norito_pair(
            &gov_dir.join(format!("dag_block_{index}_v1")),
            block,
            governance_dag_block_json(block),
        )?;
    }
    write_norito_pair(
        &gov_dir.join("dag_head_v1"),
        &head,
        governance_dag_head_json(&head),
    )?;
    let head_bytes = norito::to_bytes(&head)?;
    let block_bytes = blocks
        .iter()
        .map(norito::to_bytes)
        .collect::<Result<Vec<_>, _>>()?;
    let block_inputs = block_bytes
        .iter()
        .enumerate()
        .map(|(index, bytes)| (bytes.as_slice(), format!("dag_block_{index}_v1.to")))
        .collect::<Vec<_>>();
    let outcome = sorafs_manifest::validate_governance_dag_block_bytes(
        &block_bytes[0],
        "dag_block_0_v1.to",
        None,
        123,
    );
    write_expected_success_validation_outcome(
        &gov_dir.join("dag_block_validation_outcome_v1.json"),
        &outcome,
    )?;

    let expected_mismatch_cid = [0x7F; 32];
    let outcome = sorafs_manifest::validate_governance_dag_block_bytes(
        &block_bytes[0],
        "governance-dag-block.to",
        Some(&expected_mismatch_cid),
        123,
    );
    write_expected_validation_outcome(
        &gov_dir.join("dag_block_cid_mismatch_validation_outcome_v1.json"),
        &outcome,
        "SFS-GOV-004",
    )?;

    let outcome =
        validate_governance_dag_head_chain_bytes(&head_bytes, "dag_head_v1.to", &block_inputs, 123);
    if !outcome.is_ok() {
        return Err(
            format!("generated governance DAG fixture failed validation: {outcome:?}").into(),
        );
    }
    fs::write(
        gov_dir.join("dag_head_validation_outcome_v1.json"),
        format!("{}\n", to_string_pretty(&outcome)?),
    )?;

    let mut bad_block_signature = blocks[0].clone();
    *bad_block_signature
        .block_signature
        .signature
        .first_mut()
        .ok_or("governance DAG fixture block signature must not be empty")? ^= 1;
    write_norito_pair(
        &gov_dir.join("dag_block_bad_signature_v1"),
        &bad_block_signature,
        governance_dag_block_json(&bad_block_signature),
    )?;
    let bad_block_signature_bytes = norito::to_bytes(&bad_block_signature)?;
    let outcome = sorafs_manifest::validate_governance_dag_block_bytes(
        &bad_block_signature_bytes,
        "dag_block_bad_signature_v1.to",
        None,
        123,
    );
    write_expected_validation_outcome(
        &gov_dir.join("dag_block_bad_signature_validation_outcome_v1.json"),
        &outcome,
        "SFS-SIG-006",
    )?;

    let mut bad_head_signature = head.clone();
    *bad_head_signature
        .head_signature
        .signature
        .first_mut()
        .ok_or("governance DAG fixture head signature must not be empty")? ^= 1;
    write_norito_pair(
        &gov_dir.join("dag_head_bad_signature_v1"),
        &bad_head_signature,
        governance_dag_head_json(&bad_head_signature),
    )?;
    let bad_head_signature_bytes = norito::to_bytes(&bad_head_signature)?;
    let outcome = validate_governance_dag_head_chain_bytes(
        &bad_head_signature_bytes,
        "dag_head_bad_signature_v1.to",
        &block_inputs,
        123,
    );
    write_expected_validation_outcome(
        &gov_dir.join("dag_head_bad_signature_validation_outcome_v1.json"),
        &outcome,
        "SFS-SIG-007",
    )?;

    let bad_predecessor_block =
        governance_dag_block(second_dag_node, Some(vec![0xDD; 32]), 1, 1_700_000_860)?;
    let bad_predecessor_blocks = [blocks[0].clone(), bad_predecessor_block.clone()];
    let bad_predecessor_head = governance_dag_head(&bad_predecessor_blocks)?;
    write_norito_pair(
        &gov_dir.join("dag_block_1_bad_predecessor_v1"),
        &bad_predecessor_block,
        governance_dag_block_json(&bad_predecessor_block),
    )?;
    write_norito_pair(
        &gov_dir.join("dag_head_bad_predecessor_v1"),
        &bad_predecessor_head,
        governance_dag_head_json(&bad_predecessor_head),
    )?;
    let bad_predecessor_head_bytes = norito::to_bytes(&bad_predecessor_head)?;
    let bad_predecessor_block_bytes = norito::to_bytes(&bad_predecessor_block)?;
    let bad_predecessor_inputs = [
        (block_bytes[0].as_slice(), "dag_block_0_v1.to".to_owned()),
        (
            bad_predecessor_block_bytes.as_slice(),
            "dag_block_1_bad_predecessor_v1.to".to_owned(),
        ),
    ];
    let outcome = validate_governance_dag_head_chain_bytes(
        &bad_predecessor_head_bytes,
        "dag_head_bad_predecessor_v1.to",
        &bad_predecessor_inputs,
        123,
    );
    write_expected_validation_outcome(
        &gov_dir.join("dag_head_bad_predecessor_validation_outcome_v1.json"),
        &outcome,
        "SFS-GOV-006",
    )?;

    let mut trailing_block_bytes = block_bytes[0].clone();
    trailing_block_bytes.push(0);
    fs::write(
        gov_dir.join("dag_block_trailing_bytes_v1.to"),
        &trailing_block_bytes,
    )?;
    let outcome = sorafs_manifest::validate_governance_dag_block_bytes(
        &trailing_block_bytes,
        "dag_block_trailing_bytes_v1.to",
        None,
        123,
    );
    write_expected_validation_outcome(
        &gov_dir.join("dag_block_trailing_bytes_validation_outcome_v1.json"),
        &outcome,
        "SFS-NORITO-001",
    )?;

    let reordered_inputs = [
        (
            block_bytes[1].as_slice(),
            "governance-dag-block-0.to".to_owned(),
        ),
        (
            block_bytes[0].as_slice(),
            "governance-dag-block-1.to".to_owned(),
        ),
    ];
    let outcome = validate_governance_dag_head_chain_bytes(
        &head_bytes,
        "governance-dag-head.to",
        &reordered_inputs,
        123,
    );
    write_expected_validation_outcome(
        &gov_dir.join("dag_head_reordered_validation_outcome_v1.json"),
        &outcome,
        "SFS-GOV-006",
    )?;

    write_governance_sdk_fixture_inventory(&gov_dir)?;
    write_reference_sdk_bundle_outcomes(&fixtures_root)?;
    write_reference_sdk_fixture_inventory(&fixtures_root)?;

    Ok(())
}

fn sign_governance_log_node_mldsa(
    node: &mut GovernanceLogNodeV1,
    seed: &[u8],
) -> Result<(), Box<dyn Error>> {
    let seed = blake3::hash(seed);
    let key_pair = soranet_pq::generate_mldsa_keypair_from_seed(
        MlDsaSuite::MlDsa65,
        HedgedRngSeed::from_entropy(*seed.as_bytes()),
        b"sorafs-fixture-governance-mldsa-keypair-v1",
    )?;
    let payload_bytes = node.signature_payload_bytes()?;
    let mut signing_rng = deterministic_chacha20_rng(
        HedgedRngSeed::from_entropy(
            *blake3::hash(b"sorafs-fixture-governance-mldsa-sign-v1").as_bytes(),
        ),
        b"sorafs-fixture-governance-mldsa-sign-v1",
    );
    let signature = sign_mldsa(
        MlDsaSuite::MlDsa65,
        key_pair.secret_key(),
        &[],
        &payload_bytes,
        &mut signing_rng,
    )?;
    node.publisher_signature = GovernanceLogSignatureV1 {
        algorithm: GovernanceSignatureAlgorithm::Dilithium3,
        public_key: key_pair.public_key().to_vec(),
        signature: signature.as_bytes().to_vec(),
    };
    Ok(())
}

fn sign_potr_receipt_fixture_v1(
    mut receipt: PotrReceiptV1,
    gateway_key: &KeyPair,
    provider_key: &KeyPair,
) -> Result<PotrReceiptV1, Box<dyn Error>> {
    // Production ML-DSA signing intentionally draws fresh entropy. Fixtures use
    // a domain-separated deterministic stream so regeneration is byte-identical.
    let (gateway_algorithm, gateway_public_key) = gateway_key.public_key().try_to_bytes()?;
    if gateway_algorithm != Algorithm::Ed25519 {
        return Err("PoTR fixture gateway key must use Ed25519".into());
    }
    let gateway_public_key = gateway_public_key.to_vec();

    let (provider_algorithm, provider_public_key) = provider_key.public_key().try_to_bytes()?;
    if provider_algorithm != Algorithm::MlDsa {
        return Err("PoTR fixture provider key must use ML-DSA-65".into());
    }
    let provider_public_key = provider_public_key.to_vec();
    let (provider_private_algorithm, provider_private_key) = provider_key.private_key().to_bytes();
    if provider_private_algorithm != Algorithm::MlDsa {
        return Err("PoTR fixture provider private key must use ML-DSA-65".into());
    }

    receipt.gateway_signature = None;
    receipt.provider_signature = None;
    let payload = receipt.signing_payload_bytes()?;
    let gateway_signature = Signature::try_new(gateway_key.private_key(), &payload)?;
    let mut provider_signing_rng = deterministic_chacha20_rng(
        HedgedRngSeed::from_entropy(*blake3::hash(b"sorafs-fixture-potr-mldsa-sign-v1").as_bytes()),
        b"sorafs-fixture-potr-mldsa-sign-v1",
    );
    let provider_signature = sign_mldsa(
        MlDsaSuite::MlDsa65,
        &provider_private_key,
        &[],
        &payload,
        &mut provider_signing_rng,
    )?;
    receipt.gateway_signature = Some(PotrSignatureV1 {
        algorithm: PotrSignatureAlgorithm::Ed25519,
        public_key: gateway_public_key,
        signature: gateway_signature.payload().to_vec(),
    });
    receipt.provider_signature = Some(PotrSignatureV1 {
        algorithm: PotrSignatureAlgorithm::MlDsa65,
        public_key: provider_public_key,
        signature: provider_signature.as_bytes().to_vec(),
    });
    receipt.validate()?;
    Ok(receipt)
}

fn empty_governance_ed25519_signature() -> GovernanceLogSignatureV1 {
    GovernanceLogSignatureV1 {
        algorithm: GovernanceSignatureAlgorithm::Ed25519,
        public_key: Vec::new(),
        signature: Vec::new(),
    }
}

fn signed_governance_node(
    payload: GovernanceLogPayloadV1,
    prev_cid: Option<Vec<u8>>,
    timestamp: u64,
    publisher_peer_id: &[u8],
) -> Result<GovernanceLogNodeV1, Box<dyn Error>> {
    let publisher_peer_id = publisher_peer_id.to_vec();
    let node_cid =
        governance_log_node_cid_v1(prev_cid.as_deref(), timestamp, &publisher_peer_id, &payload)?;
    let mut node = GovernanceLogNodeV1 {
        version: GOVERNANCE_LOG_VERSION_V1,
        node_cid,
        prev_cid,
        timestamp,
        publisher_peer_id,
        payload,
        publisher_signature: empty_governance_ed25519_signature(),
    };
    let signing_key = SigningKey::from_bytes(&GOVERNANCE_FIXTURE_SIGNING_SEED);
    let signature = signing_key.sign(&node.signature_payload_bytes()?);
    node.publisher_signature = GovernanceLogSignatureV1 {
        algorithm: GovernanceSignatureAlgorithm::Ed25519,
        public_key: signing_key.verifying_key().to_bytes().to_vec(),
        signature: signature.to_bytes().to_vec(),
    };
    node.validate()?;
    node.verify_publisher_signature()?;
    Ok(node)
}

fn governance_dag_node(
    proof: PorProofV1,
    prev_cid: Option<Vec<u8>>,
    timestamp: u64,
) -> Result<GovernanceLogNodeV1, Box<dyn Error>> {
    signed_governance_node(
        GovernanceLogPayloadV1::PorProof(proof),
        prev_cid,
        timestamp,
        b"12D3KooWGovernanceDagPublisher",
    )
}

fn governance_dag_block(
    node: GovernanceLogNodeV1,
    prev_block_cid: Option<Vec<u8>>,
    sequence: u64,
    timestamp: u64,
) -> Result<GovernanceDagBlockV1, Box<dyn Error>> {
    let publisher_peer_id = b"12D3KooWGovernanceDagPublisher".to_vec();
    let block_cid = governance_dag_block_cid_v1(
        prev_block_cid.as_deref(),
        sequence,
        timestamp,
        &publisher_peer_id,
        &node,
    )?;
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
    let signing_key = SigningKey::from_bytes(&GOVERNANCE_FIXTURE_SIGNING_SEED);
    let signature = signing_key.sign(&block.signature_payload_bytes()?);
    block.block_signature = GovernanceLogSignatureV1 {
        algorithm: GovernanceSignatureAlgorithm::Ed25519,
        public_key: signing_key.verifying_key().to_bytes().to_vec(),
        signature: signature.to_bytes().to_vec(),
    };
    block.validate()?;
    Ok(block)
}

fn governance_dag_head(
    blocks: &[GovernanceDagBlockV1],
) -> Result<GovernanceDagHeadV1, Box<dyn Error>> {
    let mut head = GovernanceDagHeadV1 {
        version: GOVERNANCE_DAG_HEAD_VERSION_V1,
        head_block_cid: blocks
            .last()
            .ok_or("governance DAG fixture chain must not be empty")?
            .block_cid
            .clone(),
        block_count: u64::try_from(blocks.len())?,
        generated_at: 1_700_001_000,
        publisher_peer_id: b"12D3KooWGovernanceDagPublisher".to_vec(),
        checkpoint_cid: None,
        head_signature: empty_governance_ed25519_signature(),
    };
    let signing_key = SigningKey::from_bytes(&GOVERNANCE_FIXTURE_SIGNING_SEED);
    let signature = signing_key.sign(&head.signature_payload_bytes()?);
    head.head_signature = GovernanceLogSignatureV1 {
        algorithm: GovernanceSignatureAlgorithm::Ed25519,
        public_key: signing_key.verifying_key().to_bytes().to_vec(),
        signature: signature.to_bytes().to_vec(),
    };
    head.validate()?;
    Ok(head)
}

fn write_norito_pair<T>(
    base_path: &Path,
    value: &T,
    json_value: Value,
) -> Result<(), Box<dyn Error>>
where
    T: NoritoSerialize,
{
    let bytes = norito::to_bytes(value)?;
    fs::write(base_path.with_extension("to"), &bytes)?;
    let json = to_string_pretty(&json_value)?;
    fs::write(base_path.with_extension("json"), json)?;
    Ok(())
}

fn write_expected_success_validation_outcome(
    path: &Path,
    outcome: &sorafs_manifest::ValidationOutcomeV1,
) -> Result<(), Box<dyn Error>> {
    if !outcome.is_ok() || outcome.code != "SFS-OK-000" {
        return Err(format!(
            "generated positive governance DAG fixture returned {}, expected SFS-OK-000",
            outcome.code
        )
        .into());
    }
    fs::write(path, format!("{}\n", to_string_pretty(outcome)?))?;
    Ok(())
}

fn write_expected_validation_outcome(
    path: &Path,
    outcome: &sorafs_manifest::ValidationOutcomeV1,
    expected_code: &str,
) -> Result<(), Box<dyn Error>> {
    if outcome.is_ok() || outcome.code != expected_code {
        return Err(format!(
            "generated negative governance DAG fixture returned {}, expected {expected_code}",
            outcome.code
        )
        .into());
    }
    fs::write(path, format!("{}\n", to_string_pretty(outcome)?))?;
    Ok(())
}

fn write_governance_sdk_fixture_inventory(gov_dir: &Path) -> Result<(), Box<dyn Error>> {
    const PAYLOAD_SPECS: [(&str, &str, &str, &str); 17] = [
        (
            "dag_block_0_v1.json",
            "governance_dag_block",
            "json",
            "valid",
        ),
        (
            "dag_block_0_v1.to",
            "governance_dag_block",
            "norito",
            "valid",
        ),
        (
            "dag_block_1_bad_predecessor_v1.json",
            "governance_dag_block",
            "json",
            "valid",
        ),
        (
            "dag_block_1_bad_predecessor_v1.to",
            "governance_dag_block",
            "norito",
            "valid",
        ),
        (
            "dag_block_1_v1.json",
            "governance_dag_block",
            "json",
            "valid",
        ),
        (
            "dag_block_1_v1.to",
            "governance_dag_block",
            "norito",
            "valid",
        ),
        (
            "dag_block_bad_signature_v1.json",
            "governance_dag_block",
            "json",
            "invalid_signature",
        ),
        (
            "dag_block_bad_signature_v1.to",
            "governance_dag_block",
            "norito",
            "invalid_signature",
        ),
        (
            "dag_block_trailing_bytes_v1.to",
            "governance_dag_block",
            "norito",
            "noncanonical_trailing_bytes",
        ),
        (
            "dag_head_bad_predecessor_v1.json",
            "governance_dag_head",
            "json",
            "valid",
        ),
        (
            "dag_head_bad_predecessor_v1.to",
            "governance_dag_head",
            "norito",
            "valid",
        ),
        (
            "dag_head_bad_signature_v1.json",
            "governance_dag_head",
            "json",
            "invalid_signature",
        ),
        (
            "dag_head_bad_signature_v1.to",
            "governance_dag_head",
            "norito",
            "invalid_signature",
        ),
        ("dag_head_v1.json", "governance_dag_head", "json", "valid"),
        ("dag_head_v1.to", "governance_dag_head", "norito", "valid"),
        ("node_v1.json", "governance_log_node", "json", "valid"),
        ("node_v1.to", "governance_log_node", "norito", "valid"),
    ];
    const OUTCOME_SPECS: [(&str, &str, &str, &str); 8] = [
        (
            "dag_block_bad_signature_validation_outcome_v1.json",
            "block_bad_signature",
            "Error",
            "SFS-SIG-006",
        ),
        (
            "dag_block_cid_mismatch_validation_outcome_v1.json",
            "block_expected_cid_mismatch",
            "Error",
            "SFS-GOV-004",
        ),
        (
            "dag_block_trailing_bytes_validation_outcome_v1.json",
            "block_noncanonical_trailing_bytes",
            "Error",
            "SFS-NORITO-001",
        ),
        (
            "dag_block_validation_outcome_v1.json",
            "block_valid",
            "Ok",
            "SFS-OK-000",
        ),
        (
            "dag_head_bad_predecessor_validation_outcome_v1.json",
            "head_bad_predecessor",
            "Error",
            "SFS-GOV-006",
        ),
        (
            "dag_head_bad_signature_validation_outcome_v1.json",
            "head_bad_signature",
            "Error",
            "SFS-SIG-007",
        ),
        (
            "dag_head_reordered_validation_outcome_v1.json",
            "head_reordered_blocks",
            "Error",
            "SFS-GOV-006",
        ),
        (
            "dag_head_validation_outcome_v1.json",
            "head_valid",
            "Ok",
            "SFS-OK-000",
        ),
    ];

    let mut payloads = Vec::with_capacity(PAYLOAD_SPECS.len());
    for (path, kind, encoding, signature_expectation) in PAYLOAD_SPECS {
        let (byte_length, digest) = governance_sdk_fixture_binding(&gov_dir.join(path))?;
        payloads.push(GovernanceSdkPayloadInventoryEntryV1 {
            path: path.to_owned(),
            kind: kind.to_owned(),
            encoding: encoding.to_owned(),
            signature_expectation: signature_expectation.to_owned(),
            byte_length,
            sha256: digest,
        });
    }

    let mut outcomes = Vec::with_capacity(OUTCOME_SPECS.len());
    for (path, scenario, status, code) in OUTCOME_SPECS {
        let (byte_length, digest) = governance_sdk_fixture_binding(&gov_dir.join(path))?;
        outcomes.push(GovernanceSdkOutcomeInventoryEntryV1 {
            path: path.to_owned(),
            scenario: scenario.to_owned(),
            status: status.to_owned(),
            code: code.to_owned(),
            byte_length,
            sha256: digest,
        });
    }

    let unsigned = GovernanceSdkUnsignedInventoryV1 {
        schema: GOVERNANCE_SDK_INVENTORY_SCHEMA.to_owned(),
        scope: GOVERNANCE_SDK_INVENTORY_SCOPE.to_owned(),
        signing_domain: GOVERNANCE_SDK_INVENTORY_SCHEMA.to_owned(),
        payloads: payloads.clone(),
        outcomes: outcomes.clone(),
    };
    let unsigned_value = parse_value(&to_string(&unsigned)?)?;
    let canonical_unsigned = to_string(&unsigned_value)?;
    let mut signing_payload =
        Vec::with_capacity(GOVERNANCE_SDK_INVENTORY_SCHEMA.len() + 1 + canonical_unsigned.len());
    signing_payload.extend_from_slice(GOVERNANCE_SDK_INVENTORY_SCHEMA.as_bytes());
    signing_payload.push(0);
    signing_payload.extend_from_slice(canonical_unsigned.as_bytes());

    // This seed is checked-in fixture material only. Production release keys
    // remain runtime-only and never pass through this generator.
    let signing_key = SigningKey::from_bytes(&GOVERNANCE_FIXTURE_SIGNING_SEED);
    let public_key = signing_key.verifying_key().to_bytes();
    let public_key_hex = encode(public_key);
    let public_key_fingerprint_sha256 = encode(sha256(public_key));
    if public_key_hex != GOVERNANCE_FIXTURE_PUBLIC_KEY_HEX
        || public_key_fingerprint_sha256 != GOVERNANCE_FIXTURE_PUBLIC_KEY_FINGERPRINT_SHA256
    {
        return Err("Governance DAG fixture key identity changed unexpectedly".into());
    }
    let signature_hex = encode(signing_key.sign(&signing_payload).to_bytes());

    let inventory = GovernanceSdkFixtureInventoryV1 {
        schema: GOVERNANCE_SDK_INVENTORY_SCHEMA.to_owned(),
        scope: GOVERNANCE_SDK_INVENTORY_SCOPE.to_owned(),
        signing_domain: GOVERNANCE_SDK_INVENTORY_SCHEMA.to_owned(),
        payloads,
        outcomes,
        signature: GovernanceSdkInventorySignatureV1 {
            algorithm: "ed25519".to_owned(),
            key_usage: "test_only_governance_fixture".to_owned(),
            public_key_hex,
            public_key_fingerprint_sha256,
            signature_hex,
        },
    };
    fs::write(
        gov_dir.join("sdk_validation_inventory_v1.json"),
        format!("{}\n", to_string_pretty(&inventory)?),
    )?;
    Ok(())
}

fn governance_sdk_fixture_binding(path: &Path) -> Result<(u64, String), Box<dyn Error>> {
    let bytes = fs::read(path)?;
    Ok((u64::try_from(bytes.len())?, encode(sha256(&bytes))))
}

fn write_reference_sdk_bundle_outcomes(fixtures_root: &Path) -> Result<(), Box<dyn Error>> {
    const GENERATED_AT: u64 = 1_700_001_234;
    const LINKED_NOW: u64 = 1_700_000_001;
    const ADMISSION_NOW: u64 = 300;
    const HETEROGENEOUS_POSITIVE: &[(FixtureBundlePayloadKindV1, &str)] = &[
        (
            FixtureBundlePayloadKindV1::ReplicationOrder,
            "replication_order/order_v1.to",
        ),
        (
            FixtureBundlePayloadKindV1::PdpCommitment,
            "pdp/commitment_v1.to",
        ),
        (
            FixtureBundlePayloadKindV1::PdpChallenge,
            "pdp/challenge_v1.to",
        ),
        (FixtureBundlePayloadKindV1::PdpProof, "pdp/proof_v1.to"),
        (
            FixtureBundlePayloadKindV1::PorChallenge,
            "por/challenge_v1.to",
        ),
        (FixtureBundlePayloadKindV1::PorProof, "por/proof_v1.to"),
        (
            FixtureBundlePayloadKindV1::PotrReceipt,
            "potr/receipt_v1.to",
        ),
        (
            FixtureBundlePayloadKindV1::RepairTaskRecord,
            "repair/task_v1.to",
        ),
        (
            FixtureBundlePayloadKindV1::OrderbookOrderRequest,
            "orderbook/order_request_v1.to",
        ),
        (
            FixtureBundlePayloadKindV1::OrderbookOrderCancel,
            "orderbook/order_cancel_v1.to",
        ),
        (
            FixtureBundlePayloadKindV1::OrderbookTradeEvent,
            "orderbook/trade_event_v1.to",
        ),
        (
            FixtureBundlePayloadKindV1::OrderbookSettlementChannel,
            "orderbook/settlement_channel_v1.to",
        ),
        (
            FixtureBundlePayloadKindV1::OrderbookSettlementReceipt,
            "orderbook/settlement_receipt_v1.to",
        ),
    ];
    const ROUTING_ADMISSION_POSITIVE: &[(FixtureBundlePayloadKindV1, &str)] = &[
        (
            FixtureBundlePayloadKindV1::ProviderAdvert,
            "provider_admission/advert_v1.to",
        ),
        (
            FixtureBundlePayloadKindV1::ProviderAdmissionEnvelope,
            "provider_admission/envelope_v1.to",
        ),
    ];
    const ORDERBOOK_BAD_SIGNATURE_NEGATIVE: &[(FixtureBundlePayloadKindV1, &str)] = &[
        (
            FixtureBundlePayloadKindV1::ReplicationOrder,
            "replication_order/order_v1.to",
        ),
        (
            FixtureBundlePayloadKindV1::PorChallenge,
            "por/challenge_v1.to",
        ),
        (FixtureBundlePayloadKindV1::PorProof, "por/proof_v1.to"),
        (
            FixtureBundlePayloadKindV1::OrderbookOrderRequest,
            "orderbook/negative/order_request_bad_signature_v1.to",
        ),
    ];
    const ORDERBOOK_TRAILING_BYTES_NEGATIVE: &[(FixtureBundlePayloadKindV1, &str)] = &[
        (
            FixtureBundlePayloadKindV1::ReplicationOrder,
            "replication_order/order_v1.to",
        ),
        (
            FixtureBundlePayloadKindV1::PorChallenge,
            "por/challenge_v1.to",
        ),
        (FixtureBundlePayloadKindV1::PorProof, "por/proof_v1.to"),
        (
            FixtureBundlePayloadKindV1::OrderbookOrderRequest,
            "orderbook/negative/order_request_trailing_bytes_v1.to",
        ),
    ];
    const PDP_DUPLICATE_HOT_LEAF_NEGATIVE: &[(FixtureBundlePayloadKindV1, &str)] = &[
        (
            FixtureBundlePayloadKindV1::ReplicationOrder,
            "replication_order/order_v1.to",
        ),
        (
            FixtureBundlePayloadKindV1::PdpCommitment,
            "pdp/commitment_v1.to",
        ),
        (
            FixtureBundlePayloadKindV1::PdpChallenge,
            "pdp/negative/duplicate_hot_leaf_challenge_v1.to",
        ),
    ];
    const PDP_MISSING_SIGNATURE_NEGATIVE: &[(FixtureBundlePayloadKindV1, &str)] = &[
        (
            FixtureBundlePayloadKindV1::ReplicationOrder,
            "replication_order/order_v1.to",
        ),
        (
            FixtureBundlePayloadKindV1::PdpCommitment,
            "pdp/commitment_v1.to",
        ),
        (
            FixtureBundlePayloadKindV1::PdpChallenge,
            "pdp/challenge_v1.to",
        ),
        (
            FixtureBundlePayloadKindV1::PdpProof,
            "pdp/negative/missing_signature_proof_v1.to",
        ),
    ];
    const PDP_WRONG_PROVIDER_NEGATIVE: &[(FixtureBundlePayloadKindV1, &str)] = &[
        (
            FixtureBundlePayloadKindV1::ReplicationOrder,
            "replication_order/order_v1.to",
        ),
        (
            FixtureBundlePayloadKindV1::PdpCommitment,
            "pdp/commitment_v1.to",
        ),
        (
            FixtureBundlePayloadKindV1::PdpChallenge,
            "pdp/challenge_v1.to",
        ),
        (
            FixtureBundlePayloadKindV1::PdpProof,
            "pdp/negative/wrong_provider_proof_v1.to",
        ),
    ];
    const REPAIR_MANIFEST_MISMATCH_NEGATIVE: &[(FixtureBundlePayloadKindV1, &str)] = &[
        (
            FixtureBundlePayloadKindV1::ReplicationOrder,
            "replication_order/order_v1.to",
        ),
        (
            FixtureBundlePayloadKindV1::RepairTaskRecord,
            "repair/negative/task_manifest_mismatch_v1.to",
        ),
    ];
    const REPAIR_PROVIDER_UNASSIGNED_NEGATIVE: &[(FixtureBundlePayloadKindV1, &str)] = &[
        (
            FixtureBundlePayloadKindV1::ReplicationOrder,
            "replication_order/order_v1.to",
        ),
        (
            FixtureBundlePayloadKindV1::RepairTaskRecord,
            "repair/negative/task_provider_unassigned_v1.to",
        ),
    ];

    let output_dir = fixtures_root.join("reference_sdk");
    fs::create_dir_all(&output_dir)?;
    let scenarios = [
        (
            "bundle_heterogeneous_positive_validation_outcome_v1.json",
            HETEROGENEOUS_POSITIVE,
            LINKED_NOW,
            "Ok",
            "SFS-PDP-DIAG-000",
        ),
        (
            "bundle_orderbook_bad_signature_negative_validation_outcome_v1.json",
            ORDERBOOK_BAD_SIGNATURE_NEGATIVE,
            LINKED_NOW,
            "Error",
            "SFS-BND-001",
        ),
        (
            "bundle_orderbook_trailing_bytes_negative_validation_outcome_v1.json",
            ORDERBOOK_TRAILING_BYTES_NEGATIVE,
            LINKED_NOW,
            "Error",
            "SFS-BND-001",
        ),
        (
            "bundle_pdp_duplicate_hot_leaf_negative_validation_outcome_v1.json",
            PDP_DUPLICATE_HOT_LEAF_NEGATIVE,
            LINKED_NOW,
            "Error",
            "SFS-BND-001",
        ),
        (
            "bundle_pdp_missing_signature_negative_validation_outcome_v1.json",
            PDP_MISSING_SIGNATURE_NEGATIVE,
            LINKED_NOW,
            "Error",
            "SFS-BND-001",
        ),
        (
            "bundle_pdp_wrong_provider_negative_validation_outcome_v1.json",
            PDP_WRONG_PROVIDER_NEGATIVE,
            LINKED_NOW,
            "Error",
            "SFS-BND-001",
        ),
        (
            "bundle_repair_manifest_mismatch_negative_validation_outcome_v1.json",
            REPAIR_MANIFEST_MISMATCH_NEGATIVE,
            LINKED_NOW,
            "Error",
            "SFS-BND-002",
        ),
        (
            "bundle_repair_provider_unassigned_negative_validation_outcome_v1.json",
            REPAIR_PROVIDER_UNASSIGNED_NEGATIVE,
            LINKED_NOW,
            "Error",
            "SFS-BND-003",
        ),
        (
            "bundle_routing_admission_positive_validation_outcome_v1.json",
            ROUTING_ADMISSION_POSITIVE,
            ADMISSION_NOW,
            "Ok",
            "SFS-OK-000",
        ),
    ];
    for (path, specs, now, expected_status, expected_code) in scenarios {
        let bytes = specs
            .iter()
            .map(|(_, path)| fs::read(fixtures_root.join(path)))
            .collect::<Result<Vec<_>, _>>()?;
        let payloads = specs
            .iter()
            .zip(&bytes)
            .map(|((kind, label), bytes)| {
                FixtureBundlePayloadV1::new(*kind, (*label).to_owned(), bytes)
            })
            .collect::<Vec<_>>();
        let outcome = validate_fixture_bundle_payloads(&payloads, now, GENERATED_AT);
        if outcome.status != expected_status || outcome.code != expected_code {
            return Err(format!(
                "reference SDK bundle `{path}` returned {}/{}, expected {expected_status}/{expected_code}: {outcome:?}",
                outcome.status,
                outcome.code
            )
            .into());
        }
        fs::write(
            output_dir.join(path),
            format!("{}\n", to_string_pretty(&outcome)?),
        )?;
    }
    Ok(())
}

fn write_reference_sdk_fixture_inventory(fixtures_root: &Path) -> Result<(), Box<dyn Error>> {
    const PAYLOAD_SPECS: &[(&str, &str, &str, &str, &str)] = &[
        (
            "governance/dag_block_0_v1.json",
            "governance_dag",
            "governance_dag_block",
            "json",
            "valid",
        ),
        (
            "governance/dag_block_0_v1.to",
            "governance_dag",
            "governance_dag_block",
            "norito",
            "valid",
        ),
        (
            "governance/dag_block_1_bad_predecessor_v1.json",
            "governance_dag",
            "governance_dag_block",
            "json",
            "chain_invalid_predecessor",
        ),
        (
            "governance/dag_block_1_bad_predecessor_v1.to",
            "governance_dag",
            "governance_dag_block",
            "norito",
            "chain_invalid_predecessor",
        ),
        (
            "governance/dag_block_1_v1.json",
            "governance_dag",
            "governance_dag_block",
            "json",
            "valid",
        ),
        (
            "governance/dag_block_1_v1.to",
            "governance_dag",
            "governance_dag_block",
            "norito",
            "valid",
        ),
        (
            "governance/dag_block_bad_signature_v1.json",
            "governance_dag",
            "governance_dag_block",
            "json",
            "invalid_signature",
        ),
        (
            "governance/dag_block_bad_signature_v1.to",
            "governance_dag",
            "governance_dag_block",
            "norito",
            "invalid_signature",
        ),
        (
            "governance/dag_block_trailing_bytes_v1.to",
            "governance_dag",
            "governance_dag_block",
            "norito",
            "noncanonical_trailing_bytes",
        ),
        (
            "governance/dag_head_bad_predecessor_v1.json",
            "governance_dag",
            "governance_dag_head",
            "json",
            "chain_invalid_predecessor",
        ),
        (
            "governance/dag_head_bad_predecessor_v1.to",
            "governance_dag",
            "governance_dag_head",
            "norito",
            "chain_invalid_predecessor",
        ),
        (
            "governance/dag_head_bad_signature_v1.json",
            "governance_dag",
            "governance_dag_head",
            "json",
            "invalid_signature",
        ),
        (
            "governance/dag_head_bad_signature_v1.to",
            "governance_dag",
            "governance_dag_head",
            "norito",
            "invalid_signature",
        ),
        (
            "governance/dag_head_v1.json",
            "governance_dag",
            "governance_dag_head",
            "json",
            "valid",
        ),
        (
            "governance/dag_head_v1.to",
            "governance_dag",
            "governance_dag_head",
            "norito",
            "valid",
        ),
        (
            "governance/node_v1.json",
            "governance_dag",
            "governance_log_node",
            "json",
            "valid",
        ),
        (
            "governance/node_v1.to",
            "governance_dag",
            "governance_log_node",
            "norito",
            "valid",
        ),
        (
            "moderation/governance_node_v1.json",
            "moderation",
            "moderation_ballot_governance_node",
            "json",
            "valid",
        ),
        (
            "moderation/governance_node_v1.to",
            "moderation",
            "moderation_ballot_governance_node",
            "norito",
            "valid",
        ),
        (
            "orderbook/negative/order_request_bad_signature_v1.json",
            "orderbook",
            "orderbook_order_request",
            "json",
            "invalid_signature",
        ),
        (
            "orderbook/negative/order_request_bad_signature_v1.to",
            "orderbook",
            "orderbook_order_request",
            "norito",
            "invalid_signature",
        ),
        (
            "orderbook/negative/order_request_trailing_bytes_v1.to",
            "orderbook",
            "orderbook_order_request",
            "norito",
            "noncanonical_trailing_bytes",
        ),
        (
            "orderbook/order_cancel_v1.json",
            "orderbook",
            "orderbook_order_cancel",
            "json",
            "valid",
        ),
        (
            "orderbook/order_cancel_v1.to",
            "orderbook",
            "orderbook_order_cancel",
            "norito",
            "valid",
        ),
        (
            "orderbook/order_request_v1.json",
            "orderbook",
            "orderbook_order_request",
            "json",
            "valid",
        ),
        (
            "orderbook/order_request_v1.to",
            "orderbook",
            "orderbook_order_request",
            "norito",
            "valid",
        ),
        (
            "orderbook/settlement_channel_v1.json",
            "orderbook",
            "orderbook_settlement_channel",
            "json",
            "valid",
        ),
        (
            "orderbook/settlement_channel_v1.to",
            "orderbook",
            "orderbook_settlement_channel",
            "norito",
            "valid",
        ),
        (
            "orderbook/settlement_receipt_v1.json",
            "orderbook",
            "orderbook_settlement_receipt",
            "json",
            "valid",
        ),
        (
            "orderbook/settlement_receipt_v1.to",
            "orderbook",
            "orderbook_settlement_receipt",
            "norito",
            "valid",
        ),
        (
            "orderbook/trade_event_v1.json",
            "orderbook",
            "orderbook_trade_event",
            "json",
            "valid",
        ),
        (
            "orderbook/trade_event_v1.to",
            "orderbook",
            "orderbook_trade_event",
            "norito",
            "valid",
        ),
        (
            "pdp/challenge_v1.json",
            "pdp",
            "pdp_challenge",
            "json",
            "valid",
        ),
        (
            "pdp/challenge_v1.to",
            "pdp",
            "pdp_challenge",
            "norito",
            "valid",
        ),
        (
            "pdp/commitment_v1.json",
            "pdp",
            "pdp_commitment",
            "json",
            "valid",
        ),
        (
            "pdp/commitment_v1.to",
            "pdp",
            "pdp_commitment",
            "norito",
            "valid",
        ),
        (
            "pdp/negative/duplicate_hot_leaf_challenge_v1.json",
            "pdp",
            "pdp_challenge",
            "json",
            "invalid_duplicate_hot_leaf",
        ),
        (
            "pdp/negative/duplicate_hot_leaf_challenge_v1.to",
            "pdp",
            "pdp_challenge",
            "norito",
            "invalid_duplicate_hot_leaf",
        ),
        (
            "pdp/negative/late_proof_v1.json",
            "pdp",
            "pdp_proof",
            "json",
            "invalid_late_proof",
        ),
        (
            "pdp/negative/late_proof_v1.to",
            "pdp",
            "pdp_proof",
            "norito",
            "invalid_late_proof",
        ),
        (
            "pdp/negative/missing_hot_leaf_path_proof_v1.json",
            "pdp",
            "pdp_proof",
            "json",
            "invalid_missing_hot_leaf_path",
        ),
        (
            "pdp/negative/missing_hot_leaf_path_proof_v1.to",
            "pdp",
            "pdp_proof",
            "norito",
            "invalid_missing_hot_leaf_path",
        ),
        (
            "pdp/negative/missing_segment_path_proof_v1.json",
            "pdp",
            "pdp_proof",
            "json",
            "invalid_missing_segment_path",
        ),
        (
            "pdp/negative/missing_segment_path_proof_v1.to",
            "pdp",
            "pdp_proof",
            "norito",
            "invalid_missing_segment_path",
        ),
        (
            "pdp/negative/missing_signature_proof_v1.json",
            "pdp",
            "pdp_proof",
            "json",
            "invalid_missing_signature",
        ),
        (
            "pdp/negative/missing_signature_proof_v1.to",
            "pdp",
            "pdp_proof",
            "norito",
            "invalid_missing_signature",
        ),
        (
            "pdp/negative/wrong_manifest_proof_v1.json",
            "pdp",
            "pdp_proof",
            "json",
            "invalid_manifest_binding",
        ),
        (
            "pdp/negative/wrong_manifest_proof_v1.to",
            "pdp",
            "pdp_proof",
            "norito",
            "invalid_manifest_binding",
        ),
        (
            "pdp/negative/wrong_path_proof_v1.json",
            "pdp",
            "pdp_proof",
            "json",
            "invalid_merkle_path",
        ),
        (
            "pdp/negative/wrong_path_proof_v1.to",
            "pdp",
            "pdp_proof",
            "norito",
            "invalid_merkle_path",
        ),
        (
            "pdp/negative/wrong_provider_proof_v1.json",
            "pdp",
            "pdp_proof",
            "json",
            "invalid_provider_binding",
        ),
        (
            "pdp/negative/wrong_provider_proof_v1.to",
            "pdp",
            "pdp_proof",
            "norito",
            "invalid_provider_binding",
        ),
        ("pdp/proof_v1.json", "pdp", "pdp_proof", "json", "valid"),
        ("pdp/proof_v1.to", "pdp", "pdp_proof", "norito", "valid"),
        (
            "por/challenge_v1.json",
            "por",
            "por_challenge",
            "json",
            "valid",
        ),
        (
            "por/challenge_v1.to",
            "por",
            "por_challenge",
            "norito",
            "valid",
        ),
        ("por/proof_v1.json", "por", "por_proof", "json", "valid"),
        ("por/proof_v1.to", "por", "por_proof", "norito", "valid"),
        (
            "por/verdict_v1.json",
            "por",
            "por_audit_verdict",
            "json",
            "valid",
        ),
        (
            "por/verdict_v1.to",
            "por",
            "por_audit_verdict",
            "norito",
            "valid",
        ),
        (
            "potr/receipt_v1.json",
            "potr",
            "potr_receipt",
            "json",
            "valid",
        ),
        (
            "potr/receipt_v1.to",
            "potr",
            "potr_receipt",
            "norito",
            "valid",
        ),
        (
            "provider_admission/advert_v1.json",
            "routing",
            "provider_advert",
            "json",
            "valid",
        ),
        (
            "provider_admission/advert_v1.to",
            "routing",
            "provider_advert",
            "norito",
            "valid",
        ),
        (
            "provider_admission/envelope_v1.json",
            "routing",
            "provider_admission_envelope",
            "json",
            "valid",
        ),
        (
            "provider_admission/envelope_v1.to",
            "routing",
            "provider_admission_envelope",
            "norito",
            "valid",
        ),
        (
            "repair/negative/task_manifest_mismatch_v1.json",
            "repair",
            "repair_task_record",
            "json",
            "invalid_manifest_binding",
        ),
        (
            "repair/negative/task_manifest_mismatch_v1.to",
            "repair",
            "repair_task_record",
            "norito",
            "invalid_manifest_binding",
        ),
        (
            "repair/negative/task_provider_unassigned_v1.json",
            "repair",
            "repair_task_record",
            "json",
            "invalid_provider_assignment",
        ),
        (
            "repair/negative/task_provider_unassigned_v1.to",
            "repair",
            "repair_task_record",
            "norito",
            "invalid_provider_assignment",
        ),
        (
            "repair/task_v1.json",
            "repair",
            "repair_task_record",
            "json",
            "valid",
        ),
        (
            "repair/task_v1.to",
            "repair",
            "repair_task_record",
            "norito",
            "valid",
        ),
        (
            "replication_order/order_v1.json",
            "routing",
            "replication_order",
            "json",
            "valid",
        ),
        (
            "replication_order/order_v1.to",
            "routing",
            "replication_order",
            "norito",
            "valid",
        ),
    ];
    const OUTCOME_SPECS: &[(&str, &str, &str, &str, &str)] = &[
        (
            "governance/dag_block_bad_signature_validation_outcome_v1.json",
            "governance_dag",
            "block_bad_signature",
            "Error",
            "SFS-SIG-006",
        ),
        (
            "governance/dag_block_cid_mismatch_validation_outcome_v1.json",
            "governance_dag",
            "block_expected_cid_mismatch",
            "Error",
            "SFS-GOV-004",
        ),
        (
            "governance/dag_block_trailing_bytes_validation_outcome_v1.json",
            "governance_dag",
            "block_noncanonical_trailing_bytes",
            "Error",
            "SFS-NORITO-001",
        ),
        (
            "governance/dag_block_validation_outcome_v1.json",
            "governance_dag",
            "block_valid",
            "Ok",
            "SFS-OK-000",
        ),
        (
            "governance/dag_head_bad_predecessor_validation_outcome_v1.json",
            "governance_dag",
            "head_bad_predecessor",
            "Error",
            "SFS-GOV-006",
        ),
        (
            "governance/dag_head_bad_signature_validation_outcome_v1.json",
            "governance_dag",
            "head_bad_signature",
            "Error",
            "SFS-SIG-007",
        ),
        (
            "governance/dag_head_reordered_validation_outcome_v1.json",
            "governance_dag",
            "head_reordered_blocks",
            "Error",
            "SFS-GOV-006",
        ),
        (
            "governance/dag_head_validation_outcome_v1.json",
            "governance_dag",
            "head_valid",
            "Ok",
            "SFS-OK-000",
        ),
        (
            "moderation/governance_node_validation_outcome_v1.json",
            "moderation",
            "moderation_ballot_governance_node_valid",
            "Ok",
            "SFS-OK-000",
        ),
        (
            "orderbook/negative/order_request_bad_signature_validation_outcome_v1.json",
            "orderbook",
            "order_request_bad_signature",
            "Error",
            "SFS-SIG-007",
        ),
        (
            "orderbook/negative/order_request_trailing_bytes_validation_outcome_v1.json",
            "orderbook",
            "order_request_noncanonical_trailing_bytes",
            "Error",
            "SFS-NORITO-001",
        ),
        (
            "orderbook/order_request_validation_outcome_v1.json",
            "orderbook",
            "order_request_valid",
            "Ok",
            "SFS-OK-000",
        ),
        (
            "pdp/bundle_validation_outcome_v1.json",
            "pdp",
            "pdp_bundle_valid",
            "Ok",
            "SFS-PDP-DIAG-000",
        ),
        (
            "pdp/negative/duplicate_hot_leaf_challenge_validation_outcome_v1.json",
            "pdp",
            "duplicate_hot_leaf_challenge",
            "Error",
            "SFS-PDP-001",
        ),
        (
            "pdp/negative/late_proof_validation_outcome_v1.json",
            "pdp",
            "late_proof",
            "Error",
            "SFS-POL-002",
        ),
        (
            "pdp/negative/missing_hot_leaf_path_proof_validation_outcome_v1.json",
            "pdp",
            "missing_hot_leaf_path",
            "Error",
            "SFS-PDP-001",
        ),
        (
            "pdp/negative/missing_segment_path_proof_validation_outcome_v1.json",
            "pdp",
            "missing_segment_path",
            "Error",
            "SFS-PDP-001",
        ),
        (
            "pdp/negative/missing_signature_proof_validation_outcome_v1.json",
            "pdp",
            "missing_proof_signature",
            "Error",
            "SFS-SIG-008",
        ),
        (
            "pdp/negative/wrong_manifest_proof_validation_outcome_v1.json",
            "pdp",
            "wrong_manifest",
            "Error",
            "SFS-PDP-003",
        ),
        (
            "pdp/negative/wrong_path_proof_validation_outcome_v1.json",
            "pdp",
            "wrong_merkle_path",
            "Error",
            "SFS-PDP-003",
        ),
        (
            "pdp/negative/wrong_provider_proof_validation_outcome_v1.json",
            "pdp",
            "wrong_provider",
            "Error",
            "SFS-PDP-003",
        ),
        (
            "reference_sdk/bundle_heterogeneous_positive_validation_outcome_v1.json",
            "reference_sdk",
            "bundle_heterogeneous_positive",
            "Ok",
            "SFS-PDP-DIAG-000",
        ),
        (
            "reference_sdk/bundle_orderbook_bad_signature_negative_validation_outcome_v1.json",
            "reference_sdk",
            "bundle_orderbook_bad_signature_negative",
            "Error",
            "SFS-BND-001",
        ),
        (
            "reference_sdk/bundle_orderbook_trailing_bytes_negative_validation_outcome_v1.json",
            "reference_sdk",
            "bundle_orderbook_trailing_bytes_negative",
            "Error",
            "SFS-BND-001",
        ),
        (
            "reference_sdk/bundle_pdp_duplicate_hot_leaf_negative_validation_outcome_v1.json",
            "reference_sdk",
            "bundle_pdp_duplicate_hot_leaf_negative",
            "Error",
            "SFS-BND-001",
        ),
        (
            "reference_sdk/bundle_pdp_missing_signature_negative_validation_outcome_v1.json",
            "reference_sdk",
            "bundle_pdp_missing_signature_negative",
            "Error",
            "SFS-BND-001",
        ),
        (
            "reference_sdk/bundle_pdp_wrong_provider_negative_validation_outcome_v1.json",
            "reference_sdk",
            "bundle_pdp_wrong_provider_negative",
            "Error",
            "SFS-BND-001",
        ),
        (
            "reference_sdk/bundle_repair_manifest_mismatch_negative_validation_outcome_v1.json",
            "reference_sdk",
            "bundle_repair_manifest_mismatch_negative",
            "Error",
            "SFS-BND-002",
        ),
        (
            "reference_sdk/bundle_repair_provider_unassigned_negative_validation_outcome_v1.json",
            "reference_sdk",
            "bundle_repair_provider_unassigned_negative",
            "Error",
            "SFS-BND-003",
        ),
        (
            "reference_sdk/bundle_routing_admission_positive_validation_outcome_v1.json",
            "reference_sdk",
            "bundle_routing_admission_positive",
            "Ok",
            "SFS-OK-000",
        ),
    ];

    if PAYLOAD_SPECS.windows(2).any(|pair| pair[0].0 >= pair[1].0)
        || OUTCOME_SPECS.windows(2).any(|pair| pair[0].0 >= pair[1].0)
    {
        return Err("reference SDK fixture inventory paths must be unique and sorted".into());
    }

    let mut payloads = Vec::with_capacity(PAYLOAD_SPECS.len());
    for (path, domain, kind, encoding, expectation) in PAYLOAD_SPECS {
        let (byte_length, digest) = governance_sdk_fixture_binding(&fixtures_root.join(path))?;
        payloads.push(ReferenceSdkPayloadInventoryEntryV1 {
            path: (*path).to_owned(),
            domain: (*domain).to_owned(),
            kind: (*kind).to_owned(),
            encoding: (*encoding).to_owned(),
            expectation: (*expectation).to_owned(),
            byte_length,
            sha256: digest,
        });
    }

    let mut outcomes = Vec::with_capacity(OUTCOME_SPECS.len());
    for (path, domain, scenario, status, code) in OUTCOME_SPECS {
        let (byte_length, digest) = governance_sdk_fixture_binding(&fixtures_root.join(path))?;
        outcomes.push(ReferenceSdkOutcomeInventoryEntryV1 {
            path: (*path).to_owned(),
            domain: (*domain).to_owned(),
            scenario: (*scenario).to_owned(),
            status: (*status).to_owned(),
            code: (*code).to_owned(),
            byte_length,
            sha256: digest,
        });
    }

    let unsigned = ReferenceSdkUnsignedInventoryV1 {
        schema: REFERENCE_SDK_INVENTORY_SCHEMA.to_owned(),
        scope: REFERENCE_SDK_INVENTORY_SCOPE.to_owned(),
        signing_domain: REFERENCE_SDK_INVENTORY_SCHEMA.to_owned(),
        payloads: payloads.clone(),
        outcomes: outcomes.clone(),
    };
    let unsigned_value = parse_value(&to_string(&unsigned)?)?;
    let canonical_unsigned = to_string(&unsigned_value)?;
    let mut signing_payload =
        Vec::with_capacity(REFERENCE_SDK_INVENTORY_SCHEMA.len() + 1 + canonical_unsigned.len());
    signing_payload.extend_from_slice(REFERENCE_SDK_INVENTORY_SCHEMA.as_bytes());
    signing_payload.push(0);
    signing_payload.extend_from_slice(canonical_unsigned.as_bytes());

    // This deterministic seed is checked-in fixture material only. Production
    // signing keys remain runtime-only and are never accepted by this profile.
    let signing_key = SigningKey::from_bytes(&GOVERNANCE_FIXTURE_SIGNING_SEED);
    let public_key = signing_key.verifying_key().to_bytes();
    let public_key_hex = encode(public_key);
    let public_key_fingerprint_sha256 = encode(sha256(public_key));
    if public_key_hex != GOVERNANCE_FIXTURE_PUBLIC_KEY_HEX
        || public_key_fingerprint_sha256 != GOVERNANCE_FIXTURE_PUBLIC_KEY_FINGERPRINT_SHA256
    {
        return Err("reference SDK fixture key identity changed unexpectedly".into());
    }
    let signature_hex = encode(signing_key.sign(&signing_payload).to_bytes());

    let inventory = ReferenceSdkFixtureInventoryV1 {
        schema: REFERENCE_SDK_INVENTORY_SCHEMA.to_owned(),
        scope: REFERENCE_SDK_INVENTORY_SCOPE.to_owned(),
        signing_domain: REFERENCE_SDK_INVENTORY_SCHEMA.to_owned(),
        payloads,
        outcomes,
        signature: GovernanceSdkInventorySignatureV1 {
            algorithm: "ed25519".to_owned(),
            key_usage: "test_only_reference_sdk_fixture".to_owned(),
            public_key_hex,
            public_key_fingerprint_sha256,
            signature_hex,
        },
    };
    fs::write(
        fixtures_root.join("reference_sdk_validation_inventory_v1.json"),
        format!("{}\n", to_string_pretty(&inventory)?),
    )?;
    Ok(())
}

fn challenge_json(challenge: &PorChallengeV1) -> Value {
    let mut map = Map::new();
    map.insert("version".into(), Value::from(challenge.version));
    map.insert(
        "challenge_id_hex".into(),
        Value::from(encode(challenge.challenge_id)),
    );
    map.insert(
        "manifest_digest_hex".into(),
        Value::from(encode(challenge.manifest_digest)),
    );
    map.insert(
        "provider_id_hex".into(),
        Value::from(encode(challenge.provider_id)),
    );
    map.insert("epoch_id".into(), Value::from(challenge.epoch_id));
    map.insert("drand_round".into(), Value::from(challenge.drand_round));
    map.insert(
        "drand_randomness_hex".into(),
        Value::from(encode(challenge.drand_randomness)),
    );
    map.insert(
        "drand_signature_hex".into(),
        Value::from(encode(challenge.drand_signature)),
    );
    map.insert(
        "vrf_output_hex".into(),
        match challenge.vrf_output {
            Some(output) => Value::from(encode(output)),
            None => Value::Null,
        },
    );
    map.insert(
        "vrf_proof_hex".into(),
        match &challenge.vrf_proof {
            Some(iroha_crypto::vrf::VrfProof::SigInG1(proof)) => Value::from(encode(proof)),
            Some(iroha_crypto::vrf::VrfProof::SigInG2(proof)) => Value::from(encode(proof)),
            None => Value::Null,
        },
    );
    map.insert("forced".into(), Value::from(challenge.forced));
    map.insert(
        "chunking_profile".into(),
        Value::from(challenge.chunking_profile.clone()),
    );
    map.insert("seed_hex".into(), Value::from(encode(challenge.seed)));
    map.insert("sample_tier".into(), Value::from(challenge.sample_tier));
    map.insert("sample_count".into(), Value::from(challenge.sample_count));
    map.insert(
        "sample_indices".into(),
        Value::Array(
            challenge
                .sample_indices
                .iter()
                .map(|idx| Value::from(*idx))
                .collect(),
        ),
    );
    map.insert("issued_at".into(), Value::from(challenge.issued_at));
    map.insert("deadline_at".into(), Value::from(challenge.deadline_at));
    Value::Object(map)
}

fn proof_json(proof: &PorProofV1, digest: [u8; 32]) -> Value {
    let mut map = Map::new();
    map.insert("version".into(), Value::from(proof.version));
    map.insert(
        "challenge_id_hex".into(),
        Value::from(encode(proof.challenge_id)),
    );
    map.insert(
        "manifest_digest_hex".into(),
        Value::from(encode(proof.manifest_digest)),
    );
    map.insert(
        "provider_id_hex".into(),
        Value::from(encode(proof.provider_id)),
    );
    map.insert("submitted_at".into(), Value::from(proof.submitted_at));
    let samples = proof
        .samples
        .iter()
        .map(|sample| {
            let mut sample_map = Map::new();
            sample_map.insert("sample_index".into(), Value::from(sample.sample_index));
            sample_map.insert("chunk_offset".into(), Value::from(sample.chunk_offset));
            sample_map.insert("chunk_size".into(), Value::from(sample.chunk_size));
            sample_map.insert(
                "chunk_digest_hex".into(),
                Value::from(encode(sample.chunk_digest)),
            );
            sample_map.insert(
                "leaf_digest_hex".into(),
                Value::from(encode(sample.leaf_digest)),
            );
            Value::Object(sample_map)
        })
        .collect();
    map.insert("samples".into(), Value::Array(samples));
    map.insert(
        "auth_path_hex".into(),
        Value::Array(
            proof
                .auth_path
                .iter()
                .map(|node| Value::from(encode(node)))
                .collect(),
        ),
    );
    let mut sig = Map::new();
    let algorithm = match proof.signature.algorithm {
        SignatureAlgorithm::Ed25519 => "ed25519",
        SignatureAlgorithm::MultiSig => "multisig",
    };
    sig.insert("algorithm".into(), Value::from(algorithm));
    sig.insert(
        "public_key_hex".into(),
        Value::from(encode(&proof.signature.public_key)),
    );
    sig.insert(
        "signature_hex".into(),
        Value::from(encode(&proof.signature.signature)),
    );
    map.insert("signature".into(), Value::Object(sig));
    map.insert("proof_digest_hex".into(), Value::from(encode(digest)));
    Value::Object(map)
}

fn verdict_json(verdict: &AuditVerdictV1) -> Value {
    let mut map = Map::new();
    map.insert("version".into(), Value::from(verdict.version));
    map.insert(
        "manifest_digest_hex".into(),
        Value::from(encode(verdict.manifest_digest)),
    );
    map.insert(
        "provider_id_hex".into(),
        Value::from(encode(verdict.provider_id)),
    );
    map.insert(
        "challenge_id_hex".into(),
        Value::from(encode(verdict.challenge_id)),
    );
    map.insert(
        "proof_digest_hex".into(),
        verdict
            .proof_digest
            .as_ref()
            .map(|digest| Value::from(encode(digest)))
            .unwrap_or(Value::Null),
    );
    let outcome = match verdict.outcome {
        AuditOutcomeV1::Success => "success",
        AuditOutcomeV1::Failed => "failed",
        AuditOutcomeV1::Repaired => "repaired",
    };
    map.insert("outcome".into(), Value::from(outcome));
    map.insert(
        "failure_reason".into(),
        verdict
            .failure_reason
            .as_ref()
            .map(|s| Value::from(s.clone()))
            .unwrap_or(Value::Null),
    );
    map.insert("decided_at".into(), Value::from(verdict.decided_at));
    let signatures = verdict
        .auditor_signatures
        .iter()
        .map(|sig| {
            let mut sig_map = Map::new();
            let algorithm = match sig.algorithm {
                SignatureAlgorithm::Ed25519 => "ed25519",
                SignatureAlgorithm::MultiSig => "multisig",
            };
            sig_map.insert("algorithm".into(), Value::from(algorithm));
            sig_map.insert(
                "public_key_hex".into(),
                Value::from(encode(&sig.public_key)),
            );
            sig_map.insert("signature_hex".into(), Value::from(encode(&sig.signature)));
            Value::Object(sig_map)
        })
        .collect();
    map.insert("auditor_signatures".into(), Value::Array(signatures));
    let metadata = verdict
        .metadata
        .iter()
        .map(|entry| {
            let mut meta_map = Map::new();
            meta_map.insert("key".into(), Value::from(entry.key.clone()));
            meta_map.insert("value".into(), Value::from(entry.value.clone()));
            Value::Object(meta_map)
        })
        .collect();
    map.insert("metadata".into(), Value::Array(metadata));
    Value::Object(map)
}

fn potr_receipt_json(receipt: &PotrReceiptV1) -> Value {
    let mut map = Map::new();
    map.insert("version".into(), Value::from(receipt.version));
    map.insert(
        "manifest_digest_hex".into(),
        Value::from(encode(receipt.manifest_digest)),
    );
    map.insert(
        "provider_id_hex".into(),
        Value::from(encode(receipt.provider_id)),
    );
    map.insert("tier".into(), Value::from(proof_stream_tier(receipt.tier)));
    map.insert("deadline_ms".into(), Value::from(receipt.deadline_ms));
    map.insert("latency_ms".into(), Value::from(receipt.latency_ms));
    map.insert("status".into(), Value::from(potr_status(receipt.status)));
    map.insert(
        "requested_at_ms".into(),
        Value::from(receipt.requested_at_ms),
    );
    map.insert(
        "responded_at_ms".into(),
        Value::from(receipt.responded_at_ms),
    );
    map.insert("recorded_at_ms".into(), Value::from(receipt.recorded_at_ms));
    map.insert("range_start".into(), Value::from(receipt.range_start));
    map.insert("range_end".into(), Value::from(receipt.range_end));
    map.insert(
        "request_id_hex".into(),
        receipt
            .request_id
            .map(|request_id| Value::from(encode(request_id)))
            .unwrap_or(Value::Null),
    );
    map.insert(
        "trace_id_hex".into(),
        receipt
            .trace_id
            .map(|trace_id| Value::from(encode(trace_id)))
            .unwrap_or(Value::Null),
    );
    map.insert(
        "note".into(),
        receipt
            .note
            .as_ref()
            .map(|note| Value::from(note.clone()))
            .unwrap_or(Value::Null),
    );
    map.insert(
        "gateway_signature".into(),
        if receipt.gateway_signature.is_some() {
            Value::from("present")
        } else {
            Value::Null
        },
    );
    map.insert(
        "provider_signature".into(),
        if receipt.provider_signature.is_some() {
            Value::from("present")
        } else {
            Value::Null
        },
    );
    Value::Object(map)
}

fn repair_task_json(task: &RepairTaskRecordV1) -> Value {
    let mut map = Map::new();
    map.insert("version".into(), Value::from(task.version));
    map.insert("ticket_id".into(), Value::from(task.ticket_id.to_string()));
    map.insert(
        "manifest_digest_hex".into(),
        Value::from(encode(task.manifest_digest)),
    );
    map.insert(
        "provider_id_hex".into(),
        Value::from(encode(task.provider_id)),
    );
    map.insert(
        "auditor_account".into(),
        Value::from(task.auditor_account.clone()),
    );
    map.insert("state".into(), repair_state_json(&task.state));
    map.insert(
        "por_history_id".into(),
        task.por_history_id.map(Value::from).unwrap_or(Value::Null),
    );
    map.insert(
        "sla_deadline_unix".into(),
        task.sla_deadline_unix
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    map.insert(
        "scheduler_notes".into(),
        task.scheduler_notes
            .as_ref()
            .map(|notes| Value::from(notes.clone()))
            .unwrap_or(Value::Null),
    );
    map.insert(
        "slash_proposal_digest_hex".into(),
        task.slash_proposal_digest
            .map(|digest| Value::from(encode(digest)))
            .unwrap_or(Value::Null),
    );
    Value::Object(map)
}

fn repair_state_json(state: &RepairTaskStateV1) -> Value {
    let mut map = Map::new();
    match state {
        RepairTaskStateV1::Queued(queued) => {
            map.insert("state".into(), Value::from("queued"));
            map.insert("queued_at_unix".into(), Value::from(queued.queued_at_unix));
            map.insert(
                "sla_deadline_unix".into(),
                queued
                    .sla_deadline_unix
                    .map(Value::from)
                    .unwrap_or(Value::Null),
            );
        }
        RepairTaskStateV1::InProgress(in_progress) => {
            map.insert("state".into(), Value::from("in_progress"));
            map.insert(
                "queued_at_unix".into(),
                Value::from(in_progress.queued_at_unix),
            );
            map.insert(
                "started_at_unix".into(),
                Value::from(in_progress.started_at_unix),
            );
            map.insert(
                "repair_agent".into(),
                in_progress
                    .repair_agent
                    .as_ref()
                    .map(|agent| Value::from(agent.clone()))
                    .unwrap_or(Value::Null),
            );
        }
        RepairTaskStateV1::Completed(completed) => {
            map.insert("state".into(), Value::from("completed"));
            map.insert(
                "queued_at_unix".into(),
                Value::from(completed.queued_at_unix),
            );
            map.insert(
                "started_at_unix".into(),
                Value::from(completed.started_at_unix),
            );
            map.insert(
                "completed_at_unix".into(),
                Value::from(completed.completed_at_unix),
            );
            map.insert(
                "resolution_notes".into(),
                completed
                    .resolution_notes
                    .as_ref()
                    .map(|notes| Value::from(notes.clone()))
                    .unwrap_or(Value::Null),
            );
        }
        RepairTaskStateV1::Failed(failed) => {
            map.insert("state".into(), Value::from("failed"));
            map.insert("queued_at_unix".into(), Value::from(failed.queued_at_unix));
            map.insert("failed_at_unix".into(), Value::from(failed.failed_at_unix));
            map.insert("reason".into(), Value::from(failed.reason.clone()));
        }
        RepairTaskStateV1::Escalated(escalated) => {
            map.insert("state".into(), Value::from("escalated"));
            map.insert(
                "queued_at_unix".into(),
                Value::from(escalated.queued_at_unix),
            );
            map.insert(
                "escalated_at_unix".into(),
                Value::from(escalated.escalated_at_unix),
            );
            map.insert("reason".into(), Value::from(escalated.reason.clone()));
        }
    }
    Value::Object(map)
}

fn proof_stream_tier(tier: ProofStreamTier) -> &'static str {
    match tier {
        ProofStreamTier::Hot => "hot",
        ProofStreamTier::Warm => "warm",
        ProofStreamTier::Archive => "archive",
    }
}

fn potr_status(status: PotrStatus) -> &'static str {
    match status {
        PotrStatus::Success => "success",
        PotrStatus::MissedDeadline => "missed_deadline",
        PotrStatus::ProviderError => "provider_error",
        PotrStatus::GatewayError => "gateway_error",
        PotrStatus::ClientCancelled => "client_cancelled",
    }
}

fn governance_node_json(node: &GovernanceLogNodeV1, proof_digest: [u8; 32]) -> Value {
    let mut map = Map::new();
    map.insert("version".into(), Value::from(node.version));
    map.insert("node_cid_hex".into(), Value::from(encode(&node.node_cid)));
    map.insert(
        "prev_cid_hex".into(),
        node.prev_cid
            .as_ref()
            .map(|cid| Value::from(encode(cid)))
            .unwrap_or(Value::Null),
    );
    map.insert("timestamp".into(), Value::from(node.timestamp));
    map.insert(
        "publisher_peer_id".into(),
        Value::from(String::from_utf8_lossy(&node.publisher_peer_id).into_owned()),
    );
    map.insert("payload_kind".into(), Value::from("por_proof"));
    let mut sig = Map::new();
    let algorithm = match node.publisher_signature.algorithm {
        GovernanceSignatureAlgorithm::Ed25519 => "ed25519",
        GovernanceSignatureAlgorithm::Dilithium3 => "dilithium3",
    };
    sig.insert("algorithm".into(), Value::from(algorithm));
    sig.insert(
        "public_key_hex".into(),
        Value::from(encode(&node.publisher_signature.public_key)),
    );
    sig.insert(
        "signature_hex".into(),
        Value::from(encode(&node.publisher_signature.signature)),
    );
    map.insert("publisher_signature".into(), Value::Object(sig));
    map.insert(
        "embedded_proof_digest_hex".into(),
        Value::from(encode(proof_digest)),
    );
    Value::Object(map)
}

fn moderation_governance_node_json(node: &GovernanceLogNodeV1) -> Result<Value, Box<dyn Error>> {
    let GovernanceLogPayloadV1::ModerationBallotEvent(event) = &node.payload else {
        return Err("moderation fixture node must carry a moderation ballot event".into());
    };
    let mut map = Map::new();
    map.insert("version".into(), Value::from(node.version));
    map.insert("node_cid_hex".into(), Value::from(encode(&node.node_cid)));
    map.insert(
        "prev_cid_hex".into(),
        node.prev_cid
            .as_ref()
            .map(|cid| Value::from(encode(cid)))
            .unwrap_or(Value::Null),
    );
    map.insert("timestamp".into(), Value::from(node.timestamp));
    map.insert(
        "publisher_peer_id".into(),
        Value::from(String::from_utf8_lossy(&node.publisher_peer_id).into_owned()),
    );
    map.insert(
        "payload_kind".into(),
        Value::from("moderation_ballot_event"),
    );

    let mut event_map = Map::new();
    event_map.insert("version".into(), Value::from(event.version));
    event_map.insert("sequence".into(), Value::from(event.sequence));
    event_map.insert("kind".into(), Value::from(event.kind.as_str()));
    event_map.insert(
        "generated_at_unix_ms".into(),
        Value::from(event.generated_at_unix_ms),
    );
    event_map.insert("case_id".into(), Value::from(event.case_id.clone()));
    event_map.insert("round_id".into(), Value::from(event.round_id.clone()));
    event_map.insert(
        "juror_id".into(),
        event
            .juror_id
            .as_ref()
            .map(|juror_id| Value::from(juror_id.clone()))
            .unwrap_or(Value::Null),
    );
    event_map.insert("committed_count".into(), Value::from(event.committed_count));
    event_map.insert("revealed_count".into(), Value::from(event.revealed_count));
    event_map.insert("challenge_count".into(), Value::from(event.challenge_count));
    event_map.insert("challenge".into(), Value::Null);
    event_map.insert(
        "tally".into(),
        event
            .tally
            .as_ref()
            .map(|tally| {
                let mut tally_map = Map::new();
                tally_map.insert("case_id".into(), Value::from(tally.case_id.clone()));
                tally_map.insert("round_id".into(), Value::from(tally.round_id.clone()));
                let mut counts = Map::new();
                counts.insert("uphold".into(), Value::from(tally.counts.uphold));
                counts.insert("overturn".into(), Value::from(tally.counts.overturn));
                counts.insert("modify".into(), Value::from(tally.counts.modify));
                counts.insert("escalate".into(), Value::from(tally.counts.escalate));
                tally_map.insert("counts".into(), Value::Object(counts));
                tally_map.insert("votes_total".into(), Value::from(tally.votes_total));
                tally_map.insert("quorum".into(), Value::from(tally.quorum));
                tally_map.insert(
                    "winning_choice".into(),
                    tally
                        .winning_choice
                        .map(|choice| Value::from(choice.as_str()))
                        .unwrap_or(Value::Null),
                );
                tally_map.insert("contested".into(), Value::from(tally.contested));
                tally_map.insert(
                    "tallied_at_unix_ms".into(),
                    Value::from(tally.tallied_at_unix_ms),
                );
                Value::Object(tally_map)
            })
            .unwrap_or(Value::Null),
    );
    map.insert("moderation_event".into(), Value::Object(event_map));

    let mut signature = Map::new();
    signature.insert("algorithm".into(), Value::from("ed25519"));
    signature.insert(
        "public_key_hex".into(),
        Value::from(encode(&node.publisher_signature.public_key)),
    );
    signature.insert(
        "signature_hex".into(),
        Value::from(encode(&node.publisher_signature.signature)),
    );
    map.insert("publisher_signature".into(), Value::Object(signature));
    Ok(Value::Object(map))
}

fn governance_dag_block_json(block: &GovernanceDagBlockV1) -> Value {
    let mut map = Map::new();
    map.insert("version".into(), Value::from(block.version));
    map.insert(
        "block_cid_hex".into(),
        Value::from(encode(&block.block_cid)),
    );
    map.insert(
        "prev_block_cid_hex".into(),
        block
            .prev_block_cid
            .as_ref()
            .map(|cid| Value::from(encode(cid)))
            .unwrap_or(Value::Null),
    );
    map.insert("sequence".into(), Value::from(block.sequence));
    map.insert("timestamp".into(), Value::from(block.timestamp));
    map.insert(
        "publisher_peer_id".into(),
        Value::from(String::from_utf8_lossy(&block.publisher_peer_id).into_owned()),
    );
    map.insert(
        "node_cid_hex".into(),
        Value::from(encode(&block.node.node_cid)),
    );
    map.insert(
        "node_prev_cid_hex".into(),
        block
            .node
            .prev_cid
            .as_ref()
            .map(|cid| Value::from(encode(cid)))
            .unwrap_or(Value::Null),
    );
    map.insert("node_timestamp".into(), Value::from(block.node.timestamp));
    map.insert(
        "node_publisher_peer_id".into(),
        Value::from(String::from_utf8_lossy(&block.node.publisher_peer_id).into_owned()),
    );
    let node_signature_algorithm = match block.node.publisher_signature.algorithm {
        GovernanceSignatureAlgorithm::Ed25519 => "ed25519",
        GovernanceSignatureAlgorithm::Dilithium3 => "dilithium3",
    };
    map.insert(
        "node_signature_algorithm".into(),
        Value::from(node_signature_algorithm),
    );
    map.insert(
        "node_signature_public_key_hex".into(),
        Value::from(encode(&block.node.publisher_signature.public_key)),
    );
    map.insert(
        "node_signature_hex".into(),
        Value::from(encode(&block.node.publisher_signature.signature)),
    );
    map.insert("signature_algorithm".into(), Value::from("ed25519"));
    map.insert(
        "signature_public_key_hex".into(),
        Value::from(encode(&block.block_signature.public_key)),
    );
    map.insert(
        "signature_hex".into(),
        Value::from(encode(&block.block_signature.signature)),
    );
    Value::Object(map)
}

fn governance_dag_head_json(head: &GovernanceDagHeadV1) -> Value {
    let mut map = Map::new();
    map.insert("version".into(), Value::from(head.version));
    map.insert(
        "head_block_cid_hex".into(),
        Value::from(encode(&head.head_block_cid)),
    );
    map.insert("block_count".into(), Value::from(head.block_count));
    map.insert("generated_at".into(), Value::from(head.generated_at));
    map.insert(
        "publisher_peer_id".into(),
        Value::from(String::from_utf8_lossy(&head.publisher_peer_id).into_owned()),
    );
    map.insert(
        "checkpoint_cid_hex".into(),
        head.checkpoint_cid
            .as_ref()
            .map(|cid| Value::from(encode(cid)))
            .unwrap_or(Value::Null),
    );
    map.insert("signature_algorithm".into(), Value::from("ed25519"));
    map.insert(
        "signature_public_key_hex".into(),
        Value::from(encode(&head.head_signature.public_key)),
    );
    map.insert(
        "signature_hex".into(),
        Value::from(encode(&head.head_signature.signature)),
    );
    Value::Object(map)
}
