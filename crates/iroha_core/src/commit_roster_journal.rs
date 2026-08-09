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

/// Exact durable publication shape for truncating one commit-roster journal.
///
/// The projection is intentionally serializable because canonical-prune recovery
/// authenticates the pre-publication identity and the exact remaining allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
pub(crate) struct CommitRosterJournalPruneProjectionV2 {
    /// Whether truncation removes at least one journal row.
    pub(crate) required: bool,
    /// Digest selected by `current` before truncation, if the journal is published.
    pub(crate) current_digest: Option<[u8; 32]>,
    /// Digest selected after truncation, or the unchanged current digest for a no-op.
    pub(crate) retained_digest: Option<[u8; 32]>,
    /// Exact canonical retained generation length.
    pub(crate) retained_payload_bytes: u64,
    /// New physical generation bytes required before pointer publication.
    pub(crate) generation_allocation_bytes: u64,
    /// Exact deterministic pointer temporary allocated during publication.
    pub(crate) pointer_temporary_bytes: u64,
    /// Stable pointer growth retained after publication when `current` was absent.
    pub(crate) current_pointer_growth_bytes: u64,
}

impl CommitRosterJournalPruneProjectionV2 {
    /// Canonical projection for an absent in-memory-only journal.
    #[must_use]
    pub(crate) const fn none() -> Self {
        Self {
            required: false,
            current_digest: None,
            retained_digest: None,
            retained_payload_bytes: 0,
            generation_allocation_bytes: 0,
            pointer_temporary_bytes: 0,
            current_pointer_growth_bytes: 0,
        }
    }

    /// Return whether every encoded byte count and identity has canonical shape.
    #[must_use]
    pub(crate) fn is_canonical(self) -> bool {
        if self.required {
            self.retained_digest.is_some()
                && self.retained_payload_bytes > 0
                && self.retained_payload_bytes <= CommitRosterJournal::MAX_PAYLOAD_BYTES
                && (self.generation_allocation_bytes == 0
                    || self.generation_allocation_bytes == self.retained_payload_bytes)
                && self.pointer_temporary_bytes == CommitRosterJournal::POINTER_BYTES
                && self.current_pointer_growth_bytes
                    == if self.current_digest.is_none() {
                        CommitRosterJournal::POINTER_BYTES
                    } else {
                        0
                    }
                && self.current_digest != self.retained_digest
        } else {
            self.current_digest == self.retained_digest
                && self.generation_allocation_bytes == 0
                && self.pointer_temporary_bytes == 0
                && self.current_pointer_growth_bytes == 0
                && ((self.retained_digest.is_none() && self.retained_payload_bytes == 0)
                    || (self.retained_digest.is_some()
                        && self.retained_payload_bytes > 0
                        && self.retained_payload_bytes <= CommitRosterJournal::MAX_PAYLOAD_BYTES))
        }
    }

    /// Return whether a recovery-time projection is an authorized pre- or post-publication state.
    #[must_use]
    pub(crate) fn authorizes(self, remaining: Self) -> bool {
        if !self.is_canonical() || !remaining.is_canonical() {
            return false;
        }
        if !self.required {
            return remaining == self;
        }
        if remaining.required {
            remaining.current_digest == self.current_digest
                && remaining.retained_digest == self.retained_digest
                && remaining.retained_payload_bytes == self.retained_payload_bytes
                && remaining.generation_allocation_bytes <= self.generation_allocation_bytes
                && remaining.pointer_temporary_bytes == self.pointer_temporary_bytes
                && remaining.current_pointer_growth_bytes == self.current_pointer_growth_bytes
        } else {
            remaining.current_digest == self.retained_digest
                && remaining.retained_digest == self.retained_digest
                && remaining.retained_payload_bytes == self.retained_payload_bytes
        }
    }

    /// Exact additional physical peak for this publication followed by a sidecar rewrite.
    pub(crate) fn allocation_peak_with_sidecar(self, sidecar_peak_bytes: u64) -> Option<u64> {
        if !self.required {
            return Some(sidecar_peak_bytes);
        }
        self.current_pointer_growth_bytes
            .checked_add(sidecar_peak_bytes)
            .map(|post_pointer| self.pointer_temporary_bytes.max(post_pointer))
            .and_then(|publication_peak| {
                self.generation_allocation_bytes
                    .checked_add(publication_peak)
            })
    }
}

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
    fail_generation_persist_once: bool,
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
    const CURRENT_TEMP_FILE: &'static str = "current.tmp";
    const GENERATIONS_DIR: &'static str = "generations";
    const MAX_PAYLOAD_BYTES: u64 = 16 * 1024 * 1024;
    const MAX_GENERATIONS: usize = 4096;
    const POINTER_BYTES: u64 = 65;
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
            fail_generation_persist_once: false,
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

    /// Inject one test-only failure after the generation temp is durable.
    #[cfg(test)]
    fn fail_generation_persist_once_for_tests(&mut self) {
        self.fail_generation_persist_once = true;
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
        let _publication_guard = COMMIT_ROSTER_PUBLICATION_LOCK.lock();
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
        Self::reconcile_publication_residues(&path)?;
        verify_roster_directory_identity(&path, root_identity).map_err(|source| {
            CommitRosterJournalError::Read {
                path: path.clone(),
                source,
            }
        })?;
        journal.validate_publication_namespace_is_clean()?;
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

    fn digest_bytes(digest: &str) -> Option<[u8; 32]> {
        let bytes = hex::decode(digest).ok()?;
        bytes.try_into().ok()
    }

    fn digest_text(digest: [u8; 32]) -> String {
        hex::encode(digest)
    }

    fn generation_path_for_digest(&self, digest: [u8; 32]) -> PathBuf {
        self.path
            .join(Self::GENERATIONS_DIR)
            .join(format!("{}.norito", Self::digest_text(digest)))
    }

    fn generation_temp_path_for_digest(&self, digest: [u8; 32]) -> PathBuf {
        self.path
            .join(Self::GENERATIONS_DIR)
            .join(format!("{}.norito.tmp", Self::digest_text(digest)))
    }

    fn current_digest_bytes(&self) -> Result<Option<[u8; 32]>, CommitRosterJournalError> {
        let path = self.path.join(Self::CURRENT_FILE);
        match fs::symlink_metadata(&path) {
            Ok(_) => {
                let digest = Self::read_current_digest(&path)?;
                Self::digest_bytes(&digest).map(Some).ok_or(
                    CommitRosterJournalError::InvalidStorage {
                        path,
                        reason: "current pointer digest cannot be represented canonically",
                    },
                )
            }
            Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(CommitRosterJournalError::Read { path, source }),
        }
    }

    fn reconcile_publication_residues(path: &Path) -> Result<(), CommitRosterJournalError> {
        let root_identity = direct_roster_directory_identity(path).map_err(|source| {
            CommitRosterJournalError::Read {
                path: path.to_path_buf(),
                source,
            }
        })?;
        let generations = path.join(Self::GENERATIONS_DIR);
        let generations_metadata = match fs::symlink_metadata(&generations) {
            Ok(metadata) => metadata,
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                let current_temp = path.join(Self::CURRENT_TEMP_FILE);
                if fs::symlink_metadata(&current_temp).is_ok() {
                    return Err(CommitRosterJournalError::InvalidStorage {
                        path: current_temp,
                        reason: "current-pointer temp exists without a generations directory",
                    });
                }
                return Ok(());
            }
            Err(source) => {
                return Err(CommitRosterJournalError::Read {
                    path: generations,
                    source,
                });
            }
        };
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
        let mut generation_temp = None;
        let mut scanned = 0_usize;
        for entry in
            fs::read_dir(&generations).map_err(|source| CommitRosterJournalError::Read {
                path: generations.clone(),
                source,
            })?
        {
            let entry = entry.map_err(|source| CommitRosterJournalError::Read {
                path: generations.clone(),
                source,
            })?;
            scanned = scanned.saturating_add(1);
            if scanned > Self::MAX_GENERATIONS {
                return Err(CommitRosterJournalError::InvalidStorage {
                    path: generations,
                    reason: "commit-roster generation count exceeds the hard scan bound",
                });
            }
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                return Err(CommitRosterJournalError::InvalidStorage {
                    path: entry.path(),
                    reason: "commit-roster generation name is not UTF-8",
                });
            };
            let Some(digest) = name.strip_suffix(".norito.tmp") else {
                continue;
            };
            if digest.len() != 64
                || Self::digest_bytes(digest).is_none_or(|bytes| Self::digest_text(bytes) != digest)
            {
                return Err(CommitRosterJournalError::InvalidStorage {
                    path: entry.path(),
                    reason: "generation temp name is not a canonical digest",
                });
            }
            if generation_temp
                .replace((entry.path(), digest.to_owned()))
                .is_some()
            {
                return Err(CommitRosterJournalError::InvalidStorage {
                    path: generations,
                    reason: "multiple commit-roster generation temps are unresolved",
                });
            }
        }
        if let Some((temporary, digest)) = generation_temp {
            let bytes =
                read_bound_roster_file(&temporary, Self::MAX_PAYLOAD_BYTES).map_err(|source| {
                    CommitRosterJournalError::Read {
                        path: temporary.clone(),
                        source,
                    }
                })?;
            if hex::encode(Sha256::digest(&bytes)) != digest {
                // A process may stop after exclusive creation but before the
                // generation temp is complete and synced. Its digest-named
                // target has not been published, so rollback is unambiguous.
                fs::remove_file(&temporary).map_err(|source| CommitRosterJournalError::Write {
                    path: temporary.clone(),
                    source,
                })?;
            } else {
                Self::decode_canonical_payload(&temporary, &bytes)?;
                let stable = generations.join(format!("{digest}.norito"));
                match fs::symlink_metadata(&stable) {
                    Ok(_) => {
                        let existing = read_bound_roster_file(&stable, Self::MAX_PAYLOAD_BYTES)
                            .map_err(|source| CommitRosterJournalError::Read {
                                path: stable.clone(),
                                source,
                            })?;
                        if existing != bytes {
                            return Err(CommitRosterJournalError::InvalidStorage {
                                path: stable,
                                reason: "generation temp conflicts with the stable digest path",
                            });
                        }
                        fs::remove_file(&temporary).map_err(|source| {
                            CommitRosterJournalError::Write {
                                path: temporary,
                                source,
                            }
                        })?;
                    }
                    Err(source) if source.kind() == io::ErrorKind::NotFound => {
                        fs::rename(&temporary, &stable).map_err(|source| {
                            CommitRosterJournalError::Write {
                                path: stable.clone(),
                                source,
                            }
                        })?;
                        let readback = read_bound_roster_file(&stable, Self::MAX_PAYLOAD_BYTES)
                            .map_err(|source| CommitRosterJournalError::Read {
                                path: stable.clone(),
                                source,
                            })?;
                        if readback != bytes {
                            return Err(CommitRosterJournalError::InvalidStorage {
                                path: stable,
                                reason: "recovered generation differs from its synced temp",
                            });
                        }
                    }
                    Err(source) => {
                        return Err(CommitRosterJournalError::Read {
                            path: stable,
                            source,
                        });
                    }
                }
            }
            sync_dir(&generations).map_err(|source| CommitRosterJournalError::NamespaceSync {
                path: generations.clone(),
                source,
            })?;
        }

        let current_temp = path.join(Self::CURRENT_TEMP_FILE);
        match fs::symlink_metadata(&current_temp) {
            Ok(_) => {
                let bytes = read_bound_roster_file(&current_temp, Self::POINTER_BYTES).map_err(
                    |source| CommitRosterJournalError::Read {
                        path: current_temp.clone(),
                        source,
                    },
                )?;
                let digest = std::str::from_utf8(&bytes)
                    .ok()
                    .and_then(|text| text.strip_suffix('\n'))
                    .and_then(|digest| {
                        Self::digest_bytes(digest)
                            .filter(|bytes| Self::digest_text(*bytes) == digest)
                            .map(|_| digest.to_owned())
                    });
                if let Some(digest) = digest {
                    let generation = generations.join(format!("{digest}.norito"));
                    let generation_bytes =
                        read_bound_roster_file(&generation, Self::MAX_PAYLOAD_BYTES).map_err(
                            |source| CommitRosterJournalError::Read {
                                path: generation.clone(),
                                source,
                            },
                        )?;
                    if hex::encode(Sha256::digest(&generation_bytes)) != digest {
                        return Err(CommitRosterJournalError::InvalidStorage {
                            path: generation,
                            reason: "current-pointer temp names an invalid generation",
                        });
                    }
                    Self::decode_canonical_payload(&generation, &generation_bytes)?;
                    let current = path.join(Self::CURRENT_FILE);
                    fs::rename(&current_temp, &current).map_err(|source| {
                        CommitRosterJournalError::Write {
                            path: current.clone(),
                            source,
                        }
                    })?;
                    sync_dir(path).map_err(|source| CommitRosterJournalError::NamespaceSync {
                        path: path.to_path_buf(),
                        source,
                    })?;
                    if Self::read_current_digest(&current)? != digest {
                        return Err(CommitRosterJournalError::InvalidStorage {
                            path: current,
                            reason: "recovered current pointer differs from its synced temp",
                        });
                    }
                } else {
                    // An incomplete pointer temp was never the selected
                    // generation. Roll it back and retain the stable pointer.
                    fs::remove_file(&current_temp).map_err(|source| {
                        CommitRosterJournalError::Write {
                            path: current_temp.clone(),
                            source,
                        }
                    })?;
                    sync_dir(path).map_err(|source| CommitRosterJournalError::NamespaceSync {
                        path: path.to_path_buf(),
                        source,
                    })?;
                }
            }
            Err(source) if source.kind() == io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(CommitRosterJournalError::Read {
                    path: current_temp,
                    source,
                });
            }
        }
        verify_roster_directory_identity(path, root_identity).map_err(|source| {
            CommitRosterJournalError::Read {
                path: path.to_path_buf(),
                source,
            }
        })?;
        verify_roster_directory_identity(&generations, generations_identity).map_err(|source| {
            CommitRosterJournalError::Read {
                path: generations,
                source,
            }
        })
    }

    fn validate_publication_namespace_is_clean(&self) -> Result<(), CommitRosterJournalError> {
        let root = match fs::symlink_metadata(&self.path) {
            Ok(metadata) => metadata,
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(source) => {
                return Err(CommitRosterJournalError::Read {
                    path: self.path.clone(),
                    source,
                });
            }
        };
        if root.file_type().is_symlink() || !root.is_dir() {
            return Err(CommitRosterJournalError::InvalidStorage {
                path: self.path.clone(),
                reason: "commit-roster root must be a direct directory",
            });
        }
        for entry in fs::read_dir(&self.path).map_err(|source| CommitRosterJournalError::Read {
            path: self.path.clone(),
            source,
        })? {
            let entry = entry.map_err(|source| CommitRosterJournalError::Read {
                path: self.path.clone(),
                source,
            })?;
            let name = entry.file_name();
            if name != Self::CURRENT_FILE && name != Self::GENERATIONS_DIR {
                return Err(CommitRosterJournalError::InvalidStorage {
                    path: entry.path(),
                    reason: "unexpected commit-roster publication artifact",
                });
            }
        }
        let generations = self.path.join(Self::GENERATIONS_DIR);
        match fs::symlink_metadata(&generations) {
            Ok(metadata) if !metadata.file_type().is_symlink() && metadata.is_dir() => {}
            Ok(_) => {
                return Err(CommitRosterJournalError::InvalidStorage {
                    path: generations,
                    reason: "commit-roster generations must be a direct directory",
                });
            }
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(source) => {
                return Err(CommitRosterJournalError::Read {
                    path: generations,
                    source,
                });
            }
        }
        let mut count = 0_usize;
        for entry in
            fs::read_dir(&generations).map_err(|source| CommitRosterJournalError::Read {
                path: generations.clone(),
                source,
            })?
        {
            let entry = entry.map_err(|source| CommitRosterJournalError::Read {
                path: generations.clone(),
                source,
            })?;
            count = count.saturating_add(1);
            if count > Self::MAX_GENERATIONS {
                return Err(CommitRosterJournalError::InvalidStorage {
                    path: generations,
                    reason: "commit-roster generation count exceeds the hard scan bound",
                });
            }
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                return Err(CommitRosterJournalError::InvalidStorage {
                    path: entry.path(),
                    reason: "commit-roster generation name is not UTF-8",
                });
            };
            let Some(digest) = name.strip_suffix(".norito") else {
                return Err(CommitRosterJournalError::InvalidStorage {
                    path: entry.path(),
                    reason: "unexpected commit-roster generation artifact",
                });
            };
            if digest.len() != 64
                || Self::digest_bytes(digest).is_none_or(|bytes| Self::digest_text(bytes) != digest)
            {
                return Err(CommitRosterJournalError::InvalidStorage {
                    path: entry.path(),
                    reason: "commit-roster generation name is not a canonical digest",
                });
            }
        }
        Ok(())
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

    fn canonical_payload_bytes(&mut self) -> Result<Vec<u8>, CommitRosterJournalError> {
        // Ensure the persisted payload honours the configured retention window.
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
        Ok(bytes)
    }

    /// Project an exact truncate publication without mutating journal memory or storage.
    pub(crate) fn project_truncate_to_height(
        &self,
        height: u64,
    ) -> Result<CommitRosterJournalPruneProjectionV2, CommitRosterJournalError> {
        if self.storage_unknown {
            return Err(CommitRosterJournalError::StorageUnknown {
                path: self.path.clone(),
            });
        }
        if self.path.as_os_str().is_empty() {
            return Ok(CommitRosterJournalPruneProjectionV2::none());
        }
        self.validate_publication_namespace_is_clean()?;
        let current_digest = self.current_digest_bytes()?;
        let required = self.has_entries_above(height);
        if !required {
            let retained_payload_bytes = if let Some(digest) = current_digest {
                let path = self.generation_path_for_digest(digest);
                u64::try_from(
                    read_bound_roster_file(&path, Self::MAX_PAYLOAD_BYTES)
                        .map_err(|source| CommitRosterJournalError::Read { path, source })?
                        .len(),
                )
                .unwrap_or(u64::MAX)
            } else {
                0
            };
            let projection = CommitRosterJournalPruneProjectionV2 {
                required: false,
                current_digest,
                retained_digest: current_digest,
                retained_payload_bytes,
                generation_allocation_bytes: 0,
                pointer_temporary_bytes: 0,
                current_pointer_growth_bytes: 0,
            };
            if !projection.is_canonical() {
                return Err(CommitRosterJournalError::InvalidStorage {
                    path: self.path.clone(),
                    reason: "current commit-roster identity has a non-canonical projection",
                });
            }
            return Ok(projection);
        }

        let mut retained = self.clone();
        retained
            .entries
            .retain(|(entry_height, _), _| *entry_height <= height);
        retained.dirty = true;
        let bytes = retained.canonical_payload_bytes()?;
        let retained_payload_bytes = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        let retained_digest = Sha256::digest(&bytes).into();
        let generation_path = self.generation_path_for_digest(retained_digest);
        let generation_allocation_bytes = match fs::symlink_metadata(&generation_path) {
            Ok(_) => {
                let existing = read_bound_roster_file(&generation_path, Self::MAX_PAYLOAD_BYTES)
                    .map_err(|source| CommitRosterJournalError::Read {
                        path: generation_path.clone(),
                        source,
                    })?;
                if existing != bytes {
                    return Err(CommitRosterJournalError::InvalidStorage {
                        path: generation_path,
                        reason: "retained digest path conflicts with its canonical payload",
                    });
                }
                0
            }
            Err(source) if source.kind() == io::ErrorKind::NotFound => retained_payload_bytes,
            Err(source) => {
                return Err(CommitRosterJournalError::Read {
                    path: generation_path,
                    source,
                });
            }
        };
        let projection = CommitRosterJournalPruneProjectionV2 {
            required: true,
            current_digest,
            retained_digest: Some(retained_digest),
            retained_payload_bytes,
            generation_allocation_bytes,
            pointer_temporary_bytes: Self::POINTER_BYTES,
            current_pointer_growth_bytes: if current_digest.is_none() {
                Self::POINTER_BYTES
            } else {
                0
            },
        };
        if !projection.is_canonical() {
            return Err(CommitRosterJournalError::InvalidStorage {
                path: self.path.clone(),
                reason: "commit-roster truncate projection is not canonical",
            });
        }
        Ok(projection)
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
        let bytes = self.canonical_payload_bytes()?;
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
        Self::reconcile_publication_residues(&self.path)?;
        self.validate_publication_namespace_is_clean()?;
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
        let digest_bytes: [u8; 32] = Sha256::digest(&bytes).into();
        let digest = Self::digest_text(digest_bytes);
        let generation_path = self.generation_path_for_digest(digest_bytes);
        let generation_temp_path = self.generation_temp_path_for_digest(digest_bytes);
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
        match fs::symlink_metadata(&generation_path) {
            Ok(_) => {
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
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                let mut temporary = fs::OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(&generation_temp_path)
                    .map_err(|source| CommitRosterJournalError::Write {
                        path: generation_temp_path.clone(),
                        source,
                    })?;
                temporary
                    .write_all(&bytes)
                    .and_then(|()| temporary.flush())
                    .and_then(|()| temporary.sync_all())
                    .map_err(|source| CommitRosterJournalError::Write {
                        path: generation_temp_path.clone(),
                        source,
                    })?;
                #[cfg(test)]
                if self.fail_generation_persist_once {
                    self.fail_generation_persist_once = false;
                    self.storage_unknown = true;
                    return Err(CommitRosterJournalError::Write {
                        path: generation_temp_path,
                        source: io::Error::other(
                            "injected durable generation-temp publication failure",
                        ),
                    });
                }
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
                if fs::symlink_metadata(&generation_path).is_ok() {
                    return Err(CommitRosterJournalError::InvalidStorage {
                        path: generation_path,
                        reason: "generation target appeared during deterministic publication",
                    });
                }
                fs::rename(&generation_temp_path, &generation_path).map_err(|source| {
                    self.storage_unknown = true;
                    CommitRosterJournalError::Write {
                        path: generation_path.clone(),
                        source,
                    }
                })?;
                sync_dir(&generations).map_err(|source| {
                    self.storage_unknown = true;
                    CommitRosterJournalError::NamespaceSync {
                        path: generations.clone(),
                        source,
                    }
                })?;
            }
            Err(source) => {
                return Err(CommitRosterJournalError::Read {
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
        let pointer_temp_path = self.path.join(Self::CURRENT_TEMP_FILE);
        let mut pointer_file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&pointer_temp_path)
            .map_err(|source| CommitRosterJournalError::Write {
                path: pointer_temp_path.clone(),
                source,
            })?;
        verify_roster_directory_identity(&self.path, root_identity).map_err(|source| {
            CommitRosterJournalError::Read {
                path: self.path.clone(),
                source,
            }
        })?;
        pointer_file
            .write_all(pointer_bytes.as_bytes())
            .and_then(|()| pointer_file.flush())
            .and_then(|()| pointer_file.sync_all())
            .map_err(|source| CommitRosterJournalError::Write {
                path: pointer_temp_path.clone(),
                source,
            })?;
        let pointer_metadata =
            pointer_file
                .metadata()
                .map_err(|source| CommitRosterJournalError::Read {
                    path: pointer_temp_path.clone(),
                    source,
                })?;
        let pointer_readback = read_bound_roster_file(&pointer_temp_path, Self::POINTER_BYTES)
            .map_err(|source| CommitRosterJournalError::Read {
                path: pointer_temp_path.clone(),
                source,
            })?;
        if pointer_readback != pointer_bytes.as_bytes() {
            return Err(CommitRosterJournalError::InvalidStorage {
                path: pointer_temp_path,
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
        if let Err(source) = fs::rename(&pointer_temp_path, &current_path) {
            self.storage_unknown = true;
            return Err(CommitRosterJournalError::Write {
                path: current_path.clone(),
                source,
            });
        }
        if let Err(source) = pointer_file.sync_all() {
            self.storage_unknown = true;
            return Err(CommitRosterJournalError::NamespaceSync {
                path: current_path.clone(),
                source,
            });
        }
        let persisted_metadata = match pointer_file.metadata() {
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

    /// Apply one prune-authorized truncation and require exact durable projection readback.
    pub(crate) fn truncate_to_height_with_projection(
        &mut self,
        height: u64,
        authorized: CommitRosterJournalPruneProjectionV2,
    ) -> Result<(), CommitRosterJournalError> {
        let before = self.project_truncate_to_height(height)?;
        if !authorized.authorizes(before) {
            return Err(CommitRosterJournalError::InvalidStorage {
                path: self.path.clone(),
                reason: "commit-roster state exceeds the authenticated prune projection",
            });
        }
        if before.required {
            self.entries
                .retain(|(entry_height, _), _| *entry_height <= height);
            self.dirty = true;
            self.persist()?;
        }
        let after = self.project_truncate_to_height(height)?;
        if after.required || !authorized.authorizes(after) {
            self.storage_unknown = true;
            return Err(CommitRosterJournalError::InvalidStorage {
                path: self.path.clone(),
                reason: "commit-roster prune publication failed exact durable readback",
            });
        }
        Ok(())
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
    include!("commit_roster_journal/tests.rs");
}
