//! Offline canonical validator for public SCCP release-lane evidence.
//!
//! This binary is intentionally read-only and deterministic. It accepts one
//! bounded canonical Norito JSON file, invokes the same typed native verifier
//! used by admission, and emits one bounded machine-readable validation
//! receipt. It never accesses a network, signs data, or writes files.

use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
    process::ExitCode,
};

use iroha_data_model::bridge::{
    SccpDestinationDeploymentV1, SccpEvmDestinationDeploymentV1, SccpGovernedRouteV1,
    SccpGroth16Bn254VerifyingKeyV1, SccpNativeTrustAnchorV1, SccpNetworkV1, SccpSourceIdentityV1,
    SccpTronDestinationDeploymentV1, sccp_network_identity_hash_v1,
};
use iroha_sccp::{
    SccpNativeInboundMessageProofV1, ValidatedSccpNativeInboundMessageV1,
    canonical_sccp_groth16_bn254_verifying_key_bytes_v1, sccp_groth16_bn254_verifying_key_hash_v1,
    sccp_native_inbound_source_available_v1, verify_sccp_native_inbound_message_proof_v1,
};
use sha2::{Digest, Sha256};
use tiny_keccak::{Hasher as _, Keccak};

const INPUT_SCHEMA: &str = "sccp-release-lane-evidence-v1";
const OUTPUT_SCHEMA: &str = "sccp-release-lane-validation-v1";
const RELEASE_SIGNATURE_OUTPUT_SCHEMA: &str = "sccp-release-signature-validation-v1";
const PRODUCTION_POLICY_SCHEMA: &str = "sccp-release-trust-policy-v1";
const TEST_POLICY_SCHEMA: &str = "sccp-release-test-trust-policy-v1";
const RELEASE_EVIDENCE_SCHEMA: &str = "sccp-release-evidence-v1";
const VALIDATOR_PROTOCOL_VERSION: u8 = 1;
const MAX_INPUT_BYTES: u64 = 40 * 1024 * 1024;
const MAX_LANE_INPUT_BYTES: u64 = 16 * 1024 * 1024;
const MAX_RELEASE_POLICY_BYTES: u64 = 64 * 1024;
const MAX_RELEASE_EVIDENCE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_TRANSCRIPT_BYTES: u64 = 4 * 1024 * 1024;
const MAX_TOTAL_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 16 * 1024;
const MAX_RUNTIME_CODE_BYTES: usize = 24_576;
const MAX_DESTINATION_ATTESTATION_AGE_MS: u64 = 24 * 60 * 60 * 1_000;
const BUILD_ID_DOMAIN: &[u8] = b"sccp:release-evidence-validator:v1\0";
const DESTINATION_ATTESTATION_DOMAIN: &[u8] = b"iroha:sccp:destination-state-attestation:v1\0";
const RELEASE_SIGNING_DOMAIN: &[u8] = b"iroha:sccp:release-evidence:v1\0";
const CIRCUIT_AUDIT_DOMAIN: &[u8] = b"iroha:sccp:circuit-policy-audit:v1\0";
const RELEASE_PROFILES: [&str; 3] = ["ethereum-mainnet", "bsc-mainnet", "tron-mainnet"];
const RELEASE_DOMAINS: [u32; 3] = [1, 2, 5];
const RELEASE_ROLES: [&str; 2] = ["release-engineering", "release-security"];
const CIRCUIT_AUDIT_ROLES: [&str; 2] = ["semantic-security-audit", "prover-reproducibility-audit"];
const REQUIRED_PHASES: [&str; 10] = [
    "rust-sccp",
    "evidence-scripts",
    "js-sdk",
    "python-sdk",
    "swift-sdk",
    "kotlin-sdk",
    "java-android",
    "dotnet-sdk",
    "contract-smoke",
    "core-admission",
];
const FORBIDDEN_ALGEBRAIC_SMOKE_VK: [u8; 32] = [
    0x9e, 0xf8, 0x06, 0x7d, 0x26, 0x05, 0x32, 0xf8, 0x8e, 0x60, 0xcf, 0xa4, 0xb4, 0x58, 0xfe, 0x67,
    0x8f, 0xc4, 0x6b, 0x9c, 0x24, 0x2d, 0xe1, 0x8f, 0xc9, 0x1b, 0xa6, 0x46, 0xe0, 0x85, 0x7f, 0xc4,
];
const OUTBOUND_UNAVAILABLE_REASON: &str = "authenticated-destination-state-is-unavailable";

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct ReleaseLaneEvidenceV1 {
    schema: String,
    version: u8,
    profile: SccpNetworkV1,
    inbound: ReleaseInboundEvidenceV1,
    outbound: ReleaseOutboundEvidenceV1,
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
#[norito(tag = "status", content = "evidence", rename_all = "snake_case")]
enum ReleaseInboundEvidenceV1 {
    Available(AvailableInboundEvidenceV1),
    Unavailable(UnavailableDirectionV1),
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct AvailableInboundEvidenceV1 {
    proof: SccpNativeInboundMessageProofV1,
    governed_source_identity: SccpSourceIdentityV1,
    governed_trust_anchor: SccpNativeTrustAnchorV1,
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
#[norito(tag = "status", content = "evidence", rename_all = "snake_case")]
enum ReleaseOutboundEvidenceV1 {
    Available(AvailableOutboundEvidenceV1),
    Unavailable(UnavailableDirectionV1),
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct AvailableOutboundEvidenceV1 {
    statement: DestinationStateStatementV1,
    attestor_id: String,
    signature_hex: String,
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
#[norito(tag = "family", content = "state", rename_all = "snake_case")]
enum DestinationStateStatementV1 {
    Evm(EvmDestinationStateV1),
    Tron(TronDestinationStateV1),
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct EvmDestinationStateV1 {
    schema: String,
    profile: SccpNetworkV1,
    observed_at_unix_ms: u64,
    finalized_block_height: u64,
    finalized_block_hash: [u8; 32],
    rpc_chain_id: u64,
    network_identity_hash: [u8; 32],
    governed_route: SccpGovernedRouteV1,
    route_revision: u32,
    token_bridge_address: [u8; 20],
    route_token_address: [u8; 20],
    route_verifier_address: [u8; 20],
    token_runtime_code_hex: String,
    verifier_runtime_code_hex: String,
    route_runtime_code_hex: String,
    verifier_key_hash: [u8; 32],
    verifying_key: SccpGroth16Bn254VerifyingKeyV1,
    destination_binding_hash: [u8; 32],
    route_configuration_hash: [u8; 32],
    governed_route_configuration_hash: [u8; 32],
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct TronDestinationStateV1 {
    schema: String,
    profile: SccpNetworkV1,
    observed_at_unix_ms: u64,
    solid_block_height: u64,
    solid_block_hash: [u8; 32],
    network_magic: u32,
    network_identity_hash: [u8; 32],
    governed_route: SccpGovernedRouteV1,
    route_revision: u32,
    token_bridge_address: [u8; 20],
    route_token_address: [u8; 20],
    route_verifier_address: [u8; 20],
    token_runtime_code_hex: String,
    verifier_runtime_code_hex: String,
    route_runtime_code_hex: String,
    verifier_key_hash: [u8; 32],
    verifying_key: SccpGroth16Bn254VerifyingKeyV1,
    destination_binding_hash: [u8; 32],
    route_configuration_hash: [u8; 32],
    governed_route_configuration_hash: [u8; 32],
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct UnavailableDirectionV1 {
    reason: String,
}

#[derive(Debug, Clone, norito::JsonSerialize)]
struct ReleaseLaneValidationV1 {
    schema: String,
    validator: ValidatorIdentityV1,
    trust_policy_id: String,
    trust_policy_sha256_hex: String,
    release_id: String,
    release_evidence_sha256_hex: String,
    artifact_sha256_hex: String,
    profile: String,
    inbound_status: String,
    outbound_status: String,
    unavailable_reasons: Vec<String>,
    source_profile: Option<String>,
    target_profile: Option<String>,
    lane_hash_hex: Option<String>,
    source_identity_hash_hex: Option<String>,
    native_anchor_hash_hex: Option<String>,
    message_id_hex: Option<String>,
    payload_hash_hex: Option<String>,
    source_event_digest_hex: Option<String>,
    finality_height: Option<String>,
    finality_block_hash_hex: Option<String>,
    destination_attestor_id: Option<String>,
    destination_statement_sha256_hex: Option<String>,
    destination_observed_at_unix_ms: Option<String>,
    destination_finality_height: Option<String>,
    destination_finality_block_hash_hex: Option<String>,
    destination_binding_hash_hex: Option<String>,
    route_configuration_hash_hex: Option<String>,
    governed_route_configuration_hash_hex: Option<String>,
    verifier_key_hash_hex: Option<String>,
    route_revision: Option<String>,
    verifying_key_sha256_hex: Option<String>,
    semantic_circuit_id: Option<String>,
    circuit_artifact_sha256_hex: Option<String>,
    prover_build_sha256_hex: Option<String>,
    toolchain_lock_sha256_hex: Option<String>,
    destination_build_policy_sha256_hex: Option<String>,
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct ValidatorIdentityV1 {
    protocol_version: u8,
    crate_version: String,
    source_sha256_hex: String,
    build_identity_hex: String,
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct ReleaseTrustPolicyV1 {
    schema: String,
    environment: String,
    policy_id: String,
    roles: Vec<TrustedReleaseRoleV1>,
    destination_attestors: Vec<TrustedDestinationAttestorV1>,
    circuit_auditors: Vec<TrustedCircuitAuditorV1>,
    proof_systems: Vec<ProofSystemPolicyV1>,
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct TrustedReleaseRoleV1 {
    role: String,
    signer_id: String,
    public_key_hex: String,
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct TrustedDestinationAttestorV1 {
    counterparty_profile: String,
    attestor_id: String,
    public_key_hex: String,
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct TrustedCircuitAuditorV1 {
    role: String,
    auditor_id: String,
    public_key_hex: String,
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct ProofSystemPolicyV1 {
    counterparty_profile: String,
    circuit_id: String,
    semantics: Vec<String>,
    circuit_artifact_sha256_hex: String,
    verifier_key_hash_hex: String,
    route_revision: u32,
    verifying_key_sha256_hex: String,
    prover_build_sha256_hex: String,
    toolchain_lock_sha256_hex: String,
    destination_build: DestinationBuildPolicyV1,
    audit_attestations: Vec<CircuitAuditAttestationV1>,
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct DestinationBuildPolicyV1 {
    source_bundle_sha256_hex: String,
    compiler_build_sha256_hex: String,
    token_artifact_sha256_hex: String,
    token_interface_sha256_hex: String,
    token_runtime_hash_hex: String,
    verifier_artifact_sha256_hex: String,
    verifier_interface_sha256_hex: String,
    verifier_runtime_hash_hex: String,
    route_artifact_sha256_hex: String,
    route_interface_sha256_hex: String,
    route_runtime_hash_hex: String,
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct CircuitAuditAttestationV1 {
    role: String,
    auditor_id: String,
    algorithm: String,
    public_key_hex: String,
    report_sha256_hex: String,
    signature_b64: String,
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct ReleaseEvidenceSignaturesV1 {
    schema: String,
    release_id: String,
    protocol_version: u8,
    hub_profile: String,
    hub_chain_id: String,
    created_at_unix_ms: u64,
    trust_policy_id: String,
    validator: ValidatorIdentityV1,
    lanes: Vec<SignedLaneSummaryV1>,
    artifacts: Vec<ReleaseArtifactV1>,
    validation: ReleaseValidationV1,
    provenance: Vec<ReleaseProvenanceV1>,
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct SignedLaneSummaryV1 {
    counterparty_profile: String,
    counterparty_domain: u32,
    inbound_status: String,
    outbound_status: String,
    evidence_artifact_path: String,
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct ReleaseArtifactV1 {
    path: String,
    kind: String,
    sha256_hex: String,
    size_bytes: u64,
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct ReleaseValidationV1 {
    corridor: String,
    phases: Vec<ReleaseValidationPhaseV1>,
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct ReleaseValidationPhaseV1 {
    name: String,
    status: String,
    artifact_path: String,
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct ReleaseProvenanceV1 {
    role: String,
    signer_id: String,
    algorithm: String,
    public_key_hex: String,
    signature_b64: String,
}

#[derive(Debug, Clone, norito::JsonSerialize)]
struct ReleaseSignatureValidationV1 {
    schema: String,
    environment: String,
    policy_id: String,
    release_id: String,
    policy_sha256_hex: String,
    evidence_sha256_hex: String,
    release_signatures_verified: u8,
    circuit_audit_signatures_verified: u8,
    destination_attestors_validated: u8,
    distinct_trust_identities: u8,
}

#[derive(Debug, Clone)]
struct ApprovedProofSystemV1 {
    circuit_id: String,
    circuit_artifact_sha256: [u8; 32],
    verifier_key_hash: [u8; 32],
    route_revision: u32,
    verifying_key_sha256: [u8; 32],
    prover_build_sha256: [u8; 32],
    toolchain_lock_sha256: [u8; 32],
    destination_build_policy_sha256: [u8; 32],
    token_runtime_hash: [u8; 32],
    verifier_runtime_hash: [u8; 32],
    route_runtime_hash: [u8; 32],
}

fn lowercase_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn validator_identity() -> ValidatorIdentityV1 {
    let source_hash = sha256(include_bytes!("sccp_release_evidence.rs"));
    let mut build = Sha256::new();
    build.update(BUILD_ID_DOMAIN);
    build.update(source_hash);
    build.update(env!("CARGO_PKG_VERSION").as_bytes());
    ValidatorIdentityV1 {
        protocol_version: VALIDATOR_PROTOCOL_VERSION,
        crate_version: env!("CARGO_PKG_VERSION").to_owned(),
        source_sha256_hex: lowercase_hex(&source_hash),
        build_identity_hex: lowercase_hex(&build.finalize()),
    }
}

fn decode_lower_hex<const N: usize>(value: &str, label: &str) -> Result<[u8; N], String> {
    if value.len() != N * 2
        || !value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(*byte, b'a'..=b'f'))
    {
        return Err(format!(
            "{label} must be exactly {N} bytes of lowercase hex"
        ));
    }
    let mut decoded = [0_u8; N];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let digit = |byte: u8| match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            _ => unreachable!("alphabet checked above"),
        };
        decoded[index] = (digit(pair[0]) << 4) | digit(pair[1]);
    }
    if decoded.iter().all(|byte| *byte == 0) {
        return Err(format!("{label} must not be zero"));
    }
    Ok(decoded)
}

fn canonical_identifier(value: &str) -> bool {
    (1..=128).contains(&value.len())
        && value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && value.as_bytes().iter().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(*byte, b'.' | b'_' | b':' | b'+' | b'-')
        })
}

fn canonical_relative_path(value: &str) -> bool {
    let bytes = value.as_bytes();
    if !(1..=240).contains(&bytes.len())
        || !value.is_ascii()
        || value.starts_with('/')
        || value.ends_with('/')
        || value.contains('\\')
    {
        return false;
    }
    value.split('/').all(|segment| {
        let bytes = segment.as_bytes();
        (1..=96).contains(&bytes.len())
            && bytes
                .first()
                .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
            && bytes.iter().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(*byte, b'.' | b'_' | b'-')
            })
    })
}

fn decode_signature_base64(value: &str) -> Result<[u8; 64], String> {
    if value.len() != 88 || !value.ends_with("==") || !value.is_ascii() {
        return Err("signature must be canonical padded base64 for 64 bytes".to_owned());
    }
    let digit = |byte: u8| -> Option<u8> {
        match byte {
            b'A'..=b'Z' => Some(byte - b'A'),
            b'a'..=b'z' => Some(byte - b'a' + 26),
            b'0'..=b'9' => Some(byte - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    };
    let bytes = value.as_bytes();
    let mut decoded = [0_u8; 64];
    for group in 0..21 {
        let offset = group * 4;
        let a = digit(bytes[offset]).ok_or_else(|| "signature base64 is invalid".to_owned())?;
        let b = digit(bytes[offset + 1]).ok_or_else(|| "signature base64 is invalid".to_owned())?;
        let c = digit(bytes[offset + 2]).ok_or_else(|| "signature base64 is invalid".to_owned())?;
        let d = digit(bytes[offset + 3]).ok_or_else(|| "signature base64 is invalid".to_owned())?;
        decoded[group * 3] = (a << 2) | (b >> 4);
        decoded[group * 3 + 1] = (b << 4) | (c >> 2);
        decoded[group * 3 + 2] = (c << 6) | d;
    }
    let a = digit(bytes[84]).ok_or_else(|| "signature base64 is invalid".to_owned())?;
    let b = digit(bytes[85]).ok_or_else(|| "signature base64 is invalid".to_owned())?;
    if b & 0x0f != 0 {
        return Err("signature base64 has non-canonical pad bits".to_owned());
    }
    decoded[63] = (a << 2) | (b >> 4);
    Ok(decoded)
}

fn validate_ed25519_public_key(value: &str, label: &str) -> Result<[u8; 32], String> {
    let key = decode_lower_hex::<32>(value, label)?;
    iroha_crypto::PublicKey::from_bytes(iroha_crypto::Algorithm::Ed25519, &key)
        .map_err(|_| format!("{label} is not a strict Ed25519 public key"))?;
    Ok(key)
}

fn verify_ed25519_signature(
    key: &[u8; 32],
    signature: &[u8; 64],
    message: &[u8],
) -> Result<(), String> {
    iroha_crypto::ed25519_verify_batch_deterministic(
        &[message],
        &[signature.as_slice()],
        &[key.as_slice()],
        sha256(message),
    )
    .map_err(|_| "detached Ed25519 signature is invalid".to_owned())
}

fn parse_canonical_sorted_json(bytes: &[u8], label: &str) -> Result<norito::json::Value, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| format!("{label} must be UTF-8"))?;
    if text.as_bytes().contains(&0) {
        return Err(format!("{label} must not contain NUL"));
    }
    let value = norito::json::parse_value(text)
        .map_err(|_| format!("{label} is not strict Norito JSON"))?;
    validate_json_shape(&value, label)?;
    let canonical = norito::json::to_json(&value)
        .map_err(|_| format!("{label} cannot be canonically encoded"))?;
    if format!("{canonical}\n") != text {
        return Err(format!("{label} must be sorted canonical JSON plus one LF"));
    }
    Ok(value)
}

fn validate_json_shape(value: &norito::json::Value, label: &str) -> Result<(), String> {
    const MAX_JSON_DEPTH: usize = 32;
    const MAX_JSON_NODES: usize = 32_768;

    let mut pending = vec![(value, 0_usize)];
    let mut nodes = 0_usize;
    while let Some((current, depth)) = pending.pop() {
        nodes = nodes
            .checked_add(1)
            .ok_or_else(|| format!("{label} JSON node count overflowed"))?;
        if nodes > MAX_JSON_NODES || depth > MAX_JSON_DEPTH {
            return Err(format!("{label} exceeds the canonical JSON shape limits"));
        }
        match current {
            norito::json::Value::Array(values) => {
                pending.extend(values.iter().map(|child| (child, depth + 1)));
            }
            norito::json::Value::Object(values) => {
                pending.extend(values.values().map(|child| (child, depth + 1)));
            }
            norito::json::Value::Number(norito::json::native::Number::F64(_)) => {
                return Err(format!("{label} must not contain floating-point numbers"));
            }
            norito::json::Value::Null
            | norito::json::Value::Bool(_)
            | norito::json::Value::Number(_)
            | norito::json::Value::String(_) => {}
        }
    }
    Ok(())
}

fn require_hash(value: &str, label: &str) -> Result<[u8; 32], String> {
    decode_lower_hex::<32>(value, label)
}

fn validate_destination_build_policy(value: &DestinationBuildPolicyV1) -> Result<(), String> {
    let fields = [
        ("source_bundle_sha256_hex", &value.source_bundle_sha256_hex),
        (
            "compiler_build_sha256_hex",
            &value.compiler_build_sha256_hex,
        ),
        (
            "token_artifact_sha256_hex",
            &value.token_artifact_sha256_hex,
        ),
        (
            "token_interface_sha256_hex",
            &value.token_interface_sha256_hex,
        ),
        ("token_runtime_hash_hex", &value.token_runtime_hash_hex),
        (
            "verifier_artifact_sha256_hex",
            &value.verifier_artifact_sha256_hex,
        ),
        (
            "verifier_interface_sha256_hex",
            &value.verifier_interface_sha256_hex,
        ),
        (
            "verifier_runtime_hash_hex",
            &value.verifier_runtime_hash_hex,
        ),
        (
            "route_artifact_sha256_hex",
            &value.route_artifact_sha256_hex,
        ),
        (
            "route_interface_sha256_hex",
            &value.route_interface_sha256_hex,
        ),
        ("route_runtime_hash_hex", &value.route_runtime_hash_hex),
    ];
    let mut distinct = BTreeSet::new();
    for (field, digest) in fields {
        require_hash(digest, field)?;
        if !distinct.insert(digest.as_str()) {
            return Err("every destination build role must have a distinct digest".to_owned());
        }
    }
    Ok(())
}

struct ValidatedReleaseTrustV1 {
    release_keys: [[u8; 32]; 2],
}

fn validate_release_trust_policy(
    policy: &ReleaseTrustPolicyV1,
    expected_environment: &str,
    signature_set: &mut BTreeSet<[u8; 64]>,
) -> Result<ValidatedReleaseTrustV1, String> {
    let expected_schema = match expected_environment {
        "production" => PRODUCTION_POLICY_SCHEMA,
        "test-fixture" => TEST_POLICY_SCHEMA,
        _ => return Err("release signature mode is invalid".to_owned()),
    };
    if policy.schema != expected_schema
        || policy.environment != expected_environment
        || !canonical_identifier(&policy.policy_id)
        || policy.roles.len() != RELEASE_ROLES.len()
        || policy.destination_attestors.len() != RELEASE_PROFILES.len()
        || policy.circuit_auditors.len() != CIRCUIT_AUDIT_ROLES.len()
        || policy.proof_systems.len() != RELEASE_PROFILES.len()
    {
        return Err("release trust policy has the wrong schema, mode, or cardinality".to_owned());
    }

    let mut identities = BTreeSet::new();
    let mut keys = BTreeSet::new();
    let mut key_encodings = BTreeSet::new();
    let mut release_keys = [[0_u8; 32]; 2];
    for (index, role) in policy.roles.iter().enumerate() {
        if role.role != RELEASE_ROLES[index]
            || !canonical_identifier(&role.signer_id)
            || key_encodings.contains(&role.signer_id)
            || !identities.insert(role.signer_id.clone())
        {
            return Err("release role identity is invalid or reused".to_owned());
        }
        let key = validate_ed25519_public_key(&role.public_key_hex, "release role key")?;
        if identities.contains(&role.public_key_hex)
            || !keys.insert(key)
            || !key_encodings.insert(role.public_key_hex.clone())
        {
            return Err("release trust-policy key is reused".to_owned());
        }
        release_keys[index] = key;
    }

    for (index, attestor) in policy.destination_attestors.iter().enumerate() {
        if attestor.counterparty_profile != RELEASE_PROFILES[index]
            || !canonical_identifier(&attestor.attestor_id)
            || key_encodings.contains(&attestor.attestor_id)
            || !identities.insert(attestor.attestor_id.clone())
        {
            return Err("destination attestor profile or identity is invalid".to_owned());
        }
        let key =
            validate_ed25519_public_key(&attestor.public_key_hex, "destination attestor key")?;
        if identities.contains(&attestor.public_key_hex)
            || !keys.insert(key)
            || !key_encodings.insert(attestor.public_key_hex.clone())
        {
            return Err("destination attestor key is reused".to_owned());
        }
    }

    let mut audit_keys = [[0_u8; 32]; 2];
    for (index, auditor) in policy.circuit_auditors.iter().enumerate() {
        if auditor.role != CIRCUIT_AUDIT_ROLES[index]
            || !canonical_identifier(&auditor.auditor_id)
            || key_encodings.contains(&auditor.auditor_id)
            || !identities.insert(auditor.auditor_id.clone())
        {
            return Err("circuit auditor role or identity is invalid".to_owned());
        }
        let key = validate_ed25519_public_key(&auditor.public_key_hex, "circuit auditor key")?;
        if identities.contains(&auditor.public_key_hex)
            || !keys.insert(key)
            || !key_encodings.insert(auditor.public_key_hex.clone())
        {
            return Err("circuit auditor key is reused".to_owned());
        }
        audit_keys[index] = key;
    }

    for (profile_index, proof) in policy.proof_systems.iter().enumerate() {
        if proof.counterparty_profile != RELEASE_PROFILES[profile_index]
            || !canonical_identifier(&proof.circuit_id)
            || proof.circuit_id.contains("smoke")
            || proof.circuit_id.contains("test")
            || proof.semantics.len() != 2
            || proof.semantics[0] != "nexus-finality-v1"
            || proof.semantics[1] != "sccp-exact-statement-v1"
            || proof.audit_attestations.len() != CIRCUIT_AUDIT_ROLES.len()
        {
            return Err("semantic proof-system policy is invalid".to_owned());
        }
        require_hash(
            &proof.circuit_artifact_sha256_hex,
            "circuit artifact digest",
        )?;
        let verifier_key = require_hash(&proof.verifier_key_hash_hex, "verifier key hash")?;
        if proof.route_revision == 0 {
            return Err("proof-system route revision must be nonzero".to_owned());
        }
        require_hash(&proof.verifying_key_sha256_hex, "full verifying-key digest")?;
        if verifier_key == FORBIDDEN_ALGEBRAIC_SMOKE_VK {
            return Err("algebraic smoke-test verifier key is forbidden".to_owned());
        }
        require_hash(&proof.prover_build_sha256_hex, "prover build digest")?;
        require_hash(&proof.toolchain_lock_sha256_hex, "toolchain lock digest")?;
        validate_destination_build_policy(&proof.destination_build)?;

        let unsigned = value_without_field(
            norito::json::to_value(proof)
                .map_err(|_| "proof policy cannot be encoded".to_owned())?,
            "audit_attestations",
            "proof-system policy",
        )?;
        let unsigned_json = norito::json::to_json(&unsigned)
            .map_err(|_| "proof policy cannot be canonically encoded".to_owned())?;
        for (audit_index, audit) in proof.audit_attestations.iter().enumerate() {
            let trusted = &policy.circuit_auditors[audit_index];
            if audit.role != CIRCUIT_AUDIT_ROLES[audit_index]
                || audit.auditor_id != trusted.auditor_id
                || audit.public_key_hex != trusted.public_key_hex
                || audit.algorithm != "ed25519"
            {
                return Err("circuit audit does not match its trusted role".to_owned());
            }
            let report_hash = require_hash(&audit.report_sha256_hex, "circuit audit report")?;
            let signature = decode_signature_base64(&audit.signature_b64)?;
            if !signature_set.insert(signature) {
                return Err("detached signature is replayed across trust roles".to_owned());
            }
            let mut payload = Vec::with_capacity(
                CIRCUIT_AUDIT_DOMAIN.len() + unsigned_json.len() + report_hash.len(),
            );
            payload.extend_from_slice(CIRCUIT_AUDIT_DOMAIN);
            payload.extend_from_slice(unsigned_json.as_bytes());
            payload.extend_from_slice(&report_hash);
            verify_ed25519_signature(&audit_keys[audit_index], &signature, &payload)?;
        }
    }
    Ok(ValidatedReleaseTrustV1 { release_keys })
}

fn validate_release_evidence_envelope(
    evidence: &ReleaseEvidenceSignaturesV1,
) -> Result<(), String> {
    if evidence.created_at_unix_ms == 0
        || evidence.validation.corridor != "sccp-production-corridor-v1"
        || evidence.validation.phases.len() != REQUIRED_PHASES.len()
        || evidence.artifacts.len() != RELEASE_PROFILES.len() + REQUIRED_PHASES.len()
    {
        return Err("release evidence inventory or corridor is not exact".to_owned());
    }

    let expected_validator = validator_identity();
    if evidence.validator.protocol_version != expected_validator.protocol_version
        || evidence.validator.crate_version != expected_validator.crate_version
        || evidence.validator.source_sha256_hex != expected_validator.source_sha256_hex
        || evidence.validator.build_identity_hex != expected_validator.build_identity_hex
    {
        return Err("release evidence selects a different Rust validator build".to_owned());
    }

    let mut artifact_by_path = BTreeMap::new();
    let mut artifact_hashes = BTreeSet::new();
    let mut previous_path: Option<&str> = None;
    let mut total_size = 0_u64;
    for artifact in &evidence.artifacts {
        if !canonical_relative_path(&artifact.path)
            || previous_path.is_some_and(|previous| artifact.path.as_str() <= previous)
        {
            return Err("release artifact paths are unsafe, duplicated, or unsorted".to_owned());
        }
        previous_path = Some(&artifact.path);
        let maximum = match artifact.kind.as_str() {
            "phase-transcript" => MAX_TRANSCRIPT_BYTES,
            "lane-evidence" => MAX_LANE_INPUT_BYTES,
            _ => return Err("release artifact kind is not part of SCCP V1".to_owned()),
        };
        if artifact.size_bytes == 0 || artifact.size_bytes > maximum {
            return Err("release artifact size is outside its kind-specific bound".to_owned());
        }
        total_size = total_size
            .checked_add(artifact.size_bytes)
            .ok_or_else(|| "release artifact total size overflowed".to_owned())?;
        if total_size > MAX_TOTAL_ARTIFACT_BYTES {
            return Err("release artifact total exceeds the SCCP V1 bound".to_owned());
        }
        let digest = require_hash(&artifact.sha256_hex, "release artifact digest")?;
        if !artifact_hashes.insert(digest)
            || artifact_by_path
                .insert(artifact.path.as_str(), artifact)
                .is_some()
        {
            return Err("release artifact paths and digests must be distinct".to_owned());
        }
    }

    let mut referenced = BTreeSet::new();
    for (index, phase) in evidence.validation.phases.iter().enumerate() {
        if phase.name != REQUIRED_PHASES[index]
            || phase.status != "passed"
            || !canonical_relative_path(&phase.artifact_path)
        {
            return Err("release validation phases are not exact, ordered passes".to_owned());
        }
        let artifact = artifact_by_path
            .get(phase.artifact_path.as_str())
            .ok_or_else(|| "release validation phase references no artifact".to_owned())?;
        if artifact.kind != "phase-transcript" || !referenced.insert(phase.artifact_path.as_str()) {
            return Err("release validation phases must reference distinct transcripts".to_owned());
        }
    }

    for (index, lane) in evidence.lanes.iter().enumerate() {
        if lane.counterparty_profile != RELEASE_PROFILES[index]
            || lane.counterparty_domain != RELEASE_DOMAINS[index]
            || !matches!(lane.inbound_status.as_str(), "verified" | "unavailable")
            || !matches!(lane.outbound_status.as_str(), "verified" | "unavailable")
            || !canonical_relative_path(&lane.evidence_artifact_path)
        {
            return Err("release evidence lane matrix is not exact".to_owned());
        }
        let artifact = artifact_by_path
            .get(lane.evidence_artifact_path.as_str())
            .ok_or_else(|| "release lane references no artifact".to_owned())?;
        if artifact.kind != "lane-evidence"
            || !referenced.insert(lane.evidence_artifact_path.as_str())
        {
            return Err("release lanes must reference distinct typed evidence".to_owned());
        }
    }
    if referenced.len() != artifact_by_path.len() {
        return Err("release evidence contains an unreferenced artifact".to_owned());
    }
    Ok(())
}

fn validate_release_context(
    policy_bytes: &[u8],
    evidence_bytes: &[u8],
    expected_environment: &str,
) -> Result<
    (
        ReleaseTrustPolicyV1,
        ReleaseEvidenceSignaturesV1,
        ReleaseSignatureValidationV1,
    ),
    String,
> {
    let policy_value = parse_canonical_sorted_json(policy_bytes, "release trust policy")?;
    let evidence_value = parse_canonical_sorted_json(evidence_bytes, "release evidence")?;
    let policy_text =
        std::str::from_utf8(policy_bytes).map_err(|_| "release trust policy is not UTF-8")?;
    let evidence_text =
        std::str::from_utf8(evidence_bytes).map_err(|_| "release evidence is not UTF-8")?;
    let policy = norito::json::from_str::<ReleaseTrustPolicyV1>(policy_text)
        .map_err(|_| "release trust policy does not match its typed schema".to_owned())?;
    let evidence = norito::json::from_str::<ReleaseEvidenceSignaturesV1>(evidence_text)
        .map_err(|_| "release evidence does not match its typed signature schema".to_owned())?;
    if norito::json::to_value(&policy).ok().as_ref() != Some(&policy_value)
        || norito::json::to_value(&evidence).ok().as_ref() != Some(&evidence_value)
    {
        return Err("release policy or evidence has unknown or mistyped fields".to_owned());
    }

    let mut signatures = BTreeSet::new();
    let trust = validate_release_trust_policy(&policy, expected_environment, &mut signatures)?;
    if evidence.schema != RELEASE_EVIDENCE_SCHEMA
        || evidence.protocol_version != 1
        || !canonical_identifier(&evidence.release_id)
        || evidence.trust_policy_id != policy.policy_id
        || evidence.lanes.len() != RELEASE_PROFILES.len()
        || evidence.provenance.len() != RELEASE_ROLES.len()
    {
        return Err("release evidence signature envelope is invalid".to_owned());
    }
    match evidence.hub_profile.as_str() {
        "sora-nexus" if evidence.hub_chain_id == "00000000-0000-0000-0000-000000000753" => {}
        "sora-taira" if evidence.hub_chain_id == "809574f5-fee7-5e69-bfcf-52451e42d50f" => {}
        _ => return Err("release evidence SORA hub identity is invalid".to_owned()),
    }
    validate_release_evidence_envelope(&evidence)?;

    let unsigned = value_without_field(evidence_value, "provenance", "release evidence")?;
    let unsigned_json = norito::json::to_json(&unsigned)
        .map_err(|_| "release evidence signing payload cannot be encoded".to_owned())?;
    let mut payload = Vec::with_capacity(RELEASE_SIGNING_DOMAIN.len() + unsigned_json.len());
    payload.extend_from_slice(RELEASE_SIGNING_DOMAIN);
    payload.extend_from_slice(unsigned_json.as_bytes());
    for (index, provenance) in evidence.provenance.iter().enumerate() {
        let trusted = &policy.roles[index];
        if provenance.role != RELEASE_ROLES[index]
            || provenance.signer_id != trusted.signer_id
            || provenance.public_key_hex != trusted.public_key_hex
            || provenance.algorithm != "ed25519"
        {
            return Err("release provenance does not match its trusted role".to_owned());
        }
        let signature = decode_signature_base64(&provenance.signature_b64)?;
        if !signatures.insert(signature) {
            return Err("release signature is replayed across trust roles".to_owned());
        }
        verify_ed25519_signature(&trust.release_keys[index], &signature, &payload)?;
    }

    let receipt = ReleaseSignatureValidationV1 {
        schema: RELEASE_SIGNATURE_OUTPUT_SCHEMA.to_owned(),
        environment: expected_environment.to_owned(),
        policy_id: policy.policy_id.clone(),
        release_id: evidence.release_id.clone(),
        policy_sha256_hex: lowercase_hex(&sha256(policy_bytes)),
        evidence_sha256_hex: lowercase_hex(&sha256(evidence_bytes)),
        release_signatures_verified: 2,
        circuit_audit_signatures_verified: 6,
        destination_attestors_validated: 3,
        distinct_trust_identities: 7,
    };
    Ok((policy, evidence, receipt))
}

fn validate_release_signatures(
    policy_bytes: &[u8],
    evidence_bytes: &[u8],
    expected_environment: &str,
) -> Result<ReleaseSignatureValidationV1, String> {
    validate_release_context(policy_bytes, evidence_bytes, expected_environment)
        .map(|(_, _, receipt)| receipt)
}

fn value_without_field(
    value: norito::json::Value,
    field: &str,
    label: &str,
) -> Result<norito::json::Value, String> {
    let norito::json::Value::Object(mut object) = value else {
        return Err(format!("{label} must be an object"));
    };
    if object.remove(field).is_none() {
        return Err(format!("{label} is missing {field}"));
    }
    Ok(norito::json::Value::Object(object))
}

fn decode_runtime_code(value: &str, label: &str) -> Result<Vec<u8>, String> {
    if value.is_empty()
        || !value.len().is_multiple_of(2)
        || value.len() > MAX_RUNTIME_CODE_BYTES * 2
        || !value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(*byte, b'a'..=b'f'))
    {
        return Err(format!(
            "{label} must be bounded lowercase runtime-code hex"
        ));
    }
    let mut decoded = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks_exact(2) {
        let digit = |byte: u8| match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            _ => unreachable!("alphabet checked above"),
        };
        decoded.push((digit(pair[0]) << 4) | digit(pair[1]));
    }
    Ok(decoded)
}

fn keccak256(bytes: &[u8]) -> [u8; 32] {
    let mut hash = [0_u8; 32];
    let mut hasher = Keccak::v256();
    hasher.update(bytes);
    hasher.finalize(&mut hash);
    hash
}

#[derive(Debug)]
struct ValidatedDestinationStateV1 {
    observed_at_unix_ms: u64,
    finality_height: u64,
    finality_block_hash: [u8; 32],
    destination_binding_hash: [u8; 32],
    route_configuration_hash: [u8; 32],
    governed_route_configuration_hash: [u8; 32],
    verifier_key_hash: [u8; 32],
    route_revision: u32,
    verifying_key_sha256: [u8; 32],
    token_runtime_hash: [u8; 32],
    verifier_runtime_hash: [u8; 32],
    route_runtime_hash: [u8; 32],
}

#[derive(Debug)]
struct EvmDestinationReadback<'a> {
    deployment: SccpEvmDestinationDeploymentV1,
    token_bridge_address: [u8; 20],
    route_token_address: [u8; 20],
    route_verifier_address: [u8; 20],
    token_runtime_code_hex: &'a str,
    verifier_runtime_code_hex: &'a str,
    route_runtime_code_hex: &'a str,
    verifier_key_hash: [u8; 32],
    route_revision: u32,
    verifying_key: SccpGroth16Bn254VerifyingKeyV1,
    destination_binding_hash: [u8; 32],
    route_configuration_hash: [u8; 32],
    governed_route_configuration_hash: [u8; 32],
    observed_at_unix_ms: u64,
    finality_height: u64,
    finality_block_hash: [u8; 32],
}

fn validate_evm_destination_state(
    expected_profile: SccpNetworkV1,
    state: &EvmDestinationStateV1,
) -> Result<ValidatedDestinationStateV1, String> {
    let expected_chain_id = match expected_profile {
        SccpNetworkV1::EthereumMainnet => 1,
        SccpNetworkV1::BscMainnet => 56,
        _ => return Err("EVM destination attestation uses the wrong profile".to_owned()),
    };
    if state.schema != "sccp-evm-destination-state-v1"
        || state.profile != expected_profile
        || state.rpc_chain_id != expected_chain_id
        || state.network_identity_hash != sccp_network_identity_hash_v1(expected_profile)
        || state.observed_at_unix_ms == 0
        || state.finalized_block_height == 0
        || state.finalized_block_hash.iter().all(|byte| *byte == 0)
    {
        return Err("EVM destination chain identity/finality is not canonical".to_owned());
    }
    let governed_route = &state.governed_route;
    governed_route
        .validate()
        .map_err(|error| format!("governed EVM route is invalid: {error}"))?;
    if governed_route.lane_id.source != expected_profile
        || governed_route.lane_id.target != SccpNetworkV1::SoraTaira
        || !governed_route.activation.allows_outbound()
    {
        return Err("governed EVM route is not outbound-active for this profile".to_owned());
    }
    let SccpDestinationDeploymentV1::Evm(deployment) = governed_route.destination else {
        return Err("governed EVM route has a different destination family".to_owned());
    };
    validate_destination_readback(
        governed_route,
        EvmDestinationReadback {
            deployment,
            token_bridge_address: state.token_bridge_address,
            route_token_address: state.route_token_address,
            route_verifier_address: state.route_verifier_address,
            token_runtime_code_hex: &state.token_runtime_code_hex,
            verifier_runtime_code_hex: &state.verifier_runtime_code_hex,
            route_runtime_code_hex: &state.route_runtime_code_hex,
            verifier_key_hash: state.verifier_key_hash,
            route_revision: state.route_revision,
            verifying_key: state.verifying_key,
            destination_binding_hash: state.destination_binding_hash,
            route_configuration_hash: state.route_configuration_hash,
            governed_route_configuration_hash: state.governed_route_configuration_hash,
            observed_at_unix_ms: state.observed_at_unix_ms,
            finality_height: state.finalized_block_height,
            finality_block_hash: state.finalized_block_hash,
        },
    )
}

fn validate_destination_readback(
    route: &SccpGovernedRouteV1,
    readback: EvmDestinationReadback<'_>,
) -> Result<ValidatedDestinationStateV1, String> {
    let expected_binding = route
        .destination_binding_hash()
        .map_err(|error| format!("destination binding derivation failed: {error}"))?;
    let expected_route_configuration = route
        .destination
        .route_configuration_hash(
            route.lane_id,
            &route.route_id,
            &route.asset_key,
            route.settlement.payload_amount_scale,
        )
        .map_err(|error| format!("route configuration derivation failed: {error}"))?;
    let expected_governed_configuration = route
        .route_configuration_hash()
        .map_err(|error| format!("governed route derivation failed: {error}"))?;
    let token_code_hash = keccak256(&decode_runtime_code(
        readback.token_runtime_code_hex,
        "token runtime code",
    )?);
    let verifier_code_hash = keccak256(&decode_runtime_code(
        readback.verifier_runtime_code_hex,
        "verifier runtime code",
    )?);
    let route_code_hash = keccak256(&decode_runtime_code(
        readback.route_runtime_code_hex,
        "route runtime code",
    )?);
    let verifying_key_bytes = canonical_sccp_groth16_bn254_verifying_key_bytes_v1(
        &readback.verifying_key,
    )
    .ok_or_else(|| "authenticated EVM verifying key is not a canonical subgroup key".to_owned())?;
    let verifying_key_hash = sccp_groth16_bn254_verifying_key_hash_v1(&readback.verifying_key)
        .ok_or_else(|| "authenticated EVM verifying key cannot be hashed".to_owned())?;
    if readback.token_bridge_address != readback.deployment.route_address
        || readback.route_token_address != readback.deployment.token_address
        || readback.route_verifier_address != readback.deployment.verifier_address
        || token_code_hash != readback.deployment.token_code_hash
        || verifier_code_hash != readback.deployment.verifier_code_hash
        || route_code_hash != readback.deployment.route_code_hash
        || readback.verifier_key_hash != readback.deployment.verifier_key_hash
        || readback.verifying_key != readback.deployment.verifying_key
        || verifying_key_hash != readback.verifier_key_hash
        || readback.route_revision == 0
        || readback.route_revision != route.revision
        || readback.destination_binding_hash != expected_binding
        || readback.route_configuration_hash != expected_route_configuration
        || readback.governed_route_configuration_hash != expected_governed_configuration
    {
        return Err("authenticated EVM destination state differs from governed route".to_owned());
    }
    Ok(ValidatedDestinationStateV1 {
        observed_at_unix_ms: readback.observed_at_unix_ms,
        finality_height: readback.finality_height,
        finality_block_hash: readback.finality_block_hash,
        destination_binding_hash: readback.destination_binding_hash,
        route_configuration_hash: readback.route_configuration_hash,
        governed_route_configuration_hash: readback.governed_route_configuration_hash,
        verifier_key_hash: readback.verifier_key_hash,
        route_revision: readback.route_revision,
        verifying_key_sha256: sha256(&verifying_key_bytes),
        token_runtime_hash: token_code_hash,
        verifier_runtime_hash: verifier_code_hash,
        route_runtime_hash: route_code_hash,
    })
}

fn validate_tron_destination_state(
    expected_profile: SccpNetworkV1,
    state: &TronDestinationStateV1,
) -> Result<ValidatedDestinationStateV1, String> {
    if expected_profile != SccpNetworkV1::TronMainnet
        || state.schema != "sccp-tron-destination-state-v1"
        || state.profile != expected_profile
        || state.network_magic != 0x2b66_53dc
        || state.network_identity_hash != sccp_network_identity_hash_v1(expected_profile)
        || state.observed_at_unix_ms == 0
        || state.solid_block_height == 0
        || state.solid_block_hash.iter().all(|byte| *byte == 0)
    {
        return Err("TRON destination chain identity/finality is not canonical".to_owned());
    }
    let governed_route = &state.governed_route;
    governed_route
        .validate()
        .map_err(|error| format!("governed TRON route is invalid: {error}"))?;
    if governed_route.lane_id.source != expected_profile
        || governed_route.lane_id.target != SccpNetworkV1::SoraTaira
        || !governed_route.activation.allows_outbound()
    {
        return Err("governed TRON route is not outbound-active for this profile".to_owned());
    }
    let SccpDestinationDeploymentV1::Tron(deployment) = governed_route.destination else {
        return Err("governed TRON route has a different destination family".to_owned());
    };
    validate_tron_destination_readback(state, governed_route, deployment)
}

fn validate_tron_destination_readback(
    state: &TronDestinationStateV1,
    route: &SccpGovernedRouteV1,
    deployment: SccpTronDestinationDeploymentV1,
) -> Result<ValidatedDestinationStateV1, String> {
    let expected_binding = route
        .destination_binding_hash()
        .map_err(|error| format!("destination binding derivation failed: {error}"))?;
    let expected_route_configuration = route
        .destination
        .route_configuration_hash(
            route.lane_id,
            &route.route_id,
            &route.asset_key,
            route.settlement.payload_amount_scale,
        )
        .map_err(|error| format!("route configuration derivation failed: {error}"))?;
    let expected_governed_configuration = route
        .route_configuration_hash()
        .map_err(|error| format!("governed route derivation failed: {error}"))?;
    let token_code_hash = keccak256(&decode_runtime_code(
        &state.token_runtime_code_hex,
        "TRON token runtime code",
    )?);
    let verifier_code_hash = keccak256(&decode_runtime_code(
        &state.verifier_runtime_code_hex,
        "TRON verifier runtime code",
    )?);
    let route_code_hash = keccak256(&decode_runtime_code(
        &state.route_runtime_code_hex,
        "TRON route runtime code",
    )?);
    let verifying_key_bytes = canonical_sccp_groth16_bn254_verifying_key_bytes_v1(
        &state.verifying_key,
    )
    .ok_or_else(|| "authenticated TRON verifying key is not a canonical subgroup key".to_owned())?;
    let verifying_key_hash = sccp_groth16_bn254_verifying_key_hash_v1(&state.verifying_key)
        .ok_or_else(|| "authenticated TRON verifying key cannot be hashed".to_owned())?;
    if state.token_bridge_address != deployment.route_address
        || state.route_token_address != deployment.token_address
        || state.route_verifier_address != deployment.verifier_address
        || token_code_hash != deployment.token_code_hash
        || verifier_code_hash != deployment.verifier_code_hash
        || route_code_hash != deployment.route_code_hash
        || state.verifier_key_hash != deployment.verifier_key_hash
        || state.verifying_key != deployment.verifying_key
        || verifying_key_hash != state.verifier_key_hash
        || state.route_revision == 0
        || state.route_revision != route.revision
        || state.destination_binding_hash != expected_binding
        || state.route_configuration_hash != expected_route_configuration
        || state.governed_route_configuration_hash != expected_governed_configuration
    {
        return Err("authenticated TRON destination state differs from governed route".to_owned());
    }
    Ok(ValidatedDestinationStateV1 {
        observed_at_unix_ms: state.observed_at_unix_ms,
        finality_height: state.solid_block_height,
        finality_block_hash: state.solid_block_hash,
        destination_binding_hash: state.destination_binding_hash,
        route_configuration_hash: state.route_configuration_hash,
        governed_route_configuration_hash: state.governed_route_configuration_hash,
        verifier_key_hash: state.verifier_key_hash,
        route_revision: state.route_revision,
        verifying_key_sha256: sha256(&verifying_key_bytes),
        token_runtime_hash: token_code_hash,
        verifier_runtime_hash: verifier_code_hash,
        route_runtime_hash: route_code_hash,
    })
}

fn authenticate_destination_state(
    expected_profile: SccpNetworkV1,
    available: &AvailableOutboundEvidenceV1,
    expected_attestor_id: &str,
    expected_attestor_public_key: [u8; 32],
    approved_proof_system: &ApprovedProofSystemV1,
) -> Result<(ValidatedDestinationStateV1, [u8; 32]), String> {
    if available.attestor_id != expected_attestor_id
        || expected_attestor_id.is_empty()
        || expected_attestor_id.len() > 128
        || !expected_attestor_id.as_bytes().iter().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(*byte, b'.' | b'_' | b':' | b'+' | b'-')
        })
    {
        return Err("destination attestor identity does not match pinned policy".to_owned());
    }
    let signature = decode_lower_hex::<64>(&available.signature_hex, "destination signature")?;
    let statement_json = norito::json::to_json(&available.statement)
        .map_err(|_| "destination statement cannot be canonically encoded".to_owned())?;
    let mut signed =
        Vec::with_capacity(DESTINATION_ATTESTATION_DOMAIN.len() + statement_json.len());
    signed.extend_from_slice(DESTINATION_ATTESTATION_DOMAIN);
    signed.extend_from_slice(statement_json.as_bytes());
    iroha_crypto::ed25519_verify_batch_deterministic(
        &[signed.as_slice()],
        &[signature.as_slice()],
        &[expected_attestor_public_key.as_slice()],
        sha256(&signed),
    )
    .map_err(|_| "destination state has an invalid pinned-attestor signature".to_owned())?;
    let validated = match &available.statement {
        DestinationStateStatementV1::Evm(state) => {
            validate_evm_destination_state(expected_profile, state)?
        }
        DestinationStateStatementV1::Tron(state) => {
            validate_tron_destination_state(expected_profile, state)?
        }
    };
    validate_approved_proof_system(&validated, approved_proof_system)?;
    Ok((validated, sha256(statement_json.as_bytes())))
}

fn validate_approved_proof_system(
    validated: &ValidatedDestinationStateV1,
    approved: &ApprovedProofSystemV1,
) -> Result<(), String> {
    if approved.circuit_id.is_empty()
        || approved.circuit_id.len() > 128
        || !approved.circuit_id.as_bytes().iter().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(*byte, b'.' | b'_' | b':' | b'+' | b'-')
        })
        || approved.circuit_id.contains("smoke")
        || approved.circuit_id.contains("test")
        || approved.verifier_key_hash == FORBIDDEN_ALGEBRAIC_SMOKE_VK
        || validated.verifier_key_hash != approved.verifier_key_hash
        || validated.route_revision != approved.route_revision
        || validated.verifying_key_sha256 != approved.verifying_key_sha256
        || validated.token_runtime_hash != approved.token_runtime_hash
        || validated.verifier_runtime_hash != approved.verifier_runtime_hash
        || validated.route_runtime_hash != approved.route_runtime_hash
    {
        return Err("destination verifier is not the policy-approved semantic circuit".to_owned());
    }
    Ok(())
}

fn approved_proof_system_from_policy(
    proof: &ProofSystemPolicyV1,
) -> Result<ApprovedProofSystemV1, String> {
    let destination_build_value = norito::json::to_value(&proof.destination_build)
        .map_err(|_| "destination build policy cannot be represented as JSON".to_owned())?;
    let destination_build_json = norito::json::to_json(&destination_build_value)
        .map_err(|_| "destination build policy cannot be canonically encoded".to_owned())?;
    Ok(ApprovedProofSystemV1 {
        circuit_id: proof.circuit_id.clone(),
        circuit_artifact_sha256: require_hash(
            &proof.circuit_artifact_sha256_hex,
            "circuit artifact digest",
        )?,
        verifier_key_hash: require_hash(&proof.verifier_key_hash_hex, "verifier key hash")?,
        route_revision: proof.route_revision,
        verifying_key_sha256: require_hash(
            &proof.verifying_key_sha256_hex,
            "full verifying-key digest",
        )?,
        prover_build_sha256: require_hash(&proof.prover_build_sha256_hex, "prover build digest")?,
        toolchain_lock_sha256: require_hash(
            &proof.toolchain_lock_sha256_hex,
            "toolchain lock digest",
        )?,
        destination_build_policy_sha256: sha256(destination_build_json.as_bytes()),
        token_runtime_hash: require_hash(
            &proof.destination_build.token_runtime_hash_hex,
            "token runtime hash",
        )?,
        verifier_runtime_hash: require_hash(
            &proof.destination_build.verifier_runtime_hash_hex,
            "verifier runtime hash",
        )?,
        route_runtime_hash: require_hash(
            &proof.destination_build.route_runtime_hash_hex,
            "route runtime hash",
        )?,
    })
}

fn unavailable_reason_is_canonical(reason: &str) -> bool {
    (1..=160).contains(&reason.len())
        && reason
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && reason
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric)
        && reason
            .as_bytes()
            .iter()
            .copied()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !reason.contains("--")
}

fn exact_unavailable_reason(profile: SccpNetworkV1) -> Option<&'static str> {
    release_profile_supported(profile)
        .then_some("authenticated-native-inbound-proof-is-unavailable")
}

fn release_profile_supported(profile: SccpNetworkV1) -> bool {
    matches!(
        profile,
        SccpNetworkV1::EthereumMainnet | SccpNetworkV1::BscMainnet | SccpNetworkV1::TronMainnet
    )
}

#[cfg(unix)]
fn metadata_is_single_link(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    metadata.nlink() == 1
}

#[cfg(not(unix))]
fn metadata_is_single_link(_metadata: &fs::Metadata) -> bool {
    true
}

#[cfg(unix)]
fn metadata_identity_matches(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(not(unix))]
fn metadata_identity_matches(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.len() == right.len() && left.modified().ok() == right.modified().ok()
}

fn read_direct_input(path: &Path, maximum: u64) -> Result<Vec<u8>, String> {
    let before = fs::symlink_metadata(path).map_err(|_| "input is not accessible")?;
    if before.file_type().is_symlink()
        || !before.is_file()
        || !metadata_is_single_link(&before)
        || before.len() == 0
        || before.len() > maximum
    {
        return Err("input must be one bounded direct regular file".to_owned());
    }
    let mut file = File::open(path).map_err(|_| "input cannot be opened")?;
    let opened = file
        .metadata()
        .map_err(|_| "input metadata cannot be read")?;
    if !opened.is_file()
        || !metadata_is_single_link(&opened)
        || !metadata_identity_matches(&before, &opened)
    {
        return Err("input changed while opening".to_owned());
    }
    let mut bytes = Vec::with_capacity(usize::try_from(before.len()).unwrap_or(0));
    file.by_ref()
        .take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "input cannot be read")?;
    let after = fs::symlink_metadata(path).map_err(|_| "input disappeared while reading")?;
    if bytes.len() as u64 != before.len()
        || !metadata_identity_matches(&before, &after)
        || !metadata_identity_matches(&opened, &after)
    {
        return Err("input changed while reading".to_owned());
    }
    Ok(bytes)
}

fn validated_fields(
    validated: ValidatedSccpNativeInboundMessageV1,
) -> (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
) {
    (
        lowercase_hex(&validated.lane_hash),
        lowercase_hex(&validated.source_identity_hash),
        lowercase_hex(&validated.trust_anchor.anchor_hash),
        lowercase_hex(&validated.message_key.message_id),
        lowercase_hex(&validated.payload_hash),
        lowercase_hex(&validated.source_event_digest),
        validated.source_finality.height.to_string(),
        lowercase_hex(&validated.source_finality.block_hash),
    )
}

fn validate_input(
    bytes: &[u8],
    expected_attestor_id: &str,
    expected_attestor_public_key: [u8; 32],
    approved_proof_system: &ApprovedProofSystemV1,
) -> Result<ReleaseLaneValidationV1, String> {
    let json = std::str::from_utf8(bytes).map_err(|_| "input must be canonical UTF-8")?;
    if json.as_bytes().contains(&0) {
        return Err("input must not contain NUL".to_owned());
    }
    let evidence = norito::json::from_str::<ReleaseLaneEvidenceV1>(json)
        .map_err(|_| "input is not strict SCCP release-lane Norito JSON")?;
    let canonical =
        norito::json::to_json(&evidence).map_err(|_| "input cannot be canonically encoded")?;
    if format!("{canonical}\n") != json {
        return Err("input must equal its canonical Norito JSON encoding plus one LF".to_owned());
    }
    if evidence.schema != INPUT_SCHEMA
        || evidence.version != 1
        || !release_profile_supported(evidence.profile)
    {
        return Err("input does not select an exact production SCCP V1 profile".to_owned());
    }

    let mut receipt = ReleaseLaneValidationV1 {
        schema: OUTPUT_SCHEMA.to_owned(),
        validator: validator_identity(),
        trust_policy_id: String::new(),
        trust_policy_sha256_hex: String::new(),
        release_id: String::new(),
        release_evidence_sha256_hex: String::new(),
        artifact_sha256_hex: lowercase_hex(&sha256(bytes)),
        profile: evidence.profile.profile_key().to_owned(),
        inbound_status: String::new(),
        outbound_status: String::new(),
        unavailable_reasons: Vec::new(),
        source_profile: None,
        target_profile: None,
        lane_hash_hex: None,
        source_identity_hash_hex: None,
        native_anchor_hash_hex: None,
        message_id_hex: None,
        payload_hash_hex: None,
        source_event_digest_hex: None,
        finality_height: None,
        finality_block_hash_hex: None,
        destination_attestor_id: None,
        destination_statement_sha256_hex: None,
        destination_observed_at_unix_ms: None,
        destination_finality_height: None,
        destination_finality_block_hash_hex: None,
        destination_binding_hash_hex: None,
        route_configuration_hash_hex: None,
        governed_route_configuration_hash_hex: None,
        verifier_key_hash_hex: None,
        route_revision: None,
        verifying_key_sha256_hex: None,
        semantic_circuit_id: None,
        circuit_artifact_sha256_hex: None,
        prover_build_sha256_hex: None,
        toolchain_lock_sha256_hex: None,
        destination_build_policy_sha256_hex: None,
    };

    match &evidence.outbound {
        ReleaseOutboundEvidenceV1::Available(available) => {
            let (validated, statement_hash) = authenticate_destination_state(
                evidence.profile,
                available,
                expected_attestor_id,
                expected_attestor_public_key,
                approved_proof_system,
            )?;
            receipt.outbound_status = "verified".to_owned();
            receipt.destination_attestor_id = Some(available.attestor_id.clone());
            receipt.destination_statement_sha256_hex = Some(lowercase_hex(&statement_hash));
            receipt.destination_observed_at_unix_ms =
                Some(validated.observed_at_unix_ms.to_string());
            receipt.destination_finality_height = Some(validated.finality_height.to_string());
            receipt.destination_finality_block_hash_hex =
                Some(lowercase_hex(&validated.finality_block_hash));
            receipt.destination_binding_hash_hex =
                Some(lowercase_hex(&validated.destination_binding_hash));
            receipt.route_configuration_hash_hex =
                Some(lowercase_hex(&validated.route_configuration_hash));
            receipt.governed_route_configuration_hash_hex =
                Some(lowercase_hex(&validated.governed_route_configuration_hash));
            receipt.verifier_key_hash_hex = Some(lowercase_hex(&validated.verifier_key_hash));
            receipt.route_revision = Some(validated.route_revision.to_string());
            receipt.verifying_key_sha256_hex = Some(lowercase_hex(&validated.verifying_key_sha256));
            receipt.semantic_circuit_id = Some(approved_proof_system.circuit_id.clone());
            receipt.circuit_artifact_sha256_hex = Some(lowercase_hex(
                &approved_proof_system.circuit_artifact_sha256,
            ));
            receipt.prover_build_sha256_hex =
                Some(lowercase_hex(&approved_proof_system.prover_build_sha256));
            receipt.toolchain_lock_sha256_hex =
                Some(lowercase_hex(&approved_proof_system.toolchain_lock_sha256));
            receipt.destination_build_policy_sha256_hex = Some(lowercase_hex(
                &approved_proof_system.destination_build_policy_sha256,
            ));
        }
        ReleaseOutboundEvidenceV1::Unavailable(outbound) => {
            if outbound.reason != OUTBOUND_UNAVAILABLE_REASON {
                return Err("outbound must use the exact fail-closed V1 reason".to_owned());
            }
            receipt.outbound_status = "unavailable".to_owned();
            receipt.unavailable_reasons.push(outbound.reason.clone());
        }
    }

    match evidence.inbound {
        ReleaseInboundEvidenceV1::Available(available) => {
            if !sccp_native_inbound_source_available_v1(evidence.profile)
                || available.proof.source.lane.source != evidence.profile
            {
                return Err("profile cannot use an available native inbound proof".to_owned());
            }
            let validated = verify_sccp_native_inbound_message_proof_v1(
                &available.proof,
                &available.governed_source_identity,
                available.governed_trust_anchor,
            )
            .map_err(|error| format!("native inbound proof failed: {error}"))?;
            let source = available.proof.source.lane.source.profile_key().to_owned();
            let target = available.proof.source.lane.target.profile_key().to_owned();
            let (lane, identity, anchor, message, payload, event, height, block) =
                validated_fields(validated);
            receipt.inbound_status = "verified".to_owned();
            receipt.source_profile = Some(source);
            receipt.target_profile = Some(target);
            receipt.lane_hash_hex = Some(lane);
            receipt.source_identity_hash_hex = Some(identity);
            receipt.native_anchor_hash_hex = Some(anchor);
            receipt.message_id_hex = Some(message);
            receipt.payload_hash_hex = Some(payload);
            receipt.source_event_digest_hex = Some(event);
            receipt.finality_height = Some(height);
            receipt.finality_block_hash_hex = Some(block);
        }
        ReleaseInboundEvidenceV1::Unavailable(unavailable) => {
            if !unavailable_reason_is_canonical(&unavailable.reason) {
                return Err("inbound unavailable reason is not canonical".to_owned());
            }
            if let Some(expected) = exact_unavailable_reason(evidence.profile)
                && unavailable.reason != expected
            {
                return Err("inbound must use the exact fail-closed V1 reason".to_owned());
            }
            receipt.inbound_status = "unavailable".to_owned();
            receipt.unavailable_reasons.push(unavailable.reason);
        }
    }
    Ok(receipt)
}

fn validate_lane_in_release_context(
    lane_bytes: &[u8],
    policy_bytes: &[u8],
    evidence_bytes: &[u8],
    expected_environment: &str,
) -> Result<ReleaseLaneValidationV1, String> {
    let (policy, evidence, release_receipt) =
        validate_release_context(policy_bytes, evidence_bytes, expected_environment)?;
    let lane_json = std::str::from_utf8(lane_bytes)
        .map_err(|_| "lane evidence must be canonical UTF-8".to_owned())?;
    let lane_value = norito::json::from_str::<ReleaseLaneEvidenceV1>(lane_json)
        .map_err(|_| "lane evidence does not match the typed SCCP V1 schema".to_owned())?;
    let profile = lane_value.profile.profile_key();
    let profile_index = RELEASE_PROFILES
        .iter()
        .position(|expected| *expected == profile)
        .ok_or_else(|| "lane evidence profile is outside SCCP V1".to_owned())?;
    let trusted_attestor = &policy.destination_attestors[profile_index];
    let trusted_key =
        validate_ed25519_public_key(&trusted_attestor.public_key_hex, "destination attestor key")?;
    let approved = approved_proof_system_from_policy(&policy.proof_systems[profile_index])?;
    let mut receipt = validate_input(
        lane_bytes,
        &trusted_attestor.attestor_id,
        trusted_key,
        &approved,
    )?;

    let signed_lane = &evidence.lanes[profile_index];
    let signed_artifact = evidence
        .artifacts
        .iter()
        .find(|artifact| artifact.path == signed_lane.evidence_artifact_path)
        .ok_or_else(|| "signed lane artifact is absent from the inventory".to_owned())?;
    if signed_artifact.kind != "lane-evidence"
        || signed_artifact.size_bytes != lane_bytes.len() as u64
        || signed_artifact.sha256_hex != receipt.artifact_sha256_hex
        || signed_lane.inbound_status != receipt.inbound_status
        || signed_lane.outbound_status != receipt.outbound_status
    {
        return Err("typed lane result does not match its signed evidence inventory".to_owned());
    }
    if let Some(observed_at_unix_ms) = receipt.destination_observed_at_unix_ms.as_deref() {
        let observed_at_unix_ms = observed_at_unix_ms
            .parse::<u64>()
            .map_err(|_| "destination observation time is not a canonical u64".to_owned())?;
        if observed_at_unix_ms > evidence.created_at_unix_ms
            || evidence.created_at_unix_ms - observed_at_unix_ms
                > MAX_DESTINATION_ATTESTATION_AGE_MS
        {
            return Err("destination state attestation is future-dated or stale".to_owned());
        }
    }
    receipt.trust_policy_id = release_receipt.policy_id;
    receipt.trust_policy_sha256_hex = release_receipt.policy_sha256_hex;
    receipt.release_id = release_receipt.release_id;
    receipt.release_evidence_sha256_hex = release_receipt.evidence_sha256_hex;
    Ok(receipt)
}

fn print_receipt(receipt: &ReleaseLaneValidationV1) -> Result<(), String> {
    let json = norito::json::to_json(receipt)
        .map_err(|_| "validation receipt cannot be encoded".to_owned())?;
    if json.len() > MAX_OUTPUT_BYTES {
        return Err("validation receipt exceeds the output bound".to_owned());
    }
    println!("{json}");
    Ok(())
}

fn print_release_signature_receipt(receipt: &ReleaseSignatureValidationV1) -> Result<(), String> {
    let json = norito::json::to_json(receipt)
        .map_err(|_| "release signature receipt cannot be encoded".to_owned())?;
    if json.len() > MAX_OUTPUT_BYTES {
        return Err("release signature receipt exceeds the output bound".to_owned());
    }
    println!("{json}");
    Ok(())
}

#[cfg(feature = "test-fixtures")]
fn emit_ethereum_fixture() -> Result<(), String> {
    let (proof, governed_source_identity, governed_trust_anchor) =
        iroha_sccp::sccp_native_ethereum_transfer_inbound_test_fixture_v1();
    let evidence = ReleaseLaneEvidenceV1 {
        schema: INPUT_SCHEMA.to_owned(),
        version: 1,
        profile: SccpNetworkV1::EthereumMainnet,
        inbound: ReleaseInboundEvidenceV1::Available(AvailableInboundEvidenceV1 {
            proof,
            governed_source_identity,
            governed_trust_anchor,
        }),
        outbound: ReleaseOutboundEvidenceV1::Unavailable(UnavailableDirectionV1 {
            reason: OUTBOUND_UNAVAILABLE_REASON.to_owned(),
        }),
    };
    let json =
        norito::json::to_json(&evidence).map_err(|_| "fixture cannot be encoded".to_owned())?;
    if json.len() as u64 > MAX_INPUT_BYTES {
        return Err("fixture exceeds input bound".to_owned());
    }
    println!("{json}");
    Ok(())
}

#[cfg(feature = "test-fixtures")]
fn emit_unavailable_fixture(profile: &str) -> Result<(), String> {
    let network = SccpNetworkV1::from_profile_key(profile)
        .filter(|network| release_profile_supported(*network))
        .ok_or_else(|| "fixture profile must be one supported production profile".to_owned())?;
    let reason = exact_unavailable_reason(network)
        .unwrap_or("no-canonical-native-proof-in-release-fixture")
        .to_owned();
    let evidence = ReleaseLaneEvidenceV1 {
        schema: INPUT_SCHEMA.to_owned(),
        version: 1,
        profile: network,
        inbound: ReleaseInboundEvidenceV1::Unavailable(UnavailableDirectionV1 { reason }),
        outbound: ReleaseOutboundEvidenceV1::Unavailable(UnavailableDirectionV1 {
            reason: OUTBOUND_UNAVAILABLE_REASON.to_owned(),
        }),
    };
    let json =
        norito::json::to_json(&evidence).map_err(|_| "fixture cannot be encoded".to_owned())?;
    println!("{json}");
    Ok(())
}

fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let command = args
        .next()
        .and_then(|value| value.into_string().ok())
        .ok_or_else(|| {
            "usage: sccp_release_evidence validate-release <trust-policy-json> <evidence-json> <production|test-fixture> | validate <lane-json> <trust-policy-json> <evidence-json> <production|test-fixture>".to_owned()
        })?;
    match command.as_str() {
        "validate-release" => {
            let policy_path = args
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| "validate-release requires one trust-policy path".to_owned())?;
            let evidence_path = args
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| "validate-release requires one evidence path".to_owned())?;
            let environment = args
                .next()
                .and_then(|value| value.into_string().ok())
                .ok_or_else(|| "validate-release requires one environment".to_owned())?;
            if args.next().is_some() {
                return Err("validate-release accepts exactly three arguments".to_owned());
            }
            let policy_bytes = read_direct_input(&policy_path, MAX_RELEASE_POLICY_BYTES)?;
            let evidence_bytes = read_direct_input(&evidence_path, MAX_RELEASE_EVIDENCE_BYTES)?;
            let receipt =
                validate_release_signatures(&policy_bytes, &evidence_bytes, &environment)?;
            print_release_signature_receipt(&receipt)
        }
        "validate" => {
            let lane_path = args
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| "validate requires one lane-evidence path".to_owned())?;
            let policy_path = args
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| "validate requires one trust-policy path".to_owned())?;
            let evidence_path = args
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| "validate requires one release-evidence path".to_owned())?;
            let environment = args
                .next()
                .and_then(|value| value.into_string().ok())
                .ok_or_else(|| "validate requires one environment".to_owned())?;
            if args.next().is_some() {
                return Err("validate accepts exactly four arguments".to_owned());
            }
            let lane_bytes = read_direct_input(&lane_path, MAX_LANE_INPUT_BYTES)?;
            let policy_bytes = read_direct_input(&policy_path, MAX_RELEASE_POLICY_BYTES)?;
            let evidence_bytes = read_direct_input(&evidence_path, MAX_RELEASE_EVIDENCE_BYTES)?;
            let receipt = validate_lane_in_release_context(
                &lane_bytes,
                &policy_bytes,
                &evidence_bytes,
                &environment,
            )?;
            print_receipt(&receipt)
        }
        #[cfg(feature = "test-fixtures")]
        "emit-ethereum-fixture" => {
            if args.next().is_some() {
                return Err("emit-ethereum-fixture accepts no arguments".to_owned());
            }
            emit_ethereum_fixture()
        }
        #[cfg(feature = "test-fixtures")]
        "emit-unavailable-fixture" => {
            let profile = args
                .next()
                .and_then(|value| value.into_string().ok())
                .ok_or_else(|| "emit-unavailable-fixture requires one profile".to_owned())?;
            if args.next().is_some() {
                return Err("emit-unavailable-fixture accepts exactly one profile".to_owned());
            }
            emit_unavailable_fixture(&profile)
        }
        _ => Err("unknown command; expected validate-release or validate".to_owned()),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln_bounded_stderr(&error);
            ExitCode::FAILURE
        }
    }
}

fn writeln_bounded_stderr(error: &str) -> io::Result<()> {
    use std::io::Write as _;
    let mut bytes = error.as_bytes();
    if bytes.len() > 1024 {
        bytes = &bytes[..1024];
    }
    let stderr = io::stderr();
    let mut lock = stderr.lock();
    lock.write_all(b"SCCP release evidence validation failed: ")?;
    lock.write_all(bytes)?;
    lock.write_all(b"\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validator_identity_is_stable_and_nonzero() {
        let first = validator_identity();
        let second = validator_identity();
        assert_eq!(first.source_sha256_hex, second.source_sha256_hex);
        assert_eq!(first.build_identity_hex, second.build_identity_hex);
        assert_ne!(first.source_sha256_hex, "00".repeat(32));
        assert_ne!(first.build_identity_hex, "00".repeat(32));
    }

    #[test]
    fn unavailable_reason_rejects_aliases_and_controls() {
        assert!(unavailable_reason_is_canonical(
            "sound-proof-is-unavailable"
        ));
        for invalid in [
            "",
            "UPPERCASE",
            "-leading",
            "trailing-",
            "double--separator",
            " leading",
            "trailing ",
            "contains_underscore",
            "contains\ncontrol",
        ] {
            assert!(!unavailable_reason_is_canonical(invalid));
        }
    }

    #[test]
    fn first_release_profiles_exclude_domains_three_and_four() {
        for supported in [
            SccpNetworkV1::EthereumMainnet,
            SccpNetworkV1::BscMainnet,
            SccpNetworkV1::TronMainnet,
        ] {
            assert!(release_profile_supported(supported));
        }
        for unsupported in [
            "solana-mainnet-beta",
            "ton-mainnet",
            "ethereum-sepolia",
            "tron-nile",
        ] {
            if let Some(profile) = SccpNetworkV1::from_profile_key(unsupported) {
                assert!(!release_profile_supported(profile));
            }
        }
    }

    #[test]
    fn runtime_decoder_and_keccak_are_strict() {
        assert_eq!(decode_runtime_code("6000", "runtime").unwrap(), [0x60, 0]);
        assert_eq!(
            lowercase_hex(&keccak256(b"abc")),
            "4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45"
        );
        for invalid in ["", "0", "0X00", "AA", "zz"] {
            assert!(decode_runtime_code(invalid, "runtime").is_err());
        }
    }

    #[test]
    fn fixed_hex_decoder_rejects_zero_uppercase_and_length_drift() {
        assert_eq!(decode_lower_hex::<2>("0102", "value").unwrap(), [1, 2]);
        for invalid in ["0000", "010", "01020", "ABCD", "0x01"] {
            assert!(decode_lower_hex::<2>(invalid, "value").is_err());
        }
    }

    #[test]
    fn release_signature_decoder_and_crypto_reject_noncanonical_material() {
        let key = decode_lower_hex::<32>(
            "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a",
            "key",
        )
        .unwrap();
        let signature = decode_signature_base64(
            "5VZDAMNgrHKQhuLMgG6CioSHfx645dl02HPgZSJJAVVfuIIVkKM7rMYeOXAc+bRr0lv18FlbviRlUUFDjnoQCw==",
        )
        .unwrap();
        verify_ed25519_signature(&key, &signature, b"").unwrap();

        let mut high_s = signature;
        high_s[32..].fill(0xff);
        assert!(verify_ed25519_signature(&key, &high_s, b"").is_err());
        assert!(decode_signature_base64(
            "5VZDAMNgrHKQhuLMgG6CioSHfx645dl02HPgZSJJAVVfuIIVkKM7rMYeOXAc+bRr0lv18FlbviRlUUFDjnoQCx=="
        )
        .is_err());
        assert!(
            validate_ed25519_public_key(
                "0100000000000000000000000000000000000000000000000000000000000000",
                "small-order key"
            )
            .is_err()
        );
    }

    #[test]
    fn sorted_json_parser_rejects_duplicates_and_layout_drift() {
        assert!(parse_canonical_sorted_json(b"{\"a\":1}\n", "json").is_ok());
        assert!(parse_canonical_sorted_json(b"{\"a\":1,\"a\":2}\n", "json").is_err());
        assert!(parse_canonical_sorted_json(b"{ \"a\": 1 }\n", "json").is_err());
        assert!(parse_canonical_sorted_json(b"{\"b\":1,\"a\":2}\n", "json").is_err());
    }

    fn validated_destination() -> ValidatedDestinationStateV1 {
        ValidatedDestinationStateV1 {
            observed_at_unix_ms: 1,
            finality_height: 1,
            finality_block_hash: [1; 32],
            destination_binding_hash: [2; 32],
            route_configuration_hash: [3; 32],
            governed_route_configuration_hash: [4; 32],
            verifier_key_hash: [5; 32],
            route_revision: 1,
            verifying_key_sha256: [13; 32],
            token_runtime_hash: [6; 32],
            verifier_runtime_hash: [7; 32],
            route_runtime_hash: [8; 32],
        }
    }

    fn approved_proof_system() -> ApprovedProofSystemV1 {
        ApprovedProofSystemV1 {
            circuit_id: "nexus-finality-exact-sccp-v1".to_owned(),
            circuit_artifact_sha256: [9; 32],
            verifier_key_hash: [5; 32],
            route_revision: 1,
            verifying_key_sha256: [13; 32],
            prover_build_sha256: [10; 32],
            toolchain_lock_sha256: [11; 32],
            destination_build_policy_sha256: [12; 32],
            token_runtime_hash: [6; 32],
            verifier_runtime_hash: [7; 32],
            route_runtime_hash: [8; 32],
        }
    }

    fn release_evidence_envelope() -> ReleaseEvidenceSignaturesV1 {
        let lanes = RELEASE_PROFILES
            .iter()
            .zip(RELEASE_DOMAINS)
            .map(|(profile, domain)| SignedLaneSummaryV1 {
                counterparty_profile: (*profile).to_owned(),
                counterparty_domain: domain,
                inbound_status: "unavailable".to_owned(),
                outbound_status: "unavailable".to_owned(),
                evidence_artifact_path: format!("artifacts/lanes/{profile}.json"),
            })
            .collect::<Vec<_>>();
        let phases = REQUIRED_PHASES
            .iter()
            .map(|name| ReleaseValidationPhaseV1 {
                name: (*name).to_owned(),
                status: "passed".to_owned(),
                artifact_path: format!("artifacts/phases/{name}.log"),
            })
            .collect::<Vec<_>>();
        let mut artifact_specs = lanes
            .iter()
            .map(|lane| (lane.evidence_artifact_path.clone(), "lane-evidence"))
            .chain(
                phases
                    .iter()
                    .map(|phase| (phase.artifact_path.clone(), "phase-transcript")),
            )
            .collect::<Vec<_>>();
        artifact_specs.sort_unstable_by(|left, right| left.0.cmp(&right.0));
        let artifacts = artifact_specs
            .into_iter()
            .enumerate()
            .map(|(index, (path, kind))| ReleaseArtifactV1 {
                path,
                kind: kind.to_owned(),
                sha256_hex: lowercase_hex(&[u8::try_from(index + 1).unwrap(); 32]),
                size_bytes: 1,
            })
            .collect();
        ReleaseEvidenceSignaturesV1 {
            schema: RELEASE_EVIDENCE_SCHEMA.to_owned(),
            release_id: "release-envelope-test-v1".to_owned(),
            protocol_version: 1,
            hub_profile: "sora-taira".to_owned(),
            hub_chain_id: "809574f5-fee7-5e69-bfcf-52451e42d50f".to_owned(),
            created_at_unix_ms: 1,
            trust_policy_id: "external-policy-v1".to_owned(),
            validator: validator_identity(),
            lanes,
            artifacts,
            validation: ReleaseValidationV1 {
                corridor: "sccp-production-corridor-v1".to_owned(),
                phases,
            },
            provenance: Vec::new(),
        }
    }

    #[test]
    fn approved_proof_system_binds_semantics_vk_and_every_runtime() {
        let validated = validated_destination();
        let approved = approved_proof_system();
        validate_approved_proof_system(&validated, &approved).unwrap();

        for mutation in 0..=7 {
            let mut candidate = approved.clone();
            match mutation {
                0 => candidate.circuit_id = "algebraic-smoke-v1".to_owned(),
                1 => candidate.verifier_key_hash = FORBIDDEN_ALGEBRAIC_SMOKE_VK,
                2 => candidate.verifier_key_hash[0] ^= 1,
                3 => candidate.token_runtime_hash[0] ^= 1,
                4 => candidate.verifier_runtime_hash[0] ^= 1,
                5 => candidate.route_runtime_hash[0] ^= 1,
                6 => candidate.route_revision = 2,
                7 => candidate.verifying_key_sha256[0] ^= 1,
                _ => unreachable!(),
            }
            assert!(validate_approved_proof_system(&validated, &candidate).is_err());
        }
    }

    #[test]
    fn release_envelope_binds_exact_profiles_inventory_and_corridor() {
        let evidence = release_evidence_envelope();
        validate_release_evidence_envelope(&evidence).unwrap();

        for mutation in 0..=6 {
            let mut candidate = release_evidence_envelope();
            match mutation {
                0 => candidate.lanes[0].counterparty_profile = "solana-mainnet-beta".to_owned(),
                1 => candidate.lanes[0].counterparty_domain = 3,
                2 => candidate.lanes[0].evidence_artifact_path = "../escape.json".to_owned(),
                3 => candidate.validation.phases[0].status = "skipped".to_owned(),
                4 => candidate.artifacts[1].sha256_hex = candidate.artifacts[0].sha256_hex.clone(),
                5 => candidate.artifacts[0].size_bytes = 0,
                6 => candidate.artifacts.swap(0, 1),
                _ => unreachable!(),
            }
            assert!(validate_release_evidence_envelope(&candidate).is_err());
        }
    }

    #[test]
    fn relative_release_paths_are_a_narrow_ascii_subset() {
        assert!(canonical_relative_path(
            "artifacts/lanes/ethereum-mainnet.json"
        ));
        for invalid in [
            "",
            "/absolute",
            "../escape",
            "artifacts//lane",
            "artifacts/./lane",
            "artifacts/../lane",
            "artifacts\\lane",
            "artifacts/UPPERCASE",
            "artifacts/ lane",
        ] {
            assert!(!canonical_relative_path(invalid), "accepted {invalid:?}");
        }
    }
}
