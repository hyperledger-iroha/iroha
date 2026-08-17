//! Independent semantic validation for privacy-protocol controller-origin evidence.
//!
//! Descriptor ownership, immutability, and filesystem identity are established
//! by the authority transport before this module is called. This validator
//! nevertheless reads and hashes every descriptor itself, then reconstructs
//! the exact v2 structural subject from the preserved bytes. Caller-provided
//! self-hashes are accepted only when every link in that reconstruction agrees.

use super::{protocol::TairaAuthorityArtifactManifestEntryV1, service::TairaAuthorityErrorV1};
use norito::json::{Map, Value};
use sha2::{Digest as _, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{Read as _, Seek as _, SeekFrom},
};

const RECEIPT_SCHEMA_V2: &str = "iroha.taira.privacy_protocol_four_peer_receipt";
const RECEIPT_SCHEMA_VERSION_V2: u64 = 2;
const TRANSCRIPT_SCHEMA_V1: &str = "iroha.taira.privacy_protocol_case_transcript";
const RESULT_SCHEMA_V1: &str = "iroha.taira.privacy_protocol_case_result";

const AUTHORITY_SCHEMA_V1: &str = "iroha.taira.privacy-protocol-controller-origin-authority.v1";
const AUTHENTICATED_RUN_SCHEMA_V1: &str = "iroha.taira.privacy-protocol-authenticated-run-nonce.v1";
const REPLAY_NAMESPACE_V1: &str = "iroha.taira.privacy-protocol-controller-origin-replay.v1";

const RECEIPT_NAME_V2: &str = "evidence/privacy-protocol-four-peer-receipt-v2.json";
const MAX_RECEIPT_BYTES_V2: u64 = 1024 * 1024;
const MAX_RESULT_BYTES_V1: u64 = 128 * 1024;
const MAX_COMMAND_OUTPUT_BYTES_V1: usize = 2 * 1024 * 1024;
const MAX_TRANSCRIPT_BYTES_V1: u64 = 6 * 1024 * 1024;
const MAX_DRIVER_BYTES_V1: u64 = 512 * 1024 * 1024;
const MAX_EVIDENCE_TOTAL_BYTES_V2: u64 = 2 * 1024 * 1024 * 1024;
const MAX_RECEIPT_LIFETIME_SECONDS_V2: u64 = 24 * 60 * 60;
const MAX_FUTURE_CLOCK_SKEW_SECONDS_V2: u64 = 5 * 60;
const PEER_COUNT_V2: u64 = 4;

const ACTION_DRIVER_V1: &str = "privacy-exact12-action-driver";
const JINDO_DRIVER_V1: &str = "iroha-core-tests";
const NETWORK_DRIVER_V1: &str = "network-functional";
const JINDO_SECURITY_FILTER_V1: &str = "privacy_engines::jindo::security::tests";

const DRIVER_EVIDENCE_V1: [(&str, &str); 3] = [
    (ACTION_DRIVER_V1, "evidence/privacy-action-driver-v1.bin"),
    (JINDO_DRIVER_V1, "evidence/privacy-jindo-test-driver-v1.bin"),
    (
        NETWORK_DRIVER_V1,
        "evidence/privacy-network-test-driver-v1.bin",
    ),
];

const CASE_DEFINITIONS_V1: [(&str, &str); 7] = [
    (
        "privacy_exact12_retained_network::canonical_retained_exact12_actions_survive_four_peer_adversarial_replay_and_restart",
        "protocol",
    ),
    (
        "privacy_exact12_activation_network::canonical_zk_ams_action_survives_four_peer_activation_replay_and_restart",
        "protocol",
    ),
    (
        "privacy_exact12_zk_ams_vega_network::canonical_zk_ams_and_vega_actions_survive_four_validator_activation_replay_and_restart",
        "protocol",
    ),
    (
        "privacy_exact12_zk_x509_network::canonical_zk_x509_action_survives_four_peer_activation_replay_and_restart",
        "protocol",
    ),
    (
        "privacy_exact12_jindo_network::canonical_jindo_direct_action_survives_four_peer_activation_replay_and_restart",
        "protocol",
    ),
    (
        "privacy_exact12_orchard_pq_masp_network::canonical_orchard_and_pq_masp_actions_survive_four_peer_da_replay_and_restart",
        "protocol",
    ),
    (
        "privacy_exact12_activation_network::canonical_exact12_governance_survives_four_peer_activation_replay_and_restart",
        "governance",
    ),
];

#[derive(Clone, Copy)]
struct OutcomeV1 {
    protocol: &'static str,
    case_index: u64,
    profile: &'static str,
    production_outcome: &'static str,
    closed_reason: &'static str,
    security_boundary: &'static str,
}

const OUTCOMES_V1: [OutcomeV1; 12] = [
    OutcomeV1 {
        protocol: "zk-ace-pq-authorization-v0",
        case_index: 0,
        profile: "available",
        production_outcome: "accepted-and-queried-across-four-peers",
        closed_reason: "none",
        security_boundary: "none",
    },
    OutcomeV1 {
        protocol: "anonymous-pgc-k-out-of-n-v1",
        case_index: 0,
        profile: "available",
        production_outcome: "accepted-and-queried-across-four-peers",
        closed_reason: "none",
        security_boundary: "none",
    },
    OutcomeV1 {
        protocol: "verange-transparent-range-v1",
        case_index: 0,
        profile: "available",
        production_outcome: "accepted-and-queried-across-four-peers",
        closed_reason: "none",
        security_boundary: "none",
    },
    OutcomeV1 {
        protocol: "iroha-zk-ams-v1",
        case_index: 1,
        profile: "available",
        production_outcome: "accepted-and-queried-across-four-peers",
        closed_reason: "none",
        security_boundary: "none",
    },
    OutcomeV1 {
        protocol: "vega-existing-credential-zk-v0",
        case_index: 2,
        profile: "available",
        production_outcome: "accepted-and-queried-across-four-peers",
        closed_reason: "none",
        security_boundary: "none",
    },
    OutcomeV1 {
        protocol: "iroha-zk-x509-stark-p256-v0",
        case_index: 3,
        profile: "available",
        production_outcome: "accepted-and-queried-across-four-peers",
        closed_reason: "none",
        security_boundary: "none",
    },
    OutcomeV1 {
        protocol: "iroha-jindo-polynomial-commitment-v0",
        case_index: 4,
        profile: "available-experimental",
        production_outcome: "accepted-and-queried-across-four-peers",
        closed_reason: "none",
        security_boundary: "JindoSecurityCertificateErrorV1::MissingDistributionWideKnowledgeSoundnessEvidence",
    },
    OutcomeV1 {
        protocol: "iroha-bootle-lantern-anoncred-v1",
        case_index: 0,
        profile: "available",
        production_outcome: "accepted-and-queried-across-four-peers",
        closed_reason: "none",
        security_boundary: "none",
    },
    OutcomeV1 {
        protocol: "orchard-halo2-actions-v1",
        case_index: 5,
        profile: "available",
        production_outcome: "accepted-and-queried-across-four-peers",
        closed_reason: "none",
        security_boundary: "none",
    },
    OutcomeV1 {
        protocol: "monero-fcmp-plus-plus-v1",
        case_index: 0,
        profile: "available",
        production_outcome: "accepted-and-queried-across-four-peers",
        closed_reason: "none",
        security_boundary: "none",
    },
    OutcomeV1 {
        protocol: "iroha-ivm-private-note-stark-v1",
        case_index: 0,
        profile: "available",
        production_outcome: "accepted-and-queried-across-four-peers",
        closed_reason: "none",
        security_boundary: "none",
    },
    OutcomeV1 {
        protocol: "pq-masp-stark-v0",
        case_index: 5,
        profile: "available",
        production_outcome: "accepted-and-queried-across-four-peers",
        closed_reason: "none",
        security_boundary: "none",
    },
];

// The Python client sorts all logical evidence names before it constructs the
// ordered descriptor manifest. Keep the literal order here synchronized with
// that operation, including result-before-transcript for every case.
const MANIFEST_V2: [(&str, u64); 18] = [
    ("evidence/privacy-action-driver-v1.bin", MAX_DRIVER_BYTES_V1),
    (
        "evidence/privacy-jindo-test-driver-v1.bin",
        MAX_DRIVER_BYTES_V1,
    ),
    (
        "evidence/privacy-network-test-driver-v1.bin",
        MAX_DRIVER_BYTES_V1,
    ),
    (
        "evidence/privacy-protocol-case-00-result-v1.json",
        MAX_RESULT_BYTES_V1,
    ),
    (
        "evidence/privacy-protocol-case-00-transcript-v1.json",
        MAX_TRANSCRIPT_BYTES_V1,
    ),
    (
        "evidence/privacy-protocol-case-01-result-v1.json",
        MAX_RESULT_BYTES_V1,
    ),
    (
        "evidence/privacy-protocol-case-01-transcript-v1.json",
        MAX_TRANSCRIPT_BYTES_V1,
    ),
    (
        "evidence/privacy-protocol-case-02-result-v1.json",
        MAX_RESULT_BYTES_V1,
    ),
    (
        "evidence/privacy-protocol-case-02-transcript-v1.json",
        MAX_TRANSCRIPT_BYTES_V1,
    ),
    (
        "evidence/privacy-protocol-case-03-result-v1.json",
        MAX_RESULT_BYTES_V1,
    ),
    (
        "evidence/privacy-protocol-case-03-transcript-v1.json",
        MAX_TRANSCRIPT_BYTES_V1,
    ),
    (
        "evidence/privacy-protocol-case-04-result-v1.json",
        MAX_RESULT_BYTES_V1,
    ),
    (
        "evidence/privacy-protocol-case-04-transcript-v1.json",
        MAX_TRANSCRIPT_BYTES_V1,
    ),
    (
        "evidence/privacy-protocol-case-05-result-v1.json",
        MAX_RESULT_BYTES_V1,
    ),
    (
        "evidence/privacy-protocol-case-05-transcript-v1.json",
        MAX_TRANSCRIPT_BYTES_V1,
    ),
    (
        "evidence/privacy-protocol-case-06-result-v1.json",
        MAX_RESULT_BYTES_V1,
    ),
    (
        "evidence/privacy-protocol-case-06-transcript-v1.json",
        MAX_TRANSCRIPT_BYTES_V1,
    ),
    (RECEIPT_NAME_V2, MAX_RECEIPT_BYTES_V2),
];

const SUBJECT_FIELDS_V1: [&str; 6] = [
    "authenticated_run_schema",
    "authority_schema",
    "expected",
    "replay_namespace",
    "structural_subject",
    "validation_time_unix",
];
const EXPECTED_FIELDS_V1: [&str; 6] = [
    "artifact_handoff_sha256",
    "exact12_matrix_sha256",
    "linux_release_archive_sha256",
    "receipt_id",
    "source",
    "validator_binary_sha256",
];
const SOURCE_FIELDS_V1: [&str; 4] = [
    "cargo_lock_sha256",
    "commit",
    "dpn_validator_release_commit",
    "workspace_source_manifest_sha256",
];
const STRUCTURAL_FIELDS_V1: [&str; 9] = [
    "artifact_handoff_sha256",
    "case_count",
    "cases",
    "drivers",
    "exact12_matrix_sha256",
    "linux_release_archive_sha256",
    "outcomes",
    "receipt_id",
    "validator_binary_sha256",
];

/// Validate one controller-origin request before replay state is consumed.
///
/// `now_unix` is the authority's current Unix time in seconds. The request's
/// recorded validation time is checked separately so a previously prepared,
/// still-live administrator assignment remains usable without trusting that
/// caller-provided clock as the authority clock.
pub(super) fn validate_privacy_protocol_origin_v1(
    subject: &Value,
    manifest: &[TairaAuthorityArtifactManifestEntryV1],
    artifacts: &mut [File],
    now_unix: u64,
) -> Result<Value, TairaAuthorityErrorV1> {
    validate_manifest(manifest, artifacts.len())?;
    let subject = exact_object(subject, &SUBJECT_FIELDS_V1)?;
    if required_str(subject, "authority_schema")? != AUTHORITY_SCHEMA_V1
        || required_str(subject, "authenticated_run_schema")? != AUTHENTICATED_RUN_SCHEMA_V1
        || required_str(subject, "replay_namespace")? != REPLAY_NAMESPACE_V1
    {
        return rejected();
    }

    let expected = exact_object(
        subject
            .get("expected")
            .ok_or(TairaAuthorityErrorV1::Rejected)?,
        &EXPECTED_FIELDS_V1,
    )?;
    let expected_source = validate_source_identity(
        expected
            .get("source")
            .ok_or(TairaAuthorityErrorV1::Rejected)?,
    )?;
    let expected_artifact_handoff = required_digest(expected, "artifact_handoff_sha256")?;
    let expected_exact12_matrix = required_digest(expected, "exact12_matrix_sha256")?;
    let expected_linux_archive = required_digest(expected, "linux_release_archive_sha256")?;
    let expected_receipt_id = required_digest(expected, "receipt_id")?;
    let expected_validator = required_digest(expected, "validator_binary_sha256")?;
    let validation_time_unix = required_u64_allow_zero(subject, "validation_time_unix")?;

    let receipt_bytes =
        read_named_artifact(manifest, artifacts, RECEIPT_NAME_V2, MAX_RECEIPT_BYTES_V2)?;
    let receipt = decode_canonical_object(&receipt_bytes, MAX_RECEIPT_BYTES_V2)?;
    let receipt = exact_object(
        &receipt,
        &[
            "candidate",
            "cases",
            "expires_at_unix",
            "issued_at_unix",
            "outcomes",
            "platform",
            "receipt_id",
            "schema",
            "schema_version",
        ],
    )?;
    if required_str(receipt, "schema")? != RECEIPT_SCHEMA_V2
        || required_u64_allow_zero(receipt, "schema_version")? != RECEIPT_SCHEMA_VERSION_V2
    {
        return rejected();
    }
    let receipt_id = required_digest(receipt, "receipt_id")?;
    if receipt_id != expected_receipt_id
        || domain_id_without_field(
            b"iroha.taira.privacy_protocol_four_peer_receipt.v2\0",
            receipt,
            "receipt_id",
        )? != receipt_id
    {
        return rejected();
    }
    validate_platform(
        receipt
            .get("platform")
            .ok_or(TairaAuthorityErrorV1::Rejected)?,
    )?;
    let issued_at_unix = required_u64_allow_zero(receipt, "issued_at_unix")?;
    let expires_at_unix = required_u64_allow_zero(receipt, "expires_at_unix")?;
    validate_receipt_time(issued_at_unix, expires_at_unix, validation_time_unix)?;
    validate_receipt_time(issued_at_unix, expires_at_unix, now_unix)?;

    let (candidate, driver_digests, candidate_binding_sha256) = validate_candidate(
        receipt
            .get("candidate")
            .ok_or(TairaAuthorityErrorV1::Rejected)?,
        &expected_source,
        expected_artifact_handoff,
        expected_exact12_matrix,
        expected_linux_archive,
        expected_validator,
    )?;
    for (driver, artifact_name) in DRIVER_EVIDENCE_V1 {
        let actual = hash_named_artifact(manifest, artifacts, artifact_name, MAX_DRIVER_BYTES_V1)?;
        if driver_digests.get(driver).copied() != Some(actual) {
            return rejected();
        }
    }

    let case_rows = receipt
        .get("cases")
        .and_then(Value::as_array)
        .filter(|rows| rows.len() == CASE_DEFINITIONS_V1.len())
        .ok_or(TairaAuthorityErrorV1::Rejected)?;
    let mut seen_transcripts = BTreeSet::new();
    let mut seen_results = BTreeSet::new();
    let mut normalized_cases = Vec::with_capacity(CASE_DEFINITIONS_V1.len());
    for (index, row) in case_rows.iter().enumerate() {
        let row = validate_case_row(
            row,
            index,
            candidate_binding_sha256,
            &driver_digests,
            manifest,
            artifacts,
        )?;
        let transcript_id = required_digest(&row, "transcript_id")?;
        let result_id = required_digest(&row, "result_id")?;
        if !seen_transcripts.insert(transcript_id) || !seen_results.insert(result_id) {
            return rejected();
        }
        normalized_cases.push(Value::Object(row));
    }

    let outcomes = receipt
        .get("outcomes")
        .and_then(Value::as_array)
        .filter(|rows| rows.len() == OUTCOMES_V1.len())
        .ok_or(TairaAuthorityErrorV1::Rejected)?;
    let mut normalized_outcomes = Vec::with_capacity(OUTCOMES_V1.len());
    for (index, (row, expected_outcome)) in outcomes.iter().zip(OUTCOMES_V1).enumerate() {
        let expected_row = outcome_value(index, expected_outcome);
        if row != &expected_row {
            return rejected();
        }
        normalized_outcomes.push(expected_row);
    }

    let candidate = candidate
        .as_object()
        .ok_or(TairaAuthorityErrorV1::Rejected)?;
    let normalized = object([
        (
            "artifact_handoff_sha256",
            candidate
                .get("artifact_handoff_sha256")
                .cloned()
                .ok_or(TairaAuthorityErrorV1::Rejected)?,
        ),
        ("case_count", Value::from(CASE_DEFINITIONS_V1.len() as u64)),
        ("cases", Value::Array(normalized_cases)),
        (
            "drivers",
            candidate
                .get("drivers")
                .cloned()
                .ok_or(TairaAuthorityErrorV1::Rejected)?,
        ),
        (
            "exact12_matrix_sha256",
            candidate
                .get("exact12_matrix_sha256")
                .cloned()
                .ok_or(TairaAuthorityErrorV1::Rejected)?,
        ),
        (
            "linux_release_archive_sha256",
            candidate
                .get("linux_release_archive_sha256")
                .cloned()
                .ok_or(TairaAuthorityErrorV1::Rejected)?,
        ),
        ("outcomes", Value::Array(normalized_outcomes)),
        ("receipt_id", Value::from(hex::encode(receipt_id))),
        (
            "validator_binary_sha256",
            candidate
                .get("validator_binary_sha256")
                .cloned()
                .ok_or(TairaAuthorityErrorV1::Rejected)?,
        ),
    ]);
    exact_object(&normalized, &STRUCTURAL_FIELDS_V1)?;
    if subject.get("structural_subject") != Some(&normalized) {
        return rejected();
    }
    Ok(normalized)
}

fn validate_manifest(
    manifest: &[TairaAuthorityArtifactManifestEntryV1],
    artifact_count: usize,
) -> Result<(), TairaAuthorityErrorV1> {
    if manifest.len() != MANIFEST_V2.len() || artifact_count != MANIFEST_V2.len() {
        return rejected();
    }
    let mut total = 0_u64;
    for (index, (entry, (name, maximum))) in manifest.iter().zip(MANIFEST_V2).enumerate() {
        if usize::from(entry.ordinal) != index
            || entry.name != name
            || entry.size == 0
            || entry.size > maximum
            || entry.sha256 == [0; 32]
        {
            return rejected();
        }
        total = total
            .checked_add(entry.size)
            .ok_or(TairaAuthorityErrorV1::Rejected)?;
        if total > MAX_EVIDENCE_TOTAL_BYTES_V2 {
            return rejected();
        }
    }
    Ok(())
}

fn validate_source_identity(value: &Value) -> Result<Value, TairaAuthorityErrorV1> {
    let source = exact_object(value, &SOURCE_FIELDS_V1)?;
    required_digest(source, "cargo_lock_sha256")?;
    required_commit(source, "commit")?;
    required_commit(source, "dpn_validator_release_commit")?;
    required_digest(source, "workspace_source_manifest_sha256")?;
    Ok(value.clone())
}

fn validate_platform(value: &Value) -> Result<(), TairaAuthorityErrorV1> {
    let platform = exact_object(value, &["arch", "os", "peer_count"])?;
    if required_str(platform, "arch")? != "arm64"
        || required_str(platform, "os")? != "macos"
        || required_u64_allow_zero(platform, "peer_count")? != PEER_COUNT_V2
    {
        return rejected();
    }
    Ok(())
}

fn validate_receipt_time(
    issued: u64,
    expires: u64,
    current: u64,
) -> Result<(), TairaAuthorityErrorV1> {
    if expires <= issued
        || expires - issued > MAX_RECEIPT_LIFETIME_SECONDS_V2
        || issued > current.saturating_add(MAX_FUTURE_CLOCK_SKEW_SECONDS_V2)
        || current > expires
    {
        return rejected();
    }
    Ok(())
}

fn validate_candidate(
    value: &Value,
    expected_source: &Value,
    expected_artifact_handoff: [u8; 32],
    expected_exact12_matrix: [u8; 32],
    expected_linux_archive: [u8; 32],
    expected_validator: [u8; 32],
) -> Result<(Value, BTreeMap<String, [u8; 32]>, [u8; 32]), TairaAuthorityErrorV1> {
    let candidate = exact_object(
        value,
        &[
            "artifact_handoff_sha256",
            "drivers",
            "exact12_matrix_sha256",
            "linux_release_archive_sha256",
            "source",
            "validator_binary_sha256",
        ],
    )?;
    if &validate_source_identity(
        candidate
            .get("source")
            .ok_or(TairaAuthorityErrorV1::Rejected)?,
    )? != expected_source
        || required_digest(candidate, "artifact_handoff_sha256")? != expected_artifact_handoff
        || required_digest(candidate, "exact12_matrix_sha256")? != expected_exact12_matrix
        || required_digest(candidate, "linux_release_archive_sha256")? != expected_linux_archive
        || required_digest(candidate, "validator_binary_sha256")? != expected_validator
    {
        return rejected();
    }
    let drivers = exact_object(
        candidate
            .get("drivers")
            .ok_or(TairaAuthorityErrorV1::Rejected)?,
        &[ACTION_DRIVER_V1, JINDO_DRIVER_V1, NETWORK_DRIVER_V1],
    )?;
    let mut driver_digests = BTreeMap::new();
    for driver in [ACTION_DRIVER_V1, JINDO_DRIVER_V1, NETWORK_DRIVER_V1] {
        driver_digests.insert(driver.to_owned(), required_digest(drivers, driver)?);
    }
    let binding = domain_id(b"iroha.taira.privacy_protocol_candidate.v2\0", value)?;
    Ok((value.clone(), driver_digests, binding))
}

fn validate_case_row(
    value: &Value,
    index: usize,
    candidate_binding_sha256: [u8; 32],
    driver_digests: &BTreeMap<String, [u8; 32]>,
    manifest: &[TairaAuthorityArtifactManifestEntryV1],
    artifacts: &mut [File],
) -> Result<Map, TairaAuthorityErrorV1> {
    let row = exact_object(
        value,
        &[
            "case",
            "index",
            "kind",
            "result_id",
            "result_path",
            "result_sha256",
            "result_size",
            "transcript_id",
            "transcript_path",
            "transcript_sha256",
            "transcript_size",
        ],
    )?;
    let (case, kind) = CASE_DEFINITIONS_V1[index];
    let transcript_basename = transcript_name(index);
    let result_basename = result_name(index);
    if required_u64_allow_zero(row, "index")? != index as u64
        || required_str(row, "case")? != case
        || required_str(row, "kind")? != kind
        || required_str(row, "transcript_path")? != transcript_basename
        || required_str(row, "result_path")? != result_basename
    {
        return rejected();
    }
    let transcript_artifact = format!("evidence/{transcript_basename}");
    let result_artifact = format!("evidence/{result_basename}");
    let transcript_bytes = read_named_artifact(
        manifest,
        artifacts,
        &transcript_artifact,
        MAX_TRANSCRIPT_BYTES_V1,
    )?;
    let result_bytes =
        read_named_artifact(manifest, artifacts, &result_artifact, MAX_RESULT_BYTES_V1)?;
    if required_digest(row, "transcript_sha256")? != sha256(&transcript_bytes)
        || required_u64_positive(row, "transcript_size")? != transcript_bytes.len() as u64
        || required_digest(row, "result_sha256")? != sha256(&result_bytes)
        || required_u64_positive(row, "result_size")? != result_bytes.len() as u64
    {
        return rejected();
    }
    let transcript_id = validate_transcript(
        &transcript_bytes,
        index,
        candidate_binding_sha256,
        driver_digests,
    )?;
    let result_id = validate_result(
        &result_bytes,
        index,
        candidate_binding_sha256,
        transcript_id,
        sha256(&transcript_bytes),
        transcript_bytes.len() as u64,
    )?;
    if required_digest(row, "transcript_id")? != transcript_id
        || required_digest(row, "result_id")? != result_id
    {
        return rejected();
    }
    Ok(row.clone())
}

fn validate_transcript(
    bytes: &[u8],
    index: usize,
    candidate_binding_sha256: [u8; 32],
    driver_digests: &BTreeMap<String, [u8; 32]>,
) -> Result<[u8; 32], TairaAuthorityErrorV1> {
    let value = decode_canonical_object(bytes, MAX_TRANSCRIPT_BYTES_V1)?;
    let transcript = exact_object(
        &value,
        &[
            "candidate_binding_sha256",
            "case",
            "commands",
            "index",
            "kind",
            "schema",
            "schema_version",
            "transcript_id",
        ],
    )?;
    let (case, kind) = CASE_DEFINITIONS_V1[index];
    if required_str(transcript, "schema")? != TRANSCRIPT_SCHEMA_V1
        || required_u64_allow_zero(transcript, "schema_version")? != 1
        || required_u64_allow_zero(transcript, "index")? != index as u64
        || required_str(transcript, "case")? != case
        || required_str(transcript, "kind")? != kind
        || required_digest(transcript, "candidate_binding_sha256")? != candidate_binding_sha256
    {
        return rejected();
    }
    let transcript_id = required_digest(transcript, "transcript_id")?;
    if domain_id_without_field(
        b"iroha.taira.privacy_protocol_case_transcript.v1\0",
        transcript,
        "transcript_id",
    )? != transcript_id
    {
        return rejected();
    }
    let commands = transcript
        .get("commands")
        .and_then(Value::as_array)
        .ok_or(TairaAuthorityErrorV1::Rejected)?;
    let expected_count = if index == 4 { 2 } else { 1 };
    if commands.len() != expected_count {
        return rejected();
    }
    for (command_index, command) in commands.iter().enumerate() {
        let command = exact_object(
            command,
            &[
                "args",
                "driver",
                "driver_sha256",
                "exit_code",
                "index",
                "output_base64",
                "output_sha256",
                "output_size",
            ],
        )?;
        let expected_driver = if command_index == 0 {
            NETWORK_DRIVER_V1
        } else {
            JINDO_DRIVER_V1
        };
        let expected_args = command_args(index, command_index);
        if required_u64_allow_zero(command, "index")? != command_index as u64
            || required_str(command, "driver")? != expected_driver
            || command.get("args") != Some(&expected_args)
            || required_u64_allow_zero(command, "exit_code")? != 0
            || required_digest(command, "driver_sha256")?
                != driver_digests
                    .get(expected_driver)
                    .copied()
                    .ok_or(TairaAuthorityErrorV1::Rejected)?
        {
            return rejected();
        }
        let output_size = required_u64_positive(command, "output_size")?;
        if output_size > MAX_COMMAND_OUTPUT_BYTES_V1 as u64 {
            return rejected();
        }
        let encoded = required_str(command, "output_base64")?;
        if !encoded.is_ascii() {
            return rejected();
        }
        let output = decode_base64(encoded).ok_or(TairaAuthorityErrorV1::Rejected)?;
        if output.len() != output_size as usize
            || sha256(&output) != required_digest(command, "output_sha256")?
        {
            return rejected();
        }
        validate_test_output(&output, case, expected_driver)?;
    }
    Ok(transcript_id)
}

fn validate_result(
    bytes: &[u8],
    index: usize,
    candidate_binding_sha256: [u8; 32],
    transcript_id: [u8; 32],
    transcript_sha256: [u8; 32],
    transcript_size: u64,
) -> Result<[u8; 32], TairaAuthorityErrorV1> {
    let value = decode_canonical_object(bytes, MAX_RESULT_BYTES_V1)?;
    let result = exact_object(
        &value,
        &[
            "candidate_binding_sha256",
            "case",
            "index",
            "kind",
            "result_id",
            "schema",
            "schema_version",
            "status",
            "transcript_id",
            "transcript_path",
            "transcript_sha256",
            "transcript_size",
        ],
    )?;
    let (case, kind) = CASE_DEFINITIONS_V1[index];
    if required_digest(result, "candidate_binding_sha256")? != candidate_binding_sha256
        || required_str(result, "case")? != case
        || required_u64_allow_zero(result, "index")? != index as u64
        || required_str(result, "kind")? != kind
        || required_str(result, "schema")? != RESULT_SCHEMA_V1
        || required_u64_allow_zero(result, "schema_version")? != 1
        || required_str(result, "status")? != "passed"
        || required_digest(result, "transcript_id")? != transcript_id
        || required_str(result, "transcript_path")? != transcript_name(index)
        || required_digest(result, "transcript_sha256")? != transcript_sha256
        || required_u64_positive(result, "transcript_size")? != transcript_size
    {
        return rejected();
    }
    let result_id = required_digest(result, "result_id")?;
    if domain_id_without_field(
        b"iroha.taira.privacy_protocol_case_result.v1\0",
        result,
        "result_id",
    )? != result_id
    {
        return rejected();
    }
    Ok(result_id)
}

fn command_args(case_index: usize, command_index: usize) -> Value {
    let values = if command_index == 0 {
        vec![
            CASE_DEFINITIONS_V1[case_index].0,
            "--exact",
            "--nocapture",
            "--test-threads=1",
        ]
    } else {
        vec![JINDO_SECURITY_FILTER_V1, "--nocapture", "--test-threads=1"]
    };
    Value::Array(values.into_iter().map(Value::from).collect())
}

fn validate_test_output(
    output: &[u8],
    case: &str,
    driver: &str,
) -> Result<(), TairaAuthorityErrorV1> {
    let text = std::str::from_utf8(output).map_err(|_| TairaAuthorityErrorV1::Rejected)?;
    let lowered = text.to_lowercase();
    if [
        "engine-unavailable",
        "engine_unavailable",
        "fixture-only",
        "fixture_only",
        "running 0 tests",
        "test result: failed",
    ]
    .iter()
    .any(|forbidden| lowered.contains(forbidden))
    {
        return rejected();
    }
    let lines = text.split('\n').collect::<Vec<_>>();
    let summaries = lines
        .iter()
        .filter_map(|line| parse_pass_summary(line))
        .collect::<Vec<_>>();
    let running = lines
        .iter()
        .filter_map(|line| parse_running_count(line))
        .collect::<Vec<_>>();
    if summaries.len() != 1 || running.len() != 1 || summaries[0] != running[0] {
        return rejected();
    }
    if driver == NETWORK_DRIVER_V1 {
        let expected = format!("test {case} ... ok");
        if summaries[0] != 1
            || !lines
                .iter()
                .any(|line| line.strip_suffix('\r').unwrap_or(line) == expected)
        {
            return rejected();
        }
    } else {
        let passed = lines
            .iter()
            .filter_map(|line| parse_passed_test_name(line))
            .collect::<Vec<_>>();
        if passed.len() as u64 != summaries[0]
            || passed
                .iter()
                .any(|name| !name.to_lowercase().contains("jindo"))
        {
            return rejected();
        }
    }
    Ok(())
}

fn parse_running_count(line: &str) -> Option<u64> {
    let line = line.to_ascii_lowercase();
    let body = line.strip_prefix("running ")?;
    let digits = body
        .strip_suffix(" tests")
        .or_else(|| body.strip_suffix(" test"))?;
    parse_positive_decimal(digits)
}

fn parse_pass_summary(line: &str) -> Option<u64> {
    let body = line.strip_prefix("test result: ok. ")?;
    let digit_end = body.bytes().take_while(u8::is_ascii_digit).count();
    let (digits, tail) = body.split_at(digit_end);
    if !tail.starts_with(" passed; 0 failed; 0 ignored;") {
        return None;
    }
    parse_positive_decimal(digits)
}

fn parse_passed_test_name(line: &str) -> Option<&str> {
    let line = line.strip_suffix('\r').unwrap_or(line);
    line.strip_prefix("test ")?
        .strip_suffix(" ... ok")
        .filter(|name| !name.is_empty() && !name.contains('\r') && !name.contains('\n'))
}

fn parse_positive_decimal(value: &str) -> Option<u64> {
    if value.is_empty()
        || value.starts_with('0')
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    value.parse().ok()
}

fn outcome_value(index: usize, outcome: OutcomeV1) -> Value {
    object([
        ("case_index", Value::from(outcome.case_index)),
        ("closed_reason", Value::from(outcome.closed_reason)),
        ("index", Value::from(index as u64)),
        (
            "production_outcome",
            Value::from(outcome.production_outcome),
        ),
        ("profile", Value::from(outcome.profile)),
        ("protocol", Value::from(outcome.protocol)),
        ("security_boundary", Value::from(outcome.security_boundary)),
    ])
}

fn transcript_name(index: usize) -> String {
    format!("privacy-protocol-case-{index:02}-transcript-v1.json")
}

fn result_name(index: usize) -> String {
    format!("privacy-protocol-case-{index:02}-result-v1.json")
}

fn decode_canonical_object(bytes: &[u8], maximum: u64) -> Result<Value, TairaAuthorityErrorV1> {
    if bytes.is_empty() || bytes.len() as u64 > maximum {
        return rejected();
    }
    let text = std::str::from_utf8(bytes).map_err(|_| TairaAuthorityErrorV1::Rejected)?;
    let value: Value = norito::json::from_str(text).map_err(|_| TairaAuthorityErrorV1::Rejected)?;
    if !value.is_object() || canonical_json_bytes(&value)? != bytes {
        return rejected();
    }
    Ok(value)
}

fn canonical_json_bytes(value: &Value) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
    let mut rendered =
        norito::json::to_json_pretty(value).map_err(|_| TairaAuthorityErrorV1::Rejected)?;
    rendered.push('\n');
    Ok(rendered.into_bytes())
}

fn domain_id(domain: &[u8], value: &Value) -> Result<[u8; 32], TairaAuthorityErrorV1> {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(canonical_json_bytes(value)?);
    Ok(digest.finalize().into())
}

fn domain_id_without_field(
    domain: &[u8],
    value: &Map,
    field: &str,
) -> Result<[u8; 32], TairaAuthorityErrorV1> {
    let mut body = value.clone();
    if body.remove(field).is_none() {
        return rejected();
    }
    domain_id(domain, &Value::Object(body))
}

fn read_named_artifact(
    manifest: &[TairaAuthorityArtifactManifestEntryV1],
    artifacts: &mut [File],
    name: &str,
    maximum: u64,
) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
    let index = artifact_index(manifest, name, maximum)?;
    let expected = &manifest[index];
    let capacity = usize::try_from(expected.size).map_err(|_| TairaAuthorityErrorV1::Rejected)?;
    let file = artifacts
        .get_mut(index)
        .ok_or(TairaAuthorityErrorV1::Rejected)?;
    file.seek(SeekFrom::Start(0))
        .map_err(|_| TairaAuthorityErrorV1::Rejected)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(capacity)
        .map_err(|_| TairaAuthorityErrorV1::Rejected)?;
    file.take(expected.size.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| TairaAuthorityErrorV1::Rejected)?;
    if bytes.len() != capacity || sha256(&bytes) != expected.sha256 {
        return rejected();
    }
    Ok(bytes)
}

fn hash_named_artifact(
    manifest: &[TairaAuthorityArtifactManifestEntryV1],
    artifacts: &mut [File],
    name: &str,
    maximum: u64,
) -> Result<[u8; 32], TairaAuthorityErrorV1> {
    let index = artifact_index(manifest, name, maximum)?;
    let expected = &manifest[index];
    let file = artifacts
        .get_mut(index)
        .ok_or(TairaAuthorityErrorV1::Rejected)?;
    file.seek(SeekFrom::Start(0))
        .map_err(|_| TairaAuthorityErrorV1::Rejected)?;
    let mut reader = file.take(expected.size.saturating_add(1));
    let mut digest = Sha256::new();
    let mut observed = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|_| TairaAuthorityErrorV1::Rejected)?;
        if count == 0 {
            break;
        }
        observed = observed
            .checked_add(count as u64)
            .ok_or(TairaAuthorityErrorV1::Rejected)?;
        digest.update(&buffer[..count]);
    }
    let actual: [u8; 32] = digest.finalize().into();
    if observed != expected.size || actual != expected.sha256 {
        return rejected();
    }
    Ok(actual)
}

fn artifact_index(
    manifest: &[TairaAuthorityArtifactManifestEntryV1],
    name: &str,
    maximum: u64,
) -> Result<usize, TairaAuthorityErrorV1> {
    let index = manifest
        .iter()
        .position(|entry| entry.name == name)
        .ok_or(TairaAuthorityErrorV1::Rejected)?;
    let expected = &manifest[index];
    if expected.size == 0 || expected.size > maximum {
        return rejected();
    }
    Ok(index)
}

fn exact_object<'a>(value: &'a Value, fields: &[&str]) -> Result<&'a Map, TairaAuthorityErrorV1> {
    let object = value.as_object().ok_or(TairaAuthorityErrorV1::Rejected)?;
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return rejected();
    }
    Ok(object)
}

fn required_str<'a>(object: &'a Map, field: &str) -> Result<&'a str, TairaAuthorityErrorV1> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or(TairaAuthorityErrorV1::Rejected)
}

fn required_u64_allow_zero(object: &Map, field: &str) -> Result<u64, TairaAuthorityErrorV1> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .ok_or(TairaAuthorityErrorV1::Rejected)
}

fn required_u64_positive(object: &Map, field: &str) -> Result<u64, TairaAuthorityErrorV1> {
    required_u64_allow_zero(object, field).and_then(|value| {
        (value > 0)
            .then_some(value)
            .ok_or(TairaAuthorityErrorV1::Rejected)
    })
}

fn required_commit<'a>(object: &'a Map, field: &str) -> Result<&'a str, TairaAuthorityErrorV1> {
    let value = required_str(object, field)?;
    (value.len() == 40 && value.bytes().all(is_lower_hex))
        .then_some(value)
        .ok_or(TairaAuthorityErrorV1::Rejected)
}

fn required_digest(object: &Map, field: &str) -> Result<[u8; 32], TairaAuthorityErrorV1> {
    digest_from_str(required_str(object, field)?)
}

fn digest_from_str(value: &str) -> Result<[u8; 32], TairaAuthorityErrorV1> {
    if value.len() != 64 || !value.bytes().all(is_lower_hex) {
        return rejected();
    }
    hex::decode(value)
        .map_err(|_| TairaAuthorityErrorV1::Rejected)?
        .try_into()
        .map_err(|_| TairaAuthorityErrorV1::Rejected)
}

fn is_lower_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn rejected<T>() -> Result<T, TairaAuthorityErrorV1> {
    Err(TairaAuthorityErrorV1::Rejected)
}

fn object<const N: usize>(fields: [(&str, Value); N]) -> Value {
    let mut object = Map::new();
    for (name, value) in fields {
        object.insert(name.to_owned(), value);
    }
    Value::Object(object)
}

fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        output.push(char::from(TABLE[usize::from(first >> 2)]));
        output.push(char::from(
            TABLE[usize::from(((first & 0x03) << 4) | (second >> 4))],
        ));
        output.push(if chunk.len() > 1 {
            char::from(TABLE[usize::from(((second & 0x0f) << 2) | (third >> 6))])
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            char::from(TABLE[usize::from(third & 0x3f)])
        } else {
            '='
        });
    }
    output
}

fn decode_base64(value: &str) -> Option<Vec<u8>> {
    if value.len() % 4 != 0 {
        return None;
    }
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(value.len() / 4 * 3);
    for (block_index, chunk) in bytes.chunks_exact(4).enumerate() {
        let last = block_index + 1 == bytes.len() / 4;
        let a = base64_value(chunk[0])?;
        let b = base64_value(chunk[1])?;
        let c = if chunk[2] == b'=' {
            if !last || chunk[3] != b'=' || b & 0x0f != 0 {
                return None;
            }
            0
        } else {
            base64_value(chunk[2])?
        };
        let d = if chunk[3] == b'=' {
            if !last || chunk[2] == b'=' && c != 0 || chunk[2] != b'=' && c & 0x03 != 0 {
                return None;
            }
            0
        } else {
            if chunk[2] == b'=' {
                return None;
            }
            base64_value(chunk[3])?
        };
        output.push((a << 2) | (b >> 4));
        if chunk[2] != b'=' {
            output.push((b << 4) | (c >> 2));
        }
        if chunk[3] != b'=' {
            output.push((c << 6) | d);
        }
    }
    (encode_base64(&output) == value).then_some(output)
}

fn base64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::{fs, path::PathBuf};

    const NOW_UNIX: u64 = 1_900_000_000;

    struct Fixture {
        _directory: tempfile::TempDir,
        subject: Value,
        manifest: Vec<TairaAuthorityArtifactManifestEntryV1>,
        artifacts: Vec<File>,
        paths: Vec<PathBuf>,
    }

    impl Fixture {
        fn validate(&mut self, now_unix: u64) -> Result<Value, TairaAuthorityErrorV1> {
            validate_privacy_protocol_origin_v1(
                &self.subject,
                &self.manifest,
                &mut self.artifacts,
                now_unix,
            )
        }

        fn artifact_index(&self, name: &str) -> usize {
            self.manifest
                .iter()
                .position(|entry| entry.name == name)
                .expect("fixture artifact")
        }

        fn load_json(&self, name: &str) -> Value {
            let bytes =
                fs::read(&self.paths[self.artifact_index(name)]).expect("read fixture JSON");
            norito::json::from_str(std::str::from_utf8(&bytes).expect("UTF-8 fixture"))
                .expect("parse fixture JSON")
        }

        fn write_bytes(&mut self, name: &str, bytes: &[u8]) {
            let index = self.artifact_index(name);
            fs::write(&self.paths[index], bytes).expect("rewrite fixture artifact");
            self.manifest[index].size = bytes.len() as u64;
            self.manifest[index].sha256 = sha256(bytes);
        }

        fn write_json(&mut self, name: &str, value: &Value) -> Vec<u8> {
            let bytes = canonical_json_bytes(value).expect("canonical fixture JSON");
            self.write_bytes(name, &bytes);
            bytes
        }

        fn synchronize_outer_receipt(&mut self, receipt: &Value) {
            let receipt = receipt.as_object().expect("receipt object");
            let receipt_id = receipt.get("receipt_id").cloned().expect("receipt ID");
            self.subject
                .get_mut("expected")
                .and_then(Value::as_object_mut)
                .expect("expected object")
                .insert("receipt_id".into(), receipt_id.clone());
            let structural = self
                .subject
                .get_mut("structural_subject")
                .and_then(Value::as_object_mut)
                .expect("structural object");
            structural.insert("receipt_id".into(), receipt_id);
            structural.insert(
                "cases".into(),
                receipt.get("cases").cloned().expect("receipt cases"),
            );
            structural.insert(
                "outcomes".into(),
                receipt.get("outcomes").cloned().expect("receipt outcomes"),
            );
        }

        fn rewrite_receipt(&mut self, mut receipt: Value) {
            receipt
                .as_object_mut()
                .expect("receipt object")
                .remove("receipt_id");
            let receipt_id = domain_id(
                b"iroha.taira.privacy_protocol_four_peer_receipt.v2\0",
                &receipt,
            )
            .expect("receipt ID");
            receipt
                .as_object_mut()
                .expect("receipt object")
                .insert("receipt_id".into(), Value::from(hex::encode(receipt_id)));
            self.write_json(RECEIPT_NAME_V2, &receipt);
            self.synchronize_outer_receipt(&receipt);
        }

        fn replace_case_output(&mut self, index: usize, output: &[u8]) {
            let transcript_artifact = format!("evidence/{}", transcript_name(index));
            let mut transcript = self.load_json(&transcript_artifact);
            let command = transcript
                .get_mut("commands")
                .and_then(Value::as_array_mut)
                .and_then(|commands| commands.first_mut())
                .and_then(Value::as_object_mut)
                .expect("first transcript command");
            command.insert("output_base64".into(), Value::from(encode_base64(output)));
            command.insert(
                "output_sha256".into(),
                Value::from(hex::encode(sha256(output))),
            );
            command.insert("output_size".into(), Value::from(output.len() as u64));
            transcript
                .as_object_mut()
                .expect("transcript object")
                .remove("transcript_id");
            let transcript_id = domain_id(
                b"iroha.taira.privacy_protocol_case_transcript.v1\0",
                &transcript,
            )
            .expect("transcript ID");
            transcript
                .as_object_mut()
                .expect("transcript object")
                .insert(
                    "transcript_id".into(),
                    Value::from(hex::encode(transcript_id)),
                );
            let transcript_bytes = self.write_json(&transcript_artifact, &transcript);

            let result_artifact = format!("evidence/{}", result_name(index));
            let mut result = self.load_json(&result_artifact);
            {
                let result_object = result.as_object_mut().expect("result object");
                result_object.insert(
                    "transcript_id".into(),
                    Value::from(hex::encode(transcript_id)),
                );
                result_object.insert(
                    "transcript_sha256".into(),
                    Value::from(hex::encode(sha256(&transcript_bytes))),
                );
                result_object.insert(
                    "transcript_size".into(),
                    Value::from(transcript_bytes.len() as u64),
                );
                result_object.remove("result_id");
            }
            let result_id = domain_id(b"iroha.taira.privacy_protocol_case_result.v1\0", &result)
                .expect("result ID");
            result
                .as_object_mut()
                .expect("result object")
                .insert("result_id".into(), Value::from(hex::encode(result_id)));
            let result_bytes = self.write_json(&result_artifact, &result);

            let mut receipt = self.load_json(RECEIPT_NAME_V2);
            let row = receipt
                .get_mut("cases")
                .and_then(Value::as_array_mut)
                .and_then(|rows| rows.get_mut(index))
                .and_then(Value::as_object_mut)
                .expect("receipt case row");
            row.insert(
                "transcript_id".into(),
                Value::from(hex::encode(transcript_id)),
            );
            row.insert(
                "transcript_sha256".into(),
                Value::from(hex::encode(sha256(&transcript_bytes))),
            );
            row.insert(
                "transcript_size".into(),
                Value::from(transcript_bytes.len() as u64),
            );
            row.insert("result_id".into(), Value::from(hex::encode(result_id)));
            row.insert(
                "result_sha256".into(),
                Value::from(hex::encode(sha256(&result_bytes))),
            );
            row.insert("result_size".into(), Value::from(result_bytes.len() as u64));
            self.rewrite_receipt(receipt);
        }
    }

    #[test]
    fn accepts_exact_python_v2_contract_and_returns_reconstructed_subject() {
        let mut fixture = fixture();
        let expected = fixture
            .subject
            .get("structural_subject")
            .cloned()
            .expect("structural fixture");
        assert_eq!(fixture.validate(NOW_UNIX), Ok(expected));
    }

    #[test]
    fn candidate_domain_digest_matches_python_golden() {
        let source = object([
            ("cargo_lock_sha256", Value::from("33".repeat(32))),
            ("commit", Value::from("11".repeat(20))),
            ("dpn_validator_release_commit", Value::from("22".repeat(20))),
            (
                "workspace_source_manifest_sha256",
                Value::from("44".repeat(32)),
            ),
        ]);
        let drivers = object([
            (ACTION_DRIVER_V1, Value::from("aa".repeat(32))),
            (JINDO_DRIVER_V1, Value::from("bb".repeat(32))),
            (NETWORK_DRIVER_V1, Value::from("cc".repeat(32))),
        ]);
        let candidate = object([
            ("artifact_handoff_sha256", Value::from("55".repeat(32))),
            ("drivers", drivers),
            ("exact12_matrix_sha256", Value::from("66".repeat(32))),
            ("linux_release_archive_sha256", Value::from("77".repeat(32))),
            ("source", source),
            ("validator_binary_sha256", Value::from("88".repeat(32))),
        ]);
        assert_eq!(
            hex::encode(
                domain_id(b"iroha.taira.privacy_protocol_candidate.v2\0", &candidate,).unwrap()
            ),
            "f0487dc5279fcdf628f5f4d53644ad7938ecd065e42fe8c53d245c0f32d50254"
        );
    }

    #[test]
    fn rejects_each_outer_and_expected_field_mutation() {
        for field in SUBJECT_FIELDS_V1 {
            let mut fixture = fixture();
            fixture
                .subject
                .as_object_mut()
                .expect("subject object")
                .insert(field.into(), Value::Null);
            assert_eq!(
                fixture.validate(NOW_UNIX),
                Err(TairaAuthorityErrorV1::Rejected),
                "outer field {field}"
            );
        }
        for field in EXPECTED_FIELDS_V1 {
            let mut fixture = fixture();
            fixture
                .subject
                .get_mut("expected")
                .and_then(Value::as_object_mut)
                .expect("expected object")
                .insert(field.into(), Value::Null);
            assert_eq!(
                fixture.validate(NOW_UNIX),
                Err(TairaAuthorityErrorV1::Rejected),
                "expected field {field}"
            );
        }
    }

    #[test]
    fn rejects_every_manifest_name_ordinal_size_and_digest_mutation() {
        for index in 0..MANIFEST_V2.len() {
            let mut candidate = fixture();
            candidate.manifest[index].name.push_str("-substituted");
            assert_eq!(
                candidate.validate(NOW_UNIX),
                Err(TairaAuthorityErrorV1::Rejected),
                "manifest name {index}"
            );

            let mut candidate = fixture();
            candidate.manifest[index].ordinal = u16::MAX;
            assert_eq!(
                candidate.validate(NOW_UNIX),
                Err(TairaAuthorityErrorV1::Rejected),
                "manifest ordinal {index}"
            );

            let mut candidate = fixture();
            candidate.manifest[index].size = candidate.manifest[index].size.saturating_add(1);
            assert_eq!(
                candidate.validate(NOW_UNIX),
                Err(TairaAuthorityErrorV1::Rejected),
                "manifest size {index}"
            );

            let mut candidate = fixture();
            candidate.manifest[index].sha256[0] ^= 0x80;
            assert_eq!(
                candidate.validate(NOW_UNIX),
                Err(TairaAuthorityErrorV1::Rejected),
                "manifest digest {index}"
            );
        }
    }

    #[test]
    fn rejects_stale_and_future_receipts_using_authority_time() {
        let mut stale = fixture();
        assert_eq!(
            stale.validate(NOW_UNIX + 3_601),
            Err(TairaAuthorityErrorV1::Rejected)
        );
        let mut future = fixture();
        assert_eq!(
            future.validate(NOW_UNIX - MAX_FUTURE_CLOCK_SKEW_SECONDS_V2 - 1),
            Err(TairaAuthorityErrorV1::Rejected)
        );
    }

    #[test]
    fn recomputed_self_hashes_cannot_promote_marker_only_output() {
        let mut fixture = fixture();
        let case = CASE_DEFINITIONS_V1[0].0;
        let output = format!(
            "TAIRA_PRIVACY_PROTOCOL_FOUR_PEER_CASE_V1:{case}:passed\n\
             running 1 test\n\
             test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n"
        );
        fixture.replace_case_output(0, output.as_bytes());
        assert_eq!(
            fixture.validate(NOW_UNIX),
            Err(TairaAuthorityErrorV1::Rejected)
        );
    }

    #[test]
    fn rejects_each_recomputed_outcome_substitution() {
        for index in 0..OUTCOMES_V1.len() {
            let mut fixture = fixture();
            let mut receipt = fixture.load_json(RECEIPT_NAME_V2);
            receipt
                .get_mut("outcomes")
                .and_then(Value::as_array_mut)
                .and_then(|rows| rows.get_mut(index))
                .and_then(Value::as_object_mut)
                .expect("outcome row")
                .insert("profile".into(), Value::from("engine-unavailable"));
            fixture.rewrite_receipt(receipt);
            assert_eq!(
                fixture.validate(NOW_UNIX),
                Err(TairaAuthorityErrorV1::Rejected),
                "outcome {index}"
            );
        }
    }

    fn fixture() -> Fixture {
        let directory = tempfile::tempdir().expect("fixture directory");
        let source = object([
            ("cargo_lock_sha256", Value::from("33".repeat(32))),
            ("commit", Value::from("11".repeat(20))),
            ("dpn_validator_release_commit", Value::from("22".repeat(20))),
            (
                "workspace_source_manifest_sha256",
                Value::from("44".repeat(32)),
            ),
        ]);
        let driver_payloads = [
            (ACTION_DRIVER_V1, b"native-action-driver-v1\0".as_slice()),
            (JINDO_DRIVER_V1, b"native-jindo-test-driver-v1\0".as_slice()),
            (
                NETWORK_DRIVER_V1,
                b"native-network-test-driver-v1\0".as_slice(),
            ),
        ];
        let drivers = object(
            driver_payloads
                .map(|(name, payload)| (name, Value::from(hex::encode(sha256(payload))))),
        );
        let candidate = object([
            ("artifact_handoff_sha256", Value::from("55".repeat(32))),
            ("drivers", drivers.clone()),
            ("exact12_matrix_sha256", Value::from("66".repeat(32))),
            ("linux_release_archive_sha256", Value::from("77".repeat(32))),
            ("source", source.clone()),
            ("validator_binary_sha256", Value::from("88".repeat(32))),
        ]);
        let candidate_id = domain_id(b"iroha.taira.privacy_protocol_candidate.v2\0", &candidate)
            .expect("candidate ID");
        let driver_map = drivers.as_object().expect("driver map");
        let mut payloads = BTreeMap::<String, Vec<u8>>::new();
        for ((_, artifact_name), (_, payload)) in DRIVER_EVIDENCE_V1.iter().zip(driver_payloads) {
            payloads.insert((*artifact_name).to_owned(), payload.to_vec());
        }

        let mut case_rows = Vec::new();
        for (index, (case, kind)) in CASE_DEFINITIONS_V1.iter().copied().enumerate() {
            let command_count = if index == 4 { 2 } else { 1 };
            let mut commands = Vec::with_capacity(command_count);
            for command_index in 0..command_count {
                let driver = if command_index == 0 {
                    NETWORK_DRIVER_V1
                } else {
                    JINDO_DRIVER_V1
                };
                let output = if driver == NETWORK_DRIVER_V1 {
                    network_output(case)
                } else {
                    jindo_output()
                };
                commands.push(object([
                    ("args", command_args(index, command_index)),
                    ("driver", Value::from(driver)),
                    (
                        "driver_sha256",
                        driver_map.get(driver).cloned().expect("driver digest"),
                    ),
                    ("exit_code", Value::from(0_u64)),
                    ("index", Value::from(command_index as u64)),
                    ("output_base64", Value::from(encode_base64(&output))),
                    ("output_sha256", Value::from(hex::encode(sha256(&output)))),
                    ("output_size", Value::from(output.len() as u64)),
                ]));
            }
            let mut transcript = object([
                (
                    "candidate_binding_sha256",
                    Value::from(hex::encode(candidate_id)),
                ),
                ("case", Value::from(case)),
                ("commands", Value::Array(commands)),
                ("index", Value::from(index as u64)),
                ("kind", Value::from(kind)),
                ("schema", Value::from(TRANSCRIPT_SCHEMA_V1)),
                ("schema_version", Value::from(1_u64)),
            ]);
            let transcript_id = domain_id(
                b"iroha.taira.privacy_protocol_case_transcript.v1\0",
                &transcript,
            )
            .expect("transcript ID");
            transcript
                .as_object_mut()
                .expect("transcript object")
                .insert(
                    "transcript_id".into(),
                    Value::from(hex::encode(transcript_id)),
                );
            let transcript_bytes = canonical_json_bytes(&transcript).expect("transcript JSON");
            let transcript_basename = transcript_name(index);
            payloads.insert(
                format!("evidence/{transcript_basename}"),
                transcript_bytes.clone(),
            );

            let mut result = object([
                (
                    "candidate_binding_sha256",
                    Value::from(hex::encode(candidate_id)),
                ),
                ("case", Value::from(case)),
                ("index", Value::from(index as u64)),
                ("kind", Value::from(kind)),
                ("schema", Value::from(RESULT_SCHEMA_V1)),
                ("schema_version", Value::from(1_u64)),
                ("status", Value::from("passed")),
                ("transcript_id", Value::from(hex::encode(transcript_id))),
                ("transcript_path", Value::from(transcript_basename.clone())),
                (
                    "transcript_sha256",
                    Value::from(hex::encode(sha256(&transcript_bytes))),
                ),
                (
                    "transcript_size",
                    Value::from(transcript_bytes.len() as u64),
                ),
            ]);
            let result_id = domain_id(b"iroha.taira.privacy_protocol_case_result.v1\0", &result)
                .expect("result ID");
            result
                .as_object_mut()
                .expect("result object")
                .insert("result_id".into(), Value::from(hex::encode(result_id)));
            let result_bytes = canonical_json_bytes(&result).expect("result JSON");
            let result_basename = result_name(index);
            payloads.insert(format!("evidence/{result_basename}"), result_bytes.clone());
            case_rows.push(object([
                ("case", Value::from(case)),
                ("index", Value::from(index as u64)),
                ("kind", Value::from(kind)),
                ("result_id", Value::from(hex::encode(result_id))),
                ("result_path", Value::from(result_basename)),
                (
                    "result_sha256",
                    Value::from(hex::encode(sha256(&result_bytes))),
                ),
                ("result_size", Value::from(result_bytes.len() as u64)),
                ("transcript_id", Value::from(hex::encode(transcript_id))),
                ("transcript_path", Value::from(transcript_basename)),
                (
                    "transcript_sha256",
                    Value::from(hex::encode(sha256(&transcript_bytes))),
                ),
                (
                    "transcript_size",
                    Value::from(transcript_bytes.len() as u64),
                ),
            ]));
        }

        let outcomes = Value::Array(
            OUTCOMES_V1
                .into_iter()
                .enumerate()
                .map(|(index, outcome)| outcome_value(index, outcome))
                .collect(),
        );
        let mut receipt = object([
            ("candidate", candidate.clone()),
            ("cases", Value::Array(case_rows.clone())),
            ("expires_at_unix", Value::from(NOW_UNIX + 3_600)),
            ("issued_at_unix", Value::from(NOW_UNIX)),
            ("outcomes", outcomes.clone()),
            (
                "platform",
                object([
                    ("arch", Value::from("arm64")),
                    ("os", Value::from("macos")),
                    ("peer_count", Value::from(PEER_COUNT_V2)),
                ]),
            ),
            ("schema", Value::from(RECEIPT_SCHEMA_V2)),
            ("schema_version", Value::from(RECEIPT_SCHEMA_VERSION_V2)),
        ]);
        let receipt_id = domain_id(
            b"iroha.taira.privacy_protocol_four_peer_receipt.v2\0",
            &receipt,
        )
        .expect("receipt ID");
        receipt
            .as_object_mut()
            .expect("receipt object")
            .insert("receipt_id".into(), Value::from(hex::encode(receipt_id)));
        payloads.insert(
            RECEIPT_NAME_V2.to_owned(),
            canonical_json_bytes(&receipt).expect("receipt JSON"),
        );

        let structural = object([
            ("artifact_handoff_sha256", Value::from("55".repeat(32))),
            ("case_count", Value::from(CASE_DEFINITIONS_V1.len() as u64)),
            ("cases", Value::Array(case_rows)),
            ("drivers", drivers),
            ("exact12_matrix_sha256", Value::from("66".repeat(32))),
            ("linux_release_archive_sha256", Value::from("77".repeat(32))),
            ("outcomes", outcomes),
            ("receipt_id", Value::from(hex::encode(receipt_id))),
            ("validator_binary_sha256", Value::from("88".repeat(32))),
        ]);
        let expected = object([
            ("artifact_handoff_sha256", Value::from("55".repeat(32))),
            ("exact12_matrix_sha256", Value::from("66".repeat(32))),
            ("linux_release_archive_sha256", Value::from("77".repeat(32))),
            ("receipt_id", Value::from(hex::encode(receipt_id))),
            ("source", source),
            ("validator_binary_sha256", Value::from("88".repeat(32))),
        ]);
        let subject = object([
            ("authority_schema", Value::from(AUTHORITY_SCHEMA_V1)),
            (
                "authenticated_run_schema",
                Value::from(AUTHENTICATED_RUN_SCHEMA_V1),
            ),
            ("expected", expected),
            ("replay_namespace", Value::from(REPLAY_NAMESPACE_V1)),
            ("structural_subject", structural),
            ("validation_time_unix", Value::from(NOW_UNIX)),
        ]);

        let mut manifest = Vec::new();
        let mut artifacts = Vec::new();
        let mut paths = Vec::new();
        for (ordinal, (name, _)) in MANIFEST_V2.into_iter().enumerate() {
            let bytes = payloads.remove(name).expect("fixture payload");
            let path = directory.path().join(format!("artifact-{ordinal:02}"));
            fs::write(&path, &bytes).expect("write fixture artifact");
            paths.push(path.clone());
            artifacts.push(File::open(path).expect("open fixture artifact"));
            manifest.push(TairaAuthorityArtifactManifestEntryV1 {
                ordinal: ordinal as u16,
                name: name.to_owned(),
                size: bytes.len() as u64,
                sha256: sha256(&bytes),
            });
        }
        assert!(payloads.is_empty());
        Fixture {
            _directory: directory,
            subject,
            manifest,
            artifacts,
            paths,
        }
    }

    /// Return the exact semantic fixture as portable bytes for the full
    /// authority-service tests.  Those tests restage the artifacts under the
    /// authority service identity before exercising descriptor admission.
    pub(crate) fn service_fixture_material() -> (Value, Vec<(String, Vec<u8>)>) {
        let fixture = fixture();
        let artifacts = fixture
            .manifest
            .iter()
            .zip(&fixture.paths)
            .map(|(entry, path)| {
                (
                    entry.name.clone(),
                    fs::read(path).expect("read privacy-protocol service fixture artifact"),
                )
            })
            .collect();
        (fixture.subject, artifacts)
    }

    fn network_output(case: &str) -> Vec<u8> {
        format!(
            "running 1 test\n\
             test {case} ... ok\n\n\
             test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 100 filtered out; finished in 1.00s\n"
        )
        .into_bytes()
    }

    fn jindo_output() -> Vec<u8> {
        b"running 1 test\n\
          test privacy_engines::jindo::security::tests::release_boundary ... ok\n\n\
          test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 100 filtered out; finished in 0.01s\n"
            .to_vec()
    }
}
