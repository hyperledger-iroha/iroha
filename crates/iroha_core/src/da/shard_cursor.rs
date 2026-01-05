//! Durable shard cursor tracking for DA commitments.
//!
//! Each shard cursor records the highest `(epoch, sequence)` observed for a
//! shard-aligned lane along with the block height that advanced it. This index
//! is rebuilt from the Kura block log on startup so it remains consistent with
//! committed history even without a dedicated column family.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io,
    path::{Path, PathBuf},
};

use iroha_config::parameters::actual::LaneConfig;
use iroha_data_model::{
    da::commitment::{DaCommitmentBundle, DaCommitmentRecord},
    nexus::{LaneId, ShardId},
};
use iroha_logger::warn;
use norito::{
    codec::{Decode, Encode},
    decode_from_bytes, to_bytes,
};
use thiserror::Error;

/// Cursor describing the latest DA commitment accepted for a shard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DaShardCursor {
    /// Shard identifier derived from lane configuration.
    pub shard_id: u32,
    /// Epoch of the last commitment observed for this shard.
    pub epoch: u64,
    /// Sequence of the last commitment observed for this shard.
    pub sequence: u64,
    /// Block height that recorded the cursor advance.
    pub last_block_height: u64,
}

impl DaShardCursor {
    /// Construct a new cursor.
    #[must_use]
    pub fn new(shard_id: u32, epoch: u64, sequence: u64, last_block_height: u64) -> Self {
        Self {
            shard_id,
            epoch,
            sequence,
            last_block_height,
        }
    }
}

/// Errors surfaced when attempting to regress or derive a shard cursor.
#[allow(variant_size_differences)]
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum DaShardCursorError {
    /// A commitment attempted to move the cursor backwards.
    #[error(
        "shard {shard_id} cursor regression for lane {lane_id} at block {block_height}: observed epoch {observed_epoch} sequence {observed_sequence} was behind current epoch {current_epoch} sequence {current_sequence}"
    )]
    Regression {
        /// Shard identifier being advanced.
        shard_id: u32,
        /// Lane identifier tied to the shard.
        lane_id: LaneId,
        /// Epoch advertised by the new commitment.
        observed_epoch: u64,
        /// Sequence advertised by the new commitment.
        observed_sequence: u64,
        /// Current cursor epoch.
        current_epoch: u64,
        /// Current cursor sequence.
        current_sequence: u64,
        /// Block height attempting to advance the cursor.
        block_height: u64,
    },
    /// Shard cursor required for a lane was missing.
    #[error(
        "missing DA shard cursor for lane {lane_id} (shard {shard_id}) at block {block_height}"
    )]
    MissingCursor {
        /// Lane identifier that required a cursor entry.
        lane_id: LaneId,
        /// Shard identifier derived from the lane mapping.
        shard_id: u32,
        /// Block height requiring a cursor advance.
        block_height: u64,
    },
    /// Shard cursor existed but was not advanced for the current block.
    #[error(
        "stale DA shard cursor for lane {lane_id} (shard {shard_id}): last advanced at block {observed_height}, required {required_height}"
    )]
    StaleCursor {
        /// Lane identifier that required a cursor entry.
        lane_id: LaneId,
        /// Shard identifier derived from the lane mapping.
        shard_id: u32,
        /// Block height recorded on the cursor.
        observed_height: u64,
        /// Block height requiring a cursor advance.
        required_height: u64,
    },
    /// Lane was not present in the lane catalog mapping.
    #[error("lane {lane_id} is not present in the shard mapping at block {block_height}")]
    UnknownLane {
        /// Lane identifier with no mapping.
        lane_id: LaneId,
        /// Block height attempting to advance the cursor.
        block_height: u64,
    },
}

/// In-memory shard cursor index reconstructed from committed DA bundles.
#[derive(Debug, Default, Clone)]
pub struct DaShardCursorIndex {
    mapping: BTreeMap<LaneId, ShardId>,
    cursors: BTreeMap<u32, DaShardCursor>,
}

impl DaShardCursorIndex {
    /// Build a cursor index seeded with the current lane mapping.
    #[must_use]
    pub fn new(lane_config: &LaneConfig) -> Self {
        let mut index = Self::default();
        index.sync_mapping(lane_config);
        index
    }

    /// Refresh the lane→shard mapping and drop cursors whose shard disappeared.
    pub fn sync_mapping(&mut self, lane_config: &LaneConfig) {
        self.mapping = lane_config.shard_mapping();
        let allowed: BTreeSet<_> = self.mapping.values().map(|shard| shard.as_u32()).collect();
        self.cursors
            .retain(|shard_id, _| allowed.contains(shard_id));
    }

    /// Return the current cursor for a shard.
    #[must_use]
    pub fn get(&self, shard_id: u32) -> Option<&DaShardCursor> {
        self.cursors.get(&shard_id)
    }

    /// Advance the shard cursor using the supplied commitment.
    ///
    /// # Errors
    ///
    /// Returns [`DaShardCursorError::Regression`] when the commitment attempts
    /// to move the cursor backwards.
    pub fn advance(
        &mut self,
        shard_id: u32,
        record: &DaCommitmentRecord,
        block_height: u64,
    ) -> Result<(), DaShardCursorError> {
        if let Some(cursor) = self.cursors.get(&shard_id) {
            let current = (cursor.epoch, cursor.sequence);
            let observed = (record.epoch, record.sequence);
            if observed <= current {
                return Err(DaShardCursorError::Regression {
                    shard_id,
                    lane_id: record.lane_id,
                    observed_epoch: record.epoch,
                    observed_sequence: record.sequence,
                    current_epoch: cursor.epoch,
                    current_sequence: cursor.sequence,
                    block_height,
                });
            }
        }

        let cursor = DaShardCursor::new(shard_id, record.epoch, record.sequence, block_height);
        self.cursors.insert(shard_id, cursor);
        Ok(())
    }

    /// Bulk-advance cursors using all commitments in the bundle.
    ///
    /// # Errors
    ///
    /// Returns the first regression encountered.
    pub fn advance_bundle(
        &mut self,
        shard_id_for_lane: impl Fn(LaneId) -> u32,
        records: &[DaCommitmentRecord],
        block_height: u64,
    ) -> Result<(), DaShardCursorError> {
        for record in records {
            let shard_id = shard_id_for_lane(record.lane_id);
            self.advance(shard_id, record, block_height)?;
        }
        Ok(())
    }

    /// Record all commitments in the bundle against the shard cursor index.
    ///
    /// # Errors
    ///
    /// Returns [`DaShardCursorError::Regression`] if any commitment attempts to
    /// regress an existing cursor or [`DaShardCursorError::UnknownLane`] when a
    /// commitment references an unmapped lane.
    pub fn record_bundle(
        &mut self,
        lane_config: &LaneConfig,
        bundle: &DaCommitmentBundle,
        block_height: u64,
    ) -> Result<(), DaShardCursorError> {
        self.record_records(lane_config, &bundle.commitments, block_height)
    }

    /// Record a slice of commitments against the shard cursor index.
    ///
    /// # Errors
    ///
    /// Returns [`DaShardCursorError::Regression`] if any commitment attempts to
    /// regress an existing cursor or [`DaShardCursorError::UnknownLane`] when a
    /// commitment references an unmapped lane.
    pub fn record_records(
        &mut self,
        lane_config: &LaneConfig,
        records: &[DaCommitmentRecord],
        block_height: u64,
    ) -> Result<(), DaShardCursorError> {
        self.sync_mapping(lane_config);
        for record in records {
            let shard_id = self.shard_id_for_lane(record.lane_id, block_height)?;
            self.advance(shard_id, record, block_height)?;
        }
        Ok(())
    }

    fn shard_id_for_lane(
        &self,
        lane_id: LaneId,
        block_height: u64,
    ) -> Result<u32, DaShardCursorError> {
        self.mapping
            .get(&lane_id)
            .map(|id| id.as_u32())
            .ok_or(DaShardCursorError::UnknownLane {
                lane_id,
                block_height,
            })
    }

    /// Return true when no shard cursors are tracked.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cursors.is_empty()
    }

    /// Restore a cursor entry sourced from a persisted journal.
    ///
    /// Returns `Ok(true)` when the entry was installed, `Ok(false)` when the entry was skipped
    /// because an equal or newer cursor already exists, or an error when the lane mapping no
    /// longer matches the journal entry.
    ///
    /// # Errors
    ///
    /// Returns [`DaShardCursorError::UnknownLane`] when the lane map no longer matches the stored
    /// shard identifier.
    pub fn restore_from_journal_entry(
        &mut self,
        lane_config: &LaneConfig,
        entry: &LaneShardCursor,
    ) -> Result<bool, DaShardCursorError> {
        self.sync_mapping(lane_config);
        let shard_id = self.shard_id_for_lane(entry.lane_id, entry.last_block_height)?;
        if shard_id != entry.shard_id.as_u32() {
            return Err(DaShardCursorError::UnknownLane {
                lane_id: entry.lane_id,
                block_height: entry.last_block_height,
            });
        }

        let candidate = (entry.epoch, entry.sequence, entry.last_block_height);
        let should_update = self.cursors.get(&shard_id).is_none_or(|existing| {
            (
                existing.epoch,
                existing.sequence,
                existing.last_block_height,
            ) < candidate
        });
        if should_update {
            self.cursors.insert(
                shard_id,
                DaShardCursor::new(
                    shard_id,
                    entry.epoch,
                    entry.sequence,
                    entry.last_block_height,
                ),
            );
            return Ok(true);
        }
        Ok(false)
    }
}

/// Persisted cursor for a `(shard_id, lane_id)` pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
pub struct LaneShardCursor {
    /// Shard identifier associated with the lane.
    pub shard_id: ShardId,
    /// Lane identifier.
    pub lane_id: LaneId,
    /// Epoch tied to the latest observed commitment.
    pub epoch: u64,
    /// Sequence tied to the latest observed commitment.
    pub sequence: u64,
    /// Block height that last advanced the cursor.
    #[norito(default)]
    pub last_block_height: u64,
}

/// Persisted representation of shard cursors.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
struct PersistedShardCursors {
    /// Journal version for forward compatibility.
    version: u32,
    /// Stored cursor entries.
    entries: Vec<LaneShardCursor>,
}

/// Errors returned when loading or persisting shard cursors.
#[derive(Debug, Error)]
pub enum ShardCursorJournalError {
    /// Failed to read the persisted journal.
    #[error("failed to read DA shard cursor journal {path}: {source}")]
    Read {
        /// Path that failed.
        path: PathBuf,
        /// Source error.
        #[source]
        source: std::io::Error,
    },
    /// Failed to decode the persisted journal.
    #[error("failed to decode DA shard cursor journal {path}: {source}")]
    Decode {
        /// Path that failed.
        path: PathBuf,
        /// Source decode error.
        #[source]
        source: norito::core::Error,
    },
    /// Failed to write the journal to disk.
    #[error("failed to persist DA shard cursor journal {path}: {source}")]
    Write {
        /// Path that failed.
        path: PathBuf,
        /// Source error.
        #[source]
        source: std::io::Error,
    },
    /// Failed to encode the journal payload.
    #[error("failed to encode DA shard cursor journal: {0}")]
    Encode(#[source] norito::core::Error),
    /// Lane was not present in the lane catalog mapping.
    #[error("lane {lane_id} is not present in the shard mapping")]
    UnknownLane {
        /// Lane identifier with no mapping.
        lane_id: LaneId,
    },
    /// Persisted journal uses an unsupported version.
    #[error("unsupported DA shard cursor journal version {version} at {path}")]
    UnsupportedVersion {
        /// Path for the journal.
        path: PathBuf,
        /// Unsupported version encountered.
        version: u32,
    },
}

/// Journal that records shard cursors derived from DA commitments.
#[derive(Debug)]
pub struct DaShardCursorJournal {
    mapping: BTreeMap<LaneId, ShardId>,
    cursors: BTreeMap<(ShardId, LaneId), LaneShardCursor>,
    path: PathBuf,
}

impl DaShardCursorJournal {
    /// Filename used to persist shard cursor journals next to the DA spool.
    pub const JOURNAL_FILE: &'static str = "da-shard-cursors.norito";
    const JOURNAL_VERSION: u32 = 1;

    /// Build the canonical journal path under the provided root.
    #[must_use]
    pub fn journal_path(root: &Path) -> PathBuf {
        root.join(Self::JOURNAL_FILE)
    }

    /// Construct a fresh journal with an empty cursor set.
    #[must_use]
    pub fn new(lane_config: &LaneConfig, path: impl Into<PathBuf>) -> Self {
        Self {
            mapping: lane_config.shard_mapping(),
            cursors: BTreeMap::new(),
            path: path.into(),
        }
    }

    /// Load a journal from disk, dropping entries whose lane→shard binding no longer matches.
    ///
    /// Missing files are treated as empty journals.
    ///
    /// # Errors
    ///
    /// Returns [`ShardCursorJournalError::Read`] or [`ShardCursorJournalError::Decode`] when persistence fails.
    pub fn load(
        lane_config: &LaneConfig,
        path: impl Into<PathBuf>,
    ) -> Result<Self, ShardCursorJournalError> {
        let path = path.into();
        let mut journal = Self::new(lane_config, path.clone());
        let tmp_path = Self::temp_path(&path);
        let persisted = match Self::read_persisted(&path) {
            Ok(Some(persisted)) => {
                if let Ok(Some(_)) = Self::read_persisted(&tmp_path) {
                    if let Err(err) = fs::remove_file(&tmp_path) {
                        warn!(
                            ?err,
                            path = %tmp_path.display(),
                            "failed to remove stale DA shard cursor journal temp file"
                        );
                    }
                }
                Some(persisted)
            }
            Ok(None) => match Self::read_persisted(&tmp_path) {
                Ok(Some(persisted)) => {
                    Self::promote_temp(&tmp_path, &path);
                    Some(persisted)
                }
                Ok(None) => None,
                Err(err) => {
                    warn!(
                        ?err,
                        path = %tmp_path.display(),
                        "failed to read DA shard cursor journal temp file"
                    );
                    let _ = fs::remove_file(&tmp_path);
                    None
                }
            },
            Err(err) => match Self::read_persisted(&tmp_path) {
                Ok(Some(persisted)) => {
                    warn!(
                        ?err,
                        path = %path.display(),
                        "DA shard cursor journal invalid; recovering from temp file"
                    );
                    Self::promote_temp(&tmp_path, &path);
                    Some(persisted)
                }
                Ok(None) => return Err(err),
                Err(tmp_err) => {
                    warn!(
                        ?tmp_err,
                        path = %tmp_path.display(),
                        "failed to read DA shard cursor journal temp file"
                    );
                    return Err(err);
                }
            },
        };

        if let Some(persisted) = persisted {
            for entry in persisted.entries {
            let Some(expected) = journal.mapping.get(&entry.lane_id).copied() else {
                warn!(
                    lane = %entry.lane_id.as_u32(),
                    shard = %entry.shard_id.as_u32(),
                    "dropping shard cursor: lane missing from catalog"
                );
                continue;
            };

            if expected != entry.shard_id {
                warn!(
                    lane = %entry.lane_id.as_u32(),
                    stored = %entry.shard_id.as_u32(),
                    expected = %expected.as_u32(),
                    "dropping shard cursor: lane resharded since last run"
                );
                continue;
            }

            journal.upsert(entry);
            }
        }

        Ok(journal)
    }

    /// Record cursors for every commitment in the bundle.
    ///
    /// # Errors
    ///
    /// Returns [`ShardCursorJournalError::UnknownLane`] when a commitment references an unmapped lane.
    pub fn record_bundle(
        &mut self,
        block_height: u64,
        bundle: &DaCommitmentBundle,
    ) -> Result<(), ShardCursorJournalError> {
        for record in &bundle.commitments {
            self.record_commitment(block_height, record)?;
        }
        Ok(())
    }

    /// Record a single commitment against the shard journal.
    ///
    /// # Errors
    ///
    /// Returns [`ShardCursorJournalError::UnknownLane`] when the lane is not present in the catalog.
    pub fn record_commitment(
        &mut self,
        block_height: u64,
        record: &DaCommitmentRecord,
    ) -> Result<(), ShardCursorJournalError> {
        let shard_id =
            *self
                .mapping
                .get(&record.lane_id)
                .ok_or(ShardCursorJournalError::UnknownLane {
                    lane_id: record.lane_id,
                })?;

        let entry = LaneShardCursor {
            shard_id,
            lane_id: record.lane_id,
            epoch: record.epoch,
            sequence: record.sequence,
            last_block_height: block_height,
        };
        self.upsert(entry);
        Ok(())
    }

    /// Retrieve the cursor for a lane if present.
    #[must_use]
    pub fn cursor_for_lane(&self, lane_id: LaneId) -> Option<&LaneShardCursor> {
        self.mapping
            .get(&lane_id)
            .and_then(|shard_id| self.cursors.get(&(*shard_id, lane_id)))
    }

    /// Snapshot the stored cursor entries (deduplicated per lane).
    pub fn entries(&self) -> impl Iterator<Item = &LaneShardCursor> {
        self.cursors.values()
    }

    /// Expose the underlying mapping for diagnostics.
    #[must_use]
    pub fn mapping(&self) -> &BTreeMap<LaneId, ShardId> {
        &self.mapping
    }

    /// Persist the current cursor set to disk.
    ///
    /// # Errors
    ///
    /// Returns [`ShardCursorJournalError::Write`] when the journal cannot be written or
    /// [`ShardCursorJournalError::Encode`] when encoding fails.
    pub fn persist(&self) -> Result<(), ShardCursorJournalError> {
        let entries: Vec<_> = self.cursors.values().copied().collect();
        let payload = PersistedShardCursors {
            version: Self::JOURNAL_VERSION,
            entries,
        };
        let bytes = to_bytes(&payload).map_err(ShardCursorJournalError::Encode)?;

        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|source| ShardCursorJournalError::Write {
                path: self.path.clone(),
                source,
            })?;
        }
        let tmp_path = Self::temp_path(&self.path);
        {
            let mut file = fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&tmp_path)
                .map_err(|source| ShardCursorJournalError::Write {
                    path: tmp_path.clone(),
                    source,
                })?;
            file.write_all(&bytes)
                .map_err(|source| ShardCursorJournalError::Write {
                    path: tmp_path.clone(),
                    source,
                })?;
            file.sync_all()
                .map_err(|source| ShardCursorJournalError::Write {
                    path: tmp_path.clone(),
                    source,
                })?;
        }
        if let Err(err) = fs::rename(&tmp_path, &self.path) {
            if err.kind() == io::ErrorKind::AlreadyExists {
                fs::remove_file(&self.path).map_err(|source| ShardCursorJournalError::Write {
                    path: self.path.clone(),
                    source,
                })?;
                fs::rename(&tmp_path, &self.path)
                    .map_err(|source| ShardCursorJournalError::Write {
                        path: self.path.clone(),
                        source,
                    })?;
            } else {
                return Err(ShardCursorJournalError::Write {
                    path: self.path.clone(),
                    source: err,
                });
            }
        }
        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                sync_dir(parent).map_err(|source| ShardCursorJournalError::Write {
                    path: self.path.clone(),
                    source,
                })?;
            }
        }
        Ok(())
    }

    /// Build a journal populated from an in-memory cursor index.
    #[must_use]
    pub fn from_index(
        lane_config: &LaneConfig,
        index: &DaShardCursorIndex,
        path: impl Into<PathBuf>,
    ) -> Self {
        let mut journal = Self::new(lane_config, path);
        let mapping = journal.mapping.clone();
        for (lane_id, shard_id) in mapping {
            if let Some(cursor) = index.get(shard_id.as_u32()) {
                let entry = LaneShardCursor {
                    shard_id,
                    lane_id,
                    epoch: cursor.epoch,
                    sequence: cursor.sequence,
                    last_block_height: cursor.last_block_height,
                };
                journal.upsert(entry);
            }
        }
        journal
    }

    fn upsert(&mut self, entry: LaneShardCursor) {
        let key = (entry.shard_id, entry.lane_id);
        let should_update = self
            .cursors
            .get(&key)
            .is_none_or(|existing| Self::should_advance(existing, &entry));
        if should_update {
            self.cursors.insert(key, entry);
        }
    }

    fn should_advance(current: &LaneShardCursor, candidate: &LaneShardCursor) -> bool {
        (candidate.epoch, candidate.sequence) > (current.epoch, current.sequence)
    }

    fn temp_path(path: &Path) -> PathBuf {
        path.with_extension("norito.tmp")
    }

    fn read_persisted(
        path: &Path,
    ) -> Result<Option<PersistedShardCursors>, ShardCursorJournalError> {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => {
                return Err(ShardCursorJournalError::Read {
                    path: path.to_path_buf(),
                    source,
                });
            }
        };

        let persisted: PersistedShardCursors =
            decode_from_bytes(&bytes).map_err(|source| ShardCursorJournalError::Decode {
                path: path.to_path_buf(),
                source,
            })?;

        if persisted.version != Self::JOURNAL_VERSION {
            return Err(ShardCursorJournalError::UnsupportedVersion {
                path: path.to_path_buf(),
                version: persisted.version,
            });
        }

        Ok(Some(persisted))
    }

    fn promote_temp(tmp_path: &Path, path: &Path) {
        let promoted = match fs::rename(tmp_path, path) {
            Ok(()) => true,
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                if let Err(remove_err) = fs::remove_file(path) {
                    warn!(
                        ?remove_err,
                        path = %path.display(),
                        "failed to remove DA shard cursor journal before temp promotion"
                    );
                    false
                } else if let Err(rename_err) = fs::rename(tmp_path, path) {
                    warn!(
                        ?rename_err,
                        path = %tmp_path.display(),
                        "failed to promote DA shard cursor journal temp file after removal"
                    );
                    false
                } else {
                    true
                }
            }
            Err(err) => {
                warn!(
                    ?err,
                    path = %tmp_path.display(),
                    "failed to promote DA shard cursor journal temp file"
                );
                false
            }
        };

        if promoted {
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    if let Err(err) = sync_dir(parent) {
                        warn!(
                            ?err,
                            path = %parent.display(),
                            "failed to sync DA shard cursor journal directory"
                        );
                    }
                }
            }
        }
    }
}

fn sync_dir(path: &Path) -> std::io::Result<()> {
    let file = fs::File::open(path)?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, num::NonZeroU32, path::PathBuf};

    use iroha_config::parameters::actual::LaneConfig as ConfigLaneConfig;
    use iroha_crypto::{Hash, Signature};
    use iroha_data_model::{
        da::{
            commitment::{
                DaCommitmentBundle, DaCommitmentRecord, DaProofScheme, KzgCommitment,
                RetentionClass,
            },
            types::{BlobDigest, StorageTicketId},
        },
        nexus::{
            DataSpaceId, LaneCatalog, LaneConfig as ModelLaneConfig, LaneId, LaneStorageProfile,
            LaneVisibility, ShardId,
        },
        sorafs::pin_registry::ManifestDigest,
    };
    use tempfile::tempdir;

    use super::*;

    fn sample_record(lane_id: u32, epoch: u64, sequence: u64) -> DaCommitmentRecord {
        let lane_byte = u8::try_from(lane_id).expect("lane fits in u8 for test record");
        let epoch_byte = u8::try_from(epoch).expect("epoch fits in u8 for test record");
        let sequence_byte = u8::try_from(sequence).expect("sequence fits in u8 for test record");
        DaCommitmentRecord::new(
            LaneId::new(lane_id),
            epoch,
            sequence,
            BlobDigest::new([lane_byte; 32]),
            ManifestDigest::new([epoch_byte; 32]),
            DaProofScheme::MerkleSha256,
            Hash::prehashed([sequence_byte; 32]),
            Some(KzgCommitment::new([0x11; 48])),
            None,
            RetentionClass::default(),
            StorageTicketId::new([0x22; 32]),
            Signature::from_bytes(&[0x33; 64]),
        )
    }

    fn lane_config_with_mapping(lane_id: u32, shard_id: u32) -> ConfigLaneConfig {
        let mut metadata = BTreeMap::new();
        metadata.insert("da_shard_id".to_string(), shard_id.to_string());
        let lane_count = NonZeroU32::new(lane_id.saturating_add(1)).expect("lane count");
        let catalog = LaneCatalog::new(
            lane_count,
            vec![ModelLaneConfig {
                id: LaneId::new(lane_id),
                dataspace_id: DataSpaceId::GLOBAL,
                alias: format!("lane{lane_id}"),
                metadata,
                ..ModelLaneConfig::default()
            }],
        )
        .expect("lane catalog");

        ConfigLaneConfig::from_catalog(&catalog)
    }

    #[test]
    fn advances_and_records_height() {
        let mut index = DaShardCursorIndex::default();
        let record = sample_record(1, 2, 3);

        index
            .advance(99, &record, 7)
            .expect("advance should succeed");
        let cursor = index.get(99).expect("cursor present");
        assert_eq!(cursor.shard_id, 99);
        assert_eq!(cursor.epoch, 2);
        assert_eq!(cursor.sequence, 3);
        assert_eq!(cursor.last_block_height, 7);
    }

    #[test]
    fn rejects_regression() {
        let mut index = DaShardCursorIndex::default();
        let first = sample_record(1, 5, 7);
        index
            .advance(1, &first, 10)
            .expect("first advance should succeed");

        let regressing = sample_record(1, 5, 6);
        let err = index
            .advance(1, &regressing, 11)
            .expect_err("regression should be rejected");
        match err {
            DaShardCursorError::Regression {
                shard_id, lane_id, ..
            } => {
                assert_eq!(shard_id, 1);
                assert_eq!(lane_id, LaneId::new(1));
            }
            other => panic!("expected regression, got {other:?}"),
        }
        let cursor = index.get(1).expect("cursor should remain unchanged");
        assert_eq!(cursor.sequence, 7);
        assert_eq!(cursor.last_block_height, 10);
    }

    #[test]
    fn advance_bundle_handles_multiple_lanes() {
        let mut index = DaShardCursorIndex::default();
        let records = vec![
            sample_record(1, 1, 1),
            sample_record(2, 2, 3),
            sample_record(1, 2, 0),
        ];

        index
            .advance_bundle(|lane| lane.as_u32() + 10, &records, 5)
            .expect("bundle should advance cursors");

        let shard_one = index.get(11).expect("lane 1 shard cursor present");
        assert_eq!(shard_one.epoch, 2);
        assert_eq!(shard_one.sequence, 0);
        assert_eq!(shard_one.last_block_height, 5);

        let shard_two = index.get(12).expect("lane 2 shard cursor present");
        assert_eq!(shard_two.epoch, 2);
        assert_eq!(shard_two.sequence, 3);
        assert_eq!(shard_two.last_block_height, 5);
    }

    #[test]
    fn record_bundle_syncs_mapping() {
        let bundle = DaCommitmentBundle::new(vec![sample_record(1, 7, 9)]);
        let mut index = DaShardCursorIndex::new(&ConfigLaneConfig::default());
        let config = lane_config_with_mapping(1, 10);

        index
            .record_bundle(&config, &bundle, 5)
            .expect("record bundle");

        let cursor = index.get(10).expect("cursor present");
        assert_eq!(cursor.epoch, 7);
        assert_eq!(cursor.sequence, 9);
        assert_eq!(cursor.last_block_height, 5);
    }

    #[test]
    fn bundle_rejects_unknown_lane() {
        let config = ConfigLaneConfig::default();
        let bundle = DaCommitmentBundle::new(vec![sample_record(5, 1, 1)]);
        let mut index = DaShardCursorIndex::default();

        let err = index
            .record_bundle(&config, &bundle, 1)
            .expect_err("lane missing from config");
        assert!(matches!(err, DaShardCursorError::UnknownLane { .. }));
    }

    #[test]
    fn record_records_rejects_unknown_lane() {
        let config = lane_config_with_mapping(0, 1);
        let mut index = DaShardCursorIndex::new(&config);
        let unknown = sample_record(3, 1, 0);

        let err = index
            .record_records(&config, &[unknown], 9)
            .expect_err("lane must be present in mapping");
        assert!(
            matches!(err, DaShardCursorError::UnknownLane { lane_id, .. } if lane_id == LaneId::new(3))
        );
        assert!(index.get(3).is_none(), "cursor should remain untouched");
    }

    #[test]
    fn record_records_respects_reshard_mapping() {
        let initial_config = lane_config_with_mapping(4, 5);
        let mut index = DaShardCursorIndex::new(&initial_config);
        index
            .record_records(&initial_config, &[sample_record(4, 1, 1)], 3)
            .expect("initial record");
        assert!(index.get(5).is_some(), "cursor stored under original shard");

        let resharded_config = lane_config_with_mapping(4, 7);
        index
            .record_records(&resharded_config, &[sample_record(4, 2, 0)], 4)
            .expect("reshard record");

        assert!(index.get(5).is_none(), "old shard cursor should be removed");
        let cursor = index.get(7).expect("cursor stored under new shard id");
        assert_eq!((cursor.epoch, cursor.sequence), (2, 0));
        assert_eq!(cursor.last_block_height, 4);
    }

    #[test]
    fn journal_persists_and_recovers_cursors() {
        let dir = tempdir().expect("tempdir");
        let catalog = LaneCatalog::new(
            NonZeroU32::new(1).expect("lane count"),
            vec![ModelLaneConfig {
                id: LaneId::new(0),
                dataspace_id: DataSpaceId::GLOBAL,
                alias: "lane".to_string(),
                description: None,
                visibility: LaneVisibility::Public,
                lane_type: None,
                governance: None,
                settlement: None,
                storage: LaneStorageProfile::FullReplica,
                proof_scheme: DaProofScheme::default(),
                metadata: BTreeMap::default(),
            }],
        )
        .expect("catalog");
        let lane_config = ConfigLaneConfig::from_catalog(&catalog);
        let path = DaShardCursorJournal::journal_path(dir.path());

        {
            let mut journal = DaShardCursorJournal::new(&lane_config, path.clone());
            let record = DaCommitmentRecord::new(
                LaneId::new(0),
                1,
                2,
                BlobDigest::new([0xAA; 32]),
                ManifestDigest::new([0xBB; 32]),
                DaProofScheme::MerkleSha256,
                Hash::prehashed([0xCC; 32]),
                Some(KzgCommitment::new([0xDD; 48])),
                None,
                RetentionClass::default(),
                StorageTicketId::new([0xEE; 32]),
                Signature::from_bytes(&[0x11; 64]),
            );
            journal
                .record_commitment(1, &record)
                .expect("record commitment");
            journal.persist().expect("persist");
        }

        let loaded = DaShardCursorJournal::load(&lane_config, path).expect("load");
        let cursor = loaded.cursor_for_lane(LaneId::new(0)).expect("cursor");
        assert_eq!(cursor.shard_id, ShardId::new(0));
        assert_eq!(cursor.epoch, 1);
        assert_eq!(cursor.sequence, 2);
    }

    #[test]
    fn journal_drops_resharded_entries() {
        let dir = tempdir().expect("tempdir");
        let path = DaShardCursorJournal::journal_path(dir.path());
        let initial_config = lane_config_with_mapping(1, 2);
        let mut journal = DaShardCursorJournal::new(&initial_config, path.clone());
        journal
            .record_commitment(1, &sample_record(1, 5, 6))
            .expect("record");
        journal.persist().expect("persist");

        let resharded_config = lane_config_with_mapping(1, 3);
        let loaded = DaShardCursorJournal::load(&resharded_config, path).expect("load");
        assert!(loaded.cursor_for_lane(LaneId::new(1)).is_none());
    }

    #[test]
    fn journal_advances_only_forward() {
        let dir = tempdir().expect("tempdir");
        let path = DaShardCursorJournal::journal_path(dir.path());
        let config = lane_config_with_mapping(0, 0);
        let mut journal = DaShardCursorJournal::new(&config, path);

        journal
            .record_commitment(1, &sample_record(0, 1, 1))
            .expect("record");
        journal
            .record_commitment(2, &sample_record(0, 1, 0))
            .expect("record");

        let cursor = journal.cursor_for_lane(LaneId::new(0)).expect("cursor");
        assert_eq!((cursor.epoch, cursor.sequence), (1, 1));
    }

    #[test]
    fn journal_bundle_updates_multiple_lanes() {
        let catalog = LaneCatalog::new(
            NonZeroU32::new(2).expect("lane count"),
            vec![
                ModelLaneConfig {
                    id: LaneId::new(0),
                    alias: "lane0".into(),
                    ..ModelLaneConfig::default()
                },
                ModelLaneConfig {
                    id: LaneId::new(1),
                    alias: "lane1".into(),
                    metadata: {
                        let mut map = BTreeMap::new();
                        map.insert("da_shard_id".to_string(), "5".to_string());
                        map
                    },
                    ..ModelLaneConfig::default()
                },
            ],
        )
        .expect("catalog");
        let config = ConfigLaneConfig::from_catalog(&catalog);
        let mut journal = DaShardCursorJournal::new(&config, PathBuf::from("unused"));

        let bundle = DaCommitmentBundle::new(vec![sample_record(0, 0, 1), sample_record(1, 2, 3)]);
        journal.record_bundle(1, &bundle).expect("record bundle");

        let shard0 = journal
            .cursor_for_lane(LaneId::new(0))
            .expect("lane0 cursor");
        assert_eq!(shard0.shard_id, ShardId::new(0));
        let shard1 = journal
            .cursor_for_lane(LaneId::new(1))
            .expect("lane1 cursor");
        assert_eq!(shard1.shard_id, ShardId::new(5));
        assert_eq!((shard1.epoch, shard1.sequence), (2, 3));
    }

    #[test]
    fn rejects_unknown_journal_version() {
        let dir = tempdir().expect("tempdir");
        let path = DaShardCursorJournal::journal_path(dir.path());
        let catalog = LaneCatalog::new(
            NonZeroU32::new(1).expect("lane count"),
            vec![ModelLaneConfig {
                id: LaneId::new(0),
                dataspace_id: DataSpaceId::GLOBAL,
                alias: "lane0".into(),
                ..ModelLaneConfig::default()
            }],
        )
        .expect("catalog");
        let config = ConfigLaneConfig::from_catalog(&catalog);
        let payload = PersistedShardCursors {
            version: DaShardCursorJournal::JOURNAL_VERSION + 1,
            entries: Vec::new(),
        };

        fs::write(&path, to_bytes(&payload).expect("encode")).expect("write");

        match DaShardCursorJournal::load(&config, path).expect_err("version mismatch") {
            ShardCursorJournalError::UnsupportedVersion { version, .. } => {
                assert_eq!(version, DaShardCursorJournal::JOURNAL_VERSION + 1);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
