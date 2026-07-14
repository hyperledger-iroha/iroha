//! Generate and finalize calibrated ABI-20 Kagemusha release bundles.
//!
//! Candidate generation runs the current recursion source exactly once and
//! publishes ten immutable `KRV4KEY` artifacts plus the canonical unsigned
//! manifest and role-separated signing payloads. Finalization never regenerates
//! proof material: it authenticates the unchanged candidate against the supplied
//! release policy, attestation, and evidence before publishing a distinct final
//! directory atomically.

use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    error::Error,
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use iroha_core::zk::kagemusha_artifact_v4::{
    read_kagemusha_pasta_cycle_artifact_v4, write_kagemusha_pasta_cycle_artifact_v4,
};
use iroha_core::zk::kagemusha_v2::{
    generate_kagemusha_pasta_cycle_artifacts_v4, validate_kagemusha_proof_pair_measurement_v4,
    validate_kagemusha_step_bootstrap_payload_v4,
};
use iroha_data_model::{
    ChainId,
    asset::AssetDefinitionId,
    offline::{
        KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_HEADER_MAX_BYTES_V4,
        KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_HEADER_VERSION_V4,
        KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_KEY_MAGIC_V4,
        KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4,
        KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V4,
        KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4,
        KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4,
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4,
        KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4,
        KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_FILE_NAME_V4,
        KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_BOOTSTRAP_FILE_NAME_V4,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_PARAMS_FILE_NAME_V4,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PARAMETERS_FILE_NAME_V4,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PROVING_KEY_FILE_NAME_V4,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFYING_KEY_FILE_NAME_V4,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_BOOTSTRAP_FILE_NAME_V4,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_PARAMS_FILE_NAME_V4,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PARAMETERS_FILE_NAME_V4,
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
        KagemushaRecursiveSpendReleaseApprovalRoleV1, KagemushaRecursiveSpendReleaseAttestationV4,
        KagemushaRecursiveSpendReleasePolicyV1, KagemushaStepCircuitParamsV4,
        KagemushaTopUpFinalityRosterArtifactReferenceV4, KagemushaTopUpFinalityRosterArtifactV2,
    },
};
use sha2::{Digest, Sha256};

const HELP: &str = "\
Generate an unsigned ABI-20 candidate, then finalize those exact bytes after approval.

Usage:
  cargo run -p iroha_core --bin kagemusha_recursive_spend_v4_bundle -- \\
    generate-candidate \\
    --out-dir <new-directory> \\
    --chain-id <chain> --asset-definition-id <asset> --asset-scale <u32> \\
    --generation <id> --parameter-generation <id> \\
    --source-commit <40-lower-hex> --source-tree-sha256 <64-lower-hex> \\
    --source-repo-dirty <true|false> \\
    --activation-height <u64> --withdrawal-height <u64> \\
    --benchmark-evidence-sha256 <64-lower-hex> \\
    --cryptographic-review-sha256 <64-lower-hex> \\
    --step-eq-circuit-params <canonical-norito-file> \\
    --step-ep-circuit-params <canonical-norito-file> \\
    --topup-finality-roster <canonical-norito-file>

  cargo run -p iroha_core --bin kagemusha_recursive_spend_v4_bundle -- \\
    finalize-release \\
    --candidate-dir <generated-candidate> \\
    --out-dir <new-final-directory> \\
    --release-policy <canonical-norito-file> \\
    --release-attestation <canonical-norito-file> \\
    --benchmark-evidence <exact-file> \\
    --cryptographic-review <exact-file>

Candidate generation emits five roles per parity in exact Eq-then-Ep order:
ParamsIPA, raw canonical CircuitParamsV4, processed proving key, processed
verifying key, and the final-VK selector-zero BootstrapWitness. It writes a
zero-attestation candidate manifest and canonical role-separated signing
payloads; that directory is not an approved release. Finalization verifies the
signed attestation thresholds and both evidence files against the candidate,
rechecks every staged inode/size/hash, and copies those exact bytes without
running keygen or proof generation. Both output directories must be new.
";

const GENERATE_OPTIONS: &[&str] = &[
    "out-dir",
    "chain-id",
    "asset-definition-id",
    "asset-scale",
    "generation",
    "parameter-generation",
    "source-commit",
    "source-tree-sha256",
    "source-repo-dirty",
    "activation-height",
    "withdrawal-height",
    "benchmark-evidence-sha256",
    "cryptographic-review-sha256",
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

const MANIFEST_JSON_FILE_NAME: &str = "manifest.json";
const MANIFEST_NORITO_FILE_NAME: &str = "manifest.norito";
const MANIFEST_NORITO_SHA256_FILE_NAME: &str = "manifest.norito.sha256";
const CANDIDATE_MANIFEST_JSON_FILE_NAME: &str = "candidate-manifest.json";
const CANDIDATE_MANIFEST_NORITO_FILE_NAME: &str = "candidate-manifest.norito";
const CANDIDATE_MANIFEST_SHA256_FILE_NAME: &str = "candidate-manifest.norito.sha256";
const SIGNING_SUBJECT_FILE_NAME: &str = "release-signing-subject.norito";
const SIGNING_SUBJECT_SHA256_FILE_NAME: &str = "release-signing-subject.norito.sha256";
const RELEASE_APPROVAL_PAYLOAD_FILE_NAME: &str = "approval-release.norito";
const CRYPTOGRAPHIC_REVIEW_APPROVAL_PAYLOAD_FILE_NAME: &str =
    "approval-cryptographic-review.norito";
const BENCHMARK_APPROVAL_PAYLOAD_FILE_NAME: &str = "approval-physical-device-benchmark.norito";
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const MAX_POLICY_BYTES: u64 = 64 * 1024;
const MAX_ATTESTATION_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct InputSpec {
    file_name: &'static str,
    parity: KagemushaPastaCycleParityV1,
    kind: KagemushaPastaCycleArtifactKindV4,
}

const INPUTS: [InputSpec; 10] = [
    InputSpec {
        file_name: KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PARAMETERS_FILE_NAME_V4,
        parity: KagemushaPastaCycleParityV1::StepEq,
        kind: KagemushaPastaCycleArtifactKindV4::Parameters,
    },
    InputSpec {
        file_name: KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_PARAMS_FILE_NAME_V4,
        parity: KagemushaPastaCycleParityV1::StepEq,
        kind: KagemushaPastaCycleArtifactKindV4::CircuitParams,
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
        file_name: KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PARAMETERS_FILE_NAME_V4,
        parity: KagemushaPastaCycleParityV1::StepEp,
        kind: KagemushaPastaCycleArtifactKindV4::Parameters,
    },
    InputSpec {
        file_name: KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_PARAMS_FILE_NAME_V4,
        parity: KagemushaPastaCycleParityV1::StepEp,
        kind: KagemushaPastaCycleArtifactKindV4::CircuitParams,
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
}

struct OpenedInput {
    file: File,
    path: PathBuf,
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
        let current = fs::symlink_metadata(&self.path)?;
        if current.file_type().is_symlink()
            || !current.is_file()
            || FileSnapshot::from_metadata(&opened) != self.snapshot
            || FileSnapshot::from_metadata(&current) != self.snapshot
        {
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
    source_repo_dirty: bool,
    activation_height: u64,
    withdrawal_height: u64,
    benchmark_evidence_sha256: [u8; 32],
    cryptographic_review_sha256: [u8; 32],
    max_proof_bytes: u32,
    measured_proof_pair: Vec<u8>,
    profiles: [ProfileMetadata; 2],
}

struct PreparedArtifact {
    spec: InputSpec,
    payload: Vec<u8>,
    circuit_params: KagemushaStepCircuitParamsV4,
    compiled_protocol_structure_sha256: [u8; 32],
    step_proof_size_bytes: u32,
    header: KagemushaPastaCycleFramedArtifactHeaderV4,
    total_size: u64,
}

struct GeneratedArtifact {
    spec: InputSpec,
    payload: Vec<u8>,
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
        "finalize-release" => FINALIZE_OPTIONS,
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
        "generate-candidate" => build_candidate(&options),
        "finalize-release" => finalize_release(&options),
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

fn approval_payload_files() -> [(KagemushaRecursiveSpendReleaseApprovalRoleV1, &'static str); 3] {
    [
        (
            KagemushaRecursiveSpendReleaseApprovalRoleV1::Release,
            RELEASE_APPROVAL_PAYLOAD_FILE_NAME,
        ),
        (
            KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview,
            CRYPTOGRAPHIC_REVIEW_APPROVAL_PAYLOAD_FILE_NAME,
        ),
        (
            KagemushaRecursiveSpendReleaseApprovalRoleV1::PhysicalDeviceBenchmark,
            BENCHMARK_APPROVAL_PAYLOAD_FILE_NAME,
        ),
    ]
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

fn parse_bool(options: &BTreeMap<String, String>, name: &str) -> Result<bool, Box<dyn Error>> {
    match required(options, name) {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!("--{name} must be exactly true or false").into()),
    }
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
    let mut buffer = [0_u8; 64 * 1024];
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

fn prepare_bundle_metadata(
    options: &BTreeMap<String, String>,
    step_eq_params_input: &mut OpenedInput,
    step_ep_params_input: &mut OpenedInput,
) -> Result<(BundleMetadata, Vec<GeneratedArtifact>), Box<dyn Error>> {
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
    if !iroha_data_model::offline::is_kagemusha_v3_portable_identifier(&generation)
        || !iroha_data_model::offline::is_kagemusha_v3_portable_identifier(&parameter_generation)
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
    let activation_height = parse_u64(options, "activation-height")?;
    let withdrawal_height = parse_u64(options, "withdrawal-height")?;
    if activation_height == 0 || withdrawal_height <= activation_height {
        return Err("release heights must define a non-empty, nonzero activation window".into());
    }

    let benchmark_evidence_sha256 = parse_digest(options, "benchmark-evidence-sha256")?;
    let cryptographic_review_sha256 = parse_digest(options, "cryptographic-review-sha256")?;
    if benchmark_evidence_sha256 == cryptographic_review_sha256 {
        return Err("release evidence digests must be distinct".into());
    }

    let requested_step_eq_params =
        decode_canonical_circuit_params(step_eq_params_input, "--step-eq-circuit-params")?;
    let requested_step_ep_params =
        decode_canonical_circuit_params(step_ep_params_input, "--step-ep-circuit-params")?;
    let generated = generate_kagemusha_pasta_cycle_artifacts_v4(
        requested_step_eq_params,
        requested_step_ep_params,
    )
    .map_err(|error| format!("current-source Kagemusha V4 generation failed: {error}"))?;
    let step_eq = generated.step_eq;
    let step_ep = generated.step_ep;
    let measured_proof_pair = generated.measured_live_pair_bytes;

    let eq_layout = step_eq
        .circuit_params
        .validate()
        .map_err(|error| format!("generated Eq CircuitParamsV4 validation failed: {error}"))?;
    let ep_layout = step_ep
        .circuit_params
        .validate()
        .map_err(|error| format!("generated Ep CircuitParamsV4 validation failed: {error}"))?;
    if step_eq.circuit_params.k != step_ep.circuit_params.k || eq_layout != ep_layout {
        return Err("generated Eq/Ep profiles select different IPA/public layouts".into());
    }
    if step_eq.compiled_protocol_structure_sha256 == [0; 32]
        || step_ep.compiled_protocol_structure_sha256 == [0; 32]
        || step_eq.compiled_protocol_structure_sha256 == step_ep.compiled_protocol_structure_sha256
        || step_eq.step_proof_size_bytes != step_eq.circuit_params.max_parent_proof_bytes
        || step_ep.step_proof_size_bytes != step_ep.circuit_params.max_parent_proof_bytes
    {
        return Err("generated V4 profile calibration metadata is inconsistent".into());
    }

    let measured_eq = validate_kagemusha_step_bootstrap_payload_v4(
        &step_eq.bootstrap_witness,
        &step_eq.circuit_params,
        KagemushaPastaCycleParityV1::StepEq,
        step_eq.compiled_protocol_structure_sha256,
    )?;
    let measured_ep = validate_kagemusha_step_bootstrap_payload_v4(
        &step_ep.bootstrap_witness,
        &step_ep.circuit_params,
        KagemushaPastaCycleParityV1::StepEp,
        step_ep.compiled_protocol_structure_sha256,
    )?;
    if u32::try_from(measured_eq) != Ok(step_eq.step_proof_size_bytes)
        || u32::try_from(measured_ep) != Ok(step_ep.step_proof_size_bytes)
    {
        return Err("generated bootstrap measurements differ from calibrated Step sizes".into());
    }

    let max_proof_bytes = u32::try_from(measured_proof_pair.len())
        .map_err(|_| "measured V4 proof pair length does not fit u32")?;
    let measured_steps = step_eq
        .step_proof_size_bytes
        .checked_add(step_ep.step_proof_size_bytes)
        .ok_or("generated V4 Step-size sum overflow")?;
    if max_proof_bytes <= measured_steps
        || max_proof_bytes > KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4
    {
        return Err("generated V4 proof pair exceeds its provisional rejection ceiling".into());
    }
    let measured = validate_kagemusha_proof_pair_measurement_v4(
        &measured_proof_pair,
        &step_eq.circuit_params,
        &step_ep.circuit_params,
        max_proof_bytes,
    )?;
    if measured != measured_proof_pair.len() {
        return Err("V4 proof-pair validator returned a different measurement".into());
    }

    let step_eq_circuit_params = norito::to_bytes(&step_eq.circuit_params)?;
    let step_ep_circuit_params = norito::to_bytes(&step_ep.circuit_params)?;
    let generated_artifacts = vec![
        GeneratedArtifact {
            spec: INPUTS[0],
            payload: step_eq.parameters,
        },
        GeneratedArtifact {
            spec: INPUTS[1],
            payload: step_eq_circuit_params,
        },
        GeneratedArtifact {
            spec: INPUTS[2],
            payload: step_eq.proving_key,
        },
        GeneratedArtifact {
            spec: INPUTS[3],
            payload: step_eq.verifying_key,
        },
        GeneratedArtifact {
            spec: INPUTS[4],
            payload: step_eq.bootstrap_witness,
        },
        GeneratedArtifact {
            spec: INPUTS[5],
            payload: step_ep.parameters,
        },
        GeneratedArtifact {
            spec: INPUTS[6],
            payload: step_ep_circuit_params,
        },
        GeneratedArtifact {
            spec: INPUTS[7],
            payload: step_ep.proving_key,
        },
        GeneratedArtifact {
            spec: INPUTS[8],
            payload: step_ep.verifying_key,
        },
        GeneratedArtifact {
            spec: INPUTS[9],
            payload: step_ep.bootstrap_witness,
        },
    ];
    let evidence = BTreeSet::from([benchmark_evidence_sha256, cryptographic_review_sha256]);
    let mut payload_digests = BTreeMap::new();
    for artifact in &generated_artifacts {
        let payload_size = u64::try_from(artifact.payload.len())?;
        let digest: [u8; 32] = Sha256::digest(&artifact.payload).into();
        let duplicate = payload_digests.insert(digest, artifact.spec);
        let allowed_circuit_params_alias = duplicate.is_some_and(|previous| {
            previous.kind == KagemushaPastaCycleArtifactKindV4::CircuitParams
                && artifact.spec.kind == KagemushaPastaCycleArtifactKindV4::CircuitParams
                && previous.parity != artifact.spec.parity
        });
        if payload_size == 0
            || payload_size >= KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4
            || evidence.contains(&digest)
            || (duplicate.is_some() && !allowed_circuit_params_alias)
        {
            return Err(format!(
                "generated {} payload violates the V4 artifact corridor",
                artifact.spec.file_name
            )
            .into());
        }
    }

    let metadata =
        BundleMetadata {
            chain_id,
            asset,
            asset_scale,
            generation,
            parameter_generation,
            source_commit,
            source_tree_sha256: parse_digest(options, "source-tree-sha256")?,
            source_repo_dirty: parse_bool(options, "source-repo-dirty")?,
            activation_height,
            withdrawal_height,
            benchmark_evidence_sha256,
            cryptographic_review_sha256,
            max_proof_bytes,
            measured_proof_pair,
            profiles: [
                ProfileMetadata {
                    parity: KagemushaPastaCycleParityV1::StepEq,
                    circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4,
                    circuit_params_sha256: step_eq.circuit_params.sha256().map_err(|error| {
                        format!("failed to identify Eq CircuitParamsV4: {error}")
                    })?,
                    circuit_params: step_eq.circuit_params,
                    compiled_protocol_structure_sha256: step_eq.compiled_protocol_structure_sha256,
                    step_proof_size_bytes: step_eq.step_proof_size_bytes,
                },
                ProfileMetadata {
                    parity: KagemushaPastaCycleParityV1::StepEp,
                    circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
                    circuit_params_sha256: step_ep.circuit_params.sha256().map_err(|error| {
                        format!("failed to identify Ep CircuitParamsV4: {error}")
                    })?,
                    circuit_params: step_ep.circuit_params,
                    compiled_protocol_structure_sha256: step_ep.compiled_protocol_structure_sha256,
                    step_proof_size_bytes: step_ep.step_proof_size_bytes,
                },
            ],
        };
    Ok((metadata, generated_artifacts))
}

fn profile_for(metadata: &BundleMetadata, parity: KagemushaPastaCycleParityV1) -> &ProfileMetadata {
    match parity {
        KagemushaPastaCycleParityV1::StepEq => &metadata.profiles[0],
        KagemushaPastaCycleParityV1::StepEp => &metadata.profiles[1],
    }
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
        || !iroha_data_model::offline::is_kagemusha_v3_portable_identifier(&header.generation)
        || header.parity != profile.parity
        || header.circuit_id != expected_circuit_id
        || header.circuit_id != profile.circuit_id
        || !iroha_data_model::offline::is_kagemusha_v3_portable_identifier(
            &header.parameter_generation,
        )
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
        || header.manifest_schema != manifest.schema
        || header.bridge_abi_version != manifest.bridge_abi_version
        || header.proof_backend != manifest.proof_backend
        || header.transcript_profile != manifest.transcript_profile
        || header.generation != manifest.generation
        || header.parameter_generation != profile.parameter_generation
        || header.ipa_k != profile.ipa_k
        || descriptor.kind != header.kind
        || descriptor.file_name != expected_spec.file_name
        || descriptor.size_bytes != expected_size
        || descriptor.payload_size_bytes != header.payload_size_bytes
        || descriptor.payload_sha256 != header.payload_sha256
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
    let payload_size_bytes = u64::try_from(payload.len())?;
    let payload_sha256 = Sha256::digest(&payload).into();
    if spec.kind == KagemushaPastaCycleArtifactKindV4::CircuitParams
        && norito::to_bytes(&profile.circuit_params)? != payload
    {
        return Err(format!(
            "{} is not the raw canonical circuit-parameter payload",
            spec.file_name
        )
        .into());
    }
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

fn build_candidate(options: &BTreeMap<String, String>) -> Result<(), Box<dyn Error>> {
    #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos")))]
    return Err(
        "Kagemusha V4 bundle publication requires Linux, Android, or macOS atomic directory publication"
            .into(),
    );

    #[cfg(any(target_os = "linux", target_os = "android", target_os = "macos"))]
    {
        let out_dir = PathBuf::from(required(options, "out-dir"));
        if out_dir.exists() {
            return Err(format!("output directory already exists: {}", out_dir.display()).into());
        }
        let trusted_parent = TrustedOutputParent::open(&out_dir)?;
        let mut step_eq_params_input = open_input(
            Path::new(required(options, "step-eq-circuit-params")),
            1024 * 1024,
            "Eq inline circuit parameters",
        )?;
        let mut step_ep_params_input = open_input(
            Path::new(required(options, "step-ep-circuit-params")),
            1024 * 1024,
            "Ep inline circuit parameters",
        )?;
        let (metadata, generated_artifacts) = prepare_bundle_metadata(
            options,
            &mut step_eq_params_input,
            &mut step_ep_params_input,
        )?;
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
        if roster_input.snapshot.identity() == step_eq_params_input.snapshot.identity()
            || roster_input.snapshot.identity() == step_ep_params_input.snapshot.identity()
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

        use std::os::unix::fs::PermissionsExt as _;

        let mut staging_builder = tempfile::Builder::new();
        staging_builder
            .prefix(".kagemusha-v4-staging-")
            .permissions(fs::Permissions::from_mode(0o700));
        let staging = staging_builder.tempdir_in(&trusted_parent.path)?;
        let staging_name = staging
            .path()
            .file_name()
            .ok_or("temporary publication directory has no file name")?
            .to_owned();
        let publication = PublicationDirectory::open_at(
            &trusted_parent.file,
            staging.path().to_owned(),
            &staging_name,
        )?;
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
        publication.verify_candidate_inventory()?;
        publication.sync()?;
        trusted_parent.publish(&staging_name)?;
        let _published = staging.keep();
        Ok(())
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

fn finalize_release(options: &BTreeMap<String, String>) -> Result<(), Box<dyn Error>> {
    #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos")))]
    return Err(
        "Kagemusha V4 release finalization requires Linux, Android, or macOS atomic directory publication"
            .into(),
    );

    #[cfg(any(target_os = "linux", target_os = "android", target_os = "macos"))]
    {
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
        let candidate_manifest: KagemushaRecursiveSpendArtifactManifestV4 =
            decode_canonical_norito(&candidate_manifest_bytes, "V4 candidate manifest")?;
        candidate_manifest
            .validate_unsigned_candidate()
            .map_err(|error| format!("invalid V4 candidate manifest: {error}"))?;
        if candidate_manifest.validate().is_ok() {
            return Err("unsigned V4 candidate was accepted as a finalized manifest".into());
        }
        tracked_metadata.push(candidate_manifest_input);

        let mut candidate_json = norito::json::to_string_pretty(&candidate_manifest)?;
        candidate_json.push('\n');
        let candidate_manifest_sha256: [u8; 32] = Sha256::digest(&candidate_manifest_bytes).into();
        let candidate_manifest_sha256_text =
            format!("{}\n", hex::encode(candidate_manifest_sha256));
        let candidate_subject = candidate_manifest
            .release_attestation_candidate_subject()
            .map_err(|error| format!("failed to derive V4 candidate subject: {error}"))?;
        let candidate_subject_bytes =
            canonical_norito_bytes(&candidate_subject, "V4 candidate signing subject")?;
        let candidate_subject_sha256_text = format!(
            "{}\n",
            hex::encode(Sha256::digest(&candidate_subject_bytes))
        );

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
            (
                SIGNING_SUBJECT_FILE_NAME,
                MAX_MANIFEST_BYTES,
                "V4 candidate signing subject",
                candidate_subject_bytes.as_slice(),
            ),
            (
                SIGNING_SUBJECT_SHA256_FILE_NAME,
                65,
                "V4 candidate signing subject digest",
                candidate_subject_sha256_text.as_bytes(),
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
        for (role, name) in approval_payload_files() {
            let payload = candidate_subject.approval_payload(role);
            let bytes = canonical_norito_bytes(&payload, "V4 candidate approval payload")?;
            open_and_match_candidate_file(
                &candidate,
                name,
                MAX_MANIFEST_BYTES,
                "V4 candidate approval payload",
                &bytes,
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
            evidence_maximum,
            "V4 cryptographic review evidence",
        )?;
        let review_bytes = review_input.read_all()?;

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

        let mut manifest = candidate_manifest.clone();
        manifest.release_attestation_sha256 = Sha256::digest(&attestation_bytes).into();
        manifest
            .validate()
            .map_err(|error| format!("final V4 manifest is invalid: {error}"))?;
        if manifest
            .release_attestation_subject()
            .map_err(|error| format!("failed to derive finalized V4 subject: {error}"))?
            != candidate_subject
        {
            return Err("final V4 manifest changed the candidate signing subject".into());
        }
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
            return Err("V4 candidate does not contain the exact ten-artifact inventory".into());
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
        let manifest_sha256_text = format!("{}\n", hex::encode(Sha256::digest(&manifest_bytes)));

        let out_dir = PathBuf::from(required(options, "out-dir"));
        if out_dir.exists() {
            return Err(format!("output directory already exists: {}", out_dir.display()).into());
        }
        let trusted_parent = TrustedOutputParent::open(&out_dir)?;
        use std::os::unix::fs::PermissionsExt as _;

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
        for descriptor in manifest
            .profiles
            .iter()
            .flat_map(|profile| profile.artifacts.iter())
        {
            let header = headers
                .get(&descriptor.file_name)
                .ok_or("validated V4 artifact header disappeared")?;
            publication.verify_framed_artifact(&authenticated, descriptor, header)?;
        }
        publication.verify_file_digest(
            &roster_descriptor.file_name,
            roster_descriptor.size_bytes,
            roster_descriptor.sha256,
            KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_V2,
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
        publication.verify_final_inventory()?;
        publication.sync()?;
        trusted_parent.publish(&staging_name)?;
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
    if roster.chain_id != metadata.chain_id || roster.artifact_generation != metadata.generation {
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

fn write_candidate(
    publication: &PublicationDirectory,
    metadata: BundleMetadata,
    prepared: Vec<PreparedArtifact>,
    roster_bytes: &[u8],
    roster_descriptor: KagemushaTopUpFinalityRosterArtifactReferenceV4,
) -> Result<(), Box<dyn Error>> {
    let mut eq_artifacts = Vec::with_capacity(5);
    let mut ep_artifacts = Vec::with_capacity(5);
    let mut staged_headers = Vec::with_capacity(10);
    for artifact in prepared {
        let (header, descriptor) = package_artifact(publication, artifact)?;
        match header.parity {
            KagemushaPastaCycleParityV1::StepEq => eq_artifacts.push(descriptor.clone()),
            KagemushaPastaCycleParityV1::StepEp => ep_artifacts.push(descriptor.clone()),
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
        source_repo_dirty: metadata.source_repo_dirty,
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
                artifacts: eq_artifacts,
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
                artifacts: ep_artifacts,
            },
        ],
        topup_finality_roster_artifact: roster_descriptor,
        benchmark_evidence_sha256: metadata.benchmark_evidence_sha256,
        cryptographic_review_sha256: metadata.cryptographic_review_sha256,
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
        || u32::try_from(measured) != Ok(manifest.max_proof_bytes)
    {
        return Err("generated V4 manifest changed the measured proof-pair limit".into());
    }

    let manifest_norito = canonical_norito_bytes(&manifest, "V4 candidate manifest")?;
    let mut manifest_json = norito::json::to_string_pretty(&manifest)?;
    manifest_json.push('\n');
    let manifest_sha256: [u8; 32] = Sha256::digest(&manifest_norito).into();
    let manifest_sha256_text = format!("{}\n", hex::encode(manifest_sha256));
    let subject = manifest
        .release_attestation_candidate_subject()
        .map_err(|error| format!("failed to derive V4 candidate signing subject: {error}"))?;
    let subject_norito = canonical_norito_bytes(&subject, "V4 signing subject")?;
    let subject_sha256_text = format!("{}\n", hex::encode(Sha256::digest(&subject_norito)));

    for (name, bytes) in [
        (
            CANDIDATE_MANIFEST_NORITO_FILE_NAME,
            manifest_norito.as_slice(),
        ),
        (CANDIDATE_MANIFEST_JSON_FILE_NAME, manifest_json.as_bytes()),
        (
            CANDIDATE_MANIFEST_SHA256_FILE_NAME,
            manifest_sha256_text.as_bytes(),
        ),
        (SIGNING_SUBJECT_FILE_NAME, subject_norito.as_slice()),
        (
            SIGNING_SUBJECT_SHA256_FILE_NAME,
            subject_sha256_text.as_bytes(),
        ),
    ] {
        publication.write_exact_file(name, bytes)?;
    }
    for (role, name) in approval_payload_files() {
        let payload = subject.approval_payload(role);
        let bytes = canonical_norito_bytes(&payload, "V4 role-separated approval payload")?;
        publication.write_exact_file(name, &bytes)?;
        publication.verify_exact_file(name, &bytes, MAX_MANIFEST_BYTES)?;
    }

    for (name, bytes, maximum) in [
        (
            CANDIDATE_MANIFEST_NORITO_FILE_NAME,
            manifest_norito.as_slice(),
            MAX_MANIFEST_BYTES,
        ),
        (
            CANDIDATE_MANIFEST_JSON_FILE_NAME,
            manifest_json.as_bytes(),
            MAX_MANIFEST_BYTES,
        ),
        (
            CANDIDATE_MANIFEST_SHA256_FILE_NAME,
            manifest_sha256_text.as_bytes(),
            65,
        ),
        (
            SIGNING_SUBJECT_FILE_NAME,
            subject_norito.as_slice(),
            MAX_MANIFEST_BYTES,
        ),
        (
            SIGNING_SUBJECT_SHA256_FILE_NAME,
            subject_sha256_text.as_bytes(),
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
    let export_profile = KagemushaPastaCycleProofProfileV4 {
        parity: artifact.header.parity,
        circuit_id: artifact.header.circuit_id.clone(),
        parameter_generation: artifact.header.parameter_generation.clone(),
        ipa_k: artifact.header.ipa_k,
        circuit_params: artifact.circuit_params,
        compiled_protocol_structure_sha256: artifact.compiled_protocol_structure_sha256,
        step_proof_size_bytes: artifact.step_proof_size_bytes,
        artifacts: Vec::new(),
    };
    let mut output = publication.create_file(artifact.spec.file_name)?;
    let descriptor = write_kagemusha_pasta_cycle_artifact_v4(
        &mut output,
        &artifact.header.generation,
        &export_profile,
        artifact.spec.kind,
        &artifact.payload,
    )?;
    output.sync_all()?;
    drop(output);
    if descriptor.file_name != artifact.spec.file_name
        || descriptor.size_bytes != artifact.total_size
        || descriptor.payload_size_bytes != artifact.header.payload_size_bytes
        || descriptor.payload_sha256 != artifact.header.payload_sha256
    {
        return Err(format!(
            "core framing changed generated {} metadata",
            artifact.spec.file_name
        )
        .into());
    }
    Ok((artifact.header, descriptor))
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
        use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

        let output_name = out_dir
            .file_name()
            .filter(|name| !name.is_empty())
            .ok_or("--out-dir must end in one directory name")?
            .to_owned();
        let parent = out_dir
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
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
        let final_path = path.join(&output_name);
        match fs::symlink_metadata(&final_path) {
            Ok(_) => {
                return Err(
                    format!("output directory already exists: {}", final_path.display()).into(),
                );
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let file = File::open(&path)?;
        let opened = file.metadata()?;
        let current = fs::metadata(&path)?;
        if !opened.is_dir() || opened.dev() != current.dev() || opened.ino() != current.ino() {
            return Err("output parent changed while it was opened".into());
        }
        Ok(Self {
            path,
            file,
            output_name,
        })
    }

    fn publish(&self, staging_name: &std::ffi::OsStr) -> Result<(), Box<dyn Error>> {
        self.file.sync_all()?;
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
        Self::validate(path, file)
    }

    #[cfg(unix)]
    fn open_existing(path: PathBuf) -> io::Result<Self> {
        let before = fs::symlink_metadata(&path)?;
        if before.file_type().is_symlink() || !before.is_dir() {
            return Err(io::Error::other(
                "publication directory is not a real directory",
            ));
        }
        let path = fs::canonicalize(path)?;
        use rustix::fs::{Mode, OFlags};

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
        Self::validate(path, file)
    }

    fn validate(path: PathBuf, file: File) -> io::Result<Self> {
        let opened = file.metadata()?;
        if !opened.is_dir() {
            return Err(io::Error::other(
                "publication descriptor is not a directory",
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

            let current = fs::metadata(&path)?;
            if opened.dev() != current.dev()
                || opened.ino() != current.ino()
                || opened.permissions().mode() & 0o077 != 0
            {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "publication directory must remain owner-private",
                ));
            }
        }
        Ok(Self { path, file })
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
        let current = fs::symlink_metadata(&path)?;
        let snapshot = FileSnapshot::from_metadata(&opened);
        if current.file_type().is_symlink()
            || !current.is_file()
            || FileSnapshot::from_metadata(&current) != snapshot
            || snapshot.length == 0
            || snapshot.length > maximum
        {
            return Err(format!("{label} changed while it was opened: {}", path.display()).into());
        }
        let sha256 = hash_open_file(&mut file, snapshot.length, &path)?;
        file.seek(SeekFrom::Start(0))?;
        Ok(OpenedInput {
            file,
            path,
            snapshot,
            size_bytes: snapshot.length,
            sha256,
        })
    }

    fn verify_identity(&self) -> io::Result<()> {
        let opened = self.file.metadata()?;
        let current = fs::symlink_metadata(&self.path)?;
        if current.file_type().is_symlink()
            || !current.is_dir()
            || FileSnapshot::from_metadata(&opened) != FileSnapshot::from_metadata(&current)
        {
            return Err(io::Error::other("publication directory identity changed"));
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
    ) -> Result<(), Box<dyn Error>> {
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
        Ok(())
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
        let mut buffer = [0_u8; 64 * 1024];
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
                SIGNING_SUBJECT_FILE_NAME,
                SIGNING_SUBJECT_SHA256_FILE_NAME,
                RELEASE_APPROVAL_PAYLOAD_FILE_NAME,
                CRYPTOGRAPHIC_REVIEW_APPROVAL_PAYLOAD_FILE_NAME,
                BENCHMARK_APPROVAL_PAYLOAD_FILE_NAME,
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
            ])
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        self.verify_inventory(&expected)
    }

    fn verify_inventory(&self, expected: &BTreeSet<String>) -> io::Result<()> {
        self.verify_identity()?;
        let mut actual = BTreeSet::new();
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
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt as _;
                if metadata.nlink() != 1 {
                    return Err(io::Error::other(
                        "publication file has an external hard link",
                    ));
                }
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
    use std::os::unix::fs::PermissionsExt as _;

    use super::*;

    #[test]
    fn artifact_inventory_is_exact_eq_then_ep_five_role_order() {
        assert_eq!(INPUTS.len(), 10);
        assert_eq!(
            INPUTS.map(|spec| (spec.parity, spec.kind)),
            [
                (
                    KagemushaPastaCycleParityV1::StepEq,
                    KagemushaPastaCycleArtifactKindV4::Parameters,
                ),
                (
                    KagemushaPastaCycleParityV1::StepEq,
                    KagemushaPastaCycleArtifactKindV4::CircuitParams,
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
                    KagemushaPastaCycleArtifactKindV4::Parameters,
                ),
                (
                    KagemushaPastaCycleParityV1::StepEp,
                    KagemushaPastaCycleArtifactKindV4::CircuitParams,
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
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PARAMETERS_FILE_NAME_V4,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_PARAMS_FILE_NAME_V4,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PROVING_KEY_FILE_NAME_V4,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFYING_KEY_FILE_NAME_V4,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_BOOTSTRAP_FILE_NAME_V4,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PARAMETERS_FILE_NAME_V4,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_PARAMS_FILE_NAME_V4,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PROVING_KEY_FILE_NAME_V4,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFYING_KEY_FILE_NAME_V4,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_BOOTSTRAP_FILE_NAME_V4,
            ]
        );
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
            num_advice_per_phase: vec![8, 1, 1],
            num_lookup_advice_per_phase: vec![1, 0, 0],
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
            kind: KagemushaPastaCycleArtifactKindV4::Parameters,
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
                payload: payload.to_vec(),
                circuit_params,
                compiled_protocol_structure_sha256: [0x43; 32],
                step_proof_size_bytes: 4096,
                header: header.clone(),
                total_size,
            },
        )
        .expect("frame test payload");
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
        assert_eq!(descriptor.payload_sha256, Sha256::digest(payload).into());
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
