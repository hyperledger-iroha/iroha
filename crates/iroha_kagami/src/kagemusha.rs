//! Authenticated Kagemusha ABI-19/V3 and ABI-20/V4 release verification.

use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use clap::{Args as ClapArgs, Subcommand};
use color_eyre::eyre::{WrapErr as _, bail, eyre};
use iroha_core::zk::kagemusha_artifact_v4::{
    KagemushaPastaCycleProverArtifactsV4, KagemushaValidatedArtifactPayloadV4,
    read_kagemusha_pasta_cycle_artifact_v4,
};
use iroha_core::zk::kagemusha_v2::{
    KagemushaPastaCycleProverArtifactsV3, KagemushaValidatedArtifactPayloadV3,
    read_kagemusha_pasta_cycle_artifact_v3, validate_kagemusha_step_bootstrap_payload_v4,
};
use iroha_data_model::offline::{
    KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4, KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3,
    KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
    KAGEMUSHA_RECURSIVE_SPEND_PROMOTED_RELEASE_SCHEMA_V3,
    KAGEMUSHA_RECURSIVE_SPEND_PROMOTED_RELEASE_SCHEMA_V4,
    KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_FILE_NAME_V1,
    KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_FILE_NAME_V4,
    KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V1,
    KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V4,
    KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1,
    KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_V2, KagemushaAuthenticatedReleaseV3,
    KagemushaAuthenticatedReleaseV4, KagemushaPastaCycleArtifactV3, KagemushaPastaCycleArtifactV4,
    KagemushaPastaCycleParityV1, KagemushaRecursiveSpendArtifactManifestV3,
    KagemushaRecursiveSpendArtifactManifestV4, KagemushaRecursiveSpendPromotedReleaseV3,
    KagemushaRecursiveSpendPromotedReleaseV4, KagemushaRecursiveSpendReleaseAttestationV1,
    KagemushaRecursiveSpendReleaseAttestationV4, KagemushaRecursiveSpendReleasePolicyV1,
    KagemushaTopUpFinalityRosterArtifactV2, kagemusha_recursive_spend_native_capabilities_v1,
    kagemusha_recursive_spend_release_sha256,
};

use crate::{Outcome, RunArgs};

type Result<T> = color_eyre::Result<T>;

const RELEASE_TRUST_ROOT_ENV: &str = "IROHA_KAGEMUSHA_RELEASE_TRUST_ROOT_NORITO_HEX";
const EMBEDDED_RELEASE_TRUST_ROOT_HEX: Option<&str> =
    option_env!("IROHA_KAGEMUSHA_RELEASE_TRUST_ROOT_NORITO_HEX");
const MANIFEST_JSON_FILE_NAME: &str = "manifest.json";
const MANIFEST_NORITO_FILE_NAME: &str = "manifest.norito";
const MANIFEST_NORITO_SHA256_FILE_NAME: &str = "manifest.norito.sha256";
const RELEASE_ATTESTATION_FILE_NAME: &str =
    KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_FILE_NAME_V1;
const RELEASE_ATTESTATION_FILE_NAME_V4: &str =
    KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_FILE_NAME_V4;
const MAX_MANIFEST_BYTES: usize = 1024 * 1024;
const MAX_POLICY_BYTES: usize = 64 * 1024;
const MAX_ATTESTATION_BYTES: usize = 1024 * 1024;
const RECURSIVE_STEP_VERIFIER_COMMITMENT_DOMAIN: &[u8] =
    b"iroha:kagemusha:recursive-step-verifier-commitment:v1";
const RECURSIVE_STEP_VERIFIER_COMMITMENT_DOMAIN_V4: &[u8] =
    b"iroha:kagemusha:recursive-step-verifier-commitment:v4";
const REPORT_ARTIFACT_PURPOSES: [&str; 6] = [
    "step_eq_parameters",
    "step_eq_proving_key",
    "step_eq_verifying_key",
    "step_ep_parameters",
    "step_ep_proving_key",
    "step_ep_verifying_key",
];
const REPORT_ROSTER_PURPOSE: &str = "topup_finality_roster";
const REPORT_ARTIFACT_PURPOSES_V4: [&str; 8] = KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4;

/// Kagemusha release-management command group.
#[derive(Debug, ClapArgs)]
pub(super) struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Verify one complete authenticated ABI-19/V3 release directory.
    #[command(name = "verify-release")]
    VerifyRelease(VerifyReleaseArgs),
    /// Verify a release and atomically write its typed promotion record.
    #[command(name = "promote-release")]
    PromoteRelease(PromoteReleaseArgs),
    /// Verify one complete authenticated ABI-20/V4 release directory.
    #[command(name = "verify-release-v4")]
    VerifyReleaseV4(VerifyReleaseV4Args),
    /// Verify an ABI-20/V4 release and atomically write its typed promotion record.
    #[command(name = "promote-release-v4")]
    PromoteReleaseV4(PromoteReleaseV4Args),
}

#[derive(Debug, ClapArgs)]
struct VerifyReleaseArgs {
    /// Immutable directory containing the exact release inventory selected by the subcommand.
    #[arg(long)]
    bundle_dir: PathBuf,
    /// Signed physical-device benchmark evidence file.
    #[arg(long)]
    benchmark_evidence: PathBuf,
    /// Independent cryptographic review evidence file.
    #[arg(long)]
    cryptographic_review: PathBuf,
}

#[derive(Debug, ClapArgs)]
struct PromoteReleaseArgs {
    /// Immutable directory containing the exact release inventory selected by the subcommand.
    #[arg(long)]
    bundle_dir: PathBuf,
    /// New path for the canonical Norito promotion record; it is never overwritten.
    #[arg(long)]
    promotion_record: PathBuf,
    /// Signed physical-device benchmark evidence file.
    #[arg(long)]
    benchmark_evidence: PathBuf,
    /// Independent cryptographic review evidence file.
    #[arg(long)]
    cryptographic_review: PathBuf,
}

#[derive(Debug, ClapArgs)]
struct VerifyReleaseV4Args {
    /// Immutable directory containing the exact ABI-20/V4 release inventory.
    #[arg(long)]
    bundle_dir: PathBuf,
    /// Canonical policy file whose exact bytes must match Kagami's embedded trust root.
    #[arg(long)]
    release_policy: PathBuf,
    /// Signed physical-device benchmark evidence file.
    #[arg(long)]
    benchmark_evidence: PathBuf,
    /// Independent cryptographic review evidence file.
    #[arg(long)]
    cryptographic_review: PathBuf,
}

#[derive(Debug, ClapArgs)]
struct PromoteReleaseV4Args {
    /// Immutable directory containing the exact ABI-20/V4 release inventory.
    #[arg(long)]
    bundle_dir: PathBuf,
    /// Canonical policy file whose exact bytes must match Kagami's embedded trust root.
    #[arg(long)]
    release_policy: PathBuf,
    /// New path for the canonical Norito promotion record; it is never overwritten.
    #[arg(long)]
    promotion_record: PathBuf,
    /// Signed physical-device benchmark evidence file.
    #[arg(long)]
    benchmark_evidence: PathBuf,
    /// Independent cryptographic review evidence file.
    #[arg(long)]
    cryptographic_review: PathBuf,
}

impl<T: Write> RunArgs<T> for Args {
    fn run(self, writer: &mut std::io::BufWriter<T>) -> Outcome {
        match self.command {
            Command::VerifyRelease(args) => {
                let report = verify_with_embedded_policy(
                    &args.bundle_dir,
                    &args.benchmark_evidence,
                    &args.cryptographic_review,
                )?;
                writeln!(writer, "{}", report.canonical_json()?)?;
            }
            Command::PromoteRelease(args) => {
                let verified = verify_release_directory(
                    &args.bundle_dir,
                    &embedded_policy_bytes()?,
                    &args.benchmark_evidence,
                    &args.cryptographic_review,
                )?;
                let record = verified.promotion_record();
                let record_bytes = norito::to_bytes(&record)
                    .wrap_err("failed to encode Kagemusha promotion record")?;
                write_new_durable_file(&args.promotion_record, &record_bytes)?;
                writeln!(writer, "{}", verified.report.canonical_json()?)?;
            }
            Command::VerifyReleaseV4(args) => {
                let policy_bytes = explicit_embedded_policy_bytes(&args.release_policy)?;
                let report = verify_release_directory_v4(
                    &args.bundle_dir,
                    &policy_bytes,
                    &args.benchmark_evidence,
                    &args.cryptographic_review,
                )?
                .report;
                writeln!(writer, "{}", report.canonical_json()?)?;
            }
            Command::PromoteReleaseV4(args) => {
                let policy_bytes = explicit_embedded_policy_bytes(&args.release_policy)?;
                let verified = verify_release_directory_v4(
                    &args.bundle_dir,
                    &policy_bytes,
                    &args.benchmark_evidence,
                    &args.cryptographic_review,
                )?;
                let record = verified.promotion_record();
                record.validate().map_err(|error| eyre!(error))?;
                let record_bytes = norito::to_bytes(&record)
                    .wrap_err("failed to encode Kagemusha V4 promotion record")?;
                write_new_durable_file(&args.promotion_record, &record_bytes)?;
                writeln!(writer, "{}", verified.report.canonical_json()?)?;
            }
        }
        Ok(())
    }
}

fn verify_with_embedded_policy(
    bundle_dir: &Path,
    benchmark_evidence: &Path,
    cryptographic_review: &Path,
) -> Result<VerificationReport> {
    let policy_bytes = embedded_policy_bytes()?;
    Ok(verify_release_directory(
        bundle_dir,
        &policy_bytes,
        benchmark_evidence,
        cryptographic_review,
    )?
    .report)
}

fn embedded_policy_bytes() -> Result<Vec<u8>> {
    let encoded = EMBEDDED_RELEASE_TRUST_ROOT_HEX.ok_or_else(|| {
        eyre!(
            "Kagami was built without {RELEASE_TRUST_ROOT_ENV}; authenticated Kagemusha release verification is unavailable"
        )
    })?;
    if encoded.is_empty()
        || encoded.len() > MAX_POLICY_BYTES * 2
        || encoded.len() % 2 != 0
        || !encoded
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        bail!("embedded Kagemusha release trust root is not canonical lowercase Norito hex");
    }
    let decoded = hex::decode(encoded).wrap_err("failed to decode embedded release trust root")?;
    if decoded.is_empty() || decoded.iter().all(|byte| *byte == 0) {
        bail!("embedded Kagemusha release trust root is empty or all zero");
    }
    Ok(decoded)
}

fn explicit_embedded_policy_bytes(path: &Path) -> Result<Vec<u8>> {
    let supplied = read_external_bounded(path, MAX_POLICY_BYTES, "Kagemusha V4 release policy")?;
    let supplied_policy: KagemushaRecursiveSpendReleasePolicyV1 =
        decode_canonical_norito(&supplied, "supplied Kagemusha V4 release policy")?;
    supplied_policy.validate().map_err(|error| eyre!(error))?;

    let embedded = embedded_policy_bytes()?;
    let embedded_policy: KagemushaRecursiveSpendReleasePolicyV1 =
        decode_canonical_norito(&embedded, "embedded Kagemusha V4 release trust root")?;
    embedded_policy.validate().map_err(|error| eyre!(error))?;
    if supplied != embedded || supplied_policy != embedded_policy {
        bail!(
            "supplied Kagemusha V4 release policy does not exactly match the embedded trust root"
        );
    }
    Ok(supplied)
}

struct VerifiedRelease {
    authenticated: KagemushaAuthenticatedReleaseV3,
    report: VerificationReport,
}

impl VerifiedRelease {
    /// Mint a marker only after the private end-to-end verifier has checked all seven artifacts.
    fn promotion_record(&self) -> KagemushaRecursiveSpendPromotedReleaseV3 {
        let capabilities = kagemusha_recursive_spend_native_capabilities_v1();
        KagemushaRecursiveSpendPromotedReleaseV3 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_PROMOTED_RELEASE_SCHEMA_V3.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V1,
            generation: self.authenticated.manifest().generation.clone(),
            manifest_sha256: self.authenticated.manifest_sha256(),
            release_attestation_sha256: self.authenticated.release_attestation_sha256(),
            release_policy_sha256: self.authenticated.release_policy_sha256(),
            approved_signers: self.authenticated.approved_signers().to_vec(),
            artifact_inventory_verified: true,
            proof_backend_available: capabilities.proof_backend_available,
            missing_gates: capabilities.missing_gates,
        }
    }
}

struct VerifiedReleaseV4 {
    authenticated: KagemushaAuthenticatedReleaseV4,
    report: VerificationReport,
}

impl VerifiedReleaseV4 {
    fn promotion_record(&self) -> KagemushaRecursiveSpendPromotedReleaseV4 {
        KagemushaRecursiveSpendPromotedReleaseV4 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_PROMOTED_RELEASE_SCHEMA_V4.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V4,
            generation: self.authenticated.manifest().generation.clone(),
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
        }
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
        decode_canonical_norito(policy_bytes, "Kagemusha release trust root")?;
    policy.validate().map_err(|error| eyre!(error))?;
    let trust_root_sha256 = kagemusha_recursive_spend_release_sha256(policy_bytes);

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

    let benchmark = read_external_evidence(benchmark_evidence_path, "physical-device benchmark")?;
    let review = read_external_evidence(cryptographic_review_path, "cryptographic review")?;
    let authenticated = KagemushaAuthenticatedReleaseV4::verify(
        &manifest,
        &policy,
        &attestation,
        &benchmark,
        &review,
    )
    .map_err(|error| eyre!("Kagemusha V4 release authentication failed: {error}"))?;

    let descriptors: Vec<&KagemushaPastaCycleArtifactV4> = manifest
        .profiles
        .iter()
        .flat_map(|profile| profile.artifacts.iter())
        .collect();
    if descriptors.len() != REPORT_ARTIFACT_PURPOSES_V4.len() {
        bail!("Kagemusha V4 release does not contain the exact eight-artifact inventory");
    }
    let mut validated = Vec::with_capacity(descriptors.len());
    for descriptor in &descriptors {
        let bytes = read_regular_bounded(
            &root,
            &descriptor.file_name,
            usize::try_from(descriptor.size_bytes)
                .map_err(|_| eyre!("Kagemusha V4 artifact size does not fit this host"))?,
            "Kagemusha V4 artifact",
        )?;
        if u64::try_from(bytes.len()).ok() != Some(descriptor.size_bytes) {
            bail!("Kagemusha V4 artifact size changed while it was read");
        }
        let payload = read_kagemusha_pasta_cycle_artifact_v4(
            &mut std::io::Cursor::new(bytes),
            &authenticated,
            descriptor,
        )
        .map_err(|error| eyre!(error))?;
        validated.push(payload);
    }
    let [
        step_eq_parameters,
        step_eq_proving_key,
        step_eq_verifying_key,
        step_eq_bootstrap_witness,
        step_ep_parameters,
        step_ep_proving_key,
        step_ep_verifying_key,
        step_ep_bootstrap_witness,
    ]: [KagemushaValidatedArtifactPayloadV4; 8] = validated
        .try_into()
        .map_err(|_| eyre!("Kagemusha V4 artifact inventory length changed"))?;

    for (bootstrap, profile, parity) in [
        (
            &step_eq_bootstrap_witness,
            &manifest.profiles[0],
            KagemushaPastaCycleParityV1::StepEq,
        ),
        (
            &step_ep_bootstrap_witness,
            &manifest.profiles[1],
            KagemushaPastaCycleParityV1::StepEp,
        ),
    ] {
        let measured = validate_kagemusha_step_bootstrap_payload_v4(
            bootstrap.payload(),
            &profile.circuit_params,
            parity,
            profile.compiled_protocol_structure_sha256,
        )
        .map_err(|error| eyre!(error))?;
        if u32::try_from(measured) != Ok(profile.step_proof_size_bytes) {
            bail!("Kagemusha V4 bootstrap proof size does not match its authenticated profile");
        }
    }

    let material = KagemushaPastaCycleProverArtifactsV4::new(
        &authenticated,
        step_eq_parameters,
        step_eq_proving_key,
        step_eq_verifying_key,
        step_eq_bootstrap_witness,
        step_ep_parameters,
        step_ep_proving_key,
        step_ep_verifying_key,
        step_ep_bootstrap_witness,
    )
    .map_err(|error| eyre!(error))?;
    if material.manifest_sha256() != manifest_sha256
        || material.max_proof_bytes() != manifest.max_proof_bytes
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
        trust_root_sha256,
        recursive_step_verifier_commitment,
    );
    Ok(VerifiedReleaseV4 {
        authenticated,
        report,
    })
}

fn verify_release_directory(
    bundle_dir: &Path,
    policy_bytes: &[u8],
    benchmark_evidence_path: &Path,
    cryptographic_review_path: &Path,
) -> Result<VerifiedRelease> {
    let root = canonical_release_root(bundle_dir)?;
    let policy: KagemushaRecursiveSpendReleasePolicyV1 =
        decode_canonical_norito(policy_bytes, "Kagemusha release trust root")?;
    policy.validate().map_err(|error| eyre!(error))?;
    let trust_root_sha256 = kagemusha_recursive_spend_release_sha256(policy_bytes);

    let manifest_bytes = read_regular_bounded(
        &root,
        MANIFEST_NORITO_FILE_NAME,
        MAX_MANIFEST_BYTES,
        "canonical Kagemusha manifest",
    )?;
    let manifest: KagemushaRecursiveSpendArtifactManifestV3 =
        decode_canonical_norito(&manifest_bytes, "canonical Kagemusha manifest")?;
    manifest.validate().map_err(|error| eyre!(error))?;
    let manifest_sha256 = kagemusha_recursive_spend_release_sha256(&manifest_bytes);

    let manifest_digest = read_regular_bounded(
        &root,
        MANIFEST_NORITO_SHA256_FILE_NAME,
        65,
        "Kagemusha manifest digest",
    )?;
    if manifest_digest != format!("{}\n", hex::encode(manifest_sha256)).as_bytes() {
        bail!("Kagemusha manifest digest sidecar does not match manifest.norito");
    }

    let manifest_json = read_regular_bounded(
        &root,
        MANIFEST_JSON_FILE_NAME,
        MAX_MANIFEST_BYTES,
        "Kagemusha manifest JSON",
    )?;
    let manifest_json = std::str::from_utf8(&manifest_json)
        .wrap_err("Kagemusha manifest JSON is not strict UTF-8")?;
    let manifest_from_json: KagemushaRecursiveSpendArtifactManifestV3 =
        norito::json::from_str(manifest_json)
            .wrap_err("Kagemusha manifest JSON is malformed or non-canonical in shape")?;
    if manifest_from_json != manifest {
        bail!("Kagemusha JSON and Norito manifests are not semantically identical");
    }

    let attestation_bytes = read_regular_bounded(
        &root,
        RELEASE_ATTESTATION_FILE_NAME,
        MAX_ATTESTATION_BYTES,
        "Kagemusha release attestation",
    )?;
    let attestation: KagemushaRecursiveSpendReleaseAttestationV1 =
        decode_canonical_norito(&attestation_bytes, "Kagemusha release attestation")?;

    let benchmark = read_external_evidence(benchmark_evidence_path, "physical-device benchmark")?;
    let review = read_external_evidence(cryptographic_review_path, "cryptographic review")?;
    let authenticated = KagemushaAuthenticatedReleaseV3::verify(
        &manifest,
        &policy,
        &attestation,
        &benchmark,
        &review,
    )
    .map_err(|error| eyre!("Kagemusha release authentication failed: {error}"))?;

    let descriptors: Vec<&KagemushaPastaCycleArtifactV3> = manifest
        .profiles
        .iter()
        .flat_map(|profile| profile.artifacts.iter())
        .collect();
    if descriptors.len() != 6 {
        bail!("Kagemusha release does not contain the exact six-key inventory");
    }
    let mut validated = Vec::with_capacity(descriptors.len());
    for descriptor in &descriptors {
        let bytes = read_regular_bounded(
            &root,
            &descriptor.file_name,
            usize::try_from(descriptor.size_bytes)
                .map_err(|_| eyre!("Kagemusha artifact size does not fit this host"))?,
            "Kagemusha key artifact",
        )?;
        if u64::try_from(bytes.len()).ok() != Some(descriptor.size_bytes) {
            bail!("Kagemusha artifact size changed while it was read");
        }
        let payload = read_kagemusha_pasta_cycle_artifact_v3(
            &mut std::io::Cursor::new(bytes),
            &manifest,
            descriptor,
        )
        .map_err(|error| eyre!(error))?;
        validated.push(payload);
    }
    let [
        step_eq_parameters,
        step_eq_proving_key,
        step_eq_verifying_key,
        step_ep_parameters,
        step_ep_proving_key,
        step_ep_verifying_key,
    ]: [KagemushaValidatedArtifactPayloadV3; 6] = validated
        .try_into()
        .map_err(|_| eyre!("Kagemusha key inventory length changed"))?;
    let material = KagemushaPastaCycleProverArtifactsV3::new(
        &manifest,
        step_eq_parameters,
        step_eq_proving_key,
        step_eq_verifying_key,
        step_ep_parameters,
        step_ep_proving_key,
        step_ep_verifying_key,
    )
    .map_err(|error| eyre!(error))?;
    if material.manifest_sha256() != manifest_sha256 {
        bail!("authenticated Kagemusha material does not bind the canonical manifest");
    }

    verify_roster(&root, &manifest)?;
    verify_exact_inventory(&root, &manifest)?;

    let subject = manifest
        .release_attestation_subject()
        .map_err(|error| eyre!(error))?;
    let recursive_step_verifier_commitment = recursive_step_verifier_commitment(&manifest)?;
    let report = VerificationReport::from_manifest(
        &manifest,
        manifest_sha256,
        subject.manifest_subject_sha256,
        trust_root_sha256,
        recursive_step_verifier_commitment,
    );
    Ok(VerifiedRelease {
        authenticated,
        report,
    })
}

fn verify_roster(root: &Path, manifest: &KagemushaRecursiveSpendArtifactManifestV3) -> Outcome {
    let descriptor = &manifest.topup_finality_roster_artifact;
    let bytes = read_regular_bounded(
        root,
        &descriptor.file_name,
        usize::try_from(KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_V2)
            .expect("roster bound fits usize"),
        "Kagemusha top-up finality roster",
    )?;
    if u64::try_from(bytes.len()).ok() != Some(descriptor.size_bytes)
        || kagemusha_recursive_spend_release_sha256(&bytes) != descriptor.sha256
    {
        bail!("Kagemusha top-up finality roster size or digest mismatch");
    }
    let roster: KagemushaTopUpFinalityRosterArtifactV2 =
        decode_canonical_norito(&bytes, "Kagemusha top-up finality roster")?;
    roster.validate().map_err(|error| eyre!(error))?;
    if roster.chain_id != manifest.chain_id
        || roster.artifact_generation != manifest.generation
        || roster.window_at(manifest.activation_height).is_err()
        || roster
            .window_at(manifest.withdrawal_height.saturating_sub(1))
            .is_err()
    {
        bail!("Kagemusha top-up finality roster release binding mismatch");
    }
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

fn verify_exact_inventory(
    root: &Path,
    manifest: &KagemushaRecursiveSpendArtifactManifestV3,
) -> Outcome {
    let mut expected = std::collections::BTreeSet::from([
        MANIFEST_JSON_FILE_NAME.to_owned(),
        MANIFEST_NORITO_FILE_NAME.to_owned(),
        MANIFEST_NORITO_SHA256_FILE_NAME.to_owned(),
        RELEASE_ATTESTATION_FILE_NAME.to_owned(),
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
        .wrap_err("failed to enumerate Kagemusha release directory")?
        .map(|entry| {
            entry
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .wrap_err("failed to inspect Kagemusha release inventory")
        })
        .collect::<Result<std::collections::BTreeSet<_>>>()?;
    if observed != expected {
        bail!(
            "Kagemusha release inventory is not exact (missing={:?}, unexpected={:?})",
            expected.difference(&observed).collect::<Vec<_>>(),
            observed.difference(&expected).collect::<Vec<_>>()
        );
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

fn recursive_step_verifier_commitment(
    manifest: &KagemushaRecursiveSpendArtifactManifestV3,
) -> Result<[u8; 32]> {
    let profiles = norito::to_bytes(&manifest.profiles)
        .wrap_err("failed to encode Kagemusha verifier profiles")?;
    let mut preimage =
        Vec::with_capacity(RECURSIVE_STEP_VERIFIER_COMMITMENT_DOMAIN.len() + 1 + profiles.len());
    preimage.extend_from_slice(RECURSIVE_STEP_VERIFIER_COMMITMENT_DOMAIN);
    preimage.push(0);
    preimage.extend_from_slice(&profiles);
    Ok(kagemusha_recursive_spend_release_sha256(&preimage))
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

fn read_regular_bounded(root: &Path, name: &str, max_bytes: usize, label: &str) -> Result<Vec<u8>> {
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
    let mut file = File::open(&path).wrap_err_with(|| format!("failed to open {label}"))?;
    if !same_file_snapshot(
        &before,
        &file.metadata().wrap_err("failed to inspect open file")?,
    ) {
        bail!("{label} changed while it was opened");
    }
    let mut bytes = Vec::with_capacity(length);
    Read::by_ref(&mut file)
        .take(u64::try_from(max_bytes).expect("file bound fits u64") + 1)
        .read_to_end(&mut bytes)
        .wrap_err_with(|| format!("failed to read {label}"))?;
    let after = file.metadata().wrap_err("failed to re-inspect open file")?;
    if bytes.len() != length || bytes.len() > max_bytes || !same_file_snapshot(&before, &after) {
        bail!("{label} changed while it was read");
    }
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

fn read_external_evidence(path: &Path, label: &str) -> Result<Vec<u8>> {
    read_external_bounded(
        path,
        KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1,
        label,
    )
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

#[derive(Debug, crate::json_macros::JsonSerialize)]
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
    trust_root_sha256: String,
    generation: String,
    chain_id: String,
    asset_definition_id: String,
    asset_scale: u32,
    bridge_abi_version: u32,
    recursive_step_verifier_commitment: String,
    artifacts: Vec<VerificationArtifact>,
}

impl VerificationReport {
    fn from_manifest(
        manifest: &KagemushaRecursiveSpendArtifactManifestV3,
        envelope_sha256: [u8; 32],
        manifest_body_sha256: [u8; 32],
        trust_root_sha256: [u8; 32],
        recursive_step_verifier_commitment: [u8; 32],
    ) -> Self {
        let mut artifacts = manifest
            .profiles
            .iter()
            .flat_map(|profile| profile.artifacts.iter())
            .zip(REPORT_ARTIFACT_PURPOSES)
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
            trust_root_sha256: hex::encode(trust_root_sha256),
            generation: manifest.generation.clone(),
            chain_id: manifest.chain_id.to_string(),
            asset_definition_id: manifest.asset.to_string(),
            asset_scale: manifest.asset_scale,
            bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3,
            recursive_step_verifier_commitment: hex::encode(recursive_step_verifier_commitment),
            artifacts,
        }
    }

    fn from_manifest_v4(
        manifest: &KagemushaRecursiveSpendArtifactManifestV4,
        envelope_sha256: [u8; 32],
        manifest_body_sha256: [u8; 32],
        trust_root_sha256: [u8; 32],
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
            trust_root_sha256: hex::encode(trust_root_sha256),
            generation: manifest.generation.clone(),
            chain_id: manifest.chain_id.to_string(),
            asset_definition_id: manifest.asset.to_string(),
            asset_scale: manifest.asset_scale,
            bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
            recursive_step_verifier_commitment: hex::encode(recursive_step_verifier_commitment),
            artifacts,
        }
    }

    fn canonical_json(&self) -> Result<String> {
        norito::json::to_json(self).wrap_err("failed to encode Kagemusha verification JSON")
    }
}

#[cfg(test)]
mod tests {
    use super::{
        REPORT_ARTIFACT_PURPOSES, REPORT_ARTIFACT_PURPOSES_V4, REPORT_ROSTER_PURPOSE,
        VerificationArtifact, VerificationReport,
    };

    #[test]
    fn verification_report_is_one_canonical_ordered_json_line() {
        let report = VerificationReport {
            status: "verified".to_owned(),
            envelope_sha256: "11".repeat(32),
            manifest_body_sha256: "22".repeat(32),
            trust_root_sha256: "33".repeat(32),
            generation: "release-1".to_owned(),
            chain_id: "chain".to_owned(),
            asset_definition_id: "asset".to_owned(),
            asset_scale: 2,
            bridge_abi_version: 19,
            recursive_step_verifier_commitment: "44".repeat(32),
            artifacts: vec![VerificationArtifact {
                purpose: REPORT_ARTIFACT_PURPOSES[0].to_owned(),
                file_name: "step-eq.parameters.krv3".to_owned(),
                size_bytes: 9,
                sha256: "55".repeat(32),
                payload_size_bytes: Some(3),
                payload_sha256: Some("66".repeat(32)),
            }],
        };
        let json = report.canonical_json().expect("canonical report JSON");
        assert!(!json.contains('\n') && !json.contains('\r'));
        assert_eq!(
            json,
            format!(
                concat!(
                    "{{\"status\":\"verified\",",
                    "\"envelope_sha256\":\"{}\",",
                    "\"manifest_body_sha256\":\"{}\",",
                    "\"trust_root_sha256\":\"{}\",",
                    "\"generation\":\"release-1\",",
                    "\"chain_id\":\"chain\",",
                    "\"asset_definition_id\":\"asset\",",
                    "\"asset_scale\":2,",
                    "\"bridge_abi_version\":19,",
                    "\"recursive_step_verifier_commitment\":\"{}\",",
                    "\"artifacts\":[{{",
                    "\"purpose\":\"step_eq_parameters\",",
                    "\"file_name\":\"step-eq.parameters.krv3\",",
                    "\"size_bytes\":9,",
                    "\"sha256\":\"{}\",",
                    "\"payload_size_bytes\":3,",
                    "\"payload_sha256\":\"{}\"",
                    "}}]}}"
                ),
                "11".repeat(32),
                "22".repeat(32),
                "33".repeat(32),
                "44".repeat(32),
                "55".repeat(32),
                "66".repeat(32),
            )
        );
        assert_eq!(
            REPORT_ARTIFACT_PURPOSES,
            [
                "step_eq_parameters",
                "step_eq_proving_key",
                "step_eq_verifying_key",
                "step_ep_parameters",
                "step_ep_proving_key",
                "step_ep_verifying_key",
            ]
        );
        assert_eq!(REPORT_ROSTER_PURPOSE, "topup_finality_roster");
    }

    #[test]
    fn v4_report_inventory_is_the_canonical_eight_role_abi20_order() {
        assert_eq!(
            REPORT_ARTIFACT_PURPOSES_V4,
            [
                "step_eq_parameters",
                "step_eq_proving_key",
                "step_eq_verifying_key",
                "step_eq_bootstrap_witness",
                "step_ep_parameters",
                "step_ep_proving_key",
                "step_ep_verifying_key",
                "step_ep_bootstrap_witness",
            ]
        );
        assert_eq!(REPORT_ARTIFACT_PURPOSES_V4.len(), 8);
    }
}
