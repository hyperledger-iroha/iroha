//! Generates PoR, PoTR, repair, and governance DAG fixtures.
//!
//! Pass exactly one of `--write` or `--check`. The write mode transactionally
//! publishes the closed fixture set; check mode performs no managed writes.

use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    error::Error,
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
#[cfg(windows)]
use std::os::windows::fs::MetadataExt as _;

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
const FIXTURES_ROOT: &str = "fixtures/sorafs_manifest";
const MANAGED_DIRECTORIES: [&str; 6] = [
    "governance",
    "moderation",
    "por",
    "potr",
    "reference_sdk",
    "repair",
];
const INPUT_DIRECTORIES: [&str; 5] = [
    "appeal_finance",
    "orderbook",
    "pdp",
    "provider_admission",
    "replication_order",
];
const ROOT_INVENTORY: &str = "reference_sdk_validation_inventory_v1.json";
const PUBLICATION_LOCK: &str = ".generate_por_fixtures.lock";
// Closed-set tripwire: changing the generator's path inventory must be an
// explicit source change, never an accidental side effect of regeneration.
const EXPECTED_MANAGED_FIXTURE_COUNT: usize = 53;
const MAX_FIXTURE_BYTES: u64 = 8 << 20;
const MAX_TOTAL_FIXTURE_BYTES: u64 = 64 << 20;
const MAX_PATH_BYTES: usize = 4 << 10;
const MAX_PATH_COMPONENTS: usize = 64;
const MAX_TEMP_ATTEMPTS: u64 = 64;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Check,
    Write,
}

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
    let mode = parse_mode(env::args_os().skip(1))?;
    let fixtures_root = PathBuf::from(FIXTURES_ROOT);
    if mode == Mode::Check {
        ensure_no_publication_lock(&fixtures_root)?;
    }
    let rendered = render_managed_fixtures(&fixtures_root)?;
    match mode {
        Mode::Check => {
            ensure_generation_inputs_match(&fixtures_root, &rendered.inputs)?;
            check_managed_fixtures(&fixtures_root, &rendered.managed)?;
            ensure_generation_inputs_match(&fixtures_root, &rendered.inputs)?;
            ensure_no_publication_lock(&fixtures_root)
        }
        Mode::Write => {
            publish_managed_fixtures(&fixtures_root, &rendered.managed, &rendered.inputs)
        }
    }
}

fn ensure_no_publication_lock(fixtures_root: &Path) -> Result<(), Box<dyn Error>> {
    match fs::symlink_metadata(fixtures_root.join(PUBLICATION_LOCK)) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(format!(
            "fixture verification is blocked by publication lock `{PUBLICATION_LOCK}`"
        )
        .into()),
        Err(error) => Err(error.into()),
    }
}

fn parse_mode(args: impl IntoIterator<Item = OsString>) -> Result<Mode, Box<dyn Error>> {
    let args = args.into_iter().collect::<Vec<_>>();
    let usage = "usage: generate_por_fixtures (--write | --check) (exactly one mode is required)";
    if args.len() != 1 {
        return Err(usage.into());
    }
    match args[0].to_str() {
        Some("--write") => Ok(Mode::Write),
        Some("--check") => Ok(Mode::Check),
        Some(argument) => Err(format!("{usage}; unrecognized argument `{argument}`").into()),
        None => Err(format!("{usage}; arguments must be valid UTF-8").into()),
    }
}

struct RenderedFixtureSet {
    managed: BTreeMap<PathBuf, Vec<u8>>,
    inputs: BTreeMap<PathBuf, Vec<u8>>,
}

fn render_managed_fixtures(fixtures_root: &Path) -> Result<RenderedFixtureSet, Box<dyn Error>> {
    ensure_existing_real_directory(fixtures_root)?;
    let fixtures_parent = fixtures_root
        .parent()
        .ok_or("fixture root must have a parent directory")?;
    ensure_existing_real_directory(fixtures_parent)?;
    let inputs = read_generation_input_map(fixtures_root)?;
    let staging = TemporaryDirectory::create(fixtures_parent, ".generate_por_fixtures.render")?;
    for directory in INPUT_DIRECTORIES {
        ensure_real_directory(&staging.path().join(directory))?;
    }
    for (relative, bytes) in &inputs {
        let destination = staging.path().join(relative);
        if let Some(parent) = destination.parent() {
            ensure_real_directory(parent)?;
        }
        write_new_regular_file(&destination, bytes)?;
    }
    generate_fixtures(staging.path())?;
    ensure_generation_inputs_match(fixtures_root, &inputs)?;
    let managed = read_rendered_fixture_map(staging.path())?;
    validate_fixture_map(&managed)?;
    Ok(RenderedFixtureSet { managed, inputs })
}

fn generate_fixtures(fixtures_root: &Path) -> Result<(), Box<dyn Error>> {
    let por_dir = fixtures_root.join("por");
    let potr_dir = fixtures_root.join("potr");
    let repair_dir = fixtures_root.join("repair");
    let gov_dir = fixtures_root.join("governance");
    let moderation_dir = fixtures_root.join("moderation");
    ensure_real_directory(&por_dir)?;
    ensure_real_directory(&potr_dir)?;
    ensure_real_directory(&repair_dir)?;
    ensure_real_directory(&repair_dir.join("negative"))?;
    ensure_real_directory(&gov_dir)?;
    ensure_real_directory(&moderation_dir)?;

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
    write_new_regular_file(
        &gov_dir.join("dag_head_validation_outcome_v1.json"),
        format!("{}\n", to_string_pretty(&outcome)?).as_bytes(),
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
    write_new_regular_file(
        &gov_dir.join("dag_block_trailing_bytes_v1.to"),
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

fn validate_fixture_map(fixtures: &BTreeMap<PathBuf, Vec<u8>>) -> Result<(), Box<dyn Error>> {
    if fixtures.len() != EXPECTED_MANAGED_FIXTURE_COUNT {
        return Err(format!(
            "managed fixture count must remain exactly {EXPECTED_MANAGED_FIXTURE_COUNT}, got {}",
            fixtures.len()
        )
        .into());
    }
    if !fixtures.contains_key(Path::new(ROOT_INVENTORY)) {
        return Err(format!("managed fixtures must include `{ROOT_INVENTORY}`").into());
    }
    for directory in MANAGED_DIRECTORIES {
        if !fixtures.keys().any(|path| path.starts_with(directory)) {
            return Err(format!(
                "managed fixtures must include at least one output beneath `{directory}`"
            )
            .into());
        }
    }
    let mut total_bytes = 0_u64;
    for (relative, bytes) in fixtures {
        validate_managed_relative_path(relative)?;
        let byte_length = u64::try_from(bytes.len())?;
        if byte_length > MAX_FIXTURE_BYTES {
            return Err(format!(
                "managed fixture `{}` exceeds the {MAX_FIXTURE_BYTES}-byte bound",
                relative.display()
            )
            .into());
        }
        total_bytes = total_bytes
            .checked_add(byte_length)
            .ok_or("managed fixture byte total overflowed")?;
    }
    if total_bytes > MAX_TOTAL_FIXTURE_BYTES {
        return Err(format!(
            "managed fixtures exceed the {MAX_TOTAL_FIXTURE_BYTES}-byte aggregate bound"
        )
        .into());
    }
    Ok(())
}

fn validate_managed_relative_path(path: &Path) -> Result<(), Box<dyn Error>> {
    use std::path::Component;

    if path.as_os_str().len() > MAX_PATH_BYTES || path.is_absolute() {
        return Err(format!(
            "managed fixture path `{}` must be bounded and relative",
            path.display()
        )
        .into());
    }
    let components = path.components().collect::<Vec<_>>();
    if components.is_empty()
        || components.len() > 4
        || components
            .iter()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!(
            "managed fixture path `{}` contains traversal or lies outside the closed layout",
            path.display()
        )
        .into());
    }
    if path == Path::new(ROOT_INVENTORY) {
        return Ok(());
    }
    let Some(Component::Normal(first)) = path.components().next() else {
        return Err("managed fixture path lacks a normal first component".into());
    };
    if !MANAGED_DIRECTORIES
        .iter()
        .any(|directory| first == std::ffi::OsStr::new(directory))
        || !matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("json" | "to")
        )
    {
        return Err(format!(
            "managed fixture path `{}` lies outside the closed generated layout",
            path.display()
        )
        .into());
    }
    Ok(())
}

fn read_rendered_fixture_map(
    staging_root: &Path,
) -> Result<BTreeMap<PathBuf, Vec<u8>>, Box<dyn Error>> {
    let expected_top_level = INPUT_DIRECTORIES
        .into_iter()
        .chain(MANAGED_DIRECTORIES)
        .map(OsString::from)
        .chain([OsString::from(ROOT_INVENTORY)])
        .collect::<BTreeSet<_>>();
    let actual_top_level = sorted_directory_entries(staging_root)?
        .into_iter()
        .map(|entry| entry.file_name())
        .collect::<BTreeSet<_>>();
    if actual_top_level != expected_top_level {
        return Err(format!(
            "fixture renderer produced an unexpected top-level layout (expected={expected_top_level:?}, actual={actual_top_level:?})"
        )
        .into());
    }

    let mut fixtures = BTreeMap::new();
    for directory in MANAGED_DIRECTORIES {
        collect_rendered_files(staging_root, &staging_root.join(directory), &mut fixtures)?;
    }
    let inventory_path = staging_root.join(ROOT_INVENTORY);
    fixtures.insert(
        PathBuf::from(ROOT_INVENTORY),
        read_regular_file(&inventory_path)?,
    );
    Ok(fixtures)
}

fn collect_rendered_files(
    root: &Path,
    directory: &Path,
    fixtures: &mut BTreeMap<PathBuf, Vec<u8>>,
) -> Result<(), Box<dyn Error>> {
    ensure_existing_real_directory(directory)?;
    for entry in sorted_directory_entries(directory)? {
        let path = entry.path();
        let relative = path.strip_prefix(root)?.to_path_buf();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "rendered fixture entry `{}` must not be a symlink",
                relative.display()
            )
            .into());
        }
        if metadata.is_dir() {
            collect_rendered_files(root, &path, fixtures)?;
        } else if metadata.is_file() {
            ensure_single_hard_link(&metadata, &path)?;
            validate_managed_relative_path(&relative)?;
            if fixtures
                .insert(relative.clone(), read_regular_file(&path)?)
                .is_some()
            {
                return Err(format!(
                    "renderer produced duplicate fixture path `{}`",
                    relative.display()
                )
                .into());
            }
        } else {
            return Err(format!(
                "rendered fixture entry `{}` must be a regular file or directory",
                relative.display()
            )
            .into());
        }
    }
    Ok(())
}

fn check_managed_fixtures(
    fixtures_root: &Path,
    fixtures: &BTreeMap<PathBuf, Vec<u8>>,
) -> Result<(), Box<dyn Error>> {
    validate_fixture_map(fixtures)?;
    let expected_paths = fixtures.keys().cloned().collect::<BTreeSet<_>>();
    let actual_paths = scan_managed_fixture_paths(fixtures_root, &expected_paths)?;
    if actual_paths != expected_paths {
        let missing = expected_paths
            .difference(&actual_paths)
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>();
        return Err(format!("managed fixture path set differs; missing={missing:?}").into());
    }

    let mut differing = Vec::new();
    for (relative, expected) in fixtures {
        if read_regular_file(&fixtures_root.join(relative))? != *expected {
            differing.push(relative.display().to_string());
        }
    }
    if !differing.is_empty() {
        return Err(format!(
            "managed fixture bytes differ from deterministic generation: {differing:?}"
        )
        .into());
    }
    let final_paths = scan_managed_fixture_paths(fixtures_root, &expected_paths)?;
    if final_paths != actual_paths {
        return Err("managed fixture layout changed during verification".into());
    }
    Ok(())
}

fn scan_managed_fixture_paths(
    fixtures_root: &Path,
    expected_paths: &BTreeSet<PathBuf>,
) -> Result<BTreeSet<PathBuf>, Box<dyn Error>> {
    ensure_existing_real_directory(fixtures_root)?;
    let expected_directories = expected_fixture_directories(expected_paths);
    let mut actual_paths = BTreeSet::new();
    let mut unexpected = BTreeSet::new();
    for directory in MANAGED_DIRECTORIES {
        let path = fixtures_root.join(directory);
        match fs::symlink_metadata(&path) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(format!(
                        "managed fixture root `{directory}` must be a real directory"
                    )
                    .into());
                }
                scan_managed_directory(
                    fixtures_root,
                    &path,
                    expected_paths,
                    &expected_directories,
                    &mut actual_paths,
                    &mut unexpected,
                )?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }

    let inventory = fixtures_root.join(ROOT_INVENTORY);
    match fs::symlink_metadata(&inventory) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(format!(
                    "managed root inventory `{ROOT_INVENTORY}` must be a regular non-symlink file"
                )
                .into());
            }
            ensure_single_hard_link(&metadata, &inventory)?;
            actual_paths.insert(PathBuf::from(ROOT_INVENTORY));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    if !unexpected.is_empty() {
        return Err(format!(
            "managed fixture directories contain unexpected entries: {:?}",
            unexpected
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
        )
        .into());
    }
    Ok(actual_paths)
}

fn expected_fixture_directories(expected_paths: &BTreeSet<PathBuf>) -> BTreeSet<PathBuf> {
    let mut directories = MANAGED_DIRECTORIES
        .into_iter()
        .map(PathBuf::from)
        .collect::<BTreeSet<_>>();
    for path in expected_paths {
        let mut parent = path.parent();
        while let Some(directory) = parent {
            if directory.as_os_str().is_empty() {
                break;
            }
            directories.insert(directory.to_path_buf());
            parent = directory.parent();
        }
    }
    directories
}

#[allow(clippy::too_many_arguments)]
fn scan_managed_directory(
    fixtures_root: &Path,
    directory: &Path,
    expected_paths: &BTreeSet<PathBuf>,
    expected_directories: &BTreeSet<PathBuf>,
    actual_paths: &mut BTreeSet<PathBuf>,
    unexpected: &mut BTreeSet<PathBuf>,
) -> Result<(), Box<dyn Error>> {
    for entry in sorted_directory_entries(directory)? {
        let path = entry.path();
        let relative = path.strip_prefix(fixtures_root)?.to_path_buf();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "managed fixture entry `{}` must not be a symlink",
                relative.display()
            )
            .into());
        }
        if metadata.is_dir() {
            if expected_directories.contains(&relative) {
                scan_managed_directory(
                    fixtures_root,
                    &path,
                    expected_paths,
                    expected_directories,
                    actual_paths,
                    unexpected,
                )?;
            } else {
                unexpected.insert(relative);
            }
        } else if metadata.is_file() {
            ensure_single_hard_link(&metadata, &path)?;
            if is_managed_readme(&relative) {
                continue;
            }
            if expected_paths.contains(&relative) {
                actual_paths.insert(relative);
            } else {
                unexpected.insert(relative);
            }
        } else {
            return Err(format!(
                "managed fixture entry `{}` must be a regular file or directory",
                relative.display()
            )
            .into());
        }
    }
    Ok(())
}

fn is_managed_readme(relative: &Path) -> bool {
    MANAGED_DIRECTORIES
        .iter()
        .any(|directory| relative == Path::new(directory).join("README.md"))
}

fn read_generation_input_map(
    fixtures_root: &Path,
) -> Result<BTreeMap<PathBuf, Vec<u8>>, Box<dyn Error>> {
    let mut inputs = BTreeMap::new();
    for directory in INPUT_DIRECTORIES {
        collect_generation_inputs(fixtures_root, &fixtures_root.join(directory), &mut inputs)?;
    }
    let mut total_bytes = 0_u64;
    for bytes in inputs.values() {
        total_bytes = total_bytes
            .checked_add(u64::try_from(bytes.len())?)
            .ok_or("generation input byte total overflowed")?;
    }
    if total_bytes > MAX_TOTAL_FIXTURE_BYTES {
        return Err(format!(
            "generation inputs exceed the {MAX_TOTAL_FIXTURE_BYTES}-byte aggregate bound"
        )
        .into());
    }
    Ok(inputs)
}

fn collect_generation_inputs(
    fixtures_root: &Path,
    directory: &Path,
    inputs: &mut BTreeMap<PathBuf, Vec<u8>>,
) -> Result<(), Box<dyn Error>> {
    ensure_existing_real_directory(directory)?;
    for entry in sorted_directory_entries(directory)? {
        let path = entry.path();
        let relative = path.strip_prefix(fixtures_root)?.to_path_buf();
        validate_output_path(&relative)?;
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "generation input `{}` must not be a symlink",
                relative.display()
            )
            .into());
        }
        if metadata.is_dir() {
            collect_generation_inputs(fixtures_root, &path, inputs)?;
        } else if metadata.is_file() {
            ensure_single_hard_link(&metadata, &path)?;
            if inputs
                .insert(relative.clone(), read_regular_file(&path)?)
                .is_some()
            {
                return Err(format!("duplicate generation input `{}`", relative.display()).into());
            }
        } else {
            return Err(format!(
                "generation input `{}` must be a regular file or directory",
                relative.display()
            )
            .into());
        }
    }
    Ok(())
}

fn ensure_generation_inputs_match(
    fixtures_root: &Path,
    expected: &BTreeMap<PathBuf, Vec<u8>>,
) -> Result<(), Box<dyn Error>> {
    let actual = read_generation_input_map(fixtures_root)?;
    if &actual == expected {
        Ok(())
    } else {
        Err("generation inputs changed while rendering or publishing fixtures".into())
    }
}

fn sorted_directory_entries(directory: &Path) -> Result<Vec<fs::DirEntry>, Box<dyn Error>> {
    let mut entries = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);
    Ok(entries)
}

fn ensure_real_directory(path: &Path) -> Result<(), Box<dyn Error>> {
    validate_output_path(path)?;
    if path.as_os_str().is_empty() {
        return Ok(());
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) => ensure_directory_metadata(&metadata, path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = path
                .parent()
                .ok_or("fixture directory must have a parent")?;
            ensure_real_directory(parent)?;
            match fs::create_dir(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
            let metadata = fs::symlink_metadata(path)?;
            ensure_directory_metadata(&metadata, path)?;
            sync_directory(parent)
        }
        Err(error) => Err(error.into()),
    }
}

fn ensure_existing_real_directory(path: &Path) -> Result<(), Box<dyn Error>> {
    validate_output_path(path)?;
    if path.as_os_str().is_empty() {
        return Ok(());
    }
    if let Some(parent) = path.parent()
        && parent != path
        && !parent.as_os_str().is_empty()
    {
        ensure_existing_real_directory(parent)?;
    }
    let metadata = fs::symlink_metadata(path)?;
    ensure_directory_metadata(&metadata, path)
}

fn validate_output_path(path: &Path) -> Result<(), Box<dyn Error>> {
    if path.as_os_str().len() > MAX_PATH_BYTES {
        return Err(format!("path `{}` exceeds the byte bound", path.display()).into());
    }
    let mut component_count = 0_usize;
    for component in path.components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err(format!("path `{}` contains parent traversal", path.display()).into());
        }
        component_count += 1;
    }
    if component_count > MAX_PATH_COMPONENTS {
        return Err(format!("path `{}` exceeds the component bound", path.display()).into());
    }
    Ok(())
}

fn ensure_directory_metadata(metadata: &fs::Metadata, path: &Path) -> Result<(), Box<dyn Error>> {
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!("`{}` must be a real directory", path.display()).into());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileIdentity {
    volume: u64,
    file: u64,
}

#[cfg(unix)]
fn file_identity(metadata: &fs::Metadata, _path: &Path) -> Result<FileIdentity, Box<dyn Error>> {
    Ok(FileIdentity {
        volume: metadata.dev(),
        file: metadata.ino(),
    })
}

#[cfg(windows)]
fn file_identity(metadata: &fs::Metadata, path: &Path) -> Result<FileIdentity, Box<dyn Error>> {
    Ok(FileIdentity {
        volume: u64::from(
            metadata.volume_serial_number().ok_or_else(|| {
                format!("could not read volume identity for `{}`", path.display())
            })?,
        ),
        file: metadata
            .file_index()
            .ok_or_else(|| format!("could not read file identity for `{}`", path.display()))?,
    })
}

#[cfg(not(any(unix, windows)))]
fn file_identity(_metadata: &fs::Metadata, path: &Path) -> Result<FileIdentity, Box<dyn Error>> {
    Err(format!(
        "fixture publication cannot authenticate file identity on this platform: `{}`",
        path.display()
    )
    .into())
}

#[cfg(unix)]
fn ensure_single_hard_link(metadata: &fs::Metadata, path: &Path) -> Result<(), Box<dyn Error>> {
    if metadata.nlink() != 1 {
        return Err(format!(
            "fixture file `{}` must have exactly one hard link",
            path.display()
        )
        .into());
    }
    Ok(())
}

#[cfg(windows)]
fn ensure_single_hard_link(metadata: &fs::Metadata, path: &Path) -> Result<(), Box<dyn Error>> {
    if metadata.number_of_links() != Some(1) {
        return Err(format!(
            "fixture file `{}` must have exactly one hard link",
            path.display()
        )
        .into());
    }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn ensure_single_hard_link(_metadata: &fs::Metadata, path: &Path) -> Result<(), Box<dyn Error>> {
    Err(format!(
        "fixture publication cannot authenticate hard links on this platform: `{}`",
        path.display()
    )
    .into())
}

#[derive(Clone)]
struct FileSnapshot {
    identity: FileIdentity,
    bytes: Vec<u8>,
}

fn read_regular_file(path: &Path) -> Result<Vec<u8>, Box<dyn Error>> {
    Ok(read_regular_file_snapshot(path)?.bytes)
}

fn read_regular_file_snapshot(path: &Path) -> Result<FileSnapshot, Box<dyn Error>> {
    let before = fs::symlink_metadata(path)?;
    validate_regular_metadata(&before, path)?;
    if before.len() > MAX_FIXTURE_BYTES {
        return Err(format!(
            "fixture file `{}` exceeds the {MAX_FIXTURE_BYTES}-byte bound",
            path.display()
        )
        .into());
    }
    let before_identity = file_identity(&before, path)?;
    let file = File::open(path)?;
    let opened = file.metadata()?;
    validate_regular_metadata(&opened, path)?;
    if file_identity(&opened, path)? != before_identity {
        return Err(format!(
            "fixture file `{}` changed identity while opening",
            path.display()
        )
        .into());
    }
    let mut bytes = Vec::with_capacity(usize::try_from(opened.len())?);
    file.take(MAX_FIXTURE_BYTES + 1).read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len())? > MAX_FIXTURE_BYTES
        || u64::try_from(bytes.len())? != opened.len()
    {
        return Err(format!(
            "fixture file `{}` changed length while reading",
            path.display()
        )
        .into());
    }
    let after = fs::symlink_metadata(path)?;
    validate_regular_metadata(&after, path)?;
    if file_identity(&after, path)? != before_identity || after.len() != opened.len() {
        return Err(format!("fixture file `{}` changed while reading", path.display()).into());
    }
    Ok(FileSnapshot {
        identity: before_identity,
        bytes,
    })
}

fn validate_regular_metadata(metadata: &fs::Metadata, path: &Path) -> Result<(), Box<dyn Error>> {
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "fixture file `{}` must be a regular non-symlink file",
            path.display()
        )
        .into());
    }
    ensure_single_hard_link(metadata, path)
}

fn write_new_regular_file(path: &Path, bytes: &[u8]) -> Result<FileIdentity, Box<dyn Error>> {
    if u64::try_from(bytes.len())? > MAX_FIXTURE_BYTES {
        return Err(format!(
            "fixture file `{}` exceeds the {MAX_FIXTURE_BYTES}-byte bound",
            path.display()
        )
        .into());
    }
    let parent = path
        .parent()
        .ok_or("fixture output file must have a parent")?;
    ensure_existing_real_directory(parent)?;
    let parent_before = file_identity(&fs::symlink_metadata(parent)?, parent)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o644);
    let mut file = options.open(path)?;
    #[cfg(unix)]
    file.set_permissions(fs::Permissions::from_mode(0o644))?;
    let created = file.metadata()?;
    validate_regular_metadata(&created, path)?;
    let identity = file_identity(&created, path)?;
    let mut cleanup = TemporaryFileGuard::new(path.to_path_buf(), identity);
    file.write_all(bytes)?;
    file.sync_all()?;
    let metadata = file.metadata()?;
    validate_regular_metadata(&metadata, path)?;
    if file_identity(&metadata, path)? != identity {
        return Err(format!(
            "fixture output `{}` changed identity while writing",
            path.display()
        )
        .into());
    }
    drop(file);
    let parent_after = file_identity(&fs::symlink_metadata(parent)?, parent)?;
    if parent_after != parent_before {
        return Err(format!(
            "fixture output parent `{}` changed during publication",
            parent.display()
        )
        .into());
    }
    let published = fs::symlink_metadata(path)?;
    validate_regular_metadata(&published, path)?;
    if file_identity(&published, path)? != identity {
        return Err(format!(
            "fixture output `{}` changed identity after writing",
            path.display()
        )
        .into());
    }
    sync_directory(parent)?;
    cleanup.disarm();
    Ok(identity)
}

fn sync_directory(path: &Path) -> Result<(), Box<dyn Error>> {
    File::open(path)?.sync_all()?;
    Ok(())
}

struct TemporaryFileGuard {
    path: Option<PathBuf>,
    identity: FileIdentity,
}

impl TemporaryFileGuard {
    fn new(path: PathBuf, identity: FileIdentity) -> Self {
        Self {
            path: Some(path),
            identity,
        }
    }

    fn disarm(&mut self) {
        self.path = None;
    }
}

impl Drop for TemporaryFileGuard {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                return;
            };
            if metadata.is_file()
                && !metadata.file_type().is_symlink()
                && file_identity(&metadata, &path).ok() == Some(self.identity)
                && fs::remove_file(&path).is_ok()
                && let Some(parent) = path.parent()
            {
                let _ = sync_directory(parent);
            }
        }
    }
}

struct TemporaryDirectory {
    path: PathBuf,
    parent: PathBuf,
    identity: FileIdentity,
    cleanup: bool,
}

impl TemporaryDirectory {
    fn create(parent: &Path, prefix: &str) -> Result<Self, Box<dyn Error>> {
        ensure_existing_real_directory(parent)?;
        for _ in 0..MAX_TEMP_ATTEMPTS {
            let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!("{prefix}.{}.{}", std::process::id(), sequence));
            match fs::create_dir(&path) {
                Ok(()) => {
                    let metadata = fs::symlink_metadata(&path)?;
                    ensure_directory_metadata(&metadata, &path)?;
                    let identity = file_identity(&metadata, &path)?;
                    sync_directory(parent)?;
                    return Ok(Self {
                        path,
                        parent: parent.to_path_buf(),
                        identity,
                        cleanup: true,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Err(format!("could not allocate private fixture staging directory for `{prefix}`").into())
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn preserve(&mut self) {
        self.cleanup = false;
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        if !self.cleanup {
            return;
        }
        let Ok(metadata) = fs::symlink_metadata(&self.path) else {
            return;
        };
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || file_identity(&metadata, &self.path).ok() != Some(self.identity)
        {
            return;
        }
        if fs::remove_dir_all(&self.path).is_ok() {
            let _ = sync_directory(&self.parent);
        }
    }
}

struct PublicationLock {
    path: PathBuf,
    parent: PathBuf,
    identity: FileIdentity,
    release: bool,
}

impl PublicationLock {
    fn acquire(fixtures_root: &Path) -> Result<Self, Box<dyn Error>> {
        ensure_existing_real_directory(fixtures_root)?;
        let path = fixtures_root.join(PUBLICATION_LOCK);
        let identity =
            write_new_regular_file(&path, format!("pid={}\n", std::process::id()).as_bytes())
                .map_err(|error| {
                    format!(
                        "could not acquire exclusive fixture publication lock `{}`: {error}",
                        path.display()
                    )
                })?;
        Ok(Self {
            path,
            parent: fixtures_root.to_path_buf(),
            identity,
            release: true,
        })
    }

    fn preserve(&mut self) {
        self.release = false;
    }
}

impl Drop for PublicationLock {
    fn drop(&mut self) {
        if !self.release {
            return;
        }
        let Ok(metadata) = fs::symlink_metadata(&self.path) else {
            return;
        };
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || ensure_single_hard_link(&metadata, &self.path).is_err()
            || file_identity(&metadata, &self.path).ok() != Some(self.identity)
        {
            return;
        }
        if fs::remove_file(&self.path).is_ok() {
            let _ = sync_directory(&self.parent);
        }
    }
}

struct CreatedDirectoryGuard {
    directories: Vec<PathBuf>,
    committed: bool,
}

impl CreatedDirectoryGuard {
    fn create(
        fixtures_root: &Path,
        expected_paths: &BTreeSet<PathBuf>,
    ) -> Result<Self, Box<dyn Error>> {
        let mut expected_directories = expected_fixture_directories(expected_paths)
            .into_iter()
            .collect::<Vec<_>>();
        expected_directories.sort_by_key(|path| path.components().count());
        let mut created = Vec::new();
        for relative in expected_directories {
            validate_managed_directory_path(&relative)?;
            let path = fixtures_root.join(relative);
            match fs::symlink_metadata(&path) {
                Ok(metadata) => ensure_directory_metadata(&metadata, &path)?,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    let parent = path
                        .parent()
                        .ok_or("managed fixture directory must have a parent")?;
                    ensure_existing_real_directory(parent)?;
                    fs::create_dir(&path)?;
                    let metadata = fs::symlink_metadata(&path)?;
                    ensure_directory_metadata(&metadata, &path)?;
                    sync_directory(parent)?;
                    created.push(path);
                }
                Err(error) => return Err(error.into()),
            }
        }
        Ok(Self {
            directories: created,
            committed: false,
        })
    }

    fn commit(&mut self) {
        self.committed = true;
    }
}

impl Drop for CreatedDirectoryGuard {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        for directory in self.directories.iter().rev() {
            if fs::remove_dir(directory).is_ok()
                && let Some(parent) = directory.parent()
            {
                let _ = sync_directory(parent);
            }
        }
    }
}

fn validate_managed_directory_path(path: &Path) -> Result<(), Box<dyn Error>> {
    use std::path::Component;

    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err("managed fixture directory must be non-empty and relative".into());
    }
    let components = path.components().collect::<Vec<_>>();
    if components.len() > 3
        || components
            .iter()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!(
            "managed fixture directory `{}` lies outside the closed layout",
            path.display()
        )
        .into());
    }
    let Some(Component::Normal(first)) = path.components().next() else {
        return Err("managed fixture directory lacks a normal first component".into());
    };
    if !MANAGED_DIRECTORIES
        .iter()
        .any(|directory| first == std::ffi::OsStr::new(directory))
    {
        return Err(format!(
            "managed fixture directory `{}` lies outside a generated root",
            path.display()
        )
        .into());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PublicationState {
    Prepared,
    OriginalMoved,
    Published,
}

struct PublicationEntry {
    relative: PathBuf,
    expected: Vec<u8>,
    original: Option<FileSnapshot>,
    staged_path: PathBuf,
    backup_path: PathBuf,
    staged_identity: FileIdentity,
    state: PublicationState,
}

struct PublicationRollbackGuard<'a> {
    fixtures_root: &'a Path,
    entries: &'a mut [PublicationEntry],
    transaction: &'a mut TemporaryDirectory,
    publication_lock: &'a mut PublicationLock,
    committed: bool,
}

impl<'a> PublicationRollbackGuard<'a> {
    fn new(
        fixtures_root: &'a Path,
        entries: &'a mut [PublicationEntry],
        transaction: &'a mut TemporaryDirectory,
        publication_lock: &'a mut PublicationLock,
    ) -> Self {
        Self {
            fixtures_root,
            entries,
            transaction,
            publication_lock,
            committed: false,
        }
    }

    fn entries_mut(&mut self) -> &mut [PublicationEntry] {
        self.entries
    }

    fn rollback(&mut self) -> Result<(), Box<dyn Error>> {
        let result = rollback_entries(self.fixtures_root, self.entries);
        self.committed = true;
        if result.is_err() {
            self.transaction.preserve();
            self.publication_lock.preserve();
        }
        result
    }

    fn commit(&mut self) {
        self.committed = true;
    }

    fn recovery_path(&self) -> &Path {
        self.transaction.path()
    }
}

impl Drop for PublicationRollbackGuard<'_> {
    fn drop(&mut self) {
        if !self.committed && rollback_entries(self.fixtures_root, self.entries).is_err() {
            self.transaction.preserve();
            self.publication_lock.preserve();
        }
    }
}

fn publish_managed_fixtures(
    fixtures_root: &Path,
    fixtures: &BTreeMap<PathBuf, Vec<u8>>,
    generation_inputs: &BTreeMap<PathBuf, Vec<u8>>,
) -> Result<(), Box<dyn Error>> {
    validate_fixture_map(fixtures)?;
    let mut publication_lock = PublicationLock::acquire(fixtures_root)?;
    ensure_generation_inputs_match(fixtures_root, generation_inputs)?;
    let expected_paths = fixtures.keys().cloned().collect::<BTreeSet<_>>();
    let mut created_directories = CreatedDirectoryGuard::create(fixtures_root, &expected_paths)?;
    let actual_paths = scan_managed_fixture_paths(fixtures_root, &expected_paths)?;
    let mut originals = BTreeMap::new();
    for relative in &actual_paths {
        originals.insert(
            relative.clone(),
            read_regular_file_snapshot(&fixtures_root.join(relative))?,
        );
    }

    let mut changed = fixtures
        .iter()
        .filter(|(relative, expected)| {
            originals
                .get(*relative)
                .is_none_or(|snapshot| snapshot.bytes.as_slice() != expected.as_slice())
        })
        .map(|(relative, expected)| (relative.clone(), expected.clone()))
        .collect::<Vec<_>>();
    // The signed release-wide inventory is the commit marker. Publishing it
    // last makes interruption fail closed under the old bindings; rollback
    // visits it first before restoring any earlier payload.
    changed.sort_by_key(|(relative, _)| relative == Path::new(ROOT_INVENTORY));
    if changed.is_empty() {
        check_managed_fixtures(fixtures_root, fixtures)?;
        ensure_generation_inputs_match(fixtures_root, generation_inputs)?;
        created_directories.commit();
        return Ok(());
    }

    let fixtures_parent = fixtures_root
        .parent()
        .ok_or("fixture root must have a parent directory")?;
    let mut transaction =
        TemporaryDirectory::create(fixtures_parent, ".generate_por_fixtures.publish")?;
    let new_dir = transaction.path().join("new");
    let old_dir = transaction.path().join("old");
    ensure_real_directory(&new_dir)?;
    ensure_real_directory(&old_dir)?;

    let mut entries = Vec::with_capacity(changed.len());
    for (index, (relative, expected)) in changed.into_iter().enumerate() {
        let staged_path = new_dir.join(format!("{index:04}.fixture"));
        let backup_path = old_dir.join(format!("{index:04}.fixture"));
        let staged_identity = write_new_regular_file(&staged_path, &expected)?;
        entries.push(PublicationEntry {
            original: originals.remove(&relative),
            relative,
            expected,
            staged_path,
            backup_path,
            staged_identity,
            state: PublicationState::Prepared,
        });
    }
    sync_directory(&new_dir)?;
    sync_directory(&old_dir)?;
    sync_directory(transaction.path())?;

    let mut rollback_guard = PublicationRollbackGuard::new(
        fixtures_root,
        &mut entries,
        &mut transaction,
        &mut publication_lock,
    );
    let publication = publish_entries(fixtures_root, rollback_guard.entries_mut())
        .and_then(|()| check_managed_fixtures(fixtures_root, fixtures))
        .and_then(|()| ensure_generation_inputs_match(fixtures_root, generation_inputs));
    if let Err(error) = publication {
        let rollback = rollback_guard.rollback();
        return match rollback {
            Ok(()) => Err(format!(
                "fixture publication failed and was rolled back without partial managed results: {error}"
            )
            .into()),
            Err(rollback_error) => {
                let recovery_path = rollback_guard.recovery_path().display().to_string();
                Err(format!(
                    "fixture publication failed ({error}); rollback also failed ({rollback_error}); preserved recovery transaction `{}` and publication lock",
                    recovery_path
                )
                .into())
            }
        };
    }

    rollback_guard.commit();
    drop(rollback_guard);
    created_directories.commit();
    Ok(())
}

fn publish_entries(
    fixtures_root: &Path,
    entries: &mut [PublicationEntry],
) -> Result<(), Box<dyn Error>> {
    publish_entries_with_hook(fixtures_root, entries, |_| Ok(()))
}

fn publish_entries_with_hook(
    fixtures_root: &Path,
    entries: &mut [PublicationEntry],
    mut before_entry: impl FnMut(usize) -> Result<(), Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    for (index, entry) in entries.iter_mut().enumerate() {
        before_entry(index)?;
        let destination = fixtures_root.join(&entry.relative);
        match &entry.original {
            Some(original) => {
                let current = read_regular_file_snapshot(&destination)?;
                if current.identity != original.identity || current.bytes != original.bytes {
                    return Err(format!(
                        "managed fixture `{}` changed after publication planning",
                        entry.relative.display()
                    )
                    .into());
                }
                entry.state = PublicationState::OriginalMoved;
                move_regular_file_no_replace(&destination, &entry.backup_path, original.identity)?;
            }
            None => ensure_path_absent(&destination)?,
        }

        entry.state = PublicationState::Published;
        move_regular_file_no_replace(&entry.staged_path, &destination, entry.staged_identity)?;
        let published = read_regular_file_snapshot(&destination)?;
        if published.identity != entry.staged_identity || published.bytes != entry.expected {
            return Err(format!(
                "published fixture `{}` failed identity or byte verification",
                entry.relative.display()
            )
            .into());
        }
    }
    Ok(())
}

fn rollback_entries(
    fixtures_root: &Path,
    entries: &mut [PublicationEntry],
) -> Result<(), Box<dyn Error>> {
    let mut errors = Vec::new();
    for entry in entries.iter_mut().rev() {
        if let Err(error) = rollback_entry(fixtures_root, entry) {
            errors.push(format!(
                "reconcile `{}` with its pre-publication snapshot: {error}",
                entry.relative.display()
            ));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; ").into())
    }
}

fn rollback_entry(
    fixtures_root: &Path,
    entry: &mut PublicationEntry,
) -> Result<(), Box<dyn Error>> {
    let destination = fixtures_root.join(&entry.relative);
    let mut destination_snapshot = read_optional_regular_file_snapshot(&destination)?;
    if destination_snapshot
        .as_ref()
        .is_some_and(|snapshot| snapshot.identity == entry.staged_identity)
    {
        ensure_path_absent(&entry.staged_path)?;
        move_regular_file_no_replace(&destination, &entry.staged_path, entry.staged_identity)?;
        destination_snapshot = None;
    }
    let staged = read_optional_regular_file_snapshot(&entry.staged_path)?
        .ok_or("staged replacement is missing during rollback")?;
    if staged.identity != entry.staged_identity || staged.bytes != entry.expected {
        return Err("staged replacement changed identity or bytes during rollback".into());
    }

    match &entry.original {
        Some(original) => match destination_snapshot {
            Some(current)
                if current.identity == original.identity && current.bytes == original.bytes =>
            {
                ensure_path_absent(&entry.backup_path)?;
            }
            None => {
                let backup = read_regular_file_snapshot(&entry.backup_path)?;
                if backup.identity != original.identity || backup.bytes != original.bytes {
                    return Err("original backup changed identity or bytes".into());
                }
                move_regular_file_no_replace(&entry.backup_path, &destination, original.identity)?;
                let restored = read_regular_file_snapshot(&destination)?;
                if restored.identity != original.identity || restored.bytes != original.bytes {
                    return Err("restored original changed identity or bytes".into());
                }
            }
            Some(_) => {
                return Err("destination contains neither the staged nor original fixture".into());
            }
        },
        None => {
            if destination_snapshot.is_some() {
                return Err("new fixture destination remains occupied during rollback".into());
            }
            ensure_path_absent(&entry.backup_path)?;
        }
    }
    entry.state = PublicationState::Prepared;
    Ok(())
}

fn read_optional_regular_file_snapshot(
    path: &Path,
) -> Result<Option<FileSnapshot>, Box<dyn Error>> {
    match fs::symlink_metadata(path) {
        Ok(_) => read_regular_file_snapshot(path).map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn ensure_path_absent(path: &Path) -> Result<(), Box<dyn Error>> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(format!(
            "refusing to replace unplanned fixture target `{}`",
            path.display()
        )
        .into()),
        Err(error) => Err(error.into()),
    }
}

fn move_regular_file_no_replace(
    source: &Path,
    destination: &Path,
    expected_identity: FileIdentity,
) -> Result<(), Box<dyn Error>> {
    let source_parent = source
        .parent()
        .ok_or("fixture move source must have a parent")?;
    let destination_parent = destination
        .parent()
        .ok_or("fixture move destination must have a parent")?;
    ensure_existing_real_directory(source_parent)?;
    ensure_existing_real_directory(destination_parent)?;
    let source_metadata = fs::symlink_metadata(source)?;
    validate_regular_metadata(&source_metadata, source)?;
    if file_identity(&source_metadata, source)? != expected_identity {
        return Err(format!(
            "fixture move source `{}` changed identity",
            source.display()
        )
        .into());
    }
    ensure_path_absent(destination)?;

    fs::hard_link(source, destination)?;
    let destination_metadata = fs::symlink_metadata(destination)?;
    if destination_metadata.file_type().is_symlink()
        || !destination_metadata.is_file()
        || file_identity(&destination_metadata, destination)? != expected_identity
        || hard_link_count(&destination_metadata)? != 2
    {
        remove_matching_path(destination, expected_identity);
        return Err(format!(
            "fixture move destination `{}` failed hard-link identity verification",
            destination.display()
        )
        .into());
    }
    if let Err(error) = fs::remove_file(source) {
        remove_matching_path(destination, expected_identity);
        return Err(error.into());
    }
    let destination_metadata = fs::symlink_metadata(destination)?;
    if let Err(error) =
        validate_regular_metadata(&destination_metadata, destination).and_then(|()| {
            if file_identity(&destination_metadata, destination)? == expected_identity {
                Ok(())
            } else {
                Err("fixture move destination identity changed after unlink".into())
            }
        })
    {
        let _ = restore_failed_move(source, destination, expected_identity);
        return Err(error);
    }
    sync_directory(source_parent)?;
    if destination_parent != source_parent {
        sync_directory(destination_parent)?;
    }
    Ok(())
}

fn restore_failed_move(
    source: &Path,
    destination: &Path,
    expected_identity: FileIdentity,
) -> Result<(), Box<dyn Error>> {
    ensure_path_absent(source)?;
    fs::hard_link(destination, source)?;
    let restored = fs::symlink_metadata(source)?;
    if restored.file_type().is_symlink()
        || !restored.is_file()
        || file_identity(&restored, source)? != expected_identity
    {
        remove_matching_path(source, expected_identity);
        return Err("could not authenticate restored fixture move source".into());
    }
    fs::remove_file(destination)?;
    let restored = fs::symlink_metadata(source)?;
    validate_regular_metadata(&restored, source)?;
    let source_parent = source
        .parent()
        .ok_or("restored fixture source must have a parent")?;
    let destination_parent = destination
        .parent()
        .ok_or("restored fixture destination must have a parent")?;
    sync_directory(source_parent)?;
    if destination_parent != source_parent {
        sync_directory(destination_parent)?;
    }
    Ok(())
}

fn remove_matching_path(path: &Path, expected_identity: FileIdentity) {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return;
    };
    if metadata.is_file()
        && !metadata.file_type().is_symlink()
        && file_identity(&metadata, path).ok() == Some(expected_identity)
    {
        let _ = fs::remove_file(path);
    }
}

#[cfg(unix)]
fn hard_link_count(metadata: &fs::Metadata) -> Result<u64, Box<dyn Error>> {
    Ok(metadata.nlink())
}

#[cfg(windows)]
fn hard_link_count(metadata: &fs::Metadata) -> Result<u64, Box<dyn Error>> {
    metadata
        .number_of_links()
        .map(u64::from)
        .ok_or_else(|| "could not read fixture hard-link count".into())
}

#[cfg(not(any(unix, windows)))]
fn hard_link_count(_metadata: &fs::Metadata) -> Result<u64, Box<dyn Error>> {
    Err("fixture publication cannot authenticate hard links on this platform".into())
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
    write_new_regular_file(&base_path.with_extension("to"), &bytes)?;
    let json = to_string_pretty(&json_value)?;
    write_new_regular_file(&base_path.with_extension("json"), json.as_bytes())?;
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
    write_new_regular_file(path, format!("{}\n", to_string_pretty(outcome)?).as_bytes())?;
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
    write_new_regular_file(path, format!("{}\n", to_string_pretty(outcome)?).as_bytes())?;
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
    write_new_regular_file(
        &gov_dir.join("sdk_validation_inventory_v1.json"),
        format!("{}\n", to_string_pretty(&inventory)?).as_bytes(),
    )?;
    Ok(())
}

fn governance_sdk_fixture_binding(path: &Path) -> Result<(u64, String), Box<dyn Error>> {
    let bytes = read_regular_file(path)?;
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
    ensure_real_directory(&output_dir)?;
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
            .map(|(_, path)| read_regular_file(&fixtures_root.join(path)))
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
        write_new_regular_file(
            &output_dir.join(path),
            format!("{}\n", to_string_pretty(&outcome)?).as_bytes(),
        )?;
    }
    Ok(())
}

fn write_reference_sdk_fixture_inventory(fixtures_root: &Path) -> Result<(), Box<dyn Error>> {
    const PAYLOAD_SPECS: &[(&str, &str, &str, &str, &str)] = &[
        (
            "appeal_finance/cancel_asset_lock_v1.json",
            "appeal_finance",
            "cancel_asset_lock",
            "json",
            "valid",
        ),
        (
            "appeal_finance/cancel_asset_lock_v1.to",
            "appeal_finance",
            "cancel_asset_lock",
            "norito",
            "valid",
        ),
        (
            "appeal_finance/negative/cancel_asset_lock_legacy_missing_expected_v1.json",
            "appeal_finance",
            "cancel_asset_lock",
            "json",
            "invalid_missing_expected_remaining_amount",
        ),
        (
            "appeal_finance/negative/cancel_asset_lock_legacy_missing_expected_v1.to",
            "appeal_finance",
            "cancel_asset_lock",
            "norito",
            "invalid_missing_expected_remaining_amount",
        ),
        (
            "appeal_finance/negative/cancel_asset_lock_noncanonical_quantity_v1.json",
            "appeal_finance",
            "cancel_asset_lock",
            "json",
            "invalid_noncanonical_quantity",
        ),
        (
            "appeal_finance/negative/cancel_asset_lock_trailing_bytes_v1.to",
            "appeal_finance",
            "cancel_asset_lock",
            "norito",
            "noncanonical_trailing_bytes",
        ),
        (
            "appeal_finance/negative/cancel_asset_lock_zero_expected_v1.json",
            "appeal_finance",
            "cancel_asset_lock",
            "json",
            "invalid_zero_expected_remaining_amount",
        ),
        (
            "appeal_finance/negative/cancel_asset_lock_zero_expected_v1.to",
            "appeal_finance",
            "cancel_asset_lock",
            "norito",
            "invalid_zero_expected_remaining_amount",
        ),
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
    write_new_regular_file(
        &fixtures_root.join("reference_sdk_validation_inventory_v1.json"),
        format!("{}\n", to_string_pretty(&inventory)?).as_bytes(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn managed_fixture_paths_reject_traversal_and_open_layouts() {
        for path in [
            "../por/a.to",
            "por/../../a.to",
            "/por/a.to",
            "unmanaged/a.to",
            "por/a.txt",
        ] {
            validate_managed_relative_path(Path::new(path))
                .expect_err("path outside the closed layout must be rejected");
        }
        for path in ["../por", "/por", "unmanaged", "por/a/b/c"] {
            validate_managed_directory_path(Path::new(path))
                .expect_err("directory outside the closed layout must be rejected");
        }
    }

    #[test]
    fn multi_file_publication_failure_restores_every_original() {
        let temporary = tempdir().expect("create publication transaction root");
        let temporary_root = temporary
            .path()
            .canonicalize()
            .expect("canonicalize publication transaction root");
        let fixture_root = temporary_root.join("fixtures");
        let staging = temporary_root.join("staging");
        let backup = temporary_root.join("backup");
        ensure_real_directory(&fixture_root).expect("create fixture root");
        ensure_real_directory(&fixture_root.join("por")).expect("create managed directory");
        ensure_real_directory(&staging).expect("create staging directory");
        ensure_real_directory(&backup).expect("create backup directory");

        let relative_a = PathBuf::from("por/a.to");
        let relative_b = PathBuf::from("por/b.to");
        let destination_a = fixture_root.join(&relative_a);
        let destination_b = fixture_root.join(&relative_b);
        write_new_regular_file(&destination_a, b"old-a").expect("write original a");
        write_new_regular_file(&destination_b, b"old-b").expect("write original b");
        let original_a = read_regular_file_snapshot(&destination_a).expect("snapshot original a");
        let original_b = read_regular_file_snapshot(&destination_b).expect("snapshot original b");

        let staged_a = staging.join("a.to");
        let staged_b = staging.join("b.to");
        let staged_a_identity =
            write_new_regular_file(&staged_a, b"new-a").expect("stage replacement a");
        let staged_b_identity =
            write_new_regular_file(&staged_b, b"new-b").expect("stage replacement b");
        assert_ne!(
            staged_a_identity, staged_b_identity,
            "independent staged files must have independent identities"
        );

        let mut entries = [
            PublicationEntry {
                relative: relative_a,
                expected: b"new-a".to_vec(),
                original: Some(original_a),
                staged_path: staged_a,
                backup_path: backup.join("a.to"),
                staged_identity: staged_a_identity,
                state: PublicationState::Prepared,
            },
            PublicationEntry {
                relative: relative_b,
                expected: b"new-b".to_vec(),
                original: Some(original_b),
                staged_path: staged_b,
                backup_path: backup.join("b.to"),
                staged_identity: staged_b_identity,
                state: PublicationState::Prepared,
            },
        ];
        publish_entries_with_hook(&fixture_root, &mut entries, |index| {
            if index == 1 {
                Err("injected failure before second fixture".into())
            } else {
                Ok(())
            }
        })
        .expect_err("injected second-entry failure must stop publication");
        rollback_entries(&fixture_root, &mut entries)
            .expect("transaction rollback must restore every original");

        assert_eq!(
            read_regular_file(&destination_a).expect("read restored a"),
            b"old-a".to_vec()
        );
        assert_eq!(
            read_regular_file(&destination_b).expect("read restored b"),
            b"old-b".to_vec()
        );
        assert_eq!(
            entries.iter().map(|entry| entry.state).collect::<Vec<_>>(),
            vec![PublicationState::Prepared, PublicationState::Prepared]
        );
    }
}
