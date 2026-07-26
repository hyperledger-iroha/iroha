//! Durable commit-roster journal persisted alongside the block store.
//!
//! This journal is a content-addressed archival projection of legacy-v1 commit certificates and
//! validator-set checkpoints. Structural validation and durable hashing do **not** authenticate
//! the legacy BLS certificate. Live first-release consensus, block sync, and startup recovery must
//! use Kura's cryptographically verified Sumeragi-v2 finality artifacts instead.

use std::{
    collections::{BTreeMap, btree_map::Entry},
    fs,
    io::{self, Read, Write},
    num::NonZeroUsize,
    path::{Path, PathBuf},
};

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    block::BlockHeader,
    consensus::{Qc, VALIDATOR_SET_HASH_VERSION_V1, ValidatorSetCheckpoint},
};
use iroha_logger::warn;
use norito::{
    codec::{Decode, Encode},
    decode_from_bytes, to_bytes,
};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::sumeragi::{
    consensus::{NPOS_TAG, PERMISSIONED_TAG, Phase},
    stake_snapshot::CommitStakeSnapshot,
};

static COMMIT_ROSTER_PUBLICATION_LOCK: parking_lot::Mutex<()> = parking_lot::Mutex::new(());

/// Persisted commit-roster journal payload.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
struct PersistedCommitRosters {
    /// Journal version for format control.
    version: u32,
    /// Shared stake snapshots referenced by stored commit roster entries.
    #[norito(default)]
    stake_snapshots: Vec<CommitStakeSnapshot>,
    /// Stored commit roster entries.
    entries: Vec<CommitRosterRecord>,
}

/// Persisted commit-roster entry.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
struct CommitRosterRecord {
    /// Block height certified by this entry.
    height: u64,
    /// Block hash certified by this entry.
    block_hash: HashOf<BlockHeader>,
    /// Commit certificate for the block.
    commit_qc: Qc,
    /// Validator set checkpoint for the block.
    validator_checkpoint: ValidatorSetCheckpoint,
    /// Optional index into the payload-level stake snapshot table.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    stake_snapshot_index: Option<u32>,
    /// Optional stake snapshot aligned to the validator set.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    stake_snapshot: Option<CommitStakeSnapshot>,
}

/// Errors returned when loading or persisting commit rosters.
#[derive(Debug, Error)]
pub enum CommitRosterJournalError {
    /// Commit-roster storage layout is structurally invalid.
    #[error("invalid commit roster storage layout at {path}: {reason}")]
    InvalidStorage {
        /// Path whose shape was rejected.
        path: PathBuf,
        /// Stable structural reason.
        reason: &'static str,
    },
    /// Failed to read the persisted journal.
    #[error("failed to read commit roster journal {path}: {source}")]
    Read {
        /// Path that failed.
        path: PathBuf,
        /// Source error.
        #[source]
        source: std::io::Error,
    },
    /// Failed to decode the persisted journal.
    #[error("failed to decode commit roster journal {path}: {source}")]
    Decode {
        /// Path that failed.
        path: PathBuf,
        /// Source decode error.
        #[source]
        source: norito::core::Error,
    },
    /// Failed to write the journal to disk.
    #[error("failed to persist commit roster journal {path}: {source}")]
    Write {
        /// Path that failed.
        path: PathBuf,
        /// Source error.
        #[source]
        source: std::io::Error,
    },
    /// The journal was renamed into place but its parent-directory sync was not acknowledged.
    #[error("failed to confirm commit roster journal namespace replacement at {path}: {source}")]
    NamespaceSync {
        /// Parent directory whose namespace sync failed.
        path: PathBuf,
        /// Source error.
        #[source]
        source: std::io::Error,
    },
    /// The journal namespace may refer to either side of a failed atomic replacement.
    #[error("commit roster journal storage state is unknown at {path}; restart before retrying")]
    StorageUnknown {
        /// Path whose durable namespace could not be established.
        path: PathBuf,
    },
    /// Failed to encode the journal payload.
    #[error("failed to encode commit roster journal: {0}")]
    Encode(#[source] norito::core::Error),
    /// Persisted journal uses an unsupported version.
    #[error("unsupported commit roster journal version {version} at {path}")]
    UnsupportedVersion {
        /// Path for the journal.
        path: PathBuf,
        /// Unsupported version encountered.
        version: u32,
    },
    /// A decodable row does not describe one exact signed commit subject.
    #[error("invalid commit roster journal entry at height {height} in {path}: {reason}")]
    InvalidEntry {
        /// Path for the journal.
        path: PathBuf,
        /// Height carried by the invalid row.
        height: u64,
        /// Stable validation failure reason.
        reason: &'static str,
    },
}

/// Structurally validated archival projection of a legacy commit certificate and checkpoint.
///
/// This value is not cryptographic finality authority. Consensus and recovery must use the
/// corresponding verified Sumeragi-v2 finality artifact from Kura.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitRosterSnapshot {
    /// Commit certificate for the block.
    pub commit_qc: Qc,
    /// Validator set checkpoint for the block.
    pub validator_checkpoint: ValidatorSetCheckpoint,
    /// Optional stake snapshot aligned to the validator set.
    pub stake_snapshot: Option<CommitStakeSnapshot>,
}

/// Journal that records commit rosters derived from committed blocks.
#[derive(Debug, Clone)]
pub struct CommitRosterJournal {
    entries: BTreeMap<(u64, HashOf<BlockHeader>), CommitRosterSnapshot>,
    path: PathBuf,
    retention: NonZeroUsize,
    dirty: bool,
    storage_unknown: bool,
    #[cfg(test)]
    fail_after_rename_once: bool,
    #[cfg(test)]
    fail_pointer_persist_once: bool,
    #[cfg(test)]
    replace_current_before_gc_once: bool,
}

impl CommitRosterJournal {
    /// Extra non-genesis row retained for an authenticated successor before Kura commits it.
    const AUTHENTICATED_PRE_KURA_SUCCESSOR_RESERVE: usize = 1;

    /// Directory used for content-addressed commit-roster generations.
    pub const JOURNAL_FILE: &'static str = "commit-rosters";
    const LEGACY_JOURNAL_FILE: &'static str = "commit-rosters.norito";
    const CURRENT_FILE: &'static str = "current";
    const GENERATIONS_DIR: &'static str = "generations";
    const MAX_PAYLOAD_BYTES: u64 = 16 * 1024 * 1024;
    const MAX_GENERATIONS: usize = 4096;
    const JOURNAL_VERSION: u32 = 2;

    /// Build the canonical journal path under the provided root.
    #[must_use]
    pub fn journal_path(root: &Path) -> PathBuf {
        if root.as_os_str().is_empty() {
            PathBuf::new()
        } else {
            root.join(Self::JOURNAL_FILE)
        }
    }

    /// Construct a fresh journal with no entries.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>, retention: NonZeroUsize) -> Self {
        Self {
            entries: BTreeMap::new(),
            path: path.into(),
            retention,
            dirty: false,
            storage_unknown: false,
            #[cfg(test)]
            fail_after_rename_once: false,
            #[cfg(test)]
            fail_pointer_persist_once: false,
            #[cfg(test)]
            replace_current_before_gc_once: false,
        }
    }

    /// Inject one test-only persistence failure after the atomic rename and before directory sync.
    #[cfg(test)]
    pub(crate) fn fail_after_rename_once_for_tests(&mut self) {
        self.fail_after_rename_once = true;
    }

    /// Inject one test-only failure at the atomic pointer-persist boundary.
    #[cfg(test)]
    fn fail_pointer_persist_once_for_tests(&mut self) {
        self.fail_pointer_persist_once = true;
    }

    /// Inject one test-only current-pointer substitution immediately before GC validation.
    #[cfg(test)]
    fn replace_current_before_gc_once_for_tests(&mut self) {
        self.replace_current_before_gc_once = true;
    }

    /// Fence this process from using a journal whose durable namespace is ambiguous.
    pub(crate) fn mark_storage_unknown(&mut self) {
        self.storage_unknown = true;
    }

    /// Return whether journal access requires a process restart to recover durable state.
    #[must_use]
    pub(crate) fn storage_is_unknown(&self) -> bool {
        self.storage_unknown
    }

    /// Load a journal from disk, accepting only exact duplicate rows for one block subject.
    ///
    /// Missing files are treated as empty journals. Unsupported versions surface an error.
    ///
    /// # Errors
    ///
    /// Returns [`CommitRosterJournalError::Read`], [`CommitRosterJournalError::Decode`],
    /// [`CommitRosterJournalError::UnsupportedVersion`], or
    /// [`CommitRosterJournalError::InvalidEntry`] when the durable payload cannot be accepted.
    pub fn load(
        path: impl Into<PathBuf>,
        retention: NonZeroUsize,
    ) -> Result<Self, CommitRosterJournalError> {
        let path = path.into();
        let mut journal = Self::new(path.clone(), retention);
        if path.as_os_str().is_empty() {
            return Ok(journal);
        }
        let legacy = path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(Self::LEGACY_JOURNAL_FILE);
        if fs::symlink_metadata(&legacy).is_ok() {
            return Err(CommitRosterJournalError::InvalidStorage {
                path: legacy,
                reason: "legacy mutable commit-roster journals are unsupported",
            });
        }
        let root_metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(journal),
            Err(source) => {
                return Err(CommitRosterJournalError::Read {
                    path: path.clone(),
                    source,
                });
            }
        };
        if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
            return Err(CommitRosterJournalError::InvalidStorage {
                path: path.clone(),
                reason: "commit-roster root must be a direct directory",
            });
        }
        let root_identity = direct_roster_directory_identity(&path).map_err(|source| {
            CommitRosterJournalError::Read {
                path: path.clone(),
                source,
            }
        })?;
        let current_path = path.join(Self::CURRENT_FILE);
        if let Err(source) = fs::symlink_metadata(&current_path) {
            if source.kind() != io::ErrorKind::NotFound {
                return Err(CommitRosterJournalError::Read {
                    path: current_path,
                    source,
                });
            }
            let generations = path.join(Self::GENERATIONS_DIR);
            match fs::symlink_metadata(&generations) {
                Ok(_) => {
                    direct_roster_directory_identity(&generations).map_err(|source| {
                        CommitRosterJournalError::Read {
                            path: generations,
                            source,
                        }
                    })?;
                }
                Err(source) if source.kind() == io::ErrorKind::NotFound => {}
                Err(source) => {
                    return Err(CommitRosterJournalError::Read {
                        path: generations,
                        source,
                    });
                }
            }
            verify_roster_directory_identity(&path, root_identity).map_err(|source| {
                CommitRosterJournalError::Read {
                    path: path.clone(),
                    source,
                }
            })?;
            // A generation without a published pointer is an uncommitted crash orphan. Loading is
            // deliberately read-only; the next authorized publication may reuse or collect it.
            return Ok(journal);
        }
        let pointer = read_bound_roster_file(&current_path, 65).map_err(|source| {
            CommitRosterJournalError::Read {
                path: current_path.clone(),
                source,
            }
        })?;
        let pointer = std::str::from_utf8(&pointer).map_err(|_| {
            CommitRosterJournalError::InvalidStorage {
                path: current_path.clone(),
                reason: "current pointer is not UTF-8",
            }
        })?;
        let Some(digest) = pointer.strip_suffix('\n') else {
            return Err(CommitRosterJournalError::InvalidStorage {
                path: current_path,
                reason: "current pointer is not canonical",
            });
        };
        if digest.len() != 64
            || hex::decode(digest)
                .ok()
                .is_none_or(|bytes| bytes.len() != 32 || hex::encode(bytes) != digest)
        {
            return Err(CommitRosterJournalError::InvalidStorage {
                path: current_path,
                reason: "current pointer is not a lowercase SHA-256 digest",
            });
        }
        let generations = path.join(Self::GENERATIONS_DIR);
        let generations_metadata = fs::symlink_metadata(&generations).map_err(|source| {
            CommitRosterJournalError::Read {
                path: generations.clone(),
                source,
            }
        })?;
        if generations_metadata.file_type().is_symlink() || !generations_metadata.is_dir() {
            return Err(CommitRosterJournalError::InvalidStorage {
                path: generations,
                reason: "commit-roster generations must be a direct directory",
            });
        }
        let generations_identity =
            direct_roster_directory_identity(&generations).map_err(|source| {
                CommitRosterJournalError::Read {
                    path: generations.clone(),
                    source,
                }
            })?;
        let generation_path = generations.join(format!("{digest}.norito"));
        let bytes = read_bound_roster_file(&generation_path, Self::MAX_PAYLOAD_BYTES).map_err(
            |source| CommitRosterJournalError::Read {
                path: generation_path.clone(),
                source,
            },
        )?;
        if hex::encode(Sha256::digest(&bytes)) != digest {
            return Err(CommitRosterJournalError::InvalidStorage {
                path: generation_path.clone(),
                reason: "generation payload digest does not match current pointer",
            });
        }
        let root_after =
            fs::symlink_metadata(&path).map_err(|source| CommitRosterJournalError::Read {
                path: path.clone(),
                source,
            })?;
        let generations_after = fs::symlink_metadata(&generations).map_err(|source| {
            CommitRosterJournalError::Read {
                path: generations.clone(),
                source,
            }
        })?;
        if root_after.file_type().is_symlink()
            || !root_after.is_dir()
            || generations_after.file_type().is_symlink()
            || !generations_after.is_dir()
            || roster_file_identity(&root_after) != root_identity
            || roster_file_identity(&generations_after) != generations_identity
        {
            return Err(CommitRosterJournalError::InvalidStorage {
                path,
                reason: "commit-roster directory identity changed while loading",
            });
        }
        let persisted = Self::decode_canonical_payload(&generation_path, &bytes)?;
        let read_path = generation_path;

        let PersistedCommitRosters {
            version: _,
            stake_snapshots,
            entries,
        } = persisted;
        let persisted_entry_count = entries.len();

        let mut decoded_entries = BTreeMap::new();
        for entry in entries {
            let snapshot = Self::validate_record(&read_path, entry, &stake_snapshots)?;
            let key = (
                snapshot.commit_qc.height,
                snapshot.commit_qc.subject_block_hash,
            );
            match decoded_entries.entry(key) {
                Entry::Occupied(existing) if existing.get() != &snapshot => {
                    return Err(CommitRosterJournalError::InvalidEntry {
                        path: read_path,
                        height: key.0,
                        reason: "divergent duplicate rows for the same block subject",
                    });
                }
                Entry::Occupied(_) => {}
                Entry::Vacant(entry) => {
                    entry.insert(snapshot);
                }
            }
        }

        journal.entries = decoded_entries;

        // Duplicate rows and retention can make memory differ from disk. In that case the next
        // authorized archival durability boundary also repairs the journal payload.
        journal.dirty = journal.entries.len() != persisted_entry_count;
        journal.enforce_retention();
        Ok(journal)
    }

    fn decode_canonical_payload(
        path: &Path,
        bytes: &[u8],
    ) -> Result<PersistedCommitRosters, CommitRosterJournalError> {
        let persisted: PersistedCommitRosters =
            decode_from_bytes(&bytes).map_err(|source| CommitRosterJournalError::Decode {
                path: path.to_path_buf(),
                source,
            })?;
        if persisted.version != Self::JOURNAL_VERSION {
            return Err(CommitRosterJournalError::UnsupportedVersion {
                path: path.to_path_buf(),
                version: persisted.version,
            });
        }
        let canonical = to_bytes(&persisted).map_err(CommitRosterJournalError::Encode)?;
        if canonical != bytes {
            return Err(CommitRosterJournalError::InvalidStorage {
                path: path.to_path_buf(),
                reason: "generation payload is not canonical Norito",
            });
        }
        Ok(persisted)
    }

    fn read_current_digest(path: &Path) -> Result<String, CommitRosterJournalError> {
        let bytes =
            read_bound_roster_file(path, 65).map_err(|source| CommitRosterJournalError::Read {
                path: path.to_path_buf(),
                source,
            })?;
        let text =
            std::str::from_utf8(&bytes).map_err(|_| CommitRosterJournalError::InvalidStorage {
                path: path.to_path_buf(),
                reason: "current pointer is not UTF-8",
            })?;
        let Some(digest) = text.strip_suffix('\n') else {
            return Err(CommitRosterJournalError::InvalidStorage {
                path: path.to_path_buf(),
                reason: "current pointer is not canonical",
            });
        };
        if digest.len() != 64
            || hex::decode(digest)
                .ok()
                .is_none_or(|bytes| bytes.len() != 32 || hex::encode(bytes) != digest)
        {
            return Err(CommitRosterJournalError::InvalidStorage {
                path: path.to_path_buf(),
                reason: "current pointer is not a lowercase SHA-256 digest",
            });
        }
        Ok(digest.to_owned())
    }

    fn gc_generations(
        &self,
        current_digest: &str,
        previous_digest: Option<&str>,
        root_identity: RosterFileIdentity,
        generations_identity: RosterFileIdentity,
    ) -> Result<(), CommitRosterJournalError> {
        let generations = self.path.join(Self::GENERATIONS_DIR);
        let verify_directories = || -> Result<(), CommitRosterJournalError> {
            verify_roster_directory_identity(&self.path, root_identity).map_err(|source| {
                CommitRosterJournalError::Read {
                    path: self.path.clone(),
                    source,
                }
            })?;
            verify_roster_directory_identity(&generations, generations_identity).map_err(
                |source| CommitRosterJournalError::Read {
                    path: generations.clone(),
                    source,
                },
            )?;
            if Self::read_current_digest(&self.path.join(Self::CURRENT_FILE))? != current_digest {
                return Err(CommitRosterJournalError::InvalidStorage {
                    path: self.path.join(Self::CURRENT_FILE),
                    reason: "current pointer changed before generation GC",
                });
            }
            Ok(())
        };
        verify_directories()?;
        let mut candidates = Vec::new();
        let mut scanned = 0_usize;
        for entry in
            fs::read_dir(&generations).map_err(|source| CommitRosterJournalError::Read {
                path: generations.clone(),
                source,
            })?
        {
            if scanned == Self::MAX_GENERATIONS {
                return Err(CommitRosterJournalError::InvalidStorage {
                    path: generations,
                    reason: "commit-roster generation count exceeds the hard scan bound",
                });
            }
            scanned += 1;
            let entry = entry.map_err(|source| CommitRosterJournalError::Read {
                path: generations.clone(),
                source,
            })?;
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Some(digest) = name.strip_suffix(".norito") else {
                continue;
            };
            if digest == current_digest || previous_digest == Some(digest) {
                continue;
            }
            if digest.len() != 64
                || hex::decode(digest)
                    .ok()
                    .is_none_or(|bytes| bytes.len() != 32 || hex::encode(bytes) != digest)
            {
                continue;
            }
            let path = entry.path();
            let metadata =
                fs::symlink_metadata(&path).map_err(|source| CommitRosterJournalError::Read {
                    path: path.clone(),
                    source,
                })?;
            if metadata.file_type().is_symlink()
                || !metadata.is_file()
                || !roster_file_is_single_link(&metadata)
            {
                continue;
            }
            let bytes = match read_bound_roster_file(&path, Self::MAX_PAYLOAD_BYTES) {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };
            if hex::encode(Sha256::digest(&bytes)) != digest
                || Self::decode_canonical_payload(&path, &bytes).is_err()
            {
                continue;
            }
            let metadata_after =
                fs::symlink_metadata(&path).map_err(|source| CommitRosterJournalError::Read {
                    path: path.clone(),
                    source,
                })?;
            if !roster_file_metadata_unchanged(&metadata, &metadata_after) {
                return Err(CommitRosterJournalError::InvalidStorage {
                    path,
                    reason: "generation identity changed during GC authentication",
                });
            }
            candidates.push((path, roster_file_identity(&metadata), metadata.len()));
        }
        verify_directories()?;
        for (path, identity, len) in candidates {
            verify_directories()?;
            let metadata =
                fs::symlink_metadata(&path).map_err(|source| CommitRosterJournalError::Read {
                    path: path.clone(),
                    source,
                })?;
            if metadata.file_type().is_symlink()
                || !metadata.is_file()
                || !roster_file_is_single_link(&metadata)
                || roster_file_identity(&metadata) != identity
                || metadata.len() != len
            {
                return Err(CommitRosterJournalError::InvalidStorage {
                    path,
                    reason: "generation identity changed before GC",
                });
            }
            fs::remove_file(&path)
                .map_err(|source| CommitRosterJournalError::Write { path, source })?;
        }
        verify_directories()?;
        sync_dir(&generations).map_err(|source| CommitRosterJournalError::NamespaceSync {
            path: generations,
            source,
        })
    }

    fn validate_record(
        path: &Path,
        entry: CommitRosterRecord,
        stake_snapshots: &[CommitStakeSnapshot],
    ) -> Result<CommitRosterSnapshot, CommitRosterJournalError> {
        let entry_height = entry.height;
        let invalid = |reason| CommitRosterJournalError::InvalidEntry {
            path: path.to_path_buf(),
            height: entry_height,
            reason,
        };
        let qc = &entry.commit_qc;
        let checkpoint = &entry.validator_checkpoint;
        if entry.height == 0 {
            return Err(invalid("height is zero"));
        }
        if qc.phase != Phase::Commit {
            return Err(invalid("certificate phase is not Commit"));
        }
        if qc.highest_qc.is_some() {
            return Err(invalid("commit certificate carries a highest-QC reference"));
        }
        if !matches!(qc.mode_tag.as_str(), PERMISSIONED_TAG | NPOS_TAG) {
            return Err(invalid("certificate mode tag is unsupported"));
        }
        if entry.height != qc.height || entry.block_hash != qc.subject_block_hash {
            return Err(invalid("certificate subject does not match row key"));
        }
        if entry.height != checkpoint.height || entry.block_hash != checkpoint.block_hash {
            return Err(invalid("checkpoint subject does not match row key"));
        }
        if qc.validator_set.is_empty() {
            return Err(invalid("validator set is empty"));
        }
        if qc
            .validator_set
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != qc.validator_set.len()
        {
            return Err(invalid("validator set contains duplicate peers"));
        }
        if qc.validator_set_hash_version != VALIDATOR_SET_HASH_VERSION_V1
            || qc.validator_set_hash != HashOf::new(&qc.validator_set)
        {
            return Err(invalid("certificate validator-set commitment is invalid"));
        }
        if checkpoint.view != qc.view
            || checkpoint.validator_set_hash_version != qc.validator_set_hash_version
            || checkpoint.validator_set_hash != qc.validator_set_hash
            || checkpoint.validator_set != qc.validator_set
            || checkpoint.parent_state_root != qc.parent_state_root
            || checkpoint.post_state_root != qc.post_state_root
            || checkpoint.chain_order_hash != qc.chain_order_hash
            || checkpoint.rechain_seq != qc.rechain_seq
            || checkpoint.signers_bitmap != qc.aggregate.signers_bitmap
            || checkpoint.bls_aggregate_signature != qc.aggregate.bls_aggregate_signature
        {
            return Err(invalid(
                "checkpoint does not exactly match the signed certificate subject",
            ));
        }
        if checkpoint.expires_at_height.is_some() {
            return Err(invalid("canonical checkpoint carries an expiry"));
        }
        let expected_bitmap_len = qc.validator_set.len().div_ceil(8);
        if qc.aggregate.signers_bitmap.len() != expected_bitmap_len {
            return Err(invalid("signer bitmap length does not match validator set"));
        }
        if let Some(last) = qc.aggregate.signers_bitmap.last().copied() {
            let used_bits = qc.validator_set.len() % 8;
            if used_bits != 0 && last & !((1_u8 << used_bits) - 1) != 0 {
                return Err(invalid("signer bitmap sets bits outside validator set"));
            }
        }
        let zero_root = Hash::prehashed([0; Hash::LENGTH]);
        let genesis_stub = entry.height == 1
            && qc.view == 0
            && qc.epoch == 0
            && qc.rechain_seq == 0
            && qc.parent_state_root == zero_root
            && qc.post_state_root == zero_root
            && qc.aggregate.bls_aggregate_signature.is_empty()
            && qc.aggregate.signers_bitmap.iter().all(|byte| *byte == 0);
        if entry.height == 1 && !genesis_stub {
            return Err(invalid(
                "height-one certificate is not the canonical unsigned genesis stub",
            ));
        }
        if !genesis_stub && qc.aggregate.bls_aggregate_signature.is_empty() {
            return Err(invalid("non-genesis certificate signature is empty"));
        }
        let stake_snapshot = match (entry.stake_snapshot, entry.stake_snapshot_index) {
            (Some(_), _) => {
                return Err(invalid(
                    "inline stake snapshots are unsupported; use the indexed table",
                ));
            }
            (None, Some(index)) => {
                let index = usize::try_from(index)
                    .map_err(|_| invalid("stake snapshot index exceeds usize"))?;
                Some(
                    stake_snapshots
                        .get(index)
                        .cloned()
                        .ok_or_else(|| invalid("stake snapshot index is out of bounds"))?,
                )
            }
            (None, None) => None,
        };
        if stake_snapshot
            .as_ref()
            .is_some_and(|snapshot| !snapshot.matches_roster(&qc.validator_set))
        {
            return Err(invalid("stake snapshot does not match validator set"));
        }
        if genesis_stub && stake_snapshot.is_some() {
            return Err(invalid(
                "canonical genesis certificate carries a stake snapshot",
            ));
        }
        match qc.mode_tag.as_str() {
            PERMISSIONED_TAG if stake_snapshot.is_some() => {
                return Err(invalid("permissioned certificate carries a stake snapshot"));
            }
            NPOS_TAG if entry.height > 1 && stake_snapshot.is_none() => {
                return Err(invalid(
                    "non-genesis NPoS certificate lacks a stake snapshot",
                ));
            }
            PERMISSIONED_TAG | NPOS_TAG => {}
            _ => unreachable!("unsupported mode rejected before snapshot decoding"),
        }
        Ok(CommitRosterSnapshot {
            commit_qc: entry.commit_qc,
            validator_checkpoint: entry.validator_checkpoint,
            stake_snapshot,
        })
    }

    /// Insert an exact commit-roster tuple without replacing a prepared tuple for the same block.
    ///
    /// Returns `true` when the tuple was inserted or was an exact retry. Returns `false` when the
    /// journal is fenced by unknown storage durability or already contains a different QC,
    /// checkpoint, or stake snapshot for the same `(height, block_hash)` key. The first accepted
    /// tuple remains immutable.
    pub fn upsert(
        &mut self,
        commit_qc: Qc,
        validator_checkpoint: ValidatorSetCheckpoint,
        stake_snapshot: Option<CommitStakeSnapshot>,
    ) -> bool {
        if self.storage_unknown {
            return false;
        }
        let key = (commit_qc.height, commit_qc.subject_block_hash);
        let snapshot = CommitRosterSnapshot {
            commit_qc,
            validator_checkpoint,
            stake_snapshot,
        };
        let accepted = match self.entries.entry(key) {
            Entry::Occupied(entry) => entry.get() == &snapshot,
            Entry::Vacant(entry) => {
                entry.insert(snapshot);
                self.dirty = true;
                true
            }
        };
        self.enforce_retention();
        accepted
    }

    /// Return whether a test journal has changes not yet acknowledged by persistence.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn needs_persistence(&self) -> bool {
        self.dirty
    }

    /// Persist the journal to disk.
    ///
    /// # Errors
    ///
    /// Returns [`CommitRosterJournalError::Write`] when the journal cannot be written,
    /// [`CommitRosterJournalError::Encode`] when encoding fails,
    /// [`CommitRosterJournalError::NamespaceSync`] when a published rename is not acknowledged,
    /// or [`CommitRosterJournalError::StorageUnknown`] after an ambiguous namespace replacement.
    pub fn persist(&mut self) -> Result<(), CommitRosterJournalError> {
        self.persist_durable()
    }

    fn persist_durable(&mut self) -> Result<(), CommitRosterJournalError> {
        let _publication_guard = COMMIT_ROSTER_PUBLICATION_LOCK.lock();
        if self.storage_unknown {
            return Err(CommitRosterJournalError::StorageUnknown {
                path: self.path.clone(),
            });
        }
        if self.path.as_os_str().is_empty() {
            self.dirty = false;
            return Ok(());
        }
        // Ensure persisted payload honours the configured retention window.
        self.enforce_retention();
        let mut stake_snapshots = Vec::new();
        let mut entries = Vec::with_capacity(self.entries.len());
        for ((height, block_hash), snapshot) in &self.entries {
            let stake_snapshot_index = if let Some(stake) = snapshot.stake_snapshot.as_ref() {
                let position = stake_snapshots
                    .iter()
                    .position(|existing| existing == stake)
                    .unwrap_or_else(|| {
                        stake_snapshots.push(stake.clone());
                        stake_snapshots.len() - 1
                    });
                Some(u32::try_from(position).map_err(|_| {
                    CommitRosterJournalError::InvalidStorage {
                        path: self.path.clone(),
                        reason: "stake snapshot table exceeds the canonical u32 index space",
                    }
                })?)
            } else {
                None
            };
            entries.push(CommitRosterRecord {
                height: *height,
                block_hash: *block_hash,
                commit_qc: snapshot.commit_qc.clone(),
                validator_checkpoint: snapshot.validator_checkpoint.clone(),
                stake_snapshot_index,
                stake_snapshot: None,
            });
        }
        let payload = PersistedCommitRosters {
            version: Self::JOURNAL_VERSION,
            stake_snapshots,
            entries,
        };
        let bytes = to_bytes(&payload).map_err(CommitRosterJournalError::Encode)?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > Self::MAX_PAYLOAD_BYTES {
            return Err(CommitRosterJournalError::InvalidStorage {
                path: self.path.clone(),
                reason: "canonical payload exceeds the commit-roster hard size bound",
            });
        }
        let legacy = self
            .path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(Self::LEGACY_JOURNAL_FILE);
        if fs::symlink_metadata(&legacy).is_ok() {
            return Err(CommitRosterJournalError::InvalidStorage {
                path: legacy,
                reason: "legacy mutable commit-roster journals are unsupported",
            });
        }
        fs::create_dir_all(&self.path).map_err(|source| CommitRosterJournalError::Write {
            path: self.path.clone(),
            source,
        })?;
        let root_metadata =
            fs::symlink_metadata(&self.path).map_err(|source| CommitRosterJournalError::Read {
                path: self.path.clone(),
                source,
            })?;
        if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
            return Err(CommitRosterJournalError::InvalidStorage {
                path: self.path.clone(),
                reason: "commit-roster root must be a direct directory",
            });
        }
        let root_identity = direct_roster_directory_identity(&self.path).map_err(|source| {
            CommitRosterJournalError::Read {
                path: self.path.clone(),
                source,
            }
        })?;
        let generations = self.path.join(Self::GENERATIONS_DIR);
        fs::create_dir_all(&generations).map_err(|source| CommitRosterJournalError::Write {
            path: generations.clone(),
            source,
        })?;
        let generations_metadata = fs::symlink_metadata(&generations).map_err(|source| {
            CommitRosterJournalError::Read {
                path: generations.clone(),
                source,
            }
        })?;
        if generations_metadata.file_type().is_symlink() || !generations_metadata.is_dir() {
            return Err(CommitRosterJournalError::InvalidStorage {
                path: generations,
                reason: "commit-roster generations must be a direct directory",
            });
        }
        let generations_identity =
            direct_roster_directory_identity(&generations).map_err(|source| {
                CommitRosterJournalError::Read {
                    path: generations.clone(),
                    source,
                }
            })?;
        let mut generation_count = 0_usize;
        for entry in
            fs::read_dir(&generations).map_err(|source| CommitRosterJournalError::Read {
                path: generations.clone(),
                source,
            })?
        {
            let _ = entry.map_err(|source| CommitRosterJournalError::Read {
                path: generations.clone(),
                source,
            })?;
            generation_count = generation_count.saturating_add(1);
            if generation_count > Self::MAX_GENERATIONS {
                return Err(CommitRosterJournalError::InvalidStorage {
                    path: generations.clone(),
                    reason: "commit-roster generation count exceeds the hard scan bound",
                });
            }
        }
        let digest = hex::encode(Sha256::digest(&bytes));
        let generation_path = generations.join(format!("{digest}.norito"));
        if generation_count == Self::MAX_GENERATIONS {
            match fs::symlink_metadata(&generation_path) {
                Ok(_) => {}
                Err(source) if source.kind() == io::ErrorKind::NotFound => {
                    return Err(CommitRosterJournalError::InvalidStorage {
                        path: generations,
                        reason: "commit-roster generation count is at the hard publication bound",
                    });
                }
                Err(source) => {
                    return Err(CommitRosterJournalError::Read {
                        path: generation_path,
                        source,
                    });
                }
            }
        }
        match fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&generation_path)
        {
            Ok(mut file) => {
                file.write_all(&bytes)
                    .and_then(|()| file.flush())
                    .and_then(|()| file.sync_all())
                    .map_err(|source| CommitRosterJournalError::Write {
                        path: generation_path.clone(),
                        source,
                    })?;
            }
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {
                let existing = read_bound_roster_file(&generation_path, Self::MAX_PAYLOAD_BYTES)
                    .map_err(|source| CommitRosterJournalError::Read {
                        path: generation_path.clone(),
                        source,
                    })?;
                if existing != bytes {
                    return Err(CommitRosterJournalError::InvalidStorage {
                        path: generation_path,
                        reason: "content-addressed generation conflicts with its digest",
                    });
                }
            }
            Err(source) => {
                return Err(CommitRosterJournalError::Write {
                    path: generation_path,
                    source,
                });
            }
        }
        verify_roster_directory_identity(&self.path, root_identity).map_err(|source| {
            CommitRosterJournalError::Read {
                path: self.path.clone(),
                source,
            }
        })?;
        verify_roster_directory_identity(&generations, generations_identity).map_err(|source| {
            CommitRosterJournalError::Read {
                path: generations.clone(),
                source,
            }
        })?;
        let generation_readback = read_bound_roster_file(&generation_path, Self::MAX_PAYLOAD_BYTES)
            .map_err(|source| CommitRosterJournalError::Read {
                path: generation_path.clone(),
                source,
            })?;
        if generation_readback != bytes {
            return Err(CommitRosterJournalError::InvalidStorage {
                path: generation_path,
                reason: "published generation differs from the canonical payload",
            });
        }
        sync_dir(&generations).map_err(|source| CommitRosterJournalError::NamespaceSync {
            path: generations.clone(),
            source,
        })?;

        let current_path = self.path.join(Self::CURRENT_FILE);
        let previous_digest = match fs::symlink_metadata(&current_path) {
            Ok(_) => Some(Self::read_current_digest(&current_path)?),
            Err(source) if source.kind() == io::ErrorKind::NotFound => None,
            Err(source) => {
                return Err(CommitRosterJournalError::Read {
                    path: current_path.clone(),
                    source,
                });
            }
        };
        let pointer_bytes = format!("{digest}\n");
        let mut pointer_file = tempfile::Builder::new()
            .prefix(".commit-roster-current-")
            .tempfile_in(&self.path)
            .map_err(|source| CommitRosterJournalError::Write {
                path: self.path.clone(),
                source,
            })?;
        verify_roster_directory_identity(&self.path, root_identity).map_err(|source| {
            CommitRosterJournalError::Read {
                path: self.path.clone(),
                source,
            }
        })?;
        pointer_file
            .as_file_mut()
            .write_all(pointer_bytes.as_bytes())
            .and_then(|()| pointer_file.as_file_mut().flush())
            .and_then(|()| pointer_file.as_file().sync_all())
            .map_err(|source| CommitRosterJournalError::Write {
                path: pointer_file.path().to_path_buf(),
                source,
            })?;
        let pointer_metadata =
            pointer_file
                .as_file()
                .metadata()
                .map_err(|source| CommitRosterJournalError::Read {
                    path: pointer_file.path().to_path_buf(),
                    source,
                })?;
        let pointer_readback =
            read_bound_roster_file(pointer_file.path(), 65).map_err(|source| {
                CommitRosterJournalError::Read {
                    path: pointer_file.path().to_path_buf(),
                    source,
                }
            })?;
        if pointer_readback != pointer_bytes.as_bytes() {
            return Err(CommitRosterJournalError::InvalidStorage {
                path: pointer_file.path().to_path_buf(),
                reason: "synced current-pointer stage differs from the intended digest",
            });
        }
        verify_roster_directory_identity(&self.path, root_identity).map_err(|source| {
            CommitRosterJournalError::Read {
                path: self.path.clone(),
                source,
            }
        })?;
        #[cfg(test)]
        if self.fail_pointer_persist_once {
            self.fail_pointer_persist_once = false;
            self.storage_unknown = true;
            return Err(CommitRosterJournalError::Write {
                path: current_path.clone(),
                source: io::Error::other("injected atomic pointer-persist boundary failure"),
            });
        }
        let persisted_pointer = match pointer_file.persist(&current_path) {
            Ok(pointer) => pointer,
            Err(error) => {
                // `persist` is intended to fail before replacement on supported platforms, but
                // the journal must not infer durable namespace state from an OS rename error.
                // Fence every failure at this commit boundary until restart and exact readback.
                self.storage_unknown = true;
                return Err(CommitRosterJournalError::Write {
                    path: current_path.clone(),
                    source: error.error,
                });
            }
        };
        if let Err(source) = persisted_pointer.sync_all() {
            self.storage_unknown = true;
            return Err(CommitRosterJournalError::NamespaceSync {
                path: current_path.clone(),
                source,
            });
        }
        let persisted_metadata = match persisted_pointer.metadata() {
            Ok(metadata) => metadata,
            Err(source) => {
                self.storage_unknown = true;
                return Err(CommitRosterJournalError::Read {
                    path: current_path.clone(),
                    source,
                });
            }
        };
        let path_metadata = match fs::symlink_metadata(&current_path) {
            Ok(metadata) => metadata,
            Err(source) => {
                self.storage_unknown = true;
                return Err(CommitRosterJournalError::Read {
                    path: current_path.clone(),
                    source,
                });
            }
        };
        if !roster_file_same_object(&pointer_metadata, &persisted_metadata)
            || !roster_file_metadata_unchanged(&persisted_metadata, &path_metadata)
            || persisted_metadata.len() != u64::try_from(pointer_bytes.len()).unwrap_or(u64::MAX)
        {
            self.storage_unknown = true;
            return Err(CommitRosterJournalError::InvalidStorage {
                path: current_path.clone(),
                reason: "current pointer changed during atomic publication",
            });
        }
        #[cfg(test)]
        if self.fail_after_rename_once {
            self.fail_after_rename_once = false;
            self.storage_unknown = true;
            return Err(CommitRosterJournalError::NamespaceSync {
                path: self.path.clone(),
                source: io::Error::other("injected post-rename commit roster journal failure"),
            });
        }
        if let Err(source) = sync_dir(&self.path) {
            self.storage_unknown = true;
            return Err(CommitRosterJournalError::NamespaceSync {
                path: self.path.clone(),
                source,
            });
        }
        let pointer_matches = matches!(
            Self::read_current_digest(&current_path),
            Ok(ref current_digest) if current_digest == &digest
        );
        let generation_matches = matches!(
            read_bound_roster_file(&generation_path, Self::MAX_PAYLOAD_BYTES),
            Ok(ref current_bytes) if current_bytes == &bytes
        );
        if verify_roster_directory_identity(&self.path, root_identity).is_err()
            || verify_roster_directory_identity(&generations, generations_identity).is_err()
            || !pointer_matches
            || !generation_matches
        {
            self.storage_unknown = true;
            return Err(CommitRosterJournalError::InvalidStorage {
                path: self.path.clone(),
                reason: "commit-roster publication changed before durable readback",
            });
        }
        #[cfg(test)]
        if self.replace_current_before_gc_once {
            self.replace_current_before_gc_once = false;
            fs::write(&current_path, format!("{}\n", "0".repeat(64))).map_err(|source| {
                CommitRosterJournalError::Write {
                    path: current_path.clone(),
                    source,
                }
            })?;
        }
        // An exact retry observes the just-published digest as both current and
        // previous. The first publication already reduced the directory to the
        // current and its distinct predecessor; running GC again with two
        // identical protected digests would incorrectly delete that predecessor.
        if previous_digest.as_deref() != Some(digest.as_str()) {
            if let Err(error) = self.gc_generations(
                &digest,
                previous_digest.as_deref(),
                root_identity,
                generations_identity,
            ) {
                match error {
                    CommitRosterJournalError::Write { .. }
                    | CommitRosterJournalError::NamespaceSync { .. } => {
                        warn!(?error, path = %self.path.display(), "commit-roster generation GC deferred");
                    }
                    integrity_error => {
                        self.storage_unknown = true;
                        return Err(integrity_error);
                    }
                }
            }
        }
        self.dirty = false;
        Ok(())
    }

    /// Drop entries above `height` and persist the updated journal.
    ///
    /// # Errors
    ///
    /// Returns [`CommitRosterJournalError::Write`], [`CommitRosterJournalError::Encode`],
    /// [`CommitRosterJournalError::NamespaceSync`], or
    /// [`CommitRosterJournalError::StorageUnknown`] when persistence fails.
    pub fn truncate_to_height(&mut self, height: u64) -> Result<(), CommitRosterJournalError> {
        let before = self.entries.len();
        self.entries
            .retain(|(entry_height, _), _| *entry_height <= height);
        if self.entries.len() == before {
            return Ok(());
        }
        self.dirty = true;
        self.persist()
    }

    /// Retrieve the structurally validated archival projection for `height`/`block_hash`.
    ///
    /// The returned value is not cryptographic finality authority.
    #[must_use]
    pub fn get(
        &self,
        height: u64,
        block_hash: HashOf<BlockHeader>,
    ) -> Option<CommitRosterSnapshot> {
        self.entries.get(&(height, block_hash)).cloned()
    }

    /// Return whether a test-only archival snapshot satisfies the same structural invariants as a
    /// decoded journal entry.
    ///
    /// This is used before promoting independently persisted recovery metadata into the
    /// first-tuple-wins in-memory journal.
    #[must_use]
    #[cfg(test)]
    pub(crate) fn snapshot_is_canonical(snapshot: &CommitRosterSnapshot) -> bool {
        let stake_snapshots = snapshot
            .stake_snapshot
            .clone()
            .into_iter()
            .collect::<Vec<_>>();
        let record = CommitRosterRecord {
            height: snapshot.commit_qc.height,
            block_hash: snapshot.commit_qc.subject_block_hash,
            commit_qc: snapshot.commit_qc.clone(),
            validator_checkpoint: snapshot.validator_checkpoint.clone(),
            stake_snapshot_index: (!stake_snapshots.is_empty()).then_some(0),
            stake_snapshot: None,
        };
        Self::validate_record(Path::new("commit-roster-sidecar"), record, &stake_snapshots).is_ok()
    }

    /// Re-open a test journal and require an exact tuple match.
    ///
    /// An empty path is reserved for in-memory unit-test journals, where no durable artifact
    /// exists to re-open. Every production path is decoded from disk so stale in-memory state can
    /// never satisfy a pre-Kura recovery-fence readback after deletion or corruption.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn durable_entry_matches_exact(
        &self,
        commit_qc: &Qc,
        checkpoint: &ValidatorSetCheckpoint,
        stake_snapshot: Option<&CommitStakeSnapshot>,
    ) -> bool {
        if self.storage_unknown {
            return false;
        }
        let snapshot = if self.path.as_os_str().is_empty() {
            self.get(commit_qc.height, commit_qc.subject_block_hash)
        } else {
            let durable = match Self::load(self.path.clone(), self.retention) {
                Ok(journal) => journal,
                Err(err) => {
                    warn!(
                        ?err,
                        path = %self.path.display(),
                        height = commit_qc.height,
                        block = %commit_qc.subject_block_hash,
                        "commit roster durable exact readback failed"
                    );
                    return false;
                }
            };
            durable.get(commit_qc.height, commit_qc.subject_block_hash)
        };
        snapshot.is_some_and(|snapshot| {
            snapshot.commit_qc == *commit_qc
                && snapshot.validator_checkpoint == *checkpoint
                && snapshot.stake_snapshot.as_ref() == stake_snapshot
        })
    }

    /// Return all stored snapshots in height/hash order.
    #[must_use]
    pub fn snapshots(&self) -> Vec<CommitRosterSnapshot> {
        self.entries.values().cloned().collect()
    }

    /// Return whether the durable journal state contains any entry above `height`.
    #[must_use]
    pub(crate) fn has_entries_above(&self, height: u64) -> bool {
        self.entries
            .keys()
            .next_back()
            .is_some_and(|(entry_height, _)| *entry_height > height)
    }

    #[cfg(test)]
    pub(crate) fn empty_payload_bytes_for_version(version: u32) -> Vec<u8> {
        to_bytes(&PersistedCommitRosters {
            version,
            stake_snapshots: Vec::new(),
            entries: Vec::new(),
        })
        .expect("encode empty commit-roster test payload")
    }

    fn enforce_retention(&mut self) {
        // The configured window counts recent non-genesis *heights*, not rows. Every row at a
        // retained height must survive until restart authentication can detect independently valid
        // conflicting QCs. The canonical unsigned genesis stub is a permanent restart anchor and
        // one additional height is reserved for the authenticated pre-Kura successor.
        let retained_non_genesis_heights = self
            .retention
            .get()
            .saturating_add(Self::AUTHENTICATED_PRE_KURA_SUCCESSOR_RESERVE);
        let mut non_genesis_heights = self
            .entries
            .keys()
            .filter(|(height, _)| *height != 1)
            .map(|(height, _)| *height)
            .collect::<Vec<_>>();
        non_genesis_heights.dedup();
        let excess_heights = non_genesis_heights
            .len()
            .saturating_sub(retained_non_genesis_heights);
        if excess_heights == 0 {
            return;
        }
        let first_retained_height = non_genesis_heights[excess_heights];
        let before = self.entries.len();
        self.entries
            .retain(|(height, _), _| *height == 1 || *height >= first_retained_height);
        self.dirty |= self.entries.len() != before;
    }
}

#[cfg(unix)]
type RosterFileIdentity = (u64, u64);
#[cfg(windows)]
type RosterFileIdentity = (Option<u32>, Option<u64>);
#[cfg(not(any(unix, windows)))]
type RosterFileIdentity = ();

#[cfg(unix)]
fn roster_file_identity(metadata: &fs::Metadata) -> RosterFileIdentity {
    use std::os::unix::fs::MetadataExt;
    (metadata.dev(), metadata.ino())
}

#[cfg(windows)]
fn roster_file_identity(metadata: &fs::Metadata) -> RosterFileIdentity {
    use std::os::windows::fs::MetadataExt;
    (metadata.volume_serial_number(), metadata.file_index())
}

#[cfg(not(any(unix, windows)))]
fn roster_file_identity(_metadata: &fs::Metadata) -> RosterFileIdentity {}

#[cfg(unix)]
const fn roster_file_identity_available(_identity: RosterFileIdentity) -> bool {
    true
}

#[cfg(windows)]
const fn roster_file_identity_available(identity: RosterFileIdentity) -> bool {
    identity.0.is_some() && identity.1.is_some()
}

#[cfg(not(any(unix, windows)))]
const fn roster_file_identity_available(_identity: RosterFileIdentity) -> bool {
    false
}

fn roster_file_is_single_link(metadata: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        metadata.nlink() == 1
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.number_of_links() == Some(1)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = metadata;
        false
    }
}

fn direct_roster_directory_identity(path: &Path) -> io::Result<RosterFileIdentity> {
    let metadata = fs::symlink_metadata(path)?;
    let identity = roster_file_identity(&metadata);
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || !roster_file_identity_available(identity)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "commit-roster directory must be direct and have a stable filesystem identity",
        ));
    }
    Ok(identity)
}

fn verify_roster_directory_identity(path: &Path, expected: RosterFileIdentity) -> io::Result<()> {
    if direct_roster_directory_identity(path)? != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "commit-roster directory identity changed",
        ));
    }
    Ok(())
}

fn roster_file_same_object(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    let identity = roster_file_identity(left);
    roster_file_identity_available(identity)
        && identity == roster_file_identity(right)
        && roster_file_is_single_link(left)
        && roster_file_is_single_link(right)
        && left.len() == right.len()
}

#[cfg(unix)]
fn roster_file_metadata_unchanged(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    roster_file_identity(left) == roster_file_identity(right)
        && left.nlink() == 1
        && right.nlink() == 1
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(windows)]
fn roster_file_metadata_unchanged(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    roster_file_identity_available(roster_file_identity(left))
        && roster_file_identity(left) == roster_file_identity(right)
        && left.number_of_links() == Some(1)
        && right.number_of_links() == Some(1)
        && left.file_size() == right.file_size()
        && left.last_write_time() == right.last_write_time()
        && left.creation_time() == right.creation_time()
}

#[cfg(not(any(unix, windows)))]
fn roster_file_metadata_unchanged(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    false
}

fn read_bound_roster_file(path: &Path, max_bytes: u64) -> io::Result<Vec<u8>> {
    let path_before = fs::symlink_metadata(path)?;
    if path_before.file_type().is_symlink()
        || !path_before.is_file()
        || !roster_file_is_single_link(&path_before)
        || path_before.len() > max_bytes
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "commit-roster artifact must be a bounded direct single-link regular file",
        ));
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let mut file = options.open(path)?;
    let opened_before = file.metadata()?;
    if !roster_file_identity_available(roster_file_identity(&path_before))
        || !roster_file_metadata_unchanged(&path_before, &opened_before)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "commit-roster artifact identity changed while opening",
        ));
    }
    let mut bytes = Vec::with_capacity(usize::try_from(opened_before.len()).unwrap_or(0));
    Read::by_ref(&mut file)
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    let opened_after = file.metadata()?;
    let path_after = fs::symlink_metadata(path)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes
        || path_after.file_type().is_symlink()
        || !path_after.is_file()
        || !roster_file_is_single_link(&path_after)
        || !roster_file_metadata_unchanged(&opened_before, &opened_after)
        || !roster_file_metadata_unchanged(&opened_before, &path_after)
        || opened_after.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "commit-roster artifact changed while reading",
        ));
    }
    Ok(bytes)
}

fn sync_dir(path: &Path) -> std::io::Result<()> {
    let file = fs::File::open(path)?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use std::{num::NonZeroU64, path::Path};

    use iroha_crypto::{Algorithm, HashOf, KeyPair};
    use iroha_data_model::{
        block::BlockHeader, consensus::VALIDATOR_SET_HASH_VERSION_V1, peer::PeerId,
    };
    use iroha_primitives::numeric::Quantity;
    use tempfile::tempdir;

    use super::*;
    use crate::sumeragi::{
        consensus::{NPOS_TAG, PERMISSIONED_TAG, Phase, QcAggregate},
        stake_snapshot::CommitStakeSnapshotEntry,
    };

    fn sample_cert(view: u64) -> (Qc, ValidatorSetCheckpoint) {
        cert_with_height(2, view)
    }

    fn checked_random_bls_keypair() -> KeyPair {
        KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
            .expect("generate checked commit roster journal BLS fixture keypair")
    }

    fn cert_with_height(height: u64, view: u64) -> (Qc, ValidatorSetCheckpoint) {
        let kp = checked_random_bls_keypair();
        let peer = PeerId::new(kp.public_key().clone());
        cert_with_height_and_roster(height, view, vec![peer])
    }

    fn cert_with_height_and_roster(
        height: u64,
        view: u64,
        roster: Vec<PeerId>,
    ) -> (Qc, ValidatorSetCheckpoint) {
        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("non-zero"),
            None,
            None,
            None,
            0,
            0,
        );
        let block_hash = header.hash();
        let signers_bitmap = vec![0b0000_0001];
        let bls_aggregate_signature = vec![0xAB; 96];
        let cert = Qc {
            phase: Phase::Commit,
            subject_block_hash: block_hash,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height,
            view,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            mode_tag: PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&roster),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set: roster.clone(),
            aggregate: QcAggregate {
                signers_bitmap: signers_bitmap.clone(),
                bls_aggregate_signature: bls_aggregate_signature.clone(),
            },
        };
        let checkpoint = ValidatorSetCheckpoint::new(
            height,
            view,
            block_hash,
            cert.parent_state_root,
            cert.post_state_root,
            roster,
            signers_bitmap,
            bls_aggregate_signature,
            VALIDATOR_SET_HASH_VERSION_V1,
            None,
        );
        (cert, checkpoint)
    }

    fn sample_stake_snapshot(roster: &[PeerId]) -> CommitStakeSnapshot {
        CommitStakeSnapshot {
            validator_set_hash: HashOf::new(&roster.to_vec()),
            entries: roster
                .iter()
                .map(|peer| CommitStakeSnapshotEntry {
                    peer_id: peer.clone(),
                    stake: Quantity::from(10_u32),
                })
                .collect(),
        }
    }

    fn retention(limit: usize) -> NonZeroUsize {
        NonZeroUsize::new(limit).expect("non-zero retention")
    }

    fn write_test_generation(path: &Path, bytes: &[u8]) -> PathBuf {
        let generations = path.join(CommitRosterJournal::GENERATIONS_DIR);
        std::fs::create_dir_all(&generations).expect("create generation directory");
        let digest = hex::encode(Sha256::digest(bytes));
        let generation = generations.join(format!("{digest}.norito"));
        std::fs::write(&generation, bytes).expect("write generation payload");
        std::fs::write(
            path.join(CommitRosterJournal::CURRENT_FILE),
            format!("{digest}\n"),
        )
        .expect("write current pointer");
        generation
    }

    fn read_test_generation(path: &Path) -> Vec<u8> {
        let digest =
            CommitRosterJournal::read_current_digest(&path.join(CommitRosterJournal::CURRENT_FILE))
                .expect("read current digest");
        std::fs::read(
            path.join(CommitRosterJournal::GENERATIONS_DIR)
                .join(format!("{digest}.norito")),
        )
        .expect("read generation payload")
    }

    #[test]
    fn canonical_snapshot_validation_rejects_signed_subject_mismatch() {
        let (commit_qc, validator_checkpoint) = sample_cert(1);
        let snapshot = CommitRosterSnapshot {
            commit_qc,
            validator_checkpoint,
            stake_snapshot: None,
        };
        assert!(CommitRosterJournal::snapshot_is_canonical(&snapshot));

        let mut mismatched = snapshot;
        mismatched.validator_checkpoint.post_state_root =
            iroha_crypto::Hash::new(b"mismatched post-state root");
        assert!(!CommitRosterJournal::snapshot_is_canonical(&mismatched));
    }

    #[test]
    fn canonical_snapshot_validation_accepts_indexed_npos_and_rejects_roster_mismatch() {
        let kp = checked_random_bls_keypair();
        let roster = vec![PeerId::new(kp.public_key().clone())];
        let (mut commit_qc, validator_checkpoint) =
            cert_with_height_and_roster(2, 0, roster.clone());
        commit_qc.mode_tag = NPOS_TAG.to_owned();
        let snapshot = CommitRosterSnapshot {
            commit_qc,
            validator_checkpoint,
            stake_snapshot: Some(sample_stake_snapshot(&roster)),
        };
        assert!(CommitRosterJournal::snapshot_is_canonical(&snapshot));

        let other = PeerId::new(checked_random_bls_keypair().public_key().clone());
        let mut mismatched = snapshot;
        mismatched.stake_snapshot = Some(sample_stake_snapshot(&[other]));
        assert!(!CommitRosterJournal::snapshot_is_canonical(&mismatched));
    }

    #[test]
    fn storage_unknown_fence_rejects_use_until_reload() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (cert, checkpoint) = sample_cert(1);
        let mut journal = CommitRosterJournal::new(path.clone(), retention(4));

        journal.mark_storage_unknown();

        assert!(journal.storage_is_unknown());
        assert!(!journal.upsert(cert.clone(), checkpoint.clone(), None));
        assert!(matches!(
            journal.persist(),
            Err(CommitRosterJournalError::StorageUnknown { .. })
        ));
        assert!(!journal.durable_entry_matches_exact(&cert, &checkpoint, None));

        let reloaded = CommitRosterJournal::load(path, retention(4)).expect("restart reload");
        assert!(
            !reloaded.storage_is_unknown(),
            "only reconstruction from the resolved post-crash namespace clears the process fence"
        );
    }

    #[test]
    fn journal_roundtrips_entries() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (cert, checkpoint) = sample_cert(1);
        let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
        journal.upsert(cert.clone(), checkpoint.clone(), None);
        journal.persist().expect("persist");

        let loaded = CommitRosterJournal::load(path, retention(4)).expect("load");
        let snapshots = loaded.snapshots();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(
            snapshots[0],
            CommitRosterSnapshot {
                commit_qc: cert,
                validator_checkpoint: checkpoint,
                stake_snapshot: None,
            }
        );
    }

    #[test]
    fn post_publication_namespace_failure_fences_process_until_reload() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (cert, checkpoint) = sample_cert(1);
        let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
        assert!(journal.upsert(cert.clone(), checkpoint.clone(), None));
        journal.fail_after_rename_once_for_tests();

        assert!(matches!(
            journal.persist(),
            Err(CommitRosterJournalError::NamespaceSync { .. })
        ));
        assert!(journal.storage_is_unknown());
        assert!(matches!(
            journal.persist(),
            Err(CommitRosterJournalError::StorageUnknown { .. })
        ));

        let reloaded = CommitRosterJournal::load(path, retention(4)).expect("restart reload");
        assert_eq!(
            reloaded.get(cert.height, cert.subject_block_hash),
            Some(CommitRosterSnapshot {
                commit_qc: cert,
                validator_checkpoint: checkpoint,
                stake_snapshot: None,
            })
        );
    }

    #[test]
    fn atomic_pointer_persist_error_fences_process_until_reload() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (cert, checkpoint) = sample_cert(1);
        let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
        assert!(journal.upsert(cert, checkpoint, None));
        journal.fail_pointer_persist_once_for_tests();

        assert!(matches!(
            journal.persist(),
            Err(CommitRosterJournalError::Write { .. })
        ));
        assert!(journal.storage_is_unknown());
        assert!(matches!(
            journal.persist(),
            Err(CommitRosterJournalError::StorageUnknown { .. })
        ));
        let reloaded = CommitRosterJournal::load(path, retention(4))
            .expect("unpublished generation is ignored on restart");
        assert!(reloaded.snapshots().is_empty());
        assert!(!reloaded.storage_is_unknown());
    }

    #[test]
    fn current_pointer_substitution_before_gc_fails_and_fences_process() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (cert, checkpoint) = sample_cert(1);
        let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
        assert!(journal.upsert(cert, checkpoint, None));
        journal.replace_current_before_gc_once_for_tests();

        let error = journal
            .persist()
            .expect_err("post-publication pointer substitution must fail");
        assert!(matches!(
            error,
            CommitRosterJournalError::InvalidStorage {
                reason: "current pointer changed before generation GC",
                ..
            }
        ));
        assert!(journal.storage_is_unknown());
        assert!(matches!(
            journal.persist(),
            Err(CommitRosterJournalError::StorageUnknown { .. })
        ));
        assert!(
            CommitRosterJournal::load(path, retention(4)).is_err(),
            "restart must reject the substituted pointer without its exact generation"
        );
    }

    #[test]
    fn load_rejects_digest_mismatch_and_malformed_pointer() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (cert, checkpoint) = sample_cert(1);
        let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
        assert!(journal.upsert(cert, checkpoint, None));
        journal.persist().expect("persist");

        let digest =
            CommitRosterJournal::read_current_digest(&path.join(CommitRosterJournal::CURRENT_FILE))
                .expect("current digest");
        let generation = path
            .join(CommitRosterJournal::GENERATIONS_DIR)
            .join(format!("{digest}.norito"));
        std::fs::write(&generation, b"same name, different bytes").expect("corrupt generation");
        let error = CommitRosterJournal::load(path.clone(), retention(4))
            .expect_err("digest mismatch must fail closed");
        assert!(matches!(
            error,
            CommitRosterJournalError::InvalidStorage { .. }
        ));

        std::fs::write(
            path.join(CommitRosterJournal::CURRENT_FILE),
            digest.to_uppercase(),
        )
        .expect("write malformed pointer");
        let error = CommitRosterJournal::load(path, retention(4))
            .expect_err("noncanonical pointer must fail closed");
        assert!(matches!(
            error,
            CommitRosterJournalError::InvalidStorage { .. }
        ));
    }

    #[test]
    fn load_ignores_unpublished_generation_after_pointer_loss_without_mutating_disk() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (cert, checkpoint) = sample_cert(1);
        let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
        assert!(journal.upsert(cert, checkpoint, None));
        journal.persist().expect("persist");
        let generations_before = std::fs::read_dir(path.join(CommitRosterJournal::GENERATIONS_DIR))
            .expect("read generations")
            .map(|entry| entry.expect("generation entry").file_name())
            .collect::<Vec<_>>();
        std::fs::remove_file(path.join(CommitRosterJournal::CURRENT_FILE))
            .expect("remove current pointer");

        let loaded = CommitRosterJournal::load(path.clone(), retention(4))
            .expect("unpublished generation is not durable authority");
        assert!(loaded.snapshots().is_empty());
        let generations_after = std::fs::read_dir(path.join(CommitRosterJournal::GENERATIONS_DIR))
            .expect("reread generations")
            .map(|entry| entry.expect("generation entry").file_name())
            .collect::<Vec<_>>();
        assert_eq!(generations_after, generations_before);
        assert!(!path.join(CommitRosterJournal::CURRENT_FILE).exists());
    }

    #[cfg(unix)]
    #[test]
    fn load_rejects_symlink_and_hardlink_artifacts() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (cert, checkpoint) = sample_cert(1);
        let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
        assert!(journal.upsert(cert, checkpoint, None));
        journal.persist().expect("persist");

        let current = path.join(CommitRosterJournal::CURRENT_FILE);
        let direct_pointer = dir.path().join("direct-current");
        std::fs::rename(&current, &direct_pointer).expect("move direct pointer");
        symlink(&direct_pointer, &current).expect("install pointer symlink");
        assert!(CommitRosterJournal::load(path.clone(), retention(4)).is_err());

        std::fs::remove_file(&current).expect("remove pointer symlink");
        std::fs::rename(&direct_pointer, &current).expect("restore direct pointer");
        let digest = CommitRosterJournal::read_current_digest(&current).expect("current digest");
        let generation = path
            .join(CommitRosterJournal::GENERATIONS_DIR)
            .join(format!("{digest}.norito"));
        std::fs::hard_link(&generation, dir.path().join("generation-hardlink"))
            .expect("create generation hardlink");
        assert!(CommitRosterJournal::load(path, retention(4)).is_err());
    }

    #[test]
    fn journal_durable_persist_clears_dirty_state() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (cert, checkpoint) = sample_cert(1);
        let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
        journal.upsert(cert.clone(), checkpoint.clone(), None);
        assert!(journal.needs_persistence());
        journal.persist().expect("persist durable");
        assert!(!journal.needs_persistence());

        let loaded = CommitRosterJournal::load(path, retention(4)).expect("load");
        let snapshot = loaded
            .get(cert.height, cert.subject_block_hash)
            .expect("snapshot must be present");
        assert_eq!(snapshot.commit_qc, cert);
        assert_eq!(snapshot.validator_checkpoint, checkpoint);
    }

    #[test]
    fn journal_truncate_to_height_drops_future_entries() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (cert1, checkpoint1) = cert_with_height(1, 0);
        let (cert2, checkpoint2) = cert_with_height(2, 0);
        let mut journal = CommitRosterJournal::new(path, retention(4));
        journal.upsert(cert1.clone(), checkpoint1, None);
        journal.upsert(cert2.clone(), checkpoint2, None);
        assert!(journal.has_entries_above(1));

        journal.truncate_to_height(1).expect("truncate to height");
        assert!(!journal.has_entries_above(1));

        assert!(
            journal
                .get(cert1.height, cert1.subject_block_hash)
                .is_some()
        );
        assert!(
            journal
                .get(cert2.height, cert2.subject_block_hash)
                .is_none()
        );
    }

    #[test]
    fn journal_persist_overwrites_existing_file() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (cert1, checkpoint1) = cert_with_height(2, 1);
        let (cert2, checkpoint2) = cert_with_height(3, 1);

        let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
        journal.upsert(cert1.clone(), checkpoint1, None);
        journal.persist().expect("persist first journal");

        let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
        journal.upsert(cert2.clone(), checkpoint2.clone(), None);
        journal.persist().expect("persist second journal");

        let loaded = CommitRosterJournal::load(path, retention(4)).expect("load journal");
        assert!(
            loaded.get(cert1.height, cert1.subject_block_hash).is_none(),
            "old entry should be overwritten"
        );
        let snapshot = loaded
            .get(cert2.height, cert2.subject_block_hash)
            .expect("new entry should exist");
        assert_eq!(snapshot.commit_qc, cert2);
        assert_eq!(snapshot.validator_checkpoint, checkpoint2);
    }

    #[test]
    fn journal_ignores_unpublished_legacy_temp_next_to_committed_main() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let tmp_path = path.with_extension("norito.tmp");

        let (cert1, checkpoint1) = cert_with_height(2, 1);
        let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
        journal.upsert(cert1.clone(), checkpoint1.clone(), None);
        journal.persist().expect("persist main journal");

        let (cert2, checkpoint2) = cert_with_height(3, 1);
        let payload = PersistedCommitRosters {
            version: CommitRosterJournal::JOURNAL_VERSION,
            stake_snapshots: Vec::new(),
            entries: vec![
                CommitRosterRecord {
                    height: cert1.height,
                    block_hash: cert1.subject_block_hash,
                    commit_qc: cert1.clone(),
                    validator_checkpoint: checkpoint1.clone(),
                    stake_snapshot_index: None,
                    stake_snapshot: None,
                },
                CommitRosterRecord {
                    height: cert2.height,
                    block_hash: cert2.subject_block_hash,
                    commit_qc: cert2.clone(),
                    validator_checkpoint: checkpoint2.clone(),
                    stake_snapshot_index: None,
                    stake_snapshot: None,
                },
            ],
        };
        let bytes = to_bytes(&payload).expect("encode temp journal");
        std::fs::write(&tmp_path, bytes).expect("write unpublished temp journal");

        let loaded = CommitRosterJournal::load(path.clone(), retention(4)).expect("load journal");
        assert!(loaded.get(cert2.height, cert2.subject_block_hash).is_none());
        assert!(loaded.get(cert1.height, cert1.subject_block_hash).is_some());
        assert!(
            tmp_path.exists(),
            "read-only load must not promote temp state"
        );
    }

    #[test]
    fn journal_preserves_prepared_tuple_against_higher_view_replacement() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (low_view_cert, low_view_checkpoint) = sample_cert(1);
        let (high_view_cert, high_view_checkpoint) = sample_cert(3);
        let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
        assert!(journal.upsert(low_view_cert.clone(), low_view_checkpoint.clone(), None));
        assert!(
            !journal.upsert(high_view_cert, high_view_checkpoint, None),
            "a divergent higher-view tuple must not replace prepared authority"
        );
        journal.persist().expect("persist");

        let loaded = CommitRosterJournal::load(path, retention(4)).expect("load");
        let snapshots = loaded.snapshots();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].commit_qc, low_view_cert);
        assert_eq!(snapshots[0].validator_checkpoint, low_view_checkpoint);
    }

    #[test]
    fn journal_exact_retry_is_idempotent() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (cert, checkpoint) = sample_cert(1);
        let mut journal = CommitRosterJournal::new(path, retention(4));

        assert!(journal.upsert(cert.clone(), checkpoint.clone(), None));
        journal.persist().expect("persist prepared tuple");
        assert!(!journal.needs_persistence());

        assert!(
            journal.upsert(cert.clone(), checkpoint.clone(), None),
            "an exact retry must be accepted"
        );
        assert!(
            !journal.needs_persistence(),
            "upsert must not manufacture a logical change; the durability boundary still rewrites"
        );
        assert_eq!(
            journal.get(cert.height, cert.subject_block_hash),
            Some(CommitRosterSnapshot {
                commit_qc: cert,
                validator_checkpoint: checkpoint,
                stake_snapshot: None,
            })
        );
    }

    #[test]
    fn journal_exact_retry_repersists_deleted_durable_file() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (cert, checkpoint) = sample_cert(1);
        let mut journal = CommitRosterJournal::new(path.clone(), retention(4));

        assert!(journal.upsert(cert.clone(), checkpoint.clone(), None));
        journal.persist().expect("persist prepared tuple");
        assert!(!journal.needs_persistence());
        std::fs::remove_dir_all(&path).expect("delete durable journal");

        assert!(
            journal.upsert(cert.clone(), checkpoint.clone(), None),
            "the exact in-memory retry remains admissible"
        );
        assert!(
            !journal.needs_persistence(),
            "the adversary deletes disk state without changing the in-memory tuple"
        );
        journal
            .persist()
            .expect("an exact retry must rewrite and fsync the durable journal");

        assert!(journal.durable_entry_matches_exact(&cert, &checkpoint, None));
        assert!(
            path.exists(),
            "the exact retry must restore the deleted file"
        );
    }

    #[test]
    fn journal_durable_exact_readback_fails_closed_after_deletion_or_corruption() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (cert, checkpoint) = sample_cert(1);
        let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
        assert!(journal.upsert(cert.clone(), checkpoint.clone(), None));
        journal.persist().expect("persist prepared tuple");
        assert!(journal.durable_entry_matches_exact(&cert, &checkpoint, None));

        std::fs::remove_dir_all(&path).expect("delete durable journal");
        assert!(
            !journal.durable_entry_matches_exact(&cert, &checkpoint, None),
            "stale memory must not hide deletion of the recovery fence"
        );

        std::fs::write(&path, b"corrupted commit roster journal")
            .expect("write corrupt durable journal");
        assert!(
            !journal.durable_entry_matches_exact(&cert, &checkpoint, None),
            "stale memory must not hide corruption of the recovery fence"
        );
    }

    #[test]
    fn journal_rejects_legacy_v1_payload() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (cert, checkpoint) = sample_cert(1);
        let payload = PersistedCommitRosters {
            version: 1,
            stake_snapshots: Vec::new(),
            entries: vec![CommitRosterRecord {
                height: cert.height,
                block_hash: cert.subject_block_hash,
                commit_qc: cert.clone(),
                validator_checkpoint: checkpoint.clone(),
                stake_snapshot_index: None,
                stake_snapshot: None,
            }],
        };
        let bytes = norito::to_bytes(&payload).expect("encode payload");
        write_test_generation(&path, &bytes);

        let err = CommitRosterJournal::load(path, retention(4)).expect_err("reject v1 journal");
        assert!(
            matches!(
                err,
                CommitRosterJournalError::UnsupportedVersion { version: 1, .. }
            ),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn journal_load_rejects_divergent_duplicate_block_subject_rows() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (first_cert, first_checkpoint) = sample_cert(1);
        let (replacement_cert, replacement_checkpoint) = sample_cert(3);
        assert_eq!(
            first_cert.subject_block_hash, replacement_cert.subject_block_hash,
            "fixture must target one block subject"
        );
        let record =
            |commit_qc: Qc, validator_checkpoint: ValidatorSetCheckpoint| CommitRosterRecord {
                height: commit_qc.height,
                block_hash: commit_qc.subject_block_hash,
                commit_qc,
                validator_checkpoint,
                stake_snapshot_index: None,
                stake_snapshot: None,
            };
        let payload = PersistedCommitRosters {
            version: CommitRosterJournal::JOURNAL_VERSION,
            stake_snapshots: Vec::new(),
            entries: vec![
                record(first_cert, first_checkpoint),
                record(replacement_cert, replacement_checkpoint),
            ],
        };
        write_test_generation(&path, &to_bytes(&payload).expect("encode payload"));

        let err = CommitRosterJournal::load(path, retention(4))
            .expect_err("divergent duplicate rows must fail closed");
        assert!(
            matches!(
                err,
                CommitRosterJournalError::InvalidEntry {
                    reason: "divergent duplicate rows for the same block subject",
                    ..
                }
            ),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn journal_load_accepts_exact_duplicate_rows_as_idempotent() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (cert, checkpoint) = sample_cert(1);
        let record = CommitRosterRecord {
            height: cert.height,
            block_hash: cert.subject_block_hash,
            commit_qc: cert.clone(),
            validator_checkpoint: checkpoint.clone(),
            stake_snapshot_index: None,
            stake_snapshot: None,
        };
        let payload = PersistedCommitRosters {
            version: CommitRosterJournal::JOURNAL_VERSION,
            stake_snapshots: Vec::new(),
            entries: vec![record.clone(), record],
        };
        write_test_generation(&path, &to_bytes(&payload).expect("encode payload"));

        let loaded = CommitRosterJournal::load(path, retention(4))
            .expect("exact duplicate rows are idempotent");
        assert_eq!(
            loaded.get(cert.height, cert.subject_block_hash),
            Some(CommitRosterSnapshot {
                commit_qc: cert,
                validator_checkpoint: checkpoint,
                stake_snapshot: None,
            })
        );
        assert!(
            loaded.needs_persistence(),
            "the next durability boundary should canonicalize duplicate rows"
        );
    }

    #[test]
    fn journal_rejects_checkpoint_that_differs_from_qc_subject() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (cert, mut checkpoint) = sample_cert(1);
        checkpoint.post_state_root =
            iroha_crypto::Hash::prehashed([0xA5; iroha_crypto::Hash::LENGTH]);
        let payload = PersistedCommitRosters {
            version: CommitRosterJournal::JOURNAL_VERSION,
            stake_snapshots: Vec::new(),
            entries: vec![CommitRosterRecord {
                height: cert.height,
                block_hash: cert.subject_block_hash,
                commit_qc: cert,
                validator_checkpoint: checkpoint,
                stake_snapshot_index: None,
                stake_snapshot: None,
            }],
        };
        write_test_generation(&path, &to_bytes(&payload).expect("encode payload"));

        let err = CommitRosterJournal::load(path, retention(4))
            .expect_err("reject mismatched signed subject");
        assert!(
            matches!(
                err,
                CommitRosterJournalError::InvalidEntry {
                    reason: "checkpoint does not exactly match the signed certificate subject",
                    ..
                }
            ),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn journal_rejects_row_key_that_differs_from_qc_subject() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (cert, checkpoint) = sample_cert(1);
        let payload = PersistedCommitRosters {
            version: CommitRosterJournal::JOURNAL_VERSION,
            stake_snapshots: Vec::new(),
            entries: vec![CommitRosterRecord {
                height: cert.height.saturating_add(1),
                block_hash: cert.subject_block_hash,
                commit_qc: cert,
                validator_checkpoint: checkpoint,
                stake_snapshot_index: None,
                stake_snapshot: None,
            }],
        };
        write_test_generation(&path, &to_bytes(&payload).expect("encode payload"));

        let err =
            CommitRosterJournal::load(path, retention(4)).expect_err("reject mismatched row key");
        assert!(
            matches!(
                err,
                CommitRosterJournalError::InvalidEntry {
                    reason: "certificate subject does not match row key",
                    ..
                }
            ),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn journal_rejects_noncanonical_height_one_finality_metadata() {
        for case in ["nonzero_root", "nonzero_epoch", "nonzero_rechain", "signed"] {
            let dir = tempdir().expect("tempdir");
            let path = CommitRosterJournal::journal_path(dir.path());
            let (mut cert, mut checkpoint) = cert_with_height(1, 0);
            cert.aggregate.signers_bitmap.fill(0);
            cert.aggregate.bls_aggregate_signature.clear();
            checkpoint.signers_bitmap.fill(0);
            checkpoint.bls_aggregate_signature.clear();
            match case {
                "nonzero_root" => {
                    let root = Hash::prehashed([0xA5; Hash::LENGTH]);
                    cert.parent_state_root = root;
                    checkpoint.parent_state_root = root;
                }
                "nonzero_epoch" => cert.epoch = 1,
                "nonzero_rechain" => {
                    cert.rechain_seq = 1;
                    checkpoint.rechain_seq = 1;
                }
                "signed" => {
                    cert.aggregate.bls_aggregate_signature = vec![0xA5; 96];
                    checkpoint.bls_aggregate_signature = vec![0xA5; 96];
                }
                _ => unreachable!(),
            }
            let payload = PersistedCommitRosters {
                version: CommitRosterJournal::JOURNAL_VERSION,
                stake_snapshots: Vec::new(),
                entries: vec![CommitRosterRecord {
                    height: 1,
                    block_hash: cert.subject_block_hash,
                    commit_qc: cert,
                    validator_checkpoint: checkpoint,
                    stake_snapshot_index: None,
                    stake_snapshot: None,
                }],
            };
            write_test_generation(&path, &to_bytes(&payload).expect("encode payload"));
            let err = CommitRosterJournal::load(path, retention(4))
                .expect_err("noncanonical height-one metadata must fail closed");
            assert!(
                matches!(
                    err,
                    CommitRosterJournalError::InvalidEntry {
                        reason: "height-one certificate is not the canonical unsigned genesis stub",
                        ..
                    }
                ),
                "unexpected {case} error: {err}"
            );
        }
    }

    #[test]
    fn journal_roundtrips_stake_snapshot() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (mut cert, checkpoint) = sample_cert(1);
        cert.mode_tag = NPOS_TAG.to_string();
        let stake_snapshot = sample_stake_snapshot(&cert.validator_set);
        let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
        journal.upsert(
            cert.clone(),
            checkpoint.clone(),
            Some(stake_snapshot.clone()),
        );
        journal.persist().expect("persist");

        let loaded = CommitRosterJournal::load(path, retention(4)).expect("load");
        let snapshots = loaded.snapshots();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].stake_snapshot, Some(stake_snapshot));
    }

    #[test]
    fn journal_rejects_inline_stake_snapshot_representation() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let kp = checked_random_bls_keypair();
        let roster = vec![PeerId::new(kp.public_key().clone())];
        let (mut cert, checkpoint) = cert_with_height_and_roster(2, 0, roster.clone());
        cert.mode_tag = NPOS_TAG.to_owned();
        let payload = PersistedCommitRosters {
            version: CommitRosterJournal::JOURNAL_VERSION,
            stake_snapshots: Vec::new(),
            entries: vec![CommitRosterRecord {
                height: cert.height,
                block_hash: cert.subject_block_hash,
                commit_qc: cert,
                validator_checkpoint: checkpoint,
                stake_snapshot_index: None,
                stake_snapshot: Some(sample_stake_snapshot(&roster)),
            }],
        };
        write_test_generation(&path, &to_bytes(&payload).expect("encode inline fixture"));

        let error = CommitRosterJournal::load(path, retention(4))
            .expect_err("inline stake snapshots must fail closed");
        assert!(matches!(
            error,
            CommitRosterJournalError::InvalidEntry {
                reason: "inline stake snapshots are unsupported; use the indexed table",
                ..
            }
        ));
    }

    #[test]
    fn journal_rejects_non_exact_indexed_stake_snapshots() {
        let roster = (0..3)
            .map(|_| PeerId::new(checked_random_bls_keypair().public_key().clone()))
            .collect::<Vec<_>>();
        let (mut cert, checkpoint) = cert_with_height_and_roster(2, 0, roster.clone());
        cert.mode_tag = NPOS_TAG.to_owned();
        let base = sample_stake_snapshot(&roster);
        let mut reordered = base.clone();
        reordered.entries.swap(0, 1);
        let mut duplicate_inflated = base.clone();
        duplicate_inflated.entries[1] = CommitStakeSnapshotEntry {
            peer_id: roster[0].clone(),
            stake: Quantity::from(1_000_000_u64),
        };
        let mut missing = base.clone();
        missing.entries.pop();
        let mut extra = base.clone();
        extra.entries.push(CommitStakeSnapshotEntry {
            peer_id: PeerId::new(checked_random_bls_keypair().public_key().clone()),
            stake: Quantity::from(1_u64),
        });
        let mut zero = base;
        zero.entries[0].stake = Quantity::zero();

        for malformed in [reordered, duplicate_inflated, missing, extra, zero] {
            let dir = tempdir().expect("tempdir");
            let path = CommitRosterJournal::journal_path(dir.path());
            let payload = PersistedCommitRosters {
                version: CommitRosterJournal::JOURNAL_VERSION,
                stake_snapshots: vec![malformed],
                entries: vec![CommitRosterRecord {
                    height: cert.height,
                    block_hash: cert.subject_block_hash,
                    commit_qc: cert.clone(),
                    validator_checkpoint: checkpoint.clone(),
                    stake_snapshot_index: Some(0),
                    stake_snapshot: None,
                }],
            };
            write_test_generation(
                &path,
                &to_bytes(&payload).expect("encode malformed fixture"),
            );
            let error = CommitRosterJournal::load(path, retention(4))
                .expect_err("non-exact indexed stake snapshot must fail closed");
            assert!(matches!(
                error,
                CommitRosterJournalError::InvalidEntry {
                    reason: "stake snapshot does not match validator set",
                    ..
                }
            ));
        }
    }

    #[test]
    fn journal_deduplicates_persisted_stake_snapshots() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let kp = checked_random_bls_keypair();
        let roster = vec![PeerId::new(kp.public_key().clone())];
        let stake_snapshot = sample_stake_snapshot(&roster);
        let (mut cert1, checkpoint1) = cert_with_height_and_roster(2, 0, roster.clone());
        let (mut cert2, checkpoint2) = cert_with_height_and_roster(3, 0, roster.clone());
        cert1.mode_tag = NPOS_TAG.to_string();
        cert2.mode_tag = NPOS_TAG.to_string();
        let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
        journal.upsert(cert1.clone(), checkpoint1, Some(stake_snapshot.clone()));
        journal.upsert(cert2.clone(), checkpoint2, Some(stake_snapshot.clone()));

        journal.persist().expect("persist");

        let bytes = read_test_generation(&path);
        let payload: PersistedCommitRosters =
            decode_from_bytes(&bytes).expect("decode persisted journal");
        assert_eq!(payload.version, CommitRosterJournal::JOURNAL_VERSION);
        assert_eq!(payload.stake_snapshots, vec![stake_snapshot.clone()]);
        assert!(payload.entries.iter().all(|entry| {
            entry.stake_snapshot.is_none() && entry.stake_snapshot_index == Some(0)
        }));

        let loaded = CommitRosterJournal::load(path, retention(4)).expect("load");
        let snapshots = loaded.snapshots();
        assert_eq!(snapshots.len(), 2);
        assert!(
            snapshots
                .iter()
                .all(|snapshot| snapshot.stake_snapshot == Some(stake_snapshot.clone()))
        );
    }

    #[test]
    fn journal_ignores_unpublished_legacy_temp_when_current_is_missing() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let tmp_path = path.with_extension("norito.tmp");
        let (cert, checkpoint) = sample_cert(1);
        let payload = PersistedCommitRosters {
            version: CommitRosterJournal::JOURNAL_VERSION,
            stake_snapshots: Vec::new(),
            entries: vec![CommitRosterRecord {
                height: cert.height,
                block_hash: cert.subject_block_hash,
                commit_qc: cert.clone(),
                validator_checkpoint: checkpoint.clone(),
                stake_snapshot_index: None,
                stake_snapshot: None,
            }],
        };
        let bytes = norito::to_bytes(&payload).expect("encode payload");
        std::fs::write(&tmp_path, bytes).expect("write temp payload");

        let loaded = CommitRosterJournal::load(path.clone(), retention(4)).expect("load");
        assert!(loaded.snapshots().is_empty());
        assert!(!path.exists(), "read-only load must not create storage");
        assert!(
            tmp_path.exists(),
            "read-only load must leave unpublished temp state untouched"
        );
    }

    #[test]
    fn journal_rejects_corrupt_root_instead_of_promoting_legacy_temp() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let tmp_path = path.with_extension("norito.tmp");
        let (cert, checkpoint) = sample_cert(1);
        let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
        journal.upsert(cert.clone(), checkpoint.clone(), None);
        journal.persist().expect("persist");

        std::fs::rename(&path, &tmp_path).expect("move journal to temp");
        std::fs::write(&path, b"corrupted").expect("write corrupted journal");

        let error = CommitRosterJournal::load(path.clone(), retention(4))
            .expect_err("corrupt storage root must fail closed");
        assert!(matches!(
            error,
            CommitRosterJournalError::InvalidStorage { .. }
        ));
        assert!(
            tmp_path.exists(),
            "read-only load must not promote the legacy temp"
        );
    }

    #[test]
    fn journal_rejects_unsupported_version() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let payload = PersistedCommitRosters {
            version: 3,
            stake_snapshots: Vec::new(),
            entries: Vec::new(),
        };
        let bytes = norito::to_bytes(&payload).expect("encode payload");
        write_test_generation(&path, &bytes);

        let err = CommitRosterJournal::load(path, retention(4)).expect_err("unsupported version");
        assert!(matches!(
            err,
            CommitRosterJournalError::UnsupportedVersion { .. }
        ));
    }

    #[test]
    fn get_returns_matching_snapshot() {
        let (cert, checkpoint) = sample_cert(2);
        let mut journal = CommitRosterJournal::new(PathBuf::from("unused"), retention(4));
        journal.upsert(cert.clone(), checkpoint.clone(), None);

        let found = journal
            .get(cert.height, cert.subject_block_hash)
            .expect("snapshot must be present");
        assert_eq!(found.commit_qc, cert);
        assert_eq!(found.validator_checkpoint, checkpoint);

        assert!(
            journal
                .get(cert.height + 1, cert.subject_block_hash)
                .is_none(),
            "mismatched height should not return a snapshot"
        );
    }

    #[test]
    fn retention_drops_oldest_entries() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let mut journal = CommitRosterJournal::new(path.clone(), retention(2));
        for height in 1..=6 {
            let (mut cert, mut checkpoint) = cert_with_height(height, 0);
            if height == 1 {
                cert.aggregate.signers_bitmap.fill(0);
                cert.aggregate.bls_aggregate_signature.clear();
                checkpoint.signers_bitmap.fill(0);
                checkpoint.bls_aggregate_signature.clear();
            }
            journal.upsert(cert, checkpoint, None);
        }
        let snapshots = journal.snapshots();
        let heights: Vec<_> = snapshots
            .iter()
            .map(|snapshot| snapshot.commit_qc.height)
            .collect();
        assert_eq!(heights, vec![1, 4, 5, 6]);

        journal.persist().expect("persist");
        let reloaded = CommitRosterJournal::load(path, retention(2)).expect("load");
        let reloaded_heights: Vec<_> = reloaded
            .snapshots()
            .into_iter()
            .map(|snapshot| snapshot.commit_qc.height)
            .collect();
        assert_eq!(reloaded_heights, vec![1, 4, 5, 6]);
    }

    #[test]
    fn retention_one_keeps_all_tip_conflicts_and_prepared_successor_durable() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let mut journal = CommitRosterJournal::new(path.clone(), retention(1));

        let (mut genesis_qc, mut genesis_checkpoint) = cert_with_height(1, 0);
        genesis_qc.aggregate.signers_bitmap.fill(0);
        genesis_qc.aggregate.bls_aggregate_signature.clear();
        genesis_checkpoint.signers_bitmap.fill(0);
        genesis_checkpoint.bls_aggregate_signature.clear();
        let (tip_qc, tip_checkpoint) = cert_with_height(2, 0);
        let mut conflicting_tip_qc = tip_qc.clone();
        let conflicting_tip_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xA5; Hash::LENGTH]));
        conflicting_tip_qc.subject_block_hash = conflicting_tip_hash;
        conflicting_tip_qc.parent_state_root = Hash::prehashed([0xB5; Hash::LENGTH]);
        let mut conflicting_tip_checkpoint = tip_checkpoint.clone();
        conflicting_tip_checkpoint.block_hash = conflicting_tip_hash;
        conflicting_tip_checkpoint.parent_state_root = conflicting_tip_qc.parent_state_root;
        let (prepared_successor_qc, prepared_successor_checkpoint) = cert_with_height(3, 0);

        assert!(journal.upsert(genesis_qc.clone(), genesis_checkpoint.clone(), None));
        assert!(journal.upsert(tip_qc.clone(), tip_checkpoint.clone(), None));
        assert!(journal.upsert(
            conflicting_tip_qc.clone(),
            conflicting_tip_checkpoint.clone(),
            None,
        ));
        assert!(journal.upsert(
            prepared_successor_qc.clone(),
            prepared_successor_checkpoint.clone(),
            None,
        ));
        journal
            .persist()
            .expect("persist genesis, committed tip, and prepared successor");

        assert!(journal.durable_entry_matches_exact(&genesis_qc, &genesis_checkpoint, None,));
        assert!(journal.durable_entry_matches_exact(&tip_qc, &tip_checkpoint, None));
        assert!(journal.durable_entry_matches_exact(
            &conflicting_tip_qc,
            &conflicting_tip_checkpoint,
            None,
        ));
        assert!(journal.durable_entry_matches_exact(
            &prepared_successor_qc,
            &prepared_successor_checkpoint,
            None,
        ));
        let reloaded = CommitRosterJournal::load(path, retention(1)).expect("reload journal");
        let heights = reloaded
            .snapshots()
            .into_iter()
            .map(|snapshot| snapshot.commit_qc.height)
            .collect::<Vec<_>>();
        assert_eq!(heights, vec![1, 2, 2, 3]);
    }

    #[test]
    fn journal_persist_removes_temp_file() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (cert, checkpoint) = sample_cert(1);
        let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
        journal.upsert(cert, checkpoint, None);
        journal.persist().expect("persist");

        let tmp_path = path.with_extension("norito.tmp");
        assert!(!tmp_path.exists(), "temp journal file should be removed");
    }

    #[test]
    fn journal_path_empty_root_is_empty() {
        let path = CommitRosterJournal::journal_path(Path::new(""));
        assert!(path.as_os_str().is_empty());
    }
}
