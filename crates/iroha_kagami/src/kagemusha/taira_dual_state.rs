//! Typed BOI Taira dual-state resource generation and semantic validation.
//!
//! This command group is deliberately fail closed. Decoding a JSON shape is
//! only an input boundary; a semantic report is emitted only after the
//! instructions have executed through the pinned Iroha executor and the
//! resulting state has been verified.

mod canonical_closure;
mod source_closure;

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    fs::OpenOptions,
    io::Write,
    num::NonZeroU64,
    os::unix::ffi::OsStrExt as _,
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, Stdio},
};

use clap::{Args as ClapArgs, Subcommand};
use color_eyre::eyre::{WrapErr as _, bail, eyre};
use iroha_config::{
    base::{read::ConfigReader, toml::TomlSource, util::Emitter},
    parameters::{actual, user},
};
use iroha_core::{
    offline_readiness::{
        ensure_mandatory_offline_ready, evaluate_committed_mandatory_offline,
        mandatory_offline_policy_from_reviewed_public_inputs,
    },
    query::store::LiveQueryStore,
    smartcontracts::isi::offline::KagemushaReleaseCatalogV4,
    state::{State, StateBlock, World, WorldReadOnly},
};
use iroha_crypto::{Hash, PublicKey, Signature};
use iroha_data_model::{
    ChainId, Identifiable as _,
    account::{Account, NewAccount, address::ChainDiscriminantGuard},
    alias_setup::{
        AccountAliasRoleV1, AccountProvisionV1, AliasAccountIntentV1, AliasDataSpaceIntentV1,
        AliasDomainIntentV1, AliasIntentV1, AliasLeaseAcquisitionV1, AliasQuoteGuardV1,
        ResolvedAccountAliasV1, ResolvedDataSpaceV1, ResolvedDomainV1,
    },
    asset::{
        AssetBalancePolicy, AssetDefinitionAlias, AssetId,
        definition::{AssetConfidentialPolicy, Mintable, NewAssetDefinition},
    },
    block::BlockHeader,
    confidential::ConfidentialStatus,
    governance::types::{EnactmentSignatureScheme, ParliamentEnactmentCertificate},
    isi::{
        Grant, InstructionBox, Mint, Register, SetAssetDefinitionAlias,
        alias_setup::EnsureAlias,
        domain_link::SetAccountAliasBinding,
        offline::{ActivateKagemushaRecursiveReleaseV4, EnactOfflineAssetBootstrapV1},
        verifying_keys::RegisterVerifyingKey,
        zk::{RegisterZkAsset, ZkAssetMode},
    },
    metadata::Metadata,
    nexus::DataSpaceId,
    offline::{OfflineStatus, kagemusha_recursive_spend_release_sha256, offline_escrow_account_id},
    peer::PeerId,
    permission::Permission,
    prelude::{AccountId, AssetDefinitionId, DomainId},
    proof::{VerifyingKeyId, VerifyingKeyRecord},
    zk::BackendTag,
};
use iroha_genesis::{RawGenesisTransaction, genesis_instructions_json};
use iroha_primitives::{
    json::Json,
    numeric::{NumericSpec, Quantity},
};

use crate::{Outcome, RunArgs};

const SCHEMA_VERSION: u8 = 1;
#[cfg(test)]
const REVIEWED_IROHA_COMMIT: &str = "2ec519cb54180104bdef3a2172a73ec75b7ee0fc";
const TAIRA_CHAIN_ID: &str = "fc56984b-2be7-431d-840e-21514d1883f0";
const TAIRA_CHAIN_DISCRIMINANT: u16 = 369;
const IS_DATASPACE_ID: u64 = 6_647_857_470_246_403_404;
const IS2_DATASPACE_ID: u64 = 8_477_022_798_449_861_195;
const IS_ASSET_ID: &str = "56HTweMpySR2JErjpkisQ2FBTGnN";
const IS2_ASSET_ID: &str = "7qLPT84S9kGBDsodp8oK6pRJTY5p";
const IS2_MANIFEST_HASH: &str = "4be27d6e526fa47522b2865462b79d228450d02fb3b63b011fb8731932405c2b";
const REVIEWED_BASE_GENESIS_SHA256: &str =
    "3145a17a5fc9ddad4f9750d402b4b01a0eb267fea83f66fba68815203cb279c1";
const REVIEWED_BASE_CONFIG_SHA256: &str =
    "ca18c2ab9ebfe876bc3e74b77393572cc08ff2e019fc827a31500b45780743dd";
const RESOURCE_FILE_PHASE_ONE: &str = "phase-one-is2-resources.json";
const RESOURCE_FILE_PHASE_TWO: &str = "phase-two-is-resources.json";
const GOVERNANCE_FILE_PHASE_ONE: &str = "phase-one-alias-lease-governance.json";
const GOVERNANCE_FILE_PHASE_TWO: &str = "phase-two-alias-lease-governance.json";
const ROOT_PUBLISHED_GENERATED_CANDIDATE_SCHEMA: &str =
    "iroha.kagemusha.root_published_generated_candidate.v1";
const ROOT_ATOMIC_PUBLICATION_PROTOCOL: &str =
    "iroha.kagemusha.distinct_uid_root_atomic_publish.v1";
const ROOT_PUBLISHED_GENERATION_STATUS: &str = "root_published_boi_generation_output";
const PROVISIONAL_GENERATION_STATUS: &str = "provisional_boi_generation_worker_output";
const PROVISIONAL_CROSS_STAGE_STATUS: &str =
    "blocked_pending_root_descriptor_copy_atomic_publication_receipt";
const CANDIDATE_BUILD_RECEIPT_SCHEMA: &str = "iroha.kagemusha.root_published_candidate_build.v1";
const CANDIDATE_BUILD_REPORT_SCHEMA: &str = "iroha.kagemusha.sealed_candidate_build.v1";
const GENERATION_WORKER_LAUNCH_SCHEMA: &str = "boi.taira.generation_worker_launch.v1";
const PRODUCTION_CLOSURE_SCHEMA: &str = "iroha.kagemusha.production_build_closure.v1";
const PRODUCTION_PROVISIONING_PROTOCOL: &str = "iroha.kagemusha.root_private_atomic_publish.v1";
const CANDIDATE_BUILD_RECEIPT_NAME: &[u8] = b"root-published-candidate-build.json";
const CANDIDATE_BUILD_BINARY_NAME: &[u8] = b"kagemusha_recursive_spend_v4_bundle";
const CANDIDATE_BUILD_REPORT_NAME: &[u8] = b"sealed-kagemusha-candidate-build.json";
const GENERATED_ROOT_RECEIPT_NAME: &[u8] = b"root-published-generated-candidate.json";
const GENERATION_WORKER_LAUNCH_NAME: &[u8] = b"generation-worker-launch.json";
const CANDIDATE_GENERATED_FILES: [&[u8]; 12] = [
    b"candidate-manifest.json",
    b"candidate-manifest.norito",
    b"candidate-manifest.norito.sha256",
    b"step-eq.params-ipa.krv4",
    b"step-eq.proving-key.krv4",
    b"step-eq.verifying-key.krv4",
    b"step-eq.bootstrap-witness.krv4",
    b"step-ep.params-ipa.krv4",
    b"step-ep.proving-key.krv4",
    b"step-ep.verifying-key.krv4",
    b"step-ep.bootstrap-witness.krv4",
    b"topup-finality-roster-v4.norito",
];
const GENERATION_REPORT_FILES: [&[u8]; 2] = [
    b"kagemusha_resource.jsonl",
    b"kagemusha_resource_summary.json",
];
const MINIMUM_GENERATION_STORAGE_BYTES: u64 = 64 * 1024_u64.pow(3);
const MINIMUM_GENERATION_RESERVE_BYTES: u64 = 16 * 1024_u64.pow(3);
const MAX_RECORDED_STORAGE_BYTES: u64 = 1024_u64.pow(6);
const SEMANTIC_TRUST_AUTHORITY_FINGERPRINT: &str = "9D1C8BFA5A0C1FEF5A8B1E5F552C2D0FD7C40BEB";
const SEMANTIC_TRUST_PUBLIC_KEY_SHA256: &str =
    "f446aafe7de3c8294900a81a987750dc1fb541365499e5c27c011ca31d624acc";
const FI_NAMES: [&str; 7] = [
    "leumi",
    "hapoalim",
    "discount",
    "mizrahi",
    "fibi",
    "onezero",
    "jerusalem",
];
const IS_DOMAINS: [&str; 8] = [
    "boi.is",
    "leumi.is",
    "hapoalim.is",
    "discount.is",
    "mizrahi.is",
    "fibi.is",
    "onezero.is",
    "jerusalem.is",
];
const BASE_VERIFIER_ROLES: [&str; 3] = [
    "confidential_transfer_v2_verifier_record",
    "kagemusha_topup_shield_v2_verifier_record",
    "confidential_unshield_v3_verifier_record",
];
const RECURSIVE_VERIFIER_ROLES: [&str; 2] = [
    "kagemusha_recursive_step_eq_v4_verifier_record",
    "kagemusha_recursive_step_ep_v4_verifier_record",
];
const OFFLINE_ISSUER_PERMISSIONS: [&str; 3] = [
    "CanManageOfflineEscrow",
    "CanActivateKagemushaRecursiveReleaseV4",
    "CanManageOfflineDeviceAttestationPolicy",
];

/// BOI Taira dual-state resource command group.
#[derive(Debug, ClapArgs)]
pub(super) struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Generate canonical typed `.is2` and `.is` resource instruction files.
    #[command(name = "generate-resources")]
    GenerateResources(GenerateResourcesArgs),
    /// Execute and validate the exact two-phase candidate, then emit a bound report.
    #[command(name = "validate-semantic")]
    ValidateSemantic(ValidateSemanticArgs),
}

#[derive(Debug, ClapArgs)]
struct GenerateResourcesArgs {
    /// Independently pinned exact source closure used to build this Kagami binary.
    #[command(flatten)]
    reviewed_source: ReviewedSourceArgs,
    /// Secret-free, operator-supplied typed resource specification.
    #[arg(long)]
    spec: PathBuf,
    /// New directory receiving canonical resource instruction files.
    #[arg(long)]
    output_dir: PathBuf,
}

#[derive(Debug, ClapArgs)]
struct ValidateSemanticArgs {
    /// Independently pinned exact source closure used to build this Kagami binary.
    #[command(flatten)]
    reviewed_source: ReviewedSourceArgs,
    /// Secret-free operator specification used to derive the exact expected resources.
    #[arg(long)]
    spec: PathBuf,
    /// Exact reviewed raw Taira genesis manifest.
    #[arg(long)]
    base_genesis: PathBuf,
    /// Exact reviewed Taira node configuration.
    #[arg(long)]
    base_config: PathBuf,
    /// Canonical Kagemusha V4 release policy.
    #[arg(long)]
    release_policy: PathBuf,
    /// Root containing authenticated Kagemusha V4 release directories.
    #[arg(long)]
    artifact_root: PathBuf,
    /// Generated `.is2` phase-one resource instruction array.
    #[arg(long)]
    phase_one_resources: PathBuf,
    /// Exact atomic ABI-21/V4 activation instruction array.
    #[arg(long)]
    phase_one_activation: PathBuf,
    /// Generated `.is` phase-two resource instruction array.
    #[arg(long)]
    phase_two_resources: PathBuf,
    /// Dual-asset offline runtime-transition artifact.
    #[arg(long)]
    runtime_transition: PathBuf,
    /// Exact reviewed `.is2` manifest authorization.
    #[arg(long)]
    is2_manifest_authorization: PathBuf,
    /// Exact reviewed approval trust bundle.
    #[arg(long)]
    approval_trust_bundle: PathBuf,
    /// Canonical four-validator, public-key signed phase-one readiness evidence.
    #[arg(long)]
    phase_one_readiness_evidence: PathBuf,
    /// Canonical four-validator, public-key signed final readiness evidence.
    #[arg(long)]
    final_readiness_evidence: PathBuf,
    /// Independently reviewed artifact and exact h1/h3 identity pins.
    #[arg(long)]
    semantic_trust: PathBuf,
    /// Externally recorded SHA-256 of the exact semantic-trust descriptor bytes.
    #[arg(long)]
    semantic_trust_sha256: String,
    /// Detached OpenPGP signature over the exact semantic-trust descriptor bytes.
    #[arg(long)]
    semantic_trust_signature: PathBuf,
    /// Exact binary public-key export for the pinned BOI/Taira operator authority.
    #[arg(long)]
    semantic_trust_public_key: PathBuf,
    /// Compatibility assertion for the exact OpenPGP verifier path authenticated
    /// by the root-owned production-closure provenance.
    #[arg(long)]
    gpgv: PathBuf,
    /// Compatibility assertion for the verifier SHA-256 authenticated by the
    /// root-owned production-closure provenance.
    #[arg(long)]
    gpgv_sha256: String,
    /// Root-published generated-candidate/source-seal receipt.
    #[arg(long)]
    root_published_generated_candidate_receipt: PathBuf,
    /// Immutable production build closure whose tree is pinned by the publication receipt.
    #[arg(long)]
    production_closure_root: PathBuf,
    /// New path receiving `taira-dual-state-semantic-validation-v1`.
    #[arg(long)]
    output: PathBuf,
}

#[derive(Debug, Clone, ClapArgs)]
struct ReviewedSourceArgs {
    /// Exact canonical root of the source repository used by this build.
    #[arg(long)]
    source_root: PathBuf,
    /// Canonical independently reviewed full-source closure descriptor.
    #[arg(long)]
    reviewed_source_closure: PathBuf,
    /// External SHA-256 pin of the reviewed source closure descriptor bytes.
    #[arg(long)]
    reviewed_source_closure_sha256: String,
    /// External exact source commit pin; never inferred from this binary.
    #[arg(long)]
    expected_source_commit: String,
    /// External exact full-source tree SHA-256 pin.
    #[arg(long)]
    expected_source_tree_sha256: String,
}

#[derive(Debug, Clone, crate::json_macros::JsonSerialize, crate::json_macros::JsonDeserialize)]
struct ResourceSpecV1 {
    schema_version: u8,
    chain_id: ChainId,
    chain_discriminant: u16,
    base_genesis_authority: AccountId,
    funding_amount: Quantity,
    offline_command_fee_funding_amount: Quantity,
    phase_one: PhaseResourceSpecV1,
    phase_two: PhaseResourceSpecV1,
    phase_two_offline_bootstrap: EnactOfflineAssetBootstrapV1,
    base_verifiers: Vec<VerifierSpecV1>,
    validator_roster: Vec<ValidatorRosterEntryV1>,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
struct ValidatorRosterEntryV1 {
    validator_public_key: PublicKey,
    torii_port: u16,
}

#[derive(Debug, Clone, crate::json_macros::JsonSerialize, crate::json_macros::JsonDeserialize)]
struct PhaseResourceSpecV1 {
    alias_lease_governance: AliasLeaseGovernanceV1,
    accounts: PhaseAccountsV1,
}

#[derive(Debug, Clone, crate::json_macros::JsonSerialize, crate::json_macros::JsonDeserialize)]
struct AliasLeaseGovernanceV1 {
    execution_context: String,
    transaction_authority: AccountId,
    payment: AliasLeasePaymentV1,
    lifecycle: AliasLeaseLifecycleV1,
}

#[derive(Debug, Clone, crate::json_macros::JsonSerialize, crate::json_macros::JsonDeserialize)]
struct AliasLeasePaymentV1 {
    charge_source: String,
    balance_requirement: String,
    expected_policy_version: u16,
    expected_payment_asset: AssetDefinitionId,
    max_amount_per_resource: Quantity,
    quote_valid_until_ms: u64,
}

#[derive(Debug, Clone, crate::json_macros::JsonSerialize, crate::json_macros::JsonDeserialize)]
struct AliasLeaseLifecycleV1 {
    term_years: u8,
    pricing_class_hint: Option<u8>,
    renewal_mode: String,
    renewal_authority: AccountId,
    renew_before_expiry_ms: u64,
    expiry_behavior: String,
}

#[derive(Debug, Clone, crate::json_macros::JsonSerialize, crate::json_macros::JsonDeserialize)]
struct PhaseAccountsV1 {
    treasury: AccountId,
    escrow: AccountId,
    issuer: AccountId,
    reserves: BTreeMap<String, AccountId>,
}

#[derive(Debug, Clone, crate::json_macros::JsonSerialize, crate::json_macros::JsonDeserialize)]
struct VerifierSpecV1 {
    id: VerifyingKeyId,
    record: VerifyingKeyRecord,
}

#[derive(Debug, Clone, crate::json_macros::JsonSerialize, crate::json_macros::JsonDeserialize)]
struct PhaseOneReadinessEvidenceV1 {
    schema_version: u8,
    kind: String,
    readiness_stage: String,
    reviewed_iroha_commit: String,
    bridge_abi_version: u16,
    release_manifest_version: u8,
    cash_handoff_capability: String,
    phase_one_instructions_sha256: String,
    phase_two_instructions_sha256: Option<String>,
    validators: Vec<SignedValidatorReadinessV1>,
}

#[derive(Debug, Clone, crate::json_macros::JsonSerialize, crate::json_macros::JsonDeserialize)]
struct SignedValidatorReadinessV1 {
    body: ValidatorReadinessBodyV1,
    signature: Signature,
}

#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
    PartialEq,
    Eq,
)]
struct ValidatorReadinessBodyV1 {
    validator_public_key: PublicKey,
    torii_port: u16,
    readiness_stage: String,
    artifact_sha256: String,
    chain_id: ChainId,
    reviewed_iroha_commit: String,
    phase_one_instructions_sha256: String,
    phase_two_instructions_sha256: Option<String>,
    bridge_abi_version: u16,
    release_manifest_version: u8,
    cash_handoff_capability: String,
    asset_aliases: Vec<String>,
    release_policy_sha256: String,
    verifier_commitments: BTreeMap<String, String>,
    roster_root_sha256: String,
    evaluated_block_height: u64,
    evaluated_block_hash: String,
    ready: bool,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
struct BlockIdentityPinV1 {
    evaluated_block_height: u64,
    evaluated_block_hash: String,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
struct SemanticTrustV1 {
    schema_version: u8,
    kind: String,
    expected_validator_artifact_sha256: String,
    phase_one_identity: BlockIdentityPinV1,
    final_identity: BlockIdentityPinV1,
    root_published_generated_candidate_receipt_sha256: String,
    root_published_generated_candidate_tree_sha256: String,
    candidate_tree_sha256: String,
    candidate_build_artifact_tree_sha256: String,
    candidate_build_receipt_sha256: String,
    production_closure_tree_sha256: String,
    reviewed_source_closure_descriptor_sha256: String,
    source_commit: String,
    source_tree_sha256: String,
    toolchain_provenance_sha256: String,
    generation_worker_launch_receipt_sha256: String,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
struct RootPublishedGeneratedCandidateReceiptV1 {
    artifact_root: String,
    artifact_tree_sha256: String,
    build_uid: u64,
    build_user_name: String,
    candidate_build_artifact_tree_sha256: String,
    candidate_build_receipt_path: String,
    candidate_build_receipt_sha256: String,
    candidate_dir_path: String,
    candidate_tree_sha256: String,
    generation_resource_report_path: String,
    generation_resource_report_tree_sha256: String,
    generation_summary_path: String,
    generation_summary_sha256: String,
    production_closure_tree_sha256: String,
    provisional_cross_stage_status: String,
    provisional_generation_publication_status: String,
    publication_protocol: String,
    publication_status: String,
    reviewed_source_closure_descriptor_sha256: String,
    schema: String,
    source_commit: String,
    source_tree_sha256: String,
    toolchain_provenance_sha256: String,
    worker_launch_receipt_sha256: String,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
struct CandidateBuildReceiptV1 {
    artifact_root: String,
    artifact_tree_sha256: String,
    binary_path: String,
    binary_sha256: String,
    binary_size_bytes: u64,
    build_uid: u64,
    build_user_name: String,
    production_closure_tree_sha256: String,
    publication_protocol: String,
    reviewed_source_closure_descriptor_sha256: String,
    schema: String,
    sealed_build_report_path: String,
    sealed_build_report_sha256: String,
    source_commit: String,
    source_tree_sha256: String,
    toolchain_provenance_sha256: String,
}

#[derive(
    Debug, Clone, PartialEq, crate::json_macros::JsonSerialize, crate::json_macros::JsonDeserialize,
)]
struct CandidateBuildReportV1 {
    apple_developer_dir_path: String,
    apple_sdk_path: String,
    binary_path: String,
    binary_sha256: String,
    binary_size_bytes: u64,
    build_profile: String,
    build_uid: u64,
    build_user_name: String,
    cargo_home_path: String,
    cargo_path: String,
    cargo_sha256: String,
    cargo_vendor_path: String,
    clang_resource_dir_path: String,
    git_exec_path: String,
    git_path: String,
    git_sha256: String,
    gpg_path: String,
    gpg_sha256: String,
    linker_path: String,
    linker_sha256: String,
    minimum_build_physical_memory_bytes: u64,
    physical_memory_bytes_at_admission: u64,
    production_closure_root: String,
    production_closure_tree_sha256: String,
    publication_status: String,
    python_path: String,
    python_sha256: String,
    reviewed_source_closure: norito::json::Value,
    reviewed_source_closure_descriptor_sha256: String,
    rustc_path: String,
    rustc_sha256: String,
    rustc_sysroot_path: String,
    schema: String,
    source_commit: String,
    source_repo_dirty: bool,
    source_signing_key_fingerprint: String,
    source_tree_sha256: String,
    target_dir: String,
    toolchain_provenance_sha256: String,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
struct GenerationWorkerLaunchReceiptV1 {
    build_uid: u64,
    build_user_name: String,
    candidate_build_receipt_path: String,
    candidate_build_receipt_sha256: String,
    candidate_output_leaf: String,
    generation_command_sha256: String,
    resource_report_leaf: String,
    schema: String,
    storage_available_bytes_after_generation: u64,
    storage_available_bytes_at_admission: u64,
    storage_device: u64,
    storage_minimum_available_bytes: u64,
    storage_post_build_reserve_bytes: u64,
    worker_root: String,
    worker_root_device: u64,
    worker_root_inode: u64,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
struct ProductionClosureProvenanceV1 {
    apple_developer_dir_path: String,
    apple_sdk_path: String,
    cargo_home_path: String,
    cargo_path: String,
    cargo_sha256: String,
    cargo_vendor_path: String,
    clang_resource_dir_path: String,
    closure_root: String,
    closure_tree_sha256: String,
    git_exec_path: String,
    git_path: String,
    git_sha256: String,
    gnupghome_path: String,
    gpg_path: String,
    gpg_sha256: String,
    linker_path: String,
    linker_sha256: String,
    provisioning_protocol: String,
    python_path: String,
    python_sha256: String,
    reviewed_source_closure_path: String,
    rustc_path: String,
    rustc_sha256: String,
    rustc_sysroot_path: String,
    schema: String,
    source_root: String,
    source_signing_key_fingerprint: String,
}

#[derive(Debug, Clone, crate::json_macros::JsonSerialize, crate::json_macros::JsonDeserialize)]
struct RuntimeTransitionV1 {
    schema_version: u8,
    kind: String,
    base_config_sha256: String,
    escrow_accounts: BTreeMap<AssetDefinitionId, AccountId>,
    release_policy_sha256: String,
    phase_one_artifact_manifest_sha256: String,
    phase_two_artifact_manifest_sha256: String,
    apply_after_phase_two_commit: bool,
    coordinated_four_validator_restart_required: bool,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
struct Is2ManifestAuthorizationBodyV1 {
    schema_version: u8,
    kind: String,
    reviewed_iroha_commit: String,
    chain_id: ChainId,
    base_genesis_sha256: String,
    phase_one_instructions_sha256: String,
    authority: AccountId,
}

#[derive(Debug, Clone, crate::json_macros::JsonSerialize, crate::json_macros::JsonDeserialize)]
struct SignedIs2ManifestAuthorizationV1 {
    body: Is2ManifestAuthorizationBodyV1,
    signature: Signature,
}

#[derive(Debug, Clone, crate::json_macros::JsonSerialize, crate::json_macros::JsonDeserialize)]
struct ApprovalTrustBundleV1 {
    schema_version: u8,
    kind: String,
    reviewed_iroha_commit: String,
    chain_id: ChainId,
    base_genesis_sha256: String,
    phase_two_instructions_sha256: String,
    manifest_fingerprint: [u8; 32],
    certificate: ParliamentEnactmentCertificate,
}

#[derive(Debug, Clone, crate::json_macros::JsonSerialize, crate::json_macros::JsonDeserialize)]
struct SemanticValidationReportV1 {
    schema_version: u8,
    kind: String,
    status: String,
    validator: SemanticValidatorV1,
    bindings: SemanticBindingsV1,
    checks: SemanticChecksV1,
}

#[derive(Debug, Clone, crate::json_macros::JsonSerialize, crate::json_macros::JsonDeserialize)]
struct SemanticValidatorV1 {
    tool: String,
    iroha_commit: String,
    binary_sha256: String,
}

#[derive(Debug, Clone, crate::json_macros::JsonSerialize, crate::json_macros::JsonDeserialize)]
struct SemanticBindingsV1 {
    chain_id: ChainId,
    chain_discriminant: u16,
    reviewed_final_commit: String,
    base_genesis_sha256: String,
    base_config_sha256: String,
    is2_manifest_authorization_sha256: String,
    is2_manifest_hash: String,
    is2_dataspace_id: String,
    phase_one_resources_sha256: String,
    phase_one_alias_lease_governance_sha256: String,
    phase_one_activation_sha256: String,
    phase_one_instructions_sha256: String,
    phase_two_resources_sha256: String,
    phase_two_alias_lease_governance_sha256: String,
    runtime_transition_sha256: String,
    approval_trust_bundle_sha256: String,
    expected_validator_artifact_sha256: String,
    phase_one_readiness_evidence_sha256: String,
    phase_one_evaluated_block_height: u64,
    phase_one_evaluated_block_hash: String,
    final_readiness_evidence_sha256: String,
    final_evaluated_block_height: u64,
    final_evaluated_block_hash: String,
    semantic_trust_sha256: String,
    semantic_trust_signature_sha256: String,
    semantic_trust_public_key_sha256: String,
    gpgv_sha256: String,
    root_published_generated_candidate_receipt_sha256: String,
    root_published_generated_candidate_tree_sha256: String,
    candidate_tree_sha256: String,
    candidate_build_artifact_tree_sha256: String,
    candidate_build_receipt_sha256: String,
    production_closure_tree_sha256: String,
    reviewed_source_closure_descriptor_sha256: String,
    source_commit: String,
    source_tree_sha256: String,
    toolchain_provenance_sha256: String,
    generation_worker_launch_receipt_sha256: String,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
struct SemanticChecksV1 {
    canonical_iroha_instructions: bool,
    base_genesis_exact_is2_domains: bool,
    base_genesis_contains_no_is_domains: bool,
    phase_one_contains_no_is_namespace_mutation: bool,
    phase_two_contains_no_is2_namespace_mutation: bool,
    is2_asset_fixed_scale: u8,
    is_asset_fixed_scale: u8,
    cash_handoff_capability: String,
    bridge_abi_version: u16,
    release_manifest_version: u8,
    proof_backend: String,
    recursive_lineage: bool,
    five_distinct_verifier_roles: bool,
    device_attestation_policy_governed: bool,
    spend_authority_hardware_backed: bool,
    issuer_authorized_and_funded: bool,
    all_fi_reserves_funded: bool,
    sns_aliases_use_governed_ensure_alias: bool,
    sns_alias_transaction_authority_is_payer: bool,
    sns_alias_authority_prefunded_for_aggregate_cap: bool,
    sns_alias_lifecycle_is_explicit_and_fail_closed: bool,
    account_alias_ensure_immediately_precedes_legacy_binding: bool,
    phase_two_domains_use_dependency_ordered_ensure_alias: bool,
    phase_two_requires_phase_one_four_validator_readiness: bool,
}

struct SemanticReadinessOutcome {
    phase_one: OfflineStatus,
    final_state: OfflineStatus,
}

struct SemanticTrustAuthorization {
    signature_sha256: String,
    public_key_sha256: String,
    gpgv_sha256: String,
}

struct AuthenticatedSemanticTrust {
    trust: SemanticTrustV1,
    authorization: SemanticTrustAuthorization,
}

struct OpenPgpVerifierAnchor {
    path: PathBuf,
    sha256: String,
    verifier: canonical_closure::StableFile,
}

struct CapturedInput<T> {
    value: T,
    sha256: String,
}

struct CapturedBytes {
    bytes: Vec<u8>,
    sha256: String,
}

impl<T: Write> RunArgs<T> for Args {
    fn run(self, writer: &mut std::io::BufWriter<T>) -> Outcome {
        match self.command {
            Command::GenerateResources(args) => {
                let source_identity = validate_reviewed_source_args(&args.reviewed_source)?;
                let loaded = read_resource_spec(&args.spec)?;
                let resources = generate_resources(&loaded.spec)?;
                let output_dir = create_new_private_directory(&args.output_dir)?;
                let phase_one_bytes = instruction_bytes(&resources.phase_one)?;
                let phase_two_bytes = instruction_bytes(&resources.phase_two)?;
                let phase_one_governance =
                    typed_json_bytes(&loaded.spec.phase_one.alias_lease_governance)?;
                let phase_two_governance =
                    typed_json_bytes(&loaded.spec.phase_two.alias_lease_governance)?;
                write_new_private_file(
                    &output_dir.join(RESOURCE_FILE_PHASE_ONE),
                    &phase_one_bytes,
                )?;
                write_new_private_file(
                    &output_dir.join(RESOURCE_FILE_PHASE_TWO),
                    &phase_two_bytes,
                )?;
                write_new_private_file(
                    &output_dir.join(GOVERNANCE_FILE_PHASE_ONE),
                    &phase_one_governance,
                )?;
                write_new_private_file(
                    &output_dir.join(GOVERNANCE_FILE_PHASE_TWO),
                    &phase_two_governance,
                )?;
                writeln!(
                    writer,
                    "{{\"status\":\"generated\",\"schema_version\":1,\"reviewed_iroha_commit\":\"{}\",\"reviewed_source_tree_sha256\":\"{}\",\"reviewed_source_closure_descriptor_sha256\":\"{}\",\"phase_one_instruction_count\":{},\"phase_one_resources_sha256\":\"{}\",\"phase_two_instruction_count\":{},\"phase_two_resources_sha256\":\"{}\"}}",
                    source_identity.source_commit,
                    source_identity.source_tree_sha256,
                    source_identity.descriptor_sha256,
                    resources.phase_one.len(),
                    sha256_hex(&phase_one_bytes),
                    resources.phase_two.len(),
                    sha256_hex(&phase_two_bytes),
                )?;
                Ok(())
            }
            Command::ValidateSemantic(args) => {
                let source_identity = validate_reviewed_source_args(&args.reviewed_source)?;
                let loaded = read_resource_spec(&args.spec)?;
                let expected = generate_resources(&loaded.spec)?;
                let phase_one = read_instruction_file(
                    &args.phase_one_resources,
                    "phase-one resource instructions",
                )?;
                let activation = read_instruction_file(
                    &args.phase_one_activation,
                    "phase-one activation instructions",
                )?;
                let phase_two = read_instruction_file(
                    &args.phase_two_resources,
                    "phase-two resource instructions",
                )?;
                if phase_one != expected.phase_one {
                    bail!(
                        "phase-one resources differ from the exact typed resources derived from \
                         --spec"
                    );
                }
                if phase_two != expected.phase_two {
                    bail!(
                        "phase-two resources differ from the exact typed resources derived from \
                         --spec"
                    );
                }
                let phase_one_bytes = instruction_bytes(&phase_one)?;
                let activation_bytes = instruction_bytes(&activation)?;
                let phase_two_bytes = instruction_bytes(&phase_two)?;
                let phase_one_resources_sha256 = sha256_hex(&phase_one_bytes);
                let phase_one_activation_sha256 = sha256_hex(&activation_bytes);
                let phase_two_resources_sha256 = sha256_hex(&phase_two_bytes);
                let mut combined_phase_one = phase_one.clone();
                combined_phase_one.extend(activation.iter().cloned());
                let phase_one_instructions_sha256 =
                    sha256_hex(&instruction_bytes(&combined_phase_one)?);
                let mut verifier_anchor =
                    load_openpgp_verifier_anchor(&args.production_closure_root)?;
                let authenticated_semantic_trust =
                    validate_semantic_trust_authorization(&args, &mut verifier_anchor)?;
                let semantic_trust_authorization = authenticated_semantic_trust.authorization;
                let semantic_trust = authenticated_semantic_trust.trust;
                let root_published_receipt = read_root_published_generated_candidate_receipt(
                    &args.root_published_generated_candidate_receipt,
                )?;
                validate_semantic_trust_and_root_receipt(
                    &semantic_trust,
                    &root_published_receipt.value,
                    &root_published_receipt.sha256,
                    &args.root_published_generated_candidate_receipt,
                    &args.production_closure_root,
                    &source_identity,
                )?;
                let phase_one_readiness =
                    read_phase_one_readiness_evidence(&args.phase_one_readiness_evidence)?;
                let final_readiness =
                    read_phase_one_readiness_evidence(&args.final_readiness_evidence)?;
                let mut release_policy =
                    canonical_closure::stable_file(&args.release_policy, 64 * 1024 * 1024)
                        .wrap_err("failed to capture exact Kagemusha release policy")?;
                let release_policy_sha256 = release_policy.sha256().to_owned();
                let release_policy_bytes = release_policy
                    .read_bytes(64 * 1024 * 1024)
                    .wrap_err("failed to read captured Kagemusha release policy")?;
                validate_phase_one_readiness_evidence(
                    &phase_one_readiness.value,
                    &loaded.spec.validator_roster,
                    &phase_one_instructions_sha256,
                    &release_policy_sha256,
                    &activation,
                    &loaded.spec,
                    &semantic_trust,
                    &source_identity.source_commit,
                )?;
                validate_final_readiness_evidence(
                    &final_readiness.value,
                    &loaded.spec.validator_roster,
                    &phase_one_instructions_sha256,
                    &phase_two_resources_sha256,
                    &release_policy_sha256,
                    &loaded.spec,
                    &semantic_trust,
                    &source_identity.source_commit,
                )?;
                let runtime_transition = read_runtime_transition(&args.runtime_transition)?;
                validate_activation_and_runtime_transition(
                    &activation,
                    &runtime_transition.value,
                    &loaded.spec,
                    &release_policy_sha256,
                )?;
                let catalog = KagemushaReleaseCatalogV4::load_from_policy_bytes(
                    &release_policy_bytes,
                    &args.artifact_root,
                )
                .map_err(|error| {
                    eyre!("failed to authenticate the exact Kagemusha V4 release catalog: {error}")
                })?;
                let base_genesis = capture_bytes(
                    &args.base_genesis,
                    256 * 1024 * 1024,
                    "reviewed base genesis",
                )?;
                validate_exact_digest(
                    &base_genesis.sha256,
                    REVIEWED_BASE_GENESIS_SHA256,
                    "reviewed base genesis",
                )?;
                let base_config =
                    capture_bytes(&args.base_config, 16 * 1024 * 1024, "reviewed base config")?;
                validate_exact_digest(
                    &base_config.sha256,
                    REVIEWED_BASE_CONFIG_SHA256,
                    "reviewed base config",
                )?;
                let is2_authorization =
                    read_is2_manifest_authorization(&args.is2_manifest_authorization)?;
                validate_is2_manifest_authorization(
                    &is2_authorization.value,
                    &loaded.spec,
                    &phase_one_instructions_sha256,
                    &source_identity.source_commit,
                )?;
                let approval_trust_bundle =
                    read_approval_trust_bundle(&args.approval_trust_bundle)?;
                validate_approval_trust_bundle(
                    &approval_trust_bundle.value,
                    &loaded.spec,
                    &phase_two_resources_sha256,
                    &source_identity.source_commit,
                )?;
                let readiness_outcome = execute_candidate_semantics(
                    &loaded.spec,
                    &base_genesis.bytes,
                    &args.base_genesis,
                    &base_config.bytes,
                    &args.base_config,
                    &args.release_policy,
                    &args.artifact_root,
                    catalog,
                    &phase_one,
                    &activation,
                    &phase_two,
                )?;
                let checks = semantic_checks_from_readiness(&readiness_outcome)?;
                let mut executing_image =
                    canonical_closure::stable_executing_image(1024 * 1024 * 1024)
                        .wrap_err("failed to authenticate the exact running Kagami image")?;
                let executing_image_sha256 = executing_image.sha256().to_owned();
                let report = SemanticValidationReportV1 {
                    schema_version: SCHEMA_VERSION,
                    kind: "taira-dual-state-semantic-validation-v1".to_owned(),
                    status: "PASS".to_owned(),
                    validator: SemanticValidatorV1 {
                        tool: "kagami".to_owned(),
                        iroha_commit: source_identity.source_commit.clone(),
                        binary_sha256: executing_image_sha256,
                    },
                    bindings: SemanticBindingsV1 {
                        chain_id: loaded.spec.chain_id,
                        chain_discriminant: TAIRA_CHAIN_DISCRIMINANT,
                        reviewed_final_commit: source_identity.source_commit.clone(),
                        base_genesis_sha256: REVIEWED_BASE_GENESIS_SHA256.to_owned(),
                        base_config_sha256: REVIEWED_BASE_CONFIG_SHA256.to_owned(),
                        is2_manifest_authorization_sha256: is2_authorization.sha256,
                        is2_manifest_hash: IS2_MANIFEST_HASH.to_owned(),
                        is2_dataspace_id: IS2_DATASPACE_ID.to_string(),
                        phase_one_resources_sha256,
                        phase_one_alias_lease_governance_sha256: sha256_hex(&typed_json_bytes(
                            &loaded.spec.phase_one.alias_lease_governance,
                        )?),
                        phase_one_activation_sha256,
                        phase_one_instructions_sha256,
                        phase_two_resources_sha256,
                        phase_two_alias_lease_governance_sha256: sha256_hex(&typed_json_bytes(
                            &loaded.spec.phase_two.alias_lease_governance,
                        )?),
                        runtime_transition_sha256: runtime_transition.sha256,
                        approval_trust_bundle_sha256: approval_trust_bundle.sha256,
                        expected_validator_artifact_sha256: semantic_trust
                            .expected_validator_artifact_sha256
                            .clone(),
                        phase_one_readiness_evidence_sha256: phase_one_readiness.sha256,
                        phase_one_evaluated_block_height: semantic_trust
                            .phase_one_identity
                            .evaluated_block_height,
                        phase_one_evaluated_block_hash: semantic_trust
                            .phase_one_identity
                            .evaluated_block_hash
                            .clone(),
                        final_readiness_evidence_sha256: final_readiness.sha256,
                        final_evaluated_block_height: semantic_trust
                            .final_identity
                            .evaluated_block_height,
                        final_evaluated_block_hash: semantic_trust
                            .final_identity
                            .evaluated_block_hash
                            .clone(),
                        semantic_trust_sha256: args.semantic_trust_sha256.clone(),
                        semantic_trust_signature_sha256: semantic_trust_authorization
                            .signature_sha256,
                        semantic_trust_public_key_sha256: semantic_trust_authorization
                            .public_key_sha256,
                        gpgv_sha256: semantic_trust_authorization.gpgv_sha256,
                        root_published_generated_candidate_receipt_sha256: semantic_trust
                            .root_published_generated_candidate_receipt_sha256
                            .clone(),
                        root_published_generated_candidate_tree_sha256: semantic_trust
                            .root_published_generated_candidate_tree_sha256
                            .clone(),
                        candidate_tree_sha256: semantic_trust.candidate_tree_sha256.clone(),
                        candidate_build_artifact_tree_sha256: semantic_trust
                            .candidate_build_artifact_tree_sha256
                            .clone(),
                        candidate_build_receipt_sha256: semantic_trust
                            .candidate_build_receipt_sha256
                            .clone(),
                        production_closure_tree_sha256: semantic_trust
                            .production_closure_tree_sha256
                            .clone(),
                        reviewed_source_closure_descriptor_sha256: source_identity
                            .descriptor_sha256
                            .clone(),
                        source_commit: source_identity.source_commit.clone(),
                        source_tree_sha256: source_identity.source_tree_sha256.clone(),
                        toolchain_provenance_sha256: semantic_trust
                            .toolchain_provenance_sha256
                            .clone(),
                        generation_worker_launch_receipt_sha256: semantic_trust
                            .generation_worker_launch_receipt_sha256
                            .clone(),
                    },
                    checks,
                };
                let report_bytes = typed_json_bytes(&report)?;
                let report_sha256 = sha256_hex(&report_bytes);
                executing_image
                    .verify_unchanged()
                    .wrap_err("running Kagami image changed before report publication")?;
                write_new_private_file(&args.output, &report_bytes)?;
                writeln!(
                    writer,
                    "{{\"status\":\"PASS\",\"schema_version\":1,\"report_sha256\":\"{}\"}}",
                    report_sha256
                )?;
                Ok(())
            }
        }
    }
}

fn validate_reviewed_source_args(
    args: &ReviewedSourceArgs,
) -> color_eyre::Result<source_closure::ValidatedSourceIdentity> {
    source_closure::validate_reviewed_source(
        &args.source_root,
        &args.reviewed_source_closure,
        &args.reviewed_source_closure_sha256,
        &args.expected_source_commit,
        &args.expected_source_tree_sha256,
    )
}

struct LoadedResourceSpec {
    spec: ResourceSpecV1,
}

struct GeneratedResources {
    phase_one: Vec<InstructionBox>,
    phase_two: Vec<InstructionBox>,
}

fn read_resource_spec(path: &Path) -> color_eyre::Result<LoadedResourceSpec> {
    let (bytes, _) = canonical_closure::stable_file_bytes(path, 16 * 1024 * 1024)
        .wrap_err("failed to read typed dual-state resource specification")?;
    let text = std::str::from_utf8(&bytes)
        .wrap_err("typed dual-state resource specification is not valid UTF-8")?;
    let value: norito::json::Value = norito::json::from_str(text)
        .wrap_err("typed dual-state resource specification is not valid Norito JSON")?;
    if bytes != canonical_json_bytes(&value)? {
        bail!(
            "typed dual-state resource specification must be compact canonical Norito JSON with \
             sorted keys and one trailing newline"
        );
    }
    let discriminant = value
        .get("chain_discriminant")
        .and_then(norito::json::Value::as_u64)
        .ok_or_else(|| eyre!("resource specification omits numeric chain_discriminant"))?;
    let discriminant = u16::try_from(discriminant)
        .wrap_err("resource specification chain_discriminant exceeds u16")?;
    let _chain_guard = ChainDiscriminantGuard::enter(discriminant);
    let spec: ResourceSpecV1 = norito::json::value::from_value(value)
        .wrap_err("failed to decode typed dual-state resource specification")?;
    if typed_json_bytes(&spec)? != bytes {
        bail!(
            "typed dual-state resource specification changed across canonical typed JSON \
             round-trip"
        );
    }
    validate_resource_spec(&spec)?;
    Ok(LoadedResourceSpec { spec })
}

fn read_phase_one_readiness_evidence(
    path: &Path,
) -> color_eyre::Result<CapturedInput<PhaseOneReadinessEvidenceV1>> {
    capture_canonical_typed_file(path, 16 * 1024 * 1024, "phase-one readiness evidence")
}

fn parse_canonical_typed_bytes<T>(bytes: &[u8], label: &str) -> color_eyre::Result<T>
where
    T: norito::json::JsonDeserialize + norito::json::JsonSerialize,
{
    let text =
        std::str::from_utf8(bytes).wrap_err_with(|| format!("{label} is not valid UTF-8"))?;
    let value: norito::json::Value = norito::json::from_str(text)
        .wrap_err_with(|| format!("{label} is not valid Norito JSON"))?;
    if canonical_json_bytes(&value)? != bytes {
        bail!(
            "{label} must be compact canonical Norito JSON with sorted keys and one trailing newline"
        );
    }
    let decoded: T = norito::json::value::from_value(value)
        .wrap_err_with(|| format!("{label} typed schema is not exact"))?;
    if typed_json_bytes(&decoded)? != bytes {
        bail!("{label} changed across canonical typed JSON round-trip");
    }
    Ok(decoded)
}

fn capture_bytes(
    path: &Path,
    maximum_bytes: u64,
    label: &str,
) -> color_eyre::Result<CapturedBytes> {
    let (bytes, sha256) = canonical_closure::stable_file_bytes(path, maximum_bytes)
        .wrap_err_with(|| format!("failed to capture {label}"))?;
    Ok(CapturedBytes { bytes, sha256 })
}

fn capture_canonical_typed_file<T>(
    path: &Path,
    maximum_bytes: u64,
    label: &str,
) -> color_eyre::Result<CapturedInput<T>>
where
    T: norito::json::JsonDeserialize + norito::json::JsonSerialize,
{
    let capture = capture_bytes(path, maximum_bytes, label)?;
    let value = parse_canonical_typed_bytes(&capture.bytes, label)?;
    Ok(CapturedInput {
        value,
        sha256: capture.sha256,
    })
}

fn read_root_published_generated_candidate_receipt(
    path: &Path,
) -> color_eyre::Result<CapturedInput<RootPublishedGeneratedCandidateReceiptV1>> {
    capture_canonical_typed_file(
        path,
        16 * 1024 * 1024,
        "root-published generated-candidate receipt",
    )
}

fn read_runtime_transition(path: &Path) -> color_eyre::Result<CapturedInput<RuntimeTransitionV1>> {
    capture_canonical_typed_file(path, 16 * 1024 * 1024, "dual-asset runtime transition")
}

fn read_is2_manifest_authorization(
    path: &Path,
) -> color_eyre::Result<CapturedInput<SignedIs2ManifestAuthorizationV1>> {
    capture_canonical_typed_file(path, 16 * 1024 * 1024, "is2 manifest authorization")
}

fn read_approval_trust_bundle(
    path: &Path,
) -> color_eyre::Result<CapturedInput<ApprovalTrustBundleV1>> {
    capture_canonical_typed_file(path, 16 * 1024 * 1024, "approval trust bundle")
}

fn validate_is2_manifest_authorization(
    authorization: &SignedIs2ManifestAuthorizationV1,
    spec: &ResourceSpecV1,
    phase_one_instructions_sha256: &str,
    expected_source_commit: &str,
) -> Outcome {
    let expected = Is2ManifestAuthorizationBodyV1 {
        schema_version: SCHEMA_VERSION,
        kind: "taira-is2-signed-genesis-manifest-authorization-v1".to_owned(),
        reviewed_iroha_commit: expected_source_commit.to_owned(),
        chain_id: spec.chain_id.clone(),
        base_genesis_sha256: REVIEWED_BASE_GENESIS_SHA256.to_owned(),
        phase_one_instructions_sha256: phase_one_instructions_sha256.to_owned(),
        authority: spec.base_genesis_authority.clone(),
    };
    if authorization.body != expected {
        bail!(
            "is2 manifest authorization does not bind the exact reviewed genesis, commit, chain, \
             authority, and phase-one instruction digest"
        );
    }
    let public_key = authorization
        .body
        .authority
        .controller()
        .single_signatory()
        .ok_or_else(|| eyre!("is2 manifest authority must have exactly one controller key"))?;
    authorization
        .signature
        .verify(
            public_key,
            &is2_manifest_authorization_signing_bytes(&authorization.body)?,
        )
        .wrap_err("is2 manifest authorization signature is invalid")?;
    Ok(())
}

fn is2_manifest_authorization_signing_bytes(
    body: &Is2ManifestAuthorizationBodyV1,
) -> color_eyre::Result<Vec<u8>> {
    const DOMAIN: &[u8] = b"iroha:taira-dual-state:is2-manifest-authorization:v1\0";
    let body = typed_json_bytes(body)?;
    let mut bytes = Vec::with_capacity(DOMAIN.len() + body.len());
    bytes.extend_from_slice(DOMAIN);
    bytes.extend_from_slice(&body);
    Ok(bytes)
}

fn validate_approval_trust_bundle(
    bundle: &ApprovalTrustBundleV1,
    spec: &ResourceSpecV1,
    phase_two_instructions_sha256: &str,
    expected_source_commit: &str,
) -> Outcome {
    let enactment = &spec.phase_two_offline_bootstrap;
    if bundle.schema_version != SCHEMA_VERSION
        || bundle.kind != "taira-is-offline-bootstrap-approval-trust-v1"
        || bundle.reviewed_iroha_commit != expected_source_commit
        || bundle.chain_id != spec.chain_id
        || bundle.base_genesis_sha256 != REVIEWED_BASE_GENESIS_SHA256
        || bundle.phase_two_instructions_sha256 != phase_two_instructions_sha256
        || bundle.manifest_fingerprint != enactment.manifest().fingerprint()
        || &bundle.certificate != enactment.certificate()
        || bundle.certificate.signatures.scheme != EnactmentSignatureScheme::SimpleThreshold
    {
        bail!(
            "approval trust bundle does not bind the exact reviewed chain, phase-two resources, \
             manifest fingerprint, and Parliament certificate"
        );
    }
    for approval in &bundle.certificate.signatures.signatures {
        approval
            .signature
            .verify(&approval.public_key, &bundle.certificate.payload)
            .wrap_err_with(|| {
                format!(
                    "approval trust bundle contains an invalid signature for {}",
                    approval.signer
                )
            })?;
    }
    Ok(())
}

fn validate_activation_and_runtime_transition(
    activation_instructions: &[InstructionBox],
    transition: &RuntimeTransitionV1,
    spec: &ResourceSpecV1,
    policy_sha256: &str,
) -> color_eyre::Result<BTreeMap<AssetDefinitionId, String>> {
    if activation_instructions.len() != 1 {
        bail!("phase-one activation must contain exactly one instruction");
    }
    let activation = activation_instructions[0]
        .as_any()
        .downcast_ref::<ActivateKagemushaRecursiveReleaseV4>()
        .ok_or_else(|| {
            eyre!(
                "phase-one activation must be an actual \
                 ActivateKagemushaRecursiveReleaseV4 instruction"
            )
        })?;
    let phase_one_manifest_sha256 = validate_release_activation(
        activation,
        spec,
        &asset_id_for_phase(1)?,
        policy_sha256,
        "phase-one",
        "ds#boi.is2",
    )?;
    let phase_two_manifest_sha256 = validate_release_activation(
        &spec.phase_two_offline_bootstrap.manifest().release,
        spec,
        &asset_id_for_phase(2)?,
        policy_sha256,
        "phase-two",
        "ds#boi.is",
    )?;

    let expected_escrows = BTreeMap::from([
        (
            asset_id_for_phase(1)?,
            spec.phase_one.accounts.escrow.clone(),
        ),
        (
            asset_id_for_phase(2)?,
            spec.phase_two.accounts.escrow.clone(),
        ),
    ]);
    if transition.schema_version != SCHEMA_VERSION
        || transition.kind != "taira-dual-offline-runtime-transition-v1"
        || transition.base_config_sha256 != REVIEWED_BASE_CONFIG_SHA256
        || transition.escrow_accounts != expected_escrows
        || transition.release_policy_sha256 != policy_sha256
        || transition.phase_one_artifact_manifest_sha256 != phase_one_manifest_sha256
        || transition.phase_two_artifact_manifest_sha256 != phase_two_manifest_sha256
        || !transition.apply_after_phase_two_commit
        || !transition.coordinated_four_validator_restart_required
    {
        bail!(
            "dual-asset runtime transition does not bind the exact config, assets, escrows, \
             authenticated release, and coordinated post-phase-two restart"
        );
    }
    Ok(BTreeMap::from([
        (asset_id_for_phase(1)?, phase_one_manifest_sha256),
        (asset_id_for_phase(2)?, phase_two_manifest_sha256),
    ]))
}

fn validate_release_activation(
    activation: &ActivateKagemushaRecursiveReleaseV4,
    spec: &ResourceSpecV1,
    expected_asset: &AssetDefinitionId,
    expected_policy_sha256: &str,
    phase_label: &str,
    asset_alias: &str,
) -> color_eyre::Result<String> {
    let release = activation.activation();
    let manifest = &release.release_record.manifest;
    let expected_activation_height = if phase_label == "phase-one" { 1 } else { 3 };
    if manifest.version != 4
        || manifest.bridge_abi_version != 21
        || manifest.proof_backend != "halo2/ipa"
        || manifest.chain_id != spec.chain_id
        || &manifest.asset != expected_asset
        || manifest.asset_scale != 2
        || manifest.activation_height != expected_activation_height
        || manifest.withdrawal_height <= expected_activation_height
        || release.release_record.promotion_record.bridge_abi_version != 21
    {
        bail!(
            "{phase_label} activation is not the exact asset-bound ABI-21/V4 halo2/ipa release \
             for {asset_alias}"
        );
    }
    release
        .release_record
        .validate_structure()
        .map_err(|error| eyre!("{phase_label} release structure is invalid: {error}"))?;
    let recursive_records = [
        (
            RECURSIVE_VERIFIER_ROLES[0],
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4,
            iroha_data_model::offline::kagemusha_recursive_spend_step_eq_public_inputs_schema_hash_v4(),
            &release.step_eq_verifier_record,
        ),
        (
            RECURSIVE_VERIFIER_ROLES[1],
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
            iroha_data_model::offline::kagemusha_recursive_spend_step_ep_public_inputs_schema_hash_v4(),
            &release.step_ep_verifier_record,
        ),
    ];
    let mut recursive_commitments = BTreeSet::new();
    for (role, circuit_id, schema_hash, record) in recursive_records {
        let key = record
            .key
            .as_ref()
            .ok_or_else(|| eyre!("{phase_label} {role} verifier omits inline key bytes"))?;
        if record.version == 0
            || record.backend != BackendTag::Halo2IpaPasta
            || record.circuit_id != circuit_id
            || record.public_inputs_schema_hash != schema_hash
            || record.status != ConfidentialStatus::Active
            || record.activation_height != Some(expected_activation_height)
            || record
                .withdraw_height
                .is_some_and(|height| height <= expected_activation_height)
            || record.max_proof_bytes == 0
            || record.max_proof_bytes
                > iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4
            || iroha_core::zk::hash_vk(key) != record.commitment
            || !recursive_commitments.insert(record.commitment)
        {
            bail!(
                "{phase_label} {role} is not the exact distinct commitment-bound halo2/ipa verifier"
            );
        }
    }
    let manifest_sha256 = hex::encode(
        manifest
            .canonical_sha256()
            .map_err(|error| eyre!("{phase_label} release manifest is invalid: {error}"))?,
    );
    if hex::encode(release.configured_policy_sha256) != expected_policy_sha256 {
        bail!("{phase_label} activation does not bind the exact --release-policy bytes");
    }
    Ok(manifest_sha256)
}

fn validate_exact_digest(observed: &str, expected: &str, label: &str) -> Outcome {
    if observed != expected {
        bail!("{label} SHA-256 mismatch: expected {expected}, observed {observed}");
    }
    Ok(())
}

#[cfg(test)]
fn sha256_file(path: &Path, label: &str) -> color_eyre::Result<String> {
    let bytes = fs::read(path).wrap_err_with(|| format!("failed to read {label}"))?;
    Ok(sha256_hex(&bytes))
}

#[cfg(test)]
fn stable_pinned_file_digest(
    path: &Path,
    maximum_bytes: u64,
    label: &str,
) -> color_eyre::Result<String> {
    let path = canonical_closure::validate_absolute_normalized(path, label)?;
    let (digest, _) = canonical_closure::stable_file_digest(&path, maximum_bytes)
        .wrap_err_with(|| format!("failed to authenticate {label}"))?;
    Ok(digest)
}

fn load_openpgp_verifier_anchor(
    production_closure_root: &Path,
) -> color_eyre::Result<OpenPgpVerifierAnchor> {
    let root = canonical_closure::validate_absolute_normalized(
        production_closure_root,
        "--production-closure-root",
    )?;
    if fs::canonicalize(&root).wrap_err("production closure root is unavailable")? != root {
        bail!("production closure root must be one exact non-symlink canonical path");
    }
    let tree_sha256 = root
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .ok_or_else(|| eyre!("production closure root lacks its content-addressed leaf"))?;
    if !is_lower_nonzero_sha256(tree_sha256) {
        bail!("production closure root leaf is not one canonical SHA-256");
    }
    let policy = canonical_closure::HardenedTreePolicy {
        trusted_uid: 0,
        root_mode: 0o555,
        directory_mode: 0o555,
        allowed_file_modes: &[0o444, 0o555],
        allow_internal_symlinks: true,
    };
    canonical_closure::validate_trusted_parent_chain(&root, 0)?;
    let before = canonical_closure::inventory_hardened_tree(&root, policy, None)?;
    if canonical_closure::production_closure_digest(&before)? != tree_sha256 {
        bail!("root-owned production closure does not recompute to its content-addressed identity");
    }
    let provenance_entry = tree_entry(
        &before,
        canonical_closure::PRODUCTION_PROVENANCE_FILE_NAME,
        "production closure provenance",
    )?;
    if provenance_entry.kind != canonical_closure::TreeEntryKind::File
        || provenance_entry.mode != 0o444
    {
        bail!("production closure provenance is not one root-owned read-only regular file");
    }
    let provenance_path = root.join(std::ffi::OsStr::from_bytes(
        canonical_closure::PRODUCTION_PROVENANCE_FILE_NAME,
    ));
    let (provenance_bytes, provenance_sha256) =
        canonical_closure::stable_file_bytes(&provenance_path, 4 * 1024 * 1024)?;
    if hex::encode(provenance_entry.sha256) != provenance_sha256 {
        bail!("production closure provenance differs from its authenticated tree descriptor");
    }
    let provenance: ProductionClosureProvenanceV1 =
        parse_canonical_typed_bytes(&provenance_bytes, "production closure provenance")?;
    if provenance.schema != PRODUCTION_CLOSURE_SCHEMA
        || provenance.provisioning_protocol != PRODUCTION_PROVISIONING_PROTOCOL
        || provenance.closure_root != root.to_string_lossy()
        || provenance.closure_tree_sha256 != tree_sha256
        || provenance.source_signing_key_fingerprint != SEMANTIC_TRUST_AUTHORITY_FINGERPRINT
        || !is_lower_nonzero_sha256(&provenance.gpg_sha256)
    {
        bail!(
            "root-owned production closure provenance does not authenticate the expected OpenPGP \
             verifier"
        );
    }
    let verifier_path = path_from_receipt(&provenance.gpg_path, "production OpenPGP verifier")?;
    if fs::canonicalize(&verifier_path).wrap_err("production OpenPGP verifier is unavailable")?
        != verifier_path
    {
        bail!("production OpenPGP verifier must be one exact non-symlink canonical path");
    }
    let verifier_relative =
        path_relative_to_root(&root, &provenance.gpg_path, "production OpenPGP verifier")?;
    let verifier_entry = tree_entry(&before, &verifier_relative, "production OpenPGP verifier")?;
    if verifier_entry.kind != canonical_closure::TreeEntryKind::File
        || verifier_entry.mode != 0o555
        || hex::encode(verifier_entry.sha256) != provenance.gpg_sha256
    {
        bail!(
            "production OpenPGP verifier differs from the root-owned provenance and tree \
             inventory"
        );
    }
    let verifier = canonical_closure::stable_file(&verifier_path, 256 * 1024 * 1024)
        .wrap_err("failed to retain production OpenPGP verifier inode")?;
    if verifier.sha256() != provenance.gpg_sha256
        || canonical_closure::inventory_hardened_tree(&root, policy, None)? != before
    {
        bail!("production OpenPGP verifier anchor changed while authenticated");
    }
    Ok(OpenPgpVerifierAnchor {
        path: verifier_path,
        sha256: provenance.gpg_sha256,
        verifier,
    })
}

fn validate_verifier_invocation_matches_anchor(
    path: &Path,
    supplied_sha256: &str,
    anchor: &OpenPgpVerifierAnchor,
) -> Outcome {
    if !is_lower_nonzero_sha256(supplied_sha256) {
        bail!("supplied OpenPGP verifier SHA-256 is not canonical and nonzero");
    }
    let path = canonical_closure::validate_absolute_normalized(path, "supplied OpenPGP verifier")?;
    if path != anchor.path || supplied_sha256 != anchor.sha256 {
        bail!(
            "supplied OpenPGP verifier identity differs from the independently authenticated \
             production-closure anchor"
        );
    }
    // The caller-provided path and digest are compatibility assertions only.
    // Execution is bound below to the retained production-closure descriptor.
    Ok(())
}

fn validate_semantic_trust_authorization(
    args: &ValidateSemanticArgs,
    verifier_anchor: &mut OpenPgpVerifierAnchor,
) -> color_eyre::Result<AuthenticatedSemanticTrust> {
    if !is_lower_nonzero_sha256(&args.semantic_trust_sha256) {
        bail!("semantic-trust external SHA-256 pin must be canonical and nonzero");
    }
    let mut semantic_trust = canonical_closure::stable_file(&args.semantic_trust, 16 * 1024 * 1024)
        .wrap_err("failed to retain exact semantic trust descriptor")?;
    let semantic_trust_sha256 = semantic_trust.sha256().to_owned();
    let semantic_trust_bytes = semantic_trust
        .read_bytes(16 * 1024 * 1024)
        .wrap_err("failed to read exact semantic trust descriptor bytes")?;
    if semantic_trust_sha256 != args.semantic_trust_sha256 {
        bail!("semantic trust bytes differ from their external exact SHA-256 pin");
    }
    let mut signature = canonical_closure::stable_file(&args.semantic_trust_signature, 1024 * 1024)
        .wrap_err("failed to retain semantic trust detached signature")?;
    let signature_sha256 = signature.sha256().to_owned();
    let mut public_key =
        canonical_closure::stable_file(&args.semantic_trust_public_key, 4 * 1024 * 1024)
            .wrap_err("failed to retain semantic trust public key")?;
    let public_key_sha256 = public_key.sha256().to_owned();
    if public_key_sha256 != SEMANTIC_TRUST_PUBLIC_KEY_SHA256 {
        bail!(
            "semantic trust public key differs from the compiled operator/root key-material anchor"
        );
    }
    validate_verifier_invocation_matches_anchor(&args.gpgv, &args.gpgv_sha256, verifier_anchor)?;

    let verifier_descriptor = verifier_anchor
        .verifier
        .inherited_descriptor()
        .wrap_err("failed to prepare descriptor-bound OpenPGP verifier")?;
    let verifier_executable = verifier_anchor
        .verifier
        .descriptor_bound_executable_path(&verifier_descriptor)
        .wrap_err("failed to resolve descriptor-bound OpenPGP verifier")?;
    let signature_descriptor = signature
        .inherited_descriptor()
        .wrap_err("failed to prepare descriptor-bound detached signature")?;
    let public_key_descriptor = public_key
        .inherited_descriptor()
        .wrap_err("failed to prepare descriptor-bound semantic trust public key")?;
    let mut child = ProcessCommand::new(&verifier_executable)
        .env_clear()
        .env("HOME", "/var/empty")
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .env("PATH", "/usr/bin:/bin")
        .arg("--batch")
        .arg("--no-options")
        .arg("--homedir")
        .arg("/var/empty")
        .arg("--lock-never")
        .arg("--no-default-keyring")
        .arg("--keyring")
        .arg(public_key_descriptor.alias_path())
        .arg("--no-auto-check-trustdb")
        .arg("--status-fd=1")
        .arg("--verify")
        .arg(signature_descriptor.alias_path())
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .wrap_err("failed to execute production-closure OpenPGP verifier")?;
    child
        .stdin
        .take()
        .ok_or_else(|| eyre!("OpenPGP verifier stdin is unavailable"))?
        .write_all(&semantic_trust_bytes)
        .wrap_err("failed to feed exact semantic trust bytes to OpenPGP verifier")?;
    let output = child
        .wait_with_output()
        .wrap_err("failed to wait for production-closure OpenPGP verifier")?;
    if !output.status.success() {
        bail!("semantic trust detached OpenPGP signature is invalid");
    }
    let status = std::str::from_utf8(&output.stdout)
        .wrap_err("gpgv status output is not canonical UTF-8")?;
    let valid_signatures = status
        .lines()
        .filter_map(|line| line.strip_prefix("[GNUPG:] VALIDSIG "))
        .collect::<Vec<_>>();
    if valid_signatures.len() != 1 {
        bail!("gpgv did not emit exactly one valid-signature status");
    }
    let fields = valid_signatures[0]
        .split_ascii_whitespace()
        .collect::<Vec<_>>();
    let signing_fingerprint = fields
        .first()
        .copied()
        .ok_or_else(|| eyre!("gpgv valid-signature status omits its fingerprint"))?;
    let primary_fingerprint = fields.last().copied().unwrap_or(signing_fingerprint);
    if signing_fingerprint != SEMANTIC_TRUST_AUTHORITY_FINGERPRINT
        && primary_fingerprint != SEMANTIC_TRUST_AUTHORITY_FINGERPRINT
    {
        bail!("semantic trust signature does not descend from the pinned operator/root authority");
    }

    semantic_trust
        .verify_unchanged()
        .wrap_err("semantic trust descriptor changed during verification")?;
    signature
        .verify_unchanged()
        .wrap_err("semantic trust detached signature changed during verification")?;
    public_key
        .verify_unchanged()
        .wrap_err("semantic trust public key changed during verification")?;
    verifier_anchor
        .verifier
        .verify_unchanged()
        .wrap_err("production OpenPGP verifier changed during verification")?;
    let trust = parse_canonical_typed_bytes(&semantic_trust_bytes, "semantic trust descriptor")?;
    Ok(AuthenticatedSemanticTrust {
        trust,
        authorization: SemanticTrustAuthorization {
            signature_sha256,
            public_key_sha256,
            gpgv_sha256: verifier_anchor.sha256.clone(),
        },
    })
}

fn validate_semantic_trust_receipt_bindings(
    trust: &SemanticTrustV1,
    receipt: &RootPublishedGeneratedCandidateReceiptV1,
    receipt_sha256: &str,
    receipt_path: &Path,
    source_identity: &source_closure::ValidatedSourceIdentity,
) -> Outcome {
    if trust.schema_version != SCHEMA_VERSION
        || trust.kind != "taira-dual-state-semantic-trust-v1"
        || trust.phase_one_identity.evaluated_block_height != 1
        || trust.final_identity.evaluated_block_height != 3
        || !is_lower_nonzero_sha256(&trust.phase_one_identity.evaluated_block_hash)
        || !is_lower_nonzero_sha256(&trust.final_identity.evaluated_block_hash)
        || !is_lower_nonzero_sha256(&trust.expected_validator_artifact_sha256)
        || trust.source_commit != source_identity.source_commit
        || trust.source_tree_sha256 != source_identity.source_tree_sha256
        || trust.reviewed_source_closure_descriptor_sha256 != source_identity.descriptor_sha256
    {
        bail!(
            "semantic trust descriptor must independently pin the reviewed source, validator \
             artifact, and exact phase-one h1/final h3 identities"
        );
    }
    for (label, digest) in [
        (
            "root-published generated-candidate receipt",
            &trust.root_published_generated_candidate_receipt_sha256,
        ),
        (
            "root-published generated-candidate tree",
            &trust.root_published_generated_candidate_tree_sha256,
        ),
        ("candidate tree", &trust.candidate_tree_sha256),
        (
            "candidate build artifact tree",
            &trust.candidate_build_artifact_tree_sha256,
        ),
        (
            "candidate build receipt",
            &trust.candidate_build_receipt_sha256,
        ),
        (
            "production closure tree",
            &trust.production_closure_tree_sha256,
        ),
        (
            "reviewed source closure descriptor",
            &trust.reviewed_source_closure_descriptor_sha256,
        ),
        ("source tree", &trust.source_tree_sha256),
        ("toolchain provenance", &trust.toolchain_provenance_sha256),
        (
            "generation worker launch receipt",
            &trust.generation_worker_launch_receipt_sha256,
        ),
    ] {
        if !is_lower_nonzero_sha256(digest) {
            bail!("semantic trust descriptor contains malformed {label} SHA-256");
        }
    }
    if receipt.schema != ROOT_PUBLISHED_GENERATED_CANDIDATE_SCHEMA
        || receipt.publication_protocol != ROOT_ATOMIC_PUBLICATION_PROTOCOL
        || receipt.publication_status != ROOT_PUBLISHED_GENERATION_STATUS
        || receipt.provisional_generation_publication_status != PROVISIONAL_GENERATION_STATUS
        || receipt.provisional_cross_stage_status != PROVISIONAL_CROSS_STAGE_STATUS
        || receipt.build_uid == 0
        || receipt.build_user_name != "boi-build"
        || receipt.source_commit != source_identity.source_commit
    {
        bail!("root-published generated-candidate receipt contract is not exact");
    }
    for (label, digest) in [
        ("artifact tree", &receipt.artifact_tree_sha256),
        (
            "candidate build artifact tree",
            &receipt.candidate_build_artifact_tree_sha256,
        ),
        (
            "candidate build receipt",
            &receipt.candidate_build_receipt_sha256,
        ),
        ("candidate tree", &receipt.candidate_tree_sha256),
        (
            "generation resource report tree",
            &receipt.generation_resource_report_tree_sha256,
        ),
        ("generation summary", &receipt.generation_summary_sha256),
        (
            "production closure tree",
            &receipt.production_closure_tree_sha256,
        ),
        (
            "reviewed source closure descriptor",
            &receipt.reviewed_source_closure_descriptor_sha256,
        ),
        ("source tree", &receipt.source_tree_sha256),
        ("toolchain provenance", &receipt.toolchain_provenance_sha256),
        (
            "worker launch receipt",
            &receipt.worker_launch_receipt_sha256,
        ),
    ] {
        if !is_lower_nonzero_sha256(digest) {
            bail!("root-published receipt contains malformed {label} SHA-256");
        }
    }
    if receipt_sha256 != trust.root_published_generated_candidate_receipt_sha256
        || receipt.artifact_tree_sha256 != trust.root_published_generated_candidate_tree_sha256
        || receipt.candidate_tree_sha256 != trust.candidate_tree_sha256
        || receipt.candidate_build_artifact_tree_sha256
            != trust.candidate_build_artifact_tree_sha256
        || receipt.candidate_build_receipt_sha256 != trust.candidate_build_receipt_sha256
        || receipt.production_closure_tree_sha256 != trust.production_closure_tree_sha256
        || receipt.reviewed_source_closure_descriptor_sha256
            != trust.reviewed_source_closure_descriptor_sha256
        || receipt.source_commit != trust.source_commit
        || receipt.source_tree_sha256 != trust.source_tree_sha256
        || receipt.toolchain_provenance_sha256 != trust.toolchain_provenance_sha256
        || receipt.worker_launch_receipt_sha256 != trust.generation_worker_launch_receipt_sha256
    {
        bail!(
            "semantic trust descriptor and root-published generated-candidate/source-seal receipt \
             do not bind the same production closure"
        );
    }
    let artifact_root = Path::new(&receipt.artifact_root);
    let candidate_dir = Path::new(&receipt.candidate_dir_path);
    let resource_report = Path::new(&receipt.generation_resource_report_path);
    let generation_summary = Path::new(&receipt.generation_summary_path);
    if !artifact_root.is_absolute()
        || artifact_root.file_name().and_then(|name| name.to_str())
            != Some(receipt.artifact_tree_sha256.as_str())
        || receipt_path != artifact_root.join("root-published-generated-candidate.json")
        || candidate_dir != artifact_root.join("candidate")
        || resource_report != artifact_root.join("resource-report")
        || generation_summary != resource_report.join("kagemusha_resource_summary.json")
    {
        bail!("root-published generated-candidate receipt paths are not exact");
    }
    // Upstream receipt, worker-launch, and generation-summary bytes are
    // descriptor-captured against their authenticated tree entries in
    // `validate_independent_publication_closure`.
    Ok(())
}

fn validate_semantic_trust_and_root_receipt(
    trust: &SemanticTrustV1,
    receipt: &RootPublishedGeneratedCandidateReceiptV1,
    receipt_sha256: &str,
    receipt_path: &Path,
    production_closure_root: &Path,
    source_identity: &source_closure::ValidatedSourceIdentity,
) -> Outcome {
    validate_semantic_trust_receipt_bindings(
        trust,
        receipt,
        receipt_sha256,
        receipt_path,
        source_identity,
    )?;
    validate_independent_publication_closure(
        trust,
        receipt,
        receipt_sha256,
        receipt_path,
        production_closure_root,
        source_identity,
    )
}

fn tree_entry<'a>(
    entries: &'a [canonical_closure::TreeEntry],
    relative: &[u8],
    label: &str,
) -> color_eyre::Result<&'a canonical_closure::TreeEntry> {
    entries
        .iter()
        .find(|entry| entry.relative == relative)
        .ok_or_else(|| eyre!("{label} is absent from its authenticated tree inventory"))
}

fn capture_inventory_typed<T>(
    root: &Path,
    relative: &[u8],
    entries: &[canonical_closure::TreeEntry],
    maximum_bytes: u64,
    label: &str,
) -> color_eyre::Result<CapturedInput<T>>
where
    T: norito::json::JsonDeserialize + norito::json::JsonSerialize,
{
    let entry = tree_entry(entries, relative, label)?;
    if entry.kind != canonical_closure::TreeEntryKind::File
        || entry.size > maximum_bytes
        || usize::try_from(entry.size).is_err()
    {
        bail!("{label} tree entry is not one bounded regular file");
    }
    let path = root.join(std::ffi::OsStr::from_bytes(relative));
    let captured = capture_canonical_typed_file(&path, maximum_bytes, label)?;
    if captured.sha256 != hex::encode(entry.sha256)
        || typed_json_bytes(&captured.value)?.len()
            != usize::try_from(entry.size).expect("bounded entry size fits usize")
    {
        bail!("{label} bytes differ from the authenticated tree entry");
    }
    Ok(captured)
}

fn exact_file_inventory(
    entries: &[canonical_closure::TreeEntry],
    expected: &[&[u8]],
    required_mode: u32,
    label: &str,
) -> Outcome {
    let observed = entries
        .iter()
        .map(|entry| entry.relative.clone())
        .collect::<BTreeSet<_>>();
    let expected = expected
        .iter()
        .map(|path| path.to_vec())
        .collect::<BTreeSet<_>>();
    if observed != expected
        || entries.iter().any(|entry| {
            entry.kind != canonical_closure::TreeEntryKind::File || entry.mode != required_mode
        })
    {
        bail!("{label} file inventory, types, or modes are not exact");
    }
    Ok(())
}

fn path_from_receipt(value: &str, label: &str) -> color_eyre::Result<PathBuf> {
    canonical_closure::validate_absolute_normalized(Path::new(value), label)
}

fn path_relative_to_root(root: &Path, value: &str, label: &str) -> color_eyre::Result<Vec<u8>> {
    use std::os::unix::ffi::OsStrExt as _;

    let path = path_from_receipt(value, label)?;
    let relative = path
        .strip_prefix(root)
        .wrap_err_with(|| format!("{label} escapes the production closure"))?;
    if relative.as_os_str().is_empty() {
        bail!("{label} must name a child inside the production closure");
    }
    Ok(relative.as_os_str().as_bytes().to_vec())
}

fn require_inventory_file_digest(
    entries: &[canonical_closure::TreeEntry],
    root: &Path,
    path: &str,
    expected_sha256: &str,
    label: &str,
) -> Outcome {
    let relative = path_relative_to_root(root, path, label)?;
    let entry = tree_entry(entries, &relative, label)?;
    if entry.kind != canonical_closure::TreeEntryKind::File
        || hex::encode(entry.sha256) != expected_sha256
    {
        bail!("{label} is not the exact pinned production-closure file");
    }
    Ok(())
}

fn require_inventory_directory(
    entries: &[canonical_closure::TreeEntry],
    root: &Path,
    path: &str,
    label: &str,
) -> Outcome {
    let relative = path_relative_to_root(root, path, label)?;
    if tree_entry(entries, &relative, label)?.kind != canonical_closure::TreeEntryKind::Directory {
        bail!("{label} is not an authenticated production-closure directory");
    }
    Ok(())
}

fn validate_production_provenance(
    provenance: &ProductionClosureProvenanceV1,
    provenance_sha256: &str,
    production_root: &Path,
    production_entries: &[canonical_closure::TreeEntry],
    expected_tree_sha256: &str,
    source_identity: &source_closure::ValidatedSourceIdentity,
) -> Outcome {
    if provenance.schema != PRODUCTION_CLOSURE_SCHEMA
        || provenance.provisioning_protocol != PRODUCTION_PROVISIONING_PROTOCOL
        || provenance.closure_root != production_root.to_string_lossy()
        || provenance.closure_tree_sha256 != expected_tree_sha256
        || provenance.source_root != production_root.join("source").to_string_lossy()
        || provenance.reviewed_source_closure_path
            != production_root
                .join("reviewed-source-closure.json")
                .to_string_lossy()
        || provenance.source_signing_key_fingerprint != SEMANTIC_TRUST_AUTHORITY_FINGERPRINT
    {
        bail!("production closure provenance identity and paths are not exact");
    }
    let provenance_entry = tree_entry(
        production_entries,
        canonical_closure::PRODUCTION_PROVENANCE_FILE_NAME,
        "production closure provenance",
    )?;
    if provenance_entry.kind != canonical_closure::TreeEntryKind::File
        || provenance_entry.mode != 0o444
        || hex::encode(provenance_entry.sha256) != provenance_sha256
    {
        bail!("production closure provenance bytes do not match the authenticated inventory");
    }
    require_inventory_file_digest(
        production_entries,
        production_root,
        &provenance.reviewed_source_closure_path,
        &source_identity.descriptor_sha256,
        "reviewed source closure",
    )?;
    for (path, digest, label) in [
        (&provenance.cargo_path, &provenance.cargo_sha256, "cargo"),
        (&provenance.git_path, &provenance.git_sha256, "git"),
        (&provenance.gpg_path, &provenance.gpg_sha256, "gpg"),
        (&provenance.linker_path, &provenance.linker_sha256, "linker"),
        (&provenance.python_path, &provenance.python_sha256, "python"),
        (&provenance.rustc_path, &provenance.rustc_sha256, "rustc"),
    ] {
        if !is_lower_nonzero_sha256(digest) {
            bail!("production closure {label} digest is malformed");
        }
        require_inventory_file_digest(production_entries, production_root, path, digest, label)?;
    }
    for (path, label) in [
        (
            &provenance.apple_developer_dir_path,
            "Apple developer directory",
        ),
        (&provenance.apple_sdk_path, "Apple SDK"),
        (&provenance.cargo_home_path, "Cargo home"),
        (&provenance.cargo_vendor_path, "Cargo vendor tree"),
        (
            &provenance.clang_resource_dir_path,
            "Clang resource directory",
        ),
        (&provenance.git_exec_path, "Git exec path"),
        (&provenance.gnupghome_path, "GnuPG home"),
        (&provenance.rustc_sysroot_path, "Rust sysroot"),
        (&provenance.source_root, "source root"),
    ] {
        require_inventory_directory(production_entries, production_root, path, label)?;
    }
    Ok(())
}

fn validate_candidate_report(
    report: &CandidateBuildReportV1,
    receipt: &CandidateBuildReceiptV1,
    provenance: &ProductionClosureProvenanceV1,
    production_root: &Path,
    source_identity: &source_closure::ValidatedSourceIdentity,
) -> Outcome {
    let nested_descriptor_sha256 =
        sha256_hex(&canonical_json_bytes(&report.reviewed_source_closure)?);
    let nested_dirty = report
        .reviewed_source_closure
        .get("source_repo_dirty")
        .and_then(norito::json::Value::as_bool)
        .ok_or_else(|| eyre!("candidate report nested source closure omits source_repo_dirty"))?;
    if report.schema != CANDIDATE_BUILD_REPORT_SCHEMA
        || report.publication_status != "provisional_boi_build_worker_output"
        || report.build_profile != "release"
        || report.build_uid != receipt.build_uid
        || report.build_user_name != receipt.build_user_name
        || report.binary_sha256 != receipt.binary_sha256
        || report.binary_size_bytes != receipt.binary_size_bytes
        || Path::new(&report.binary_path).file_name()
            != Some(std::ffi::OsStr::from_bytes(CANDIDATE_BUILD_BINARY_NAME))
        || report.production_closure_root != production_root.to_string_lossy()
        || report.production_closure_tree_sha256 != receipt.production_closure_tree_sha256
        || report.reviewed_source_closure_descriptor_sha256 != source_identity.descriptor_sha256
        || nested_descriptor_sha256 != source_identity.descriptor_sha256
        || report.source_commit != source_identity.source_commit
        || report.source_tree_sha256 != source_identity.source_tree_sha256
        || report.source_repo_dirty != nested_dirty
        || report.toolchain_provenance_sha256 != receipt.toolchain_provenance_sha256
        || report.source_signing_key_fingerprint != SEMANTIC_TRUST_AUTHORITY_FINGERPRINT
        || report.minimum_build_physical_memory_bytes == 0
        || report.physical_memory_bytes_at_admission < report.minimum_build_physical_memory_bytes
    {
        bail!("sealed candidate build report does not bind the exact admitted source and closure");
    }
    for (report_value, provenance_value, label) in [
        (
            &report.apple_developer_dir_path,
            &provenance.apple_developer_dir_path,
            "Apple developer directory",
        ),
        (
            &report.apple_sdk_path,
            &provenance.apple_sdk_path,
            "Apple SDK",
        ),
        (
            &report.cargo_home_path,
            &provenance.cargo_home_path,
            "Cargo home",
        ),
        (&report.cargo_path, &provenance.cargo_path, "Cargo path"),
        (
            &report.cargo_sha256,
            &provenance.cargo_sha256,
            "Cargo digest",
        ),
        (
            &report.cargo_vendor_path,
            &provenance.cargo_vendor_path,
            "Cargo vendor",
        ),
        (
            &report.clang_resource_dir_path,
            &provenance.clang_resource_dir_path,
            "Clang resource directory",
        ),
        (
            &report.git_exec_path,
            &provenance.git_exec_path,
            "Git exec path",
        ),
        (&report.git_path, &provenance.git_path, "Git path"),
        (&report.git_sha256, &provenance.git_sha256, "Git digest"),
        (&report.gpg_path, &provenance.gpg_path, "GPG path"),
        (&report.gpg_sha256, &provenance.gpg_sha256, "GPG digest"),
        (&report.linker_path, &provenance.linker_path, "linker"),
        (
            &report.linker_sha256,
            &provenance.linker_sha256,
            "linker digest",
        ),
        (&report.python_path, &provenance.python_path, "Python path"),
        (
            &report.python_sha256,
            &provenance.python_sha256,
            "Python digest",
        ),
        (&report.rustc_path, &provenance.rustc_path, "rustc path"),
        (
            &report.rustc_sha256,
            &provenance.rustc_sha256,
            "rustc digest",
        ),
        (
            &report.rustc_sysroot_path,
            &provenance.rustc_sysroot_path,
            "Rust sysroot",
        ),
    ] {
        if report_value != provenance_value {
            bail!("sealed candidate report {label} differs from production provenance");
        }
    }
    let _ = path_from_receipt(&report.binary_path, "candidate report binary path")?;
    let _ = path_from_receipt(&report.target_dir, "candidate report target directory")?;
    Ok(())
}

fn validate_independent_publication_closure(
    trust: &SemanticTrustV1,
    receipt: &RootPublishedGeneratedCandidateReceiptV1,
    receipt_sha256: &str,
    receipt_path: &Path,
    production_closure_root: &Path,
    source_identity: &source_closure::ValidatedSourceIdentity,
) -> Outcome {
    let root = path_from_receipt(&receipt.artifact_root, "generated artifact root")?;
    if receipt_path != root.join(std::ffi::OsStr::from_bytes(GENERATED_ROOT_RECEIPT_NAME))
        || root.file_name().and_then(std::ffi::OsStr::to_str)
            != Some(receipt.artifact_tree_sha256.as_str())
    {
        bail!("generated artifact root is not its exact content-addressed publication");
    }
    let generated_policy = canonical_closure::HardenedTreePolicy {
        trusted_uid: 0,
        root_mode: 0o555,
        directory_mode: 0o555,
        allowed_file_modes: &[0o444],
        allow_internal_symlinks: false,
    };
    canonical_closure::validate_trusted_parent_chain(&root, 0)?;
    let generated_before =
        canonical_closure::inventory_hardened_tree(&root, generated_policy, None)?;
    let expected_top = [
        b"candidate".to_vec(),
        GENERATION_WORKER_LAUNCH_NAME.to_vec(),
        b"resource-report".to_vec(),
        GENERATED_ROOT_RECEIPT_NAME.to_vec(),
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    if canonical_closure::exact_top_level(&generated_before) != expected_top {
        bail!("generated artifact top-level inventory is not exact");
    }
    if hex::encode(
        tree_entry(
            &generated_before,
            GENERATED_ROOT_RECEIPT_NAME,
            "generated root receipt",
        )?
        .sha256,
    ) != receipt_sha256
        || receipt_sha256 != trust.root_published_generated_candidate_receipt_sha256
    {
        bail!("generated root receipt bytes differ from the signed semantic-trust pin");
    }
    let generated_digest_entries = generated_before
        .iter()
        .filter(|entry| entry.relative != GENERATED_ROOT_RECEIPT_NAME)
        .cloned()
        .collect::<Vec<_>>();
    if canonical_closure::flat_tree_digest(
        &generated_digest_entries,
        canonical_closure::GENERATED_ARTIFACT_DOMAIN,
    )? != receipt.artifact_tree_sha256
    {
        bail!("generated artifact root digest does not recompute to its publication identity");
    }
    let candidate_entries = canonical_closure::subtree_entries(&generated_before, b"candidate");
    exact_file_inventory(
        &candidate_entries,
        &CANDIDATE_GENERATED_FILES,
        0o444,
        "generated candidate subtree",
    )?;
    if canonical_closure::flat_tree_digest(
        &candidate_entries,
        canonical_closure::GENERATED_SUBTREE_DOMAIN,
    )? != receipt.candidate_tree_sha256
    {
        bail!("generated candidate subtree digest differs");
    }
    let report_entries = canonical_closure::subtree_entries(&generated_before, b"resource-report");
    exact_file_inventory(
        &report_entries,
        &GENERATION_REPORT_FILES,
        0o444,
        "generation resource-report subtree",
    )?;
    if canonical_closure::flat_tree_digest(
        &report_entries,
        canonical_closure::GENERATED_SUBTREE_DOMAIN,
    )? != receipt.generation_resource_report_tree_sha256
    {
        bail!("generation resource-report subtree digest differs");
    }
    let summary = tree_entry(
        &report_entries,
        b"kagemusha_resource_summary.json",
        "generation summary",
    )?;
    if hex::encode(summary.sha256) != receipt.generation_summary_sha256 {
        bail!("generation summary digest differs from its authenticated subtree");
    }
    let launch = capture_inventory_typed::<GenerationWorkerLaunchReceiptV1>(
        &root,
        GENERATION_WORKER_LAUNCH_NAME,
        &generated_before,
        16 * 1024 * 1024,
        "generation worker launch receipt",
    )?;
    let launch = launch.value;
    let worker_root = path_from_receipt(&launch.worker_root, "generation worker output root")?;
    let run_leaf = worker_root
        .parent()
        .and_then(Path::parent)
        .and_then(Path::file_name)
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or_default();
    let canonical_run_leaf = run_leaf.strip_prefix("run-").is_some_and(|suffix| {
        suffix.len() == 32
            && suffix
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    });
    if launch.schema != GENERATION_WORKER_LAUNCH_SCHEMA
        || launch.build_uid != receipt.build_uid
        || launch.build_user_name != receipt.build_user_name
        || launch.candidate_build_receipt_path != receipt.candidate_build_receipt_path
        || launch.candidate_build_receipt_sha256 != receipt.candidate_build_receipt_sha256
        || launch.candidate_output_leaf != "candidate"
        || launch.resource_report_leaf != "resource-report"
        || !is_lower_nonzero_sha256(&launch.generation_command_sha256)
        || launch.storage_available_bytes_at_admission < launch.storage_minimum_available_bytes
        || launch.storage_available_bytes_after_generation < launch.storage_post_build_reserve_bytes
        || launch.storage_available_bytes_at_admission > MAX_RECORDED_STORAGE_BYTES
        || launch.storage_available_bytes_after_generation > MAX_RECORDED_STORAGE_BYTES
        || launch.storage_minimum_available_bytes > MAX_RECORDED_STORAGE_BYTES
        || launch.storage_post_build_reserve_bytes > MAX_RECORDED_STORAGE_BYTES
        || launch.storage_minimum_available_bytes < MINIMUM_GENERATION_STORAGE_BYTES
        || launch.storage_post_build_reserve_bytes < MINIMUM_GENERATION_RESERVE_BYTES
        || launch.storage_minimum_available_bytes <= launch.storage_post_build_reserve_bytes
        || launch.storage_device == 0
        || launch.storage_device != launch.worker_root_device
        || launch.worker_root_device == 0
        || launch.worker_root_inode == 0
        || worker_root.file_name().and_then(std::ffi::OsStr::to_str) != Some("generation-output")
        || worker_root
            .parent()
            .and_then(Path::file_name)
            .and_then(std::ffi::OsStr::to_str)
            != Some("worker")
        || !canonical_run_leaf
    {
        bail!("generation worker launch receipt contract is not exact");
    }
    if hex::encode(
        tree_entry(
            &generated_before,
            GENERATION_WORKER_LAUNCH_NAME,
            "generation worker launch receipt",
        )?
        .sha256,
    ) != receipt.worker_launch_receipt_sha256
    {
        bail!("generation worker launch receipt digest differs from its tree entry");
    }

    let candidate_receipt_path = path_from_receipt(
        &receipt.candidate_build_receipt_path,
        "candidate build receipt path",
    )?;
    let candidate_root = candidate_receipt_path
        .parent()
        .ok_or_else(|| eyre!("candidate build receipt has no artifact root"))?
        .to_path_buf();
    let candidate_policy = canonical_closure::HardenedTreePolicy {
        trusted_uid: 0,
        root_mode: 0o555,
        directory_mode: 0o555,
        allowed_file_modes: &[0o444, 0o555],
        allow_internal_symlinks: false,
    };
    canonical_closure::validate_trusted_parent_chain(&candidate_root, 0)?;
    let candidate_before =
        canonical_closure::inventory_hardened_tree(&candidate_root, candidate_policy, None)?;
    let candidate_expected = [
        CANDIDATE_BUILD_BINARY_NAME.to_vec(),
        CANDIDATE_BUILD_REPORT_NAME.to_vec(),
        CANDIDATE_BUILD_RECEIPT_NAME.to_vec(),
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    let candidate_observed = candidate_before
        .iter()
        .map(|entry| entry.relative.clone())
        .collect::<BTreeSet<_>>();
    if candidate_observed != candidate_expected
        || candidate_before.iter().any(|entry| {
            entry.kind != canonical_closure::TreeEntryKind::File
                || if entry.relative == CANDIDATE_BUILD_BINARY_NAME {
                    entry.mode != 0o555
                } else {
                    entry.mode != 0o444
                }
        })
    {
        bail!("candidate build artifact inventory, types, or modes are not exact");
    }
    let candidate_receipt = capture_inventory_typed::<CandidateBuildReceiptV1>(
        &candidate_root,
        CANDIDATE_BUILD_RECEIPT_NAME,
        &candidate_before,
        16 * 1024 * 1024,
        "candidate build receipt",
    )?;
    if hex::encode(
        tree_entry(
            &candidate_before,
            CANDIDATE_BUILD_RECEIPT_NAME,
            "candidate build receipt",
        )?
        .sha256,
    ) != receipt.candidate_build_receipt_sha256
        || receipt.candidate_build_receipt_sha256 != trust.candidate_build_receipt_sha256
    {
        bail!("candidate build receipt bytes differ from the signed publication chain");
    }
    let candidate_receipt = candidate_receipt.value;
    if candidate_receipt.schema != CANDIDATE_BUILD_RECEIPT_SCHEMA
        || candidate_receipt.publication_protocol != ROOT_ATOMIC_PUBLICATION_PROTOCOL
        || candidate_receipt.artifact_root != candidate_root.to_string_lossy()
        || candidate_root.file_name().and_then(std::ffi::OsStr::to_str)
            != Some(candidate_receipt.artifact_tree_sha256.as_str())
        || candidate_receipt.artifact_tree_sha256 != receipt.candidate_build_artifact_tree_sha256
        || candidate_receipt.build_uid != receipt.build_uid
        || candidate_receipt.build_user_name != receipt.build_user_name
        || candidate_receipt.production_closure_tree_sha256
            != receipt.production_closure_tree_sha256
        || candidate_receipt.reviewed_source_closure_descriptor_sha256
            != source_identity.descriptor_sha256
        || candidate_receipt.source_commit != source_identity.source_commit
        || candidate_receipt.source_tree_sha256 != source_identity.source_tree_sha256
        || candidate_receipt.toolchain_provenance_sha256 != receipt.toolchain_provenance_sha256
        || candidate_receipt.binary_path
            != candidate_root
                .join(std::ffi::OsStr::from_bytes(CANDIDATE_BUILD_BINARY_NAME))
                .to_string_lossy()
        || candidate_receipt.sealed_build_report_path
            != candidate_root
                .join(std::ffi::OsStr::from_bytes(CANDIDATE_BUILD_REPORT_NAME))
                .to_string_lossy()
    {
        bail!("candidate build receipt does not bind the exact content-addressed artifact");
    }
    let candidate_digest_entries = candidate_before
        .iter()
        .filter(|entry| entry.relative != CANDIDATE_BUILD_RECEIPT_NAME)
        .cloned()
        .collect::<Vec<_>>();
    if canonical_closure::candidate_artifact_digest(&candidate_digest_entries)?
        != candidate_receipt.artifact_tree_sha256
    {
        bail!("candidate build artifact digest does not independently recompute");
    }
    let binary_entry = tree_entry(
        &candidate_before,
        CANDIDATE_BUILD_BINARY_NAME,
        "candidate build executable",
    )?;
    let report_entry = tree_entry(
        &candidate_before,
        CANDIDATE_BUILD_REPORT_NAME,
        "sealed candidate build report",
    )?;
    if candidate_receipt.binary_size_bytes == 0
        || candidate_receipt.binary_size_bytes != binary_entry.size
        || candidate_receipt.binary_sha256 != hex::encode(binary_entry.sha256)
        || candidate_receipt.sealed_build_report_sha256 != hex::encode(report_entry.sha256)
    {
        bail!("candidate build receipt file bindings differ from the authenticated artifact");
    }

    let production_root = canonical_closure::validate_absolute_normalized(
        production_closure_root,
        "--production-closure-root",
    )?;
    if fs::canonicalize(&production_root).wrap_err("production closure root is unavailable")?
        != production_root
        || production_root
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            != Some(receipt.production_closure_tree_sha256.as_str())
    {
        bail!("production closure root is not its exact canonical content-addressed path");
    }
    let production_policy = canonical_closure::HardenedTreePolicy {
        trusted_uid: 0,
        root_mode: 0o555,
        directory_mode: 0o555,
        allowed_file_modes: &[0o444, 0o555],
        allow_internal_symlinks: true,
    };
    canonical_closure::validate_trusted_parent_chain(&production_root, 0)?;
    let production_before =
        canonical_closure::inventory_hardened_tree(&production_root, production_policy, None)?;
    if canonical_closure::production_closure_digest(&production_before)?
        != receipt.production_closure_tree_sha256
        || receipt.production_closure_tree_sha256 != trust.production_closure_tree_sha256
    {
        bail!("production closure digest does not independently recompute to its trust pin");
    }
    let provenance = capture_inventory_typed::<ProductionClosureProvenanceV1>(
        &production_root,
        canonical_closure::PRODUCTION_PROVENANCE_FILE_NAME,
        &production_before,
        4 * 1024 * 1024,
        "production closure provenance",
    )?;
    let provenance_sha256 = provenance.sha256;
    if provenance_sha256 != receipt.toolchain_provenance_sha256
        || provenance_sha256 != trust.toolchain_provenance_sha256
    {
        bail!("production closure provenance digest differs from the signed trust chain");
    }
    let provenance = provenance.value;
    validate_production_provenance(
        &provenance,
        &provenance_sha256,
        &production_root,
        &production_before,
        &receipt.production_closure_tree_sha256,
        source_identity,
    )?;
    let closure_source_identity = source_closure::validate_reviewed_source(
        Path::new(&provenance.source_root),
        Path::new(&provenance.reviewed_source_closure_path),
        &source_identity.descriptor_sha256,
        &source_identity.source_commit,
        &source_identity.source_tree_sha256,
    )?;
    if closure_source_identity != *source_identity {
        bail!("production closure source tree differs from the external reviewed source");
    }

    let candidate_report = capture_inventory_typed::<CandidateBuildReportV1>(
        &candidate_root,
        CANDIDATE_BUILD_REPORT_NAME,
        &candidate_before,
        16 * 1024 * 1024,
        "sealed candidate build report",
    )?;
    validate_candidate_report(
        &candidate_report.value,
        &candidate_receipt,
        &provenance,
        &production_root,
        source_identity,
    )?;

    if canonical_closure::inventory_hardened_tree(&root, generated_policy, None)?
        != generated_before
        || canonical_closure::inventory_hardened_tree(&candidate_root, candidate_policy, None)?
            != candidate_before
        || canonical_closure::inventory_hardened_tree(&production_root, production_policy, None)?
            != production_before
    {
        bail!("publication closure changed while independently validated");
    }
    Ok(())
}

fn validate_phase_one_readiness_evidence(
    evidence: &PhaseOneReadinessEvidenceV1,
    trusted_roster: &[ValidatorRosterEntryV1],
    expected_phase_one_instructions_sha256: &str,
    expected_release_policy_sha256: &str,
    activation_instructions: &[InstructionBox],
    spec: &ResourceSpecV1,
    trust: &SemanticTrustV1,
    expected_source_commit: &str,
) -> Outcome {
    let expectation = PhaseOneReadinessExpectationV1 {
        kind: "taira-phase-one-four-validator-readiness-v1",
        readiness_stage: "phase_one",
        chain_id: spec.chain_id.clone(),
        expected_artifact_sha256: trust.expected_validator_artifact_sha256.clone(),
        phase_one_instructions_sha256: expected_phase_one_instructions_sha256.to_owned(),
        phase_two_instructions_sha256: None,
        release_policy_sha256: expected_release_policy_sha256.to_owned(),
        verifier_commitments: phase_one_expected_verifier_commitments(
            spec,
            activation_instructions,
        )?,
        roster_root_sha256: sha256_hex(&typed_json_bytes(&trusted_roster.to_vec())?),
        asset_aliases: vec!["ds#boi.is2".to_owned()],
        evaluated_block_identity: trust.phase_one_identity.clone(),
        expected_source_commit: expected_source_commit.to_owned(),
    };
    validate_phase_one_readiness_evidence_against(evidence, trusted_roster, &expectation)
}

fn validate_final_readiness_evidence(
    evidence: &PhaseOneReadinessEvidenceV1,
    trusted_roster: &[ValidatorRosterEntryV1],
    expected_phase_one_instructions_sha256: &str,
    expected_phase_two_instructions_sha256: &str,
    expected_release_policy_sha256: &str,
    spec: &ResourceSpecV1,
    trust: &SemanticTrustV1,
    expected_source_commit: &str,
) -> Outcome {
    let expectation = PhaseOneReadinessExpectationV1 {
        kind: "taira-final-four-validator-readiness-v1",
        readiness_stage: "final",
        chain_id: spec.chain_id.clone(),
        expected_artifact_sha256: trust.expected_validator_artifact_sha256.clone(),
        phase_one_instructions_sha256: expected_phase_one_instructions_sha256.to_owned(),
        phase_two_instructions_sha256: Some(expected_phase_two_instructions_sha256.to_owned()),
        release_policy_sha256: expected_release_policy_sha256.to_owned(),
        verifier_commitments: phase_two_expected_verifier_commitments(spec)?,
        roster_root_sha256: sha256_hex(&typed_json_bytes(&trusted_roster.to_vec())?),
        asset_aliases: vec!["ds#boi.is2".to_owned(), "ds#boi.is".to_owned()],
        evaluated_block_identity: trust.final_identity.clone(),
        expected_source_commit: expected_source_commit.to_owned(),
    };
    validate_phase_one_readiness_evidence_against(evidence, trusted_roster, &expectation)
}

struct PhaseOneReadinessExpectationV1 {
    kind: &'static str,
    readiness_stage: &'static str,
    chain_id: ChainId,
    expected_artifact_sha256: String,
    phase_one_instructions_sha256: String,
    phase_two_instructions_sha256: Option<String>,
    release_policy_sha256: String,
    verifier_commitments: BTreeMap<String, String>,
    roster_root_sha256: String,
    asset_aliases: Vec<String>,
    evaluated_block_identity: BlockIdentityPinV1,
    expected_source_commit: String,
}

fn validate_phase_one_readiness_evidence_against(
    evidence: &PhaseOneReadinessEvidenceV1,
    trusted_roster: &[ValidatorRosterEntryV1],
    expectation: &PhaseOneReadinessExpectationV1,
) -> Outcome {
    if evidence.schema_version != SCHEMA_VERSION
        || evidence.kind != expectation.kind
        || evidence.readiness_stage != expectation.readiness_stage
        || evidence.reviewed_iroha_commit != expectation.expected_source_commit
        || evidence.bridge_abi_version != 21
        || evidence.release_manifest_version != 4
        || evidence.cash_handoff_capability != "cash_handoff_v1"
        || evidence.phase_one_instructions_sha256 != expectation.phase_one_instructions_sha256
        || evidence.phase_two_instructions_sha256 != expectation.phase_two_instructions_sha256
    {
        bail!(
            "{} readiness evidence does not bind the exact reviewed ABI-21/V4 cash_handoff_v1 \
             candidate",
            expectation.readiness_stage
        );
    }
    if evidence.validators.len() != 4 {
        bail!(
            "{} readiness evidence must contain exactly four validator attestations",
            expectation.readiness_stage
        );
    }
    let trusted = trusted_roster
        .iter()
        .map(|entry| (entry.validator_public_key.clone(), entry.torii_port))
        .collect::<BTreeSet<_>>();
    let mut attested = BTreeSet::new();
    for attestation in &evidence.validators {
        let body = &attestation.body;
        if !body.ready
            || body.readiness_stage != expectation.readiness_stage
            || body.artifact_sha256 != expectation.expected_artifact_sha256
            || body.chain_id != expectation.chain_id
            || body.reviewed_iroha_commit != expectation.expected_source_commit
            || body.phase_one_instructions_sha256 != expectation.phase_one_instructions_sha256
            || body.phase_two_instructions_sha256 != expectation.phase_two_instructions_sha256
            || body.bridge_abi_version != 21
            || body.release_manifest_version != 4
            || body.cash_handoff_capability != "cash_handoff_v1"
            || body.asset_aliases != expectation.asset_aliases
            || body.release_policy_sha256 != expectation.release_policy_sha256
            || body.verifier_commitments != expectation.verifier_commitments
            || body.roster_root_sha256 != expectation.roster_root_sha256
            || body.evaluated_block_height
                != expectation.evaluated_block_identity.evaluated_block_height
            || body.evaluated_block_hash
                != expectation.evaluated_block_identity.evaluated_block_hash
            || !trusted.contains(&(body.validator_public_key.clone(), body.torii_port))
            || !attested.insert((body.validator_public_key.clone(), body.torii_port))
        {
            bail!(
                "{} readiness evidence contains an untrusted, duplicate, non-ready, malformed, \
                 or independently unpinned validator attestation",
                expectation.readiness_stage
            );
        }
        let signing_bytes = readiness_signing_bytes(body)?;
        attestation
            .signature
            .verify(&body.validator_public_key, &signing_bytes)
            .wrap_err_with(|| {
                format!(
                    "invalid readiness signature from validator {}",
                    body.validator_public_key
                )
            })?;
    }
    if attested != trusted {
        bail!(
            "{} readiness evidence does not cover the exact trusted validator roster",
            expectation.readiness_stage
        );
    }
    Ok(())
}

fn phase_one_expected_verifier_commitments(
    spec: &ResourceSpecV1,
    activation_instructions: &[InstructionBox],
) -> color_eyre::Result<BTreeMap<String, String>> {
    if activation_instructions.len() != 1 {
        bail!("phase-one readiness requires exactly one activation instruction");
    }
    let activation = activation_instructions[0]
        .as_any()
        .downcast_ref::<ActivateKagemushaRecursiveReleaseV4>()
        .ok_or_else(|| eyre!("phase-one readiness activation has the wrong instruction type"))?
        .activation();
    let mut commitments = spec
        .base_verifiers
        .iter()
        .map(|verifier| {
            (
                verifier.id.name.clone(),
                hex::encode(verifier.record.commitment),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for (role, record) in [
        (
            RECURSIVE_VERIFIER_ROLES[0],
            &activation.step_eq_verifier_record,
        ),
        (
            RECURSIVE_VERIFIER_ROLES[1],
            &activation.step_ep_verifier_record,
        ),
    ] {
        if commitments
            .insert(role.to_owned(), hex::encode(record.commitment))
            .is_some()
        {
            bail!("phase-one readiness verifier roles are not distinct");
        }
    }
    if commitments.len() != 5
        || commitments.values().collect::<BTreeSet<_>>().len() != commitments.len()
    {
        bail!("phase-one readiness requires five cryptographically distinct commitments");
    }
    Ok(commitments)
}

fn phase_two_expected_verifier_commitments(
    spec: &ResourceSpecV1,
) -> color_eyre::Result<BTreeMap<String, String>> {
    let activation = spec
        .phase_two_offline_bootstrap
        .manifest()
        .release
        .activation();
    let mut commitments = spec
        .base_verifiers
        .iter()
        .map(|verifier| {
            (
                verifier.id.name.clone(),
                hex::encode(verifier.record.commitment),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for (role, record) in [
        (
            RECURSIVE_VERIFIER_ROLES[0],
            &activation.step_eq_verifier_record,
        ),
        (
            RECURSIVE_VERIFIER_ROLES[1],
            &activation.step_ep_verifier_record,
        ),
    ] {
        if commitments
            .insert(role.to_owned(), hex::encode(record.commitment))
            .is_some()
        {
            bail!("final readiness verifier roles are not distinct");
        }
    }
    if commitments.len() != 5
        || commitments.values().collect::<BTreeSet<_>>().len() != commitments.len()
    {
        bail!("final readiness requires five cryptographically distinct commitments");
    }
    Ok(commitments)
}

fn readiness_signing_bytes(body: &ValidatorReadinessBodyV1) -> color_eyre::Result<Vec<u8>> {
    const DOMAIN: &[u8] = b"iroha:taira-dual-state:validator-readiness:v1\0";
    let body = typed_json_bytes(body)?;
    let mut bytes = Vec::with_capacity(DOMAIN.len() + body.len());
    bytes.extend_from_slice(DOMAIN);
    bytes.extend_from_slice(&body);
    Ok(bytes)
}

fn is_lower_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_lower_nonzero_sha256(value: &str) -> bool {
    is_lower_sha256(value) && value.bytes().any(|byte| byte != b'0')
}

fn load_public_nexus_config_bytes(
    bytes: &[u8],
    logical_path: &Path,
) -> color_eyre::Result<actual::Nexus> {
    let text = std::str::from_utf8(bytes).wrap_err("reviewed Taira config is not valid UTF-8")?;
    let mut root = text
        .parse::<toml::Table>()
        .wrap_err("reviewed Taira config is not valid TOML")?;
    let nexus = root
        .remove("nexus")
        .and_then(|value| match value {
            toml::Value::Table(table) => Some(table),
            _ => None,
        })
        .ok_or_else(|| eyre!("reviewed Taira config omits the [nexus] table"))?;
    // `logical_path` is diagnostic metadata only. The TOML source table was
    // parsed from the one retained byte capture above and is never reopened.
    let source = TomlSource::new(logical_path.to_path_buf(), nexus);
    let user = ConfigReader::new()
        .with_toml_source(source)
        .read_and_complete::<user::Nexus>()
        .map_err(|error| eyre!("failed to decode reviewed public Nexus config: {error:?}"))?;
    let mut emitter = Emitter::new();
    let nexus = user.parse(&mut emitter);
    emitter
        .into_result()
        .map_err(|error| eyre!("reviewed public Nexus config is invalid: {error:?}"))?;
    nexus.ok_or_else(|| eyre!("reviewed public Nexus config did not produce a runtime value"))
}

fn parse_base_genesis_bytes(bytes: &[u8]) -> color_eyre::Result<RawGenesisTransaction> {
    let text = std::str::from_utf8(bytes).wrap_err("reviewed base genesis is not valid UTF-8")?;
    let raw: norito::json::Value =
        norito::json::from_str(text).wrap_err("reviewed base genesis is not valid Norito JSON")?;
    let object = raw
        .as_object()
        .ok_or_else(|| eyre!("reviewed base genesis must be one JSON object"))?;
    if object
        .get("executor")
        .is_some_and(|executor| !executor.is_null())
    {
        bail!(
            "reviewed base genesis cannot reference an external executor when admitted from exact bytes"
        );
    }
    let transactions = object
        .get("transactions")
        .and_then(norito::json::Value::as_array)
        .ok_or_else(|| eyre!("reviewed base genesis omits its transaction array"))?;
    if transactions.is_empty()
        || transactions.iter().any(|transaction| {
            transaction
                .get("ivm_triggers")
                .and_then(norito::json::Value::as_array)
                .is_some_and(|triggers| !triggers.is_empty())
        })
    {
        bail!(
            "reviewed base genesis must be nonempty and cannot reference external IVM trigger bytes"
        );
    }
    iroha_genesis::init_instruction_registry();
    norito::json::from_str::<RawGenesisTransaction>(text)
        .wrap_err("reviewed base genesis failed typed Iroha decoding")
}

fn validate_base_genesis_typed(genesis: &RawGenesisTransaction) -> Outcome {
    if genesis.chain_id().as_str() != TAIRA_CHAIN_ID
        || genesis.chain_discriminant() != TAIRA_CHAIN_DISCRIMINANT
    {
        bail!("reviewed base genesis does not target the exact Taira chain/discriminant");
    }
    let mut is2_domains = BTreeSet::new();
    for instruction in genesis.instructions() {
        if let Some(register) = instruction
            .as_any()
            .downcast_ref::<Register<iroha_data_model::domain::Domain>>()
        {
            let literal = register.object().id().to_string();
            if literal.ends_with(".is2") {
                is2_domains.insert(literal);
            } else if literal.ends_with(".is") {
                bail!("reviewed base genesis prematurely registers .is domain {literal}");
            }
        }
        if let Some(alias) = instruction
            .as_any()
            .downcast_ref::<SetAssetDefinitionAlias>()
            .and_then(|instruction| instruction.alias().as_ref())
            .map(ToString::to_string)
            && (alias == "ds#boi.is" || alias == "ds#boi.is2")
        {
            bail!("reviewed base genesis prematurely binds Digital Shekel alias {alias}");
        }
        if let Some(register) = instruction
            .as_any()
            .downcast_ref::<Register<iroha_data_model::asset::AssetDefinition>>()
            && (register.object().id() == &asset_id_for_phase(1)?
                || register.object().id() == &asset_id_for_phase(2)?)
        {
            bail!("reviewed base genesis prematurely registers a Digital Shekel asset");
        }
    }
    let expected = [
        "boi.is2",
        "leumi.is2",
        "hapoalim.is2",
        "discount.is2",
        "mizrahi.is2",
        "fibi.is2",
        "onezero.is2",
        "jerusalem.is2",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<BTreeSet<_>>();
    if is2_domains != expected {
        bail!("reviewed base genesis must contain exactly the eight approved .is2 domains");
    }
    Ok(())
}

fn validate_validator_roster_against_base_genesis(
    genesis: &RawGenesisTransaction,
    roster: &[ValidatorRosterEntryV1],
) -> Outcome {
    let topology_keys = genesis
        .transactions()
        .iter()
        .flat_map(|transaction| transaction.topology())
        .map(|entry| entry.peer.public_key().clone())
        .collect::<BTreeSet<_>>();
    let roster_keys = roster
        .iter()
        .map(|entry| entry.validator_public_key.clone())
        .collect::<BTreeSet<_>>();
    if topology_keys.len() != 4 || topology_keys != roster_keys {
        bail!(
            "resource validator roster must contain exactly the four public keys signed into the reviewed base-genesis topology"
        );
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn execute_candidate_semantics(
    spec: &ResourceSpecV1,
    base_genesis_bytes: &[u8],
    _base_genesis_logical_path: &Path,
    base_config_bytes: &[u8],
    base_config_logical_path: &Path,
    release_policy_path: &Path,
    artifact_root: &Path,
    release_catalog: KagemushaReleaseCatalogV4,
    phase_one: &[InstructionBox],
    phase_one_activation: &[InstructionBox],
    phase_two: &[InstructionBox],
) -> color_eyre::Result<SemanticReadinessOutcome> {
    let genesis = parse_base_genesis_bytes(base_genesis_bytes)?;
    validate_base_genesis_typed(&genesis)?;
    validate_validator_roster_against_base_genesis(&genesis, &spec.validator_roster)?;
    let genesis_batches = genesis
        .clone()
        .parse()
        .wrap_err("reviewed base genesis failed normalized instruction derivation")?;
    let nexus = load_public_nexus_config_bytes(base_config_bytes, base_config_logical_path)?;
    validate_required_dataspaces(&nexus)?;
    if nexus.fees.fee_asset_id
        != spec
            .phase_one
            .alias_lease_governance
            .payment
            .expected_payment_asset
            .to_string()
    {
        bail!(
            "phase-one offline command funding asset does not equal the reviewed Nexus fee asset"
        );
    }

    let query = LiveQueryStore::start_test();
    let mut state = State::new_with_pre_genesis_nexus_and_chain_for_testing(
        World::new(),
        nexus,
        query,
        spec.chain_id.clone(),
    );
    let phase_one_escrows = BTreeMap::from([(
        asset_id_for_phase(1)?,
        spec.phase_one.accounts.escrow.clone(),
    )]);
    state.set_settlement(settlement_for_semantic_validation(
        phase_one_escrows.clone(),
        release_policy_path,
        artifact_root,
    ));
    state.set_kagemusha_release_catalog(release_catalog);

    let now_ms: u64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .wrap_err("system clock precedes Unix epoch")?
        .as_millis()
        .try_into()
        .wrap_err("current Unix timestamp does not fit u64")?;
    if now_ms
        >= spec
            .phase_one
            .alias_lease_governance
            .payment
            .quote_valid_until_ms
        || now_ms
            >= spec
                .phase_two
                .alias_lease_governance
                .payment
                .quote_valid_until_ms
    {
        bail!("alias quote guards have expired at semantic validation time");
    }

    let mut genesis_block = state.block(BlockHeader::new(
        NonZeroU64::new(1).expect("one is nonzero"),
        None,
        None,
        None,
        now_ms,
        0,
    ));
    for (index, batch) in genesis_batches.iter().enumerate() {
        execute_instruction_batch(
            &mut genesis_block,
            &spec.base_genesis_authority,
            Some(DataSpaceId::UNIVERSAL),
            &format!("base genesis transaction {index}"),
            batch,
        )?;
    }
    execute_instruction_batch(
        &mut genesis_block,
        &spec.phase_one.alias_lease_governance.transaction_authority,
        Some(DataSpaceId::new(IS2_DATASPACE_ID)),
        "phase-one is2 resources",
        phase_one,
    )?;
    execute_instruction_batch(
        &mut genesis_block,
        &spec.phase_one.alias_lease_governance.transaction_authority,
        Some(DataSpaceId::new(IS2_DATASPACE_ID)),
        "phase-one ABI-21/V4 activation",
        phase_one_activation,
    )?;
    genesis_block
        .commit()
        .wrap_err("failed to commit staged height-one genesis semantics")?;
    validate_phase_post_state(&state, spec, 1)?;
    let issuer_public_key = spec
        .phase_one
        .accounts
        .issuer
        .try_signatory()
        .ok_or_else(|| eyre!("phase-one issuer must be a sole-signatory account"))?
        .clone();
    let phase_one_policy = mandatory_offline_policy_from_reviewed_public_inputs(
        &spec.chain_id,
        phase_one_escrows,
        spec.phase_one.accounts.issuer.clone(),
        issuer_public_key,
        spec.offline_command_fee_funding_amount.clone(),
    )
    .map_err(|error| eyre!("phase-one mandatory offline policy is invalid: {error}"))?;
    let peer_id = PeerId::new(
        spec.validator_roster
            .first()
            .expect("validated four-validator roster")
            .validator_public_key
            .clone(),
    );
    let phase_one_status =
        evaluate_committed_mandatory_offline(&state.view(), &phase_one_policy, Some(&peer_id))
            .map_err(|error| eyre!("phase-one offline snapshot is invalid: {error}"))?;
    ensure_mandatory_offline_ready(&phase_one_status)
        .map_err(|error| eyre!("phase-one mandatory offline readiness failed: {error}"))?;

    let final_escrows = BTreeMap::from([
        (
            asset_id_for_phase(1)?,
            spec.phase_one.accounts.escrow.clone(),
        ),
        (
            asset_id_for_phase(2)?,
            spec.phase_two.accounts.escrow.clone(),
        ),
    ]);
    state.set_settlement(settlement_for_semantic_validation(
        final_escrows.clone(),
        release_policy_path,
        artifact_root,
    ));
    let mut phase_two_block = state.block(BlockHeader::new(
        NonZeroU64::new(2).expect("two is nonzero"),
        None,
        None,
        None,
        now_ms.saturating_add(1),
        0,
    ));
    execute_instruction_batch(
        &mut phase_two_block,
        &spec.phase_two.alias_lease_governance.transaction_authority,
        Some(DataSpaceId::new(IS_DATASPACE_ID)),
        "phase-two is resources",
        phase_two,
    )?;
    phase_two_block
        .commit()
        .wrap_err("failed to commit staged phase-two semantics")?;
    validate_phase_post_state(&state, spec, 2)?;
    let activation_block = state.block(BlockHeader::new(
        NonZeroU64::new(3).expect("three is nonzero"),
        None,
        None,
        None,
        now_ms.saturating_add(2),
        0,
    ));
    activation_block
        .commit()
        .wrap_err("failed to commit the phase-two release activation-height block")?;
    let final_policy = mandatory_offline_policy_from_reviewed_public_inputs(
        &spec.chain_id,
        final_escrows,
        spec.phase_one.accounts.issuer.clone(),
        spec.phase_one
            .accounts
            .issuer
            .try_signatory()
            .expect("validated sole-signatory issuer")
            .clone(),
        spec.offline_command_fee_funding_amount.clone(),
    )
    .map_err(|error| eyre!("final mandatory offline policy is invalid: {error}"))?;
    let final_status =
        evaluate_committed_mandatory_offline(&state.view(), &final_policy, Some(&peer_id))
            .map_err(|error| eyre!("final offline snapshot is invalid: {error}"))?;
    ensure_mandatory_offline_ready(&final_status)
        .map_err(|error| eyre!("final mandatory offline readiness failed: {error}"))?;
    Ok(SemanticReadinessOutcome {
        phase_one: phase_one_status,
        final_state: final_status,
    })
}

fn settlement_for_semantic_validation(
    escrow_accounts: BTreeMap<AssetDefinitionId, AccountId>,
    release_policy_path: &Path,
    artifact_root: &Path,
) -> actual::Settlement {
    let mut settlement = actual::Settlement::default();
    settlement.offline.escrow_accounts = escrow_accounts;
    settlement.offline.kagemusha_release_policy_path = Some(release_policy_path.to_path_buf());
    settlement.offline.kagemusha_artifact_dir = Some(artifact_root.to_path_buf());
    settlement
}

fn semantic_checks_from_readiness(
    outcome: &SemanticReadinessOutcome,
) -> color_eyre::Result<SemanticChecksV1> {
    ensure_mandatory_offline_ready(&outcome.phase_one)
        .map_err(|error| eyre!("phase-one readiness regressed before report creation: {error}"))?;
    ensure_mandatory_offline_ready(&outcome.final_state)
        .map_err(|error| eyre!("final readiness regressed before report creation: {error}"))?;
    let expected_phase_one = BTreeSet::from([IS2_ASSET_ID.to_owned()]);
    let expected_final = BTreeSet::from([IS2_ASSET_ID.to_owned(), IS_ASSET_ID.to_owned()]);
    let phase_one_assets = outcome
        .phase_one
        .assets
        .iter()
        .map(|asset| asset.asset_definition_id.clone())
        .collect::<BTreeSet<_>>();
    let final_assets = outcome
        .final_state
        .assets
        .iter()
        .map(|asset| asset.asset_definition_id.clone())
        .collect::<BTreeSet<_>>();
    if phase_one_assets != expected_phase_one || final_assets != expected_final {
        bail!(
            "mandatory readiness did not transition from exactly ds#boi.is2 to exactly the is2/is pair"
        );
    }
    for asset in outcome
        .phase_one
        .assets
        .iter()
        .chain(outcome.final_state.assets.iter())
    {
        let verifiers = [
            asset.active_transfer_verifier.as_ref(),
            asset.active_topup_shield_verifier.as_ref(),
            asset.active_unshield_verifier.as_ref(),
            asset.active_recursive_step_eq_verifier.as_ref(),
            asset.active_recursive_step_ep_verifier.as_ref(),
        ];
        let verifiers = verifiers
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| {
                eyre!(
                    "ready offline asset {} omits one of five verifier roles",
                    asset.asset_definition_id
                )
            })?;
        let expected_roles = BASE_VERIFIER_ROLES
            .into_iter()
            .chain(RECURSIVE_VERIFIER_ROLES)
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        let observed_roles = verifiers
            .iter()
            .map(|verifier| {
                if verifier.id.backend != "halo2/ipa"
                    || !is_lower_nonzero_sha256(&verifier.commitment)
                    || !is_lower_nonzero_sha256(&verifier.public_inputs_schema_hash)
                {
                    bail!(
                        "asset {} exposes a malformed verifier identity or commitment",
                        asset.asset_definition_id
                    );
                }
                Ok(verifier.id.name.clone())
            })
            .collect::<color_eyre::Result<BTreeSet<_>>>()?;
        let distinct_ids = verifiers
            .iter()
            .map(|verifier| (verifier.id.backend.clone(), verifier.id.name.clone()))
            .collect::<BTreeSet<_>>();
        let distinct_commitments = verifiers
            .iter()
            .map(|verifier| verifier.commitment.clone())
            .collect::<BTreeSet<_>>();
        if observed_roles != expected_roles
            || distinct_ids.len() != 5
            || distinct_commitments.len() != 5
            || !asset.proof_backend_available
            || !asset.recursive_lineage_supported
            || asset.asset_scale != Some(2)
            || asset.artifact_set.is_none()
        {
            bail!(
                "asset {} does not expose five distinct reviewed halo2/ipa roles and authenticated recursive lineage",
                asset.asset_definition_id
            );
        }
    }
    if outcome.phase_one.cash_handoff_capability != "cash_handoff_v1"
        || outcome.final_state.cash_handoff_capability != "cash_handoff_v1"
        || outcome.phase_one.required_bridge_abi_version != 21
        || outcome.final_state.required_bridge_abi_version != 21
    {
        bail!("mandatory readiness does not expose exact cash_handoff_v1 ABI-21 identity");
    }
    Ok(SemanticChecksV1 {
        canonical_iroha_instructions: true,
        base_genesis_exact_is2_domains: true,
        base_genesis_contains_no_is_domains: true,
        phase_one_contains_no_is_namespace_mutation: true,
        phase_two_contains_no_is2_namespace_mutation: true,
        is2_asset_fixed_scale: 2,
        is_asset_fixed_scale: 2,
        cash_handoff_capability: "cash_handoff_v1".to_owned(),
        bridge_abi_version: 21,
        release_manifest_version: 4,
        proof_backend: "halo2/ipa".to_owned(),
        recursive_lineage: true,
        five_distinct_verifier_roles: true,
        device_attestation_policy_governed: true,
        spend_authority_hardware_backed: true,
        issuer_authorized_and_funded: true,
        all_fi_reserves_funded: true,
        sns_aliases_use_governed_ensure_alias: true,
        sns_alias_transaction_authority_is_payer: true,
        sns_alias_authority_prefunded_for_aggregate_cap: true,
        sns_alias_lifecycle_is_explicit_and_fail_closed: true,
        account_alias_ensure_immediately_precedes_legacy_binding: true,
        phase_two_domains_use_dependency_ordered_ensure_alias: true,
        phase_two_requires_phase_one_four_validator_readiness: true,
    })
}

fn validate_required_dataspaces(nexus: &actual::Nexus) -> Outcome {
    let expected = [
        ("is", DataSpaceId::new(IS_DATASPACE_ID)),
        ("is2", DataSpaceId::new(IS2_DATASPACE_ID)),
    ];
    for (alias, expected_id) in expected {
        let observed = nexus
            .dataspace_catalog
            .by_alias(alias)
            .map(|entry| entry.id)
            .ok_or_else(|| eyre!("reviewed Nexus config omits {alias} dataspace"))?;
        if observed != expected_id {
            bail!("reviewed Nexus config maps {alias} to {observed}, expected {expected_id}");
        }
    }
    Ok(())
}

fn execute_instruction_batch(
    block: &mut StateBlock<'_>,
    authority: &AccountId,
    dataspace_id: Option<DataSpaceId>,
    label: &str,
    instructions: &[InstructionBox],
) -> Outcome {
    let mut transaction = block.transaction();
    transaction.bind_execution_context_for_testing(
        dataspace_id,
        Some(iroha_crypto::Hash::new(label.as_bytes())),
    );
    transaction.bind_direct_signed_instruction_authority_for_testing(Some(authority.clone()));
    let executor = transaction.world.executor().clone();
    let execution_result =
        instructions
            .iter()
            .cloned()
            .enumerate()
            .try_for_each(|(index, instruction)| {
                executor
                    .execute_instruction(&mut transaction, authority, instruction)
                    .map_err(|error| {
                        eyre!("{label} instruction[{index}] failed pinned Core execution: {error}")
                    })
            });
    transaction.bind_direct_signed_instruction_authority_for_testing(None);
    execution_result?;
    transaction.apply();
    Ok(())
}

fn validate_phase_post_state(state: &State, spec: &ResourceSpecV1, phase_number: u8) -> Outcome {
    let phase = if phase_number == 1 {
        &spec.phase_one
    } else {
        &spec.phase_two
    };
    let asset_id = asset_id_for_phase(phase_number)?;
    let view = state.view();
    let world = view.world();
    let definition = world.asset_definition(&asset_id).map_err(|error| {
        eyre!("phase-{phase_number} post-state omits Digital Shekel definition: {error}")
    })?;
    if definition.spec().scale() != Some(2) {
        bail!("phase-{phase_number} post-state Digital Shekel scale is not exactly 2");
    }
    let namespace = if phase_number == 1 { "is2" } else { "is" };
    let expected_alias: AssetDefinitionAlias = format!("ds#boi.{namespace}")
        .parse()
        .expect("reviewed asset alias");
    if world.asset_definition_id_by_alias(&expected_alias) != Some(asset_id.clone()) {
        bail!(
            "phase-{phase_number} post-state omits the authoritative {expected_alias} asset binding"
        );
    }
    if !world.contains_zk_asset(&asset_id) {
        bail!("phase-{phase_number} post-state omits the Digital Shekel ZK asset binding");
    }
    if state.settlement().offline.escrow_accounts.get(&asset_id) != Some(&phase.accounts.escrow) {
        bail!("phase-{phase_number} post-state escrow binding differs from the reviewed account");
    }
    let issuer_permissions = world
        .account_permissions_iter(&phase.accounts.issuer)
        .map_err(|error| eyre!("phase-{phase_number} issuer permissions unavailable: {error}"))?
        .map(|permission| permission.name())
        .collect::<BTreeSet<_>>();
    if !OFFLINE_ISSUER_PERMISSIONS
        .into_iter()
        .all(|required| issuer_permissions.contains(required))
    {
        bail!(
            "phase-{phase_number} post-state issuer lacks one or more exact offline lifecycle permissions"
        );
    }
    for account in
        std::iter::once(phase.accounts.issuer.clone()).chain(FI_NAMES.into_iter().map(|name| {
            phase
                .accounts
                .reserves
                .get(name)
                .expect("validated reserve")
                .clone()
        }))
    {
        let balance = world
            .asset(&AssetId::of(asset_id.clone(), account.clone()))
            .map(|asset| asset.value().as_ref().clone())
            .unwrap_or_else(|_| Quantity::zero());
        if balance < spec.funding_amount {
            bail!(
                "phase-{phase_number} post-state account {account} is not funded to the required \
                 amount"
            );
        }
    }
    Ok(())
}

fn validate_resource_spec(spec: &ResourceSpecV1) -> Outcome {
    if spec.schema_version != SCHEMA_VERSION {
        bail!("resource specification schema_version must be {SCHEMA_VERSION}");
    }
    if spec.chain_id.as_str() != TAIRA_CHAIN_ID {
        bail!("resource specification chain_id must be exact reviewed Taira chain");
    }
    if spec.chain_discriminant != TAIRA_CHAIN_DISCRIMINANT {
        bail!("resource specification chain_discriminant must be {TAIRA_CHAIN_DISCRIMINANT}");
    }
    if spec.funding_amount <= Quantity::zero() {
        bail!("resource specification funding_amount must be positive");
    }
    if spec.offline_command_fee_funding_amount <= Quantity::zero() {
        bail!("resource specification offline_command_fee_funding_amount must be positive");
    }
    validate_phase_spec(spec, &spec.phase_one, 1)?;
    validate_phase_spec(spec, &spec.phase_two, 2)?;
    if spec.phase_one.alias_lease_governance.transaction_authority != spec.base_genesis_authority {
        bail!("phase-one alias authority must equal the exact base-genesis transaction authority");
    }
    if spec.phase_one.accounts.treasury != spec.base_genesis_authority {
        bail!(
            "phase-one treasury must register the exact base-genesis transaction authority before \
             any alias prefunding or lease acquisition"
        );
    }
    if spec.phase_two.alias_lease_governance.transaction_authority
        != spec.phase_one.accounts.treasury
    {
        bail!(
            "phase-two alias authority must be the already-registered phase-one treasury account"
        );
    }
    if spec.phase_two.accounts.treasury
        != spec.phase_two.alias_lease_governance.transaction_authority
    {
        bail!(
            "phase-two treasury must reuse the already-registered phase-one treasury authority so \
            post-bootstrap reserve funding is authorized by the asset owner"
        );
    }
    let phase_one_accounts = phase_accounts_in_alias_order(&spec.phase_one)
        .into_iter()
        .collect::<BTreeSet<_>>();
    if phase_accounts_in_alias_order(&spec.phase_two)
        .into_iter()
        .filter(|account| account != &spec.phase_two.accounts.treasury)
        .any(|account| phase_one_accounts.contains(&account))
    {
        bail!(
            "phase-two issuer, escrow, and reserve accounts must be new; only the treasury \
             authority is intentionally shared with phase one"
        );
    }
    validate_verifiers(&spec.base_verifiers)?;
    validate_validator_roster(&spec.validator_roster)?;
    validate_phase_two_offline_bootstrap(spec)?;
    Ok(())
}

fn validate_phase_two_offline_bootstrap(spec: &ResourceSpecV1) -> Outcome {
    let enactment = &spec.phase_two_offline_bootstrap;
    let manifest = enactment.manifest();
    let expected_asset = asset_id_for_phase(2)?;
    let expected_definition = asset_definition_for_phase(spec, 2)?;
    let expected_zk_asset = zk_asset_for_phase(spec, 2)?;
    let expected_allocations = std::iter::once(spec.phase_two.accounts.issuer.clone())
        .chain(FI_NAMES.into_iter().map(|name| {
            spec.phase_two
                .accounts
                .reserves
                .get(name)
                .expect("validated reserve")
                .clone()
        }))
        .map(|account| (account, spec.funding_amount.clone()))
        .collect::<BTreeMap<_, _>>();
    let expected_prefix_hash =
        iroha_data_model::isi::offline::offline_asset_bootstrap_prefix_fingerprint(
            &generate_phase_resource_prefix(spec, 2)?,
        );
    if manifest.chain_id != spec.chain_id
        || manifest.enactment_authority
            != spec.phase_two.alias_lease_governance.transaction_authority
        || manifest.parliament_selection_height != 2
        || manifest.parliament_roster_root == [0_u8; 32]
        || manifest.preceding_instructions_hash != expected_prefix_hash
        || manifest.dataspace_id != DataSpaceId::new(IS_DATASPACE_ID)
        || manifest.asset_definition_owner != spec.phase_two.accounts.treasury
        || manifest.issuer != spec.phase_two.accounts.issuer
        || manifest.initial_allocations != expected_allocations
        || manifest.escrow != spec.phase_two.accounts.escrow
        || typed_json_bytes(&manifest.asset_definition)? != typed_json_bytes(&expected_definition)?
        || typed_json_bytes(&manifest.zk_asset)? != typed_json_bytes(&expected_zk_asset)?
    {
        bail!(
            "phase-two governed offline bootstrap must bind the exact chain, height-two \
             Parliament roster, directly signed transaction authority, complete preceding \
             instruction prefix, is dataspace, asset definition, ZK policy, owner, funded \
             issuer/FI reserves, and deterministic escrow"
        );
    }
    let release = manifest.release.activation();
    if release.release_record.manifest.chain_id != spec.chain_id
        || release.release_record.manifest.asset != expected_asset
        || release.release_record.manifest.asset_scale != 2
        || release.release_record.manifest.version != 4
        || release.release_record.manifest.bridge_abi_version != 21
        || release.release_record.manifest.proof_backend != "halo2/ipa"
        || release.release_record.promotion_record.bridge_abi_version != 21
    {
        bail!(
            "phase-two governed offline bootstrap must carry the exact ds#boi.is \
             ABI-21/V4 halo2/ipa release"
        );
    }
    release
        .release_record
        .validate_structure()
        .map_err(|error| eyre!("phase-two governed release structure is invalid: {error}"))?;
    let certificate = enactment.certificate();
    if certificate.payload.preimage_hash != manifest.fingerprint()
        || certificate.payload.at_window.lower != 2
        || certificate.payload.at_window.upper != 2
        || certificate.signatures.scheme != EnactmentSignatureScheme::SimpleThreshold
        || certificate.signatures.signatures.is_empty()
        || !certificate
            .signatures
            .signatures
            .windows(2)
            .all(|pair| pair[0].signer < pair[1].signer)
    {
        bail!(
            "phase-two Parliament certificate must bind the exact bootstrap fingerprint, exact \
             height two, and an ordered non-empty signer set"
        );
    }
    Ok(())
}

fn validate_validator_roster(roster: &[ValidatorRosterEntryV1]) -> Outcome {
    if roster.len() != 4 {
        bail!("resource specification validator_roster must contain exactly four records");
    }
    let expected_ports = [29_080_u16, 29_081, 29_082, 29_083];
    if roster
        .iter()
        .map(|entry| entry.torii_port)
        .ne(expected_ports)
    {
        bail!(
            "resource specification validator_roster must bind the exact ordered Torii ports \
             29080, 29081, 29082, and 29083"
        );
    }
    let public_keys = roster
        .iter()
        .map(|entry| entry.validator_public_key.clone())
        .collect::<BTreeSet<_>>();
    if public_keys.len() != roster.len() {
        bail!("resource specification validator_roster contains a duplicate public key");
    }
    Ok(())
}

fn validate_phase_spec(
    spec: &ResourceSpecV1,
    phase: &PhaseResourceSpecV1,
    phase_number: u8,
) -> Outcome {
    let expected_context = if phase_number == 1 {
        "genesis"
    } else {
        "ordinary_transaction"
    };
    let governance = &phase.alias_lease_governance;
    if governance.execution_context != expected_context {
        bail!(
            "phase-{phase_number} alias governance execution_context must be \
             {expected_context:?}"
        );
    }
    if governance.payment.charge_source != "transaction_authority"
        || governance.payment.balance_requirement != "prefunded_for_aggregate_cap"
        || governance.payment.expected_policy_version == 0
        || governance.payment.max_amount_per_resource <= Quantity::zero()
        || governance.payment.quote_valid_until_ms == 0
    {
        bail!("phase-{phase_number} alias payment governance is incomplete or unbounded");
    }
    if governance.lifecycle.term_years == 0
        || governance.lifecycle.renewal_mode != "manual_guarded"
        || governance.lifecycle.renewal_authority != governance.transaction_authority
        || governance.lifecycle.renew_before_expiry_ms == 0
        || governance.lifecycle.expiry_behavior != "fail_closed"
    {
        bail!("phase-{phase_number} alias lifecycle must be explicit and fail closed");
    }
    let lease_ms = u64::from(governance.lifecycle.term_years)
        .checked_mul(31_536_000_000)
        .ok_or_else(|| eyre!("phase-{phase_number} alias lease duration overflows"))?;
    if governance.lifecycle.renew_before_expiry_ms >= lease_ms {
        bail!("phase-{phase_number} renew_before_expiry_ms must be shorter than the lease term");
    }
    let asset_id = asset_id_for_phase(phase_number)?;
    let expected_escrow = offline_escrow_account_id(&spec.chain_id, &asset_id);
    if phase.accounts.escrow != expected_escrow {
        bail!(
            "phase-{phase_number} escrow must equal deterministic offline escrow \
             {expected_escrow}"
        );
    }
    let expected_reserves = FI_NAMES
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    if phase
        .accounts
        .reserves
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>()
        != expected_reserves
    {
        bail!(
            "phase-{phase_number} reserves must contain exactly leumi, hapoalim, discount, \
             mizrahi, fibi, onezero, and jerusalem"
        );
    }
    let accounts = phase_accounts_in_alias_order(phase);
    if accounts.iter().collect::<BTreeSet<_>>().len() != accounts.len() {
        bail!("phase-{phase_number} BOI/FI resource accounts must all be distinct");
    }
    Ok(())
}

fn validate_verifiers(verifiers: &[VerifierSpecV1]) -> Outcome {
    if verifiers.len() != BASE_VERIFIER_ROLES.len() {
        bail!("resource specification must contain exactly three base verifier records");
    }
    let mut names = BTreeSet::new();
    let mut commitments = BTreeSet::new();
    for verifier in verifiers {
        if verifier.id.backend.as_str() != "halo2/ipa"
            || !BASE_VERIFIER_ROLES.contains(&verifier.id.name.as_str())
            || !names.insert(verifier.id.name.clone())
        {
            bail!("base verifier IDs must be the three distinct reviewed halo2/ipa role names");
        }
        let record = &verifier.record;
        let (expected_circuit_id, expected_schema_hash) = match verifier.id.name.as_str() {
            role if role == BASE_VERIFIER_ROLES[0] => (
                iroha_core::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID,
                <[u8; 32]>::from(Hash::new(
                    iroha_core::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1,
                )),
            ),
            role if role == BASE_VERIFIER_ROLES[1] => (
                iroha_core::zk::confidential_v2::KAGEMUSHA_TOPUP_SHIELD_V2_CIRCUIT_ID,
                <[u8; 32]>::from(Hash::new(
                    iroha_core::zk::confidential_v2::KAGEMUSHA_TOPUP_SHIELD_V2_PUBLIC_INPUTS_SCHEMA_V2,
                )),
            ),
            role if role == BASE_VERIFIER_ROLES[2] => (
                iroha_core::zk::confidential_v2::CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID,
                <[u8; 32]>::from(Hash::new(
                    iroha_core::zk::confidential_v2::CONFIDENTIAL_UNSHIELD_V3_PUBLIC_INPUTS_SCHEMA_V1,
                )),
            ),
            _ => unreachable!("role membership checked above"),
        };
        let key = record
            .key
            .as_ref()
            .ok_or_else(|| eyre!("verifier {} omits inline key bytes", verifier.id.name))?;
        if record.version == 0
            || record.circuit_id != expected_circuit_id
            || record.backend != BackendTag::Halo2IpaPasta
            || record.curve != "pallas"
            || record.public_inputs_schema_hash != expected_schema_hash
            || record.max_proof_bytes == 0
            || record.max_proof_bytes
                > iroha_core::zk::confidential_v2::CONFIDENTIAL_V2_MAX_PROOF_BYTES
            || record.activation_height.is_some_and(|height| height > 1)
            || record.withdraw_height.is_some_and(|height| height <= 1)
            || record.status != ConfidentialStatus::Active
            || key.bytes.is_empty()
            || usize::try_from(record.vk_len).ok() != Some(key.bytes.len())
            || iroha_core::zk::hash_vk(key) != record.commitment
            || !commitments.insert(record.commitment)
        {
            bail!(
                "verifier {} must be the exact active, distinct, commitment-matched halo2/ipa Pallas circuit record",
                verifier.id.name
            );
        }
    }
    Ok(())
}

fn generate_resources(spec: &ResourceSpecV1) -> color_eyre::Result<GeneratedResources> {
    let phase_one = generate_phase_resources(spec, 1)?;
    let phase_two = generate_phase_resources(spec, 2)?;
    Ok(GeneratedResources {
        phase_one,
        phase_two,
    })
}

fn generate_phase_resources(
    spec: &ResourceSpecV1,
    phase_number: u8,
) -> color_eyre::Result<Vec<InstructionBox>> {
    let mut instructions = generate_phase_resource_prefix(spec, phase_number)?;
    if phase_number == 2 {
        let observed = iroha_data_model::isi::offline::offline_asset_bootstrap_prefix_fingerprint(
            &instructions,
        );
        if spec
            .phase_two_offline_bootstrap
            .manifest()
            .preceding_instructions_hash
            != observed
        {
            bail!(
                "phase-two governed enactment does not bind the exact generated instruction prefix"
            );
        }
        instructions.push(spec.phase_two_offline_bootstrap.clone().into());
    }
    Ok(instructions)
}

fn generate_phase_resource_prefix(
    spec: &ResourceSpecV1,
    phase_number: u8,
) -> color_eyre::Result<Vec<InstructionBox>> {
    let phase = if phase_number == 1 {
        &spec.phase_one
    } else {
        &spec.phase_two
    };
    let namespace = if phase_number == 1 { "is2" } else { "is" };
    let dataspace_id = DataSpaceId::new(if phase_number == 1 {
        IS2_DATASPACE_ID
    } else {
        IS_DATASPACE_ID
    });
    let asset_id = asset_id_for_phase(phase_number)?;
    let mut instructions = Vec::new();

    if phase_number == 2 {
        instructions.push(
            governed_ensure_alias(
                AliasIntentV1::Dataspace(AliasDataSpaceIntentV1 {
                    dataspace: ResolvedDataSpaceV1::new(
                        "is".parse().expect("reviewed dataspace name"),
                        dataspace_id,
                    ),
                    owner: phase.alias_lease_governance.transaction_authority.clone(),
                }),
                &phase.alias_lease_governance,
            )
            .into(),
        );
        for domain in IS_DOMAINS {
            instructions.push(
                governed_ensure_alias(
                    AliasIntentV1::Domain(AliasDomainIntentV1 {
                        domain: ResolvedDomainV1::new(
                            DomainId::parse_fully_qualified(domain)
                                .expect("reviewed domain is canonical"),
                            dataspace_id,
                        ),
                        owner: phase.alias_lease_governance.transaction_authority.clone(),
                    }),
                    &phase.alias_lease_governance,
                )
                .into(),
            );
        }
    }

    for (purpose, account) in phase_account_aliases(phase, namespace)? {
        if phase_number == 2 && account == spec.phase_one.accounts.treasury {
            continue;
        }
        let mut metadata = Metadata::default();
        metadata.insert(
            "purpose".parse().expect("metadata key"),
            Json::new(purpose.clone()),
        );
        let new_account: NewAccount = Account::new(account).with_metadata(metadata);
        instructions.push(Register::account(new_account).into());
    }

    if phase_number == 1 {
        instructions.push(
            Register::asset_definition(asset_definition_for_phase(spec, phase_number)?).into(),
        );
    }

    if phase_number == 1 {
        for (governance, count) in [
            (&spec.phase_one.alias_lease_governance, 10_usize),
            (&spec.phase_two.alias_lease_governance, 19_usize),
        ] {
            instructions.push(
                Mint::asset_quantity(
                    repeated_quantity(&governance.payment.max_amount_per_resource, count)?,
                    AssetId::of(
                        governance.payment.expected_payment_asset.clone(),
                        governance.transaction_authority.clone(),
                    ),
                )
                .into(),
            );
        }
        instructions.push(
            Mint::asset_quantity(
                spec.offline_command_fee_funding_amount.clone(),
                AssetId::of(
                    spec.phase_one
                        .alias_lease_governance
                        .payment
                        .expected_payment_asset
                        .clone(),
                    spec.phase_one.accounts.issuer.clone(),
                ),
            )
            .into(),
        );
    }

    if phase_number == 1 {
        for permission_name in OFFLINE_ISSUER_PERMISSIONS {
            instructions.push(
                Grant::account_permission(
                    Permission::new(
                        permission_name.parse().expect("reviewed permission name"),
                        Json::new(()),
                    ),
                    phase.accounts.issuer.clone(),
                )
                .into(),
            );
        }
        instructions.push(
            Grant::account_permission(
                Permission::new(
                    "CanManageVerifyingKeys"
                        .parse()
                        .expect("reviewed permission name"),
                    Json::new(()),
                ),
                phase.alias_lease_governance.transaction_authority.clone(),
            )
            .into(),
        );
        for verifier in &spec.base_verifiers {
            instructions.push(
                RegisterVerifyingKey {
                    id: verifier.id.clone(),
                    record: verifier.record.clone(),
                }
                .into(),
            );
        }
        instructions.push(zk_asset_for_phase(spec, phase_number)?.into());
    }

    for (literal, account) in phase_account_aliases(phase, namespace)? {
        let canonical_name = literal
            .parse()
            .wrap_err_with(|| format!("reviewed account alias {literal} is invalid"))?;
        let resolved = ResolvedAccountAliasV1::new(canonical_name, dataspace_id);
        instructions.push(
            governed_ensure_alias(
                AliasIntentV1::AccountAlias(AliasAccountIntentV1 {
                    alias: resolved.clone(),
                    target_account: account.clone(),
                    provision: AccountProvisionV1::Existing,
                    role: AccountAliasRoleV1::Additional,
                }),
                &phase.alias_lease_governance,
            )
            .into(),
        );
        instructions
            .push(SetAccountAliasBinding::bind(account, resolved.account_alias(), None).into());
    }

    if phase_number == 1 {
        let funded_accounts = std::iter::once(phase.accounts.issuer.clone()).chain(
            FI_NAMES.into_iter().map(|name| {
                phase
                    .accounts
                    .reserves
                    .get(name)
                    .expect("validated reserve")
                    .clone()
            }),
        );
        for account in funded_accounts {
            instructions.push(
                Mint::asset_quantity(
                    spec.funding_amount.clone(),
                    AssetId::of(asset_id.clone(), account),
                )
                .into(),
            );
        }
    }
    Ok(instructions)
}

fn phase_accounts_in_alias_order(phase: &PhaseResourceSpecV1) -> Vec<AccountId> {
    let mut accounts = vec![
        phase.accounts.treasury.clone(),
        phase.accounts.escrow.clone(),
        phase.accounts.issuer.clone(),
    ];
    accounts.extend(FI_NAMES.into_iter().map(|name| {
        phase
            .accounts
            .reserves
            .get(name)
            .expect("validated reserve")
            .clone()
    }));
    accounts
}

fn phase_account_aliases(
    phase: &PhaseResourceSpecV1,
    namespace: &str,
) -> color_eyre::Result<Vec<(String, AccountId)>> {
    let mut aliases = vec![
        (
            format!("treasury@boi.{namespace}"),
            phase.accounts.treasury.clone(),
        ),
        (
            format!("offline-escrow@boi.{namespace}"),
            phase.accounts.escrow.clone(),
        ),
        (
            format!("issuance@boi.{namespace}"),
            phase.accounts.issuer.clone(),
        ),
    ];
    for name in FI_NAMES {
        aliases.push((
            format!("reserve@{name}.{namespace}"),
            phase
                .accounts
                .reserves
                .get(name)
                .ok_or_else(|| eyre!("missing reserve account for {name}"))?
                .clone(),
        ));
    }
    Ok(aliases)
}

fn governed_ensure_alias(
    intent: AliasIntentV1,
    governance: &AliasLeaseGovernanceV1,
) -> EnsureAlias {
    EnsureAlias::new(
        intent,
        AliasLeaseAcquisitionV1::new(
            governance.lifecycle.term_years,
            governance.lifecycle.pricing_class_hint,
        ),
        AliasQuoteGuardV1 {
            expected_policy_version: governance.payment.expected_policy_version,
            expected_payment_asset: governance.payment.expected_payment_asset.clone(),
            max_amount: governance.payment.max_amount_per_resource.clone(),
            valid_until_ms: governance.payment.quote_valid_until_ms,
        },
    )
}

fn repeated_quantity(value: &Quantity, count: usize) -> color_eyre::Result<Quantity> {
    let mut total = Quantity::zero();
    for _ in 0..count {
        total = total
            .try_add(value)
            .wrap_err("aggregate alias quote cap overflows Quantity")?;
    }
    Ok(total)
}

fn asset_id_for_phase(phase_number: u8) -> color_eyre::Result<AssetDefinitionId> {
    let literal = if phase_number == 1 {
        IS2_ASSET_ID
    } else {
        IS_ASSET_ID
    };
    literal
        .parse()
        .wrap_err_with(|| format!("reviewed phase-{phase_number} asset ID is invalid"))
}

fn asset_definition_for_phase(
    spec: &ResourceSpecV1,
    phase_number: u8,
) -> color_eyre::Result<NewAssetDefinition> {
    let namespace = if phase_number == 1 { "is2" } else { "is" };
    let mut metadata = Metadata::default();
    metadata.insert(
        "offline.enabled".parse().expect("metadata key"),
        Json::new(true),
    );
    metadata.insert(
        "currency.code".parse().expect("metadata key"),
        Json::new("DS".to_owned()),
    );
    metadata.insert(
        "offline.validator_roster_sha256"
            .parse()
            .expect("metadata key"),
        Json::new(sha256_hex(&typed_json_bytes(&spec.validator_roster)?)),
    );
    Ok(NewAssetDefinition {
        id: asset_id_for_phase(phase_number)?,
        name: "ds".to_owned(),
        description: Some("Digital Shekel".to_owned()),
        alias: Some(
            format!("ds#boi.{namespace}")
                .parse::<AssetDefinitionAlias>()
                .expect("reviewed asset alias"),
        ),
        spec: NumericSpec::fractional(2),
        mintable: Mintable::Infinitely,
        logo: None,
        metadata,
        balance_scope_policy: AssetBalancePolicy::Global,
        confidential_policy: AssetConfidentialPolicy::convertible(),
    })
}

fn zk_asset_for_phase(
    spec: &ResourceSpecV1,
    phase_number: u8,
) -> color_eyre::Result<RegisterZkAsset> {
    let verifier_id = |role: &str| {
        spec.base_verifiers
            .iter()
            .find(|verifier| verifier.id.name == role)
            .map(|verifier| verifier.id.clone())
            .expect("validated verifier role")
    };
    Ok(RegisterZkAsset::new(
        asset_id_for_phase(phase_number)?,
        ZkAssetMode::Hybrid,
        true,
        true,
        Some(verifier_id(BASE_VERIFIER_ROLES[0])),
        Some(verifier_id(BASE_VERIFIER_ROLES[2])),
        Some(verifier_id(BASE_VERIFIER_ROLES[1])),
    ))
}

fn instruction_bytes(instructions: &[InstructionBox]) -> color_eyre::Result<Vec<u8>> {
    canonical_json_bytes(&genesis_instructions_json::instructions_to_value(
        instructions,
    ))
}

fn typed_json_bytes<T: norito::json::JsonSerialize>(value: &T) -> color_eyre::Result<Vec<u8>> {
    let value =
        norito::json::value::to_value(value).wrap_err("failed to convert typed value to JSON")?;
    canonical_json_bytes(&value)
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(kagemusha_recursive_spend_release_sha256(bytes))
}

fn create_new_private_directory(path: &Path) -> color_eyre::Result<PathBuf> {
    if !path.is_absolute() {
        bail!("--output-dir must be an absolute path");
    }
    let parent = path
        .parent()
        .ok_or_else(|| eyre!("--output-dir has no parent"))?;
    let parent_metadata =
        fs::symlink_metadata(parent).wrap_err("output directory parent is unavailable")?;
    if parent_metadata.file_type().is_symlink() || !parent_metadata.is_dir() {
        bail!("output directory parent must be an existing non-symlink directory");
    }
    if fs::symlink_metadata(path).is_ok() {
        bail!("refusing to overwrite or follow existing --output-dir");
    }
    fs::create_dir(path).wrap_err("failed to create new dual-state output directory")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .wrap_err("failed to set private output directory permissions")?;
    }
    path.canonicalize()
        .wrap_err("failed to canonicalize new dual-state output directory")
}

fn write_new_private_file(path: &Path, bytes: &[u8]) -> Outcome {
    let parent = path
        .parent()
        .ok_or_else(|| eyre!("output path has no parent"))?;
    let parent_metadata =
        fs::symlink_metadata(parent).wrap_err("output file parent is unavailable")?;
    if parent_metadata.file_type().is_symlink() || !parent_metadata.is_dir() {
        bail!("output file parent must be a non-symlink directory");
    }
    let canonical_parent = parent
        .canonicalize()
        .wrap_err("failed to canonicalize output file parent")?;
    let file_name = path
        .file_name()
        .ok_or_else(|| eyre!("output path has no file name"))?;
    let target = canonical_parent.join(file_name);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options
        .open(&target)
        .wrap_err("refusing to overwrite or follow an existing output file")?;
    file.write_all(bytes)
        .wrap_err("failed to write dual-state output file")?;
    file.sync_all()
        .wrap_err("failed to durably sync dual-state output file")?;
    fs::File::open(&canonical_parent)
        .and_then(|directory| directory.sync_all())
        .wrap_err("failed to durably sync dual-state output directory")?;
    Ok(())
}

fn read_instruction_file(path: &Path, label: &str) -> color_eyre::Result<Vec<InstructionBox>> {
    let (bytes, _) = canonical_closure::stable_file_bytes(path, 64 * 1024 * 1024)
        .wrap_err_with(|| format!("failed to read {label}"))?;
    let text =
        std::str::from_utf8(&bytes).wrap_err_with(|| format!("{label} is not valid UTF-8"))?;
    let value: norito::json::Value = norito::json::from_str(text)
        .wrap_err_with(|| format!("{label} is not valid canonical Norito JSON"))?;
    let canonical_input = canonical_json_bytes(&value)?;
    if bytes != canonical_input {
        bail!(
            "{label} bytes are not canonical Norito JSON (sorted compact object keys and one \
             trailing newline are required)"
        );
    }
    let instructions = genesis_instructions_json::from_value(&value)
        .wrap_err_with(|| format!("{label} do not decode as registered Iroha instructions"))?;
    let roundtrip = genesis_instructions_json::instructions_to_value(&instructions);
    let reparsed = genesis_instructions_json::from_value(&roundtrip)
        .wrap_err_with(|| format!("{label} failed typed instruction round-trip"))?;
    if instructions != reparsed {
        bail!("{label} changed across typed instruction round-trip");
    }
    if canonical_json_bytes(&roundtrip)? != canonical_input {
        bail!("{label} changed across canonical typed JSON round-trip");
    }
    Ok(instructions)
}

fn canonical_json_bytes(value: &norito::json::Value) -> color_eyre::Result<Vec<u8>> {
    let mut rendered =
        norito::json::to_json(value).wrap_err("failed to render canonical Norito JSON")?;
    rendered.push('\n');
    Ok(rendered.into_bytes())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        domain::Domain,
        isi::{InstructionBox, Register},
        prelude::DomainId,
    };
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn typed_instruction_roundtrip_rejects_unknown_shapes() {
        let value: norito::json::Value =
            norito::json::from_str(r#"[{"NotAnIrohaInstruction":{}}]"#).expect("fixture JSON");
        assert!(genesis_instructions_json::from_value(&value).is_err());
    }

    #[test]
    fn typed_instruction_roundtrip_accepts_registered_instruction() {
        let domain = DomainId::try_new("fixture", "universal").expect("domain");
        let instructions = vec![InstructionBox::from(Register::domain(Domain::new(domain)))];
        let value = genesis_instructions_json::instructions_to_value(&instructions);
        let decoded = genesis_instructions_json::from_value(&value).expect("typed decode");
        assert_eq!(decoded, instructions);
    }

    #[test]
    fn instruction_file_requires_exact_canonical_bytes() {
        let directory = tempdir().expect("temporary directory");
        let directory_root = directory
            .path()
            .canonicalize()
            .expect("canonical temporary directory");
        let path = directory_root.join("instructions.json");
        let domain = DomainId::try_new("fixture", "universal").expect("domain");
        let instructions = vec![InstructionBox::from(Register::domain(Domain::new(domain)))];
        let canonical = instruction_bytes(&instructions).expect("canonical instructions");

        fs::write(&path, &canonical).expect("write canonical fixture");
        assert_eq!(
            read_instruction_file(&path, "fixture").expect("canonical file"),
            instructions
        );

        fs::write(&path, canonical.strip_suffix(b"\n").expect("trailing LF"))
            .expect("write fixture without LF");
        assert!(
            read_instruction_file(&path, "fixture").is_err(),
            "missing canonical trailing LF must fail closed"
        );

        let mut padded = b" ".to_vec();
        padded.extend_from_slice(&canonical);
        fs::write(&path, padded).expect("write padded fixture");
        assert!(
            read_instruction_file(&path, "fixture").is_err(),
            "non-canonical whitespace must fail closed"
        );
    }

    #[test]
    fn output_creation_is_create_only() {
        let directory = tempdir().expect("temporary directory");
        let output = directory.path().join("generated");
        let created = create_new_private_directory(&output).expect("create output");
        assert_eq!(created, output.canonicalize().expect("canonical output"));
        assert!(
            create_new_private_directory(&output).is_err(),
            "an existing output directory must never be reused"
        );

        let file = output.join("resource.json");
        write_new_private_file(&file, b"first").expect("create output file");
        assert!(
            write_new_private_file(&file, b"second").is_err(),
            "an existing output file must never be overwritten"
        );
        assert_eq!(fs::read(file).expect("read output"), b"first");
    }

    #[cfg(unix)]
    #[test]
    fn output_creation_rejects_symlink_parent() {
        use std::os::unix::fs::symlink;

        let directory = tempdir().expect("temporary directory");
        let real_parent = directory.path().join("real");
        fs::create_dir(&real_parent).expect("create real parent");
        let linked_parent = directory.path().join("linked");
        symlink(&real_parent, &linked_parent).expect("create symlink");
        assert!(
            create_new_private_directory(&linked_parent.join("generated")).is_err(),
            "a symlink parent must fail closed"
        );
    }

    #[cfg(unix)]
    #[test]
    fn supplied_stand_in_verifier_is_rejected_even_with_its_matching_digest() {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempdir().expect("temporary directory");
        let directory_root = directory
            .path()
            .canonicalize()
            .expect("canonical temp root");
        let stand_in = directory_root.join("stand-in-gpg");
        fs::write(&stand_in, b"untrusted verifier stand-in").expect("write stand-in verifier");
        fs::set_permissions(&stand_in, fs::Permissions::from_mode(0o755))
            .expect("mark stand-in executable");
        let stand_in_sha256 =
            stable_pinned_file_digest(&stand_in, 1024, "stand-in verifier").expect("stand-in hash");
        let anchor = OpenPgpVerifierAnchor {
            path: directory_root.join("root-owned-production-closure/gpg"),
            sha256: stand_in_sha256.clone(),
            verifier: canonical_closure::stable_file(&stand_in, 1024)
                .expect("retain stand-in descriptor"),
        };
        let error =
            validate_verifier_invocation_matches_anchor(&stand_in, &stand_in_sha256, &anchor)
                .expect_err("an untrusted path cannot become authoritative through its own digest");
        assert!(
            error
                .to_string()
                .contains("independently authenticated production-closure anchor"),
            "unexpected stand-in rejection: {error}",
        );
    }

    #[test]
    fn replaced_semantic_trust_path_cannot_change_authenticated_parsed_bytes() {
        let directory = tempdir().expect("temporary directory");
        let directory_root = directory
            .path()
            .canonicalize()
            .expect("canonical temp root");
        let path = directory_root.join("semantic-trust.json");
        let replacement_path = directory_root.join("replacement.json");
        let original = SemanticTrustV1 {
            schema_version: SCHEMA_VERSION,
            kind: "taira-dual-state-semantic-trust-v1".to_owned(),
            expected_validator_artifact_sha256: "11".repeat(32),
            phase_one_identity: BlockIdentityPinV1 {
                evaluated_block_height: 1,
                evaluated_block_hash: "22".repeat(32),
            },
            final_identity: BlockIdentityPinV1 {
                evaluated_block_height: 3,
                evaluated_block_hash: "33".repeat(32),
            },
            root_published_generated_candidate_receipt_sha256: "44".repeat(32),
            root_published_generated_candidate_tree_sha256: "55".repeat(32),
            candidate_tree_sha256: "66".repeat(32),
            candidate_build_artifact_tree_sha256: "77".repeat(32),
            candidate_build_receipt_sha256: "88".repeat(32),
            production_closure_tree_sha256: "99".repeat(32),
            reviewed_source_closure_descriptor_sha256: "aa".repeat(32),
            source_commit: "b1".repeat(20),
            source_tree_sha256: "bb".repeat(32),
            toolchain_provenance_sha256: "cc".repeat(32),
            generation_worker_launch_receipt_sha256: "dd".repeat(32),
        };
        let mut replacement = original.clone();
        replacement.expected_validator_artifact_sha256 = "ee".repeat(32);
        fs::write(
            &path,
            typed_json_bytes(&original).expect("original trust bytes"),
        )
        .expect("write original trust");
        let (authenticated_bytes, _) =
            canonical_closure::stable_file_bytes(&path, 16 * 1024 * 1024)
                .expect("authenticate exact trust bytes once");
        fs::write(
            &replacement_path,
            typed_json_bytes(&replacement).expect("replacement trust bytes"),
        )
        .expect("write replacement trust");
        fs::rename(&replacement_path, &path).expect("replace trust path after authentication");

        let parsed: SemanticTrustV1 =
            parse_canonical_typed_bytes(&authenticated_bytes, "captured semantic trust")
                .expect("parse authenticated descriptor bytes");
        assert_eq!(parsed, original);
        let on_disk: SemanticTrustV1 =
            parse_canonical_typed_bytes(&fs::read(&path).expect("read replacement"), "replacement")
                .expect("parse replacement");
        assert_eq!(on_disk, replacement);
        assert_ne!(parsed, on_disk);
    }

    #[test]
    fn genesis_config_and_report_evidence_use_one_exact_capture() {
        let directory = tempdir().expect("temporary directory");
        let directory_root = directory
            .path()
            .canonicalize()
            .expect("canonical temporary directory");
        for (name, original, replacement) in [
            (
                "genesis.json",
                b"original genesis".as_slice(),
                b"replacement genesis".as_slice(),
            ),
            (
                "config.toml",
                b"original config".as_slice(),
                b"replacement config".as_slice(),
            ),
            (
                "readiness-report.json",
                b"original report evidence".as_slice(),
                b"replacement report evidence".as_slice(),
            ),
        ] {
            let path = directory_root.join(name);
            let replacement_path = directory_root.join(format!("{name}.replacement"));
            fs::write(&path, original).expect("write exact input");
            fs::write(&replacement_path, replacement).expect("write replacement input");
            let captured = capture_bytes(&path, 1024, name).expect("capture exact input once");
            fs::rename(&path, directory_root.join(format!("{name}.held")))
                .expect("hold captured inode");
            fs::rename(&replacement_path, &path).expect("replace caller path");

            assert_eq!(captured.bytes, original);
            assert_eq!(captured.sha256, sha256_hex(original));
            assert_eq!(fs::read(&path).expect("read substituted path"), replacement);
        }
    }

    #[test]
    fn report_digest_is_derived_from_exact_serialized_bytes_before_write() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("semantic-report.json");
        let bytes = b"{\"kind\":\"test-report\",\"status\":\"PASS\"}\n";
        let expected = sha256_hex(bytes);
        write_new_private_file(&path, bytes).expect("write exact report bytes");
        assert_eq!(sha256_hex(bytes), expected);
        assert_eq!(fs::read(path).expect("read report fixture"), bytes);
    }

    #[test]
    fn is2_manifest_authorization_signature_is_domain_and_body_bound() {
        let keypair = KeyPair::from_seed(vec![0x51; 32], Algorithm::Ed25519);
        let mut body = Is2ManifestAuthorizationBodyV1 {
            schema_version: SCHEMA_VERSION,
            kind: "taira-is2-signed-genesis-manifest-authorization-v1".to_owned(),
            reviewed_iroha_commit: REVIEWED_IROHA_COMMIT.to_owned(),
            chain_id: ChainId::from(TAIRA_CHAIN_ID),
            base_genesis_sha256: REVIEWED_BASE_GENESIS_SHA256.to_owned(),
            phase_one_instructions_sha256: "ab".repeat(32),
            authority: AccountId::new(keypair.public_key().clone()),
        };
        let signing_bytes =
            is2_manifest_authorization_signing_bytes(&body).expect("authorization preimage");
        let signature = Signature::new(keypair.private_key(), &signing_bytes);
        signature
            .verify(keypair.public_key(), &signing_bytes)
            .expect("exact authorization verifies");

        body.phase_one_instructions_sha256 = "cd".repeat(32);
        assert!(
            signature
                .verify(
                    keypair.public_key(),
                    &is2_manifest_authorization_signing_bytes(&body)
                        .expect("mutated authorization preimage"),
                )
                .is_err(),
            "a phase-one digest mutation must invalidate the signed authorization"
        );
    }

    #[test]
    fn semantic_trust_binds_root_published_source_and_candidate_receipt_chain() {
        let temporary = tempdir().expect("temporary directory");
        let artifact_tree_sha256 = "11".repeat(32);
        let artifact_root = temporary.path().join(&artifact_tree_sha256);
        let candidate_dir = artifact_root.join("candidate");
        let resource_report = artifact_root.join("resource-report");
        fs::create_dir_all(&candidate_dir).expect("candidate directory");
        fs::create_dir_all(&resource_report).expect("resource report directory");

        let candidate_receipt = temporary.path().join("root-published-candidate-build.json");
        fs::write(&candidate_receipt, b"candidate receipt\n").expect("candidate receipt");
        let candidate_receipt_sha256 =
            sha256_file(&candidate_receipt, "candidate receipt").expect("candidate receipt hash");
        let worker_launch = artifact_root.join("generation-worker-launch.json");
        fs::write(&worker_launch, b"worker launch\n").expect("worker launch");
        let worker_launch_sha256 =
            sha256_file(&worker_launch, "worker launch").expect("worker launch hash");
        let generation_summary = resource_report.join("kagemusha_resource_summary.json");
        fs::write(&generation_summary, b"generation summary\n").expect("generation summary");
        let generation_summary_sha256 = sha256_file(&generation_summary, "generation summary")
            .expect("generation summary hash");
        let receipt_path = artifact_root.join("root-published-generated-candidate.json");
        let receipt = RootPublishedGeneratedCandidateReceiptV1 {
            artifact_root: artifact_root.display().to_string(),
            artifact_tree_sha256: artifact_tree_sha256.clone(),
            build_uid: 501,
            build_user_name: "boi-build".to_owned(),
            candidate_build_artifact_tree_sha256: "22".repeat(32),
            candidate_build_receipt_path: candidate_receipt.display().to_string(),
            candidate_build_receipt_sha256: candidate_receipt_sha256.clone(),
            candidate_dir_path: candidate_dir.display().to_string(),
            candidate_tree_sha256: "33".repeat(32),
            generation_resource_report_path: resource_report.display().to_string(),
            generation_resource_report_tree_sha256: "44".repeat(32),
            generation_summary_path: generation_summary.display().to_string(),
            generation_summary_sha256,
            production_closure_tree_sha256: "55".repeat(32),
            provisional_cross_stage_status: PROVISIONAL_CROSS_STAGE_STATUS.to_owned(),
            provisional_generation_publication_status: PROVISIONAL_GENERATION_STATUS.to_owned(),
            publication_protocol: ROOT_ATOMIC_PUBLICATION_PROTOCOL.to_owned(),
            publication_status: ROOT_PUBLISHED_GENERATION_STATUS.to_owned(),
            reviewed_source_closure_descriptor_sha256: "66".repeat(32),
            schema: ROOT_PUBLISHED_GENERATED_CANDIDATE_SCHEMA.to_owned(),
            source_commit: REVIEWED_IROHA_COMMIT.to_owned(),
            source_tree_sha256: "77".repeat(32),
            toolchain_provenance_sha256: "88".repeat(32),
            worker_launch_receipt_sha256: worker_launch_sha256.clone(),
        };
        fs::write(
            &receipt_path,
            typed_json_bytes(&receipt).expect("canonical receipt"),
        )
        .expect("root-published receipt");
        let trust = SemanticTrustV1 {
            schema_version: SCHEMA_VERSION,
            kind: "taira-dual-state-semantic-trust-v1".to_owned(),
            expected_validator_artifact_sha256: "99".repeat(32),
            phase_one_identity: BlockIdentityPinV1 {
                evaluated_block_height: 1,
                evaluated_block_hash: "aa".repeat(32),
            },
            final_identity: BlockIdentityPinV1 {
                evaluated_block_height: 3,
                evaluated_block_hash: "bb".repeat(32),
            },
            root_published_generated_candidate_receipt_sha256: sha256_file(
                &receipt_path,
                "root-published receipt",
            )
            .expect("receipt hash"),
            root_published_generated_candidate_tree_sha256: artifact_tree_sha256,
            candidate_tree_sha256: receipt.candidate_tree_sha256.clone(),
            candidate_build_artifact_tree_sha256: receipt
                .candidate_build_artifact_tree_sha256
                .clone(),
            candidate_build_receipt_sha256: candidate_receipt_sha256,
            production_closure_tree_sha256: receipt.production_closure_tree_sha256.clone(),
            reviewed_source_closure_descriptor_sha256: receipt
                .reviewed_source_closure_descriptor_sha256
                .clone(),
            source_commit: REVIEWED_IROHA_COMMIT.to_owned(),
            source_tree_sha256: receipt.source_tree_sha256.clone(),
            toolchain_provenance_sha256: receipt.toolchain_provenance_sha256.clone(),
            generation_worker_launch_receipt_sha256: worker_launch_sha256,
        };
        let source_identity = source_closure::ValidatedSourceIdentity {
            source_commit: REVIEWED_IROHA_COMMIT.to_owned(),
            source_tree_sha256: receipt.source_tree_sha256.clone(),
            descriptor_sha256: receipt.reviewed_source_closure_descriptor_sha256.clone(),
        };
        validate_semantic_trust_receipt_bindings(
            &trust,
            &receipt,
            &trust.root_published_generated_candidate_receipt_sha256,
            &receipt_path,
            &source_identity,
        )
        .expect("exact independently pinned publication chain");

        let mut substituted = trust;
        substituted.source_tree_sha256 = "cc".repeat(32);
        assert!(
            validate_semantic_trust_receipt_bindings(
                &substituted,
                &receipt,
                &substituted.root_published_generated_candidate_receipt_sha256,
                &receipt_path,
                &source_identity,
            )
            .is_err(),
            "a valid-looking source-tree substitution must fail the independent receipt binding"
        );
    }

    #[test]
    fn four_validator_readiness_requires_valid_identical_signed_artifact() {
        let keypairs = (1_u8..=4)
            .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519))
            .collect::<Vec<_>>();
        let trusted_roster = keypairs
            .iter()
            .zip([29_080_u16, 29_081, 29_082, 29_083])
            .map(|(keypair, torii_port)| ValidatorRosterEntryV1 {
                validator_public_key: keypair.public_key().clone(),
                torii_port,
            })
            .collect::<Vec<_>>();
        let artifact_sha256 = "ab".repeat(32);
        let phase_one_instructions_sha256 = "cd".repeat(32);
        let release_policy_sha256 = "ef".repeat(32);
        let verifier_commitments = BASE_VERIFIER_ROLES
            .into_iter()
            .chain(RECURSIVE_VERIFIER_ROLES)
            .enumerate()
            .map(|(index, role)| (role.to_owned(), format!("{:02x}", index + 1).repeat(32)))
            .collect::<BTreeMap<_, _>>();
        let roster_root_sha256 =
            sha256_hex(&typed_json_bytes(&trusted_roster).expect("roster bytes"));
        let expectation = PhaseOneReadinessExpectationV1 {
            kind: "taira-phase-one-four-validator-readiness-v1",
            readiness_stage: "phase_one",
            chain_id: ChainId::from(TAIRA_CHAIN_ID),
            expected_artifact_sha256: artifact_sha256.clone(),
            phase_one_instructions_sha256: phase_one_instructions_sha256.clone(),
            phase_two_instructions_sha256: None,
            release_policy_sha256: release_policy_sha256.clone(),
            verifier_commitments: verifier_commitments.clone(),
            roster_root_sha256: roster_root_sha256.clone(),
            asset_aliases: vec!["ds#boi.is2".to_owned()],
            evaluated_block_identity: BlockIdentityPinV1 {
                evaluated_block_height: 1,
                evaluated_block_hash: "12".repeat(32),
            },
            expected_source_commit: REVIEWED_IROHA_COMMIT.to_owned(),
        };
        let validators = keypairs
            .iter()
            .zip([29_080_u16, 29_081, 29_082, 29_083])
            .map(|(keypair, torii_port)| {
                let body = ValidatorReadinessBodyV1 {
                    validator_public_key: keypair.public_key().clone(),
                    torii_port,
                    readiness_stage: "phase_one".to_owned(),
                    artifact_sha256: artifact_sha256.clone(),
                    chain_id: ChainId::from(TAIRA_CHAIN_ID),
                    reviewed_iroha_commit: REVIEWED_IROHA_COMMIT.to_owned(),
                    phase_one_instructions_sha256: phase_one_instructions_sha256.clone(),
                    phase_two_instructions_sha256: None,
                    bridge_abi_version: 21,
                    release_manifest_version: 4,
                    cash_handoff_capability: "cash_handoff_v1".to_owned(),
                    asset_aliases: vec!["ds#boi.is2".to_owned()],
                    release_policy_sha256: release_policy_sha256.clone(),
                    verifier_commitments: verifier_commitments.clone(),
                    roster_root_sha256: roster_root_sha256.clone(),
                    evaluated_block_height: 1,
                    evaluated_block_hash: "12".repeat(32),
                    ready: true,
                };
                let signing_bytes = readiness_signing_bytes(&body).expect("signing bytes");
                SignedValidatorReadinessV1 {
                    signature: Signature::new(keypair.private_key(), &signing_bytes),
                    body,
                }
            })
            .collect();
        let mut evidence = PhaseOneReadinessEvidenceV1 {
            schema_version: SCHEMA_VERSION,
            kind: "taira-phase-one-four-validator-readiness-v1".to_owned(),
            readiness_stage: "phase_one".to_owned(),
            reviewed_iroha_commit: REVIEWED_IROHA_COMMIT.to_owned(),
            bridge_abi_version: 21,
            release_manifest_version: 4,
            cash_handoff_capability: "cash_handoff_v1".to_owned(),
            phase_one_instructions_sha256,
            phase_two_instructions_sha256: None,
            validators,
        };

        validate_phase_one_readiness_evidence_against(&evidence, &trusted_roster, &expectation)
            .expect("valid four-validator evidence");

        let resign = |candidate: &mut PhaseOneReadinessEvidenceV1, index: usize| {
            let signing_bytes = readiness_signing_bytes(&candidate.validators[index].body)
                .expect("mutated signing bytes");
            candidate.validators[index].signature =
                Signature::new(keypairs[index].private_key(), &signing_bytes);
        };

        let mut artifact_substitution = evidence.clone();
        for index in 0..artifact_substitution.validators.len() {
            artifact_substitution.validators[index].body.artifact_sha256 = "34".repeat(32);
            resign(&mut artifact_substitution, index);
        }
        assert!(
            validate_phase_one_readiness_evidence_against(
                &artifact_substitution,
                &trusted_roster,
                &expectation,
            )
            .is_err(),
            "all four valid signatures over one artifact that differs from the independent pin \
             must fail"
        );

        let mut block_substitution = evidence.clone();
        block_substitution.validators[0].body.evaluated_block_hash = "56".repeat(32);
        resign(&mut block_substitution, 0);
        assert!(
            validate_phase_one_readiness_evidence_against(
                &block_substitution,
                &trusted_roster,
                &expectation,
            )
            .is_err(),
            "validly signed disagreement about evaluated block identity must fail"
        );

        let mut height_substitution = evidence.clone();
        for index in 0..height_substitution.validators.len() {
            height_substitution.validators[index]
                .body
                .evaluated_block_height = 2;
            resign(&mut height_substitution, index);
        }
        assert!(
            validate_phase_one_readiness_evidence_against(
                &height_substitution,
                &trusted_roster,
                &expectation,
            )
            .is_err(),
            "four valid signatures over the wrong exact phase-one height must fail"
        );

        let mut verifier_substitution = evidence.clone();
        verifier_substitution.validators[0]
            .body
            .verifier_commitments
            .remove(BASE_VERIFIER_ROLES[0]);
        resign(&mut verifier_substitution, 0);
        assert!(
            validate_phase_one_readiness_evidence_against(
                &verifier_substitution,
                &trusted_roster,
                &expectation,
            )
            .is_err(),
            "validly signed omission of one verifier role must fail"
        );

        let mut roster_substitution = evidence.clone();
        roster_substitution.validators[0].body.roster_root_sha256 = "78".repeat(32);
        resign(&mut roster_substitution, 0);
        assert!(
            validate_phase_one_readiness_evidence_against(
                &roster_substitution,
                &trusted_roster,
                &expectation,
            )
            .is_err(),
            "validly signed substitution of the topology-bound roster root must fail"
        );

        let final_expectation = PhaseOneReadinessExpectationV1 {
            kind: "taira-final-four-validator-readiness-v1",
            readiness_stage: "final",
            chain_id: expectation.chain_id.clone(),
            expected_artifact_sha256: expectation.expected_artifact_sha256.clone(),
            phase_one_instructions_sha256: expectation.phase_one_instructions_sha256.clone(),
            phase_two_instructions_sha256: Some("90".repeat(32)),
            release_policy_sha256: expectation.release_policy_sha256.clone(),
            verifier_commitments: expectation.verifier_commitments.clone(),
            roster_root_sha256: expectation.roster_root_sha256.clone(),
            asset_aliases: vec!["ds#boi.is2".to_owned(), "ds#boi.is".to_owned()],
            evaluated_block_identity: BlockIdentityPinV1 {
                evaluated_block_height: 3,
                evaluated_block_hash: "91".repeat(32),
            },
            expected_source_commit: REVIEWED_IROHA_COMMIT.to_owned(),
        };
        let mut final_evidence = evidence.clone();
        final_evidence.kind = final_expectation.kind.to_owned();
        final_evidence.readiness_stage = final_expectation.readiness_stage.to_owned();
        final_evidence.phase_two_instructions_sha256 =
            final_expectation.phase_two_instructions_sha256.clone();
        for index in 0..final_evidence.validators.len() {
            let body = &mut final_evidence.validators[index].body;
            body.readiness_stage = final_expectation.readiness_stage.to_owned();
            body.phase_two_instructions_sha256 =
                final_expectation.phase_two_instructions_sha256.clone();
            body.asset_aliases = final_expectation.asset_aliases.clone();
            body.evaluated_block_height = 3;
            body.evaluated_block_hash = "91".repeat(32);
            resign(&mut final_evidence, index);
        }
        validate_phase_one_readiness_evidence_against(
            &final_evidence,
            &trusted_roster,
            &final_expectation,
        )
        .expect("valid exact final h3 readiness evidence");

        let mut final_digest_substitution = final_evidence;
        for index in 0..final_digest_substitution.validators.len() {
            final_digest_substitution.validators[index]
                .body
                .phase_two_instructions_sha256 = Some("92".repeat(32));
            resign(&mut final_digest_substitution, index);
        }
        final_digest_substitution.phase_two_instructions_sha256 = Some("92".repeat(32));
        assert!(
            validate_phase_one_readiness_evidence_against(
                &final_digest_substitution,
                &trusted_roster,
                &final_expectation,
            )
            .is_err(),
            "four valid final signatures over a substituted phase-two digest must fail"
        );

        evidence.validators[0].body.ready = false;
        assert!(
            validate_phase_one_readiness_evidence_against(
                &evidence,
                &trusted_roster,
                &expectation,
            )
            .is_err(),
            "a non-ready validator must fail the gate"
        );
    }
}
