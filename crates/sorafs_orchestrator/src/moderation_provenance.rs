//! Durable, bounded, tamper-evident provenance for signed moderation results.
//!
//! Each update takes an exclusive sibling lock, re-reads and validates the
//! latest canonical Norito segment, appends one hash-chained payload, writes a
//! mode-`0600` temporary file, synchronizes it, atomically renames it, and
//! synchronizes the containing directory. Unix file identity and link-count
//! checks reject symlink and hard-link substitution. Non-Unix platforms fail
//! closed until equivalent primitives are implemented.
use std::{
    collections::BTreeSet,
    io,
    path::{Path, PathBuf},
};
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
#[cfg(unix)]
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    sync::atomic::{AtomicU64, Ordering},
};
use iroha_crypto::PublicKey;
#[cfg(unix)]
use iroha_data_model::sorafs::moderation::MODERATION_PROVENANCE_MAX_ENTRIES_V1;
use iroha_data_model::sorafs::moderation::{
    ModerationCommitteeAggregateError, ModerationCommitteeAggregateV1, ModerationProvenanceError,
    ModerationProvenanceLogV1, ModerationProvenancePayloadV1, ModerationReproManifestV1,
    ModerationSignedResultError, ModerationSignedScreeningResultV1, ModerationTrustPolicyError,
    ModerationTrustPolicyV1,
};
#[cfg(unix)]
use norito::core::DecodeLimits;
use thiserror::Error;
#[cfg(unix)]
const MAX_PROVENANCE_FILE_BYTES: u64 = 64 * 1024 * 1024;
#[cfg(unix)]
const MAX_PROVENANCE_STRING_BYTES: usize = 4096;
#[cfg(unix)]
const MAX_PROVENANCE_SEQUENCE_ELEMENTS: usize = MODERATION_PROVENANCE_MAX_ENTRIES_V1;
#[cfg(unix)]
const MAX_PROVENANCE_DECODE_DEPTH: usize = 64;
#[cfg(unix)]
const TEMPFILE_ATTEMPTS: u64 = 64;
#[cfg(unix)]
static TEMPFILE_COUNTER: AtomicU64 = AtomicU64::new(0);
/// Durable moderation provenance storage failures.
#[derive(Debug, Error)]
pub enum ModerationProvenanceStoreError {
    /// Platform lacks the required no-follow, identity, and locking primitives.
    #[error("durable moderation provenance is unsupported on this platform")]
    UnsupportedPlatform,
    /// Store path or parent directory is unsafe.
    #[error("unsafe moderation provenance path `{path}`: {reason}")]
    UnsafePath {
        /// Rejected path.
        path: PathBuf,
        /// Rejection reason.
        reason: String,
    },
    /// Store I/O failed.
    #[error("moderation provenance I/O failed at `{path}`: {source}")]
    Io {
        /// Failing path.
        path: PathBuf,
        /// Underlying I/O failure.
        #[source]
        source: io::Error,
    },
    /// Another writer holds the exclusive store lock.
    #[error("moderation provenance store `{0}` is locked by another writer")]
    Locked(PathBuf),
    /// Persisted file is empty, oversized, truncated, or changed during read.
    #[error("invalid moderation provenance file `{path}`: {reason}")]
    InvalidFile {
        /// Invalid file path.
        path: PathBuf,
        /// Rejection reason.
        reason: String,
    },
    /// Bounded Norito decoding failed.
    #[error("failed to decode moderation provenance `{path}`: {reason}")]
    Decode {
        /// Invalid file path.
        path: PathBuf,
        /// Decoder failure.
        reason: String,
    },
    /// Persisted bytes are not the unique canonical Norito encoding.
    #[error("moderation provenance `{0}` is not canonically encoded")]
    NonCanonical(PathBuf),
    /// Hash-chain validation failed.
    #[error("invalid moderation provenance chain: {0}")]
    InvalidChain(#[from] ModerationProvenanceError),
    /// Existing segment identifier differs from the configured identifier.
    #[error("moderation provenance log id does not match the configured segment")]
    LogIdMismatch,
    /// External trust-policy validation failed.
    #[error("invalid moderation trust policy: {0}")]
    InvalidTrustPolicy(#[from] ModerationTrustPolicyError),
    /// Signed result validation failed.
    #[error("invalid signed moderation result: {0}")]
    InvalidSignedResult(#[from] ModerationSignedResultError),
    /// Authenticated committee reconstruction failed.
    #[error("invalid authenticated moderation aggregate: {0}")]
    InvalidAggregate(#[from] ModerationCommitteeAggregateError),
    /// Supplied aggregate differs from deterministic authenticated reconstruction.
    #[error("moderation committee aggregate does not match its signed member results")]
    AggregateMismatch,
    /// Bounded persistence allocation or encoding failed.
    #[error("failed to encode moderation provenance: {0}")]
    Encode(String),
}
/// Concurrent-writer-safe durable provenance segment.
#[derive(Clone, Debug)]
pub struct ModerationProvenanceStoreV1 {
    path: PathBuf,
    log_id: [u8; 16],
}
impl ModerationProvenanceStoreV1 {
    /// Open an existing segment or atomically create an empty one.
    pub fn open(
        path: impl AsRef<Path>,
        log_id: [u8; 16],
    ) -> Result<Self, ModerationProvenanceStoreError> {
        let store = Self {
            path: path.as_ref().to_path_buf(),
            log_id,
        };
        store.with_locked_log(|_, _| Ok(()))?;
        Ok(store)
    }
    /// Return the configured durable path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
    /// Read and verify the latest durable snapshot under the writer lock.
    pub fn snapshot(&self) -> Result<ModerationProvenanceLogV1, ModerationProvenanceStoreError> {
        self.with_locked_log(|log, _| Ok(log.clone()))
    }
    /// Validate and durably append one externally authorized signed result.
    pub fn append_signed_result(
        &self,
        manifest: &ModerationReproManifestV1,
        policy: &ModerationTrustPolicyV1,
        trust_anchors: &BTreeSet<PublicKey>,
        minimum_governance_quorum: u16,
        result: ModerationSignedScreeningResultV1,
        recorded_at_unix: u64,
    ) -> Result<[u8; 32], ModerationProvenanceStoreError> {
        policy.validate_with_trust_anchors(
            manifest,
            trust_anchors,
            minimum_governance_quorum,
            recorded_at_unix,
        )?;
        result.validate(manifest, policy, recorded_at_unix)?;
        self.update(|log| {
            log.append(
                ModerationProvenancePayloadV1::SignedScreeningResult(result),
                recorded_at_unix,
            )
            .map_err(ModerationProvenanceStoreError::from)
        })
    }
    /// Reconstruct, compare, and durably append the complete signed member set
    /// followed by one authenticated committee aggregate. Distinct signer,
    /// freshness, revocation, external governance, and quorum checks are all
    /// repeated before the single atomic write.
    #[expect(
        clippy::too_many_arguments,
        reason = "this public API keeps each authenticated aggregate input explicit and independently typed"
    )]
    pub fn append_authenticated_aggregate(
        &self,
        manifest: &ModerationReproManifestV1,
        policy: &ModerationTrustPolicyV1,
        trust_anchors: &BTreeSet<PublicKey>,
        minimum_governance_quorum: u16,
        results: &[ModerationSignedScreeningResultV1],
        aggregate: ModerationCommitteeAggregateV1,
        recorded_at_unix: u64,
    ) -> Result<[u8; 32], ModerationProvenanceStoreError> {
        policy.validate_with_trust_anchors(
            manifest,
            trust_anchors,
            minimum_governance_quorum,
            recorded_at_unix,
        )?;
        let reconstructed = ModerationCommitteeAggregateV1::aggregate_authenticated(
            manifest,
            policy,
            trust_anchors,
            minimum_governance_quorum,
            results,
            aggregate.aggregated_at_unix,
        )?;
        if reconstructed != aggregate {
            return Err(ModerationProvenanceStoreError::AggregateMismatch);
        }
        let mut ordered_results = results.to_vec();
        ordered_results.sort_by(|left, right| left.signer_public_key.cmp(&right.signer_public_key));
        self.update(|log| {
            for result in ordered_results {
                log.append(
                    ModerationProvenancePayloadV1::SignedScreeningResult(result),
                    recorded_at_unix,
                )?;
            }
            log.append(
                ModerationProvenancePayloadV1::CommitteeAggregate(aggregate),
                recorded_at_unix,
            )
            .map_err(ModerationProvenanceStoreError::from)
        })
    }
    #[cfg(unix)]
    fn update<T>(
        &self,
        update: impl FnOnce(&mut ModerationProvenanceLogV1) -> Result<T, ModerationProvenanceStoreError>,
    ) -> Result<T, ModerationProvenanceStoreError> {
        self.with_locked_log(|log, parent| {
            let output = update(log)?;
            persist_log(&parent.target, parent, log)?;
            Ok(output)
        })
    }
    #[cfg(not(unix))]
    fn update<T>(
        &self,
        _update: impl FnOnce(
            &mut ModerationProvenanceLogV1,
        ) -> Result<T, ModerationProvenanceStoreError>,
    ) -> Result<T, ModerationProvenanceStoreError> {
        Err(ModerationProvenanceStoreError::UnsupportedPlatform)
    }
    fn with_locked_log<T>(
        &self,
        operation: impl FnOnce(
            &mut ModerationProvenanceLogV1,
            &ValidatedParent,
        ) -> Result<T, ModerationProvenanceStoreError>,
    ) -> Result<T, ModerationProvenanceStoreError> {
        #[cfg(not(unix))]
        {
            let _ = operation;
            return Err(ModerationProvenanceStoreError::UnsupportedPlatform);
        }
        #[cfg(unix)]
        {
            let parent = validate_parent(&self.path)?;
            let _lock = acquire_lock(&self.path, &parent)?;
            verify_parent(&parent)?;
            let (mut log, created) = match read_log(&parent.target)? {
                Some(log) => (log, false),
                None => (ModerationProvenanceLogV1::new(self.log_id)?, true),
            };
            if log.log_id != self.log_id {
                return Err(ModerationProvenanceStoreError::LogIdMismatch);
            }
            log.validate_chain()?;
            let output = operation(&mut log, &parent)?;
            if created {
                persist_log(&parent.target, &parent, &log)?;
            }
            Ok(output)
        }
    }
}
#[cfg(unix)]
type ParentIdentity = (u64, u64);
#[cfg(unix)]
#[derive(Debug)]
struct ValidatedParent {
    canonical: PathBuf,
    target: PathBuf,
    identity: ParentIdentity,
    handle: File,
}
#[cfg(not(unix))]
struct ValidatedParent;
#[cfg(unix)]
fn validate_parent(path: &Path) -> Result<ValidatedParent, ModerationProvenanceStoreError> {
    let filename = path
        .file_name()
        .ok_or_else(|| unsafe_path(path, "store path has no file name"))?;
    let parent = path
        .parent()
        .filter(|value| !value.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let metadata = fs::symlink_metadata(parent).map_err(|source| io_error(parent, source))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(unsafe_path(parent, "parent must be a real directory"));
    }
    if metadata.mode() & 0o022 != 0 {
        return Err(unsafe_path(
            parent,
            "parent directory must not be group- or world-writable",
        ));
    }
    let canonical = fs::canonicalize(parent).map_err(|source| io_error(parent, source))?;
    let mut options = OpenOptions::new();
    options.read(true);
    set_no_follow(&mut options);
    let handle = options
        .open(&canonical)
        .map_err(|source| io_error(&canonical, source))?;
    let opened = handle
        .metadata()
        .map_err(|source| io_error(&canonical, source))?;
    if !opened.is_dir() || parent_identity(&opened) != parent_identity(&metadata) {
        return Err(unsafe_path(parent, "parent identity changed while opening"));
    }
    Ok(ValidatedParent {
        target: canonical.join(filename),
        canonical,
        identity: parent_identity(&opened),
        handle,
    })
}
#[cfg(unix)]
fn verify_parent(parent: &ValidatedParent) -> Result<(), ModerationProvenanceStoreError> {
    let metadata = fs::symlink_metadata(&parent.canonical)
        .map_err(|source| io_error(&parent.canonical, source))?;
    let opened = parent
        .handle
        .metadata()
        .map_err(|source| io_error(&parent.canonical, source))?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || parent_identity(&metadata) != parent.identity
        || parent_identity(&opened) != parent.identity
    {
        return Err(unsafe_path(
            &parent.canonical,
            "parent identity changed during transaction",
        ));
    }
    Ok(())
}
#[cfg(unix)]
fn acquire_lock(
    path: &Path,
    parent: &ValidatedParent,
) -> Result<StoreLock, ModerationProvenanceStoreError> {
    let filename = path
        .file_name()
        .ok_or_else(|| unsafe_path(path, "store path has no file name"))?;
    let lock_path = parent
        .canonical
        .join(format!(".{}.lock", filename.to_string_lossy()));
    match fs::create_dir(&lock_path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            return Err(ModerationProvenanceStoreError::Locked(path.to_path_buf()));
        }
        Err(source) => return Err(io_error(&lock_path, source)),
    }
    if let Err(source) = fs::set_permissions(&lock_path, fs::Permissions::from_mode(0o700)) {
        let _ = fs::remove_dir(&lock_path);
        return Err(io_error(&lock_path, source));
    }
    Ok(StoreLock { path: lock_path })
}
#[cfg(unix)]
#[derive(Debug)]
struct StoreLock {
    path: PathBuf,
}
#[cfg(unix)]
impl Drop for StoreLock {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.path);
    }
}
#[cfg(unix)]
fn read_log(
    path: &Path,
) -> Result<Option<ModerationProvenanceLogV1>, ModerationProvenanceStoreError> {
    let mut options = OpenOptions::new();
    options.read(true);
    set_no_follow(&mut options);
    let mut file = match options.open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(io_error(path, source)),
    };
    let before = file.metadata().map_err(|source| io_error(path, source))?;
    validate_regular_single_link(path, &before)?;
    if before.len() == 0 || before.len() > MAX_PROVENANCE_FILE_BYTES {
        return Err(ModerationProvenanceStoreError::InvalidFile {
            path: path.to_path_buf(),
            reason: format!(
                "file size {} is outside 1..={MAX_PROVENANCE_FILE_BYTES}",
                before.len()
            ),
        });
    }
    let length =
        usize::try_from(before.len()).map_err(|_| ModerationProvenanceStoreError::InvalidFile {
            path: path.to_path_buf(),
            reason: "file length does not fit usize".to_string(),
        })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).map_err(|error| {
        ModerationProvenanceStoreError::InvalidFile {
            path: path.to_path_buf(),
            reason: format!("bounded allocation failed: {error}"),
        }
    })?;
    bytes.resize(length, 0);
    file.read_exact(&mut bytes)
        .map_err(|source| io_error(path, source))?;
    let mut trailing = [0_u8; 1];
    if file
        .read(&mut trailing)
        .map_err(|source| io_error(path, source))?
        != 0
    {
        return Err(ModerationProvenanceStoreError::InvalidFile {
            path: path.to_path_buf(),
            reason: "file grew while being read".to_string(),
        });
    }
    let after = file.metadata().map_err(|source| io_error(path, source))?;
    if file_identity(&before) != file_identity(&after) {
        return Err(ModerationProvenanceStoreError::InvalidFile {
            path: path.to_path_buf(),
            reason: "file changed while being read".to_string(),
        });
    }
    let allocation_limit = usize::try_from(MAX_PROVENANCE_FILE_BYTES)
        .expect("64 MiB provenance limit fits usize on supported Unix targets");
    let limits = DecodeLimits::new(
        MAX_PROVENANCE_STRING_BYTES,
        allocation_limit,
        MAX_PROVENANCE_SEQUENCE_ELEMENTS,
        allocation_limit,
        MAX_PROVENANCE_DECODE_DEPTH,
    );
    let log: ModerationProvenanceLogV1 = norito::decode_from_bytes_with_limits(&bytes, limits)
        .map_err(|error| ModerationProvenanceStoreError::Decode {
            path: path.to_path_buf(),
            reason: error.to_string(),
        })?;
    let canonical = norito::to_bytes(&log)
        .map_err(|error| ModerationProvenanceStoreError::Encode(error.to_string()))?;
    if canonical != bytes {
        return Err(ModerationProvenanceStoreError::NonCanonical(
            path.to_path_buf(),
        ));
    }
    log.validate_chain()?;
    Ok(Some(log))
}
#[cfg(unix)]
fn persist_log(
    path: &Path,
    parent: &ValidatedParent,
    log: &ModerationProvenanceLogV1,
) -> Result<(), ModerationProvenanceStoreError> {
    log.validate_chain()?;
    let bytes = norito::to_bytes(log)
        .map_err(|error| ModerationProvenanceStoreError::Encode(error.to_string()))?;
    if bytes.is_empty()
        || u64::try_from(bytes.len())
            .ok()
            .is_none_or(|length| length > MAX_PROVENANCE_FILE_BYTES)
    {
        return Err(ModerationProvenanceStoreError::InvalidFile {
            path: path.to_path_buf(),
            reason: "encoded segment exceeds the durable file bound".to_string(),
        });
    }
    verify_parent(parent)?;
    let filename = path
        .file_name()
        .ok_or_else(|| unsafe_path(path, "store path has no file name"))?
        .to_string_lossy();
    let mut temporary = None;
    for _ in 0..TEMPFILE_ATTEMPTS {
        let counter = TEMPFILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.canonical.join(format!(
            ".{filename}.tmp.{}.{}",
            std::process::id(),
            counter
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true).mode(0o600);
        set_no_follow(&mut options);
        match options.open(&candidate) {
            Ok(file) => {
                temporary = Some((candidate, file));
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(source) => return Err(io_error(&candidate, source)),
        }
    }
    let (temporary_path, mut temporary_file) =
        temporary.ok_or_else(|| ModerationProvenanceStoreError::InvalidFile {
            path: path.to_path_buf(),
            reason: "could not allocate a unique temporary file".to_string(),
        })?;
    let transaction = (|| {
        temporary_file
            .write_all(&bytes)
            .map_err(|source| io_error(&temporary_path, source))?;
        temporary_file
            .sync_all()
            .map_err(|source| io_error(&temporary_path, source))?;
        validate_regular_single_link(
            &temporary_path,
            &temporary_file
                .metadata()
                .map_err(|source| io_error(&temporary_path, source))?,
        )?;
        verify_parent(parent)?;
        fs::rename(&temporary_path, path).map_err(|source| io_error(path, source))?;
        parent
            .handle
            .sync_all()
            .map_err(|source| io_error(&parent.canonical, source))?;
        Ok(())
    })();
    if transaction.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    transaction
}
#[cfg(unix)]
fn validate_regular_single_link(
    path: &Path,
    metadata: &fs::Metadata,
) -> Result<(), ModerationProvenanceStoreError> {
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.nlink() != 1 {
        return Err(unsafe_path(
            path,
            "file must be a regular non-symlink with exactly one hard link",
        ));
    }
    Ok(())
}
#[cfg(unix)]
fn file_identity(metadata: &fs::Metadata) -> (u64, u64, u64, i64, i64) {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
    )
}
#[cfg(unix)]
fn parent_identity(metadata: &fs::Metadata) -> ParentIdentity {
    (metadata.dev(), metadata.ino())
}
#[cfg(unix)]
fn set_no_follow(options: &mut OpenOptions) {
    options.custom_flags(platform_no_follow_flag());
}
#[cfg(any(target_os = "linux", target_os = "android"))]
const fn platform_no_follow_flag() -> i32 {
    0o400000
}
#[cfg(all(
    unix,
    not(any(target_os = "linux", target_os = "android")),
    any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )
))]
const fn platform_no_follow_flag() -> i32 {
    0x100
}
fn io_error(path: &Path, source: io::Error) -> ModerationProvenanceStoreError {
    ModerationProvenanceStoreError::Io {
        path: path.to_path_buf(),
        source,
    }
}
fn unsafe_path(path: &Path, reason: impl Into<String>) -> ModerationProvenanceStoreError {
    ModerationProvenanceStoreError::UnsafePath {
        path: path.to_path_buf(),
        reason: reason.into(),
    }
}
#[cfg(all(test, unix))]
mod tests {
    use iroha_crypto::{KeyPair, SignatureOf};
    use iroha_data_model::sorafs::moderation::{
        MODERATION_SIGNED_RESULT_VERSION_V1, ModerationSignedScreeningBodyV1,
        ModerationSignedScreeningResultV1,
    };
    use tempfile::TempDir;
    use super::*;
    fn signed_payload(timestamp: u64) -> ModerationProvenancePayloadV1 {
        let keypair = KeyPair::try_random().expect("keypair");
        let mut body = ModerationSignedScreeningBodyV1 {
            schema_version: MODERATION_SIGNED_RESULT_VERSION_V1,
            manifest_id: [1; 16],
            manifest_digest: [2; 32],
            runner_hash: [3; 32],
            trust_policy_id: [4; 16],
            trust_policy_digest: [5; 32],
            subject: "cid:provenance-test".to_string(),
            subject_digest: [6; 32],
            model_scores: Vec::new(),
            combined_score_bps: 0,
            verdict: "pass".to_string(),
            screened_at_unix: timestamp,
            expires_at_unix: timestamp + 60,
            policy_digest: [7; 32],
            evidence_digest: [0; 32],
            notes: None,
        };
        body.refresh_evidence_digest().expect("evidence digest");
        let signature = SignatureOf::try_new(keypair.private_key(), &body).expect("signature");
        ModerationProvenancePayloadV1::SignedScreeningResult(ModerationSignedScreeningResultV1 {
            body,
            signer_public_key: keypair.public_key().clone(),
            signature,
        })
    }
    #[test]
    fn store_persists_canonical_hash_chain_across_reopen() {
        let directory = TempDir::new().expect("tempdir");
        let path = directory.path().join("provenance.to");
        let store = ModerationProvenanceStoreV1::open(&path, [0xA1; 16]).expect("open");
        store
            .update(|log| {
                log.append(signed_payload(100), 101)
                    .map_err(ModerationProvenanceStoreError::from)
            })
            .expect("append");
        let reopened =
            ModerationProvenanceStoreV1::open(&path, [0xA1; 16]).expect("reopen canonical");
        let snapshot = reopened.snapshot().expect("snapshot");
        assert_eq!(snapshot.entries.len(), 1);
        snapshot.validate_chain().expect("valid chain");
    }
    #[test]
    fn store_rejects_noncanonical_trailing_bytes() {
        let directory = TempDir::new().expect("tempdir");
        let path = directory.path().join("provenance.to");
        ModerationProvenanceStoreV1::open(&path, [0xA2; 16]).expect("open");
        let mut options = OpenOptions::new();
        options.append(true);
        let mut file = options.open(&path).expect("open for adversarial append");
        file.write_all(&[0]).expect("append trailing byte");
        file.sync_all().expect("sync tamper");
        assert!(matches!(
            ModerationProvenanceStoreV1::open(&path, [0xA2; 16])
                .expect_err("trailing bytes must fail"),
            ModerationProvenanceStoreError::Decode { .. }
                | ModerationProvenanceStoreError::NonCanonical(_)
        ));
    }
    #[test]
    fn store_fails_fast_on_concurrent_writer_lock() {
        let directory = TempDir::new().expect("tempdir");
        let path = directory.path().join("provenance.to");
        let store = ModerationProvenanceStoreV1::open(&path, [0xA4; 16]).expect("open");
        let lock_path = directory.path().join(".provenance.to.lock");
        fs::create_dir(&lock_path).expect("simulate another writer");
        assert!(matches!(
            store.snapshot().expect_err("concurrent lock must fail fast"),
            ModerationProvenanceStoreError::Locked(locked) if locked == path
        ));
        fs::remove_dir(lock_path).expect("remove simulated lock");
    }
    #[test]
    fn store_file_is_owner_read_write_only() {
        let directory = TempDir::new().expect("tempdir");
        let path = directory.path().join("provenance.to");
        ModerationProvenanceStoreV1::open(&path, [0xA5; 16]).expect("open");
        let mode = fs::metadata(path).expect("metadata").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
    #[cfg(unix)]
    #[test]
    fn store_rejects_symlink_and_hardlink_substitution() {
        use std::os::unix::fs::symlink;
        let directory = TempDir::new().expect("tempdir");
        let target = directory.path().join("target.to");
        ModerationProvenanceStoreV1::open(&target, [0xA3; 16]).expect("target");
        let symlink_path = directory.path().join("symlink.to");
        symlink(&target, &symlink_path).expect("symlink");
        assert!(ModerationProvenanceStoreV1::open(&symlink_path, [0xA3; 16]).is_err());
        let hardlink_path = directory.path().join("hardlink.to");
        fs::hard_link(&target, &hardlink_path).expect("hardlink");
        assert!(ModerationProvenanceStoreV1::open(&hardlink_path, [0xA3; 16]).is_err());
    }
}
