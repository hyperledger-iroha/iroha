//! Fail-closed KAGEMUSHA V1 release authentication.

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
use iroha_crypto::{ExposedPrivateKey, KeyPair, SignatureOf, sha256, sha256_reader_bounded};
use iroha_data_model::{
    id::NetworkId,
    kagemusha::{
        KAGEMUSHA_HALO2_K_V1, KAGEMUSHA_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1,
        KAGEMUSHA_RELEASE_ATTESTATION_MAX_BYTES_V1,
        KAGEMUSHA_RELEASE_AUTHORITY_POLICY_MAX_BYTES_V1,
        KAGEMUSHA_RELEASE_EVIDENCE_FILE_MAX_BYTES_V1, KAGEMUSHA_RELEASE_MANIFEST_MAX_BYTES_V1,
        KAGEMUSHA_WIRE_VERSION_V1, KagemushaArtifactBindingV1, KagemushaArtifactRoleV1,
        KagemushaAuthenticatedReleaseV1, KagemushaEvidenceFileV1,
        KagemushaInternalValidationReceiptV1, KagemushaReleaseApprovalV1,
        KagemushaReleaseAttestationV1, KagemushaReleaseAuthorityPolicyV1,
        KagemushaReleaseManifestV1, KagemushaReleasePurposeV1, KagemushaTestnetExperimentScopeV1,
    },
};
use norito::json::{Map as JsonMap, Value as JsonValue};
use std::{
    collections::BTreeMap,
    fmt::Write as _,
    io::{Read as _, Write},
    path::{Path, PathBuf},
};
#[cfg(unix)]
use std::{fs, fs::OpenOptions};
use zeroize::Zeroizing;

const KAGEMUSHA_RELEASE_ARTIFACT_ROLE_COUNT_V1: usize = 50;
const _: [(); KAGEMUSHA_RELEASE_ARTIFACT_ROLE_COUNT_V1] = [(); KagemushaArtifactRoleV1::ALL.len()];
const EXPERIMENTAL_ARTIFACT_INVENTORY_JSON_MAX_BYTES_V1: usize = 64 * 1024;
const AUTHORITY_REVIEW_PROJECTION_MAX_BYTES_V1: usize = 128 * 1024 * 1024;
const AUTHORITY_REVIEW_PROJECTION_SCHEMA_V1: &str =
    "iroha.kagemusha_v1.authority_review_projection";
const AUTHORITY_REVIEW_VERIFICATION_SCOPE_V1: &str = "closed filesystem provenance, exact Rust-compatible release identities, derived measurements, and threshold-signed observations from a separately pinned trusted verifier policy; candidate code is never executed by this verifier";
const TESTNET_AUTHORITY_REVIEW_PROJECTION_SCHEMA_V1: &str =
    "iroha.kagemusha_v1.testnet_experiment_authority_review_projection";
const TESTNET_AUTHORITY_REVIEW_VERIFICATION_SCOPE_V1: &str = "testnet experiment only: closed structural evidence, exact Rust-compatible release identities, and threshold-signed observations; no production hardware, endurance, resource, or independent-review qualification";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AuthorityReviewPurposeV1 {
    Production,
    TestnetExperiment,
}
const CANDIDATE_CONTEXT_DIGEST_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:release-candidate-context";
const VERIFICATION_RECORDS_DIGEST_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:verification-records";
const NATIVE_ARTIFACT_MANIFEST_MAX_BYTES_V1: usize = 64 * 1024;
const NATIVE_ARTIFACT_SCHEMA_V1: &str = "iroha.native-sdk-abi23-artifact.v1";
const AUTHENTICATED_RELEASE_REPORT_SCHEMA_V1: &str =
    "iroha.kagemusha.v1.authenticated-release-report";
const AUTHENTICATED_EXPERIMENTAL_RELEASE_REPORT_SCHEMA_V1: &str =
    "iroha.kagemusha.v1.authenticated-experimental-release-report";
const REQUIRED_PRIVACY_C_EXPORTS_V1: [&str; 5] = [
    "iroha_privacy_compiled_profile_catalog_v1",
    "iroha_privacy_validate_compiled_profile_catalog_v1",
    "iroha_privacy_exact12_fixture_bundle_v1",
    "iroha_privacy_validate_exact12_fixture_bundle_v1",
    "iroha_privacy_free_buffer",
];
const REQUIRED_C_JNI_SYMBOLS_V1: [&str; 72] = [
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
    "connect_norito_kagemusha_core_coordinator_close_v1",
    "connect_norito_kagemusha_testnet_state_proof_observe_v1",
    "connect_norito_kagemusha_testnet_finalized_mint_observe_v1",
    "connect_norito_kagemusha_testnet_value_admit_v1",
    "connect_norito_kagemusha_testnet_value_credit_v1",
    "connect_norito_kagemusha_device_capabilities_v1",
    "connect_norito_kagemusha_device_execute_v1",
    "connect_norito_kagemusha_device_command_response_v1_verify",
    "connect_norito_kagemusha_reserve_finality_hint_v1",
    "connect_norito_kagemusha_reserve_finality_verify_v1",
    "connect_norito_kagemusha_top_up_signed_request_validate_v1",
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
    "Java_org_hyperledger_iroha_sdk_offline_KagemushaDeviceLifecycleBridgeV1_00024NativeEndpoint_nativeVerifyCommandResponseV1",
    "Java_org_hyperledger_iroha_sdk_offline_KagemushaCoreCoordinatorJniV1_nativeContractV1",
    "Java_org_hyperledger_iroha_sdk_offline_KagemushaCoreCoordinatorJniV1_nativeOpenV1",
    "Java_org_hyperledger_iroha_sdk_offline_KagemushaCoreCoordinatorJniV1_nativeInvokeV1",
    "Java_org_hyperledger_iroha_sdk_offline_KagemushaCoreCoordinatorJniV1_nativeCloseV1",
    "Java_org_hyperledger_iroha_sdk_offline_probe_KagemushaTestnetStateProofObservationJniV1_nativeContractV1",
    "Java_org_hyperledger_iroha_sdk_offline_probe_KagemushaTestnetStateProofObservationJniV1_nativeObserveV1",
    "Java_org_hyperledger_iroha_sdk_offline_probe_KagemushaTestnetFinalizedMintObservationJniV1_nativeContractV1",
    "Java_org_hyperledger_iroha_sdk_offline_probe_KagemushaTestnetFinalizedMintObservationJniV1_nativeObserveV1",
    "Java_org_hyperledger_iroha_sdk_offline_probe_KagemushaTestnetValueAdmissionJniV1_nativeContractV1",
    "Java_org_hyperledger_iroha_sdk_offline_probe_KagemushaTestnetValueAdmissionJniV1_nativeAdmitV1",
    "Java_org_hyperledger_iroha_sdk_offline_probe_KagemushaTestnetValueCreditJniV1_nativeContractV1",
    "Java_org_hyperledger_iroha_sdk_offline_probe_KagemushaTestnetValueCreditJniV1_nativeCreditV1",
    "Java_org_hyperledger_iroha_sdk_offline_probe_Pixel6TestnetDiagnosticSelectionJniV1_nativeContractV1",
    "Java_org_hyperledger_iroha_sdk_offline_probe_Pixel6TestnetDiagnosticSelectionJniV1_nativeCreateV1",
    "Java_org_hyperledger_iroha_sdk_offline_wallet_KagemushaReserveFinalityJniV1_nativeBridgeAbiVersion",
    "Java_org_hyperledger_iroha_sdk_offline_wallet_KagemushaReserveFinalityJniV1_nativeHint",
    "Java_org_hyperledger_iroha_sdk_offline_wallet_KagemushaReserveFinalityJniV1_nativeVerify",
    "Java_org_hyperledger_iroha_sdk_offline_wallet_KagemushaTopUpSubmissionJniV1_nativeBridgeAbiVersion",
    "Java_org_hyperledger_iroha_sdk_offline_wallet_KagemushaTopUpSubmissionJniV1_nativeValidate",
    "connect_norito_sorafs_reference_validate_appeal_finance_cancel_asset_lock_json",
];

/// Authenticate the first-release format and its deployment evidence.
#[derive(Debug, ClapArgs)]
pub struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Prepare an unsigned testnet candidate from checked artifacts and typed evidence.
    #[command(name = "prepare-experimental-release-v1")]
    PrepareExperimentalReleaseV1(PrepareExperimentalReleaseV1Args),
    /// Authenticate one complete KAGEMUSHA V1 release and its deployment evidence.
    #[command(name = "authenticate-release-v1")]
    AuthenticateReleaseV1(AuthenticateReleaseV1Args),
    /// Authenticate one signed proof-only testnet release and its exact 50 artifacts.
    #[command(name = "authenticate-experimental-release-v1")]
    AuthenticateExperimentalReleaseV1(AuthenticateExperimentalReleaseV1Args),
    /// Sign one experimental release approval with one owner-held authority key.
    #[command(name = "sign-experimental-release-approval-v1")]
    SignExperimentalReleaseApprovalV1(SignExperimentalReleaseApprovalV1Args),
    /// Assemble independently signed approvals into one testnet attestation.
    #[command(name = "assemble-experimental-release-v1")]
    AssembleExperimentalReleaseV1(AssembleExperimentalReleaseV1Args),
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
            Command::PrepareExperimentalReleaseV1(args) => {
                prepare_experimental_release_v1(&args, writer)
            }
            Command::AuthenticateReleaseV1(args) => authenticate_release_v1(&args, writer),
            Command::AuthenticateExperimentalReleaseV1(args) => {
                authenticate_experimental_release_v1(&args, writer)
            }
            Command::SignExperimentalReleaseApprovalV1(args) => {
                sign_experimental_release_approval_v1(&args, writer)
            }
            Command::AssembleExperimentalReleaseV1(args) => {
                assemble_experimental_release_v1(&args, writer)
            }
        }
    }
}

#[derive(Debug, ClapArgs)]
struct PrepareExperimentalReleaseV1Args {
    /// Canonical Norito typed structural-evidence receipt from a trusted evidence producer.
    #[arg(long, value_name = "PATH")]
    validation_receipt: PathBuf,
    /// Typed JSON array of all 50 role-to-content-address bindings.
    #[arg(long, value_name = "PATH")]
    artifact_inventory: PathBuf,
    /// Canonical absolute directory of the 50 content-addressed proof artifacts.
    #[arg(long, value_name = "PATH")]
    artifact_root: PathBuf,
    /// Canonical absolute directory of all SHA-256-addressed receipt evidence files.
    #[arg(long, value_name = "PATH")]
    evidence_root: PathBuf,
    /// Independently trusted canonical Norito release-authority policy.
    #[arg(long, value_name = "PATH")]
    authority_policy: PathBuf,
    /// Canonical projection from the separately trusted release-evidence verifier.
    #[arg(long, value_name = "PATH")]
    authority_review_projection: PathBuf,
    /// Independently reviewed SHA-256 pin of that exact projection.
    #[arg(long, value_name = "LOWER_HEX")]
    authority_review_projection_sha256: String,
    /// Exact checked genesis-derived network identity.
    #[arg(long, value_name = "NETWORK_ID")]
    network_id: String,
    /// Exact normalized asset identity digest.
    #[arg(long, value_name = "LOWER_HEX")]
    asset_identity_digest: String,
    /// Exact asset incarnation.
    #[arg(long, value_name = "LOWER_HEX")]
    asset_incarnation: String,
    /// Decimal asset scale.
    #[arg(long, value_name = "DECIMAL")]
    asset_scale: u32,
    /// Exact reserve-liability pool identifier.
    #[arg(long, value_name = "LOWER_HEX")]
    liability_pool_id: String,
    /// New owner-only directory for the canonical unsigned manifest and verified receipt.
    #[arg(long, value_name = "DIR")]
    output_dir: PathBuf,
}

#[derive(Debug, Clone, ClapArgs)]
struct AuthenticateExperimentalReleaseV1Args {
    /// Canonical Norito testnet-experimental release manifest.
    #[arg(long, value_name = "PATH")]
    manifest: PathBuf,
    /// Canonical Norito structurally evidenced experimental validation receipt.
    #[arg(long, value_name = "PATH")]
    validation_receipt: PathBuf,
    /// Independently trusted canonical Norito release-authority policy.
    #[arg(long, value_name = "PATH")]
    authority_policy: PathBuf,
    /// Canonical Norito threshold attestation over the experimental release.
    #[arg(long, value_name = "PATH")]
    attestation: PathBuf,
    /// Canonical absolute directory containing all 50 signed artifacts.
    #[arg(long, value_name = "PATH")]
    artifact_root: PathBuf,
    #[command(flatten)]
    pins: ExperimentalOperatorPinsV1,
}

#[derive(Debug, Clone, ClapArgs)]
struct ExperimentalOperatorPinsV1 {
    /// Independently pinned genesis-derived network identity as lowercase hex.
    #[arg(long, value_name = "LOWER_HEX")]
    expected_network_id: String,
    /// Independently pinned release identifier as lowercase hex.
    #[arg(long, value_name = "LOWER_HEX")]
    expected_release_id: String,
    /// Independently pinned asset identity digest as lowercase hex.
    #[arg(long, value_name = "LOWER_HEX")]
    expected_asset_identity_digest: String,
    /// Independently pinned asset incarnation as lowercase hex.
    #[arg(long, value_name = "LOWER_HEX")]
    expected_asset_incarnation: String,
    /// Independently pinned decimal asset scale.
    #[arg(long, value_name = "DECIMAL")]
    expected_asset_scale: u32,
    /// Independently pinned reserve-liability pool identifier as lowercase hex.
    #[arg(long, value_name = "LOWER_HEX")]
    expected_liability_pool_id: String,
}

#[derive(Debug, ClapArgs)]
struct SignExperimentalReleaseApprovalV1Args {
    /// Canonical Norito testnet-experimental release manifest.
    #[arg(long, value_name = "PATH")]
    manifest: PathBuf,
    /// Canonical Norito structurally evidenced experimental validation receipt.
    #[arg(long, value_name = "PATH")]
    validation_receipt: PathBuf,
    /// Independently trusted canonical Norito release-authority policy.
    #[arg(long, value_name = "PATH")]
    authority_policy: PathBuf,
    /// Canonical absolute directory containing all 50 signed artifacts.
    #[arg(long, value_name = "PATH")]
    artifact_root: PathBuf,
    /// One owner-held mode-0600 Kagami private-key record.
    #[arg(long, value_name = "PATH")]
    signer_private_key: PathBuf,
    /// New owner-only file for this authority's canonical Norito approval.
    #[arg(long, value_name = "PATH")]
    approval_output: PathBuf,
    #[command(flatten)]
    pins: ExperimentalOperatorPinsV1,
}

#[derive(Debug, ClapArgs)]
struct AssembleExperimentalReleaseV1Args {
    /// Canonical Norito testnet-experimental release manifest.
    #[arg(long, value_name = "PATH")]
    manifest: PathBuf,
    /// Canonical Norito structurally evidenced experimental validation receipt.
    #[arg(long, value_name = "PATH")]
    validation_receipt: PathBuf,
    /// Independently trusted canonical Norito release-authority policy.
    #[arg(long, value_name = "PATH")]
    authority_policy: PathBuf,
    /// Canonical absolute directory containing all 50 signed artifacts.
    #[arg(long, value_name = "PATH")]
    artifact_root: PathBuf,
    /// One canonical Norito approval; repeat for each independent authority.
    #[arg(long, value_name = "PATH", required = true)]
    approval: Vec<PathBuf>,
    /// New owner-only file for the canonical threshold attestation.
    #[arg(long, value_name = "PATH")]
    attestation_output: PathBuf,
    #[command(flatten)]
    pins: ExperimentalOperatorPinsV1,
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
        AuthorityReviewPurposeV1::Production,
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

fn experimental_receipt_evidence_files_v1(
    receipt: &KagemushaInternalValidationReceiptV1,
) -> color_eyre::Result<Vec<KagemushaEvidenceFileV1>> {
    let mut files = vec![
        receipt.evidence_closure.evidence_manifest,
        receipt.evidence_closure.observer_policy,
        receipt.circuit_shape_report,
    ];
    for optional in [
        receipt.security_review_report,
        receipt.kat_report,
        receipt.fuzz_report,
        receipt.resource_report,
    ] {
        append_optional_experimental_evidence_v1(&mut files, optional)?;
    }
    for profile in &receipt.profile_qualifications {
        files.push(profile.profile.qualification_report);
        files.extend(profile.relations.iter().map(|row| row.report));
        files.extend(profile.helper_circuits.iter().map(|row| row.report));
        for optional in profile
            .recursive_depths
            .iter()
            .map(|row| row.report)
            .chain([
                profile.aggregate_balance.report,
                profile.thermal.report,
                profile.envelope.report,
            ])
            .chain(profile.acceptance_cases.iter().map(|row| row.report))
        {
            append_optional_experimental_evidence_v1(&mut files, optional)?;
        }
    }
    for optional in receipt.reproducible_builds.iter().map(|row| row.report) {
        append_optional_experimental_evidence_v1(&mut files, optional)?;
    }
    Ok(files)
}

fn append_optional_experimental_evidence_v1(
    files: &mut Vec<KagemushaEvidenceFileV1>,
    file: KagemushaEvidenceFileV1,
) -> Outcome {
    match (file.sha256 == [0; 32], file.byte_len) {
        (true, 0) => Ok(()),
        (false, 1..=KAGEMUSHA_RELEASE_EVIDENCE_FILE_MAX_BYTES_V1) => {
            files.push(file);
            Ok(())
        }
        _ => bail!("optional experimental evidence has a partial or oversized file binding"),
    }
}

fn rehash_experimental_receipt_evidence_v1(
    receipt: &KagemushaInternalValidationReceiptV1,
    evidence_root: &Path,
) -> color_eyre::Result<usize> {
    rehash_experimental_evidence_files_v1(
        &experimental_receipt_evidence_files_v1(receipt)?,
        evidence_root,
    )
}

fn rehash_experimental_evidence_files_v1(
    files: &[KagemushaEvidenceFileV1],
    evidence_root: &Path,
) -> color_eyre::Result<usize> {
    let root = canonical_artifact_root(evidence_root)?;
    let mut unique = BTreeMap::new();
    for file in files {
        if let Some(previous_len) = unique.insert(file.sha256, file.byte_len)
            && previous_len != file.byte_len
        {
            bail!("experimental receipt reuses an evidence digest with a different length");
        }
    }
    for (digest, byte_len) in &unique {
        let path = root.join(hex::encode(digest));
        if hash_immutable_file_exact(&path, *byte_len, "KAGEMUSHA V1 receipt evidence")? != *digest
        {
            bail!("experimental receipt evidence differs from its typed SHA-256 binding");
        }
    }
    Ok(unique.len())
}

fn prepare_experimental_release_v1<T: Write>(
    args: &PrepareExperimentalReleaseV1Args,
    writer: &mut std::io::BufWriter<T>,
) -> Outcome {
    let receipt_bytes = read_bounded_immutable_file(
        &args.validation_receipt,
        KAGEMUSHA_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1,
        "KAGEMUSHA V1 typed experimental validation receipt",
    )?;
    let receipt =
        KagemushaInternalValidationReceiptV1::decode_canonical_experimental_exact(&receipt_bytes)
            .map_err(|source| eyre!("invalid typed experimental receipt: {source}"))?;
    let inventory_bytes = read_bounded_immutable_file(
        &args.artifact_inventory,
        EXPERIMENTAL_ARTIFACT_INVENTORY_JSON_MAX_BYTES_V1,
        "KAGEMUSHA V1 experimental artifact inventory",
    )?;
    let artifacts: Vec<KagemushaArtifactBindingV1> = norito::json::from_slice(&inventory_bytes)
        .map_err(|source| eyre!("invalid typed experimental artifact inventory: {source}"))?;
    validate_exact_release_inventory_v1(&artifacts)?;
    let artifact_root = canonical_artifact_root(&args.artifact_root)?;
    rehash_all_release_artifacts_v1(&artifacts, &artifact_root)?;
    let evidence_file_count =
        rehash_experimental_receipt_evidence_v1(&receipt, &args.evidence_root)?;
    let policy_bytes = read_bounded_immutable_file(
        &args.authority_policy,
        KAGEMUSHA_RELEASE_AUTHORITY_POLICY_MAX_BYTES_V1,
        "KAGEMUSHA V1 experimental authority policy",
    )?;
    let policy = KagemushaReleaseAuthorityPolicyV1::decode_canonical_exact(&policy_bytes)
        .map_err(|source| eyre!("invalid experimental authority policy: {source}"))?;
    let network_id = args
        .network_id
        .parse::<NetworkId>()
        .map_err(|source| eyre!("invalid genesis-derived network identity: {source}"))?;
    let scope = KagemushaTestnetExperimentScopeV1 {
        asset_identity_digest: parse_lower_sha256(
            &args.asset_identity_digest,
            "experimental asset identity digest",
        )?,
        asset_incarnation: parse_lower_sha256(
            &args.asset_incarnation,
            "experimental asset incarnation",
        )?,
        asset_scale: args.asset_scale,
        liability_pool_id: parse_lower_sha256(
            &args.liability_pool_id,
            "experimental reserve-liability pool identifier",
        )?,
    };
    scope
        .validate()
        .map_err(|source| eyre!("invalid experimental monetary scope: {source}"))?;
    let receipt_digest = receipt
        .canonical_experimental_digest()
        .map_err(|source| eyre!("invalid experimental receipt digest: {source}"))?;
    let manifest = KagemushaReleaseManifestV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        network_id,
        purpose: KagemushaReleasePurposeV1::TestnetExperiment(scope),
        release_id: [0; 32],
        source_tree_digest: receipt.source_tree_digest,
        cargo_lock_digest: receipt.cargo_lock_digest,
        profile_digest: receipt.profile_digest,
        eq_protocol_digest: receipt.eq_protocol_digest,
        ep_protocol_digest: receipt.ep_protocol_digest,
        hardware_policy_digest: receipt.hardware_policy_digest,
        validation_receipt_digest: receipt_digest,
        halo2_k: KAGEMUSHA_HALO2_K_V1,
        helper_protocols: receipt.helper_protocols.clone(),
        enabled_profiles: receipt
            .profile_qualifications
            .iter()
            .map(|qualification| qualification.profile)
            .collect(),
        artifacts,
    }
    .seal()
    .map_err(|source| eyre!("cannot seal experimental release manifest: {source}"))?;
    let subject = manifest
        .experimental_release_attestation_subject(&receipt, &policy)
        .map_err(|source| eyre!("experimental release candidate is inconsistent: {source}"))?;
    let projection_bytes = read_bounded_immutable_file(
        &args.authority_review_projection,
        AUTHORITY_REVIEW_PROJECTION_MAX_BYTES_V1,
        "KAGEMUSHA V1 experimental authority-review projection",
    )?;
    let projection_sha256 = parse_lower_sha256(
        &args.authority_review_projection_sha256,
        "experimental authority-review projection SHA-256",
    )?;
    if sha256(&projection_bytes) != projection_sha256 {
        bail!("experimental authority-review projection differs from its independent SHA-256 pin");
    }
    validate_authority_review_projection_v1(
        &projection_bytes,
        &manifest,
        &receipt,
        AuthorityReviewPurposeV1::TestnetExperiment,
    )?;
    let manifest_bytes = norito::encode_canonical(&manifest)?;
    let output_dir = crate::secure_fs::prepare_empty_private_directory(&args.output_dir)?;
    crate::secure_fs::write_private_file_atomic(
        &output_dir.join("validation-receipt.norito"),
        &receipt_bytes,
    )?;
    crate::secure_fs::write_private_file_atomic(
        &output_dir.join("manifest.norito"),
        &manifest_bytes,
    )?;
    // TODO: the trusted evidence producer must validate each report's semantic claims and
    // source-tree provenance; this preparer proves byte existence and closed release bindings.
    let mut report = JsonMap::new();
    insert_json_field(
        &mut report,
        "status",
        "prepared_unsigned_experimental_candidate",
    )?;
    insert_json_field(&mut report, "release_authenticated", &false)?;
    insert_json_field(&mut report, "hardware_qualified", &false)?;
    insert_json_field(&mut report, "monetary_admission", &false)?;
    insert_json_field(&mut report, "release_id", &hex::encode(manifest.release_id))?;
    insert_json_field(
        &mut report,
        "validation_receipt_digest",
        &hex::encode(receipt_digest),
    )?;
    insert_json_field(
        &mut report,
        "authority_policy_digest",
        &hex::encode(subject.authority_policy_digest),
    )?;
    insert_json_field(&mut report, "artifact_count", &50_u64)?;
    insert_json_field(
        &mut report,
        "evidence_file_count",
        &u64::try_from(evidence_file_count)?,
    )?;
    write!(
        writer,
        "{}",
        norito::json::to_json(&JsonValue::Object(report))?
    )?;
    Ok(())
}

fn authenticate_experimental_release_v1<T: Write>(
    args: &AuthenticateExperimentalReleaseV1Args,
    writer: &mut std::io::BufWriter<T>,
) -> Outcome {
    let manifest_bytes = read_bounded_immutable_file(
        &args.manifest,
        KAGEMUSHA_RELEASE_MANIFEST_MAX_BYTES_V1,
        "KAGEMUSHA V1 experimental release manifest",
    )?;
    let receipt_bytes = read_bounded_immutable_file(
        &args.validation_receipt,
        KAGEMUSHA_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1,
        "KAGEMUSHA V1 experimental validation receipt",
    )?;
    let policy_bytes = read_bounded_immutable_file(
        &args.authority_policy,
        KAGEMUSHA_RELEASE_AUTHORITY_POLICY_MAX_BYTES_V1,
        "KAGEMUSHA V1 experimental authority policy",
    )?;
    let attestation_bytes = read_bounded_immutable_file(
        &args.attestation,
        KAGEMUSHA_RELEASE_ATTESTATION_MAX_BYTES_V1,
        "KAGEMUSHA V1 experimental release attestation",
    )?;
    let manifest = KagemushaReleaseManifestV1::decode_canonical_exact(&manifest_bytes)
        .map_err(|source| eyre!("invalid experimental release manifest: {source}"))?;
    let receipt =
        KagemushaInternalValidationReceiptV1::decode_canonical_experimental_exact(&receipt_bytes)
            .map_err(|source| eyre!("invalid experimental validation receipt: {source}"))?;
    let policy = KagemushaReleaseAuthorityPolicyV1::decode_canonical_exact(&policy_bytes)
        .map_err(|source| eyre!("invalid experimental authority policy: {source}"))?;
    let attestation = KagemushaReleaseAttestationV1::decode_canonical_exact(&attestation_bytes)
        .map_err(|source| eyre!("invalid experimental release attestation: {source}"))?;
    let authenticated = manifest
        .authenticate_experimental(&receipt, &policy, &attestation)
        .map_err(|source| eyre!("experimental release authentication failed: {source}"))?;
    let KagemushaReleasePurposeV1::TestnetExperiment(signed_scope) = authenticated.purpose() else {
        bail!("experimental command requires a signed testnet-experiment purpose");
    };
    validate_experimental_operator_pins_v1(
        &args.pins,
        *authenticated.network_id().as_bytes(),
        authenticated.release_id(),
        signed_scope,
    )?;
    validate_exact_release_inventory_v1(&manifest.artifacts)?;
    let artifact_root = canonical_artifact_root(&args.artifact_root)?;
    rehash_all_release_artifacts_v1(&manifest.artifacts, &artifact_root)?;

    let approved_signers = authenticated
        .approved_signers()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let mut report = JsonMap::new();
    insert_json_field(
        &mut report,
        "schema",
        AUTHENTICATED_EXPERIMENTAL_RELEASE_REPORT_SCHEMA_V1,
    )?;
    insert_json_field(&mut report, "schema_version", &1_u64)?;
    insert_json_field(&mut report, "status", "authenticated")?;
    insert_json_field(&mut report, "purpose", "testnet_experiment")?;
    insert_json_field(&mut report, "proof_only", &true)?;
    insert_json_field(&mut report, "hardware_qualified", &false)?;
    insert_json_field(&mut report, "monetary_admission", &false)?;
    insert_json_field(&mut report, "runtime_loaded", &false)?;
    insert_json_field(&mut report, "artifacts_rehashed", &true)?;
    insert_json_field(
        &mut report,
        "network_id",
        &hex::encode(authenticated.network_id().as_bytes()),
    )?;
    insert_json_field(
        &mut report,
        "release_id",
        &hex::encode(authenticated.release_id()),
    )?;
    insert_json_field(
        &mut report,
        "manifest_digest",
        &hex::encode(authenticated.manifest_digest()),
    )?;
    insert_json_field(
        &mut report,
        "validation_receipt_digest",
        &hex::encode(authenticated.receipt_digest()),
    )?;
    insert_json_field(
        &mut report,
        "authority_policy_digest",
        &hex::encode(authenticated.authority_policy_digest()),
    )?;
    insert_json_field(
        &mut report,
        "attestation_digest",
        &hex::encode(authenticated.attestation_digest()),
    )?;
    insert_json_field(
        &mut report,
        "asset_identity_digest",
        &hex::encode(signed_scope.asset_identity_digest),
    )?;
    insert_json_field(
        &mut report,
        "asset_incarnation",
        &hex::encode(signed_scope.asset_incarnation),
    )?;
    insert_json_field(&mut report, "asset_scale", &signed_scope.asset_scale)?;
    insert_json_field(
        &mut report,
        "liability_pool_id",
        &hex::encode(signed_scope.liability_pool_id),
    )?;
    insert_json_field(
        &mut report,
        "authority_threshold",
        &u64::from(policy.threshold),
    )?;
    insert_json_field(&mut report, "approved_signers", &approved_signers)?;
    write!(
        writer,
        "{}",
        norito::json::to_json(&JsonValue::Object(report))?
    )?;
    Ok(())
}

fn validate_experimental_operator_pins_v1(
    pins: &ExperimentalOperatorPinsV1,
    signed_network_id: [u8; 32],
    signed_release_id: [u8; 32],
    signed_scope: KagemushaTestnetExperimentScopeV1,
) -> Outcome {
    let expected_scope = KagemushaTestnetExperimentScopeV1 {
        asset_identity_digest: parse_lower_sha256(
            &pins.expected_asset_identity_digest,
            "expected asset identity digest",
        )?,
        asset_incarnation: parse_lower_sha256(
            &pins.expected_asset_incarnation,
            "expected asset incarnation",
        )?,
        asset_scale: pins.expected_asset_scale,
        liability_pool_id: parse_lower_sha256(
            &pins.expected_liability_pool_id,
            "expected liability pool identifier",
        )?,
    };
    expected_scope
        .validate()
        .map_err(|source| eyre!("invalid independent experimental scope pins: {source}"))?;
    if signed_network_id
        != parse_lower_sha256(&pins.expected_network_id, "expected network identity")?
        || signed_release_id
            != parse_lower_sha256(&pins.expected_release_id, "expected release identifier")?
        || signed_scope != expected_scope
    {
        bail!("signed experimental release differs from independent operator pins");
    }
    Ok(())
}

fn read_experimental_release_material_v1(
    manifest_path: &Path,
    receipt_path: &Path,
    policy_path: &Path,
) -> color_eyre::Result<(
    KagemushaReleaseManifestV1,
    KagemushaInternalValidationReceiptV1,
    KagemushaReleaseAuthorityPolicyV1,
)> {
    let manifest_bytes = read_bounded_immutable_file(
        manifest_path,
        KAGEMUSHA_RELEASE_MANIFEST_MAX_BYTES_V1,
        "KAGEMUSHA V1 experimental release manifest",
    )?;
    let receipt_bytes = read_bounded_immutable_file(
        receipt_path,
        KAGEMUSHA_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1,
        "KAGEMUSHA V1 experimental validation receipt",
    )?;
    let policy_bytes = read_bounded_immutable_file(
        policy_path,
        KAGEMUSHA_RELEASE_AUTHORITY_POLICY_MAX_BYTES_V1,
        "KAGEMUSHA V1 experimental authority policy",
    )?;
    let manifest = KagemushaReleaseManifestV1::decode_canonical_exact(&manifest_bytes)
        .map_err(|source| eyre!("invalid experimental release manifest: {source}"))?;
    let receipt =
        KagemushaInternalValidationReceiptV1::decode_canonical_experimental_exact(&receipt_bytes)
            .map_err(|source| eyre!("invalid experimental validation receipt: {source}"))?;
    let policy = KagemushaReleaseAuthorityPolicyV1::decode_canonical_exact(&policy_bytes)
        .map_err(|source| eyre!("invalid experimental authority policy: {source}"))?;
    Ok((manifest, receipt, policy))
}

fn check_experimental_release_material_v1(
    manifest: &KagemushaReleaseManifestV1,
    receipt: &KagemushaInternalValidationReceiptV1,
    policy: &KagemushaReleaseAuthorityPolicyV1,
    pins: &ExperimentalOperatorPinsV1,
    artifact_root: &Path,
) -> color_eyre::Result<iroha_data_model::kagemusha::KagemushaReleaseAttestationSubjectV1> {
    let subject = manifest
        .experimental_release_attestation_subject(receipt, policy)
        .map_err(|source| eyre!("invalid experimental release signing subject: {source}"))?;
    let KagemushaReleasePurposeV1::TestnetExperiment(signed_scope) = manifest.purpose else {
        bail!("experimental signing requires a signed testnet-experiment purpose");
    };
    validate_experimental_operator_pins_v1(
        pins,
        *manifest.network_id.as_bytes(),
        manifest.release_id,
        signed_scope,
    )?;
    validate_exact_release_inventory_v1(&manifest.artifacts)?;
    let artifact_root = canonical_artifact_root(artifact_root)?;
    rehash_all_release_artifacts_v1(&manifest.artifacts, &artifact_root)?;
    Ok(subject)
}

fn load_experimental_signing_key_v1(path: &Path) -> color_eyre::Result<KeyPair> {
    let raw = Zeroizing::new(crate::secure_fs::read_private_file(path)?);
    let text = std::str::from_utf8(raw.as_slice())
        .map_err(|_| eyre!("experimental authority private-key file is not UTF-8"))?;
    let canonical = text
        .strip_suffix('\n')
        .ok_or_else(|| eyre!("experimental authority private-key file lacks its final newline"))?;
    if canonical.is_empty() || canonical.chars().any(char::is_whitespace) {
        bail!("experimental authority private-key file is not one canonical key record");
    }
    let exposed = canonical
        .parse::<ExposedPrivateKey>()
        .map_err(|_| eyre!("invalid experimental authority private-key record"))?;
    let reencoded = Zeroizing::new(exposed.to_string());
    if reencoded.as_str() != canonical {
        bail!("experimental authority private-key encoding is not canonical");
    }
    KeyPair::from_private_key(exposed.0)
        .map_err(|_| eyre!("cannot derive experimental authority public key"))
}

fn sign_experimental_release_approval_v1<T: Write>(
    args: &SignExperimentalReleaseApprovalV1Args,
    writer: &mut std::io::BufWriter<T>,
) -> Outcome {
    let (manifest, receipt, policy) = read_experimental_release_material_v1(
        &args.manifest,
        &args.validation_receipt,
        &args.authority_policy,
    )?;
    let subject = check_experimental_release_material_v1(
        &manifest,
        &receipt,
        &policy,
        &args.pins,
        &args.artifact_root,
    )?;
    let signing_key = load_experimental_signing_key_v1(&args.signer_private_key)?;
    if policy
        .authorized_signers
        .binary_search(signing_key.public_key())
        .is_err()
    {
        bail!("experimental signing key is absent from the independently trusted policy");
    }
    let payload = subject.approval_payload();
    let approval = KagemushaReleaseApprovalV1 {
        public_key: signing_key.public_key().clone(),
        signature: SignatureOf::try_new(signing_key.private_key(), &payload)
            .map_err(|_| eyre!("experimental release approval signature failed"))?,
    };
    approval
        .signature
        .verify(&approval.public_key, &payload)
        .map_err(|_| eyre!("experimental release approval self-verification failed"))?;
    let encoded = norito::encode_canonical(&approval)?;
    crate::secure_fs::write_private_file_atomic(&args.approval_output, &encoded)?;
    let mut report = JsonMap::new();
    insert_json_field(&mut report, "status", "signed_one_approval")?;
    insert_json_field(&mut report, "release_id", &hex::encode(subject.release_id))?;
    insert_json_field(&mut report, "signer", &approval.public_key.to_string())?;
    write!(
        writer,
        "{}",
        norito::json::to_json(&JsonValue::Object(report))?
    )?;
    Ok(())
}

fn assemble_experimental_release_v1<T: Write>(
    args: &AssembleExperimentalReleaseV1Args,
    writer: &mut std::io::BufWriter<T>,
) -> Outcome {
    let (manifest, receipt, policy) = read_experimental_release_material_v1(
        &args.manifest,
        &args.validation_receipt,
        &args.authority_policy,
    )?;
    let subject = check_experimental_release_material_v1(
        &manifest,
        &receipt,
        &policy,
        &args.pins,
        &args.artifact_root,
    )?;
    let mut approvals = Vec::with_capacity(args.approval.len());
    for path in &args.approval {
        let bytes = read_bounded_immutable_file(
            path,
            KAGEMUSHA_RELEASE_ATTESTATION_MAX_BYTES_V1,
            "KAGEMUSHA V1 experimental release approval",
        )?;
        let approval: KagemushaReleaseApprovalV1 = norito::decode_canonical(&bytes)
            .map_err(|source| eyre!("invalid canonical experimental approval: {source}"))?;
        approvals.push(approval);
    }
    approvals.sort_by(|left, right| left.public_key.cmp(&right.public_key));
    let attestation = KagemushaReleaseAttestationV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        subject,
        approvals,
    };
    let authenticated = manifest
        .authenticate_experimental(&receipt, &policy, &attestation)
        .map_err(|source| eyre!("experimental threshold assembly failed: {source}"))?;
    let encoded = norito::encode_canonical(&attestation)?;
    crate::secure_fs::write_private_file_atomic(&args.attestation_output, &encoded)?;
    let mut report = JsonMap::new();
    insert_json_field(&mut report, "status", "assembled_experimental_attestation")?;
    insert_json_field(
        &mut report,
        "release_id",
        &hex::encode(authenticated.release_id()),
    )?;
    insert_json_field(
        &mut report,
        "attestation_digest",
        &hex::encode(authenticated.attestation_digest()),
    )?;
    insert_json_field(
        &mut report,
        "approved_signers",
        &authenticated
            .approved_signers()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
    )?;
    write!(
        writer,
        "{}",
        norito::json::to_json(&JsonValue::Object(report))?
    )?;
    Ok(())
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
    purpose: AuthorityReviewPurposeV1,
) -> color_eyre::Result<()> {
    let (expected_schema, expected_scope) =
        authority_review_contract_v1(purpose, manifest.purpose)?;
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
    if json_string(root, "schema")? != expected_schema
        || json_u64(root, "schema_version")? != 1
        || json_string(root, "verification_scope")? != expected_scope
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

fn authority_review_contract_v1(
    purpose: AuthorityReviewPurposeV1,
    release_purpose: KagemushaReleasePurposeV1,
) -> color_eyre::Result<(&'static str, &'static str)> {
    Ok(match (purpose, release_purpose) {
        (AuthorityReviewPurposeV1::Production, KagemushaReleasePurposeV1::Production) => (
            AUTHORITY_REVIEW_PROJECTION_SCHEMA_V1,
            AUTHORITY_REVIEW_VERIFICATION_SCOPE_V1,
        ),
        (
            AuthorityReviewPurposeV1::TestnetExperiment,
            KagemushaReleasePurposeV1::TestnetExperiment(_),
        ) => (
            TESTNET_AUTHORITY_REVIEW_PROJECTION_SCHEMA_V1,
            TESTNET_AUTHORITY_REVIEW_VERIFICATION_SCOPE_V1,
        ),
        _ => bail!("authority-review projection purpose differs from signed release"),
    })
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
                } else if matches!(
                    field.as_str(),
                    "governance_credential_public_key" | "issuer_signature"
                ) {
                    let mut tuple = value
                        .as_array()
                        .ok_or_else(|| eyre!("{field} must be a JSON tuple"))?
                        .clone();
                    if tuple.len() != 1 {
                        bail!("{field} JSON tuple is malformed");
                    }
                    let width = if field == "issuer_signature" { 64 } else { 65 };
                    JsonValue::String(fixed_byte_array_to_hex(tuple.remove(0), width, &field)?)
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
            | "native_profile_digest"
            | "eq_protocol_digest"
            | "ep_protocol_digest"
            | "artifact_set_digest"
            | "hardware_policy_digest"
            | "provider_policy_root"
            | "provider_authority_commitment"
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
            | "app_attestation_authority_policy_digest"
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
        "native_profile_digest",
        &hex::encode(inputs.receipt.native_profile_digest),
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
        "provider_policy_root",
        &hex::encode(inputs.receipt.provider_policy_root),
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

    fn experimental_args() -> AuthenticateExperimentalReleaseV1Args {
        AuthenticateExperimentalReleaseV1Args {
            manifest: PathBuf::from("manifest.norito"),
            validation_receipt: PathBuf::from("receipt.norito"),
            authority_policy: PathBuf::from("policy.norito"),
            attestation: PathBuf::from("attestation.norito"),
            artifact_root: PathBuf::from("artifacts"),
            pins: ExperimentalOperatorPinsV1 {
                expected_network_id: "11".repeat(32),
                expected_release_id: "22".repeat(32),
                expected_asset_identity_digest: "33".repeat(32),
                expected_asset_incarnation: "44".repeat(32),
                expected_asset_scale: 2,
                expected_liability_pool_id: "55".repeat(32),
            },
        }
    }

    #[test]
    fn experimental_command_requires_all_independent_pins() {
        use clap::Parser as _;

        let command = [
            "kagami",
            "kagemusha",
            "authenticate-experimental-release-v1",
            "--manifest",
            "manifest.norito",
            "--validation-receipt",
            "receipt.norito",
            "--authority-policy",
            "policy.norito",
            "--attestation",
            "attestation.norito",
            "--artifact-root",
            "artifacts",
            "--expected-network-id",
            "11",
            "--expected-release-id",
            "22",
            "--expected-asset-identity-digest",
            "33",
            "--expected-asset-incarnation",
            "44",
            "--expected-asset-scale",
            "2",
            "--expected-liability-pool-id",
            "55",
        ];
        assert!(crate::Cli::try_parse_from(command).is_ok());
        assert!(crate::Cli::try_parse_from(command[..command.len() - 2].iter().copied()).is_err());
    }

    #[test]
    fn experimental_prepare_command_requires_evidence_and_output_custody() {
        use clap::Parser as _;

        let command = "kagami kagemusha prepare-experimental-release-v1 \
                       --validation-receipt receipt.norito --artifact-inventory artifacts.json \
                       --artifact-root artifacts --evidence-root evidence \
                       --authority-policy policy.norito \
                       --authority-review-projection projection.json \
                       --authority-review-projection-sha256 11 \
                       --network-id network --asset-identity-digest 22 \
                       --asset-incarnation 33 --asset-scale 2 \
                       --liability-pool-id 44 --output-dir out";
        assert!(crate::Cli::try_parse_from(command.split_whitespace()).is_ok());
        assert!(
            crate::Cli::try_parse_from(
                command
                    .replace("--evidence-root evidence", "")
                    .split_whitespace()
            )
            .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn experimental_preparation_rehashes_every_artifact_and_evidence_file() {
        let parent = fs::canonicalize(std::env::temp_dir()).expect("canonical temporary parent");
        let artifact_dir = tempfile::Builder::new()
            .prefix(".kagemusha-experimental-artifacts-")
            .tempdir_in(&parent)
            .expect("artifact fixture directory");
        let evidence_dir = tempfile::Builder::new()
            .prefix(".kagemusha-experimental-evidence-")
            .tempdir_in(parent)
            .expect("evidence fixture directory");
        let resolver = KagemushaDirectoryArtifactResolverV1::new(artifact_dir.path())
            .expect("content-addressed artifact resolver");
        let artifacts = KagemushaArtifactRoleV1::ALL
            .into_iter()
            .enumerate()
            .map(|(index, role)| {
                use iroha_core::zk::kagemusha_v1_recursion::{
                    KagemushaArtifactDescriptorV1, KagemushaArtifactKindV1,
                };
                let descriptor = KagemushaArtifactDescriptorV1::for_role(role);
                let byte_len = if descriptor.kind == KagemushaArtifactKindV1::Parameters {
                    usize::try_from(descriptor.byte_limit).expect("fixed parameter length")
                } else {
                    1
                };
                let bytes = vec![u8::try_from(index + 1).expect("bounded role index"); byte_len];
                let digest = sha256(&bytes);
                fs::write(resolver.path_for_digest(digest), &bytes)
                    .expect("write real content-addressed artifact");
                KagemushaArtifactBindingV1 {
                    role,
                    sha256: digest,
                    byte_len: u64::try_from(byte_len).expect("bounded artifact length"),
                }
            })
            .collect::<Vec<_>>();
        rehash_all_release_artifacts_v1(&artifacts, artifact_dir.path())
            .expect("all 50 real artifact bindings match");
        let evidence_bytes = b"observed structural circuit rows";
        let evidence = KagemushaEvidenceFileV1 {
            sha256: sha256(evidence_bytes),
            byte_len: u64::try_from(evidence_bytes.len()).expect("small evidence"),
        };
        let evidence_path = evidence_dir.path().join(hex::encode(evidence.sha256));
        fs::write(&evidence_path, evidence_bytes).expect("write real evidence file");
        assert_eq!(
            rehash_experimental_evidence_files_v1(&[evidence, evidence], evidence_dir.path())
                .expect("deduplicated evidence rehash"),
            1
        );
        let wrong_length = KagemushaEvidenceFileV1 {
            byte_len: evidence.byte_len + 1,
            ..evidence
        };
        assert!(
            rehash_experimental_evidence_files_v1(&[evidence, wrong_length], evidence_dir.path())
                .is_err()
        );
        fs::write(&evidence_path, vec![0xee; evidence_bytes.len()])
            .expect("substitute evidence bytes");
        assert!(rehash_experimental_evidence_files_v1(&[evidence], evidence_dir.path()).is_err());
        fs::write(resolver.path_for_digest(artifacts[0].sha256), [0xff])
            .expect("substitute one proof artifact");
        assert!(rehash_all_release_artifacts_v1(&artifacts, artifact_dir.path()).is_err());
    }

    #[test]
    fn experimental_optional_evidence_requires_a_complete_binding_when_present() {
        let absent = KagemushaEvidenceFileV1 {
            sha256: [0; 32],
            byte_len: 0,
        };
        let present = KagemushaEvidenceFileV1 {
            sha256: sha256(b"reviewed experimental evidence"),
            byte_len: 30,
        };
        let mut files = Vec::new();
        append_optional_experimental_evidence_v1(&mut files, absent)
            .expect("canonical absence needs no file");
        assert!(files.is_empty());
        append_optional_experimental_evidence_v1(&mut files, present)
            .expect("complete optional binding is rehashed");
        assert_eq!(files, vec![present]);
        assert!(
            append_optional_experimental_evidence_v1(
                &mut files,
                KagemushaEvidenceFileV1 {
                    sha256: present.sha256,
                    byte_len: 0,
                },
            )
            .is_err()
        );
        assert!(
            append_optional_experimental_evidence_v1(
                &mut files,
                KagemushaEvidenceFileV1 {
                    sha256: [0; 32],
                    byte_len: present.byte_len,
                },
            )
            .is_err()
        );
        assert!(
            append_optional_experimental_evidence_v1(
                &mut files,
                KagemushaEvidenceFileV1 {
                    sha256: present.sha256,
                    byte_len: KAGEMUSHA_RELEASE_EVIDENCE_FILE_MAX_BYTES_V1 + 1,
                },
            )
            .is_err()
        );
        assert_eq!(files, vec![present]);
    }

    #[test]
    fn experimental_issuance_commands_require_one_key_or_approval_at_a_time() {
        use clap::Parser as _;

        let pins = "--expected-network-id 11 --expected-release-id 22 \
                    --expected-asset-identity-digest 33 --expected-asset-incarnation 44 \
                    --expected-asset-scale 2 --expected-liability-pool-id 55";
        let sign = format!(
            "kagami kagemusha sign-experimental-release-approval-v1 \
             --manifest manifest.norito --validation-receipt receipt.norito \
             --authority-policy policy.norito --artifact-root artifacts \
             --signer-private-key signer.key --approval-output approval.norito {pins}"
        );
        assert!(crate::Cli::try_parse_from(sign.split_whitespace()).is_ok());
        assert!(
            crate::Cli::try_parse_from(
                sign.replace("--signer-private-key signer.key", "")
                    .split_whitespace()
            )
            .is_err()
        );
        let assemble = format!(
            "kagami kagemusha assemble-experimental-release-v1 \
             --manifest manifest.norito --validation-receipt receipt.norito \
             --authority-policy policy.norito --artifact-root artifacts \
             --approval approval-a.norito --approval approval-b.norito \
             --attestation-output attestation.norito {pins}"
        );
        assert!(crate::Cli::try_parse_from(assemble.split_whitespace()).is_ok());
        assert!(
            crate::Cli::try_parse_from(
                assemble
                    .replace(
                        "--approval approval-a.norito --approval approval-b.norito",
                        ""
                    )
                    .split_whitespace()
            )
            .is_err()
        );
    }

    #[test]
    fn experimental_operator_pins_reject_every_changed_monetary_identity() {
        let args = experimental_args();
        let scope = KagemushaTestnetExperimentScopeV1 {
            asset_identity_digest: [0x33; 32],
            asset_incarnation: [0x44; 32],
            asset_scale: 2,
            liability_pool_id: [0x55; 32],
        };
        validate_experimental_operator_pins_v1(&args.pins, [0x11; 32], [0x22; 32], scope)
            .expect("all signed identities match independent pins");
        assert!(
            validate_experimental_operator_pins_v1(&args.pins, [0x12; 32], [0x22; 32], scope)
                .is_err()
        );
        assert!(
            validate_experimental_operator_pins_v1(&args.pins, [0x11; 32], [0x23; 32], scope)
                .is_err()
        );
        for changed in [
            KagemushaTestnetExperimentScopeV1 {
                asset_identity_digest: [0x34; 32],
                ..scope
            },
            KagemushaTestnetExperimentScopeV1 {
                asset_incarnation: [0x45; 32],
                ..scope
            },
            KagemushaTestnetExperimentScopeV1 {
                asset_scale: 3,
                ..scope
            },
            KagemushaTestnetExperimentScopeV1 {
                liability_pool_id: [0x56; 32],
                ..scope
            },
        ] {
            assert!(
                validate_experimental_operator_pins_v1(&args.pins, [0x11; 32], [0x22; 32], changed)
                    .is_err()
            );
        }
        let mut malformed = args;
        malformed.pins.expected_asset_identity_digest = "AA".repeat(32);
        assert!(
            validate_experimental_operator_pins_v1(&malformed.pins, [0x11; 32], [0x22; 32], scope)
                .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn experimental_signing_key_requires_owner_only_canonical_custody() {
        let parent = fs::canonicalize(std::env::temp_dir()).expect("canonical temporary parent");
        let directory = tempfile::Builder::new()
            .prefix(".kagemusha-approval-key-")
            .tempdir_in(parent)
            .expect("private key fixture directory");
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
            .expect("harden fixture directory");
        let key = KeyPair::random();
        let key_path = directory.path().join("signer.key");
        let canonical = Zeroizing::new(
            format!("{}\n", ExposedPrivateKey(key.private_key().clone())).into_bytes(),
        );
        crate::secure_fs::write_private_file_atomic(&key_path, canonical.as_slice())
            .expect("write owner-only key fixture");
        assert_eq!(
            load_experimental_signing_key_v1(&key_path)
                .expect("load canonical signer")
                .public_key(),
            key.public_key()
        );
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o644))
            .expect("weaken fixture custody");
        assert!(load_experimental_signing_key_v1(&key_path).is_err());
    }

    #[test]
    fn parser_rejects_epoch_key_derivation_commands() {
        use clap::{Parser as _, error::ErrorKind};

        for command in [
            "derive-mint-finality-next-epoch-v1",
            "derive-mint-finality-epoch-schedule-v1",
        ] {
            let error = crate::Cli::try_parse_from(["kagami", "kagemusha", command])
                .err()
                .expect("epoch-specific public key commands must not remain available");
            assert_eq!(error.kind(), ErrorKind::InvalidSubcommand);
        }
    }

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
    fn provider_issuer_signature_projection_requires_exact_raw_tuple_width() {
        let raw = norito::json!({ "issuer_signature": (vec![vec![1_u8; 64]]) });
        let normalized = normalize_release_projection_value(raw).unwrap();
        assert_eq!(
            normalized
                .get("issuer_signature")
                .and_then(JsonValue::as_str),
            Some("01".repeat(64).as_str())
        );
        for value in [
            norito::json!({ "issuer_signature": (vec![vec![1_u8; 63]]) }),
            norito::json!({ "issuer_signature": (vec![vec![1_u8; 65]]) }),
            norito::json!({ "issuer_signature": [] }),
            norito::json!({ "issuer_signature": [[1_u8], [1_u8]] }),
        ] {
            assert!(normalize_release_projection_value(value).is_err());
        }
    }

    #[test]
    fn app_attestation_policy_digest_projection_is_exact_hex() {
        let raw = norito::json!({
            "app_attestation_authority_policy_digest": vec![0x5a_u8; 32],
        });
        let normalized = normalize_release_projection_value(raw).unwrap();
        assert_eq!(
            normalized
                .get("app_attestation_authority_policy_digest")
                .and_then(JsonValue::as_str),
            Some("5a".repeat(32).as_str()),
        );
        assert!(
            normalize_release_projection_value(norito::json!({
                "app_attestation_authority_policy_digest": vec![0x5a_u8; 31],
            }))
            .is_err(),
        );
    }

    #[test]
    fn authority_projection_rejects_corruption_and_noncanonical_json() {
        assert!(decode_authority_review_projection_json_v1(b"{not-json}").is_err());
        assert!(decode_authority_review_projection_json_v1(b"{}").is_err());
        assert!(decode_authority_review_projection_json_v1(b"{}\n").is_ok());
    }

    #[test]
    fn authority_projection_contract_rejects_release_purpose_confusion() {
        assert_eq!(
            authority_review_contract_v1(
                AuthorityReviewPurposeV1::Production,
                KagemushaReleasePurposeV1::Production,
            )
            .expect("production contract"),
            (
                AUTHORITY_REVIEW_PROJECTION_SCHEMA_V1,
                AUTHORITY_REVIEW_VERIFICATION_SCOPE_V1,
            )
        );
        let experimental =
            KagemushaReleasePurposeV1::TestnetExperiment(KagemushaTestnetExperimentScopeV1 {
                asset_identity_digest: [1; 32],
                asset_incarnation: [2; 32],
                asset_scale: 2,
                liability_pool_id: [3; 32],
            });
        assert_eq!(
            authority_review_contract_v1(
                AuthorityReviewPurposeV1::TestnetExperiment,
                experimental,
            )
            .expect("experimental contract"),
            (
                TESTNET_AUTHORITY_REVIEW_PROJECTION_SCHEMA_V1,
                TESTNET_AUTHORITY_REVIEW_VERIFICATION_SCOPE_V1,
            )
        );
        assert!(
            authority_review_contract_v1(AuthorityReviewPurposeV1::Production, experimental)
                .is_err()
        );
        assert!(
            authority_review_contract_v1(
                AuthorityReviewPurposeV1::TestnetExperiment,
                KagemushaReleasePurposeV1::Production,
            )
            .is_err()
        );
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
