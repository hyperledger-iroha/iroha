//! Crash-safe local signing guard for automatic lane drain certificates.
use crate::{lane_consensus::validate_lane_drain_certificate_body, sumeragi::consensus::Phase};
use iroha_crypto::Hash;
use iroha_data_model::{
    block::consensus::LaneBlockVoteBodyV1,
    merge::LaneDrainCertificateBodyV1,
    nexus::{DataSpaceId, LaneId},
};
use norito::codec::{Decode, Encode};
use parking_lot::Mutex;
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use thiserror::Error;
const GUARD_VERSION: u8 = 1;
const GUARD_DIRECTORY: &str = "lane-drain-signing-guard-v1";
const RECORD_EXTENSION: &str = "norito";
const TEMP_EXTENSION: &str = "norito.tmp";
const LOCK_FILENAME: &str = "owner.lock";
const RECORD_KEY_DOMAIN: &[u8] = b"iroha:lane-drain:signing-record:v1\0";
const RECORD_INTEGRITY_DOMAIN: &[u8] = b"iroha:lane-drain:signing-record-integrity:v1\0";
const MAX_RECORD_BYTES: usize = 32 * 1024;
const MAX_RECORDS: usize = 65_536;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
struct LaneDrainSigningKeyV1 {
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
}
impl LaneDrainSigningKeyV1 {
    fn from_commit_vote(body: &LaneBlockVoteBodyV1) -> Self {
        Self {
            lane_id: body.lane_id,
            dataspace_id: body.dataspace_id,
            lane_incarnation: body.lane_incarnation,
        }
    }
    fn from_drain(body: &LaneDrainCertificateBodyV1) -> Self {
        Self {
            lane_id: body.intent.lane_id,
            dataspace_id: body.intent.dataspace_id,
            lane_incarnation: body.intent.lane_incarnation,
        }
    }
    fn digest(self) -> Hash {
        let encoded = self.encode();
        Hash::new_from_chunks(&[RECORD_KEY_DOMAIN, encoded.as_slice()])
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
struct LaneCommitVoteLockV1 {
    proposal_height: u64,
    lane_block_height: u64,
    proposal_hash: Hash,
    descriptor_hash: Hash,
    vote_body_digest: Hash,
}
impl LaneCommitVoteLockV1 {
    fn from_body(body: &LaneBlockVoteBodyV1) -> Self {
        Self {
            proposal_height: body.proposal_height,
            lane_block_height: body.lane_block_height,
            proposal_hash: body.proposal_hash,
            descriptor_hash: body.descriptor_hash,
            vote_body_digest: Hash::new(body.signature_preimage()),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
struct LaneDrainSigningRecordV1 {
    version: u8,
    key: LaneDrainSigningKeyV1,
    highest_commit_vote: Option<LaneCommitVoteLockV1>,
    drain_body: Option<LaneDrainCertificateBodyV1>,
    integrity_hash: Hash,
}
impl LaneDrainSigningRecordV1 {
    fn empty(key: LaneDrainSigningKeyV1) -> Self {
        Self {
            version: GUARD_VERSION,
            key,
            highest_commit_vote: None,
            drain_body: None,
            integrity_hash: Hash::prehashed([0; Hash::LENGTH]),
        }
    }
    fn computed_integrity_hash(&self) -> Hash {
        let mut payload = self.version.encode();
        payload.extend(self.key.encode());
        payload.extend(self.highest_commit_vote.encode());
        payload.extend(self.drain_body.encode());
        Hash::new_from_chunks(&[RECORD_INTEGRITY_DOMAIN, payload.as_slice()])
    }
    fn canonical_for_persistence(&self) -> Self {
        let mut canonical = self.clone();
        canonical.integrity_hash = canonical.computed_integrity_hash();
        canonical
    }
}
/// Failure to establish a durable anti-equivocation decision before signing.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub(crate) enum LaneDrainSigningGuardError {
    /// A malformed or unsafe journal entry made the guard fail closed.
    #[error("lane drain signing guard is unsafe: {0}")]
    UnsafeJournal(String),
    /// The attempted lane commit vote conflicts with a durable vote decision.
    #[error("lane commit vote conflicts with the durable signing decision")]
    CommitVoteEquivocation,
    /// The attempted lane commit vote follows a durable drain decision.
    #[error("lane incarnation is durably closed to further commit votes")]
    LaneClosed,
    /// The attempted drain body conflicts with a durable drain decision.
    #[error("lane drain body conflicts with the durable signing decision")]
    DrainEquivocation,
    /// The drain frontier is below, or conflicts with, a locally signed commit vote.
    #[error("lane drain frontier does not cover the durable commit-vote high-water")]
    DrainFrontierBelowSignedCommit,
    /// The supplied vote or drain body is structurally invalid.
    #[error("invalid lane drain signing input: {0}")]
    InvalidInput(String),
}
/// Crash-safe, per-incarnation journal used before lane commit and drain signatures.
///
/// Each decision is written, fsynced, atomically renamed, and directory-fsynced
/// before the caller may create a signature. A crash can therefore retain a
/// harmless decision whose signature was never sent, but cannot leave a sent
/// signature without its durable anti-equivocation lock.
#[derive(Debug)]
pub(crate) struct LaneDrainSigningGuard {
    directory: PathBuf,
    _owner_lock: File,
    serial: Mutex<()>,
}
impl LaneDrainSigningGuard {
    /// Open the journal below the Kura root and discard records for finalized,
    /// inactive incarnations. Malformed files, symlinks, and non-canonical
    /// encodings fail startup closed.
    pub(crate) fn open(
        store_root: &Path,
        active_incarnations: &BTreeSet<(LaneId, Hash)>,
    ) -> Result<Self, LaneDrainSigningGuardError> {
        let directory = store_root.join(GUARD_DIRECTORY);
        ensure_regular_directory(&directory)?;
        let owner_lock = acquire_owner_lock(&directory)?;
        reconcile_temps(&directory)?;
        let guard = Self {
            directory,
            _owner_lock: owner_lock,
            serial: Mutex::new(()),
        };
        guard.validate_and_prune(active_incarnations)?;
        Ok(guard)
    }
    fn record_path_for(directory: &Path, key: LaneDrainSigningKeyV1) -> PathBuf {
        directory.join(format!("{}.{}", key.digest(), RECORD_EXTENSION))
    }
    fn record_path(&self, key: LaneDrainSigningKeyV1) -> PathBuf {
        Self::record_path_for(&self.directory, key)
    }
    fn read_record(
        directory: &Path,
        path: &Path,
    ) -> Result<LaneDrainSigningRecordV1, LaneDrainSigningGuardError> {
        let path_metadata =
            fs::symlink_metadata(path).map_err(|error| unsafe_journal(path, error.to_string()))?;
        if path_metadata.file_type().is_symlink() || !path_metadata.file_type().is_file() {
            return Err(unsafe_journal(path, "unsafe record file type"));
        }
        let mut file = OpenOptions::new()
            .read(true)
            .open(path)
            .map_err(|error| unsafe_journal(path, error.to_string()))?;
        let opened_metadata = file
            .metadata()
            .map_err(|error| unsafe_journal(path, error.to_string()))?;
        if !opened_metadata.file_type().is_file()
            || opened_metadata.len() > MAX_RECORD_BYTES as u64
            || opened_metadata.len() != path_metadata.len()
        {
            return Err(unsafe_journal(path, "unsafe record file type or size"));
        }
        let initial_len = opened_metadata.len();
        let mut bytes = Vec::with_capacity(initial_len as usize);
        (&mut file)
            .take((MAX_RECORD_BYTES as u64).saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|error| unsafe_journal(path, error.to_string()))?;
        if bytes.len() > MAX_RECORD_BYTES || bytes.len() as u64 != initial_len {
            return Err(unsafe_journal(path, "record changed size while being read"));
        }
        let record = norito::decode_from_bytes::<LaneDrainSigningRecordV1>(&bytes)
            .map_err(|error| unsafe_journal(path, error.to_string()))?;
        let canonical =
            norito::to_bytes(&record).map_err(|error| unsafe_journal(path, error.to_string()))?;
        if record.version != GUARD_VERSION
            || canonical != bytes
            || Self::record_path_for(directory, record.key) != path
            || record.integrity_hash != record.computed_integrity_hash()
        {
            return Err(unsafe_journal(
                path,
                "unsupported, non-canonical, misnamed, or corrupt record",
            ));
        }
        if record.highest_commit_vote.is_none() && record.drain_body.is_none() {
            return Err(unsafe_journal(path, "empty signing record"));
        }
        if let Some(body) = record.drain_body.as_ref() {
            validate_lane_drain_certificate_body(body)
                .map_err(|error| unsafe_journal(path, error.to_string()))?;
            if LaneDrainSigningKeyV1::from_drain(body) != record.key {
                return Err(unsafe_journal(path, "drain body uses another lane key"));
            }
            ensure_drain_covers_commit(record.highest_commit_vote, body)
                .map_err(|error| unsafe_journal(path, error.to_string()))?;
        }
        Ok(record)
    }
    fn validate_and_prune(
        &self,
        active_incarnations: &BTreeSet<(LaneId, Hash)>,
    ) -> Result<(), LaneDrainSigningGuardError> {
        let mut records = 0_usize;
        let mut removed = false;
        for item in fs::read_dir(&self.directory)
            .map_err(|error| unsafe_journal(&self.directory, error.to_string()))?
        {
            let item = item.map_err(|error| unsafe_journal(&self.directory, error.to_string()))?;
            let path = item.path();
            let name = item.file_name();
            let name = name.to_string_lossy();
            if name == LOCK_FILENAME {
                continue;
            }
            if !valid_record_filename(&name) {
                return Err(unsafe_journal(&path, "unknown journal file"));
            }
            records = records.saturating_add(1);
            if records > MAX_RECORDS {
                return Err(unsafe_journal(
                    &self.directory,
                    "record count exceeds hard limit",
                ));
            }
            let record = Self::read_record(&self.directory, &path)?;
            if !active_incarnations.contains(&(record.key.lane_id, record.key.lane_incarnation)) {
                fs::remove_file(&path).map_err(|error| unsafe_journal(&path, error.to_string()))?;
                removed = true;
            }
        }
        if removed {
            sync_directory(&self.directory)?;
        }
        Ok(())
    }
    fn load_or_empty(
        &self,
        key: LaneDrainSigningKeyV1,
    ) -> Result<(LaneDrainSigningRecordV1, bool), LaneDrainSigningGuardError> {
        let path = self.record_path(key);
        match fs::symlink_metadata(&path) {
            Ok(_) => Self::read_record(&self.directory, &path).map(|record| (record, false)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok((LaneDrainSigningRecordV1::empty(key), true))
            }
            Err(error) => Err(unsafe_journal(&path, error.to_string())),
        }
    }
    fn ensure_capacity_for_new_record(&self) -> Result<(), LaneDrainSigningGuardError> {
        let mut count = 0_usize;
        for item in fs::read_dir(&self.directory)
            .map_err(|error| unsafe_journal(&self.directory, error.to_string()))?
        {
            let item = item.map_err(|error| unsafe_journal(&self.directory, error.to_string()))?;
            if valid_record_filename(&item.file_name().to_string_lossy()) {
                count = count.saturating_add(1);
                if count >= MAX_RECORDS {
                    return Err(unsafe_journal(
                        &self.directory,
                        "record count reached hard limit",
                    ));
                }
            }
        }
        Ok(())
    }
    fn persist_record(
        &self,
        record: &LaneDrainSigningRecordV1,
    ) -> Result<(), LaneDrainSigningGuardError> {
        let record = record.canonical_for_persistence();
        let bytes = norito::to_bytes(&record)
            .map_err(|error| unsafe_journal(&self.directory, error.to_string()))?;
        if bytes.len() > MAX_RECORD_BYTES {
            return Err(unsafe_journal(
                &self.directory,
                "record exceeds hard byte limit",
            ));
        }
        let path = self.record_path(record.key);
        let temp = path.with_extension(TEMP_EXTENSION);
        match fs::symlink_metadata(&temp) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
                    return Err(unsafe_journal(&temp, "unsafe temporary record"));
                }
                fs::remove_file(&temp).map_err(|error| unsafe_journal(&temp, error.to_string()))?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(unsafe_journal(&temp, error.to_string())),
        }
        {
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temp)
                .map_err(|error| unsafe_journal(&temp, error.to_string()))?;
            file.write_all(&bytes)
                .map_err(|error| unsafe_journal(&temp, error.to_string()))?;
            file.sync_all()
                .map_err(|error| unsafe_journal(&temp, error.to_string()))?;
        }
        fs::rename(&temp, &path).map_err(|error| unsafe_journal(&path, error.to_string()))?;
        sync_directory(&self.directory)
    }
    /// Durably authorize one lane commit-vote body before its BLS signature is
    /// created. Heights may advance monotonically; the same height may only be
    /// retried for the exact domain-separated vote body.
    ///
    /// The caller remains responsible for validating that the body was derived
    /// from a canonical proposal; this guard only enforces durable local
    /// anti-equivocation across bodies that reached the signing boundary.
    pub(crate) fn authorize_commit_vote(
        &self,
        body: &LaneBlockVoteBodyV1,
    ) -> Result<(), LaneDrainSigningGuardError> {
        if body.phase != Phase::Commit || body.lane_block_height == 0 {
            return Err(LaneDrainSigningGuardError::InvalidInput(
                "expected a non-zero lane Commit vote".to_owned(),
            ));
        }
        let _serial = self.serial.lock();
        let key = LaneDrainSigningKeyV1::from_commit_vote(body);
        let (mut record, is_new) = self.load_or_empty(key)?;
        if record.drain_body.is_some() {
            return Err(LaneDrainSigningGuardError::LaneClosed);
        }
        let attempted = LaneCommitVoteLockV1::from_body(body);
        match record.highest_commit_vote {
            Some(existing) if attempted.lane_block_height < existing.lane_block_height => {
                return Err(LaneDrainSigningGuardError::CommitVoteEquivocation);
            }
            Some(existing) if attempted.lane_block_height == existing.lane_block_height => {
                return if attempted == existing {
                    Ok(())
                } else {
                    Err(LaneDrainSigningGuardError::CommitVoteEquivocation)
                };
            }
            _ => {}
        }
        if is_new {
            self.ensure_capacity_for_new_record()?;
        }
        record.highest_commit_vote = Some(attempted);
        self.persist_record(&record)
    }
    /// Durably close one lane incarnation before producing a drain vote.
    /// The certified frontier must cover the exact descriptor of every locally
    /// signed lane commit high-water. Structural validation includes the exact
    /// embedded committee length, hash, uniqueness, BLS key type, and quorum.
    /// The caller must still derive the final frontier from canonical state.
    /// An unchanged intent may advance monotonically when delayed pre-close
    /// work reaches the global frontier; the lane remains permanently closed
    /// to Commit votes throughout that refresh.
    pub(crate) fn authorize_drain(
        &self,
        body: &LaneDrainCertificateBodyV1,
    ) -> Result<(), LaneDrainSigningGuardError> {
        validate_lane_drain_certificate_body(body)
            .map_err(|error| LaneDrainSigningGuardError::InvalidInput(error.to_string()))?;
        let _serial = self.serial.lock();
        let key = LaneDrainSigningKeyV1::from_drain(body);
        let (mut record, is_new) = self.load_or_empty(key)?;
        ensure_drain_covers_commit(record.highest_commit_vote, body)?;
        if let Some(existing) = record.drain_body.as_ref() {
            if existing == body {
                return Ok(());
            }
            if existing.intent != body.intent
                || body.final_frontier.lane_block_height
                    <= existing.final_frontier.lane_block_height
            {
                return Err(LaneDrainSigningGuardError::DrainEquivocation);
            }
        }
        if is_new {
            self.ensure_capacity_for_new_record()?;
        }
        record.drain_body = Some(body.clone());
        self.persist_record(&record)
    }
    #[cfg(test)]
    fn decision(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_incarnation: Hash,
    ) -> Result<Option<LaneDrainSigningRecordV1>, LaneDrainSigningGuardError> {
        let _serial = self.serial.lock();
        let key = LaneDrainSigningKeyV1 {
            lane_id,
            dataspace_id,
            lane_incarnation,
        };
        let path = self.record_path(key);
        match fs::symlink_metadata(&path) {
            Ok(_) => Self::read_record(&self.directory, &path).map(Some),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(unsafe_journal(&path, error.to_string())),
        }
    }
}
fn ensure_drain_covers_commit(
    highest_commit_vote: Option<LaneCommitVoteLockV1>,
    body: &LaneDrainCertificateBodyV1,
) -> Result<(), LaneDrainSigningGuardError> {
    let Some(highest) = highest_commit_vote else {
        return Ok(());
    };
    if body.final_frontier.lane_block_height < highest.lane_block_height
        || (body.final_frontier.lane_block_height == highest.lane_block_height
            && body.final_frontier.lane_block_descriptor_hash != Some(highest.descriptor_hash))
    {
        return Err(LaneDrainSigningGuardError::DrainFrontierBelowSignedCommit);
    }
    Ok(())
}
fn valid_record_filename(name: &str) -> bool {
    let suffix = format!(".{RECORD_EXTENSION}");
    let Some(stem) = name.strip_suffix(&suffix) else {
        return false;
    };
    stem.len() == Hash::LENGTH * 2 && stem.bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn valid_temp_filename(name: &str) -> bool {
    let suffix = format!(".{TEMP_EXTENSION}");
    let Some(stem) = name.strip_suffix(&suffix) else {
        return false;
    };
    stem.len() == Hash::LENGTH * 2 && stem.bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn acquire_owner_lock(directory: &Path) -> Result<File, LaneDrainSigningGuardError> {
    let path = directory.join(LOCK_FILENAME);
    let before = match fs::symlink_metadata(&path) {
        Ok(metadata) => {
            validate_owner_lock_metadata(&path, &metadata)?;
            Some(metadata)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(unsafe_journal(&path, error.to_string())),
    };
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true);
    set_no_follow_flag(&mut options);
    let file = options
        .open(&path)
        .map_err(|error| unsafe_journal(&path, error.to_string()))?;
    let opened = file
        .metadata()
        .map_err(|error| unsafe_journal(&path, error.to_string()))?;
    validate_owner_lock_metadata(&path, &opened)?;
    if before
        .as_ref()
        .is_some_and(|metadata| !metadata_identifies_same_file(metadata, &opened))
    {
        return Err(unsafe_journal(
            &path,
            "owner lock changed between inspection and open",
        ));
    }
    let after =
        fs::symlink_metadata(&path).map_err(|error| unsafe_journal(&path, error.to_string()))?;
    validate_owner_lock_metadata(&path, &after)?;
    if !metadata_identifies_same_file(&opened, &after) {
        return Err(unsafe_journal(
            &path,
            "owner lock path changed while opening",
        ));
    }
    match file.try_lock() {
        Ok(()) => {}
        Err(fs::TryLockError::WouldBlock) => {
            return Err(unsafe_journal(
                &path,
                "lane drain signing directory is already owned by another process",
            ));
        }
        Err(fs::TryLockError::Error(error)) => {
            return Err(unsafe_journal(&path, error.to_string()));
        }
    }
    let locked =
        fs::symlink_metadata(&path).map_err(|error| unsafe_journal(&path, error.to_string()))?;
    validate_owner_lock_metadata(&path, &locked)?;
    if !metadata_identifies_same_file(&opened, &locked) {
        return Err(unsafe_journal(
            &path,
            "owner lock path changed while locking",
        ));
    }
    if before.is_none() {
        file.sync_all()
            .map_err(|error| unsafe_journal(&path, error.to_string()))?;
        sync_directory(directory)?;
    }
    Ok(file)
}
fn validate_owner_lock_metadata(
    path: &Path,
    metadata: &fs::Metadata,
) -> Result<(), LaneDrainSigningGuardError> {
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() || metadata.len() != 0 {
        return Err(unsafe_journal(
            path,
            "owner lock must be an empty regular file",
        ));
    }
    #[cfg(unix)]
    if metadata.nlink() != 1 {
        return Err(unsafe_journal(
            path,
            "owner lock must have exactly one hard link",
        ));
    }
    Ok(())
}
#[cfg(unix)]
fn metadata_identifies_same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.dev() == right.dev() && left.ino() == right.ino()
}
#[cfg(not(unix))]
fn metadata_identifies_same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.len() == right.len()
}
#[cfg(unix)]
fn set_no_follow_flag(options: &mut OpenOptions) {
    options.custom_flags(platform_no_follow_flag());
}
#[cfg(not(unix))]
fn set_no_follow_flag(_options: &mut OpenOptions) {}
#[cfg(any(target_os = "linux", target_os = "android"))]
fn platform_no_follow_flag() -> i32 {
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
fn platform_no_follow_flag() -> i32 {
    0x100
}
#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))
))]
fn platform_no_follow_flag() -> i32 {
    0
}
fn ensure_regular_directory(path: &Path) -> Result<(), LaneDrainSigningGuardError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
                return Err(unsafe_journal(path, "unsafe journal directory"));
            }
            return Ok(());
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(unsafe_journal(path, error.to_string())),
    }
    fs::create_dir_all(path).map_err(|error| unsafe_journal(path, error.to_string()))?;
    let metadata =
        fs::symlink_metadata(path).map_err(|error| unsafe_journal(path, error.to_string()))?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
        return Err(unsafe_journal(path, "unsafe journal directory"));
    }
    sync_directory(path)?;
    if let Some(parent) = path.parent() {
        // Persist the new directory entry itself before any signature can rely
        // on a record below it surviving a power loss.
        sync_directory(if parent.as_os_str().is_empty() {
            Path::new(".")
        } else {
            parent
        })?;
    }
    Ok(())
}
fn reconcile_temps(directory: &Path) -> Result<(), LaneDrainSigningGuardError> {
    let mut removed = false;
    for item in
        fs::read_dir(directory).map_err(|error| unsafe_journal(directory, error.to_string()))?
    {
        let item = item.map_err(|error| unsafe_journal(directory, error.to_string()))?;
        let path = item.path();
        let name = item.file_name();
        let name = name.to_string_lossy();
        if name == LOCK_FILENAME {
            continue;
        }
        if valid_record_filename(&name) {
            continue;
        }
        if !valid_temp_filename(&name) {
            return Err(unsafe_journal(&path, "unknown journal file"));
        }
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| unsafe_journal(&path, error.to_string()))?;
        if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
            return Err(unsafe_journal(&path, "unsafe temporary record"));
        }
        // A signature is produced only after the final rename and directory
        // fsync, so an unpublished temp can always be discarded on restart.
        fs::remove_file(&path).map_err(|error| unsafe_journal(&path, error.to_string()))?;
        removed = true;
    }
    if removed {
        sync_directory(directory)?;
    }
    Ok(())
}
fn sync_directory(path: &Path) -> Result<(), LaneDrainSigningGuardError> {
    let directory = OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(|error| unsafe_journal(path, error.to_string()))?;
    directory
        .sync_all()
        .map_err(|error| unsafe_journal(path, error.to_string()))
}
fn unsafe_journal(path: &Path, message: impl Into<String>) -> LaneDrainSigningGuardError {
    LaneDrainSigningGuardError::UnsafeJournal(format!("{}: {}", path.display(), message.into()))
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, HashOf, KeyPair};
    use iroha_data_model::{
        consensus::VALIDATOR_SET_HASH_VERSION_V1,
        merge::{LaneDrainFrontierV1, LaneDrainIntentV1},
        peer::PeerId,
    };
    fn incarnation() -> Hash {
        Hash::new(b"lane-drain-signing-guard-incarnation")
    }
    fn active_incarnations() -> BTreeSet<(LaneId, Hash)> {
        BTreeSet::from([(LaneId::new(3), incarnation())])
    }
    fn validator_set() -> Vec<PeerId> {
        vec![PeerId::new(
            KeyPair::try_from_seed(b"lane-drain-guard-validator".to_vec(), Algorithm::BlsNormal)
                .expect("derive BLS validator")
                .public_key()
                .clone(),
        )]
    }
    fn commit_vote(height: u64, descriptor_byte: u8) -> LaneBlockVoteBodyV1 {
        LaneBlockVoteBodyV1 {
            phase: Phase::Commit,
            lane_id: LaneId::new(3),
            dataspace_id: DataSpaceId::new(7),
            lane_incarnation: incarnation(),
            proposal_height: 11,
            lane_block_height: height,
            lane_block_view: 0,
            proposal_hash: Hash::prehashed([descriptor_byte.wrapping_add(1); Hash::LENGTH]),
            descriptor_hash: Hash::prehashed([descriptor_byte; Hash::LENGTH]),
            subject_hash: Hash::new(b"subject"),
            payload_ownership_hash: Hash::new(b"ownership"),
            rbc_instance_hash: Hash::new(b"rbc"),
            accepted_candidate_indices: vec![0],
            accepted_transaction_hashes: vec![Hash::new(b"tx")],
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::from_untyped_unchecked(Hash::new(b"validators")),
            validator_count: 1,
            min_quorum: 1,
            qc_mode_tag: "permissioned".to_owned(),
        }
    }
    fn drain_body(height: u64, descriptor_byte: u8) -> LaneDrainCertificateBodyV1 {
        let validator_set = validator_set();
        LaneDrainCertificateBodyV1 {
            version: 1,
            intent: LaneDrainIntentV1 {
                version: 1,
                network_id: iroha_data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
                    iroha_data_model::block::BlockHeader,
                >::from_untyped_unchecked(
                    Hash::new(b"lane-drain-genesis"),
                )),
                lane_id: LaneId::new(3),
                dataspace_id: DataSpaceId::new(7),
                lane_incarnation: incarnation(),
                close_global_height: 12,
                initial_frontier: LaneDrainFrontierV1::ordinary(
                    LaneId::new(3),
                    DataSpaceId::new(7),
                    incarnation(),
                    4,
                    Some(Hash::prehashed([4; Hash::LENGTH])),
                ),
                validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                validator_set_hash: HashOf::new(&validator_set),
                validator_set,
                validator_count: 1,
                min_quorum: 1,
            },
            final_frontier: LaneDrainFrontierV1::ordinary(
                LaneId::new(3),
                DataSpaceId::new(7),
                incarnation(),
                height,
                Some(Hash::prehashed([descriptor_byte; Hash::LENGTH])),
            ),
        }
    }
    #[test]
    fn drain_must_cover_durable_commit_vote_high_water() {
        let temp = tempfile::tempdir().expect("tempdir");
        let guard =
            LaneDrainSigningGuard::open(temp.path(), &active_incarnations()).expect("open guard");
        guard
            .authorize_commit_vote(&commit_vote(5, 5))
            .expect("authorize commit vote");
        assert_eq!(
            guard.authorize_drain(&drain_body(4, 4)),
            Err(LaneDrainSigningGuardError::DrainFrontierBelowSignedCommit)
        );
        assert_eq!(
            guard.authorize_drain(&drain_body(5, 9)),
            Err(LaneDrainSigningGuardError::DrainFrontierBelowSignedCommit)
        );
        guard
            .authorize_drain(&drain_body(5, 5))
            .expect("matching frontier closes lane");
    }
    #[test]
    fn drain_guard_rejects_an_intent_with_a_non_exact_embedded_committee() {
        let temp = tempfile::tempdir().expect("tempdir");
        let guard =
            LaneDrainSigningGuard::open(temp.path(), &active_incarnations()).expect("open guard");
        let mut body = drain_body(5, 5);
        body.intent.validator_set.clear();
        assert!(matches!(
            guard.authorize_drain(&body),
            Err(LaneDrainSigningGuardError::InvalidInput(_))
        ));
        assert!(
            guard
                .decision(LaneId::new(3), DataSpaceId::new(7), incarnation())
                .expect("read decision")
                .is_none(),
            "invalid committee input must not create a durable close decision"
        );
    }
    #[test]
    fn maximum_supported_committee_drain_record_fits_and_reopens() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut body = drain_body(5, 5);
        let mut committee = (0..crate::lane_consensus::MAX_LANE_BLOCK_VALIDATORS)
            .map(|index| {
                let seed = u8::try_from(index + 1).expect("fixture index fits in u8");
                let keypair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("derive maximum-committee validator");
                PeerId::new(keypair.public_key().clone())
            })
            .collect::<Vec<_>>();
        committee.sort();
        body.intent.validator_count =
            u32::try_from(committee.len()).expect("maximum committee count fits u32");
        body.intent.min_quorum = u32::try_from(
            crate::sumeragi::network_topology::commit_quorum_from_len(committee.len()),
        )
        .expect("maximum committee quorum fits u32");
        body.intent.validator_set_hash = HashOf::new(&committee);
        body.intent.validator_set = committee;
        let record_path = {
            let guard = LaneDrainSigningGuard::open(temp.path(), &active_incarnations())
                .expect("open guard");
            guard
                .authorize_drain(&body)
                .expect("persist maximum-committee drain body");
            guard.record_path(LaneDrainSigningKeyV1::from_drain(&body))
        };
        let record_len = fs::metadata(&record_path)
            .expect("maximum-committee record metadata")
            .len();
        assert!(record_len <= MAX_RECORD_BYTES as u64);
        let reopened =
            LaneDrainSigningGuard::open(temp.path(), &active_incarnations()).expect("reopen guard");
        assert_eq!(
            reopened
                .decision(LaneId::new(3), DataSpaceId::new(7), incarnation())
                .expect("read maximum-committee decision")
                .expect("maximum-committee decision exists")
                .drain_body,
            Some(body)
        );
    }
    #[test]
    fn drain_lock_survives_restart_allows_frontier_advance_and_rejects_commit() {
        let temp = tempfile::tempdir().expect("tempdir");
        let body = drain_body(5, 5);
        {
            let guard = LaneDrainSigningGuard::open(temp.path(), &active_incarnations())
                .expect("open guard");
            guard
                .authorize_commit_vote(&commit_vote(5, 5))
                .expect("authorize commit vote");
            guard.authorize_drain(&body).expect("authorize drain");
        }
        let reopened =
            LaneDrainSigningGuard::open(temp.path(), &active_incarnations()).expect("reopen guard");
        reopened.authorize_drain(&body).expect("idempotent retry");
        assert_eq!(
            reopened.authorize_commit_vote(&commit_vote(6, 6)),
            Err(LaneDrainSigningGuardError::LaneClosed)
        );
        let advanced = drain_body(6, 6);
        reopened
            .authorize_drain(&advanced)
            .expect("delayed pre-close work may advance the closed frontier");
        assert_eq!(
            reopened.authorize_drain(&body),
            Err(LaneDrainSigningGuardError::DrainEquivocation)
        );
        assert_eq!(
            reopened.authorize_drain(&drain_body(6, 7)),
            Err(LaneDrainSigningGuardError::DrainEquivocation)
        );
        drop(reopened);
        let refreshed =
            LaneDrainSigningGuard::open(temp.path(), &active_incarnations()).expect("reopen guard");
        assert_eq!(
            refreshed.authorize_commit_vote(&commit_vote(7, 7)),
            Err(LaneDrainSigningGuardError::LaneClosed)
        );
        let decision = refreshed
            .decision(LaneId::new(3), DataSpaceId::new(7), incarnation())
            .expect("read decision")
            .expect("decision exists");
        assert_eq!(decision.drain_body, Some(advanced));
    }
    #[test]
    fn same_height_commit_equivocation_and_regression_are_rejected() {
        let temp = tempfile::tempdir().expect("tempdir");
        let guard =
            LaneDrainSigningGuard::open(temp.path(), &active_incarnations()).expect("open guard");
        let vote = commit_vote(5, 5);
        guard
            .authorize_commit_vote(&vote)
            .expect("authorize first vote");
        guard
            .authorize_commit_vote(&vote)
            .expect("idempotent retry");
        assert_eq!(
            guard.authorize_commit_vote(&commit_vote(5, 6)),
            Err(LaneDrainSigningGuardError::CommitVoteEquivocation)
        );
        assert_eq!(
            guard.authorize_commit_vote(&commit_vote(4, 4)),
            Err(LaneDrainSigningGuardError::CommitVoteEquivocation)
        );
        guard
            .authorize_commit_vote(&commit_vote(6, 6))
            .expect("monotonic advance");
    }
    #[test]
    fn same_hashes_cannot_mask_a_different_commit_vote_body() {
        let temp = tempfile::tempdir().expect("tempdir");
        let guard =
            LaneDrainSigningGuard::open(temp.path(), &active_incarnations()).expect("open guard");
        let vote = commit_vote(5, 5);
        guard
            .authorize_commit_vote(&vote)
            .expect("authorize first vote");
        let mut conflicting = vote;
        conflicting.subject_hash = Hash::new(b"different subject behind unchanged outer hashes");
        assert_eq!(
            guard.authorize_commit_vote(&conflicting),
            Err(LaneDrainSigningGuardError::CommitVoteEquivocation)
        );
    }
    #[test]
    fn corrupt_record_fails_restart_closed() {
        let temp = tempfile::tempdir().expect("tempdir");
        let record_path = {
            let guard = LaneDrainSigningGuard::open(temp.path(), &active_incarnations())
                .expect("open guard");
            let vote = commit_vote(5, 5);
            guard.authorize_commit_vote(&vote).expect("authorize vote");
            guard.record_path(LaneDrainSigningKeyV1::from_commit_vote(&vote))
        };
        let bytes = fs::read(&record_path).expect("read signing record");
        let mut record = norito::decode_from_bytes::<LaneDrainSigningRecordV1>(&bytes)
            .expect("decode canonical signing record");
        record
            .highest_commit_vote
            .as_mut()
            .expect("commit-vote high-water")
            .lane_block_height = 1;
        let corrupt_but_canonical =
            norito::to_bytes(&record).expect("re-encode structurally canonical corrupt record");
        fs::write(&record_path, corrupt_but_canonical).expect("corrupt signing record");
        assert!(matches!(
            LaneDrainSigningGuard::open(temp.path(), &active_incarnations()),
            Err(LaneDrainSigningGuardError::UnsafeJournal(_))
        ));
    }
    #[test]
    fn unpublished_regular_temp_is_discarded_on_restart() {
        let temp = tempfile::tempdir().expect("tempdir");
        let (record_path, temp_path) = {
            let guard = LaneDrainSigningGuard::open(temp.path(), &active_incarnations())
                .expect("open guard");
            let vote = commit_vote(5, 5);
            guard.authorize_commit_vote(&vote).expect("authorize vote");
            let record_path = guard.record_path(LaneDrainSigningKeyV1::from_commit_vote(&vote));
            let temp_path = record_path.with_extension(TEMP_EXTENSION);
            fs::copy(&record_path, &temp_path).expect("simulate unpublished temp record");
            (record_path, temp_path)
        };
        LaneDrainSigningGuard::open(temp.path(), &active_incarnations())
            .expect("restart discards unpublished temp");
        assert!(record_path.exists());
        assert!(!temp_path.exists());
    }
    #[cfg(unix)]
    #[test]
    fn journal_directory_record_and_temp_symlinks_fail_closed() {
        use std::os::unix::fs::symlink;
        let target = tempfile::tempdir().expect("symlink target root");
        let directory_link_root = tempfile::tempdir().expect("directory symlink root");
        symlink(
            target.path(),
            directory_link_root.path().join(GUARD_DIRECTORY),
        )
        .expect("create journal directory symlink");
        assert!(matches!(
            LaneDrainSigningGuard::open(directory_link_root.path(), &active_incarnations()),
            Err(LaneDrainSigningGuardError::UnsafeJournal(_))
        ));
        let owner_link_root = tempfile::tempdir().expect("owner-lock symlink root");
        let owner_directory = owner_link_root.path().join(GUARD_DIRECTORY);
        fs::create_dir_all(&owner_directory).expect("create owner-lock directory");
        let owner_target = owner_link_root.path().join("owner-target");
        fs::write(&owner_target, b"").expect("create owner-lock target");
        symlink(&owner_target, owner_directory.join(LOCK_FILENAME))
            .expect("create owner-lock symlink");
        assert!(matches!(
            LaneDrainSigningGuard::open(owner_link_root.path(), &active_incarnations()),
            Err(LaneDrainSigningGuardError::UnsafeJournal(_))
        ));
        let owner_hardlink_root = tempfile::tempdir().expect("owner-lock hardlink root");
        let owner_directory = owner_hardlink_root.path().join(GUARD_DIRECTORY);
        fs::create_dir_all(&owner_directory).expect("create owner-hardlink directory");
        let owner_target = owner_hardlink_root.path().join("owner-target");
        fs::write(&owner_target, b"").expect("create owner-hardlink target");
        fs::hard_link(&owner_target, owner_directory.join(LOCK_FILENAME))
            .expect("create owner-lock hardlink");
        assert!(matches!(
            LaneDrainSigningGuard::open(owner_hardlink_root.path(), &active_incarnations()),
            Err(LaneDrainSigningGuardError::UnsafeJournal(_))
        ));
        let source_root = tempfile::tempdir().expect("source record root");
        let vote = commit_vote(5, 5);
        let source_path = {
            let guard = LaneDrainSigningGuard::open(source_root.path(), &active_incarnations())
                .expect("open source guard");
            guard
                .authorize_commit_vote(&vote)
                .expect("authorize source vote");
            guard.record_path(LaneDrainSigningKeyV1::from_commit_vote(&vote))
        };
        let record_link_root = tempfile::tempdir().expect("record symlink root");
        let record_guard = LaneDrainSigningGuard::open(
            record_link_root.path(),
            &BTreeSet::<(LaneId, Hash)>::new(),
        )
        .expect("create empty record-link guard");
        let record_link = record_guard.record_path(LaneDrainSigningKeyV1::from_commit_vote(&vote));
        symlink(&source_path, &record_link).expect("create record symlink");
        drop(record_guard);
        assert!(matches!(
            LaneDrainSigningGuard::open(record_link_root.path(), &active_incarnations()),
            Err(LaneDrainSigningGuardError::UnsafeJournal(_))
        ));
        let temp_link_root = tempfile::tempdir().expect("temp symlink root");
        let temp_guard =
            LaneDrainSigningGuard::open(temp_link_root.path(), &BTreeSet::<(LaneId, Hash)>::new())
                .expect("create empty temp-link guard");
        let temp_link = temp_guard
            .record_path(LaneDrainSigningKeyV1::from_commit_vote(&vote))
            .with_extension(TEMP_EXTENSION);
        symlink(&source_path, &temp_link).expect("create temp symlink");
        drop(temp_guard);
        assert!(matches!(
            LaneDrainSigningGuard::open(temp_link_root.path(), &active_incarnations()),
            Err(LaneDrainSigningGuardError::UnsafeJournal(_))
        ));
    }
    #[test]
    fn inactive_records_are_pruned_but_unknown_files_fail_closed() {
        let temp = tempfile::tempdir().expect("tempdir");
        {
            let guard = LaneDrainSigningGuard::open(temp.path(), &active_incarnations())
                .expect("open guard");
            guard
                .authorize_commit_vote(&commit_vote(5, 5))
                .expect("authorize vote");
        }
        let empty = BTreeSet::new();
        let guard = LaneDrainSigningGuard::open(temp.path(), &empty).expect("prune inactive");
        assert!(
            guard
                .decision(LaneId::new(3), DataSpaceId::new(7), incarnation())
                .expect("read decision")
                .is_none()
        );
        std::fs::write(guard.directory.join("unexpected"), b"bad").expect("write unknown file");
        assert!(matches!(
            LaneDrainSigningGuard::open(temp.path(), &empty),
            Err(LaneDrainSigningGuardError::UnsafeJournal(_))
        ));
    }
    #[test]
    fn signing_directory_has_one_process_owner_and_releases_on_drop() {
        let temp = tempfile::tempdir().expect("tempdir");
        let first =
            LaneDrainSigningGuard::open(temp.path(), &active_incarnations()).expect("first owner");
        assert!(matches!(
            LaneDrainSigningGuard::open(temp.path(), &active_incarnations()),
            Err(LaneDrainSigningGuardError::UnsafeJournal(message))
                if message.contains("already owned by another process")
        ));
        drop(first);
        LaneDrainSigningGuard::open(temp.path(), &active_incarnations())
            .expect("ownership lock releases when the process guard drops");
    }
}
