//! Fail-closed authentication for the sole KAGEMUSHA V1 proof release.

use crate::{Outcome, RunArgs, json_macros::JsonDeserialize};
use clap::{Args as ClapArgs, Subcommand};
#[cfg(unix)]
use color_eyre::eyre::WrapErr as _;
use color_eyre::eyre::{bail, eyre};
use iroha_core::{
    smartcontracts::isi::kagemusha::{
        KAGEMUSHA_RECURSIVE_PROFILE_MAX_BYTES_V1, load_authenticated_kagemusha_v1_runtime_verifier,
    },
    zk::kagemusha_v1_recursion::{
        KagemushaArtifactByteResolverV1, KagemushaDirectoryArtifactResolverV1,
    },
};
use iroha_crypto::{sha256, sha256_reader_bounded};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1, KAGEMUSHA_RELEASE_ATTESTATION_MAX_BYTES_V1,
    KAGEMUSHA_RELEASE_AUTHORITY_POLICY_MAX_BYTES_V1, KAGEMUSHA_RELEASE_MANIFEST_MAX_BYTES_V1,
    KagemushaArtifactBindingV1, KagemushaArtifactRoleV1, KagemushaAuthenticatedReleaseV1,
    KagemushaInternalValidationReceiptV1, KagemushaReleaseAttestationV1,
    KagemushaReleaseAuthorityPolicyV1, KagemushaReleaseManifestV1,
};
use norito::json::{Map as JsonMap, Value as JsonValue};
use std::{
    fmt::Write as _,
    io::{Read as _, Write},
    path::{Path, PathBuf},
};
#[cfg(unix)]
use std::{fs, fs::OpenOptions};

const KAGEMUSHA_RELEASE_ARTIFACT_ROLE_COUNT_V1: usize = 50;
const _: [(); KAGEMUSHA_RELEASE_ARTIFACT_ROLE_COUNT_V1] = [(); KagemushaArtifactRoleV1::ALL.len()];
const AUTHORITY_REVIEW_PROJECTION_MAX_BYTES_V1: usize = 128 * 1024 * 1024;
const AUTHORITY_REVIEW_PROJECTION_SCHEMA_V1: &str =
    "iroha.kagemusha_v1.authority_review_projection";
const AUTHORITY_REVIEW_VERIFICATION_SCOPE_V1: &str = "closed filesystem provenance, exact Rust-compatible release identities, derived measurements, and threshold-signed observations from a separately pinned trusted verifier policy; candidate code is never executed by this verifier";
const CANDIDATE_CONTEXT_DIGEST_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:release-candidate-context";
const VERIFICATION_RECORDS_DIGEST_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:verification-records";
const NATIVE_ARTIFACT_MANIFEST_MAX_BYTES_V1: usize = 64 * 1024;
const NATIVE_ARTIFACT_SCHEMA_V1: &str = "iroha.native-sdk-abi23-artifact.v1";
const AUTHENTICATED_RELEASE_REPORT_SCHEMA_V1: &str =
    "iroha.kagemusha.v1.authenticated-release-report";
const REQUIRED_PRIVACY_C_EXPORTS_V1: [&str; 5] = [
    "iroha_privacy_compiled_profile_catalog_v1",
    "iroha_privacy_validate_compiled_profile_catalog_v1",
    "iroha_privacy_exact12_fixture_bundle_v1",
    "iroha_privacy_validate_exact12_fixture_bundle_v1",
    "iroha_privacy_free_buffer",
];
const REQUIRED_C_JNI_SYMBOLS_V1: [&str; 48] = [
    "connect_norito_bridge_abi_version",
    "connect_norito_free",
    "connect_norito_kagemusha_v1_payment_request_validate",
    "connect_norito_kagemusha_v1_payment_validate",
    "connect_norito_kagemusha_v1_acknowledgement_validate",
    "connect_norito_kagemusha_v1_complete_exchange_validate",
    "connect_norito_kagemusha_v1_mint_authorization_validate",
    "connect_norito_kagemusha_v1_mint_credit_validate",
    "connect_norito_kagemusha_v1_mint_credit_against_authorization_validate",
    "connect_norito_kagemusha_v1_redemption_voucher_validate",
    "connect_norito_kagemusha_v1_payment_request_text_validate",
    "connect_norito_kagemusha_v1_payment_text_validate",
    "connect_norito_kagemusha_v1_acknowledgement_text_validate",
    "connect_norito_kagemusha_v1_complete_exchange_text_validate",
    "connect_norito_kagemusha_v1_mint_authorization_text_validate",
    "connect_norito_kagemusha_v1_mint_credit_text_validate",
    "connect_norito_kagemusha_v1_mint_credit_against_authorization_text_validate",
    "connect_norito_kagemusha_v1_redemption_voucher_text_validate",
    "connect_norito_kagemusha_device_mint_stage_command_v1_validate",
    "connect_norito_kagemusha_device_mint_stage_result_v1_validate",
    "connect_norito_kagemusha_contract_vector_v1",
    "connect_norito_kagemusha_core_coordinator_contract_v1",
    "connect_norito_kagemusha_core_coordinator_open_v1",
    "connect_norito_kagemusha_core_coordinator_invoke_v1",
    "connect_norito_kagemusha_device_capabilities_v1",
    "connect_norito_kagemusha_device_execute_v1",
    "connect_norito_kagemusha_device_response_authenticator_v1_verify",
    "connect_norito_validation_fee_hijiri_quote_request_v1",
    "connect_norito_validation_fee_hijiri_quote_response_verify_v1",
    "connect_norito_private_settlement_committee_proof_response_verify_v1",
    "connect_norito_private_settlement_auditor_capsule_response_verify_with_request_v1",
    "connect_norito_private_settlement_audit_approval_response_verify_v1",
    "Java_org_hyperledger_iroha_sdk_client_AtomicPrivateSettlementNativeResponseVerifierV1_nativeBridgeAbiVersion",
    "Java_org_hyperledger_iroha_sdk_client_AtomicPrivateSettlementNativeResponseVerifierV1_nativeVerifyCommitteeProofResponseV1",
    "Java_org_hyperledger_iroha_sdk_client_AtomicPrivateSettlementNativeResponseVerifierV1_nativeVerifyAuditorCapsuleResponseWithRequestV1",
    "Java_org_hyperledger_iroha_sdk_client_AtomicPrivateSettlementNativeResponseVerifierV1_nativeVerifyAuditApprovalResponseV1",
    "Java_org_hyperledger_iroha_android_client_AtomicPrivateSettlementNativeResponseVerifierV1_nativeBridgeAbiVersion",
    "Java_org_hyperledger_iroha_android_client_AtomicPrivateSettlementNativeResponseVerifierV1_nativeVerifyCommitteeProofResponseV1",
    "Java_org_hyperledger_iroha_android_client_AtomicPrivateSettlementNativeResponseVerifierV1_nativeVerifyAuditorCapsuleResponseWithRequestV1",
    "Java_org_hyperledger_iroha_android_client_AtomicPrivateSettlementNativeResponseVerifierV1_nativeVerifyAuditApprovalResponseV1",
    "Java_org_hyperledger_iroha_sdk_offline_KagemushaDeviceLifecycleBridgeV1_00024NativeEndpoint_nativeCapabilitiesV1",
    "Java_org_hyperledger_iroha_sdk_offline_KagemushaDeviceLifecycleBridgeV1_00024NativeEndpoint_nativeContractVectorV1",
    "Java_org_hyperledger_iroha_sdk_offline_KagemushaDeviceLifecycleBridgeV1_00024NativeEndpoint_nativeExecuteV1",
    "Java_org_hyperledger_iroha_sdk_offline_KagemushaDeviceLifecycleBridgeV1_00024NativeEndpoint_nativeVerifyResponseAuthenticatorV1",
    "Java_pg_bpng_digitalkina_KagemushaNativeCoreJniV1_nativeContractV1",
    "Java_pg_bpng_digitalkina_KagemushaNativeCoreJniV1_nativeOpenV1",
    "Java_pg_bpng_digitalkina_KagemushaNativeCoreJniV1_nativeInvokeV1",
    "connect_norito_sorafs_reference_validate_appeal_finance_cancel_asset_lock_json",
];

/// Authenticate the sole first-release KAGEMUSHA release format.
#[derive(Debug, ClapArgs)]
pub struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Authenticate one complete KAGEMUSHA V1 release and its deployment evidence.
    #[command(name = "authenticate-release-v1")]
    AuthenticateReleaseV1(AuthenticateReleaseV1Args),
}

#[derive(Debug, ClapArgs)]
struct AuthenticateReleaseV1Args {
    /// Canonical Norito KAGEMUSHA V1 release manifest.
    #[arg(long, value_name = "PATH")]
    manifest: PathBuf,
    /// Canonical Norito KAGEMUSHA V1 internal-validation receipt.
    #[arg(long, value_name = "PATH")]
    validation_receipt: PathBuf,
    /// Canonical Norito locally trusted KAGEMUSHA V1 release-authority policy.
    #[arg(long, value_name = "PATH")]
    authority_policy: PathBuf,
    /// Canonical Norito KAGEMUSHA V1 threshold attestation.
    #[arg(long, value_name = "PATH")]
    attestation: PathBuf,
    /// Canonical JSON recursive-verifier profile consumed by Core.
    #[arg(long, value_name = "PATH")]
    recursive_profile: PathBuf,
    /// Absolute directory containing all 50 SHA-256-addressed release artifacts.
    #[arg(long, value_name = "PATH")]
    artifact_root: PathBuf,
    /// Canonical output from the separately pinned authority-review verifier.
    #[arg(long, value_name = "PATH")]
    authority_review_projection: PathBuf,
    /// SHA-256 pin for the exact authority-review projection bytes.
    #[arg(long, value_name = "LOWER_HEX")]
    authority_review_projection_sha256: String,
    /// Canonical ABI23 c-jni native-artifact evidence manifest.
    #[arg(long, value_name = "PATH")]
    native_artifact_manifest: PathBuf,
    /// SHA-256 pin for the exact native-artifact manifest bytes.
    #[arg(long, value_name = "LOWER_HEX")]
    native_artifact_manifest_sha256: String,
    /// Exact c-jni library whose bytes must match the native-artifact manifest.
    #[arg(long, value_name = "PATH")]
    native_artifact: PathBuf,
}

impl<T: Write> RunArgs<T> for Args {
    fn run(self, writer: &mut std::io::BufWriter<T>) -> Outcome {
        match self.command {
            Command::AuthenticateReleaseV1(args) => authenticate_release_v1(&args, writer),
        }
    }
}

struct AuthenticatedReleaseInputsV1 {
    manifest: KagemushaReleaseManifestV1,
    receipt: KagemushaInternalValidationReceiptV1,
    policy: KagemushaReleaseAuthorityPolicyV1,
    authenticated: KagemushaAuthenticatedReleaseV1,
}

#[derive(Debug, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct NativeArtifactManifestV1 {
    artifact_sha256: String,
    artifact_size: u64,
    bridge_abi_version: u32,
    privacy_c_exports: Vec<String>,
    privacy_c_exports_inspected: bool,
    required_symbols: Vec<String>,
    schema: String,
    sdk: String,
    source_commit: String,
    source_tree_clean: bool,
    target: String,
    workspace_source_manifest_sha256: String,
}

fn authenticate_release_v1<T: Write>(
    args: &AuthenticateReleaseV1Args,
    writer: &mut std::io::BufWriter<T>,
) -> Outcome {
    let manifest_bytes = read_bounded_immutable_file(
        &args.manifest,
        KAGEMUSHA_RELEASE_MANIFEST_MAX_BYTES_V1,
        "KAGEMUSHA V1 release manifest",
    )?;
    let receipt_bytes = read_bounded_immutable_file(
        &args.validation_receipt,
        KAGEMUSHA_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1,
        "KAGEMUSHA V1 validation receipt",
    )?;
    let policy_bytes = read_bounded_immutable_file(
        &args.authority_policy,
        KAGEMUSHA_RELEASE_AUTHORITY_POLICY_MAX_BYTES_V1,
        "KAGEMUSHA V1 release-authority policy",
    )?;
    let attestation_bytes = read_bounded_immutable_file(
        &args.attestation,
        KAGEMUSHA_RELEASE_ATTESTATION_MAX_BYTES_V1,
        "KAGEMUSHA V1 release attestation",
    )?;
    let inputs = decode_authenticated_release_inputs_v1(
        &manifest_bytes,
        &receipt_bytes,
        &policy_bytes,
        &attestation_bytes,
    )?;
    validate_exact_release_inventory_v1(&inputs.manifest.artifacts)?;

    let recursive_profile_bytes = read_bounded_immutable_file(
        &args.recursive_profile,
        KAGEMUSHA_RECURSIVE_PROFILE_MAX_BYTES_V1,
        "KAGEMUSHA V1 recursive verifier profile",
    )?;
    let authority_projection_bytes = read_bounded_immutable_file(
        &args.authority_review_projection,
        AUTHORITY_REVIEW_PROJECTION_MAX_BYTES_V1,
        "KAGEMUSHA V1 authority-review projection",
    )?;
    let authority_projection_sha256 = parse_lower_sha256(
        &args.authority_review_projection_sha256,
        "KAGEMUSHA V1 authority-review projection SHA-256",
    )?;
    if sha256(&authority_projection_bytes) != authority_projection_sha256 {
        bail!("KAGEMUSHA V1 authority-review projection does not match its SHA-256 pin");
    }
    validate_authority_review_projection_v1(
        &authority_projection_bytes,
        &inputs.manifest,
        &inputs.receipt,
    )?;

    let artifact_root = canonical_artifact_root(&args.artifact_root)?;
    rehash_all_release_artifacts_v1(&inputs.manifest.artifacts, &artifact_root)?;
    let _runtime = load_authenticated_kagemusha_v1_runtime_verifier(
        &manifest_bytes,
        &receipt_bytes,
        &policy_bytes,
        &attestation_bytes,
        &recursive_profile_bytes,
        &artifact_root,
    )
    .map_err(|source| eyre!("failed to load authenticated KAGEMUSHA V1 runtime: {source}"))?;

    let native_manifest_bytes = read_bounded_immutable_file(
        &args.native_artifact_manifest,
        NATIVE_ARTIFACT_MANIFEST_MAX_BYTES_V1,
        "ABI23 c-jni native-artifact manifest",
    )?;
    let native_manifest_sha256 = parse_lower_sha256(
        &args.native_artifact_manifest_sha256,
        "ABI23 c-jni native-artifact manifest SHA-256",
    )?;
    if sha256(&native_manifest_bytes) != native_manifest_sha256 {
        bail!("ABI23 c-jni native-artifact manifest does not match its SHA-256 pin");
    }
    let native_manifest = validate_native_artifact_manifest_v1(&native_manifest_bytes)?;
    let native_artifact_sha256 = parse_lower_sha256(
        &native_manifest.artifact_sha256,
        "ABI23 c-jni native artifact SHA-256",
    )?;
    authenticate_native_artifact_bytes_v1(
        &args.native_artifact,
        native_manifest.artifact_size,
        native_artifact_sha256,
    )?;

    let report = authenticated_release_report_v1(
        &inputs,
        sha256(&recursive_profile_bytes),
        authority_projection_sha256,
        native_manifest_sha256,
        &native_manifest,
    )?;
    write!(writer, "{}", norito::json::to_json(&report)?)?;
    Ok(())
}

fn decode_authenticated_release_inputs_v1(
    manifest_bytes: &[u8],
    receipt_bytes: &[u8],
    policy_bytes: &[u8],
    attestation_bytes: &[u8],
) -> color_eyre::Result<AuthenticatedReleaseInputsV1> {
    let manifest = KagemushaReleaseManifestV1::decode_canonical_exact(manifest_bytes)
        .map_err(|source| eyre!("invalid canonical KAGEMUSHA V1 release manifest: {source}"))?;
    let receipt = KagemushaInternalValidationReceiptV1::decode_canonical_exact(receipt_bytes)
        .map_err(|source| eyre!("invalid canonical KAGEMUSHA V1 validation receipt: {source}"))?;
    let policy = KagemushaReleaseAuthorityPolicyV1::decode_canonical_exact(policy_bytes).map_err(
        |source| eyre!("invalid canonical KAGEMUSHA V1 release-authority policy: {source}"),
    )?;
    let attestation = KagemushaReleaseAttestationV1::decode_canonical_exact(attestation_bytes)
        .map_err(|source| eyre!("invalid canonical KAGEMUSHA V1 release attestation: {source}"))?;
    let authenticated = manifest
        .authenticate(&receipt, &policy, &attestation)
        .map_err(|source| eyre!("KAGEMUSHA V1 release authentication failed: {source}"))?;

    Ok(AuthenticatedReleaseInputsV1 {
        manifest,
        receipt,
        policy,
        authenticated,
    })
}

fn validate_exact_release_inventory_v1(
    artifacts: &[KagemushaArtifactBindingV1],
) -> color_eyre::Result<()> {
    if artifacts.len() != KAGEMUSHA_RELEASE_ARTIFACT_ROLE_COUNT_V1
        || artifacts
            .iter()
            .zip(KagemushaArtifactRoleV1::ALL)
            .any(|(binding, expected)| binding.role != expected)
    {
        bail!("KAGEMUSHA V1 release requires the exact ordered 50-role artifact inventory");
    }
    for (index, binding) in artifacts.iter().enumerate() {
        if binding.sha256 == [0; 32]
            || binding.byte_len == 0
            || artifacts[..index]
                .iter()
                .any(|prior| prior.sha256 == binding.sha256)
        {
            bail!(
                "KAGEMUSHA V1 release artifact inventory contains an invalid or duplicate binding"
            );
        }
    }
    Ok(())
}

fn parse_lower_sha256(value: &str, description: &str) -> color_eyre::Result<[u8; 32]> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        bail!("{description} must be exactly 64 lowercase hexadecimal characters");
    }
    let mut digest = [0_u8; 32];
    hex::decode_to_slice(value, &mut digest)
        .map_err(|source| eyre!("invalid {description}: {source}"))?;
    Ok(digest)
}

#[cfg(unix)]
fn canonical_artifact_root(path: &Path) -> color_eyre::Result<PathBuf> {
    if !path.is_absolute() {
        bail!("KAGEMUSHA V1 artifact root must be absolute");
    }
    let metadata = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("failed to inspect artifact root `{}`", path.display()))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        bail!("KAGEMUSHA V1 artifact root must be one non-symlink directory");
    }
    let canonical = fs::canonicalize(path)
        .wrap_err_with(|| format!("failed to canonicalize artifact root `{}`", path.display()))?;
    if canonical != path {
        bail!("KAGEMUSHA V1 artifact root must already be its canonical absolute path");
    }
    Ok(canonical)
}

#[cfg(not(unix))]
fn canonical_artifact_root(path: &Path) -> color_eyre::Result<PathBuf> {
    let _ = path;
    bail!(
        "KAGEMUSHA V1 artifact authentication is unavailable on this platform because immutable no-follow file identity cannot be established"
    )
}

fn rehash_all_release_artifacts_v1(
    artifacts: &[KagemushaArtifactBindingV1],
    artifact_root: &Path,
) -> color_eyre::Result<()> {
    validate_exact_release_inventory_v1(artifacts)?;
    let resolver = KagemushaDirectoryArtifactResolverV1::new(artifact_root)
        .wrap_err("failed to open KAGEMUSHA V1 content-addressed artifact root")?;
    for binding in artifacts {
        rehash_release_artifact_v1(&resolver, *binding)?;
    }
    Ok(())
}

fn rehash_release_artifact_v1(
    resolver: &KagemushaDirectoryArtifactResolverV1,
    binding: KagemushaArtifactBindingV1,
) -> color_eyre::Result<()> {
    let reader = resolver.open_reader(binding).map_err(|source| {
        eyre!(
            "failed to resolve KAGEMUSHA V1 artifact {:?}: {source}",
            binding.role
        )
    })?;
    let (digest, byte_len) = sha256_reader_bounded(reader, binding.byte_len).map_err(|source| {
        eyre!(
            "failed to rehash KAGEMUSHA V1 artifact {:?}: {source}",
            binding.role
        )
    })?;
    if byte_len != binding.byte_len || digest != binding.sha256 {
        bail!(
            "KAGEMUSHA V1 artifact {:?} does not match its authenticated content address",
            binding.role
        );
    }
    Ok(())
}

fn validate_authority_review_projection_v1(
    bytes: &[u8],
    manifest: &KagemushaReleaseManifestV1,
    receipt: &KagemushaInternalValidationReceiptV1,
) -> color_eyre::Result<()> {
    let projection = decode_authority_review_projection_json_v1(bytes)?;
    let root = exact_json_object(
        &projection,
        "KAGEMUSHA V1 authority-review projection",
        &[
            "artifact_inventory",
            "artifact_inventory_review_sha256",
            "candidate_context",
            "manifest_sha256",
            "receipt_projection",
            "schema",
            "schema_version",
            "verification_scope",
            "verifier_commands",
        ],
    )?;
    if json_string(root, "schema")? != AUTHORITY_REVIEW_PROJECTION_SCHEMA_V1
        || json_u64(root, "schema_version")? != 1
        || json_string(root, "verification_scope")? != AUTHORITY_REVIEW_VERIFICATION_SCOPE_V1
    {
        bail!("KAGEMUSHA V1 authority-review projection contract is unsupported");
    }

    let expected_receipt = normalize_release_projection_value(norito::json::to_value(receipt)?)?;
    if root.get("receipt_projection") != Some(&expected_receipt) {
        bail!("KAGEMUSHA V1 authority-review receipt projection differs from the supplied receipt");
    }
    let expected_inventory =
        normalize_release_projection_value(norito::json::to_value(&manifest.artifacts)?)?;
    let inventory = root
        .get("artifact_inventory")
        .ok_or_else(|| eyre!("authority-review projection lacks artifact inventory"))?;
    if inventory != &expected_inventory {
        bail!("KAGEMUSHA V1 authority-review artifact inventory differs from the manifest");
    }
    let inventory_digest = sha256(python_canonical_json_bytes(inventory, true)?);
    if parse_lower_sha256(
        json_string(root, "artifact_inventory_review_sha256")?,
        "KAGEMUSHA V1 authority-review artifact-inventory digest",
    )? != inventory_digest
    {
        bail!("KAGEMUSHA V1 authority-review artifact-inventory digest is stale");
    }
    if parse_lower_sha256(
        json_string(root, "manifest_sha256")?,
        "KAGEMUSHA V1 evidence-manifest SHA-256",
    )? != receipt.evidence_closure.evidence_manifest.sha256
    {
        bail!("KAGEMUSHA V1 authority-review projection names a different evidence manifest");
    }

    let candidate_context = root
        .get("candidate_context")
        .ok_or_else(|| eyre!("authority-review projection lacks candidate context"))?;
    if raw_python_projection_digest(CANDIDATE_CONTEXT_DIGEST_DOMAIN_V1, candidate_context)?
        != receipt.evidence_closure.candidate_context_digest
    {
        bail!("KAGEMUSHA V1 authority-review candidate-context digest is stale");
    }
    let commands = root
        .get("verifier_commands")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| eyre!("authority-review verifier commands must be an array"))?;
    if commands.len()
        != usize::try_from(receipt.evidence_closure.verification_record_count)
            .map_err(|_| eyre!("verification-record count does not fit usize"))?
        || commands.is_empty()
    {
        bail!("KAGEMUSHA V1 authority-review verifier-command count differs from the receipt");
    }
    let command_value = JsonValue::Array(commands.clone());
    if raw_python_projection_digest(VERIFICATION_RECORDS_DIGEST_DOMAIN_V1, &command_value)?
        != receipt.evidence_closure.verification_records_digest
    {
        bail!("KAGEMUSHA V1 authority-review verification-record digest is stale");
    }
    Ok(())
}

fn decode_authority_review_projection_json_v1(bytes: &[u8]) -> color_eyre::Result<JsonValue> {
    let projection = norito::json::from_slice_value(bytes).map_err(|source| {
        eyre!("invalid KAGEMUSHA V1 authority-review projection JSON: {source}")
    })?;
    if python_canonical_json_bytes(&projection, true)? != bytes {
        bail!("KAGEMUSHA V1 authority-review projection JSON is not canonical");
    }
    Ok(projection)
}

fn exact_json_object<'a>(
    value: &'a JsonValue,
    description: &str,
    fields: &[&str],
) -> color_eyre::Result<&'a JsonMap> {
    let object = value
        .as_object()
        .ok_or_else(|| eyre!("{description} must be a JSON object"))?;
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        bail!("{description} field inventory is not exact");
    }
    Ok(object)
}

fn json_string<'a>(object: &'a JsonMap, field: &str) -> color_eyre::Result<&'a str> {
    object
        .get(field)
        .and_then(JsonValue::as_str)
        .ok_or_else(|| eyre!("JSON field `{field}` must be a string"))
}

fn json_u64(object: &JsonMap, field: &str) -> color_eyre::Result<u64> {
    object
        .get(field)
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| eyre!("JSON field `{field}` must be an unsigned integer"))
}

fn raw_python_projection_digest(domain: &[u8], value: &JsonValue) -> color_eyre::Result<[u8; 32]> {
    let payload = python_canonical_json_bytes(value, true)?;
    let payload_len = u64::try_from(payload.len())
        .map_err(|_| eyre!("canonical authority-review projection length does not fit u64"))?;
    let mut prefix = Vec::with_capacity(domain.len() + 9);
    prefix.extend_from_slice(domain);
    prefix.push(0);
    prefix.extend_from_slice(&payload_len.to_le_bytes());
    let limit = u64::try_from(prefix.len())
        .ok()
        .and_then(|length| length.checked_add(payload_len))
        .ok_or_else(|| eyre!("authority-review digest input length overflow"))?;
    let reader = std::io::Cursor::new(prefix).chain(std::io::Cursor::new(payload));
    sha256_reader_bounded(reader, limit)
        .map(|(digest, _)| digest)
        .wrap_err("failed to hash authority-review projection")
}

fn normalize_release_projection_value(value: JsonValue) -> color_eyre::Result<JsonValue> {
    match value {
        JsonValue::Array(values) => values
            .into_iter()
            .map(normalize_release_projection_value)
            .collect::<color_eyre::Result<Vec<_>>>()
            .map(JsonValue::Array),
        JsonValue::Object(values) => {
            let mut normalized = JsonMap::new();
            for (field, value) in values {
                let value = if is_release_digest_field(&field) {
                    JsonValue::String(fixed_byte_array_to_hex(value, 32, &field)?)
                } else if field == "governance_credential_public_key" {
                    let mut tuple = value
                        .as_array()
                        .ok_or_else(|| eyre!("governance credential key must be a JSON tuple"))?
                        .clone();
                    if tuple.len() != 1 {
                        bail!("governance credential key JSON tuple is malformed");
                    }
                    JsonValue::String(fixed_byte_array_to_hex(tuple.remove(0), 65, &field)?)
                } else if let Some(tag) = release_unit_enum_tag(&field) {
                    JsonValue::String(tagged_unit_enum_name(value, tag, &field)?)
                } else {
                    normalize_release_projection_value(value)?
                };
                normalized.insert(field, value);
            }
            Ok(JsonValue::Object(normalized))
        }
        value => Ok(value),
    }
}

fn is_release_digest_field(field: &str) -> bool {
    matches!(
        field,
        "sha256"
            | "source_tree_digest"
            | "cargo_lock_digest"
            | "profile_digest"
            | "eq_protocol_digest"
            | "ep_protocol_digest"
            | "artifact_set_digest"
            | "hardware_policy_digest"
            | "verification_records_digest"
            | "candidate_context_digest"
            | "hardware_profile_id"
            | "suite_id"
            | "vk_digest"
            | "qualification_digest"
            | "builder_id"
            | "provider_id"
            | "product_class_digest"
            | "firmware_policy_digest"
            | "enrollment_attestation_verifier_digest"
            | "attestation_trust_roots_digest"
            | "allowed_suite_commitment"
            | "qualification_report_digest"
    )
}

fn release_unit_enum_tag(field: &str) -> Option<&'static str> {
    match field {
        "role" => Some("role"),
        "relation" => Some("relation"),
        "helper" => Some("helper"),
        "platform_class" => Some("class"),
        "case" => Some("case"),
        _ => None,
    }
}

fn tagged_unit_enum_name(
    value: JsonValue,
    tag: &str,
    description: &str,
) -> color_eyre::Result<String> {
    let object = value
        .as_object()
        .ok_or_else(|| eyre!("{description} must be a tagged unit enum"))?;
    if object.len() != 2 || !object.get("value").is_some_and(JsonValue::is_null) {
        bail!("{description} tagged unit enum is malformed");
    }
    object
        .get(tag)
        .and_then(JsonValue::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| eyre!("{description} tagged unit enum lacks tag `{tag}`"))
}

fn fixed_byte_array_to_hex(
    value: JsonValue,
    expected_len: usize,
    description: &str,
) -> color_eyre::Result<String> {
    let values = value
        .as_array()
        .ok_or_else(|| eyre!("{description} must be a fixed byte array"))?;
    if values.len() != expected_len {
        bail!("{description} fixed byte array has the wrong length");
    }
    let bytes = values
        .iter()
        .map(|value| {
            value
                .as_u64()
                .and_then(|byte| u8::try_from(byte).ok())
                .ok_or_else(|| eyre!("{description} contains a non-byte value"))
        })
        .collect::<color_eyre::Result<Vec<_>>>()?;
    Ok(hex::encode(bytes))
}

fn python_canonical_json_bytes(value: &JsonValue, pretty: bool) -> color_eyre::Result<Vec<u8>> {
    let mut output = String::new();
    write_python_canonical_json(value, pretty, 0, &mut output)?;
    output.push('\n');
    Ok(output.into_bytes())
}

fn write_python_canonical_json(
    value: &JsonValue,
    pretty: bool,
    depth: usize,
    output: &mut String,
) -> color_eyre::Result<()> {
    match value {
        JsonValue::Null => output.push_str("null"),
        JsonValue::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
        JsonValue::Number(_) => {
            if let Some(value) = value.as_u128() {
                write!(output, "{value}").expect("writing to String cannot fail");
            } else if let Some(value) = value.as_i64() {
                write!(output, "{value}").expect("writing to String cannot fail");
            } else {
                bail!("canonical authority-review projection must contain only integers");
            }
        }
        JsonValue::String(value) => write_python_json_string(value, output),
        JsonValue::Array(values) => {
            output.push('[');
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.push(',');
                }
                if pretty {
                    output.push('\n');
                    output.push_str(&"  ".repeat(depth + 1));
                }
                write_python_canonical_json(value, pretty, depth + 1, output)?;
            }
            if pretty && !values.is_empty() {
                output.push('\n');
                output.push_str(&"  ".repeat(depth));
            }
            output.push(']');
        }
        JsonValue::Object(values) => {
            output.push('{');
            for (index, (field, value)) in values.iter().enumerate() {
                if index != 0 {
                    output.push(',');
                }
                if pretty {
                    output.push('\n');
                    output.push_str(&"  ".repeat(depth + 1));
                }
                write_python_json_string(field, output);
                output.push(':');
                if pretty {
                    output.push(' ');
                }
                write_python_canonical_json(value, pretty, depth + 1, output)?;
            }
            if pretty && !values.is_empty() {
                output.push('\n');
                output.push_str(&"  ".repeat(depth));
            }
            output.push('}');
        }
    }
    Ok(())
}

fn write_python_json_string(value: &str, output: &mut String) {
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\u{08}' => output.push_str("\\b"),
            '\u{0c}' => output.push_str("\\f"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character <= '\u{1f}' => {
                write!(output, "\\u{:04x}", u32::from(character))
                    .expect("writing to String cannot fail");
            }
            character if character.is_ascii() => output.push(character),
            character if u32::from(character) <= 0xffff => {
                write!(output, "\\u{:04x}", u32::from(character))
                    .expect("writing to String cannot fail");
            }
            character => {
                let scalar = u32::from(character) - 0x1_0000;
                let high = 0xd800 + (scalar >> 10);
                let low = 0xdc00 + (scalar & 0x3ff);
                write!(output, "\\u{high:04x}\\u{low:04x}").expect("writing to String cannot fail");
            }
        }
    }
    output.push('"');
}

fn validate_native_artifact_manifest_v1(
    bytes: &[u8],
) -> color_eyre::Result<NativeArtifactManifestV1> {
    let value = norito::json::from_slice_value(bytes)
        .map_err(|source| eyre!("invalid ABI23 c-jni native-artifact manifest JSON: {source}"))?;
    if python_canonical_json_bytes(&value, false)? != bytes {
        bail!("ABI23 c-jni native-artifact manifest JSON is not canonical");
    }
    let manifest: NativeArtifactManifestV1 = norito::json::from_slice(bytes)
        .map_err(|source| eyre!("invalid ABI23 c-jni native-artifact manifest: {source}"))?;
    if manifest.schema != NATIVE_ARTIFACT_SCHEMA_V1
        || manifest.sdk != "c-jni"
        || manifest.bridge_abi_version != 23
        || !manifest.source_tree_clean
        || !manifest.privacy_c_exports_inspected
        || manifest.artifact_size == 0
        || !valid_native_target(&manifest.target)
        || !is_lower_hex(&manifest.source_commit, 40)
    {
        bail!("ABI23 c-jni native-artifact manifest identity is unsupported");
    }
    parse_lower_sha256(
        &manifest.artifact_sha256,
        "ABI23 c-jni native artifact SHA-256",
    )?;
    let source_manifest_sha256 = parse_lower_sha256(
        &manifest.workspace_source_manifest_sha256,
        "ABI23 c-jni workspace source-manifest SHA-256",
    )?;
    if source_manifest_sha256 == [0; 32]
        || !manifest
            .required_symbols
            .iter()
            .map(String::as_str)
            .eq(REQUIRED_C_JNI_SYMBOLS_V1)
        || !manifest
            .privacy_c_exports
            .iter()
            .map(String::as_str)
            .eq(REQUIRED_PRIVACY_C_EXPORTS_V1)
    {
        bail!("ABI23 c-jni native-artifact manifest inventory is not exact");
    }
    Ok(manifest)
}

fn valid_native_target(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 128
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'.' | b'_' | b'+' | b'-')
        })
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn authenticate_native_artifact_bytes_v1(
    path: &Path,
    expected_len: u64,
    expected_sha256: [u8; 32],
) -> color_eyre::Result<()> {
    let observed = hash_immutable_file_exact(path, expected_len, "ABI23 c-jni native artifact")?;
    if observed != expected_sha256 {
        bail!("ABI23 c-jni native artifact bytes do not match the evidence manifest");
    }
    Ok(())
}

#[cfg(unix)]
fn hash_immutable_file_exact(
    path: &Path,
    expected_len: u64,
    description: &str,
) -> color_eyre::Result<[u8; 32]> {
    if !path.is_absolute() {
        bail!("{description} path must be absolute");
    }
    let canonical = fs::canonicalize(path).wrap_err_with(|| {
        format!(
            "failed to canonicalize {description} at `{}`",
            path.display()
        )
    })?;
    if canonical != path {
        bail!("{description} path must already be canonical and contain no symlink component");
    }
    let before = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("failed to inspect {description} at `{}`", path.display()))?;
    if !before.is_file()
        || before.file_type().is_symlink()
        || std::os::unix::fs::MetadataExt::nlink(&before) != 1
        || before.len() != expected_len
        || expected_len == 0
    {
        bail!("{description} does not have the exact immutable manifest identity");
    }
    let mut options = OpenOptions::new();
    options.read(true);
    use std::os::unix::fs::OpenOptionsExt as _;
    options.custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK);
    let file = options
        .open(path)
        .wrap_err_with(|| format!("failed to open {description} at `{}`", path.display()))?;
    let opened = file
        .metadata()
        .wrap_err_with(|| format!("failed to inspect opened {description}"))?;
    if !same_input_metadata(&before, &opened) {
        bail!("{description} changed before its immutable hash snapshot was opened");
    }
    let (digest, observed_len) = sha256_reader_bounded(&file, expected_len)
        .wrap_err_with(|| format!("failed to hash {description}"))?;
    let after = file
        .metadata()
        .wrap_err_with(|| format!("failed to re-inspect opened {description}"))?;
    if observed_len != expected_len || !same_input_metadata(&opened, &after) {
        bail!("{description} changed while its immutable hash snapshot was read");
    }
    Ok(digest)
}

#[cfg(not(unix))]
fn hash_immutable_file_exact(
    path: &Path,
    expected_len: u64,
    description: &str,
) -> color_eyre::Result<[u8; 32]> {
    let _ = (path, expected_len);
    bail!(
        "{description} authentication is unavailable on this platform because immutable no-follow file identity cannot be established"
    )
}

fn authenticated_release_report_v1(
    inputs: &AuthenticatedReleaseInputsV1,
    recursive_profile_sha256: [u8; 32],
    authority_review_projection_sha256: [u8; 32],
    native_artifact_manifest_sha256: [u8; 32],
    native: &NativeArtifactManifestV1,
) -> color_eyre::Result<JsonValue> {
    let closure = &inputs.receipt.evidence_closure;
    let approved_signers = inputs
        .authenticated
        .approved_signers()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let artifacts =
        normalize_release_projection_value(norito::json::to_value(&inputs.manifest.artifacts)?)?;
    let enabled_profiles = normalize_release_projection_value(norito::json::to_value(
        &inputs.manifest.enabled_profiles,
    )?)?;
    let mut report = JsonMap::new();
    insert_json_field(
        &mut report,
        "schema",
        AUTHENTICATED_RELEASE_REPORT_SCHEMA_V1,
    )?;
    insert_json_field(&mut report, "schema_version", &1_u64)?;
    insert_json_field(&mut report, "status", "authenticated")?;
    insert_json_field(&mut report, "runtime_loaded", &true)?;
    insert_json_field(&mut report, "native_artifact_manifest_authenticated", &true)?;
    insert_json_field(&mut report, "native_artifact_hash_verified", &true)?;
    insert_json_field(&mut report, "native_bridge_probe_performed", &false)?;
    insert_json_field(
        &mut report,
        "native_bridge_abi_version",
        &u64::from(native.bridge_abi_version),
    )?;
    insert_json_field(
        &mut report,
        "release_id",
        &hex::encode(inputs.authenticated.release_id()),
    )?;
    insert_json_field(
        &mut report,
        "manifest_digest",
        &hex::encode(inputs.authenticated.manifest_digest()),
    )?;
    insert_json_field(
        &mut report,
        "validation_receipt_digest",
        &hex::encode(inputs.authenticated.receipt_digest()),
    )?;
    insert_json_field(
        &mut report,
        "authority_policy_digest",
        &hex::encode(inputs.authenticated.authority_policy_digest()),
    )?;
    insert_json_field(
        &mut report,
        "attestation_digest",
        &hex::encode(inputs.authenticated.attestation_digest()),
    )?;
    insert_json_field(
        &mut report,
        "source_tree_digest",
        &hex::encode(inputs.receipt.source_tree_digest),
    )?;
    insert_json_field(
        &mut report,
        "cargo_lock_digest",
        &hex::encode(inputs.receipt.cargo_lock_digest),
    )?;
    insert_json_field(
        &mut report,
        "profile_digest",
        &hex::encode(inputs.receipt.profile_digest),
    )?;
    insert_json_field(
        &mut report,
        "artifact_set_digest",
        &hex::encode(inputs.receipt.artifact_set_digest),
    )?;
    insert_json_field(
        &mut report,
        "hardware_policy_digest",
        &hex::encode(inputs.receipt.hardware_policy_digest),
    )?;
    insert_json_field(
        &mut report,
        "evidence_manifest_sha256",
        &hex::encode(closure.evidence_manifest.sha256),
    )?;
    insert_json_field(
        &mut report,
        "evidence_manifest_byte_len",
        &closure.evidence_manifest.byte_len,
    )?;
    insert_json_field(
        &mut report,
        "observer_policy_sha256",
        &hex::encode(closure.observer_policy.sha256),
    )?;
    insert_json_field(
        &mut report,
        "observer_policy_byte_len",
        &closure.observer_policy.byte_len,
    )?;
    insert_json_field(
        &mut report,
        "candidate_context_digest",
        &hex::encode(closure.candidate_context_digest),
    )?;
    insert_json_field(
        &mut report,
        "verification_records_digest",
        &hex::encode(closure.verification_records_digest),
    )?;
    insert_json_field(
        &mut report,
        "verification_record_count",
        &u64::from(closure.verification_record_count),
    )?;
    insert_json_field(
        &mut report,
        "total_evidence_bytes",
        &closure.total_evidence_bytes,
    )?;
    insert_json_field(
        &mut report,
        "total_transcript_bytes",
        &closure.total_transcript_bytes,
    )?;
    insert_json_field(
        &mut report,
        "total_command_input_bytes",
        &closure.total_command_input_bytes,
    )?;
    insert_json_field(
        &mut report,
        "total_observed_duration_ms",
        &closure.total_observed_duration_ms,
    )?;
    insert_json_field(
        &mut report,
        "total_observed_cpu_ms",
        &closure.total_observed_cpu_ms,
    )?;
    insert_json_field(
        &mut report,
        "authority_set_id",
        &hex::encode(inputs.policy.authority_set_id),
    )?;
    insert_json_field(
        &mut report,
        "authority_threshold",
        &u64::from(inputs.policy.threshold),
    )?;
    insert_json_field(&mut report, "approved_signers", &approved_signers)?;
    insert_json_field(&mut report, "artifacts", &artifacts)?;
    insert_json_field(&mut report, "enabled_profiles", &enabled_profiles)?;
    insert_json_field(
        &mut report,
        "recursive_profile_sha256",
        &hex::encode(recursive_profile_sha256),
    )?;
    insert_json_field(
        &mut report,
        "authority_review_projection_sha256",
        &hex::encode(authority_review_projection_sha256),
    )?;
    insert_json_field(
        &mut report,
        "native_artifact_manifest_sha256",
        &hex::encode(native_artifact_manifest_sha256),
    )?;
    insert_json_field(
        &mut report,
        "native_artifact_sha256",
        &native.artifact_sha256,
    )?;
    insert_json_field(&mut report, "native_artifact_size", &native.artifact_size)?;
    insert_json_field(&mut report, "native_sdk", &native.sdk)?;
    insert_json_field(&mut report, "native_target", &native.target)?;
    insert_json_field(&mut report, "native_source_commit", &native.source_commit)?;
    insert_json_field(
        &mut report,
        "native_source_manifest_sha256",
        &native.workspace_source_manifest_sha256,
    )?;
    Ok(JsonValue::Object(report))
}

fn insert_json_field<T: norito::json::JsonSerialize + ?Sized>(
    object: &mut JsonMap,
    field: &str,
    value: &T,
) -> color_eyre::Result<()> {
    object.insert(field.to_owned(), norito::json::to_value(value)?);
    Ok(())
}

fn read_bounded_immutable_file(
    path: &Path,
    maximum_bytes: usize,
    description: &str,
) -> color_eyre::Result<Vec<u8>> {
    #[cfg(not(unix))]
    {
        let _ = (path, maximum_bytes);
        bail!(
            "{description} authentication is unavailable on this platform because immutable \
             no-follow file identity cannot be established"
        );
    }
    #[cfg(unix)]
    {
        read_bounded_immutable_file_unix(path, maximum_bytes, description)
    }
}

#[cfg(unix)]
fn read_bounded_immutable_file_unix(
    path: &Path,
    maximum_bytes: usize,
    description: &str,
) -> color_eyre::Result<Vec<u8>> {
    let before = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("failed to inspect {description} at `{}`", path.display()))?;
    if !before.is_file() || before.file_type().is_symlink() {
        bail!("{description} must be one non-symlink regular file");
    }
    #[cfg(unix)]
    if std::os::unix::fs::MetadataExt::nlink(&before) != 1 {
        bail!("{description} must have exactly one filesystem link");
    }
    let maximum_bytes_u64 = u64::try_from(maximum_bytes).expect("release input cap fits u64");
    if before.len() > maximum_bytes_u64 {
        bail!("{description} exceeds the fixed {maximum_bytes}-byte limit");
    }

    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options
        .open(path)
        .wrap_err_with(|| format!("failed to open {description} at `{}`", path.display()))?;
    let opened = file
        .metadata()
        .wrap_err_with(|| format!("failed to inspect opened {description}"))?;
    if !same_input_metadata(&before, &opened) {
        bail!("{description} changed before its immutable snapshot was opened");
    }

    let mut bytes = Vec::new();
    (&file)
        .take(maximum_bytes_u64.saturating_add(1))
        .read_to_end(&mut bytes)
        .wrap_err_with(|| format!("failed to read {description}"))?;
    if bytes.len() > maximum_bytes {
        bail!("{description} exceeds the fixed {maximum_bytes}-byte limit");
    }
    if u64::try_from(bytes.len()).ok() != Some(opened.len()) {
        bail!("{description} immutable snapshot was not read in full");
    }
    let after = file
        .metadata()
        .wrap_err_with(|| format!("failed to re-inspect opened {description}"))?;
    if !same_input_metadata(&opened, &after) {
        bail!("{description} changed while its immutable snapshot was read");
    }
    Ok(bytes)
}

#[cfg(unix)]
fn same_input_metadata(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.is_file()
        && right.is_file()
        && left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.mode() == right.mode()
        && left.uid() == right.uid()
        && left.gid() == right.gid()
        && left.nlink() == 1
        && right.nlink() == 1
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact_inventory() -> Vec<KagemushaArtifactBindingV1> {
        KagemushaArtifactRoleV1::ALL
            .into_iter()
            .enumerate()
            .map(|(index, role)| KagemushaArtifactBindingV1 {
                role,
                sha256: [u8::try_from(index + 1).expect("role index fits u8"); 32],
                byte_len: 1,
            })
            .collect()
    }

    #[cfg(unix)]
    #[test]
    fn bounded_reader_accepts_the_exact_limit_and_rejects_the_next_byte() {
        let directory = tempfile::tempdir().expect("create KAGEMUSHA input directory");
        let exact = directory.path().join("exact.norito");
        fs::write(&exact, [0x5a; 16]).expect("write exact input");
        assert_eq!(
            read_bounded_immutable_file(&exact, 16, "test release input")
                .expect("read exact input"),
            [0x5a; 16]
        );
        fs::write(&exact, [0x5a; 17]).expect("write oversized input");
        let error = read_bounded_immutable_file(&exact, 16, "test release input")
            .expect_err("reject oversized input");
        assert!(error.to_string().contains("fixed 16-byte limit"));
    }

    #[test]
    fn authentication_rejects_noncanonical_manifest_bytes() {
        let error = decode_authenticated_release_inputs_v1(&[0x01], &[], &[], &[])
            .err()
            .expect("reject malformed manifest before granting release authority");
        assert!(
            error
                .to_string()
                .contains("invalid canonical KAGEMUSHA V1 release manifest")
        );
    }

    #[test]
    fn exact_inventory_rejects_omission_reorder_and_duplicate_hash() {
        let inventory = artifact_inventory();
        validate_exact_release_inventory_v1(&inventory).expect("accept ordered inventory");

        let mut missing = inventory.clone();
        missing.pop();
        assert!(validate_exact_release_inventory_v1(&missing).is_err());

        let mut reordered = inventory.clone();
        reordered.swap(0, 1);
        assert!(validate_exact_release_inventory_v1(&reordered).is_err());

        let mut duplicate = inventory;
        duplicate[41].sha256 = duplicate[40].sha256;
        assert!(validate_exact_release_inventory_v1(&duplicate).is_err());
    }

    #[test]
    fn authority_projection_rejects_corruption_and_noncanonical_json() {
        assert!(decode_authority_review_projection_json_v1(b"{not-json}").is_err());
        assert!(decode_authority_review_projection_json_v1(b"{}").is_err());
        assert!(decode_authority_review_projection_json_v1(b"{}\n").is_ok());
    }

    #[test]
    fn runtime_loader_fails_closed_for_corrupt_release_inputs() {
        assert!(
            load_authenticated_kagemusha_v1_runtime_verifier(
                b"corrupt",
                b"corrupt",
                b"corrupt",
                b"corrupt",
                br#"{}"#,
                Path::new("/"),
            )
            .is_err()
        );
    }

    #[test]
    fn native_manifest_rejects_abi_drift() {
        let value = norito::json!({
            "artifact_sha256": ("11".repeat(32)),
            "artifact_size": 4_u64,
            "bridge_abi_version": 22_u64,
            "privacy_c_exports": (REQUIRED_PRIVACY_C_EXPORTS_V1.to_vec()),
            "privacy_c_exports_inspected": true,
            "required_symbols": (REQUIRED_C_JNI_SYMBOLS_V1.to_vec()),
            "schema": NATIVE_ARTIFACT_SCHEMA_V1,
            "sdk": "c-jni",
            "source_commit": ("22".repeat(20)),
            "source_tree_clean": true,
            "target": "aarch64-apple-ios",
            "workspace_source_manifest_sha256": ("33".repeat(32)),
        });
        let bytes = python_canonical_json_bytes(&value, false).expect("canonical manifest");
        assert!(validate_native_artifact_manifest_v1(&bytes).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn artifact_and_native_substitution_are_rejected() {
        let directory = tempfile::tempdir().expect("artifact directory");
        let resolver = KagemushaDirectoryArtifactResolverV1::new(directory.path())
            .expect("directory resolver");
        let expected = b"good";
        let binding = KagemushaArtifactBindingV1 {
            role: KagemushaArtifactRoleV1::StateVkEq,
            sha256: sha256(expected),
            byte_len: u64::try_from(expected.len()).expect("small fixture"),
        };
        fs::write(resolver.path_for_digest(binding.sha256), b"evil")
            .expect("write substituted artifact");
        assert!(rehash_release_artifact_v1(&resolver, binding).is_err());

        let native = directory.path().join("native.dylib");
        fs::write(&native, b"evil").expect("write substituted native artifact");
        let native = fs::canonicalize(native).expect("canonical native path");
        assert!(authenticate_native_artifact_bytes_v1(&native, 4, sha256(expected)).is_err());
    }

    #[test]
    fn canonical_report_encoding_has_no_trailing_newline() {
        let value = norito::json!({"z": 2_u64, "a": 1_u64});
        let encoded = norito::json::to_json(&value).expect("canonical report JSON");
        assert_eq!(encoded, r#"{"a":1,"z":2}"#);
        assert!(!encoded.ends_with('\n'));
    }

    #[cfg(unix)]
    #[test]
    fn bounded_reader_rejects_symlinks_and_hard_links() {
        use std::{os::unix::fs::symlink, path::Path};

        let directory = tempfile::tempdir().expect("create KAGEMUSHA input directory");
        let source = directory.path().join("source.norito");
        let symlink_path = directory.path().join("symlink.norito");
        let hardlink_path = directory.path().join("hardlink.norito");
        fs::write(&source, [0x5a]).expect("write source");
        symlink(Path::new("source.norito"), &symlink_path).expect("create symlink");
        assert!(read_bounded_immutable_file(&symlink_path, 1, "test release input").is_err());
        fs::hard_link(&source, &hardlink_path).expect("create hard link");
        assert!(read_bounded_immutable_file(&source, 1, "test release input").is_err());
    }
}
