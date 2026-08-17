//! Independent semantic validation for the native-evidence authority role.
//!
//! Descriptor ownership, immutability, size, and SHA-256 are established by
//! the authority service before this module is called. This module validates
//! the role-specific subject and the contents linked by those descriptors.

use super::{protocol::TairaAuthorityArtifactManifestEntryV1, service::TairaAuthorityErrorV1};
use iroha_data_model::privacy::PrivacyProtocolIdV1;
use norito::{
    codec::{Decode, Encode},
    derive::{JsonDeserialize, JsonSerialize},
    json::{Map, Value},
};
use sha2::{Digest as _, Sha256};
use sorafs_car::bundle_archive::{
    BUNDLE_ARCHIVE_PROTOCOL_MAX_COMPRESSED_BYTES, BUNDLE_ARCHIVE_PROTOCOL_MAX_DECODED_BYTES,
    BUNDLE_ARCHIVE_PROTOCOL_MAX_ENTRIES, BUNDLE_ARCHIVE_PROTOCOL_MAX_FILE_BYTES,
    BUNDLE_ARCHIVE_PROTOCOL_MAX_TOTAL_FILE_BYTES, BundleArchiveEntryKind, BundleArchiveLimits,
    visit_gzip_ustar,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{self, Read, Seek as _, SeekFrom},
};

const SUBJECT_SCHEMA_V1: &str = "iroha.taira.exact12_release_authority";
const NATIVE_VERIFIER_PROTOCOL_V1: &str = "sorafs-validate-release-manifest-v1";
const REGISTRY_SHA256_V1: &str = "734eafb58f0c54f5319b9cc26557920e564453f689071931393dcdba91123e51";
const PROTOCOL_COUNT_V1: usize = 12;
const CASE_COUNT_V1: usize = 4;
const STAGE_COUNT_V1: usize = PROTOCOL_COUNT_V1 * CASE_COUNT_V1;
const MAX_SMALL_ARTIFACT_BYTES_V1: u64 = 2 * 1024 * 1024;
const MAX_PROOF_ARTIFACT_BYTES_V1: u64 = 9 * 1024 * 1024;
const MAX_TOTAL_PROOF_ARTIFACTS_V1: usize = STAGE_COUNT_V1 + 6;
const MAX_TOTAL_PROOF_BYTES_V1: u64 =
    MAX_PROOF_ARTIFACT_BYTES_V1 * MAX_TOTAL_PROOF_ARTIFACTS_V1 as u64;
const MAX_TYPED_NORITO_BYTES_V1: u64 =
    MAX_TOTAL_PROOF_BYTES_V1 + STAGE_COUNT_V1 as u64 * 4 * 1024 + 64 * 1024;
const MAX_TYPED_JSON_BYTES_V1: u64 = MAX_TOTAL_PROOF_ARTIFACTS_V1 as u64
    * (MAX_PROOF_ARTIFACT_BYTES_V1.div_ceil(3) * 4)
    + STAGE_COUNT_V1 as u64 * 8 * 1024
    + 256 * 1024;

const TOP_LEVEL_FIELDS_V1: [&str; 12] = [
    "commit",
    "dpn_validator_release_commit",
    "exact12",
    "native_release_evidence",
    "native_verifier_protocol",
    "native_verifier_sha256",
    "release_profile",
    "schema",
    "schema_version",
    "signing_authority_fingerprint_sha256",
    "subject",
    "workspace_source_manifest_sha256",
];

const PROTOCOLS_V1: [(&str, &str); PROTOCOL_COUNT_V1] = [
    ("zk-ace-pq-authorization-v0", "ZkAcePqAuthorizationV0"),
    ("anonymous-pgc-k-out-of-n-v1", "AnonymousPgcKOutOfNV1"),
    ("verange-transparent-range-v1", "VeRangeTransparentRangeV1"),
    ("iroha-zk-ams-v1", "IrohaZkAmsV1"),
    (
        "vega-existing-credential-zk-v0",
        "VegaExistingCredentialZkV0",
    ),
    ("iroha-zk-x509-stark-p256-v0", "IrohaZkX509StarkP256V0"),
    (
        "iroha-jindo-polynomial-commitment-v0",
        "IrohaJindoPolynomialCommitmentV0",
    ),
    (
        "iroha-bootle-lantern-anoncred-v1",
        "IrohaBootleLanternAnoncredV1",
    ),
    ("orchard-halo2-actions-v1", "OrchardHalo2ActionsV1"),
    ("monero-fcmp-plus-plus-v1", "MoneroFcmpPlusPlusV1"),
    (
        "iroha-ivm-private-note-stark-v1",
        "IrohaIvmPrivateNoteStarkV1",
    ),
    ("pq-masp-stark-v0", "PqMaspStarkV0"),
];

const RETIRED_LABELS_V1: [&str; 10] = [
    "zkat-policy-private-auth-v1",
    "zk-ams-recursive-admission-v0",
    "silent-threshold-anoncred-v0",
    "zk-x509-onchain-identity-v0",
    "jindo-lattice-pcs-zk-v0",
    "sis-hints-anoncred-pq-v0",
    "sis-with-hints",
    "penumbra-masp-v1",
    "miden-stark-note-v1",
    "aztec-private-rollup-v1",
];

// Subject rows are sorted by their stable symbolic key.
const EVIDENCE_ROWS_V1: [(&str, &str); 16] = [
    ("cargo_lock", "provenance/Cargo.lock"),
    (
        "command_manifest_json",
        "provenance/privacy-native/command-manifest-v1.json",
    ),
    (
        "command_manifest_norito",
        "provenance/privacy-native/command-manifest-v1.norito",
    ),
    (
        "dpn_validator_build_provenance",
        "provenance/dpn-validator-build.provenance.json",
    ),
    ("exact12_matrix", "provenance/privacy-native/exact12-v1.tsv"),
    (
        "expectations_json",
        "provenance/privacy-native/expectations-v1.json",
    ),
    (
        "expectations_norito",
        "provenance/privacy-native/expectations-v1.norito",
    ),
    ("receipt_json", "provenance/privacy-native/receipt-v1.json"),
    (
        "receipt_norito",
        "provenance/privacy-native/receipt-v1.norito",
    ),
    ("runner_binary", "bin/taira_privacy_release_runner"),
    (
        "stage_artifacts_json",
        "provenance/privacy-native/stage-artifacts-v1.json",
    ),
    (
        "stage_artifacts_norito",
        "provenance/privacy-native/stage-artifacts-v1.norito",
    ),
    ("validator_binary", "bin/iroha3d"),
    (
        "workspace_source_manifest",
        "provenance/privacy-native/workspace-source-manifest.sha256",
    ),
    (
        "x509_resource_json",
        "provenance/privacy-native/zk-x509-resource-v1.json",
    ),
    (
        "x509_resource_norito",
        "provenance/privacy-native/zk-x509-resource-v1.norito",
    ),
];

// Descriptor order follows the Python EVIDENCE_PATHS declaration, not the
// alphabetically sorted subject rows above.
const MANIFEST_NAMES_V1: [&str; 16] = [
    "evidence/provenance/Cargo.lock",
    "evidence/provenance/dpn-validator-build.provenance.json",
    "evidence/provenance/privacy-native/command-manifest-v1.json",
    "evidence/provenance/privacy-native/command-manifest-v1.norito",
    "evidence/provenance/privacy-native/exact12-v1.tsv",
    "evidence/provenance/privacy-native/expectations-v1.json",
    "evidence/provenance/privacy-native/expectations-v1.norito",
    "evidence/provenance/privacy-native/zk-x509-resource-v1.json",
    "evidence/provenance/privacy-native/zk-x509-resource-v1.norito",
    "evidence/provenance/privacy-native/receipt-v1.json",
    "evidence/provenance/privacy-native/receipt-v1.norito",
    "evidence/bin/taira_privacy_release_runner",
    "evidence/provenance/privacy-native/stage-artifacts-v1.json",
    "evidence/provenance/privacy-native/stage-artifacts-v1.norito",
    "evidence/bin/iroha3d",
    "evidence/provenance/privacy-native/workspace-source-manifest.sha256",
];

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct BuildProvenanceV1 {
    dpn_validator_release_commit: String,
    iroha_git_head: String,
    iroha_source_attested: bool,
    iroha_source_bundle_provenance_sha256: String,
    iroha_source_tree_sha256: String,
    iroha_tracked_patch_sha256: String,
    iroha_worktree_clean: bool,
    schema_version: u16,
    validator_lock_sha256: String,
    workspace_source_manifest_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PrivacyReleaseIsolationPolicyV1 {
    stage_rayon_threads: u16,
    main_thread_stack_bytes: u64,
    rayon_worker_stack_bytes: u64,
    watchdog_thread_stack_bytes: u64,
    max_stage_tasks: u64,
    max_stage_open_files: u64,
    max_stage_result_file_bytes: u64,
    max_stage_diagnostic_bytes: u64,
    core_dump_bytes: u64,
    static_elf_only: bool,
    anonymous_sealed_runner: bool,
    anonymous_result_descriptor_only: bool,
    exact_environment_only: bool,
    landlock_abi_minimum: u16,
    seccomp_tsync: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PrivacyReleaseCommandManifestV1 {
    schema_version: u16,
    build_profile: String,
    source_sha256: [u8; 32],
    exact12_matrix_sha256: [u8; 32],
    expectations_norito_sha256: [u8; 32],
    expectations_json_sha256: [u8; 32],
    x509_resource_norito_sha256: [u8; 32],
    x509_resource_json_sha256: [u8; 32],
    cargo_lock_sha256: [u8; 32],
    validator_binary_sha256: [u8; 32],
    runner_binary_sha256: [u8; 32],
    isolation_policy: PrivacyReleaseIsolationPolicyV1,
    command_arguments: Vec<String>,
    stage_command_template: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, JsonSerialize, JsonDeserialize)]
#[norito(
    tag = "case",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
enum PrivacyReleaseCaseKindV1 {
    PositiveCanonicalEndToEnd,
    PublicStatementBindingMutation,
    ProofCorruptionAndTruncation,
    MaximumShapeResource,
}

impl PrivacyReleaseCaseKindV1 {
    const ALL: [Self; CASE_COUNT_V1] = [
        Self::PositiveCanonicalEndToEnd,
        Self::PublicStatementBindingMutation,
        Self::ProofCorruptionAndTruncation,
        Self::MaximumShapeResource,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, JsonSerialize, JsonDeserialize)]
#[norito(
    tag = "failure_class",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
enum PrivacyReleaseFailureClassV1 {
    NotApplicable,
    PublicStatementBindingRejected,
    CanonicalWireCorruptionAndTruncationRejected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PrivacyReleaseResourceFactsV1 {
    primary_units: u64,
    primary_ceiling: u64,
    secondary_units: u64,
    secondary_ceiling: u64,
    relation_depth: u64,
    relation_depth_ceiling: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PrivacyReleaseProofArtifactEvidenceV1 {
    artifact_ordinal: u8,
    #[norito(with = "canonical_proof_bytes_v1")]
    canonical_proof_bytes: Vec<u8>,
    proof_sha256: [u8; 32],
    proof_bytes_ceiling: u64,
}

mod canonical_proof_bytes_v1 {
    use norito::json::{JsonSerialize as _, Parser};

    pub fn serialize(bytes: &[u8], out: &mut String) {
        super::encode_base64(bytes).json_serialize(out);
    }

    pub fn deserialize(parser: &mut Parser<'_>) -> Result<Vec<u8>, norito::json::Error> {
        let encoded = parser.parse_string()?;
        super::decode_base64(&encoded).ok_or_else(|| {
            norito::json::Error::Message("invalid canonical proof base64".to_owned())
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PrivacyReleaseStageEvidenceV1 {
    schema_version: u16,
    stage_ordinal: u16,
    protocol_id: PrivacyProtocolIdV1,
    case_kind: PrivacyReleaseCaseKindV1,
    protocol_descriptor: String,
    public_statement_sha256: [u8; 32],
    proof_artifacts: Vec<PrivacyReleaseProofArtifactEvidenceV1>,
    failure_class: PrivacyReleaseFailureClassV1,
    resources: PrivacyReleaseResourceFactsV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PrivacyReleaseExpectedStageV1 {
    evidence: PrivacyReleaseStageEvidenceV1,
    max_elapsed_millis: u64,
    max_peak_rss_bytes: u64,
    max_address_space_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PrivacyReleaseExpectationsV1 {
    schema_version: u16,
    stage_count: u16,
    stages: Vec<PrivacyReleaseExpectedStageV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PrivacyReleaseMeasuredStageV1 {
    evidence: PrivacyReleaseStageEvidenceV1,
    elapsed_millis: u64,
    peak_rss_bytes: u64,
    peak_address_space_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PrivacyReleaseStageArtifactsV1 {
    schema_version: u16,
    stage_count: u16,
    stages: Vec<PrivacyReleaseMeasuredStageV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PrivacyReleaseArtifactPairDigestV1 {
    norito_sha256: [u8; 32],
    json_sha256: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PrivacyReleaseReceiptV1 {
    schema_version: u16,
    build_profile: String,
    source_sha256: [u8; 32],
    exact12_matrix_sha256: [u8; 32],
    expectations: PrivacyReleaseArtifactPairDigestV1,
    x509_resource: PrivacyReleaseArtifactPairDigestV1,
    cargo_lock_sha256: [u8; 32],
    validator_binary_sha256: [u8; 32],
    runner_binary_sha256: [u8; 32],
    command_manifest: PrivacyReleaseArtifactPairDigestV1,
    stage_artifacts: PrivacyReleaseArtifactPairDigestV1,
    fixed_stage_count: u16,
    all_native_stages_passed: bool,
    contains_witnesses: bool,
    contains_canonical_proof_artifacts: bool,
    isolation_policy_enforced: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PrivacyReleaseZkX509ResourceEnvironmentV1 {
    operating_system: String,
    architecture: String,
    endianness: String,
    kernel_minimum_major: u16,
    kernel_minimum_minor: u16,
    rustc_release: String,
    rustc_host: String,
    rustc_commit_hash: String,
    rustc_commit_date: String,
    instance_type: String,
    cpu_model: String,
    logical_cpu_count: u16,
    online_cpu_count: u16,
    affinity_cpu_count: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PrivacyReleaseZkX509ResourceProcessLimitsV1 {
    elapsed_ceiling_millis: u64,
    peak_rss_ceiling_bytes: u64,
    address_space_ceiling_bytes: u64,
    main_thread_stack_bytes: u64,
    rayon_worker_stack_bytes: u64,
    watchdog_thread_stack_bytes: u64,
    rayon_worker_count: u16,
    max_stage_tasks: u16,
    max_stage_open_files: u16,
    core_dump_bytes: u64,
    landlock_abi_minimum: u16,
    minimum_effective_memory_bytes: u64,
    cgroup_v2: bool,
    cpu_quota_unlimited: bool,
    landlock_restrict_self: bool,
    anchored_openat2: bool,
    memfd_exec: bool,
    memfd_seal_exec: bool,
    static_elf_only: bool,
    seccomp_tsync: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PrivacyReleaseZkX509ResourceObservationV1 {
    case_kind: PrivacyReleaseCaseKindV1,
    elapsed_millis: u64,
    peak_rss_bytes: u64,
    peak_address_space_bytes: u64,
    primary_units: u64,
    primary_ceiling: u64,
    secondary_units: u64,
    secondary_ceiling: u64,
    relation_depth: u64,
    relation_depth_ceiling: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PrivacyReleaseZkX509ResourceCertificateV1 {
    schema_version: u16,
    protocol_id: PrivacyProtocolIdV1,
    compiled_profile_digest: [u8; 32],
    environment: PrivacyReleaseZkX509ResourceEnvironmentV1,
    expectations_norito_sha256: [u8; 32],
    expectations_json_sha256: [u8; 32],
    kat_proof_bytes: u32,
    kat_proof_sha256: [u8; 32],
    process_limits: PrivacyReleaseZkX509ResourceProcessLimitsV1,
    positive: PrivacyReleaseZkX509ResourceObservationV1,
    maximum: PrivacyReleaseZkX509ResourceObservationV1,
    certificate_sha256: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Exact12SummaryV1 {
    labels: Vec<String>,
    retired_labels: Vec<String>,
}

/// Validate one native-evidence request before the service consumes replay state.
pub(super) fn validate_native_evidence_v1(
    subject: &Value,
    manifest: &[TairaAuthorityArtifactManifestEntryV1],
    artifacts: &mut [File],
) -> Result<Value, TairaAuthorityErrorV1> {
    let subject = exact_object(subject, &TOP_LEVEL_FIELDS_V1)?;
    if required_str(subject, "schema")? != SUBJECT_SCHEMA_V1
        || required_u64(subject, "schema_version")? != 1
        || required_str(subject, "release_profile")? != "release"
        || required_str(subject, "native_verifier_protocol")? != NATIVE_VERIFIER_PROTOCOL_V1
    {
        return rejected();
    }
    let commit = required_commit(subject, "commit")?;
    let dpn_commit = required_commit(subject, "dpn_validator_release_commit")?;
    let workspace_source_sha256 = required_digest(subject, "workspace_source_manifest_sha256")?;
    required_digest(subject, "native_verifier_sha256")?;
    required_digest(subject, "signing_authority_fingerprint_sha256")?;

    let release_subject = subject
        .get("subject")
        .ok_or(TairaAuthorityErrorV1::Rejected)?;
    let archive_mode = release_subject
        .as_object()
        .and_then(|value| value.get("kind"))
        .and_then(Value::as_str)
        == Some("taira-rollout-tar-gzip-v1");
    validate_manifest_shape(manifest, artifacts.len(), archive_mode)?;
    if archive_mode {
        // Authenticate the outer archive binding before reading the larger
        // descriptor. Member identities are checked after all standalone
        // evidence descriptors have passed the shared semantic validator.
        validate_release_subject(release_subject, manifest, workspace_source_sha256)?;
    }
    let evidence = validate_evidence_rows(subject, manifest)?;

    let source_bytes = read_named_artifact(
        manifest,
        artifacts,
        "evidence/provenance/privacy-native/workspace-source-manifest.sha256",
        65,
    )?;
    if source_bytes.len() != 65
        || source_bytes[64] != b'\n'
        || digest_from_ascii(&source_bytes[..64])? != workspace_source_sha256
    {
        return rejected();
    }

    let cargo_sha256 = evidence_digest(&evidence, "cargo_lock")?;
    let exact12_sha256 = evidence_digest(&evidence, "exact12_matrix")?;
    let runner_sha256 = evidence_digest(&evidence, "runner_binary")?;
    let validator_sha256 = evidence_digest(&evidence, "validator_binary")?;
    if runner_sha256 == validator_sha256 {
        return rejected();
    }

    let provenance_bytes = read_named_artifact(
        manifest,
        artifacts,
        "evidence/provenance/dpn-validator-build.provenance.json",
        MAX_SMALL_ARTIFACT_BYTES_V1,
    )?;
    let provenance: BuildProvenanceV1 = decode_canonical_json(&provenance_bytes)?;
    if provenance.schema_version != 1
        || !provenance.iroha_source_attested
        || provenance.iroha_git_head != commit
        || provenance.dpn_validator_release_commit != dpn_commit
        || digest_from_str(&provenance.validator_lock_sha256)? != cargo_sha256
        || digest_from_str(&provenance.workspace_source_manifest_sha256)? != workspace_source_sha256
        || digest_from_str(&provenance.iroha_source_bundle_provenance_sha256).is_err()
        || digest_from_str(&provenance.iroha_source_tree_sha256).is_err()
        || digest_from_str(&provenance.iroha_tracked_patch_sha256).is_err()
    {
        return rejected();
    }

    let matrix_bytes = read_named_artifact(
        manifest,
        artifacts,
        "evidence/provenance/privacy-native/exact12-v1.tsv",
        MAX_SMALL_ARTIFACT_BYTES_V1,
    )?;
    let exact12 = validate_exact12_matrix(&matrix_bytes)?;
    validate_exact12_subject(
        subject
            .get("exact12")
            .ok_or(TairaAuthorityErrorV1::Rejected)?,
        &exact12,
    )?;

    let expectations = decode_pair::<PrivacyReleaseExpectationsV1>(
        manifest,
        artifacts,
        "evidence/provenance/privacy-native/expectations-v1.norito",
        "evidence/provenance/privacy-native/expectations-v1.json",
        MAX_TYPED_NORITO_BYTES_V1,
        MAX_TYPED_JSON_BYTES_V1,
    )?;
    validate_expectations(&expectations)?;
    let stages = decode_pair::<PrivacyReleaseStageArtifactsV1>(
        manifest,
        artifacts,
        "evidence/provenance/privacy-native/stage-artifacts-v1.norito",
        "evidence/provenance/privacy-native/stage-artifacts-v1.json",
        MAX_TYPED_NORITO_BYTES_V1,
        MAX_TYPED_JSON_BYTES_V1,
    )?;
    validate_measured_stages(&stages, &expectations)?;
    let x509 = decode_pair::<PrivacyReleaseZkX509ResourceCertificateV1>(
        manifest,
        artifacts,
        "evidence/provenance/privacy-native/zk-x509-resource-v1.norito",
        "evidence/provenance/privacy-native/zk-x509-resource-v1.json",
        MAX_SMALL_ARTIFACT_BYTES_V1,
        MAX_SMALL_ARTIFACT_BYTES_V1,
    )?;
    validate_x509_resource(&x509, &evidence)?;
    let command = decode_pair::<PrivacyReleaseCommandManifestV1>(
        manifest,
        artifacts,
        "evidence/provenance/privacy-native/command-manifest-v1.norito",
        "evidence/provenance/privacy-native/command-manifest-v1.json",
        MAX_SMALL_ARTIFACT_BYTES_V1,
        MAX_SMALL_ARTIFACT_BYTES_V1,
    )?;
    validate_command_manifest(
        &command,
        workspace_source_sha256,
        exact12_sha256,
        cargo_sha256,
        runner_sha256,
        validator_sha256,
        &evidence,
    )?;
    let receipt = decode_pair::<PrivacyReleaseReceiptV1>(
        manifest,
        artifacts,
        "evidence/provenance/privacy-native/receipt-v1.norito",
        "evidence/provenance/privacy-native/receipt-v1.json",
        MAX_SMALL_ARTIFACT_BYTES_V1,
        MAX_SMALL_ARTIFACT_BYTES_V1,
    )?;
    validate_receipt(
        &receipt,
        workspace_source_sha256,
        exact12_sha256,
        cargo_sha256,
        runner_sha256,
        validator_sha256,
        &evidence,
    )?;
    validate_release_subject(release_subject, manifest, workspace_source_sha256)?;
    if archive_mode {
        validate_archive_evidence(release_subject, &evidence, manifest, artifacts)?;
    }

    Ok(Value::Object(Map::new()))
}

fn validate_archive_evidence(
    release_subject: &Value,
    evidence: &[(String, [u8; 32], u64)],
    manifest: &[TairaAuthorityArtifactManifestEntryV1],
    artifacts: &mut [File],
) -> Result<(), TairaAuthorityErrorV1> {
    let subject = exact_object(release_subject, &["kind", "name", "sha256", "size"])?;
    if required_str(subject, "kind")? != "taira-rollout-tar-gzip-v1" {
        return rejected();
    }
    let archive_name = required_str(subject, "name")?;
    let prefix = archive_name
        .strip_suffix(".tar.gz")
        .filter(|prefix| !prefix.is_empty())
        .ok_or(TairaAuthorityErrorV1::Rejected)?;
    let archive_index = manifest
        .iter()
        .position(|entry| entry.name == "subject/release-archive")
        .ok_or(TairaAuthorityErrorV1::Rejected)?;
    let archive = &manifest[archive_index];
    if archive.size > BUNDLE_ARCHIVE_PROTOCOL_MAX_COMPRESSED_BYTES
        || archive.sha256 != required_digest(subject, "sha256")?
        || archive.size != required_u64(subject, "size")?
    {
        return rejected();
    }

    let mut expected = BTreeMap::new();
    for ((symbolic_name, relative_path), (observed_name, digest, size)) in
        EVIDENCE_ROWS_V1.iter().zip(evidence)
    {
        if symbolic_name != observed_name {
            return rejected();
        }
        let archive_path = format!("{prefix}/{relative_path}");
        if expected.insert(archive_path, (*digest, *size)).is_some() {
            return rejected();
        }
    }

    let archive_file = artifacts
        .get_mut(archive_index)
        .ok_or(TairaAuthorityErrorV1::Rejected)?;
    archive_file
        .seek(SeekFrom::Start(0))
        .map_err(|_| TairaAuthorityErrorV1::Rejected)?;
    let mut authenticated_reader = AuthenticatedArchiveReaderV1::new(archive_file);
    let limits = BundleArchiveLimits {
        max_compressed_bytes: BUNDLE_ARCHIVE_PROTOCOL_MAX_COMPRESSED_BYTES,
        max_decoded_bytes: BUNDLE_ARCHIVE_PROTOCOL_MAX_DECODED_BYTES,
        max_entries: BUNDLE_ARCHIVE_PROTOCOL_MAX_ENTRIES,
        max_file_bytes: BUNDLE_ARCHIVE_PROTOCOL_MAX_FILE_BYTES,
        max_total_file_bytes: BUNDLE_ARCHIVE_PROTOCOL_MAX_TOTAL_FILE_BYTES,
    };
    let mut verified = BTreeSet::new();
    let summary = visit_gzip_ustar(&mut authenticated_reader, limits, |entry, payload| {
        let components = entry.path_components();
        if components.first().map(String::as_str) != Some(prefix) {
            return Err(invalid_archive_member());
        }
        if components.len() == 1 {
            if entry.kind() != BundleArchiveEntryKind::Directory {
                return Err(invalid_archive_member());
            }
            return io::copy(payload, &mut io::sink()).map(|_| ());
        }
        if let Some((expected_digest, expected_size)) = expected.get(entry.path()) {
            if entry.kind() != BundleArchiveEntryKind::File || entry.size() != *expected_size {
                return Err(invalid_archive_member());
            }
            let (observed_digest, observed_size) = hash_archive_member(payload)?;
            if observed_size != *expected_size
                || observed_digest != *expected_digest
                || !verified.insert(entry.path().to_owned())
            {
                return Err(invalid_archive_member());
            }
        } else {
            io::copy(payload, &mut io::sink())?;
        }
        Ok(())
    })
    .map_err(|_| TairaAuthorityErrorV1::Rejected)?;
    if summary.compressed_bytes() != archive.size
        || authenticated_reader.bytes_read != archive.size
        || authenticated_reader.finalize() != archive.sha256
        || verified.len() != expected.len()
        || expected.keys().any(|path| !verified.contains(path))
    {
        return rejected();
    }
    Ok(())
}

struct AuthenticatedArchiveReaderV1<R> {
    inner: R,
    digest: Sha256,
    bytes_read: u64,
}

impl<R> AuthenticatedArchiveReaderV1<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            digest: Sha256::new(),
            bytes_read: 0,
        }
    }

    fn finalize(&self) -> [u8; 32] {
        self.digest.clone().finalize().into()
    }
}

impl<R: Read> Read for AuthenticatedArchiveReaderV1<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let count = self.inner.read(buffer)?;
        self.bytes_read = self
            .bytes_read
            .checked_add(count as u64)
            .ok_or_else(invalid_archive_member)?;
        self.digest.update(&buffer[..count]);
        Ok(count)
    }
}

fn hash_archive_member(reader: &mut dyn Read) -> io::Result<([u8; 32], u64)> {
    let mut digest = Sha256::new();
    let mut size = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        size = size
            .checked_add(count as u64)
            .ok_or_else(invalid_archive_member)?;
        digest.update(&buffer[..count]);
    }
    Ok((digest.finalize().into(), size))
}

fn invalid_archive_member() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "archive member was rejected")
}

fn validate_manifest_shape(
    manifest: &[TairaAuthorityArtifactManifestEntryV1],
    artifact_count: usize,
    archive_mode: bool,
) -> Result<(), TairaAuthorityErrorV1> {
    let expected_len = MANIFEST_NAMES_V1.len() + usize::from(archive_mode);
    if manifest.len() != expected_len || artifact_count != expected_len {
        return rejected();
    }
    for (index, (entry, expected_name)) in manifest.iter().zip(MANIFEST_NAMES_V1).enumerate() {
        if usize::from(entry.ordinal) != index
            || entry.name != expected_name
            || entry.size == 0
            || entry.sha256 == [0; 32]
        {
            return rejected();
        }
    }
    if archive_mode {
        let entry = manifest.last().ok_or(TairaAuthorityErrorV1::Rejected)?;
        if usize::from(entry.ordinal) != MANIFEST_NAMES_V1.len()
            || entry.name != "subject/release-archive"
            || entry.size == 0
            || entry.sha256 == [0; 32]
        {
            return rejected();
        }
    }
    Ok(())
}

fn validate_evidence_rows(
    subject: &Map,
    manifest: &[TairaAuthorityArtifactManifestEntryV1],
) -> Result<Vec<(String, [u8; 32], u64)>, TairaAuthorityErrorV1> {
    let rows = subject
        .get("native_release_evidence")
        .and_then(Value::as_array)
        .filter(|rows| rows.len() == EVIDENCE_ROWS_V1.len())
        .ok_or(TairaAuthorityErrorV1::Rejected)?;
    let mut validated = Vec::with_capacity(rows.len());
    for (row, (expected_name, expected_path)) in rows.iter().zip(EVIDENCE_ROWS_V1) {
        let row = exact_object(row, &["name", "path", "sha256", "size"])?;
        let digest = required_digest(row, "sha256")?;
        let size = required_u64(row, "size")?;
        if required_str(row, "name")? != expected_name
            || required_str(row, "path")? != expected_path
        {
            return rejected();
        }
        let descriptor_name = format!("evidence/{expected_path}");
        let descriptor = manifest
            .iter()
            .find(|entry| entry.name == descriptor_name)
            .ok_or(TairaAuthorityErrorV1::Rejected)?;
        if descriptor.sha256 != digest || descriptor.size != size {
            return rejected();
        }
        validated.push((expected_name.to_owned(), digest, size));
    }
    Ok(validated)
}

fn evidence_digest(
    evidence: &[(String, [u8; 32], u64)],
    name: &str,
) -> Result<[u8; 32], TairaAuthorityErrorV1> {
    evidence
        .iter()
        .find_map(|(candidate, digest, _)| (candidate == name).then_some(*digest))
        .ok_or(TairaAuthorityErrorV1::Rejected)
}

fn validate_exact12_matrix(bytes: &[u8]) -> Result<Exact12SummaryV1, TairaAuthorityErrorV1> {
    if bytes.is_empty() || !bytes.ends_with(b"\n") || bytes.contains(&b'\r') || bytes.contains(&0) {
        return rejected();
    }
    let text = std::str::from_utf8(bytes).map_err(|_| TairaAuthorityErrorV1::Rejected)?;
    let mut versions = Vec::new();
    let mut registries = Vec::new();
    let mut protocols = Vec::new();
    let mut envelopes = Vec::new();
    let mut retired = Vec::new();
    for line in text[..text.len() - 1].split('\n') {
        if line.starts_with('#') {
            continue;
        }
        if line.is_empty() {
            return rejected();
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        match fields.first().copied() {
            Some("matrix-version") if fields.len() == 2 => versions.push(fields),
            Some("registry-sha256") if fields.len() == 2 => registries.push(fields),
            Some("protocol") if fields.len() == 5 => protocols.push(fields),
            Some("typed-envelope") if fields.len() == 6 => envelopes.push(fields),
            Some("retired") if fields.len() == 2 => retired.push(fields),
            _ => return rejected(),
        }
    }
    if versions != [vec!["matrix-version", "1"]]
        || registries != [vec!["registry-sha256", REGISTRY_SHA256_V1]]
        || protocols.len() != PROTOCOL_COUNT_V1
        || envelopes.len() != PROTOCOL_COUNT_V1
        || retired.len() != RETIRED_LABELS_V1.len()
    {
        return rejected();
    }
    for (index, ((label, variant), row)) in PROTOCOLS_V1.iter().zip(&protocols).enumerate() {
        if row.as_slice() != ["protocol", &index.to_string(), label, variant, variant] {
            return rejected();
        }
    }
    let mut registry = Sha256::new();
    for (label, _) in PROTOCOLS_V1 {
        registry.update(label.as_bytes());
        registry.update(b"\n");
    }
    if hex::encode(registry.finalize()) != REGISTRY_SHA256_V1 {
        return rejected();
    }
    let mut statements = BTreeSet::new();
    let mut typed_envelopes = BTreeSet::new();
    for (index, ((label, variant), row)) in PROTOCOLS_V1.iter().zip(&envelopes).enumerate() {
        if row[0] != "typed-envelope"
            || row[1] != *label
            || row[2] != *variant
            || row[3] != *variant
            || digest_from_str(row[4]).is_err()
            || digest_from_str(row[5]).is_err()
            || !statements.insert(row[4])
            || !typed_envelopes.insert(row[5])
            || index >= PROTOCOL_COUNT_V1
        {
            return rejected();
        }
    }
    for (row, expected) in retired.iter().zip(RETIRED_LABELS_V1) {
        if row.as_slice() != ["retired", expected] {
            return rejected();
        }
    }
    let labels = PROTOCOLS_V1
        .iter()
        .map(|(label, _)| (*label).to_owned())
        .collect::<Vec<_>>();
    if labels
        .iter()
        .any(|label| RETIRED_LABELS_V1.contains(&label.as_str()))
    {
        return rejected();
    }
    Ok(Exact12SummaryV1 {
        labels,
        retired_labels: RETIRED_LABELS_V1
            .iter()
            .map(|label| (*label).to_owned())
            .collect(),
    })
}

fn validate_exact12_subject(
    value: &Value,
    summary: &Exact12SummaryV1,
) -> Result<(), TairaAuthorityErrorV1> {
    let object = exact_object(
        value,
        &[
            "protocol_count",
            "protocol_labels",
            "registry_sha256",
            "retired_labels",
            "stage_count",
            "typed_envelope_count",
        ],
    )?;
    if required_u64(object, "protocol_count")? != PROTOCOL_COUNT_V1 as u64
        || required_u64(object, "stage_count")? != STAGE_COUNT_V1 as u64
        || required_u64(object, "typed_envelope_count")? != PROTOCOL_COUNT_V1 as u64
        || required_str(object, "registry_sha256")? != REGISTRY_SHA256_V1
        || string_array(object, "protocol_labels")? != summary.labels
        || string_array(object, "retired_labels")? != summary.retired_labels
    {
        return rejected();
    }
    Ok(())
}

fn validate_expectations(
    expectations: &PrivacyReleaseExpectationsV1,
) -> Result<(), TairaAuthorityErrorV1> {
    if expectations.schema_version != 1
        || usize::from(expectations.stage_count) != STAGE_COUNT_V1
        || expectations.stages.len() != STAGE_COUNT_V1
    {
        return rejected();
    }
    let mut proof_count = 0_usize;
    let mut proof_bytes = 0_u64;
    let mut descriptors = Vec::with_capacity(PROTOCOL_COUNT_V1);
    for (index, stage) in expectations.stages.iter().enumerate() {
        let protocol_index = index / CASE_COUNT_V1;
        let case_index = index % CASE_COUNT_V1;
        let expected_protocol = PrivacyProtocolIdV1::ALL[protocol_index];
        let expected_case = PrivacyReleaseCaseKindV1::ALL[case_index];
        validate_stage_evidence(
            &stage.evidence,
            index,
            expected_protocol,
            expected_case,
            &mut proof_count,
            &mut proof_bytes,
        )?;
        if stage.max_elapsed_millis == 0
            || stage.max_peak_rss_bytes == 0
            || stage.max_address_space_bytes == 0
        {
            return rejected();
        }
        if case_index == 0 {
            descriptors.push(stage.evidence.protocol_descriptor.clone());
        } else if descriptors[protocol_index] != stage.evidence.protocol_descriptor {
            return rejected();
        }
    }
    if proof_count == 0
        || proof_count > MAX_TOTAL_PROOF_ARTIFACTS_V1
        || proof_bytes == 0
        || proof_bytes > MAX_TOTAL_PROOF_BYTES_V1
        || descriptors.iter().collect::<BTreeSet<_>>().len() != PROTOCOL_COUNT_V1
    {
        return rejected();
    }
    Ok(())
}

fn validate_stage_evidence(
    evidence: &PrivacyReleaseStageEvidenceV1,
    index: usize,
    expected_protocol: PrivacyProtocolIdV1,
    expected_case: PrivacyReleaseCaseKindV1,
    proof_count: &mut usize,
    proof_bytes: &mut u64,
) -> Result<(), TairaAuthorityErrorV1> {
    if evidence.schema_version != 1
        || usize::from(evidence.stage_ordinal) != index
        || evidence.protocol_id != expected_protocol
        || evidence.case_kind != expected_case
        || evidence.protocol_descriptor.is_empty()
        || evidence.protocol_descriptor.len() > 16 * 1024
        || evidence.public_statement_sha256 == [0; 32]
        || evidence.proof_artifacts.is_empty()
    {
        return rejected();
    }
    let expected_failure = match expected_case {
        PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd
        | PrivacyReleaseCaseKindV1::MaximumShapeResource => {
            PrivacyReleaseFailureClassV1::NotApplicable
        }
        PrivacyReleaseCaseKindV1::PublicStatementBindingMutation => {
            PrivacyReleaseFailureClassV1::PublicStatementBindingRejected
        }
        PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation => {
            PrivacyReleaseFailureClassV1::CanonicalWireCorruptionAndTruncationRejected
        }
    };
    if evidence.failure_class != expected_failure
        || evidence.resources.primary_units == 0
        || evidence.resources.primary_ceiling == 0
        || evidence.resources.primary_units > evidence.resources.primary_ceiling
        || evidence.resources.secondary_units > evidence.resources.secondary_ceiling
        || evidence.resources.relation_depth > evidence.resources.relation_depth_ceiling
    {
        return rejected();
    }
    for (ordinal, proof) in evidence.proof_artifacts.iter().enumerate() {
        let length = u64::try_from(proof.canonical_proof_bytes.len())
            .map_err(|_| TairaAuthorityErrorV1::Rejected)?;
        if usize::from(proof.artifact_ordinal) != ordinal
            || length == 0
            || length > MAX_PROOF_ARTIFACT_BYTES_V1
            || proof.proof_bytes_ceiling < length
            || proof.proof_bytes_ceiling > MAX_PROOF_ARTIFACT_BYTES_V1
            || sha256(&proof.canonical_proof_bytes) != proof.proof_sha256
        {
            return rejected();
        }
        *proof_count = proof_count
            .checked_add(1)
            .ok_or(TairaAuthorityErrorV1::Rejected)?;
        *proof_bytes = proof_bytes
            .checked_add(length)
            .ok_or(TairaAuthorityErrorV1::Rejected)?;
    }
    Ok(())
}

fn validate_measured_stages(
    stages: &PrivacyReleaseStageArtifactsV1,
    expectations: &PrivacyReleaseExpectationsV1,
) -> Result<(), TairaAuthorityErrorV1> {
    if stages.schema_version != 1
        || usize::from(stages.stage_count) != STAGE_COUNT_V1
        || stages.stages.len() != STAGE_COUNT_V1
    {
        return rejected();
    }
    for (actual, expected) in stages.stages.iter().zip(&expectations.stages) {
        if actual.evidence != expected.evidence
            || actual.elapsed_millis == 0
            || actual.elapsed_millis > expected.max_elapsed_millis
            || actual.peak_rss_bytes == 0
            || actual.peak_rss_bytes > expected.max_peak_rss_bytes
            || actual.peak_address_space_bytes == 0
            || actual.peak_address_space_bytes > expected.max_address_space_bytes
        {
            return rejected();
        }
    }
    Ok(())
}

fn validate_x509_resource(
    certificate: &PrivacyReleaseZkX509ResourceCertificateV1,
    evidence: &[(String, [u8; 32], u64)],
) -> Result<(), TairaAuthorityErrorV1> {
    if certificate.schema_version != 1
        || certificate.protocol_id != PrivacyProtocolIdV1::IrohaZkX509StarkP256V0
        || certificate.compiled_profile_digest == [0; 32]
        || certificate.certificate_sha256 == [0; 32]
        || certificate.expectations_norito_sha256
            != evidence_digest(evidence, "expectations_norito")?
        || certificate.expectations_json_sha256 != evidence_digest(evidence, "expectations_json")?
        || certificate.kat_proof_bytes == 0
        || certificate.kat_proof_sha256 == [0; 32]
        || certificate.environment.operating_system != "linux"
        || certificate.environment.endianness != "little"
        || !matches!(
            certificate.environment.architecture.as_str(),
            "x86_64" | "aarch64"
        )
        || certificate.environment.kernel_minimum_major == 0
        || certificate.environment.rustc_release.is_empty()
        || certificate.environment.rustc_host.is_empty()
        || certificate.environment.rustc_commit_hash.is_empty()
        || certificate.environment.rustc_commit_date.is_empty()
        || certificate.environment.instance_type.is_empty()
        || certificate.environment.cpu_model.is_empty()
        || certificate.environment.logical_cpu_count == 0
        || certificate.environment.online_cpu_count == 0
        || certificate.environment.affinity_cpu_count == 0
        || certificate.environment.online_cpu_count > certificate.environment.logical_cpu_count
        || certificate.environment.affinity_cpu_count > certificate.environment.online_cpu_count
    {
        return rejected();
    }
    let limits = certificate.process_limits;
    if limits.elapsed_ceiling_millis == 0
        || limits.peak_rss_ceiling_bytes == 0
        || limits.address_space_ceiling_bytes == 0
        || limits.main_thread_stack_bytes == 0
        || limits.rayon_worker_stack_bytes == 0
        || limits.watchdog_thread_stack_bytes == 0
        || limits.rayon_worker_count == 0
        || limits.max_stage_tasks == 0
        || limits.max_stage_open_files == 0
        || limits.landlock_abi_minimum == 0
        || limits.minimum_effective_memory_bytes == 0
        || !limits.cgroup_v2
        || !limits.cpu_quota_unlimited
        || !limits.landlock_restrict_self
        || !limits.anchored_openat2
        || !limits.memfd_exec
        || !limits.memfd_seal_exec
        || !limits.static_elf_only
        || !limits.seccomp_tsync
    {
        return rejected();
    }
    validate_x509_observation(
        certificate.positive,
        PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd,
        limits,
    )?;
    validate_x509_observation(
        certificate.maximum,
        PrivacyReleaseCaseKindV1::MaximumShapeResource,
        limits,
    )
}

fn validate_x509_observation(
    observation: PrivacyReleaseZkX509ResourceObservationV1,
    expected_case: PrivacyReleaseCaseKindV1,
    limits: PrivacyReleaseZkX509ResourceProcessLimitsV1,
) -> Result<(), TairaAuthorityErrorV1> {
    if observation.case_kind != expected_case
        || observation.elapsed_millis == 0
        || observation.elapsed_millis > limits.elapsed_ceiling_millis
        || observation.peak_rss_bytes == 0
        || observation.peak_rss_bytes > limits.peak_rss_ceiling_bytes
        || observation.peak_address_space_bytes == 0
        || observation.peak_address_space_bytes > limits.address_space_ceiling_bytes
        || observation.primary_units == 0
        || observation.primary_ceiling == 0
        || observation.primary_units > observation.primary_ceiling
        || observation.secondary_units > observation.secondary_ceiling
        || observation.relation_depth > observation.relation_depth_ceiling
    {
        return rejected();
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_command_manifest(
    command: &PrivacyReleaseCommandManifestV1,
    source_sha256: [u8; 32],
    exact12_sha256: [u8; 32],
    cargo_sha256: [u8; 32],
    runner_sha256: [u8; 32],
    validator_sha256: [u8; 32],
    evidence: &[(String, [u8; 32], u64)],
) -> Result<(), TairaAuthorityErrorV1> {
    if command.schema_version != 1
        || command.build_profile != "release"
        || command.source_sha256 != source_sha256
        || command.exact12_matrix_sha256 != exact12_sha256
        || command.cargo_lock_sha256 != cargo_sha256
        || command.runner_binary_sha256 != runner_sha256
        || command.validator_binary_sha256 != validator_sha256
        || command.expectations_norito_sha256 != evidence_digest(evidence, "expectations_norito")?
        || command.expectations_json_sha256 != evidence_digest(evidence, "expectations_json")?
        || command.x509_resource_norito_sha256 != evidence_digest(evidence, "x509_resource_norito")?
        || command.x509_resource_json_sha256 != evidence_digest(evidence, "x509_resource_json")?
        || command.command_arguments.is_empty()
        || command.stage_command_template.is_empty()
        || command
            .command_arguments
            .iter()
            .chain(&command.stage_command_template)
            .any(|argument| {
                argument.is_empty() || argument.contains('\0') || argument.contains('\n')
            })
    {
        return rejected();
    }
    let isolation = &command.isolation_policy;
    if isolation.stage_rayon_threads == 0
        || isolation.main_thread_stack_bytes == 0
        || isolation.rayon_worker_stack_bytes == 0
        || isolation.watchdog_thread_stack_bytes == 0
        || isolation.max_stage_tasks == 0
        || isolation.max_stage_open_files == 0
        || isolation.max_stage_result_file_bytes == 0
        || isolation.max_stage_diagnostic_bytes == 0
        || isolation.landlock_abi_minimum == 0
        || !isolation.static_elf_only
        || !isolation.anonymous_sealed_runner
        || !isolation.anonymous_result_descriptor_only
        || !isolation.exact_environment_only
        || !isolation.seccomp_tsync
    {
        return rejected();
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_receipt(
    receipt: &PrivacyReleaseReceiptV1,
    source_sha256: [u8; 32],
    exact12_sha256: [u8; 32],
    cargo_sha256: [u8; 32],
    runner_sha256: [u8; 32],
    validator_sha256: [u8; 32],
    evidence: &[(String, [u8; 32], u64)],
) -> Result<(), TairaAuthorityErrorV1> {
    let pair = |norito_name: &str,
                json_name: &str|
     -> Result<PrivacyReleaseArtifactPairDigestV1, TairaAuthorityErrorV1> {
        Ok(PrivacyReleaseArtifactPairDigestV1 {
            norito_sha256: evidence_digest(evidence, norito_name)?,
            json_sha256: evidence_digest(evidence, json_name)?,
        })
    };
    if receipt.schema_version != 1
        || receipt.build_profile != "release"
        || receipt.source_sha256 != source_sha256
        || receipt.exact12_matrix_sha256 != exact12_sha256
        || receipt.cargo_lock_sha256 != cargo_sha256
        || receipt.runner_binary_sha256 != runner_sha256
        || receipt.validator_binary_sha256 != validator_sha256
        || receipt.expectations != pair("expectations_norito", "expectations_json")?
        || receipt.x509_resource != pair("x509_resource_norito", "x509_resource_json")?
        || receipt.command_manifest != pair("command_manifest_norito", "command_manifest_json")?
        || receipt.stage_artifacts != pair("stage_artifacts_norito", "stage_artifacts_json")?
        || usize::from(receipt.fixed_stage_count) != STAGE_COUNT_V1
        || !receipt.all_native_stages_passed
        || receipt.contains_witnesses
        || !receipt.contains_canonical_proof_artifacts
        || !receipt.isolation_policy_enforced
    {
        return rejected();
    }
    Ok(())
}

fn validate_release_subject(
    value: &Value,
    manifest: &[TairaAuthorityArtifactManifestEntryV1],
    source_sha256: [u8; 32],
) -> Result<(), TairaAuthorityErrorV1> {
    let object = value.as_object().ok_or(TairaAuthorityErrorV1::Rejected)?;
    match object.get("kind").and_then(Value::as_str) {
        Some("taira-rollout-tar-gzip-v1") => {
            let object = exact_object(value, &["kind", "name", "sha256", "size"])?;
            let name = required_str(object, "name")?;
            let digest = required_digest(object, "sha256")?;
            let size = required_u64(object, "size")?;
            let archive = manifest
                .last()
                .filter(|entry| entry.name == "subject/release-archive")
                .ok_or(TairaAuthorityErrorV1::Rejected)?;
            if !valid_archive_name(name) || archive.sha256 != digest || archive.size != size {
                return rejected();
            }
        }
        Some("taira-validator-oci-image-v1") => {
            if manifest.len() != MANIFEST_NAMES_V1.len() {
                return rejected();
            }
            let object = exact_object(
                value,
                &["image_id", "kind", "manifest_digest", "name", "tags"],
            )?;
            if required_str(object, "name")? != "taira-validator"
                || !valid_prefixed_digest(required_str(object, "image_id")?)
                || !valid_prefixed_digest(required_str(object, "manifest_digest")?)
            {
                return rejected();
            }
            let tags = string_array(object, "tags")?;
            validate_image_tags(&tags, source_sha256)?;
        }
        _ => return rejected(),
    }
    Ok(())
}

fn validate_image_tags(
    tags: &[String],
    source_sha256: [u8; 32],
) -> Result<(), TairaAuthorityErrorV1> {
    if tags.is_empty()
        || tags.windows(2).any(|pair| pair[0] >= pair[1])
        || tags.iter().any(|tag| !valid_image_tag(tag))
    {
        return rejected();
    }
    let source = hex::encode(source_sha256);
    let prefixes = [
        format!("hyperledger/iroha:taira-source-{source}"),
        format!("docker.soramitsu.co.jp/iroha3/iroha:taira-source-{source}"),
    ];
    let mut immutable = BTreeSet::new();
    for prefix in &prefixes {
        let matching = tags
            .iter()
            .filter(|tag| tag.as_str() == prefix || tag.starts_with(&format!("{prefix}-")))
            .collect::<Vec<_>>();
        if matching.len() != 1 {
            return rejected();
        }
        immutable.insert(matching[0].as_str());
    }
    let extras = tags
        .iter()
        .map(String::as_str)
        .filter(|tag| !immutable.contains(tag))
        .collect::<BTreeSet<_>>();
    let latest = BTreeSet::from([
        "docker.soramitsu.co.jp/iroha3/iroha:taira-latest",
        "hyperledger/iroha:taira-latest",
    ]);
    if !extras.is_empty() && extras != latest {
        return rejected();
    }
    Ok(())
}

fn valid_archive_name(name: &str) -> bool {
    name.ends_with(".tar.gz")
        && name.len() > ".tar.gz".len()
        && name.len() <= 255
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn valid_prefixed_digest(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|digest| digest_from_str(digest).is_ok())
}

fn valid_image_tag(value: &str) -> bool {
    let Some((repository, tag)) = value.split_once(':') else {
        return false;
    };
    !repository.is_empty()
        && repository.len() <= 191
        && !tag.is_empty()
        && tag.len() <= 128
        && !repository.contains(':')
        && repository.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || (index > 0 && matches!(byte, b'.' | b'/' | b'_' | b'-'))
        })
        && tag.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || (index > 0 && matches!(byte, b'.' | b'_' | b'-'))
        })
}

fn decode_pair<T>(
    manifest: &[TairaAuthorityArtifactManifestEntryV1],
    artifacts: &mut [File],
    norito_name: &str,
    json_name: &str,
    max_norito_bytes: u64,
    max_json_bytes: u64,
) -> Result<T, TairaAuthorityErrorV1>
where
    T: PartialEq
        + norito::NoritoSerialize
        + norito::json::JsonSerialize
        + norito::json::JsonDeserialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    let norito_bytes = read_named_artifact(manifest, artifacts, norito_name, max_norito_bytes)?;
    let authoritative: T = decode_canonical_norito(&norito_bytes)?;
    let json_bytes = read_named_artifact(manifest, artifacts, json_name, max_json_bytes)?;
    let projection: T = decode_canonical_json(&json_bytes)?;
    if projection != authoritative {
        return rejected();
    }
    Ok(authoritative)
}

fn decode_canonical_norito<T>(bytes: &[u8]) -> Result<T, TairaAuthorityErrorV1>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    let value: T =
        norito::decode_canonical_with_limits(bytes, norito::canonical_decode_limits(bytes.len()))
            .map_err(|_| TairaAuthorityErrorV1::Rejected)?;
    let canonical =
        norito::encode_canonical(&value).map_err(|_| TairaAuthorityErrorV1::Rejected)?;
    if canonical != bytes {
        return rejected();
    }
    Ok(value)
}

fn decode_canonical_json<T>(bytes: &[u8]) -> Result<T, TairaAuthorityErrorV1>
where
    T: PartialEq + norito::json::JsonSerialize + norito::json::JsonDeserialize,
{
    let text = std::str::from_utf8(bytes).map_err(|_| TairaAuthorityErrorV1::Rejected)?;
    let value: T = norito::json::from_str(text).map_err(|_| TairaAuthorityErrorV1::Rejected)?;
    let mut canonical =
        norito::json::to_json_pretty(&value).map_err(|_| TairaAuthorityErrorV1::Rejected)?;
    canonical.push('\n');
    if canonical.as_bytes() != bytes {
        return rejected();
    }
    Ok(value)
}

fn read_named_artifact(
    manifest: &[TairaAuthorityArtifactManifestEntryV1],
    artifacts: &mut [File],
    name: &str,
    maximum: u64,
) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
    let index = manifest
        .iter()
        .position(|entry| entry.name == name)
        .ok_or(TairaAuthorityErrorV1::Rejected)?;
    let expected = &manifest[index];
    if expected.size == 0 || expected.size > maximum {
        return rejected();
    }
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

fn required_u64(object: &Map, field: &str) -> Result<u64, TairaAuthorityErrorV1> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .filter(|value| *value > 0)
        .ok_or(TairaAuthorityErrorV1::Rejected)
}

fn required_commit<'a>(object: &'a Map, field: &str) -> Result<&'a str, TairaAuthorityErrorV1> {
    required_str(object, field).and_then(|value| {
        (value.len() == 40 && value.bytes().all(is_lower_hex))
            .then_some(value)
            .ok_or(TairaAuthorityErrorV1::Rejected)
    })
}

fn required_digest(object: &Map, field: &str) -> Result<[u8; 32], TairaAuthorityErrorV1> {
    digest_from_str(required_str(object, field)?)
}

fn digest_from_ascii(value: &[u8]) -> Result<[u8; 32], TairaAuthorityErrorV1> {
    let value = std::str::from_utf8(value).map_err(|_| TairaAuthorityErrorV1::Rejected)?;
    digest_from_str(value)
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

fn string_array(object: &Map, field: &str) -> Result<Vec<String>, TairaAuthorityErrorV1> {
    object
        .get(field)
        .and_then(Value::as_array)
        .ok_or(TairaAuthorityErrorV1::Rejected)?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or(TairaAuthorityErrorV1::Rejected)
        })
        .collect()
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn rejected<T>() -> Result<T, TairaAuthorityErrorV1> {
    Err(TairaAuthorityErrorV1::Rejected)
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
pub(super) mod tests {
    use super::*;
    use std::{fs, os::unix::fs::PermissionsExt as _, path::PathBuf};

    pub(crate) struct AuthorityServiceFixtureV1 {
        pub(crate) _directory: tempfile::TempDir,
        pub(crate) subject: Value,
        pub(crate) manifest: Value,
        pub(crate) paths: Vec<PathBuf>,
    }

    struct Fixture {
        _directory: tempfile::TempDir,
        subject: Value,
        manifest: Vec<TairaAuthorityArtifactManifestEntryV1>,
        artifacts: Vec<File>,
        paths: Vec<PathBuf>,
    }

    impl Fixture {
        fn validate(&mut self) -> Result<Value, TairaAuthorityErrorV1> {
            validate_native_evidence_v1(&self.subject, &self.manifest, &mut self.artifacts)
        }

        fn refresh(&mut self, manifest_name: &str) {
            let index = self
                .manifest
                .iter()
                .position(|entry| entry.name == manifest_name)
                .expect("fixture manifest entry");
            let bytes = fs::read(&self.paths[index]).expect("read mutated fixture artifact");
            self.manifest[index].size = bytes.len() as u64;
            self.manifest[index].sha256 = sha256(&bytes);
            if let Some(path) = manifest_name.strip_prefix("evidence/") {
                let rows = self
                    .subject
                    .as_object_mut()
                    .and_then(|subject| subject.get_mut("native_release_evidence"))
                    .and_then(Value::as_array_mut)
                    .expect("fixture evidence rows");
                let row = rows
                    .iter_mut()
                    .find(|row| row.get("path").and_then(Value::as_str) == Some(path))
                    .and_then(Value::as_object_mut)
                    .expect("fixture evidence row");
                row.insert("size".into(), Value::from(bytes.len() as u64));
                row.insert("sha256".into(), Value::from(hex::encode(sha256(&bytes))));
            }
        }

        fn write_and_refresh(&mut self, manifest_name: &str, bytes: &[u8]) {
            let index = self
                .manifest
                .iter()
                .position(|entry| entry.name == manifest_name)
                .expect("fixture manifest entry");
            fs::write(&self.paths[index], bytes).expect("write mutated fixture artifact");
            self.refresh(manifest_name);
        }

        fn use_archive_subject(&mut self) {
            let prefix = "taira-test";
            let payloads = MANIFEST_NAMES_V1
                .iter()
                .enumerate()
                .map(|(index, name)| {
                    (
                        format!("{prefix}/{}", name.strip_prefix("evidence/").unwrap()),
                        fs::read(&self.paths[index]).expect("read archive fixture member"),
                    )
                })
                .collect::<Vec<_>>();
            let files = payloads
                .iter()
                .map(|(path, payload)| {
                    sorafs_car::bundle_archive::BundleArchiveFile::new(path, 0o644, payload)
                })
                .collect::<Vec<_>>();
            let bytes = sorafs_car::bundle_archive::write_gzip_ustar(Vec::new(), &files)
                .expect("encode canonical fixture archive");
            self.install_archive("taira-test.tar.gz", &bytes);
        }

        fn install_archive(&mut self, name: &str, bytes: &[u8]) {
            let path = self._directory.path().join("taira-test.tar.gz");
            fs::write(&path, bytes).expect("write archive fixture");
            if let Some(index) = self
                .manifest
                .iter()
                .position(|entry| entry.name == "subject/release-archive")
            {
                self.manifest[index].size = bytes.len() as u64;
                self.manifest[index].sha256 = sha256(bytes);
            } else {
                self.manifest.push(TairaAuthorityArtifactManifestEntryV1 {
                    ordinal: u16::try_from(self.manifest.len()).expect("manifest ordinal"),
                    name: "subject/release-archive".to_owned(),
                    size: bytes.len() as u64,
                    sha256: sha256(bytes),
                });
                self.artifacts
                    .push(File::open(&path).expect("open archive fixture"));
                self.paths.push(path);
            }
            let release = object([
                ("kind", Value::from("taira-rollout-tar-gzip-v1")),
                ("name", Value::from(name)),
                ("sha256", Value::from(hex::encode(sha256(bytes)))),
                ("size", Value::from(bytes.len() as u64)),
            ]);
            self.subject
                .as_object_mut()
                .expect("fixture subject")
                .insert("subject".into(), release);
        }
    }

    #[test]
    fn accepts_exact_image_and_canonical_archive_subjects() {
        let mut image = fixture();
        assert_eq!(image.validate().unwrap(), Value::Object(Map::new()));

        let mut archive = fixture();
        archive.use_archive_subject();
        assert_eq!(archive.validate().unwrap(), Value::Object(Map::new()));
    }

    #[test]
    fn archive_rejects_every_mutated_or_omitted_evidence_member() {
        for index in 0..MANIFEST_NAMES_V1.len() {
            let mut mutated = fixture();
            let mut members = archive_members(&mutated);
            members[index].1.push(0xa5);
            let bytes = canonical_archive(&members);
            mutated.install_archive("taira-test.tar.gz", &bytes);
            assert_eq!(
                mutated.validate(),
                Err(TairaAuthorityErrorV1::Rejected),
                "mutated archive evidence member {index}"
            );

            let mut omitted = fixture();
            let mut members = archive_members(&omitted);
            members.remove(index);
            let bytes = canonical_archive(&members);
            omitted.install_archive("taira-test.tar.gz", &bytes);
            assert_eq!(
                omitted.validate(),
                Err(TairaAuthorityErrorV1::Rejected),
                "omitted archive evidence member {index}"
            );
        }
    }

    #[test]
    fn archive_rejects_outside_prefix_and_descriptor_drift() {
        let mut outside = fixture();
        let mut members = archive_members(&outside);
        members.push(("aaa/outside-prefix".to_owned(), b"outside".to_vec()));
        let bytes = canonical_archive(&members);
        outside.install_archive("taira-test.tar.gz", &bytes);
        assert_eq!(outside.validate(), Err(TairaAuthorityErrorV1::Rejected));

        let mut descriptor = fixture();
        descriptor.use_archive_subject();
        let archive_index = descriptor
            .manifest
            .iter()
            .position(|entry| entry.name == "subject/release-archive")
            .unwrap();
        descriptor.manifest[archive_index].sha256[0] ^= 0x80;
        assert_eq!(descriptor.validate(), Err(TairaAuthorityErrorV1::Rejected));

        let mut bytes = fixture();
        bytes.use_archive_subject();
        let archive_index = bytes
            .manifest
            .iter()
            .position(|entry| entry.name == "subject/release-archive")
            .unwrap();
        fs::write(&bytes.paths[archive_index], b"post-manifest mutation")
            .expect("mutate archive descriptor");
        assert_eq!(bytes.validate(), Err(TairaAuthorityErrorV1::Rejected));
    }

    #[test]
    fn archive_rejects_nonregular_duplicate_reordered_and_oversized_entries() {
        let source = fixture();
        let mut normal = archive_members(&source)
            .into_iter()
            .map(|(path, payload)| RawArchiveEntry {
                path,
                kind: b'0',
                payload,
                declared_size: None,
            })
            .collect::<Vec<_>>();
        normal.sort_by(|left, right| left.path.cmp(&right.path));

        // Prove the hostile-archive encoder itself produces a reader-accepted
        // canonical stream before individual policy mutations are applied.
        let mut control = fixture();
        control.install_archive("taira-test.tar.gz", &stored_gzip(&raw_ustar(&normal)));
        assert_eq!(control.validate().unwrap(), Value::Object(Map::new()));

        let mut expected_directory = normal.clone();
        expected_directory[0].kind = b'5';
        expected_directory[0].payload.clear();
        expected_directory[0].declared_size = Some(0);
        assert_raw_archive_rejected(&expected_directory);

        let mut nonregular = normal.clone();
        nonregular.push(RawArchiveEntry {
            path: "taira-test/zz-link".to_owned(),
            kind: b'2',
            payload: Vec::new(),
            declared_size: Some(0),
        });
        nonregular.sort_by(|left, right| left.path.cmp(&right.path));
        assert_raw_archive_rejected(&nonregular);

        let mut duplicate = normal.clone();
        let repeated = duplicate[0].clone();
        duplicate.insert(1, repeated);
        assert_raw_archive_rejected(&duplicate);

        let mut reordered = normal.clone();
        reordered.swap(0, 1);
        assert_raw_archive_rejected(&reordered);

        let mut oversized = normal;
        oversized.push(RawArchiveEntry {
            path: "taira-test/zz-oversized".to_owned(),
            kind: b'0',
            payload: Vec::new(),
            declared_size: Some(BUNDLE_ARCHIVE_PROTOCOL_MAX_FILE_BYTES + 1),
        });
        oversized.sort_by(|left, right| left.path.cmp(&right.path));
        assert_raw_archive_rejected(&oversized);
    }

    #[test]
    fn rejects_each_top_level_and_evidence_or_manifest_row_mutation() {
        let mut fixture = fixture();
        for field in TOP_LEVEL_FIELDS_V1 {
            let mut subject = fixture.subject.clone();
            subject
                .as_object_mut()
                .expect("subject object")
                .remove(field);
            assert_eq!(
                validate_native_evidence_v1(&subject, &fixture.manifest, &mut fixture.artifacts),
                Err(TairaAuthorityErrorV1::Rejected),
                "removed top-level field {field}"
            );
        }

        for index in 0..EVIDENCE_ROWS_V1.len() {
            let mut subject = fixture.subject.clone();
            let row = subject
                .as_object_mut()
                .and_then(|subject| subject.get_mut("native_release_evidence"))
                .and_then(Value::as_array_mut)
                .and_then(|rows| rows.get_mut(index))
                .and_then(Value::as_object_mut)
                .expect("evidence row");
            row.insert("name".into(), Value::from(format!("mutated-{index}")));
            assert_eq!(
                validate_native_evidence_v1(&subject, &fixture.manifest, &mut fixture.artifacts),
                Err(TairaAuthorityErrorV1::Rejected),
                "mutated evidence row {index}"
            );
        }

        for index in 0..MANIFEST_NAMES_V1.len() {
            let mut manifest = fixture.manifest.clone();
            manifest[index].name.push_str("-mutated");
            assert_eq!(
                validate_native_evidence_v1(&fixture.subject, &manifest, &mut fixture.artifacts),
                Err(TairaAuthorityErrorV1::Rejected),
                "mutated manifest row {index}"
            );
        }
    }

    #[test]
    fn rejects_provenance_exact12_pair_and_binary_binding_mutations() {
        let mut provenance = fixture();
        let path = artifact_path(
            &provenance,
            "evidence/provenance/dpn-validator-build.provenance.json",
        );
        let mut value: BuildProvenanceV1 = decode_canonical_json(&fs::read(path).unwrap()).unwrap();
        value.iroha_git_head = "9".repeat(40);
        provenance.write_and_refresh(
            "evidence/provenance/dpn-validator-build.provenance.json",
            &canonical_json(&value),
        );
        assert_eq!(provenance.validate(), Err(TairaAuthorityErrorV1::Rejected));

        let mut matrix = fixture();
        let path = artifact_path(&matrix, "evidence/provenance/privacy-native/exact12-v1.tsv");
        let bytes = fs::read(path).unwrap();
        let mutated = String::from_utf8(bytes)
            .unwrap()
            .replace("matrix-version\t1", "matrix-version\t2");
        matrix.write_and_refresh(
            "evidence/provenance/privacy-native/exact12-v1.tsv",
            mutated.as_bytes(),
        );
        assert_eq!(matrix.validate(), Err(TairaAuthorityErrorV1::Rejected));

        let mut pair = fixture();
        let path = artifact_path(
            &pair,
            "evidence/provenance/privacy-native/command-manifest-v1.json",
        );
        let mut command: PrivacyReleaseCommandManifestV1 =
            decode_canonical_json(&fs::read(path).unwrap()).unwrap();
        command.build_profile = "mutated".to_owned();
        pair.write_and_refresh(
            "evidence/provenance/privacy-native/command-manifest-v1.json",
            &canonical_json(&command),
        );
        assert_eq!(pair.validate(), Err(TairaAuthorityErrorV1::Rejected));

        let mut binary = fixture();
        binary.write_and_refresh(
            "evidence/bin/taira_privacy_release_runner",
            b"substituted runner\n",
        );
        assert_eq!(binary.validate(), Err(TairaAuthorityErrorV1::Rejected));
    }

    #[test]
    fn rejects_image_and_archive_subject_mutations() {
        let mut image = fixture();
        let tags = image
            .subject
            .as_object_mut()
            .and_then(|subject| subject.get_mut("subject"))
            .and_then(Value::as_object_mut)
            .and_then(|subject| subject.get_mut("tags"))
            .and_then(Value::as_array_mut)
            .expect("image tags");
        tags.reverse();
        assert_eq!(image.validate(), Err(TairaAuthorityErrorV1::Rejected));

        let mut archive = fixture();
        archive.use_archive_subject();
        archive
            .subject
            .as_object_mut()
            .and_then(|subject| subject.get_mut("subject"))
            .and_then(Value::as_object_mut)
            .expect("archive subject")
            .insert("name".into(), Value::from("../escape.tar.gz"));
        assert_eq!(archive.validate(), Err(TairaAuthorityErrorV1::Rejected));
    }

    fn fixture() -> Fixture {
        let directory = tempfile::tempdir().expect("fixture directory");
        let source_sha256 = [0x44; 32];
        let cargo = b"fixture Cargo.lock\n".to_vec();
        let runner = b"fixture runner\n".to_vec();
        let validator = b"fixture validator\n".to_vec();
        let matrix = include_bytes!("../../../../../fixtures/privacy/exact12_v1.tsv").to_vec();

        let expectations = expectations_fixture();
        let stages = PrivacyReleaseStageArtifactsV1 {
            schema_version: 1,
            stage_count: STAGE_COUNT_V1 as u16,
            stages: expectations
                .stages
                .iter()
                .map(|expected| PrivacyReleaseMeasuredStageV1 {
                    evidence: expected.evidence.clone(),
                    elapsed_millis: 1,
                    peak_rss_bytes: 1,
                    peak_address_space_bytes: 1,
                })
                .collect(),
        };
        let (expectations_norito, expectations_json) = typed_pair(&expectations);
        let (stages_norito, stages_json) = typed_pair(&stages);
        let x509 = x509_fixture(sha256(&expectations_norito), sha256(&expectations_json));
        let (x509_norito, x509_json) = typed_pair(&x509);
        let provenance = BuildProvenanceV1 {
            dpn_validator_release_commit: "5".repeat(40),
            iroha_git_head: "1".repeat(40),
            iroha_source_attested: true,
            iroha_source_bundle_provenance_sha256: "a".repeat(64),
            iroha_source_tree_sha256: "b".repeat(64),
            iroha_tracked_patch_sha256: "c".repeat(64),
            iroha_worktree_clean: false,
            schema_version: 1,
            validator_lock_sha256: hex::encode(sha256(&cargo)),
            workspace_source_manifest_sha256: hex::encode(source_sha256),
        };
        let provenance_json = canonical_json(&provenance);
        let command = PrivacyReleaseCommandManifestV1 {
            schema_version: 1,
            build_profile: "release".to_owned(),
            source_sha256,
            exact12_matrix_sha256: sha256(&matrix),
            expectations_norito_sha256: sha256(&expectations_norito),
            expectations_json_sha256: sha256(&expectations_json),
            x509_resource_norito_sha256: sha256(&x509_norito),
            x509_resource_json_sha256: sha256(&x509_json),
            cargo_lock_sha256: sha256(&cargo),
            validator_binary_sha256: sha256(&validator),
            runner_binary_sha256: sha256(&runner),
            isolation_policy: isolation_fixture(),
            command_arguments: vec!["verify".to_owned()],
            stage_command_template: vec!["--stage".to_owned()],
        };
        let (command_norito, command_json) = typed_pair(&command);
        let receipt = PrivacyReleaseReceiptV1 {
            schema_version: 1,
            build_profile: "release".to_owned(),
            source_sha256,
            exact12_matrix_sha256: sha256(&matrix),
            expectations: pair_digest(&expectations_norito, &expectations_json),
            x509_resource: pair_digest(&x509_norito, &x509_json),
            cargo_lock_sha256: sha256(&cargo),
            validator_binary_sha256: sha256(&validator),
            runner_binary_sha256: sha256(&runner),
            command_manifest: pair_digest(&command_norito, &command_json),
            stage_artifacts: pair_digest(&stages_norito, &stages_json),
            fixed_stage_count: STAGE_COUNT_V1 as u16,
            all_native_stages_passed: true,
            contains_witnesses: false,
            contains_canonical_proof_artifacts: true,
            isolation_policy_enforced: true,
        };
        let (receipt_norito, receipt_json) = typed_pair(&receipt);
        let source = format!("{}\n", hex::encode(source_sha256)).into_bytes();

        let payloads = [
            cargo,
            provenance_json,
            command_json,
            command_norito,
            matrix,
            expectations_json,
            expectations_norito,
            x509_json,
            x509_norito,
            receipt_json,
            receipt_norito,
            runner,
            stages_json,
            stages_norito,
            validator,
            source,
        ];
        let mut paths = Vec::new();
        let mut manifest = Vec::new();
        let mut artifacts = Vec::new();
        for (ordinal, (name, payload)) in MANIFEST_NAMES_V1.iter().zip(payloads).enumerate() {
            let path = directory.path().join(format!("artifact-{ordinal:02}"));
            fs::write(&path, &payload).expect("write fixture artifact");
            paths.push(path.clone());
            artifacts.push(File::open(&path).expect("open fixture artifact"));
            manifest.push(TairaAuthorityArtifactManifestEntryV1 {
                ordinal: ordinal as u16,
                name: (*name).to_owned(),
                size: payload.len() as u64,
                sha256: sha256(&payload),
            });
        }
        let evidence_rows = EVIDENCE_ROWS_V1
            .iter()
            .map(|(name, path)| {
                let entry = manifest
                    .iter()
                    .find(|entry| entry.name == format!("evidence/{path}"))
                    .expect("evidence descriptor");
                object([
                    ("name", Value::from(*name)),
                    ("path", Value::from(*path)),
                    ("sha256", Value::from(hex::encode(entry.sha256))),
                    ("size", Value::from(entry.size)),
                ])
            })
            .collect();
        let exact12 = object([
            ("protocol_count", Value::from(PROTOCOL_COUNT_V1 as u64)),
            (
                "protocol_labels",
                Value::Array(
                    PROTOCOLS_V1
                        .iter()
                        .map(|(label, _)| Value::from(*label))
                        .collect(),
                ),
            ),
            ("registry_sha256", Value::from(REGISTRY_SHA256_V1)),
            (
                "retired_labels",
                Value::Array(
                    RETIRED_LABELS_V1
                        .iter()
                        .map(|label| Value::from(*label))
                        .collect(),
                ),
            ),
            ("stage_count", Value::from(STAGE_COUNT_V1 as u64)),
            (
                "typed_envelope_count",
                Value::from(PROTOCOL_COUNT_V1 as u64),
            ),
        ]);
        let mut tags = vec![
            format!(
                "hyperledger/iroha:taira-source-{}",
                hex::encode(source_sha256)
            ),
            format!(
                "docker.soramitsu.co.jp/iroha3/iroha:taira-source-{}",
                hex::encode(source_sha256)
            ),
        ];
        tags.sort();
        let release_subject = object([
            (
                "image_id",
                Value::from(format!("sha256:{}", "6".repeat(64))),
            ),
            ("kind", Value::from("taira-validator-oci-image-v1")),
            (
                "manifest_digest",
                Value::from(format!("sha256:{}", "5".repeat(64))),
            ),
            ("name", Value::from("taira-validator")),
            (
                "tags",
                Value::Array(tags.into_iter().map(Value::from).collect()),
            ),
        ]);
        let subject = object([
            ("commit", Value::from("1".repeat(40))),
            ("dpn_validator_release_commit", Value::from("5".repeat(40))),
            ("exact12", exact12),
            ("native_release_evidence", Value::Array(evidence_rows)),
            (
                "native_verifier_protocol",
                Value::from(NATIVE_VERIFIER_PROTOCOL_V1),
            ),
            ("native_verifier_sha256", Value::from("3".repeat(64))),
            ("release_profile", Value::from("release")),
            ("schema", Value::from(SUBJECT_SCHEMA_V1)),
            ("schema_version", Value::from(1_u64)),
            (
                "signing_authority_fingerprint_sha256",
                Value::from("2".repeat(64)),
            ),
            ("subject", release_subject),
            (
                "workspace_source_manifest_sha256",
                Value::from(hex::encode(source_sha256)),
            ),
        ]);
        Fixture {
            _directory: directory,
            subject,
            manifest,
            artifacts,
            paths,
        }
    }

    pub(crate) fn authority_service_fixture() -> AuthorityServiceFixtureV1 {
        let Fixture {
            _directory,
            subject,
            manifest,
            artifacts: _,
            paths,
        } = fixture();
        for path in &paths {
            fs::set_permissions(path, fs::Permissions::from_mode(0o400))
                .expect("make authority-service fixture immutable");
        }
        let manifest = Value::Array(
            manifest
                .into_iter()
                .map(|entry| {
                    object([
                        ("name", Value::from(entry.name)),
                        ("ordinal", Value::from(u64::from(entry.ordinal))),
                        ("sha256", Value::from(hex::encode(entry.sha256))),
                        ("size", Value::from(entry.size)),
                    ])
                })
                .collect(),
        );
        AuthorityServiceFixtureV1 {
            _directory,
            subject,
            manifest,
            paths,
        }
    }

    fn expectations_fixture() -> PrivacyReleaseExpectationsV1 {
        let mut stages = Vec::with_capacity(STAGE_COUNT_V1);
        for (protocol_index, protocol) in PrivacyProtocolIdV1::ALL.into_iter().enumerate() {
            for (case_index, case) in PrivacyReleaseCaseKindV1::ALL.into_iter().enumerate() {
                let ordinal = protocol_index * CASE_COUNT_V1 + case_index;
                let proof = vec![u8::try_from(ordinal).unwrap(), 0xa5];
                let failure_class = match case {
                    PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd
                    | PrivacyReleaseCaseKindV1::MaximumShapeResource => {
                        PrivacyReleaseFailureClassV1::NotApplicable
                    }
                    PrivacyReleaseCaseKindV1::PublicStatementBindingMutation => {
                        PrivacyReleaseFailureClassV1::PublicStatementBindingRejected
                    }
                    PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation => {
                        PrivacyReleaseFailureClassV1::CanonicalWireCorruptionAndTruncationRejected
                    }
                };
                stages.push(PrivacyReleaseExpectedStageV1 {
                    evidence: PrivacyReleaseStageEvidenceV1 {
                        schema_version: 1,
                        stage_ordinal: ordinal as u16,
                        protocol_id: protocol,
                        case_kind: case,
                        protocol_descriptor: format!("descriptor-{protocol_index}"),
                        public_statement_sha256: [u8::try_from(ordinal + 1).unwrap(); 32],
                        proof_artifacts: vec![PrivacyReleaseProofArtifactEvidenceV1 {
                            artifact_ordinal: 0,
                            canonical_proof_bytes: proof.clone(),
                            proof_sha256: sha256(&proof),
                            proof_bytes_ceiling: 1024,
                        }],
                        failure_class,
                        resources: PrivacyReleaseResourceFactsV1 {
                            primary_units: 1,
                            primary_ceiling: 1,
                            secondary_units: 0,
                            secondary_ceiling: 1,
                            relation_depth: 0,
                            relation_depth_ceiling: 1,
                        },
                    },
                    max_elapsed_millis: 10,
                    max_peak_rss_bytes: 10,
                    max_address_space_bytes: 10,
                });
            }
        }
        PrivacyReleaseExpectationsV1 {
            schema_version: 1,
            stage_count: STAGE_COUNT_V1 as u16,
            stages,
        }
    }

    fn isolation_fixture() -> PrivacyReleaseIsolationPolicyV1 {
        PrivacyReleaseIsolationPolicyV1 {
            stage_rayon_threads: 4,
            main_thread_stack_bytes: 1,
            rayon_worker_stack_bytes: 1,
            watchdog_thread_stack_bytes: 1,
            max_stage_tasks: 6,
            max_stage_open_files: 4,
            max_stage_result_file_bytes: 1,
            max_stage_diagnostic_bytes: 1,
            core_dump_bytes: 0,
            static_elf_only: true,
            anonymous_sealed_runner: true,
            anonymous_result_descriptor_only: true,
            exact_environment_only: true,
            landlock_abi_minimum: 3,
            seccomp_tsync: true,
        }
    }

    fn x509_fixture(
        expectations_norito_sha256: [u8; 32],
        expectations_json_sha256: [u8; 32],
    ) -> PrivacyReleaseZkX509ResourceCertificateV1 {
        let process_limits = PrivacyReleaseZkX509ResourceProcessLimitsV1 {
            elapsed_ceiling_millis: 10,
            peak_rss_ceiling_bytes: 10,
            address_space_ceiling_bytes: 10,
            main_thread_stack_bytes: 1,
            rayon_worker_stack_bytes: 1,
            watchdog_thread_stack_bytes: 1,
            rayon_worker_count: 4,
            max_stage_tasks: 6,
            max_stage_open_files: 4,
            core_dump_bytes: 0,
            landlock_abi_minimum: 3,
            minimum_effective_memory_bytes: 1,
            cgroup_v2: true,
            cpu_quota_unlimited: true,
            landlock_restrict_self: true,
            anchored_openat2: true,
            memfd_exec: true,
            memfd_seal_exec: true,
            static_elf_only: true,
            seccomp_tsync: true,
        };
        let observation = |case_kind| PrivacyReleaseZkX509ResourceObservationV1 {
            case_kind,
            elapsed_millis: 1,
            peak_rss_bytes: 1,
            peak_address_space_bytes: 1,
            primary_units: 1,
            primary_ceiling: 1,
            secondary_units: 0,
            secondary_ceiling: 1,
            relation_depth: 0,
            relation_depth_ceiling: 1,
        };
        PrivacyReleaseZkX509ResourceCertificateV1 {
            schema_version: 1,
            protocol_id: PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
            compiled_profile_digest: [0x11; 32],
            environment: PrivacyReleaseZkX509ResourceEnvironmentV1 {
                operating_system: "linux".to_owned(),
                architecture: "aarch64".to_owned(),
                endianness: "little".to_owned(),
                kernel_minimum_major: 6,
                kernel_minimum_minor: 1,
                rustc_release: "1.90.0".to_owned(),
                rustc_host: "aarch64-unknown-linux-gnu".to_owned(),
                rustc_commit_hash: "1".repeat(40),
                rustc_commit_date: "2025-01-01".to_owned(),
                instance_type: "fixture".to_owned(),
                cpu_model: "fixture".to_owned(),
                logical_cpu_count: 4,
                online_cpu_count: 4,
                affinity_cpu_count: 4,
            },
            expectations_norito_sha256,
            expectations_json_sha256,
            kat_proof_bytes: 1,
            kat_proof_sha256: [0x22; 32],
            process_limits,
            positive: observation(PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd),
            maximum: observation(PrivacyReleaseCaseKindV1::MaximumShapeResource),
            certificate_sha256: [0x33; 32],
        }
    }

    fn pair_digest(norito: &[u8], json: &[u8]) -> PrivacyReleaseArtifactPairDigestV1 {
        PrivacyReleaseArtifactPairDigestV1 {
            norito_sha256: sha256(norito),
            json_sha256: sha256(json),
        }
    }

    fn typed_pair<T>(value: &T) -> (Vec<u8>, Vec<u8>)
    where
        T: norito::NoritoSerialize + norito::json::JsonSerialize,
    {
        (
            norito::encode_canonical(value).expect("encode fixture Norito"),
            canonical_json(value),
        )
    }

    fn canonical_json<T: norito::json::JsonSerialize>(value: &T) -> Vec<u8> {
        let mut json = norito::json::to_json_pretty(value).expect("encode fixture JSON");
        json.push('\n');
        json.into_bytes()
    }

    fn archive_members(fixture: &Fixture) -> Vec<(String, Vec<u8>)> {
        MANIFEST_NAMES_V1
            .iter()
            .enumerate()
            .map(|(index, name)| {
                (
                    format!(
                        "taira-test/{}",
                        name.strip_prefix("evidence/").expect("evidence prefix")
                    ),
                    fs::read(&fixture.paths[index]).expect("read archive member fixture"),
                )
            })
            .collect()
    }

    fn canonical_archive(members: &[(String, Vec<u8>)]) -> Vec<u8> {
        let files = members
            .iter()
            .map(|(path, payload)| {
                sorafs_car::bundle_archive::BundleArchiveFile::new(path, 0o644, payload)
            })
            .collect::<Vec<_>>();
        sorafs_car::bundle_archive::write_gzip_ustar(Vec::new(), &files)
            .expect("encode canonical archive fixture")
    }

    #[derive(Clone)]
    struct RawArchiveEntry {
        path: String,
        kind: u8,
        payload: Vec<u8>,
        declared_size: Option<u64>,
    }

    fn assert_raw_archive_rejected(entries: &[RawArchiveEntry]) {
        let decoded = raw_ustar(entries);
        let archive = stored_gzip(&decoded);
        let mut fixture = fixture();
        fixture.install_archive("taira-test.tar.gz", &archive);
        assert_eq!(fixture.validate(), Err(TairaAuthorityErrorV1::Rejected));
    }

    fn raw_ustar(entries: &[RawArchiveEntry]) -> Vec<u8> {
        const BLOCK: usize = 512;
        let mut archive = Vec::new();
        for entry in entries {
            assert!(entry.path.len() <= 100);
            let declared_size = entry.declared_size.unwrap_or(entry.payload.len() as u64);
            let mut header = [0_u8; BLOCK];
            header[..entry.path.len()].copy_from_slice(entry.path.as_bytes());
            write_test_octal(
                &mut header[100..108],
                if entry.kind == b'5' { 0o755 } else { 0o644 },
            );
            write_test_octal(&mut header[108..116], 0);
            write_test_octal(&mut header[116..124], 0);
            write_test_octal(&mut header[124..136], declared_size);
            write_test_octal(&mut header[136..148], 0);
            header[156] = entry.kind;
            header[257..263].copy_from_slice(b"ustar\0");
            header[263..265].copy_from_slice(b"00");
            write_test_octal(&mut header[329..337], 0);
            write_test_octal(&mut header[337..345], 0);
            header[148..156].fill(b' ');
            let checksum: u64 = header.iter().map(|byte| u64::from(*byte)).sum();
            let rendered = format!("{checksum:06o}\0 ");
            header[148..156].copy_from_slice(rendered.as_bytes());
            archive.extend_from_slice(&header);
            if entry.kind == b'0' {
                archive.extend_from_slice(&entry.payload);
                let padding = (BLOCK - entry.payload.len() % BLOCK) % BLOCK;
                archive.resize(archive.len() + padding, 0);
            }
        }
        archive.resize(archive.len() + BLOCK * 2, 0);
        archive
    }

    fn write_test_octal(field: &mut [u8], value: u64) {
        let digits = field.len() - 1;
        let rendered = format!("{value:0digits$o}");
        assert_eq!(rendered.len(), digits);
        field[..digits].copy_from_slice(rendered.as_bytes());
        field[digits] = 0;
    }

    fn stored_gzip(decoded: &[u8]) -> Vec<u8> {
        let mut gzip = vec![0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff];
        let block_count = decoded.len().div_ceil(u16::MAX as usize);
        for (index, chunk) in decoded.chunks(u16::MAX as usize).enumerate() {
            gzip.push(u8::from(index + 1 == block_count));
            let length = u16::try_from(chunk.len()).expect("stored DEFLATE block length");
            gzip.extend_from_slice(&length.to_le_bytes());
            gzip.extend_from_slice(&(!length).to_le_bytes());
            gzip.extend_from_slice(chunk);
        }
        gzip.extend_from_slice(&crc32(decoded).to_le_bytes());
        gzip.extend_from_slice(&(decoded.len() as u32).to_le_bytes());
        gzip
    }

    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = u32::MAX;
        for byte in bytes {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = if crc & 1 == 0 {
                    crc >> 1
                } else {
                    (crc >> 1) ^ 0xedb8_8320
                };
            }
        }
        !crc
    }

    fn object<const N: usize>(fields: [(&str, Value); N]) -> Value {
        let mut object = Map::new();
        for (name, value) in fields {
            object.insert(name.to_owned(), value);
        }
        Value::Object(object)
    }

    fn artifact_path(fixture: &Fixture, name: &str) -> PathBuf {
        let index = fixture
            .manifest
            .iter()
            .position(|entry| entry.name == name)
            .expect("fixture artifact name");
        fixture.paths[index].clone()
    }
}
