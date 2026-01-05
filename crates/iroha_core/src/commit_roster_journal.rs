//! Durable commit-roster journal persisted alongside the block store.
//!
//! This journal keeps per-height commit certificates and validator set
//! checkpoints so block-sync consumers can rebuild validator rosters after a
//! restart without depending on in-memory status caches.

use std::{
    collections::{BTreeMap, btree_map::Entry},
    fs,
    io::Write,
    num::NonZeroUsize,
    path::{Path, PathBuf},
};

use iroha_crypto::HashOf;
use iroha_data_model::{
    block::BlockHeader,
    consensus::{CommitCertificate, ValidatorSetCheckpoint},
};
use iroha_logger::warn;
use norito::{
    codec::{Decode, Encode},
    decode_from_bytes, to_bytes,
};
use thiserror::Error;

use crate::sumeragi::stake_snapshot::CommitStakeSnapshot;

/// Persisted commit-roster journal payload.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
struct PersistedCommitRosters {
    /// Journal version for format control.
    version: u32,
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
    commit_certificate: CommitCertificate,
    /// Validator set checkpoint for the block.
    validator_checkpoint: ValidatorSetCheckpoint,
    /// Optional stake snapshot aligned to the validator set.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    stake_snapshot: Option<CommitStakeSnapshot>,
}

/// Errors returned when loading or persisting commit rosters.
#[derive(Debug, Error)]
pub enum CommitRosterJournalError {
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
}

/// Snapshot combining commit certificate and validator checkpoint for a block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitRosterSnapshot {
    /// Commit certificate for the block.
    pub commit_certificate: CommitCertificate,
    /// Validator set checkpoint for the block.
    pub validator_checkpoint: ValidatorSetCheckpoint,
    /// Optional stake snapshot aligned to the validator set.
    pub stake_snapshot: Option<CommitStakeSnapshot>,
}

/// Journal that records commit rosters derived from committed blocks.
#[derive(Debug)]
pub struct CommitRosterJournal {
    entries: BTreeMap<(u64, HashOf<BlockHeader>), CommitRosterSnapshot>,
    path: PathBuf,
    retention: NonZeroUsize,
}

impl CommitRosterJournal {
    /// Filename used to persist commit roster journals next to the block store.
    pub const JOURNAL_FILE: &'static str = "commit-rosters.norito";
    const JOURNAL_VERSION: u32 = 1;

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
        }
    }

    /// Load a journal from disk, preferring higher-view entries when duplicates exist.
    ///
    /// Missing files are treated as empty journals. Unsupported versions surface an error.
    ///
    /// # Errors
    ///
    /// Returns [`CommitRosterJournalError::Read`] or [`CommitRosterJournalError::Decode`] when
    /// persistence fails.
    pub fn load(
        path: impl Into<PathBuf>,
        retention: NonZeroUsize,
    ) -> Result<Self, CommitRosterJournalError> {
        let path = path.into();
        let mut journal = Self::new(path.clone(), retention);
        let tmp_path = path.with_extension("norito.tmp");
        if path.as_os_str().is_empty() {
            return Ok(journal);
        }

        let (persisted, read_path) = if path.exists() {
            match Self::load_persisted(&path) {
                Ok(persisted) => (persisted, path.clone()),
                Err(err) => {
                    if tmp_path.exists() {
                        match Self::load_persisted(&tmp_path) {
                            Ok(persisted) => (persisted, tmp_path.clone()),
                            Err(_) => return Err(err),
                        }
                    } else {
                        return Err(err);
                    }
                }
            }
        } else if tmp_path.exists() {
            (Self::load_persisted(&tmp_path)?, tmp_path.clone())
        } else {
            return Ok(journal);
        };

        for entry in persisted.entries {
            if entry.height != entry.commit_certificate.height
                || entry.block_hash != entry.commit_certificate.subject_block_hash
            {
                warn!(
                    height = entry.height,
                    block = %entry.block_hash,
                    cert_height = entry.commit_certificate.height,
                    cert_block = %entry.commit_certificate.subject_block_hash,
                    "dropping commit roster entry with mismatched commit certificate metadata"
                );
                continue;
            }
            if entry.height != entry.validator_checkpoint.height
                || entry.block_hash != entry.validator_checkpoint.block_hash
            {
                warn!(
                    height = entry.height,
                    block = %entry.block_hash,
                    checkpoint_height = entry.validator_checkpoint.height,
                    checkpoint_block = %entry.validator_checkpoint.block_hash,
                    "dropping commit roster entry with mismatched checkpoint metadata"
                );
                continue;
            }
            journal.upsert(
                entry.commit_certificate,
                entry.validator_checkpoint,
                entry.stake_snapshot,
            );
        }

        if read_path != path {
            Self::promote_temp_journal(&read_path, &path);
        }

        journal.enforce_retention();
        Ok(journal)
    }

    fn load_persisted(path: &Path) -> Result<PersistedCommitRosters, CommitRosterJournalError> {
        let bytes = fs::read(path).map_err(|source| CommitRosterJournalError::Read {
            path: path.to_path_buf(),
            source,
        })?;
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
        Ok(persisted)
    }

    fn promote_temp_journal(from: &Path, to: &Path) {
        if let Err(err) = fs::rename(from, to) {
            if to.exists() {
                if let Err(remove_err) = fs::remove_file(to) {
                    warn!(
                        ?remove_err,
                        path = %to.display(),
                        "failed to remove commit roster journal before promotion"
                    );
                    return;
                }
                if let Err(err) = fs::rename(from, to) {
                    warn!(
                        ?err,
                        from = %from.display(),
                        to = %to.display(),
                        "failed to promote commit roster journal temp file after removal"
                    );
                    return;
                }
            } else {
                warn!(
                    ?err,
                    from = %from.display(),
                    to = %to.display(),
                    "failed to promote commit roster journal temp file"
                );
                return;
            }
        }
        if let Some(parent) = to.parent() {
            if let Err(err) = sync_dir(parent) {
                warn!(
                    ?err,
                    path = %parent.display(),
                    "failed to sync commit roster journal parent after temp promotion"
                );
            }
        }
    }

    /// Upsert a commit roster entry, replacing older views for the same block hash/height.
    pub fn upsert(
        &mut self,
        commit_certificate: CommitCertificate,
        validator_checkpoint: ValidatorSetCheckpoint,
        stake_snapshot: Option<CommitStakeSnapshot>,
    ) {
        let key = (
            commit_certificate.height,
            commit_certificate.subject_block_hash,
        );
        match self.entries.entry(key) {
            Entry::Occupied(mut entry) => {
                if entry.get().commit_certificate.view <= commit_certificate.view {
                    let stake_snapshot =
                        stake_snapshot.or_else(|| entry.get().stake_snapshot.clone());
                    entry.insert(CommitRosterSnapshot {
                        commit_certificate,
                        validator_checkpoint,
                        stake_snapshot,
                    });
                }
            }
            Entry::Vacant(entry) => {
                entry.insert(CommitRosterSnapshot {
                    commit_certificate,
                    validator_checkpoint,
                    stake_snapshot,
                });
            }
        }
        self.enforce_retention();
    }

    /// Persist the journal to disk.
    ///
    /// # Errors
    ///
    /// Returns [`CommitRosterJournalError::Write`] when the journal cannot be written or
    /// [`CommitRosterJournalError::Encode`] when encoding fails.
    pub fn persist(&mut self) -> Result<(), CommitRosterJournalError> {
        if self.path.as_os_str().is_empty() {
            return Ok(());
        }
        // Ensure persisted payload honours the configured retention window.
        self.enforce_retention();
        let payload = PersistedCommitRosters {
            version: Self::JOURNAL_VERSION,
            entries: self
                .entries
                .iter()
                .map(|((height, block_hash), snapshot)| CommitRosterRecord {
                    height: *height,
                    block_hash: *block_hash,
                    commit_certificate: snapshot.commit_certificate.clone(),
                    validator_checkpoint: snapshot.validator_checkpoint.clone(),
                    stake_snapshot: snapshot.stake_snapshot.clone(),
                })
                .collect(),
        };
        let bytes = to_bytes(&payload).map_err(CommitRosterJournalError::Encode)?;
        if let Some(parent) = self.path.parent() {
            if let Err(err) = fs::create_dir_all(parent) {
                return Err(CommitRosterJournalError::Write {
                    path: self.path.clone(),
                    source: err,
                });
            }
        }
        let tmp_path = self.path.with_extension("norito.tmp");
        {
            let mut file = fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&tmp_path)
                .map_err(|source| CommitRosterJournalError::Write {
                    path: tmp_path.clone(),
                    source,
                })?;
            file.write_all(&bytes)
                .and_then(|_| file.flush())
                .and_then(|_| file.sync_data())
                .map_err(|source| CommitRosterJournalError::Write {
                    path: tmp_path.clone(),
                    source,
                })?;
        }
        fs::rename(&tmp_path, &self.path).map_err(|source| CommitRosterJournalError::Write {
            path: self.path.clone(),
            source,
        })?;
        if let Some(parent) = self.path.parent() {
            sync_dir(parent).map_err(|source| CommitRosterJournalError::Write {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        Ok(())
    }

    /// Retrieve the snapshot for `height`/`block_hash` if present.
    #[must_use]
    pub fn get(
        &self,
        height: u64,
        block_hash: HashOf<BlockHeader>,
    ) -> Option<CommitRosterSnapshot> {
        self.entries.get(&(height, block_hash)).cloned()
    }

    /// Return all stored snapshots in height/hash order.
    #[must_use]
    pub fn snapshots(&self) -> Vec<CommitRosterSnapshot> {
        self.entries.values().cloned().collect()
    }

    fn enforce_retention(&mut self) {
        while self.entries.len() > self.retention.get() {
            if let Some(oldest) = self.entries.keys().next().copied() {
                self.entries.remove(&oldest);
            } else {
                break;
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
    use std::{num::NonZeroU64, path::Path};

    use iroha_crypto::{Algorithm, HashOf, KeyPair};
    use iroha_data_model::{
        block::BlockHeader, consensus::VALIDATOR_SET_HASH_VERSION_V1, peer::PeerId,
    };
    use iroha_primitives::numeric::Numeric;
    use tempfile::tempdir;

    use super::*;
    use crate::sumeragi::{
        consensus::{CommitAggregate, PERMISSIONED_TAG, Phase},
        stake_snapshot::CommitStakeSnapshotEntry,
    };

    fn sample_cert(view: u64) -> (CommitCertificate, ValidatorSetCheckpoint) {
        cert_with_height(2, view)
    }

    fn cert_with_height(height: u64, view: u64) -> (CommitCertificate, ValidatorSetCheckpoint) {
        let kp = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let peer = PeerId::new(kp.public_key().clone());
        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("non-zero"),
            None,
            None,
            None,
            0,
            0,
        );
        let block_hash = header.hash();
        let roster = vec![peer];
        let signers_bitmap = vec![0b0000_0001];
        let bls_aggregate_signature = vec![0xAB; 96];
        let cert = CommitCertificate {
            phase: Phase::Commit,
            subject_block_hash: block_hash,
            height,
            view,
            epoch: 0,
            mode_tag: PERMISSIONED_TAG.to_string(),
            highest_cert: None,
            validator_set_hash: HashOf::new(&roster),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set: roster.clone(),
            aggregate: CommitAggregate {
                signers_bitmap: signers_bitmap.clone(),
                bls_aggregate_signature: bls_aggregate_signature.clone(),
            },
        };
        let checkpoint = ValidatorSetCheckpoint::new(
            height,
            block_hash,
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
                    stake: Numeric::new(10, 0),
                })
                .collect(),
        }
    }

    fn retention(limit: usize) -> NonZeroUsize {
        NonZeroUsize::new(limit).expect("non-zero retention")
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
                commit_certificate: cert,
                validator_checkpoint: checkpoint,
                stake_snapshot: None,
            }
        );
    }

    #[test]
    fn journal_prefers_higher_view_for_same_block() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (low_view_cert, checkpoint) = sample_cert(1);
        let (high_view_cert, _) = sample_cert(3);
        let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
        journal.upsert(low_view_cert, checkpoint.clone(), None);
        journal.upsert(high_view_cert.clone(), checkpoint, None);
        journal.persist().expect("persist");

        let loaded = CommitRosterJournal::load(path, retention(4)).expect("load");
        let snapshots = loaded.snapshots();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].commit_certificate.view, high_view_cert.view);
    }

    #[test]
    fn journal_loads_v1_payload_without_stake_snapshot() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (cert, checkpoint) = sample_cert(1);
        let payload = PersistedCommitRosters {
            version: 1,
            entries: vec![CommitRosterRecord {
                height: cert.height,
                block_hash: cert.subject_block_hash,
                commit_certificate: cert.clone(),
                validator_checkpoint: checkpoint.clone(),
                stake_snapshot: None,
            }],
        };
        let bytes = norito::to_bytes(&payload).expect("encode payload");
        std::fs::write(&path, bytes).expect("write payload");

        let loaded = CommitRosterJournal::load(path, retention(4)).expect("load");
        let snapshots = loaded.snapshots();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(
            snapshots[0],
            CommitRosterSnapshot {
                commit_certificate: cert,
                validator_checkpoint: checkpoint,
                stake_snapshot: None,
            }
        );
    }

    #[test]
    fn journal_roundtrips_stake_snapshot() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (cert, checkpoint) = sample_cert(1);
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
    fn journal_loads_from_temp_when_main_missing() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let tmp_path = path.with_extension("norito.tmp");
        let (cert, checkpoint) = sample_cert(1);
        let payload = PersistedCommitRosters {
            version: 1,
            entries: vec![CommitRosterRecord {
                height: cert.height,
                block_hash: cert.subject_block_hash,
                commit_certificate: cert.clone(),
                validator_checkpoint: checkpoint.clone(),
                stake_snapshot: None,
            }],
        };
        let bytes = norito::to_bytes(&payload).expect("encode payload");
        std::fs::write(&tmp_path, bytes).expect("write temp payload");

        let loaded = CommitRosterJournal::load(path.clone(), retention(4)).expect("load");
        let snapshots = loaded.snapshots();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(
            snapshots[0],
            CommitRosterSnapshot {
                commit_certificate: cert,
                validator_checkpoint: checkpoint,
                stake_snapshot: None,
            }
        );
        assert!(path.exists(), "temp journal should be promoted");
        assert!(
            !tmp_path.exists(),
            "temp journal should be removed after promotion"
        );
    }

    #[test]
    fn journal_loads_from_temp_when_main_corrupted() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let tmp_path = path.with_extension("norito.tmp");
        let (cert, checkpoint) = sample_cert(1);
        let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
        journal.upsert(cert.clone(), checkpoint.clone(), None);
        journal.persist().expect("persist");

        std::fs::rename(&path, &tmp_path).expect("move journal to temp");
        std::fs::write(&path, b"corrupted").expect("write corrupted journal");

        let loaded = CommitRosterJournal::load(path.clone(), retention(4)).expect("load");
        let snapshots = loaded.snapshots();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(
            snapshots[0],
            CommitRosterSnapshot {
                commit_certificate: cert,
                validator_checkpoint: checkpoint,
                stake_snapshot: None,
            }
        );
        assert!(path.exists(), "temp journal should be promoted");
        assert!(
            !tmp_path.exists(),
            "temp journal should be removed after promotion"
        );
    }

    #[test]
    fn journal_rejects_unsupported_version() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let payload = PersistedCommitRosters {
            version: 2,
            entries: Vec::new(),
        };
        let bytes = norito::to_bytes(&payload).expect("encode payload");
        std::fs::write(&path, bytes).expect("write payload");

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
        assert_eq!(found.commit_certificate, cert);
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
        for height in 1..=3 {
            let (cert, checkpoint) = cert_with_height(height, 0);
            journal.upsert(cert, checkpoint, None);
        }
        let snapshots = journal.snapshots();
        let heights: Vec<_> = snapshots
            .iter()
            .map(|snapshot| snapshot.commit_certificate.height)
            .collect();
        assert_eq!(heights, vec![2, 3]);

        journal.persist().expect("persist");
        let reloaded = CommitRosterJournal::load(path, retention(2)).expect("load");
        let reloaded_heights: Vec<_> = reloaded
            .snapshots()
            .into_iter()
            .map(|snapshot| snapshot.commit_certificate.height)
            .collect();
        assert_eq!(reloaded_heights, vec![2, 3]);
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
