//! Authenticated Kagemusha ABI-21/V4 release verification and activation preparation.

use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use clap::{Args as ClapArgs, Subcommand};
use color_eyre::eyre::{WrapErr as _, bail, eyre};
use iroha_core::smartcontracts::isi::offline::KagemushaReleaseCatalogV4;
use iroha_core::zk::kagemusha_artifact_v4::read_kagemusha_pasta_cycle_artifact_v4;
use iroha_core::zk::kagemusha_v2::validate_kagemusha_step_bootstrap_payload_v4;
use iroha_crypto::HashOf;
use iroha_data_model::isi::{InstructionBox, offline::ActivateKagemushaRecursiveReleaseV4};
use iroha_data_model::offline::{
    KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4,
    KAGEMUSHA_RECURSIVE_SPEND_BENCHMARK_EVIDENCE_FILE_NAME_V1,
    KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_FILE_NAME_V1,
    KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_MAX_BYTES_V4,
    KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
    KAGEMUSHA_RECURSIVE_SPEND_PROMOTED_RELEASE_SCHEMA_V4,
    KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_FILE_NAME_V4,
    KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V4,
    KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1,
    KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_V2, KagemushaAuthenticatedReleaseV4,
    KagemushaPastaCycleArtifactKindV4, KagemushaPastaCycleParityV1,
    KagemushaRecursiveSpendArtifactManifestV4, KagemushaRecursiveSpendCandidateV4,
    KagemushaRecursiveSpendPromotedReleaseV4, KagemushaRecursiveSpendReleaseAttestationV4,
    KagemushaRecursiveSpendReleasePolicyV1, KagemushaTopUpFinalityRosterArtifactV2,
    OfflineDeviceAttestationPolicy, kagemusha_recursive_spend_release_sha256,
};

use crate::{Outcome, RunArgs};

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
pub(super) struct Args {
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
}

#[derive(Debug, ClapArgs)]
struct VerifyReleaseV4Args {
    /// Immutable directory containing the exact ABI-21/V4 release inventory.
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
}

#[derive(Debug, ClapArgs)]
struct PromoteReleaseV4Args {
    /// Immutable directory containing the exact ABI-21/V4 release inventory.
    #[arg(long)]
    bundle_dir: PathBuf,
    /// Canonical release policy provisioned alongside the candidate release.
    #[arg(long)]
    release_policy: PathBuf,
    /// New path for the canonical Norito promotion record; it is never overwritten.
    #[arg(long)]
    promotion_record: PathBuf,
    /// Signed physical-device benchmark evidence file.
    #[arg(long)]
    benchmark_evidence: PathBuf,
    /// Canonical signed, candidate-bound cryptographic review Norito file.
    #[arg(long)]
    cryptographic_review: PathBuf,
}

#[derive(Debug, ClapArgs)]
struct PrepareActivationV4Args {
    /// Root containing lowercase manifest-digest release directories.
    #[arg(long)]
    artifact_root: PathBuf,
    /// Canonical release policy configured on every validator.
    #[arg(long)]
    release_policy: PathBuf,
    /// Exact lowercase SHA-256 directory name of the release to activate.
    #[arg(long, value_parser = parse_manifest_sha256)]
    manifest_sha256: [u8; 32],
    /// Next atomic Eq/Ep verifier version observed from live consensus state.
    #[arg(long)]
    verifier_version: u32,
    /// Exact governed verifier policy derived from authenticated physical-device evidence.
    /// The policy and release are embedded in one composite consensus instruction.
    #[arg(long)]
    device_attestation_policy: PathBuf,
    /// New private file receiving a JSON array accepted by `iroha multisig propose`.
    #[arg(long)]
    output: PathBuf,
}

impl<T: Write> RunArgs<T> for Args {
    fn run(self, writer: &mut std::io::BufWriter<T>) -> Outcome {
        match self.command {
            Command::VerifyReleaseV4(args) => {
                let policy_bytes = configured_policy_bytes(&args.release_policy)?;
                let verified = verify_release_directory_v4(
                    &args.bundle_dir,
                    &policy_bytes,
                    &args.benchmark_evidence,
                    &args.cryptographic_review,
                )?;
                let report = verified.verification_report()?;
                writeln!(writer, "{}", report.canonical_json()?)?;
            }
            Command::PromoteReleaseV4(args) => {
                let policy_bytes = configured_policy_bytes(&args.release_policy)?;
                let verified = verify_release_directory_v4(
                    &args.bundle_dir,
                    &policy_bytes,
                    &args.benchmark_evidence,
                    &args.cryptographic_review,
                )?;
                let record = verified.promotion_record()?;
                record.validate().map_err(|error| eyre!(error))?;
                let record_bytes = norito::to_bytes(&record)
                    .wrap_err("failed to encode Kagemusha V4 promotion record")?;
                write_new_durable_file(&args.promotion_record, &record_bytes)?;
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
                let policy = configured_device_attestation_policy(&args.device_attestation_policy)?;
                let state_bytes = norito::to_bytes(&policy)
                    .wrap_err("failed to encode governed device-attestation policy state")?;
                let policy_state_sha256 =
                    hex::encode(kagemusha_recursive_spend_release_sha256(&state_bytes));
                let instruction = ActivateKagemushaRecursiveReleaseV4::new(activation, policy);
                let instructions = vec![InstructionBox::from(instruction)];
                let instructions_hash = HashOf::new(&instructions);
                let mut instruction_json = norito::json::to_string(&instructions)
                    .wrap_err("failed to encode Kagemusha V4 activation instruction JSON")?;
                instruction_json.push('\n');
                write_new_durable_file(&args.output, instruction_json.as_bytes())?;
                writeln!(
                    writer,
                    "{{\"status\":\"prepared\",\"manifest_sha256\":\"{}\",\"verifier_version\":{},\"instruction_count\":1,\"instructions_hash\":\"{}\",\"device_attestation_policy_state_sha256\":\"{}\"}}",
                    hex::encode(args.manifest_sha256),
                    args.verifier_version,
                    instructions_hash,
                    policy_state_sha256,
                )?;
            }
        }
        Ok(())
    }
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

fn configured_device_attestation_policy(path: &Path) -> Result<OfflineDeviceAttestationPolicy> {
    let raw = read_external_bounded(
        path,
        MAX_POLICY_BYTES,
        "governed Offline device-attestation policy",
    )?;
    let policy: OfflineDeviceAttestationPolicy = norito::json::from_slice(&raw)
        .wrap_err("failed to decode governed Offline device-attestation policy JSON")?;
    validate_device_attestation_policy_for_atomic_activation(&policy)?;
    let canonical = norito::json::to_string(&policy)
        .wrap_err("failed to encode canonical Offline device-attestation policy JSON")?;
    let reparsed: OfflineDeviceAttestationPolicy = norito::json::from_str(&canonical)
        .wrap_err("failed to reparse canonical Offline device-attestation policy JSON")?;
    if reparsed != policy {
        bail!("Offline device-attestation policy JSON is not lossless");
    }
    Ok(policy)
}

fn validate_device_attestation_policy_for_atomic_activation(
    policy: &OfflineDeviceAttestationPolicy,
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
    let mut root_ids = BTreeSet::new();
    let mut platforms = BTreeSet::new();
    for root in &policy.trusted_roots {
        if !matches!(root.platform.as_str(), "ios-appattest" | "android-keymint")
            || root.der.is_empty()
            || root.der.len() > 16 * 1024
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
    let mut revoked = BTreeSet::new();
    for digest in &policy.revoked_certificate_sha256 {
        if digest.len() != 32 || !revoked.insert(digest.as_slice()) {
            bail!("atomic Kagemusha activation contains an invalid duplicate revocation");
        }
    }
    let mut ios_ids = BTreeSet::new();
    for app in &policy.ios_apps {
        if app.team_id.is_empty()
            || !app.team_id.is_ascii()
            || app.bundle_id.is_empty()
            || !app.bundle_id.is_ascii()
            || app.environment != "production"
            || app.allowed_validation_categories.is_empty()
            || app.allowed_bundle_versions.is_empty()
            || app.allow_legacy_auth_data_without_extensions
            || app
                .allowed_bundle_versions
                .iter()
                .any(|value| value.is_empty() || !value.is_ascii())
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
    let mut android_ids = BTreeSet::new();
    for app in &policy.android_apps {
        if app.package_name.is_empty()
            || !app.package_name.is_ascii()
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
            candidate_sha256: candidate
                .sha256()
                .map_err(|error| eyre!("failed to identify immutable V4 candidate: {error}"))?,
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

fn verify_release_directory_v4(
    bundle_dir: &Path,
    policy_bytes: &[u8],
    benchmark_evidence_path: &Path,
    cryptographic_review_path: &Path,
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
    if manifest_from_json != manifest {
        bail!("Kagemusha V4 JSON and Norito manifests are not semantically identical");
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
    let authenticated = KagemushaAuthenticatedReleaseV4::verify(
        &manifest,
        &policy,
        &attestation,
        &benchmark,
        &review,
    )
    .map_err(|error| eyre!("Kagemusha V4 release authentication failed: {error}"))?;

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
    if authenticated.manifest_sha256() == [0; 32]
        || authenticated.manifest_sha256() != manifest_sha256
        || authenticated.manifest().max_proof_bytes != manifest.max_proof_bytes
    {
        bail!("authenticated Kagemusha V4 material does not bind the canonical manifest");
    }

    verify_roster_v4(&root, &manifest)?;
    verify_exact_inventory_v4(&root, &manifest)?;

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
    Ok(verified)
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
    if roster.chain_id != manifest.chain_id
        || roster.artifact_generation != manifest.generation
        || roster.window_at(manifest.activation_height).is_err()
        || roster
            .window_at(manifest.withdrawal_height.saturating_sub(1))
            .is_err()
    {
        bail!("Kagemusha V4 top-up finality roster release binding mismatch");
    }
    Ok(())
}

fn verify_exact_inventory_v4(
    root: &Path,
    manifest: &KagemushaRecursiveSpendArtifactManifestV4,
) -> Outcome {
    let mut expected = std::collections::BTreeSet::from([
        MANIFEST_JSON_FILE_NAME.to_owned(),
        MANIFEST_NORITO_FILE_NAME.to_owned(),
        MANIFEST_NORITO_SHA256_FILE_NAME.to_owned(),
        RELEASE_ATTESTATION_FILE_NAME_V4.to_owned(),
        KAGEMUSHA_RECURSIVE_SPEND_BENCHMARK_EVIDENCE_FILE_NAME_V1.to_owned(),
        KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_FILE_NAME_V1.to_owned(),
        PROMOTION_RECORD_FILE_NAME_V4.to_owned(),
        manifest.topup_finality_roster_artifact.file_name.clone(),
    ]);
    for artifact in manifest
        .profiles
        .iter()
        .flat_map(|profile| profile.artifacts.iter())
    {
        expected.insert(artifact.file_name.clone());
    }
    let observed = fs::read_dir(root)
        .wrap_err("failed to enumerate Kagemusha V4 release directory")?
        .map(|entry| {
            entry
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .wrap_err("failed to inspect Kagemusha V4 release inventory")
        })
        .collect::<Result<std::collections::BTreeSet<_>>>()?;
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
    let mut bytes = Vec::with_capacity(opened.length);
    Read::by_ref(&mut opened.file)
        .take(u64::try_from(max_bytes).expect("file bound fits u64") + 1)
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

fn write_new_durable_file(path: &Path, bytes: &[u8]) -> Outcome {
    let parent = path
        .parent()
        .ok_or_else(|| eyre!("promotion-record path has no parent"))?;
    let parent_metadata =
        fs::symlink_metadata(parent).wrap_err("promotion-record parent is unavailable")?;
    if parent_metadata.file_type().is_symlink() || !parent_metadata.is_dir() {
        bail!("promotion-record parent must be a non-symlink directory");
    }
    reject_unsafe_mode(&parent_metadata, "promotion-record parent")?;
    let canonical_parent = parent
        .canonicalize()
        .wrap_err("failed to canonicalize promotion-record parent")?;
    let file_name = path
        .file_name()
        .ok_or_else(|| eyre!("promotion-record path has no file name"))?;
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
        .wrap_err("refusing to overwrite or alias an existing promotion record")?;
    file.write_all(bytes)
        .wrap_err("failed to write Kagemusha promotion record")?;
    file.sync_all()
        .wrap_err("failed to durably sync Kagemusha promotion record")?;
    File::open(&canonical_parent)
        .and_then(|directory| directory.sync_all())
        .wrap_err("failed to durably sync promotion-record directory")?;
    Ok(())
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
    release_policy_sha256: String,
    generation: String,
    chain_id: String,
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
            release_policy_sha256: hex::encode(release_policy_sha256),
            generation: manifest.generation.clone(),
            chain_id: manifest.chain_id.to_string(),
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
    promotion_record_sha256: String,
    release_policy_sha256: String,
    generation: String,
    chain_id: String,
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
            promotion_record_sha256: hex::encode(promotion_record_sha256),
            release_policy_sha256: report.release_policy_sha256.clone(),
            generation: report.generation.clone(),
            chain_id: report.chain_id.clone(),
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
    use std::{cell::Cell, rc::Rc};

    use iroha_data_model::offline::{
        OfflineAndroidAppAttestationPolicy, OfflineDeviceAttestationPolicy,
        OfflineDeviceAttestationTrustedRoot, OfflineIosAppAttestationPolicy,
    };

    use super::{
        AUTHENTICATED_ARTIFACT_ROLES_V4, REPORT_ARTIFACT_PURPOSES_V4, REPORT_ROSTER_PURPOSE,
        parse_manifest_sha256, validate_artifacts_sequentially,
        validate_device_attestation_policy_for_atomic_activation,
    };

    struct LivePayload {
        live: Rc<Cell<usize>>,
    }

    impl Drop for LivePayload {
        fn drop(&mut self) {
            self.live.set(self.live.get() - 1);
        }
    }

    fn valid_device_attestation_policy() -> OfflineDeviceAttestationPolicy {
        OfflineDeviceAttestationPolicy {
            version: 1,
            trusted_roots: vec![
                OfflineDeviceAttestationTrustedRoot {
                    platform: "android-keymint".to_owned(),
                    der: vec![0x30, 0x01],
                    not_before_ms: None,
                    not_after_ms: None,
                },
                OfflineDeviceAttestationTrustedRoot {
                    platform: "ios-appattest".to_owned(),
                    der: vec![0x30, 0x02],
                    not_before_ms: None,
                    not_after_ms: None,
                },
            ],
            revoked_certificate_sha256: vec![],
            ios_apps: vec![OfflineIosAppAttestationPolicy {
                team_id: "YLWWUD25VZ".to_owned(),
                bundle_id: "pk.retail.wallet.ios".to_owned(),
                environment: "production".to_owned(),
                allowed_validation_categories: vec![4],
                allowed_bundle_versions: vec!["202605050324".to_owned()],
                allow_legacy_auth_data_without_extensions: false,
            }],
            android_apps: vec![OfflineAndroidAppAttestationPolicy {
                package_name: "com.pk.retailwallet".to_owned(),
                signing_certificate_sha256: vec![vec![0x11; 32]],
            }],
            require_ios_app_policy: true,
            require_android_app_policy: true,
        }
    }

    #[test]
    fn activation_manifest_digest_parser_is_lowercase_and_exact() {
        assert_eq!(parse_manifest_sha256(&"ab".repeat(32)), Ok([0xab; 32]));
        assert!(parse_manifest_sha256(&"AB".repeat(32)).is_err());
        assert!(parse_manifest_sha256(&"a".repeat(63)).is_err());
        assert!(parse_manifest_sha256(&format!("{}g", "a".repeat(63))).is_err());
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
        assert!(validate_device_attestation_policy_for_atomic_activation(&policy).is_ok());

        for mutate in [
            |policy: &mut OfflineDeviceAttestationPolicy| {
                policy.require_android_app_policy = false;
            },
            |policy: &mut OfflineDeviceAttestationPolicy| {
                policy.ios_apps[0].environment = "development".to_owned();
            },
            |policy: &mut OfflineDeviceAttestationPolicy| {
                policy.ios_apps[0].allow_legacy_auth_data_without_extensions = true;
            },
            |policy: &mut OfflineDeviceAttestationPolicy| {
                policy.android_apps[0].signing_certificate_sha256[0].pop();
            },
        ] {
            let mut changed = policy.clone();
            mutate(&mut changed);
            assert!(validate_device_attestation_policy_for_atomic_activation(&changed).is_err());
        }
    }

    #[test]
    fn atomic_activation_rejects_missing_duplicate_and_noncanonical_policy_entries() {
        let policy = valid_device_attestation_policy();

        let mut missing_platform = policy.clone();
        missing_platform.trusted_roots.pop();
        assert!(
            validate_device_attestation_policy_for_atomic_activation(&missing_platform).is_err()
        );

        let mut duplicate_root = policy.clone();
        duplicate_root
            .trusted_roots
            .push(duplicate_root.trusted_roots[0].clone());
        assert!(validate_device_attestation_policy_for_atomic_activation(&duplicate_root).is_err());

        let mut unsorted_categories = policy.clone();
        unsorted_categories.ios_apps[0].allowed_validation_categories = vec![8, 7];
        assert!(
            validate_device_attestation_policy_for_atomic_activation(&unsorted_categories).is_err()
        );

        let mut duplicate_signer = policy;
        duplicate_signer.android_apps[0]
            .signing_certificate_sha256
            .push(vec![0x11; 32]);
        assert!(
            validate_device_attestation_policy_for_atomic_activation(&duplicate_signer).is_err()
        );
    }
}
