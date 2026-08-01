//! Isolated native exact-12 privacy release evidence runner for Taira.
//!
//! The public modes never prove in-process. Each of the 48 mandatory stages is
//! re-executed through this exact executable's hidden child mode, with a
//! parent-enforced wall-clock, resident-memory, and virtual-address-space
//! ceilings. Norito is authoritative; JSON is only a typed projection.

#![cfg(unix)]

use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    error::Error,
    ffi::OsString,
    fs::{self, File, Metadata, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    mem::MaybeUninit,
    os::fd::{AsRawFd, FromRawFd, RawFd},
    os::unix::{
        fs::{MetadataExt, OpenOptionsExt},
        process::{CommandExt, ExitStatusExt},
    },
    path::{Component, Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

#[cfg(target_os = "linux")]
use std::{
    ffi::CString,
    os::{
        fd::IntoRawFd,
        unix::{ffi::OsStrExt, fs::FileExt},
    },
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};

use iroha_core::privacy_release_evidence::{
    PRIVACY_RELEASE_CASE_COUNT_V1, PRIVACY_RELEASE_EVIDENCE_SCHEMA_VERSION_V1,
    PRIVACY_RELEASE_MAX_PROOF_ARTIFACT_BYTES_V1, PRIVACY_RELEASE_MAX_PROOF_ARTIFACTS_V1,
    PRIVACY_RELEASE_MAX_TOTAL_PROOF_ARTIFACT_BYTES_V1, PRIVACY_RELEASE_PROOF_ARTIFACT_COUNT_V1,
    PRIVACY_RELEASE_RAYON_THREAD_COUNT_V1, PRIVACY_RELEASE_STAGE_COORDINATES_V1,
    PRIVACY_RELEASE_STAGE_COUNT_V1, PRIVACY_RELEASE_STAGE_STACK_BYTES_V1, PrivacyReleaseCaseKindV1,
    PrivacyReleaseFailureClassV1, PrivacyReleaseProofArtifactEvidenceV1,
    PrivacyReleaseResourceFactsV1, PrivacyReleaseStageCoordinateV1, PrivacyReleaseStageEvidenceV1,
    initialize_privacy_release_rayon_pool_v1, privacy_exact12_matrix_bytes_v1,
    privacy_exact12_typed_envelope_rows_v1, privacy_release_process_profile_v1,
    privacy_release_proof_artifact_ceiling_v1, privacy_release_proof_artifact_count_v1,
    privacy_release_protocol_descriptor_v1, privacy_release_resource_facts_v1,
    privacy_release_stage_ordinal_v1, run_privacy_release_stage_v1,
    validate_privacy_release_proof_artifacts_v1, validate_privacy_release_stage_coordinates_v1,
};
use iroha_crypto::{sha256, sha256_reader_bounded};
use iroha_data_model::privacy::{PRIVACY_RETIRED_PROTOCOL_LABELS_V1, PrivacyProtocolIdV1};
#[cfg(all(
    test,
    target_os = "linux",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
use nix::sys::resource::getrlimit;
use nix::{
    libc,
    sys::resource::{Resource, rlim_t, setrlimit},
    unistd::Pid,
};
use norito::{
    DecodeLimits,
    derive::{JsonDeserialize, JsonSerialize},
};

#[path = "taira_privacy_release_runner/expectation_pins.rs"]
mod expectation_pins;
#[path = "taira_privacy_release_runner/process_resources.rs"]
mod process_resources;
#[path = "taira_privacy_release_runner/resource_certificate.rs"]
mod resource_certificate;
use expectation_pins::empty_expected_evidence;
#[cfg(all(
    target_os = "linux",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
use process_resources::install_pre_exec_stage_stack_limit_v1;
#[cfg(test)]
use process_resources::{
    SampledProcessMemoryV1, parse_process_status_memory_v1, rusage_peak_rss_bytes, wait4_exact_pid,
};
use process_resources::{
    StageChildGuardV1, StageProcessCeilingsV1, WaitedStageChildV1,
    canonical_stage_process_ceilings_v1, checked_stage_cpu_limit_seconds_v1, current_process_id_v1,
    elapsed_millis_ceil, install_hidden_stage_resource_limits, kill_stage_process_group,
    sample_process_memory_v1, validate_process_ceilings, validate_stage_process_ceilings_v1,
};

const ARTIFACT_SCHEMA_VERSION_V1: u16 = 1;
const MAX_EXACT12_BYTES: u64 = 64 * 1024;
// Per-stage structural allowance for ordinals, hashes, the closed descriptor,
// resource facts, timing ceilings, vector frames, and codec alignment.
// Canonical proof payloads are budgeted separately at the consensus cap.
const RELEASE_STAGE_METADATA_NORITO_BYTES_V1: u64 = 4 * 1024;
// JSON needs field names, indentation, and decimal rendering in addition to
// the exact base64 payload budget computed below.
const RELEASE_STAGE_METADATA_JSON_BYTES_V1: u64 = 8 * 1024;
// Fixed top-level collection/version framing, separate from per-stage bounds.
const RELEASE_COLLECTION_FRAMING_BYTES_V1: u64 = 64 * 1024;
const RELEASE_JSON_COLLECTION_FRAMING_BYTES_V1: u64 = 256 * 1024;
const RELEASE_DECODE_STRUCTURAL_ELEMENTS_V1: usize = 16 * 1024;
const MAX_RELEASE_PROOF_JSON_BASE64_BYTES_V1: u64 = checked_release_size_mul_v1(
    PRIVACY_RELEASE_PROOF_ARTIFACT_COUNT_V1 as u64,
    base64_encoded_len_v1(PRIVACY_RELEASE_MAX_PROOF_ARTIFACT_BYTES_V1),
);
const MAX_EXPECTATIONS_NORITO_BYTES: u64 = checked_release_size_add_v1(
    checked_release_size_add_v1(
        PRIVACY_RELEASE_MAX_TOTAL_PROOF_ARTIFACT_BYTES_V1,
        checked_release_size_mul_v1(
            PRIVACY_RELEASE_STAGE_COUNT_V1 as u64,
            RELEASE_STAGE_METADATA_NORITO_BYTES_V1,
        ),
    ),
    RELEASE_COLLECTION_FRAMING_BYTES_V1,
);
const MAX_EXPECTATIONS_JSON_BYTES: u64 = checked_release_size_add_v1(
    checked_release_size_add_v1(
        MAX_RELEASE_PROOF_JSON_BASE64_BYTES_V1,
        checked_release_size_mul_v1(
            PRIVACY_RELEASE_STAGE_COUNT_V1 as u64,
            RELEASE_STAGE_METADATA_JSON_BYTES_V1,
        ),
    ),
    RELEASE_JSON_COLLECTION_FRAMING_BYTES_V1,
);
const MAX_COMMAND_MANIFEST_NORITO_BYTES: u64 = 1024 * 1024;
const MAX_COMMAND_MANIFEST_JSON_BYTES: u64 = 2 * 1024 * 1024;
const MAX_STAGE_ARTIFACTS_NORITO_BYTES: u64 = MAX_EXPECTATIONS_NORITO_BYTES;
const MAX_STAGE_ARTIFACTS_JSON_BYTES: u64 = MAX_EXPECTATIONS_JSON_BYTES;
const MAX_RECEIPT_NORITO_BYTES: u64 = 1024 * 1024;
const MAX_RECEIPT_JSON_BYTES: u64 = 2 * 1024 * 1024;
const MAX_CARGO_LOCK_BYTES: u64 = 64 * 1024 * 1024;
const MAX_EXECUTABLE_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_CHILD_RESULT_BYTES: u64 = checked_release_size_add_v1(
    checked_release_size_mul_v1(
        PRIVACY_RELEASE_MAX_PROOF_ARTIFACT_BYTES_V1,
        PRIVACY_RELEASE_MAX_PROOF_ARTIFACTS_V1 as u64,
    ),
    RELEASE_STAGE_METADATA_NORITO_BYTES_V1,
);
const MAX_CHILD_DIAGNOSTIC_BYTES: u64 = 16 * 1024;
const MAX_STAGE_ELAPSED_MILLIS: u64 = 60 * 60 * 1_000;
const MIN_STAGE_PEAK_RSS_BYTES: u64 = 1024 * 1024;
const MAX_STAGE_PEAK_RSS_BYTES: u64 = 64 * 1024 * 1024 * 1024;
const MIN_STAGE_ADDRESS_SPACE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_STAGE_ADDRESS_SPACE_BYTES: u64 = 1024 * 1024 * 1024 * 1024;
const STAGE_RAYON_THREAD_COUNT_V1: u16 = PRIVACY_RELEASE_RAYON_THREAD_COUNT_V1;
const STAGE_RAYON_THREADS_V1: &str = "4";
const STAGE_WATCHDOG_THREAD_COUNT_V1: u64 = 1;
// The leader, four eagerly initialized global Rayon workers, and the
// parent-death watchdog are the complete immutable proof-phase task set.
const MAX_STAGE_TASKS_V1: u64 =
    1 + STAGE_RAYON_THREAD_COUNT_V1 as u64 + STAGE_WATCHDOG_THREAD_COUNT_V1;
// stdin, stdout, stderr, and the sole anonymous result descriptor. Native
// release engines are compute-only and must not open any additional file.
const MAX_STAGE_OPEN_FILES_V1: u64 = 4;
// FD 4 retains a pre-Landlock `/proc/self/task` directory anchor while FD 5
// transiently holds the Landlock ruleset. The task anchor proves that the
// eagerly initialized worker/watchdog topology is exact after restriction;
// both descriptors close before this ceiling is irreversibly lowered.
const MAX_STAGE_SETUP_OPEN_FILES_V1: u64 = MAX_STAGE_OPEN_FILES_V1 + 2;
const CANONICAL_STAGE_RESULT_FD_V1: RawFd = 3;
const STAGE_TASK_DIRECTORY_FD_V1: RawFd = 4;
const STAGE_LANDLOCK_RULESET_FD_V1: RawFd = 5;
const MINIMUM_LANDLOCK_ABI_V1: u16 = 3;
const CHILD_POLL_INTERVAL: Duration = Duration::from_millis(10);

const fn checked_release_size_add_v1(left: u64, right: u64) -> u64 {
    match left.checked_add(right) {
        Some(sum) => sum,
        None => panic!("privacy release encoded-size addition overflow"),
    }
}

const fn checked_release_size_mul_v1(left: u64, right: u64) -> u64 {
    match left.checked_mul(right) {
        Some(product) => product,
        None => panic!("privacy release encoded-size multiplication overflow"),
    }
}

const fn base64_encoded_len_v1(byte_len: u64) -> u64 {
    checked_release_size_mul_v1(byte_len.div_ceil(3), 4)
}

type DynError = Box<dyn Error + Send + Sync>;

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::codec::Encode,
    norito::codec::Decode,
    JsonSerialize,
    JsonDeserialize,
)]
#[norito(deny_unknown_fields)]
struct PrivacyReleaseExpectedStageV1 {
    evidence: PrivacyReleaseStageEvidenceV1,
    max_elapsed_millis: u64,
    max_peak_rss_bytes: u64,
    max_address_space_bytes: u64,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::codec::Encode,
    norito::codec::Decode,
    JsonSerialize,
    JsonDeserialize,
)]
#[norito(deny_unknown_fields)]
struct PrivacyReleaseExpectationsV1 {
    schema_version: u16,
    stage_count: u16,
    stages: Vec<PrivacyReleaseExpectedStageV1>,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::codec::Encode,
    norito::codec::Decode,
    JsonSerialize,
    JsonDeserialize,
)]
#[norito(deny_unknown_fields)]
struct PrivacyReleaseIsolationPolicyV1 {
    stage_rayon_threads: u16,
    main_thread_stack_bytes: u64,
    rayon_worker_stack_bytes: u64,
    watchdog_thread_stack_bytes: u64,
    max_stage_tasks: u64,
    max_stage_open_files: u64,
    max_stage_result_file_bytes: u64,
    max_stage_diagnostic_bytes: u64,
    core_dump_bytes: u64,
    static_elf_only: bool,
    anonymous_sealed_runner: bool,
    anonymous_result_descriptor_only: bool,
    exact_environment_only: bool,
    landlock_abi_minimum: u16,
    seccomp_tsync: bool,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::codec::Encode,
    norito::codec::Decode,
    JsonSerialize,
    JsonDeserialize,
)]
#[norito(deny_unknown_fields)]
struct PrivacyReleaseCommandManifestV1 {
    schema_version: u16,
    build_profile: String,
    source_sha256: [u8; 32],
    exact12_matrix_sha256: [u8; 32],
    expectations_norito_sha256: [u8; 32],
    expectations_json_sha256: [u8; 32],
    x509_resource_norito_sha256: [u8; 32],
    x509_resource_json_sha256: [u8; 32],
    cargo_lock_sha256: [u8; 32],
    validator_binary_sha256: [u8; 32],
    runner_binary_sha256: [u8; 32],
    isolation_policy: PrivacyReleaseIsolationPolicyV1,
    command_arguments: Vec<String>,
    stage_command_template: Vec<String>,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::codec::Encode,
    norito::codec::Decode,
    JsonSerialize,
    JsonDeserialize,
)]
#[norito(deny_unknown_fields)]
struct PrivacyReleaseMeasuredStageV1 {
    evidence: PrivacyReleaseStageEvidenceV1,
    elapsed_millis: u64,
    peak_rss_bytes: u64,
    peak_address_space_bytes: u64,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::codec::Encode,
    norito::codec::Decode,
    JsonSerialize,
    JsonDeserialize,
)]
#[norito(deny_unknown_fields)]
struct PrivacyReleaseStageArtifactsV1 {
    schema_version: u16,
    stage_count: u16,
    stages: Vec<PrivacyReleaseMeasuredStageV1>,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::codec::Encode,
    norito::codec::Decode,
    JsonSerialize,
    JsonDeserialize,
)]
#[norito(deny_unknown_fields)]
struct PrivacyReleaseArtifactPairDigestV1 {
    norito_sha256: [u8; 32],
    json_sha256: [u8; 32],
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::codec::Encode,
    norito::codec::Decode,
    JsonSerialize,
    JsonDeserialize,
)]
#[norito(deny_unknown_fields)]
struct PrivacyReleaseReceiptV1 {
    schema_version: u16,
    build_profile: String,
    source_sha256: [u8; 32],
    exact12_matrix_sha256: [u8; 32],
    expectations: PrivacyReleaseArtifactPairDigestV1,
    x509_resource: PrivacyReleaseArtifactPairDigestV1,
    cargo_lock_sha256: [u8; 32],
    validator_binary_sha256: [u8; 32],
    runner_binary_sha256: [u8; 32],
    command_manifest: PrivacyReleaseArtifactPairDigestV1,
    stage_artifacts: PrivacyReleaseArtifactPairDigestV1,
    fixed_stage_count: u16,
    all_native_stages_passed: bool,
    contains_witnesses: bool,
    contains_canonical_proof_artifacts: bool,
    isolation_policy_enforced: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct FileIdentityV1 {
    device: u64,
    inode: u64,
}

#[cfg(target_os = "linux")]
struct OutputParentAnchorV1 {
    absolute_path: PathBuf,
    directory: File,
    identity: FileIdentityV1,
}

#[cfg(target_os = "linux")]
struct OutputTargetV1 {
    absolute_path: PathBuf,
    parent_index: usize,
    basename: CString,
}

#[cfg(target_os = "linux")]
struct CreatedOutputV1 {
    target_index: usize,
    file: File,
    identity: FileIdentityV1,
    expected_length: u64,
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy)]
struct OutputEntryFactsV1 {
    identity: FileIdentityV1,
    mode: u32,
    link_count: u64,
    length: u64,
}

struct SecureInputV1 {
    bytes: Vec<u8>,
    sha256: [u8; 32],
    identity: FileIdentityV1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SecureInputErrorClassV1 {
    ExternalHardLinkAlias,
}

#[derive(Debug)]
struct SecureInputErrorV1 {
    class: SecureInputErrorClassV1,
    label: String,
    observed_links: u64,
}

impl std::fmt::Display for SecureInputErrorV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.class {
            SecureInputErrorClassV1::ExternalHardLinkAlias => write!(
                formatter,
                "{} has {} filesystem links; release inputs require exactly one",
                self.label, self.observed_links
            ),
        }
    }
}

impl Error for SecureInputErrorV1 {}

struct HashedInputV1 {
    sha256: [u8; 32],
    identity: FileIdentityV1,
    mode: u32,
}

struct ImmutableRunnerV1 {
    executable: File,
    source_path: PathBuf,
    source_identity: FileIdentityV1,
    sha256: [u8; 32],
}

#[derive(Clone)]
struct CommonInputsV1 {
    build_profile: String,
    source_sha256: [u8; 32],
    exact12_path: PathBuf,
    expectations_norito_path: PathBuf,
    expectations_json_path: PathBuf,
    x509_resource_paths: resource_certificate::ResourceInputPathsV1,
    cargo_lock_path: PathBuf,
    validator_binary_path: PathBuf,
}

struct LoadedInputsV1 {
    common: CommonInputsV1,
    exact12_sha256: [u8; 32],
    expectations: PrivacyReleaseExpectationsV1,
    expectations_norito_bytes: Vec<u8>,
    expectations_json_bytes: Vec<u8>,
    x509_resource: resource_certificate::LoadedResourceCertificateV1,
    cargo_lock_sha256: [u8; 32],
    validator_binary_sha256: [u8; 32],
    runner_binary_sha256: [u8; 32],
    runner: ImmutableRunnerV1,
}

#[derive(Clone)]
struct GenerateOutputsV1 {
    command_manifest_norito: PathBuf,
    command_manifest_json: PathBuf,
    stage_artifacts_norito: PathBuf,
    stage_artifacts_json: PathBuf,
    receipt_norito: PathBuf,
    receipt_json: PathBuf,
}

#[derive(Clone)]
struct VerifyArtifactsV1 {
    command_manifest_norito: PathBuf,
    command_manifest_json: PathBuf,
    stage_artifacts_norito: PathBuf,
    stage_artifacts_json: PathBuf,
    receipt_norito: PathBuf,
    receipt_json: PathBuf,
}

#[derive(Clone)]
struct CaptureOptionsV1 {
    exact12_path: PathBuf,
    expectations_norito_out: PathBuf,
    expectations_json_out: PathBuf,
    x509_resource: resource_certificate::CaptureResourceOptionsV1,
    max_elapsed_millis: u64,
    max_peak_rss_bytes: u64,
    max_address_space_bytes: u64,
}

#[derive(Clone)]
struct CapturedFixtureValidationOptionsV1 {
    exact12_path: PathBuf,
    expectations_norito_path: PathBuf,
    expectations_json_path: PathBuf,
    x509_resource_paths: resource_certificate::ResourceInputPathsV1,
}

#[derive(Clone, Debug)]
struct MeasuredStageV1 {
    evidence: PrivacyReleaseStageEvidenceV1,
    elapsed_millis: u64,
    peak_rss_bytes: u64,
    peak_address_space_bytes: u64,
}

fn main() {
    if let Err(error) = real_main() {
        eprintln!("taira privacy release evidence failed: {error}");
        std::process::exit(1);
    }
}

fn real_main() -> Result<(), DynError> {
    let mut arguments = env::args_os();
    let _program = arguments.next();
    let mode = arguments.next().ok_or(
        "missing mode; expected generate, verify, capture-expectations, validate-captured-fixtures, or hidden __stage",
    )?;
    let mode = mode.to_str().ok_or("mode must be valid UTF-8")?.to_owned();
    let rest: Vec<OsString> = arguments.collect();

    match mode.as_str() {
        "generate" => {
            let options = parse_options(&rest, &generate_option_names())?;
            let common = parse_common_inputs(&options)?;
            let outputs = GenerateOutputsV1 {
                command_manifest_norito: path_option(&options, "command-manifest-norito-out")?,
                command_manifest_json: path_option(&options, "command-manifest-json-out")?,
                stage_artifacts_norito: path_option(&options, "stage-artifacts-norito-out")?,
                stage_artifacts_json: path_option(&options, "stage-artifacts-json-out")?,
                receipt_norito: path_option(&options, "receipt-norito-out")?,
                receipt_json: path_option(&options, "receipt-json-out")?,
            };
            generate(common, outputs)
        }
        "verify" => {
            let options = parse_options(&rest, &verify_option_names())?;
            let common = parse_common_inputs(&options)?;
            let artifacts = VerifyArtifactsV1 {
                command_manifest_norito: path_option(&options, "command-manifest-norito")?,
                command_manifest_json: path_option(&options, "command-manifest-json")?,
                stage_artifacts_norito: path_option(&options, "stage-artifacts-norito")?,
                stage_artifacts_json: path_option(&options, "stage-artifacts-json")?,
                receipt_norito: path_option(&options, "receipt-norito")?,
                receipt_json: path_option(&options, "receipt-json")?,
            };
            verify(common, artifacts)
        }
        "capture-expectations" => {
            let options = parse_options(&rest, &resource_certificate::capture_option_names())?;
            let capture = CaptureOptionsV1 {
                exact12_path: path_option(&options, "exact12-matrix")?,
                expectations_norito_out: path_option(&options, "expectations-norito-out")?,
                expectations_json_out: path_option(&options, "expectations-json-out")?,
                x509_resource: resource_certificate::CaptureResourceOptionsV1::parse(&options)?,
                max_elapsed_millis: canonical_u64_option(&options, "elapsed-ceiling-ms")?,
                max_peak_rss_bytes: canonical_u64_option(&options, "peak-rss-ceiling-bytes")?,
                max_address_space_bytes: canonical_u64_option(
                    &options,
                    "address-space-ceiling-bytes",
                )?,
            };
            capture_expectations(capture)
        }
        "validate-captured-fixtures" => {
            let options = parse_options(&rest, &captured_fixture_validation_option_names())?;
            validate_captured_fixtures(CapturedFixtureValidationOptionsV1 {
                exact12_path: path_option(&options, "exact12-matrix")?,
                expectations_norito_path: path_option(&options, "expectations-norito")?,
                expectations_json_path: path_option(&options, "expectations-json")?,
                x509_resource_paths: resource_certificate::ResourceInputPathsV1::parse(&options)?,
            })
        }
        "__stage" => {
            let options = parse_options(&rest, &process_resources::stage_option_names())?;
            run_hidden_stage(&options)
        }
        _ => Err(format!("unknown mode `{mode}`").into()),
    }
}

fn common_option_names() -> Vec<&'static str> {
    vec![
        "build-profile",
        "source-sha256",
        "exact12-matrix",
        "expectations-norito",
        "expectations-json",
        "x509-resource-norito",
        "x509-resource-json",
        "cargo-lock",
        "validator-binary",
    ]
}

fn generate_option_names() -> Vec<&'static str> {
    let mut names = common_option_names();
    names.extend([
        "command-manifest-norito-out",
        "command-manifest-json-out",
        "stage-artifacts-norito-out",
        "stage-artifacts-json-out",
        "receipt-norito-out",
        "receipt-json-out",
    ]);
    names
}

fn verify_option_names() -> Vec<&'static str> {
    let mut names = common_option_names();
    names.extend([
        "command-manifest-norito",
        "command-manifest-json",
        "stage-artifacts-norito",
        "stage-artifacts-json",
        "receipt-norito",
        "receipt-json",
    ]);
    names
}

fn captured_fixture_validation_option_names() -> Vec<&'static str> {
    vec![
        "exact12-matrix",
        "expectations-norito",
        "expectations-json",
        "x509-resource-norito",
        "x509-resource-json",
    ]
}

fn parse_options(
    arguments: &[OsString],
    allowed_names: &[&str],
) -> Result<BTreeMap<String, String>, DynError> {
    if arguments.len() % 2 != 0 {
        return Err("every option requires exactly one value".into());
    }
    let allowed: BTreeSet<&str> = allowed_names.iter().copied().collect();
    let mut parsed = BTreeMap::new();
    for pair in arguments.chunks_exact(2) {
        let raw_name = pair[0].to_str().ok_or("option name must be valid UTF-8")?;
        let name = raw_name
            .strip_prefix("--")
            .ok_or_else(|| format!("invalid positional argument `{raw_name}`"))?;
        if !allowed.contains(name) {
            return Err(format!("unknown option `--{name}`").into());
        }
        let value = pair[1]
            .to_str()
            .ok_or_else(|| format!("value for --{name} must be valid UTF-8"))?;
        if value.is_empty() || value.starts_with("--") {
            return Err(format!("invalid empty value for --{name}").into());
        }
        if parsed.insert(name.to_owned(), value.to_owned()).is_some() {
            return Err(format!("duplicate option `--{name}`").into());
        }
    }
    let missing = allowed
        .iter()
        .filter(|name| !parsed.contains_key(**name))
        .copied()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(format!("missing required option(s): {}", missing.join(", ")).into());
    }
    Ok(parsed)
}

fn parse_common_inputs(options: &BTreeMap<String, String>) -> Result<CommonInputsV1, DynError> {
    let build_profile = option(options, "build-profile")?.to_owned();
    if build_profile != "debug" && build_profile != "release" {
        return Err("--build-profile must be exactly `debug` or `release`".into());
    }
    let compiled_build_profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    if build_profile != compiled_build_profile {
        return Err(format!(
            "--build-profile `{build_profile}` does not match this `{compiled_build_profile}` runner binary"
        )
        .into());
    }
    let source_sha256 = parse_sha256(option(options, "source-sha256")?)?;
    if source_sha256 == [0; 32] {
        return Err("--source-sha256 must not be the all-zero sentinel".into());
    }
    Ok(CommonInputsV1 {
        build_profile,
        source_sha256,
        exact12_path: path_option(options, "exact12-matrix")?,
        expectations_norito_path: path_option(options, "expectations-norito")?,
        expectations_json_path: path_option(options, "expectations-json")?,
        x509_resource_paths: resource_certificate::ResourceInputPathsV1::parse(options)?,
        cargo_lock_path: path_option(options, "cargo-lock")?,
        validator_binary_path: path_option(options, "validator-binary")?,
    })
}

fn option<'a>(options: &'a BTreeMap<String, String>, name: &str) -> Result<&'a str, DynError> {
    options
        .get(name)
        .map(String::as_str)
        .ok_or_else(|| format!("missing --{name}").into())
}

fn path_option(options: &BTreeMap<String, String>, name: &str) -> Result<PathBuf, DynError> {
    let value = option(options, name)?;
    let path = PathBuf::from(value);
    if path.as_os_str().is_empty() {
        return Err(format!("--{name} path is empty").into());
    }
    Ok(path)
}

fn canonical_u64_option(options: &BTreeMap<String, String>, name: &str) -> Result<u64, DynError> {
    let value = option(options, name)?;
    if value != "0" && (value.starts_with('0') || !value.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return Err(format!("--{name} must be canonical unsigned decimal").into());
    }
    value
        .parse::<u64>()
        .map_err(|_| format!("--{name} is outside u64").into())
}

fn canonical_raw_fd_option(
    options: &BTreeMap<String, String>,
    name: &str,
) -> Result<RawFd, DynError> {
    let value = canonical_u64_option(options, name)?;
    let fd = i32::try_from(value).map_err(|_| format!("--{name} exceeds a raw file descriptor"))?;
    if fd < 3 {
        return Err(format!("--{name} must not alias stdin, stdout, or stderr").into());
    }
    Ok(fd)
}

fn canonical_stage_result_fd_option(options: &BTreeMap<String, String>) -> Result<RawFd, DynError> {
    let fd = canonical_raw_fd_option(options, "out-fd")?;
    if fd != CANONICAL_STAGE_RESULT_FD_V1 {
        return Err("--out-fd must be canonical stage-result FD 3".into());
    }
    Ok(fd)
}

fn generate(common: CommonInputsV1, outputs: GenerateOutputsV1) -> Result<(), DynError> {
    ensure_taira_release_platform()?;
    let output_paths = generate_output_paths(&outputs);
    preflight_output_paths(&output_paths)?;
    let loaded = load_common_inputs(common, &output_paths)?;
    let measured = run_all_stages(&loaded.expectations, &loaded.runner)?;
    validate_measured_against_expectations(&measured, &loaded.expectations)?;

    let command_manifest = build_command_manifest(&loaded);
    let stage_artifacts = PrivacyReleaseStageArtifactsV1 {
        schema_version: ARTIFACT_SCHEMA_VERSION_V1,
        stage_count: u16::try_from(PRIVACY_RELEASE_STAGE_COUNT_V1)
            .expect("fixed release stage count fits u16"),
        stages: measured
            .into_iter()
            .map(|stage| PrivacyReleaseMeasuredStageV1 {
                evidence: stage.evidence,
                elapsed_millis: stage.elapsed_millis,
                peak_rss_bytes: stage.peak_rss_bytes,
                peak_address_space_bytes: stage.peak_address_space_bytes,
            })
            .collect(),
    };
    validate_stage_artifacts(&stage_artifacts, &loaded.expectations)?;

    let command_norito = canonical_norito_bytes(&command_manifest, "command manifest")?;
    let command_json = canonical_json_bytes(&command_manifest, "command manifest")?;
    let stages_norito = canonical_norito_bytes(&stage_artifacts, "stage artifacts")?;
    let stages_json = canonical_json_bytes(&stage_artifacts, "stage artifacts")?;
    enforce_encoded_size(
        command_norito.len(),
        MAX_COMMAND_MANIFEST_NORITO_BYTES,
        "command manifest Norito",
    )?;
    enforce_encoded_size(
        command_json.len(),
        MAX_COMMAND_MANIFEST_JSON_BYTES,
        "command manifest JSON",
    )?;
    enforce_encoded_size(
        stages_norito.len(),
        MAX_STAGE_ARTIFACTS_NORITO_BYTES,
        "stage artifacts Norito",
    )?;
    enforce_encoded_size(
        stages_json.len(),
        MAX_STAGE_ARTIFACTS_JSON_BYTES,
        "stage artifacts JSON",
    )?;

    let receipt = PrivacyReleaseReceiptV1 {
        schema_version: ARTIFACT_SCHEMA_VERSION_V1,
        build_profile: loaded.common.build_profile.clone(),
        source_sha256: loaded.common.source_sha256,
        exact12_matrix_sha256: loaded.exact12_sha256,
        expectations: PrivacyReleaseArtifactPairDigestV1 {
            norito_sha256: sha256_bytes(&loaded.expectations_norito_bytes),
            json_sha256: sha256_bytes(&loaded.expectations_json_bytes),
        },
        x509_resource: PrivacyReleaseArtifactPairDigestV1 {
            norito_sha256: sha256_bytes(&loaded.x509_resource.norito_bytes),
            json_sha256: sha256_bytes(&loaded.x509_resource.json_bytes),
        },
        cargo_lock_sha256: loaded.cargo_lock_sha256,
        validator_binary_sha256: loaded.validator_binary_sha256,
        runner_binary_sha256: loaded.runner_binary_sha256,
        command_manifest: PrivacyReleaseArtifactPairDigestV1 {
            norito_sha256: sha256_bytes(&command_norito),
            json_sha256: sha256_bytes(&command_json),
        },
        stage_artifacts: PrivacyReleaseArtifactPairDigestV1 {
            norito_sha256: sha256_bytes(&stages_norito),
            json_sha256: sha256_bytes(&stages_json),
        },
        fixed_stage_count: u16::try_from(PRIVACY_RELEASE_STAGE_COUNT_V1)
            .expect("fixed release stage count fits u16"),
        all_native_stages_passed: true,
        contains_witnesses: false,
        contains_canonical_proof_artifacts: true,
        isolation_policy_enforced: true,
    };
    let receipt_norito = canonical_norito_bytes(&receipt, "receipt")?;
    let receipt_json = canonical_json_bytes(&receipt, "receipt")?;
    enforce_encoded_size(
        receipt_norito.len(),
        MAX_RECEIPT_NORITO_BYTES,
        "receipt Norito",
    )?;
    enforce_encoded_size(receipt_json.len(), MAX_RECEIPT_JSON_BYTES, "receipt JSON")?;

    let artifacts = [
        (
            outputs.command_manifest_norito.as_path(),
            command_norito.as_slice(),
        ),
        (
            outputs.command_manifest_json.as_path(),
            command_json.as_slice(),
        ),
        (
            outputs.stage_artifacts_norito.as_path(),
            stages_norito.as_slice(),
        ),
        (
            outputs.stage_artifacts_json.as_path(),
            stages_json.as_slice(),
        ),
        (outputs.receipt_norito.as_path(), receipt_norito.as_slice()),
        (outputs.receipt_json.as_path(), receipt_json.as_slice()),
    ];
    write_artifact_set_create_new(&artifacts)?;
    println!(
        "Taira native privacy evidence generated: {} exact stages",
        PRIVACY_RELEASE_STAGE_COUNT_V1
    );
    Ok(())
}

fn verify(common: CommonInputsV1, artifacts: VerifyArtifactsV1) -> Result<(), DynError> {
    ensure_taira_release_platform()?;
    let artifact_paths = verify_artifact_paths(&artifacts);
    let loaded = load_common_inputs(common, &artifact_paths)?;
    let (command_manifest, command_norito, command_json) =
        load_typed_pair::<PrivacyReleaseCommandManifestV1>(
            &artifacts.command_manifest_norito,
            MAX_COMMAND_MANIFEST_NORITO_BYTES,
            &artifacts.command_manifest_json,
            MAX_COMMAND_MANIFEST_JSON_BYTES,
            "command manifest",
        )?;
    let (stage_artifacts, stages_norito, stages_json) =
        load_typed_pair::<PrivacyReleaseStageArtifactsV1>(
            &artifacts.stage_artifacts_norito,
            MAX_STAGE_ARTIFACTS_NORITO_BYTES,
            &artifacts.stage_artifacts_json,
            MAX_STAGE_ARTIFACTS_JSON_BYTES,
            "stage artifacts",
        )?;
    let (receipt, _receipt_norito, _receipt_json) = load_typed_pair::<PrivacyReleaseReceiptV1>(
        &artifacts.receipt_norito,
        MAX_RECEIPT_NORITO_BYTES,
        &artifacts.receipt_json,
        MAX_RECEIPT_JSON_BYTES,
        "receipt",
    )?;

    let expected_command = build_command_manifest(&loaded);
    if command_manifest != expected_command {
        return Err(
            "command manifest does not bind the current inputs, binaries, source digest, profile, and canonical argument contract"
                .into(),
        );
    }
    validate_stage_artifacts(&stage_artifacts, &loaded.expectations)?;
    validate_receipt(
        &receipt,
        &loaded,
        &command_norito,
        &command_json,
        &stages_norito,
        &stages_json,
    )?;

    // Stored evidence is not sufficient: run all 48 production stages again
    // using this exact executable and compare every deterministic field.
    let rerun = run_all_stages(&loaded.expectations, &loaded.runner)?;
    validate_measured_against_expectations(&rerun, &loaded.expectations)?;
    for (index, (stored, current)) in stage_artifacts.stages.iter().zip(&rerun).enumerate() {
        if stored.evidence != current.evidence {
            return Err(format!(
                "stage {index} deterministic native evidence differs on verification rerun"
            )
            .into());
        }
    }
    println!(
        "Taira native privacy evidence verified: {} exact stages",
        PRIVACY_RELEASE_STAGE_COUNT_V1
    );
    Ok(())
}

fn capture_expectations(options: CaptureOptionsV1) -> Result<(), DynError> {
    ensure_taira_release_platform()?;
    expectation_pins::require_capture_open_v1()?;
    validate_process_ceilings(
        options.max_elapsed_millis,
        options.max_peak_rss_bytes,
        options.max_address_space_bytes,
    )?;
    let mut capture_outputs = vec![
        options.expectations_norito_out.clone(),
        options.expectations_json_out.clone(),
    ];
    capture_outputs.extend(options.x509_resource.output_paths());
    preflight_output_paths(&capture_outputs)?;
    let exact12 = secure_read(&options.exact12_path, MAX_EXACT12_BYTES, "exact12 matrix")?;
    validate_exact12_matrix(&exact12.bytes)?;
    let runner = prepare_immutable_runner()?;
    if exact12.identity == runner.source_identity {
        return Err("exact12 matrix aliases the release runner executable".into());
    }
    let x509_environment = resource_certificate::load_capture_environment_v1(
        &options.x509_resource,
        &exact12,
        &runner,
    )?;

    let provisional_stages = canonical_stage_coordinates()?
        .iter()
        .copied()
        .map(|coordinate| {
            let PrivacyReleaseStageCoordinateV1 {
                protocol_id,
                case_kind,
                ..
            } = coordinate;
            let ceilings = canonical_stage_process_ceilings_v1(
                protocol_id,
                options.max_elapsed_millis,
                options.max_peak_rss_bytes,
                options.max_address_space_bytes,
            )?;
            Ok(PrivacyReleaseExpectedStageV1 {
                evidence: empty_expected_evidence(protocol_id, case_kind),
                max_elapsed_millis: ceilings.elapsed_millis,
                max_peak_rss_bytes: ceilings.peak_rss_bytes,
                max_address_space_bytes: ceilings.address_space_bytes,
            })
        })
        .collect::<Result<Vec<_>, DynError>>()?;
    let provisional = PrivacyReleaseExpectationsV1 {
        schema_version: ARTIFACT_SCHEMA_VERSION_V1,
        stage_count: u16::try_from(PRIVACY_RELEASE_STAGE_COUNT_V1)
            .expect("fixed stage count fits u16"),
        stages: provisional_stages,
    };
    let measured = run_all_stages(&provisional, &runner)?;
    let x509_measurements = resource_certificate::capture_measurements_v1(&measured)?;
    let expectations = PrivacyReleaseExpectationsV1 {
        schema_version: ARTIFACT_SCHEMA_VERSION_V1,
        stage_count: u16::try_from(PRIVACY_RELEASE_STAGE_COUNT_V1)
            .expect("fixed stage count fits u16"),
        stages: measured
            .into_iter()
            .zip(provisional.stages)
            .map(|(stage, provisional)| PrivacyReleaseExpectedStageV1 {
                evidence: stage.evidence,
                max_elapsed_millis: provisional.max_elapsed_millis,
                max_peak_rss_bytes: provisional.max_peak_rss_bytes,
                max_address_space_bytes: provisional.max_address_space_bytes,
            })
            .collect(),
    };
    validate_expectations(&expectations)?;
    let norito = canonical_norito_bytes(&expectations, "captured expectations")?;
    let json = canonical_json_bytes(&expectations, "captured expectations")?;
    enforce_encoded_size(
        norito.len(),
        MAX_EXPECTATIONS_NORITO_BYTES,
        "expectations Norito",
    )?;
    enforce_encoded_size(json.len(), MAX_EXPECTATIONS_JSON_BYTES, "expectations JSON")?;
    let x509_resource = resource_certificate::build_capture_artifacts_v1(
        x509_measurements,
        &norito,
        &json,
        x509_environment,
    )?;
    write_artifact_set_create_new(&[
        (&options.expectations_norito_out, norito.as_slice()),
        (&options.expectations_json_out, json.as_slice()),
        (
            &options.x509_resource.norito_out,
            x509_resource.norito.as_slice(),
        ),
        (
            &options.x509_resource.json_out,
            x509_resource.json.as_slice(),
        ),
    ])?;
    println!(
        "captured {} native stages; review and freeze both expectation projections",
        PRIVACY_RELEASE_STAGE_COUNT_V1
    );
    Ok(())
}

fn validate_captured_fixtures(options: CapturedFixtureValidationOptionsV1) -> Result<(), DynError> {
    ensure_taira_release_platform()?;
    // This corridor is deliberately limited to the attested zero-pin runner.
    // Once any capture-owned source pin is populated, installed-fixture
    // validation must use the ordinary generate/verify modes instead.
    expectation_pins::require_capture_open_v1()?;
    let runner = prepare_immutable_runner()?;
    let all_paths = [
        options.exact12_path.clone(),
        options.expectations_norito_path.clone(),
        options.expectations_json_path.clone(),
        options.x509_resource_paths.norito.clone(),
        options.x509_resource_paths.json.clone(),
        runner.source_path.clone(),
    ];
    reject_lexical_path_aliases(&all_paths)?;
    reject_existing_inode_aliases(&all_paths)?;

    let exact12 = secure_read(&options.exact12_path, MAX_EXACT12_BYTES, "exact12 matrix")?;
    validate_exact12_matrix(&exact12.bytes)?;
    let (expectations, expectations_norito, expectations_json) =
        expectation_pins::load_capture_pair_v1(
            &options.expectations_norito_path,
            &options.expectations_json_path,
        )?;
    let x509_resource = resource_certificate::load_capture_pair_v1(
        &options.x509_resource_paths.norito,
        &options.x509_resource_paths.json,
    )?;

    let identities = [
        exact12.identity,
        expectations_norito.identity,
        expectations_json.identity,
        x509_resource.norito_identity,
        x509_resource.json_identity,
        runner.source_identity,
    ];
    if identities.iter().copied().collect::<BTreeSet<_>>().len() != identities.len() {
        return Err("capture validation inputs alias the same inode".into());
    }
    resource_certificate::validate_capture_expectation_binding_v1(
        &x509_resource.certificate,
        &expectations,
        expectations_norito.sha256,
        expectations_json.sha256,
    )?;

    // Structural self-consistency is insufficient for a first release: rerun
    // every production stage through the sealed child corridor and compare all
    // deterministic evidence with the candidate fixture before installation.
    let measured = run_all_stages(&expectations, &runner)?;
    validate_measured_against_expectations(&measured, &expectations)?;
    println!(
        "validated four canonical captured fixtures against the current exact12 matrix and {} native stages",
        PRIVACY_RELEASE_STAGE_COUNT_V1
    );
    Ok(())
}

fn load_common_inputs(
    common: CommonInputsV1,
    other_paths: &[PathBuf],
) -> Result<LoadedInputsV1, DynError> {
    let runner = prepare_immutable_runner()?;
    let all_paths = [
        vec![
            common.exact12_path.clone(),
            common.expectations_norito_path.clone(),
            common.expectations_json_path.clone(),
            common.x509_resource_paths.norito.clone(),
            common.x509_resource_paths.json.clone(),
            common.cargo_lock_path.clone(),
            common.validator_binary_path.clone(),
            runner.source_path.clone(),
        ],
        other_paths.to_vec(),
    ]
    .concat();
    reject_lexical_path_aliases(&all_paths)?;
    reject_existing_inode_aliases(&all_paths)?;

    let exact12 = secure_read(&common.exact12_path, MAX_EXACT12_BYTES, "exact12 matrix")?;
    validate_exact12_matrix(&exact12.bytes)?;
    let (expectations, expectations_norito, expectations_json) =
        expectation_pins::load_pinned_pair_v1(
            &common.expectations_norito_path,
            &common.expectations_json_path,
        )?;
    let x509_resource = resource_certificate::load_pinned_pair_v1(
        &common.x509_resource_paths.norito,
        &common.x509_resource_paths.json,
    )?;
    if x509_resource.certificate.expectations_norito_sha256 != expectations_norito.sha256
        || x509_resource.certificate.expectations_json_sha256 != expectations_json.sha256
    {
        return Err("X.509 resource certificate binds a different expectation pair".into());
    }

    let cargo_lock = secure_hash(&common.cargo_lock_path, MAX_CARGO_LOCK_BYTES, "Cargo.lock")?;
    let validator = secure_hash(
        &common.validator_binary_path,
        MAX_EXECUTABLE_BYTES,
        "validator binary",
    )?;
    if validator.mode & 0o111 == 0 {
        return Err("validator and runner inputs must both be executable regular files".into());
    }

    let identities = [
        exact12.identity,
        expectations_norito.identity,
        expectations_json.identity,
        x509_resource.norito_identity,
        x509_resource.json_identity,
        cargo_lock.identity,
        validator.identity,
        runner.source_identity,
    ];
    if identities.iter().copied().collect::<BTreeSet<_>>().len() != identities.len() {
        return Err("input paths alias the same inode; hard-link aliases are forbidden".into());
    }

    Ok(LoadedInputsV1 {
        common,
        exact12_sha256: exact12.sha256,
        expectations,
        expectations_norito_bytes: expectations_norito.bytes,
        expectations_json_bytes: expectations_json.bytes,
        x509_resource,
        cargo_lock_sha256: cargo_lock.sha256,
        validator_binary_sha256: validator.sha256,
        runner_binary_sha256: runner.sha256,
        runner,
    })
}

fn build_command_manifest(loaded: &LoadedInputsV1) -> PrivacyReleaseCommandManifestV1 {
    PrivacyReleaseCommandManifestV1 {
        schema_version: ARTIFACT_SCHEMA_VERSION_V1,
        build_profile: loaded.common.build_profile.clone(),
        source_sha256: loaded.common.source_sha256,
        exact12_matrix_sha256: loaded.exact12_sha256,
        expectations_norito_sha256: sha256_bytes(&loaded.expectations_norito_bytes),
        expectations_json_sha256: sha256_bytes(&loaded.expectations_json_bytes),
        x509_resource_norito_sha256: sha256_bytes(&loaded.x509_resource.norito_bytes),
        x509_resource_json_sha256: sha256_bytes(&loaded.x509_resource.json_bytes),
        cargo_lock_sha256: loaded.cargo_lock_sha256,
        validator_binary_sha256: loaded.validator_binary_sha256,
        runner_binary_sha256: loaded.runner_binary_sha256,
        isolation_policy: canonical_isolation_policy_v1(),
        command_arguments: vec![
            "generate".to_owned(),
            "--build-profile".to_owned(),
            loaded.common.build_profile.clone(),
            "--source-sha256".to_owned(),
            hex_sha256(&loaded.common.source_sha256),
            "--exact12-matrix".to_owned(),
            "<sha256-bound-relocatable-input>".to_owned(),
            "--expectations-norito".to_owned(),
            "<sha256-bound-relocatable-input>".to_owned(),
            "--expectations-json".to_owned(),
            "<sha256-bound-relocatable-input>".to_owned(),
            "--x509-resource-norito".to_owned(),
            "<sha256-bound-relocatable-input>".to_owned(),
            "--x509-resource-json".to_owned(),
            "<sha256-bound-relocatable-input>".to_owned(),
            "--cargo-lock".to_owned(),
            "<sha256-bound-relocatable-input>".to_owned(),
            "--validator-binary".to_owned(),
            "<sha256-bound-relocatable-input>".to_owned(),
            "--six-create-new-output-paths".to_owned(),
            "<relocatable-outputs>".to_owned(),
        ],
        stage_command_template: vec![
            "<sealed-anonymous-runner-fd-sha256-bound>".to_owned(),
            "__stage".to_owned(),
            "--protocol".to_owned(),
            "<PrivacyProtocolIdV1::ALL canonical label>".to_owned(),
            "--case".to_owned(),
            "<PrivacyReleaseCaseKindV1::ALL canonical label>".to_owned(),
            "--out-fd".to_owned(),
            "<single-inherited-anonymous-Norito-descriptor>".to_owned(),
            "--elapsed-ceiling-ms".to_owned(),
            "<frozen-stage-wall-clock-ceiling>".to_owned(),
            "--peak-rss-ceiling-bytes".to_owned(),
            "<frozen-stage-resident-memory-ceiling>".to_owned(),
            "--address-space-ceiling-bytes".to_owned(),
            "<frozen-stage-virtual-address-space-ceiling>".to_owned(),
            "<environment:RAYON_NUM_THREADS=4-and-no-other-variables>".to_owned(),
        ],
    }
}

fn canonical_isolation_policy_v1() -> PrivacyReleaseIsolationPolicyV1 {
    let stage_stack_bytes = u64::try_from(PRIVACY_RELEASE_STAGE_STACK_BYTES_V1)
        .expect("frozen 8 MiB release stack size fits u64");
    PrivacyReleaseIsolationPolicyV1 {
        stage_rayon_threads: STAGE_RAYON_THREAD_COUNT_V1,
        main_thread_stack_bytes: stage_stack_bytes,
        rayon_worker_stack_bytes: stage_stack_bytes,
        watchdog_thread_stack_bytes: stage_stack_bytes,
        max_stage_tasks: MAX_STAGE_TASKS_V1,
        max_stage_open_files: MAX_STAGE_OPEN_FILES_V1,
        max_stage_result_file_bytes: MAX_CHILD_RESULT_BYTES,
        max_stage_diagnostic_bytes: MAX_CHILD_DIAGNOSTIC_BYTES,
        core_dump_bytes: 0,
        static_elf_only: true,
        anonymous_sealed_runner: true,
        anonymous_result_descriptor_only: true,
        exact_environment_only: true,
        landlock_abi_minimum: MINIMUM_LANDLOCK_ABI_V1,
        seccomp_tsync: true,
    }
}

fn run_all_stages(
    expectations: &PrivacyReleaseExpectationsV1,
    runner: &ImmutableRunnerV1,
) -> Result<Vec<MeasuredStageV1>, DynError> {
    validate_expectation_stage_coordinates_v1(expectations)?;
    reset_parent_sigchld_disposition_v1()?;
    let mut measured = Vec::with_capacity(PRIVACY_RELEASE_STAGE_COUNT_V1);
    for expected in &expectations.stages {
        let stage = run_stage_child(
            runner,
            expected.evidence.protocol_id,
            expected.evidence.case_kind,
            expected.max_elapsed_millis,
            expected.max_peak_rss_bytes,
            expected.max_address_space_bytes,
        )?;
        measured.push(stage);
    }
    Ok(measured)
}

fn run_stage_child(
    runner: &ImmutableRunnerV1,
    protocol_id: PrivacyProtocolIdV1,
    case_kind: PrivacyReleaseCaseKindV1,
    elapsed_ceiling_millis: u64,
    peak_rss_ceiling_bytes: u64,
    address_space_ceiling_bytes: u64,
) -> Result<MeasuredStageV1, DynError> {
    validate_stage_process_ceilings_v1(
        protocol_id,
        elapsed_ceiling_millis,
        peak_rss_ceiling_bytes,
        address_space_ceiling_bytes,
    )?;
    let mut result_file = secure_anonymous_stage_file("child stage result")?;
    let mut stdout_file = secure_anonymous_stage_file("child stdout")?;
    let mut stderr_file = secure_anonymous_stage_file("child stderr")?;
    let result_child = result_file
        .try_clone()
        .map_err(|error| format!("cannot clone child-result descriptor: {error}"))?;
    let stdout_child = stdout_file
        .try_clone()
        .map_err(|error| format!("cannot clone child-stdout descriptor: {error}"))?;
    let stderr_child = stderr_file
        .try_clone()
        .map_err(|error| format!("cannot clone child-stderr descriptor: {error}"))?;
    let result_fd = result_child.as_raw_fd();

    // Keep a dedicated executable duplicate above the canonical result slot.
    // The child may then move its result descriptor to FD 3 without replacing
    // the `/proc/self/fd/N` executable anchor before `execve` resolves it.
    // SAFETY: F_DUPFD_CLOEXEC duplicates the live runner descriptor and
    // returns one independently owned descriptor on success.
    let runner_fd = unsafe {
        libc::fcntl(
            runner.executable.as_raw_fd(),
            libc::F_DUPFD_CLOEXEC,
            CANONICAL_STAGE_RESULT_FD_V1 + 1,
        )
    };
    if runner_fd < 0 {
        return Err(format!(
            "cannot duplicate the immutable runner above the stage-result slot: {}",
            std::io::Error::last_os_error()
        )
        .into());
    }
    // SAFETY: successful F_DUPFD_CLOEXEC returned one newly owned descriptor.
    let runner_exec_file = unsafe { File::from_raw_fd(runner_fd) };
    let runner_exec_path = immutable_runner_exec_path(runner_fd)?;
    let start = Instant::now();
    let mut command = Command::new(&runner_exec_path);
    command
        .arg("__stage")
        .arg("--protocol")
        .arg(protocol_id.canonical_label())
        .arg("--case")
        .arg(case_kind.canonical_label())
        .arg("--out-fd")
        .arg(CANONICAL_STAGE_RESULT_FD_V1.to_string())
        .arg("--elapsed-ceiling-ms")
        .arg(elapsed_ceiling_millis.to_string())
        .arg("--peak-rss-ceiling-bytes")
        .arg(peak_rss_ceiling_bytes.to_string())
        .arg("--address-space-ceiling-bytes")
        .arg(address_space_ceiling_bytes.to_string())
        .env_clear()
        .env("RAYON_NUM_THREADS", STAGE_RAYON_THREADS_V1)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout_child))
        .stderr(Stdio::from(stderr_child))
        .process_group(0);
    let parent_pid = current_process_id_v1()?;
    // SAFETY: the callback invokes only async-signal-safe Linux syscalls. All
    // values and the seccomp program are prepared without child-side allocation.
    unsafe {
        command.pre_exec(move || {
            install_pre_exec_stage_controls(
                parent_pid,
                result_fd,
                elapsed_ceiling_millis,
                address_space_ceiling_bytes,
            )
        });
    }
    let child = command
        .spawn()
        .map_err(|error| format!("failed to spawn isolated native stage: {error}"))?;
    drop(runner_exec_file);
    drop(result_child);
    let mut child = StageChildGuardV1::new(child)?;
    let pid_raw = child.pid_raw();
    let pid = Pid::from_raw(pid_raw);
    let mut sampled_peak_rss = 0_u64;
    let mut sampled_peak_address_space = 0_u64;
    let mut killed_for: Option<&'static str> = None;
    let waited: WaitedStageChildV1;
    loop {
        let sampled_memory = sample_process_memory_v1(pid_raw)?;
        sampled_peak_rss = sampled_peak_rss.max(sampled_memory.peak_rss_bytes);
        sampled_peak_address_space =
            sampled_peak_address_space.max(sampled_memory.peak_address_space_bytes);
        let elapsed_millis = elapsed_millis_ceil(start.elapsed())?;
        let diagnostic_bytes = anonymous_file_len(&stdout_file, "child stdout")?
            .checked_add(anonymous_file_len(&stderr_file, "child stderr")?)
            .ok_or("child diagnostic byte count overflowed")?;
        if diagnostic_bytes > MAX_CHILD_DIAGNOSTIC_BYTES && killed_for.is_none() {
            killed_for = Some("diagnostic-output ceiling");
            let _ = kill_stage_process_group(pid_raw, pid);
        }
        if sampled_peak_rss > peak_rss_ceiling_bytes && killed_for.is_none() {
            killed_for = Some("resident-memory ceiling");
            let _ = kill_stage_process_group(pid_raw, pid);
        }
        if sampled_peak_address_space > address_space_ceiling_bytes && killed_for.is_none() {
            killed_for = Some("address-space ceiling");
            let _ = kill_stage_process_group(pid_raw, pid);
        }
        if elapsed_millis > elapsed_ceiling_millis && killed_for.is_none() {
            killed_for = Some("elapsed-time ceiling");
            let _ = kill_stage_process_group(pid_raw, pid);
        }
        if let Some(observed) = child.try_wait4()? {
            waited = observed;
            break;
        }
        thread::sleep(CHILD_POLL_INTERVAL);
    }
    let elapsed_millis = elapsed_millis_ceil(start.elapsed())?;
    let peak_rss_bytes = sampled_peak_rss.max(waited.peak_rss_bytes);
    let peak_address_space_bytes = sampled_peak_address_space;

    if let Some(reason) = killed_for {
        return Err(format!(
            "stage {}/{} exceeded its {reason} (elapsed={elapsed_millis}ms, peak_rss={peak_rss_bytes} bytes, peak_address_space={peak_address_space_bytes} bytes)",
            protocol_id.canonical_label(),
            case_kind.canonical_label()
        )
        .into());
    }
    if elapsed_millis > elapsed_ceiling_millis {
        return Err(format!(
            "stage {}/{} exceeded elapsed ceiling after exit: {elapsed_millis} > {elapsed_ceiling_millis} ms",
            protocol_id.canonical_label(),
            case_kind.canonical_label()
        )
        .into());
    }
    if !waited.status.success() {
        let diagnostic = read_bounded_anonymous_file(
            &mut stderr_file,
            MAX_CHILD_DIAGNOSTIC_BYTES,
            "child stderr",
        )
        .unwrap_or_else(|_| b"<unavailable child diagnostic>".to_vec());
        let diagnostic = String::from_utf8_lossy(&diagnostic);
        return Err(format!(
            "isolated stage {}/{} exited {}: {}",
            protocol_id.canonical_label(),
            case_kind.canonical_label(),
            exit_status_description(waited.status),
            diagnostic.trim()
        )
        .into());
    }
    let result = read_bounded_anonymous_file(
        &mut result_file,
        MAX_CHILD_RESULT_BYTES,
        "child stage result",
    )?;
    let evidence: PrivacyReleaseStageEvidenceV1 =
        decode_canonical_norito(&result, MAX_CHILD_RESULT_BYTES, "child stage result")?;
    if peak_rss_bytes == 0 {
        return Err("exact-PID wait4 resident-memory accounting was unavailable".into());
    }
    if peak_rss_bytes > peak_rss_ceiling_bytes {
        return Err(format!(
            "stage {}/{} exceeded peak RSS ceiling: {peak_rss_bytes} > {peak_rss_ceiling_bytes} bytes",
            protocol_id.canonical_label(),
            case_kind.canonical_label()
        )
        .into());
    }
    if peak_address_space_bytes == 0 {
        return Err("exact-PID /proc peak-address-space accounting was unavailable".into());
    }
    if peak_address_space_bytes > address_space_ceiling_bytes {
        return Err(format!(
            "stage {}/{} exceeded peak address-space ceiling: {peak_address_space_bytes} > {address_space_ceiling_bytes} bytes",
            protocol_id.canonical_label(),
            case_kind.canonical_label()
        )
        .into());
    }
    let stdout =
        read_bounded_anonymous_file(&mut stdout_file, MAX_CHILD_DIAGNOSTIC_BYTES, "child stdout")?;
    if !stdout.is_empty() {
        return Err("hidden stage wrote unexpected stdout".into());
    }
    let stderr =
        read_bounded_anonymous_file(&mut stderr_file, MAX_CHILD_DIAGNOSTIC_BYTES, "child stderr")?;
    if !stderr.is_empty() {
        return Err("hidden stage wrote unexpected stderr on success".into());
    }
    if evidence.protocol_id != protocol_id
        || evidence.case_kind != case_kind
        || evidence.stage_ordinal != privacy_release_stage_ordinal_v1(protocol_id, case_kind)
    {
        return Err("isolated child returned evidence for a different stage".into());
    }
    Ok(MeasuredStageV1 {
        evidence,
        elapsed_millis,
        peak_rss_bytes,
        peak_address_space_bytes,
    })
}

fn hidden_stage_process_ceilings_v1(
    protocol_id: PrivacyProtocolIdV1,
    options: &BTreeMap<String, String>,
) -> Result<StageProcessCeilingsV1, DynError> {
    let ceilings = StageProcessCeilingsV1 {
        elapsed_millis: canonical_u64_option(options, "elapsed-ceiling-ms")?,
        peak_rss_bytes: canonical_u64_option(options, "peak-rss-ceiling-bytes")?,
        address_space_bytes: canonical_u64_option(options, "address-space-ceiling-bytes")?,
    };
    validate_stage_process_ceilings_v1(
        protocol_id,
        ceilings.elapsed_millis,
        ceilings.peak_rss_bytes,
        ceilings.address_space_bytes,
    )?;
    Ok(ceilings)
}

fn run_hidden_stage(options: &BTreeMap<String, String>) -> Result<(), DynError> {
    ensure_taira_release_platform()?;
    let protocol_label = option(options, "protocol")?;
    let protocol_id = PrivacyProtocolIdV1::from_canonical_label(protocol_label)
        .ok_or("hidden stage protocol label is not exact first-release canonical form")?;
    let case_label = option(options, "case")?;
    let case_kind = PrivacyReleaseCaseKindV1::from_canonical_label(case_label)
        .ok_or("hidden stage case label is not exact canonical form")?;
    let out_fd = canonical_stage_result_fd_option(options)?;
    let ceilings = hidden_stage_process_ceilings_v1(protocol_id, options)?;
    let environment = env::vars_os().collect::<Vec<_>>();
    validate_hidden_stage_environment_v1(&environment)?;
    install_hidden_stage_resource_limits(ceilings.elapsed_millis, ceilings.address_space_bytes)?;
    close_post_exec_descriptors_v1(out_fd)?;
    drop_stage_privileges_and_rearm_parent_death_v1()?;
    let task_directory = StageTaskDirectoryV1::open()?;
    let initial_task_count = task_directory.count()?;
    if initial_task_count != 1 {
        return Err(format!(
            "hidden stage must have exactly one task before Landlock, observed {initial_task_count}"
        )
        .into());
    }
    install_stage_landlock_v1()?;
    initialize_stage_rayon_pool_v1()?;
    let _parent_death_watchdog = StageParentDeathWatchdogV1::start()?;
    let proof_task_count = task_directory.count()?;
    if proof_task_count != MAX_STAGE_TASKS_V1 {
        return Err(format!(
            "hidden stage task topology is not exact: observed {proof_task_count}, expected {MAX_STAGE_TASKS_V1}"
        )
        .into());
    }
    task_directory.close()?;
    seal_hidden_stage_open_file_limit_v1(out_fd)?;
    install_post_exec_stage_controls_v1()?;
    let evidence = run_privacy_release_stage_v1(protocol_id, case_kind)?;
    if evidence.protocol_id != protocol_id
        || evidence.case_kind != case_kind
        || evidence.stage_ordinal != privacy_release_stage_ordinal_v1(protocol_id, case_kind)
    {
        return Err("core evidence API returned a mismatched stage coordinate".into());
    }
    let encoded = canonical_norito_bytes(&evidence, "child stage result")?;
    enforce_encoded_size(encoded.len(), MAX_CHILD_RESULT_BYTES, "child stage result")?;
    secure_write_anonymous_stage_result(out_fd, &encoded)?;
    Ok(())
}

fn validate_hidden_stage_environment_v1(
    environment: &[(OsString, OsString)],
) -> Result<(), DynError> {
    if environment
        != [(
            OsString::from("RAYON_NUM_THREADS"),
            OsString::from(STAGE_RAYON_THREADS_V1),
        )]
    {
        return Err("hidden stage environment is not the exact frozen Rayon policy".into());
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn close_post_exec_descriptors_v1(result_fd: RawFd) -> Result<(), DynError> {
    if result_fd != CANONICAL_STAGE_RESULT_FD_V1 {
        return Err("hidden-stage result descriptor is not canonical FD 3".into());
    }
    // SAFETY: this child owns its post-exec descriptor table. The range
    // deliberately preserves only stdin/stdout/stderr and canonical result
    // FD 3, and closes the CLOEXEC executable anchor plus every spawn helper.
    if unsafe {
        libc::syscall(
            libc::SYS_close_range,
            u32::try_from(CANONICAL_STAGE_RESULT_FD_V1 + 1)
                .map_err(|_| "canonical descriptor boundary exceeds u32")?,
            u32::MAX,
            0_u32,
        )
    } < 0
    {
        return Err(format!(
            "cannot close noncanonical post-exec descriptors: {}",
            std::io::Error::last_os_error()
        )
        .into());
    }

    let stdin = fstat_stage_descriptor_v1(libc::STDIN_FILENO, "stdin")?;
    let stdout = fstat_stage_descriptor_v1(libc::STDOUT_FILENO, "stdout")?;
    let stderr = fstat_stage_descriptor_v1(libc::STDERR_FILENO, "stderr")?;
    let result = fstat_stage_descriptor_v1(result_fd, "result")?;
    // Linux's canonical null device is character device 1:3. Requiring the
    // exact device prevents a replaced `/dev/null` from becoming hidden input.
    const LINUX_DEV_NULL_RDEV_V1: libc::dev_t = (1 << 8) | 3;
    if stdin.st_mode & libc::S_IFMT != libc::S_IFCHR
        || stdin.st_rdev != LINUX_DEV_NULL_RDEV_V1
        || stage_descriptor_access_mode_v1(libc::STDIN_FILENO, "stdin")? != libc::O_RDONLY
    {
        return Err("post-exec stdin is not the exact read-only null device".into());
    }
    for (fd, label, facts) in [
        (libc::STDOUT_FILENO, "stdout", &stdout),
        (libc::STDERR_FILENO, "stderr", &stderr),
        (result_fd, "result", &result),
    ] {
        if facts.st_mode & libc::S_IFMT != libc::S_IFREG
            || facts.st_mode & 0o777 != 0o600
            || facts.st_nlink != 0
            || facts.st_size != 0
            || facts.st_uid != unsafe { libc::geteuid() }
            || stage_descriptor_access_mode_v1(fd, label)? != libc::O_RDWR
        {
            return Err(format!(
                "post-exec {label} is not its exact empty anonymous read-write file"
            )
            .into());
        }
    }
    let regular_identities = [
        (stdout.st_dev, stdout.st_ino),
        (stderr.st_dev, stderr.st_ino),
        (result.st_dev, result.st_ino),
    ];
    if regular_identities[0] == regular_identities[1]
        || regular_identities[0] == regular_identities[2]
        || regular_identities[1] == regular_identities[2]
    {
        return Err("post-exec stdout, stderr, and result descriptors alias one inode".into());
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn fstat_stage_descriptor_v1(fd: RawFd, label: &str) -> Result<libc::stat, DynError> {
    let mut facts = MaybeUninit::<libc::stat>::uninit();
    // SAFETY: `facts` points to valid writable storage and the descriptor is
    // required to be live in this exact post-exec descriptor table.
    if unsafe { libc::fstat(fd, facts.as_mut_ptr()) } != 0 {
        return Err(format!(
            "cannot stat post-exec {label} descriptor: {}",
            std::io::Error::last_os_error()
        )
        .into());
    }
    // SAFETY: successful fstat initialized the complete structure.
    Ok(unsafe { facts.assume_init() })
}

#[cfg(target_os = "linux")]
fn stage_descriptor_access_mode_v1(fd: RawFd, label: &str) -> Result<libc::c_int, DynError> {
    // SAFETY: F_GETFL is a read-only query on a required live descriptor.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return Err(format!(
            "cannot query post-exec {label} descriptor flags: {}",
            std::io::Error::last_os_error()
        )
        .into());
    }
    Ok(flags & libc::O_ACCMODE)
}

#[cfg(not(target_os = "linux"))]
fn close_post_exec_descriptors_v1(_result_fd: RawFd) -> Result<(), DynError> {
    Err("Taira post-exec descriptor closure requires Linux".into())
}

#[cfg(target_os = "linux")]
#[repr(C)]
struct ReleaseCapabilityHeaderV1 {
    version: u32,
    pid: i32,
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy)]
#[repr(C)]
struct ReleaseCapabilityDataV1 {
    effective: u32,
    permitted: u32,
    inheritable: u32,
}

#[cfg(target_os = "linux")]
fn drop_stage_privileges_and_rearm_parent_death_v1() -> Result<(), DynError> {
    const LINUX_CAPABILITY_VERSION_3: u32 = 0x2008_0522;
    const UNPRIVILEGED_STAGE_ID: libc::uid_t = 65_534;
    // SAFETY: identity queries have no preconditions.
    let expected_parent = unsafe { libc::getppid() };
    if expected_parent <= 1 {
        return Err("hidden stage lost its release-runner parent before privilege drop".into());
    }

    // Root-running builder layers are converted to the kernel's conventional
    // unprivileged identity inside the stage only. A non-root caller keeps its
    // exact identity but must not bring supplementary groups into the proof.
    // SAFETY: geteuid has no preconditions.
    if unsafe { libc::geteuid() } == 0 {
        // SAFETY: a zero group count permits a null list.
        if unsafe { libc::setgroups(0, std::ptr::null()) } != 0
            || unsafe {
                libc::setresgid(
                    UNPRIVILEGED_STAGE_ID,
                    UNPRIVILEGED_STAGE_ID,
                    UNPRIVILEGED_STAGE_ID,
                )
            } != 0
            || unsafe {
                libc::setresuid(
                    UNPRIVILEGED_STAGE_ID,
                    UNPRIVILEGED_STAGE_ID,
                    UNPRIVILEGED_STAGE_ID,
                )
            } != 0
        {
            return Err(format!(
                "cannot drop root credentials for hidden stage: {}",
                std::io::Error::last_os_error()
            )
            .into());
        }
    }

    // SAFETY: getgroups with a zero count queries the required count.
    let supplementary_group_count = unsafe { libc::getgroups(0, std::ptr::null_mut()) };
    if supplementary_group_count != 0 {
        return Err("hidden stage retained supplementary groups".into());
    }
    let mut real_uid = 0;
    let mut effective_uid = 0;
    let mut saved_uid = 0;
    let mut real_gid = 0;
    let mut effective_gid = 0;
    let mut saved_gid = 0;
    // SAFETY: every pointer names valid writable identity storage.
    if unsafe { libc::getresuid(&mut real_uid, &mut effective_uid, &mut saved_uid) } != 0
        || unsafe { libc::getresgid(&mut real_gid, &mut effective_gid, &mut saved_gid) } != 0
        || real_uid == 0
        || effective_uid == 0
        || saved_uid == 0
        || real_uid != effective_uid
        || real_uid != saved_uid
        || real_gid == 0
        || effective_gid == 0
        || saved_gid == 0
        || real_gid != effective_gid
        || real_gid != saved_gid
    {
        return Err("hidden stage did not reach one exact non-root UID/GID identity".into());
    }

    let mut header = ReleaseCapabilityHeaderV1 {
        version: LINUX_CAPABILITY_VERSION_3,
        pid: 0,
    };
    let mut cleared = [ReleaseCapabilityDataV1 {
        effective: 0,
        permitted: 0,
        inheritable: 0,
    }; 2];
    // SAFETY: capset reads the exact version-3 header and two initialized words.
    let _ = unsafe {
        libc::syscall(
            libc::SYS_capset,
            &mut header as *mut ReleaseCapabilityHeaderV1,
            cleared.as_mut_ptr(),
        )
    };
    // SAFETY: PR_CAP_AMBIENT_CLEAR_ALL has no fourth/fifth arguments.
    if unsafe {
        libc::prctl(
            libc::PR_CAP_AMBIENT,
            libc::PR_CAP_AMBIENT_CLEAR_ALL,
            0,
            0,
            0,
        )
    } != 0
    {
        return Err(format!(
            "cannot clear hidden-stage ambient capabilities: {}",
            std::io::Error::last_os_error()
        )
        .into());
    }
    let mut observed = [ReleaseCapabilityDataV1 {
        effective: u32::MAX,
        permitted: u32::MAX,
        inheritable: u32::MAX,
    }; 2];
    // SAFETY: capget writes exactly the two version-3 capability words.
    if unsafe {
        libc::syscall(
            libc::SYS_capget,
            &mut header as *mut ReleaseCapabilityHeaderV1,
            observed.as_mut_ptr(),
        )
    } != 0
        || observed
            .iter()
            .any(|word| word.effective != 0 || word.permitted != 0 || word.inheritable != 0)
    {
        return Err("hidden stage retained process capabilities".into());
    }

    // Credential changes clear PDEATHSIG, so re-arm it after the final change
    // and close the race with the same exact-parent check used before exec.
    // SAFETY: PR_SET_PDEATHSIG accepts one signal number.
    if unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) } != 0
        || unsafe { libc::getppid() } != expected_parent
    {
        return Err("hidden stage lost its parent while credentials were dropped".into());
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn drop_stage_privileges_and_rearm_parent_death_v1() -> Result<(), DynError> {
    Err("Taira hidden-stage privilege dropping requires Linux".into())
}

#[cfg(target_os = "linux")]
struct StageTaskDirectoryV1 {
    file: File,
}

#[cfg(target_os = "linux")]
impl StageTaskDirectoryV1 {
    fn open() -> Result<Self, DynError> {
        // Open before entering Landlock so the same kernel-owned procfs
        // directory can attest the task topology after all restricted worker
        // threads have started. No pathname access is needed after this call.
        // SAFETY: the static procfs path is NUL-terminated and the flags
        // request one read-only directory descriptor.
        let fd = unsafe {
            libc::open(
                c"/proc/self/task".as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            )
        };
        if fd < 0 {
            return Err(format!(
                "cannot open the hidden-stage task directory: {}",
                std::io::Error::last_os_error()
            )
            .into());
        }
        // SAFETY: successful open returned one newly owned descriptor.
        let file = unsafe { File::from_raw_fd(fd) };
        if fd != STAGE_TASK_DIRECTORY_FD_V1 {
            return Err(format!(
                "hidden-stage task directory occupied FD {fd}, expected canonical FD {STAGE_TASK_DIRECTORY_FD_V1}"
            )
            .into());
        }
        let facts = file
            .metadata()
            .map_err(|error| format!("cannot stat hidden-stage task directory: {error}"))?;
        if !facts.file_type().is_dir() {
            return Err("hidden-stage task anchor is not a procfs directory".into());
        }
        let mut filesystem = MaybeUninit::<libc::statfs>::uninit();
        // SAFETY: `filesystem` points to complete writable statfs storage and
        // `fd` is the live task-directory anchor.
        if unsafe { libc::fstatfs(fd, filesystem.as_mut_ptr()) } != 0 {
            return Err(format!(
                "cannot identify hidden-stage task filesystem: {}",
                std::io::Error::last_os_error()
            )
            .into());
        }
        // SAFETY: successful fstatfs initialized the complete structure.
        let filesystem = unsafe { filesystem.assume_init() };
        const PROC_SUPER_MAGIC_V1: libc::c_long = 0x9fa0;
        if filesystem.f_type != PROC_SUPER_MAGIC_V1 {
            return Err("hidden-stage task anchor is not kernel procfs".into());
        }
        Ok(Self { file })
    }

    fn count(&self) -> Result<u64, DynError> {
        count_linux_task_directory_entries_v1(self.file.as_raw_fd())
    }

    fn close(self) -> Result<(), DynError> {
        let fd = self.file.into_raw_fd();
        // SAFETY: ownership of the exact task-directory descriptor was moved
        // out of File and is consumed by this close.
        if unsafe { libc::close(fd) } != 0 {
            return Err(format!(
                "cannot close hidden-stage task directory: {}",
                std::io::Error::last_os_error()
            )
            .into());
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn count_linux_task_directory_entries_v1(fd: RawFd) -> Result<u64, DynError> {
    const LINUX_DIRENT64_NAME_OFFSET: usize = 19;
    // SAFETY: seeking a procfs directory back to offset zero refreshes its
    // enumeration cursor without allocating another descriptor.
    if unsafe { libc::lseek(fd, 0, libc::SEEK_SET) } != 0 {
        return Err(format!(
            "cannot rewind hidden-stage task directory: {}",
            std::io::Error::last_os_error()
        )
        .into());
    }
    let mut count = 0_u64;
    let mut buffer = [0_u8; 4096];
    loop {
        // SAFETY: getdents64 writes at most `buffer.len()` initialized bytes
        // to this aligned byte buffer and does not retain its pointer.
        let read = unsafe {
            libc::syscall(
                libc::SYS_getdents64,
                fd,
                buffer.as_mut_ptr().cast::<libc::c_void>(),
                buffer.len(),
            )
        };
        if read < 0 {
            return Err(format!(
                "cannot enumerate hidden-stage task directory: {}",
                std::io::Error::last_os_error()
            )
            .into());
        }
        if read == 0 {
            break;
        }
        let read = usize::try_from(read).map_err(|_| "task-directory byte count exceeds usize")?;
        let mut offset = 0_usize;
        while offset < read {
            let header_end = offset
                .checked_add(LINUX_DIRENT64_NAME_OFFSET)
                .ok_or("task-directory record offset overflowed")?;
            if header_end > read {
                return Err("task-directory record has a truncated header".into());
            }
            let record_len = usize::from(u16::from_ne_bytes([
                buffer[offset + 16],
                buffer[offset + 17],
            ]));
            let record_end = offset
                .checked_add(record_len)
                .ok_or("task-directory record length overflowed")?;
            if record_len <= LINUX_DIRENT64_NAME_OFFSET
                || record_len % std::mem::align_of::<u64>() != 0
                || record_end > read
            {
                return Err("task-directory record has a noncanonical length".into());
            }
            let name_bytes = &buffer[header_end..record_end];
            let name_end = name_bytes
                .iter()
                .position(|byte| *byte == 0)
                .ok_or("task-directory record lacks a NUL-terminated name")?;
            let name = &name_bytes[..name_end];
            if name != b"." && name != b".." {
                if name.is_empty() || name[0] == b'0' || !name.iter().all(u8::is_ascii_digit) {
                    return Err("task-directory contains a noncanonical task identifier".into());
                }
                count = count
                    .checked_add(1)
                    .ok_or("hidden-stage task count overflowed")?;
            }
            offset = record_end;
        }
    }
    Ok(count)
}

#[cfg(not(target_os = "linux"))]
struct StageTaskDirectoryV1;

#[cfg(not(target_os = "linux"))]
impl StageTaskDirectoryV1 {
    fn open() -> Result<Self, DynError> {
        Err("Taira hidden-stage task attestation requires Linux procfs".into())
    }

    fn count(&self) -> Result<u64, DynError> {
        Err("Taira hidden-stage task attestation requires Linux procfs".into())
    }

    fn close(self) -> Result<(), DynError> {
        Err("Taira hidden-stage task attestation requires Linux procfs".into())
    }
}

#[cfg(target_os = "linux")]
#[repr(C)]
struct ReleaseLandlockRulesetAttrV1 {
    handled_access_fs: u64,
}

#[cfg(target_os = "linux")]
fn install_stage_landlock_v1() -> Result<(), DynError> {
    const LANDLOCK_CREATE_RULESET_VERSION: u32 = 1;
    const LANDLOCK_MINIMUM_ABI: libc::c_long = MINIMUM_LANDLOCK_ABI_V1 as libc::c_long;
    const LANDLOCK_ACCESS_FS_EXECUTE: u64 = 1 << 0;
    const LANDLOCK_ACCESS_FS_WRITE_FILE: u64 = 1 << 1;
    const LANDLOCK_ACCESS_FS_READ_FILE: u64 = 1 << 2;
    const LANDLOCK_ACCESS_FS_READ_DIR: u64 = 1 << 3;
    const LANDLOCK_ACCESS_FS_REMOVE_DIR: u64 = 1 << 4;
    const LANDLOCK_ACCESS_FS_REMOVE_FILE: u64 = 1 << 5;
    const LANDLOCK_ACCESS_FS_MAKE_CHAR: u64 = 1 << 6;
    const LANDLOCK_ACCESS_FS_MAKE_DIR: u64 = 1 << 7;
    const LANDLOCK_ACCESS_FS_MAKE_REG: u64 = 1 << 8;
    const LANDLOCK_ACCESS_FS_MAKE_SOCK: u64 = 1 << 9;
    const LANDLOCK_ACCESS_FS_MAKE_FIFO: u64 = 1 << 10;
    const LANDLOCK_ACCESS_FS_MAKE_BLOCK: u64 = 1 << 11;
    const LANDLOCK_ACCESS_FS_MAKE_SYM: u64 = 1 << 12;
    const LANDLOCK_ACCESS_FS_REFER: u64 = 1 << 13;
    const LANDLOCK_ACCESS_FS_TRUNCATE: u64 = 1 << 14;
    const HANDLED_ACCESS: u64 = LANDLOCK_ACCESS_FS_EXECUTE
        | LANDLOCK_ACCESS_FS_WRITE_FILE
        | LANDLOCK_ACCESS_FS_READ_FILE
        | LANDLOCK_ACCESS_FS_READ_DIR
        | LANDLOCK_ACCESS_FS_REMOVE_DIR
        | LANDLOCK_ACCESS_FS_REMOVE_FILE
        | LANDLOCK_ACCESS_FS_MAKE_CHAR
        | LANDLOCK_ACCESS_FS_MAKE_DIR
        | LANDLOCK_ACCESS_FS_MAKE_REG
        | LANDLOCK_ACCESS_FS_MAKE_SOCK
        | LANDLOCK_ACCESS_FS_MAKE_FIFO
        | LANDLOCK_ACCESS_FS_MAKE_BLOCK
        | LANDLOCK_ACCESS_FS_MAKE_SYM
        | LANDLOCK_ACCESS_FS_REFER
        | LANDLOCK_ACCESS_FS_TRUNCATE;

    // SAFETY: the VERSION query requires null attributes and a zero size.
    let abi = unsafe {
        libc::syscall(
            libc::SYS_landlock_create_ruleset,
            std::ptr::null::<ReleaseLandlockRulesetAttrV1>(),
            0_usize,
            LANDLOCK_CREATE_RULESET_VERSION,
        )
    };
    if abi < LANDLOCK_MINIMUM_ABI {
        return Err(format!(
            "Taira stage requires Landlock ABI >= {LANDLOCK_MINIMUM_ABI}, observed {abi}"
        )
        .into());
    }
    let attributes = ReleaseLandlockRulesetAttrV1 {
        handled_access_fs: HANDLED_ACCESS,
    };
    // A ruleset with no path-beneath rules denies every handled filesystem
    // operation. Existing stdin/stdout/stderr/result descriptors remain
    // usable without filesystem authority. The temporary ruleset descriptor
    // is closed before the runtime descriptor ceiling is sealed.
    // SAFETY: the pointer and size exactly match the version-1 ruleset struct.
    let ruleset_fd = unsafe {
        libc::syscall(
            libc::SYS_landlock_create_ruleset,
            &attributes as *const ReleaseLandlockRulesetAttrV1,
            std::mem::size_of::<ReleaseLandlockRulesetAttrV1>(),
            0_u32,
        )
    };
    if ruleset_fd < 0 {
        return Err(format!(
            "cannot create deny-all stage Landlock ruleset: {}",
            std::io::Error::last_os_error()
        )
        .into());
    }
    let ruleset_fd = match i32::try_from(ruleset_fd) {
        Ok(fd) => fd,
        Err(_) => {
            return Err("Landlock FD exceeds i32".into());
        }
    };
    if ruleset_fd != STAGE_LANDLOCK_RULESET_FD_V1 {
        // SAFETY: a successful ruleset creation returned one uniquely owned
        // descriptor even though its position violated the canonical table.
        let _ = unsafe { libc::close(ruleset_fd) };
        return Err(format!(
            "hidden-stage Landlock ruleset occupied FD {ruleset_fd}, expected canonical FD {STAGE_LANDLOCK_RULESET_FD_V1}"
        )
        .into());
    }
    // SAFETY: the owned ruleset FD and zero flags follow landlock_restrict_self.
    let restricted = unsafe { libc::syscall(libc::SYS_landlock_restrict_self, ruleset_fd, 0_u32) };
    let restrict_error = (restricted != 0).then(std::io::Error::last_os_error);
    // SAFETY: the ruleset FD is uniquely owned by this function.
    let close_result = unsafe { libc::close(ruleset_fd) };
    if restricted != 0 {
        return Err(format!(
            "cannot enter deny-all stage Landlock domain: {}",
            restrict_error.expect("failed restriction captured errno")
        )
        .into());
    }
    if close_result != 0 {
        return Err(format!(
            "cannot close stage Landlock ruleset: {}",
            std::io::Error::last_os_error()
        )
        .into());
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn install_stage_landlock_v1() -> Result<(), DynError> {
    Err("Taira hidden-stage filesystem confinement requires Linux Landlock".into())
}

#[cfg(target_os = "linux")]
fn initialize_stage_rayon_pool_v1() -> Result<(), DynError> {
    initialize_privacy_release_rayon_pool_v1()
        .map_err(|error| format!("cannot initialize exact hidden-stage Rayon pool: {error}").into())
}

#[cfg(not(target_os = "linux"))]
fn initialize_stage_rayon_pool_v1() -> Result<(), DynError> {
    Err("Taira hidden-stage Rayon initialization requires Linux".into())
}

#[cfg(target_os = "linux")]
fn seal_hidden_stage_open_file_limit_v1(result_fd: RawFd) -> Result<(), DynError> {
    if result_fd != CANONICAL_STAGE_RESULT_FD_V1 {
        return Err("hidden-stage result descriptor is not canonical FD 3".into());
    }
    // The retained task-directory FD 4 and transient Landlock FD 5 must both
    // be closed before the proof phase begins.
    for fd in [STAGE_TASK_DIRECTORY_FD_V1, STAGE_LANDLOCK_RULESET_FD_V1] {
        // SAFETY: F_GETFD is a read-only descriptor query.
        if unsafe { libc::fcntl(fd, libc::F_GETFD) } >= 0
            || std::io::Error::last_os_error().raw_os_error() != Some(libc::EBADF)
        {
            return Err(format!(
                "hidden-stage setup leaked descriptor FD {fd} into the proof phase"
            )
            .into());
        }
    }
    let runtime_limit: rlim_t = MAX_STAGE_OPEN_FILES_V1
        .try_into()
        .map_err(|_| "stage runtime open-file ceiling exceeds rlim_t")?;
    setrlimit(Resource::RLIMIT_NOFILE, runtime_limit, runtime_limit)
        .map_err(|error| format!("cannot seal hidden-stage runtime open-file limit: {error}"))?;
    let mut observed = MaybeUninit::<libc::rlimit>::uninit();
    // SAFETY: successful getrlimit initializes the complete output structure.
    if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, observed.as_mut_ptr()) } != 0 {
        return Err(format!(
            "cannot verify hidden-stage runtime open-file limit: {}",
            std::io::Error::last_os_error()
        )
        .into());
    }
    // SAFETY: successful getrlimit initialized the structure.
    let observed = unsafe { observed.assume_init() };
    if observed.rlim_cur != runtime_limit || observed.rlim_max != runtime_limit {
        return Err("hidden-stage runtime open-file limit did not seal exactly".into());
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn seal_hidden_stage_open_file_limit_v1(_result_fd: RawFd) -> Result<(), DynError> {
    Err("Taira hidden-stage open-file sealing requires Linux".into())
}

#[cfg(target_os = "linux")]
struct StageParentDeathWatchdogV1 {
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

#[cfg(target_os = "linux")]
impl StageParentDeathWatchdogV1 {
    fn start() -> Result<Self, DynError> {
        // SAFETY: getppid has no preconditions.
        let expected_parent = unsafe { libc::getppid() };
        if expected_parent <= 1 {
            return Err("hidden stage has no live release-runner parent".into());
        }
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let (ready_sender, ready_receiver) = mpsc::sync_channel(0);
        let thread = thread::Builder::new()
            .stack_size(PRIVACY_RELEASE_STAGE_STACK_BYTES_V1)
            .spawn(move || {
                // Every Linux thread starts with its own cleared parent-death
                // signal. This dedicated thread installs SIGKILL and remains
                // alive for the whole proof so raw leader-thread exit cannot
                // strand Rayon or other compute workers after parent death.
                // SAFETY: PR_SET_PDEATHSIG accepts one signal number.
                let installed = unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) } == 0;
                // SAFETY: getppid has no preconditions.
                let parent_still_matches =
                    installed && unsafe { libc::getppid() } == expected_parent;
                let _ = ready_sender.send(parent_still_matches);
                if !parent_still_matches {
                    // SAFETY: SIGKILL directed at the current process kills the
                    // complete thread group and cannot run a user handler.
                    let _ = unsafe { libc::kill(libc::getpid(), libc::SIGKILL) };
                    return;
                }
                while !thread_stop.load(Ordering::Acquire) {
                    thread::sleep(CHILD_POLL_INTERVAL);
                }
                // Close the last race between the stop observation and normal
                // child exit. The leader retains its own PDEATHSIG throughout.
                // SAFETY: getppid has no preconditions.
                if unsafe { libc::getppid() } != expected_parent {
                    // SAFETY: see the group-fatal signal rationale above.
                    let _ = unsafe { libc::kill(libc::getpid(), libc::SIGKILL) };
                }
            })
            .map_err(|error| format!("cannot start hidden-stage parent-death watchdog: {error}"))?;
        match ready_receiver.recv() {
            Ok(true) => Ok(Self {
                stop,
                thread: Some(thread),
            }),
            Ok(false) | Err(_) => {
                stop.store(true, Ordering::Release);
                let _ = thread.join();
                Err("hidden-stage parent-death watchdog did not arm".into())
            }
        }
    }
}

#[cfg(target_os = "linux")]
impl Drop for StageParentDeathWatchdogV1 {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(not(target_os = "linux"))]
struct StageParentDeathWatchdogV1;

#[cfg(not(target_os = "linux"))]
impl StageParentDeathWatchdogV1 {
    fn start() -> Result<Self, DynError> {
        Err("Taira parent-death thread-group containment requires Linux".into())
    }
}

#[cfg(target_os = "linux")]
fn reset_parent_sigchld_disposition_v1() -> Result<(), DynError> {
    // A launcher can otherwise make children auto-reap by inheriting SIG_IGN or
    // SA_NOCLDWAIT. The release runner is a dedicated process, so it owns this
    // disposition for the complete 48-stage sequence.
    // SAFETY: zero is the required initial representation before filling every
    // relevant `sigaction` field below.
    let mut action = unsafe { std::mem::zeroed::<libc::sigaction>() };
    action.sa_sigaction = libc::SIG_DFL;
    action.sa_flags = 0;
    // SAFETY: `action.sa_mask` is valid writable signal-set storage.
    if unsafe { libc::sigemptyset(&mut action.sa_mask) } != 0 {
        return Err(format!(
            "cannot clear the SIGCHLD action mask: {}",
            std::io::Error::last_os_error()
        )
        .into());
    }
    // SAFETY: the initialized action has process lifetime for this call.
    if unsafe { libc::sigaction(libc::SIGCHLD, &action, std::ptr::null_mut()) } != 0 {
        return Err(format!(
            "cannot normalize the parent SIGCHLD disposition: {}",
            std::io::Error::last_os_error()
        )
        .into());
    }
    // SAFETY: zeroed storage is fully initialized by a successful query.
    let mut observed = unsafe { std::mem::zeroed::<libc::sigaction>() };
    // SAFETY: a null new-action pointer makes this a read-only query.
    if unsafe { libc::sigaction(libc::SIGCHLD, std::ptr::null(), &mut observed) } != 0 {
        return Err(format!(
            "cannot verify the parent SIGCHLD disposition: {}",
            std::io::Error::last_os_error()
        )
        .into());
    }
    if observed.sa_sigaction != libc::SIG_DFL || observed.sa_flags & libc::SA_NOCLDWAIT != 0 {
        return Err("parent SIGCHLD disposition remained auto-reaping".into());
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn reset_parent_sigchld_disposition_v1() -> Result<(), DynError> {
    Err("Taira stage child lifecycle normalization requires Linux".into())
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const RELEASE_AUDIT_ARCH_V1: u32 = 0xc000_003e;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const RELEASE_FORBIDDEN_SYSCALL_ABI_MASK_V1: u32 = 0x4000_0000;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const RELEASE_AUDIT_ARCH_V1: u32 = 0xc000_00b7;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const RELEASE_FORBIDDEN_SYSCALL_ABI_MASK_V1: u32 = 0;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const RELEASE_NR_FORK_V1: u32 = 57;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const RELEASE_NR_VFORK_V1: u32 = 58;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const RELEASE_NR_CLONE_V1: u32 = 56;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const RELEASE_NR_SETSID_V1: u32 = 112;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const RELEASE_NR_SETPGID_V1: u32 = 109;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const RELEASE_NR_UNSHARE_V1: u32 = 272;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const RELEASE_NR_SETNS_V1: u32 = 308;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const RELEASE_NR_CLONE3_V1: u32 = 435;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const RELEASE_NR_EXECVE_V1: u32 = 59;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const RELEASE_NR_EXECVEAT_V1: u32 = 322;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const RELEASE_NR_PRCTL_V1: u32 = 157;

// AArch64 exposes process creation through clone/clone3; fork and vfork have
// no syscall numbers there, so impossible sentinel values retain one fixed BPF
// layout across both supported Taira release architectures.
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const RELEASE_NR_FORK_V1: u32 = u32::MAX;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const RELEASE_NR_VFORK_V1: u32 = u32::MAX - 1;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const RELEASE_NR_CLONE_V1: u32 = 220;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const RELEASE_NR_SETSID_V1: u32 = 157;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const RELEASE_NR_SETPGID_V1: u32 = 154;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const RELEASE_NR_UNSHARE_V1: u32 = 97;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const RELEASE_NR_SETNS_V1: u32 = 268;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const RELEASE_NR_CLONE3_V1: u32 = 435;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const RELEASE_NR_EXECVE_V1: u32 = 221;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const RELEASE_NR_EXECVEAT_V1: u32 = 281;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const RELEASE_NR_PRCTL_V1: u32 = 167;

#[cfg(target_os = "linux")]
const RELEASE_SECCOMP_RET_KILL_PROCESS_V1: u32 = 0x8000_0000;
#[cfg(target_os = "linux")]
const RELEASE_SECCOMP_RET_ALLOW_V1: u32 = 0x7fff_0000;
#[cfg(target_os = "linux")]
const RELEASE_SECCOMP_RET_ERRNO_V1: u32 = 0x0005_0000;

#[cfg(target_os = "linux")]
const fn release_bpf_statement_v1(code: u16, k: u32) -> libc::sock_filter {
    libc::sock_filter {
        code,
        jt: 0,
        jf: 0,
        k,
    }
}

#[cfg(target_os = "linux")]
const fn release_bpf_jump_v1(code: u16, k: u32, jt: u8, jf: u8) -> libc::sock_filter {
    libc::sock_filter { code, jt, jf, k }
}

#[cfg(all(
    target_os = "linux",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn install_pre_exec_stage_controls(
    expected_parent_pid: i32,
    result_fd: RawFd,
    elapsed_ceiling_millis: u64,
    address_space_ceiling_bytes: u64,
) -> std::io::Result<()> {
    if expected_parent_pid <= 1 || result_fd < 3 {
        return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
    }
    install_pre_exec_stage_stack_limit_v1()?;

    let allocation_limit: libc::rlim_t = address_space_ceiling_bytes
        .try_into()
        .map_err(|_| std::io::Error::from_raw_os_error(libc::EOVERFLOW))?;
    let cpu_seconds = checked_stage_cpu_limit_seconds_v1(elapsed_ceiling_millis)
        .ok_or_else(|| std::io::Error::from_raw_os_error(libc::EOVERFLOW))?;
    let cpu_limit: libc::rlim_t = cpu_seconds
        .try_into()
        .map_err(|_| std::io::Error::from_raw_os_error(libc::EOVERFLOW))?;
    let file_size_limit: libc::rlim_t = MAX_CHILD_RESULT_BYTES
        .try_into()
        .map_err(|_| std::io::Error::from_raw_os_error(libc::EOVERFLOW))?;
    let core = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    let file_size = libc::rlimit {
        rlim_cur: file_size_limit,
        rlim_max: file_size_limit,
    };
    let address_space = libc::rlimit {
        rlim_cur: allocation_limit,
        rlim_max: allocation_limit,
    };
    let cpu = libc::rlimit {
        rlim_cur: cpu_limit,
        rlim_max: cpu_limit,
    };
    let open_file_limit: libc::rlim_t = MAX_STAGE_SETUP_OPEN_FILES_V1
        .try_into()
        .map_err(|_| std::io::Error::from_raw_os_error(libc::EOVERFLOW))?;
    let open_files = libc::rlimit {
        rlim_cur: open_file_limit,
        rlim_max: open_file_limit,
    };
    // SAFETY: each pointer addresses a fully initialized rlimit and these
    // syscalls affect only the about-to-exec child.
    if unsafe { libc::setrlimit(libc::RLIMIT_CORE, &core) } != 0
        || unsafe { libc::setrlimit(libc::RLIMIT_FSIZE, &file_size) } != 0
        || unsafe { libc::setrlimit(libc::RLIMIT_AS, &address_space) } != 0
        || unsafe { libc::setrlimit(libc::RLIMIT_CPU, &cpu) } != 0
        || unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &open_files) } != 0
    {
        return Err(std::io::Error::last_os_error());
    }

    // Mark every non-stdio descriptor close-on-exec, then move the
    // parent-created anonymous result descriptor to canonical FD 3. The
    // dedicated executable memfd remains above FD 3 and CLOEXEC:
    // `/proc/self/fd/N` is resolved before the successful exec closes it, so
    // the hidden image cannot inherit or reuse the runner descriptor.
    // SAFETY: close_range with CLOEXEC mutates descriptor flags only.
    if unsafe {
        libc::syscall(
            libc::SYS_close_range,
            3_u32,
            u32::MAX,
            4_u32, // CLOSE_RANGE_CLOEXEC
        )
    } < 0
    {
        return Err(std::io::Error::last_os_error());
    }
    if result_fd != CANONICAL_STAGE_RESULT_FD_V1 {
        // SAFETY: `result_fd` is the live clone dedicated to this child and
        // dup3 atomically replaces only the canonical result slot.
        if unsafe { libc::dup3(result_fd, CANONICAL_STAGE_RESULT_FD_V1, 0) }
            != CANONICAL_STAGE_RESULT_FD_V1
        {
            return Err(std::io::Error::last_os_error());
        }
    } else {
        // SAFETY: the canonical result descriptor is live and uniquely
        // inherited by this about-to-exec child.
        let descriptor_flags = unsafe { libc::fcntl(CANONICAL_STAGE_RESULT_FD_V1, libc::F_GETFD) };
        if descriptor_flags < 0
            || unsafe {
                libc::fcntl(
                    CANONICAL_STAGE_RESULT_FD_V1,
                    libc::F_SETFD,
                    descriptor_flags & !libc::FD_CLOEXEC,
                )
            } < 0
        {
            return Err(std::io::Error::last_os_error());
        }
    }
    let mut result_stat = MaybeUninit::<libc::stat>::uninit();
    // SAFETY: the output points to valid storage and canonical FD 3 is live.
    if unsafe { libc::fstat(CANONICAL_STAGE_RESULT_FD_V1, result_stat.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: successful fstat initialized the complete structure.
    let result_stat = unsafe { result_stat.assume_init() };
    if result_stat.st_mode & libc::S_IFMT != libc::S_IFREG
        || result_stat.st_mode & 0o777 != 0o600
        || result_stat.st_nlink != 0
        || result_stat.st_size != 0
        || result_stat.st_uid != unsafe { libc::geteuid() }
    {
        return Err(std::io::Error::from_raw_os_error(libc::EPERM));
    }

    // Install the parent-death signal before checking the parent identity; the
    // follow-up getppid closes the documented prctl race.
    // SAFETY: PR_SET_PDEATHSIG accepts one signal number.
    if unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: getppid has no preconditions.
    if unsafe { libc::getppid() } != expected_parent_pid {
        return Err(std::io::Error::from_raw_os_error(libc::ECHILD));
    }
    install_pre_exec_seccomp_v1()
}

#[cfg(all(
    target_os = "linux",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn install_pre_exec_seccomp_v1() -> std::io::Result<()> {
    const BPF_LD_W_ABS: u16 = 0x20;
    const BPF_JMP_JEQ_K: u16 = 0x15;
    const BPF_JMP_JSET_K: u16 = 0x45;
    const BPF_RET_K: u16 = 0x06;
    const BPF_ALU_AND_K: u16 = 0x54;
    const SECCOMP_DATA_NR_OFFSET: u32 = 0;
    const SECCOMP_DATA_ARCH_OFFSET: u32 = 4;
    const SECCOMP_DATA_ARG0_LOW_OFFSET: u32 = 16;
    let deny = RELEASE_SECCOMP_RET_ERRNO_V1 | u32::try_from(libc::EPERM).unwrap_or(1);
    let unsupported = RELEASE_SECCOMP_RET_ERRNO_V1 | u32::try_from(libc::ENOSYS).unwrap_or(38);
    let mut filter = [
        release_bpf_statement_v1(BPF_LD_W_ABS, SECCOMP_DATA_ARCH_OFFSET),
        release_bpf_jump_v1(BPF_JMP_JEQ_K, RELEASE_AUDIT_ARCH_V1, 1, 0),
        release_bpf_statement_v1(BPF_RET_K, RELEASE_SECCOMP_RET_KILL_PROCESS_V1),
        release_bpf_statement_v1(BPF_LD_W_ABS, SECCOMP_DATA_NR_OFFSET),
        release_bpf_jump_v1(BPF_JMP_JSET_K, RELEASE_FORBIDDEN_SYSCALL_ABI_MASK_V1, 0, 1),
        release_bpf_statement_v1(BPF_RET_K, deny),
        release_bpf_jump_v1(BPF_JMP_JEQ_K, RELEASE_NR_FORK_V1, 0, 1),
        release_bpf_statement_v1(BPF_RET_K, deny),
        release_bpf_jump_v1(BPF_JMP_JEQ_K, RELEASE_NR_VFORK_V1, 0, 1),
        release_bpf_statement_v1(BPF_RET_K, deny),
        release_bpf_jump_v1(BPF_JMP_JEQ_K, RELEASE_NR_SETSID_V1, 0, 1),
        release_bpf_statement_v1(BPF_RET_K, deny),
        release_bpf_jump_v1(BPF_JMP_JEQ_K, RELEASE_NR_SETPGID_V1, 0, 1),
        release_bpf_statement_v1(BPF_RET_K, deny),
        release_bpf_jump_v1(BPF_JMP_JEQ_K, RELEASE_NR_UNSHARE_V1, 0, 1),
        release_bpf_statement_v1(BPF_RET_K, deny),
        release_bpf_jump_v1(BPF_JMP_JEQ_K, RELEASE_NR_SETNS_V1, 0, 1),
        release_bpf_statement_v1(BPF_RET_K, deny),
        release_bpf_jump_v1(BPF_JMP_JEQ_K, RELEASE_NR_CLONE3_V1, 0, 1),
        release_bpf_statement_v1(BPF_RET_K, unsupported),
        release_bpf_jump_v1(BPF_JMP_JEQ_K, RELEASE_NR_CLONE_V1, 0, 4),
        release_bpf_statement_v1(BPF_LD_W_ABS, SECCOMP_DATA_ARG0_LOW_OFFSET),
        release_bpf_statement_v1(BPF_ALU_AND_K, libc::CLONE_THREAD as u32),
        release_bpf_jump_v1(BPF_JMP_JEQ_K, libc::CLONE_THREAD as u32, 1, 0),
        release_bpf_statement_v1(BPF_RET_K, deny),
        release_bpf_statement_v1(BPF_RET_K, RELEASE_SECCOMP_RET_ALLOW_V1),
    ];
    install_seccomp_filter_v1(&mut filter, 0)
}

#[cfg(all(
    target_os = "linux",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
const RELEASE_POST_EXEC_UNCONDITIONAL_SYSCALLS_V1: &[libc::c_long] = &[
    libc::SYS_read,
    libc::SYS_write,
    libc::SYS_readv,
    libc::SYS_writev,
    libc::SYS_pread64,
    libc::SYS_pwrite64,
    libc::SYS_close,
    libc::SYS_lseek,
    libc::SYS_fstat,
    libc::SYS_fchmod,
    libc::SYS_fsync,
    libc::SYS_fdatasync,
    libc::SYS_ftruncate,
    libc::SYS_brk,
    libc::SYS_munmap,
    libc::SYS_mremap,
    libc::SYS_madvise,
    libc::SYS_msync,
    libc::SYS_mlock,
    libc::SYS_munlock,
    libc::SYS_rt_sigaction,
    libc::SYS_rt_sigprocmask,
    libc::SYS_rt_sigreturn,
    libc::SYS_sigaltstack,
    libc::SYS_futex,
    libc::SYS_set_robust_list,
    libc::SYS_set_tid_address,
    libc::SYS_rseq,
    libc::SYS_membarrier,
    libc::SYS_exit,
    libc::SYS_exit_group,
    libc::SYS_clock_gettime,
    libc::SYS_clock_getres,
    libc::SYS_clock_nanosleep,
    libc::SYS_nanosleep,
    libc::SYS_gettimeofday,
    libc::SYS_sched_yield,
    libc::SYS_sched_getaffinity,
    libc::SYS_getcpu,
    libc::SYS_getrandom,
    libc::SYS_getpid,
    libc::SYS_getppid,
    libc::SYS_gettid,
    libc::SYS_getpgid,
    libc::SYS_getuid,
    libc::SYS_geteuid,
    libc::SYS_getgid,
    libc::SYS_getegid,
    libc::SYS_getresuid,
    libc::SYS_getresgid,
    libc::SYS_getgroups,
    libc::SYS_getrusage,
    libc::SYS_uname,
    libc::SYS_sysinfo,
    libc::SYS_restart_syscall,
];

#[cfg(all(
    target_os = "linux",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn install_post_exec_stage_controls_v1() -> Result<(), DynError> {
    const BPF_LD_W_ABS: u16 = 0x20;
    const BPF_JMP_JEQ_K: u16 = 0x15;
    const BPF_JMP_JSET_K: u16 = 0x45;
    const BPF_RET_K: u16 = 0x06;
    const SECCOMP_DATA_NR_OFFSET: u32 = 0;
    const SECCOMP_DATA_ARCH_OFFSET: u32 = 4;
    const SECCOMP_DATA_ARG0_LOW_OFFSET: u32 = 16;
    const SECCOMP_DATA_ARG2_LOW_OFFSET: u32 = 32;

    let mut parent_death_signal = 0;
    // SAFETY: PR_GET_PDEATHSIG writes one integer to the supplied pointer.
    if unsafe { libc::prctl(libc::PR_GET_PDEATHSIG, &mut parent_death_signal) } != 0
        || parent_death_signal != libc::SIGKILL
    {
        return Err("hidden stage did not inherit the mandatory parent-death signal".into());
    }
    // SAFETY: these prctl queries have no pointer arguments.
    if unsafe { libc::prctl(libc::PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0) } != 1
        || unsafe { libc::prctl(libc::PR_GET_SECCOMP, 0, 0, 0, 0) } != 2
    {
        return Err("hidden stage did not inherit the mandatory seccomp boundary".into());
    }
    // SAFETY: getpid/getpgrp are infallible identity queries.
    if unsafe { libc::getpid() } != unsafe { libc::getpgrp() } {
        return Err("hidden stage is not the leader of its private process group".into());
    }
    // SAFETY: PR_SET_DUMPABLE accepts one boolean scalar.
    if unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0) } != 0
        || unsafe { libc::prctl(libc::PR_GET_DUMPABLE, 0, 0, 0, 0) } != 0
    {
        return Err("hidden stage could not disable ptrace-style dumpability".into());
    }

    let deny = RELEASE_SECCOMP_RET_ERRNO_V1
        | u32::try_from(libc::EPERM).map_err(|_| "EPERM is outside seccomp errno width")?;
    let unsupported = RELEASE_SECCOMP_RET_ERRNO_V1
        | u32::try_from(libc::ENOSYS).map_err(|_| "ENOSYS is outside seccomp errno width")?;
    let mut filter = vec![
        release_bpf_statement_v1(BPF_LD_W_ABS, SECCOMP_DATA_ARCH_OFFSET),
        release_bpf_jump_v1(BPF_JMP_JEQ_K, RELEASE_AUDIT_ARCH_V1, 1, 0),
        release_bpf_statement_v1(BPF_RET_K, RELEASE_SECCOMP_RET_KILL_PROCESS_V1),
        release_bpf_statement_v1(BPF_LD_W_ABS, SECCOMP_DATA_NR_OFFSET),
        release_bpf_jump_v1(BPF_JMP_JSET_K, RELEASE_FORBIDDEN_SYSCALL_ABI_MASK_V1, 0, 1),
        release_bpf_statement_v1(BPF_RET_K, deny),
    ];
    // Closed compute/runtime allowlist. Filesystem path access is independently
    // denied by Landlock, and the post-exec FD sweep leaves only stdio plus the
    // exact result descriptor. Every syscall omitted here fails with EPERM.
    for &syscall_number in RELEASE_POST_EXEC_UNCONDITIONAL_SYSCALLS_V1 {
        filter.push(release_bpf_jump_v1(
            BPF_JMP_JEQ_K,
            u32::try_from(syscall_number)
                .map_err(|_| "allowlisted syscall number is outside seccomp width")?,
            0,
            1,
        ));
        filter.push(release_bpf_statement_v1(
            BPF_RET_K,
            RELEASE_SECCOMP_RET_ALLOW_V1,
        ));
    }

    for syscall_number in [libc::SYS_mmap, libc::SYS_mprotect] {
        filter.extend([
            release_bpf_jump_v1(
                BPF_JMP_JEQ_K,
                u32::try_from(syscall_number)
                    .map_err(|_| "memory syscall number is outside seccomp width")?,
                0,
                4,
            ),
            release_bpf_statement_v1(BPF_LD_W_ABS, SECCOMP_DATA_ARG2_LOW_OFFSET),
            release_bpf_jump_v1(BPF_JMP_JSET_K, libc::PROT_EXEC as u32, 0, 1),
            release_bpf_statement_v1(BPF_RET_K, deny),
            release_bpf_statement_v1(BPF_RET_K, RELEASE_SECCOMP_RET_ALLOW_V1),
        ]);
    }
    // Rayon and the watchdog are already fully initialized and the exact
    // six-task topology was attested through procfs. Freeze it: neither a
    // process nor another thread may be created in the proof phase.
    for syscall_number in [RELEASE_NR_FORK_V1, RELEASE_NR_VFORK_V1, RELEASE_NR_CLONE_V1] {
        filter.extend([
            release_bpf_jump_v1(BPF_JMP_JEQ_K, syscall_number, 0, 1),
            release_bpf_statement_v1(BPF_RET_K, deny),
        ]);
    }
    filter.extend([
        release_bpf_jump_v1(BPF_JMP_JEQ_K, RELEASE_NR_CLONE3_V1, 0, 1),
        release_bpf_statement_v1(BPF_RET_K, unsupported),
    ]);
    // The watchdog may kill only its own thread group; panic machinery may use
    // tgkill only when its TGID is this exact hidden-stage PID.
    // SAFETY: getpid is an infallible identity query performed before sealing.
    let own_pid = u32::try_from(unsafe { libc::getpid() })
        .map_err(|_| "hidden-stage PID is outside seccomp argument width")?;
    for syscall_number in [libc::SYS_kill, libc::SYS_tgkill] {
        filter.extend([
            release_bpf_jump_v1(
                BPF_JMP_JEQ_K,
                u32::try_from(syscall_number)
                    .map_err(|_| "signal syscall number is outside seccomp width")?,
                0,
                4,
            ),
            release_bpf_statement_v1(BPF_LD_W_ABS, SECCOMP_DATA_ARG0_LOW_OFFSET),
            release_bpf_jump_v1(BPF_JMP_JEQ_K, own_pid, 0, 1),
            release_bpf_statement_v1(BPF_RET_K, RELEASE_SECCOMP_RET_ALLOW_V1),
            release_bpf_statement_v1(BPF_RET_K, deny),
        ]);
    }
    filter.push(release_bpf_statement_v1(BPF_RET_K, deny));
    install_seccomp_filter_v1(&mut filter, libc::SECCOMP_FILTER_FLAG_TSYNC)
        .map_err(|error| format!("cannot seal the post-exec stage boundary: {error}").into())
}

#[cfg(all(
    target_os = "linux",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn install_seccomp_filter_v1(
    filter: &mut [libc::sock_filter],
    flags: libc::c_ulong,
) -> std::io::Result<()> {
    let len = u16::try_from(filter.len())
        .map_err(|_| std::io::Error::from_raw_os_error(libc::EOVERFLOW))?;
    let program = libc::sock_fprog {
        len,
        filter: filter.as_mut_ptr(),
    };
    // SAFETY: no_new_privs receives only scalar arguments and seccomp copies
    // the complete filter before returning. The caller selects TSYNC only for
    // the post-exec boundary, where every extant thread must be covered.
    if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: the operation/flag pair and program pointer follow seccomp(2).
    let installed = unsafe {
        libc::syscall(
            libc::SYS_seccomp,
            libc::SECCOMP_SET_MODE_FILTER,
            flags,
            &program as *const libc::sock_fprog,
        )
    };
    if installed != 0 {
        return if installed < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "seccomp TSYNC left one thread outside the release filter",
            ))
        };
    }
    Ok(())
}

#[cfg(all(
    target_os = "linux",
    not(all(
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))
))]
fn install_pre_exec_stage_controls(
    _expected_parent_pid: i32,
    _result_fd: RawFd,
    _elapsed_ceiling_millis: u64,
    _address_space_ceiling_bytes: u64,
) -> std::io::Result<()> {
    Err(std::io::Error::from_raw_os_error(libc::ENOTSUP))
}

#[cfg(any(
    not(target_os = "linux"),
    all(
        target_os = "linux",
        not(all(
            target_endian = "little",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ))
    )
))]
fn install_post_exec_stage_controls_v1() -> Result<(), DynError> {
    Err("Taira post-exec confinement supports only Linux x86_64 and aarch64".into())
}

#[cfg(not(target_os = "linux"))]
fn install_pre_exec_stage_controls(
    _expected_parent_pid: i32,
    _result_fd: RawFd,
    _elapsed_ceiling_millis: u64,
    _address_space_ceiling_bytes: u64,
) -> std::io::Result<()> {
    Err(std::io::Error::from_raw_os_error(libc::ENOTSUP))
}

fn immutable_runner_exec_path(fd: RawFd) -> Result<PathBuf, DynError> {
    if fd < 0 {
        return Err("immutable runner file descriptor is negative".into());
    }
    #[cfg(target_os = "linux")]
    {
        Ok(PathBuf::from(format!("/proc/self/fd/{fd}")))
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err("Taira release stages require Linux sealed-fd execution".into())
    }
}

fn ensure_taira_release_platform() -> Result<(), DynError> {
    #[cfg(all(
        target_os = "linux",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    {
        Ok(())
    }
    #[cfg(not(all(
        target_os = "linux",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )))]
    {
        Err("Taira privacy release evidence requires little-endian Linux x86_64 or aarch64".into())
    }
}

#[cfg(target_os = "linux")]
fn linux_open_absolute(
    path: &Path,
    flags: libc::c_int,
    mode: libc::mode_t,
    label: &str,
) -> Result<File, DynError> {
    let absolute = absolute_normalized(path)?;
    let relative = absolute
        .strip_prefix(Path::new("/"))
        .map_err(|_| format!("{label} is not rooted at `/`"))?;
    if relative.as_os_str().is_empty() {
        return Err(format!("{label} must not name the filesystem root").into());
    }
    let relative = CString::new(relative.as_os_str().as_bytes())
        .map_err(|_| format!("{label} path contains NUL"))?;
    // SAFETY: the static root path is NUL-terminated and the flags request one
    // read-only path anchor.
    let root_fd = unsafe {
        libc::open(
            c"/".as_ptr(),
            libc::O_PATH | libc::O_DIRECTORY | libc::O_CLOEXEC,
        )
    };
    if root_fd < 0 {
        return Err(format!(
            "cannot open root anchor for {label}: {}",
            std::io::Error::last_os_error()
        )
        .into());
    }
    let how = libc::open_how {
        flags: u64::try_from(flags).map_err(|_| format!("{label} open flags are negative"))?,
        mode: u64::from(mode),
        resolve: libc::RESOLVE_BENEATH | libc::RESOLVE_NO_MAGICLINKS | libc::RESOLVE_NO_SYMLINKS,
    };
    // SAFETY: `root_fd` is a live directory descriptor, `relative` is
    // NUL-terminated, and `how` has the kernel's exact openat2 layout.
    let opened = unsafe {
        libc::syscall(
            libc::SYS_openat2,
            root_fd,
            relative.as_ptr(),
            &how,
            std::mem::size_of::<libc::open_how>(),
        )
    };
    let open_error = (opened < 0).then(std::io::Error::last_os_error);
    // SAFETY: `root_fd` is uniquely owned by this function.
    let close_result = unsafe { libc::close(root_fd) };
    if close_result != 0 {
        if let Ok(opened_fd) = i32::try_from(opened)
            && opened_fd >= 0
        {
            // SAFETY: a successful openat2 result is uniquely owned here.
            let _ = unsafe { libc::close(opened_fd) };
        }
        return Err(format!(
            "cannot close root anchor for {label}: {}",
            std::io::Error::last_os_error()
        )
        .into());
    }
    if opened < 0 {
        return Err(format!(
            "cannot open {label} with anchored openat2: {}",
            open_error.expect("negative openat2 result captured an error")
        )
        .into());
    }
    let opened =
        i32::try_from(opened).map_err(|_| format!("{label} descriptor exceeds positive i32"))?;
    // SAFETY: successful openat2 returned one newly owned descriptor.
    Ok(unsafe { File::from_raw_fd(opened) })
}

#[cfg(target_os = "linux")]
fn open_release_input(path: &Path, label: &str) -> Result<File, DynError> {
    linux_open_absolute(
        path,
        libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        0,
        label,
    )
}

#[cfg(not(target_os = "linux"))]
fn open_release_input(path: &Path, label: &str) -> Result<File, DynError> {
    let absolute = absolute_normalized(path)?;
    reject_symlink_ancestors(&absolute, true)?;
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&absolute)
        .map_err(|error| format!("cannot open {label} without following links: {error}").into())
}

#[cfg(not(target_os = "linux"))]
fn open_release_output_create_new(path: &Path, label: &str) -> Result<File, DynError> {
    let absolute = absolute_normalized(path)?;
    reject_symlink_ancestors(absolute.parent().ok_or("output path has no parent")?, true)?;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&absolute)
        .map_err(|error| format!("cannot securely create new {label}: {error}").into())
}

#[cfg(target_os = "linux")]
fn open_live_process_executable() -> Result<File, DynError> {
    // `/proc/self/exe` is the kernel-owned identity of the image executing this
    // process. Following this one magic link is intentional; all caller paths
    // use the no-magic-link openat2 corridor above.
    // SAFETY: the static proc path is NUL-terminated.
    let fd = unsafe { libc::open(c"/proc/self/exe".as_ptr(), libc::O_RDONLY | libc::O_CLOEXEC) };
    if fd < 0 {
        return Err(format!(
            "cannot open live process executable: {}",
            std::io::Error::last_os_error()
        )
        .into());
    }
    // SAFETY: successful open returned one newly owned descriptor.
    Ok(unsafe { File::from_raw_fd(fd) })
}

#[cfg(target_os = "linux")]
fn validate_static_release_elf_v1(file: &File, file_len: u64) -> Result<(), DynError> {
    const ELF64_HEADER_BYTES: usize = 64;
    const ELF64_PROGRAM_HEADER_BYTES: usize = 56;
    const ELF64_DYNAMIC_ENTRY_BYTES: usize = 16;
    const ET_EXEC: u16 = 2;
    const ET_DYN: u16 = 3;
    const EM_X86_64: u16 = 62;
    const EM_AARCH64: u16 = 183;
    const PT_LOAD: u32 = 1;
    const PT_DYNAMIC: u32 = 2;
    const PT_INTERP: u32 = 3;
    const DT_NULL: i64 = 0;
    const DT_NEEDED: i64 = 1;
    const MAX_PROGRAM_HEADERS: u16 = 256;
    const MAX_DYNAMIC_TABLE_BYTES: u64 = 1024 * 1024;

    if file_len < ELF64_HEADER_BYTES as u64 {
        return Err("release runner is shorter than one ELF64 header".into());
    }
    let mut header = [0_u8; ELF64_HEADER_BYTES];
    file.read_exact_at(&mut header, 0)
        .map_err(|error| format!("cannot read release runner ELF header: {error}"))?;
    if &header[..4] != b"\x7fELF" || header[4] != 2 || header[5] != 1 || header[6] != 1 {
        return Err("release runner must be canonical little-endian ELF64".into());
    }
    let elf_type = u16::from_le_bytes([header[16], header[17]]);
    if elf_type != ET_EXEC && elf_type != ET_DYN {
        return Err("release runner ELF type is neither executable nor static PIE".into());
    }
    let machine = u16::from_le_bytes([header[18], header[19]]);
    let expected_machine = if cfg!(target_arch = "x86_64") {
        EM_X86_64
    } else if cfg!(target_arch = "aarch64") {
        EM_AARCH64
    } else {
        return Err("release runner ELF machine is unsupported".into());
    };
    if machine != expected_machine {
        return Err("release runner ELF machine differs from the executing architecture".into());
    }
    let program_header_offset = u64::from_le_bytes(
        header[32..40]
            .try_into()
            .expect("fixed ELF program-header offset slice"),
    );
    let program_header_size = u16::from_le_bytes([header[54], header[55]]);
    let program_header_count = u16::from_le_bytes([header[56], header[57]]);
    if usize::from(program_header_size) != ELF64_PROGRAM_HEADER_BYTES
        || program_header_count == 0
        || program_header_count > MAX_PROGRAM_HEADERS
    {
        return Err("release runner has a non-canonical ELF64 program-header table".into());
    }
    let table_bytes = u64::from(program_header_size)
        .checked_mul(u64::from(program_header_count))
        .ok_or("release runner program-header table length overflowed")?;
    let table_end = program_header_offset
        .checked_add(table_bytes)
        .ok_or("release runner program-header table end overflowed")?;
    if table_end > file_len {
        return Err("release runner program-header table exceeds the file".into());
    }

    let mut saw_load = false;
    for index in 0..program_header_count {
        let offset = program_header_offset
            .checked_add(
                u64::from(index)
                    .checked_mul(u64::from(program_header_size))
                    .ok_or("release runner program-header offset overflowed")?,
            )
            .ok_or("release runner program-header offset overflowed")?;
        let mut program = [0_u8; ELF64_PROGRAM_HEADER_BYTES];
        file.read_exact_at(&mut program, offset)
            .map_err(|error| format!("cannot read release runner program header: {error}"))?;
        let program_type = u32::from_le_bytes(
            program[..4]
                .try_into()
                .expect("fixed ELF program-type slice"),
        );
        saw_load |= program_type == PT_LOAD;
        if program_type == PT_INTERP {
            return Err(
                "release runner contains PT_INTERP and is not a fully static executable".into(),
            );
        }
        if program_type != PT_DYNAMIC {
            continue;
        }
        let dynamic_offset = u64::from_le_bytes(
            program[8..16]
                .try_into()
                .expect("fixed ELF segment-offset slice"),
        );
        let dynamic_bytes = u64::from_le_bytes(
            program[32..40]
                .try_into()
                .expect("fixed ELF segment-size slice"),
        );
        if dynamic_bytes == 0
            || dynamic_bytes > MAX_DYNAMIC_TABLE_BYTES
            || dynamic_bytes % ELF64_DYNAMIC_ENTRY_BYTES as u64 != 0
            || dynamic_offset
                .checked_add(dynamic_bytes)
                .is_none_or(|end| end > file_len)
        {
            return Err("release runner has a malformed ELF dynamic table".into());
        }
        let mut saw_null = false;
        for entry_offset in (0..dynamic_bytes).step_by(ELF64_DYNAMIC_ENTRY_BYTES) {
            let mut entry = [0_u8; ELF64_DYNAMIC_ENTRY_BYTES];
            file.read_exact_at(
                &mut entry,
                dynamic_offset
                    .checked_add(entry_offset)
                    .ok_or("release runner dynamic-entry offset overflowed")?,
            )
            .map_err(|error| format!("cannot read release runner dynamic entry: {error}"))?;
            let tag =
                i64::from_le_bytes(entry[..8].try_into().expect("fixed ELF dynamic-tag slice"));
            if tag == DT_NEEDED {
                return Err(
                    "release runner contains DT_NEEDED and depends on mutable shared objects"
                        .into(),
                );
            }
            if tag == DT_NULL {
                saw_null = true;
                break;
            }
        }
        if !saw_null {
            return Err("release runner ELF dynamic table lacks DT_NULL".into());
        }
    }
    if !saw_load {
        return Err("release runner ELF contains no loadable segment".into());
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn prepare_immutable_runner() -> Result<ImmutableRunnerV1, DynError> {
    let source_path = env::current_exe().map_err(|error| {
        format!("failed to resolve current evidence runner executable: {error}")
    })?;
    let mut source = open_live_process_executable()?;
    let before = source
        .metadata()
        .map_err(|error| format!("cannot stat live runner binary: {error}"))?;
    validate_regular_metadata(&before, MAX_EXECUTABLE_BYTES, "runner binary")?;
    if before.mode() & 0o111 == 0 || before.nlink() != 1 {
        return Err("runner binary must be executable and have exactly one filesystem link".into());
    }
    validate_static_release_elf_v1(&source, before.len())?;
    let source_identity = file_identity(&before);
    let named_source = open_release_input(&source_path, "resolved runner binary")?;
    let named_metadata = named_source
        .metadata()
        .map_err(|error| format!("cannot stat resolved runner binary: {error}"))?;
    if file_identity(&named_metadata) != source_identity
        || named_metadata.len() != before.len()
        || named_metadata.mode() != before.mode()
    {
        return Err(
            "resolved runner pathname does not identify the live process executable".into(),
        );
    }
    let (source_digest, source_bytes) = sha256_reader_bounded(&mut source, MAX_EXECUTABLE_BYTES)
        .map_err(|error| format!("cannot hash runner binary: {error}"))?;
    if source_bytes != before.len() {
        return Err("runner binary length changed while it was hashed".into());
    }
    source
        .seek(SeekFrom::Start(0))
        .map_err(|error| format!("cannot rewind runner binary: {error}"))?;

    let name =
        CString::new("taira-privacy-release-runner-v1").expect("fixed memfd label contains no NUL");
    // SAFETY: the fixed C string is NUL-terminated and flags are valid for
    // `memfd_create(2)`.
    let raw_fd = unsafe {
        libc::memfd_create(
            name.as_ptr(),
            libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING | libc::MFD_EXEC,
        )
    };
    if raw_fd < 0 {
        return Err(format!(
            "cannot create anonymous sealed runner: {}",
            std::io::Error::last_os_error()
        )
        .into());
    }
    // SAFETY: `memfd_create` returned one newly owned descriptor.
    let mut executable = unsafe { File::from_raw_fd(raw_fd) };
    let mut copied = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = source
            .read(&mut buffer)
            .map_err(|error| format!("cannot copy runner into sealed storage: {error}"))?;
        if read == 0 {
            break;
        }
        copied = copied
            .checked_add(u64::try_from(read).map_err(|_| "runner copy length exceeds u64")?)
            .ok_or("runner copy length overflowed u64")?;
        if copied > MAX_EXECUTABLE_BYTES || copied > before.len() {
            return Err("runner copy exceeded its authenticated source length".into());
        }
        executable
            .write_all(&buffer[..read])
            .map_err(|error| format!("cannot write anonymous runner copy: {error}"))?;
    }
    if copied != before.len() {
        return Err("runner copy length differs from authenticated source".into());
    }
    executable
        .sync_all()
        .map_err(|error| format!("cannot sync anonymous runner copy: {error}"))?;
    // SAFETY: the descriptor is owned, open, and the mode contains only
    // permission bits.
    if unsafe { libc::fchmod(raw_fd, 0o500) } < 0 {
        return Err(format!(
            "cannot make anonymous runner executable: {}",
            std::io::Error::last_os_error()
        )
        .into());
    }
    let required_seals = libc::F_SEAL_WRITE
        | libc::F_SEAL_GROW
        | libc::F_SEAL_SHRINK
        | libc::F_SEAL_EXEC
        | libc::F_SEAL_SEAL;
    // SAFETY: F_ADD_SEALS is valid for an MFD_ALLOW_SEALING memfd.
    if unsafe { libc::fcntl(raw_fd, libc::F_ADD_SEALS, required_seals) } < 0 {
        return Err(format!(
            "cannot seal anonymous runner against mutation: {}",
            std::io::Error::last_os_error()
        )
        .into());
    }
    // SAFETY: F_GET_SEALS reads the seal mask without mutating the descriptor.
    let observed_seals = unsafe { libc::fcntl(raw_fd, libc::F_GET_SEALS) };
    if observed_seals < 0 || observed_seals & required_seals != required_seals {
        return Err("anonymous runner did not retain every mandatory write/size seal".into());
    }

    executable
        .seek(SeekFrom::Start(0))
        .map_err(|error| format!("cannot rewind sealed runner: {error}"))?;
    let (sealed_digest, sealed_bytes) =
        sha256_reader_bounded(&mut executable, MAX_EXECUTABLE_BYTES)
            .map_err(|error| format!("cannot hash sealed runner: {error}"))?;
    if sealed_bytes != source_bytes || sealed_digest != source_digest {
        return Err("sealed runner bytes differ from the authenticated executable".into());
    }
    let after = source
        .metadata()
        .map_err(|error| format!("cannot restat opened runner binary: {error}"))?;
    if file_identity(&after) != source_identity
        || after.len() != before.len()
        || after.mode() != before.mode()
        || after.nlink() != before.nlink()
        || after.mtime() != before.mtime()
        || after.mtime_nsec() != before.mtime_nsec()
        || after.ctime() != before.ctime()
        || after.ctime_nsec() != before.ctime_nsec()
    {
        return Err("runner binary changed while its sealed copy was prepared".into());
    }

    Ok(ImmutableRunnerV1 {
        executable,
        source_path,
        source_identity,
        sha256: sealed_digest,
    })
}

#[cfg(not(target_os = "linux"))]
fn prepare_immutable_runner() -> Result<ImmutableRunnerV1, DynError> {
    Err("anonymous sealed runner preparation requires Linux memfd seals".into())
}

fn exit_status_description(status: ExitStatus) -> String {
    if let Some(code) = status.code() {
        format!("with code {code}")
    } else if let Some(signal) = status.signal() {
        format!("from signal {signal}")
    } else {
        "with unknown status".to_owned()
    }
}

fn canonical_stage_coordinates()
-> Result<&'static [PrivacyReleaseStageCoordinateV1; PRIVACY_RELEASE_STAGE_COUNT_V1], DynError> {
    if !validate_privacy_release_stage_coordinates_v1(&PRIVACY_RELEASE_STAGE_COORDINATES_V1) {
        return Err(
            "frozen 48-stage declaration drifted from the closed protocol-by-case enum product"
                .into(),
        );
    }
    Ok(&PRIVACY_RELEASE_STAGE_COORDINATES_V1)
}

fn validate_expectation_stage_coordinates_v1(
    expectations: &PrivacyReleaseExpectationsV1,
) -> Result<(), DynError> {
    if usize::from(expectations.stage_count) != PRIVACY_RELEASE_STAGE_COUNT_V1
        || expectations.stages.len() != PRIVACY_RELEASE_STAGE_COUNT_V1
    {
        return Err(format!(
            "expectations must contain exactly {PRIVACY_RELEASE_STAGE_COUNT_V1} stages"
        )
        .into());
    }
    for (index, (expected, coordinate)) in expectations
        .stages
        .iter()
        .zip(canonical_stage_coordinates()?.iter())
        .enumerate()
    {
        if expected.evidence.stage_ordinal != coordinate.stage_ordinal
            || expected.evidence.protocol_id != coordinate.protocol_id
            || expected.evidence.case_kind != coordinate.case_kind
            || usize::from(coordinate.stage_ordinal) != index
        {
            return Err(format!("stage {index} is outside the frozen exact-48 declaration").into());
        }
    }
    Ok(())
}

fn validate_expectations(expectations: &PrivacyReleaseExpectationsV1) -> Result<(), DynError> {
    if expectations.schema_version != ARTIFACT_SCHEMA_VERSION_V1 {
        return Err("expectations schema version is not exactly v1".into());
    }
    validate_expectation_stage_coordinates_v1(expectations)?;
    for (index, (expected, coordinate)) in expectations
        .stages
        .iter()
        .zip(canonical_stage_coordinates()?.iter())
        .enumerate()
    {
        validate_stage_process_ceilings_v1(
            coordinate.protocol_id,
            expected.max_elapsed_millis,
            expected.max_peak_rss_bytes,
            expected.max_address_space_bytes,
        )?;
        validate_stage_evidence(
            &expected.evidence,
            coordinate.protocol_id,
            coordinate.case_kind,
            index,
        )?;
    }
    validate_aggregate_proof_artifact_bytes_v1(
        expectations.stages.iter().map(|stage| &stage.evidence),
    )?;
    for protocol_stages in expectations
        .stages
        .chunks_exact(PRIVACY_RELEASE_CASE_COUNT_V1)
    {
        let descriptor = &protocol_stages[0].evidence.protocol_descriptor;
        if protocol_stages
            .iter()
            .any(|stage| stage.evidence.protocol_descriptor != *descriptor)
        {
            return Err("one protocol uses inconsistent descriptors across its four cases".into());
        }
        let positive = &protocol_stages[0].evidence;
        let mutation = &protocol_stages[1].evidence;
        let corruption = &protocol_stages[2].evidence;
        let maximum = &protocol_stages[3].evidence;
        if positive.public_statement_sha256 == mutation.public_statement_sha256 {
            return Err(
                "public-statement mutation case did not change canonical public material".into(),
            );
        }
        if positive.public_statement_sha256 != corruption.public_statement_sha256 {
            return Err(
                "proof-corruption case did not preserve the positive public statement".into(),
            );
        }
        if maximum.resources.primary_units != maximum.resources.primary_ceiling
            || maximum.resources.secondary_units != maximum.resources.secondary_ceiling
            || maximum.resources.relation_depth != maximum.resources.relation_depth_ceiling
        {
            return Err(
                "maximum-shape case does not exercise every declared relation dimension ceiling"
                    .into(),
            );
        }
    }
    Ok(())
}

fn validate_aggregate_proof_artifact_bytes_v1<'a, I>(stages: I) -> Result<u64, DynError>
where
    I: Clone + Iterator<Item = &'a PrivacyReleaseStageEvidenceV1>,
{
    validate_aggregate_proof_artifact_lengths_v1(stages.flat_map(|stage| {
        stage
            .proof_artifacts
            .iter()
            .map(|artifact| artifact.canonical_proof_bytes.len())
    }))
}

fn validate_aggregate_proof_artifact_lengths_v1<I>(artifact_lengths: I) -> Result<u64, DynError>
where
    I: Clone + Iterator<Item = usize>,
{
    let mut artifact_count = 0_usize;
    for _ in artifact_lengths.clone() {
        artifact_count = artifact_count
            .checked_add(1)
            .ok_or("proof-artifact count overflowed usize")?;
        if artifact_count > PRIVACY_RELEASE_PROOF_ARTIFACT_COUNT_V1 {
            return Err(format!(
                "release evidence contains more than the exact {} proof artifacts",
                PRIVACY_RELEASE_PROOF_ARTIFACT_COUNT_V1
            )
            .into());
        }
    }
    if artifact_count != PRIVACY_RELEASE_PROOF_ARTIFACT_COUNT_V1 {
        return Err(format!(
            "release evidence must contain exactly {} proof artifacts",
            PRIVACY_RELEASE_PROOF_ARTIFACT_COUNT_V1
        )
        .into());
    }

    let mut total_bytes = 0_u64;
    for artifact_length in artifact_lengths {
        let artifact_bytes = u64::try_from(artifact_length)
            .map_err(|_| "canonical proof artifact length exceeds u64")?;
        total_bytes = total_bytes
            .checked_add(artifact_bytes)
            .ok_or("aggregate canonical proof bytes overflowed u64")?;
        if total_bytes > PRIVACY_RELEASE_MAX_TOTAL_PROOF_ARTIFACT_BYTES_V1 {
            return Err(format!(
                "aggregate canonical proof bytes exceed {}",
                PRIVACY_RELEASE_MAX_TOTAL_PROOF_ARTIFACT_BYTES_V1
            )
            .into());
        }
    }
    Ok(total_bytes)
}

fn validate_stage_evidence(
    evidence: &PrivacyReleaseStageEvidenceV1,
    protocol_id: PrivacyProtocolIdV1,
    case_kind: PrivacyReleaseCaseKindV1,
    index: usize,
) -> Result<(), DynError> {
    if evidence.schema_version != PRIVACY_RELEASE_EVIDENCE_SCHEMA_VERSION_V1 {
        return Err(format!("stage {index} core evidence schema is not exactly v1").into());
    }
    let ordinal = privacy_release_stage_ordinal_v1(protocol_id, case_kind);
    if evidence.stage_ordinal != ordinal
        || evidence.protocol_id != protocol_id
        || evidence.case_kind != case_kind
        || usize::from(ordinal) != index
    {
        return Err(format!("stage {index} is outside canonical exact-48 order").into());
    }
    if evidence.protocol_descriptor != privacy_release_protocol_descriptor_v1(protocol_id) {
        return Err(format!("stage {index} substituted its canonical protocol descriptor").into());
    }
    if evidence.public_statement_sha256 == [0; 32] {
        return Err(format!("stage {index} contains a zero public-statement digest").into());
    }
    if !validate_privacy_release_proof_artifacts_v1(
        protocol_id,
        case_kind,
        &evidence.proof_artifacts,
    ) {
        return Err(
            format!("stage {index} has an invalid ordered proof-artifact collection").into(),
        );
    }
    validate_resource_facts(&evidence.resources, protocol_id, case_kind, index)?;
    let expected_failure = match case_kind {
        PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd
        | PrivacyReleaseCaseKindV1::MaximumShapeResource => {
            PrivacyReleaseFailureClassV1::NotApplicable
        }
        PrivacyReleaseCaseKindV1::PublicStatementBindingMutation => {
            PrivacyReleaseFailureClassV1::PublicStatementBindingRejected
        }
        PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation => {
            PrivacyReleaseFailureClassV1::CanonicalWireCorruptionAndTruncationRejected
        }
    };
    if evidence.failure_class != expected_failure {
        return Err(format!("stage {index} has the wrong stable failure classification").into());
    }
    Ok(())
}

fn validate_resource_facts(
    resources: &PrivacyReleaseResourceFactsV1,
    protocol_id: PrivacyProtocolIdV1,
    case_kind: PrivacyReleaseCaseKindV1,
    index: usize,
) -> Result<(), DynError> {
    if resources.primary_units == 0
        || resources.primary_units > resources.primary_ceiling
        || resources.secondary_units > resources.secondary_ceiling
        || resources.relation_depth > resources.relation_depth_ceiling
    {
        return Err(format!("stage {index} exceeds or omits a governed resource fact").into());
    }
    if resources.primary_ceiling == 0 {
        return Err(format!("stage {index} has an invalid frozen resource ceiling").into());
    }
    let expected = privacy_release_resource_facts_v1(protocol_id, case_kind)
        .ok_or_else(|| format!("stage {index} has no canonical resource-fact declaration"))?;
    if resources != &expected {
        return Err(format!(
            "stage {index} substituted a frozen resource fact or its governed ceiling"
        )
        .into());
    }
    Ok(())
}

fn validate_measured_against_expectations(
    measured: &[MeasuredStageV1],
    expectations: &PrivacyReleaseExpectationsV1,
) -> Result<(), DynError> {
    validate_expectations(expectations)?;
    if measured.len() != PRIVACY_RELEASE_STAGE_COUNT_V1
        || expectations.stages.len() != PRIVACY_RELEASE_STAGE_COUNT_V1
    {
        return Err("native measurement did not produce the exact 48-stage closure".into());
    }
    validate_aggregate_proof_artifact_bytes_v1(measured.iter().map(|stage| &stage.evidence))?;
    for (index, ((actual, expected), coordinate)) in measured
        .iter()
        .zip(&expectations.stages)
        .zip(canonical_stage_coordinates()?.iter())
        .enumerate()
    {
        validate_stage_evidence(
            &actual.evidence,
            coordinate.protocol_id,
            coordinate.case_kind,
            index,
        )?;
        if actual.evidence != expected.evidence {
            return Err(format!(
                "stage {index} native deterministic fields do not match frozen expectations"
            )
            .into());
        }
        if actual.elapsed_millis > expected.max_elapsed_millis {
            return Err(format!(
                "stage {index} elapsed time {} exceeds frozen ceiling {}",
                actual.elapsed_millis, expected.max_elapsed_millis
            )
            .into());
        }
        if actual.peak_rss_bytes > expected.max_peak_rss_bytes {
            return Err(format!(
                "stage {index} peak RSS {} exceeds frozen ceiling {}",
                actual.peak_rss_bytes, expected.max_peak_rss_bytes
            )
            .into());
        }
        if actual.peak_address_space_bytes == 0
            || actual.peak_address_space_bytes > expected.max_address_space_bytes
        {
            return Err(format!(
                "stage {index} peak address space {} is zero or exceeds frozen ceiling {}",
                actual.peak_address_space_bytes, expected.max_address_space_bytes
            )
            .into());
        }
    }
    Ok(())
}

fn validate_stage_artifacts(
    artifacts: &PrivacyReleaseStageArtifactsV1,
    expectations: &PrivacyReleaseExpectationsV1,
) -> Result<(), DynError> {
    validate_expectations(expectations)?;
    if artifacts.schema_version != ARTIFACT_SCHEMA_VERSION_V1 {
        return Err("stage-artifact schema version is not exactly v1".into());
    }
    if usize::from(artifacts.stage_count) != PRIVACY_RELEASE_STAGE_COUNT_V1
        || artifacts.stages.len() != PRIVACY_RELEASE_STAGE_COUNT_V1
    {
        return Err(format!(
            "stage artifacts must contain exactly {PRIVACY_RELEASE_STAGE_COUNT_V1} blocks"
        )
        .into());
    }
    validate_aggregate_proof_artifact_bytes_v1(
        artifacts.stages.iter().map(|stage| &stage.evidence),
    )?;
    for (index, ((stored, expected), coordinate)) in artifacts
        .stages
        .iter()
        .zip(&expectations.stages)
        .zip(canonical_stage_coordinates()?.iter())
        .enumerate()
    {
        validate_stage_evidence(
            &stored.evidence,
            coordinate.protocol_id,
            coordinate.case_kind,
            index,
        )?;
        if stored.evidence != expected.evidence {
            return Err(
                format!("stored stage {index} differs from frozen native KAT fields").into(),
            );
        }
        if stored.elapsed_millis == 0
            || stored.elapsed_millis > expected.max_elapsed_millis
            || stored.peak_rss_bytes == 0
            || stored.peak_rss_bytes > expected.max_peak_rss_bytes
            || stored.peak_address_space_bytes == 0
            || stored.peak_address_space_bytes > expected.max_address_space_bytes
        {
            return Err(
                format!("stored stage {index} violates its process resource ceiling").into(),
            );
        }
    }
    Ok(())
}

fn validate_receipt(
    receipt: &PrivacyReleaseReceiptV1,
    loaded: &LoadedInputsV1,
    command_norito: &[u8],
    command_json: &[u8],
    stages_norito: &[u8],
    stages_json: &[u8],
) -> Result<(), DynError> {
    let expected = PrivacyReleaseReceiptV1 {
        schema_version: ARTIFACT_SCHEMA_VERSION_V1,
        build_profile: loaded.common.build_profile.clone(),
        source_sha256: loaded.common.source_sha256,
        exact12_matrix_sha256: loaded.exact12_sha256,
        expectations: PrivacyReleaseArtifactPairDigestV1 {
            norito_sha256: sha256_bytes(&loaded.expectations_norito_bytes),
            json_sha256: sha256_bytes(&loaded.expectations_json_bytes),
        },
        x509_resource: PrivacyReleaseArtifactPairDigestV1 {
            norito_sha256: sha256_bytes(&loaded.x509_resource.norito_bytes),
            json_sha256: sha256_bytes(&loaded.x509_resource.json_bytes),
        },
        cargo_lock_sha256: loaded.cargo_lock_sha256,
        validator_binary_sha256: loaded.validator_binary_sha256,
        runner_binary_sha256: loaded.runner_binary_sha256,
        command_manifest: PrivacyReleaseArtifactPairDigestV1 {
            norito_sha256: sha256_bytes(command_norito),
            json_sha256: sha256_bytes(command_json),
        },
        stage_artifacts: PrivacyReleaseArtifactPairDigestV1 {
            norito_sha256: sha256_bytes(stages_norito),
            json_sha256: sha256_bytes(stages_json),
        },
        fixed_stage_count: u16::try_from(PRIVACY_RELEASE_STAGE_COUNT_V1)
            .expect("fixed stage count fits u16"),
        all_native_stages_passed: true,
        contains_witnesses: false,
        contains_canonical_proof_artifacts: true,
        isolation_policy_enforced: true,
    };
    if receipt != &expected {
        return Err(
            "receipt does not bind the current runner, validator, Cargo.lock, exact12 matrix, expectations, source, profile, and typed artifacts"
                .into(),
        );
    }
    Ok(())
}

fn validate_exact12_matrix(bytes: &[u8]) -> Result<(), DynError> {
    validate_exact12_matrix_structure(bytes)
}

fn validate_exact12_matrix_structure(bytes: &[u8]) -> Result<(), DynError> {
    let generated = privacy_exact12_matrix_bytes_v1()
        .map_err(|error| format!("cannot generate compiled exact12 matrix: {error}"))?;
    let generated_text = std::str::from_utf8(&generated)
        .map_err(|_| "compiled exact12 generator did not produce UTF-8")?;
    let generated_header = generated_text
        .lines()
        .take_while(|line| line.starts_with('#'))
        .collect::<Vec<_>>();
    if bytes.is_empty() || bytes.contains(&b'\r') || bytes.last() != Some(&b'\n') {
        return Err(
            "exact12 matrix must be non-empty canonical UTF-8 with LF and final newline".into(),
        );
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "exact12 matrix is not UTF-8")?;
    if text
        .strip_suffix('\n')
        .ok_or("exact12 matrix is missing its terminal LF")?
        .lines()
        .any(str::is_empty)
    {
        return Err("exact12 matrix must not contain empty rows".into());
    }
    let mut matrix_version = None;
    let mut declared_registry_digest = None;
    let mut protocols = Vec::new();
    let mut typed_envelopes = Vec::new();
    let mut retired = Vec::new();
    let mut data_row_index = 0_usize;
    let mut header_row_index = 0_usize;
    let mut saw_data_row = false;
    for (line_index, line) in text.lines().enumerate() {
        if line.starts_with('#') {
            if saw_data_row || generated_header.get(header_row_index).copied() != Some(line) {
                return Err(format!(
                    "exact12 matrix header row {} is not canonical",
                    line_index + 1
                )
                .into());
            }
            header_row_index = header_row_index
                .checked_add(1)
                .ok_or("exact12 matrix header row count overflowed")?;
            continue;
        }
        saw_data_row = true;
        let fields = line.split('\t').collect::<Vec<_>>();
        let expected_kind = match data_row_index {
            0 => "matrix-version",
            1 => "registry-sha256",
            index if index < 2 + PrivacyProtocolIdV1::COUNT => "protocol",
            index if index < 2 + 2 * PrivacyProtocolIdV1::COUNT => "typed-envelope",
            index
                if index
                    < 2 + 2 * PrivacyProtocolIdV1::COUNT
                        + PRIVACY_RETIRED_PROTOCOL_LABELS_V1.len() =>
            {
                "retired"
            }
            _ => {
                return Err(format!(
                    "exact12 matrix contains an extra data row at line {}",
                    line_index + 1
                )
                .into());
            }
        };
        if fields.first().copied() != Some(expected_kind) {
            return Err(format!(
                "exact12 matrix data row {data_row_index} must be `{expected_kind}`"
            )
            .into());
        }
        data_row_index = data_row_index
            .checked_add(1)
            .ok_or("exact12 matrix row count overflowed")?;
        match fields.as_slice() {
            ["matrix-version", version] => {
                if *version != "1" || matrix_version.replace(*version).is_some() {
                    return Err("exact12 matrix version row is not exactly v1".into());
                }
            }
            ["registry-sha256", digest] => {
                if declared_registry_digest
                    .replace(parse_sha256(digest)?)
                    .is_some()
                {
                    return Err("exact12 matrix contains duplicate registry digest rows".into());
                }
            }
            ["protocol", index, label, statement_variant, proof_variant] => {
                protocols.push((
                    parse_canonical_usize(index)?,
                    *label,
                    *statement_variant,
                    *proof_variant,
                ));
            }
            [
                "typed-envelope",
                label,
                statement_variant,
                proof_variant,
                statement_digest,
                envelope_digest,
            ] => {
                typed_envelopes.push((
                    *label,
                    *statement_variant,
                    *proof_variant,
                    parse_nonzero_sha256(statement_digest, "typed statement digest")?,
                    parse_nonzero_sha256(envelope_digest, "typed envelope digest")?,
                ));
            }
            ["retired", label] => {
                if label.is_empty() {
                    return Err("exact12 retired row is malformed".into());
                }
                retired.push(*label);
            }
            [other, ..] => {
                return Err(format!(
                    "exact12 matrix row {} has unknown kind or arity `{other}`",
                    line_index + 1
                )
                .into());
            }
            [] => unreachable!("split always returns at least one field"),
        }
    }
    if matrix_version != Some("1") || declared_registry_digest.is_none() {
        return Err("exact12 matrix must contain one version and one registry digest row".into());
    }
    if header_row_index != generated_header.len() {
        return Err(format!(
            "exact12 matrix contains {header_row_index} canonical header rows, expected {}",
            generated_header.len()
        )
        .into());
    }
    let expected_data_rows =
        2 + 2 * PrivacyProtocolIdV1::COUNT + PRIVACY_RETIRED_PROTOCOL_LABELS_V1.len();
    if data_row_index != expected_data_rows {
        return Err(format!(
            "exact12 matrix contains {data_row_index} data rows, expected {expected_data_rows}"
        )
        .into());
    }
    if protocols.len() != PrivacyProtocolIdV1::COUNT {
        return Err(format!(
            "exact12 matrix contains {} protocol rows, expected {}",
            protocols.len(),
            PrivacyProtocolIdV1::COUNT
        )
        .into());
    }
    let mut registry_material = Vec::new();
    for (expected_index, ((actual_index, label, statement_variant, proof_variant), protocol_id)) in
        protocols.iter().zip(PrivacyProtocolIdV1::ALL).enumerate()
    {
        let canonical_variant = protocol_id.canonical_typed_variant_label();
        if *actual_index != expected_index
            || *label != protocol_id.canonical_label()
            || *statement_variant != canonical_variant
            || *proof_variant != canonical_variant
        {
            return Err(format!(
                "exact12 protocol row {expected_index} does not match the closed typed registry"
            )
            .into());
        }
        registry_material.extend_from_slice(label.as_bytes());
        registry_material.push(b'\n');
    }
    if declared_registry_digest != Some(sha256_bytes(&registry_material)) {
        return Err("exact12 registry digest does not match its ordered protocol labels".into());
    }
    if typed_envelopes.len() != PrivacyProtocolIdV1::COUNT {
        return Err(format!(
            "exact12 matrix contains {} typed-envelope rows, expected {}",
            typed_envelopes.len(),
            PrivacyProtocolIdV1::COUNT
        )
        .into());
    }
    let compiled_rows = privacy_exact12_typed_envelope_rows_v1()
        .map_err(|error| format!("cannot recompute compiled exact12 semantics: {error}"))?;
    for (
        expected_index,
        (
            (label, statement_variant, proof_variant, statement_digest, envelope_digest),
            (protocol_id, compiled),
        ),
    ) in typed_envelopes
        .iter()
        .zip(PrivacyProtocolIdV1::ALL.into_iter().zip(compiled_rows))
        .enumerate()
    {
        let canonical_variant = protocol_id.canonical_typed_variant_label();
        if *label != protocol_id.canonical_label()
            || *statement_variant != canonical_variant
            || *proof_variant != canonical_variant
            || compiled.protocol_id != protocol_id
            || compiled.statement_variant != *statement_variant
            || compiled.proof_variant != *proof_variant
            || compiled.statement_digest != *statement_digest
            || compiled.envelope_sha256 != *envelope_digest
        {
            return Err(format!(
                "exact12 typed-envelope row {expected_index} does not match current compiled types and canonical Norito"
            )
            .into());
        }
    }
    let statement_digests = typed_envelopes
        .iter()
        .map(|(_, _, _, digest, _)| *digest)
        .collect::<BTreeSet<_>>();
    let envelope_digests = typed_envelopes
        .iter()
        .map(|(_, _, _, _, digest)| *digest)
        .collect::<BTreeSet<_>>();
    if statement_digests.len() != PrivacyProtocolIdV1::COUNT
        || envelope_digests.len() != PrivacyProtocolIdV1::COUNT
    {
        return Err("exact12 typed statement and envelope digests must each be unique".into());
    }
    if retired.as_slice() != PRIVACY_RETIRED_PROTOCOL_LABELS_V1 {
        return Err(
            "exact12 retired rows do not exactly match the closed reserved-label order".into(),
        );
    }
    if bytes != generated.as_slice() {
        return Err(
            "exact12 matrix is not byte-identical to the compiled canonical generator".into(),
        );
    }
    Ok(())
}

fn parse_nonzero_sha256(value: &str, label: &str) -> Result<[u8; 32], DynError> {
    let digest = parse_sha256(value)?;
    if digest == [0; 32] {
        return Err(format!("{label} must not be the zero placeholder").into());
    }
    Ok(digest)
}

fn parse_canonical_usize(value: &str) -> Result<usize, DynError> {
    if value != "0" && (value.starts_with('0') || !value.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return Err("exact12 protocol index is not canonical unsigned decimal".into());
    }
    value
        .parse::<usize>()
        .map_err(|_| "exact12 protocol index exceeds usize".into())
}

fn canonical_norito_bytes<T>(value: &T, label: &str) -> Result<Vec<u8>, DynError>
where
    T: PartialEq + norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    let bytes = norito::encode_canonical(value)
        .map_err(|error| format!("cannot encode {label}: {error}"))?;
    let decoded: T =
        norito::decode_canonical_with_limits(&bytes, artifact_decode_limits(bytes.len()))
            .map_err(|error| format!("cannot canonical-roundtrip {label}: {error}"))?;
    if &decoded != value {
        return Err(format!("{label} changed value during canonical Norito roundtrip").into());
    }
    Ok(bytes)
}

fn decode_canonical_norito<T>(bytes: &[u8], maximum_bytes: u64, label: &str) -> Result<T, DynError>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    enforce_encoded_size(bytes.len(), maximum_bytes, label)?;
    let value = norito::decode_canonical_with_limits(bytes, artifact_decode_limits(bytes.len()))
        .map_err(|error| format!("{label} is not bounded canonical Norito: {error}"))?;
    let canonical = norito::encode_canonical(&value)
        .map_err(|error| format!("cannot re-encode typed {label}: {error}"))?;
    if canonical != bytes {
        return Err(format!("{label} is not its exact canonical typed Norito re-encoding").into());
    }
    Ok(value)
}

fn canonical_json_bytes<T>(value: &T, label: &str) -> Result<Vec<u8>, DynError>
where
    T: PartialEq + norito::json::JsonSerialize + norito::json::JsonDeserialize,
{
    let mut json = norito::json::to_json_pretty(value)
        .map_err(|error| format!("cannot encode {label} JSON: {error}"))?;
    json.push('\n');
    let decoded: T = norito::json::from_str(&json)
        .map_err(|error| format!("cannot typed-roundtrip {label} JSON: {error}"))?;
    if &decoded != value {
        return Err(format!("{label} changed value during typed JSON roundtrip").into());
    }
    Ok(json.into_bytes())
}

fn decode_canonical_json<T>(bytes: &[u8], label: &str) -> Result<T, DynError>
where
    T: PartialEq + norito::json::JsonSerialize + norito::json::JsonDeserialize,
{
    let text =
        std::str::from_utf8(bytes).map_err(|_| format!("{label} must be canonical UTF-8 JSON"))?;
    let value: T = norito::json::from_str(text)
        .map_err(|error| format!("cannot decode typed {label}: {error}"))?;
    let canonical = canonical_json_bytes(&value, label)?;
    if canonical != bytes {
        return Err(format!("{label} is not the deterministic typed JSON projection").into());
    }
    Ok(value)
}

fn load_typed_pair<T>(
    norito_path: &Path,
    max_norito_bytes: u64,
    json_path: &Path,
    max_json_bytes: u64,
    label: &str,
) -> Result<(T, Vec<u8>, Vec<u8>), DynError>
where
    T: PartialEq
        + norito::NoritoSerialize
        + norito::json::JsonSerialize
        + norito::json::JsonDeserialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    let norito_input = secure_read(norito_path, max_norito_bytes, &format!("{label} Norito"))?;
    let json_input = secure_read(json_path, max_json_bytes, &format!("{label} JSON"))?;
    if norito_input.identity == json_input.identity {
        return Err(format!("{label} Norito and JSON paths alias one inode").into());
    }
    let authoritative: T = decode_canonical_norito(&norito_input.bytes, max_norito_bytes, label)?;
    let projection: T = decode_canonical_json(&json_input.bytes, label)?;
    if authoritative != projection {
        return Err(format!("{label} JSON is not typed-equal to authoritative Norito").into());
    }
    Ok((authoritative, norito_input.bytes, json_input.bytes))
}

fn artifact_decode_limits(payload_len: usize) -> DecodeLimits {
    // Canonical decoding validates and then deterministically re-encodes the
    // complete governed enclosure. Use Norito's payload-derived cumulative
    // allocation budget for that exact operation; a local multiplier would
    // undercount aligned archive copies and owned nested proof sequences.
    let canonical_limits = norito::canonical_decode_limits(payload_len);
    let maximum_proof_sequence = usize::try_from(PRIVACY_RELEASE_MAX_PROOF_ARTIFACT_BYTES_V1)
        .expect("Taira proof-artifact cap fits usize");
    let maximum_sequence_elements = payload_len.min(maximum_proof_sequence);
    let maximum_total_elements = payload_len.saturating_add(RELEASE_DECODE_STRUCTURAL_ELEMENTS_V1);
    DecodeLimits::new(
        maximum_sequence_elements,
        payload_len,
        maximum_total_elements,
        canonical_limits.max_total_allocated_bytes(),
        32,
    )
}

fn secure_anonymous_stage_file(label: &str) -> Result<File, DynError> {
    let file = tempfile::tempfile()
        .map_err(|error| format!("cannot create anonymous {label} file: {error}"))?;
    // SAFETY: `file` is a newly owned anonymous regular descriptor.
    if unsafe { libc::fchmod(file.as_raw_fd(), 0o600) } != 0 {
        return Err(format!(
            "cannot set exact mode on anonymous {label}: {}",
            std::io::Error::last_os_error()
        )
        .into());
    }
    let metadata = file
        .metadata()
        .map_err(|error| format!("cannot stat anonymous {label}: {error}"))?;
    validate_anonymous_stage_metadata(&metadata, 0, label)?;
    Ok(file)
}

fn validate_anonymous_stage_metadata(
    metadata: &Metadata,
    maximum: u64,
    label: &str,
) -> Result<(), DynError> {
    if !metadata.file_type().is_file()
        || metadata.nlink() != 0
        || metadata.mode() & 0o777 != 0o600
        || metadata.len() > maximum
    {
        return Err(format!(
            "anonymous {label} must be one 0600 regular descriptor within {maximum} bytes"
        )
        .into());
    }
    Ok(())
}

fn anonymous_file_len(file: &File, label: &str) -> Result<u64, DynError> {
    let metadata = file
        .metadata()
        .map_err(|error| format!("cannot stat anonymous {label}: {error}"))?;
    if !metadata.file_type().is_file() || metadata.nlink() != 0 || metadata.mode() & 0o777 != 0o600
    {
        return Err(format!("anonymous {label} descriptor identity changed").into());
    }
    Ok(metadata.len())
}

fn read_bounded_anonymous_file(
    file: &mut File,
    maximum: u64,
    label: &str,
) -> Result<Vec<u8>, DynError> {
    let before = file
        .metadata()
        .map_err(|error| format!("cannot stat anonymous {label}: {error}"))?;
    validate_anonymous_stage_metadata(&before, maximum, label)?;
    let identity = file_identity(&before);
    file.seek(SeekFrom::Start(0))
        .map_err(|error| format!("cannot rewind anonymous {label}: {error}"))?;
    let capacity = usize::try_from(before.len()).map_err(|_| format!("{label} exceeds usize"))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(capacity)
        .map_err(|_| format!("cannot reserve bounded anonymous {label} buffer"))?;
    Read::by_ref(file)
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read anonymous {label}: {error}"))?;
    if u64::try_from(bytes.len()).map_err(|_| format!("{label} length exceeds u64"))? > maximum {
        return Err(format!("anonymous {label} exceeds {maximum} bytes").into());
    }
    let after = file
        .metadata()
        .map_err(|error| format!("cannot restat anonymous {label}: {error}"))?;
    if file_identity(&after) != identity
        || after.len() != before.len()
        || after.mode() != before.mode()
        || after.nlink() != before.nlink()
        || after.mtime() != before.mtime()
        || after.mtime_nsec() != before.mtime_nsec()
        || after.ctime() != before.ctime()
        || after.ctime_nsec() != before.ctime_nsec()
        || u64::try_from(bytes.len()).ok() != Some(after.len())
    {
        return Err(format!("anonymous {label} changed while it was read").into());
    }
    Ok(bytes)
}

fn secure_write_anonymous_stage_result(fd: RawFd, bytes: &[u8]) -> Result<(), DynError> {
    // SAFETY: `fd` is the one inherited, parent-created anonymous result
    // descriptor and ownership transfers to this hidden child.
    let mut file = unsafe { File::from_raw_fd(fd) };
    let before = file
        .metadata()
        .map_err(|error| format!("cannot stat inherited child-result descriptor: {error}"))?;
    validate_anonymous_stage_metadata(&before, 0, "child stage result")?;
    let identity = file_identity(&before);
    file.write_all(bytes)
        .map_err(|error| format!("cannot write child stage result: {error}"))?;
    file.sync_all()
        .map_err(|error| format!("cannot sync child stage result: {error}"))?;
    let after = file
        .metadata()
        .map_err(|error| format!("cannot restat child stage result: {error}"))?;
    if !after.file_type().is_file()
        || file_identity(&after) != identity
        || after.nlink() != 0
        || after.mode() & 0o777 != 0o600
        || after.len() != u64::try_from(bytes.len()).map_err(|_| "result length exceeds u64")?
    {
        return Err("inherited child-result descriptor changed during write".into());
    }
    Ok(())
}

fn secure_read(path: &Path, maximum: u64, label: &str) -> Result<SecureInputV1, DynError> {
    let mut file = open_release_input(path, label)?;
    let before = file
        .metadata()
        .map_err(|error| format!("cannot stat opened {label}: {error}"))?;
    validate_regular_metadata(&before, maximum, label)?;
    let identity = file_identity(&before);
    let capacity = usize::try_from(before.len()).map_err(|_| format!("{label} exceeds usize"))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(capacity)
        .map_err(|_| format!("cannot reserve bounded buffer for {label}"))?;
    Read::by_ref(&mut file)
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read {label}: {error}"))?;
    if u64::try_from(bytes.len()).map_err(|_| format!("{label} length exceeds u64"))? > maximum {
        return Err(format!("{label} exceeds the {maximum}-byte ceiling").into());
    }
    let after = file
        .metadata()
        .map_err(|error| format!("cannot restat opened {label}: {error}"))?;
    if file_identity(&after) != identity
        || after.len() != before.len()
        || after.mode() != before.mode()
        || after.nlink() != before.nlink()
        || after.mtime() != before.mtime()
        || after.mtime_nsec() != before.mtime_nsec()
        || after.ctime() != before.ctime()
        || after.ctime_nsec() != before.ctime_nsec()
        || u64::try_from(bytes.len()).ok() != Some(after.len())
    {
        return Err(format!("{label} changed while it was read").into());
    }
    Ok(SecureInputV1 {
        sha256: sha256_bytes(&bytes),
        bytes,
        identity,
    })
}

fn secure_hash(path: &Path, maximum: u64, label: &str) -> Result<HashedInputV1, DynError> {
    let mut file = open_release_input(path, label)?;
    let before = file
        .metadata()
        .map_err(|error| format!("cannot stat opened {label}: {error}"))?;
    validate_regular_metadata(&before, maximum, label)?;
    let identity = file_identity(&before);
    let (digest, total) = sha256_reader_bounded(&mut file, maximum)
        .map_err(|error| format!("cannot hash {label}: {error}"))?;
    let after = file
        .metadata()
        .map_err(|error| format!("cannot restat opened {label}: {error}"))?;
    if file_identity(&after) != identity
        || after.len() != before.len()
        || after.mode() != before.mode()
        || after.nlink() != before.nlink()
        || after.mtime() != before.mtime()
        || after.mtime_nsec() != before.mtime_nsec()
        || after.ctime() != before.ctime()
        || after.ctime_nsec() != before.ctime_nsec()
        || total != after.len()
    {
        return Err(format!("{label} changed while it was hashed").into());
    }
    Ok(HashedInputV1 {
        sha256: digest,
        identity,
        mode: after.mode(),
    })
}

fn validate_regular_metadata(
    metadata: &Metadata,
    maximum: u64,
    label: &str,
) -> Result<(), DynError> {
    if !metadata.file_type().is_file() {
        return Err(format!("{label} is not a regular file").into());
    }
    if metadata.len() == 0 {
        return Err(format!("{label} must not be empty").into());
    }
    if metadata.len() > maximum {
        return Err(format!("{label} exceeds the {maximum}-byte ceiling").into());
    }
    if metadata.nlink() != 1 {
        return Err(Box::new(SecureInputErrorV1 {
            class: SecureInputErrorClassV1::ExternalHardLinkAlias,
            label: label.to_owned(),
            observed_links: metadata.nlink(),
        }));
    }
    Ok(())
}

fn file_identity(metadata: &Metadata) -> FileIdentityV1 {
    FileIdentityV1 {
        device: metadata.dev(),
        inode: metadata.ino(),
    }
}

fn generate_output_paths(outputs: &GenerateOutputsV1) -> Vec<PathBuf> {
    vec![
        outputs.command_manifest_norito.clone(),
        outputs.command_manifest_json.clone(),
        outputs.stage_artifacts_norito.clone(),
        outputs.stage_artifacts_json.clone(),
        outputs.receipt_norito.clone(),
        outputs.receipt_json.clone(),
    ]
}

fn verify_artifact_paths(artifacts: &VerifyArtifactsV1) -> Vec<PathBuf> {
    vec![
        artifacts.command_manifest_norito.clone(),
        artifacts.command_manifest_json.clone(),
        artifacts.stage_artifacts_norito.clone(),
        artifacts.stage_artifacts_json.clone(),
        artifacts.receipt_norito.clone(),
        artifacts.receipt_json.clone(),
    ]
}

fn preflight_output_paths(paths: &[PathBuf]) -> Result<(), DynError> {
    if paths.is_empty() {
        return Err("output path set is empty".into());
    }
    reject_lexical_path_aliases(paths)?;
    for path in paths {
        let absolute = absolute_normalized(path)?;
        let parent = absolute
            .parent()
            .ok_or("output path has no parent directory")?;
        reject_symlink_ancestors(parent, true)?;
        let parent_metadata = fs::symlink_metadata(parent).map_err(|error| {
            format!("cannot inspect output parent {}: {error}", parent.display())
        })?;
        if parent_metadata.file_type().is_symlink() || !parent_metadata.file_type().is_dir() {
            return Err(format!(
                "output parent must be a non-symlink directory: {}",
                parent.display()
            )
            .into());
        }
        match fs::symlink_metadata(&absolute) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(
                    format!("cannot inspect output {}: {error}", absolute.display()).into(),
                );
            }
            Ok(_) => {
                return Err(format!(
                    "refusing to overwrite existing output {}",
                    absolute.display()
                )
                .into());
            }
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn open_output_parent_directory(path: &Path) -> Result<File, DynError> {
    let absolute = absolute_normalized(path)?;
    if absolute == Path::new("/") {
        let flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW;
        let how = libc::open_how {
            flags: u64::try_from(flags).map_err(|_| "output root flags are negative")?,
            mode: 0,
            resolve: libc::RESOLVE_BENEATH
                | libc::RESOLVE_NO_MAGICLINKS
                | libc::RESOLVE_NO_SYMLINKS,
        };
        // SAFETY: the static root path is NUL-terminated and the flags request
        // one path-only directory anchor.
        let root_fd = unsafe {
            libc::open(
                c"/".as_ptr(),
                libc::O_PATH | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            )
        };
        if root_fd < 0 {
            return Err(format!(
                "cannot open output root anchor: {}",
                std::io::Error::last_os_error()
            )
            .into());
        }
        // SAFETY: `root_fd` is live, `.` is a NUL-terminated relative
        // component, and `how` has the kernel ABI layout.
        let opened = unsafe {
            libc::syscall(
                libc::SYS_openat2,
                root_fd,
                c".".as_ptr(),
                &how,
                std::mem::size_of::<libc::open_how>(),
            )
        };
        let open_error = (opened < 0).then(std::io::Error::last_os_error);
        // SAFETY: `root_fd` is uniquely owned by this function.
        let close_result = unsafe { libc::close(root_fd) };
        if close_result != 0 {
            if let Ok(opened_fd) = i32::try_from(opened)
                && opened_fd >= 0
            {
                // SAFETY: a successful openat2 result is uniquely owned here.
                let _ = unsafe { libc::close(opened_fd) };
            }
            return Err(format!(
                "cannot close output root anchor: {}",
                std::io::Error::last_os_error()
            )
            .into());
        }
        if opened < 0 {
            return Err(format!(
                "cannot open output root through anchored openat2: {}",
                open_error.expect("negative openat2 result captured an error")
            )
            .into());
        }
        let opened =
            i32::try_from(opened).map_err(|_| "output root descriptor exceeds positive i32")?;
        // SAFETY: successful `openat2` returned one newly owned descriptor.
        return Ok(unsafe { File::from_raw_fd(opened) });
    }
    linux_open_absolute(
        &absolute,
        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        0,
        "output parent directory",
    )
}

#[cfg(target_os = "linux")]
fn verify_output_parent_anchor_live(anchor: &OutputParentAnchorV1) -> Result<(), DynError> {
    let metadata = anchor.directory.metadata().map_err(|error| {
        format!(
            "cannot stat held output parent {}: {error}",
            anchor.absolute_path.display()
        )
    })?;
    if !metadata.file_type().is_dir() || file_identity(&metadata) != anchor.identity {
        return Err(format!(
            "held output parent identity changed: {}",
            anchor.absolute_path.display()
        )
        .into());
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn verify_output_parent_anchor_named(anchor: &OutputParentAnchorV1) -> Result<(), DynError> {
    verify_output_parent_anchor_live(anchor)?;
    let named = open_output_parent_directory(&anchor.absolute_path).map_err(|error| {
        format!(
            "cannot re-open named output parent {}: {error}",
            anchor.absolute_path.display()
        )
    })?;
    let metadata = named.metadata().map_err(|error| {
        format!(
            "cannot stat named output parent {}: {error}",
            anchor.absolute_path.display()
        )
    })?;
    if !metadata.file_type().is_dir() || file_identity(&metadata) != anchor.identity {
        return Err(format!(
            "named output parent no longer has its anchored identity: {}",
            anchor.absolute_path.display()
        )
        .into());
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn output_entry_facts(
    anchor: &OutputParentAnchorV1,
    target: &OutputTargetV1,
) -> Result<Option<OutputEntryFactsV1>, DynError> {
    verify_output_parent_anchor_live(anchor)?;
    let mut storage = MaybeUninit::<libc::stat>::uninit();
    // SAFETY: the held directory and NUL-terminated basename are live, and
    // `storage` points to sufficient writable storage for one `stat`.
    let result = unsafe {
        libc::fstatat(
            anchor.directory.as_raw_fd(),
            target.basename.as_ptr(),
            storage.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if result != 0 {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ENOENT) {
            return Ok(None);
        }
        return Err(format!(
            "cannot inspect anchored output {}: {error}",
            target.absolute_path.display()
        )
        .into());
    }
    // SAFETY: successful `fstatat` initialized the complete structure.
    let stat = unsafe { storage.assume_init() };
    if stat.st_size < 0 {
        return Err(format!(
            "anchored output has a negative length: {}",
            target.absolute_path.display()
        )
        .into());
    }
    Ok(Some(OutputEntryFactsV1 {
        identity: FileIdentityV1 {
            device: stat.st_dev as u64,
            inode: stat.st_ino as u64,
        },
        mode: stat.st_mode,
        link_count: stat.st_nlink as u64,
        length: stat.st_size as u64,
    }))
}

#[cfg(target_os = "linux")]
fn prepare_output_plan(
    paths: &[PathBuf],
) -> Result<(Vec<OutputParentAnchorV1>, Vec<OutputTargetV1>), DynError> {
    if paths.is_empty() {
        return Err("output path set is empty".into());
    }
    reject_lexical_path_aliases(paths)?;
    let mut anchors = Vec::<OutputParentAnchorV1>::new();
    let mut parent_indices = BTreeMap::<PathBuf, usize>::new();
    let mut parent_identities = BTreeMap::<FileIdentityV1, PathBuf>::new();
    let mut targets = Vec::<OutputTargetV1>::with_capacity(paths.len());

    for path in paths {
        let absolute = absolute_normalized(path)?;
        let parent = absolute
            .parent()
            .ok_or("output path has no parent directory")?
            .to_path_buf();
        let parent_index = if let Some(index) = parent_indices.get(&parent) {
            *index
        } else {
            let directory = open_output_parent_directory(&parent)?;
            let metadata = directory.metadata().map_err(|error| {
                format!("cannot stat output parent {}: {error}", parent.display())
            })?;
            if !metadata.file_type().is_dir() {
                return Err(
                    format!("output parent is not a directory: {}", parent.display()).into(),
                );
            }
            let identity = file_identity(&metadata);
            if let Some(previous) = parent_identities.insert(identity, parent.clone()) {
                return Err(format!(
                    "output parent inode aliases are forbidden: {} and {}",
                    previous.display(),
                    parent.display()
                )
                .into());
            }
            let index = anchors.len();
            anchors.push(OutputParentAnchorV1 {
                absolute_path: parent.clone(),
                directory,
                identity,
            });
            parent_indices.insert(parent, index);
            index
        };
        let basename = absolute
            .file_name()
            .ok_or_else(|| {
                format!(
                    "output path must name a file beneath its immediate parent: {}",
                    absolute.display()
                )
            })?
            .as_bytes();
        if basename.is_empty() || basename == b"." || basename == b".." || basename.contains(&b'/')
        {
            return Err(format!(
                "output basename is not a single normal component: {}",
                absolute.display()
            )
            .into());
        }
        let basename = CString::new(basename)
            .map_err(|_| format!("output basename contains NUL: {}", absolute.display()))?;
        targets.push(OutputTargetV1 {
            absolute_path: absolute,
            parent_index,
            basename,
        });
    }

    for anchor in &anchors {
        verify_output_parent_anchor_named(anchor)?;
    }
    for target in &targets {
        let anchor = &anchors[target.parent_index];
        if output_entry_facts(anchor, target)?.is_some() {
            return Err(format!(
                "refusing to overwrite existing output {}",
                target.absolute_path.display()
            )
            .into());
        }
    }
    Ok((anchors, targets))
}

#[cfg(target_os = "linux")]
fn open_output_target_create_new(
    anchor: &OutputParentAnchorV1,
    target: &OutputTargetV1,
) -> Result<File, DynError> {
    verify_output_parent_anchor_named(anchor)?;
    let flags = libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_CLOEXEC | libc::O_NOFOLLOW;
    let how = libc::open_how {
        flags: u64::try_from(flags).map_err(|_| "output creation flags are negative")?,
        mode: 0o600,
        resolve: libc::RESOLVE_BENEATH | libc::RESOLVE_NO_MAGICLINKS | libc::RESOLVE_NO_SYMLINKS,
    };
    // SAFETY: the held parent and NUL-terminated basename are live, the
    // basename is one normal component, and `how` has the kernel ABI layout.
    let opened = unsafe {
        libc::syscall(
            libc::SYS_openat2,
            anchor.directory.as_raw_fd(),
            target.basename.as_ptr(),
            &how,
            std::mem::size_of::<libc::open_how>(),
        )
    };
    if opened < 0 {
        return Err(format!(
            "cannot securely create new output {}: {}",
            target.absolute_path.display(),
            std::io::Error::last_os_error()
        )
        .into());
    }
    let opened = i32::try_from(opened).map_err(|_| "output descriptor exceeds positive i32")?;
    // SAFETY: successful `openat2` returned one newly owned descriptor.
    Ok(unsafe { File::from_raw_fd(opened) })
}

#[cfg(target_os = "linux")]
fn validate_created_output_metadata(
    metadata: &Metadata,
    expected_identity: FileIdentityV1,
    expected_length: u64,
    path: &Path,
) -> Result<(), DynError> {
    if !metadata.file_type().is_file()
        || file_identity(metadata) != expected_identity
        || metadata.nlink() != 1
        || metadata.mode() & 0o7777 != 0o600
        || metadata.len() != expected_length
    {
        return Err(format!(
            "created output does not retain its exact identity, 0600 mode, link count, and length: {}",
            path.display()
        )
        .into());
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn validate_created_output_entry(
    anchor: &OutputParentAnchorV1,
    target: &OutputTargetV1,
    expected_identity: FileIdentityV1,
    expected_length: u64,
) -> Result<(), DynError> {
    let facts = output_entry_facts(anchor, target)?.ok_or_else(|| {
        format!(
            "created output disappeared from its anchored parent: {}",
            target.absolute_path.display()
        )
    })?;
    if facts.mode & libc::S_IFMT != libc::S_IFREG
        || facts.identity != expected_identity
        || facts.link_count != 1
        || facts.mode & 0o7777 != 0o600
        || facts.length != expected_length
    {
        return Err(format!(
            "anchored output entry changed after creation: {}",
            target.absolute_path.display()
        )
        .into());
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn remove_output_entry_if_identity(
    anchor: &OutputParentAnchorV1,
    target: &OutputTargetV1,
    expected_identity: FileIdentityV1,
) -> bool {
    let Ok(Some(facts)) = output_entry_facts(anchor, target) else {
        return false;
    };
    if facts.mode & libc::S_IFMT != libc::S_IFREG || facts.identity != expected_identity {
        return false;
    }
    // SAFETY: the held parent and NUL-terminated basename are live; the entry
    // was just checked without following links and matches our created inode.
    unsafe { libc::unlinkat(anchor.directory.as_raw_fd(), target.basename.as_ptr(), 0) == 0 }
}

#[cfg(target_os = "linux")]
fn secure_write_create_new(
    anchors: &[OutputParentAnchorV1],
    target: &OutputTargetV1,
    target_index: usize,
    bytes: &[u8],
) -> Result<CreatedOutputV1, DynError> {
    let anchor = &anchors[target.parent_index];
    let mut file = open_output_target_create_new(anchor, target)?;
    let created = file.metadata().map_err(|error| {
        format!(
            "cannot stat created output {}: {error}",
            target.absolute_path.display()
        )
    })?;
    let identity = file_identity(&created);
    let expected_length = u64::try_from(bytes.len()).map_err(|_| "output length exceeds u64")?;
    let result = (|| -> Result<(), DynError> {
        // SAFETY: `file` is the newly owned regular output descriptor.
        if unsafe { libc::fchmod(file.as_raw_fd(), 0o600) } != 0 {
            return Err(format!(
                "cannot set exact output mode for {}: {}",
                target.absolute_path.display(),
                std::io::Error::last_os_error()
            )
            .into());
        }
        validate_created_output_metadata(
            &file.metadata().map_err(|error| {
                format!(
                    "cannot restat created output {}: {error}",
                    target.absolute_path.display()
                )
            })?,
            identity,
            0,
            &target.absolute_path,
        )?;
        file.write_all(bytes).map_err(|error| {
            format!(
                "cannot write output {}: {error}",
                target.absolute_path.display()
            )
        })?;
        file.sync_all().map_err(|error| {
            format!(
                "cannot sync output {}: {error}",
                target.absolute_path.display()
            )
        })?;
        validate_created_output_metadata(
            &file.metadata().map_err(|error| {
                format!(
                    "cannot stat synced output {}: {error}",
                    target.absolute_path.display()
                )
            })?,
            identity,
            expected_length,
            &target.absolute_path,
        )?;
        validate_created_output_entry(anchor, target, identity, expected_length)?;
        verify_output_parent_anchor_named(anchor)
    })();
    if let Err(error) = result {
        drop(file);
        let _ = remove_output_entry_if_identity(anchor, target, identity);
        let _ = anchor.directory.sync_all();
        return Err(error);
    }
    Ok(CreatedOutputV1 {
        target_index,
        file,
        identity,
        expected_length,
    })
}

#[cfg(target_os = "linux")]
fn rollback_created_outputs(
    anchors: &[OutputParentAnchorV1],
    targets: &[OutputTargetV1],
    created: &[CreatedOutputV1],
) {
    let mut touched_parents = BTreeSet::<usize>::new();
    for output in created.iter().rev() {
        let target = &targets[output.target_index];
        let anchor = &anchors[target.parent_index];
        if remove_output_entry_if_identity(anchor, target, output.identity) {
            touched_parents.insert(target.parent_index);
        }
    }
    for parent_index in touched_parents {
        let anchor = &anchors[parent_index];
        if verify_output_parent_anchor_live(anchor).is_ok() {
            let _ = anchor.directory.sync_all();
        }
    }
}

#[cfg(target_os = "linux")]
fn write_artifact_set_create_new(artifacts: &[(&Path, &[u8])]) -> Result<(), DynError> {
    let paths = artifacts
        .iter()
        .map(|(path, _)| (*path).to_path_buf())
        .collect::<Vec<_>>();
    let (anchors, targets) = prepare_output_plan(&paths)?;
    let mut created = Vec::<CreatedOutputV1>::with_capacity(artifacts.len());
    for (target_index, ((_, bytes), target)) in artifacts.iter().zip(&targets).enumerate() {
        match secure_write_create_new(&anchors, target, target_index, bytes) {
            Ok(output) => created.push(output),
            Err(error) => {
                rollback_created_outputs(&anchors, &targets, &created);
                return Err(error);
            }
        }
    }

    let commit = (|| -> Result<(), DynError> {
        for output in &created {
            let target = &targets[output.target_index];
            let anchor = &anchors[target.parent_index];
            validate_created_output_metadata(
                &output.file.metadata().map_err(|error| {
                    format!(
                        "cannot restat open output {}: {error}",
                        target.absolute_path.display()
                    )
                })?,
                output.identity,
                output.expected_length,
                &target.absolute_path,
            )?;
            validate_created_output_entry(anchor, target, output.identity, output.expected_length)?;
        }
        for anchor in &anchors {
            verify_output_parent_anchor_named(anchor)?;
            anchor.directory.sync_all().map_err(|error| {
                format!(
                    "cannot sync held output parent {}: {error}",
                    anchor.absolute_path.display()
                )
            })?;
        }
        for anchor in &anchors {
            verify_output_parent_anchor_named(anchor)?;
        }
        for output in &created {
            let target = &targets[output.target_index];
            let anchor = &anchors[target.parent_index];
            validate_created_output_entry(anchor, target, output.identity, output.expected_length)?;
        }
        Ok(())
    })();
    if let Err(error) = commit {
        rollback_created_outputs(&anchors, &targets, &created);
        return Err(error);
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn secure_create_new_file(path: &Path) -> Result<File, DynError> {
    let absolute = absolute_normalized(path)?;
    let file = open_release_output_create_new(&absolute, "output file")?;
    let created = file
        .metadata()
        .map_err(|error| format!("cannot stat created output {}: {error}", absolute.display()))?;
    let identity = file_identity(&created);
    // SAFETY: `file` is the newly owned regular output descriptor.
    if unsafe { libc::fchmod(file.as_raw_fd(), 0o600) } != 0 {
        let chmod_error = std::io::Error::last_os_error();
        drop(file);
        remove_output_if_identity(&absolute, identity);
        return Err(format!(
            "cannot fix output mode for {}: {}",
            absolute.display(),
            chmod_error
        )
        .into());
    }
    let fixed = file.metadata().map_err(|error| {
        format!(
            "cannot restat created output {}: {error}",
            absolute.display()
        )
    })?;
    if !fixed.file_type().is_file()
        || file_identity(&fixed) != identity
        || fixed.nlink() != 1
        || fixed.mode() & 0o7777 != 0o600
        || fixed.len() != 0
    {
        drop(file);
        remove_output_if_identity(&absolute, identity);
        return Err(format!(
            "created output {} does not have the exact regular-file identity and 0600 mode",
            absolute.display()
        )
        .into());
    }
    Ok(file)
}

#[cfg(not(target_os = "linux"))]
fn secure_write_create_new(path: &Path, bytes: &[u8]) -> Result<FileIdentityV1, DynError> {
    let mut file = secure_create_new_file(path)?;
    let created_metadata = file
        .metadata()
        .map_err(|error| format!("cannot stat output {}: {error}", path.display()))?;
    let created_identity = file_identity(&created_metadata);
    let result = (|| -> Result<FileIdentityV1, DynError> {
        file.write_all(bytes)
            .map_err(|error| format!("cannot write output {}: {error}", path.display()))?;
        file.sync_all()
            .map_err(|error| format!("cannot sync output {}: {error}", path.display()))?;
        let metadata = file
            .metadata()
            .map_err(|error| format!("cannot stat output {}: {error}", path.display()))?;
        if !metadata.file_type().is_file()
            || file_identity(&metadata) != created_identity
            || metadata.nlink() != 1
            || metadata.mode() & 0o7777 != 0o600
            || metadata.len()
                != u64::try_from(bytes.len()).map_err(|_| "output length exceeds u64")?
        {
            return Err(format!("output {} changed during secure write", path.display()).into());
        }
        Ok(created_identity)
    })();
    drop(file);
    if result.is_err() {
        remove_output_if_identity(path, created_identity);
    }
    result
}

#[cfg(not(target_os = "linux"))]
fn write_artifact_set_create_new(artifacts: &[(&Path, &[u8])]) -> Result<(), DynError> {
    let paths = artifacts
        .iter()
        .map(|(path, _)| (*path).to_path_buf())
        .collect::<Vec<_>>();
    preflight_output_paths(&paths)?;
    let mut created = Vec::<(PathBuf, FileIdentityV1)>::new();
    for (path, bytes) in artifacts {
        match secure_write_create_new(path, bytes) {
            Ok(identity) => created.push(((*path).to_path_buf(), identity)),
            Err(error) => {
                rollback_created_outputs(&created);
                return Err(error);
            }
        }
    }
    let parent_paths = paths
        .iter()
        .filter_map(|path| absolute_normalized(path).ok())
        .filter_map(|path| path.parent().map(Path::to_path_buf))
        .collect::<BTreeSet<_>>();
    for parent in parent_paths {
        let sync_result = (|| -> Result<(), DynError> {
            let directory = File::open(&parent).map_err(|error| {
                format!("cannot open output parent {}: {error}", parent.display())
            })?;
            directory.sync_all().map_err(|error| {
                format!("cannot sync output parent {}: {error}", parent.display()).into()
            })
        })();
        if let Err(error) = sync_result {
            rollback_created_outputs(&created);
            return Err(error);
        }
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn rollback_created_outputs(created: &[(PathBuf, FileIdentityV1)]) {
    for (path, expected_identity) in created.iter().rev() {
        let Ok(metadata) = fs::symlink_metadata(path) else {
            continue;
        };
        if !metadata.file_type().is_symlink()
            && metadata.file_type().is_file()
            && file_identity(&metadata) == *expected_identity
        {
            let _ = fs::remove_file(path);
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn remove_output_if_identity(path: &Path, expected_identity: FileIdentityV1) {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return;
    };
    if !metadata.file_type().is_symlink()
        && metadata.file_type().is_file()
        && file_identity(&metadata) == expected_identity
    {
        let _ = fs::remove_file(path);
    }
}

fn reject_lexical_path_aliases(paths: &[PathBuf]) -> Result<(), DynError> {
    let mut normalized = BTreeSet::new();
    for path in paths {
        let absolute = absolute_normalized(path)?;
        if !normalized.insert(absolute.clone()) {
            return Err(format!(
                "duplicate or lexically aliased path: {}",
                absolute.display()
            )
            .into());
        }
    }
    Ok(())
}

fn reject_existing_inode_aliases(paths: &[PathBuf]) -> Result<(), DynError> {
    let mut identities = BTreeMap::<FileIdentityV1, PathBuf>::new();
    for path in paths {
        let absolute = absolute_normalized(path)?;
        let metadata = match fs::symlink_metadata(&absolute) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(format!("cannot inspect input {}: {error}", absolute.display()).into());
            }
        };
        if metadata.file_type().is_symlink() {
            return Err(
                format!("symlink input/output is forbidden: {}", absolute.display()).into(),
            );
        }
        if metadata.file_type().is_file() {
            let identity = file_identity(&metadata);
            if let Some(previous) = identities.insert(identity, absolute.clone()) {
                return Err(format!(
                    "hard-link aliases are forbidden: {} and {}",
                    previous.display(),
                    absolute.display()
                )
                .into());
            }
        }
    }
    Ok(())
}

fn absolute_normalized(path: &Path) -> Result<PathBuf, DynError> {
    if path.as_os_str().is_empty() {
        return Err("empty filesystem path".into());
    }
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()
            .map_err(|error| format!("cannot resolve current directory: {error}"))?
            .join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::RootDir | Component::Prefix(_) | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(
                        format!("path escapes filesystem root: {}", absolute.display()).into(),
                    );
                }
            }
        }
    }
    if !normalized.is_absolute() {
        return Err("normalized path is not absolute".into());
    }
    Ok(normalized)
}

fn reject_symlink_ancestors(path: &Path, include_leaf: bool) -> Result<(), DynError> {
    let absolute = absolute_normalized(path)?;
    let mut current = PathBuf::new();
    let components = absolute.components().collect::<Vec<_>>();
    let limit = if include_leaf {
        components.len()
    } else {
        components.len().saturating_sub(1)
    };
    for component in components.into_iter().take(limit) {
        current.push(component.as_os_str());
        let metadata = fs::symlink_metadata(&current).map_err(|error| {
            format!(
                "cannot inspect path component {}: {error}",
                current.display()
            )
        })?;
        if metadata.file_type().is_symlink() {
            return Err(
                format!("symlink path component is forbidden: {}", current.display()).into(),
            );
        }
    }
    Ok(())
}

fn enforce_encoded_size(observed: usize, maximum: u64, label: &str) -> Result<(), DynError> {
    let observed = u64::try_from(observed).map_err(|_| format!("{label} size exceeds u64"))?;
    if observed == 0 || observed > maximum {
        return Err(format!("{label} size {observed} is outside 1..={maximum}").into());
    }
    Ok(())
}

fn parse_sha256(value: &str) -> Result<[u8; 32], DynError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("SHA-256 must be exactly 64 lowercase hexadecimal characters".into());
    }
    let mut digest = [0_u8; 32];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        digest[index] = (hex_nibble(chunk[0])? << 4) | hex_nibble(chunk[1])?;
    }
    Ok(digest)
}

fn hex_nibble(byte: u8) -> Result<u8, DynError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err("invalid lowercase hexadecimal digit".into()),
    }
}

fn hex_sha256(digest: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in digest {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn sha256_bytes(bytes: &[u8]) -> [u8; 32] {
    sha256(bytes)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::symlink;

    use super::*;

    #[cfg(all(
        target_os = "linux",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    const STACK_LIMIT_CHILD_MARKER_V1: &str = "IROHA_PRIVACY_RELEASE_STACK_LIMIT_CHILD_MODE_V1";

    fn exact12_bytes() -> Vec<u8> {
        privacy_exact12_matrix_bytes_v1().expect("generate compiled exact12 matrix")
    }

    fn checked_in_exact12_bytes() -> Vec<u8> {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/privacy/exact12_v1.tsv");
        fs::read(path).expect("read exact12 fixture")
    }

    fn canonical_expectations_v1() -> PrivacyReleaseExpectationsV1 {
        let stages = canonical_stage_coordinates()
            .expect("frozen stage declaration")
            .iter()
            .copied()
            .map(|coordinate| {
                let PrivacyReleaseStageCoordinateV1 {
                    protocol_id,
                    case_kind,
                    ..
                } = coordinate;
                let expected_failure = match case_kind {
                    PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd
                    | PrivacyReleaseCaseKindV1::MaximumShapeResource => {
                        PrivacyReleaseFailureClassV1::NotApplicable
                    }
                    PrivacyReleaseCaseKindV1::PublicStatementBindingMutation => {
                        PrivacyReleaseFailureClassV1::PublicStatementBindingRejected
                    }
                    PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation => {
                        PrivacyReleaseFailureClassV1::CanonicalWireCorruptionAndTruncationRejected
                    }
                };
                let original_statement = sha256_bytes(protocol_id.canonical_label().as_bytes());
                let statement = if case_kind
                    == PrivacyReleaseCaseKindV1::PublicStatementBindingMutation
                {
                    sha256_bytes(format!("{}-mutated", protocol_id.canonical_label()).as_bytes())
                } else if case_kind == PrivacyReleaseCaseKindV1::MaximumShapeResource {
                    sha256_bytes(format!("{}-maximum", protocol_id.canonical_label()).as_bytes())
                } else {
                    original_statement
                };
                let proof_artifacts =
                    (0..privacy_release_proof_artifact_count_v1(protocol_id, case_kind))
                        .map(|artifact_ordinal| {
                            let canonical_proof_bytes = format!(
                                "{}-{}-{artifact_ordinal}",
                                protocol_id.canonical_label(),
                                case_kind.canonical_label()
                            )
                            .into_bytes();
                            PrivacyReleaseProofArtifactEvidenceV1 {
                                artifact_ordinal,
                                proof_sha256: sha256_bytes(&canonical_proof_bytes),
                                canonical_proof_bytes,
                                proof_bytes_ceiling: privacy_release_proof_artifact_ceiling_v1(
                                    protocol_id,
                                    case_kind,
                                    artifact_ordinal,
                                )
                                .expect("closed fixture artifact has one canonical ceiling"),
                            }
                        })
                        .collect();
                let (max_elapsed_millis, max_peak_rss_bytes, max_address_space_bytes) =
                    privacy_release_process_profile_v1(protocol_id).map_or(
                        (60_000, 1024 * 1024 * 1024, 4 * 1024 * 1024 * 1024),
                        |profile| {
                            (
                                profile.elapsed_ceiling_millis,
                                profile.peak_rss_ceiling_bytes,
                                profile.address_space_ceiling_bytes,
                            )
                        },
                    );
                PrivacyReleaseExpectedStageV1 {
                    evidence: PrivacyReleaseStageEvidenceV1 {
                        schema_version: PRIVACY_RELEASE_EVIDENCE_SCHEMA_VERSION_V1,
                        stage_ordinal: privacy_release_stage_ordinal_v1(protocol_id, case_kind),
                        protocol_id,
                        case_kind,
                        protocol_descriptor: privacy_release_protocol_descriptor_v1(protocol_id)
                            .to_owned(),
                        public_statement_sha256: statement,
                        proof_artifacts,
                        failure_class: expected_failure,
                        resources: privacy_release_resource_facts_v1(protocol_id, case_kind)
                            .expect("every exact-12 release stage has canonical resource facts"),
                    },
                    max_elapsed_millis,
                    max_peak_rss_bytes,
                    max_address_space_bytes,
                }
            })
            .collect();
        PrivacyReleaseExpectationsV1 {
            schema_version: ARTIFACT_SCHEMA_VERSION_V1,
            stage_count: u16::try_from(PRIVACY_RELEASE_STAGE_COUNT_V1).unwrap(),
            stages,
        }
    }

    fn refresh_artifact_hash(artifact: &mut PrivacyReleaseProofArtifactEvidenceV1) {
        artifact.proof_sha256 = sha256_bytes(&artifact.canonical_proof_bytes);
    }

    fn measured_from_expectations(
        expectations: &PrivacyReleaseExpectationsV1,
    ) -> Vec<MeasuredStageV1> {
        expectations
            .stages
            .iter()
            .map(|stage| MeasuredStageV1 {
                evidence: stage.evidence.clone(),
                elapsed_millis: 1,
                peak_rss_bytes: MIN_STAGE_PEAK_RSS_BYTES,
                peak_address_space_bytes: MIN_STAGE_ADDRESS_SPACE_BYTES,
            })
            .collect()
    }

    fn captured_resource_certificate_v1(
        expectations: &PrivacyReleaseExpectationsV1,
    ) -> (
        iroha_core::privacy_release_evidence::PrivacyReleaseZkX509ResourceCertificateV1,
        Vec<u8>,
        Vec<u8>,
    ) {
        let measured = measured_from_expectations(expectations);
        let measurements = resource_certificate::capture_measurements_v1(&measured)
            .expect("canonical X.509 capture measurements");
        let norito = canonical_norito_bytes(expectations, "test expectations")
            .expect("canonical expectation Norito");
        let json = canonical_json_bytes(expectations, "test expectations")
            .expect("canonical expectation JSON");
        let artifacts = resource_certificate::build_capture_artifacts_v1(
            measurements,
            &norito,
            &json,
            iroha_core::privacy_release_evidence::privacy_release_zk_x509_resource_environment_v1(),
        )
        .expect("canonical resource certificate");
        let certificate = decode_canonical_norito(
            &artifacts.norito,
            64 * 1024,
            "test X.509 resource certificate",
        )
        .expect("typed resource certificate");
        (certificate, norito, json)
    }

    #[test]
    fn capture_validation_binds_resource_certificate_to_exact_expectations() {
        let expectations = canonical_expectations_v1();
        let (certificate, norito, json) = captured_resource_certificate_v1(&expectations);
        resource_certificate::validate_capture_expectation_binding_v1(
            &certificate,
            &expectations,
            sha256_bytes(&norito),
            sha256_bytes(&json),
        )
        .expect("resource certificate binds the exact expectation pair");

        assert!(
            resource_certificate::validate_capture_expectation_binding_v1(
                &certificate,
                &expectations,
                [0xA5; 32],
                sha256_bytes(&json),
            )
            .is_err()
        );

        let mut substituted_kat = expectations;
        let ordinal = usize::from(privacy_release_stage_ordinal_v1(
            PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
            PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd,
        ));
        let artifact = &mut substituted_kat.stages[ordinal].evidence.proof_artifacts[0];
        artifact.canonical_proof_bytes.push(0xA5);
        refresh_artifact_hash(artifact);
        assert!(
            resource_certificate::validate_capture_expectation_binding_v1(
                &certificate,
                &substituted_kat,
                sha256_bytes(&norito),
                sha256_bytes(&json),
            )
            .is_err()
        );
    }

    #[test]
    fn proc_status_memory_parser_accepts_only_exact_complete_peak_fields() {
        let sampled = parse_process_status_memory_v1(
            b"Name:\tstage\nVmPeak:\t65536 kB\nVmHWM:\t1024 kB\nThreads:\t1\n",
        )
        .expect("parse canonical Linux status memory fields");
        assert_eq!(
            sampled,
            SampledProcessMemoryV1 {
                peak_rss_bytes: 1024 * 1024,
                peak_address_space_bytes: 65536 * 1024,
            }
        );

        let malformed = [
            b"VmPeak:\t65536 kB\n".as_slice(),
            b"VmHWM:\t1024 kB\n".as_slice(),
            b"VmHWM:\t1024 kB\nVmHWM:\t1025 kB\nVmPeak:\t65536 kB\n".as_slice(),
            b"VmHWM:\t1024 kB\nVmPeak:\t65536 kB\nVmPeak:\t65537 kB\n".as_slice(),
            b"VmHWM:\t1024 KB\nVmPeak:\t65536 kB\n".as_slice(),
            b"VmHWM:\t1024 kB\nVmPeak:\t65536 bytes\n".as_slice(),
            b"VmHWM:\t1024 kB extra\nVmPeak:\t65536 kB\n".as_slice(),
            b"VmHWM:\t-1 kB\nVmPeak:\t65536 kB\n".as_slice(),
            b"VmHWM:\t+1 kB\nVmPeak:\t65536 kB\n".as_slice(),
            b"VmHWM:\t01 kB\nVmPeak:\t65536 kB\n".as_slice(),
            b"VmHWM:\t18446744073709551616 kB\nVmPeak:\t65536 kB\n".as_slice(),
            b"VmHWM:\t18446744073709551615 kB\nVmPeak:\t65536 kB\n".as_slice(),
            b"VmHWM:\t1024 kB\nVmPeak:\t18446744073709551616 kB\n".as_slice(),
            b"VmHWM:\t1024 kB\nVmPeak:\t18446744073709551615 kB\n".as_slice(),
            b"VmHWM 1024 kB\nVmPeak:\t65536 kB\n".as_slice(),
            b"VmHWM:\t1024 kB\nVmPeak 65536 kB\n".as_slice(),
            b"VmHWM:\t1024 kB\nVmPeak:\t65536 kB\n\xff".as_slice(),
        ];
        for status in malformed {
            assert!(
                parse_process_status_memory_v1(status).is_err(),
                "malformed status unexpectedly parsed: {status:?}"
            );
        }
        assert!(
            parse_process_status_memory_v1(&vec![b'x'; 1024 * 1024 + 1]).is_err(),
            "oversized /proc status must fail closed"
        );
    }

    #[test]
    fn measured_and_persisted_peak_address_space_is_nonzero_and_bounded() {
        let expectations = canonical_expectations_v1();
        let measured = measured_from_expectations(&expectations);
        validate_measured_against_expectations(&measured, &expectations)
            .expect("canonical process peaks validate");

        for value in [
            0,
            expectations.stages[0]
                .max_address_space_bytes
                .checked_add(1)
                .unwrap(),
        ] {
            let mut malformed = measured.clone();
            malformed[0].peak_address_space_bytes = value;
            assert!(validate_measured_against_expectations(&malformed, &expectations).is_err());
        }

        let artifacts = PrivacyReleaseStageArtifactsV1 {
            schema_version: ARTIFACT_SCHEMA_VERSION_V1,
            stage_count: u16::try_from(PRIVACY_RELEASE_STAGE_COUNT_V1).unwrap(),
            stages: measured
                .iter()
                .map(|stage| PrivacyReleaseMeasuredStageV1 {
                    evidence: stage.evidence.clone(),
                    elapsed_millis: stage.elapsed_millis,
                    peak_rss_bytes: stage.peak_rss_bytes,
                    peak_address_space_bytes: stage.peak_address_space_bytes,
                })
                .collect(),
        };
        validate_stage_artifacts(&artifacts, &expectations)
            .expect("canonical persisted process peaks validate");
        let encoded = canonical_norito_bytes(&artifacts, "measured-stage VmPeak fixture").unwrap();
        let decoded: PrivacyReleaseStageArtifactsV1 = decode_canonical_norito(
            &encoded,
            u64::try_from(encoded.len()).unwrap(),
            "measured-stage VmPeak fixture",
        )
        .unwrap();
        assert_eq!(decoded, artifacts);
        assert_eq!(
            decoded.stages[0].peak_address_space_bytes,
            MIN_STAGE_ADDRESS_SPACE_BYTES
        );
        for value in [
            0,
            expectations.stages[0]
                .max_address_space_bytes
                .checked_add(1)
                .unwrap(),
        ] {
            let mut malformed = artifacts.clone();
            malformed.stages[0].peak_address_space_bytes = value;
            assert!(validate_stage_artifacts(&malformed, &expectations).is_err());
        }
    }

    #[test]
    fn frozen_stage_declaration_rejects_omission_duplication_reorder_and_coordinate_drift() {
        let canonical = PRIVACY_RELEASE_STAGE_COORDINATES_V1.to_vec();
        assert!(validate_privacy_release_stage_coordinates_v1(&canonical));

        let mut malformed = Vec::new();

        let mut omitted = canonical.clone();
        omitted.pop();
        malformed.push(omitted);

        let mut duplicated = canonical.clone();
        duplicated[1] = duplicated[0];
        malformed.push(duplicated);

        let mut reordered = canonical.clone();
        reordered.swap(0, 1);
        malformed.push(reordered);

        let mut ordinal_drift = canonical.clone();
        ordinal_drift[0].stage_ordinal = 1;
        malformed.push(ordinal_drift);

        let mut protocol_substitution = canonical.clone();
        protocol_substitution[0].protocol_id = PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1;
        malformed.push(protocol_substitution);

        let mut case_substitution = canonical;
        case_substitution[0].case_kind = PrivacyReleaseCaseKindV1::MaximumShapeResource;
        malformed.push(case_substitution);

        for coordinates in malformed {
            assert!(!validate_privacy_release_stage_coordinates_v1(&coordinates));
        }
    }

    #[test]
    fn expectation_schedule_rejects_omission_duplication_reorder_and_coordinate_substitution() {
        let canonical = canonical_expectations_v1();
        validate_expectation_stage_coordinates_v1(&canonical)
            .expect("canonical expectation schedule");

        let mut malformed = Vec::new();

        let mut omitted = canonical.clone();
        omitted.stages.pop();
        omitted.stage_count -= 1;
        malformed.push(omitted);

        let mut duplicated = canonical.clone();
        duplicated.stages[1] = duplicated.stages[0].clone();
        malformed.push(duplicated);

        let mut reordered = canonical.clone();
        reordered.stages.swap(0, 1);
        malformed.push(reordered);

        let mut ordinal_drift = canonical.clone();
        ordinal_drift.stages[0].evidence.stage_ordinal = 1;
        malformed.push(ordinal_drift);

        let mut protocol_substitution = canonical.clone();
        protocol_substitution.stages[0].evidence.protocol_id =
            PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1;
        malformed.push(protocol_substitution);

        let mut case_substitution = canonical;
        case_substitution.stages[0].evidence.case_kind =
            PrivacyReleaseCaseKindV1::MaximumShapeResource;
        malformed.push(case_substitution);

        for expectations in malformed {
            assert!(validate_expectation_stage_coordinates_v1(&expectations).is_err());
        }
    }

    #[test]
    fn coordinated_resource_fact_and_ceiling_substitution_rejects_at_comparison() {
        let mut expectations = canonical_expectations_v1();
        let mut measured = measured_from_expectations(&expectations);
        validate_measured_against_expectations(&measured, &expectations)
            .expect("canonical measured/expectation comparison");

        let index = usize::from(privacy_release_stage_ordinal_v1(
            PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
            PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd,
        ));
        let resources = &mut expectations.stages[index].evidence.resources;
        resources.primary_units = resources
            .primary_units
            .checked_add(1)
            .expect("fixture resource fact increment");
        resources.primary_ceiling = resources
            .primary_ceiling
            .checked_add(1)
            .expect("fixture resource ceiling increment");
        assert!(resources.primary_units <= resources.primary_ceiling);
        measured[index].evidence.resources = *resources;
        assert_eq!(
            measured[index].evidence, expectations.stages[index].evidence,
            "coordinated substitution defeats equality alone"
        );
        assert!(validate_expectations(&expectations).is_err());
        assert!(validate_measured_against_expectations(&measured, &expectations).is_err());
    }

    #[test]
    fn capture_uses_total_frozen_resources_for_every_exact12_stage() {
        for coordinate in PRIVACY_RELEASE_STAGE_COORDINATES_V1 {
            let provisional = empty_expected_evidence(coordinate.protocol_id, coordinate.case_kind);
            let expected =
                privacy_release_resource_facts_v1(coordinate.protocol_id, coordinate.case_kind)
                    .expect("every exact-12 release stage has canonical resource facts");
            assert_eq!(provisional.resources, expected);
            validate_resource_facts(
                &provisional.resources,
                coordinate.protocol_id,
                coordinate.case_kind,
                usize::from(provisional.stage_ordinal),
            )
            .expect("canonical stage resource facts validate");
        }
    }

    #[test]
    fn checked_in_exact12_artifact_is_derived_from_the_compiled_generator() {
        let generated = exact12_bytes();
        let checked_in = checked_in_exact12_bytes();
        assert_eq!(checked_in, generated);
        assert_eq!(sha256_bytes(&checked_in), sha256_bytes(&generated));
    }

    #[test]
    fn exact12_parser_accepts_only_the_closed_order_and_registry_digest() {
        let canonical = exact12_bytes();
        validate_exact12_matrix_structure(&canonical).expect("canonical exact12 structure");
        validate_exact12_matrix(&canonical).expect("canonical exact12");
        let canonical_text = String::from_utf8(canonical.clone()).unwrap();

        let reordered = canonical_text.replacen(
            "protocol\t0\tzk-ace-pq-authorization-v0",
            "protocol\t0\tanonymous-pgc-k-out-of-n-v1",
            1,
        );
        assert!(validate_exact12_matrix_structure(reordered.as_bytes()).is_err());

        let wrong_digest = canonical_text.replacen(
            "registry-sha256\t734eafb58f0c54f5319b9cc26557920e564453f689071931393dcdba91123e51",
            "registry-sha256\t0000000000000000000000000000000000000000000000000000000000000000",
            1,
        );
        assert!(validate_exact12_matrix_structure(wrong_digest.as_bytes()).is_err());

        let crlf = canonical
            .iter()
            .flat_map(|byte| {
                if *byte == b'\n' {
                    vec![b'\r', b'\n']
                } else {
                    vec![*byte]
                }
            })
            .collect::<Vec<_>>();
        assert!(validate_exact12_matrix_structure(&crlf).is_err());
    }

    #[test]
    fn exact12_parser_rejects_typed_route_retirement_and_shape_substitutions() {
        let canonical = String::from_utf8(exact12_bytes()).unwrap();
        let typed_row = canonical
            .lines()
            .find(|line| line.starts_with("typed-envelope\tzk-ace-pq-authorization-v0\t"))
            .expect("typed row")
            .to_owned();
        let second_typed_row = canonical
            .lines()
            .find(|line| line.starts_with("typed-envelope\tanonymous-pgc-k-out-of-n-v1\t"))
            .expect("second typed row")
            .to_owned();
        let first_statement_digest = typed_row
            .split('\t')
            .nth(4)
            .expect("first statement digest");
        let second_statement_digest = second_typed_row
            .split('\t')
            .nth(4)
            .expect("second statement digest");
        let first_header = canonical.lines().next().expect("canonical header row");
        let retired_row = "retired\tsis-with-hints";
        let mut substituted_digest = first_statement_digest.as_bytes().to_vec();
        substituted_digest[0] = if substituted_digest[0] == b'0' {
            b'1'
        } else {
            b'0'
        };
        let substituted_digest =
            String::from_utf8(substituted_digest).expect("canonical digest remains UTF-8");
        let valid_looking_digest_substitution =
            canonical.replacen(first_statement_digest, &substituted_digest, 1);

        let mutations = [
            canonical.replacen(first_header, "# attacker-defined parity matrix", 1),
            canonical.replacen(&format!("{first_header}\n"), "", 1),
            canonical.replacen(
                "matrix-version\t1\n",
                "matrix-version\t1\n# late extension row\n",
                1,
            ),
            canonical.replacen(
                "matrix-version\t1\nregistry-sha256\t734eafb58f0c54f5319b9cc26557920e564453f689071931393dcdba91123e51\n",
                "registry-sha256\t734eafb58f0c54f5319b9cc26557920e564453f689071931393dcdba91123e51\nmatrix-version\t1\n",
                1,
            ),
            canonical.replacen(
                "ZkAcePqAuthorizationV0\tZkAcePqAuthorizationV0",
                "AttackerStatement\tZkAcePqAuthorizationV0",
                1,
            ),
            canonical.replacen(
                "typed-envelope\tzk-ace-pq-authorization-v0\tZkAcePqAuthorizationV0",
                "typed-envelope\tanonymous-pgc-k-out-of-n-v1\tZkAcePqAuthorizationV0",
                1,
            ),
            canonical.replacen(
                first_statement_digest,
                "0000000000000000000000000000000000000000000000000000000000000000",
                1,
            ),
            valid_looking_digest_substitution,
            canonical.replacen(second_statement_digest, first_statement_digest, 1),
            canonical.replacen(&format!("{typed_row}\n"), "", 1),
            canonical.replacen(
                &format!("{typed_row}\n{second_typed_row}\n"),
                &format!("{second_typed_row}\n{typed_row}\n"),
                1,
            ),
            canonical.replacen(
                &format!("{typed_row}\n"),
                &format!("{typed_row}\n{typed_row}\n"),
                1,
            ),
            canonical.replacen(retired_row, "retired\tsis-with-hints-alias", 1),
            canonical.replacen(
                &format!("{retired_row}\n"),
                &format!("{retired_row}\n{retired_row}\n"),
                1,
            ),
            canonical.replacen("matrix-version\t1\n", "matrix-version\t1\n\n", 1),
            canonical.replacen(
                "protocol\t0\tzk-ace-pq-authorization-v0\t",
                "protocol\t0\tzk-ace-pq-authorization-v0\textra\t",
                1,
            ),
        ];
        for (index, mutation) in mutations.into_iter().enumerate() {
            assert!(
                validate_exact12_matrix_structure(mutation.as_bytes()).is_err(),
                "matrix mutation {index} must fail closed"
            );
        }
    }

    #[test]
    fn exact_pid_wait4_does_not_consume_an_unrelated_exited_child() {
        let mut unrelated = Command::new("/bin/sh")
            .args(["-c", "exit 9"])
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn unrelated wait fixture");
        let target = Command::new("/bin/sh")
            .args(["-c", "sleep 0.05; exit 7"])
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .expect("spawn exact-PID wait fixture");
        let mut target = StageChildGuardV1::new(target).expect("own exact child");
        let started = Instant::now();
        let waited = loop {
            if let Some(waited) = target.try_wait4().expect("exact-PID wait4") {
                break waited;
            }
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "bounded target child must exit"
            );
            thread::sleep(Duration::from_millis(1));
        };
        assert_eq!(waited.status.code(), Some(7));
        assert!(waited.peak_rss_bytes > 0);
        assert!(target.reaped);
        assert_eq!(
            unrelated
                .wait()
                .expect("unrelated child remains independently waitable")
                .code(),
            Some(9)
        );
    }

    #[test]
    fn exact_pid_wait4_reports_complete_child_lifetime_and_reaps() {
        let child = Command::new("/bin/sh")
            .args(["-c", "exit 7"])
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .expect("spawn bounded wait4 fixture");
        let mut guard = StageChildGuardV1::new(child).expect("own exact child");
        let started = Instant::now();
        let waited = loop {
            if let Some(waited) = guard.try_wait4().expect("exact-PID wait4") {
                break waited;
            }
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "bounded child must exit"
            );
            thread::sleep(Duration::from_millis(1));
        };
        assert_eq!(waited.status.code(), Some(7));
        assert!(waited.peak_rss_bytes > 0);
        assert!(guard.reaped);
    }

    #[test]
    fn child_guard_drop_kills_and_reaps_on_early_error_paths() {
        let child = Command::new("/bin/sleep")
            .arg("30")
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .expect("spawn cleanup fixture");
        let guard = StageChildGuardV1::new(child).expect("own cleanup fixture");
        let pid_raw = guard.pid_raw();
        drop(guard);
        // SAFETY: signal zero only probes the just-reaped PID.
        let observed = unsafe { libc::kill(pid_raw, 0) };
        assert_eq!(observed, -1);
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ESRCH)
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn immutable_runner_is_digest_identical_anonymous_and_write_sealed() {
        let live = open_live_process_executable().expect("open live test executable");
        let live_len = live.metadata().expect("stat live test executable").len();
        if let Err(error) = validate_static_release_elf_v1(&live, live_len) {
            let error = error.to_string();
            assert!(
                error.contains("PT_INTERP") || error.contains("DT_NEEDED"),
                "a non-release test binary must fail only the static-runtime contract: {error}"
            );
            let preparation_error = prepare_immutable_runner()
                .err()
                .expect("dynamic test binary must be refused")
                .to_string();
            assert!(
                preparation_error.contains(&error),
                "immutable preparation returned a different error: {preparation_error}"
            );
            return;
        }
        let runner = prepare_immutable_runner().expect("prepare sealed exact executable");
        let source = secure_hash(
            &runner.source_path,
            MAX_EXECUTABLE_BYTES,
            "test runner source",
        )
        .expect("hash source executable");
        assert_eq!(runner.sha256, source.sha256);
        assert_eq!(runner.source_identity, source.identity);
        let fd = runner.executable.as_raw_fd();
        assert_eq!(
            immutable_runner_exec_path(fd).unwrap(),
            PathBuf::from(format!("/proc/self/fd/{fd}"))
        );
        // SAFETY: F_GET_SEALS is a read-only query on the owned memfd.
        let seals = unsafe { libc::fcntl(fd, libc::F_GET_SEALS) };
        let required = libc::F_SEAL_WRITE
            | libc::F_SEAL_GROW
            | libc::F_SEAL_SHRINK
            | libc::F_SEAL_EXEC
            | libc::F_SEAL_SEAL;
        assert!(seals >= 0);
        assert_eq!(seals & required, required);
        // SAFETY: the deliberate write attack targets only the private sealed
        // fixture and must be rejected by the kernel.
        let written = unsafe { libc::pwrite(fd, [0xA5_u8].as_ptr().cast(), 1, 0 as libc::off_t) };
        assert_eq!(written, -1);
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::EPERM)
        );
    }

    #[test]
    fn sha256_parser_rejects_noncanonical_or_ambiguous_text() {
        assert_eq!(parse_sha256(&"ab".repeat(32)).unwrap(), [0xab; 32]);
        for invalid in [
            "AB".repeat(32),
            format!("{} ", "ab".repeat(32)),
            "ab".repeat(31),
            "gg".repeat(32),
        ] {
            assert!(parse_sha256(&invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn common_inputs_bind_nonzero_source_and_the_compiled_build_profile() {
        let compiled = if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        };
        let opposite = if compiled == "debug" {
            "release"
        } else {
            "debug"
        };
        let mut options = BTreeMap::from([
            ("build-profile".to_owned(), compiled.to_owned()),
            ("source-sha256".to_owned(), "11".repeat(32)),
            ("exact12-matrix".to_owned(), "/exact12".to_owned()),
            (
                "expectations-norito".to_owned(),
                "/expectations.norito".to_owned(),
            ),
            (
                "expectations-json".to_owned(),
                "/expectations.json".to_owned(),
            ),
            ("cargo-lock".to_owned(), "/Cargo.lock".to_owned()),
            ("validator-binary".to_owned(), "/irohad".to_owned()),
        ]);
        assert!(parse_common_inputs(&options).is_ok());

        options.insert("source-sha256".to_owned(), "00".repeat(32));
        assert!(parse_common_inputs(&options).is_err());
        options.insert("source-sha256".to_owned(), "11".repeat(32));
        options.insert("build-profile".to_owned(), opposite.to_owned());
        assert!(parse_common_inputs(&options).is_err());
    }

    #[test]
    fn option_parser_rejects_duplicates_unknowns_positionals_and_missing_values() {
        let allowed = ["one", "two"];
        let os = |values: &[&str]| values.iter().map(OsString::from).collect::<Vec<_>>();
        assert!(parse_options(&os(&["--one", "a", "--two", "b"]), &allowed).is_ok());
        assert!(parse_options(&os(&["--one", "a", "--one", "b", "--two", "c"]), &allowed).is_err());
        assert!(parse_options(&os(&["--one", "a", "--three", "b"]), &allowed).is_err());
        assert!(parse_options(&os(&["one", "a", "--two", "b"]), &allowed).is_err());
        assert!(parse_options(&os(&["--one", "a", "--two"]), &allowed).is_err());
    }

    #[test]
    fn hidden_stage_contract_requires_canonical_hard_resource_ceilings() {
        let os = |values: &[&str]| values.iter().map(OsString::from).collect::<Vec<_>>();
        let valid = os(&[
            "--protocol",
            "zk-ace-pq-authorization-v0",
            "--case",
            "positive-canonical-end-to-end",
            "--out-fd",
            "9",
            "--elapsed-ceiling-ms",
            "60000",
            "--peak-rss-ceiling-bytes",
            "1073741824",
            "--address-space-ceiling-bytes",
            "4294967296",
        ]);
        let parsed = parse_options(&valid, &stage_option_names()).expect("closed stage options");
        assert_eq!(
            canonical_u64_option(&parsed, "elapsed-ceiling-ms").unwrap(),
            60_000
        );
        assert_eq!(
            canonical_u64_option(&parsed, "peak-rss-ceiling-bytes").unwrap(),
            1_073_741_824
        );
        assert_eq!(
            canonical_u64_option(&parsed, "address-space-ceiling-bytes").unwrap(),
            4_294_967_296
        );
        assert_eq!(canonical_raw_fd_option(&parsed, "out-fd").unwrap(), 9);
        assert!(canonical_stage_result_fd_option(&parsed).is_err());
        let mut canonical_result = parsed.clone();
        canonical_result.insert("out-fd".to_owned(), "3".to_owned());
        assert_eq!(
            canonical_stage_result_fd_option(&canonical_result).unwrap(),
            CANONICAL_STAGE_RESULT_FD_V1
        );
        validate_process_ceilings(60_000, 1_073_741_824, 4_294_967_296).unwrap();
        assert!(validate_process_ceilings(60_000, 1_073_741_824, 536_870_912).is_err());
        assert!(
            validate_process_ceilings(60_000, 1_073_741_824, MAX_STAGE_ADDRESS_SPACE_BYTES + 1)
                .is_err()
        );
        let zk_x509_profile =
            privacy_release_process_profile_v1(PrivacyProtocolIdV1::IrohaZkX509StarkP256V0)
                .expect("zk-X509 has one canonical process profile");
        assert_eq!(
            checked_stage_cpu_limit_seconds_v1(1),
            Some(MAX_STAGE_TASKS_V1 + 1)
        );
        assert_eq!(
            checked_stage_cpu_limit_seconds_v1(1_000),
            Some(MAX_STAGE_TASKS_V1 + 1)
        );
        assert_eq!(
            checked_stage_cpu_limit_seconds_v1(1_001),
            Some(2 * MAX_STAGE_TASKS_V1 + 1)
        );
        assert_eq!(
            checked_stage_cpu_limit_seconds_v1(zk_x509_profile.elapsed_ceiling_millis),
            Some(300 * MAX_STAGE_TASKS_V1 + 1)
        );
        assert_eq!(
            checked_stage_cpu_limit_seconds_v1(MAX_STAGE_ELAPSED_MILLIS),
            Some(MAX_STAGE_ELAPSED_MILLIS / 1_000 * MAX_STAGE_TASKS_V1 + 1)
        );
        assert_eq!(checked_stage_cpu_limit_seconds_v1(0), None);
        assert_eq!(
            checked_stage_cpu_limit_seconds_v1(MAX_STAGE_ELAPSED_MILLIS + 1),
            None
        );
        assert_eq!(checked_stage_cpu_limit_seconds_v1(u64::MAX), None);

        let missing_hard_limits = os(&[
            "--protocol",
            "zk-ace-pq-authorization-v0",
            "--case",
            "positive-canonical-end-to-end",
            "--out-fd",
            "9",
        ]);
        assert!(parse_options(&missing_hard_limits, &stage_option_names()).is_err());

        let mut noncanonical = parsed;
        noncanonical.insert("elapsed-ceiling-ms".to_owned(), "060000".to_owned());
        assert!(canonical_u64_option(&noncanonical, "elapsed-ceiling-ms").is_err());
    }

    #[test]
    fn zk_x509_process_profile_overrides_capture_and_requires_exact_child_values() {
        let protocol_id = PrivacyProtocolIdV1::IrohaZkX509StarkP256V0;
        let profile = privacy_release_process_profile_v1(protocol_id)
            .expect("zk-X509 has one canonical process profile");
        assert_eq!(profile.elapsed_ceiling_millis, 300_000);
        assert_eq!(profile.peak_rss_ceiling_bytes, 12_884_901_888);
        assert_eq!(profile.address_space_ceiling_bytes, 34_359_738_368);

        for (generic_elapsed, generic_rss, generic_address_space) in [
            (
                MAX_STAGE_ELAPSED_MILLIS,
                MAX_STAGE_PEAK_RSS_BYTES,
                MAX_STAGE_ADDRESS_SPACE_BYTES,
            ),
            (
                profile.elapsed_ceiling_millis - 1,
                profile.peak_rss_ceiling_bytes - 1,
                profile.address_space_ceiling_bytes - 1,
            ),
            (
                profile.elapsed_ceiling_millis + 1,
                profile.peak_rss_ceiling_bytes + 1,
                profile.address_space_ceiling_bytes + 1,
            ),
        ] {
            let canonical = canonical_stage_process_ceilings_v1(
                protocol_id,
                generic_elapsed,
                generic_rss,
                generic_address_space,
            )
            .expect("capture replaces all generic values with the exact zk-X509 profile");
            assert_eq!(
                canonical,
                StageProcessCeilingsV1 {
                    elapsed_millis: profile.elapsed_ceiling_millis,
                    peak_rss_bytes: profile.peak_rss_ceiling_bytes,
                    address_space_bytes: profile.address_space_ceiling_bytes,
                }
            );
        }

        validate_stage_process_ceilings_v1(
            protocol_id,
            profile.elapsed_ceiling_millis,
            profile.peak_rss_ceiling_bytes,
            profile.address_space_ceiling_bytes,
        )
        .expect("exact zk-X509 process profile");
        for elapsed in [
            profile.elapsed_ceiling_millis - 1,
            profile.elapsed_ceiling_millis + 1,
        ] {
            assert!(
                validate_stage_process_ceilings_v1(
                    protocol_id,
                    elapsed,
                    profile.peak_rss_ceiling_bytes,
                    profile.address_space_ceiling_bytes,
                )
                .is_err()
            );
        }
        for peak_rss in [
            profile.peak_rss_ceiling_bytes - 1,
            profile.peak_rss_ceiling_bytes + 1,
        ] {
            assert!(
                validate_stage_process_ceilings_v1(
                    protocol_id,
                    profile.elapsed_ceiling_millis,
                    peak_rss,
                    profile.address_space_ceiling_bytes,
                )
                .is_err()
            );
        }
        for address_space in [
            profile.address_space_ceiling_bytes - 1,
            profile.address_space_ceiling_bytes + 1,
        ] {
            assert!(
                validate_stage_process_ceilings_v1(
                    protocol_id,
                    profile.elapsed_ceiling_millis,
                    profile.peak_rss_ceiling_bytes,
                    address_space,
                )
                .is_err()
            );
        }
        assert!(
            validate_stage_process_ceilings_v1(protocol_id, u64::MAX, u64::MAX, u64::MAX,).is_err()
        );

        let other_protocol = canonical_stage_process_ceilings_v1(
            PrivacyProtocolIdV1::ZkAcePqAuthorizationV0,
            MAX_STAGE_ELAPSED_MILLIS,
            MAX_STAGE_PEAK_RSS_BYTES,
            MAX_STAGE_ADDRESS_SPACE_BYTES,
        )
        .expect("generic limits remain available to other protocols");
        assert_eq!(other_protocol.elapsed_millis, MAX_STAGE_ELAPSED_MILLIS);
        assert_eq!(other_protocol.peak_rss_bytes, MAX_STAGE_PEAK_RSS_BYTES);
        assert_eq!(
            other_protocol.address_space_bytes,
            MAX_STAGE_ADDRESS_SPACE_BYTES
        );
    }

    #[test]
    fn hidden_stage_options_reject_both_directions_around_the_zk_x509_profile() {
        let protocol_id = PrivacyProtocolIdV1::IrohaZkX509StarkP256V0;
        let profile = privacy_release_process_profile_v1(protocol_id)
            .expect("zk-X509 has one canonical process profile");
        let exact = BTreeMap::from([
            (
                "elapsed-ceiling-ms".to_owned(),
                profile.elapsed_ceiling_millis.to_string(),
            ),
            (
                "peak-rss-ceiling-bytes".to_owned(),
                profile.peak_rss_ceiling_bytes.to_string(),
            ),
            (
                "address-space-ceiling-bytes".to_owned(),
                profile.address_space_ceiling_bytes.to_string(),
            ),
        ]);
        assert_eq!(
            hidden_stage_process_ceilings_v1(protocol_id, &exact)
                .expect("exact hidden-stage profile"),
            StageProcessCeilingsV1 {
                elapsed_millis: profile.elapsed_ceiling_millis,
                peak_rss_bytes: profile.peak_rss_ceiling_bytes,
                address_space_bytes: profile.address_space_ceiling_bytes,
            }
        );

        for elapsed in [
            profile.elapsed_ceiling_millis - 1,
            profile.elapsed_ceiling_millis + 1,
        ] {
            let mut mutated = exact.clone();
            mutated.insert("elapsed-ceiling-ms".to_owned(), elapsed.to_string());
            assert!(hidden_stage_process_ceilings_v1(protocol_id, &mutated).is_err());
        }
        for peak_rss in [
            profile.peak_rss_ceiling_bytes - 1,
            profile.peak_rss_ceiling_bytes + 1,
        ] {
            let mut mutated = exact.clone();
            mutated.insert("peak-rss-ceiling-bytes".to_owned(), peak_rss.to_string());
            assert!(hidden_stage_process_ceilings_v1(protocol_id, &mutated).is_err());
        }
        for address_space in [
            profile.address_space_ceiling_bytes - 1,
            profile.address_space_ceiling_bytes + 1,
        ] {
            let mut mutated = exact.clone();
            mutated.insert(
                "address-space-ceiling-bytes".to_owned(),
                address_space.to_string(),
            );
            assert!(hidden_stage_process_ceilings_v1(protocol_id, &mutated).is_err());
        }
    }

    #[test]
    fn parent_stage_entry_rejects_both_directions_around_the_zk_x509_profile() {
        let protocol_id = PrivacyProtocolIdV1::IrohaZkX509StarkP256V0;
        let profile = privacy_release_process_profile_v1(protocol_id)
            .expect("zk-X509 has one canonical process profile");
        let runner = ImmutableRunnerV1 {
            executable: File::open("/dev/null").expect("open inert runner fixture"),
            source_path: PathBuf::from("/dev/null"),
            source_identity: FileIdentityV1 {
                device: 0,
                inode: 0,
            },
            sha256: [0; 32],
        };
        for (elapsed, peak_rss, address_space) in [
            (
                profile.elapsed_ceiling_millis - 1,
                profile.peak_rss_ceiling_bytes,
                profile.address_space_ceiling_bytes,
            ),
            (
                profile.elapsed_ceiling_millis + 1,
                profile.peak_rss_ceiling_bytes,
                profile.address_space_ceiling_bytes,
            ),
            (
                profile.elapsed_ceiling_millis,
                profile.peak_rss_ceiling_bytes - 1,
                profile.address_space_ceiling_bytes,
            ),
            (
                profile.elapsed_ceiling_millis,
                profile.peak_rss_ceiling_bytes + 1,
                profile.address_space_ceiling_bytes,
            ),
            (
                profile.elapsed_ceiling_millis,
                profile.peak_rss_ceiling_bytes,
                profile.address_space_ceiling_bytes - 1,
            ),
            (
                profile.elapsed_ceiling_millis,
                profile.peak_rss_ceiling_bytes,
                profile.address_space_ceiling_bytes + 1,
            ),
        ] {
            let error = run_stage_child(
                &runner,
                protocol_id,
                PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd,
                elapsed,
                peak_rss,
                address_space,
            )
            .expect_err("parent must reject a noncanonical profile before spawning");
            assert!(
                error
                    .to_string()
                    .contains("does not match its canonical protocol-specific process profile")
            );
        }
    }

    #[test]
    fn expectations_require_exact_zk_x509_profile_for_each_of_its_four_stages() {
        let exact = canonical_expectations_v1();
        let profile =
            privacy_release_process_profile_v1(PrivacyProtocolIdV1::IrohaZkX509StarkP256V0)
                .expect("zk-X509 has one canonical process profile");
        for case_kind in PrivacyReleaseCaseKindV1::ALL {
            let index = usize::from(privacy_release_stage_ordinal_v1(
                PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
                case_kind,
            ));
            assert_eq!(
                exact.stages[index].max_elapsed_millis,
                profile.elapsed_ceiling_millis
            );
            assert_eq!(
                exact.stages[index].max_peak_rss_bytes,
                profile.peak_rss_ceiling_bytes
            );
            assert_eq!(
                exact.stages[index].max_address_space_bytes,
                profile.address_space_ceiling_bytes
            );
        }
        validate_expectations(&exact).expect("all four exact zk-X509 process caps");

        for case_kind in PrivacyReleaseCaseKindV1::ALL {
            let index = usize::from(privacy_release_stage_ordinal_v1(
                PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
                case_kind,
            ));
            for elapsed in [
                profile.elapsed_ceiling_millis - 1,
                profile.elapsed_ceiling_millis + 1,
            ] {
                let mut mutation = exact.clone();
                mutation.stages[index].max_elapsed_millis = elapsed;
                assert!(validate_expectations(&mutation).is_err());
            }
            for peak_rss in [
                profile.peak_rss_ceiling_bytes - 1,
                profile.peak_rss_ceiling_bytes + 1,
            ] {
                let mut mutation = exact.clone();
                mutation.stages[index].max_peak_rss_bytes = peak_rss;
                assert!(validate_expectations(&mutation).is_err());
            }
            for address_space in [
                profile.address_space_ceiling_bytes - 1,
                profile.address_space_ceiling_bytes + 1,
            ] {
                let mut mutation = exact.clone();
                mutation.stages[index].max_address_space_bytes = address_space;
                assert!(validate_expectations(&mutation).is_err());
            }
        }

        let other_index = usize::from(privacy_release_stage_ordinal_v1(
            PrivacyProtocolIdV1::ZkAcePqAuthorizationV0,
            PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd,
        ));
        let mut other_protocol = exact;
        other_protocol.stages[other_index].max_elapsed_millis = profile.elapsed_ceiling_millis + 1;
        other_protocol.stages[other_index].max_peak_rss_bytes = profile.peak_rss_ceiling_bytes + 1;
        other_protocol.stages[other_index].max_address_space_bytes = MAX_STAGE_ADDRESS_SPACE_BYTES;
        validate_expectations(&other_protocol)
            .expect("zk-X509 hard caps do not silently change other protocols");
    }

    #[test]
    fn isolation_policy_freezes_the_exact_compute_only_descriptor_boundary() {
        let policy = canonical_isolation_policy_v1();
        assert_eq!(policy.stage_rayon_threads, STAGE_RAYON_THREAD_COUNT_V1);
        assert_eq!(
            policy.stage_rayon_threads.to_string(),
            STAGE_RAYON_THREADS_V1
        );
        let stage_stack_bytes = u64::try_from(PRIVACY_RELEASE_STAGE_STACK_BYTES_V1).unwrap();
        assert_eq!(policy.main_thread_stack_bytes, stage_stack_bytes);
        assert_eq!(policy.rayon_worker_stack_bytes, stage_stack_bytes);
        assert_eq!(policy.watchdog_thread_stack_bytes, stage_stack_bytes);
        assert_eq!(policy.max_stage_tasks, MAX_STAGE_TASKS_V1);
        assert_eq!(
            MAX_STAGE_TASKS_V1,
            1 + u64::from(STAGE_RAYON_THREAD_COUNT_V1) + STAGE_WATCHDOG_THREAD_COUNT_V1
        );
        assert_eq!(policy.max_stage_open_files, 4);
        assert_eq!(CANONICAL_STAGE_RESULT_FD_V1, 3);
        assert_eq!(STAGE_TASK_DIRECTORY_FD_V1, 4);
        assert_eq!(STAGE_LANDLOCK_RULESET_FD_V1, 5);
        assert_eq!(MAX_STAGE_SETUP_OPEN_FILES_V1, MAX_STAGE_OPEN_FILES_V1 + 2);
        assert_eq!(policy.max_stage_result_file_bytes, MAX_CHILD_RESULT_BYTES);
        assert_eq!(
            policy.max_stage_diagnostic_bytes,
            MAX_CHILD_DIAGNOSTIC_BYTES
        );
        assert_eq!(policy.core_dump_bytes, 0);
        assert_eq!(policy.landlock_abi_minimum, MINIMUM_LANDLOCK_ABI_V1);
        assert!(policy.static_elf_only);
        assert!(policy.anonymous_sealed_runner);
        assert!(policy.anonymous_result_descriptor_only);
        assert!(policy.exact_environment_only);
        assert!(policy.seccomp_tsync);
    }

    #[test]
    fn hidden_stage_environment_rejects_rust_stack_override() {
        let canonical = vec![(
            OsString::from("RAYON_NUM_THREADS"),
            OsString::from(STAGE_RAYON_THREADS_V1),
        )];
        validate_hidden_stage_environment_v1(&canonical)
            .expect("canonical hidden-stage environment");

        let mut injected = canonical;
        injected.push((OsString::from("RUST_MIN_STACK"), OsString::from("1048576")));
        assert!(
            validate_hidden_stage_environment_v1(&injected).is_err(),
            "a stack-policy environment override must fail closed"
        );
    }

    #[cfg(all(
        target_os = "linux",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    #[test]
    fn pre_exec_stack_limit_fixture_child_v1() {
        let Some(mode) = env::var_os(STACK_LIMIT_CHILD_MARKER_V1) else {
            return;
        };
        if mode == "low-hard-limit" {
            let low = libc::rlimit {
                rlim_cur: 1024 * 1024,
                rlim_max: 1024 * 1024,
            };
            // SAFETY: `low` is fully initialized and this disposable child is
            // intentionally reducing only its own stack hard limit.
            assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_STACK, &low) }, 0);
            assert!(
                install_pre_exec_stage_stack_limit_v1().is_err(),
                "a low inherited hard limit must fail closed"
            );
            return;
        }
        assert_eq!(mode, "success");
        install_pre_exec_stage_stack_limit_v1().expect("install exact pre-exec stack limit");
        let (soft, hard) = getrlimit(Resource::RLIMIT_STACK).unwrap();
        let expected = rlim_t::try_from(PRIVACY_RELEASE_STAGE_STACK_BYTES_V1).unwrap();
        assert_eq!((soft, hard), (expected, expected));
    }

    #[cfg(all(
        target_os = "linux",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    #[test]
    fn pre_exec_stack_limit_is_exact_and_fails_under_low_hard_limit() {
        let executable = env::current_exe().expect("resolve runner test executable");
        for mode in ["success", "low-hard-limit"] {
            let output = Command::new(&executable)
                .arg("tests::pre_exec_stack_limit_fixture_child_v1")
                .arg("--exact")
                .arg("--nocapture")
                .env(STACK_LIMIT_CHILD_MARKER_V1, mode)
                .output()
                .expect("execute isolated stack-limit fixture");
            assert!(
                output.status.success(),
                "stack-limit fixture {mode} failed\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    #[cfg(all(
        target_os = "linux",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    #[test]
    fn post_exec_policy_has_no_descriptor_or_task_creation_escape() {
        let allowed = RELEASE_POST_EXEC_UNCONDITIONAL_SYSCALLS_V1
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        assert_eq!(
            allowed.len(),
            RELEASE_POST_EXEC_UNCONDITIONAL_SYSCALLS_V1.len(),
            "the unconditional syscall allowlist must not contain aliases"
        );
        for required in [
            libc::SYS_read,
            libc::SYS_write,
            libc::SYS_futex,
            libc::SYS_fsync,
            libc::SYS_exit_group,
        ] {
            assert!(allowed.contains(&required));
        }
        for forbidden in [
            libc::SYS_fcntl,
            libc::SYS_dup,
            libc::SYS_dup3,
            libc::SYS_openat,
            libc::SYS_memfd_create,
            libc::SYS_pipe2,
            libc::SYS_socket,
            libc::c_long::from(RELEASE_NR_FORK_V1),
            libc::c_long::from(RELEASE_NR_VFORK_V1),
            libc::c_long::from(RELEASE_NR_CLONE_V1),
            libc::c_long::from(RELEASE_NR_CLONE3_V1),
        ] {
            assert!(
                !allowed.contains(&forbidden),
                "creation syscall {forbidden} escaped the closed policy"
            );
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn procfs_task_counter_rewinds_without_allocating_another_descriptor() {
        let directory = File::open("/proc/self/task").expect("open current task directory");
        let first = count_linux_task_directory_entries_v1(directory.as_raw_fd())
            .expect("count current tasks");
        let second = count_linux_task_directory_entries_v1(directory.as_raw_fd())
            .expect("rewind and recount current tasks");
        assert!(first >= 1);
        assert!(second >= 1);
    }

    #[test]
    fn anonymous_stage_descriptors_reject_every_linked_file() {
        let anonymous = secure_anonymous_stage_file("unit-test result").unwrap();
        let anonymous_metadata = anonymous.metadata().unwrap();
        assert_eq!(anonymous_metadata.nlink(), 0);
        validate_anonymous_stage_metadata(&anonymous_metadata, 0, "unit-test result").unwrap();

        let temp = tempfile::tempdir().unwrap();
        let linked_path = temp.path().join("linked");
        let linked = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&linked_path)
            .unwrap();
        assert_eq!(linked.metadata().unwrap().nlink(), 1);
        assert!(
            validate_anonymous_stage_metadata(&linked.metadata().unwrap(), 0, "linked result")
                .is_err()
        );
        let hard_link = temp.path().join("hard-link");
        fs::hard_link(&linked_path, &hard_link).unwrap();
        assert_eq!(linked.metadata().unwrap().nlink(), 2);
        assert!(
            validate_anonymous_stage_metadata(&linked.metadata().unwrap(), 0, "linked result")
                .is_err()
        );
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[test]
    fn seccomp_rejects_x32_process_and_exec_bypasses_but_allows_threads() {
        install_pre_exec_seccomp_v1().expect("install release seccomp fixture");
        for syscall_number in [
            RELEASE_NR_CLONE_V1,
            RELEASE_NR_EXECVE_V1,
            RELEASE_NR_PRCTL_V1,
        ] {
            // SAFETY: the x32 ABI bit must be rejected by seccomp before the
            // kernel interprets either the syscall number or its absent args.
            let observed = unsafe {
                libc::syscall(i64::from(
                    syscall_number | RELEASE_FORBIDDEN_SYSCALL_ABI_MASK_V1,
                ))
            };
            assert_eq!(observed, -1);
            assert_eq!(
                std::io::Error::last_os_error().raw_os_error(),
                Some(libc::EPERM)
            );
        }
        let joined = std::thread::spawn(|| 42_u8)
            .join()
            .expect("CLONE_THREAD remains available");
        assert_eq!(joined, 42);
        // SAFETY: the filter rejects fork before any child can be created.
        assert_eq!(unsafe { libc::fork() }, -1);
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::EPERM)
        );
    }

    #[test]
    fn output_preflight_rejects_existing_symlink_and_duplicate_targets() {
        let temp = tempfile::tempdir().unwrap();
        let existing = temp.path().join("existing");
        fs::write(&existing, b"x").unwrap();
        assert!(preflight_output_paths(std::slice::from_ref(&existing)).is_err());

        let target = temp.path().join("target");
        fs::write(&target, b"x").unwrap();
        let link = temp.path().join("link");
        symlink(&target, &link).unwrap();
        assert!(preflight_output_paths(std::slice::from_ref(&link)).is_err());

        let fresh = temp.path().join("fresh");
        assert!(
            preflight_output_paths(&[fresh.clone(), fresh])
                .expect_err("duplicate output target")
                .to_string()
                .contains("alias")
        );
    }

    #[test]
    fn secure_inputs_reject_external_hard_link_aliases() {
        let temp = tempfile::tempdir().unwrap();
        // macOS exposes `/tmp` as a symlink. Resolve the test directory first
        // so this reaches the leaf metadata guard on every supported host.
        let canonical_temp = temp.path().canonicalize().unwrap();
        let source = canonical_temp.join("source");
        let alias = canonical_temp.join("alias");
        fs::write(&source, b"authenticated input").unwrap();
        fs::hard_link(&source, &alias).unwrap();
        let error = secure_read(&source, 1024, "hard-linked fixture")
            .err()
            .expect("a release input with an unenumerated hard-link alias must reject");
        let rejection = error
            .downcast_ref::<SecureInputErrorV1>()
            .expect("hard-link rejection retains its typed class");
        assert_eq!(
            rejection.class,
            SecureInputErrorClassV1::ExternalHardLinkAlias
        );
        assert_eq!(rejection.observed_links, 2);
    }

    #[test]
    fn input_alias_guard_rejects_hard_links() {
        let temp = tempfile::tempdir().unwrap();
        let first = temp.path().join("first");
        let second = temp.path().join("second");
        fs::write(&first, b"bounded input").unwrap();
        fs::hard_link(&first, &second).unwrap();
        assert!(reject_existing_inode_aliases(&[first, second]).is_err());
    }

    #[test]
    fn create_new_writer_never_overwrites_and_rolls_back_partial_set() {
        let temp = tempfile::tempdir().unwrap();
        let first = temp.path().join("first");
        let second = temp.path().join("second");
        fs::write(&second, b"sentinel").unwrap();
        assert!(
            write_artifact_set_create_new(&[
                (first.as_path(), b"one".as_slice()),
                (second.as_path(), b"two".as_slice()),
            ])
            .is_err()
        );
        assert!(!first.exists());
        assert_eq!(fs::read(second).unwrap(), b"sentinel");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn output_parent_replacement_is_detected_before_anchored_creation() {
        let temp = tempfile::tempdir().unwrap();
        let parent = temp.path().join("parent");
        let moved_parent = temp.path().join("moved-parent");
        fs::create_dir(&parent).unwrap();
        let output = parent.join("artifact");
        let (anchors, targets) =
            prepare_output_plan(std::slice::from_ref(&output)).expect("anchor original parent");

        fs::rename(&parent, &moved_parent).unwrap();
        fs::create_dir(&parent).unwrap();
        let error = secure_write_create_new(&anchors, &targets[0], 0, b"evidence")
            .err()
            .expect("replacement parent identity must reject");
        assert!(
            error.to_string().contains("anchored identity"),
            "unexpected error: {error}"
        );
        assert!(!parent.join("artifact").exists());
        assert!(!moved_parent.join("artifact").exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn anchored_output_open_cannot_escape_through_an_adversarial_basename() {
        let temp = tempfile::tempdir().unwrap();
        let parent = temp.path().join("parent");
        fs::create_dir(&parent).unwrap();
        let ordinary = parent.join("ordinary");
        let (anchors, _) =
            prepare_output_plan(std::slice::from_ref(&ordinary)).expect("anchor output parent");
        let escape = temp.path().join("escape");
        let adversarial = OutputTargetV1 {
            absolute_path: escape.clone(),
            parent_index: 0,
            basename: CString::new("../escape").unwrap(),
        };
        assert!(open_output_target_create_new(&anchors[0], &adversarial).is_err());
        assert!(!escape.exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn rollback_never_unlinks_a_replacement_inode() {
        let temp = tempfile::tempdir().unwrap();
        let output = temp.path().join("artifact");
        let displaced = temp.path().join("displaced-artifact");
        let (anchors, targets) =
            prepare_output_plan(std::slice::from_ref(&output)).expect("anchor output parent");
        let created = secure_write_create_new(&anchors, &targets[0], 0, b"owned")
            .expect("create anchored output");

        fs::rename(&output, &displaced).unwrap();
        fs::write(&output, b"replacement").unwrap();
        rollback_created_outputs(&anchors, &targets, std::slice::from_ref(&created));

        assert_eq!(fs::read(&output).unwrap(), b"replacement");
        assert_eq!(fs::read(&displaced).unwrap(), b"owned");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn anchored_rollback_removes_the_owned_exact_mode_inode() {
        let temp = tempfile::tempdir().unwrap();
        let output = temp.path().join("artifact");
        let (anchors, targets) =
            prepare_output_plan(std::slice::from_ref(&output)).expect("anchor output parent");
        let created = secure_write_create_new(&anchors, &targets[0], 0, b"owned")
            .expect("create anchored output");
        let metadata = fs::symlink_metadata(&output).unwrap();
        assert!(metadata.file_type().is_file());
        assert_eq!(metadata.mode() & 0o7777, 0o600);
        assert_eq!(metadata.nlink(), 1);
        assert_eq!(metadata.len(), 5);

        rollback_created_outputs(&anchors, &targets, std::slice::from_ref(&created));
        assert!(!output.exists());
    }

    #[test]
    fn canonical_norito_and_json_pairs_reject_trailing_and_projection_drift() {
        let value = PrivacyReleaseArtifactPairDigestV1 {
            norito_sha256: [1; 32],
            json_sha256: [2; 32],
        };
        let mut norito = canonical_norito_bytes(&value, "pair").unwrap();
        let json = canonical_json_bytes(&value, "pair").unwrap();
        assert_eq!(
            decode_canonical_norito::<PrivacyReleaseArtifactPairDigestV1>(
                &norito,
                1024 * 1024,
                "pair"
            )
            .unwrap(),
            value
        );
        norito.push(0);
        assert!(
            decode_canonical_norito::<PrivacyReleaseArtifactPairDigestV1>(
                &norito,
                1024 * 1024,
                "pair"
            )
            .is_err()
        );
        let mut noncanonical_json = json;
        noncanonical_json.splice(0..0, b" ".iter().copied());
        assert!(
            decode_canonical_json::<PrivacyReleaseArtifactPairDigestV1>(&noncanonical_json, "pair")
                .is_err()
        );
    }

    #[test]
    fn norito_decode_budget_admits_governed_proof_sequences_above_128_bytes() {
        let value = vec![0xA5_u8; 129];
        let encoded = canonical_norito_bytes(&value, "129-byte proof sequence").unwrap();
        let decoded: Vec<u8> = decode_canonical_norito(
            &encoded,
            u64::try_from(encoded.len()).unwrap(),
            "129-byte proof sequence",
        )
        .unwrap();
        assert_eq!(decoded, value);
    }

    #[test]
    fn norito_decode_budget_admits_exact_maximum_artifact_and_rejects_cap_plus_one() {
        let maximum = usize::try_from(PRIVACY_RELEASE_MAX_PROOF_ARTIFACT_BYTES_V1).unwrap();
        let valid = vec![0x5A_u8; maximum];
        let valid_encoded =
            canonical_norito_bytes(&valid, "maximum canonical proof sequence").unwrap();
        let decoded: Vec<u8> = decode_canonical_norito(
            &valid_encoded,
            u64::try_from(valid_encoded.len()).unwrap(),
            "maximum canonical proof sequence",
        )
        .unwrap();
        assert_eq!(decoded.len(), maximum);
        assert_eq!(decoded.first(), Some(&0x5A));
        assert_eq!(decoded.last(), Some(&0x5A));

        let oversized = vec![0xA6_u8; maximum + 1];
        let oversized_encoded =
            norito::encode_canonical(&oversized).expect("encode adversarial advertised length");
        assert!(
            decode_canonical_norito::<Vec<u8>>(
                &oversized_encoded,
                u64::try_from(oversized_encoded.len()).unwrap(),
                "oversized canonical proof sequence",
            )
            .is_err()
        );
    }

    #[test]
    fn norito_decode_budget_admits_two_max_artifacts_and_multi_stage_enclosures() {
        let maximum = usize::try_from(PRIVACY_RELEASE_MAX_PROOF_ARTIFACT_BYTES_V1).unwrap();
        let mut two_artifact_stage = empty_expected_evidence(
            PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
            PrivacyReleaseCaseKindV1::MaximumShapeResource,
        );
        assert_eq!(two_artifact_stage.proof_artifacts.len(), 2);
        for (ordinal, artifact) in two_artifact_stage.proof_artifacts.iter_mut().enumerate() {
            artifact
                .canonical_proof_bytes
                .resize(maximum, u8::try_from(ordinal + 1).unwrap());
            artifact.proof_sha256 = sha256_bytes(&artifact.canonical_proof_bytes);
        }
        let encoded =
            canonical_norito_bytes(&two_artifact_stage, "two maximum proof artifacts").unwrap();
        let decoded: PrivacyReleaseStageEvidenceV1 = decode_canonical_norito(
            &encoded,
            u64::try_from(encoded.len()).unwrap(),
            "two maximum proof artifacts",
        )
        .unwrap();
        assert_eq!(decoded, two_artifact_stage);
        drop(decoded);
        drop(encoded);
        drop(two_artifact_stage);

        let per_stage = 3 * 1024 * 1024;
        let stages = PrivacyProtocolIdV1::ALL
            .into_iter()
            .take(3)
            .enumerate()
            .map(|(index, protocol_id)| {
                let mut evidence = empty_expected_evidence(
                    protocol_id,
                    PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd,
                );
                evidence.proof_artifacts[0]
                    .canonical_proof_bytes
                    .resize(per_stage, u8::try_from(index + 1).unwrap());
                evidence.proof_artifacts[0].proof_sha256 =
                    sha256_bytes(&evidence.proof_artifacts[0].canonical_proof_bytes);
                PrivacyReleaseMeasuredStageV1 {
                    evidence,
                    elapsed_millis: 1,
                    peak_rss_bytes: MIN_STAGE_PEAK_RSS_BYTES,
                    peak_address_space_bytes: MIN_STAGE_ADDRESS_SPACE_BYTES,
                }
            })
            .collect::<Vec<_>>();
        let aggregate = PrivacyReleaseStageArtifactsV1 {
            schema_version: ARTIFACT_SCHEMA_VERSION_V1,
            stage_count: u16::try_from(stages.len()).unwrap(),
            stages,
        };
        let encoded =
            canonical_norito_bytes(&aggregate, "multi-stage proof enclosure above 9 MiB").unwrap();
        assert!(encoded.len() > maximum);
        let decoded: PrivacyReleaseStageArtifactsV1 = decode_canonical_norito(
            &encoded,
            u64::try_from(encoded.len()).unwrap(),
            "multi-stage proof enclosure above 9 MiB",
        )
        .unwrap();
        assert_eq!(decoded, aggregate);
    }

    #[test]
    fn norito_decode_budget_covers_the_governed_aggregate_payload() {
        let maximum_payload = usize::try_from(MAX_EXPECTATIONS_NORITO_BYTES).unwrap();
        let limits = artifact_decode_limits(maximum_payload);
        assert_eq!(
            limits.max_sequence_elements(),
            usize::try_from(PRIVACY_RELEASE_MAX_PROOF_ARTIFACT_BYTES_V1).unwrap()
        );
        assert_eq!(limits.max_field_bytes(), maximum_payload);
        assert!(
            limits.max_total_elements()
                >= usize::try_from(PRIVACY_RELEASE_MAX_TOTAL_PROOF_ARTIFACT_BYTES_V1).unwrap()
        );
        assert_eq!(
            limits.max_total_allocated_bytes(),
            norito::canonical_decode_limits(maximum_payload).max_total_allocated_bytes()
        );
        assert_eq!(limits.max_nesting_depth(), 32);
    }

    #[test]
    fn receipt_json_rejects_removed_combined_proof_field() {
        let pair = PrivacyReleaseArtifactPairDigestV1 {
            norito_sha256: [1; 32],
            json_sha256: [2; 32],
        };
        let receipt = PrivacyReleaseReceiptV1 {
            schema_version: ARTIFACT_SCHEMA_VERSION_V1,
            build_profile: "release".to_owned(),
            source_sha256: [3; 32],
            exact12_matrix_sha256: [4; 32],
            expectations: pair.clone(),
            x509_resource: pair.clone(),
            cargo_lock_sha256: [5; 32],
            validator_binary_sha256: [6; 32],
            runner_binary_sha256: [7; 32],
            command_manifest: pair.clone(),
            stage_artifacts: pair,
            fixed_stage_count: u16::try_from(PRIVACY_RELEASE_STAGE_COUNT_V1)
                .expect("stage count fits u16"),
            all_native_stages_passed: true,
            contains_witnesses: false,
            contains_canonical_proof_artifacts: true,
            isolation_policy_enforced: true,
        };
        let mut json = norito::json::to_json(&receipt).expect("receipt JSON encodes");
        let closing_brace = json.pop().expect("receipt JSON has a closing brace");
        assert_eq!(closing_brace, '}');
        json.push_str(",\"contains_witnesses_or_raw_proofs\":false}");
        assert!(
            norito::json::from_str::<PrivacyReleaseReceiptV1>(&json).is_err(),
            "the first-release receipt must reject the removed combined field"
        );
    }

    #[test]
    fn expectations_reject_wrong_failure_class_and_artifact_overrun() {
        let mut expectations = canonical_expectations_v1();
        validate_expectations(&expectations).unwrap();

        expectations.stages[1].evidence.failure_class = PrivacyReleaseFailureClassV1::NotApplicable;
        assert!(validate_expectations(&expectations).is_err());

        let mut overrun = canonical_expectations_v1();
        let artifact = &mut overrun.stages[0].evidence.proof_artifacts[0];
        artifact.canonical_proof_bytes.resize(
            usize::try_from(artifact.proof_bytes_ceiling).expect("proof ceiling fits usize") + 1,
            0xa5,
        );
        refresh_artifact_hash(artifact);
        assert!(validate_expectations(&overrun).is_err());
    }

    #[test]
    fn expectations_reject_every_malformed_ordered_proof_artifact_shape() {
        let ordinary_index = usize::from(privacy_release_stage_ordinal_v1(
            PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
            PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd,
        ));
        let pgc_maximum_index = usize::from(privacy_release_stage_ordinal_v1(
            PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
            PrivacyReleaseCaseKindV1::MaximumShapeResource,
        ));
        let zk_ams_maximum_index = usize::from(privacy_release_stage_ordinal_v1(
            PrivacyProtocolIdV1::IrohaZkAmsV1,
            PrivacyReleaseCaseKindV1::MaximumShapeResource,
        ));
        assert_eq!(
            canonical_expectations_v1().stages[ordinary_index]
                .evidence
                .proof_artifacts
                .len(),
            1
        );
        assert_eq!(
            canonical_expectations_v1().stages[pgc_maximum_index]
                .evidence
                .proof_artifacts
                .len(),
            2
        );
        assert_eq!(
            canonical_expectations_v1().stages[zk_ams_maximum_index]
                .evidence
                .proof_artifacts
                .len(),
            2
        );

        let mut malformed = Vec::new();

        let mut zero = canonical_expectations_v1();
        zero.stages[ordinary_index].evidence.proof_artifacts.clear();
        malformed.push(zero);

        let mut extra = canonical_expectations_v1();
        let mut extra_artifact = extra.stages[ordinary_index].evidence.proof_artifacts[0].clone();
        extra_artifact.artifact_ordinal = 1;
        extra.stages[ordinary_index]
            .evidence
            .proof_artifacts
            .push(extra_artifact);
        malformed.push(extra);

        let mut missing_required = canonical_expectations_v1();
        missing_required.stages[pgc_maximum_index]
            .evidence
            .proof_artifacts
            .pop();
        malformed.push(missing_required);

        let mut extra_required = canonical_expectations_v1();
        let mut third = extra_required.stages[zk_ams_maximum_index]
            .evidence
            .proof_artifacts[1]
            .clone();
        third.artifact_ordinal = 2;
        extra_required.stages[zk_ams_maximum_index]
            .evidence
            .proof_artifacts
            .push(third);
        malformed.push(extra_required);

        let mut reordered = canonical_expectations_v1();
        reordered.stages[pgc_maximum_index]
            .evidence
            .proof_artifacts
            .swap(0, 1);
        malformed.push(reordered);

        let mut duplicate_ordinal = canonical_expectations_v1();
        duplicate_ordinal.stages[pgc_maximum_index]
            .evidence
            .proof_artifacts[1]
            .artifact_ordinal = 0;
        malformed.push(duplicate_ordinal);

        let mut non_contiguous = canonical_expectations_v1();
        non_contiguous.stages[zk_ams_maximum_index]
            .evidence
            .proof_artifacts[1]
            .artifact_ordinal = 2;
        malformed.push(non_contiguous);

        let mut zero_hash = canonical_expectations_v1();
        zero_hash.stages[ordinary_index].evidence.proof_artifacts[0].proof_sha256 = [0; 32];
        malformed.push(zero_hash);

        let mut zero_bytes = canonical_expectations_v1();
        let artifact = &mut zero_bytes.stages[ordinary_index].evidence.proof_artifacts[0];
        artifact.canonical_proof_bytes.clear();
        refresh_artifact_hash(artifact);
        malformed.push(zero_bytes);

        let mut zero_ceiling = canonical_expectations_v1();
        zero_ceiling.stages[ordinary_index].evidence.proof_artifacts[0].proof_bytes_ceiling = 0;
        malformed.push(zero_ceiling);

        let mut substituted_ceiling = canonical_expectations_v1();
        let artifact = &mut substituted_ceiling.stages[ordinary_index]
            .evidence
            .proof_artifacts[0];
        artifact.proof_bytes_ceiling = artifact
            .proof_bytes_ceiling
            .checked_sub(1)
            .expect("FCMP++ ceiling is nonzero");
        malformed.push(substituted_ceiling);

        let mut over_ceiling = canonical_expectations_v1();
        let artifact = &mut over_ceiling.stages[ordinary_index].evidence.proof_artifacts[0];
        artifact.canonical_proof_bytes.resize(
            usize::try_from(artifact.proof_bytes_ceiling).expect("FCMP++ ceiling fits usize") + 1,
            0x5a,
        );
        refresh_artifact_hash(artifact);
        malformed.push(over_ceiling);

        let mut unbounded = canonical_expectations_v1();
        unbounded.stages[ordinary_index].evidence.proof_artifacts[0].proof_bytes_ceiling =
            PRIVACY_RELEASE_MAX_PROOF_ARTIFACT_BYTES_V1 + 1;
        malformed.push(unbounded);

        let mut hash_mismatch = canonical_expectations_v1();
        hash_mismatch.stages[ordinary_index]
            .evidence
            .proof_artifacts[0]
            .proof_sha256[0] ^= 1;
        malformed.push(hash_mismatch);

        let mut byte_mutation = canonical_expectations_v1();
        byte_mutation.stages[ordinary_index]
            .evidence
            .proof_artifacts[0]
            .canonical_proof_bytes[0] ^= 1;
        malformed.push(byte_mutation);

        for expectations in malformed {
            assert!(validate_expectations(&expectations).is_err());
        }
    }

    #[test]
    fn hash_refreshed_corrupt_proof_cannot_replace_frozen_production_evidence() {
        let expectations = canonical_expectations_v1();
        let mut measured = measured_from_expectations(&expectations);
        validate_measured_against_expectations(&measured, &expectations).unwrap();

        let stage_index = usize::from(privacy_release_stage_ordinal_v1(
            PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
            PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd,
        ));
        let artifact = &mut measured[stage_index].evidence.proof_artifacts[0];
        artifact.canonical_proof_bytes[0] ^= 1;
        refresh_artifact_hash(artifact);
        assert!(
            validate_stage_evidence(
                &measured[stage_index].evidence,
                PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
                PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd,
                stage_index,
            )
            .is_ok(),
            "hash-refreshed bytes are structurally self-consistent"
        );
        assert!(validate_measured_against_expectations(&measured, &expectations).is_err());
    }

    #[test]
    fn expectations_require_the_exact_x5s1_artifact_ceiling_below_the_outer_action_cap() {
        let exact = canonical_expectations_v1();
        let protocol_id = PrivacyProtocolIdV1::IrohaZkX509StarkP256V0;
        const EXACT_X5S1_BYTES: u64 = 8_212_538;
        assert_eq!(
            privacy_release_proof_artifact_ceiling_v1(
                protocol_id,
                PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd,
                0,
            ),
            Some(EXACT_X5S1_BYTES)
        );
        assert!(EXACT_X5S1_BYTES < PRIVACY_RELEASE_MAX_PROOF_ARTIFACT_BYTES_V1);

        for case_kind in PrivacyReleaseCaseKindV1::ALL {
            let stage_index = usize::from(privacy_release_stage_ordinal_v1(protocol_id, case_kind));
            assert_eq!(
                exact.stages[stage_index].evidence.proof_artifacts[0].proof_bytes_ceiling,
                EXACT_X5S1_BYTES
            );
            for substituted_ceiling in [
                EXACT_X5S1_BYTES - 1,
                EXACT_X5S1_BYTES + 1,
                PRIVACY_RELEASE_MAX_PROOF_ARTIFACT_BYTES_V1,
            ] {
                let mut mutation = exact.clone();
                mutation.stages[stage_index].evidence.proof_artifacts[0].proof_bytes_ceiling =
                    substituted_ceiling;
                assert!(validate_expectations(&mutation).is_err());
            }
        }
    }

    #[test]
    fn derived_encoded_size_and_aggregate_guards_accept_cap_and_reject_cap_plus_one() {
        assert_eq!(PRIVACY_RELEASE_MAX_PROOF_ARTIFACT_BYTES_V1, 9 * 1024 * 1024);
        for maximum in [
            MAX_CHILD_RESULT_BYTES,
            MAX_EXPECTATIONS_NORITO_BYTES,
            MAX_EXPECTATIONS_JSON_BYTES,
            MAX_STAGE_ARTIFACTS_NORITO_BYTES,
            MAX_STAGE_ARTIFACTS_JSON_BYTES,
        ] {
            let exact = usize::try_from(maximum).expect("release artifact cap fits usize");
            enforce_encoded_size(exact, maximum, "boundary").unwrap();
            assert!(enforce_encoded_size(exact + 1, maximum, "boundary").is_err());
        }

        let exact_lengths = [usize::try_from(PRIVACY_RELEASE_MAX_PROOF_ARTIFACT_BYTES_V1)
            .expect("Taira artifact cap fits usize");
            PRIVACY_RELEASE_PROOF_ARTIFACT_COUNT_V1];
        assert_eq!(
            validate_aggregate_proof_artifact_lengths_v1(exact_lengths.into_iter()).unwrap(),
            PRIVACY_RELEASE_MAX_TOTAL_PROOF_ARTIFACT_BYTES_V1
        );
        let mut over_lengths = exact_lengths;
        over_lengths[0] = over_lengths[0].checked_add(1).expect("cap + 1 fits usize");
        assert!(validate_aggregate_proof_artifact_lengths_v1(over_lengths.into_iter()).is_err());
        assert!(
            validate_aggregate_proof_artifact_lengths_v1(
                [1_usize; PRIVACY_RELEASE_PROOF_ARTIFACT_COUNT_V1 + 1].into_iter()
            )
            .is_err()
        );
    }

    #[test]
    fn expectations_reject_descriptor_substitution_and_zero_statement_hash() {
        let mut cross_protocol = canonical_expectations_v1();
        cross_protocol.stages[0].evidence.protocol_descriptor =
            privacy_release_protocol_descriptor_v1(PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1)
                .to_owned();
        assert!(validate_expectations(&cross_protocol).is_err());

        let mut consistently_substituted = canonical_expectations_v1();
        for stage in &mut consistently_substituted.stages[..PRIVACY_RELEASE_CASE_COUNT_V1] {
            stage.evidence.protocol_descriptor =
                "zk-ace-pq-authorization-v0; attacker-controlled descriptor".to_owned();
        }
        assert!(validate_expectations(&consistently_substituted).is_err());

        let mut zero_statement = canonical_expectations_v1();
        zero_statement.stages[0].evidence.public_statement_sha256 = [0; 32];
        assert!(validate_expectations(&zero_statement).is_err());
    }
}
