//! Authenticated Kagemusha ABI-21/V4 release verification and activation preparation.
mod taira;
use crate::{ExplicitExitError, Outcome, RunArgs};
use clap::{Args as ClapArgs, Subcommand};
use color_eyre::eyre::{WrapErr as _, bail, eyre};
use iroha_core::smartcontracts::isi::offline::{
    KagemushaReleaseCatalogV4, isi::validate_offline_attestation_policy_for_release_activation,
};
use iroha_core::zk::kagemusha_artifact_v4::{
    kagemusha_artifact_descriptor_v4, read_kagemusha_pasta_cycle_artifact_v4,
};
use iroha_core::zk::kagemusha_v2::{
    KagemushaGenerationMemoryGuardV4, KagemushaQualificationMemoryContractV4,
    start_kagemusha_generation_memory_guard_v4, validate_kagemusha_step_bootstrap_payload_v4,
    verify_candidate_recursive_step_two_receipt_v4,
};
use iroha_crypto::HashOf;
use iroha_data_model::isi::{
    InstructionBox,
    offline::{
        ActivateKagemushaRecursiveReleaseV4, CancelKagemushaRecursiveReleaseV4,
        DeactivateKagemushaRecursiveIssuanceV4, EnableKagemushaRecursiveIssuanceV4,
    },
};
use iroha_data_model::offline::{
    KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4,
    KAGEMUSHA_RECURSIVE_SPEND_BENCHMARK_EVIDENCE_FILE_NAME_V1,
    KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_FILE_NAME_V1,
    KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_MAX_BYTES_V4,
    KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_FILE_NAME_V1,
    KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1,
    KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
    KAGEMUSHA_RECURSIVE_SPEND_PROMOTED_RELEASE_SCHEMA_V4,
    KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_FILE_NAME_V4,
    KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_MAX_BYTES_V4,
    KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_FILE_NAME_V4,
    KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V4,
    KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1,
    KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_V2,
    KAGEMUSHA_V4_ISSUANCE_ENABLE_WITNESS_MAX_BYTES_V1,
    KAGEMUSHA_V4_RELEASE_TRANSITION_MAX_BYTES_V1, KagemushaAuthenticatedReleaseV4,
    KagemushaPastaCycleArtifactKindV4, KagemushaPastaCycleParityV1,
    KagemushaRecursiveSpendArtifactManifestV4, KagemushaRecursiveSpendCandidateV4,
    KagemushaRecursiveSpendPromotedReleaseV4, KagemushaRecursiveSpendQualificationReceiptV4,
    KagemushaRecursiveSpendReleaseAttestationV4, KagemushaRecursiveSpendReleasePolicyV1,
    KagemushaStepCircuitParamsV4, KagemushaTopUpFinalityRosterArtifactV2,
    KagemushaV4IssuanceEnableWitnessV1, KagemushaV4PromotionBindingV1,
    KagemushaV4ReleaseCancellationV1, KagemushaV4ReleaseDeactivationV1,
    OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_ANDROID_APPS_V1,
    OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_ANDROID_SIGNING_CERTIFICATES_V1,
    OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_APP_IDENTIFIER_BYTES_V1,
    OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_CANONICAL_BYTES_V1,
    OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_APPS_V1,
    OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_BUNDLE_VERSION_BYTES_V1,
    OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_BUNDLE_VERSIONS_V1,
    OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_VALIDATION_CATEGORIES_V1,
    OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_REVOKED_CERTIFICATES_V1,
    OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TEAM_ID_BYTES_V1,
    OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOT_DER_BYTES_V1,
    OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOTS_PER_PLATFORM_V1,
    OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOTS_V1, OfflineDeviceAttestationPolicy,
    kagemusha_recursive_spend_release_sha256,
};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};
type Result<T> = color_eyre::Result<T>;
const MANIFEST_JSON_FILE_NAME: &str = "manifest.json";
const MANIFEST_NORITO_FILE_NAME: &str = "manifest.norito";
const MANIFEST_NORITO_SHA256_FILE_NAME: &str = "manifest.norito.sha256";
const RELEASE_ATTESTATION_FILE_NAME_V4: &str =
    KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_FILE_NAME_V4;
const PROMOTION_RECORD_FILE_NAME_V4: &str = "promotion-record-v4.norito";
const MAX_MANIFEST_BYTES: usize = 1024 * 1024;
const MAX_POLICY_BYTES: usize = 64 * 1024;
const MAX_ATTESTATION_BYTES: usize = 1024 * 1024;
const RECURSIVE_STEP_VERIFIER_COMMITMENT_DOMAIN_V4: &[u8] =
    b"iroha:kagemusha:recursive-step-verifier-commitment:v4";
const REPORT_ROSTER_PURPOSE: &str = "topup_finality_roster";
const REPORT_ARTIFACT_PURPOSES_V4: [&str; 8] = KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4;
const DURABLE_FILE_PUBLICATION_OUTCOME_SCHEMA_V1: &str =
    "iroha.kagami.kagemusha.durable_file_publication.v1";
const RELEASE_CIRCUIT_PARAMS_PUBLICATION_OUTCOME_SCHEMA_V1: &str =
    "iroha.kagami.kagemusha.release_circuit_params_publication.v1";
const DURABLE_FILE_COMMIT_UNCERTAIN_EXIT_CODE: u8 = 75;
const RELEASE_STEP_EQ_CIRCUIT_PARAMS_FILE_NAME_V4: &str = "step-eq-circuit-params.norito";
const RELEASE_STEP_EP_CIRCUIT_PARAMS_FILE_NAME_V4: &str = "step-ep-circuit-params.norito";
const AUTHENTICATED_ARTIFACT_ROLES_V4: [(
    KagemushaPastaCycleParityV1,
    KagemushaPastaCycleArtifactKindV4,
); 8] = [
    (
        KagemushaPastaCycleParityV1::StepEq,
        KagemushaPastaCycleArtifactKindV4::ParamsIpa,
    ),
    (
        KagemushaPastaCycleParityV1::StepEq,
        KagemushaPastaCycleArtifactKindV4::ProvingKey,
    ),
    (
        KagemushaPastaCycleParityV1::StepEq,
        KagemushaPastaCycleArtifactKindV4::VerifyingKey,
    ),
    (
        KagemushaPastaCycleParityV1::StepEq,
        KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
    ),
    (
        KagemushaPastaCycleParityV1::StepEp,
        KagemushaPastaCycleArtifactKindV4::ParamsIpa,
    ),
    (
        KagemushaPastaCycleParityV1::StepEp,
        KagemushaPastaCycleArtifactKindV4::ProvingKey,
    ),
    (
        KagemushaPastaCycleParityV1::StepEp,
        KagemushaPastaCycleArtifactKindV4::VerifyingKey,
    ),
    (
        KagemushaPastaCycleParityV1::StepEp,
        KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
    ),
];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReleaseInventoryStateV4 {
    Candidate,
    Promoted,
}
impl ReleaseInventoryStateV4 {
    const fn includes_promotion_record(self) -> bool {
        matches!(self, Self::Promoted)
    }
    const fn exact_file_count(self) -> usize {
        match self {
            Self::Candidate => 17,
            Self::Promoted => 18,
        }
    }
    const fn label(self) -> &'static str {
        match self {
            Self::Candidate => "pre-promotion candidate",
            Self::Promoted => "promoted release",
        }
    }
}
fn verify_publish_verify_release_v4<T, V, P>(mut verify: V, publish: P) -> Result<T>
where
    V: FnMut(ReleaseInventoryStateV4) -> Result<T>,
    P: FnOnce(&T) -> Outcome,
{
    let candidate = verify(ReleaseInventoryStateV4::Candidate)?;
    publish(&candidate)?;
    verify(ReleaseInventoryStateV4::Promoted)
}
fn validate_artifacts_sequentially<I, T, E, F>(
    artifacts: I,
    mut validate: F,
) -> std::result::Result<(), E>
where
    I: IntoIterator,
    F: FnMut(I::Item) -> std::result::Result<T, E>,
{
    for artifact in artifacts {
        drop(validate(artifact)?);
    }
    Ok(())
}
/// Kagemusha release-management command group.
#[derive(Debug, ClapArgs)]
pub struct Args {
    #[command(subcommand)]
    command: Command,
}
#[derive(Debug, Subcommand)]
enum Command {
    /// Verify one complete authenticated ABI-21/V4 release directory.
    #[command(name = "verify-release-v4")]
    VerifyReleaseV4(VerifyReleaseV4Args),
    /// Verify an ABI-21/V4 release and atomically write its typed promotion record.
    #[command(name = "promote-release-v4")]
    PromoteReleaseV4(PromoteReleaseV4Args),
    /// Build one release-bound activation instruction from an authenticated V4 catalog.
    #[command(name = "prepare-activation-v4")]
    PrepareActivationV4(PrepareActivationV4Args),
    /// Build one staged-to-enabled instruction from an exact canonical witness.
    #[command(name = "prepare-enable-issuance-v4")]
    PrepareEnableIssuanceV4(PrepareEnableIssuanceV4Args),
    /// Build one permanent staged-release cancellation instruction.
    #[command(name = "prepare-cancel-release-v4")]
    PrepareCancelReleaseV4(PrepareCancelReleaseV4Args),
    /// Build one permanent enabled-issuance deactivation instruction.
    #[command(name = "prepare-deactivate-issuance-v4")]
    PrepareDeactivateIssuanceV4(PrepareDeactivateIssuanceV4Args),
    /// Atomically publish the canonical reviewed Eq/Ep first-release circuit parameters.
    #[command(name = "prepare-release-circuit-params-v4")]
    PrepareReleaseCircuitParamsV4(PrepareReleaseCircuitParamsV4Args),
    /// Build the actual rendered Taira validator roster for signed V4 release generation.
    #[command(name = "prepare-taira-release-roster-v4")]
    PrepareTairaReleaseRosterV4(taira::PrepareReleaseRosterV4Args),
    /// Append network-independent offline-cash prerequisites to a fresh Taira genesis.
    #[command(name = "prepare-taira-testnet-base-genesis-v4")]
    PrepareTairaTestnetBaseGenesisV4(taira::PrepareTestnetBaseGenesisV4Args),
}
#[derive(Debug, ClapArgs)]
struct VerifyReleaseV4Args {
    /// Immutable directory containing the exact eighteen-file promoted ABI-21/V4 inventory.
    #[arg(long)]
    bundle_dir: PathBuf,
    /// Canonical release policy provisioned alongside the candidate release.
    #[arg(long)]
    release_policy: PathBuf,
    /// Signed physical-device benchmark evidence file.
    #[arg(long)]
    benchmark_evidence: PathBuf,
    /// Canonical signed, candidate-bound cryptographic review Norito file.
    #[arg(long)]
    cryptographic_review: PathBuf,
    /// Optional nonzero byte ceiling that may only lower the built-in physical-memory limit.
    #[arg(long, value_parser = parse_nonzero_canonical_u64)]
    memory_limit_bytes: Option<u64>,
}
#[derive(Debug, ClapArgs)]
struct PromoteReleaseV4Args {
    /// Directory containing the exact seventeen-file pre-promotion ABI-21/V4 candidate.
    #[arg(long)]
    bundle_dir: PathBuf,
    /// Canonical release policy provisioned alongside the candidate release.
    #[arg(long)]
    release_policy: PathBuf,
    /// Exact absent `<bundle-dir>/promotion-record-v4.norito` leaf; it is never overwritten.
    #[arg(long)]
    promotion_record: PathBuf,
    /// Signed physical-device benchmark evidence file.
    #[arg(long)]
    benchmark_evidence: PathBuf,
    /// Canonical signed, candidate-bound cryptographic review Norito file.
    #[arg(long)]
    cryptographic_review: PathBuf,
    /// Optional nonzero byte ceiling that may only lower the built-in physical-memory limit.
    #[arg(long, value_parser = parse_nonzero_canonical_u64)]
    memory_limit_bytes: Option<u64>,
}
#[derive(Debug, ClapArgs)]
struct PrepareActivationV4Args {
    /// Unique nonzero promotion-run identity reserved before validator qualification.
    #[arg(long, value_parser = parse_promotion_id)]
    promotion_id: [u8; 32],
    /// Exact canonical controller-signed promotion binding committed by activation.
    #[arg(long)]
    promotion_binding: PathBuf,
    /// Root containing lowercase manifest-digest release directories.
    #[arg(long)]
    artifact_root: PathBuf,
    /// Canonical release policy configured on every validator.
    #[arg(long)]
    release_policy: PathBuf,
    /// Exact lowercase SHA-256 directory name of the release to activate.
    #[arg(long, value_parser = parse_manifest_sha256)]
    manifest_sha256: [u8; 32],
    /// Domain-separated SHA-256 shared by all four validator runtime projections.
    #[arg(long, value_parser = parse_manifest_sha256)]
    runtime_effective_config_sha256: [u8; 32],
    /// Next atomic Eq/Ep verifier version observed from live consensus state.
    #[arg(long)]
    verifier_version: u32,
    /// Exact governed verifier policy derived from authenticated physical-device evidence.
    /// The policy and release are embedded in one composite consensus instruction.
    #[arg(long)]
    device_attestation_policy: PathBuf,
    /// Explicit Unix timestamp used for the same certificate-validity checks as consensus.
    /// The activation is checked again against its actual block timestamp on every validator.
    #[arg(long, value_parser = parse_nonzero_canonical_u64)]
    policy_evaluation_time_ms: u64,
    /// New private file receiving exact instruction JSON for direct-lifecycle payload preparation.
    #[arg(long)]
    output: PathBuf,
}
#[derive(Debug, ClapArgs)]
struct PrepareEnableIssuanceV4Args {
    /// Exact canonical bounded staged-to-enabled witness.
    #[arg(long)]
    enable_witness: PathBuf,
    /// New private file receiving exact instruction JSON for direct-lifecycle payload preparation.
    #[arg(long)]
    output: PathBuf,
}
#[derive(Debug, ClapArgs)]
struct PrepareCancelReleaseV4Args {
    /// Exact canonical predecessor-bound staged-release cancellation.
    #[arg(long)]
    cancellation: PathBuf,
    /// New private file receiving exact instruction JSON for direct-lifecycle payload preparation.
    #[arg(long)]
    output: PathBuf,
}
#[derive(Debug, ClapArgs)]
struct PrepareDeactivateIssuanceV4Args {
    /// Exact canonical predecessor-bound enabled-issuance deactivation.
    #[arg(long)]
    deactivation: PathBuf,
    /// New private file receiving exact instruction JSON for direct-lifecycle payload preparation.
    #[arg(long)]
    output: PathBuf,
}
#[derive(Debug, ClapArgs)]
struct PrepareReleaseCircuitParamsV4Args {
    /// New owner-private directory atomically receiving the canonical Eq/Ep Norito files.
    #[arg(long)]
    output_dir: PathBuf,
}
impl<T: Write> RunArgs<T> for Args {
    #[expect(
        clippy::too_many_lines,
        reason = "the command dispatcher keeps each authenticated release operation and its publication boundary visibly ordered"
    )]
    fn run(self, writer: &mut std::io::BufWriter<T>) -> Outcome {
        match self.command {
            Command::VerifyReleaseV4(args) => {
                let memory_guard =
                    start_kagemusha_generation_memory_guard_v4(args.memory_limit_bytes)
                        .map_err(|error| eyre!("Kagemusha memory guard failed: {error}"))?;
                let policy_bytes = configured_policy_bytes(&args.release_policy)?;
                let verified = verify_release_directory_v4(
                    &args.bundle_dir,
                    &policy_bytes,
                    &args.benchmark_evidence,
                    &args.cryptographic_review,
                    ReleaseInventoryStateV4::Promoted,
                    memory_guard,
                )?;
                let report = verified.verification_report()?;
                writeln!(writer, "{}", report.canonical_json()?)?;
            }
            Command::PromoteReleaseV4(args) => {
                let memory_guard =
                    start_kagemusha_generation_memory_guard_v4(args.memory_limit_bytes)
                        .map_err(|error| eyre!("Kagemusha memory guard failed: {error}"))?;
                let policy_bytes = configured_policy_bytes(&args.release_policy)?;
                let canonical_root = canonical_release_root(&args.bundle_dir)?;
                require_new_promotion_record_path_v4(&canonical_root, &args.promotion_record)?;
                let verified = verify_publish_verify_release_v4(
                    |inventory_state| {
                        verify_release_directory_v4(
                            &canonical_root,
                            &policy_bytes,
                            &args.benchmark_evidence,
                            &args.cryptographic_review,
                            inventory_state,
                            memory_guard,
                        )
                    },
                    |candidate| {
                        let record = candidate.promotion_record()?;
                        record.validate().map_err(|error| eyre!(error))?;
                        let record_bytes = norito::to_bytes(&record)
                            .wrap_err("failed to encode Kagemusha V4 promotion record")?;
                        publish_new_durable_file(
                            &mut std::io::sink(),
                            &args.promotion_record,
                            &record_bytes,
                        )
                    },
                )?;
                writeln!(
                    writer,
                    "{}",
                    verified.verification_report()?.canonical_json()?
                )?;
            }
            Command::PrepareActivationV4(args) => {
                let policy_bytes = configured_policy_bytes(&args.release_policy)?;
                // Authenticate the explicit policy before the catalog opens any release.
                if policy_bytes.is_empty() {
                    bail!("Kagemusha V4 activation policy is empty");
                }
                let catalog =
                    KagemushaReleaseCatalogV4::load(&args.release_policy, &args.artifact_root)
                        .map_err(|error| eyre!(error))?;
                let activation = catalog
                    .build_activation(args.manifest_sha256, args.verifier_version)
                    .map_err(|error| eyre!(error))?;
                let policy = configured_device_attestation_policy(
                    &args.device_attestation_policy,
                    args.policy_evaluation_time_ms,
                )?;
                let state_bytes = norito::to_bytes(&policy)
                    .wrap_err("failed to encode governed device-attestation policy state")?;
                let policy_state_sha256 =
                    hex::encode(kagemusha_recursive_spend_release_sha256(&state_bytes));
                let promotion_binding_bytes = read_external_bounded(
                    &args.promotion_binding,
                    64 * 1024,
                    "canonical Kagemusha V4 promotion binding",
                )?;
                let promotion_binding: KagemushaV4PromotionBindingV1 =
                    norito::decode_canonical_with_limits(
                        &promotion_binding_bytes,
                        norito::canonical_decode_limits(promotion_binding_bytes.len()),
                    )
                    .wrap_err("failed to decode the canonical Kagemusha V4 promotion binding")?;
                promotion_binding.validate().map_err(|error| eyre!(error))?;
                if norito::encode_canonical(&promotion_binding)? != promotion_binding_bytes
                    || promotion_binding.promotion_id != args.promotion_id
                {
                    bail!("promotion binding is non-canonical or differs from --promotion-id");
                }
                let instruction = ActivateKagemushaRecursiveReleaseV4::new(
                    promotion_binding,
                    activation,
                    policy,
                    args.runtime_effective_config_sha256,
                );
                let instructions = vec![InstructionBox::from(instruction)];
                let instructions_hash = HashOf::new(&instructions);
                let mut instruction_json = norito::json::to_string(&instructions)
                    .wrap_err("failed to encode Kagemusha V4 activation instruction JSON")?;
                instruction_json.push('\n');
                // Activation publication has the same atomicity boundary as a
                // promotion record. A successful rename followed by failed
                // directory sync must never be reported as an ordinary success
                // (or as a safely retryable pre-commit error).
                publish_new_durable_file(writer, &args.output, instruction_json.as_bytes())?;
                writeln!(
                    writer,
                    "{{\"status\":\"prepared\",\"promotion_id\":\"{}\",\"manifest_sha256\":\"{}\",\"runtime_effective_config_sha256\":\"{}\",\"verifier_version\":{},\"instruction_count\":1,\"instructions_hash\":\"{}\",\"device_attestation_policy_state_sha256\":\"{}\",\"policy_evaluation_time_ms\":{}}}",
                    hex::encode(args.promotion_id),
                    hex::encode(args.manifest_sha256),
                    hex::encode(args.runtime_effective_config_sha256),
                    args.verifier_version,
                    instructions_hash,
                    policy_state_sha256,
                    args.policy_evaluation_time_ms,
                )?;
            }
            Command::PrepareEnableIssuanceV4(args) => {
                let bytes = read_external_bounded(
                    &args.enable_witness,
                    KAGEMUSHA_V4_ISSUANCE_ENABLE_WITNESS_MAX_BYTES_V1,
                    "Kagemusha V4 issuance-enable witness",
                )?;
                let witness = KagemushaV4IssuanceEnableWitnessV1::decode_canonical(&bytes)
                    .map_err(|error| eyre!(error))?;
                prepare_lifecycle_instruction_v4(
                    writer,
                    &args.output,
                    "enable-issuance-v4",
                    &bytes,
                    InstructionBox::from(EnableKagemushaRecursiveIssuanceV4::new(witness)),
                )?;
            }
            Command::PrepareCancelReleaseV4(args) => {
                let bytes = read_external_bounded(
                    &args.cancellation,
                    KAGEMUSHA_V4_RELEASE_TRANSITION_MAX_BYTES_V1,
                    "Kagemusha V4 staged-release cancellation",
                )?;
                let cancellation = KagemushaV4ReleaseCancellationV1::decode_canonical(&bytes)
                    .map_err(|error| eyre!(error))?;
                prepare_lifecycle_instruction_v4(
                    writer,
                    &args.output,
                    "cancel-release-v4",
                    &bytes,
                    InstructionBox::from(CancelKagemushaRecursiveReleaseV4::new(cancellation)),
                )?;
            }
            Command::PrepareDeactivateIssuanceV4(args) => {
                let bytes = read_external_bounded(
                    &args.deactivation,
                    KAGEMUSHA_V4_RELEASE_TRANSITION_MAX_BYTES_V1,
                    "Kagemusha V4 issuance deactivation",
                )?;
                let deactivation = KagemushaV4ReleaseDeactivationV1::decode_canonical(&bytes)
                    .map_err(|error| eyre!(error))?;
                prepare_lifecycle_instruction_v4(
                    writer,
                    &args.output,
                    "deactivate-issuance-v4",
                    &bytes,
                    InstructionBox::from(DeactivateKagemushaRecursiveIssuanceV4::new(deactivation)),
                )?;
            }
            Command::PrepareReleaseCircuitParamsV4(args) => {
                prepare_release_circuit_params_v4(&args, writer)?;
            }
            Command::PrepareTairaReleaseRosterV4(args) => {
                taira::prepare_release_roster_v4(&args, writer)?;
            }
            Command::PrepareTairaTestnetBaseGenesisV4(args) => {
                taira::prepare_testnet_base_genesis_v4(&args, writer)?;
            }
        }
        Ok(())
    }
}
fn prepare_lifecycle_instruction_v4<T: Write>(
    writer: &mut std::io::BufWriter<T>,
    output: &Path,
    operation: &'static str,
    canonical_input: &[u8],
    instruction: InstructionBox,
) -> Outcome {
    let instructions = vec![instruction];
    let instructions_hash = HashOf::new(&instructions);
    let mut instruction_json = norito::json::to_string(&instructions)
        .wrap_err("failed to encode Kagemusha V4 lifecycle instruction JSON")?;
    instruction_json.push('\n');
    publish_new_durable_file(writer, output, instruction_json.as_bytes())?;
    writeln!(
        writer,
        "{{\"status\":\"prepared\",\"operation\":\"{operation}\",\"instruction_count\":1,\"instructions_hash\":\"{instructions_hash}\",\"input_sha256\":\"{}\"}}",
        hex::encode(kagemusha_recursive_spend_release_sha256(canonical_input)),
    )?;
    Ok(())
}
#[derive(Clone, Debug, crate::json_macros::JsonSerialize)]
struct ReleaseCircuitParamsArtifactReportV4 {
    file_name: String,
    size_bytes: u64,
    file_sha256: String,
    circuit_params_sha256: String,
}
#[derive(Debug, crate::json_macros::JsonSerialize)]
struct ReleaseCircuitParamsReportV4 {
    status: String,
    profile: String,
    output_dir: String,
    step_eq: ReleaseCircuitParamsArtifactReportV4,
    step_ep: ReleaseCircuitParamsArtifactReportV4,
}
fn prepare_release_circuit_params_v4<T: Write>(
    args: &PrepareReleaseCircuitParamsV4Args,
    writer: &mut std::io::BufWriter<T>,
) -> Outcome {
    let output_dir = args
        .output_dir
        .to_str()
        .ok_or_else(|| eyre!("release circuit-parameter output directory is not UTF-8"))?
        .to_owned();
    let params = KagemushaStepCircuitParamsV4::reviewed_first_release_generation_profile()
        .map_err(|error| eyre!("reviewed first-release circuit parameters are invalid: {error}"))?;
    let bytes = norito::to_bytes(&params)
        .wrap_err("failed to encode reviewed first-release circuit parameters")?;
    let decoded: KagemushaStepCircuitParamsV4 = norito::decode_from_bytes(&bytes)
        .wrap_err("failed to decode canonical first-release circuit parameters")?;
    if decoded != params
        || norito::to_bytes(&decoded)
            .wrap_err("failed to re-encode canonical first-release circuit parameters")?
            != bytes
    {
        bail!("reviewed first-release circuit parameters are not canonical Norito");
    }
    let file_sha256 = kagemusha_recursive_spend_release_sha256(&bytes);
    let circuit_params_sha256 = params
        .sha256()
        .map_err(|error| eyre!("failed to identify first-release circuit parameters: {error}"))?;
    let size_bytes = u64::try_from(bytes.len())
        .map_err(|_| eyre!("first-release circuit-parameter length does not fit u64"))?;
    publish_release_circuit_params_directory_v4(writer, &args.output_dir, &bytes)?;
    let artifact = |file_name: &str| ReleaseCircuitParamsArtifactReportV4 {
        file_name: file_name.to_owned(),
        size_bytes,
        file_sha256: hex::encode(file_sha256),
        circuit_params_sha256: hex::encode(circuit_params_sha256),
    };
    let report = ReleaseCircuitParamsReportV4 {
        status: "prepared".to_owned(),
        profile: "reviewed_first_release_generation_profile".to_owned(),
        output_dir,
        step_eq: artifact(RELEASE_STEP_EQ_CIRCUIT_PARAMS_FILE_NAME_V4),
        step_ep: artifact(RELEASE_STEP_EP_CIRCUIT_PARAMS_FILE_NAME_V4),
    };
    writeln!(
        writer,
        "{}",
        norito::json::to_json(&report)
            .wrap_err("failed to encode release circuit-parameter report")?
    )?;
    Ok(())
}
fn parse_manifest_sha256(value: &str) -> std::result::Result<[u8; 32], String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err("manifest SHA-256 must be exactly 64 lowercase hexadecimal characters".into());
    }
    hex::decode(value)
        .map_err(|_| "manifest SHA-256 is malformed".to_owned())?
        .try_into()
        .map_err(|_| "manifest SHA-256 has the wrong length".to_owned())
}
fn parse_promotion_id(value: &str) -> std::result::Result<[u8; 32], String> {
    let digest = parse_manifest_sha256(value)?;
    if digest == [0; 32] {
        return Err("promotion id must be nonzero".to_owned());
    }
    Ok(digest)
}
fn parse_nonzero_canonical_u64(value: &str) -> std::result::Result<u64, String> {
    if value.is_empty()
        || value == "0"
        || value
            .as_bytes()
            .first()
            .is_none_or(|first| !first.is_ascii_digit())
        || value.starts_with('0')
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err("value must be a canonical nonzero unsigned decimal".to_owned());
    }
    value
        .parse::<u64>()
        .map_err(|error| format!("value must fit u64: {error}"))
}
fn configured_policy_bytes(path: &Path) -> Result<Vec<u8>> {
    let configured = read_external_bounded(
        path,
        MAX_POLICY_BYTES,
        "configured Kagemusha V4 release policy",
    )?;
    let policy: KagemushaRecursiveSpendReleasePolicyV1 =
        decode_canonical_norito(&configured, "configured Kagemusha V4 release policy")?;
    policy.validate().map_err(|error| eyre!(error))?;
    Ok(configured)
}
fn configured_device_attestation_policy(
    path: &Path,
    policy_evaluation_time_ms: u64,
) -> Result<OfflineDeviceAttestationPolicy> {
    let raw = read_external_bounded(
        path,
        OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_CANONICAL_BYTES_V1,
        "governed Offline device-attestation policy",
    )?;
    let policy: OfflineDeviceAttestationPolicy = norito::json::from_slice(&raw)
        .wrap_err("failed to decode governed Offline device-attestation policy JSON")?;
    validate_device_attestation_policy_for_atomic_activation(&policy, policy_evaluation_time_ms)?;
    let canonical = norito::json::to_vec(&policy)
        .wrap_err("failed to encode canonical Offline device-attestation policy JSON")?;
    if raw != canonical {
        bail!("Offline device-attestation policy JSON is not the exact canonical encoding");
    }
    Ok(policy)
}
fn validate_device_attestation_policy_for_atomic_activation(
    policy: &OfflineDeviceAttestationPolicy,
    policy_evaluation_time_ms: u64,
) -> Result<()> {
    if policy.version != 1
        || !policy.require_ios_app_policy
        || !policy.require_android_app_policy
        || policy.trusted_roots.is_empty()
        || policy.ios_apps.is_empty()
        || policy.android_apps.is_empty()
    {
        bail!(
            "atomic Kagemusha activation requires version-1 fail-closed iOS and Android app policy"
        );
    }
    validate_atomic_activation_policy_shape(policy)?;
    validate_atomic_activation_trusted_roots(policy)?;
    validate_atomic_activation_revocations(policy)?;
    validate_atomic_activation_ios_apps(policy)?;
    validate_atomic_activation_android_apps(policy)?;
    validate_atomic_activation_android_status(policy, policy_evaluation_time_ms)?;
    validate_offline_attestation_policy_for_release_activation(policy, policy_evaluation_time_ms)
        .map_err(|error| eyre!("consensus device-attestation policy validation failed: {error}"))?;
    Ok(())
}
fn validate_atomic_activation_policy_shape(policy: &OfflineDeviceAttestationPolicy) -> Result<()> {
    if policy.trusted_roots.len() > OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOTS_V1
        || policy.revoked_certificate_tbs_sha256.len()
            > OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_REVOKED_CERTIFICATES_V1
        || policy.ios_apps.len() > OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_APPS_V1
        || policy.android_apps.len() > OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_ANDROID_APPS_V1
        || policy.android_status_snapshot.as_ref().is_some_and(|snapshot| {
            snapshot.non_valid_serials.len()
                > iroha_data_model::offline::OFFLINE_ANDROID_ATTESTATION_STATUS_MAX_NON_VALID_SERIALS_V1
                || snapshot.non_valid_serials.iter().any(|serial| {
                    serial.len()
                        > iroha_data_model::offline::OFFLINE_ANDROID_ATTESTATION_STATUS_MAX_SERIAL_HEX_BYTES_V1
                })
        })
    {
        bail!("atomic Kagemusha activation device-attestation policy exceeds collection limits");
    }
    let ios_root_count = policy
        .trusted_roots
        .iter()
        .filter(|root| root.platform == "ios-appattest")
        .count();
    let android_root_count = policy
        .trusted_roots
        .iter()
        .filter(|root| root.platform == "android-keymint")
        .count();
    if ios_root_count > OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOTS_PER_PLATFORM_V1
        || android_root_count > OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOTS_PER_PLATFORM_V1
        || policy.trusted_roots.iter().any(|root| {
            root.der.len() > OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOT_DER_BYTES_V1
        })
    {
        bail!("atomic Kagemusha activation device-attestation roots exceed size limits");
    }
    if policy.ios_apps.iter().any(|app| {
        app.team_id.len() > OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TEAM_ID_BYTES_V1
            || app.bundle_id.len() > OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_APP_IDENTIFIER_BYTES_V1
            || app.allowed_validation_categories.len()
                > OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_VALIDATION_CATEGORIES_V1
            || app.allowed_bundle_versions.len()
                > OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_BUNDLE_VERSIONS_V1
            || app.allowed_bundle_versions.iter().any(|version| {
                version.len() > OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_BUNDLE_VERSION_BYTES_V1
            })
    }) {
        bail!("atomic Kagemusha activation iOS app policy exceeds size limits");
    }
    if policy.android_apps.iter().any(|app| {
        app.package_name.len() > OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_APP_IDENTIFIER_BYTES_V1
            || app.signing_certificate_sha256.len()
                > OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_ANDROID_SIGNING_CERTIFICATES_V1
    }) {
        bail!("atomic Kagemusha activation Android app policy exceeds size limits");
    }
    let canonical = norito::encode_canonical(policy)
        .wrap_err("failed to encode bounded Offline device-attestation policy")?;
    if canonical.len() > OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_CANONICAL_BYTES_V1 {
        bail!("atomic Kagemusha activation device-attestation policy exceeds canonical size limit");
    }
    Ok(())
}
fn validate_atomic_activation_trusted_roots(policy: &OfflineDeviceAttestationPolicy) -> Result<()> {
    let mut root_ids = BTreeSet::new();
    let mut platforms = BTreeSet::new();
    for root in &policy.trusted_roots {
        if !matches!(root.platform.as_str(), "ios-appattest" | "android-keymint")
            || root.der.is_empty()
            || root.der.len() > OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOT_DER_BYTES_V1
            || root.der.first() != Some(&0x30)
            || root
                .not_before_ms
                .zip(root.not_after_ms)
                .is_some_and(|(start, end)| start >= end)
            || !root_ids.insert((
                root.platform.clone(),
                kagemusha_recursive_spend_release_sha256(&root.der),
            ))
        {
            bail!("atomic Kagemusha activation contains an invalid or duplicate trusted root");
        }
        platforms.insert(root.platform.as_str());
    }
    if platforms != BTreeSet::from(["android-keymint", "ios-appattest"]) {
        bail!("atomic Kagemusha activation requires both platform trust roots");
    }
    Ok(())
}
fn validate_atomic_activation_revocations(policy: &OfflineDeviceAttestationPolicy) -> Result<()> {
    let mut revoked = BTreeSet::new();
    for digest in &policy.revoked_certificate_tbs_sha256 {
        if digest.len() != 32 || !revoked.insert(digest.as_slice()) {
            bail!(
                "atomic Kagemusha activation contains an invalid duplicate certificate TBS revocation"
            );
        }
    }
    Ok(())
}
fn is_canonical_policy_ascii(value: &str) -> bool {
    !value.is_empty()
        && value.is_ascii()
        && !value.chars().any(char::is_control)
        && value.trim() == value
}
fn validate_atomic_activation_ios_apps(policy: &OfflineDeviceAttestationPolicy) -> Result<()> {
    let mut ios_ids = BTreeSet::new();
    for app in &policy.ios_apps {
        if !is_canonical_policy_ascii(&app.team_id)
            || !is_canonical_policy_ascii(&app.bundle_id)
            || app.environment != "production"
            || app.allowed_validation_categories.is_empty()
            || app.allowed_bundle_versions.is_empty()
            || app
                .allowed_bundle_versions
                .iter()
                .any(|value| !is_canonical_policy_ascii(value))
            || app.allowed_validation_categories
                != app
                    .allowed_validation_categories
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>()
            || app.allowed_bundle_versions
                != app
                    .allowed_bundle_versions
                    .iter()
                    .cloned()
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>()
            || !ios_ids.insert((
                app.team_id.clone(),
                app.bundle_id.clone(),
                app.environment.clone(),
            ))
        {
            bail!("atomic Kagemusha activation contains an invalid iOS app policy");
        }
    }
    Ok(())
}
fn validate_atomic_activation_android_apps(policy: &OfflineDeviceAttestationPolicy) -> Result<()> {
    let mut android_ids = BTreeSet::new();
    for app in &policy.android_apps {
        if !is_canonical_policy_ascii(&app.package_name)
            || app.signing_certificate_sha256.is_empty()
            || app
                .signing_certificate_sha256
                .iter()
                .any(|digest| digest.len() != 32)
            || app.signing_certificate_sha256
                != app
                    .signing_certificate_sha256
                    .iter()
                    .cloned()
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>()
            || !android_ids.insert(app.package_name.clone())
        {
            bail!("atomic Kagemusha activation contains an invalid Android app policy");
        }
    }
    Ok(())
}
fn validate_atomic_activation_android_status(
    policy: &OfflineDeviceAttestationPolicy,
    evaluation_time_ms: u64,
) -> Result<()> {
    let snapshot = policy
        .android_status_snapshot
        .as_ref()
        .ok_or_else(|| eyre!("atomic Kagemusha activation requires an Android status snapshot"))?;
    if snapshot.version
        != iroha_data_model::offline::OFFLINE_ANDROID_ATTESTATION_STATUS_SNAPSHOT_VERSION_V1
        || snapshot.payload_sha256 == [0; 32]
        || snapshot.response_date_ms == 0
        || !snapshot.response_date_ms.is_multiple_of(1_000)
        || snapshot.cache_max_age_seconds == 0
        || snapshot.cache_max_age_seconds
            > iroha_data_model::offline::OFFLINE_ANDROID_ATTESTATION_STATUS_MAX_CACHE_AGE_SECONDS_V1
        || snapshot.last_modified_ms.is_some_and(|last_modified_ms| {
            last_modified_ms == 0
                || !last_modified_ms.is_multiple_of(1_000)
                || last_modified_ms > snapshot.response_date_ms
        })
    {
        bail!("atomic Kagemusha activation Android status metadata is invalid");
    }
    let mut previous_serial: Option<&str> = None;
    for serial in &snapshot.non_valid_serials {
        if serial.is_empty()
            || !serial
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
            || (serial.len() > 1 && serial.starts_with('0'))
            || previous_serial.is_some_and(|previous| previous >= serial.as_str())
        {
            bail!("atomic Kagemusha activation Android status serials are not canonical");
        }
        previous_serial = Some(serial);
    }
    let fresh_until_ms = snapshot
        .response_date_ms
        .checked_add(u64::from(snapshot.cache_max_age_seconds) * 1_000)
        .ok_or_else(|| eyre!("atomic Kagemusha activation Android status deadline overflows"))?;
    if evaluation_time_ms < snapshot.response_date_ms || evaluation_time_ms >= fresh_until_ms {
        bail!("atomic Kagemusha activation Android status snapshot is not fresh");
    }
    Ok(())
}
struct VerifiedReleaseV4 {
    authenticated: KagemushaAuthenticatedReleaseV4,
    report: VerificationReport,
}
impl VerifiedReleaseV4 {
    fn immutable_candidate(&self) -> Result<KagemushaRecursiveSpendCandidateV4> {
        self.authenticated
            .manifest()
            .immutable_candidate()
            .map_err(|error| eyre!("failed to reconstruct immutable V4 candidate: {error}"))
    }
    fn promotion_record(&self) -> Result<KagemushaRecursiveSpendPromotedReleaseV4> {
        let candidate = self.immutable_candidate()?;
        let record = KagemushaRecursiveSpendPromotedReleaseV4 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_PROMOTED_RELEASE_SCHEMA_V4.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V4,
            generation: self.authenticated.manifest().generation.clone(),
            authenticated_source_seal_projection_sha256: self
                .authenticated
                .manifest()
                .authenticated_source_seal_projection_sha256,
            reviewed_cargo_binary_sha256: self
                .authenticated
                .manifest()
                .reviewed_cargo_binary_sha256,
            reviewed_rustc_binary_sha256: self
                .authenticated
                .manifest()
                .reviewed_rustc_binary_sha256,
            generator_binary_sha256: self.authenticated.manifest().generator_binary_sha256,
            sealed_candidate_build_report_sha256: self
                .authenticated
                .manifest()
                .sealed_candidate_build_report_sha256,
            candidate_sha256: candidate
                .sha256()
                .map_err(|error| eyre!("failed to identify immutable V4 candidate: {error}"))?,
            qualification_receipt_sha256: self
                .authenticated
                .manifest()
                .qualification_receipt_sha256,
            qualified_candidate_sha256: self.authenticated.manifest().qualified_candidate_sha256,
            internal_validation_receipt_sha256: self
                .authenticated
                .manifest()
                .internal_validation_receipt_sha256,
            manifest_sha256: self.authenticated.manifest_sha256(),
            release_attestation_sha256: self.authenticated.release_attestation_sha256(),
            release_policy_sha256: self.authenticated.release_policy_sha256(),
            approved_signers: self.authenticated.approved_signers().to_vec(),
            artifact_inventory_verified: true,
            bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
            artifact_roles: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4
                .map(str::to_owned)
                .to_vec(),
            max_proof_bytes: self.authenticated.manifest().max_proof_bytes,
        };
        record
            .validate_against_candidate_and_authenticated_release(&candidate, &self.authenticated)
            .map_err(|error| eyre!("invalid candidate-bound V4 promotion record: {error}"))?;
        Ok(record)
    }
    fn verification_report(&self) -> Result<VerificationReportV4> {
        let candidate_sha256 = self
            .immutable_candidate()?
            .sha256()
            .map_err(|error| eyre!("failed to identify immutable V4 candidate: {error}"))?;
        let promotion_bytes = norito::to_bytes(&self.promotion_record()?)
            .wrap_err("failed to encode canonical Kagemusha V4 promotion record")?;
        Ok(VerificationReportV4::from_report(
            &self.report,
            candidate_sha256,
            kagemusha_recursive_spend_release_sha256(&promotion_bytes),
        ))
    }
}
#[expect(
    clippy::too_many_lines,
    reason = "the ordered fail-closed release verification keeps authenticated inputs and first-error checks in one auditable pass"
)]
fn verify_release_directory_v4(
    bundle_dir: &Path,
    policy_bytes: &[u8],
    benchmark_evidence_path: &Path,
    cryptographic_review_path: &Path,
    inventory_state: ReleaseInventoryStateV4,
    memory_guard: KagemushaGenerationMemoryGuardV4,
) -> Result<VerifiedReleaseV4> {
    let root = canonical_release_root(bundle_dir)?;
    let policy: KagemushaRecursiveSpendReleasePolicyV1 =
        decode_canonical_norito(policy_bytes, "configured Kagemusha release policy")?;
    policy.validate().map_err(|error| eyre!(error))?;
    let release_policy_sha256 = kagemusha_recursive_spend_release_sha256(policy_bytes);
    let manifest_bytes = read_regular_bounded(
        &root,
        MANIFEST_NORITO_FILE_NAME,
        MAX_MANIFEST_BYTES,
        "canonical Kagemusha V4 manifest",
    )?;
    let manifest: KagemushaRecursiveSpendArtifactManifestV4 =
        decode_canonical_norito(&manifest_bytes, "canonical Kagemusha V4 manifest")?;
    manifest.validate().map_err(|error| eyre!(error))?;
    if manifest.generation_memory_limit_bytes != memory_guard.effective_memory_limit_bytes()
        || manifest.generation_memory_enforcement_profile
            != memory_guard.memory_enforcement_profile()
    {
        bail!("Kagemusha V4 release memory contract differs from the active in-process guard");
    }
    let manifest_sha256 = kagemusha_recursive_spend_release_sha256(&manifest_bytes);
    let manifest_digest = read_regular_bounded(
        &root,
        MANIFEST_NORITO_SHA256_FILE_NAME,
        65,
        "Kagemusha V4 manifest digest",
    )?;
    if manifest_digest != format!("{}\n", hex::encode(manifest_sha256)).as_bytes() {
        bail!("Kagemusha V4 manifest digest sidecar does not match manifest.norito");
    }
    let manifest_json = read_regular_bounded(
        &root,
        MANIFEST_JSON_FILE_NAME,
        MAX_MANIFEST_BYTES,
        "Kagemusha V4 manifest JSON",
    )?;
    let manifest_json = std::str::from_utf8(&manifest_json)
        .wrap_err("Kagemusha V4 manifest JSON is not strict UTF-8")?;
    let manifest_from_json: KagemushaRecursiveSpendArtifactManifestV4 =
        norito::json::from_str(manifest_json)
            .wrap_err("Kagemusha V4 manifest JSON is malformed or non-canonical in shape")?;
    let mut canonical_manifest_json = norito::json::to_string_pretty(&manifest)
        .wrap_err("Kagemusha V4 manifest JSON cannot be canonically encoded")?;
    canonical_manifest_json.push('\n');
    if manifest_from_json != manifest || manifest_json.as_bytes() != canonical_manifest_json.as_bytes()
    {
        bail!("Kagemusha V4 JSON manifest is noncanonical or differs from manifest.norito");
    }
    let attestation_bytes = read_regular_bounded(
        &root,
        RELEASE_ATTESTATION_FILE_NAME_V4,
        MAX_ATTESTATION_BYTES,
        "Kagemusha V4 release attestation",
    )?;
    let attestation: KagemushaRecursiveSpendReleaseAttestationV4 =
        decode_canonical_norito(&attestation_bytes, "Kagemusha V4 release attestation")?;
    require_release_file_path(
        &root,
        benchmark_evidence_path,
        KAGEMUSHA_RECURSIVE_SPEND_BENCHMARK_EVIDENCE_FILE_NAME_V1,
        "physical-device benchmark",
    )?;
    require_release_file_path(
        &root,
        cryptographic_review_path,
        KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_FILE_NAME_V1,
        "canonical signed cryptographic review",
    )?;
    let benchmark = read_regular_bounded(
        &root,
        KAGEMUSHA_RECURSIVE_SPEND_BENCHMARK_EVIDENCE_FILE_NAME_V1,
        KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1,
        "physical-device benchmark",
    )?;
    let review = read_regular_bounded(
        &root,
        KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_FILE_NAME_V1,
        KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_MAX_BYTES_V4,
        "canonical signed cryptographic review",
    )?;
    let internal_validation_receipt = read_regular_bounded(
        &root,
        KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_FILE_NAME_V1,
        KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1,
        "canonical signed Kagemusha V4 internal-validation receipt",
    )?;
    let authenticated = KagemushaAuthenticatedReleaseV4::verify(
        &manifest,
        &policy,
        &attestation,
        &internal_validation_receipt,
        &benchmark,
        &review,
    )
    .map_err(|error| eyre!("Kagemusha V4 release authentication failed: {error}"))?;
    let candidate = manifest
        .immutable_candidate()
        .map_err(|error| eyre!("failed to reconstruct immutable V4 candidate: {error}"))?;
    let qualification_receipt_bytes = read_regular_bounded(
        &root,
        KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_FILE_NAME_V4,
        KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_MAX_BYTES_V4,
        "Kagemusha V4 recursive-step-two qualification receipt",
    )?;
    let qualification_receipt =
        KagemushaRecursiveSpendQualificationReceiptV4::decode_canonical_against_candidate(
            &qualification_receipt_bytes,
            &candidate,
        )
        .map_err(|error| eyre!("invalid Kagemusha V4 qualification receipt: {error}"))?;
    let qualification_receipt_sha256 = qualification_receipt
        .canonical_sha256_against_candidate(&candidate)
        .map_err(|error| eyre!("invalid Kagemusha V4 qualification receipt: {error}"))?;
    let qualified_candidate_sha256 = qualification_receipt
        .qualified_candidate_sha256(&candidate)
        .map_err(|error| eyre!("invalid Kagemusha V4 qualification receipt: {error}"))?;
    if qualification_receipt_sha256 != manifest.qualification_receipt_sha256
        || qualified_candidate_sha256 != manifest.qualified_candidate_sha256
    {
        bail!("Kagemusha V4 manifest does not bind the exact qualification receipt");
    }
    let descriptors: Vec<_> = manifest
        .profiles
        .iter()
        .flat_map(|profile| {
            profile
                .artifacts
                .iter()
                .map(move |descriptor| (profile, descriptor))
        })
        .collect();
    if descriptors.len() != AUTHENTICATED_ARTIFACT_ROLES_V4.len() {
        bail!("Kagemusha V4 release does not contain the exact eight-artifact inventory");
    }
    let mut payload_digests = BTreeSet::new();
    validate_artifacts_sequentially(
        descriptors.into_iter().zip(AUTHENTICATED_ARTIFACT_ROLES_V4),
        |((profile, descriptor), (expected_parity, expected_kind))| -> Result<_> {
            if profile.parity != expected_parity || descriptor.kind != expected_kind {
                bail!("Kagemusha V4 artifact inventory role order changed");
            }
            let maximum = usize::try_from(descriptor.size_bytes)
                .map_err(|_| eyre!("Kagemusha V4 artifact size does not fit this host"))?;
            let mut opened = open_regular_bounded(
                &root,
                &descriptor.file_name,
                maximum,
                "Kagemusha V4 artifact",
            )?;
            if u64::try_from(opened.length).ok() != Some(descriptor.size_bytes) {
                bail!("Kagemusha V4 artifact size changed while it was read");
            }
            let payload = read_kagemusha_pasta_cycle_artifact_v4(
                &mut opened.file,
                &authenticated,
                descriptor,
            )
            .map_err(|error| eyre!(error))?;
            opened.verify_unchanged()?;
            let header = payload.header();
            if header.parity != expected_parity || header.kind != expected_kind {
                bail!("Kagemusha V4 authenticated artifact header role changed");
            }
            if !payload_digests.insert(header.payload_sha256) {
                bail!("Kagemusha V4 authenticated artifact payloads are not distinct");
            }
            if expected_kind == KagemushaPastaCycleArtifactKindV4::BootstrapWitness {
                let measured = validate_kagemusha_step_bootstrap_payload_v4(
                    payload.payload(),
                    &profile.circuit_params,
                    expected_parity,
                    profile.compiled_protocol_structure_sha256,
                )
                .map_err(|error| eyre!(error))?;
                if u32::try_from(measured) != Ok(profile.step_proof_size_bytes) {
                    bail!(
                        "Kagemusha V4 bootstrap proof size does not match its authenticated profile"
                    );
                }
            }
            Ok(payload)
        },
    )?;
    if payload_digests.len() != AUTHENTICATED_ARTIFACT_ROLES_V4.len() {
        bail!("Kagemusha V4 authenticated artifact payload inventory changed");
    }
    verify_qualification_receipt_v4(
        &root,
        &authenticated,
        &candidate,
        &qualification_receipt,
        memory_guard,
    )?;
    if authenticated.manifest_sha256() == [0; 32]
        || authenticated.manifest_sha256() != manifest_sha256
        || authenticated.manifest().max_proof_bytes != manifest.max_proof_bytes
    {
        bail!("authenticated Kagemusha V4 material does not bind the canonical manifest");
    }
    verify_roster_v4(&root, &manifest)?;
    verify_exact_inventory_v4(&root, &manifest, inventory_state)?;
    let subject = manifest
        .release_attestation_subject()
        .map_err(|error| eyre!(error))?;
    let recursive_step_verifier_commitment = recursive_step_verifier_commitment_v4(&manifest)?;
    let report = VerificationReport::from_manifest_v4(
        &manifest,
        manifest_sha256,
        subject.manifest_subject_sha256,
        release_policy_sha256,
        recursive_step_verifier_commitment,
    );
    let verified = VerifiedReleaseV4 {
        authenticated,
        report,
    };
    if inventory_state.includes_promotion_record() {
        let expected_promotion = norito::to_bytes(&verified.promotion_record()?)
            .wrap_err("failed to encode canonical Kagemusha V4 promotion record")?;
        let promotion = read_regular_bounded(
            &root,
            PROMOTION_RECORD_FILE_NAME_V4,
            MAX_MANIFEST_BYTES,
            "Kagemusha V4 promotion record",
        )?;
        if promotion != expected_promotion {
            bail!("Kagemusha V4 promotion record is not candidate-bound to this release");
        }
    }
    Ok(verified)
}
fn roster_release_generations_match_v4(
    roster_generation: &str,
    manifest_generation: &str,
    descriptor_generation: &str,
) -> bool {
    let descriptor_matches_roster = descriptor_generation == roster_generation;
    let descriptor_matches_manifest = descriptor_generation == manifest_generation;
    descriptor_matches_roster && descriptor_matches_manifest
}
fn verify_qualification_receipt_v4(
    root: &Path,
    authenticated: &KagemushaAuthenticatedReleaseV4,
    candidate: &KagemushaRecursiveSpendCandidateV4,
    receipt: &KagemushaRecursiveSpendQualificationReceiptV4,
    memory_guard: KagemushaGenerationMemoryGuardV4,
) -> Outcome {
    let candidate_sha256 = candidate
        .sha256()
        .map_err(|error| eyre!("failed to identify immutable V4 candidate: {error}"))?;
    let candidate_manifest_bytes = norito::encode_canonical(&candidate.manifest)
        .wrap_err("failed to encode immutable V4 candidate manifest")?;
    let candidate_manifest_sha256 =
        kagemusha_recursive_spend_release_sha256(&candidate_manifest_bytes);
    let vesta_proving_key = kagemusha_artifact_descriptor_v4(
        &candidate.manifest,
        KagemushaPastaCycleParityV1::StepEq,
        KagemushaPastaCycleArtifactKindV4::ProvingKey,
    )
    .map_err(|error| eyre!(error))?;
    let pallas_proving_key = kagemusha_artifact_descriptor_v4(
        &candidate.manifest,
        KagemushaPastaCycleParityV1::StepEp,
        KagemushaPastaCycleArtifactKindV4::ProvingKey,
    )
    .map_err(|error| eyre!(error))?;
    let vesta_maximum = usize::try_from(vesta_proving_key.size_bytes)
        .map_err(|_| eyre!("Kagemusha V4 Eq proving-key size does not fit this host"))?;
    let pallas_maximum = usize::try_from(pallas_proving_key.size_bytes)
        .map_err(|_| eyre!("Kagemusha V4 Ep proving-key size does not fit this host"))?;
    let vesta_opened = open_regular_bounded(
        root,
        &vesta_proving_key.file_name,
        vesta_maximum,
        "Kagemusha V4 qualification Eq proving key",
    )?;
    let pallas_opened = open_regular_bounded(
        root,
        &pallas_proving_key.file_name,
        pallas_maximum,
        "Kagemusha V4 qualification Ep proving key",
    )?;
    let vesta_file = vesta_opened
        .file
        .try_clone()
        .wrap_err("failed to duplicate Kagemusha V4 Eq proving-key handle")?;
    let pallas_file = pallas_opened
        .file
        .try_clone()
        .wrap_err("failed to duplicate Kagemusha V4 Ep proving-key handle")?;
    let qualification_memory_contract =
        KagemushaQualificationMemoryContractV4::for_operator(&memory_guard);
    verify_candidate_recursive_step_two_receipt_v4(
        candidate,
        candidate_sha256,
        candidate_manifest_sha256,
        receipt,
        &qualification_memory_contract,
        vesta_file,
        pallas_file,
        |parity, kind| {
            if kind == KagemushaPastaCycleArtifactKindV4::ProvingKey {
                return Err(
                    "Kagemusha V4 bounded qualification loader requested a proving key".to_owned(),
                );
            }
            let descriptor =
                kagemusha_artifact_descriptor_v4(authenticated.manifest(), parity, kind)?;
            let maximum = usize::try_from(descriptor.size_bytes).map_err(|_| {
                "Kagemusha V4 qualification artifact size does not fit this host".to_owned()
            })?;
            let mut opened = open_regular_bounded(
                root,
                &descriptor.file_name,
                maximum,
                "Kagemusha V4 qualification artifact",
            )
            .map_err(|error| error.to_string())?;
            let payload = read_kagemusha_pasta_cycle_artifact_v4(
                &mut opened.file,
                authenticated,
                descriptor,
            )?;
            opened
                .verify_unchanged()
                .map_err(|error| error.to_string())?;
            Ok(payload)
        },
    )
    .map_err(|error| eyre!("Kagemusha V4 qualification proof verification failed: {error}"))?;
    vesta_opened.verify_unchanged()?;
    pallas_opened.verify_unchanged()?;
    Ok(())
}
fn verify_roster_v4(root: &Path, manifest: &KagemushaRecursiveSpendArtifactManifestV4) -> Outcome {
    let descriptor = &manifest.topup_finality_roster_artifact;
    let bytes = read_regular_bounded(
        root,
        &descriptor.file_name,
        usize::try_from(KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_V2)
            .expect("roster bound fits usize"),
        "Kagemusha V4 top-up finality roster",
    )?;
    if u64::try_from(bytes.len()).ok() != Some(descriptor.size_bytes)
        || kagemusha_recursive_spend_release_sha256(&bytes) != descriptor.sha256
    {
        bail!("Kagemusha V4 top-up finality roster size or digest mismatch");
    }
    let roster: KagemushaTopUpFinalityRosterArtifactV2 =
        decode_canonical_norito(&bytes, "Kagemusha V4 top-up finality roster")?;
    roster.validate().map_err(|error| eyre!(error))?;
    if roster.network_id != manifest.network_id
        || !roster_release_generations_match_v4(
            &roster.artifact_generation,
            &manifest.generation,
            &manifest.topup_finality_roster_artifact.artifact_generation,
        )
        || roster.window_at(manifest.activation_height).is_err()
        || roster
            .window_at(manifest.withdrawal_height.saturating_sub(1))
            .is_err()
    {
        bail!("Kagemusha V4 top-up finality roster release binding mismatch");
    }
    Ok(())
}
fn insert_expected_release_file_v4(
    expected: &mut BTreeSet<String>,
    file_name: &str,
    role: &str,
) -> Outcome {
    if !expected.insert(file_name.to_owned()) {
        bail!("Kagemusha V4 {role} aliases another release file as `{file_name}`");
    }
    Ok(())
}
fn verify_exact_inventory_v4(
    root: &Path,
    manifest: &KagemushaRecursiveSpendArtifactManifestV4,
    inventory_state: ReleaseInventoryStateV4,
) -> Outcome {
    let mut expected = BTreeSet::new();
    for (file_name, role) in [
        (MANIFEST_JSON_FILE_NAME, "manifest JSON"),
        (MANIFEST_NORITO_FILE_NAME, "manifest Norito"),
        (MANIFEST_NORITO_SHA256_FILE_NAME, "manifest digest"),
        (RELEASE_ATTESTATION_FILE_NAME_V4, "release attestation"),
        (
            KAGEMUSHA_RECURSIVE_SPEND_BENCHMARK_EVIDENCE_FILE_NAME_V1,
            "benchmark evidence",
        ),
        (
            KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_FILE_NAME_V1,
            "cryptographic review",
        ),
        (
            KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_FILE_NAME_V4,
            "qualification receipt",
        ),
        (
            KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_FILE_NAME_V1,
            "internal-validation receipt",
        ),
    ] {
        insert_expected_release_file_v4(&mut expected, file_name, role)?;
    }
    if inventory_state.includes_promotion_record() {
        insert_expected_release_file_v4(
            &mut expected,
            PROMOTION_RECORD_FILE_NAME_V4,
            "promotion record",
        )?;
    }
    insert_expected_release_file_v4(
        &mut expected,
        &manifest.topup_finality_roster_artifact.file_name,
        "top-up finality roster",
    )?;
    for artifact in manifest
        .profiles
        .iter()
        .flat_map(|profile| profile.artifacts.iter())
    {
        insert_expected_release_file_v4(
            &mut expected,
            &artifact.file_name,
            "authenticated artifact",
        )?;
    }
    if inventory_state.includes_promotion_record() && expected.len() != 18 {
        bail!("Kagemusha V4 promoted release contract must name exactly eighteen unique files");
    }
    if expected.len() != inventory_state.exact_file_count() {
        bail!(
            "Kagemusha V4 {} contract must name exactly {} unique files, found {}",
            inventory_state.label(),
            inventory_state.exact_file_count(),
            expected.len()
        );
    }
    let observed = fs::read_dir(root)
        .wrap_err("failed to enumerate Kagemusha V4 release directory")?
        .map(|entry| {
            entry
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .wrap_err("failed to inspect Kagemusha V4 release inventory")
        })
        .collect::<Result<BTreeSet<_>>>()?;
    if observed != expected {
        bail!(
            "Kagemusha V4 release inventory is not exact (missing={:?}, unexpected={:?})",
            expected.difference(&observed).collect::<Vec<_>>(),
            observed.difference(&expected).collect::<Vec<_>>()
        );
    }
    Ok(())
}
fn recursive_step_verifier_commitment_v4(
    manifest: &KagemushaRecursiveSpendArtifactManifestV4,
) -> Result<[u8; 32]> {
    let profiles = norito::to_bytes(&manifest.profiles)
        .wrap_err("failed to encode Kagemusha V4 verifier profiles")?;
    let mut preimage =
        Vec::with_capacity(RECURSIVE_STEP_VERIFIER_COMMITMENT_DOMAIN_V4.len() + 1 + profiles.len());
    preimage.extend_from_slice(RECURSIVE_STEP_VERIFIER_COMMITMENT_DOMAIN_V4);
    preimage.push(0);
    preimage.extend_from_slice(&profiles);
    Ok(kagemusha_recursive_spend_release_sha256(&preimage))
}
fn canonical_release_root(path: &Path) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(path).wrap_err_with(|| {
        format!(
            "Kagemusha release directory is unavailable: {}",
            path.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        bail!("Kagemusha release directory must be a non-symlink directory");
    }
    reject_unsafe_mode(&metadata, "Kagemusha release directory")?;
    path.canonicalize()
        .wrap_err("failed to canonicalize Kagemusha release directory")
}
fn require_new_promotion_record_path_v4(canonical_root: &Path, supplied: &Path) -> Outcome {
    let expected = canonical_root.join(PROMOTION_RECORD_FILE_NAME_V4);
    if !supplied.is_absolute() || supplied.as_os_str() != expected.as_os_str() {
        bail!(
            "promotion record must be the exact canonical absent `{}` bundle leaf",
            PROMOTION_RECORD_FILE_NAME_V4
        );
    }
    match fs::symlink_metadata(supplied) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Ok(_) => bail!("promotion record destination already exists and will not be replaced"),
        Err(error) => Err(error).wrap_err("failed to inspect promotion record destination"),
    }
}
struct PinnedRegularFile {
    file: File,
    path: PathBuf,
    before: fs::Metadata,
    length: usize,
    label: String,
}
impl PinnedRegularFile {
    fn verify_unchanged(&self) -> Outcome {
        let after_open = self
            .file
            .metadata()
            .wrap_err_with(|| format!("failed to re-inspect {}", self.label))?;
        let after_path = fs::symlink_metadata(&self.path)
            .wrap_err_with(|| format!("failed to re-inspect {} path", self.label))?;
        if after_path.file_type().is_symlink()
            || !after_path.is_file()
            || !same_file_snapshot(&self.before, &after_open)
            || !same_file_snapshot(&self.before, &after_path)
        {
            bail!("{} changed while it was read", self.label);
        }
        Ok(())
    }
}
fn open_regular_bounded(
    root: &Path,
    name: &str,
    max_bytes: usize,
    label: &str,
) -> Result<PinnedRegularFile> {
    if name.is_empty()
        || Path::new(name).is_absolute()
        || Path::new(name).components().count() != 1
        || matches!(name, "." | "..")
    {
        bail!("{label} has an unsafe file name");
    }
    let path = root.join(name);
    let before = fs::symlink_metadata(&path)
        .wrap_err_with(|| format!("{label} is unavailable: {}", path.display()))?;
    if before.file_type().is_symlink() || !before.is_file() {
        bail!("{label} must be a non-symlink regular file");
    }
    reject_unsafe_mode(&before, label)?;
    reject_hard_links(&before, label)?;
    let length = usize::try_from(before.len()).map_err(|_| eyre!("{label} is too large"))?;
    if length == 0 || length > max_bytes {
        bail!("{label} violates its size bound");
    }
    let file = File::open(&path).wrap_err_with(|| format!("failed to open {label}"))?;
    if !same_file_snapshot(
        &before,
        &file.metadata().wrap_err("failed to inspect open file")?,
    ) {
        bail!("{label} changed while it was opened");
    }
    Ok(PinnedRegularFile {
        file,
        path,
        before,
        length,
        label: label.to_owned(),
    })
}
fn read_regular_bounded(root: &Path, name: &str, max_bytes: usize, label: &str) -> Result<Vec<u8>> {
    let mut opened = open_regular_bounded(root, name, max_bytes, label)?;
    let mut bytes = Vec::with_capacity(opened.length.saturating_add(1));
    Read::by_ref(&mut opened.file)
        .take(
            u64::try_from(opened.length)
                .expect("opened file length fits u64")
                .saturating_add(1),
        )
        .read_to_end(&mut bytes)
        .wrap_err_with(|| format!("failed to read {label}"))?;
    if bytes.len() != opened.length || bytes.len() > max_bytes {
        bail!("{label} changed while it was read");
    }
    opened.verify_unchanged()?;
    Ok(bytes)
}
fn read_external_bounded(path: &Path, maximum: usize, label: &str) -> Result<Vec<u8>> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .canonicalize()
        .wrap_err_with(|| format!("failed to canonicalize {label} parent"))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| eyre!("{label} file name is not UTF-8"))?;
    read_regular_bounded(&parent, name, maximum, label)
}
fn require_release_file_path(root: &Path, supplied: &Path, name: &str, label: &str) -> Outcome {
    let supplied = supplied
        .canonicalize()
        .wrap_err_with(|| format!("failed to canonicalize {label}"))?;
    let expected = root
        .join(name)
        .canonicalize()
        .wrap_err_with(|| format!("failed to canonicalize in-release {label}"))?;
    if supplied != expected {
        bail!("{label} must be the canonical in-release `{name}` file");
    }
    Ok(())
}
fn decode_canonical_norito<T>(bytes: &[u8], label: &str) -> Result<T>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    let value =
        norito::decode_from_bytes(bytes).wrap_err_with(|| format!("failed to decode {label}"))?;
    let canonical =
        norito::to_bytes(&value).wrap_err_with(|| format!("failed to re-encode {label}"))?;
    if canonical != bytes {
        bail!("{label} is not canonical Norito");
    }
    Ok(value)
}
#[cfg(unix)]
fn reject_unsafe_mode(metadata: &fs::Metadata, label: &str) -> Outcome {
    use std::os::unix::fs::MetadataExt as _;
    if metadata.mode() & 0o022 != 0 {
        bail!("{label} must not be group/world writable");
    }
    Ok(())
}
#[cfg(not(unix))]
fn reject_unsafe_mode(_: &fs::Metadata, _: &str) -> Outcome {
    Ok(())
}
#[cfg(unix)]
fn reject_hard_links(metadata: &fs::Metadata, label: &str) -> Outcome {
    use std::os::unix::fs::MetadataExt as _;
    if metadata.nlink() != 1 {
        bail!("{label} must not be hard-linked");
    }
    Ok(())
}
#[cfg(not(unix))]
fn reject_hard_links(_: &fs::Metadata, _: &str) -> Outcome {
    Ok(())
}
#[cfg(unix)]
fn same_file_snapshot(left: &fs::Metadata, right: &fs::Metadata) -> bool {
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
fn same_file_snapshot(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.len() == right.len() && left.modified().ok() == right.modified().ok()
}
#[derive(Debug)]
#[must_use = "a post-rename commit-uncertain outcome must be handled explicitly"]
enum DurableFilePublicationOutcomeV1 {
    Committed { final_path: PathBuf },
    CommitUncertain { final_path: PathBuf, reason: String },
}
impl DurableFilePublicationOutcomeV1 {
    fn operator_record(&self) -> String {
        let (status, final_path, durable, reason) = match self {
            Self::Committed { final_path } => ("committed", final_path, true, None),
            Self::CommitUncertain { final_path, reason } => {
                ("commit-uncertain", final_path, false, Some(reason.as_str()))
            }
        };
        #[cfg(unix)]
        let final_path_hex = {
            use std::os::unix::ffi::OsStrExt as _;
            hex::encode(final_path.as_os_str().as_bytes())
        };
        #[cfg(not(unix))]
        let final_path_hex = hex::encode(final_path.to_string_lossy().as_bytes());
        let reason_hex =
            reason.map_or_else(|| "-".to_owned(), |value| hex::encode(value.as_bytes()));
        format!(
            "{DURABLE_FILE_PUBLICATION_OUTCOME_SCHEMA_V1} status={status} final_path_encoding=bytes-hex final_path_hex={final_path_hex} parent_directory_durable={} reason_utf8_hex={reason_hex}",
            u8::from(durable),
        )
    }
}
#[derive(Debug)]
#[must_use = "a post-rename circuit-parameter commit outcome must be handled explicitly"]
enum ReleaseCircuitParamsPublicationOutcomeV1 {
    Committed { final_path: PathBuf },
    CommitUncertain { final_path: PathBuf, reason: String },
}
impl ReleaseCircuitParamsPublicationOutcomeV1 {
    fn operator_record(&self) -> String {
        let (status, final_path, durable, reason) = match self {
            Self::Committed { final_path } => ("committed", final_path, true, None),
            Self::CommitUncertain { final_path, reason } => {
                ("commit-uncertain", final_path, false, Some(reason.as_str()))
            }
        };
        #[cfg(unix)]
        let final_path_hex = {
            use std::os::unix::ffi::OsStrExt as _;
            hex::encode(final_path.as_os_str().as_bytes())
        };
        #[cfg(not(unix))]
        let final_path_hex = hex::encode(final_path.to_string_lossy().as_bytes());
        let reason_hex =
            reason.map_or_else(|| "-".to_owned(), |value| hex::encode(value.as_bytes()));
        format!(
            "{RELEASE_CIRCUIT_PARAMS_PUBLICATION_OUTCOME_SCHEMA_V1} status={status} final_path_encoding=bytes-hex final_path_hex={final_path_hex} parent_directory_durable={} reason_utf8_hex={reason_hex}",
            u8::from(durable),
        )
    }
}
#[cfg(unix)]
fn stat_field_matches_v1<Actual, Expected>(actual: Actual, expected: Expected) -> bool
where
    Actual: TryInto<Expected>,
    Expected: PartialEq,
{
    actual.try_into().ok() == Some(expected)
}
#[cfg(unix)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PromotionDirectorySnapshotV1 {
    device: u64,
    inode: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    links: u64,
}
#[cfg(unix)]
impl PromotionDirectorySnapshotV1 {
    fn from_metadata(metadata: &fs::Metadata) -> Option<Self> {
        use std::os::unix::fs::MetadataExt as _;
        metadata.is_dir().then(|| Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            mode: metadata.mode(),
            uid: metadata.uid(),
            gid: metadata.gid(),
            links: metadata.nlink(),
        })
    }
    fn validate_trusted(self, label: &str) -> Result<()> {
        let effective_uid = rustix::process::geteuid().as_raw();
        if (self.uid != 0 && self.uid != effective_uid) || self.mode & 0o022 != 0 || self.links == 0
        {
            bail!(
                "{label} must be owned by root or the effective uid, non-writable by group/other, and linked"
            );
        }
        Ok(())
    }
    fn validate_private(self, label: &str) -> Result<()> {
        self.validate_trusted(label)?;
        if self.uid != rustix::process::geteuid().as_raw()
            || self.mode & 0o777 != 0o700
            || self.links == 0
        {
            bail!("{label} must be an owner-private 0700 directory");
        }
        Ok(())
    }
    fn matches_except_links(self, other: Self) -> bool {
        self.device == other.device
            && self.inode == other.inode
            && self.mode == other.mode
            && self.uid == other.uid
            && self.gid == other.gid
    }
}
#[cfg(unix)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PromotionFileSnapshotV1 {
    device: u64,
    inode: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    links: u64,
    length: u64,
}
#[cfg(unix)]
impl PromotionFileSnapshotV1 {
    fn from_metadata(metadata: &fs::Metadata) -> Option<Self> {
        use std::os::unix::fs::MetadataExt as _;
        (metadata.is_file() && metadata.nlink() == 1).then(|| Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            mode: metadata.mode(),
            uid: metadata.uid(),
            gid: metadata.gid(),
            links: metadata.nlink(),
            length: metadata.len(),
        })
    }
    fn validate_private(self) -> Result<()> {
        if self.uid != rustix::process::geteuid().as_raw()
            || self.mode & 0o777 != 0o600
            || self.links != 1
        {
            bail!("promotion-record temporary file has unsafe custody");
        }
        Ok(())
    }
}
#[cfg(unix)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NoReplaceRenameNameStateV1 {
    Missing,
    Owned,
    Foreign,
}
#[cfg(unix)]
impl NoReplaceRenameNameStateV1 {
    fn operator_label(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Owned => "owned",
            Self::Foreign => "foreign",
        }
    }
}
#[cfg(unix)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FailedNoReplaceRenameDispositionV1 {
    PreCommit,
    CommitUncertain,
}
#[cfg(unix)]
fn classify_failed_no_replace_rename_v1(
    target: Option<NoReplaceRenameNameStateV1>,
    staging: Option<NoReplaceRenameNameStateV1>,
) -> FailedNoReplaceRenameDispositionV1 {
    if matches!(
        target,
        Some(NoReplaceRenameNameStateV1::Missing | NoReplaceRenameNameStateV1::Foreign)
    ) && staging == Some(NoReplaceRenameNameStateV1::Owned)
    {
        FailedNoReplaceRenameDispositionV1::PreCommit
    } else {
        // A syscall error alone does not prove that the rename stayed on the
        // pre-commit side. An owned target or a missing, changed, or
        // uninspectable staging binding may be evidence of a completed rename
        // or a concurrent name substitution.
        FailedNoReplaceRenameDispositionV1::CommitUncertain
    }
}
#[cfg(unix)]
enum FailedNoReplaceRenameReconciliationV1 {
    PreCommit,
    CommitUncertain { reason: String },
}
#[cfg(unix)]
fn rename_name_observation_label_v1(observation: &Result<NoReplaceRenameNameStateV1>) -> String {
    match observation {
        Ok(state) => state.operator_label().to_owned(),
        Err(error) => format!("uninspectable ({error:#})"),
    }
}
#[cfg(unix)]
fn reconcile_failed_no_replace_rename_v1<C>(
    rename_error_chain: &str,
    target: &Result<NoReplaceRenameNameStateV1>,
    staging: &Result<NoReplaceRenameNameStateV1>,
    cleanup_exact_owned_staging: C,
) -> FailedNoReplaceRenameReconciliationV1
where
    C: FnOnce() -> Result<()>,
{
    let disposition = classify_failed_no_replace_rename_v1(
        target.as_ref().ok().copied(),
        staging.as_ref().ok().copied(),
    );
    if disposition == FailedNoReplaceRenameDispositionV1::PreCommit {
        return match cleanup_exact_owned_staging() {
            Ok(()) => FailedNoReplaceRenameReconciliationV1::PreCommit,
            Err(error) => FailedNoReplaceRenameReconciliationV1::CommitUncertain {
                reason: format!(
                    "{rename_error_chain}; post-error reconciliation proved an absent or foreign destination and owned staging inode, but exact staging cleanup became uncertain: {error:#}"
                ),
            },
        };
    }
    FailedNoReplaceRenameReconciliationV1::CommitUncertain {
        reason: format!(
            "{rename_error_chain}; post-error reconciliation is commit-uncertain and left all names untouched: target={}, staging={}",
            rename_name_observation_label_v1(target),
            rename_name_observation_label_v1(staging),
        ),
    }
}
#[cfg(unix)]
fn inspect_no_replace_rename_name_v1<M>(
    parent: &File,
    name: &std::ffi::OsStr,
    matches_owned: M,
    label: &str,
) -> Result<NoReplaceRenameNameStateV1>
where
    M: FnOnce(&rustix::fs::Stat) -> bool,
{
    match rustix::fs::statat(parent, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) if matches_owned(&stat) => Ok(NoReplaceRenameNameStateV1::Owned),
        Ok(_) => Ok(NoReplaceRenameNameStateV1::Foreign),
        Err(error) if error == rustix::io::Errno::NOENT => Ok(NoReplaceRenameNameStateV1::Missing),
        Err(error) => Err(error).wrap_err_with(|| format!("failed to inspect {label}")),
    }
}
#[cfg(unix)]
struct PinnedPromotionParentV1 {
    path: PathBuf,
    file: File,
    snapshot: PromotionDirectorySnapshotV1,
    path_chain: Vec<(PathBuf, PromotionDirectorySnapshotV1)>,
}
#[cfg(unix)]
impl PinnedPromotionParentV1 {
    #[expect(
        clippy::too_many_lines,
        reason = "descriptor traversal and every before/open/after identity check form one fail-closed parent-pinning operation"
    )]
    fn open(path: &Path) -> Result<Self> {
        use rustix::fs::{AtFlags, Mode, OFlags, open, openat, statat};
        use std::path::Component;
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .wrap_err("resolve promotion-record current directory")?
                .join(path)
        };
        let before =
            fs::symlink_metadata(&absolute).wrap_err("promotion-record parent is unavailable")?;
        if before.file_type().is_symlink() || !before.is_dir() {
            bail!("promotion-record parent must be a non-symlink directory");
        }
        let before = PromotionDirectorySnapshotV1::from_metadata(&before)
            .ok_or_else(|| eyre!("promotion-record parent must be a directory"))?;
        let canonical = fs::canonicalize(&absolute)
            .wrap_err("failed to canonicalize promotion-record parent")?;
        let root_path = Path::new("/");
        let root_before = fs::symlink_metadata(root_path)
            .wrap_err("failed to inspect promotion-record filesystem root")?;
        let root_snapshot = PromotionDirectorySnapshotV1::from_metadata(&root_before)
            .ok_or_else(|| eyre!("promotion-record filesystem root is not a directory"))?;
        root_snapshot.validate_trusted("promotion-record filesystem root")?;
        let mut file = File::from(
            open(
                root_path,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .wrap_err("failed to pin promotion-record filesystem root")?,
        );
        let opened_root = PromotionDirectorySnapshotV1::from_metadata(
            &file
                .metadata()
                .wrap_err("failed to inspect pinned promotion-record filesystem root")?,
        )
        .ok_or_else(|| eyre!("pinned promotion-record filesystem root is not a directory"))?;
        if !root_snapshot.matches_except_links(opened_root) || opened_root.links == 0 {
            bail!("promotion-record filesystem root changed while it was pinned");
        }
        let mut current_path = root_path.to_path_buf();
        let mut path_chain = vec![(current_path.clone(), root_snapshot)];
        let mut snapshot = root_snapshot;
        for component in canonical.components().skip(1) {
            let Component::Normal(name) = component else {
                bail!("promotion-record parent contains a non-canonical path component");
            };
            let stat_before = statat(&file, name, AtFlags::SYMLINK_NOFOLLOW)
                .wrap_err("failed to inspect promotion-record parent component")?;
            if rustix::fs::FileType::from_raw_mode(stat_before.st_mode)
                != rustix::fs::FileType::Directory
            {
                bail!("promotion-record parent chain contains a non-directory");
            }
            let next = File::from(
                openat(
                    &file,
                    name,
                    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .wrap_err("failed to pin promotion-record parent component")?,
            );
            let next_snapshot = PromotionDirectorySnapshotV1::from_metadata(
                &next
                    .metadata()
                    .wrap_err("failed to inspect pinned promotion-record parent component")?,
            )
            .ok_or_else(|| eyre!("promotion-record parent component is not a directory"))?;
            let stat_after = statat(&file, name, AtFlags::SYMLINK_NOFOLLOW)
                .wrap_err("failed to re-inspect promotion-record parent component")?;
            let stat_identity_matches =
                stat_field_matches_v1(stat_after.st_dev, next_snapshot.device)
                    && stat_field_matches_v1(stat_after.st_ino, next_snapshot.inode)
                    && stat_field_matches_v1(stat_after.st_mode, next_snapshot.mode)
                    && stat_field_matches_v1(stat_after.st_uid, next_snapshot.uid)
                    && stat_field_matches_v1(stat_after.st_gid, next_snapshot.gid)
                    && stat_after.st_nlink > 0;
            if !stat_identity_matches {
                bail!("promotion-record parent component changed while it was pinned");
            }
            current_path.push(name);
            next_snapshot.validate_trusted(&format!(
                "promotion-record parent component `{}`",
                current_path.display()
            ))?;
            file = next;
            snapshot = next_snapshot;
            path_chain.push((current_path.clone(), snapshot));
        }
        if snapshot != before {
            bail!("promotion-record parent changed while its descriptor was pinned");
        }
        let parent = Self {
            path: absolute,
            file,
            snapshot,
            path_chain,
        };
        parent.verify_path_identity()?;
        Ok(parent)
    }
    fn snapshot_after_one_staged_entry(&self, label: &str) -> Result<PromotionDirectorySnapshotV1> {
        let current =
            PromotionDirectorySnapshotV1::from_metadata(&self.file.metadata().wrap_err_with(
                || format!("failed to inspect promotion-record parent after staging {label}"),
            )?)
            .ok_or_else(|| eyre!("promotion-record parent is no longer a directory"))?;
        // Unix filesystems disagree on whether a newly staged entry increments
        // its parent's link count. Admit only that single known transition.
        if !self.snapshot.matches_except_links(current)
            || current
                .links
                .checked_sub(self.snapshot.links)
                .is_none_or(|delta| delta > 1)
        {
            bail!("promotion-record parent changed unexpectedly while staging {label}");
        }
        self.verify_path_identity_against(current)?;
        Ok(current)
    }
    fn verify_path_identity(&self) -> Result<()> {
        self.verify_path_identity_against(self.snapshot)
    }
    fn verify_path_identity_against(
        &self,
        expected_parent: PromotionDirectorySnapshotV1,
    ) -> Result<()> {
        let opened = PromotionDirectorySnapshotV1::from_metadata(
            &self
                .file
                .metadata()
                .wrap_err("failed to re-inspect pinned promotion-record parent")?,
        );
        if opened != Some(expected_parent) {
            bail!("pinned promotion-record parent changed identity");
        }
        let final_component = self.path_chain.len().saturating_sub(1);
        for (index, (path, original_expected)) in self.path_chain.iter().enumerate() {
            let current = fs::symlink_metadata(path).wrap_err_with(|| {
                format!(
                    "failed to re-inspect promotion-record ancestor {}",
                    path.display()
                )
            })?;
            let current_snapshot = PromotionDirectorySnapshotV1::from_metadata(&current);
            let identity_matches = current_snapshot.is_some_and(|current| {
                if index == final_component {
                    current == expected_parent
                } else {
                    original_expected.matches_except_links(current) && current.links > 0
                }
            });
            if current.file_type().is_symlink() || !identity_matches {
                bail!(
                    "promotion-record ancestor changed after it was pinned: {}",
                    path.display()
                );
            }
        }
        let named = fs::symlink_metadata(&self.path)
            .wrap_err("failed to re-inspect named promotion-record parent")?;
        if named.file_type().is_symlink()
            || PromotionDirectorySnapshotV1::from_metadata(&named) != Some(expected_parent)
        {
            bail!("promotion-record parent pathname changed after it was pinned");
        }
        Ok(())
    }
}
#[cfg(unix)]
fn random_promotion_temporary_name_v1(target: &std::ffi::OsStr) -> Result<std::ffi::OsString> {
    use rand::{TryRngCore as _, rngs::OsRng};
    use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};
    let mut random = [0_u8; 16];
    OsRng
        .try_fill_bytes(&mut random)
        .wrap_err("obtain OS entropy for promotion-record temporary name")?;
    let mut bytes = Vec::with_capacity(target.as_bytes().len() + 39);
    bytes.push(b'.');
    bytes.extend_from_slice(target.as_bytes());
    bytes.extend_from_slice(b".tmp.");
    bytes.extend_from_slice(hex::encode(random).as_bytes());
    Ok(std::ffi::OsString::from_vec(bytes))
}
#[cfg(unix)]
fn cleanup_promotion_temporary_v1(
    parent: &PinnedPromotionParentV1,
    temporary_name: &std::ffi::OsStr,
) -> Result<()> {
    rustix::fs::unlinkat(&parent.file, temporary_name, rustix::fs::AtFlags::empty())
        .wrap_err("failed to remove unpublished promotion-record temporary file")?;
    Ok(())
}
#[cfg(unix)]
#[expect(
    clippy::too_many_lines,
    reason = "private staging, the atomic publication point, and post-publication durability classification must remain one ordered operation"
)]
fn write_new_durable_file_with_hooks_v1<B, S>(
    path: &Path,
    bytes: &[u8],
    before_publish: B,
    sync_parent: S,
) -> Result<DurableFilePublicationOutcomeV1>
where
    B: FnOnce() -> Result<()>,
    S: FnOnce(&File) -> std::io::Result<()>,
{
    use rustix::fs::{AtFlags, Mode, OFlags, RenameFlags, openat, renameat_with, statat};
    let parent_path = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| eyre!("promotion-record path has no parent"))?;
    let target_name = path
        .file_name()
        .ok_or_else(|| eyre!("promotion-record path has no file name"))?;
    let mut components = Path::new(target_name).components();
    if !matches!(components.next(), Some(std::path::Component::Normal(name)) if name == target_name)
        || components.next().is_some()
    {
        bail!("promotion-record file name must be one normal path component");
    }
    let parent = PinnedPromotionParentV1::open(parent_path)?;
    match statat(&parent.file, target_name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(_) => bail!("refusing to overwrite or alias an existing promotion record"),
        Err(error) if error == rustix::io::Errno::NOENT => {}
        Err(error) => return Err(error).wrap_err("failed to inspect promotion-record destination"),
    }
    let temporary_name = random_promotion_temporary_name_v1(target_name)?;
    let mut temporary = File::from(
        openat(
            &parent.file,
            &temporary_name,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )
        .wrap_err("failed to create private promotion-record temporary file")?,
    );
    let publication_parent_snapshot =
        match parent.snapshot_after_one_staged_entry("a promotion-record file") {
            Ok(snapshot) => snapshot,
            Err(error) => {
                cleanup_promotion_temporary_v1(&parent, &temporary_name)?;
                return Err(error);
            }
        };
    let write_result = (|| -> Result<PromotionFileSnapshotV1> {
        PromotionFileSnapshotV1::from_metadata(
            &temporary
                .metadata()
                .wrap_err("failed to inspect promotion-record temporary file")?,
        )
        .ok_or_else(|| eyre!("promotion-record temporary is not a single-link regular file"))?
        .validate_private()?;
        temporary
            .write_all(bytes)
            .wrap_err("failed to write Kagemusha promotion record")?;
        temporary
            .sync_all()
            .wrap_err("failed to durably sync Kagemusha promotion record")?;
        let snapshot = PromotionFileSnapshotV1::from_metadata(
            &temporary
                .metadata()
                .wrap_err("failed to re-inspect promotion-record temporary file")?,
        )
        .ok_or_else(|| eyre!("promotion-record temporary lost its regular-file identity"))?;
        snapshot.validate_private()?;
        let expected_length = u64::try_from(bytes.len())
            .map_err(|_| eyre!("promotion-record length does not fit u64"))?;
        if snapshot.length != expected_length {
            bail!("promotion-record temporary file has the wrong final length");
        }
        Ok(snapshot)
    })();
    let snapshot = match write_result {
        Ok(snapshot) => snapshot,
        Err(error) => {
            cleanup_promotion_temporary_v1(&parent, &temporary_name)?;
            return Err(error);
        }
    };
    if let Err(error) = before_publish()
        .and_then(|()| parent.verify_path_identity_against(publication_parent_snapshot))
        .and_then(|()| {
            verify_pinned_file_contents_v1(
                &parent.file,
                &temporary_name,
                snapshot,
                bytes,
                "staged promotion record",
            )
        })
    {
        cleanup_promotion_temporary_v1(&parent, &temporary_name)?;
        return Err(error);
    }
    if let Err(error) = renameat_with(
        &parent.file,
        &temporary_name,
        &parent.file,
        target_name,
        RenameFlags::NOREPLACE,
    ) {
        let rename_error =
            eyre!(error).wrap_err("failed to atomically publish new promotion record");
        let rename_error_chain = format!("{rename_error:#}");
        let target_observation = inspect_no_replace_rename_name_v1(
            &parent.file,
            target_name,
            |stat| promotion_file_snapshot_has_stat_identity_v1(snapshot, stat),
            "promotion-record destination after failed rename",
        );
        let staging_observation = inspect_no_replace_rename_name_v1(
            &parent.file,
            &temporary_name,
            |stat| release_circuit_params_file_snapshot_matches_stat_v1(snapshot, stat),
            "promotion-record staging binding after failed rename",
        );
        return match reconcile_failed_no_replace_rename_v1(
            &rename_error_chain,
            &target_observation,
            &staging_observation,
            || {
                parent.verify_path_identity_against(publication_parent_snapshot)?;
                verify_pinned_file_contents_v1(
                    &parent.file,
                    &temporary_name,
                    snapshot,
                    bytes,
                    "post-error staged promotion record",
                )?;
                cleanup_promotion_temporary_v1(&parent, &temporary_name)?;
                parent.verify_path_identity()
            },
        ) {
            FailedNoReplaceRenameReconciliationV1::PreCommit => Err(rename_error),
            FailedNoReplaceRenameReconciliationV1::CommitUncertain { reason } => {
                Ok(DurableFilePublicationOutcomeV1::CommitUncertain {
                    final_path: parent.path.join(target_name),
                    reason,
                })
            }
        };
    }
    let final_path = parent.path.join(target_name);
    let after_publication = (|| -> Result<()> {
        verify_pinned_file_contents_v1(
            &parent.file,
            target_name,
            snapshot,
            bytes,
            "published promotion record",
        )?;
        sync_parent(&parent.file).wrap_err("failed to durably sync promotion-record parent")?;
        parent.verify_path_identity_against(publication_parent_snapshot)?;
        verify_pinned_file_contents_v1(
            &parent.file,
            target_name,
            snapshot,
            bytes,
            "durably published promotion record",
        )?;
        Ok(())
    })();
    Ok(match after_publication {
        Ok(()) => DurableFilePublicationOutcomeV1::Committed { final_path },
        Err(error) => DurableFilePublicationOutcomeV1::CommitUncertain {
            final_path,
            reason: format!("{error:#}"),
        },
    })
}
#[cfg(unix)]
fn promotion_file_snapshot_has_stat_identity_v1(
    snapshot: PromotionFileSnapshotV1,
    stat: &rustix::fs::Stat,
) -> bool {
    stat_field_matches_v1(stat.st_dev, snapshot.device)
        && stat_field_matches_v1(stat.st_ino, snapshot.inode)
}
#[cfg(unix)]
fn release_circuit_params_file_snapshot_matches_stat_v1(
    snapshot: PromotionFileSnapshotV1,
    stat: &rustix::fs::Stat,
) -> bool {
    stat_field_matches_v1(stat.st_dev, snapshot.device)
        && stat_field_matches_v1(stat.st_ino, snapshot.inode)
        && stat_field_matches_v1(stat.st_mode, snapshot.mode)
        && stat_field_matches_v1(stat.st_uid, snapshot.uid)
        && stat_field_matches_v1(stat.st_gid, snapshot.gid)
        && stat_field_matches_v1(stat.st_nlink, snapshot.links)
        && stat_field_matches_v1(stat.st_size, snapshot.length)
}
#[cfg(unix)]
fn verify_pinned_file_contents_v1(
    directory: &File,
    file_name: &std::ffi::OsStr,
    snapshot: PromotionFileSnapshotV1,
    expected_bytes: &[u8],
    label: &str,
) -> Result<()> {
    use rustix::fs::{AtFlags, Mode, OFlags, openat, statat};
    let expected_length = u64::try_from(expected_bytes.len())
        .map_err(|_| eyre!("{label} length does not fit u64"))?;
    if snapshot.length != expected_length {
        bail!("{label} snapshot has the wrong length");
    }
    let linked_before = statat(directory, file_name, AtFlags::SYMLINK_NOFOLLOW)
        .wrap_err_with(|| format!("failed to inspect {label} binding"))?;
    if !release_circuit_params_file_snapshot_matches_stat_v1(snapshot, &linked_before) {
        bail!("{label} changed identity or custody");
    }
    let opened = File::from(
        openat(
            directory,
            file_name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .wrap_err_with(|| format!("failed to pin {label}"))?,
    );
    let opened_snapshot = PromotionFileSnapshotV1::from_metadata(
        &opened
            .metadata()
            .wrap_err_with(|| format!("failed to inspect pinned {label}"))?,
    );
    if opened_snapshot != Some(snapshot) {
        bail!("{label} binding does not identify the staged file");
    }
    let read_limit = expected_length
        .checked_add(1)
        .ok_or_else(|| eyre!("{label} read limit overflow"))?;
    let mut actual = Vec::new();
    actual
        .try_reserve_exact(expected_bytes.len().saturating_add(1))
        .map_err(|_| eyre!("failed to reserve {label} verification buffer"))?;
    (&opened)
        .take(read_limit)
        .read_to_end(&mut actual)
        .wrap_err_with(|| format!("failed to read pinned {label}"))?;
    if actual != expected_bytes {
        bail!("{label} content does not match the authenticated bytes");
    }
    let opened_after = PromotionFileSnapshotV1::from_metadata(
        &opened
            .metadata()
            .wrap_err_with(|| format!("failed to re-inspect pinned {label}"))?,
    );
    let linked_after = statat(directory, file_name, AtFlags::SYMLINK_NOFOLLOW)
        .wrap_err_with(|| format!("failed to re-inspect {label} binding"))?;
    if opened_after != Some(snapshot)
        || !release_circuit_params_file_snapshot_matches_stat_v1(snapshot, &linked_after)
    {
        bail!("{label} changed while its content was verified");
    }
    Ok(())
}
#[cfg(unix)]
fn promotion_directory_snapshot_has_stat_identity_v1(
    snapshot: PromotionDirectorySnapshotV1,
    stat: &rustix::fs::Stat,
) -> bool {
    stat_field_matches_v1(stat.st_dev, snapshot.device)
        && stat_field_matches_v1(stat.st_ino, snapshot.inode)
}
#[cfg(unix)]
fn release_circuit_params_directory_snapshot_matches_stat_v1(
    snapshot: PromotionDirectorySnapshotV1,
    stat: &rustix::fs::Stat,
) -> bool {
    stat_field_matches_v1(stat.st_dev, snapshot.device)
        && stat_field_matches_v1(stat.st_ino, snapshot.inode)
        && stat_field_matches_v1(stat.st_mode, snapshot.mode)
        && stat_field_matches_v1(stat.st_uid, snapshot.uid)
        && stat_field_matches_v1(stat.st_gid, snapshot.gid)
        && stat_field_matches_v1(stat.st_nlink, snapshot.links)
}
#[cfg(unix)]
#[expect(
    clippy::similar_names,
    clippy::too_many_arguments,
    reason = "Eq and Ep are distinct protocol-defined leaves in one atomic publication set"
)]
fn verify_release_circuit_params_directory_contents_v1(
    parent: &File,
    directory: &File,
    directory_name: &std::ffi::OsStr,
    directory_snapshot: PromotionDirectorySnapshotV1,
    eq_snapshot: PromotionFileSnapshotV1,
    ep_snapshot: PromotionFileSnapshotV1,
    expected_bytes: &[u8],
    label: &str,
) -> Result<()> {
    use rustix::fs::{AtFlags, statat};
    let opened = PromotionDirectorySnapshotV1::from_metadata(
        &directory
            .metadata()
            .wrap_err_with(|| format!("failed to inspect {label} directory"))?,
    );
    let linked = statat(parent, directory_name, AtFlags::SYMLINK_NOFOLLOW)
        .wrap_err_with(|| format!("failed to inspect {label} directory binding"))?;
    if opened != Some(directory_snapshot)
        || !release_circuit_params_directory_snapshot_matches_stat_v1(directory_snapshot, &linked)
    {
        bail!("{label} directory changed identity or custody");
    }
    verify_pinned_file_contents_v1(
        directory,
        std::ffi::OsStr::new(RELEASE_STEP_EQ_CIRCUIT_PARAMS_FILE_NAME_V4),
        eq_snapshot,
        expected_bytes,
        &format!("{label} Eq circuit parameters"),
    )?;
    verify_pinned_file_contents_v1(
        directory,
        std::ffi::OsStr::new(RELEASE_STEP_EP_CIRCUIT_PARAMS_FILE_NAME_V4),
        ep_snapshot,
        expected_bytes,
        &format!("{label} Ep circuit parameters"),
    )?;
    let opened_after = PromotionDirectorySnapshotV1::from_metadata(
        &directory
            .metadata()
            .wrap_err_with(|| format!("failed to re-inspect {label} directory"))?,
    );
    let linked_after = statat(parent, directory_name, AtFlags::SYMLINK_NOFOLLOW)
        .wrap_err_with(|| format!("failed to re-inspect {label} directory binding"))?;
    if opened_after != Some(directory_snapshot)
        || !release_circuit_params_directory_snapshot_matches_stat_v1(
            directory_snapshot,
            &linked_after,
        )
    {
        bail!("{label} directory changed while its leaves were verified");
    }
    Ok(())
}
#[cfg(unix)]
fn write_release_circuit_params_staged_file_v1(
    directory: &File,
    file_name: &str,
    bytes: &[u8],
) -> Result<PromotionFileSnapshotV1> {
    use rustix::fs::{AtFlags, Mode, OFlags, openat, statat};
    let mut file = File::from(
        openat(
            directory,
            file_name,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )
        .wrap_err_with(|| format!("failed to create staged `{file_name}`"))?,
    );
    PromotionFileSnapshotV1::from_metadata(
        &file
            .metadata()
            .wrap_err_with(|| format!("failed to inspect staged `{file_name}`"))?,
    )
    .ok_or_else(|| eyre!("staged `{file_name}` is not a single-link regular file"))?
    .validate_private()?;
    file.write_all(bytes)
        .wrap_err_with(|| format!("failed to write staged `{file_name}`"))?;
    file.sync_all()
        .wrap_err_with(|| format!("failed to sync staged `{file_name}`"))?;
    let snapshot = PromotionFileSnapshotV1::from_metadata(
        &file
            .metadata()
            .wrap_err_with(|| format!("failed to re-inspect staged `{file_name}`"))?,
    )
    .ok_or_else(|| eyre!("staged `{file_name}` lost its regular-file identity"))?;
    snapshot.validate_private()?;
    if snapshot.length
        != u64::try_from(bytes.len())
            .map_err(|_| eyre!("staged `{file_name}` length does not fit u64"))?
    {
        bail!("staged `{file_name}` has the wrong final length");
    }
    let linked = statat(directory, file_name, AtFlags::SYMLINK_NOFOLLOW)
        .wrap_err_with(|| format!("failed to inspect staged `{file_name}` binding"))?;
    if !release_circuit_params_file_snapshot_matches_stat_v1(snapshot, &linked) {
        bail!("staged `{file_name}` changed identity or custody");
    }
    Ok(snapshot)
}
#[cfg(unix)]
fn cleanup_release_circuit_params_staging_v1(
    parent: &PinnedPromotionParentV1,
    staging: &File,
    temporary_name: &std::ffi::OsStr,
) -> Result<()> {
    use rustix::fs::{AtFlags, unlinkat};
    for file_name in [
        RELEASE_STEP_EQ_CIRCUIT_PARAMS_FILE_NAME_V4,
        RELEASE_STEP_EP_CIRCUIT_PARAMS_FILE_NAME_V4,
    ] {
        match unlinkat(staging, file_name, AtFlags::empty()) {
            Ok(()) => {}
            Err(error) if error == rustix::io::Errno::NOENT => {}
            Err(error) => {
                return Err(error)
                    .wrap_err_with(|| format!("failed to remove staged `{file_name}`"));
            }
        }
    }
    staging
        .sync_all()
        .wrap_err("failed to sync cleaned circuit-parameter staging directory")?;
    unlinkat(&parent.file, temporary_name, AtFlags::REMOVEDIR)
        .wrap_err("failed to remove circuit-parameter staging directory")?;
    parent
        .file
        .sync_all()
        .wrap_err("failed to sync circuit-parameter parent after cleanup")?;
    Ok(())
}
#[cfg(unix)]
#[expect(
    clippy::similar_names,
    clippy::too_many_lines,
    reason = "the protocol-defined Eq/Ep staging set and its durability classification form one ordered operation"
)]
fn write_release_circuit_params_directory_with_hooks_v1<B, S>(
    path: &Path,
    bytes: &[u8],
    before_publish: B,
    sync_parent: S,
) -> Result<ReleaseCircuitParamsPublicationOutcomeV1>
where
    B: FnOnce() -> Result<()>,
    S: FnOnce(&File) -> std::io::Result<()>,
{
    use rustix::fs::{
        AtFlags, Mode, OFlags, RenameFlags, mkdirat, openat, renameat_with, statat, unlinkat,
    };
    let parent_path = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| eyre!("release circuit-parameter directory has no parent"))?;
    let target_name = path
        .file_name()
        .ok_or_else(|| eyre!("release circuit-parameter directory has no file name"))?;
    let mut components = Path::new(target_name).components();
    if !matches!(components.next(), Some(std::path::Component::Normal(name)) if name == target_name)
        || components.next().is_some()
    {
        bail!("release circuit-parameter directory name must be one normal path component");
    }
    let parent = PinnedPromotionParentV1::open(parent_path)?;
    match statat(&parent.file, target_name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(_) => bail!("refusing to overwrite or alias an existing circuit-parameter directory"),
        Err(error) if error == rustix::io::Errno::NOENT => {}
        Err(error) => {
            return Err(error).wrap_err("failed to inspect circuit-parameter destination");
        }
    }
    let temporary_name = random_promotion_temporary_name_v1(target_name)?;
    mkdirat(&parent.file, &temporary_name, Mode::from_raw_mode(0o700))
        .wrap_err("failed to create private circuit-parameter staging directory")?;
    let staging = match openat(
        &parent.file,
        &temporary_name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(file) => File::from(file),
        Err(error) => {
            unlinkat(&parent.file, &temporary_name, AtFlags::REMOVEDIR)
                .wrap_err("failed to remove unopened circuit-parameter staging directory")?;
            parent
                .file
                .sync_all()
                .wrap_err("failed to sync parent after removing unopened staging directory")?;
            return Err(error).wrap_err("failed to pin circuit-parameter staging directory");
        }
    };
    let staging_snapshot = (|| -> Result<PromotionDirectorySnapshotV1> {
        let snapshot = PromotionDirectorySnapshotV1::from_metadata(
            &staging
                .metadata()
                .wrap_err("failed to inspect circuit-parameter staging directory")?,
        )
        .ok_or_else(|| eyre!("circuit-parameter staging path is not a directory"))?;
        snapshot.validate_private("circuit-parameter staging directory")?;
        Ok(snapshot)
    })();
    let staging_snapshot = match staging_snapshot {
        Ok(snapshot) => snapshot,
        Err(error) => {
            cleanup_release_circuit_params_staging_v1(&parent, &staging, &temporary_name)?;
            return Err(error);
        }
    };
    let publication_parent_snapshot =
        match parent.snapshot_after_one_staged_entry("a circuit-parameter directory") {
            Ok(snapshot) => snapshot,
            Err(error) => {
                cleanup_release_circuit_params_staging_v1(&parent, &staging, &temporary_name)?;
                return Err(error);
            }
        };
    let prepare_result = (|| -> Result<(
        PromotionDirectorySnapshotV1,
        PromotionFileSnapshotV1,
        PromotionFileSnapshotV1,
    )> {
        let eq_snapshot = write_release_circuit_params_staged_file_v1(
            &staging,
            RELEASE_STEP_EQ_CIRCUIT_PARAMS_FILE_NAME_V4,
            bytes,
        )?;
        let ep_snapshot = write_release_circuit_params_staged_file_v1(
            &staging,
            RELEASE_STEP_EP_CIRCUIT_PARAMS_FILE_NAME_V4,
            bytes,
        )?;
        staging
            .sync_all()
            .wrap_err("failed to sync complete circuit-parameter staging directory")?;
        let complete_staging_snapshot = PromotionDirectorySnapshotV1::from_metadata(
            &staging
                .metadata()
                .wrap_err("failed to inspect complete circuit-parameter staging directory")?,
        )
        .ok_or_else(|| eyre!("complete circuit-parameter staging path is not a directory"))?;
        // APFS includes staged regular entries in the directory link count.
        // Admit exactly the two known Eq/Ep leaf transitions and no decrease.
        if !staging_snapshot.matches_except_links(complete_staging_snapshot)
            || complete_staging_snapshot
                .links
                .checked_sub(staging_snapshot.links)
                .is_none_or(|delta| delta > 2)
        {
            bail!("circuit-parameter staging directory changed while its leaves were staged");
        }
        let linked = statat(&parent.file, &temporary_name, AtFlags::SYMLINK_NOFOLLOW)
            .wrap_err("failed to inspect circuit-parameter staging binding")?;
        if !release_circuit_params_directory_snapshot_matches_stat_v1(
            complete_staging_snapshot,
            &linked,
        ) {
            bail!("circuit-parameter staging directory changed identity or custody");
        }
        before_publish()?;
        parent.verify_path_identity_against(publication_parent_snapshot)?;
        verify_release_circuit_params_directory_contents_v1(
            &parent.file,
            &staging,
            &temporary_name,
            complete_staging_snapshot,
            eq_snapshot,
            ep_snapshot,
            bytes,
            "staged circuit-parameter",
        )?;
        Ok((complete_staging_snapshot, eq_snapshot, ep_snapshot))
    })();
    let (complete_staging_snapshot, eq_snapshot, ep_snapshot) = match prepare_result {
        Ok(snapshots) => snapshots,
        Err(error) => {
            cleanup_release_circuit_params_staging_v1(&parent, &staging, &temporary_name)?;
            return Err(error);
        }
    };
    if let Err(error) = renameat_with(
        &parent.file,
        &temporary_name,
        &parent.file,
        target_name,
        RenameFlags::NOREPLACE,
    ) {
        let rename_error =
            eyre!(error).wrap_err("failed to atomically publish circuit-parameter directory");
        let rename_error_chain = format!("{rename_error:#}");
        let target_observation = inspect_no_replace_rename_name_v1(
            &parent.file,
            target_name,
            |stat| {
                promotion_directory_snapshot_has_stat_identity_v1(complete_staging_snapshot, stat)
            },
            "circuit-parameter destination after failed rename",
        );
        let staging_observation = inspect_no_replace_rename_name_v1(
            &parent.file,
            &temporary_name,
            |stat| {
                release_circuit_params_directory_snapshot_matches_stat_v1(
                    complete_staging_snapshot,
                    stat,
                )
            },
            "circuit-parameter staging binding after failed rename",
        );
        return match reconcile_failed_no_replace_rename_v1(
            &rename_error_chain,
            &target_observation,
            &staging_observation,
            || {
                parent.verify_path_identity_against(publication_parent_snapshot)?;
                verify_release_circuit_params_directory_contents_v1(
                    &parent.file,
                    &staging,
                    &temporary_name,
                    complete_staging_snapshot,
                    eq_snapshot,
                    ep_snapshot,
                    bytes,
                    "post-error staged circuit-parameter",
                )?;
                cleanup_release_circuit_params_staging_v1(&parent, &staging, &temporary_name)?;
                parent.verify_path_identity()
            },
        ) {
            FailedNoReplaceRenameReconciliationV1::PreCommit => Err(rename_error),
            FailedNoReplaceRenameReconciliationV1::CommitUncertain { reason } => {
                Ok(ReleaseCircuitParamsPublicationOutcomeV1::CommitUncertain {
                    final_path: parent.path.join(target_name),
                    reason,
                })
            }
        };
    }
    let final_path = parent.path.join(target_name);
    let after_publication = (|| -> Result<()> {
        verify_release_circuit_params_directory_contents_v1(
            &parent.file,
            &staging,
            target_name,
            complete_staging_snapshot,
            eq_snapshot,
            ep_snapshot,
            bytes,
            "published circuit-parameter",
        )?;
        sync_parent(&parent.file)
            .wrap_err("failed to durably sync circuit-parameter parent directory")?;
        parent.verify_path_identity_against(publication_parent_snapshot)?;
        verify_release_circuit_params_directory_contents_v1(
            &parent.file,
            &staging,
            target_name,
            complete_staging_snapshot,
            eq_snapshot,
            ep_snapshot,
            bytes,
            "durably published circuit-parameter",
        )?;
        Ok(())
    })();
    Ok(match after_publication {
        Ok(()) => ReleaseCircuitParamsPublicationOutcomeV1::Committed { final_path },
        Err(error) => ReleaseCircuitParamsPublicationOutcomeV1::CommitUncertain {
            final_path,
            reason: format!("{error:#}"),
        },
    })
}
#[cfg(unix)]
fn write_release_circuit_params_directory_v1(
    path: &Path,
    bytes: &[u8],
) -> Result<ReleaseCircuitParamsPublicationOutcomeV1> {
    write_release_circuit_params_directory_with_hooks_v1(path, bytes, || Ok(()), File::sync_all)
}
#[cfg(not(unix))]
fn write_release_circuit_params_directory_v1(
    _path: &Path,
    _bytes: &[u8],
) -> Result<ReleaseCircuitParamsPublicationOutcomeV1> {
    bail!("safe circuit-parameter directory publication requires Unix descriptor-relative APIs")
}
fn publish_release_circuit_params_directory_v4<W: Write>(
    writer: &mut W,
    path: &Path,
    bytes: &[u8],
) -> Result<()> {
    match write_release_circuit_params_directory_v1(path, bytes)? {
        committed @ ReleaseCircuitParamsPublicationOutcomeV1::Committed { .. } => {
            writeln!(writer, "{}", committed.operator_record())?;
            Ok(())
        }
        uncertain @ ReleaseCircuitParamsPublicationOutcomeV1::CommitUncertain { .. } => {
            Err(ExplicitExitError::new(
                DURABLE_FILE_COMMIT_UNCERTAIN_EXIT_CODE,
                uncertain.operator_record(),
            )
            .into())
        }
    }
}
#[cfg(unix)]
fn write_new_durable_file(path: &Path, bytes: &[u8]) -> Result<DurableFilePublicationOutcomeV1> {
    write_new_durable_file_with_hooks_v1(path, bytes, || Ok(()), File::sync_all)
}
#[cfg(not(unix))]
fn write_new_durable_file(_path: &Path, _bytes: &[u8]) -> Result<DurableFilePublicationOutcomeV1> {
    bail!("safe Kagemusha promotion-record publication requires Unix descriptor-relative APIs")
}
fn publish_new_durable_file<W: Write>(writer: &mut W, path: &Path, bytes: &[u8]) -> Result<()> {
    match write_new_durable_file(path, bytes)? {
        committed @ DurableFilePublicationOutcomeV1::Committed { .. } => {
            writeln!(writer, "{}", committed.operator_record())?;
            Ok(())
        }
        uncertain @ DurableFilePublicationOutcomeV1::CommitUncertain { .. } => {
            Err(ExplicitExitError::new(
                DURABLE_FILE_COMMIT_UNCERTAIN_EXIT_CODE,
                uncertain.operator_record(),
            )
            .into())
        }
    }
}
#[derive(Clone, Debug, crate::json_macros::JsonSerialize)]
struct VerificationArtifact {
    purpose: String,
    file_name: String,
    size_bytes: u64,
    sha256: String,
    payload_size_bytes: Option<u64>,
    payload_sha256: Option<String>,
}
#[derive(Debug, crate::json_macros::JsonSerialize)]
struct VerificationReport {
    status: String,
    envelope_sha256: String,
    manifest_body_sha256: String,
    qualification_receipt_sha256: String,
    qualified_candidate_sha256: String,
    internal_validation_receipt_sha256: String,
    release_policy_sha256: String,
    authenticated_source_seal_projection_sha256: String,
    reviewed_cargo_binary_sha256: String,
    reviewed_rustc_binary_sha256: String,
    generator_binary_sha256: String,
    sealed_candidate_build_report_sha256: String,
    generation: String,
    generation_memory_limit_bytes: u64,
    generation_memory_enforcement_profile: String,
    network_id: String,
    asset_definition_id: String,
    asset_scale: u32,
    bridge_abi_version: u32,
    recursive_step_verifier_commitment: String,
    artifacts: Vec<VerificationArtifact>,
}
impl VerificationReport {
    fn from_manifest_v4(
        manifest: &KagemushaRecursiveSpendArtifactManifestV4,
        envelope_sha256: [u8; 32],
        manifest_body_sha256: [u8; 32],
        release_policy_sha256: [u8; 32],
        recursive_step_verifier_commitment: [u8; 32],
    ) -> Self {
        let mut artifacts = manifest
            .profiles
            .iter()
            .flat_map(|profile| profile.artifacts.iter())
            .zip(REPORT_ARTIFACT_PURPOSES_V4)
            .map(|(artifact, purpose)| VerificationArtifact {
                purpose: purpose.to_owned(),
                file_name: artifact.file_name.clone(),
                size_bytes: artifact.size_bytes,
                sha256: hex::encode(artifact.sha256),
                payload_size_bytes: Some(artifact.payload_size_bytes),
                payload_sha256: Some(hex::encode(artifact.payload_sha256)),
            })
            .collect::<Vec<_>>();
        let roster = &manifest.topup_finality_roster_artifact;
        artifacts.push(VerificationArtifact {
            purpose: REPORT_ROSTER_PURPOSE.to_owned(),
            file_name: roster.file_name.clone(),
            size_bytes: roster.size_bytes,
            sha256: hex::encode(roster.sha256),
            payload_size_bytes: None,
            payload_sha256: None,
        });
        Self {
            status: "verified".to_owned(),
            envelope_sha256: hex::encode(envelope_sha256),
            manifest_body_sha256: hex::encode(manifest_body_sha256),
            qualification_receipt_sha256: hex::encode(manifest.qualification_receipt_sha256),
            qualified_candidate_sha256: hex::encode(manifest.qualified_candidate_sha256),
            internal_validation_receipt_sha256: hex::encode(
                manifest.internal_validation_receipt_sha256,
            ),
            release_policy_sha256: hex::encode(release_policy_sha256),
            authenticated_source_seal_projection_sha256: hex::encode(
                manifest.authenticated_source_seal_projection_sha256,
            ),
            reviewed_cargo_binary_sha256: hex::encode(manifest.reviewed_cargo_binary_sha256),
            reviewed_rustc_binary_sha256: hex::encode(manifest.reviewed_rustc_binary_sha256),
            generator_binary_sha256: hex::encode(manifest.generator_binary_sha256),
            sealed_candidate_build_report_sha256: hex::encode(
                manifest.sealed_candidate_build_report_sha256,
            ),
            generation: manifest.generation.clone(),
            generation_memory_limit_bytes: manifest.generation_memory_limit_bytes,
            generation_memory_enforcement_profile: manifest
                .generation_memory_enforcement_profile
                .clone(),
            network_id: manifest.network_id.to_string(),
            asset_definition_id: manifest.asset.to_string(),
            asset_scale: manifest.asset_scale,
            bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
            recursive_step_verifier_commitment: hex::encode(recursive_step_verifier_commitment),
            artifacts,
        }
    }
}
#[derive(Debug, crate::json_macros::JsonSerialize)]
struct VerificationReportV4 {
    status: String,
    envelope_sha256: String,
    manifest_body_sha256: String,
    candidate_sha256: String,
    qualification_receipt_sha256: String,
    qualified_candidate_sha256: String,
    internal_validation_receipt_sha256: String,
    promotion_record_sha256: String,
    release_policy_sha256: String,
    authenticated_source_seal_projection_sha256: String,
    reviewed_cargo_binary_sha256: String,
    reviewed_rustc_binary_sha256: String,
    generator_binary_sha256: String,
    sealed_candidate_build_report_sha256: String,
    generation: String,
    generation_memory_limit_bytes: u64,
    generation_memory_enforcement_profile: String,
    network_id: String,
    asset_definition_id: String,
    asset_scale: u32,
    bridge_abi_version: u32,
    recursive_step_verifier_commitment: String,
    artifacts: Vec<VerificationArtifact>,
}
impl VerificationReportV4 {
    fn from_report(
        report: &VerificationReport,
        candidate_sha256: [u8; 32],
        promotion_record_sha256: [u8; 32],
    ) -> Self {
        Self {
            status: report.status.clone(),
            envelope_sha256: report.envelope_sha256.clone(),
            manifest_body_sha256: report.manifest_body_sha256.clone(),
            candidate_sha256: hex::encode(candidate_sha256),
            qualification_receipt_sha256: report.qualification_receipt_sha256.clone(),
            qualified_candidate_sha256: report.qualified_candidate_sha256.clone(),
            internal_validation_receipt_sha256: report.internal_validation_receipt_sha256.clone(),
            promotion_record_sha256: hex::encode(promotion_record_sha256),
            release_policy_sha256: report.release_policy_sha256.clone(),
            authenticated_source_seal_projection_sha256: report
                .authenticated_source_seal_projection_sha256
                .clone(),
            reviewed_cargo_binary_sha256: report.reviewed_cargo_binary_sha256.clone(),
            reviewed_rustc_binary_sha256: report.reviewed_rustc_binary_sha256.clone(),
            generator_binary_sha256: report.generator_binary_sha256.clone(),
            sealed_candidate_build_report_sha256: report
                .sealed_candidate_build_report_sha256
                .clone(),
            generation: report.generation.clone(),
            generation_memory_limit_bytes: report.generation_memory_limit_bytes,
            generation_memory_enforcement_profile: report
                .generation_memory_enforcement_profile
                .clone(),
            network_id: report.network_id.clone(),
            asset_definition_id: report.asset_definition_id.clone(),
            asset_scale: report.asset_scale,
            bridge_abi_version: report.bridge_abi_version,
            recursive_step_verifier_commitment: report.recursive_step_verifier_commitment.clone(),
            artifacts: report.artifacts.clone(),
        }
    }
    fn canonical_json(&self) -> Result<String> {
        norito::json::to_json(self).wrap_err("failed to encode Kagemusha V4 verification JSON")
    }
}
#[cfg(test)]
mod tests {
    use super::{
        AUTHENTICATED_ARTIFACT_ROLES_V4,
        KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_FILE_NAME_V1,
        PROMOTION_RECORD_FILE_NAME_V4, PrepareReleaseCircuitParamsV4Args,
        RELEASE_STEP_EP_CIRCUIT_PARAMS_FILE_NAME_V4, RELEASE_STEP_EQ_CIRCUIT_PARAMS_FILE_NAME_V4,
        REPORT_ARTIFACT_PURPOSES_V4, REPORT_ROSTER_PURPOSE, ReleaseInventoryStateV4,
        configured_device_attestation_policy, insert_expected_release_file_v4,
        parse_manifest_sha256, parse_nonzero_canonical_u64, parse_promotion_id,
        prepare_release_circuit_params_v4, read_regular_bounded,
        require_new_promotion_record_path_v4, roster_release_generations_match_v4,
        validate_artifacts_sequentially, validate_device_attestation_policy_for_atomic_activation,
        verify_publish_verify_release_v4,
    };
    #[cfg(unix)]
    use super::{
        Args, Command, DURABLE_FILE_COMMIT_UNCERTAIN_EXIT_CODE, DurableFilePublicationOutcomeV1,
        FailedNoReplaceRenameDispositionV1, FailedNoReplaceRenameReconciliationV1,
        NoReplaceRenameNameStateV1, PrepareCancelReleaseV4Args, PrepareDeactivateIssuanceV4Args,
        PrepareEnableIssuanceV4Args, ReleaseCircuitParamsPublicationOutcomeV1,
        classify_failed_no_replace_rename_v1, reconcile_failed_no_replace_rename_v1,
        write_new_durable_file_with_hooks_v1, write_release_circuit_params_directory_with_hooks_v1,
    };
    #[cfg(unix)]
    use crate::RunArgs as _;
    use iroha_core::smartcontracts::isi::offline::isi::production_offline_device_attestation_policy_v1;
    #[cfg(unix)]
    use iroha_crypto::HashOf;
    #[cfg(unix)]
    use iroha_data_model::isi::{
        InstructionBox,
        offline::{CancelKagemushaRecursiveReleaseV4, DeactivateKagemushaRecursiveIssuanceV4},
    };
    #[cfg(unix)]
    use iroha_data_model::offline::{
        KAGEMUSHA_V4_RELEASE_CANCELLATION_SCHEMA_V1, KAGEMUSHA_V4_RELEASE_DEACTIVATION_SCHEMA_V1,
        KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1, KAGEMUSHA_V4_RELEASE_TRANSITION_MAX_BYTES_V1,
        KagemushaExactBytesDigestV1, KagemushaV4ReleaseCancellationV1,
        KagemushaV4ReleaseDeactivationV1, KagemushaV4ReleaseLifecycleReasonV1,
        kagemusha_recursive_spend_release_sha256,
    };
    use iroha_data_model::offline::{
        KagemushaStepCircuitParamsV4, OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_ANDROID_APPS_V1,
        OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_ANDROID_SIGNING_CERTIFICATES_V1,
        OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_APP_IDENTIFIER_BYTES_V1,
        OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_CANONICAL_BYTES_V1,
        OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_APPS_V1,
        OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_BUNDLE_VERSION_BYTES_V1,
        OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_BUNDLE_VERSIONS_V1,
        OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_VALIDATION_CATEGORIES_V1,
        OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_REVOKED_CERTIFICATES_V1,
        OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TEAM_ID_BYTES_V1,
        OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOT_DER_BYTES_V1,
        OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOTS_PER_PLATFORM_V1,
        OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOTS_V1, OfflineAndroidAppAttestationPolicy,
        OfflineDeviceAttestationPolicy, OfflineDeviceAttestationTrustedRoot,
        OfflineIosAppAttestationPolicy,
    };
    use std::{
        cell::{Cell, RefCell},
        collections::BTreeSet,
        fs,
        path::Path,
        rc::Rc,
    };
    const POLICY_EVALUATION_TIME_MS: u64 = 1_800_000_000_000;
    #[cfg(unix)]
    #[test]
    fn failed_no_replace_rename_is_precommit_only_with_unpublished_target_and_owned_staging() {
        use NoReplaceRenameNameStateV1::{Foreign, Missing, Owned};

        let observations = [None, Some(Missing), Some(Owned), Some(Foreign)];
        for target in observations {
            for staging in observations {
                let expected =
                    if matches!(target, Some(Missing | Foreign)) && staging == Some(Owned) {
                        FailedNoReplaceRenameDispositionV1::PreCommit
                    } else {
                        FailedNoReplaceRenameDispositionV1::CommitUncertain
                    };
                assert_eq!(
                    classify_failed_no_replace_rename_v1(target, staging),
                    expected,
                    "unexpected classification for target={target:?}, staging={staging:?}"
                );
            }
        }
    }
    #[cfg(unix)]
    #[test]
    fn failed_no_replace_rename_cleanup_uncertainty_retains_the_full_error_chain() {
        use NoReplaceRenameNameStateV1::{Missing, Owned};

        let target: super::Result<_> = Ok(Missing);
        let staging: super::Result<_> = Ok(Owned);
        let reconciliation = reconcile_failed_no_replace_rename_v1(
            "rename context: injected syscall failure",
            &target,
            &staging,
            || Err(color_eyre::eyre::eyre!("injected parent identity loss")),
        );
        match reconciliation {
            FailedNoReplaceRenameReconciliationV1::CommitUncertain { reason } => {
                assert!(reason.contains("rename context: injected syscall failure"));
                assert!(reason.contains("injected parent identity loss"));
            }
            FailedNoReplaceRenameReconciliationV1::PreCommit => {
                panic!("uncertain cleanup cannot remain pre-commit")
            }
        }
    }
    #[cfg(unix)]
    #[test]
    fn failed_no_replace_rename_commit_uncertainty_never_runs_staging_cleanup() {
        use NoReplaceRenameNameStateV1::{Missing, Owned};

        let target: super::Result<_> = Ok(Owned);
        let staging: super::Result<_> = Ok(Missing);
        let cleanup_called = Cell::new(false);
        let reconciliation = reconcile_failed_no_replace_rename_v1(
            "rename context: injected syscall failure",
            &target,
            &staging,
            || {
                cleanup_called.set(true);
                Ok(())
            },
        );
        assert!(matches!(
            reconciliation,
            FailedNoReplaceRenameReconciliationV1::CommitUncertain { .. }
        ));
        assert!(
            !cleanup_called.get(),
            "an owned destination or lost staging name must leave every name untouched"
        );
    }
    struct LivePayload {
        live: Rc<Cell<usize>>,
    }
    impl Drop for LivePayload {
        fn drop(&mut self) {
            self.live.set(self.live.get() - 1);
        }
    }
    fn valid_device_attestation_policy() -> OfflineDeviceAttestationPolicy {
        production_offline_device_attestation_policy_v1(
            "YLWWUD25VZ".to_owned(),
            "pk.retail.wallet.ios".to_owned(),
            vec![4],
            vec!["202605050324".to_owned()],
            "com.pk.retailwallet".to_owned(),
            vec![[0x11; 32]],
            iroha_data_model::offline::OfflineAndroidAttestationStatusSnapshotV1 {
                version: iroha_data_model::offline::OFFLINE_ANDROID_ATTESTATION_STATUS_SNAPSHOT_VERSION_V1,
                payload_sha256: [0x99; 32],
                response_date_ms: POLICY_EVALUATION_TIME_MS,
                last_modified_ms: Some(POLICY_EVALUATION_TIME_MS),
                cache_max_age_seconds: 86_400,
                non_valid_serials: Vec::new(),
            },
            POLICY_EVALUATION_TIME_MS,
        )
        .expect("built-in production attestation roots are valid")
    }
    fn bounded_trusted_root(
        platform: &str,
        discriminator: u8,
        der_len: usize,
    ) -> OfflineDeviceAttestationTrustedRoot {
        assert!(
            der_len >= 2,
            "test root needs a sequence marker and identity byte"
        );
        let mut der = vec![discriminator; der_len];
        der[0] = 0x30;
        der[1] = discriminator;
        OfflineDeviceAttestationTrustedRoot {
            platform: platform.to_owned(),
            der,
            not_before_ms: None,
            not_after_ms: None,
        }
    }
    fn bounded_ios_app(index: usize) -> OfflineIosAppAttestationPolicy {
        OfflineIosAppAttestationPolicy {
            team_id: format!("TEAM{index:04}"),
            bundle_id: format!("com.example.ios{index:04}"),
            environment: "production".to_owned(),
            allowed_validation_categories: vec![4],
            allowed_bundle_versions: vec!["1".to_owned()],
        }
    }
    fn bounded_android_app(index: usize) -> OfflineAndroidAppAttestationPolicy {
        OfflineAndroidAppAttestationPolicy {
            package_name: format!("com.example.android{index:04}"),
            signing_certificate_sha256: vec![vec![u8::try_from(index + 1).unwrap(); 32]],
        }
    }
    fn assert_atomic_policy_shape_valid(policy: &OfflineDeviceAttestationPolicy, context: &str) {
        super::validate_atomic_activation_policy_shape(policy)
            .unwrap_or_else(|error| panic!("{context}: exact-bound policy failed: {error}"));
    }
    fn assert_atomic_policy_cap_rejected(
        policy: &OfflineDeviceAttestationPolicy,
        expected: &str,
        context: &str,
    ) {
        let error = super::validate_atomic_activation_policy_shape(policy)
            .expect_err("over-bound policy must fail");
        assert!(
            error.to_string().contains(expected),
            "{context}: unexpected over-bound rejection: {error}"
        );
    }
    fn policy_at_exact_canonical_byte_limit() -> OfflineDeviceAttestationPolicy {
        let maximum_der = OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOT_DER_BYTES_V1;
        let mut policy = valid_device_attestation_policy();
        policy.trusted_roots = vec![
            bounded_trusted_root("android-keymint", 1, maximum_der),
            bounded_trusted_root("android-keymint", 2, maximum_der),
            bounded_trusted_root("ios-appattest", 3, maximum_der),
            bounded_trusted_root("ios-appattest", 4, 2),
        ];
        let initial_len = norito::encode_canonical(&policy)
            .expect("encode canonical-size policy baseline")
            .len();
        let estimated_der_len = 2 + OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_CANONICAL_BYTES_V1
            .checked_sub(initial_len)
            .expect("three maximum roots leave room for the fourth root");
        let lower = estimated_der_len.saturating_sub(64).max(2);
        let upper = estimated_der_len
            .saturating_add(64)
            .min(OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOT_DER_BYTES_V1);
        for der_len in lower..=upper {
            policy.trusted_roots[3] = bounded_trusted_root("ios-appattest", 4, der_len);
            if norito::encode_canonical(&policy)
                .expect("encode canonical-size policy candidate")
                .len()
                == OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_CANONICAL_BYTES_V1
            {
                return policy;
            }
        }
        panic!("test could not construct a policy at the exact canonical byte limit");
    }
    #[test]
    fn activation_manifest_digest_parser_is_lowercase_and_exact() {
        assert_eq!(parse_manifest_sha256(&"ab".repeat(32)), Ok([0xab; 32]));
        assert!(parse_manifest_sha256(&"AB".repeat(32)).is_err());
        assert!(parse_manifest_sha256(&"a".repeat(63)).is_err());
        assert!(parse_manifest_sha256(&format!("{}g", "a".repeat(63))).is_err());
    }
    #[test]
    fn activation_promotion_id_parser_is_nonzero_lowercase_and_exact() {
        assert_eq!(parse_promotion_id(&"ab".repeat(32)), Ok([0xab; 32]));
        assert!(parse_promotion_id(&"00".repeat(32)).is_err());
        assert!(parse_promotion_id(&"AB".repeat(32)).is_err());
    }
    #[test]
    fn operator_memory_lowering_parser_is_nonzero_and_canonical() {
        assert_eq!(parse_nonzero_canonical_u64("1"), Ok(1));
        assert_eq!(
            parse_nonzero_canonical_u64("68719476736"),
            Ok(68_719_476_736)
        );
        for invalid in ["", "0", "01", "+1", "-1", "1 ", "18446744073709551616"] {
            assert!(parse_nonzero_canonical_u64(invalid).is_err(), "{invalid:?}");
        }
    }
    #[test]
    fn bounded_artifact_reader_accepts_exact_limit_and_rejects_overflow() {
        let root = tempfile::tempdir().expect("temporary bounded-artifact root");
        fs::write(root.path().join("exact.bin"), [0x5A; 32]).expect("write exact bounded artifact");
        assert_eq!(
            read_regular_bounded(root.path(), "exact.bin", 32, "test artifact")
                .expect("read exact bounded artifact"),
            vec![0x5A; 32]
        );
        fs::write(root.path().join("oversized.bin"), [0xA5; 33])
            .expect("write oversized bounded artifact");
        let error = read_regular_bounded(root.path(), "oversized.bin", 32, "test artifact")
            .expect_err("oversized bounded artifact must fail closed");
        assert!(error.to_string().contains("size bound"));
    }
    #[cfg(unix)]
    fn lifecycle_cancellation_fixture() -> KagemushaV4ReleaseCancellationV1 {
        KagemushaV4ReleaseCancellationV1 {
            schema: KAGEMUSHA_V4_RELEASE_CANCELLATION_SCHEMA_V1.to_owned(),
            version: KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1,
            promotion_id: [1; 32],
            manifest_sha256: [2; 32],
            expected_predecessor_lifecycle: KagemushaExactBytesDigestV1 {
                byte_len: 1,
                sha256: [3; 32],
            },
            transition_id: [4; 32],
            reason: KagemushaV4ReleaseLifecycleReasonV1::GovernanceCancelled,
            evidence: None,
        }
    }
    #[cfg(unix)]
    fn lifecycle_deactivation_fixture() -> KagemushaV4ReleaseDeactivationV1 {
        KagemushaV4ReleaseDeactivationV1 {
            schema: KAGEMUSHA_V4_RELEASE_DEACTIVATION_SCHEMA_V1.to_owned(),
            version: KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1,
            promotion_id: [5; 32],
            manifest_sha256: [6; 32],
            expected_predecessor_lifecycle: KagemushaExactBytesDigestV1 {
                byte_len: 1,
                sha256: [7; 32],
            },
            transition_id: [8; 32],
            reason: KagemushaV4ReleaseLifecycleReasonV1::EmergencyDeactivation,
            evidence: None,
        }
    }
    #[cfg(unix)]
    fn write_private_test_file(path: &Path, bytes: &[u8]) {
        use std::os::unix::fs::PermissionsExt as _;

        fs::write(path, bytes).expect("write lifecycle test input");
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .expect("secure lifecycle test input");
    }
    #[cfg(unix)]
    fn run_lifecycle_command(command: Command) -> (crate::Outcome, String) {
        iroha_genesis::init_instruction_registry();
        let mut writer = std::io::BufWriter::new(Vec::new());
        let outcome = Args { command }.run(&mut writer);
        let report = String::from_utf8(writer.into_inner().expect("flush lifecycle report"))
            .expect("lifecycle report is UTF-8");
        (outcome, report)
    }
    #[cfg(unix)]
    fn assert_prepared_lifecycle_command(
        command: Command,
        output: &Path,
        canonical_input: &[u8],
        expected: &[InstructionBox],
        operation: &str,
    ) {
        let expected_input_sha256 = kagemusha_recursive_spend_release_sha256(canonical_input);
        let (outcome, report) = run_lifecycle_command(command);
        outcome.expect("prepare lifecycle instruction");
        let decoded: Vec<InstructionBox> = norito::json::from_slice(
            &fs::read(output).expect("read prepared lifecycle instruction"),
        )
        .expect("decode prepared lifecycle instruction");
        assert_eq!(decoded.as_slice(), expected);
        let expected_instructions_hash = HashOf::new(&decoded);
        let report_lines = report.lines().collect::<Vec<_>>();
        assert_eq!(
            report_lines.len(),
            2,
            "durability record and preparation report"
        );
        assert!(report_lines[0].starts_with("iroha.kagami.kagemusha.durable_file_publication.v1 "));
        assert_eq!(
            report_lines[1],
            format!(
                "{{\"status\":\"prepared\",\"operation\":\"{operation}\",\"instruction_count\":1,\"instructions_hash\":\"{expected_instructions_hash}\",\"input_sha256\":\"{}\"}}",
                hex::encode(expected_input_sha256),
            )
        );
    }
    #[cfg(unix)]
    #[test]
    fn lifecycle_terminal_commands_publish_exact_typed_instructions_and_reports() {
        use std::os::unix::fs::PermissionsExt as _;

        let root = tempfile::tempdir().expect("temporary lifecycle preparation root");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .expect("secure lifecycle preparation root");

        let cancellation = lifecycle_cancellation_fixture();
        cancellation.validate().expect("valid cancellation fixture");
        let cancellation_bytes =
            norito::encode_canonical(&cancellation).expect("encode cancellation fixture");
        let cancellation_input = root.path().join("cancellation.norito");
        let cancellation_output = root.path().join("cancel-instruction.json");
        write_private_test_file(&cancellation_input, &cancellation_bytes);
        let expected_cancellation = vec![InstructionBox::from(
            CancelKagemushaRecursiveReleaseV4::new(cancellation),
        )];
        assert_prepared_lifecycle_command(
            Command::PrepareCancelReleaseV4(PrepareCancelReleaseV4Args {
                cancellation: cancellation_input,
                output: cancellation_output.clone(),
            }),
            &cancellation_output,
            &cancellation_bytes,
            &expected_cancellation,
            "cancel-release-v4",
        );

        let deactivation = lifecycle_deactivation_fixture();
        deactivation.validate().expect("valid deactivation fixture");
        let deactivation_bytes =
            norito::encode_canonical(&deactivation).expect("encode deactivation fixture");
        let deactivation_input = root.path().join("deactivation.norito");
        let deactivation_output = root.path().join("deactivate-instruction.json");
        write_private_test_file(&deactivation_input, &deactivation_bytes);
        let expected_deactivation = vec![InstructionBox::from(
            DeactivateKagemushaRecursiveIssuanceV4::new(deactivation),
        )];
        assert_prepared_lifecycle_command(
            Command::PrepareDeactivateIssuanceV4(PrepareDeactivateIssuanceV4Args {
                deactivation: deactivation_input,
                output: deactivation_output.clone(),
            }),
            &deactivation_output,
            &deactivation_bytes,
            &expected_deactivation,
            "deactivate-issuance-v4",
        );
    }
    #[cfg(unix)]
    #[test]
    fn lifecycle_commands_reject_tampered_noncanonical_oversized_and_malformed_inputs() {
        use std::os::unix::fs::PermissionsExt as _;

        let root = tempfile::tempdir().expect("temporary lifecycle rejection root");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .expect("secure lifecycle rejection root");

        let mut tampered = norito::encode_canonical(&lifecycle_cancellation_fixture())
            .expect("encode cancellation fixture");
        let schema = KAGEMUSHA_V4_RELEASE_CANCELLATION_SCHEMA_V1.as_bytes();
        let schema_offset = tampered
            .windows(schema.len())
            .position(|window| window == schema)
            .expect("canonical input contains its schema");
        tampered[schema_offset] ^= 1;
        let tampered_input = root.path().join("tampered-cancellation.norito");
        let tampered_output = root.path().join("tampered-output.json");
        write_private_test_file(&tampered_input, &tampered);
        let (outcome, report) = run_lifecycle_command(Command::PrepareCancelReleaseV4(
            PrepareCancelReleaseV4Args {
                cancellation: tampered_input,
                output: tampered_output.clone(),
            },
        ));
        assert!(outcome.is_err());
        assert!(report.is_empty());
        assert!(!tampered_output.exists());

        let mut noncanonical = norito::encode_canonical(&lifecycle_deactivation_fixture())
            .expect("encode deactivation fixture");
        noncanonical.push(0);
        let noncanonical_input = root.path().join("noncanonical-deactivation.norito");
        let noncanonical_output = root.path().join("noncanonical-output.json");
        write_private_test_file(&noncanonical_input, &noncanonical);
        let (outcome, report) = run_lifecycle_command(Command::PrepareDeactivateIssuanceV4(
            PrepareDeactivateIssuanceV4Args {
                deactivation: noncanonical_input,
                output: noncanonical_output.clone(),
            },
        ));
        assert!(outcome.is_err());
        assert!(report.is_empty());
        assert!(!noncanonical_output.exists());

        let oversized_input = root.path().join("oversized-cancellation.norito");
        let oversized_output = root.path().join("oversized-output.json");
        let oversized = vec![0; KAGEMUSHA_V4_RELEASE_TRANSITION_MAX_BYTES_V1 + 1];
        write_private_test_file(&oversized_input, &oversized);
        let (outcome, report) = run_lifecycle_command(Command::PrepareCancelReleaseV4(
            PrepareCancelReleaseV4Args {
                cancellation: oversized_input,
                output: oversized_output.clone(),
            },
        ));
        assert!(outcome.is_err());
        assert!(report.is_empty());
        assert!(!oversized_output.exists());

        let malformed_enable_input = root.path().join("malformed-enable-witness.norito");
        let malformed_enable_output = root.path().join("malformed-enable-output.json");
        write_private_test_file(&malformed_enable_input, b"not canonical Norito");
        let (outcome, report) = run_lifecycle_command(Command::PrepareEnableIssuanceV4(
            PrepareEnableIssuanceV4Args {
                enable_witness: malformed_enable_input,
                output: malformed_enable_output.clone(),
            },
        ));
        assert!(outcome.is_err());
        assert!(report.is_empty());
        assert!(!malformed_enable_output.exists());
    }
    #[cfg(unix)]
    #[test]
    fn lifecycle_command_refuses_to_replace_existing_output() {
        use std::os::unix::fs::PermissionsExt as _;

        let root = tempfile::tempdir().expect("temporary lifecycle no-replace root");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .expect("secure lifecycle no-replace root");
        let cancellation_input = root.path().join("cancellation.norito");
        write_private_test_file(
            &cancellation_input,
            &norito::encode_canonical(&lifecycle_cancellation_fixture())
                .expect("encode cancellation fixture"),
        );
        let output = root.path().join("existing-output.json");
        write_private_test_file(&output, b"operator-reviewed sentinel");

        let (outcome, report) = run_lifecycle_command(Command::PrepareCancelReleaseV4(
            PrepareCancelReleaseV4Args {
                cancellation: cancellation_input,
                output: output.clone(),
            },
        ));
        let error = outcome.expect_err("existing lifecycle output must never be replaced");
        assert!(error.to_string().contains("refusing to overwrite"));
        assert!(report.is_empty());
        assert_eq!(
            fs::read(&output).expect("read existing lifecycle output"),
            b"operator-reviewed sentinel"
        );
    }
    #[cfg(unix)]
    #[test]
    fn release_circuit_params_command_publishes_one_canonical_atomic_directory() {
        use std::{fs, os::unix::fs::PermissionsExt as _};
        let root = tempfile::tempdir().expect("temporary circuit-parameter root");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .expect("secure circuit-parameter root");
        let parent = root.path().join("release-inputs");
        fs::create_dir(&parent).expect("create circuit-parameter parent");
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700))
            .expect("secure circuit-parameter parent");
        let output_dir = parent.join("circuit-params-v4");
        let mut writer = std::io::BufWriter::new(Vec::new());
        prepare_release_circuit_params_v4(
            &PrepareReleaseCircuitParamsV4Args {
                output_dir: output_dir.clone(),
            },
            &mut writer,
        )
        .expect("publish canonical release circuit parameters");
        let report =
            String::from_utf8(writer.into_inner().expect("flush report")).expect("report is UTF-8");
        let expected = KagemushaStepCircuitParamsV4::reviewed_first_release_generation_profile()
            .expect("reviewed first-release profile");
        let expected_bytes = norito::to_bytes(&expected).expect("encode reviewed profile");
        let mut entries = fs::read_dir(&output_dir)
            .expect("read published circuit-parameter directory")
            .map(|entry| {
                entry
                    .expect("inspect circuit-parameter entry")
                    .file_name()
                    .into_string()
                    .expect("canonical file name")
            })
            .collect::<Vec<_>>();
        entries.sort();
        assert_eq!(
            entries,
            [
                RELEASE_STEP_EP_CIRCUIT_PARAMS_FILE_NAME_V4.to_owned(),
                RELEASE_STEP_EQ_CIRCUIT_PARAMS_FILE_NAME_V4.to_owned(),
            ]
        );
        assert_eq!(
            fs::metadata(&output_dir)
                .expect("published directory metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        for file_name in [
            RELEASE_STEP_EQ_CIRCUIT_PARAMS_FILE_NAME_V4,
            RELEASE_STEP_EP_CIRCUIT_PARAMS_FILE_NAME_V4,
        ] {
            let path = output_dir.join(file_name);
            let bytes = fs::read(&path).expect("read published circuit parameters");
            assert_eq!(bytes, expected_bytes);
            assert_eq!(
                fs::metadata(&path)
                    .expect("published file metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
            let decoded: KagemushaStepCircuitParamsV4 =
                norito::decode_from_bytes(&bytes).expect("decode published circuit parameters");
            assert_eq!(decoded, expected);
            assert_eq!(
                norito::to_bytes(&decoded).expect("re-encode profile"),
                bytes
            );
        }
        assert!(report.contains("status=committed"));
        assert!(report.contains(&hex::encode(
            super::kagemusha_recursive_spend_release_sha256(&expected_bytes)
        )));
        assert!(report.contains(&hex::encode(
            expected.sha256().expect("reviewed profile identity")
        )));
        let mut retry_report = std::io::BufWriter::new(Vec::new());
        let error = prepare_release_circuit_params_v4(
            &PrepareReleaseCircuitParamsV4Args { output_dir },
            &mut retry_report,
        )
        .expect_err("closed publication must refuse to overwrite the complete directory");
        assert!(
            error
                .to_string()
                .contains("refusing to overwrite or alias an existing circuit-parameter directory"),
            "unexpected retry error: {error:#}"
        );
    }
    #[cfg(unix)]
    #[test]
    fn release_circuit_params_publication_tracks_its_parent_link_change() {
        use std::{
            fs,
            os::unix::fs::{MetadataExt as _, PermissionsExt as _},
        };
        let root = tempfile::tempdir().expect("temporary circuit-parameter root");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .expect("secure circuit-parameter root");
        let parent = root.path().join("release-inputs");
        fs::create_dir(&parent).expect("create circuit-parameter parent");
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700))
            .expect("secure circuit-parameter parent");
        let links_before = fs::metadata(&parent)
            .expect("inspect circuit-parameter parent")
            .nlink();
        let output_dir = parent.join("circuit-params-v4");
        let bytes = b"canonical circuit parameters";

        let outcome = write_release_circuit_params_directory_with_hooks_v1(
            &output_dir,
            bytes,
            || Ok(()),
            std::fs::File::sync_all,
        )
        .expect("the publisher's staging directory is an authorized parent-link transition");

        match outcome {
            ReleaseCircuitParamsPublicationOutcomeV1::Committed { final_path } => {
                assert_eq!(final_path, output_dir);
            }
            ReleaseCircuitParamsPublicationOutcomeV1::CommitUncertain { reason, .. } => {
                panic!("unchanged parent must commit durably: {reason}");
            }
        }
        let links_after = fs::metadata(&parent)
            .expect("re-inspect circuit-parameter parent")
            .nlink();
        assert!(
            links_after
                .checked_sub(links_before)
                .is_some_and(|delta| delta <= 1)
        );
        for file_name in [
            RELEASE_STEP_EQ_CIRCUIT_PARAMS_FILE_NAME_V4,
            RELEASE_STEP_EP_CIRCUIT_PARAMS_FILE_NAME_V4,
        ] {
            assert_eq!(
                fs::read(output_dir.join(file_name)).expect("read published circuit parameters"),
                bytes
            );
        }
    }
    #[cfg(unix)]
    #[test]
    fn release_circuit_params_publication_rejects_parent_substitution_before_visibility() {
        use std::{fs, os::unix::fs::PermissionsExt as _};
        let root = tempfile::tempdir().expect("temporary circuit-parameter root");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .expect("secure circuit-parameter root");
        let parent = root.path().join("release-inputs");
        let displaced = root.path().join("displaced-release-inputs");
        fs::create_dir(&parent).expect("create circuit-parameter parent");
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700))
            .expect("secure circuit-parameter parent");
        let output_dir = parent.join("circuit-params-v4");
        let error = write_release_circuit_params_directory_with_hooks_v1(
            &output_dir,
            b"canonical circuit parameters",
            || {
                fs::rename(&parent, &displaced).expect("move pinned parent");
                fs::create_dir(&parent).expect("create same-name impostor parent");
                fs::set_permissions(&parent, fs::Permissions::from_mode(0o700))
                    .expect("secure impostor parent");
                fs::write(parent.join("attacker-sentinel"), b"must survive")
                    .expect("write attacker sentinel");
                Ok(())
            },
            std::fs::File::sync_all,
        )
        .expect_err("a swapped parent must fail before pair visibility");
        let message = error.to_string();
        assert!(
            message.contains("parent") || message.contains("ancestor"),
            "unexpected parent-substitution error: {error:#}"
        );
        assert!(!output_dir.exists());
        assert!(!displaced.join("circuit-params-v4").exists());
        assert_eq!(
            fs::read(parent.join("attacker-sentinel")).expect("read attacker sentinel"),
            b"must survive"
        );
        assert_eq!(
            fs::read_dir(&displaced)
                .expect("read cleaned pinned parent")
                .count(),
            0,
            "the unpublished private staging directory must be removed"
        );
    }
    #[cfg(unix)]
    #[test]
    fn release_circuit_params_publication_rejects_staged_leaf_substitution() {
        use std::{fs, os::unix::fs::PermissionsExt as _, os::unix::fs::symlink};
        let root = tempfile::tempdir().expect("temporary circuit-parameter root");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .expect("secure circuit-parameter root");
        let parent = root.path().join("release-inputs");
        fs::create_dir(&parent).expect("create circuit-parameter parent");
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700))
            .expect("secure circuit-parameter parent");
        let output_dir = parent.join("circuit-params-v4");
        let error = write_release_circuit_params_directory_with_hooks_v1(
            &output_dir,
            b"canonical circuit parameters",
            || {
                let staging = fs::read_dir(&parent)
                    .expect("read circuit-parameter parent")
                    .next()
                    .expect("staging directory entry")
                    .expect("read staging directory entry")
                    .path();
                fs::remove_file(staging.join(RELEASE_STEP_EQ_CIRCUIT_PARAMS_FILE_NAME_V4))
                    .expect("remove authentic staged Eq leaf");
                symlink(
                    RELEASE_STEP_EP_CIRCUIT_PARAMS_FILE_NAME_V4,
                    staging.join(RELEASE_STEP_EQ_CIRCUIT_PARAMS_FILE_NAME_V4),
                )
                .expect("substitute staged Eq leaf with a symlink");
                Ok(())
            },
            std::fs::File::sync_all,
        )
        .expect_err("a substituted staged leaf must fail before publication");
        assert!(
            error.to_string().contains("Eq circuit parameters"),
            "unexpected substitution error: {error:#}"
        );
        assert!(!output_dir.exists());
        assert_eq!(
            fs::read_dir(&parent)
                .expect("read cleaned circuit-parameter parent")
                .count(),
            0
        );
    }
    #[cfg(unix)]
    #[test]
    fn failed_no_replace_rename_for_release_circuit_params_preserves_foreign_target() {
        use std::{fs, os::unix::fs::PermissionsExt as _};
        let root = tempfile::tempdir().expect("temporary circuit-parameter root");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .expect("secure circuit-parameter root");
        let parent = root.path().join("release-inputs");
        fs::create_dir(&parent).expect("create circuit-parameter parent");
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700))
            .expect("secure circuit-parameter parent");
        let output_dir = parent.join("circuit-params-v4");
        let error = write_release_circuit_params_directory_with_hooks_v1(
            &output_dir,
            b"canonical circuit parameters",
            || {
                fs::write(&output_dir, b"foreign destination")
                    .expect("inject no-replace destination race");
                Ok(())
            },
            std::fs::File::sync_all,
        )
        .expect_err("a foreign destination must defeat the directory no-replace rename");
        assert!(
            format!("{error:#}")
                .contains("failed to atomically publish circuit-parameter directory"),
            "the original rename error chain must be retained: {error:#}"
        );
        assert_eq!(
            fs::read(&output_dir).expect("read preserved foreign destination"),
            b"foreign destination"
        );
        assert_eq!(
            fs::read_dir(&parent)
                .expect("inspect reconciled circuit-parameter parent")
                .count(),
            1,
            "only the foreign destination may remain after exact staging cleanup"
        );
    }
    #[cfg(unix)]
    #[test]
    fn release_circuit_params_parent_sync_failure_is_complete_but_commit_uncertain() {
        use std::{fs, io, os::unix::fs::PermissionsExt as _};
        let root = tempfile::tempdir().expect("temporary circuit-parameter root");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .expect("secure circuit-parameter root");
        let parent = root.path().join("release-inputs");
        fs::create_dir(&parent).expect("create circuit-parameter parent");
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700))
            .expect("secure circuit-parameter parent");
        let output_dir = parent.join("circuit-params-v4");
        let bytes = b"canonical circuit parameters";
        let outcome = write_release_circuit_params_directory_with_hooks_v1(
            &output_dir,
            bytes,
            || Ok(()),
            |_| Err(io::Error::other("injected parent sync failure")),
        )
        .expect("post-rename sync failure is an explicit outcome");
        match &outcome {
            ReleaseCircuitParamsPublicationOutcomeV1::CommitUncertain { final_path, reason } => {
                assert_eq!(final_path, &output_dir);
                assert!(reason.contains("injected parent sync failure"));
            }
            ReleaseCircuitParamsPublicationOutcomeV1::Committed { .. } => {
                panic!("failed parent sync cannot be committed")
            }
        }
        for file_name in [
            RELEASE_STEP_EQ_CIRCUIT_PARAMS_FILE_NAME_V4,
            RELEASE_STEP_EP_CIRCUIT_PARAMS_FILE_NAME_V4,
        ] {
            assert_eq!(
                fs::read(output_dir.join(file_name)).expect("read complete visible pair"),
                bytes
            );
        }
        assert!(
            outcome
                .operator_record()
                .contains("status=commit-uncertain")
        );
    }
    #[cfg(unix)]
    #[test]
    fn release_circuit_params_post_rename_mutation_is_commit_uncertain() {
        use std::{fs, os::unix::fs::PermissionsExt as _};
        let root = tempfile::tempdir().expect("temporary circuit-parameter root");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .expect("secure circuit-parameter root");
        let parent = root.path().join("release-inputs");
        fs::create_dir(&parent).expect("create circuit-parameter parent");
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700))
            .expect("secure circuit-parameter parent");
        let output_dir = parent.join("circuit-params-v4");
        let bytes = b"canonical circuit parameters";
        let outcome = write_release_circuit_params_directory_with_hooks_v1(
            &output_dir,
            bytes,
            || Ok(()),
            |parent_file| {
                fs::write(
                    output_dir.join(RELEASE_STEP_EQ_CIRCUIT_PARAMS_FILE_NAME_V4),
                    vec![b'X'; bytes.len()],
                )?;
                parent_file.sync_all()
            },
        )
        .expect("post-rename mutation has an explicit publication outcome");
        match outcome {
            ReleaseCircuitParamsPublicationOutcomeV1::CommitUncertain { final_path, reason } => {
                assert_eq!(final_path, output_dir);
                assert!(
                    reason.contains("content does not match"),
                    "unexpected mutation reason: {reason}"
                );
            }
            ReleaseCircuitParamsPublicationOutcomeV1::Committed { .. } => {
                panic!("mutated published circuit parameters cannot be committed")
            }
        }
    }
    #[cfg(unix)]
    #[test]
    fn promotion_publication_accepts_unrelated_ancestor_link_transition() {
        use std::{fs, os::unix::fs::PermissionsExt as _};

        let root = tempfile::tempdir().expect("temporary publication root");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .expect("secure temporary root");
        let ancestor = root.path().join("ancestor");
        let parent = ancestor.join("promotion");
        fs::create_dir(&ancestor).expect("create publication ancestor");
        fs::create_dir(&parent).expect("create promotion parent");
        fs::set_permissions(&ancestor, fs::Permissions::from_mode(0o700))
            .expect("secure publication ancestor");
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700))
            .expect("secure promotion parent");
        let target = parent.join("promotion-record-v4.norito");
        let bytes = b"authenticated promotion record";
        let unrelated = ancestor.join("unrelated-directory");

        let outcome = write_new_durable_file_with_hooks_v1(
            &target,
            bytes,
            || {
                fs::create_dir(&unrelated).expect("create unrelated ancestor entry");
                Ok(())
            },
            std::fs::File::sync_all,
        )
        .expect("an unrelated ancestor link transition must preserve the pinned path identity");

        assert!(matches!(
            outcome,
            DurableFilePublicationOutcomeV1::Committed { ref final_path }
                if final_path == &target
        ));
        assert_eq!(fs::read(target).expect("read published record"), bytes);
    }
    #[cfg(unix)]
    #[test]
    fn promotion_publication_rejects_same_length_staged_content_mutation() {
        use std::{fs, os::unix::fs::PermissionsExt as _};
        let root = tempfile::tempdir().expect("temporary publication root");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .expect("secure temporary root");
        let parent = root.path().join("promotion");
        fs::create_dir(&parent).expect("create promotion parent");
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700))
            .expect("secure promotion parent");
        let target = parent.join("promotion-record-v4.norito");
        let bytes = b"authenticated promotion record";
        let error = write_new_durable_file_with_hooks_v1(
            &target,
            bytes,
            || {
                let temporary = fs::read_dir(&parent)
                    .expect("read promotion parent")
                    .next()
                    .expect("promotion temporary entry")
                    .expect("read promotion temporary entry")
                    .path();
                fs::write(temporary, vec![b'X'; bytes.len()])
                    .expect("mutate the staged record without changing its length");
                Ok(())
            },
            std::fs::File::sync_all,
        )
        .expect_err("mutated staged content must fail before publication");
        assert!(
            error.to_string().contains("content does not match"),
            "unexpected mutation error: {error:#}"
        );
        assert!(!target.exists());
        assert_eq!(
            fs::read_dir(&parent)
                .expect("read cleaned promotion parent")
                .count(),
            0
        );
    }
    #[cfg(unix)]
    #[test]
    fn failed_no_replace_rename_for_promotion_preserves_foreign_target() {
        use std::{fs, os::unix::fs::PermissionsExt as _};
        let root = tempfile::tempdir().expect("temporary publication root");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .expect("secure temporary root");
        let parent = root.path().join("promotion");
        fs::create_dir(&parent).expect("create promotion parent");
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700))
            .expect("secure promotion parent");
        let target = parent.join("promotion-record-v4.norito");
        let error = write_new_durable_file_with_hooks_v1(
            &target,
            b"authenticated promotion record",
            || {
                fs::write(&target, b"foreign destination")
                    .expect("inject no-replace destination race");
                Ok(())
            },
            std::fs::File::sync_all,
        )
        .expect_err("a foreign destination must defeat the file no-replace rename");
        assert!(
            format!("{error:#}").contains("failed to atomically publish new promotion record"),
            "the original rename error chain must be retained: {error:#}"
        );
        assert_eq!(
            fs::read(&target).expect("read preserved foreign destination"),
            b"foreign destination"
        );
        assert_eq!(
            fs::read_dir(&parent)
                .expect("inspect reconciled promotion parent")
                .count(),
            1,
            "only the foreign destination may remain after exact staging cleanup"
        );
    }
    #[cfg(unix)]
    #[test]
    fn promotion_publication_rejects_a_parent_swap_before_visibility() {
        use std::{fs, os::unix::fs::PermissionsExt as _};
        let root = tempfile::tempdir().expect("temporary publication root");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .expect("secure temporary root");
        let parent = root.path().join("promotion");
        let displaced = root.path().join("displaced-promotion");
        fs::create_dir(&parent).expect("create promotion parent");
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700))
            .expect("secure promotion parent");
        let target = parent.join("promotion-record-v4.norito");
        let error = write_new_durable_file_with_hooks_v1(
            &target,
            b"authenticated promotion record",
            || {
                fs::rename(&parent, &displaced).expect("move pinned parent");
                fs::create_dir(&parent).expect("create same-name impostor parent");
                fs::set_permissions(&parent, fs::Permissions::from_mode(0o700))
                    .expect("secure impostor parent");
                fs::write(parent.join("attacker-sentinel"), b"must survive")
                    .expect("write attacker sentinel");
                Ok(())
            },
            std::fs::File::sync_all,
        )
        .expect_err("a swapped parent must fail before publication");
        let message = error.to_string();
        assert!(
            message.contains("parent") || message.contains("ancestor"),
            "unexpected parent-substitution error: {error:#}"
        );
        assert!(!target.exists(), "the impostor path must receive no record");
        assert!(
            !displaced.join("promotion-record-v4.norito").exists(),
            "the pinned directory must receive no final record after path continuity fails"
        );
        assert_eq!(
            fs::read(parent.join("attacker-sentinel")).expect("read attacker sentinel"),
            b"must survive"
        );
    }
    #[cfg(unix)]
    #[test]
    fn promotion_parent_sync_failure_is_explicitly_commit_uncertain() {
        use std::{fs, io, os::unix::fs::PermissionsExt as _};
        let root = tempfile::tempdir().expect("temporary publication root");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .expect("secure temporary root");
        let parent = root.path().join("promotion");
        fs::create_dir(&parent).expect("create promotion parent");
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700))
            .expect("secure promotion parent");
        let target = parent.join("promotion-record-v4.norito");
        let bytes = b"authenticated promotion record";
        let outcome = write_new_durable_file_with_hooks_v1(
            &target,
            bytes,
            || Ok(()),
            |_| Err(io::Error::other("injected parent sync failure")),
        )
        .expect("rename success is represented as an explicit outcome");
        match &outcome {
            DurableFilePublicationOutcomeV1::CommitUncertain { final_path, reason } => {
                assert_eq!(final_path, &target);
                assert!(reason.contains("injected parent sync failure"));
            }
            DurableFilePublicationOutcomeV1::Committed { .. } => {
                panic!("a failed post-rename parent sync cannot be committed")
            }
        }
        assert_eq!(fs::read(&target).expect("read visible final record"), bytes);
        assert_eq!(DURABLE_FILE_COMMIT_UNCERTAIN_EXIT_CODE, 75);
        assert!(
            outcome
                .operator_record()
                .contains("status=commit-uncertain")
        );
    }
    #[test]
    fn every_command_publication_handles_commit_uncertainty() {
        let source = include_str!("kagemusha.rs");
        let taira_source = include_str!("kagemusha/taira.rs");
        assert_eq!(
            source.matches("publish_new_durable_file(").count(),
            3,
            "promotion and activation must share the explicit outcome boundary"
        );
        assert!(
            source.contains("&mut std::io::sink()") && source.contains("&args.promotion_record"),
            "promotion must suppress the preliminary committed record so authenticated stdout remains one exact JSON line"
        );
        assert!(
            source.contains("publish_new_durable_file(writer, &args.output"),
            "activation must retain its committed operator record"
        );
        assert!(
            !source.contains("write_new_durable_file(&args."),
            "command dispatch must not discard a low-level publication outcome"
        );
        assert_eq!(
            taira_source.matches("publish_new_durable_file(").count(),
            3,
            "Taira roster, genesis, and operator identity publication must share the explicit outcome boundary"
        );
        assert!(
            !taira_source.contains("write_new_durable_file("),
            "Taira command publication must not discard a low-level publication outcome"
        );
        assert_eq!(
            source
                .matches("publish_release_circuit_params_directory_v4(writer, &args.output_dir")
                .count(),
            1,
            "release circuit parameters must use the explicit atomic-directory outcome boundary"
        );
        assert!(
            !source.contains("write_release_circuit_params_directory_v1(&args.output_dir"),
            "command dispatch must not discard a low-level directory publication outcome"
        );
    }
    #[test]
    fn roster_generation_binding_uses_the_authenticated_descriptor() {
        assert!(roster_release_generations_match_v4(
            "release-a",
            "release-a",
            "release-a"
        ));
        assert!(!roster_release_generations_match_v4(
            "release-a",
            "release-a",
            "release-b"
        ));
        assert!(!roster_release_generations_match_v4(
            "release-b",
            "release-a",
            "release-a"
        ));
    }
    #[test]
    fn v4_report_inventory_is_the_canonical_eight_role_abi21_order() {
        assert_eq!(
            REPORT_ARTIFACT_PURPOSES_V4,
            [
                "step_eq_params_ipa",
                "step_eq_proving_key",
                "step_eq_verifying_key",
                "step_eq_bootstrap_witness",
                "step_ep_params_ipa",
                "step_ep_proving_key",
                "step_ep_verifying_key",
                "step_ep_bootstrap_witness",
            ]
        );
        assert_eq!(REPORT_ARTIFACT_PURPOSES_V4.len(), 8);
        assert_eq!(AUTHENTICATED_ARTIFACT_ROLES_V4.len(), 8);
        assert_eq!(REPORT_ROSTER_PURPOSE, "topup_finality_roster");
    }
    #[test]
    fn promotion_flow_executes_candidate_publish_and_promoted_verification_in_order() {
        let events = RefCell::new(Vec::new());
        let result = verify_publish_verify_release_v4(
            |state| {
                events.borrow_mut().push(match state {
                    ReleaseInventoryStateV4::Candidate => "verify-17",
                    ReleaseInventoryStateV4::Promoted => "verify-18",
                });
                Ok(state.exact_file_count())
            },
            |candidate_count| {
                assert_eq!(*candidate_count, 17);
                events.borrow_mut().push("publish-no-replace");
                Ok(())
            },
        )
        .expect("non-circular promotion flow succeeds");
        assert_eq!(result, 18);
        assert_eq!(
            events.into_inner(),
            ["verify-17", "publish-no-replace", "verify-18"]
        );
    }
    #[test]
    fn promotion_flow_never_reaches_publication_or_full_verification_after_failure() {
        let events = RefCell::new(Vec::new());
        let error = verify_publish_verify_release_v4(
            |state| {
                events.borrow_mut().push(state.label());
                if state == ReleaseInventoryStateV4::Candidate {
                    return Err(color_eyre::eyre::eyre!("candidate rejected"));
                }
                Ok(())
            },
            |()| {
                events.borrow_mut().push("publish");
                Ok(())
            },
        )
        .expect_err("candidate failure must stop the flow");
        assert!(error.to_string().contains("candidate rejected"));
        assert_eq!(events.into_inner(), ["pre-promotion candidate"]);
    }
    #[test]
    fn promotion_record_target_is_the_exact_absent_canonical_bundle_leaf() {
        let root = tempfile::tempdir().expect("temporary release root");
        let canonical_root = root.path().canonicalize().expect("canonical release root");
        let expected = canonical_root.join(PROMOTION_RECORD_FILE_NAME_V4);
        require_new_promotion_record_path_v4(&canonical_root, &expected)
            .expect("the exact absent promotion leaf is accepted");

        let relative = Path::new(PROMOTION_RECORD_FILE_NAME_V4);
        assert!(require_new_promotion_record_path_v4(&canonical_root, relative).is_err());
        assert!(
            require_new_promotion_record_path_v4(
                &canonical_root,
                &canonical_root.join("other-promotion.norito"),
            )
            .is_err()
        );
        let lexical_alias = canonical_root.join(".").join(PROMOTION_RECORD_FILE_NAME_V4);
        assert!(require_new_promotion_record_path_v4(&canonical_root, &lexical_alias).is_err());

        fs::write(&expected, b"preexisting record").expect("create existing destination");
        assert!(require_new_promotion_record_path_v4(&canonical_root, &expected).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn promotion_record_target_rejects_a_dangling_symlink() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().expect("temporary release root");
        let canonical_root = root.path().canonicalize().expect("canonical release root");
        let expected = canonical_root.join(PROMOTION_RECORD_FILE_NAME_V4);
        symlink("missing-record", &expected).expect("create dangling destination symlink");
        assert!(require_new_promotion_record_path_v4(&canonical_root, &expected).is_err());
    }
    #[test]
    fn release_inventory_rejects_role_name_collision_instead_of_collapsing_it() {
        let mut expected = BTreeSet::new();
        insert_expected_release_file_v4(
            &mut expected,
            KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_FILE_NAME_V1,
            "internal-validation receipt",
        )
        .expect("first role owns its file name");
        let error = insert_expected_release_file_v4(
            &mut expected,
            KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_FILE_NAME_V1,
            "substituted artifact",
        )
        .expect_err("a descriptor cannot alias the fixed internal-validation receipt");
        assert!(error.to_string().contains("aliases another release file"));
        assert_eq!(expected.len(), 1);
    }
    #[test]
    fn authenticated_artifact_validation_drops_each_payload_before_loading_the_next() {
        let live = Rc::new(Cell::new(0));
        let peak = Rc::new(Cell::new(0));
        validate_artifacts_sequentially(0..AUTHENTICATED_ARTIFACT_ROLES_V4.len(), |_| {
            assert_eq!(live.get(), 0, "the prior artifact payload must be dropped");
            live.set(1);
            peak.set(peak.get().max(live.get()));
            Ok::<_, ()>(LivePayload {
                live: Rc::clone(&live),
            })
        })
        .expect("sequential validation succeeds");
        assert_eq!(live.get(), 0);
        assert_eq!(peak.get(), 1);
    }
    #[test]
    fn release_verifier_uses_the_sequential_artifact_path() {
        let source = include_str!("kagemusha.rs");
        let verifier = source
            .split_once("fn verify_release_directory_v4(")
            .expect("release verifier exists")
            .1
            .split_once("\nfn verify_roster_v4(")
            .expect("release verifier boundary exists")
            .0;
        assert!(verifier.contains("validate_artifacts_sequentially("));
        assert!(!verifier.contains("validated.push("));
        assert!(!verifier.contains("KagemushaPastaCycleProverArtifactsV4::new("));
    }
    #[test]
    fn atomic_activation_accepts_only_fail_closed_production_app_policy() {
        let policy = valid_device_attestation_policy();
        assert!(
            validate_device_attestation_policy_for_atomic_activation(
                &policy,
                POLICY_EVALUATION_TIME_MS,
            )
            .is_ok()
        );
        for mutate in [
            |policy: &mut OfflineDeviceAttestationPolicy| {
                policy.require_android_app_policy = false;
            },
            |policy: &mut OfflineDeviceAttestationPolicy| {
                policy.ios_apps[0].environment = "development".to_owned();
            },
            |policy: &mut OfflineDeviceAttestationPolicy| {
                policy.android_apps[0].signing_certificate_sha256[0].pop();
            },
        ] {
            let mut changed = policy.clone();
            mutate(&mut changed);
            assert!(
                validate_device_attestation_policy_for_atomic_activation(
                    &changed,
                    POLICY_EVALUATION_TIME_MS,
                )
                .is_err()
            );
        }
    }
    #[test]
    fn configured_device_policy_requires_exact_canonical_json_bytes() {
        let policy = valid_device_attestation_policy();
        let canonical = norito::json::to_vec(&policy).expect("canonical policy JSON");
        let root = tempfile::tempdir().expect("temporary policy root");
        let path = root.path().join("policy.json");
        std::fs::write(&path, &canonical).expect("write canonical policy");
        assert_eq!(
            configured_device_attestation_policy(&path, POLICY_EVALUATION_TIME_MS)
                .expect("canonical policy accepted"),
            policy
        );

        let mut padded = Vec::with_capacity(canonical.len() + 1);
        padded.push(b' ');
        padded.extend_from_slice(&canonical);
        std::fs::write(&path, padded).expect("write noncanonical policy");
        let error = configured_device_attestation_policy(&path, POLICY_EVALUATION_TIME_MS)
            .expect_err("lexically noncanonical policy must fail closed");
        assert!(error.to_string().contains("exact canonical encoding"));
    }
    #[test]
    fn atomic_activation_rejects_truncated_sequence_as_trusted_root() {
        let mut policy = valid_device_attestation_policy();
        policy.trusted_roots[0].der = vec![0x30, 0x01];
        let error = validate_device_attestation_policy_for_atomic_activation(
            &policy,
            POLICY_EVALUATION_TIME_MS,
        )
        .expect_err("a sequence marker is not a complete X.509 CA certificate");
        assert!(
            error.to_string().contains("DER is invalid"),
            "unexpected shared-validator error: {error}"
        );
    }
    #[test]
    fn atomic_activation_rejects_noncanonical_app_policy_text() {
        type PolicyMutation = fn(&mut OfflineDeviceAttestationPolicy);
        let cases: [(&str, PolicyMutation); 8] = [
            ("padded iOS Team ID", |policy| {
                policy.ios_apps[0].team_id = " YLWWUD25VZ".to_owned();
            }),
            ("control character in iOS Team ID", |policy| {
                policy.ios_apps[0].team_id = "YLWWU\nD25VZ".to_owned();
            }),
            ("padded iOS bundle ID", |policy| {
                policy.ios_apps[0].bundle_id = "pk.retail.wallet.ios ".to_owned();
            }),
            ("control character in iOS bundle ID", |policy| {
                policy.ios_apps[0].bundle_id = "pk.retail.\u{7f}wallet.ios".to_owned();
            }),
            ("padded Android package name", |policy| {
                policy.android_apps[0].package_name = " com.pk.retailwallet".to_owned();
            }),
            ("control character in Android package name", |policy| {
                policy.android_apps[0].package_name = "com.pk.\0retailwallet".to_owned();
            }),
            ("padded iOS bundle version", |policy| {
                policy.ios_apps[0].allowed_bundle_versions[0] = "202605050324 ".to_owned();
            }),
            ("control character in iOS bundle version", |policy| {
                policy.ios_apps[0].allowed_bundle_versions[0] = "202605\n050324".to_owned();
            }),
        ];

        for (context, mutate) in cases {
            let mut policy = valid_device_attestation_policy();
            mutate(&mut policy);
            assert!(
                validate_device_attestation_policy_for_atomic_activation(
                    &policy,
                    POLICY_EVALUATION_TIME_MS,
                )
                .is_err(),
                "{context} must be rejected"
            );
        }
    }
    #[test]
    fn atomic_activation_rejects_missing_duplicate_and_noncanonical_policy_entries() {
        let policy = valid_device_attestation_policy();
        let mut missing_platform = policy.clone();
        missing_platform
            .trusted_roots
            .retain(|root| root.platform != "android-keymint");
        assert!(
            validate_device_attestation_policy_for_atomic_activation(
                &missing_platform,
                POLICY_EVALUATION_TIME_MS,
            )
            .is_err()
        );
        let mut duplicate_root = policy.clone();
        duplicate_root
            .trusted_roots
            .push(duplicate_root.trusted_roots[0].clone());
        assert!(
            validate_device_attestation_policy_for_atomic_activation(
                &duplicate_root,
                POLICY_EVALUATION_TIME_MS,
            )
            .is_err()
        );
        let mut unsorted_categories = policy.clone();
        unsorted_categories.ios_apps[0].allowed_validation_categories = vec![8, 7];
        assert!(
            validate_device_attestation_policy_for_atomic_activation(
                &unsorted_categories,
                POLICY_EVALUATION_TIME_MS,
            )
            .is_err()
        );
        let mut duplicate_signer = policy;
        duplicate_signer.android_apps[0]
            .signing_certificate_sha256
            .push(vec![0x11; 32]);
        assert!(
            validate_device_attestation_policy_for_atomic_activation(
                &duplicate_signer,
                POLICY_EVALUATION_TIME_MS,
            )
            .is_err()
        );
    }
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "the boundary test enumerates every governed collection and root cap with its exact plus-one rejection"
    )]
    fn atomic_activation_policy_collection_and_root_caps_have_exact_boundaries() {
        let mut exact_roots = valid_device_attestation_policy();
        exact_roots.trusted_roots = (0..OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOTS_V1)
            .map(|index| {
                let platform = if index % 2 == 0 {
                    "android-keymint"
                } else {
                    "ios-appattest"
                };
                bounded_trusted_root(platform, u8::try_from(index + 1).unwrap(), 2)
            })
            .collect();
        assert_atomic_policy_shape_valid(&exact_roots, "total trusted-root limit");
        let mut too_many_roots = exact_roots;
        too_many_roots
            .trusted_roots
            .push(bounded_trusted_root("android-keymint", 0xF0, 2));
        assert_atomic_policy_cap_rejected(
            &too_many_roots,
            "exceeds collection limits",
            "total trusted-root limit plus one",
        );

        let mut exact_platform_roots = valid_device_attestation_policy();
        exact_platform_roots.trusted_roots = (0
            ..OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOTS_PER_PLATFORM_V1)
            .map(|index| {
                bounded_trusted_root("android-keymint", u8::try_from(index + 1).unwrap(), 2)
            })
            .chain(std::iter::once(bounded_trusted_root(
                "ios-appattest",
                0x80,
                2,
            )))
            .collect();
        assert_atomic_policy_shape_valid(&exact_platform_roots, "per-platform trusted-root limit");
        let mut too_many_platform_roots = exact_platform_roots;
        too_many_platform_roots
            .trusted_roots
            .push(bounded_trusted_root("android-keymint", 0x81, 2));
        assert_atomic_policy_cap_rejected(
            &too_many_platform_roots,
            "roots exceed size limits",
            "per-platform trusted-root limit plus one",
        );

        let mut exact_der = valid_device_attestation_policy();
        exact_der.trusted_roots[0] = bounded_trusted_root(
            "android-keymint",
            1,
            OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOT_DER_BYTES_V1,
        );
        assert_atomic_policy_shape_valid(&exact_der, "trusted-root DER limit");
        exact_der.trusted_roots[0].der.push(0);
        assert_atomic_policy_cap_rejected(
            &exact_der,
            "roots exceed size limits",
            "trusted-root DER limit plus one",
        );

        let mut exact_revocations = valid_device_attestation_policy();
        exact_revocations.revoked_certificate_tbs_sha256 = (0
            ..OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_REVOKED_CERTIFICATES_V1)
            .map(|index| {
                let mut digest = vec![0; 32];
                digest[24..].copy_from_slice(&u64::try_from(index + 1).unwrap().to_be_bytes());
                digest
            })
            .collect();
        assert_atomic_policy_shape_valid(&exact_revocations, "revocation limit");
        exact_revocations
            .revoked_certificate_tbs_sha256
            .push(vec![0xFF; 32]);
        assert_atomic_policy_cap_rejected(
            &exact_revocations,
            "exceeds collection limits",
            "revocation limit plus one",
        );

        let mut exact_ios_apps = valid_device_attestation_policy();
        exact_ios_apps.ios_apps = (0..OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_APPS_V1)
            .map(bounded_ios_app)
            .collect();
        assert_atomic_policy_shape_valid(&exact_ios_apps, "iOS app limit");
        exact_ios_apps.ios_apps.push(bounded_ios_app(
            OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_APPS_V1,
        ));
        assert_atomic_policy_cap_rejected(
            &exact_ios_apps,
            "exceeds collection limits",
            "iOS app limit plus one",
        );

        let mut exact_android_apps = valid_device_attestation_policy();
        exact_android_apps.android_apps = (0
            ..OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_ANDROID_APPS_V1)
            .map(bounded_android_app)
            .collect();
        assert_atomic_policy_shape_valid(&exact_android_apps, "Android app limit");
        exact_android_apps.android_apps.push(bounded_android_app(
            OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_ANDROID_APPS_V1,
        ));
        assert_atomic_policy_cap_rejected(
            &exact_android_apps,
            "exceeds collection limits",
            "Android app limit plus one",
        );
    }
    #[test]
    fn atomic_activation_policy_nested_app_caps_have_exact_boundaries() {
        let mut exact_categories = valid_device_attestation_policy();
        exact_categories.ios_apps[0].allowed_validation_categories = vec![1, 2, 3, 4, 5, 6, 10];
        assert_eq!(
            exact_categories.ios_apps[0]
                .allowed_validation_categories
                .len(),
            OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_VALIDATION_CATEGORIES_V1
        );
        assert_atomic_policy_shape_valid(&exact_categories, "iOS validation-category limit");
        exact_categories.ios_apps[0]
            .allowed_validation_categories
            .push(11);
        assert_atomic_policy_cap_rejected(
            &exact_categories,
            "iOS app policy exceeds size limits",
            "iOS validation-category limit plus one",
        );

        let mut exact_versions = valid_device_attestation_policy();
        exact_versions.ios_apps[0].allowed_bundle_versions = (0
            ..OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_BUNDLE_VERSIONS_V1)
            .map(|index| format!("{index:04}"))
            .collect();
        assert_atomic_policy_shape_valid(&exact_versions, "iOS bundle-version count limit");
        exact_versions.ios_apps[0]
            .allowed_bundle_versions
            .push(format!(
                "{:04}",
                OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_BUNDLE_VERSIONS_V1
            ));
        assert_atomic_policy_cap_rejected(
            &exact_versions,
            "iOS app policy exceeds size limits",
            "iOS bundle-version count limit plus one",
        );

        let mut exact_version_bytes = valid_device_attestation_policy();
        exact_version_bytes.ios_apps[0].allowed_bundle_versions =
            vec!["1".repeat(OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_BUNDLE_VERSION_BYTES_V1)];
        assert_atomic_policy_shape_valid(&exact_version_bytes, "iOS bundle-version byte limit");
        exact_version_bytes.ios_apps[0].allowed_bundle_versions[0].push('1');
        assert_atomic_policy_cap_rejected(
            &exact_version_bytes,
            "iOS app policy exceeds size limits",
            "iOS bundle-version byte limit plus one",
        );

        let mut exact_signers = valid_device_attestation_policy();
        exact_signers.android_apps[0].signing_certificate_sha256 = (1
            ..=OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_ANDROID_SIGNING_CERTIFICATES_V1)
            .map(|index| vec![u8::try_from(index).unwrap(); 32])
            .collect();
        assert_atomic_policy_shape_valid(&exact_signers, "Android signing-certificate limit");
        exact_signers.android_apps[0]
            .signing_certificate_sha256
            .push(vec![0xF0; 32]);
        assert_atomic_policy_cap_rejected(
            &exact_signers,
            "Android app policy exceeds size limits",
            "Android signing-certificate limit plus one",
        );

        let mut exact_team_id = valid_device_attestation_policy();
        exact_team_id.ios_apps[0].team_id =
            "A".repeat(OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TEAM_ID_BYTES_V1);
        assert_atomic_policy_shape_valid(&exact_team_id, "iOS Team ID byte limit");
        exact_team_id.ios_apps[0].team_id.push('A');
        assert_atomic_policy_cap_rejected(
            &exact_team_id,
            "iOS app policy exceeds size limits",
            "iOS Team ID byte limit plus one",
        );

        let mut exact_bundle_id = valid_device_attestation_policy();
        exact_bundle_id.ios_apps[0].bundle_id =
            "a".repeat(OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_APP_IDENTIFIER_BYTES_V1);
        assert_atomic_policy_shape_valid(&exact_bundle_id, "iOS bundle ID byte limit");
        exact_bundle_id.ios_apps[0].bundle_id.push('a');
        assert_atomic_policy_cap_rejected(
            &exact_bundle_id,
            "iOS app policy exceeds size limits",
            "iOS bundle ID byte limit plus one",
        );

        let mut exact_package_name = valid_device_attestation_policy();
        exact_package_name.android_apps[0].package_name =
            "a".repeat(OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_APP_IDENTIFIER_BYTES_V1);
        assert_atomic_policy_shape_valid(&exact_package_name, "Android package-name byte limit");
        exact_package_name.android_apps[0].package_name.push('a');
        assert_atomic_policy_cap_rejected(
            &exact_package_name,
            "Android app policy exceeds size limits",
            "Android package-name byte limit plus one",
        );
    }
    #[test]
    fn atomic_activation_policy_canonical_byte_cap_has_exact_boundary() {
        let exact = policy_at_exact_canonical_byte_limit();
        assert_eq!(
            norito::encode_canonical(&exact)
                .expect("encode exact-bound policy")
                .len(),
            OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_CANONICAL_BYTES_V1
        );
        assert_atomic_policy_shape_valid(&exact, "canonical policy byte limit");
        let mut over = exact;
        assert!(
            over.trusted_roots[3].der.len()
                < OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOT_DER_BYTES_V1
        );
        over.trusted_roots[3].der.push(4);
        assert!(
            norito::encode_canonical(&over)
                .expect("encode over-bound policy")
                .len()
                > OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_CANONICAL_BYTES_V1
        );
        assert_atomic_policy_cap_rejected(
            &over,
            "exceeds canonical size limit",
            "canonical policy byte limit plus one",
        );
    }
}
