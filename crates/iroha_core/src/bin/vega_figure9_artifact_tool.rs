//! Native qualification boundary for governed Vega Figure 9 artifacts.
//!
//! This binary never generates setup material and never discovers keys through
//! an environment variable, working directory, or network service.  It accepts
//! exactly one explicit owner-only PK/VK pair, authenticates both complete
//! files, invokes Core's strict canonical pair installer, executes all four
//! deterministic native release stages, and writes their canonical public
//! Norito evidence into one explicit empty owner-only directory. The packaging
//! controller hashes this binary independently and retains that digest beside
//! the deterministic validation report and evidence set.

use iroha_core::privacy_engines::vega::{
    VEGA_MDL_FIGURE9_ARTIFACT_MANIFEST_SCHEMA_V1,
    VEGA_MDL_FIGURE9_ARTIFACT_MANIFEST_SCHEMA_VERSION_V1,
    VEGA_MDL_FIGURE9_KEY_ARTIFACT_MAX_BYTES_V1, VegaMdlFigure9ArtifactBindingV1,
    VegaMdlFigure9ArtifactManifestV1, VegaMdlFigure9ArtifactRoleV1,
    VegaMdlFigure9ArtifactSourceErrorV1, VegaMdlFigure9ProverArtifactSourceV1,
    qualify_and_install_vega_mdl_figure9_prover_artifacts_v1,
};
use iroha_core::privacy_release_evidence::{
    PrivacyReleaseCaseKindV1, PrivacyReleaseFailureClassV1, PrivacyReleaseResourceFactsV1,
    PrivacyReleaseStageEvidenceV1, initialize_privacy_release_rayon_pool_v1,
    privacy_release_stage_evidence_sha256_v1, run_privacy_release_stage_v1,
    validate_privacy_release_stage_evidence_v1,
};
use iroha_data_model::privacy::PrivacyProtocolIdV1;
use norito::derive::JsonSerialize;
use sha2::{Digest as _, Sha256};
use std::{
    env,
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
    process::ExitCode,
};
use zeroize::Zeroizing;

const REPORT_SCHEMA: &str = "iroha.vega.figure9.native-artifact-validation";
const REPORT_SCHEMA_VERSION: u8 = 2;
const VALIDATOR_ROLE: &str = "prover-pair-and-four-case-release-evidence";
const RELEASE_QUALIFICATION: &str = "passed-native-four-case";
const EVIDENCE_SET_DOMAIN: &[u8] = b"iroha.vega.figure9.release-evidence-set.v1\0";
const RELEASED_CARGO_LOCK_SHA256: &str =
    "179f589da420c024725efd9a65adb9c1e34085fa022cc01a8c67bb2262e93bf7";
const MAX_EVIDENCE_ARCHIVE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_PATH_BYTES: usize = 4_096;

const BUILD_SIGNED_SOURCE_COMMIT: Option<&str> = option_env!("IROHA_VEGA_SIGNED_SOURCE_COMMIT");
const BUILD_WORKSPACE_SOURCE_MANIFEST_SHA256: Option<&str> =
    option_env!("IROHA_VEGA_WORKSPACE_SOURCE_MANIFEST_SHA256");
const BUILD_CARGO_LOCK_SHA256: Option<&str> = option_env!("IROHA_VEGA_CARGO_LOCK_SHA256");
const BUILD_SOURCE_ALLOWED_SIGNERS_SHA256: Option<&str> =
    option_env!("IROHA_VEGA_SOURCE_ALLOWED_SIGNERS_SHA256");
const BUILD_SOURCE_REVOCATION_SHA256: Option<&str> =
    option_env!("IROHA_VEGA_SOURCE_REVOCATION_SHA256");

#[derive(JsonSerialize)]
struct ArtifactIdentityV1 {
    exact_byte_len: u64,
    raw_canonical_sha256: String,
    role: String,
}

#[derive(JsonSerialize)]
struct EvidenceProofIdentityV1 {
    artifact_ordinal: u8,
    canonical_proof_exact_byte_len: u64,
    proof_bytes_ceiling: u64,
    proof_sha256: String,
}

#[derive(Clone, Copy, JsonSerialize)]
struct EvidenceResourceFactsV1 {
    primary_ceiling: u64,
    primary_units: u64,
    relation_depth: u64,
    relation_depth_ceiling: u64,
    secondary_ceiling: u64,
    secondary_units: u64,
}

#[derive(JsonSerialize)]
struct EvidenceArchiveIdentityV1 {
    archive_sha256: String,
    case_kind: String,
    exact_byte_len: u64,
    file_name: String,
    failure_class: String,
    proof_artifacts: Vec<EvidenceProofIdentityV1>,
    protocol_id: String,
    public_statement_sha256: String,
    resources: EvidenceResourceFactsV1,
    stage_ordinal: u16,
}

#[derive(JsonSerialize)]
struct NativeValidationReportV1 {
    artifact_manifest_schema: String,
    artifact_manifest_schema_version: u16,
    artifact_manifest_sha256: String,
    canonical_relation_digest: String,
    cargo_lock_sha256: String,
    compiled_profile_digest: String,
    evidence: Vec<EvidenceArchiveIdentityV1>,
    evidence_set_sha256: String,
    iroha_signed_source_commit: String,
    logical_governed_verifier_digest: String,
    proving_key: ArtifactIdentityV1,
    release_qualification: String,
    schema: String,
    schema_version: u8,
    source_allowed_signers_sha256: String,
    source_revocation_sha256: String,
    upstream_source_commit: String,
    upstream_source_tree: String,
    validator_arch: String,
    validator_os: String,
    validator_role: String,
    vendor_manifest_sha256: String,
    verifier_key: ArtifactIdentityV1,
    workspace_source_manifest_sha256: String,
}

struct StagedEvidenceArchiveV1 {
    bytes: Zeroizing<Vec<u8>>,
    identity: EvidenceArchiveIdentityV1,
}

struct InMemoryProverArtifactSourceV1<'a> {
    manifest: VegaMdlFigure9ArtifactManifestV1,
    proving_key: &'a [u8],
    verifier_key: &'a [u8],
}

impl VegaMdlFigure9ProverArtifactSourceV1 for InMemoryProverArtifactSourceV1<'_> {
    fn artifact_manifest(&self) -> &VegaMdlFigure9ArtifactManifestV1 {
        &self.manifest
    }

    fn with_prover_artifacts(
        &self,
        consume: &mut dyn FnMut(&[u8], &[u8]) -> Result<(), VegaMdlFigure9ArtifactSourceErrorV1>,
    ) -> Result<(), VegaMdlFigure9ArtifactSourceErrorV1> {
        consume(self.proving_key, self.verifier_key)
    }
}

fn lowercase_hex(value: Option<&'static str>, bytes: usize, label: &str) -> Result<String, String> {
    let value = value.ok_or_else(|| format!("build omitted {label}"))?;
    if value.len() != bytes.saturating_mul(2)
        || value.bytes().all(|byte| byte == b'0')
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("build embedded an invalid {label}"));
    }
    Ok(value.to_owned())
}

fn canonical_path(path: PathBuf, label: &str) -> Result<PathBuf, String> {
    if !path.is_absolute() || path.as_os_str().as_encoded_bytes().len() > MAX_PATH_BYTES {
        return Err(format!("{label} path must be bounded and absolute"));
    }
    let canonical =
        fs::canonicalize(&path).map_err(|error| format!("cannot resolve {label} path: {error}"))?;
    if canonical != path {
        return Err(format!("{label} path must already be canonical"));
    }
    let metadata = fs::symlink_metadata(&canonical)
        .map_err(|error| format!("cannot inspect {label}: {error}"))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(format!("{label} must be one regular non-symbolic file"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        if metadata.mode() & 0o077 != 0 || metadata.nlink() != 1 {
            return Err(format!("{label} must be owner-only and have one link"));
        }
    }
    Ok(canonical)
}

fn canonical_empty_directory(path: PathBuf, label: &str) -> Result<PathBuf, String> {
    if !path.is_absolute() || path.as_os_str().as_encoded_bytes().len() > MAX_PATH_BYTES {
        return Err(format!("{label} path must be bounded and absolute"));
    }
    let canonical =
        fs::canonicalize(&path).map_err(|error| format!("cannot resolve {label} path: {error}"))?;
    if canonical != path {
        return Err(format!("{label} path must already be canonical"));
    }
    let metadata = fs::symlink_metadata(&canonical)
        .map_err(|error| format!("cannot inspect {label}: {error}"))?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Err(format!("{label} must be one non-symbolic directory"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        if metadata.mode() & 0o077 != 0 {
            return Err(format!("{label} must be owner-only"));
        }
    }
    if fs::read_dir(&canonical)
        .map_err(|error| format!("cannot enumerate {label}: {error}"))?
        .next()
        .is_some()
    {
        return Err(format!("{label} must be empty"));
    }
    Ok(canonical)
}

fn read_bounded_artifact(path: &Path, label: &str) -> Result<Zeroizing<Vec<u8>>, String> {
    let before = fs::metadata(path).map_err(|error| format!("cannot inspect {label}: {error}"))?;
    let len = before.len();
    if len == 0 || len > VEGA_MDL_FIGURE9_KEY_ARTIFACT_MAX_BYTES_V1 {
        return Err(format!("{label} length is outside the released bound"));
    }
    let capacity =
        usize::try_from(len).map_err(|_| format!("{label} length is not addressable"))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(capacity)
        .map_err(|_| format!("cannot reserve bounded storage for {label}"))?;
    let mut file = File::open(path).map_err(|error| format!("cannot open {label}: {error}"))?;
    std::io::Read::by_ref(&mut file)
        .take(VEGA_MDL_FIGURE9_KEY_ARTIFACT_MAX_BYTES_V1 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read {label}: {error}"))?;
    let after = file
        .metadata()
        .map_err(|error| format!("cannot re-inspect {label}: {error}"))?;
    if u64::try_from(bytes.len()).ok() != Some(len) || before.len() != after.len() {
        return Err(format!("{label} changed while it was being authenticated"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        if before.dev() != after.dev()
            || before.ino() != after.ino()
            || before.mtime() != after.mtime()
            || before.mtime_nsec() != after.mtime_nsec()
            || before.mode() != after.mode()
            || before.nlink() != after.nlink()
        {
            return Err(format!(
                "{label} identity changed while it was being authenticated"
            ));
        }
    }
    Ok(Zeroizing::new(bytes))
}

fn artifact_binding(
    role: VegaMdlFigure9ArtifactRoleV1,
    bytes: &[u8],
) -> Result<VegaMdlFigure9ArtifactBindingV1, String> {
    VegaMdlFigure9ArtifactBindingV1::new(
        role,
        u64::try_from(bytes.len()).map_err(|_| "artifact length overflow".to_owned())?,
        Sha256::digest(bytes).into(),
    )
    .map_err(|error| format!("cannot bind {role:?} artifact: {error}"))
}

fn ascii40(value: &[u8; 40], label: &str) -> Result<String, String> {
    let value = std::str::from_utf8(value).map_err(|_| format!("{label} is not ASCII"))?;
    if value.len() != 40
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("{label} is not canonical lowercase hexadecimal"));
    }
    Ok(value.to_owned())
}

fn identity(
    manifest: &VegaMdlFigure9ArtifactManifestV1,
    role: VegaMdlFigure9ArtifactRoleV1,
) -> ArtifactIdentityV1 {
    let binding = manifest.artifact(role);
    ArtifactIdentityV1 {
        exact_byte_len: binding.exact_byte_len(),
        raw_canonical_sha256: hex::encode(binding.raw_canonical_sha256()),
        role: match role {
            VegaMdlFigure9ArtifactRoleV1::ProvingKey => "proving-key",
            VegaMdlFigure9ArtifactRoleV1::VerifierKey => "verifier-key",
        }
        .to_owned(),
    }
}

fn evidence_file_name(case_kind: PrivacyReleaseCaseKindV1) -> &'static str {
    match case_kind {
        PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd => {
            "vega-evidence-16-positive-canonical-end-to-end.norito"
        }
        PrivacyReleaseCaseKindV1::PublicStatementBindingMutation => {
            "vega-evidence-17-public-statement-binding-mutation.norito"
        }
        PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation => {
            "vega-evidence-18-proof-corruption-and-truncation.norito"
        }
        PrivacyReleaseCaseKindV1::MaximumShapeResource => {
            "vega-evidence-19-maximum-shape-resource.norito"
        }
    }
}

fn failure_class_label(failure_class: PrivacyReleaseFailureClassV1) -> &'static str {
    match failure_class {
        PrivacyReleaseFailureClassV1::NotApplicable => "not-applicable",
        PrivacyReleaseFailureClassV1::PublicStatementBindingRejected => {
            "public-statement-binding-rejected"
        }
        PrivacyReleaseFailureClassV1::CanonicalWireCorruptionAndTruncationRejected => {
            "canonical-wire-corruption-and-truncation-rejected"
        }
    }
}

impl From<PrivacyReleaseResourceFactsV1> for EvidenceResourceFactsV1 {
    fn from(resources: PrivacyReleaseResourceFactsV1) -> Self {
        Self {
            primary_ceiling: resources.primary_ceiling,
            primary_units: resources.primary_units,
            relation_depth: resources.relation_depth,
            relation_depth_ceiling: resources.relation_depth_ceiling,
            secondary_ceiling: resources.secondary_ceiling,
            secondary_units: resources.secondary_units,
        }
    }
}

fn stage_evidence(case_kind: PrivacyReleaseCaseKindV1) -> Result<StagedEvidenceArchiveV1, String> {
    let evidence =
        run_privacy_release_stage_v1(PrivacyProtocolIdV1::VegaExistingCredentialZkV0, case_kind)
            .map_err(|error| {
                format!(
                    "native Figure 9 {} stage failed: {error}",
                    case_kind.canonical_label()
                )
            })?;
    if evidence.protocol_id != PrivacyProtocolIdV1::VegaExistingCredentialZkV0
        || evidence.case_kind != case_kind
        || !validate_privacy_release_stage_evidence_v1(&evidence)
    {
        return Err(format!(
            "native Figure 9 {} stage returned invalid evidence",
            case_kind.canonical_label()
        ));
    }
    let bytes = norito::encode_canonical(&evidence).map_err(|error| {
        format!(
            "cannot canonically encode Figure 9 {} evidence: {error}",
            case_kind.canonical_label()
        )
    })?;
    let exact_byte_len =
        u64::try_from(bytes.len()).map_err(|_| "Figure 9 evidence length overflowed".to_owned())?;
    if exact_byte_len == 0 || exact_byte_len > MAX_EVIDENCE_ARCHIVE_BYTES {
        return Err(format!(
            "native Figure 9 {} evidence exceeded its archive bound",
            case_kind.canonical_label()
        ));
    }
    let decoded: PrivacyReleaseStageEvidenceV1 =
        norito::decode_canonical(&bytes).map_err(|error| {
            format!(
                "cannot canonically replay Figure 9 {} evidence: {error}",
                case_kind.canonical_label()
            )
        })?;
    if decoded != evidence || !validate_privacy_release_stage_evidence_v1(&decoded) {
        return Err(format!(
            "native Figure 9 {} evidence did not replay exactly",
            case_kind.canonical_label()
        ));
    }
    let archive_sha256 = privacy_release_stage_evidence_sha256_v1(&evidence).ok_or_else(|| {
        format!(
            "native Figure 9 {} evidence has no canonical release digest",
            case_kind.canonical_label()
        )
    })?;
    let encoded_sha256: [u8; 32] = Sha256::digest(&bytes).into();
    if archive_sha256 != encoded_sha256 {
        return Err(format!(
            "native Figure 9 {} evidence digest did not bind its canonical archive",
            case_kind.canonical_label()
        ));
    }
    let proof_artifacts = evidence
        .proof_artifacts
        .iter()
        .map(|artifact| {
            Ok(EvidenceProofIdentityV1 {
                artifact_ordinal: artifact.artifact_ordinal,
                canonical_proof_exact_byte_len: u64::try_from(artifact.canonical_proof_bytes.len())
                    .map_err(|_| "Figure 9 proof length overflowed".to_owned())?,
                proof_bytes_ceiling: artifact.proof_bytes_ceiling,
                proof_sha256: hex::encode(artifact.proof_sha256),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let identity = EvidenceArchiveIdentityV1 {
        archive_sha256: hex::encode(archive_sha256),
        case_kind: case_kind.canonical_label().to_owned(),
        exact_byte_len,
        file_name: evidence_file_name(case_kind).to_owned(),
        failure_class: failure_class_label(evidence.failure_class).to_owned(),
        proof_artifacts,
        protocol_id: evidence.protocol_id.canonical_label().to_owned(),
        public_statement_sha256: hex::encode(evidence.public_statement_sha256),
        resources: evidence.resources.into(),
        stage_ordinal: evidence.stage_ordinal,
    };
    Ok(StagedEvidenceArchiveV1 {
        bytes: Zeroizing::new(bytes),
        identity,
    })
}

fn evidence_set_sha256(evidence: &[EvidenceArchiveIdentityV1]) -> Result<[u8; 32], String> {
    let mut hasher = Sha256::new();
    hasher.update(EVIDENCE_SET_DOMAIN);
    for archive in evidence {
        let file_name_len = u64::try_from(archive.file_name.len())
            .map_err(|_| "Figure 9 evidence file name length overflowed".to_owned())?;
        let archive_sha256 = hex::decode(&archive.archive_sha256)
            .map_err(|_| "Figure 9 evidence archive SHA-256 was not hexadecimal".to_owned())?;
        if archive_sha256.len() != 32 {
            return Err("Figure 9 evidence archive SHA-256 had the wrong length".to_owned());
        }
        hasher.update(archive.stage_ordinal.to_be_bytes());
        hasher.update(file_name_len.to_be_bytes());
        hasher.update(archive.file_name.as_bytes());
        hasher.update(archive.exact_byte_len.to_be_bytes());
        hasher.update(archive_sha256);
    }
    Ok(hasher.finalize().into())
}

fn write_evidence_archives(
    output: &Path,
    archives: &[StagedEvidenceArchiveV1],
) -> Result<(), String> {
    for archive in archives {
        let destination = output.join(&archive.identity.file_name);
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        let mut file = options.open(&destination).map_err(|error| {
            format!(
                "cannot create Figure 9 evidence {}: {error}",
                archive.identity.file_name
            )
        })?;
        file.write_all(&archive.bytes).map_err(|error| {
            format!(
                "cannot write Figure 9 evidence {}: {error}",
                archive.identity.file_name
            )
        })?;
        file.sync_all().map_err(|error| {
            format!(
                "cannot sync Figure 9 evidence {}: {error}",
                archive.identity.file_name
            )
        })?;
        #[cfg(unix)]
        fs::set_permissions(&destination, {
            use std::os::unix::fs::PermissionsExt as _;
            fs::Permissions::from_mode(0o400)
        })
        .map_err(|error| {
            format!(
                "cannot seal Figure 9 evidence {}: {error}",
                archive.identity.file_name
            )
        })?;
    }
    File::open(output)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("cannot sync Figure 9 evidence directory: {error}"))
}

fn qualify_release(
    proving_key_path: PathBuf,
    verifier_key_path: PathBuf,
    evidence_output_path: PathBuf,
) -> Result<(), String> {
    let signed_source_commit =
        lowercase_hex(BUILD_SIGNED_SOURCE_COMMIT, 20, "signed source commit")?;
    let workspace_source_manifest_sha256 = lowercase_hex(
        BUILD_WORKSPACE_SOURCE_MANIFEST_SHA256,
        32,
        "workspace source manifest SHA-256",
    )?;
    let cargo_lock_sha256 = lowercase_hex(BUILD_CARGO_LOCK_SHA256, 32, "Cargo.lock SHA-256")?;
    if cargo_lock_sha256 != RELEASED_CARGO_LOCK_SHA256 {
        return Err("build embedded an unreviewed Cargo.lock SHA-256".to_owned());
    }
    let source_allowed_signers_sha256 = lowercase_hex(
        BUILD_SOURCE_ALLOWED_SIGNERS_SHA256,
        32,
        "source allowed-signers SHA-256",
    )?;
    let source_revocation_sha256 = lowercase_hex(
        BUILD_SOURCE_REVOCATION_SHA256,
        32,
        "source revocation SHA-256",
    )?;

    let proving_key_path = canonical_path(proving_key_path, "Vega proving key")?;
    let verifier_key_path = canonical_path(verifier_key_path, "Vega verifier key")?;
    let evidence_output = canonical_empty_directory(evidence_output_path, "Vega evidence output")?;
    if proving_key_path == verifier_key_path {
        return Err("Vega proving and verifier keys must be distinct files".to_owned());
    }
    let proving_key = read_bounded_artifact(&proving_key_path, "Vega proving key")?;
    let verifier_key = read_bounded_artifact(&verifier_key_path, "Vega verifier key")?;
    initialize_privacy_release_rayon_pool_v1().map_err(|error| {
        format!("cannot establish the Figure 9 release worker topology: {error}")
    })?;
    let manifest = VegaMdlFigure9ArtifactManifestV1::new(
        artifact_binding(VegaMdlFigure9ArtifactRoleV1::ProvingKey, &proving_key)?,
        artifact_binding(VegaMdlFigure9ArtifactRoleV1::VerifierKey, &verifier_key)?,
    )
    .map_err(|error| format!("cannot construct governed artifact manifest: {error}"))?;
    let source = InMemoryProverArtifactSourceV1 {
        manifest,
        proving_key: &proving_key,
        verifier_key: &verifier_key,
    };
    let receipt = qualify_and_install_vega_mdl_figure9_prover_artifacts_v1(&source)
        .map_err(|error| format!("native Figure 9 PK/VK qualification failed: {error}"))?;
    let manifest = receipt.manifest();
    let archives = PrivacyReleaseCaseKindV1::ALL
        .into_iter()
        .map(stage_evidence)
        .collect::<Result<Vec<_>, String>>()?;
    let evidence = archives
        .iter()
        .map(|archive| EvidenceArchiveIdentityV1 {
            archive_sha256: archive.identity.archive_sha256.clone(),
            case_kind: archive.identity.case_kind.clone(),
            exact_byte_len: archive.identity.exact_byte_len,
            file_name: archive.identity.file_name.clone(),
            failure_class: archive.identity.failure_class.clone(),
            proof_artifacts: archive
                .identity
                .proof_artifacts
                .iter()
                .map(|proof| EvidenceProofIdentityV1 {
                    artifact_ordinal: proof.artifact_ordinal,
                    canonical_proof_exact_byte_len: proof.canonical_proof_exact_byte_len,
                    proof_bytes_ceiling: proof.proof_bytes_ceiling,
                    proof_sha256: proof.proof_sha256.clone(),
                })
                .collect(),
            protocol_id: archive.identity.protocol_id.clone(),
            public_statement_sha256: archive.identity.public_statement_sha256.clone(),
            resources: archive.identity.resources,
            stage_ordinal: archive.identity.stage_ordinal,
        })
        .collect::<Vec<_>>();
    let evidence_set_sha256 = hex::encode(evidence_set_sha256(&evidence)?);
    write_evidence_archives(&evidence_output, &archives)?;
    let report = NativeValidationReportV1 {
        artifact_manifest_schema: VEGA_MDL_FIGURE9_ARTIFACT_MANIFEST_SCHEMA_V1.to_owned(),
        artifact_manifest_schema_version: VEGA_MDL_FIGURE9_ARTIFACT_MANIFEST_SCHEMA_VERSION_V1,
        artifact_manifest_sha256: hex::encode(receipt.manifest_sha256()),
        canonical_relation_digest: hex::encode(manifest.canonical_relation_digest()),
        cargo_lock_sha256,
        compiled_profile_digest: hex::encode(manifest.compiled_profile_digest()),
        evidence,
        evidence_set_sha256,
        iroha_signed_source_commit: signed_source_commit,
        logical_governed_verifier_digest: hex::encode(manifest.logical_governed_verifier_digest()),
        proving_key: identity(manifest, VegaMdlFigure9ArtifactRoleV1::ProvingKey),
        release_qualification: RELEASE_QUALIFICATION.to_owned(),
        schema: REPORT_SCHEMA.to_owned(),
        schema_version: REPORT_SCHEMA_VERSION,
        source_allowed_signers_sha256,
        source_revocation_sha256,
        upstream_source_commit: ascii40(
            manifest.upstream_source_commit(),
            "upstream source commit",
        )?,
        upstream_source_tree: ascii40(manifest.upstream_source_tree(), "upstream source tree")?,
        validator_arch: env::consts::ARCH.to_owned(),
        validator_os: env::consts::OS.to_owned(),
        validator_role: VALIDATOR_ROLE.to_owned(),
        vendor_manifest_sha256: hex::encode(manifest.vendor_manifest_sha256()),
        verifier_key: identity(manifest, VegaMdlFigure9ArtifactRoleV1::VerifierKey),
        workspace_source_manifest_sha256,
    };
    let mut encoded = norito::json::to_string(&report)
        .map_err(|error| format!("cannot encode native validation report: {error}"))?;
    encoded.push('\n');
    std::io::stdout()
        .lock()
        .write_all(encoded.as_bytes())
        .map_err(|error| format!("cannot write native validation report: {error}"))
}

fn parse_args() -> Result<(PathBuf, PathBuf, PathBuf), String> {
    let mut args = env::args_os();
    let _program = args.next();
    if args.next().as_deref() != Some(std::ffi::OsStr::new("qualify-prover-release"))
        || args.next() != Some(OsString::from("--proving-key"))
    {
        return Err(
            "usage: vega_figure9_artifact_tool qualify-prover-release --proving-key PATH \
             --verifier-key PATH --evidence-output DIRECTORY"
                .to_owned(),
        );
    }
    let proving_key = args
        .next()
        .ok_or_else(|| "missing Vega proving-key path".to_owned())?;
    if args.next() != Some(OsString::from("--verifier-key")) {
        return Err("missing --verifier-key argument".to_owned());
    }
    let verifier_key = args
        .next()
        .ok_or_else(|| "missing Vega verifier-key path".to_owned())?;
    if args.next() != Some(OsString::from("--evidence-output")) {
        return Err("missing --evidence-output argument".to_owned());
    }
    let evidence_output = args
        .next()
        .ok_or_else(|| "missing Vega evidence-output path".to_owned())?;
    if args.next().is_some() {
        return Err("unexpected trailing Vega artifact-tool argument".to_owned());
    }
    Ok((
        PathBuf::from(proving_key),
        PathBuf::from(verifier_key),
        PathBuf::from(evidence_output),
    ))
}

fn main() -> ExitCode {
    let result = parse_args().and_then(|(proving_key, verifier_key, evidence_output)| {
        qualify_release(proving_key, verifier_key, evidence_output)
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Vega Figure 9 artifact tool refused: {error}");
            ExitCode::FAILURE
        }
    }
}
