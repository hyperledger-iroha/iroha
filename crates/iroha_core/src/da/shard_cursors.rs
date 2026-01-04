//! Durable shard cursor tracking for DA commitments.
//!
//! Each shard cursor records the highest `(epoch, sequence)` observed for a
//! shard-aligned lane along with the block height that advanced it. This index
//! is rebuilt from the Kura block log on startup so it remains consistent with
//! committed history even without a dedicated column family.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use iroha_config::parameters::actual::LaneConfig;
use iroha_data_model::{
    da::commitment::{DaCommitmentBundle, DaCommitmentRecord},
    nexus::{LaneId, ShardId},
};
use iroha_logger::warn;
use nonzero_ext::nonzero;
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
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DaShardCursorError {
    /// A commitment attempted to move the cursor backwards.
    #[error("shard {shard_id} cursor regression: observed epoch {observed_epoch} sequence {observed_sequence} was behind current epoch {current_epoch} sequence {current_sequence}")]
    Regression {
        /// Shard identifier being advanced.
        shard_id: u32,
        /// Epoch advertised by the new commitment.
        observed_epoch: u64,
        /// Sequence advertised by the new commitment.
        observed_sequence: u64,
        /// Current cursor epoch.
        current_epoch: u64,
        /// Current cursor sequence.
        current_sequence: u64,
    },
    /// Lane was not present in the lane catalog mapping.
    #[error("lane {lane_id} is not present in the shard mapping")]
    UnknownLane {
        /// Lane identifier with no mapping.
        lane_id: LaneId,
    },
}

/// In-memory shard cursor index reconstructed from committed DA bundles.
#[derive(Debug, Default)]
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
        let allowed: BTreeSet<_> = self.mapping.values().map(ShardId::as_u32).collect();
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
                    observed_epoch: record.epoch,
                    observed_sequence: record.sequence,
                    current_epoch: cursor.epoch,
                    current_sequence: cursor.sequence,
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
        self.sync_mapping(lane_config);
        for record in &bundle.commitments {
            let shard_id = self.shard_id_for_lane(record.lane_id)?;
            self.advance(shard_id, record, block_height)?;
        }
        Ok(())
    }

    fn shard_id_for_lane(&self, lane_id: LaneId) -> Result<u32, DaShardCursorError> {
        self.mapping
            .get(&lane_id)
            .map(ShardId::as_u32)
            .ok_or(DaShardCursorError::UnknownLane { lane_id })
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
        if !path.exists() {
            return Ok(journal);
        }

        let bytes = fs::read(&path).map_err(|source| ShardCursorJournalError::Read {
            path: path.clone(),
            source,
        })?;

        let persisted: PersistedShardCursors =
            decode_from_bytes(&bytes).map_err(|source| ShardCursorJournalError::Decode {
                path: path.clone(),
                source,
            })?;

        if persisted.version != Self::JOURNAL_VERSION {
            return Err(ShardCursorJournalError::UnsupportedVersion {
                path,
                version: persisted.version,
            });
        }

        for entry in persisted.entries {
            let Some(expected) = journal.mapping.get(&entry.lane_id) else {
                warn!(
                    lane = %entry.lane_id.as_u32(),
                    shard = %entry.shard_id.as_u32(),
                    "dropping shard cursor: lane missing from catalog"
                );
                continue;
            };

            if expected != &entry.shard_id {
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

        Ok(journal)
    }

    /// Record cursors for every commitment in the bundle.
    ///
    /// # Errors
    ///
    /// Returns [`ShardCursorJournalError::UnknownLane`] when a commitment references an unmapped lane.
    pub fn record_bundle(
        &mut self,
        bundle: &DaCommitmentBundle,
    ) -> Result<(), ShardCursorJournalError> {
        for record in &bundle.commitments {
            self.record_commitment(record)?;
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

        fs::write(&self.path, bytes).map_err(|source| ShardCursorJournalError::Write {
            path: self.path.clone(),
            source,
        })
    }

    fn upsert(&mut self, entry: LaneShardCursor) {
        let key = (entry.shard_id, entry.lane_id);
        let should_update = self
            .cursors
            .get(&key)
            .map_or(true, |existing| Self::should_advance(existing, &entry));
        if should_update {
            self.cursors.insert(key, entry);
        }
    }

    fn should_advance(current: &LaneShardCursor, candidate: &LaneShardCursor) -> bool {
        (candidate.epoch, candidate.sequence) > (current.epoch, current.sequence)
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, num::NonZeroU32, path::PathBuf};

    use super::*;
    use iroha_config::parameters::actual::LaneConfig;
    use iroha_crypto::{Hash, Signature};
    use iroha_data_model::{
        da::{
            commitment::{DaCommitmentBundle, DaCommitmentRecord, DaProofScheme, KzgCommitment, RetentionClass},
            types::{BlobDigest, StorageTicketId},
        },
        nexus::{
            DataSpaceId, LaneCatalog, LaneId, LaneConfig, LaneStorageProfile, LaneVisibility,
            ShardId,
        },
        sorafs::pin_registry::ManifestDigest,
    };
    use tempfile::tempdir;

    fn sample_record(lane_id: u32, epoch: u64, sequence: u64) -> DaCommitmentRecord {
        let lane_byte = u8::try_from(lane_id).expect("lane fits in u8 for test record");
        let epoch_byte = u8::try_from(epoch).expect("epoch fits in u8 for test record");
        let sequence_byte =
            u8::try_from(sequence).expect("sequence fits in u8 for test record");
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

    fn lane_config_with_mapping(lane_id: u32, shard_id: u32) -> LaneConfig {
        let mut metadata = BTreeMap::new();
        metadata.insert("da_shard_id".to_string(), shard_id.to_string());
        let lane_count = nonzero!(lane_id.saturating_add(1));
        let catalog = LaneCatalog::new(
            lane_count,
            vec![LaneConfig {
                id: LaneId::new(lane_id),
                dataspace_id: DataSpaceId::GLOBAL,
                alias: format!("lane{lane_id}"),
                metadata,
                ..LaneConfig::default()
            }],
        )
        .expect("lane catalog");

        LaneConfig::from_catalog(&catalog)
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
        assert!(matches!(
            err,
            DaShardCursorError::Regression { shard_id: 1, .. }
        ));
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
    fn journal_persists_and_recovers_cursors() {
        let dir = tempdir().expect("tempdir");
        let catalog = LaneCatalog::new(
            nonzero!(1_u32),
            vec![LaneConfig {
                id: LaneId::new(0),
                dataspace_id: DataSpaceId::GLOBAL,
                alias: "lane".to_string(),
                description: None,
                visibility: LaneVisibility::Public,
                lane_type: None,
                governance: None,
                settlement: None,
                storage: LaneStorageProfile::FullReplica,
                proof_scheme: Default::default(),
                metadata: Default::default(),
            }],
        )
        .expect("catalog");
        let lane_config = LaneConfig::from_catalog(&catalog);
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
                .record_commitment(&record)
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
            .record_commitment(&sample_record(1, 5, 6))
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
            .record_commitment(&sample_record(0, 1, 1))
            .expect("record");
        journal
            .record_commitment(&sample_record(0, 1, 0))
            .expect("record");

        let cursor = journal.cursor_for_lane(LaneId::new(0)).expect("cursor");
        assert_eq!((cursor.epoch, cursor.sequence), (1, 1));
    }

    #[test]
    fn journal_bundle_updates_multiple_lanes() {
        let catalog = LaneCatalog::new(
            NonZeroU32::new(2).expect("lane count"),
            vec![
                LaneConfig {
                    id: LaneId::new(0),
                    alias: "lane0".into(),
                    ..LaneConfig::default()
                },
                LaneConfig {
                    id: LaneId::new(1),
                    alias: "lane1".into(),
                    metadata: {
                        let mut map = BTreeMap::new();
                        map.insert("da_shard_id".to_string(), "5".to_string());
                        map
                    },
                    ..LaneConfig::default()
                },
            ],
        )
        .expect("catalog");
        let config = LaneConfig::from_catalog(&catalog);
        let mut journal = DaShardCursorJournal::new(&config, PathBuf::from("unused"));

        let bundle = DaCommitmentBundle::new(vec![sample_record(0, 0, 1), sample_record(1, 2, 3)]);
        journal.record_bundle(&bundle).expect("record bundle");

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
            vec![LaneConfig {
                id: LaneId::new(0),
                dataspace_id: DataSpaceId::GLOBAL,
                alias: "lane0".into(),
                ..LaneConfig::default()
            }],
        )
        .expect("catalog");
        let config = LaneConfig::from_catalog(&catalog);
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
