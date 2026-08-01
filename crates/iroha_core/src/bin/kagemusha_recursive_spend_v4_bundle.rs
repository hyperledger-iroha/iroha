//! Generate and finalize calibrated ABI-21 Kagemusha release bundles.
//!
//! Candidate generation runs the independently reviewed recursion source
//! closure exactly once and
//! publishes eight immutable `KRV4KEY` artifacts plus one canonical pre-evidence
//! candidate record. Finalization never regenerates proof material: it binds the
//! unchanged candidate to supplied evidence and authenticates the resulting
//! release before publishing a distinct final directory atomically.

use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    error::Error,
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::Command,
};

use iroha_core::zk::kagemusha_artifact_v4::{
    KagemushaValidatedArtifactPayloadV4, read_kagemusha_pasta_cycle_artifact_v4,
    write_kagemusha_pasta_cycle_artifact_from_reader_v4, write_kagemusha_pasta_cycle_artifact_v4,
};
use iroha_core::zk::kagemusha_v2::{
    KagemushaGenerationSupervisorPermitV4, claim_kagemusha_generation_supervisor_permit_v4,
    generate_kagemusha_pasta_cycle_artifacts_v4, validate_kagemusha_proof_pair_measurement_v4,
    validate_kagemusha_step_bootstrap_payload_v4,
};
use iroha_data_model::{
    ChainId,
    asset::AssetDefinitionId,
    offline::{
        KAGEMUSHA_COMPACT_PROVING_KEY_MAX_BYTES_V5,
        KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_HEADER_MAX_BYTES_V4,
        KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_HEADER_VERSION_V4,
        KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_KEY_MAGIC_V4,
        KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4,
        KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V4,
        KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4,
        KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4,
        KAGEMUSHA_RECURSIVE_SPEND_BENCHMARK_EVIDENCE_FILE_NAME_V1,
        KAGEMUSHA_RECURSIVE_SPEND_CANDIDATE_SCHEMA_V4,
        KAGEMUSHA_RECURSIVE_SPEND_CANDIDATE_VERSION_V4,
        KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_FILE_NAME_V1,
        KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_MAX_BYTES_V4,
        KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4,
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4,
        KAGEMUSHA_RECURSIVE_SPEND_PROMOTED_RELEASE_SCHEMA_V4,
        KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4,
        KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_RELEASE_INITIALIZATION_BYTES_V4,
        KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_RELEASE_MAX_BYTES_V4,
        KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_FILE_NAME_V4,
        KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V4,
        KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_BOOTSTRAP_FILE_NAME_V4,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PARAMS_IPA_FILE_NAME_V4,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PROVING_KEY_FILE_NAME_V4,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFYING_KEY_FILE_NAME_V4,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_BOOTSTRAP_FILE_NAME_V4,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PARAMS_IPA_FILE_NAME_V4,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PROVING_KEY_FILE_NAME_V4,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFYING_KEY_FILE_NAME_V4,
        KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2, KAGEMUSHA_TOPUP_FINALITY_CIRCUIT_ID_V2,
        KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_V2,
        KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_PURPOSE_V2,
        KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_TYPE_V2,
        KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V4, KagemushaAuthenticatedReleaseV4,
        KagemushaPastaCycleArtifactKindV4, KagemushaPastaCycleArtifactV4,
        KagemushaPastaCycleFramedArtifactHeaderV4, KagemushaPastaCycleParityV1,
        KagemushaPastaCycleProofProfileV4, KagemushaRecursiveSpendArtifactManifestV4,
        KagemushaRecursiveSpendCandidateV4, KagemushaRecursiveSpendCryptographicReviewEvidenceV4,
        KagemushaRecursiveSpendPromotedReleaseV4, KagemushaRecursiveSpendReleaseAttestationV4,
        KagemushaRecursiveSpendReleasePolicyV1, KagemushaReviewedSourceClosureV1,
        KagemushaStepCircuitParamsV4, KagemushaTopUpFinalityRosterArtifactReferenceV4,
        KagemushaTopUpFinalityRosterArtifactV2,
    },
};
use norito::{JsonDeserialize, JsonSerialize};
use sha2::{Digest, Sha256};

const HELP: &str = "\
Generate an unsigned ABI-21 candidate, then finalize those exact bytes after approval.

Usage:
  python3 scripts/build_kagemusha_v4_candidate_bundle.py \\
    > sealed-kagemusha-candidate-build.json
  python3 scripts/run_kagemusha_v4_generation.py \\
    --resource-report <new-resource-report-directory> -- \\
    <binary_path-from-sealed-kagemusha-candidate-build.json> \\
    generate-candidate \\
    --out-dir <new-directory> \\
    --chain-id <chain> --asset-definition-id <asset> --asset-scale <u32> \\
    --generation <id> --parameter-generation <id> \\
    --source-commit <40-lower-hex> --source-tree-sha256 <64-lower-hex> \\
    --activation-height <u64> --withdrawal-height <u64> \\
    --step-eq-circuit-params <canonical-norito-file> \\
    --step-ep-circuit-params <canonical-norito-file> \\
    --topup-finality-roster <canonical-norito-file>

  <binary_path-from-sealed-kagemusha-candidate-build.json> \\
    finalize-release \\
    --candidate-dir <generated-candidate> \\
    --out-dir <new-final-directory> \\
    --release-policy <canonical-norito-file> \\
    --release-attestation <canonical-norito-file> \\
    --benchmark-evidence <exact-file> \\
    --cryptographic-review <exact-file>

  <binary_path-from-sealed-kagemusha-candidate-build.json> \\
    validate-candidate \\
    --candidate-dir <generated-candidate> \\
    --out-dir <new-validation-directory>

Candidate generation emits four roles per parity in exact Eq-then-Ep order:
ParamsIPA, processed proving key, processed verifying key, and the final-VK
selector-zero BootstrapWitness. Circuit parameters remain bounded inline in the
authenticated profile and are digest-bound into every artifact header. It writes a
reviewed-closure-bound, pre-evidence candidate record; that directory is not an
approved release and contains no approval payload. Candidate generation requires
the requested commit to be the signed checkout HEAD and the complete dirty
checkout to match the independently pinned source-closure descriptor.
Finalization binds the two supplied evidence files into the release manifest,
verifies signed attestation thresholds,
requires canonical signed Norito cryptographic-review evidence bound to the exact
candidate and policy reviewer identities, requires that same signed base commit
and reviewed source closure, rechecks every staged
inode/size/hash, and copies those exact bytes without keygen or proof generation.
Both output directories must be new.

The generate-candidate command is intentionally available only through
scripts/run_kagemusha_v4_generation.py, which owns the host-global lock,
process group, physical-memory ceiling, per-run staging identity, cleanup, and
resource report. If generation is terminated, the launcher removes only the
owner-private staging directory carrying that invocation's unguessable id.
Build the source-sealed release binary with the helper before entering that
64 GiB / half-physical-RAM guard: wrapping `cargo run` would include the compiler
in the guarded process group. The finalize-release and validate-candidate
commands do not require the generation guard.
";

const GENERATE_OPTIONS: &[&str] = &[
    "out-dir",
    // Injected by `run_kagemusha_v4_generation.py`; callers cannot select it.
    "staging-id",
    "staging-name",
    "output-parent-fd",
    "chain-id",
    "asset-definition-id",
    "asset-scale",
    "generation",
    "parameter-generation",
    "source-commit",
    "source-tree-sha256",
    "activation-height",
    "withdrawal-height",
    "step-eq-circuit-params",
    "step-ep-circuit-params",
    "topup-finality-roster",
];

const FINALIZE_OPTIONS: &[&str] = &[
    "candidate-dir",
    "out-dir",
    "release-policy",
    "release-attestation",
    "benchmark-evidence",
    "cryptographic-review",
];

const PUBLISH_STAGED_CANDIDATE_OPTIONS: &[&str] = &[
    "out-dir",
    "staging-id",
    "staging-name",
    "output-parent-fd",
    "source-commit",
    "source-tree-sha256",
];

const VALIDATE_CANDIDATE_OPTIONS: &[&str] = &["candidate-dir", "out-dir"];

const MANIFEST_JSON_FILE_NAME: &str = "manifest.json";
const MANIFEST_NORITO_FILE_NAME: &str = "manifest.norito";
const MANIFEST_NORITO_SHA256_FILE_NAME: &str = "manifest.norito.sha256";
const CANDIDATE_MANIFEST_JSON_FILE_NAME: &str = "candidate-manifest.json";
const CANDIDATE_MANIFEST_NORITO_FILE_NAME: &str = "candidate-manifest.norito";
const CANDIDATE_MANIFEST_SHA256_FILE_NAME: &str = "candidate-manifest.norito.sha256";
const PROMOTION_RECORD_FILE_NAME_V4: &str = "promotion-record-v4.norito";
const CANDIDATE_VALIDATION_REPORT_FILE_NAME_V1: &str = "candidate-validation-v1.json";
const CANDIDATE_VALIDATION_MANIFEST_FILE_NAME_V4: &str = "manifest-v4.norito";
const CANDIDATE_VALIDATION_REPORT_SCHEMA_V1: &str =
    "iroha.kagemusha.recursive_spend.candidate_validation.v1";
const MAX_MANIFEST_BYTES: u64 = 32 * 1024 * 1024;
const MAX_POLICY_BYTES: u64 = 64 * 1024;
const MAX_ATTESTATION_BYTES: u64 = 1024 * 1024;
const BUILD_SOURCE_COMMIT: Option<&str> = option_env!("KAGEMUSHA_BUILD_SOURCE_COMMIT");
const BUILD_SOURCE_TREE_SHA256: Option<&str> = option_env!("KAGEMUSHA_BUILD_SOURCE_TREE_SHA256");
const BUILD_REVIEWED_SOURCE_CLOSURE: Option<&str> =
    option_env!("KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE");
const BUILD_REVIEWED_SOURCE_CLOSURE_SHA256: Option<&str> =
    option_env!("KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE_SHA256");
const TRUSTED_GIT_EXECUTABLE: &str = "/usr/bin/git";
const TRUSTED_PYTHON_EXECUTABLE: &str = "/usr/bin/python3";
const TRUSTED_TOOL_PATH: &str = "/usr/bin:/bin";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct InputSpec {
    file_name: &'static str,
    parity: KagemushaPastaCycleParityV1,
    kind: KagemushaPastaCycleArtifactKindV4,
}

const INPUTS: [InputSpec; 8] = [
    InputSpec {
        file_name: KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PARAMS_IPA_FILE_NAME_V4,
        parity: KagemushaPastaCycleParityV1::StepEq,
        kind: KagemushaPastaCycleArtifactKindV4::ParamsIpa,
    },
    InputSpec {
        file_name: KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PROVING_KEY_FILE_NAME_V4,
        parity: KagemushaPastaCycleParityV1::StepEq,
        kind: KagemushaPastaCycleArtifactKindV4::ProvingKey,
    },
    InputSpec {
        file_name: KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFYING_KEY_FILE_NAME_V4,
        parity: KagemushaPastaCycleParityV1::StepEq,
        kind: KagemushaPastaCycleArtifactKindV4::VerifyingKey,
    },
    InputSpec {
        file_name: KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_BOOTSTRAP_FILE_NAME_V4,
        parity: KagemushaPastaCycleParityV1::StepEq,
        kind: KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
    },
    InputSpec {
        file_name: KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PARAMS_IPA_FILE_NAME_V4,
        parity: KagemushaPastaCycleParityV1::StepEp,
        kind: KagemushaPastaCycleArtifactKindV4::ParamsIpa,
    },
    InputSpec {
        file_name: KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PROVING_KEY_FILE_NAME_V4,
        parity: KagemushaPastaCycleParityV1::StepEp,
        kind: KagemushaPastaCycleArtifactKindV4::ProvingKey,
    },
    InputSpec {
        file_name: KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFYING_KEY_FILE_NAME_V4,
        parity: KagemushaPastaCycleParityV1::StepEp,
        kind: KagemushaPastaCycleArtifactKindV4::VerifyingKey,
    },
    InputSpec {
        file_name: KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_BOOTSTRAP_FILE_NAME_V4,
        parity: KagemushaPastaCycleParityV1::StepEp,
        kind: KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
    },
];

fn validate_artifacts_sequentially<I, T, E, F>(artifacts: I, mut validate: F) -> Result<(), E>
where
    I: IntoIterator,
    F: FnMut(I::Item) -> Result<T, E>,
{
    for artifact in artifacts {
        drop(validate(artifact)?);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileSnapshot {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    length: u64,
    #[cfg(unix)]
    modified_seconds: i64,
    #[cfg(unix)]
    modified_nanoseconds: i64,
    #[cfg(unix)]
    changed_seconds: i64,
    #[cfg(unix)]
    changed_nanoseconds: i64,
}

#[cfg(unix)]
fn checked_stat_value<T, U>(value: T) -> Option<U>
where
    U: TryFrom<T>,
{
    U::try_from(value).ok()
}

impl FileSnapshot {
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            Self {
                device: metadata.dev(),
                inode: metadata.ino(),
                length: metadata.len(),
                modified_seconds: metadata.mtime(),
                modified_nanoseconds: metadata.mtime_nsec(),
                changed_seconds: metadata.ctime(),
                changed_nanoseconds: metadata.ctime_nsec(),
            }
        }
        #[cfg(not(unix))]
        {
            Self {
                length: metadata.len(),
            }
        }
    }

    #[cfg(unix)]
    fn identity(self) -> (u64, u64) {
        (self.device, self.inode)
    }

    #[cfg(unix)]
    fn from_stat(value: &rustix::fs::Stat) -> Option<Self> {
        use rustix::fs::FileType as RustixFileType;

        (RustixFileType::from_raw_mode(value.st_mode) == RustixFileType::RegularFile
            && checked_stat_value::<_, u64>(value.st_nlink)? == 1)
            .then_some(Self {
                device: checked_stat_value(value.st_dev)?,
                inode: checked_stat_value(value.st_ino)?,
                length: checked_stat_value(value.st_size)?,
                modified_seconds: checked_stat_value(value.st_mtime)?,
                modified_nanoseconds: checked_stat_value(value.st_mtime_nsec)?,
                changed_seconds: checked_stat_value(value.st_ctime)?,
                changed_nanoseconds: checked_stat_value(value.st_ctime_nsec)?,
            })
    }
}

struct OpenedInput {
    file: File,
    path: PathBuf,
    #[cfg(unix)]
    parent: Option<File>,
    #[cfg(unix)]
    entry_name: Option<std::ffi::OsString>,
    snapshot: FileSnapshot,
    size_bytes: u64,
    sha256: [u8; 32],
}

impl OpenedInput {
    fn rewind_and_verify(&mut self) -> Result<(), Box<dyn Error>> {
        self.file.seek(SeekFrom::Start(0))?;
        self.verify_unchanged()
    }

    fn verify_unchanged(&self) -> Result<(), Box<dyn Error>> {
        let opened = self.file.metadata()?;
        #[cfg(unix)]
        let current_matches = if let (Some(parent), Some(name)) =
            (self.parent.as_ref(), self.entry_name.as_ref())
        {
            let current = rustix::fs::statat(parent, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)?;
            FileSnapshot::from_stat(&current) == Some(self.snapshot)
        } else {
            let current = fs::symlink_metadata(&self.path)?;
            !current.file_type().is_symlink()
                && current.is_file()
                && FileSnapshot::from_metadata(&current) == self.snapshot
        };
        #[cfg(not(unix))]
        let current_matches = {
            let current = fs::symlink_metadata(&self.path)?;
            !current.file_type().is_symlink()
                && current.is_file()
                && FileSnapshot::from_metadata(&current) == self.snapshot
        };
        if FileSnapshot::from_metadata(&opened) != self.snapshot || !current_matches {
            return Err(format!("input changed while packaging: {}", self.path.display()).into());
        }
        Ok(())
    }

    fn rehash_and_verify(&mut self) -> Result<(), Box<dyn Error>> {
        self.verify_unchanged()?;
        self.file.seek(SeekFrom::Start(0))?;
        let sha256 = hash_open_file(&mut self.file, self.size_bytes, &self.path)?;
        if sha256 != self.sha256 {
            return Err(format!("input content changed: {}", self.path.display()).into());
        }
        self.file.seek(SeekFrom::Start(0))?;
        self.verify_unchanged()
    }

    fn read_all(&mut self) -> Result<Vec<u8>, Box<dyn Error>> {
        let capacity = usize::try_from(self.size_bytes)?;
        let mut bytes = Vec::with_capacity(capacity);
        self.file.read_to_end(&mut bytes)?;
        if bytes.len() != capacity || <[u8; 32]>::from(Sha256::digest(&bytes)) != self.sha256 {
            return Err(format!("input changed while reading: {}", self.path.display()).into());
        }
        self.rewind_and_verify()?;
        Ok(bytes)
    }

    fn copy_exact_to(&mut self, output: &mut File) -> Result<(), Box<dyn Error>> {
        self.file.seek(SeekFrom::Start(0))?;
        let copied = io::copy(
            &mut Read::by_ref(&mut self.file).take(self.size_bytes.saturating_add(1)),
            output,
        )?;
        if copied != self.size_bytes {
            return Err(format!("input changed while copying: {}", self.path.display()).into());
        }
        output.sync_all()?;
        self.rewind_and_verify()
    }
}

struct ProfileMetadata {
    parity: KagemushaPastaCycleParityV1,
    circuit_id: &'static str,
    circuit_params: KagemushaStepCircuitParamsV4,
    circuit_params_sha256: [u8; 32],
    compiled_protocol_structure_sha256: [u8; 32],
    step_proof_size_bytes: u32,
}

struct BundleMetadata {
    chain_id: ChainId,
    asset: AssetDefinitionId,
    asset_scale: u32,
    generation: String,
    parameter_generation: String,
    source_commit: String,
    source_tree_sha256: [u8; 32],
    reviewed_source_closure: KagemushaReviewedSourceClosureV1,
    reviewed_source_closure_descriptor_sha256: [u8; 32],
    activation_height: u64,
    withdrawal_height: u64,
    max_proof_bytes: u32,
    measured_proof_pair: Vec<u8>,
    profiles: [ProfileMetadata; 2],
}

struct PreparedArtifact {
    spec: InputSpec,
    payload: GeneratedPayload,
    circuit_params: KagemushaStepCircuitParamsV4,
    compiled_protocol_structure_sha256: [u8; 32],
    step_proof_size_bytes: u32,
    header: KagemushaPastaCycleFramedArtifactHeaderV4,
    total_size: u64,
}

struct GeneratedArtifact {
    spec: InputSpec,
    payload: GeneratedPayload,
}

struct PreparedBundle {
    metadata: BundleMetadata,
    generated_artifacts: Vec<GeneratedArtifact>,
    vesta_proving_key_size_bytes: u64,
    pallas_proving_key_size_bytes: u64,
}

enum GeneratedPayload {
    Memory(Vec<u8>),
    Staged(StagedGeneratedPayload),
}

impl GeneratedPayload {
    fn identity(&self) -> Result<(u64, [u8; 32]), Box<dyn Error>> {
        match self {
            Self::Memory(bytes) => Ok((u64::try_from(bytes.len())?, Sha256::digest(bytes).into())),
            Self::Staged(staged) => Ok((staged.size_bytes, staged.sha256)),
        }
    }
}

struct StagedGeneratedPayload {
    file: File,
    size_bytes: u64,
    sha256: [u8; 32],
}

struct BoundedDigestFileWriter {
    file: File,
    maximum_bytes: u64,
    written: u64,
    hasher: Sha256,
}

impl BoundedDigestFileWriter {
    fn new(directory: &PublicationDirectory, name: &str) -> Result<Self, Box<dyn Error>> {
        #[cfg(unix)]
        let file = directory.create_unlinked_file(name)?;
        #[cfg(not(unix))]
        let file = {
            let _ = name;
            tempfile::tempfile_in(&directory.path)?
        };
        Ok(Self {
            file,
            maximum_bytes: KAGEMUSHA_COMPACT_PROVING_KEY_MAX_BYTES_V5,
            written: 0,
            hasher: Sha256::new(),
        })
    }

    fn finish(
        mut self,
        expected_size_bytes: u64,
        label: &str,
    ) -> Result<StagedGeneratedPayload, Box<dyn Error>> {
        self.file.flush()?;
        self.file.sync_all()?;
        let metadata = self.file.metadata()?;
        if !metadata.is_file()
            || self.written == 0
            || self.written != expected_size_bytes
            || metadata.len() != self.written
            || self.written > self.maximum_bytes
        {
            return Err(format!("generated {label} staging length is inconsistent").into());
        }
        self.file.seek(SeekFrom::Start(0))?;
        let sha256: [u8; 32] = self.hasher.finalize().into();
        if sha256 == [0; 32] {
            return Err(format!("generated {label} staging digest is zero").into());
        }
        Ok(StagedGeneratedPayload {
            file: self.file,
            size_bytes: self.written,
            sha256,
        })
    }
}

impl Write for BoundedDigestFileWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let requested = u64::try_from(bytes.len())
            .map_err(|_| io::Error::other("proving-key write length does not fit u64"))?;
        if self
            .written
            .checked_add(requested)
            .is_none_or(|total| total > self.maximum_bytes)
        {
            return Err(io::Error::other(format!(
                "generated proving key exceeds {} bytes",
                self.maximum_bytes
            )));
        }
        let written = self.file.write(bytes)?;
        self.hasher.update(&bytes[..written]);
        self.written = self
            .written
            .checked_add(
                u64::try_from(written)
                    .map_err(|_| io::Error::other("proving-key write count does not fit u64"))?,
            )
            .ok_or_else(|| io::Error::other("proving-key write count overflow"))?;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

#[derive(Debug, JsonSerialize)]
struct CandidateValidationArtifactV1 {
    role: String,
    file_name: String,
    framed_size_bytes: u64,
    framed_sha256: String,
    payload_size_bytes: u64,
    payload_sha256: String,
}

#[derive(Debug, JsonSerialize)]
struct CandidateValidationReportV1 {
    schema: String,
    candidate_record_sha256: String,
    candidate_manifest_sha256: String,
    source_commit: String,
    source_tree_sha256: String,
    source_repo_dirty: bool,
    reviewed_source_closure_descriptor_sha256: String,
    generation: String,
    bridge_abi_version: u32,
    artifact_count: u32,
    artifacts: Vec<CandidateValidationArtifactV1>,
    topup_finality_roster_file_name: String,
    topup_finality_roster_size_bytes: u64,
    topup_finality_roster_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonDeserialize)]
struct FullSourceTreeIdentityV1 {
    schema: String,
    source_commit: String,
    source_repo_dirty: bool,
    source_tree_sha256: String,
    reviewed_source_closure: KagemushaReviewedSourceClosureV1,
    reviewed_source_closure_descriptor_sha256: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = env::args().skip(1);
    let Some(command) = arguments.next() else {
        return Err(format!("missing command\n\n{HELP}").into());
    };
    if matches!(command.as_str(), "--help" | "-h") {
        if arguments.next().is_some() {
            return Err("--help must be the only argument".into());
        }
        print!("{HELP}");
        return Ok(());
    }
    let required_options = match command.as_str() {
        "generate-candidate" => GENERATE_OPTIONS,
        "publish-staged-candidate" => PUBLISH_STAGED_CANDIDATE_OPTIONS,
        "finalize-release" => FINALIZE_OPTIONS,
        "validate-candidate" => VALIDATE_CANDIDATE_OPTIONS,
        _ => return Err(format!("unknown command `{command}`\n\n{HELP}").into()),
    };
    let options = parse_options(arguments, required_options)?;
    if options.contains_key("help") {
        print!("{HELP}");
        return Ok(());
    }
    for option in required_options {
        if !options.contains_key(*option) {
            return Err(format!("missing required option --{option}\n\n{HELP}").into());
        }
    }
    match command.as_str() {
        "generate-candidate" => {
            build_candidate(&options, claim_kagemusha_generation_supervisor_permit_v4()?)
        }
        "publish-staged-candidate" => {
            publish_staged_candidate(&options, claim_kagemusha_generation_supervisor_permit_v4()?)
        }
        "finalize-release" => finalize_release(&options),
        "validate-candidate" => validate_candidate(&options),
        _ => unreachable!("command was checked above"),
    }
}

fn parse_options(
    arguments: impl IntoIterator<Item = String>,
    allowed_options: &[&str],
) -> Result<BTreeMap<String, String>, Box<dyn Error>> {
    let mut options = BTreeMap::new();
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        if argument == "--help" || argument == "-h" {
            if !options.is_empty() || arguments.next().is_some() {
                return Err("--help must be the only argument".into());
            }
            options.insert("help".to_owned(), String::new());
            return Ok(options);
        }
        let option = argument
            .strip_prefix("--")
            .filter(|value| allowed_options.contains(value))
            .ok_or_else(|| format!("unknown argument `{argument}`"))?;
        let value = arguments
            .next()
            .ok_or_else(|| format!("missing value for --{option}"))?;
        if value.is_empty() || value.starts_with("--") {
            return Err(format!("invalid empty value for --{option}").into());
        }
        if options.insert(option.to_owned(), value).is_some() {
            return Err(format!("duplicate option --{option}").into());
        }
    }
    Ok(options)
}

fn required<'a>(options: &'a BTreeMap<String, String>, name: &str) -> &'a str {
    options
        .get(name)
        .expect("required option checked by main")
        .as_str()
}

fn canonical_norito_bytes<T>(value: &T, label: &str) -> Result<Vec<u8>, Box<dyn Error>>
where
    T: PartialEq + norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    let bytes = norito::to_bytes(value)?;
    let decoded: T = norito::decode_from_bytes(&bytes)?;
    if &decoded != value || norito::to_bytes(&decoded)? != bytes {
        return Err(format!("canonical {label} round-trip changed its value or bytes").into());
    }
    Ok(bytes)
}

fn decode_canonical_norito<T>(bytes: &[u8], label: &str) -> Result<T, Box<dyn Error>>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    let value: T = norito::decode_from_bytes(bytes)
        .map_err(|error| format!("failed to decode {label}: {error}"))?;
    if norito::to_bytes(&value)? != bytes {
        return Err(format!("{label} is not canonical Norito").into());
    }
    Ok(value)
}

fn canonical_unsigned_decimal(value: &str) -> bool {
    value == "0"
        || value.as_bytes().split_first().is_some_and(|(first, rest)| {
            matches!(first, b'1'..=b'9') && rest.iter().all(u8::is_ascii_digit)
        })
}

fn is_kagemusha_v4_portable_identifier(value: &str) -> bool {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        || !value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        || !value
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric)
    {
        return false;
    }

    // Windows resolves these basenames as device aliases even when an
    // extension follows. Reject them so the same release identifier names
    // exactly one artifact directory on every supported build host.
    let basename = value.split('.').next().unwrap_or_default();
    if ["con", "prn", "aux", "nul"]
        .iter()
        .any(|reserved| basename.eq_ignore_ascii_case(reserved))
    {
        return false;
    }
    let basename_bytes = basename.as_bytes();
    !(basename_bytes.len() == 4
        && (basename_bytes[..3].eq_ignore_ascii_case(b"com")
            || basename_bytes[..3].eq_ignore_ascii_case(b"lpt"))
        && matches!(basename_bytes[3], b'1'..=b'9'))
}

fn parse_u32(options: &BTreeMap<String, String>, name: &str) -> Result<u32, Box<dyn Error>> {
    let value = required(options, name);
    if !canonical_unsigned_decimal(value) {
        return Err(format!("--{name} must be a canonical unsigned decimal").into());
    }
    value
        .parse::<u32>()
        .map_err(|error| format!("--{name} must fit u32: {error}").into())
}

fn parse_u64(options: &BTreeMap<String, String>, name: &str) -> Result<u64, Box<dyn Error>> {
    let value = required(options, name);
    if !canonical_unsigned_decimal(value) {
        return Err(format!("--{name} must be a canonical unsigned decimal").into());
    }
    value
        .parse::<u64>()
        .map_err(|error| format!("--{name} must fit u64: {error}").into())
}

fn parse_digest(
    options: &BTreeMap<String, String>,
    name: &str,
) -> Result<[u8; 32], Box<dyn Error>> {
    let value = required(options, name);
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(format!("--{name} must be exact lowercase 32-byte hexadecimal").into());
    }
    let mut digest = [0; 32];
    hex::decode_to_slice(value, &mut digest)?;
    if digest == [0; 32] {
        return Err(format!("--{name} must not be all zero").into());
    }
    Ok(digest)
}

fn open_input(path: &Path, maximum: u64, label: &str) -> Result<OpenedInput, Box<dyn Error>> {
    #[cfg(not(unix))]
    return Err(format!("{label} opening is unsupported on this non-Unix target").into());

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;

        use rustix::fs::{Mode, OFlags};

        let before = fs::symlink_metadata(path)?;
        if before.file_type().is_symlink() || !before.is_file() || before.nlink() != 1 {
            return Err(format!(
                "{label} must be a non-symlink regular file: {}",
                path.display()
            )
            .into());
        }
        if before.len() == 0 || before.len() > maximum {
            return Err(format!(
                "{label} length must be 1..={maximum} bytes: {}",
                path.display()
            )
            .into());
        }
        let mut file = File::from(rustix::fs::open(
            path,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        let opened = file.metadata()?;
        let after = fs::symlink_metadata(path)?;
        let snapshot = FileSnapshot::from_metadata(&opened);
        if !opened.is_file()
            || after.file_type().is_symlink()
            || !after.is_file()
            || FileSnapshot::from_metadata(&before) != snapshot
            || FileSnapshot::from_metadata(&after) != snapshot
        {
            return Err(format!("{label} changed while it was opened: {}", path.display()).into());
        }
        let sha256 = hash_open_file(&mut file, snapshot.length, path)?;
        file.seek(SeekFrom::Start(0))?;
        Ok(OpenedInput {
            file,
            path: path.to_owned(),
            parent: None,
            entry_name: None,
            snapshot,
            size_bytes: snapshot.length,
            sha256,
        })
    }
}

fn hash_open_file(
    file: &mut File,
    expected_size: u64,
    path: &Path,
) -> Result<[u8; 32], Box<dyn Error>> {
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(u64::try_from(read)?)
            .ok_or_else(|| io::Error::other("input length overflow"))?;
        if total > expected_size {
            return Err(format!("input grew while hashing: {}", path.display()).into());
        }
        hasher.update(&buffer[..read]);
    }
    if total != expected_size {
        return Err(format!("input changed while hashing: {}", path.display()).into());
    }
    Ok(hasher.finalize().into())
}

fn decode_canonical_circuit_params(
    input: &mut OpenedInput,
    label: &str,
) -> Result<KagemushaStepCircuitParamsV4, Box<dyn Error>> {
    let bytes = input.read_all()?;
    let params: KagemushaStepCircuitParamsV4 = norito::decode_from_bytes(&bytes)
        .map_err(|error| format!("{label} is not CircuitParamsV4: {error}"))?;
    params
        .validate()
        .map_err(|error| format!("{label} is invalid: {error}"))?;
    if norito::to_bytes(&params)? != bytes {
        return Err(format!("{label} is not canonical Norito").into());
    }
    Ok(params)
}

#[expect(
    clippy::too_many_lines,
    reason = "generation, calibration, and artifact extraction form one ordered fail-closed capability-consuming flow"
)]
fn prepare_bundle_metadata(
    options: &BTreeMap<String, String>,
    source_identity: &FullSourceTreeIdentityV1,
    vesta_params_input: &mut OpenedInput,
    pallas_params_input: &mut OpenedInput,
    supervisor_permit: KagemushaGenerationSupervisorPermitV4,
    vesta_proving_key_output: &mut (dyn Write + Send),
    pallas_proving_key_output: &mut (dyn Write + Send),
) -> Result<PreparedBundle, Box<dyn Error>> {
    let chain_id: ChainId = required(options, "chain-id").parse()?;
    if chain_id.as_str().is_empty()
        || chain_id.as_str().len() > 128
        || chain_id.as_str().trim() != chain_id.as_str()
        || chain_id.as_str().chars().any(char::is_control)
    {
        return Err("--chain-id must be exact non-empty text of at most 128 bytes".into());
    }
    let asset: AssetDefinitionId = required(options, "asset-definition-id").parse()?;
    let asset_scale = parse_u32(options, "asset-scale")?;
    if asset_scale > KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2 {
        return Err(format!(
            "--asset-scale exceeds the Kagemusha bound {KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2}"
        )
        .into());
    }
    let generation = required(options, "generation").to_owned();
    let parameter_generation = required(options, "parameter-generation").to_owned();
    if !is_kagemusha_v4_portable_identifier(&generation)
        || !is_kagemusha_v4_portable_identifier(&parameter_generation)
    {
        return Err(
            "release and parameter generations must be canonical portable identifiers".into(),
        );
    }
    let source_commit = required(options, "source-commit").to_owned();
    if source_commit.len() != 40
        || !source_commit.bytes().any(|byte| byte != b'0')
        || !source_commit
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err("--source-commit must be a nonzero lowercase 40-hex Git object id".into());
    }
    if source_identity.source_commit != source_commit
        || source_identity.source_tree_sha256 != required(options, "source-tree-sha256")
    {
        return Err("requested source identity differs from the reviewed closure".into());
    }
    let mut reviewed_source_closure_descriptor_sha256 = [0_u8; 32];
    hex::decode_to_slice(
        &source_identity.reviewed_source_closure_descriptor_sha256,
        &mut reviewed_source_closure_descriptor_sha256,
    )?;
    let activation_height = parse_u64(options, "activation-height")?;
    let withdrawal_height = parse_u64(options, "withdrawal-height")?;
    if activation_height == 0 || withdrawal_height <= activation_height {
        return Err("release heights must define a non-empty, nonzero activation window".into());
    }

    let requested_vesta_params =
        decode_canonical_circuit_params(vesta_params_input, "--step-eq-circuit-params")?;
    let requested_pallas_params =
        decode_canonical_circuit_params(pallas_params_input, "--step-ep-circuit-params")?;
    let generated = generate_kagemusha_pasta_cycle_artifacts_v4(
        requested_vesta_params,
        requested_pallas_params,
        supervisor_permit,
        vesta_proving_key_output,
        pallas_proving_key_output,
    )
    .map_err(|error| format!("current-source Kagemusha V4 generation failed: {error}"))?;
    let max_proof_bytes = generated.max_recursive_pair_bytes;
    let vesta_generated = generated.step_eq;
    let pallas_generated = generated.step_ep;
    let measured_proof_pair = generated.measured_live_pair_bytes;

    let vesta_layout = vesta_generated
        .circuit_params
        .validate_release_generation_profile()
        .map_err(|error| {
            format!("generated Eq CircuitParamsV4 release-profile validation failed: {error}")
        })?;
    let pallas_layout = pallas_generated
        .circuit_params
        .validate_release_generation_profile()
        .map_err(|error| {
            format!("generated Ep CircuitParamsV4 release-profile validation failed: {error}")
        })?;
    if vesta_generated.circuit_params.k != pallas_generated.circuit_params.k
        || vesta_layout != pallas_layout
    {
        return Err("generated Eq/Ep profiles select different IPA/public layouts".into());
    }
    if vesta_generated.compiled_protocol_structure_sha256 == [0; 32]
        || pallas_generated.compiled_protocol_structure_sha256 == [0; 32]
        || vesta_generated.compiled_protocol_structure_sha256
            == pallas_generated.compiled_protocol_structure_sha256
        || vesta_generated.step_proof_size_bytes
            != vesta_generated.circuit_params.max_parent_proof_bytes
        || pallas_generated.step_proof_size_bytes
            != pallas_generated.circuit_params.max_parent_proof_bytes
    {
        return Err("generated V4 profile calibration metadata is inconsistent".into());
    }

    let vesta_bootstrap_bytes = validate_kagemusha_step_bootstrap_payload_v4(
        &vesta_generated.bootstrap_witness,
        &vesta_generated.circuit_params,
        KagemushaPastaCycleParityV1::StepEq,
        vesta_generated.compiled_protocol_structure_sha256,
    )?;
    let pallas_bootstrap_bytes = validate_kagemusha_step_bootstrap_payload_v4(
        &pallas_generated.bootstrap_witness,
        &pallas_generated.circuit_params,
        KagemushaPastaCycleParityV1::StepEp,
        pallas_generated.compiled_protocol_structure_sha256,
    )?;
    if u32::try_from(vesta_bootstrap_bytes) != Ok(vesta_generated.step_proof_size_bytes)
        || u32::try_from(pallas_bootstrap_bytes) != Ok(pallas_generated.step_proof_size_bytes)
    {
        return Err("generated bootstrap measurements differ from calibrated Step sizes".into());
    }

    let measured_pair_bytes = u32::try_from(measured_proof_pair.len())
        .map_err(|_| "measured V4 proof pair length does not fit u32")?;
    let measured_steps = vesta_generated
        .step_proof_size_bytes
        .checked_add(pallas_generated.step_proof_size_bytes)
        .ok_or("generated V4 Step-size sum overflow")?;
    if max_proof_bytes <= measured_steps
        || measured_pair_bytes
            != KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_RELEASE_INITIALIZATION_BYTES_V4
        || max_proof_bytes != KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_RELEASE_MAX_BYTES_V4
        || max_proof_bytes > KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4
    {
        return Err("generated V4 recursive proof-pair maximum is inconsistent".into());
    }
    let measured = validate_kagemusha_proof_pair_measurement_v4(
        &measured_proof_pair,
        &vesta_generated.circuit_params,
        &pallas_generated.circuit_params,
        max_proof_bytes,
    )?;
    if measured != measured_proof_pair.len() {
        return Err("V4 proof-pair validator returned a different measurement".into());
    }

    let generated_artifacts = vec![
        GeneratedArtifact {
            spec: INPUTS[0],
            payload: GeneratedPayload::Memory(vesta_generated.parameters),
        },
        GeneratedArtifact {
            spec: INPUTS[2],
            payload: GeneratedPayload::Memory(vesta_generated.verifying_key),
        },
        GeneratedArtifact {
            spec: INPUTS[3],
            payload: GeneratedPayload::Memory(vesta_generated.bootstrap_witness),
        },
        GeneratedArtifact {
            spec: INPUTS[4],
            payload: GeneratedPayload::Memory(pallas_generated.parameters),
        },
        GeneratedArtifact {
            spec: INPUTS[6],
            payload: GeneratedPayload::Memory(pallas_generated.verifying_key),
        },
        GeneratedArtifact {
            spec: INPUTS[7],
            payload: GeneratedPayload::Memory(pallas_generated.bootstrap_witness),
        },
    ];
    let metadata =
        BundleMetadata {
            chain_id,
            asset,
            asset_scale,
            generation,
            parameter_generation,
            source_commit,
            source_tree_sha256: parse_digest(options, "source-tree-sha256")?,
            reviewed_source_closure: source_identity.reviewed_source_closure.clone(),
            reviewed_source_closure_descriptor_sha256,
            activation_height,
            withdrawal_height,
            max_proof_bytes,
            measured_proof_pair,
            profiles: [
                ProfileMetadata {
                    parity: KagemushaPastaCycleParityV1::StepEq,
                    circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4,
                    circuit_params_sha256: vesta_generated.circuit_params.sha256().map_err(
                        |error| format!("failed to identify Eq CircuitParamsV4: {error}"),
                    )?,
                    circuit_params: vesta_generated.circuit_params,
                    compiled_protocol_structure_sha256: vesta_generated
                        .compiled_protocol_structure_sha256,
                    step_proof_size_bytes: vesta_generated.step_proof_size_bytes,
                },
                ProfileMetadata {
                    parity: KagemushaPastaCycleParityV1::StepEp,
                    circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
                    circuit_params_sha256: pallas_generated.circuit_params.sha256().map_err(
                        |error| format!("failed to identify Ep CircuitParamsV4: {error}"),
                    )?,
                    circuit_params: pallas_generated.circuit_params,
                    compiled_protocol_structure_sha256: pallas_generated
                        .compiled_protocol_structure_sha256,
                    step_proof_size_bytes: pallas_generated.step_proof_size_bytes,
                },
            ],
        };
    Ok(PreparedBundle {
        metadata,
        generated_artifacts,
        vesta_proving_key_size_bytes: vesta_generated.proving_key_size_bytes,
        pallas_proving_key_size_bytes: pallas_generated.proving_key_size_bytes,
    })
}

fn validate_generated_artifacts(artifacts: &[GeneratedArtifact]) -> Result<(), Box<dyn Error>> {
    if artifacts.len() != INPUTS.len()
        || artifacts
            .iter()
            .zip(INPUTS)
            .any(|(artifact, expected)| artifact.spec != expected)
    {
        return Err("generated Kagemusha payload inventory is not canonical".into());
    }
    let mut payload_digests = BTreeMap::new();
    for artifact in artifacts {
        let (payload_size, digest) = artifact.payload.identity()?;
        let duplicate = payload_digests.insert(digest, artifact.spec);
        if payload_size == 0
            || payload_size >= KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4
            || duplicate.is_some()
        {
            return Err(format!(
                "generated {} payload violates the V4 artifact corridor",
                artifact.spec.file_name
            )
            .into());
        }
    }
    Ok(())
}

fn profile_for(metadata: &BundleMetadata, parity: KagemushaPastaCycleParityV1) -> &ProfileMetadata {
    match parity {
        KagemushaPastaCycleParityV1::StepEq => &metadata.profiles[0],
        KagemushaPastaCycleParityV1::StepEp => &metadata.profiles[1],
    }
}

fn framed_schema_matches_release(framed_schema: &str, release_schema: &str) -> bool {
    framed_schema == release_schema
}

fn framed_generations_match_release_and_profile(
    framed_release: &str,
    release_id: &str,
    framed_parameters: &str,
    profile_parameters: &str,
) -> bool {
    framed_release == release_id && framed_parameters == profile_parameters
}

fn roster_generation_matches_release(archive_generation: &str, release_id: &str) -> bool {
    archive_generation == release_id
}

fn roster_generation_binding_is_exact(
    archive_generation: &str,
    descriptor_generation: &str,
    release_id: &str,
) -> bool {
    roster_generation_matches_release(archive_generation, release_id)
        && descriptor_generation == release_id
        && archive_generation == descriptor_generation
}

fn descriptor_matches_framed_artifact(
    descriptor: &KagemushaPastaCycleArtifactV4,
    header: &KagemushaPastaCycleFramedArtifactHeaderV4,
    expected: InputSpec,
    expected_size: u64,
) -> bool {
    descriptor.kind == header.kind
        && descriptor.file_name == expected.file_name
        && descriptor.size_bytes == expected_size
        && descriptor.payload_size_bytes == header.payload_size_bytes
        && descriptor.payload_sha256 == header.payload_sha256
}

fn validate_header_v4(
    header: &KagemushaPastaCycleFramedArtifactHeaderV4,
    profile: &ProfileMetadata,
) -> Result<(), String> {
    profile
        .circuit_params
        .validate()
        .map_err(|error| error.to_string())?;
    let expected_circuit_id = match header.parity {
        KagemushaPastaCycleParityV1::StepEq => KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4,
        KagemushaPastaCycleParityV1::StepEp => KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
    };
    let expected_params_sha256 = profile
        .circuit_params
        .sha256()
        .map_err(|error| error.to_string())?;
    let encoded_header = norito::to_bytes(header)
        .map_err(|error| format!("failed to encode Kagemusha V4 artifact header: {error}"))?;
    let encoded_header_len = u32::try_from(encoded_header.len())
        .map_err(|_| "Kagemusha V4 header length does not fit u32".to_owned())?;
    let framed_size = u64::try_from(KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_KEY_MAGIC_V4.len() + 4)
        .ok()
        .and_then(|size| size.checked_add(u64::from(encoded_header_len)))
        .and_then(|size| size.checked_add(header.payload_size_bytes))
        .ok_or_else(|| "Kagemusha V4 framed artifact size overflow".to_owned())?;
    if header.version != KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_HEADER_VERSION_V4
        || header.manifest_schema != KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4
        || header.bridge_abi_version != KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4
        || header.proof_backend != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4
        || header.transcript_profile != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4
        || !is_kagemusha_v4_portable_identifier(&header.generation)
        || header.parity != profile.parity
        || header.circuit_id != expected_circuit_id
        || header.circuit_id != profile.circuit_id
        || !is_kagemusha_v4_portable_identifier(&header.parameter_generation)
        || header.ipa_k != profile.circuit_params.k
        || header.circuit_params_sha256 != expected_params_sha256
        || header.circuit_params_sha256 != profile.circuit_params_sha256
        || header.compiled_protocol_structure_sha256 == [0; 32]
        || header.compiled_protocol_structure_sha256 != profile.compiled_protocol_structure_sha256
        || header.step_proof_size_bytes == 0
        || header.step_proof_size_bytes != profile.step_proof_size_bytes
        || header.step_proof_size_bytes != profile.circuit_params.max_parent_proof_bytes
        || header.payload_size_bytes == 0
        || header.payload_size_bytes >= KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4
        || header.payload_sha256 == [0; 32]
        || encoded_header_len == 0
        || encoded_header_len > KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_HEADER_MAX_BYTES_V4
        || framed_size > KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4
    {
        return Err(
            "Kagemusha V4 artifact header violates its bounded profile contract".to_owned(),
        );
    }
    Ok(())
}

fn validate_header_against_candidate_v4(
    header: &KagemushaPastaCycleFramedArtifactHeaderV4,
    manifest: &KagemushaRecursiveSpendArtifactManifestV4,
    descriptor: &KagemushaPastaCycleArtifactV4,
) -> Result<(), String> {
    manifest
        .validate_unsigned_candidate()
        .map_err(|error| error.to_string())?;
    descriptor.validate().map_err(|error| error.to_string())?;
    let profile = manifest
        .profiles
        .iter()
        .find(|profile| profile.parity == header.parity)
        .ok_or_else(|| "Kagemusha V4 artifact parity is absent from the manifest".to_owned())?;
    let profile_metadata = ProfileMetadata {
        parity: profile.parity,
        circuit_id: match profile.parity {
            KagemushaPastaCycleParityV1::StepEq => KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4,
            KagemushaPastaCycleParityV1::StepEp => KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
        },
        circuit_params: profile.circuit_params.clone(),
        circuit_params_sha256: profile
            .circuit_params_sha256()
            .map_err(|error| error.to_string())?,
        compiled_protocol_structure_sha256: profile.compiled_protocol_structure_sha256,
        step_proof_size_bytes: profile.step_proof_size_bytes,
    };
    validate_header_v4(header, &profile_metadata)?;
    let expected_spec = INPUTS
        .iter()
        .find(|spec| spec.parity == header.parity && spec.kind == header.kind)
        .ok_or_else(|| "Kagemusha V4 artifact header selects an unknown role".to_owned())?;
    let encoded_header = norito::to_bytes(header)
        .map_err(|error| format!("failed to encode Kagemusha V4 artifact header: {error}"))?;
    let encoded_header_len = u32::try_from(encoded_header.len())
        .map_err(|_| "Kagemusha V4 header length does not fit u32".to_owned())?;
    let expected_size = u64::try_from(KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_KEY_MAGIC_V4.len() + 4)
        .ok()
        .and_then(|size| size.checked_add(u64::from(encoded_header_len)))
        .and_then(|size| size.checked_add(header.payload_size_bytes))
        .ok_or_else(|| "Kagemusha V4 framed artifact size overflow".to_owned())?;
    if encoded_header_len == 0
        || encoded_header_len > KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_HEADER_MAX_BYTES_V4
        || expected_size > KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4
        || !framed_schema_matches_release(&header.manifest_schema, &manifest.schema)
        || header.bridge_abi_version != manifest.bridge_abi_version
        || header.proof_backend != manifest.proof_backend
        || header.transcript_profile != manifest.transcript_profile
        || !framed_generations_match_release_and_profile(
            &header.generation,
            &manifest.generation,
            &header.parameter_generation,
            &profile.parameter_generation,
        )
        || header.ipa_k != profile.ipa_k
        || !descriptor_matches_framed_artifact(descriptor, header, *expected_spec, expected_size)
        || profile
            .artifacts
            .iter()
            .find(|artifact| artifact.kind == header.kind)
            != Some(descriptor)
    {
        return Err("Kagemusha V4 artifact header is not exactly manifest-bound".to_owned());
    }
    Ok(())
}

fn prepare_artifact(
    artifact: GeneratedArtifact,
    metadata: &BundleMetadata,
) -> Result<PreparedArtifact, Box<dyn Error>> {
    let GeneratedArtifact { spec, payload } = artifact;
    let profile = profile_for(metadata, spec.parity);
    let (payload_size_bytes, payload_sha256) = payload.identity()?;
    let header = KagemushaPastaCycleFramedArtifactHeaderV4 {
        version: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_HEADER_VERSION_V4,
        manifest_schema: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4.to_owned(),
        bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
        proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
        transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4.to_owned(),
        generation: metadata.generation.clone(),
        parity: spec.parity,
        circuit_id: profile.circuit_id.to_owned(),
        parameter_generation: metadata.parameter_generation.clone(),
        ipa_k: profile.circuit_params.k,
        circuit_params_sha256: profile.circuit_params_sha256,
        compiled_protocol_structure_sha256: profile.compiled_protocol_structure_sha256,
        step_proof_size_bytes: profile.step_proof_size_bytes,
        kind: spec.kind,
        payload_size_bytes,
        payload_sha256,
    };
    validate_header_v4(&header, profile)
        .map_err(|error| format!("invalid {} header: {error}", spec.file_name))?;
    let header_bytes = norito::to_bytes(&header)?;
    let header_len = u32::try_from(header_bytes.len())?;
    if header_len == 0 || header_len > KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_HEADER_MAX_BYTES_V4 {
        return Err(format!("{} header exceeds the V4 bound", spec.file_name).into());
    }
    let total_size = u64::try_from(KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_KEY_MAGIC_V4.len() + 4)?
        .checked_add(u64::from(header_len))
        .and_then(|size| size.checked_add(payload_size_bytes))
        .ok_or_else(|| io::Error::other("V4 framed artifact size overflow"))?;
    if total_size > KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4 {
        return Err(format!("{} exceeds the V4 framed-file bound", spec.file_name).into());
    }
    Ok(PreparedArtifact {
        spec,
        payload,
        circuit_params: profile.circuit_params.clone(),
        compiled_protocol_structure_sha256: profile.compiled_protocol_structure_sha256,
        step_proof_size_bytes: profile.step_proof_size_bytes,
        header,
        total_size,
    })
}

fn trusted_source_command(executable: &str) -> Command {
    let mut command = Command::new(executable);
    command.env_clear().env("PATH", TRUSTED_TOOL_PATH);
    for variable in ["HOME", "GNUPGHOME"] {
        if let Some(value) = env::var_os(variable) {
            command.env(variable, value);
        }
    }
    command
}

fn validate_signed_base_source(source_commit: &str) -> Result<(), Box<dyn Error>> {
    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let head = trusted_source_command(TRUSTED_GIT_EXECUTABLE)
        .arg("--no-optional-locks")
        .arg("-C")
        .arg(&repository_root)
        .args(["rev-parse", "--verify", "HEAD^{commit}"])
        .output()
        .map_err(|error| format!("failed to inspect candidate Git HEAD: {error}"))?;
    if !head.status.success() {
        return Err("candidate source is not a Git checkout with a commit HEAD".into());
    }
    let head = std::str::from_utf8(&head.stdout)
        .map_err(|_| "candidate Git HEAD is not canonical UTF-8")?
        .trim_end_matches(['\r', '\n']);
    if head != source_commit {
        return Err(format!(
            "--source-commit must exactly equal the checked-out candidate HEAD ({head})"
        )
        .into());
    }

    let signature = trusted_source_command(TRUSTED_GIT_EXECUTABLE)
        .arg("--no-optional-locks")
        .arg("-C")
        .arg(&repository_root)
        .args(["verify-commit", source_commit])
        .output()
        .map_err(|error| format!("failed to verify candidate commit signature: {error}"))?;
    if !signature.status.success() {
        return Err("candidate commit must carry a locally verifiable signature".into());
    }
    Ok(())
}

fn is_lower_hex(value: &str, expected_len: usize) -> bool {
    value.len() == expected_len
        && value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

fn read_source_tree_identity() -> Result<FullSourceTreeIdentityV1, Box<dyn Error>> {
    let (Some(reviewed_closure), Some(reviewed_closure_sha256)) = (
        BUILD_REVIEWED_SOURCE_CLOSURE,
        BUILD_REVIEWED_SOURCE_CLOSURE_SHA256,
    ) else {
        return Err("candidate generation requires an embedded reviewed source-closure pin".into());
    };
    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let seal_script = repository_root.join("scripts/kagemusha_source_tree_seal.py");
    let output = trusted_source_command(TRUSTED_PYTHON_EXECUTABLE)
        .arg("-I")
        .arg(&seal_script)
        .arg("identity")
        .arg("--root")
        .arg(&repository_root)
        .arg("--reviewed-source-closure")
        .arg(reviewed_closure)
        .arg("--reviewed-source-closure-sha256")
        .arg(reviewed_closure_sha256)
        .output()
        .map_err(|error| format!("failed to run the Kagemusha source-tree seal: {error}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Kagemusha source-tree seal rejected the checkout: {detail}").into());
    }
    let identity: FullSourceTreeIdentityV1 =
        norito::json::from_slice(&output.stdout).map_err(|error| {
            format!("Kagemusha source-tree identity is not canonical JSON: {error}")
        })?;
    if identity.schema != "iroha.kagemusha.reviewed_source_tree_identity.v1"
        || !identity.source_repo_dirty
        || !is_lower_hex(&identity.source_commit, 40)
        || !is_lower_hex(&identity.source_tree_sha256, 64)
        || !is_lower_hex(&identity.reviewed_source_closure_descriptor_sha256, 64)
        || identity.reviewed_source_closure.validate().is_err()
        || identity.reviewed_source_closure.source_commit != identity.source_commit
        || hex::encode(identity.reviewed_source_closure.source_tree_sha256)
            != identity.source_tree_sha256
        || !identity.reviewed_source_closure.source_repo_dirty
        || identity
            .reviewed_source_closure
            .canonical_descriptor_sha256()
            .map(hex::encode)
            .ok()
            .as_deref()
            != Some(identity.reviewed_source_closure_descriptor_sha256.as_str())
    {
        return Err("Kagemusha reviewed source-closure identity is malformed".into());
    }
    Ok(identity)
}

fn validate_current_source(
    expected_commit: &str,
    expected_tree_sha256: [u8; 32],
) -> Result<FullSourceTreeIdentityV1, Box<dyn Error>> {
    let expected_tree_sha256 = hex::encode(expected_tree_sha256);
    let first = read_source_tree_identity()?;
    if first.source_commit != expected_commit || first.source_tree_sha256 != expected_tree_sha256 {
        return Err(
            "Kagemusha source commit/tree pair does not identify the exact reviewed closure".into(),
        );
    }
    validate_signed_base_source(expected_commit)?;
    let second = read_source_tree_identity()?;
    if second != first {
        return Err(
            "Kagemusha reviewed source closure changed during signature verification".into(),
        );
    }
    Ok(first)
}

fn validate_current_manifest_source(
    manifest: &KagemushaRecursiveSpendArtifactManifestV4,
) -> Result<(), Box<dyn Error>> {
    let current = validate_current_source(&manifest.source_commit, manifest.source_tree_sha256)?;
    if !manifest.source_repo_dirty
        || manifest.reviewed_source_closure != current.reviewed_source_closure
        || hex::encode(manifest.reviewed_source_closure_descriptor_sha256)
            != current.reviewed_source_closure_descriptor_sha256
    {
        return Err(
            "candidate manifest does not bind the exact embedded reviewed source closure".into(),
        );
    }
    Ok(())
}

fn validate_embedded_candidate_source(
    expected_commit: &str,
    expected_tree_sha256: [u8; 32],
    embedded_commit: Option<&str>,
    embedded_tree_sha256: Option<&str>,
    debug_assertions_enabled: bool,
) -> Result<(), Box<dyn Error>> {
    if debug_assertions_enabled {
        return Err(
            "candidate generation requires a source-sealed release binary without debug assertions"
                .into(),
        );
    }
    let (Some(embedded_commit), Some(embedded_tree_sha256)) =
        (embedded_commit, embedded_tree_sha256)
    else {
        return Err(
            "candidate generation requires a source-sealed binary built by scripts/build_kagemusha_v4_candidate_bundle.py"
                .into(),
        );
    };
    if !is_lower_hex(embedded_commit, 40) || !is_lower_hex(embedded_tree_sha256, 64) {
        return Err("embedded Kagemusha candidate source seal is malformed".into());
    }
    if embedded_commit != expected_commit
        || embedded_tree_sha256 != hex::encode(expected_tree_sha256)
    {
        return Err(
            "candidate request does not match the source identity embedded in this binary".into(),
        );
    }
    Ok(())
}

#[cfg_attr(
    any(target_os = "linux", target_os = "android", target_os = "macos"),
    expect(
        clippy::too_many_lines,
        reason = "candidate generation is one ordered capability-consuming and atomically staged publication flow"
    )
)]
#[cfg_attr(
    not(any(target_os = "linux", target_os = "android", target_os = "macos")),
    expect(
        unused_variables,
        reason = "unsupported targets reject before the one-shot generation permit can be consumed"
    )
)]
fn build_candidate(
    options: &BTreeMap<String, String>,
    supervisor_permit: KagemushaGenerationSupervisorPermitV4,
) -> Result<(), Box<dyn Error>> {
    #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos")))]
    return Err(
        "Kagemusha V4 bundle publication requires Linux, Android, or macOS atomic directory publication"
            .into(),
    );

    #[cfg(any(target_os = "linux", target_os = "android", target_os = "macos"))]
    {
        let expected_tree = parse_digest(options, "source-tree-sha256")?;
        let source_identity =
            validate_current_source(required(options, "source-commit"), expected_tree)?;
        validate_embedded_candidate_source(
            required(options, "source-commit"),
            expected_tree,
            BUILD_SOURCE_COMMIT,
            BUILD_SOURCE_TREE_SHA256,
            cfg!(debug_assertions),
        )?;
        let out_dir = PathBuf::from(required(options, "out-dir"));
        if out_dir.exists() {
            return Err(format!("output directory already exists: {}", out_dir.display()).into());
        }
        let trusted_parent =
            TrustedOutputParent::open_pinned(&out_dir, required(options, "output-parent-fd"))?;

        let staging_id = required(options, "staging-id");
        if !is_lower_hex(staging_id, 32) {
            return Err("guard-supplied Kagemusha staging id is invalid".into());
        }
        let staging_prefix = format!(".kagemusha-v4-staging-{staging_id}-");
        let staging_name = required(options, "staging-name");
        if staging_name != format!("{staging_prefix}work") {
            return Err("guard-supplied Kagemusha staging name is invalid".into());
        }
        let publication = PublicationDirectory::open_at(
            &trusted_parent.file,
            trusted_parent.path.join(staging_name),
            std::ffi::OsStr::new(staging_name),
        )?;
        let mut vesta_proving_key_output =
            BoundedDigestFileWriter::new(&publication, ".step-eq-proving-key.part")?;
        let mut pallas_proving_key_output =
            BoundedDigestFileWriter::new(&publication, ".step-ep-proving-key.part")?;
        let mut vesta_params_input = open_input(
            Path::new(required(options, "step-eq-circuit-params")),
            1024 * 1024,
            "Eq inline circuit parameters",
        )?;
        let mut pallas_params_input = open_input(
            Path::new(required(options, "step-ep-circuit-params")),
            1024 * 1024,
            "Ep inline circuit parameters",
        )?;
        let PreparedBundle {
            metadata,
            mut generated_artifacts,
            vesta_proving_key_size_bytes,
            pallas_proving_key_size_bytes,
        } = prepare_bundle_metadata(
            options,
            &source_identity,
            &mut vesta_params_input,
            &mut pallas_params_input,
            supervisor_permit,
            &mut vesta_proving_key_output,
            &mut pallas_proving_key_output,
        )?;
        generated_artifacts.push(GeneratedArtifact {
            spec: INPUTS[1],
            payload: GeneratedPayload::Staged(
                vesta_proving_key_output.finish(vesta_proving_key_size_bytes, "Eq proving key")?,
            ),
        });
        generated_artifacts.push(GeneratedArtifact {
            spec: INPUTS[5],
            payload: GeneratedPayload::Staged(
                pallas_proving_key_output
                    .finish(pallas_proving_key_size_bytes, "Ep proving key")?,
            ),
        });
        generated_artifacts.sort_by_key(|artifact| {
            INPUTS
                .iter()
                .position(|expected| *expected == artifact.spec)
                .unwrap_or(INPUTS.len())
        });
        validate_generated_artifacts(&generated_artifacts)?;
        let prepared = generated_artifacts
            .into_iter()
            .map(|artifact| prepare_artifact(artifact, &metadata))
            .collect::<Result<Vec<_>, _>>()?;

        let mut roster_input = open_input(
            Path::new(required(options, "topup-finality-roster")),
            KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_V2,
            "top-up finality roster",
        )?;
        #[cfg(unix)]
        if roster_input.snapshot.identity() == vesta_params_input.snapshot.identity()
            || roster_input.snapshot.identity() == pallas_params_input.snapshot.identity()
        {
            return Err("top-up finality roster aliases an inline circuit-parameter input".into());
        }
        let (roster_bytes, roster_descriptor) =
            prepare_topup_finality_roster(&mut roster_input, &metadata)?;
        if prepared
            .iter()
            .any(|artifact| artifact.header.payload_sha256 == roster_descriptor.sha256)
        {
            return Err("top-up finality roster aliases a cryptographic artifact digest".into());
        }

        if let Err(error) = write_candidate(
            &publication,
            metadata,
            prepared,
            &roster_bytes,
            roster_descriptor,
        ) {
            return Err(format!(
                "V4 bundle publication did not complete; no output was published at {}: {error}",
                out_dir.display()
            )
            .into());
        }
        validate_current_source(required(options, "source-commit"), expected_tree)?;
        verify_staged_candidate_for_publication(
            &publication,
            required(options, "source-commit"),
            expected_tree,
        )?;
        publication.sync()?;
        validate_current_source(required(options, "source-commit"), expected_tree)?;
        // Publication is deliberately deferred until the supervising launcher
        // has observed the child's exit and the kernel RSS high-water mark.
        // `publish-staged-candidate` reopens and verifies this exact hidden
        // directory under the still-held host-global locks.
        Ok(())
    }
}

fn publish_staged_candidate(
    options: &BTreeMap<String, String>,
    _supervisor_permit: KagemushaGenerationSupervisorPermitV4,
) -> Result<(), Box<dyn Error>> {
    #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos")))]
    return Err(
        "Kagemusha V4 bundle publication requires Linux, Android, or macOS atomic directory publication"
            .into(),
    );

    #[cfg(any(target_os = "linux", target_os = "android", target_os = "macos"))]
    {
        let expected_tree = parse_digest(options, "source-tree-sha256")?;
        validate_current_source(required(options, "source-commit"), expected_tree)?;
        validate_embedded_candidate_source(
            required(options, "source-commit"),
            expected_tree,
            BUILD_SOURCE_COMMIT,
            BUILD_SOURCE_TREE_SHA256,
            cfg!(debug_assertions),
        )?;
        let out_dir = PathBuf::from(required(options, "out-dir"));
        let trusted_parent =
            TrustedOutputParent::open_pinned(&out_dir, required(options, "output-parent-fd"))?;
        let staging_id = required(options, "staging-id");
        if !is_lower_hex(staging_id, 32) {
            return Err("guard-supplied Kagemusha staging id is invalid".into());
        }
        let prefix = format!(".kagemusha-v4-staging-{staging_id}-");
        let staging_name = required(options, "staging-name");
        if staging_name != format!("{prefix}work") {
            return Err("guard-supplied Kagemusha staging name is invalid".into());
        }
        let publication = PublicationDirectory::open_at(
            &trusted_parent.file,
            trusted_parent.path.join(staging_name),
            std::ffi::OsStr::new(staging_name),
        )?;
        publication.sync()?;
        trusted_parent.file.sync_all()?;
        validate_current_source(required(options, "source-commit"), expected_tree)?;
        verify_staged_candidate_for_publication(
            &publication,
            required(options, "source-commit"),
            expected_tree,
        )?;
        trusted_parent.publish_presynced(std::ffi::OsStr::new(staging_name), &publication)
    }
}

fn open_and_match_candidate_file(
    candidate: &PublicationDirectory,
    name: &str,
    maximum: u64,
    label: &str,
    expected: &[u8],
    tracked: &mut Vec<OpenedInput>,
) -> Result<(), Box<dyn Error>> {
    let mut input = candidate.open_bound_input(name, maximum, label)?;
    if input.read_all()? != expected {
        return Err(format!("{label} does not match the canonical candidate value").into());
    }
    tracked.push(input);
    Ok(())
}

#[expect(
    clippy::too_many_lines,
    reason = "the staged-candidate audit must retain one ordered inode, digest, schema, artifact, roster, and source revalidation flow"
)]
fn verify_staged_candidate_for_publication(
    candidate: &PublicationDirectory,
    expected_commit: &str,
    expected_tree_sha256: [u8; 32],
) -> Result<(), Box<dyn Error>> {
    candidate.verify_candidate_inventory()?;
    let mut tracked_metadata = Vec::new();
    let mut candidate_input = candidate.open_bound_input(
        CANDIDATE_MANIFEST_NORITO_FILE_NAME,
        MAX_MANIFEST_BYTES,
        "staged V4 candidate record",
    )?;
    let candidate_bytes = candidate_input.read_all()?;
    let candidate_record: KagemushaRecursiveSpendCandidateV4 =
        decode_canonical_norito(&candidate_bytes, "staged V4 candidate record")?;
    candidate_record
        .validate()
        .map_err(|error| format!("invalid staged V4 candidate record: {error}"))?;
    let manifest = &candidate_record.manifest;
    let current_source = validate_current_source(expected_commit, expected_tree_sha256)?;
    if manifest.source_commit != expected_commit
        || manifest.source_tree_sha256 != expected_tree_sha256
        || !manifest.source_repo_dirty
        || manifest.reviewed_source_closure != current_source.reviewed_source_closure
        || hex::encode(manifest.reviewed_source_closure_descriptor_sha256)
            != current_source.reviewed_source_closure_descriptor_sha256
    {
        return Err("staged V4 candidate source identity changed before publication".into());
    }
    tracked_metadata.push(candidate_input);

    let candidate_sha256: [u8; 32] = Sha256::digest(&candidate_bytes).into();
    if candidate_record
        .sha256()
        .map_err(|error| format!("failed to identify staged V4 candidate: {error}"))?
        != candidate_sha256
    {
        return Err("staged V4 candidate identity changed before publication".into());
    }
    let mut candidate_json = norito::json::to_string_pretty(&candidate_record)?;
    candidate_json.push('\n');
    let candidate_sha256_text = format!("{}\n", hex::encode(candidate_sha256));
    for (name, maximum, label, expected) in [
        (
            CANDIDATE_MANIFEST_JSON_FILE_NAME,
            MAX_MANIFEST_BYTES,
            "staged V4 candidate JSON",
            candidate_json.as_bytes(),
        ),
        (
            CANDIDATE_MANIFEST_SHA256_FILE_NAME,
            65,
            "staged V4 candidate digest",
            candidate_sha256_text.as_bytes(),
        ),
    ] {
        open_and_match_candidate_file(
            candidate,
            name,
            maximum,
            label,
            expected,
            &mut tracked_metadata,
        )?;
    }

    let descriptors = manifest
        .profiles
        .iter()
        .flat_map(|profile| profile.artifacts.iter())
        .collect::<Vec<_>>();
    if descriptors.len() != INPUTS.len() {
        return Err("staged V4 candidate does not have eight artifacts".into());
    }
    let mut artifact_inputs = Vec::with_capacity(INPUTS.len());
    for (spec, descriptor) in INPUTS.iter().zip(descriptors) {
        if descriptor.file_name != spec.file_name {
            return Err("staged V4 candidate artifact order or name changed".into());
        }
        candidate.verify_candidate_framed_artifact(manifest, descriptor)?;
        let input = candidate.open_bound_input(
            &descriptor.file_name,
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4,
            "staged V4 candidate artifact",
        )?;
        if input.size_bytes != descriptor.size_bytes || input.sha256 != descriptor.sha256 {
            return Err(format!(
                "staged V4 candidate artifact changed: {}",
                descriptor.file_name
            )
            .into());
        }
        artifact_inputs.push(input);
    }

    let roster_descriptor = &manifest.topup_finality_roster_artifact;
    let mut roster_input = candidate.open_bound_input(
        &roster_descriptor.file_name,
        KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_V2,
        "staged V4 candidate top-up finality roster",
    )?;
    if roster_input.size_bytes != roster_descriptor.size_bytes
        || roster_input.sha256 != roster_descriptor.sha256
    {
        return Err("staged V4 candidate top-up finality roster changed".into());
    }
    let roster_bytes = roster_input.read_all()?;
    let roster: KagemushaTopUpFinalityRosterArtifactV2 =
        decode_canonical_norito(&roster_bytes, "staged V4 top-up finality roster")?;
    roster
        .validate()
        .map_err(|error| format!("invalid staged V4 top-up finality roster: {error}"))?;
    if roster.chain_id != manifest.chain_id
        || !roster_generation_binding_is_exact(
            &roster.artifact_generation,
            &roster_descriptor.artifact_generation,
            &manifest.generation,
        )
        || roster_descriptor.file_name != KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V4
    {
        return Err("staged V4 top-up finality roster is not manifest-bound".into());
    }
    let mut covered_until = manifest.activation_height;
    for window in &roster.windows {
        if window.withdraws_at_height <= covered_until {
            continue;
        }
        if window.activates_at_height > covered_until {
            return Err(format!(
                "staged V4 top-up finality roster has a gap at height {covered_until}"
            )
            .into());
        }
        covered_until = window.withdraws_at_height;
        if covered_until >= manifest.withdrawal_height {
            break;
        }
    }
    if covered_until < manifest.withdrawal_height {
        return Err("staged V4 top-up finality roster does not cover the release window".into());
    }

    for input in &mut tracked_metadata {
        input.rehash_and_verify()?;
    }
    for input in &mut artifact_inputs {
        input.rehash_and_verify()?;
    }
    roster_input.rehash_and_verify()?;
    candidate.verify_candidate_inventory()?;
    Ok(())
}

#[cfg_attr(
    any(target_os = "linux", target_os = "android", target_os = "macos"),
    expect(
        clippy::too_many_lines,
        reason = "candidate validation and its atomically published evidence report form one ordered fail-closed audit"
    )
)]
fn validate_candidate(options: &BTreeMap<String, String>) -> Result<(), Box<dyn Error>> {
    #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos")))]
    return Err("Kagemusha V4 candidate validation requires Linux, Android, or macOS".into());

    #[cfg(any(target_os = "linux", target_os = "android", target_os = "macos"))]
    {
        use std::os::unix::fs::PermissionsExt as _;

        let candidate =
            PublicationDirectory::open_existing(PathBuf::from(required(options, "candidate-dir")))?;
        candidate.verify_candidate_inventory()?;

        let mut tracked_metadata = Vec::new();
        let mut candidate_input = candidate.open_bound_input(
            CANDIDATE_MANIFEST_NORITO_FILE_NAME,
            MAX_MANIFEST_BYTES,
            "V4 candidate record",
        )?;
        let candidate_bytes = candidate_input.read_all()?;
        let candidate_record: KagemushaRecursiveSpendCandidateV4 =
            decode_canonical_norito(&candidate_bytes, "V4 candidate record")?;
        candidate_record
            .validate()
            .map_err(|error| format!("invalid V4 candidate record: {error}"))?;
        let manifest = &candidate_record.manifest;
        validate_current_manifest_source(manifest)?;
        tracked_metadata.push(candidate_input);

        let candidate_sha256: [u8; 32] = Sha256::digest(&candidate_bytes).into();
        if candidate_record
            .sha256()
            .map_err(|error| format!("failed to identify V4 candidate record: {error}"))?
            != candidate_sha256
        {
            return Err("canonical V4 candidate identity changed while validating".into());
        }
        let mut candidate_json = norito::json::to_string_pretty(&candidate_record)?;
        candidate_json.push('\n');
        let candidate_sha256_text = format!("{}\n", hex::encode(candidate_sha256));
        for (name, maximum, label, expected) in [
            (
                CANDIDATE_MANIFEST_JSON_FILE_NAME,
                MAX_MANIFEST_BYTES,
                "V4 candidate JSON",
                candidate_json.as_bytes(),
            ),
            (
                CANDIDATE_MANIFEST_SHA256_FILE_NAME,
                65,
                "V4 candidate digest",
                candidate_sha256_text.as_bytes(),
            ),
        ] {
            open_and_match_candidate_file(
                &candidate,
                name,
                maximum,
                label,
                expected,
                &mut tracked_metadata,
            )?;
        }

        let manifest_bytes = canonical_norito_bytes(manifest, "unsigned V4 candidate manifest")?;
        let manifest_sha256: [u8; 32] = Sha256::digest(&manifest_bytes).into();
        let descriptors = manifest
            .profiles
            .iter()
            .flat_map(|profile| profile.artifacts.iter())
            .collect::<Vec<_>>();
        if descriptors.len() != INPUTS.len() {
            return Err("V4 candidate does not contain the exact eight-artifact inventory".into());
        }

        let mut artifact_inputs = Vec::with_capacity(INPUTS.len());
        let mut artifact_report = Vec::with_capacity(INPUTS.len());
        for (((role, spec), descriptor), expected_index) in
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4
                .iter()
                .zip(INPUTS.iter())
                .zip(descriptors.iter())
                .zip(0_usize..)
        {
            if descriptor.file_name != spec.file_name {
                return Err(format!(
                    "V4 candidate artifact {expected_index} has a non-canonical file name"
                )
                .into());
            }
            candidate.verify_candidate_framed_artifact(manifest, descriptor)?;
            let input = candidate.open_bound_input(
                &descriptor.file_name,
                KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4,
                "V4 candidate artifact",
            )?;
            if input.size_bytes != descriptor.size_bytes || input.sha256 != descriptor.sha256 {
                return Err(
                    format!("V4 candidate artifact changed: {}", descriptor.file_name).into(),
                );
            }
            artifact_report.push(CandidateValidationArtifactV1 {
                role: (*role).to_owned(),
                file_name: descriptor.file_name.clone(),
                framed_size_bytes: descriptor.size_bytes,
                framed_sha256: hex::encode(descriptor.sha256),
                payload_size_bytes: descriptor.payload_size_bytes,
                payload_sha256: hex::encode(descriptor.payload_sha256),
            });
            artifact_inputs.push(input);
        }

        let roster_descriptor = &manifest.topup_finality_roster_artifact;
        let mut roster_input = candidate.open_bound_input(
            &roster_descriptor.file_name,
            KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_V2,
            "V4 candidate top-up finality roster",
        )?;
        if roster_input.size_bytes != roster_descriptor.size_bytes
            || roster_input.sha256 != roster_descriptor.sha256
        {
            return Err("V4 candidate top-up finality roster changed".into());
        }
        let roster_bytes = roster_input.read_all()?;
        let roster: KagemushaTopUpFinalityRosterArtifactV2 =
            decode_canonical_norito(&roster_bytes, "V4 candidate top-up finality roster")?;
        roster
            .validate()
            .map_err(|error| format!("invalid V4 candidate top-up finality roster: {error}"))?;
        if roster.chain_id != manifest.chain_id
            || !roster_generation_binding_is_exact(
                &roster.artifact_generation,
                &roster_descriptor.artifact_generation,
                &manifest.generation,
            )
            || roster_descriptor.file_name != KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V4
        {
            return Err("V4 candidate top-up finality roster is not manifest-bound".into());
        }
        let mut covered_until = manifest.activation_height;
        for window in &roster.windows {
            if window.withdraws_at_height <= covered_until {
                continue;
            }
            if window.activates_at_height > covered_until {
                return Err(format!(
                    "V4 candidate top-up finality roster has a gap at height {covered_until}"
                )
                .into());
            }
            covered_until = window.withdraws_at_height;
            if covered_until >= manifest.withdrawal_height {
                break;
            }
        }
        if covered_until < manifest.withdrawal_height {
            return Err(
                "V4 candidate top-up finality roster does not cover the release window".into(),
            );
        }

        let report = CandidateValidationReportV1 {
            schema: CANDIDATE_VALIDATION_REPORT_SCHEMA_V1.to_owned(),
            candidate_record_sha256: hex::encode(candidate_sha256),
            candidate_manifest_sha256: hex::encode(manifest_sha256),
            source_commit: manifest.source_commit.clone(),
            source_tree_sha256: hex::encode(manifest.source_tree_sha256),
            source_repo_dirty: manifest.source_repo_dirty,
            reviewed_source_closure_descriptor_sha256: hex::encode(
                manifest.reviewed_source_closure_descriptor_sha256,
            ),
            generation: manifest.generation.clone(),
            bridge_abi_version: manifest.bridge_abi_version,
            artifact_count: u32::try_from(artifact_report.len())?,
            artifacts: artifact_report,
            topup_finality_roster_file_name: roster_descriptor.file_name.clone(),
            topup_finality_roster_size_bytes: roster_descriptor.size_bytes,
            topup_finality_roster_sha256: hex::encode(roster_descriptor.sha256),
        };
        let mut report_json = norito::json::to_string_pretty(&report)?;
        report_json.push('\n');

        for input in &mut tracked_metadata {
            input.rehash_and_verify()?;
        }
        for input in &mut artifact_inputs {
            input.rehash_and_verify()?;
        }
        roster_input.rehash_and_verify()?;
        candidate.verify_candidate_inventory()?;
        validate_current_manifest_source(manifest)?;

        let out_dir = PathBuf::from(required(options, "out-dir"));
        if out_dir.exists() {
            return Err(format!("output directory already exists: {}", out_dir.display()).into());
        }
        let repository_root =
            fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))?;
        let output_parent = fs::canonicalize(
            out_dir
                .parent()
                .filter(|path| !path.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new(".")),
        )?;
        if output_parent.starts_with(&repository_root) {
            return Err("candidate validation output must be outside the source repository".into());
        }
        let trusted_parent = TrustedOutputParent::open(&out_dir)?;
        let mut staging_builder = tempfile::Builder::new();
        staging_builder
            .prefix(".kagemusha-v4-validation-staging-")
            .permissions(fs::Permissions::from_mode(0o700));
        let staging = staging_builder.tempdir_in(&trusted_parent.path)?;
        let staging_name = staging
            .path()
            .file_name()
            .ok_or("temporary validation directory has no file name")?
            .to_owned();
        let publication = PublicationDirectory::open_at(
            &trusted_parent.file,
            staging.path().to_owned(),
            &staging_name,
        )?;
        publication
            .write_exact_file(CANDIDATE_VALIDATION_MANIFEST_FILE_NAME_V4, &manifest_bytes)?;
        publication.write_exact_file(
            CANDIDATE_VALIDATION_REPORT_FILE_NAME_V1,
            report_json.as_bytes(),
        )?;
        publication.verify_exact_file(
            CANDIDATE_VALIDATION_MANIFEST_FILE_NAME_V4,
            &manifest_bytes,
            MAX_MANIFEST_BYTES,
        )?;
        publication.verify_exact_file(
            CANDIDATE_VALIDATION_REPORT_FILE_NAME_V1,
            report_json.as_bytes(),
            MAX_MANIFEST_BYTES,
        )?;
        publication.verify_inventory(&BTreeSet::from([
            CANDIDATE_VALIDATION_MANIFEST_FILE_NAME_V4.to_owned(),
            CANDIDATE_VALIDATION_REPORT_FILE_NAME_V1.to_owned(),
        ]))?;
        publication.sync()?;
        for input in &mut tracked_metadata {
            input.rehash_and_verify()?;
        }
        for input in &mut artifact_inputs {
            input.rehash_and_verify()?;
        }
        roster_input.rehash_and_verify()?;
        candidate.verify_candidate_inventory()?;
        validate_current_manifest_source(manifest)?;
        trusted_parent.publish(&staging_name, &publication)?;
        let _published = staging.keep();
        Ok(())
    }
}

#[cfg_attr(
    any(target_os = "linux", target_os = "android", target_os = "macos"),
    expect(
        clippy::too_many_lines,
        reason = "release authentication, byte-for-byte copying, revalidation, and atomic publication form one ordered security ceremony"
    )
)]
fn finalize_release(options: &BTreeMap<String, String>) -> Result<(), Box<dyn Error>> {
    #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos")))]
    return Err(
        "Kagemusha V4 release finalization requires Linux, Android, or macOS atomic directory publication"
            .into(),
    );

    #[cfg(any(target_os = "linux", target_os = "android", target_os = "macos"))]
    {
        use std::os::unix::fs::PermissionsExt as _;

        let candidate =
            PublicationDirectory::open_existing(PathBuf::from(required(options, "candidate-dir")))?;
        candidate.verify_candidate_inventory()?;

        let mut tracked_metadata = Vec::new();
        let mut candidate_manifest_input = candidate.open_bound_input(
            CANDIDATE_MANIFEST_NORITO_FILE_NAME,
            MAX_MANIFEST_BYTES,
            "V4 candidate manifest",
        )?;
        let candidate_manifest_bytes = candidate_manifest_input.read_all()?;
        let candidate_record: KagemushaRecursiveSpendCandidateV4 =
            decode_canonical_norito(&candidate_manifest_bytes, "V4 candidate record")?;
        candidate_record
            .validate()
            .map_err(|error| format!("invalid V4 candidate record: {error}"))?;
        validate_current_manifest_source(&candidate_record.manifest)?;
        let candidate_manifest = candidate_record.manifest.clone();
        tracked_metadata.push(candidate_manifest_input);

        let mut candidate_json = norito::json::to_string_pretty(&candidate_record)?;
        candidate_json.push('\n');
        let candidate_manifest_sha256: [u8; 32] = Sha256::digest(&candidate_manifest_bytes).into();
        let candidate_manifest_sha256_text =
            format!("{}\n", hex::encode(candidate_manifest_sha256));

        for (name, maximum, label, expected) in [
            (
                CANDIDATE_MANIFEST_JSON_FILE_NAME,
                MAX_MANIFEST_BYTES,
                "V4 candidate manifest JSON",
                candidate_json.as_bytes(),
            ),
            (
                CANDIDATE_MANIFEST_SHA256_FILE_NAME,
                65,
                "V4 candidate manifest digest",
                candidate_manifest_sha256_text.as_bytes(),
            ),
        ] {
            open_and_match_candidate_file(
                &candidate,
                name,
                maximum,
                label,
                expected,
                &mut tracked_metadata,
            )?;
        }

        let mut policy_input = open_input(
            Path::new(required(options, "release-policy")),
            MAX_POLICY_BYTES,
            "V4 release policy",
        )?;
        let policy_bytes = policy_input.read_all()?;
        let policy: KagemushaRecursiveSpendReleasePolicyV1 =
            decode_canonical_norito(&policy_bytes, "V4 release policy")?;
        policy
            .validate()
            .map_err(|error| format!("invalid V4 release policy: {error}"))?;

        let mut attestation_input = open_input(
            Path::new(required(options, "release-attestation")),
            MAX_ATTESTATION_BYTES,
            "V4 release attestation",
        )?;
        let attestation_bytes = attestation_input.read_all()?;
        let attestation: KagemushaRecursiveSpendReleaseAttestationV4 =
            decode_canonical_norito(&attestation_bytes, "V4 release attestation")?;

        let evidence_maximum =
            u64::try_from(KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1)?;
        let mut benchmark_input = open_input(
            Path::new(required(options, "benchmark-evidence")),
            evidence_maximum,
            "V4 physical-device benchmark evidence",
        )?;
        let benchmark_bytes = benchmark_input.read_all()?;
        let mut review_input = open_input(
            Path::new(required(options, "cryptographic-review")),
            u64::try_from(KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_MAX_BYTES_V4)?,
            "canonical signed V4 cryptographic review evidence",
        )?;
        let review_bytes = review_input.read_all()?;
        KagemushaRecursiveSpendCryptographicReviewEvidenceV4::validate_canonical_bytes_against_candidate(
            &review_bytes,
            &candidate_record,
        )
        .map_err(|error| format!("invalid signed V4 cryptographic review: {error}"))?;

        #[cfg(unix)]
        {
            let external_identities = BTreeSet::from([
                policy_input.snapshot.identity(),
                attestation_input.snapshot.identity(),
                benchmark_input.snapshot.identity(),
                review_input.snapshot.identity(),
            ]);
            if external_identities.len() != 4 {
                return Err(
                    "V4 policy, attestation, and evidence inputs must be distinct files".into(),
                );
            }
        }

        let benchmark_evidence_sha256: [u8; 32] = Sha256::digest(&benchmark_bytes).into();
        let cryptographic_review_sha256: [u8; 32] = Sha256::digest(&review_bytes).into();
        if benchmark_evidence_sha256 == cryptographic_review_sha256 {
            return Err("V4 benchmark and review evidence must be distinct".into());
        }
        let mut manifest = candidate_manifest.clone();
        manifest.benchmark_evidence_sha256 = benchmark_evidence_sha256;
        manifest.cryptographic_review_sha256 = cryptographic_review_sha256;
        manifest.release_attestation_sha256 = Sha256::digest(&attestation_bytes).into();
        manifest
            .validate()
            .map_err(|error| format!("final V4 manifest is invalid: {error}"))?;
        let authenticated = KagemushaAuthenticatedReleaseV4::verify(
            &manifest,
            &policy,
            &attestation,
            &benchmark_bytes,
            &review_bytes,
        )
        .map_err(|error| format!("V4 release authentication failed: {error}"))?;
        if authenticated.manifest() != &manifest
            || authenticated.release_attestation_sha256() != manifest.release_attestation_sha256
        {
            return Err("authenticated V4 release changed its manifest identity".into());
        }

        let mut headers = BTreeMap::new();
        let mut staged_inputs = Vec::with_capacity(INPUTS.len() + 1);
        for descriptor in manifest
            .profiles
            .iter()
            .flat_map(|profile| profile.artifacts.iter())
        {
            let header =
                candidate.verify_candidate_framed_artifact(&candidate_manifest, descriptor)?;
            if headers
                .insert(descriptor.file_name.clone(), header)
                .is_some()
            {
                return Err("V4 candidate repeats an artifact file name".into());
            }
            let input = candidate.open_bound_input(
                &descriptor.file_name,
                KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4,
                "V4 candidate artifact",
            )?;
            if input.size_bytes != descriptor.size_bytes || input.sha256 != descriptor.sha256 {
                return Err(
                    format!("V4 candidate artifact changed: {}", descriptor.file_name).into(),
                );
            }
            staged_inputs.push((descriptor.file_name.clone(), input));
        }
        if staged_inputs.len() != INPUTS.len() {
            return Err("V4 candidate does not contain the exact eight-artifact inventory".into());
        }
        let roster_descriptor = &manifest.topup_finality_roster_artifact;
        let roster_input = candidate.open_bound_input(
            &roster_descriptor.file_name,
            KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_V2,
            "V4 candidate top-up finality roster",
        )?;
        if roster_input.size_bytes != roster_descriptor.size_bytes
            || roster_input.sha256 != roster_descriptor.sha256
        {
            return Err("V4 candidate top-up finality roster changed".into());
        }
        staged_inputs.push((roster_descriptor.file_name.clone(), roster_input));
        candidate.verify_candidate_inventory()?;

        let manifest_bytes = canonical_norito_bytes(&manifest, "final V4 manifest")?;
        let mut manifest_json = norito::json::to_string_pretty(&manifest)?;
        manifest_json.push('\n');
        let manifest_sha256: [u8; 32] = Sha256::digest(&manifest_bytes).into();
        let manifest_sha256_text = format!("{}\n", hex::encode(manifest_sha256));

        let out_dir = PathBuf::from(required(options, "out-dir"));
        if out_dir.exists() {
            return Err(format!("output directory already exists: {}", out_dir.display()).into());
        }
        let trusted_parent = TrustedOutputParent::open(&out_dir)?;

        let mut staging_builder = tempfile::Builder::new();
        staging_builder
            .prefix(".kagemusha-v4-final-staging-")
            .permissions(fs::Permissions::from_mode(0o700));
        let staging = staging_builder.tempdir_in(&trusted_parent.path)?;
        let staging_name = staging
            .path()
            .file_name()
            .ok_or("temporary finalization directory has no file name")?
            .to_owned();
        let publication = PublicationDirectory::open_at(
            &trusted_parent.file,
            staging.path().to_owned(),
            &staging_name,
        )?;

        for (name, input) in &mut staged_inputs {
            let mut output = publication.create_file(name)?;
            input.copy_exact_to(&mut output)?;
        }
        for (name, input) in [
            (
                KAGEMUSHA_RECURSIVE_SPEND_BENCHMARK_EVIDENCE_FILE_NAME_V1,
                &mut benchmark_input,
            ),
            (
                KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_FILE_NAME_V1,
                &mut review_input,
            ),
        ] {
            let mut output = publication.create_file(name)?;
            input.copy_exact_to(&mut output)?;
        }
        for (name, bytes) in [
            (MANIFEST_NORITO_FILE_NAME, manifest_bytes.as_slice()),
            (MANIFEST_JSON_FILE_NAME, manifest_json.as_bytes()),
            (
                MANIFEST_NORITO_SHA256_FILE_NAME,
                manifest_sha256_text.as_bytes(),
            ),
            (
                KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_FILE_NAME_V4,
                attestation_bytes.as_slice(),
            ),
        ] {
            publication.write_exact_file(name, bytes)?;
        }

        for (name, bytes, maximum) in [
            (
                MANIFEST_NORITO_FILE_NAME,
                manifest_bytes.as_slice(),
                MAX_MANIFEST_BYTES,
            ),
            (
                MANIFEST_JSON_FILE_NAME,
                manifest_json.as_bytes(),
                MAX_MANIFEST_BYTES,
            ),
            (
                MANIFEST_NORITO_SHA256_FILE_NAME,
                manifest_sha256_text.as_bytes(),
                65,
            ),
            (
                KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_FILE_NAME_V4,
                attestation_bytes.as_slice(),
                MAX_ATTESTATION_BYTES,
            ),
        ] {
            publication.verify_exact_file(name, bytes, maximum)?;
        }
        publication.verify_exact_file(
            KAGEMUSHA_RECURSIVE_SPEND_BENCHMARK_EVIDENCE_FILE_NAME_V1,
            &benchmark_bytes,
            evidence_maximum,
        )?;
        publication.verify_exact_file(
            KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_FILE_NAME_V1,
            &review_bytes,
            evidence_maximum,
        )?;
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
        if descriptors.len() != INPUTS.len() {
            return Err("final V4 release does not contain exactly eight artifacts".into());
        }
        let mut payload_digests = BTreeSet::new();
        validate_artifacts_sequentially(
            descriptors.into_iter().zip(INPUTS),
            |((profile, descriptor), expected)| -> Result<_, Box<dyn Error>> {
                if profile.parity != expected.parity
                    || descriptor.kind != expected.kind
                    || descriptor.file_name != expected.file_name
                {
                    return Err("final V4 artifact inventory role order changed".into());
                }
                let header = headers
                    .get(&descriptor.file_name)
                    .ok_or("validated V4 artifact header disappeared")?;
                let payload =
                    publication.verify_framed_artifact(&authenticated, descriptor, header)?;
                if payload.header().parity != expected.parity
                    || payload.header().kind != expected.kind
                {
                    return Err("authenticated V4 artifact header role changed".into());
                }
                if !payload_digests.insert(payload.header().payload_sha256) {
                    return Err("authenticated V4 artifact payloads are not distinct".into());
                }
                if expected.kind == KagemushaPastaCycleArtifactKindV4::BootstrapWitness {
                    let measured = validate_kagemusha_step_bootstrap_payload_v4(
                        payload.payload(),
                        &profile.circuit_params,
                        expected.parity,
                        profile.compiled_protocol_structure_sha256,
                    )?;
                    if u32::try_from(measured) != Ok(profile.step_proof_size_bytes) {
                        return Err(
                            "final V4 bootstrap measurement differs from the authenticated profile"
                                .into(),
                        );
                    }
                }
                Ok(payload)
            },
        )?;
        if payload_digests.len() != INPUTS.len() {
            return Err("authenticated V4 artifact payload inventory changed".into());
        }
        if authenticated.manifest_sha256() == [0; 32]
            || authenticated.manifest_sha256() != manifest_sha256
            || authenticated.manifest().max_proof_bytes != manifest.max_proof_bytes
        {
            return Err("authenticated V4 artifact binding changed the manifest identity".into());
        }
        publication.verify_file_digest(
            &roster_descriptor.file_name,
            roster_descriptor.size_bytes,
            roster_descriptor.sha256,
            KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_V2,
        )?;
        let promotion_record = KagemushaRecursiveSpendPromotedReleaseV4 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_PROMOTED_RELEASE_SCHEMA_V4.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V4,
            generation: manifest.generation.clone(),
            candidate_sha256: candidate_record
                .sha256()
                .map_err(|error| format!("failed to identify immutable V4 candidate: {error}"))?,
            manifest_sha256: authenticated.manifest_sha256(),
            release_attestation_sha256: authenticated.release_attestation_sha256(),
            release_policy_sha256: authenticated.release_policy_sha256(),
            approved_signers: authenticated.approved_signers().to_vec(),
            artifact_inventory_verified: true,
            bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
            artifact_roles: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4
                .map(str::to_owned)
                .to_vec(),
            max_proof_bytes: manifest.max_proof_bytes,
        };
        promotion_record
            .validate_against_candidate_and_authenticated_release(&candidate_record, &authenticated)
            .map_err(|error| format!("invalid V4 promotion record: {error}"))?;
        let promotion_bytes = canonical_norito_bytes(&promotion_record, "V4 promotion record")?;
        publication.write_exact_file(PROMOTION_RECORD_FILE_NAME_V4, &promotion_bytes)?;
        publication.verify_exact_file(
            PROMOTION_RECORD_FILE_NAME_V4,
            &promotion_bytes,
            MAX_MANIFEST_BYTES,
        )?;
        publication.verify_final_inventory()?;

        for input in &mut tracked_metadata {
            input.rehash_and_verify()?;
        }
        for (_, input) in &mut staged_inputs {
            input.rehash_and_verify()?;
        }
        for input in [
            &mut policy_input,
            &mut attestation_input,
            &mut benchmark_input,
            &mut review_input,
        ] {
            input.rehash_and_verify()?;
        }
        candidate.verify_candidate_inventory()?;
        validate_current_manifest_source(&candidate_record.manifest)?;
        publication.verify_final_inventory()?;
        publication.sync()?;
        for input in &mut tracked_metadata {
            input.rehash_and_verify()?;
        }
        for (_, input) in &mut staged_inputs {
            input.rehash_and_verify()?;
        }
        for input in [
            &mut policy_input,
            &mut attestation_input,
            &mut benchmark_input,
            &mut review_input,
        ] {
            input.rehash_and_verify()?;
        }
        candidate.verify_candidate_inventory()?;
        validate_current_manifest_source(&candidate_record.manifest)?;
        trusted_parent.publish(&staging_name, &publication)?;
        let _published = staging.keep();
        Ok(())
    }
}

fn prepare_topup_finality_roster(
    input: &mut OpenedInput,
    metadata: &BundleMetadata,
) -> Result<(Vec<u8>, KagemushaTopUpFinalityRosterArtifactReferenceV4), Box<dyn Error>> {
    let bytes = input.read_all()?;
    let roster: KagemushaTopUpFinalityRosterArtifactV2 = norito::decode_from_bytes(&bytes)?;
    if norito::to_bytes(&roster)? != bytes {
        return Err("top-up finality roster is not canonical Norito".into());
    }
    roster
        .validate()
        .map_err(|error| format!("invalid top-up finality roster: {error}"))?;
    if roster.chain_id != metadata.chain_id
        || !roster_generation_matches_release(&roster.artifact_generation, &metadata.generation)
    {
        return Err("top-up finality roster chain or generation mismatch".into());
    }
    let mut covered_until = metadata.activation_height;
    for window in &roster.windows {
        if window.withdraws_at_height <= covered_until {
            continue;
        }
        if window.activates_at_height > covered_until {
            return Err(format!(
                "top-up finality roster has a release-window gap at height {covered_until}"
            )
            .into());
        }
        covered_until = window.withdraws_at_height;
        if covered_until >= metadata.withdrawal_height {
            break;
        }
    }
    if covered_until < metadata.withdrawal_height {
        return Err("top-up finality roster does not cover the complete release window".into());
    }
    let descriptor = KagemushaTopUpFinalityRosterArtifactReferenceV4 {
        file_name: KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V4.to_owned(),
        size_bytes: input.size_bytes,
        sha256: input.sha256,
        artifact_generation: roster.artifact_generation,
        circuit_id: KAGEMUSHA_TOPUP_FINALITY_CIRCUIT_ID_V2.to_owned(),
        purpose: KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_PURPOSE_V2.to_owned(),
        artifact_type: KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_TYPE_V2.to_owned(),
        required_bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
    };
    descriptor
        .validate()
        .map_err(|error| format!("invalid V4 roster descriptor: {error}"))?;
    Ok((bytes, descriptor))
}

#[expect(
    clippy::too_many_lines,
    reason = "candidate framing, manifest construction, and pre-publication verification are one ordered security boundary"
)]
fn write_candidate(
    publication: &PublicationDirectory,
    metadata: BundleMetadata,
    prepared: Vec<PreparedArtifact>,
    roster_bytes: &[u8],
    roster_descriptor: KagemushaTopUpFinalityRosterArtifactReferenceV4,
) -> Result<(), Box<dyn Error>> {
    let mut vesta_artifact_descriptors = Vec::with_capacity(4);
    let mut pallas_artifact_descriptors = Vec::with_capacity(4);
    let mut staged_headers = Vec::with_capacity(8);
    for artifact in prepared {
        let (header, descriptor) = package_artifact(publication, artifact)?;
        match header.parity {
            KagemushaPastaCycleParityV1::StepEq => {
                vesta_artifact_descriptors.push(descriptor.clone());
            }
            KagemushaPastaCycleParityV1::StepEp => {
                pallas_artifact_descriptors.push(descriptor.clone());
            }
        }
        staged_headers.push((header, descriptor));
    }

    let mut roster_output =
        publication.create_file(KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V4)?;
    roster_output.write_all(roster_bytes)?;
    roster_output.sync_all()?;
    drop(roster_output);
    publication.verify_exact_file(
        KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V4,
        roster_bytes,
        KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_V2,
    )?;

    let manifest = KagemushaRecursiveSpendArtifactManifestV4 {
        schema: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4.to_owned(),
        version: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V4,
        bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
        proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
        transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4.to_owned(),
        generation: metadata.generation.clone(),
        source_commit: metadata.source_commit,
        source_tree_sha256: metadata.source_tree_sha256,
        source_repo_dirty: true,
        reviewed_source_closure: metadata.reviewed_source_closure,
        reviewed_source_closure_descriptor_sha256: metadata
            .reviewed_source_closure_descriptor_sha256,
        chain_id: metadata.chain_id,
        asset: metadata.asset,
        asset_scale: metadata.asset_scale,
        activation_height: metadata.activation_height,
        withdrawal_height: metadata.withdrawal_height,
        max_proof_bytes: metadata.max_proof_bytes,
        profiles: vec![
            KagemushaPastaCycleProofProfileV4 {
                parity: metadata.profiles[0].parity,
                circuit_id: metadata.profiles[0].circuit_id.to_owned(),
                parameter_generation: metadata.parameter_generation.clone(),
                ipa_k: metadata.profiles[0].circuit_params.k,
                circuit_params: metadata.profiles[0].circuit_params.clone(),
                compiled_protocol_structure_sha256: metadata.profiles[0]
                    .compiled_protocol_structure_sha256,
                step_proof_size_bytes: metadata.profiles[0].step_proof_size_bytes,
                artifacts: vesta_artifact_descriptors,
            },
            KagemushaPastaCycleProofProfileV4 {
                parity: metadata.profiles[1].parity,
                circuit_id: metadata.profiles[1].circuit_id.to_owned(),
                parameter_generation: metadata.parameter_generation,
                ipa_k: metadata.profiles[1].circuit_params.k,
                circuit_params: metadata.profiles[1].circuit_params.clone(),
                compiled_protocol_structure_sha256: metadata.profiles[1]
                    .compiled_protocol_structure_sha256,
                step_proof_size_bytes: metadata.profiles[1].step_proof_size_bytes,
                artifacts: pallas_artifact_descriptors,
            },
        ],
        topup_finality_roster_artifact: roster_descriptor,
        benchmark_evidence_sha256: [0; 32],
        cryptographic_review_sha256: [0; 32],
        release_attestation_sha256: [0; 32],
    };
    manifest
        .validate_unsigned_candidate()
        .map_err(|error| format!("generated V4 candidate manifest is invalid: {error}"))?;

    for (header, descriptor) in &staged_headers {
        validate_header_against_candidate_v4(header, &manifest, descriptor)
            .map_err(|error| format!("staged V4 header is not manifest-bound: {error}"))?;
        let verified_header =
            publication.verify_candidate_framed_artifact(&manifest, descriptor)?;
        if &verified_header != header {
            return Err(format!(
                "staged V4 header changed after packaging: {}",
                descriptor.file_name
            )
            .into());
        }
    }
    let measured = validate_kagemusha_proof_pair_measurement_v4(
        &metadata.measured_proof_pair,
        &manifest.profiles[0].circuit_params,
        &manifest.profiles[1].circuit_params,
        manifest.max_proof_bytes,
    )?;
    if measured != metadata.measured_proof_pair.len()
        || u32::try_from(measured).ok()
            != Some(KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_RELEASE_INITIALIZATION_BYTES_V4)
        || manifest.max_proof_bytes != KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_RELEASE_MAX_BYTES_V4
        || u32::try_from(measured)
            .ok()
            .is_none_or(|bytes| bytes >= manifest.max_proof_bytes)
    {
        return Err("generated V4 manifest does not admit its initialization proof pair".into());
    }

    let candidate = KagemushaRecursiveSpendCandidateV4 {
        schema: KAGEMUSHA_RECURSIVE_SPEND_CANDIDATE_SCHEMA_V4.to_owned(),
        version: KAGEMUSHA_RECURSIVE_SPEND_CANDIDATE_VERSION_V4,
        manifest,
    };
    candidate
        .validate()
        .map_err(|error| format!("generated V4 candidate record is invalid: {error}"))?;
    let candidate_norito = canonical_norito_bytes(&candidate, "V4 candidate record")?;
    let mut candidate_json = norito::json::to_string_pretty(&candidate)?;
    candidate_json.push('\n');
    let candidate_sha256: [u8; 32] = Sha256::digest(&candidate_norito).into();
    let candidate_sha256_text = format!("{}\n", hex::encode(candidate_sha256));

    for (name, bytes) in [
        (
            CANDIDATE_MANIFEST_NORITO_FILE_NAME,
            candidate_norito.as_slice(),
        ),
        (CANDIDATE_MANIFEST_JSON_FILE_NAME, candidate_json.as_bytes()),
        (
            CANDIDATE_MANIFEST_SHA256_FILE_NAME,
            candidate_sha256_text.as_bytes(),
        ),
    ] {
        publication.write_exact_file(name, bytes)?;
    }

    for (name, bytes, maximum) in [
        (
            CANDIDATE_MANIFEST_NORITO_FILE_NAME,
            candidate_norito.as_slice(),
            MAX_MANIFEST_BYTES,
        ),
        (
            CANDIDATE_MANIFEST_JSON_FILE_NAME,
            candidate_json.as_bytes(),
            MAX_MANIFEST_BYTES,
        ),
        (
            CANDIDATE_MANIFEST_SHA256_FILE_NAME,
            candidate_sha256_text.as_bytes(),
            65,
        ),
    ] {
        publication.verify_exact_file(name, bytes, maximum)?;
    }
    Ok(())
}

fn package_artifact(
    publication: &PublicationDirectory,
    artifact: PreparedArtifact,
) -> Result<
    (
        KagemushaPastaCycleFramedArtifactHeaderV4,
        KagemushaPastaCycleArtifactV4,
    ),
    Box<dyn Error>,
> {
    let PreparedArtifact {
        spec,
        mut payload,
        circuit_params,
        compiled_protocol_structure_sha256,
        step_proof_size_bytes,
        header,
        total_size,
    } = artifact;
    let export_profile = KagemushaPastaCycleProofProfileV4 {
        parity: header.parity,
        circuit_id: header.circuit_id.clone(),
        parameter_generation: header.parameter_generation.clone(),
        ipa_k: header.ipa_k,
        circuit_params,
        compiled_protocol_structure_sha256,
        step_proof_size_bytes,
        artifacts: Vec::new(),
    };
    let mut output = publication.create_file(spec.file_name)?;
    let descriptor = match &mut payload {
        GeneratedPayload::Memory(bytes) => write_kagemusha_pasta_cycle_artifact_v4(
            &mut output,
            &header.generation,
            &export_profile,
            spec.kind,
            bytes,
        )?,
        GeneratedPayload::Staged(staged) => {
            staged.file.seek(SeekFrom::Start(0))?;
            write_kagemusha_pasta_cycle_artifact_from_reader_v4(
                &mut output,
                &header.generation,
                &export_profile,
                spec.kind,
                &mut staged.file,
                staged.size_bytes,
                staged.sha256,
            )?
        }
    };
    output.sync_all()?;
    drop(output);
    if descriptor.file_name != spec.file_name
        || descriptor.size_bytes != total_size
        || descriptor.payload_size_bytes != header.payload_size_bytes
        || descriptor.payload_sha256 != header.payload_sha256
    {
        return Err(format!("core framing changed generated {} metadata", spec.file_name).into());
    }
    Ok((header, descriptor))
}

#[cfg(any(target_os = "linux", target_os = "android", target_os = "macos"))]
struct TrustedOutputParent {
    path: PathBuf,
    file: File,
    output_name: std::ffi::OsString,
}

#[cfg(any(target_os = "linux", target_os = "android", target_os = "macos"))]
impl TrustedOutputParent {
    fn open(out_dir: &Path) -> Result<Self, Box<dyn Error>> {
        let (path, output_name) = Self::validated_path_and_name(out_dir)?;
        let file = File::open(&path)?;
        Self::finish_open(path, file, output_name)
    }

    fn open_pinned(out_dir: &Path, descriptor_text: &str) -> Result<Self, Box<dyn Error>> {
        let descriptor = descriptor_text
            .parse::<i32>()
            .ok()
            .filter(|descriptor| *descriptor >= 3 && descriptor.to_string() == descriptor_text)
            .ok_or("guard-supplied output-parent descriptor is invalid")?;
        let (path, output_name) = Self::validated_path_and_name(out_dir)?;
        let file = File::open(format!("/dev/fd/{descriptor}"))
            .map_err(|_| "guard-supplied output-parent descriptor is unavailable")?;
        Self::finish_open(path, file, output_name)
    }

    fn validated_path_and_name(
        out_dir: &Path,
    ) -> Result<(PathBuf, std::ffi::OsString), Box<dyn Error>> {
        use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

        let output_name = out_dir
            .file_name()
            .filter(|name| !name.is_empty())
            .ok_or("--out-dir must end in one directory name")?
            .to_owned();
        let parent = out_dir
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let path = fs::canonicalize(parent)?;
        let effective_uid = rustix::process::geteuid().as_raw();
        for ancestor in path.ancestors() {
            let metadata = fs::symlink_metadata(ancestor)?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(format!(
                    "output parent chain contains a non-directory: {}",
                    ancestor.display()
                )
                .into());
            }
            let mode = metadata.permissions().mode();
            if (metadata.uid() != 0 && metadata.uid() != effective_uid)
                || mode & 0o022 != 0 && mode & 0o1000 == 0
            {
                return Err(format!(
                    "output parent chain has untrusted ownership or permissions: {}",
                    ancestor.display()
                )
                .into());
            }
        }
        Ok((path, output_name))
    }

    fn finish_open(
        path: PathBuf,
        file: File,
        output_name: std::ffi::OsString,
    ) -> Result<Self, Box<dyn Error>> {
        use std::os::unix::fs::MetadataExt as _;

        let opened = file.metadata()?;
        let current = fs::metadata(&path)?;
        if !opened.is_dir() || opened.dev() != current.dev() || opened.ino() != current.ino() {
            return Err("output parent changed while it was opened".into());
        }
        match rustix::fs::statat(&file, &output_name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW) {
            Ok(_) => {
                return Err(format!(
                    "output directory already exists: {}",
                    path.join(&output_name).display()
                )
                .into());
            }
            Err(error) if error == rustix::io::Errno::NOENT => {}
            Err(error) => return Err(error.into()),
        }
        Ok(Self {
            path,
            file,
            output_name,
        })
    }

    fn publish(
        &self,
        staging_name: &std::ffi::OsStr,
        publication: &PublicationDirectory,
    ) -> Result<(), Box<dyn Error>> {
        self.file.sync_all()?;
        self.publish_presynced(staging_name, publication)
    }

    fn publish_presynced(
        &self,
        staging_name: &std::ffi::OsStr,
        publication: &PublicationDirectory,
    ) -> Result<(), Box<dyn Error>> {
        use rustix::fs::FileType as RustixFileType;

        publication.verify_identity()?;
        let staging = rustix::fs::statat(
            &self.file,
            staging_name,
            rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
        )?;
        if RustixFileType::from_raw_mode(staging.st_mode) != RustixFileType::Directory
            || checked_stat_value(staging.st_dev) != Some(publication.device)
            || checked_stat_value(staging.st_ino) != Some(publication.inode)
        {
            return Err(
                "staging directory name no longer identifies the verified directory".into(),
            );
        }
        rustix::fs::renameat_with(
            &self.file,
            staging_name,
            &self.file,
            &self.output_name,
            rustix::fs::RenameFlags::NOREPLACE,
        )?;
        self.file.sync_all().map_err(|error| {
            format!(
                "V4 bundle was promoted as {} but parent durability failed: {error}",
                self.path.join(&self.output_name).display()
            )
            .into()
        })
    }
}

struct PublicationDirectory {
    path: PathBuf,
    file: File,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(unix)]
    path_bound: bool,
}

impl PublicationDirectory {
    #[cfg(unix)]
    fn open_at(parent: &File, path: PathBuf, name: &std::ffi::OsStr) -> io::Result<Self> {
        use rustix::fs::{Mode, OFlags};

        let file = File::from(rustix::fs::openat(
            parent,
            name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        Self::validate(path, file, false)
    }

    #[cfg(unix)]
    fn open_existing(path: PathBuf) -> io::Result<Self> {
        use rustix::fs::{Mode, OFlags};

        let before = fs::symlink_metadata(&path)?;
        if before.file_type().is_symlink() || !before.is_dir() {
            return Err(io::Error::other(
                "publication directory is not a real directory",
            ));
        }
        let path = fs::canonicalize(path)?;
        let file = File::from(rustix::fs::open(
            &path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        let opened = file.metadata()?;
        let current = fs::symlink_metadata(&path)?;
        if current.file_type().is_symlink()
            || !current.is_dir()
            || FileSnapshot::from_metadata(&before) != FileSnapshot::from_metadata(&opened)
            || FileSnapshot::from_metadata(&current) != FileSnapshot::from_metadata(&opened)
        {
            return Err(io::Error::other(
                "publication directory changed while it was opened",
            ));
        }
        Self::validate(path, file, true)
    }

    fn validate(path: PathBuf, file: File, path_bound: bool) -> io::Result<Self> {
        let opened = file.metadata()?;
        if !opened.is_dir() {
            return Err(io::Error::other(
                "publication descriptor is not a directory",
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

            if opened.uid() != rustix::process::geteuid().as_raw()
                || opened.permissions().mode() & 0o077 != 0
            {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "publication directory must remain owner-private",
                ));
            }
            if path_bound {
                let current = fs::symlink_metadata(&path)?;
                if current.file_type().is_symlink()
                    || opened.dev() != current.dev()
                    || opened.ino() != current.ino()
                {
                    return Err(io::Error::other(
                        "publication directory path changed while opening",
                    ));
                }
            }
            Ok(Self {
                path,
                device: opened.dev(),
                inode: opened.ino(),
                file,
                path_bound,
            })
        }
        #[cfg(not(unix))]
        {
            let _ = path_bound;
            Ok(Self { path, file })
        }
    }

    fn create_file(&self, name: &str) -> io::Result<File> {
        validate_publication_file_name(name)?;
        #[cfg(unix)]
        {
            use rustix::fs::{Mode, OFlags};

            let file = File::from(rustix::fs::openat(
                &self.file,
                name,
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::from_raw_mode(0o600),
            )?);
            verify_owner_private_regular_file(&file)?;
            Ok(file)
        }
        #[cfg(not(unix))]
        {
            File::options()
                .write(true)
                .create_new(true)
                .open(self.path.join(name))
        }
    }

    #[cfg(unix)]
    fn create_unlinked_file(&self, name: &str) -> io::Result<File> {
        use rustix::fs::{Mode, OFlags};

        validate_publication_file_name(name)?;
        let file = File::from(rustix::fs::openat(
            &self.file,
            name,
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )?);
        verify_owner_private_regular_file(&file)?;
        rustix::fs::unlinkat(&self.file, name, rustix::fs::AtFlags::empty())?;
        self.file.sync_all()?;
        Ok(file)
    }

    fn write_exact_file(&self, name: &str, bytes: &[u8]) -> io::Result<()> {
        let mut file = self.create_file(name)?;
        file.write_all(bytes)?;
        file.sync_all()
    }

    fn open_file(&self, name: &str) -> io::Result<File> {
        validate_publication_file_name(name)?;
        #[cfg(unix)]
        {
            use rustix::fs::{Mode, OFlags};

            let file = File::from(rustix::fs::openat(
                &self.file,
                name,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )?);
            verify_owner_private_regular_file(&file)?;
            Ok(file)
        }
        #[cfg(not(unix))]
        {
            File::open(self.path.join(name))
        }
    }

    fn open_bound_input(
        &self,
        name: &str,
        maximum: u64,
        label: &str,
    ) -> Result<OpenedInput, Box<dyn Error>> {
        self.verify_identity()?;
        let path = self.path.join(name);
        let mut file = self.open_file(name)?;
        let opened = file.metadata()?;
        let snapshot = FileSnapshot::from_metadata(&opened);
        #[cfg(unix)]
        let current_matches = {
            let current =
                rustix::fs::statat(&self.file, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)?;
            FileSnapshot::from_stat(&current) == Some(snapshot)
        };
        #[cfg(not(unix))]
        let current_matches = {
            let current = fs::symlink_metadata(&path)?;
            !current.file_type().is_symlink()
                && current.is_file()
                && FileSnapshot::from_metadata(&current) == snapshot
        };
        if !current_matches || snapshot.length == 0 || snapshot.length > maximum {
            return Err(format!("{label} changed while it was opened: {}", path.display()).into());
        }
        let sha256 = hash_open_file(&mut file, snapshot.length, &path)?;
        file.seek(SeekFrom::Start(0))?;
        Ok(OpenedInput {
            file,
            path,
            #[cfg(unix)]
            parent: Some(self.file.try_clone()?),
            #[cfg(unix)]
            entry_name: Some(name.into()),
            snapshot,
            size_bytes: snapshot.length,
            sha256,
        })
    }

    fn verify_identity(&self) -> io::Result<()> {
        let opened = self.file.metadata()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

            if !opened.is_dir()
                || opened.dev() != self.device
                || opened.ino() != self.inode
                || opened.uid() != rustix::process::geteuid().as_raw()
                || opened.permissions().mode() & 0o077 != 0
            {
                return Err(io::Error::other("publication directory identity changed"));
            }
            if self.path_bound {
                let current = fs::symlink_metadata(&self.path)?;
                if current.file_type().is_symlink()
                    || !current.is_dir()
                    || current.dev() != self.device
                    || current.ino() != self.inode
                {
                    return Err(io::Error::other("publication directory path changed"));
                }
            }
        }
        #[cfg(not(unix))]
        {
            let current = fs::symlink_metadata(&self.path)?;
            if current.file_type().is_symlink()
                || !current.is_dir()
                || FileSnapshot::from_metadata(&opened) != FileSnapshot::from_metadata(&current)
            {
                return Err(io::Error::other("publication directory identity changed"));
            }
        }
        Ok(())
    }

    fn verify_exact_file(&self, name: &str, expected: &[u8], maximum: u64) -> io::Result<()> {
        let mut file = self.open_file(name)?;
        let expected_len = u64::try_from(expected.len())
            .map_err(|_| io::Error::other("expected staged length does not fit u64"))?;
        let metadata = file.metadata()?;
        if metadata.len() != expected_len || metadata.len() > maximum {
            return Err(io::Error::other(format!(
                "staged file has an unexpected length: {name}"
            )));
        }
        let mut actual = Vec::with_capacity(expected.len());
        file.read_to_end(&mut actual)?;
        if actual != expected {
            return Err(io::Error::other(format!(
                "staged file changed after write: {name}"
            )));
        }
        Ok(())
    }

    fn verify_file_digest(
        &self,
        name: &str,
        expected_size: u64,
        expected_sha256: [u8; 32],
        maximum: u64,
    ) -> Result<(), Box<dyn Error>> {
        let mut file = self.open_file(name)?;
        if expected_size == 0 || expected_size > maximum || file.metadata()?.len() != expected_size
        {
            return Err(format!("staged file has an unexpected length: {name}").into());
        }
        let sha256 = hash_open_file(&mut file, expected_size, &self.path.join(name))?;
        if sha256 != expected_sha256 {
            return Err(format!("staged file has an unexpected digest: {name}").into());
        }
        Ok(())
    }

    fn verify_framed_artifact(
        &self,
        release: &KagemushaAuthenticatedReleaseV4,
        descriptor: &KagemushaPastaCycleArtifactV4,
        expected_header: &KagemushaPastaCycleFramedArtifactHeaderV4,
    ) -> Result<KagemushaValidatedArtifactPayloadV4, Box<dyn Error>> {
        let mut file = self.open_file(&descriptor.file_name)?;
        let metadata = file.metadata()?;
        if metadata.len() != descriptor.size_bytes
            || metadata.len() > KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4
        {
            return Err(format!(
                "staged artifact has an unexpected length: {}",
                descriptor.file_name
            )
            .into());
        }
        let validated = read_kagemusha_pasta_cycle_artifact_v4(&mut file, release, descriptor)
            .map_err(|error| format!("production V4 reader rejected staged artifact: {error}"))?;
        if validated.header() != expected_header {
            return Err(format!(
                "staged artifact header changed after packaging: {}",
                descriptor.file_name
            )
            .into());
        }
        Ok(validated)
    }

    fn verify_candidate_framed_artifact(
        &self,
        manifest: &KagemushaRecursiveSpendArtifactManifestV4,
        descriptor: &KagemushaPastaCycleArtifactV4,
    ) -> Result<KagemushaPastaCycleFramedArtifactHeaderV4, Box<dyn Error>> {
        let mut file = self.open_file(&descriptor.file_name)?;
        if file.metadata()?.len() != descriptor.size_bytes {
            return Err(format!(
                "candidate artifact has an unexpected length: {}",
                descriptor.file_name
            )
            .into());
        }
        let mut framed_hasher = Sha256::new();
        let mut magic = [0_u8; 8];
        file.read_exact(&mut magic)?;
        framed_hasher.update(magic);
        if magic != *KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_KEY_MAGIC_V4 {
            return Err(format!(
                "candidate artifact has invalid magic: {}",
                descriptor.file_name
            )
            .into());
        }
        let mut header_len_bytes = [0_u8; 4];
        file.read_exact(&mut header_len_bytes)?;
        framed_hasher.update(header_len_bytes);
        let header_len = u32::from_le_bytes(header_len_bytes);
        if header_len == 0 || header_len > KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_HEADER_MAX_BYTES_V4 {
            return Err(format!(
                "candidate artifact has invalid header length: {}",
                descriptor.file_name
            )
            .into());
        }
        let mut header_bytes = vec![0_u8; usize::try_from(header_len)?];
        file.read_exact(&mut header_bytes)?;
        framed_hasher.update(&header_bytes);
        let header: KagemushaPastaCycleFramedArtifactHeaderV4 =
            decode_canonical_norito(&header_bytes, "candidate KRV4 header")?;
        validate_header_against_candidate_v4(&header, manifest, descriptor).map_err(|error| {
            format!(
                "candidate artifact is not manifest-bound ({}): {error}",
                descriptor.file_name
            )
        })?;
        let mut payload_hasher = Sha256::new();
        let mut remaining = descriptor.payload_size_bytes;
        let mut buffer = vec![0_u8; 64 * 1024];
        while remaining != 0 {
            let buffer_len = u64::try_from(buffer.len())?;
            let limit = usize::try_from(remaining.min(buffer_len))?;
            file.read_exact(&mut buffer[..limit])?;
            payload_hasher.update(&buffer[..limit]);
            framed_hasher.update(&buffer[..limit]);
            remaining -= u64::try_from(limit)?;
        }
        let mut trailing = [0_u8; 1];
        if file.read(&mut trailing)? != 0
            || <[u8; 32]>::from(payload_hasher.finalize()) != descriptor.payload_sha256
            || <[u8; 32]>::from(framed_hasher.finalize()) != descriptor.sha256
        {
            return Err(format!(
                "candidate artifact content changed after packaging: {}",
                descriptor.file_name
            )
            .into());
        }
        Ok(header)
    }

    fn verify_candidate_inventory(&self) -> io::Result<()> {
        let expected = INPUTS
            .iter()
            .map(|spec| spec.file_name)
            .chain([
                KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V4,
                CANDIDATE_MANIFEST_NORITO_FILE_NAME,
                CANDIDATE_MANIFEST_SHA256_FILE_NAME,
                CANDIDATE_MANIFEST_JSON_FILE_NAME,
            ])
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        self.verify_inventory(&expected)
    }

    fn verify_final_inventory(&self) -> io::Result<()> {
        let expected = INPUTS
            .iter()
            .map(|spec| spec.file_name)
            .chain([
                KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V4,
                MANIFEST_NORITO_FILE_NAME,
                MANIFEST_NORITO_SHA256_FILE_NAME,
                MANIFEST_JSON_FILE_NAME,
                KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_FILE_NAME_V4,
                KAGEMUSHA_RECURSIVE_SPEND_BENCHMARK_EVIDENCE_FILE_NAME_V1,
                KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_FILE_NAME_V1,
                PROMOTION_RECORD_FILE_NAME_V4,
            ])
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        self.verify_inventory(&expected)
    }

    fn verify_inventory(&self, expected: &BTreeSet<String>) -> io::Result<()> {
        self.verify_identity()?;
        let mut actual = BTreeSet::new();
        #[cfg(unix)]
        {
            use std::{ffi::OsStr, os::unix::ffi::OsStrExt as _};

            use rustix::fs::{AtFlags, Dir, FileType as RustixFileType};

            let mut entries = Dir::read_from(&self.file).map_err(io::Error::from)?;
            for entry in &mut entries {
                let entry = entry.map_err(io::Error::from)?;
                let name_bytes = entry.file_name().to_bytes();
                if matches!(name_bytes, b"." | b"..") {
                    continue;
                }
                let name = std::str::from_utf8(name_bytes)
                    .map_err(|_| io::Error::other("publication contains a non-UTF-8 name"))?
                    .to_owned();
                let metadata = rustix::fs::statat(
                    &self.file,
                    OsStr::from_bytes(name_bytes),
                    AtFlags::SYMLINK_NOFOLLOW,
                )
                .map_err(io::Error::from)?;
                if RustixFileType::from_raw_mode(metadata.st_mode) != RustixFileType::RegularFile
                    || metadata.st_nlink != 1
                    || !actual.insert(name)
                {
                    return Err(io::Error::other(
                        "publication contains an invalid directory entry",
                    ));
                }
            }
        }
        #[cfg(not(unix))]
        for entry in fs::read_dir(&self.path)? {
            let entry = entry?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| io::Error::other("publication contains a non-UTF-8 name"))?;
            let metadata = fs::symlink_metadata(entry.path())?;
            if metadata.file_type().is_symlink() || !metadata.is_file() || !actual.insert(name) {
                return Err(io::Error::other(
                    "publication contains an invalid directory entry",
                ));
            }
        }
        if &actual != expected {
            return Err(io::Error::other(
                "publication file inventory is incomplete or excessive",
            ));
        }
        self.verify_identity()?;
        Ok(())
    }

    fn sync(&self) -> io::Result<()> {
        self.file.sync_all()
    }
}

fn validate_publication_file_name(name: &str) -> io::Result<()> {
    let mut components = Path::new(name).components();
    if !matches!(components.next(), Some(std::path::Component::Normal(_)))
        || components.next().is_some()
    {
        return Err(io::Error::other(
            "publication file name must be one normal component",
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn verify_owner_private_regular_file(file: &File) -> io::Result<()> {
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.nlink() != 1 || metadata.permissions().mode() & 0o077 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "publication file must be owner-private, regular, and singly linked",
        ));
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use std::{cell::Cell, os::unix::fs::PermissionsExt as _, rc::Rc};

    use super::*;

    struct LivePayload {
        live: Rc<Cell<usize>>,
    }

    impl Drop for LivePayload {
        fn drop(&mut self) {
            self.live.set(self.live.get() - 1);
        }
    }

    fn assert_descriptor_binding(
        descriptor: &KagemushaPastaCycleArtifactV4,
        header: &KagemushaPastaCycleFramedArtifactHeaderV4,
        spec: InputSpec,
        total_size: u64,
    ) {
        assert!(descriptor_matches_framed_artifact(
            descriptor, header, spec, total_size
        ));
        let mut wrong_kind = descriptor.clone();
        wrong_kind.kind = KagemushaPastaCycleArtifactKindV4::VerifyingKey;
        assert!(!descriptor_matches_framed_artifact(
            &wrong_kind,
            header,
            spec,
            total_size
        ));
    }

    #[test]
    fn embedded_candidate_source_seal_must_be_present_exact_and_canonical() {
        let commit = "a".repeat(40);
        let tree = [0xbb; 32];
        let tree_hex = hex::encode(tree);
        validate_embedded_candidate_source(
            &commit,
            tree,
            Some(commit.as_str()),
            Some(tree_hex.as_str()),
            false,
        )
        .expect("matching embedded source seal");

        assert!(validate_embedded_candidate_source(&commit, tree, None, None, false).is_err());
        assert!(
            validate_embedded_candidate_source(
                &commit,
                tree,
                Some(commit.as_str()),
                Some(tree_hex.as_str()),
                true,
            )
            .is_err()
        );
        let uppercase_commit = "A".repeat(40);
        assert!(
            validate_embedded_candidate_source(
                &commit,
                tree,
                Some(uppercase_commit.as_str()),
                Some(tree_hex.as_str()),
                false,
            )
            .is_err()
        );
        let wrong_tree = "0".repeat(64);
        assert!(
            validate_embedded_candidate_source(
                &commit,
                tree,
                Some(commit.as_str()),
                Some(wrong_tree.as_str()),
                false,
            )
            .is_err()
        );
    }

    #[test]
    fn source_validation_uses_fixed_tools_and_a_sanitized_path() {
        let source = include_str!("kagemusha_recursive_spend_v4_bundle.rs");
        let validation = source
            .split_once("fn trusted_source_command(")
            .expect("trusted source command helper exists")
            .1
            .split_once("fn build_candidate(")
            .expect("source-validation boundary exists")
            .0;

        assert!(validation.contains("command.env_clear()"));
        assert!(validation.contains("TRUSTED_GIT_EXECUTABLE"));
        assert!(validation.contains("TRUSTED_PYTHON_EXECUTABLE"));
        assert!(!validation.contains("Command::new(\"git\")"));
        assert!(!validation.contains("KAGEMUSHA_SOURCE_SEAL_PYTHON"));
    }

    #[test]
    fn generated_candidate_metadata_revalidates_both_release_profiles() {
        let source = include_str!("kagemusha_recursive_spend_v4_bundle.rs");
        let preparation = source
            .split_once("fn prepare_bundle_metadata(")
            .expect("candidate metadata preparation exists")
            .1
            .split_once("fn validate_generated_artifacts(")
            .expect("candidate metadata preparation boundary exists")
            .0;

        assert_eq!(
            preparation
                .matches(".validate_release_generation_profile()")
                .count(),
            2,
            "generated Eq and Ep profiles must both pass the exact release gate"
        );
    }

    #[test]
    fn publication_rejects_a_replaced_staging_directory() {
        let root = tempfile::tempdir().expect("temporary test root");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .expect("owner-private test root");
        let out_dir = root.path().join("published");
        let trusted_parent = TrustedOutputParent::open(&out_dir).expect("open trusted parent");
        let staging_name = std::ffi::OsStr::new("staging");
        let staging_path = root.path().join(staging_name);
        fs::create_dir(&staging_path).expect("create staging directory");
        fs::set_permissions(&staging_path, fs::Permissions::from_mode(0o700))
            .expect("owner-private staging directory");
        let publication =
            PublicationDirectory::open_at(&trusted_parent.file, staging_path.clone(), staging_name)
                .expect("open staging descriptor");

        fs::rename(&staging_path, root.path().join("displaced"))
            .expect("displace staging directory");
        fs::create_dir(&staging_path).expect("replace staging directory");
        fs::set_permissions(&staging_path, fs::Permissions::from_mode(0o700))
            .expect("owner-private replacement");

        assert!(
            trusted_parent.publish(staging_name, &publication).is_err(),
            "the staging name must still identify the verified directory"
        );
        assert!(!out_dir.exists());
    }

    #[test]
    fn artifact_inventory_is_exact_eq_then_ep_four_role_order() {
        assert_eq!(INPUTS.len(), 8);
        assert_eq!(
            INPUTS.map(|spec| (spec.parity, spec.kind)),
            [
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
            ]
        );
        assert_eq!(
            INPUTS.map(|spec| spec.file_name),
            [
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PARAMS_IPA_FILE_NAME_V4,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PROVING_KEY_FILE_NAME_V4,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFYING_KEY_FILE_NAME_V4,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_BOOTSTRAP_FILE_NAME_V4,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PARAMS_IPA_FILE_NAME_V4,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PROVING_KEY_FILE_NAME_V4,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFYING_KEY_FILE_NAME_V4,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_BOOTSTRAP_FILE_NAME_V4,
            ]
        );
    }

    #[test]
    fn cross_layer_bindings_reject_top_level_and_descriptor_substitution() {
        let manifest_schema = KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4;
        assert!(framed_schema_matches_release(
            manifest_schema,
            manifest_schema
        ));
        assert!(!framed_schema_matches_release(
            "substituted.schema",
            manifest_schema
        ));

        let release_generation = "release-generation";
        let parameter_generation = "parameter-generation";
        assert!(framed_generations_match_release_and_profile(
            release_generation,
            release_generation,
            parameter_generation,
            parameter_generation,
        ));
        assert!(!framed_generations_match_release_and_profile(
            parameter_generation,
            release_generation,
            release_generation,
            parameter_generation,
        ));
        assert!(roster_generation_binding_is_exact(
            release_generation,
            release_generation,
            release_generation,
        ));
        assert!(!roster_generation_binding_is_exact(
            parameter_generation,
            release_generation,
            release_generation,
        ));
        assert!(!roster_generation_binding_is_exact(
            release_generation,
            parameter_generation,
            release_generation,
        ));
    }

    #[test]
    fn final_artifact_validation_drops_each_payload_before_loading_the_next() {
        let live = Rc::new(Cell::new(0));
        let peak = Rc::new(Cell::new(0));

        validate_artifacts_sequentially(0..INPUTS.len(), |_| {
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
    fn bounded_digest_writer_enforces_cap_and_keeps_anonymous_payload_exact() {
        let root = tempfile::tempdir().expect("temporary test root");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .expect("owner-private test root");
        let publication_path = root.path().join("publication");
        fs::create_dir(&publication_path).expect("create publication");
        fs::set_permissions(&publication_path, fs::Permissions::from_mode(0o700))
            .expect("owner-private publication");
        let publication = PublicationDirectory::open_existing(publication_path.clone())
            .expect("open publication");

        let payload_name = ".bounded-digest-payload.part";
        let mut writer =
            BoundedDigestFileWriter::new(&publication, payload_name).expect("open payload sink");
        writer.maximum_bytes = 3;
        assert!(
            !publication_path.join(payload_name).exists(),
            "the named staging entry must be unlinked immediately"
        );
        writer.write_all(b"abc").expect("write exact payload");
        assert!(
            writer.write_all(b"d").is_err(),
            "cap must reject before I/O"
        );
        assert_eq!(writer.file.metadata().expect("payload metadata").len(), 3);
        let mut staged = writer.finish(3, "test payload").expect("finish payload");
        assert_eq!(staged.size_bytes, 3);
        assert_eq!(staged.sha256, <[u8; 32]>::from(Sha256::digest(b"abc")));
        let mut payload = Vec::new();
        staged
            .file
            .read_to_end(&mut payload)
            .expect("read anonymous payload");
        assert_eq!(payload, b"abc");

        let mut wrong_length =
            BoundedDigestFileWriter::new(&publication, ".bounded-digest-wrong-length.part")
                .expect("open mismatch sink");
        wrong_length.maximum_bytes = 3;
        wrong_length
            .write_all(b"abc")
            .expect("write mismatch payload");
        assert!(wrong_length.finish(2, "test payload").is_err());
    }

    #[test]
    fn finalizer_uses_the_sequential_authenticated_artifact_path() {
        let source = include_str!("kagemusha_recursive_spend_v4_bundle.rs");
        let finalizer = source
            .split_once("fn finalize_release(")
            .expect("release finalizer exists")
            .1
            .split_once("\nfn prepare_topup_finality_roster(")
            .expect("release finalizer boundary exists")
            .0;

        assert!(finalizer.contains("validate_artifacts_sequentially("));
        assert!(!finalizer.contains("validated_artifacts.push("));
        assert!(!finalizer.contains("KagemushaPastaCycleProverArtifactsV4::new("));
    }

    #[test]
    fn parser_rejects_unknown_duplicate_and_noncanonical_numbers() {
        assert!(
            parse_options(["--unknown".to_owned(), "x".to_owned()], GENERATE_OPTIONS,).is_err()
        );
        assert!(
            parse_options(
                [
                    "--out-dir".to_owned(),
                    "a".to_owned(),
                    "--out-dir".to_owned(),
                    "b".to_owned(),
                ],
                GENERATE_OPTIONS,
            )
            .is_err()
        );
        assert!(
            parse_options(
                ["--release-policy".to_owned(), "policy".to_owned()],
                GENERATE_OPTIONS,
            )
            .is_err(),
            "candidate generation must not accept finalization inputs"
        );
        for obsolete in [
            "--source-repo-dirty",
            "--benchmark-evidence-sha256",
            "--cryptographic-review-sha256",
        ] {
            assert!(
                parse_options([obsolete.to_owned(), "value".to_owned()], GENERATE_OPTIONS).is_err(),
                "pre-evidence candidates must reject obsolete option {obsolete}"
            );
        }
        assert!(
            parse_options(
                ["--generation".to_owned(), "candidate".to_owned()],
                FINALIZE_OPTIONS,
            )
            .is_err(),
            "finalization must not accept generation inputs"
        );
        assert!(canonical_unsigned_decimal("0"));
        assert!(canonical_unsigned_decimal("19"));
        assert!(!canonical_unsigned_decimal("01"));
        assert!(!canonical_unsigned_decimal("+1"));
    }

    #[test]
    fn v4_portable_identifiers_are_single_cross_platform_components() {
        for valid in ["release-20", "parameters_v4", "cycle.2026-07"] {
            assert!(is_kagemusha_v4_portable_identifier(valid));
        }
        for invalid in ["", "-release", "release/20", "con", "COM1.bundle", "name."] {
            assert!(!is_kagemusha_v4_portable_identifier(invalid));
        }
    }

    #[test]
    fn candidate_source_must_match_the_signed_checkout_head() {
        assert!(validate_signed_base_source("0000000000000000000000000000000000000000").is_err());
    }

    #[test]
    fn krv4_framing_is_canonical_and_payload_bound() {
        let root = tempfile::tempdir().expect("temporary test root");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .expect("owner-private test root");
        let payload_path = root.path().join("payload.bin");
        let payload = b"calibrated ParamsIPA fixture bytes";
        fs::write(&payload_path, payload).expect("write payload fixture");
        fs::set_permissions(&payload_path, fs::Permissions::from_mode(0o600))
            .expect("owner-private payload");
        let k = iroha_data_model::offline::KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4;
        let layout =
            iroha_data_model::offline::KagemushaPastaPublicLayoutV4::for_ipa_round_count(k)
                .expect("test V4 public layout");
        let circuit_params = KagemushaStepCircuitParamsV4 {
            version: iroha_data_model::offline::KAGEMUSHA_STEP_CIRCUIT_PARAMS_VERSION_V4,
            k,
            num_advice_per_phase:
                iroha_data_model::offline::KAGEMUSHA_STEP_CIRCUIT_RELEASE_ADVICE_COLUMNS_V4.to_vec(),
            num_lookup_advice_per_phase:
                iroha_data_model::offline::KAGEMUSHA_STEP_CIRCUIT_RELEASE_LOOKUP_COLUMNS_V4.to_vec(),
            num_fixed: 1,
            lookup_bits: k - 1,
            num_instance_columns: 1,
            public_input_limbs: layout.instance_column_limbs,
            minimum_unusable_rows:
                iroha_data_model::offline::KAGEMUSHA_STEP_CIRCUIT_MINIMUM_UNUSABLE_ROWS_V4,
            max_parent_proof_bytes: 4096,
        };
        let header = KagemushaPastaCycleFramedArtifactHeaderV4 {
            version: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_HEADER_VERSION_V4,
            manifest_schema: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4.to_owned(),
            bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
            proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
            transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4.to_owned(),
            generation: "v4-frame-test".to_owned(),
            parity: KagemushaPastaCycleParityV1::StepEq,
            circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4.to_owned(),
            parameter_generation: "v4-frame-test-params".to_owned(),
            ipa_k: k,
            circuit_params_sha256: circuit_params.sha256().expect("test params identity"),
            compiled_protocol_structure_sha256: [0x43; 32],
            step_proof_size_bytes: 4096,
            kind: KagemushaPastaCycleArtifactKindV4::ParamsIpa,
            payload_size_bytes: u64::try_from(payload.len()).expect("small payload"),
            payload_sha256: Sha256::digest(payload).into(),
        };
        let profile_metadata = ProfileMetadata {
            parity: KagemushaPastaCycleParityV1::StepEq,
            circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4,
            circuit_params: circuit_params.clone(),
            circuit_params_sha256: circuit_params.sha256().expect("test params identity"),
            compiled_protocol_structure_sha256: [0x43; 32],
            step_proof_size_bytes: 4096,
        };
        validate_header_v4(&header, &profile_metadata).expect("valid test V4 header");
        let header_bytes = norito::to_bytes(&header).expect("encode V4 header");
        let total_size = u64::try_from(8 + 4 + header_bytes.len() + payload.len())
            .expect("small framed test artifact");
        let publication_path = root.path().join("publication");
        fs::create_dir(&publication_path).expect("create publication");
        fs::set_permissions(&publication_path, fs::Permissions::from_mode(0o700))
            .expect("owner-private publication");
        let publication = PublicationDirectory::open_existing(publication_path.clone())
            .expect("open publication");
        let spec = INPUTS[0];
        let (_, descriptor) = package_artifact(
            &publication,
            PreparedArtifact {
                spec,
                payload: GeneratedPayload::Memory(payload.to_vec()),
                circuit_params,
                compiled_protocol_structure_sha256: [0x43; 32],
                step_proof_size_bytes: 4096,
                header: header.clone(),
                total_size,
            },
        )
        .expect("frame test payload");
        assert_descriptor_binding(&descriptor, &header, spec, total_size);
        let bytes = fs::read(publication_path.join(spec.file_name)).expect("read staged frame");
        assert_eq!(&bytes[..8], KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_KEY_MAGIC_V4);
        assert_eq!(
            u32::from_le_bytes(bytes[8..12].try_into().expect("header length prefix")) as usize,
            header_bytes.len()
        );
        assert_eq!(&bytes[12..12 + header_bytes.len()], header_bytes);
        assert_eq!(&bytes[12 + header_bytes.len()..], payload);
        assert_eq!(descriptor.size_bytes, total_size);
        assert_eq!(descriptor.sha256, <[u8; 32]>::from(Sha256::digest(&bytes)));
        assert_eq!(
            descriptor.payload_sha256,
            <[u8; 32]>::from(Sha256::digest(payload))
        );
    }

    #[test]
    fn input_open_rejects_symlinks() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().expect("temporary test root");
        let target = root.path().join("target");
        fs::write(&target, b"not empty").expect("write target");
        let link = root.path().join("link");
        symlink(&target, &link).expect("create symlink");
        assert!(open_input(&link, 1024, "symlink fixture").is_err());
    }
}
