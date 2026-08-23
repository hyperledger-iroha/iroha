//! Exact native publisher for one authenticated Kagemusha V4 promotion record.

use super::{ControllerError, FileIdentity, Result};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    path::{Component, Path, PathBuf},
};

#[cfg(target_os = "macos")]
use super::{
    MONITOR_INTERVAL, SANDBOX_EXEC, Sha256, bounded_reader, configure_isolated_child,
    ensure_empty_process_group, kagemusha_python_launcher, terminate_process_group,
    terminate_unwatched_job,
};
#[cfg(target_os = "macos")]
use iroha_data_model::offline::{
    KagemushaRecursiveSpendArtifactManifestV4, KagemushaRecursiveSpendQualificationReceiptV4,
    KagemushaRecursiveSpendReleaseAttestationV4,
};
#[cfg(target_os = "macos")]
use std::{
    ffi::OsStr,
    fs::{self, File, Metadata},
    io,
    io::{Read, Seek, SeekFrom, Write},
    os::unix::{
        fs::{DirBuilderExt, MetadataExt, PermissionsExt},
        process::ExitStatusExt,
    },
    process::{Command, ExitStatus, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

const SNAPSHOT_PARENT: &str = "/private/var/db/iroha-kagemusha-promotion-v1";
const FINAL_NAME: &str = "promotion-record-v4.norito";
const TEMP_PREFIX: &str = ".promotion-record-v4.norito.tmp.";
const BENCHMARK_NAME: &str = "physical-device-benchmark.evidence";
const REVIEW_NAME: &str = "cryptographic-review.evidence";
const MAX_PROMOTION_BYTES: u64 = 1024 * 1024;
const MAX_KAGAMI_BYTES: u64 = 512 * 1024 * 1024;
const MAX_POLICY_BYTES: u64 = 64 * 1024;
const MAX_ARTIFACT_BYTES: u64 = 5 * 1024 * 1024 * 1024;
const MAX_STDOUT_BYTES: u64 = 1024 * 1024;
const MAX_STDERR_BYTES: u64 = 256 * 1024;
const WALL_SECONDS: u64 = 300;
const COMMIT_UNCERTAIN_EXIT: u8 = 75;
#[cfg(target_os = "macos")]
const RECURSIVE_STEP_VERIFIER_COMMITMENT_DOMAIN_V4: &[u8] =
    b"iroha:kagemusha:recursive-step-verifier-commitment:v4";

#[derive(Clone, Copy)]
struct CandidateFileSpec {
    name: &'static str,
    maximum: u64,
    exact_size: Option<u64>,
}

#[cfg(any(target_os = "macos", test))]
#[derive(Clone, Copy)]
struct FileBounds {
    maximum: u64,
    exact_size: Option<u64>,
    allow_empty: bool,
}

#[cfg(target_os = "macos")]
impl CandidateFileSpec {
    const fn bounds(self) -> FileBounds {
        FileBounds {
            maximum: self.maximum,
            exact_size: self.exact_size,
            allow_empty: false,
        }
    }
}

const CANDIDATE_FILES: [CandidateFileSpec; 16] = [
    CandidateFileSpec {
        name: "step-eq.params-ipa.krv4",
        maximum: MAX_ARTIFACT_BYTES,
        exact_size: None,
    },
    CandidateFileSpec {
        name: "step-eq.proving-key.krv4",
        maximum: MAX_ARTIFACT_BYTES,
        exact_size: None,
    },
    CandidateFileSpec {
        name: "step-eq.verifying-key.krv4",
        maximum: MAX_ARTIFACT_BYTES,
        exact_size: None,
    },
    CandidateFileSpec {
        name: "step-eq.bootstrap-witness.krv4",
        maximum: MAX_ARTIFACT_BYTES,
        exact_size: None,
    },
    CandidateFileSpec {
        name: "step-ep.params-ipa.krv4",
        maximum: MAX_ARTIFACT_BYTES,
        exact_size: None,
    },
    CandidateFileSpec {
        name: "step-ep.proving-key.krv4",
        maximum: MAX_ARTIFACT_BYTES,
        exact_size: None,
    },
    CandidateFileSpec {
        name: "step-ep.verifying-key.krv4",
        maximum: MAX_ARTIFACT_BYTES,
        exact_size: None,
    },
    CandidateFileSpec {
        name: "step-ep.bootstrap-witness.krv4",
        maximum: MAX_ARTIFACT_BYTES,
        exact_size: None,
    },
    CandidateFileSpec {
        name: "topup-finality-roster-v4.norito",
        maximum: 2 * 1024 * 1024,
        exact_size: None,
    },
    CandidateFileSpec {
        name: "manifest.norito",
        maximum: 1024 * 1024,
        exact_size: None,
    },
    CandidateFileSpec {
        name: "manifest.norito.sha256",
        maximum: 65,
        exact_size: Some(65),
    },
    CandidateFileSpec {
        name: "manifest.json",
        maximum: 1024 * 1024,
        exact_size: None,
    },
    CandidateFileSpec {
        name: "release-attestation-v4.norito",
        maximum: 1024 * 1024,
        exact_size: None,
    },
    CandidateFileSpec {
        name: BENCHMARK_NAME,
        maximum: 16 * 1024 * 1024,
        exact_size: None,
    },
    CandidateFileSpec {
        name: REVIEW_NAME,
        maximum: 1024 * 1024,
        exact_size: None,
    },
    CandidateFileSpec {
        name: "recursive-step-two-qualification-v4.norito",
        maximum: 2 * 384 * 1024 + 16 * 1024,
        exact_size: None,
    },
];

#[cfg(any(target_os = "macos", test))]
const REPORT_ARTIFACTS: [(&str, &str); 9] = [
    ("step_eq_params_ipa", "step-eq.params-ipa.krv4"),
    ("step_eq_proving_key", "step-eq.proving-key.krv4"),
    ("step_eq_verifying_key", "step-eq.verifying-key.krv4"),
    (
        "step_eq_bootstrap_witness",
        "step-eq.bootstrap-witness.krv4",
    ),
    ("step_ep_params_ipa", "step-ep.params-ipa.krv4"),
    ("step_ep_proving_key", "step-ep.proving-key.krv4"),
    ("step_ep_verifying_key", "step-ep.verifying-key.krv4"),
    (
        "step_ep_bootstrap_witness",
        "step-ep.bootstrap-witness.krv4",
    ),
    ("topup_finality_roster", "topup-finality-roster-v4.norito"),
];

#[cfg(any(target_os = "macos", test))]
#[derive(Clone, Debug, Eq, PartialEq, norito::JsonSerialize)]
struct CanonicalReportArtifact {
    purpose: String,
    file_name: String,
    size_bytes: u64,
    sha256: String,
    payload_size_bytes: Option<u64>,
    payload_sha256: Option<String>,
}

#[cfg(any(target_os = "macos", test))]
#[derive(Clone, Debug, Eq, PartialEq, norito::JsonSerialize)]
struct CanonicalReportV4 {
    status: String,
    envelope_sha256: String,
    manifest_body_sha256: String,
    candidate_sha256: String,
    qualification_receipt_sha256: String,
    qualified_candidate_sha256: String,
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
    artifacts: Vec<CanonicalReportArtifact>,
}

#[derive(Debug)]
struct PromotionRequest {
    expected_macos_build: String,
    kagami: PathBuf,
    kagami_sha256: [u8; 32],
    bundle_dir: PathBuf,
    release_policy: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PublicationPhase {
    Candidate,
    Staging,
    Committed,
}

pub(super) fn promote(arguments: &[OsString]) -> Result<u8> {
    #[cfg(target_os = "macos")]
    {
        return promote_macos(parse_request(arguments)?);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = arguments;
        Err(ControllerError::policy(
            "Kagemusha promotion publication requires a qualified macOS host",
        ))
    }
}

fn parse_request(arguments: &[OsString]) -> Result<PromotionRequest> {
    const OPTIONS: [&str; 5] = [
        "--expected-macos-build",
        "--kagami",
        "--kagami-sha256",
        "--bundle-dir",
        "--release-policy",
    ];
    if arguments.len() != OPTIONS.len() * 2 {
        return Err(ControllerError::policy(
            "promotion publisher arguments are not the exact reviewed set",
        ));
    }
    let mut values = BTreeMap::new();
    for (index, option) in OPTIONS.into_iter().enumerate() {
        if arguments[index * 2] != option {
            return Err(ControllerError::policy(
                "promotion publisher argument order differs from its reviewed contract",
            ));
        }
        let value = arguments[index * 2 + 1]
            .to_str()
            .ok_or_else(|| ControllerError::policy("promotion argument is not UTF-8"))?;
        if value.as_bytes().contains(&0) || value.len() > 4096 {
            return Err(ControllerError::policy(
                "promotion argument exceeds its canonical bound",
            ));
        }
        values.insert(option, value);
    }
    let expected_macos_build = values["--expected-macos-build"];
    if expected_macos_build.is_empty()
        || expected_macos_build.len() > 64
        || !expected_macos_build
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(ControllerError::policy(
            "expected macOS build is not a portable identifier",
        ));
    }
    Ok(PromotionRequest {
        expected_macos_build: expected_macos_build.to_owned(),
        kagami: normalized_absolute_path(values["--kagami"])?,
        kagami_sha256: parse_sha256(values["--kagami-sha256"], "Kagami SHA-256")?,
        bundle_dir: normalized_absolute_path(values["--bundle-dir"])?,
        release_policy: normalized_absolute_path(values["--release-policy"])?,
    })
}

fn normalized_absolute_path(value: &str) -> Result<PathBuf> {
    let path = PathBuf::from(value);
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(ControllerError::policy(
            "promotion path is not one normalized absolute path",
        ));
    }
    Ok(path)
}

fn parse_sha256(value: &str, label: &str) -> Result<[u8; 32]> {
    if value.len() != 64
        || value.bytes().all(|byte| byte == b'0')
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ControllerError::policy(format!("{label} is malformed")));
    }
    let mut digest = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        digest[index] = (hex_nibble(pair[0]) << 4) | hex_nibble(pair[1]);
    }
    Ok(digest)
}

fn hex_nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => 0,
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

fn valid_temp_name(name: &str) -> bool {
    name.strip_prefix(TEMP_PREFIX).is_some_and(|random| {
        random.len() == 32
            && random
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn stable_identity(left: &FileIdentity, right: &FileIdentity) -> bool {
    left.device == right.device
        && left.inode == right.inode
        && left.mode == right.mode
        && left.uid == right.uid
        && left.gid == right.gid
        && left.links == right.links
        && left.size == right.size
        && left.modified_seconds == right.modified_seconds
        && left.modified_nanoseconds == right.modified_nanoseconds
}

fn stable_directory_identity(left: &FileIdentity, right: &FileIdentity) -> bool {
    left.device == right.device
        && left.inode == right.inode
        && left.mode == right.mode
        && left.uid == right.uid
        && left.gid == right.gid
        && left.links == right.links
}

fn validate_regular_identity(
    name: &str,
    identity: &FileIdentity,
    maximum: u64,
    allow_empty: bool,
) -> Result<()> {
    if identity.mode & 0o170000 != 0o100000
        || identity.uid != 0
        || identity.links != 1
        || identity.mode & 0o6022 != 0
        || (!allow_empty && identity.size == 0)
        || identity.size > maximum
    {
        return Err(ControllerError::policy(format!(
            "promotion inventory entry has unsafe type, custody, links, mode, or size: {name}"
        )));
    }
    Ok(())
}

#[cfg(any(target_os = "macos", test))]
fn validate_bounded_identity(
    name: &str,
    identity: &FileIdentity,
    bounds: FileBounds,
) -> Result<()> {
    validate_regular_identity(name, identity, bounds.maximum, bounds.allow_empty)?;
    if bounds.exact_size.is_some_and(|size| identity.size != size) {
        return Err(ControllerError::policy(format!(
            "promotion inventory entry has an inexact size: {name}"
        )));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn candidate_bounds(name: &str) -> Option<FileBounds> {
    CANDIDATE_FILES
        .iter()
        .find(|spec| spec.name == name)
        .copied()
        .map(CandidateFileSpec::bounds)
}

#[cfg(target_os = "macos")]
fn inventoried_member_bounds(name: &str) -> Result<FileBounds> {
    if let Some(bounds) = candidate_bounds(name) {
        return Ok(bounds);
    }
    if name == FINAL_NAME {
        return Ok(FileBounds {
            maximum: MAX_PROMOTION_BYTES,
            exact_size: None,
            allow_empty: false,
        });
    }
    if valid_temp_name(name) {
        return Ok(FileBounds {
            maximum: MAX_PROMOTION_BYTES,
            exact_size: None,
            allow_empty: true,
        });
    }
    Err(ControllerError::policy(format!(
        "promotion inventory contains an unexpected entry: {name}"
    )))
}

#[cfg(target_os = "macos")]
fn require_candidate_binding(
    identities: &BTreeMap<String, FileIdentity>,
    name: &str,
    size: Option<u64>,
    sha256: [u8; 32],
) -> Result<()> {
    let identity = identities.get(name).ok_or_else(|| {
        ControllerError::policy(format!("captured candidate file is absent: {name}"))
    })?;
    if size.is_some_and(|expected| identity.size != expected) || identity.sha256 != sha256 {
        return Err(ControllerError::policy(format!(
            "captured candidate file differs from its canonical manifest binding: {name}"
        )));
    }
    Ok(())
}

fn classify_inventory(
    initial: &BTreeMap<String, FileIdentity>,
    current: &BTreeMap<String, FileIdentity>,
) -> Result<PublicationPhase> {
    if initial.len() != CANDIDATE_FILES.len() {
        return Err(ControllerError::policy(
            "promotion candidate snapshot does not contain exactly sixteen files",
        ));
    }
    for (name, expected) in initial {
        let observed = current.get(name).ok_or_else(|| {
            ControllerError::policy(format!("promotion candidate entry disappeared: {name}"))
        })?;
        if !stable_identity(expected, observed) {
            return Err(ControllerError::policy(format!(
                "promotion candidate entry changed: {name}"
            )));
        }
    }
    let additions = current
        .iter()
        .filter(|(name, _)| !initial.contains_key(*name))
        .collect::<Vec<_>>();
    match additions.as_slice() {
        [] => Ok(PublicationPhase::Candidate),
        [(name, identity)] if name.as_str() == FINAL_NAME => {
            validate_regular_identity(name, identity, MAX_PROMOTION_BYTES, false)?;
            Ok(PublicationPhase::Committed)
        }
        [(name, identity)] if valid_temp_name(name) => {
            validate_regular_identity(name, identity, MAX_PROMOTION_BYTES, true)?;
            Ok(PublicationPhase::Staging)
        }
        _ => Err(ControllerError::policy(
            "promotion directory contains an unexpected delta or multiple temporary files",
        )),
    }
}

fn commit_uncertain(message: impl Into<String>) -> ControllerError {
    ControllerError {
        message: format!(
            "Kagemusha promotion commit is uncertain: {}",
            message.into()
        ),
        exit: COMMIT_UNCERTAIN_EXIT,
    }
}

fn failed_child_result(
    exit: u8,
    phase: Option<PublicationPhase>,
    cleanup_succeeded: bool,
) -> Result<u8> {
    if phase == Some(PublicationPhase::Candidate) && cleanup_succeeded {
        Ok(exit)
    } else {
        Err(commit_uncertain(format!(
            "Kagami exited with status {exit} after publication may have begun"
        )))
    }
}

#[cfg(any(target_os = "macos", test))]
fn canonical_report(stdout: &[u8], expected: &CanonicalReportV4) -> Result<()> {
    if stdout.len() < 3
        || stdout.last() != Some(&b'\n')
        || stdout[..stdout.len() - 1]
            .iter()
            .any(|byte| matches!(byte, b'\n' | b'\r' | 0))
    {
        return Err(ControllerError::policy(
            "Kagami promotion stdout is not one exact JSON line",
        ));
    }
    let payload = &stdout[..stdout.len() - 1];
    let canonical = norito::json::to_json(expected).map_err(|_| {
        ControllerError::policy("expected Kagemusha promotion report could not be encoded")
    })?;
    if payload != canonical.as_bytes() {
        return Err(ControllerError::policy(
            "Kagami promotion report is not the exact canonical candidate-bound JSON",
        ));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn canonical_existing(path: &Path, directory: bool, label: &str) -> Result<PathBuf> {
    let canonical = fs::canonicalize(path)
        .map_err(|_| ControllerError::policy(format!("{label} is unavailable")))?;
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| ControllerError::policy(format!("{label} metadata is unavailable")))?;
    if canonical != path
        || metadata.file_type().is_symlink()
        || directory != metadata.is_dir()
        || (!directory && !metadata.is_file())
    {
        return Err(ControllerError::policy(format!(
            "{label} is not one canonical symlink-free {}",
            if directory { "directory" } else { "file" }
        )));
    }
    Ok(canonical)
}

#[cfg(target_os = "macos")]
fn identity_from_metadata(metadata: &Metadata, sha256: [u8; 32]) -> FileIdentity {
    FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
        mode: metadata.mode(),
        uid: metadata.uid(),
        gid: metadata.gid(),
        links: metadata.nlink(),
        size: metadata.size(),
        modified_seconds: metadata.mtime(),
        modified_nanoseconds: metadata.mtime_nsec(),
        sha256,
    }
}

#[cfg(target_os = "macos")]
fn identity_from_file(file: &mut File, hash_contents: bool) -> Result<FileIdentity> {
    identity_from_file_checked(file, hash_contents, None)
}

#[cfg(target_os = "macos")]
fn identity_from_file_checked(
    file: &mut File,
    hash_contents: bool,
    validation: Option<(&str, FileBounds)>,
) -> Result<FileIdentity> {
    let before = file
        .metadata()
        .map_err(|_| ControllerError::policy("promotion descriptor metadata is unavailable"))?;
    let before_identity = identity_from_metadata(&before, [0; 32]);
    if let Some((name, bounds)) = validation {
        // Validate the same descriptor metadata snapshot that immediately
        // precedes the first read. This keeps oversized and special files out
        // of the hashing path rather than discovering them after EOF.
        validate_bounded_identity(name, &before_identity, bounds)?;
    }
    let mut hashed_length_matches = true;
    let sha256 = if hash_contents {
        file.seek(SeekFrom::Start(0))
            .map_err(|_| ControllerError::policy("promotion descriptor seek failed"))?;
        let mut hash = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        let read_limit = validation.map_or(u64::MAX, |_| before_identity.size.saturating_add(1));
        let mut total = 0_u64;
        loop {
            if total == read_limit {
                break;
            }
            let remaining = read_limit - total;
            let capacity = usize::try_from(
                remaining.min(u64::try_from(buffer.len()).expect("hash buffer length fits u64")),
            )
            .expect("bounded hash read length fits usize");
            let count = file
                .read(&mut buffer[..capacity])
                .map_err(|_| ControllerError::policy("promotion descriptor read failed"))?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
            total = total.saturating_add(u64::try_from(count).expect("read length fits u64"));
        }
        file.seek(SeekFrom::Start(0))
            .map_err(|_| ControllerError::policy("promotion descriptor rewind failed"))?;
        hashed_length_matches = validation.is_none() || total == before_identity.size;
        hash.finish()
    } else {
        [0; 32]
    };
    let after = file
        .metadata()
        .map_err(|_| ControllerError::policy("promotion descriptor metadata is unavailable"))?;
    let before = identity_from_metadata(&before, sha256);
    let after = identity_from_metadata(&after, sha256);
    if let Some((name, bounds)) = validation {
        validate_bounded_identity(name, &after, bounds)?;
    }
    if before != after || !hashed_length_matches {
        return Err(ControllerError::policy(
            "promotion descriptor changed while it was inspected",
        ));
    }
    Ok(after)
}

#[cfg(target_os = "macos")]
fn sha256_bytes(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(bytes);
    hash.finish()
}

#[cfg(target_os = "macos")]
fn decode_canonical_norito<T>(bytes: &[u8], label: &str) -> Result<T>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    let value = norito::decode_from_bytes(bytes)
        .map_err(|_| ControllerError::policy(format!("{label} is not valid Norito")))?;
    let canonical = norito::to_bytes(&value)
        .map_err(|_| ControllerError::policy(format!("{label} cannot be canonically encoded")))?;
    if canonical != bytes {
        return Err(ControllerError::policy(format!(
            "{label} is not canonical Norito"
        )));
    }
    Ok(value)
}

#[cfg(target_os = "macos")]
fn recursive_step_verifier_commitment_v4(
    manifest: &KagemushaRecursiveSpendArtifactManifestV4,
) -> Result<[u8; 32]> {
    let profiles = norito::to_bytes(&manifest.profiles).map_err(|_| {
        ControllerError::policy("Kagemusha verifier profiles cannot be canonically encoded")
    })?;
    let mut hash = Sha256::new();
    hash.update(RECURSIVE_STEP_VERIFIER_COMMITMENT_DOMAIN_V4);
    hash.update(&[0]);
    hash.update(&profiles);
    Ok(hash.finish())
}

#[cfg(target_os = "macos")]
fn open_directory(path: &Path) -> Result<File> {
    use rustix::fs::{Mode, OFlags, open};
    Ok(File::from(
        open(
            path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| ControllerError::policy("promotion directory cannot be pinned"))?,
    ))
}

#[cfg(target_os = "macos")]
fn inventory_names(directory: &File) -> Result<BTreeSet<String>> {
    use rustix::fs::Dir;
    let mut names = BTreeSet::new();
    let mut entries = Dir::read_from(directory)
        .map_err(|_| ControllerError::policy("promotion inventory is unavailable"))?;
    for entry in &mut entries {
        let entry = entry
            .map_err(|_| ControllerError::policy("promotion inventory entry is unavailable"))?;
        let bytes = entry.file_name().to_bytes();
        if matches!(bytes, b"." | b"..") {
            continue;
        }
        let name = std::str::from_utf8(bytes)
            .map_err(|_| ControllerError::policy("promotion inventory name is not UTF-8"))?
            .to_owned();
        if !names.insert(name) || names.len() > 18 {
            return Err(ControllerError::policy(
                "promotion inventory is duplicated or exceeds its bound",
            ));
        }
    }
    Ok(names)
}

#[cfg(target_os = "macos")]
fn open_member(
    directory: &File,
    name: &str,
    bounds: FileBounds,
    hash_contents: bool,
) -> Result<(File, FileIdentity)> {
    use rustix::fs::{AtFlags, FileType, Mode, OFlags, openat, statat};
    let named = statat(directory, name, AtFlags::SYMLINK_NOFOLLOW).map_err(|_| {
        ControllerError::policy(format!("promotion entry cannot be inspected: {name}"))
    })?;
    if FileType::from_raw_mode(named.st_mode) != FileType::RegularFile {
        return Err(ControllerError::policy(format!(
            "promotion entry is not a regular file: {name}"
        )));
    }
    let mut file = File::from(
        openat(
            directory,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| {
            ControllerError::policy(format!("promotion entry cannot be opened: {name}"))
        })?,
    );
    // The pre-open stat prevents normal special-file opens. Seatbelt admits
    // only regular temp/final creation, NONBLOCK makes a raced FIFO harmless,
    // and this descriptor check rejects every raced non-regular inode before
    // xattr inspection, hashing, or any other read. An external root process
    // replacing the pathname inside the stat/open interval remains part of the
    // explicitly trusted root filesystem TCB.
    let opened = file
        .metadata()
        .map_err(|_| ControllerError::policy("promotion descriptor metadata is unavailable"))?;
    validate_bounded_identity(name, &identity_from_metadata(&opened, [0; 32]), bounds)?;
    kagemusha_python_launcher::validate_open_file_custody(&file, Path::new(name))?;
    let identity = identity_from_file_checked(&mut file, hash_contents, Some((name, bounds)))?;
    Ok((file, identity))
}

#[cfg(target_os = "macos")]
struct PinnedInput {
    path: PathBuf,
    parent: File,
    file: File,
    parent_identity: FileIdentity,
    identity: FileIdentity,
    bounds: FileBounds,
}

#[cfg(target_os = "macos")]
impl PinnedInput {
    fn open(path: &Path, maximum: u64) -> Result<Self> {
        let parent_path = path
            .parent()
            .ok_or_else(|| ControllerError::policy("promotion input has no parent"))?;
        let name = path
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or_else(|| ControllerError::policy("promotion input name is invalid"))?;
        let mut parent = open_directory(parent_path)?;
        let parent_identity = identity_from_file(&mut parent, false)?;
        let bounds = FileBounds {
            maximum,
            exact_size: None,
            allow_empty: false,
        };
        let (file, identity) = open_member(&parent, name, bounds, true)?;
        Ok(Self {
            path: path.to_path_buf(),
            parent,
            file,
            parent_identity,
            identity,
            bounds,
        })
    }

    fn verify(&mut self) -> Result<()> {
        let name = self
            .path
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or_else(|| ControllerError::policy("promotion input name is invalid"))?;
        if !stable_identity(
            &self.parent_identity,
            &identity_from_file(&mut self.parent, false)?,
        ) || self.identity
            != identity_from_file_checked(&mut self.file, true, Some((name, self.bounds)))?
        {
            return Err(ControllerError::policy(
                "promotion input descriptor identity changed",
            ));
        }
        let parent_path = self
            .path
            .parent()
            .ok_or_else(|| ControllerError::policy("promotion input has no parent"))?;
        kagemusha_python_launcher::require_root_custody(&self.path, false)?;
        let mut fresh_parent = open_directory(parent_path)?;
        if !stable_identity(
            &self.parent_identity,
            &identity_from_file(&mut fresh_parent, false)?,
        ) {
            return Err(ControllerError::policy(
                "promotion input parent path was substituted",
            ));
        }
        let (_, fresh) = open_member(&fresh_parent, name, self.bounds, true)?;
        if fresh != self.identity {
            return Err(ControllerError::policy(
                "promotion input path or bytes changed after execution",
            ));
        }
        Ok(())
    }
}

#[cfg(target_os = "macos")]
struct CandidateSnapshot {
    path: PathBuf,
    directory: File,
    directory_identity: FileIdentity,
    files: BTreeMap<String, (File, FileIdentity)>,
}

#[cfg(target_os = "macos")]
impl CandidateSnapshot {
    fn open(path: &Path) -> Result<Self> {
        let mut directory = open_directory(path)?;
        let directory_identity = identity_from_file(&mut directory, false)?;
        let expected = CANDIDATE_FILES
            .iter()
            .map(|spec| spec.name.to_owned())
            .collect::<BTreeSet<_>>();
        if inventory_names(&directory)? != expected {
            return Err(ControllerError::policy(
                "promotion requires the exact sixteen-file candidate inventory and absent final leaf",
            ));
        }
        let mut files = BTreeMap::new();
        let mut inodes = BTreeSet::new();
        for spec in CANDIDATE_FILES {
            kagemusha_python_launcher::require_root_custody(&path.join(spec.name), false)?;
            let (file, identity) = open_member(&directory, spec.name, spec.bounds(), true)?;
            if !inodes.insert((identity.device, identity.inode)) {
                return Err(ControllerError::policy(
                    "promotion candidate contains a size violation or hard-link alias",
                ));
            }
            files.insert(spec.name.to_owned(), (file, identity));
        }
        Ok(Self {
            path: path.to_path_buf(),
            directory,
            directory_identity,
            files,
        })
    }

    fn initial_identities(&self) -> BTreeMap<String, FileIdentity> {
        self.files
            .iter()
            .map(|(name, (_, identity))| (name.clone(), identity.clone()))
            .collect()
    }

    fn read_held(&mut self, name: &str) -> Result<Vec<u8>> {
        let bounds = candidate_bounds(name).ok_or_else(|| {
            ControllerError::policy("held promotion input has no reviewed size contract")
        })?;
        let (held, expected) = self.files.get_mut(name).ok_or_else(|| {
            ControllerError::policy(format!("held candidate file is absent: {name}"))
        })?;
        let expected = expected.clone();
        let before = identity_from_file_checked(held, false, Some((name, bounds)))?;
        if !stable_identity(&expected, &before) {
            return Err(ControllerError::policy(format!(
                "held candidate file changed before read: {name}"
            )));
        }
        let length = usize::try_from(expected.size).map_err(|_| {
            ControllerError::policy(format!("held candidate file is too large to read: {name}"))
        })?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length.saturating_add(1))
            .map_err(|_| {
                ControllerError::policy(format!("held candidate read allocation failed: {name}"))
            })?;
        held.seek(SeekFrom::Start(0))
            .map_err(|_| ControllerError::policy("promotion descriptor seek failed"))?;
        Read::by_ref(held)
            .take(expected.size.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|_| ControllerError::policy(format!("held candidate read failed: {name}")))?;
        held.seek(SeekFrom::Start(0))
            .map_err(|_| ControllerError::policy("promotion descriptor rewind failed"))?;
        let after = identity_from_file_checked(held, false, Some((name, bounds)))?;
        if bytes.len() != length
            || sha256_bytes(&bytes) != expected.sha256
            || !stable_identity(&expected, &after)
        {
            return Err(ControllerError::policy(format!(
                "held candidate file changed while it was read: {name}"
            )));
        }
        Ok(bytes)
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the exact report projection keeps every independently decoded candidate binding in one auditable pass"
    )]
    fn report_expectation(
        &mut self,
        bundle_leaf: &str,
        policy_sha256: [u8; 32],
        promotion_sha256: [u8; 32],
    ) -> Result<CanonicalReportV4> {
        let identities = self.initial_identities();
        let envelope_sha256 = parse_sha256(bundle_leaf, "candidate bundle manifest digest")?;
        let manifest_bytes = self.read_held("manifest.norito")?;
        let manifest: KagemushaRecursiveSpendArtifactManifestV4 =
            decode_canonical_norito(&manifest_bytes, "Kagemusha V4 manifest")?;
        manifest
            .validate()
            .map_err(|_| ControllerError::policy("Kagemusha V4 manifest validation failed"))?;
        let manifest_identity = identities
            .get("manifest.norito")
            .ok_or_else(|| ControllerError::policy("captured manifest is absent"))?;
        if manifest_identity.sha256 != envelope_sha256
            || manifest
                .canonical_sha256()
                .map_err(|_| ControllerError::policy("manifest identity derivation failed"))?
                != envelope_sha256
        {
            return Err(ControllerError::policy(
                "candidate directory leaf does not identify the canonical manifest envelope",
            ));
        }

        let manifest_sidecar = self.read_held("manifest.norito.sha256")?;
        if manifest_sidecar != format!("{}\n", hex(&envelope_sha256)).as_bytes() {
            return Err(ControllerError::policy(
                "candidate manifest digest sidecar is not exact",
            ));
        }
        let manifest_json = self.read_held("manifest.json")?;
        let manifest_from_json: KagemushaRecursiveSpendArtifactManifestV4 =
            norito::json::from_slice(&manifest_json).map_err(|_| {
                ControllerError::policy("candidate manifest JSON is not the typed V4 manifest")
            })?;
        let mut canonical_manifest_json = norito::json::to_string_pretty(&manifest)
            .map_err(|_| ControllerError::policy("candidate manifest JSON cannot be encoded"))?;
        canonical_manifest_json.push('\n');
        if manifest_from_json != manifest || manifest_json != canonical_manifest_json.as_bytes() {
            return Err(ControllerError::policy(
                "candidate manifest JSON is noncanonical or differs from manifest.norito",
            ));
        }

        let candidate = manifest.immutable_candidate().map_err(|_| {
            ControllerError::policy("immutable Kagemusha candidate derivation failed")
        })?;
        let candidate_sha256 = candidate
            .sha256()
            .map_err(|_| ControllerError::policy("immutable Kagemusha candidate digest failed"))?;
        let qualification_bytes = self.read_held("recursive-step-two-qualification-v4.norito")?;
        let qualification =
            KagemushaRecursiveSpendQualificationReceiptV4::decode_canonical_against_candidate(
                &qualification_bytes,
                &candidate,
            )
            .map_err(|_| ControllerError::policy("qualification receipt is not candidate-bound"))?;
        let qualification_sha256 = qualification
            .canonical_sha256_against_candidate(&candidate)
            .map_err(|_| {
                ControllerError::policy("qualification receipt digest derivation failed")
            })?;
        let qualified_candidate_sha256 = qualification
            .qualified_candidate_sha256(&candidate)
            .map_err(|_| ControllerError::policy("qualified candidate digest derivation failed"))?;
        require_candidate_binding(
            &identities,
            "recursive-step-two-qualification-v4.norito",
            None,
            qualification_sha256,
        )?;
        if qualification_sha256 != manifest.qualification_receipt_sha256
            || qualified_candidate_sha256 != manifest.qualified_candidate_sha256
        {
            return Err(ControllerError::policy(
                "manifest qualification identities differ from the canonical receipt",
            ));
        }

        require_candidate_binding(
            &identities,
            BENCHMARK_NAME,
            None,
            manifest.benchmark_evidence_sha256,
        )?;
        require_candidate_binding(
            &identities,
            REVIEW_NAME,
            None,
            manifest.cryptographic_review_sha256,
        )?;
        let attestation_bytes = self.read_held("release-attestation-v4.norito")?;
        let attestation: KagemushaRecursiveSpendReleaseAttestationV4 =
            decode_canonical_norito(&attestation_bytes, "Kagemusha V4 release attestation")?;
        let subject = manifest.release_attestation_subject().map_err(|_| {
            ControllerError::policy("release attestation subject derivation failed")
        })?;
        if attestation.subject != subject {
            return Err(ControllerError::policy(
                "canonical release attestation is not bound to the manifest subject",
            ));
        }
        require_candidate_binding(
            &identities,
            "release-attestation-v4.norito",
            None,
            manifest.release_attestation_sha256,
        )?;

        let descriptors = manifest
            .profiles
            .iter()
            .flat_map(|profile| profile.artifacts.iter())
            .collect::<Vec<_>>();
        if descriptors.len() + 1 != REPORT_ARTIFACTS.len() {
            return Err(ControllerError::policy(
                "manifest does not contain the exact report artifact inventory",
            ));
        }
        let mut artifacts = Vec::with_capacity(REPORT_ARTIFACTS.len());
        for (descriptor, &(purpose, name)) in descriptors.iter().zip(REPORT_ARTIFACTS[..8].iter()) {
            if descriptor.file_name != name {
                return Err(ControllerError::policy(
                    "manifest artifact role order differs from the report contract",
                ));
            }
            require_candidate_binding(
                &identities,
                name,
                Some(descriptor.size_bytes),
                descriptor.sha256,
            )?;
            artifacts.push(CanonicalReportArtifact {
                purpose: purpose.to_owned(),
                file_name: descriptor.file_name.clone(),
                size_bytes: descriptor.size_bytes,
                sha256: hex(&descriptor.sha256),
                payload_size_bytes: Some(descriptor.payload_size_bytes),
                payload_sha256: Some(hex(&descriptor.payload_sha256)),
            });
        }
        let (roster_purpose, roster_name) = REPORT_ARTIFACTS[8];
        let roster = &manifest.topup_finality_roster_artifact;
        if roster.file_name != roster_name {
            return Err(ControllerError::policy(
                "manifest roster name differs from the report contract",
            ));
        }
        require_candidate_binding(
            &identities,
            roster_name,
            Some(roster.size_bytes),
            roster.sha256,
        )?;
        artifacts.push(CanonicalReportArtifact {
            purpose: roster_purpose.to_owned(),
            file_name: roster.file_name.clone(),
            size_bytes: roster.size_bytes,
            sha256: hex(&roster.sha256),
            payload_size_bytes: None,
            payload_sha256: None,
        });

        Ok(CanonicalReportV4 {
            status: "verified".to_owned(),
            envelope_sha256: hex(&envelope_sha256),
            manifest_body_sha256: hex(&subject.manifest_subject_sha256),
            candidate_sha256: hex(&candidate_sha256),
            qualification_receipt_sha256: hex(&qualification_sha256),
            qualified_candidate_sha256: hex(&qualified_candidate_sha256),
            promotion_record_sha256: hex(&promotion_sha256),
            release_policy_sha256: hex(&policy_sha256),
            authenticated_source_seal_projection_sha256: hex(
                &manifest.authenticated_source_seal_projection_sha256
            ),
            reviewed_cargo_binary_sha256: hex(&manifest.reviewed_cargo_binary_sha256),
            reviewed_rustc_binary_sha256: hex(&manifest.reviewed_rustc_binary_sha256),
            generator_binary_sha256: hex(&manifest.generator_binary_sha256),
            sealed_candidate_build_report_sha256: hex(
                &manifest.sealed_candidate_build_report_sha256
            ),
            generation: manifest.generation.clone(),
            generation_memory_limit_bytes: manifest.generation_memory_limit_bytes,
            generation_memory_enforcement_profile: manifest
                .generation_memory_enforcement_profile
                .clone(),
            network_id: manifest.network_id.to_string(),
            asset_definition_id: manifest.asset.to_string(),
            asset_scale: manifest.asset_scale,
            bridge_abi_version: manifest.bridge_abi_version,
            recursive_step_verifier_commitment: hex(&recursive_step_verifier_commitment_v4(
                &manifest,
            )?),
            artifacts,
        })
    }

    fn current_identities_once(&self) -> Result<BTreeMap<String, FileIdentity>> {
        let names = inventory_names(&self.directory)?;
        let mut current = BTreeMap::new();
        for name in names {
            let bounds = inventoried_member_bounds(&name)?;
            let (_, identity) = open_member(&self.directory, &name, bounds, false)?;
            current.insert(name, identity);
        }
        Ok(current)
    }

    fn current_identities(&self) -> Result<BTreeMap<String, FileIdentity>> {
        let mut last = None;
        for _ in 0..3 {
            match self.current_identities_once() {
                Ok(current) => return Ok(current),
                Err(error) => {
                    last = Some(error);
                    thread::yield_now();
                }
            }
        }
        Err(last.expect("promotion inventory retry records an error"))
    }

    fn phase(&mut self) -> Result<PublicationPhase> {
        let mut path_directory = open_directory(&self.path)?;
        if !stable_directory_identity(
            &self.directory_identity,
            &identity_from_file(&mut self.directory, false)?,
        ) || !stable_directory_identity(
            &self.directory_identity,
            &identity_from_file(&mut path_directory, false)?,
        ) {
            return Err(ControllerError::policy(
                "promotion bundle directory identity changed",
            ));
        }
        classify_inventory(&self.initial_identities(), &self.current_identities()?)
    }

    fn verify_committed(&mut self) -> Result<FileIdentity> {
        if self.phase()? != PublicationPhase::Committed {
            return Err(ControllerError::policy(
                "promotion did not produce the exact seventeen-file post-state",
            ));
        }
        for (name, (held, expected)) in &mut self.files {
            let bounds = candidate_bounds(name).ok_or_else(|| {
                ControllerError::policy("held promotion input has no reviewed size contract")
            })?;
            if expected != &identity_from_file_checked(held, true, Some((name, bounds)))? {
                return Err(ControllerError::policy(format!(
                    "held promotion input changed: {name}"
                )));
            }
            kagemusha_python_launcher::require_root_custody(&self.path.join(name), false)?;
            let (_, fresh) = open_member(&self.directory, name, bounds, true)?;
            if &fresh != expected {
                return Err(ControllerError::policy(format!(
                    "promotion input changed or was substituted: {name}"
                )));
            }
        }
        kagemusha_python_launcher::require_root_custody(&self.path.join(FINAL_NAME), false)?;
        let final_bounds = inventoried_member_bounds(FINAL_NAME)?;
        let (_, final_identity) = open_member(&self.directory, FINAL_NAME, final_bounds, true)?;
        Ok(final_identity)
    }
}

#[cfg(target_os = "macos")]
struct ExecutableSnapshot {
    parent: File,
    directory: PathBuf,
    path: PathBuf,
    pinned: kagemusha_python_launcher::PinnedFile,
    cleaned: bool,
}

#[cfg(target_os = "macos")]
struct SnapshotStaging {
    parent: Option<File>,
    directory: PathBuf,
    path: PathBuf,
    armed: bool,
}

#[cfg(target_os = "macos")]
impl SnapshotStaging {
    fn create(parent_path: &Path) -> Result<Self> {
        let parent = open_directory(parent_path)?;
        let directory = parent_path.join("active");
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .map_err(|_| {
                ControllerError::policy(
                    "promotion snapshot active directory already exists or cannot be reserved",
                )
            })?;
        Ok(Self {
            parent: Some(parent),
            path: directory.join("kagami"),
            directory,
            armed: true,
        })
    }

    fn finish(mut self, pinned: kagemusha_python_launcher::PinnedFile) -> ExecutableSnapshot {
        self.armed = false;
        ExecutableSnapshot {
            parent: self.parent.take().expect("snapshot staging parent"),
            directory: self.directory.clone(),
            path: self.path.clone(),
            pinned,
            cleaned: false,
        }
    }
}

#[cfg(target_os = "macos")]
impl Drop for SnapshotStaging {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let _ = fs::remove_file(&self.path);
        if let Ok(directory) = open_directory(&self.directory) {
            let _ = directory.sync_all();
        }
        let _ = fs::remove_dir(&self.directory);
        if let Some(parent) = &self.parent {
            let _ = parent.sync_all();
        }
    }
}

#[cfg(target_os = "macos")]
impl ExecutableSnapshot {
    fn create(
        source: &mut kagemusha_python_launcher::PinnedFile,
        expected_sha256: [u8; 32],
    ) -> Result<Self> {
        use rustix::fs::{Mode, OFlags, fchmod, openat};
        let parent_path = Path::new(SNAPSHOT_PARENT);
        kagemusha_python_launcher::require_root_custody(parent_path, true)?;
        let staging = SnapshotStaging::create(parent_path)?;
        fs::set_permissions(&staging.directory, fs::Permissions::from_mode(0o700)).map_err(
            |_| ControllerError::policy("promotion snapshot directory mode could not be sealed"),
        )?;
        kagemusha_python_launcher::require_root_custody(&staging.directory, true)?;
        let run = open_directory(&staging.directory)?;
        let mut output = File::from(
            openat(
                &run,
                "kagami",
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::from_raw_mode(0o700),
            )
            .map_err(|_| ControllerError::policy("promotion Kagami snapshot creation failed"))?,
        );
        let source_size = source
            .file_mut()
            .metadata()
            .map_err(|_| ControllerError::policy("authenticated Kagami metadata is unavailable"))?
            .size();
        source
            .file_mut()
            .seek(SeekFrom::Start(0))
            .map_err(|_| ControllerError::policy("authenticated Kagami rewind failed"))?;
        let mut limited_source = Read::take(source.file_mut(), MAX_KAGAMI_BYTES + 1);
        let copied = io::copy(&mut limited_source, &mut output)
            .map_err(|_| ControllerError::policy("authenticated Kagami snapshot copy failed"))?;
        drop(limited_source);
        source
            .file_mut()
            .seek(SeekFrom::Start(0))
            .map_err(|_| ControllerError::policy("authenticated Kagami rewind failed"))?;
        if copied != source_size || copied == 0 || copied > MAX_KAGAMI_BYTES {
            return Err(ControllerError::policy(
                "authenticated Kagami snapshot length is invalid",
            ));
        }
        output
            .sync_all()
            .map_err(|_| ControllerError::policy("promotion Kagami snapshot fsync failed"))?;
        fchmod(&output, Mode::from_raw_mode(0o500))
            .map_err(|_| ControllerError::policy("promotion Kagami snapshot mode seal failed"))?;
        output
            .sync_all()
            .map_err(|_| ControllerError::policy("promotion Kagami snapshot seal fsync failed"))?;
        run.sync_all()
            .map_err(|_| ControllerError::policy("promotion snapshot directory fsync failed"))?;
        staging
            .parent
            .as_ref()
            .expect("snapshot staging parent")
            .sync_all()
            .map_err(|_| ControllerError::policy("promotion snapshot parent fsync failed"))?;
        drop(output);
        let pinned = kagemusha_python_launcher::pin_regular(&staging.path, expected_sha256)?;
        kagemusha_python_launcher::validate_pinned(source)?;
        Ok(staging.finish(pinned))
    }

    fn verify(&mut self) -> Result<()> {
        kagemusha_python_launcher::validate_pinned(&mut self.pinned)
    }

    fn cleanup(mut self) -> Result<()> {
        self.verify()?;
        fs::remove_file(&self.path)
            .map_err(|_| ControllerError::policy("promotion Kagami snapshot removal failed"))?;
        open_directory(&self.directory)?
            .sync_all()
            .map_err(|_| ControllerError::policy("promotion snapshot cleanup fsync failed"))?;
        fs::remove_dir(&self.directory)
            .map_err(|_| ControllerError::policy("promotion snapshot directory removal failed"))?;
        self.parent.sync_all().map_err(|_| {
            ControllerError::policy("promotion snapshot parent cleanup fsync failed")
        })?;
        self.cleaned = true;
        Ok(())
    }
}

#[cfg(target_os = "macos")]
impl Drop for ExecutableSnapshot {
    fn drop(&mut self) {
        if !self.cleaned {
            let _ = fs::remove_file(&self.path);
            let _ = fs::remove_dir(&self.directory);
            let _ = self.parent.sync_all();
        }
    }
}

fn seatbelt_literal(path: &Path) -> Result<String> {
    let text = path
        .to_str()
        .ok_or_else(|| ControllerError::policy("promotion Seatbelt path is not UTF-8"))?;
    if text.len() > 4096 || text.chars().any(char::is_control) {
        return Err(ControllerError::policy(
            "promotion Seatbelt path is not representable",
        ));
    }
    let mut literal = String::with_capacity(text.len() + 2);
    literal.push('"');
    for character in text.chars() {
        match character {
            '\\' => literal.push_str("\\\\"),
            '"' => literal.push_str("\\\""),
            _ => literal.push(character),
        }
    }
    literal.push('"');
    Ok(literal)
}

fn seatbelt_temp_regex(bundle: &Path) -> Result<String> {
    let prefix = bundle.join(TEMP_PREFIX);
    let text = prefix
        .to_str()
        .ok_or_else(|| ControllerError::policy("promotion Seatbelt path is not UTF-8"))?;
    if text.len() > 4096 || text.chars().any(char::is_control) {
        return Err(ControllerError::policy(
            "promotion Seatbelt path is not representable",
        ));
    }
    let mut regex = String::from("#\"^");
    for character in text.chars() {
        if matches!(
            character,
            '\\' | '"'
                | '.'
                | '^'
                | '$'
                | '*'
                | '+'
                | '?'
                | '('
                | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | '|'
        ) {
            regex.push('\\');
        }
        regex.push(character);
    }
    for _ in 0..32 {
        regex.push_str("[0-9a-f]");
    }
    regex.push_str("$\"");
    Ok(regex)
}

fn sandbox_profile(executable: &Path, bundle: &Path, release_policy: &Path) -> Result<String> {
    let mut ancestors = BTreeSet::new();
    for path in [executable, bundle, release_policy] {
        for ancestor in path.ancestors().skip(1) {
            ancestors.insert(ancestor.to_path_buf());
        }
    }
    let executable = seatbelt_literal(executable)?;
    let bundle_literal = seatbelt_literal(bundle)?;
    let policy = seatbelt_literal(release_policy)?;
    let final_leaf = seatbelt_literal(&bundle.join(FINAL_NAME))?;
    let temporary = seatbelt_temp_regex(bundle)?;
    let mut profile = format!(
        "(version 1)\n(deny default)\n(allow sysctl-read (sysctl-name \"hw.memsize\" \"hw.pagesize\" \"hw.pagesize_compat\"))\n(allow process-info* (target self))\n(allow signal (target self))\n(allow process-exec (literal {executable}))\n(deny network*)\n(deny process-fork)\n(deny file-link)\n(deny file-clone)\n(allow file-read-data (literal \"/\"))\n"
    );
    profile.push_str("(allow file-read-metadata");
    for ancestor in ancestors {
        profile.push_str(" (literal ");
        profile.push_str(&seatbelt_literal(&ancestor)?);
        profile.push(')');
    }
    profile.push_str(")\n");
    profile.push_str(&format!(
        "(allow file-read* (literal {executable}) (literal {policy}) (literal {bundle_literal}) (subpath {bundle_literal}))\n"
    ));
    for root in [
        "/usr/lib",
        "/System/Library",
        "/System/Volumes/Preboot/Cryptexes/OS/System/Library",
    ] {
        let literal = seatbelt_literal(Path::new(root))?;
        profile.push_str(&format!(
            "(allow file-read* (literal {literal}) (subpath {literal}))\n"
        ));
    }
    for device in ["/dev/null", "/dev/random", "/dev/urandom"] {
        profile.push_str(&format!(
            "(allow file-read* (literal {}))\n",
            seatbelt_literal(Path::new(device))?
        ));
    }
    profile.push_str(&format!(
        "(allow file-write* (require-all (vnode-type REGULAR-FILE) (regex {temporary})))\n(allow file-write-create (require-all (vnode-type REGULAR-FILE) (literal {final_leaf})))\n(allow file-write-data (literal {bundle_literal}))\n"
    ));
    Ok(profile)
}

#[cfg(target_os = "macos")]
struct Captured {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

#[cfg(target_os = "macos")]
fn run_sandboxed(mut command: Command, candidate: &mut CandidateSnapshot) -> Result<Captured> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_isolated_child(&mut command, MAX_PROMOTION_BYTES);
    let mut child = command
        .spawn()
        .map_err(|_| ControllerError::policy("promotion Seatbelt launch failed"))?;
    let process_group = child.id() as i32;
    let watchdog = match super::Watchdog::start(process_group) {
        Ok(watchdog) => watchdog,
        Err(error) => {
            terminate_unwatched_job(&mut child, process_group)?;
            return Err(error);
        }
    };
    let mut job = super::MacosJob::new(child, process_group, watchdog);
    let stdout = job
        .child
        .stdout
        .take()
        .ok_or_else(|| ControllerError::policy("promotion stdout pipe is unavailable"))?;
    let stderr = job
        .child
        .stderr
        .take()
        .ok_or_else(|| ControllerError::policy("promotion stderr pipe is unavailable"))?;
    let combined = Arc::new(AtomicU64::new(0));
    let overflow = Arc::new(AtomicBool::new(false));
    let stdout_reader = bounded_reader(
        stdout,
        MAX_STDOUT_BYTES,
        Some(MAX_STDOUT_BYTES + MAX_STDERR_BYTES),
        Arc::clone(&combined),
        Arc::clone(&overflow),
    );
    let stderr_reader = bounded_reader(
        stderr,
        MAX_STDERR_BYTES,
        Some(MAX_STDOUT_BYTES + MAX_STDERR_BYTES),
        Arc::clone(&combined),
        Arc::clone(&overflow),
    );
    let started = Instant::now();
    let mut failure = None;
    let status = loop {
        let monitor = candidate.phase();
        if overflow.load(Ordering::Acquire) {
            failure = Some(ControllerError::limit(
                "Kagami promotion output exceeded its bound",
            ));
        } else if started.elapsed() >= Duration::from_secs(WALL_SECONDS) {
            failure = Some(ControllerError::limit(
                "Kagami promotion exceeded its wall-time bound",
            ));
        } else if let Err(error) = monitor {
            failure = Some(error);
        }
        if failure.is_some() {
            terminate_process_group(&mut job.child, process_group)?;
            break job
                .child
                .wait()
                .map_err(|_| ControllerError::policy("promotion child reap failed"))?;
        }
        match job
            .child
            .try_wait()
            .map_err(|_| ControllerError::policy("promotion child status is unavailable"))?
        {
            Some(status) => break status,
            None => thread::sleep(MONITOR_INTERVAL),
        }
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| ControllerError::policy("promotion stdout reader failed"))?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| ControllerError::policy("promotion stderr reader failed"))?;
    ensure_empty_process_group(process_group)?;
    job.finish_watchdog()?;
    if overflow.load(Ordering::Acquire) {
        failure = Some(ControllerError::limit(
            "Kagami promotion output exceeded its bound",
        ));
    }
    if stdout.io_failed || stderr.io_failed {
        failure = Some(ControllerError::policy(
            "Kagami promotion diagnostic pipe failed",
        ));
    }
    if let Some(error) = failure {
        return Err(error);
    }
    Ok(Captured {
        status,
        stdout: stdout.bytes,
        stderr: stderr.bytes,
    })
}

#[cfg(target_os = "macos")]
fn child_exit(status: ExitStatus) -> Result<u8> {
    if let Some(code) = status.code() {
        return u8::try_from(code)
            .map_err(|_| ControllerError::policy("Kagami returned an invalid exit status"));
    }
    let signal = status
        .signal()
        .ok_or_else(|| ControllerError::policy("Kagami termination status is unavailable"))?;
    Ok(u8::try_from(128_i32.saturating_add(signal)).unwrap_or(u8::MAX))
}

#[cfg(target_os = "macos")]
fn forward_stderr(bytes: &[u8]) -> Result<()> {
    io::stderr()
        .write_all(bytes)
        .and_then(|_| io::stderr().flush())
        .map_err(|_| ControllerError::policy("Kagami promotion stderr forwarding failed"))
}

#[cfg(target_os = "macos")]
fn promote_macos(request: PromotionRequest) -> Result<u8> {
    kagemusha_python_launcher::validate_root_launch_identity()?;
    kagemusha_python_launcher::require_macos_tcb(&request.expected_macos_build)?;
    kagemusha_python_launcher::require_root_custody(Path::new(SANDBOX_EXEC), false)?;
    let controller = std::env::current_exe()
        .and_then(fs::canonicalize)
        .map_err(|_| ControllerError::policy("promotion controller identity is unavailable"))?;
    kagemusha_python_launcher::require_root_custody(&controller, false)?;
    let kagami = canonical_existing(&request.kagami, false, "Kagami executable")?;
    let bundle = canonical_existing(&request.bundle_dir, true, "candidate bundle")?;
    let policy = canonical_existing(&request.release_policy, false, "release policy")?;
    let bundle_leaf = bundle
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| ControllerError::policy("candidate bundle name is invalid"))?;
    parse_sha256(bundle_leaf, "candidate bundle manifest digest")?;
    if kagami.starts_with(&bundle)
        || policy.starts_with(&bundle)
        || kagami == policy
        || kagami.starts_with(Path::new(SNAPSHOT_PARENT))
        || policy.starts_with(Path::new(SNAPSHOT_PARENT))
        || bundle.starts_with(Path::new(SNAPSHOT_PARENT))
    {
        return Err(ControllerError::policy(
            "promotion executable, policy, candidate, and snapshot roots are not distinct",
        ));
    }
    kagemusha_python_launcher::require_root_custody(&kagami, false)?;
    kagemusha_python_launcher::require_root_custody(&bundle, true)?;
    kagemusha_python_launcher::require_root_custody(&policy, false)?;
    let mut kagami_pin = kagemusha_python_launcher::pin_regular(&kagami, request.kagami_sha256)?;
    let kagami_metadata = kagami_pin
        .file_mut()
        .metadata()
        .map_err(|_| ControllerError::policy("authenticated Kagami metadata is unavailable"))?;
    if kagami_metadata.size() > MAX_KAGAMI_BYTES
        || kagami_metadata.mode() & 0o111 == 0
        || kagami_metadata.mode() & 0o6000 != 0
    {
        return Err(ControllerError::policy(
            "authenticated Kagami executable mode or size is unsafe",
        ));
    }
    let mut candidate = CandidateSnapshot::open(&bundle)?;
    let mut policy_pin = PinnedInput::open(&policy, MAX_POLICY_BYTES)?;
    if candidate.files.values().any(|(_, identity)| {
        identity.device == policy_pin.identity.device && identity.inode == policy_pin.identity.inode
    }) {
        return Err(ControllerError::policy(
            "release policy hard-link aliases the candidate inventory",
        ));
    }
    let mut snapshot = ExecutableSnapshot::create(&mut kagami_pin, request.kagami_sha256)?;
    let profile = sandbox_profile(&snapshot.path, &bundle, &policy)?;
    super::validate_sandbox_profile(&profile)?;
    if candidate.phase()? != PublicationPhase::Candidate {
        return Err(ControllerError::policy(
            "promotion final leaf appeared before execution",
        ));
    }
    snapshot.verify()?;
    kagemusha_python_launcher::validate_pinned(&mut kagami_pin)?;
    let final_path = bundle.join(FINAL_NAME);
    let benchmark = bundle.join(BENCHMARK_NAME);
    let review = bundle.join(REVIEW_NAME);
    let mut command = Command::new(SANDBOX_EXEC);
    command
        .arg("-p")
        .arg(profile)
        .arg(&snapshot.path)
        .arg("kagemusha")
        .arg("promote-release-v4")
        .arg("--bundle-dir")
        .arg(&bundle)
        .arg("--release-policy")
        .arg(&policy)
        .arg("--promotion-record")
        .arg(&final_path)
        .arg("--benchmark-evidence")
        .arg(&benchmark)
        .arg("--cryptographic-review")
        .arg(&review)
        .current_dir("/")
        .env_clear()
        .env("HOME", "/var/empty")
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .env("PATH", "/usr/bin:/bin")
        .env("TMPDIR", "/private/var/tmp")
        .env("TZ", "UTC");
    let execution = run_sandboxed(command, &mut candidate);
    let phase = candidate.phase();
    let committed_or_ambiguous = !matches!(&phase, Ok(PublicationPhase::Candidate));
    let cleanup = snapshot.cleanup();
    let captured = match execution {
        Ok(captured) => captured,
        Err(error) => {
            if committed_or_ambiguous || cleanup.is_err() {
                return Err(commit_uncertain(error.message));
            }
            return Err(error);
        }
    };
    if let Err(error) = forward_stderr(&captured.stderr) {
        return if committed_or_ambiguous {
            Err(commit_uncertain(error.message))
        } else {
            Err(error)
        };
    }
    let exit = match child_exit(captured.status) {
        Ok(exit) => exit,
        Err(error) if committed_or_ambiguous => return Err(commit_uncertain(error.message)),
        Err(error) => return Err(error),
    };
    if exit != 0 {
        return failed_child_result(exit, phase.ok(), cleanup.is_ok());
    }
    match phase {
        Ok(PublicationPhase::Committed) => {}
        Ok(PublicationPhase::Candidate) => {
            if let Err(error) = cleanup {
                return Err(error);
            }
            return Err(ControllerError::policy(
                "Kagami reported success without publishing the fixed final leaf",
            ));
        }
        Ok(PublicationPhase::Staging) | Err(_) => {
            return Err(commit_uncertain(
                "successful Kagami exit left an ambiguous promotion directory",
            ));
        }
    }
    if cleanup.is_err() {
        return Err(commit_uncertain(
            "authenticated executable snapshot cleanup was ambiguous",
        ));
    }
    let verification = (|| {
        let final_identity = candidate.verify_committed()?;
        policy_pin.verify()?;
        kagemusha_python_launcher::validate_pinned(&mut kagami_pin)?;
        let expected_report = candidate.report_expectation(
            bundle_leaf,
            policy_pin.identity.sha256,
            final_identity.sha256,
        )?;
        canonical_report(&captured.stdout, &expected_report)?;
        if !captured.stderr.is_empty() {
            return Err(ControllerError::policy(
                "successful Kagami promotion emitted unexpected stderr",
            ));
        }
        Ok(())
    })();
    if let Err(error) = verification {
        return Err(commit_uncertain(error.message));
    }
    io::stdout()
        .write_all(&captured.stdout)
        .and_then(|_| io::stdout().flush())
        .map_err(|_| commit_uncertain("Kagami report forwarding failed"))?;
    Ok(0)
}

#[cfg(test)]
#[path = "kagemusha_promotion_publisher_tests.rs"]
mod tests;
