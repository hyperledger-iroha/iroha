//! Authenticated, one-shot zk-X.509 prover and signer worker.
//!
//! Normal operation reads a 32-byte session authentication key followed by
//! one HMAC-authenticated request frame from stdin.  The request names an
//! owner-only secret bundle and a canonical public-request JSON file; stdout
//! contains only an authenticated identity response, a non-consuming bundle
//! admission receipt, or a complete signed transaction. The `bundle`
//! subcommand is the sole native secret-bundle writer and never renders secret
//! bytes.

use iroha_core::{
    privacy_profiles::compiled_privacy_profile_v1,
    privacy_release_evidence::{
        PRIVACY_ZK_X509_WORKER_PROTOCOL_VERSION_V1,
        PRIVACY_ZK_X509_WORKER_PUBLIC_REQUEST_VERSION_V1, PrivacyZkX509WorkerErrorV1,
        PrivacyZkX509WorkerPublicRequestV1, build_signed_privacy_zk_x509_worker_action_v1,
        initialize_privacy_release_rayon_pool_v1, privacy_zk_x509_worker_release_pins_v1,
        validate_privacy_zk_x509_worker_inputs_v1,
    },
};
use iroha_data_model::privacy::PrivacyProtocolIdV1;
use iroha_version::codec::EncodeVersioned;
use norito::derive::{JsonDeserialize, JsonSerialize};
use rand_core_06::{OsRng, RngCore as _};
use sha2::{Digest as _, Sha256};
use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{self, BufReader, BufWriter, Read, Seek as _, SeekFrom, Write},
    path::{Component, Path, PathBuf},
};
use zeroize::{Zeroize as _, Zeroizing};

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
#[path = "iroha_zk_x509_prover_worker/linux_isolation.rs"]
mod linux_isolation;

const FRAME_MAGIC: &[u8; 4] = b"X5PW";
const BUNDLE_MAGIC: &[u8; 4] = b"X5WB";
const PROTOCOL_VERSION: u8 = PRIVACY_ZK_X509_WORKER_PROTOCOL_VERSION_V1;
const COMMAND_IDENTITY: u8 = 1;
const COMMAND_EXECUTE: u8 = 2;
const COMMAND_ADMIT_BUNDLE: u8 = 3;
const AUTH_TAG_BYTES: usize = 32;
const MAX_FRAME_BYTES: usize = 12 * 1024 * 1024;
const MAX_PUBLIC_REQUEST_BYTES: u64 = 1024 * 1024;
const MAX_WITNESS_BYTES: usize = 64 * 1024;
const MAX_WITNESS_BYTES_U64: u64 = 64 * 1024;
const MAX_BUNDLE_BYTES: u64 = 4 + 1 + 32 + 32 + 4 + MAX_WITNESS_BYTES_U64;
const MAX_PATH_BYTES: usize = 4096;
const BUNDLE_HEADER_BYTES: usize = 4 + 1 + 32 + 32 + 4;
const RESPONSE_OK: u8 = 0;
const RESPONSE_ERROR: u8 = 1;
const ERROR_REQUEST: u8 = 1;
const ERROR_PROFILE_UNAVAILABLE: u8 = 2;
const ERROR_CUSTODY: u8 = 3;
const ERROR_WITNESS: u8 = 4;
const ERROR_PROOF: u8 = 5;
const ERROR_FINALIZATION: u8 = 6;
const ERROR_ISOLATION_UNAVAILABLE: u8 = 7;
const QUALIFIED_ISOLATION_CONTRACT_V1: &str = "iroha.zk-x509.qualified-linux-aarch64-launcher.v1";
const UNAVAILABLE_ISOLATION_CONTRACT_V1: &str =
    "iroha.zk-x509.qualified-linux-aarch64-launcher.v1:unavailable";

#[derive(Debug, JsonDeserialize, JsonSerialize)]
#[norito(deny_unknown_fields)]
struct ExecuteRequestV1 {
    schema_version: u8,
    public_request_path: String,
    public_request_sha256: [u8; 32],
    secret_bundle_path: String,
    secret_bundle_sha256: [u8; 32],
}

#[derive(JsonSerialize)]
struct IdentityResponseV2 {
    artifact_self_hash_required: bool,
    cargo_lock_sha256: String,
    compiled_profile_sha256: Option<String>,
    expectations_json_sha256: Option<String>,
    expectations_norito_sha256: Option<String>,
    operation: String,
    production_profile_ready: bool,
    protocol_id: String,
    protocol_profile_sha256: String,
    protocol_version: u8,
    public_request_schema_version: u8,
    qualified_isolation_ready: bool,
    isolation_contract: String,
    isolation_package_sha256: Option<String>,
    kat_proof_bytes: u32,
    kat_proof_sha256: Option<String>,
    release_evidence_ready: bool,
    release_evidence_sha256: Option<String>,
    resource_certificate_sha256: Option<String>,
    schema: String,
    schema_version: u8,
    soundness_certificate_sha256: Option<String>,
    source_allowed_signers_sha256: String,
    source_closure_schema: String,
    source_commit: String,
    source_revocation_sha256: String,
    source_sha256: String,
    workspace_source_manifest_sha256: String,
}

#[derive(Debug)]
struct RequestFrame {
    command: u8,
    sequence: u64,
    payload: Vec<u8>,
}

#[derive(Clone, Copy, Debug)]
enum WorkerFailure {
    Request,
    ProfileUnavailable,
    Custody,
    Witness,
    Proof,
    Finalization,
    IsolationUnavailable,
}

impl WorkerFailure {
    const fn code(self) -> u8 {
        match self {
            Self::Request => ERROR_REQUEST,
            Self::ProfileUnavailable => ERROR_PROFILE_UNAVAILABLE,
            Self::Custody => ERROR_CUSTODY,
            Self::Witness => ERROR_WITNESS,
            Self::Proof => ERROR_PROOF,
            Self::Finalization => ERROR_FINALIZATION,
            Self::IsolationUnavailable => ERROR_ISOLATION_UNAVAILABLE,
        }
    }
}

impl From<PrivacyZkX509WorkerErrorV1> for WorkerFailure {
    fn from(error: PrivacyZkX509WorkerErrorV1) -> Self {
        match error {
            PrivacyZkX509WorkerErrorV1::InvalidPublicRequest(_) => Self::Request,
            PrivacyZkX509WorkerErrorV1::ProfileUnavailable => Self::ProfileUnavailable,
            PrivacyZkX509WorkerErrorV1::SignerCustody => Self::Custody,
            PrivacyZkX509WorkerErrorV1::WitnessPreparation => Self::Witness,
            PrivacyZkX509WorkerErrorV1::ProofConstruction => Self::Proof,
            PrivacyZkX509WorkerErrorV1::TransactionFinalization => Self::Finalization,
        }
    }
}

#[cfg(all(unix, not(target_os = "haiku")))]
fn harden_process() -> Result<(), ()> {
    rustix::process::setrlimit(
        rustix::process::Resource::Core,
        rustix::process::Rlimit {
            current: Some(0),
            maximum: Some(0),
        },
    )
    .map_err(|_| ())?;
    #[cfg(target_os = "linux")]
    rustix::process::set_dumpable_behavior(rustix::process::DumpableBehavior::NotDumpable)
        .map_err(|_| ())?;
    Ok(())
}

#[cfg(any(not(unix), target_os = "haiku"))]
fn harden_process() -> Result<(), ()> {
    Err(())
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn hmac_sha256(key: &[u8; 32], message: &[u8]) -> [u8; AUTH_TAG_BYTES] {
    const BLOCK_BYTES: usize = 64;
    let mut inner_key = Zeroizing::new([0x36_u8; BLOCK_BYTES]);
    let mut outer_key = Zeroizing::new([0x5c_u8; BLOCK_BYTES]);
    for (index, byte) in key.iter().enumerate() {
        inner_key[index] ^= byte;
        outer_key[index] ^= byte;
    }
    let mut inner = Sha256::new();
    inner.update(&inner_key[..]);
    inner.update(message);
    let inner_digest = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(&outer_key[..]);
    outer.update(inner_digest);
    outer.finalize().into()
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0_u8;
    for (left, right) in left.iter().zip(right) {
        difference |= left ^ right;
    }
    difference == 0
}

fn read_request_frame(
    reader: &mut impl Read,
    auth_key: &[u8; 32],
) -> Result<RequestFrame, WorkerFailure> {
    let mut length_bytes = [0_u8; 4];
    reader
        .read_exact(&mut length_bytes)
        .map_err(|_| WorkerFailure::Request)?;
    let length =
        usize::try_from(u32::from_be_bytes(length_bytes)).map_err(|_| WorkerFailure::Request)?;
    if !(18 + AUTH_TAG_BYTES..=MAX_FRAME_BYTES).contains(&length) {
        return Err(WorkerFailure::Request);
    }
    let mut encoded = vec![0_u8; length];
    reader
        .read_exact(&mut encoded)
        .map_err(|_| WorkerFailure::Request)?;
    let authenticated_end = encoded
        .len()
        .checked_sub(AUTH_TAG_BYTES)
        .ok_or(WorkerFailure::Request)?;
    let authenticated = &encoded[..authenticated_end];
    let actual_tag = &encoded[authenticated_end..];
    if !constant_time_eq(actual_tag, &hmac_sha256(auth_key, authenticated))
        || authenticated.get(..4) != Some(FRAME_MAGIC)
        || authenticated.get(4) != Some(&PROTOCOL_VERSION)
    {
        return Err(WorkerFailure::Request);
    }
    let command = *authenticated.get(5).ok_or(WorkerFailure::Request)?;
    let sequence = u64::from_be_bytes(
        authenticated
            .get(6..14)
            .ok_or(WorkerFailure::Request)?
            .try_into()
            .map_err(|_| WorkerFailure::Request)?,
    );
    let payload_length = usize::try_from(u32::from_be_bytes(
        authenticated
            .get(14..18)
            .ok_or(WorkerFailure::Request)?
            .try_into()
            .map_err(|_| WorkerFailure::Request)?,
    ))
    .map_err(|_| WorkerFailure::Request)?;
    if sequence == 0 || payload_length != authenticated.len().saturating_sub(18) {
        return Err(WorkerFailure::Request);
    }
    Ok(RequestFrame {
        command,
        sequence,
        payload: authenticated[18..].to_vec(),
    })
}

fn write_response_frame(
    writer: &mut impl Write,
    command: u8,
    sequence: u64,
    payload: &[u8],
    auth_key: &[u8; 32],
) -> Result<(), WorkerFailure> {
    if sequence == 0 || payload.len() > MAX_FRAME_BYTES - 18 - AUTH_TAG_BYTES {
        return Err(WorkerFailure::Finalization);
    }
    let payload_length = u32::try_from(payload.len()).map_err(|_| WorkerFailure::Finalization)?;
    let mut authenticated = Vec::with_capacity(18 + payload.len() + AUTH_TAG_BYTES);
    authenticated.extend_from_slice(FRAME_MAGIC);
    authenticated.push(PROTOCOL_VERSION);
    authenticated.push(command);
    authenticated.extend_from_slice(&sequence.to_be_bytes());
    authenticated.extend_from_slice(&payload_length.to_be_bytes());
    authenticated.extend_from_slice(payload);
    let tag = hmac_sha256(auth_key, &authenticated);
    authenticated.extend_from_slice(&tag);
    let frame_length =
        u32::try_from(authenticated.len()).map_err(|_| WorkerFailure::Finalization)?;
    writer
        .write_all(&frame_length.to_be_bytes())
        .and_then(|()| writer.write_all(&authenticated))
        .and_then(|()| writer.flush())
        .map_err(|_| WorkerFailure::Finalization)
}

fn validate_absolute_path(value: &str) -> Result<PathBuf, WorkerFailure> {
    if value.is_empty()
        || value.len() > MAX_PATH_BYTES
        || value.contains('\0')
        || !Path::new(value).is_absolute()
        || Path::new(value)
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
    {
        return Err(WorkerFailure::Request);
    }
    Ok(PathBuf::from(value))
}

#[cfg(unix)]
fn validate_owner_only_metadata(metadata: &fs::Metadata) -> Result<(), WorkerFailure> {
    use std::os::unix::fs::MetadataExt as _;
    if !metadata.is_file()
        || metadata.nlink() != 1
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o077 != 0
    {
        return Err(WorkerFailure::Custody);
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_owner_only_metadata(_metadata: &fs::Metadata) -> Result<(), WorkerFailure> {
    Err(WorkerFailure::Custody)
}

#[cfg(unix)]
fn same_file_snapshot(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.uid() == right.uid()
        && left.mode() == right.mode()
        && left.nlink() == right.nlink()
        && left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
}

#[cfg(unix)]
fn same_file_snapshot_except_length(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.mode() == right.mode()
        && left.uid() == right.uid()
        && left.nlink() == right.nlink()
}

#[cfg(unix)]
struct SecretBundleCleanupGuardV1 {
    temporary_path: PathBuf,
    published_path: PathBuf,
    expected: fs::Metadata,
    rename_attempted: bool,
    armed: bool,
}

#[cfg(unix)]
impl SecretBundleCleanupGuardV1 {
    fn new(temporary_path: PathBuf, published_path: PathBuf, expected: fs::Metadata) -> Self {
        Self {
            temporary_path,
            published_path,
            expected,
            rename_attempted: false,
            armed: true,
        }
    }

    fn mark_rename_attempted(&mut self) {
        // Once rename enters the kernel, an error is treated as an uncertain
        // publication state. Cleanup probes both names and removes only the
        // exact inode created by this invocation.
        self.rename_attempted = true;
    }

    fn disarm(&mut self) {
        self.armed = false;
    }

    fn remove_if_owned(&self, path: &Path) -> Result<(), WorkerFailure> {
        let observed = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(_) => return Err(WorkerFailure::Custody),
        };
        if observed.file_type().is_symlink()
            || !observed.is_file()
            || !same_file_snapshot_except_length(&self.expected, &observed)
        {
            return Err(WorkerFailure::Custody);
        }
        fs::remove_file(path).map_err(|_| WorkerFailure::Custody)
    }

    fn cleanup(&self) -> Result<(), WorkerFailure> {
        let temporary = self.remove_if_owned(&self.temporary_path);
        let published = if self.rename_attempted {
            self.remove_if_owned(&self.published_path)
        } else {
            Ok(())
        };
        let synchronized = self
            .temporary_path
            .parent()
            .ok_or(WorkerFailure::Custody)
            .and_then(|parent| {
                File::open(parent)
                    .and_then(|directory| directory.sync_all())
                    .map_err(|_| WorkerFailure::Custody)
            });
        temporary.and(published).and(synchronized)
    }
}

#[cfg(unix)]
impl Drop for SecretBundleCleanupGuardV1 {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.cleanup();
        }
    }
}

#[cfg(unix)]
fn remove_unadmitted_secret_path_v1(path: &Path, parent: &Path) -> Result<(), WorkerFailure> {
    fs::remove_file(path).map_err(|_| WorkerFailure::Custody)?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| WorkerFailure::Custody)
}

#[cfg(unix)]
fn persist_secret_bundle(path: &Path, bytes: &[u8]) -> Result<(), WorkerFailure> {
    use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};

    let bytes_len = u64::try_from(bytes.len()).map_err(|_| WorkerFailure::Custody)?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or(WorkerFailure::Custody)?;
    let parent_metadata = fs::symlink_metadata(parent).map_err(|_| WorkerFailure::Custody)?;
    if parent_metadata.file_type().is_symlink()
        || !parent_metadata.is_dir()
        || parent_metadata.uid() != rustix::process::geteuid().as_raw()
        || parent_metadata.mode() & 0o077 != 0
        || fs::canonicalize(parent).map_err(|_| WorkerFailure::Custody)? != parent
    {
        return Err(WorkerFailure::Custody);
    }

    let mut suffix = Zeroizing::new([0_u8; 16]);
    OsRng.fill_bytes(&mut suffix[..]);
    let temporary_path = parent.join(format!(
        ".iroha-zk-x509-secret-bundle-v1.{}.tmp",
        hex::encode(&suffix[..])
    ));
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .truncate(false)
        .mode(0o600)
        .open(&temporary_path)
        .map_err(|_| WorkerFailure::Custody)?;
    let created = match file.metadata() {
        Ok(metadata) => metadata,
        Err(_) => {
            remove_unadmitted_secret_path_v1(&temporary_path, parent)?;
            return Err(WorkerFailure::Custody);
        }
    };
    let mut cleanup =
        SecretBundleCleanupGuardV1::new(temporary_path.clone(), path.to_path_buf(), created);
    file.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|_| WorkerFailure::Custody)?;
    let opened = file.metadata().map_err(|_| WorkerFailure::Custody)?;
    if !opened.is_file()
        || opened.uid() != rustix::process::geteuid().as_raw()
        || opened.mode() & 0o777 != 0o600
        || opened.nlink() != 1
        || opened.len() != 0
    {
        return Err(WorkerFailure::Custody);
    }

    file.write_all(bytes).map_err(|_| WorkerFailure::Custody)?;
    file.sync_all().map_err(|_| WorkerFailure::Custody)?;
    file.seek(SeekFrom::Start(0))
        .map_err(|_| WorkerFailure::Custody)?;
    let mut readback = Zeroizing::new(Vec::with_capacity(bytes.len()));
    Read::by_ref(&mut file)
        .take(bytes_len.saturating_add(1))
        .read_to_end(&mut readback)
        .map_err(|_| WorkerFailure::Custody)?;
    if readback.as_slice() != bytes {
        return Err(WorkerFailure::Custody);
    }
    let written = file.metadata().map_err(|_| WorkerFailure::Custody)?;
    if !same_file_snapshot_except_length(&opened, &written)
        || written.len() != bytes_len
        || written.mode() & 0o777 != 0o600
        || written.nlink() != 1
    {
        return Err(WorkerFailure::Custody);
    }
    cleanup.mark_rename_attempted();
    rustix::fs::renameat_with(
        rustix::fs::CWD,
        &temporary_path,
        rustix::fs::CWD,
        path,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(|_| WorkerFailure::Custody)?;
    let published = fs::symlink_metadata(path).map_err(|_| WorkerFailure::Custody)?;
    if published.file_type().is_symlink()
        || !published.is_file()
        || !same_file_snapshot_except_length(&written, &published)
        || published.len() != bytes_len
        || published.mode() & 0o777 != 0o600
        || published.nlink() != 1
    {
        return Err(WorkerFailure::Custody);
    }
    drop(file);
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| WorkerFailure::Custody)?;
    cleanup.disarm();
    Ok(())
}

#[cfg(not(unix))]
fn persist_secret_bundle(_path: &Path, _bytes: &[u8]) -> Result<(), WorkerFailure> {
    Err(WorkerFailure::Custody)
}

#[cfg(not(unix))]
fn same_file_snapshot(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.len() == right.len()
        && left.created().ok() == right.created().ok()
        && left.modified().ok() == right.modified().ok()
}

fn read_stable_file_with_metadata(
    path: &Path,
    maximum: u64,
    owner_only: bool,
) -> Result<(Zeroizing<Vec<u8>>, fs::Metadata), WorkerFailure> {
    let before = fs::symlink_metadata(path).map_err(|_| WorkerFailure::Custody)?;
    if before.file_type().is_symlink() || !before.is_file() || before.len() > maximum {
        return Err(WorkerFailure::Custody);
    }
    if owner_only {
        validate_owner_only_metadata(&before)?;
    }
    let mut file = File::open(path).map_err(|_| WorkerFailure::Custody)?;
    let opened = file.metadata().map_err(|_| WorkerFailure::Custody)?;
    if !same_file_snapshot(&before, &opened) {
        return Err(WorkerFailure::Custody);
    }
    let capacity = usize::try_from(opened.len()).map_err(|_| WorkerFailure::Custody)?;
    let mut bytes = Zeroizing::new(Vec::with_capacity(capacity));
    {
        let mut bounded = (&mut file).take(opened.len());
        bounded
            .read_to_end(&mut bytes)
            .map_err(|_| WorkerFailure::Custody)?;
    }
    let mut trailing = [0_u8; 1];
    let trailing_bytes = file
        .read(&mut trailing)
        .map_err(|_| WorkerFailure::Custody)?;
    let after = file.metadata().map_err(|_| WorkerFailure::Custody)?;
    let observed_bytes = u64::try_from(bytes.len()).map_err(|_| WorkerFailure::Custody)?;
    if !same_file_snapshot(&opened, &after)
        || observed_bytes != after.len()
        || observed_bytes > maximum
        || trailing_bytes != 0
    {
        bytes.zeroize();
        return Err(WorkerFailure::Custody);
    }
    Ok((bytes, after))
}

fn read_stable_file(
    path: &Path,
    maximum: u64,
    owner_only: bool,
) -> Result<Zeroizing<Vec<u8>>, WorkerFailure> {
    read_stable_file_with_metadata(path, maximum, owner_only).map(|(bytes, _metadata)| bytes)
}

fn canonical_public_request(
    bytes: &[u8],
) -> Result<PrivacyZkX509WorkerPublicRequestV1, WorkerFailure> {
    let text = core::str::from_utf8(bytes).map_err(|_| WorkerFailure::Request)?;
    let request = norito::json::from_json::<PrivacyZkX509WorkerPublicRequestV1>(text)
        .map_err(|_| WorkerFailure::Request)?;
    let canonical = norito::json::to_json(&request).map_err(|_| WorkerFailure::Request)?;
    if canonical.as_bytes() != bytes
        || request.schema_version != PRIVACY_ZK_X509_WORKER_PUBLIC_REQUEST_VERSION_V1
    {
        return Err(WorkerFailure::Request);
    }
    Ok(request)
}

fn canonical_execute_request(bytes: &[u8]) -> Result<ExecuteRequestV1, WorkerFailure> {
    let text = core::str::from_utf8(bytes).map_err(|_| WorkerFailure::Request)?;
    let request =
        norito::json::from_json::<ExecuteRequestV1>(text).map_err(|_| WorkerFailure::Request)?;
    let canonical = norito::json::to_json(&request).map_err(|_| WorkerFailure::Request)?;
    if canonical.as_bytes() != bytes
        || request.schema_version != PROTOCOL_VERSION
        || request.public_request_sha256 == [0; 32]
        || request.secret_bundle_sha256 == [0; 32]
    {
        return Err(WorkerFailure::Request);
    }
    Ok(request)
}

fn embedded_source_commit() -> Result<&'static str, WorkerFailure> {
    let commit = option_env!("IROHA_ZK_X509_SIGNED_SOURCE_COMMIT")
        .ok_or(WorkerFailure::ProfileUnavailable)?;
    if commit.len() != 40
        || !commit
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        || commit.bytes().all(|byte| byte == b'0')
    {
        return Err(WorkerFailure::ProfileUnavailable);
    }
    Ok(commit)
}

fn embedded_source_sha256() -> Result<&'static str, WorkerFailure> {
    let digest =
        option_env!("IROHA_ZK_X509_SOURCE_SHA256").ok_or(WorkerFailure::ProfileUnavailable)?;
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        || digest.bytes().all(|byte| byte == b'0')
    {
        return Err(WorkerFailure::ProfileUnavailable);
    }
    Ok(digest)
}

fn embedded_workspace_source_manifest_sha256() -> Result<&'static str, WorkerFailure> {
    let digest = option_env!("IROHA_ZK_X509_WORKSPACE_SOURCE_MANIFEST_SHA256")
        .ok_or(WorkerFailure::ProfileUnavailable)?;
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        || digest.bytes().all(|byte| byte == b'0')
    {
        return Err(WorkerFailure::ProfileUnavailable);
    }
    Ok(digest)
}

fn embedded_cargo_lock_sha256() -> Result<&'static str, WorkerFailure> {
    let digest =
        option_env!("IROHA_ZK_X509_CARGO_LOCK_SHA256").ok_or(WorkerFailure::ProfileUnavailable)?;
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        || digest.bytes().all(|byte| byte == b'0')
    {
        return Err(WorkerFailure::ProfileUnavailable);
    }
    Ok(digest)
}

fn embedded_allowed_signers_sha256() -> Result<&'static str, WorkerFailure> {
    let digest = option_env!("IROHA_ZK_X509_ALLOWED_SIGNERS_SHA256")
        .ok_or(WorkerFailure::ProfileUnavailable)?;
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        || digest.bytes().all(|byte| byte == b'0')
    {
        return Err(WorkerFailure::ProfileUnavailable);
    }
    Ok(digest)
}

fn embedded_revocation_sha256() -> Result<&'static str, WorkerFailure> {
    let digest =
        option_env!("IROHA_ZK_X509_REVOCATION_SHA256").ok_or(WorkerFailure::ProfileUnavailable)?;
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        || digest.bytes().all(|byte| byte == b'0')
    {
        return Err(WorkerFailure::ProfileUnavailable);
    }
    Ok(digest)
}

fn require_qualified_isolation_v1() -> Result<(), WorkerFailure> {
    qualified_isolation_package_sha256_v1().map(|_| ())
}

fn require_release_rayon_pool_v1() -> Result<(), WorkerFailure> {
    initialize_privacy_release_rayon_pool_v1().map_err(|_| WorkerFailure::ProfileUnavailable)
}

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
fn qualified_isolation_package_sha256_v1() -> Result<[u8; 32], WorkerFailure> {
    linux_isolation::verified_isolation_identity_v1().map(|identity| identity.package_sha256)
}

#[cfg(not(all(target_os = "linux", target_arch = "aarch64")))]
fn qualified_isolation_package_sha256_v1() -> Result<[u8; 32], WorkerFailure> {
    Err(WorkerFailure::IsolationUnavailable)
}

fn optional_source_digest(digest: [u8; 32]) -> Option<String> {
    digest
        .iter()
        .any(|byte| *byte != 0)
        .then(|| hex::encode(digest))
}

fn identity_payload() -> Result<Vec<u8>, WorkerFailure> {
    let source_commit = embedded_source_commit()?;
    let source_sha256 = embedded_source_sha256()?;
    let workspace_source_manifest_sha256 = embedded_workspace_source_manifest_sha256()?;
    let cargo_lock_sha256 = embedded_cargo_lock_sha256()?;
    let source_allowed_signers_sha256 = embedded_allowed_signers_sha256()?;
    let source_revocation_sha256 = embedded_revocation_sha256()?;
    let release_pins = privacy_zk_x509_worker_release_pins_v1().map_err(WorkerFailure::from)?;
    let release_evidence_ready = release_pins.release_evidence_sha256.is_some();
    let isolation_package_sha256 = qualified_isolation_package_sha256_v1().ok();
    let qualified_isolation_ready = isolation_package_sha256.is_some();
    if qualified_isolation_ready {
        require_release_rayon_pool_v1()?;
    }
    let production_profile_ready = release_pins.compiled_profile_sha256.is_some()
        && release_evidence_ready
        && qualified_isolation_ready;
    let identity = IdentityResponseV2 {
        artifact_self_hash_required: true,
        cargo_lock_sha256: cargo_lock_sha256.to_owned(),
        compiled_profile_sha256: release_pins.compiled_profile_sha256.map(hex::encode),
        expectations_json_sha256: optional_source_digest(release_pins.expectations_json_sha256),
        expectations_norito_sha256: optional_source_digest(release_pins.expectations_norito_sha256),
        operation: "prove-and-sign-zk-x509-action-v1".to_owned(),
        // A compiled protocol profile and complete release evidence remain
        // necessary.  The Linux launcher additionally has to prove its sealed
        // image, Landlock/seccomp state, and bounded cgroup in this process.
        production_profile_ready,
        protocol_id: PrivacyProtocolIdV1::IrohaZkX509StarkP256V0
            .canonical_label()
            .to_owned(),
        protocol_profile_sha256: hex::encode(release_pins.protocol_profile_sha256),
        protocol_version: PROTOCOL_VERSION,
        public_request_schema_version: PRIVACY_ZK_X509_WORKER_PUBLIC_REQUEST_VERSION_V1,
        qualified_isolation_ready,
        isolation_contract: if qualified_isolation_ready {
            QUALIFIED_ISOLATION_CONTRACT_V1
        } else {
            UNAVAILABLE_ISOLATION_CONTRACT_V1
        }
        .to_owned(),
        isolation_package_sha256: isolation_package_sha256.map(hex::encode),
        kat_proof_bytes: release_pins.kat_proof_bytes,
        kat_proof_sha256: optional_source_digest(release_pins.kat_proof_sha256),
        release_evidence_ready,
        release_evidence_sha256: release_pins.release_evidence_sha256.map(hex::encode),
        resource_certificate_sha256: optional_source_digest(
            release_pins.resource_certificate_sha256,
        ),
        schema: "iroha.privacy.zk_x509_worker_identity".to_owned(),
        schema_version: 2,
        soundness_certificate_sha256: optional_source_digest(
            release_pins.soundness_certificate_sha256,
        ),
        source_allowed_signers_sha256: source_allowed_signers_sha256.to_owned(),
        source_closure_schema:
            "path-and-length-framed-sha256(ci/privacy_zk_x509_worker_source_closure_v1.txt):v3"
                .to_owned(),
        source_commit: source_commit.to_owned(),
        source_revocation_sha256: source_revocation_sha256.to_owned(),
        source_sha256: source_sha256.to_owned(),
        workspace_source_manifest_sha256: workspace_source_manifest_sha256.to_owned(),
    };
    let encoded = norito::json::to_json(&identity).map_err(|_| WorkerFailure::Finalization)?;
    Ok(encoded.into_bytes())
}

fn decode_secret_bundle<'a>(
    bytes: &'a [u8],
    expected_public_request_sha256: [u8; 32],
) -> Result<([u8; 32], &'a [u8]), WorkerFailure> {
    if bytes.len() < BUNDLE_HEADER_BYTES
        || bytes.get(..4) != Some(BUNDLE_MAGIC)
        || bytes.get(4) != Some(&PROTOCOL_VERSION)
        || bytes.get(5..37) != Some(expected_public_request_sha256.as_slice())
    {
        return Err(WorkerFailure::Custody);
    }
    let seed: [u8; 32] = bytes
        .get(37..69)
        .ok_or(WorkerFailure::Custody)?
        .try_into()
        .map_err(|_| WorkerFailure::Custody)?;
    let witness_length = usize::try_from(u32::from_be_bytes(
        bytes
            .get(69..73)
            .ok_or(WorkerFailure::Custody)?
            .try_into()
            .map_err(|_| WorkerFailure::Custody)?,
    ))
    .map_err(|_| WorkerFailure::Custody)?;
    let witness = bytes.get(73..).ok_or(WorkerFailure::Custody)?;
    if seed == [0; 32]
        || witness_length == 0
        || witness_length > MAX_WITNESS_BYTES
        || witness.len() != witness_length
    {
        return Err(WorkerFailure::Custody);
    }
    Ok((seed, witness))
}

#[cfg(unix)]
fn append_bundle_file_identity_v1(
    response: &mut Vec<u8>,
    metadata: &fs::Metadata,
) -> Result<(), WorkerFailure> {
    use std::os::unix::fs::MetadataExt as _;

    let mode = metadata.mode() & 0o7777;
    if metadata.dev() == 0
        || metadata.ino() == 0
        || metadata.len() == 0
        || mode != 0o600
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.nlink() != 1
    {
        return Err(WorkerFailure::Custody);
    }
    response.extend_from_slice(&metadata.dev().to_be_bytes());
    response.extend_from_slice(&metadata.ino().to_be_bytes());
    response.extend_from_slice(&metadata.len().to_be_bytes());
    response.extend_from_slice(&mode.to_be_bytes());
    response.extend_from_slice(&metadata.uid().to_be_bytes());
    Ok(())
}

#[cfg(not(unix))]
fn append_bundle_file_identity_v1(
    _response: &mut Vec<u8>,
    _metadata: &fs::Metadata,
) -> Result<(), WorkerFailure> {
    Err(WorkerFailure::Custody)
}

fn admit_bundle_payload(payload: &[u8]) -> Result<Vec<u8>, WorkerFailure> {
    let request = canonical_execute_request(payload)?;
    require_qualified_isolation_v1()?;
    require_release_rayon_pool_v1()?;
    embedded_source_commit()?;
    let public_path = validate_absolute_path(&request.public_request_path)?;
    let bundle_path = validate_absolute_path(&request.secret_bundle_path)?;
    let public_bytes = read_stable_file(&public_path, MAX_PUBLIC_REQUEST_BYTES, false)?;
    if sha256(&public_bytes) != request.public_request_sha256 {
        return Err(WorkerFailure::Request);
    }
    let public_request = canonical_public_request(&public_bytes)?;
    compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaZkX509StarkP256V0)
        .map_err(|_| WorkerFailure::ProfileUnavailable)?;

    let (mut bundle, metadata) =
        read_stable_file_with_metadata(&bundle_path, MAX_BUNDLE_BYTES, true)?;
    if sha256(&bundle) != request.secret_bundle_sha256 {
        return Err(WorkerFailure::Custody);
    }
    let (mut signer_seed, witness) = decode_secret_bundle(&bundle, request.public_request_sha256)?;
    let validation =
        validate_privacy_zk_x509_worker_inputs_v1(&public_request, &signer_seed, witness)
            .map_err(WorkerFailure::from);
    signer_seed.zeroize();
    validation?;

    let mut response = Vec::with_capacity(1 + 1 + 32 + 32 + 8 + 8 + 8 + 4 + 4);
    response.push(RESPONSE_OK);
    response.push(PROTOCOL_VERSION);
    response.extend_from_slice(&request.public_request_sha256);
    response.extend_from_slice(&request.secret_bundle_sha256);
    append_bundle_file_identity_v1(&mut response, &metadata)?;
    bundle.zeroize();
    Ok(response)
}

fn execute_payload(payload: &[u8]) -> Result<Vec<u8>, WorkerFailure> {
    let execute = canonical_execute_request(payload)?;
    require_qualified_isolation_v1()?;
    require_release_rayon_pool_v1()?;
    embedded_source_commit()?;
    let public_path = validate_absolute_path(&execute.public_request_path)?;
    let bundle_path = validate_absolute_path(&execute.secret_bundle_path)?;
    let public_bytes = read_stable_file(&public_path, MAX_PUBLIC_REQUEST_BYTES, false)?;
    if sha256(&public_bytes) != execute.public_request_sha256 {
        return Err(WorkerFailure::Request);
    }
    let public_request = canonical_public_request(&public_bytes)?;

    // Refuse before opening the secret bundle while any production readiness
    // pin is absent.
    compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaZkX509StarkP256V0)
        .map_err(|_| WorkerFailure::ProfileUnavailable)?;

    let mut bundle = read_stable_file(&bundle_path, MAX_BUNDLE_BYTES, true)?;
    if sha256(&bundle) != execute.secret_bundle_sha256 {
        return Err(WorkerFailure::Custody);
    }
    let (mut signer_seed, witness) = decode_secret_bundle(&bundle, execute.public_request_sha256)?;
    let action =
        build_signed_privacy_zk_x509_worker_action_v1(public_request, &signer_seed, witness)
            .map_err(WorkerFailure::from);
    signer_seed.zeroize();
    let action = action?;
    let transaction_hash = *action.signed_transaction.hash().as_ref();
    let transaction = action.signed_transaction.encode_versioned();
    let transaction_length =
        u32::try_from(transaction.len()).map_err(|_| WorkerFailure::Finalization)?;
    let mut response = Vec::with_capacity(1 + 1 + 32 + 32 + 4 + transaction.len());
    response.push(RESPONSE_OK);
    response.push(PROTOCOL_VERSION);
    response.extend_from_slice(&transaction_hash);
    response.extend_from_slice(&action.proof_sha256);
    response.extend_from_slice(&transaction_length.to_be_bytes());
    response.extend_from_slice(&transaction);
    bundle.zeroize();
    Ok(response)
}

fn write_error_response(
    writer: &mut impl Write,
    command: u8,
    sequence: u64,
    failure: WorkerFailure,
    auth_key: &[u8; 32],
) -> Result<(), WorkerFailure> {
    write_response_frame(
        writer,
        command,
        sequence,
        &[RESPONSE_ERROR, PROTOCOL_VERSION, failure.code()],
        auth_key,
    )
}

fn run_server() -> Result<(), WorkerFailure> {
    let mut input = BufReader::new(io::stdin().lock());
    let mut output = BufWriter::new(io::stdout().lock());
    let mut auth_key = Zeroizing::new([0_u8; 32]);
    input
        .read_exact(&mut auth_key[..])
        .map_err(|_| WorkerFailure::Request)?;
    if auth_key.iter().all(|byte| *byte == 0) {
        return Err(WorkerFailure::Request);
    }
    let frame = read_request_frame(&mut input, &auth_key)?;
    let result = match frame.command {
        COMMAND_IDENTITY if frame.payload.is_empty() => identity_payload().map(|mut payload| {
            payload.insert(0, RESPONSE_OK);
            payload
        }),
        COMMAND_EXECUTE => execute_payload(&frame.payload),
        COMMAND_ADMIT_BUNDLE => admit_bundle_payload(&frame.payload),
        _ => Err(WorkerFailure::Request),
    };
    match result {
        Ok(payload) => write_response_frame(
            &mut output,
            frame.command,
            frame.sequence,
            &payload,
            &auth_key,
        ),
        Err(failure) => write_error_response(
            &mut output,
            frame.command,
            frame.sequence,
            failure,
            &auth_key,
        ),
    }
}

fn write_bundle(args: &[String]) -> Result<(), WorkerFailure> {
    if args.len() != 5 || args[0] != "bundle" {
        return Err(WorkerFailure::Request);
    }
    require_qualified_isolation_v1()?;
    let public_path = validate_absolute_path(&args[1])?;
    let seed_path = validate_absolute_path(&args[2])?;
    let witness_path = validate_absolute_path(&args[3])?;
    let output_path = validate_absolute_path(&args[4])?;
    let public_bytes = read_stable_file(&public_path, MAX_PUBLIC_REQUEST_BYTES, false)?;
    canonical_public_request(&public_bytes)?;
    let public_digest = sha256(&public_bytes);
    let mut seed = read_stable_file(&seed_path, 32, true)?;
    let mut witness = read_stable_file(&witness_path, MAX_WITNESS_BYTES_U64, true)?;
    if seed.len() != 32 || seed.iter().all(|byte| *byte == 0) || witness.is_empty() {
        return Err(WorkerFailure::Custody);
    }
    let witness_length = u32::try_from(witness.len()).map_err(|_| WorkerFailure::Custody)?;
    let mut bundle = Zeroizing::new(Vec::with_capacity(BUNDLE_HEADER_BYTES + witness.len()));
    bundle.extend_from_slice(BUNDLE_MAGIC);
    bundle.push(PROTOCOL_VERSION);
    bundle.extend_from_slice(&public_digest);
    bundle.extend_from_slice(&seed);
    bundle.extend_from_slice(&witness_length.to_be_bytes());
    bundle.extend_from_slice(&witness);
    persist_secret_bundle(&output_path, &bundle)?;
    let bundle_sha256 = sha256(&bundle);
    let mut stdout = io::stdout().lock();
    stdout
        .write_all(&bundle_sha256)
        .and_then(|()| stdout.flush())
        .map_err(|_| WorkerFailure::Custody)?;
    bundle.zeroize();
    seed.zeroize();
    witness.zeroize();
    Ok(())
}

fn main() {
    if harden_process().is_err() {
        eprintln!("zk-X509 worker startup hardening failed");
        std::process::exit(63);
    }
    let args = env::args().skip(1).collect::<Vec<_>>();
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    let args = {
        let mut args = args;
        if args.first().map(String::as_str) != Some(linux_isolation::INTERNAL_LAUNCH_ARGUMENT_V1) {
            match linux_isolation::launch_v1(args) {
                Ok(status) => std::process::exit(status.code().unwrap_or(70)),
                Err(_) => {
                    eprintln!("zk-X509 worker qualified launcher failed closed");
                    std::process::exit(63);
                }
            }
        }
        args.remove(0);
        args
    };
    let result = if args.is_empty() {
        run_server()
    } else {
        write_bundle(&args)
    };
    if result.is_err() {
        eprintln!("zk-X509 worker terminated without releasing secret material");
        std::process::exit(70);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn request_frame(command: u8, sequence: u64, payload: &[u8], key: &[u8; 32]) -> Vec<u8> {
        let mut authenticated = Vec::new();
        authenticated.extend_from_slice(FRAME_MAGIC);
        authenticated.push(PROTOCOL_VERSION);
        authenticated.push(command);
        authenticated.extend_from_slice(&sequence.to_be_bytes());
        authenticated.extend_from_slice(
            &u32::try_from(payload.len())
                .expect("test payload length")
                .to_be_bytes(),
        );
        authenticated.extend_from_slice(payload);
        authenticated.extend_from_slice(&hmac_sha256(key, &authenticated));
        let mut framed = Vec::new();
        framed.extend_from_slice(
            &u32::try_from(authenticated.len())
                .expect("test frame length")
                .to_be_bytes(),
        );
        framed.extend_from_slice(&authenticated);
        framed
    }

    #[test]
    fn authenticated_frame_round_trip_preserves_command_sequence_and_payload() {
        let key = [0x31; 32];
        let encoded = request_frame(COMMAND_EXECUTE, 7, b"canonical-payload", &key);
        let frame = read_request_frame(&mut Cursor::new(encoded), &key)
            .expect("authenticated request frame");
        assert_eq!(frame.command, COMMAND_EXECUTE);
        assert_eq!(frame.sequence, 7);
        assert_eq!(frame.payload, b"canonical-payload");
    }

    #[test]
    fn authenticated_frame_rejects_payload_and_tag_mutation() {
        let key = [0x42; 32];
        let encoded = request_frame(COMMAND_EXECUTE, 11, b"payload", &key);
        for index in [4 + 18, encoded.len() - 1] {
            let mut mutated = encoded.clone();
            mutated[index] ^= 1;
            let failure = read_request_frame(&mut Cursor::new(mutated), &key)
                .expect_err("mutated request must fail closed");
            assert_eq!(failure.code(), ERROR_REQUEST);
        }
    }

    #[test]
    fn response_frame_is_bound_to_command_sequence_payload_and_session_key() {
        let key = [0x53; 32];
        let payload = b"identity";
        let mut encoded = Vec::new();
        write_response_frame(&mut encoded, COMMAND_IDENTITY, 13, payload, &key)
            .expect("authenticated response");
        let declared =
            usize::try_from(u32::from_be_bytes(encoded[..4].try_into().expect("length")))
                .expect("usize supports u32 frame lengths");
        assert_eq!(declared, encoded.len() - 4);
        let authenticated_end = encoded.len() - AUTH_TAG_BYTES;
        let authenticated = &encoded[4..authenticated_end];
        let tag = &encoded[authenticated_end..];
        assert!(constant_time_eq(tag, &hmac_sha256(&key, authenticated)));
        assert_eq!(&authenticated[..4], FRAME_MAGIC);
        assert_eq!(authenticated[4], PROTOCOL_VERSION);
        assert_eq!(authenticated[5], COMMAND_IDENTITY);
        assert_eq!(
            u64::from_be_bytes(authenticated[6..14].try_into().expect("sequence")),
            13
        );
        assert_eq!(&authenticated[18..], payload);
        let mut wrong_key = key;
        wrong_key[0] ^= 1;
        assert!(!constant_time_eq(
            tag,
            &hmac_sha256(&wrong_key, authenticated)
        ));
    }

    #[test]
    fn direct_launch_v1_has_no_promotable_isolation_flag() {
        let failure = require_qualified_isolation_v1().expect_err("v1 stays closed");
        assert_eq!(failure.code(), ERROR_ISOLATION_UNAVAILABLE);
    }

    #[test]
    fn execute_refuses_before_validating_or_opening_named_paths() {
        let request = ExecuteRequestV1 {
            schema_version: PROTOCOL_VERSION,
            public_request_path: "relative-public-request.json".to_owned(),
            public_request_sha256: [0x11; 32],
            secret_bundle_path: "relative-secret-bundle.x5wb".to_owned(),
            secret_bundle_sha256: [0x22; 32],
        };
        let payload = norito::json::to_json(&request)
            .expect("canonical request")
            .into_bytes();
        let failure = execute_payload(&payload).expect_err("isolation must fail first");
        assert_eq!(failure.code(), ERROR_ISOLATION_UNAVAILABLE);
    }

    #[test]
    fn bundle_admission_refuses_before_validating_or_opening_named_paths() {
        let request = ExecuteRequestV1 {
            schema_version: PROTOCOL_VERSION,
            public_request_path: "relative-public-request.json".to_owned(),
            public_request_sha256: [0x11; 32],
            secret_bundle_path: "relative-secret-bundle.x5wb".to_owned(),
            secret_bundle_sha256: [0x22; 32],
        };
        let payload = norito::json::to_json(&request)
            .expect("canonical request")
            .into_bytes();
        let failure = admit_bundle_payload(&payload).expect_err("isolation must fail first");
        assert_eq!(failure.code(), ERROR_ISOLATION_UNAVAILABLE);
    }

    #[test]
    fn bundle_writer_refuses_before_validating_or_opening_named_paths() {
        let args = [
            "bundle".to_owned(),
            "relative-public-request.json".to_owned(),
            "relative-signer-seed.bin".to_owned(),
            "relative-witness.bin".to_owned(),
            "relative-output.x5wb".to_owned(),
        ];
        let failure = write_bundle(&args).expect_err("isolation must fail first");
        assert_eq!(failure.code(), ERROR_ISOLATION_UNAVAILABLE);
    }

    #[cfg(unix)]
    #[test]
    fn secret_bundle_persistence_is_owner_only_atomic_and_no_replace() {
        use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

        let directory = tempfile::tempdir().expect("private temporary directory");
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
            .expect("private parent mode");
        // macOS exposes temporary directories through `/var`, which is a
        // symlink to `/private/var`. Production custody deliberately requires
        // the exact canonical parent path, so exercise that same contract.
        let private_directory =
            fs::canonicalize(directory.path()).expect("canonical private parent path");
        let output = private_directory.join("bundle.x5wb");
        let secret = b"X5WB\x01owner-only-test-bundle";
        persist_secret_bundle(&output, secret).expect("persist owner-only bundle");
        assert_eq!(fs::read(&output).expect("published bundle"), secret);
        let metadata = fs::symlink_metadata(&output).expect("published metadata");
        assert!(metadata.is_file());
        assert_eq!(metadata.mode() & 0o777, 0o600);
        assert_eq!(metadata.nlink(), 1);
        assert_eq!(metadata.uid(), rustix::process::geteuid().as_raw());
        assert!(persist_secret_bundle(&output, b"replacement").is_err());
        assert_eq!(fs::read(&output).expect("unchanged bundle"), secret);
    }

    #[cfg(unix)]
    #[test]
    fn secret_bundle_persistence_rejects_non_private_parent() {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir().expect("temporary directory");
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o755))
            .expect("replaceable parent mode");
        let output = directory.path().join("bundle.x5wb");
        assert!(persist_secret_bundle(&output, b"secret").is_err());
        assert!(!output.exists());
    }
}
