//! Durable journal for the latest query projection checkpoint descriptor.
//!
//! Query aggregate responses already expose a durable indexed-height marker.
//! This companion journal stores the latest DA-backed projection checkpoint
//! metadata so future rebuild workers can recover the most recent checkpoint
//! without scanning external DA manifests ad hoc.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use iroha_logger::warn;
use norito::{decode_from_bytes, to_bytes};
use thiserror::Error;

use crate::query::projection_checkpoint::QueryProjectionCheckpoint;

/// Errors returned when loading or persisting the query projection checkpoint journal.
#[derive(Debug, Error)]
pub enum QueryProjectionCheckpointJournalError {
    /// Failed to read the persisted journal.
    #[error("failed to read query projection checkpoint journal {path}: {source}")]
    Read {
        /// Path that failed.
        path: PathBuf,
        /// Source error.
        #[source]
        source: std::io::Error,
    },
    /// Failed to decode the persisted journal.
    #[error("failed to decode query projection checkpoint journal {path}: {source}")]
    Decode {
        /// Path that failed.
        path: PathBuf,
        /// Source decode error.
        #[source]
        source: norito::core::Error,
    },
    /// Failed to write the journal to disk.
    #[error("failed to persist query projection checkpoint journal {path}: {source}")]
    Write {
        /// Path that failed.
        path: PathBuf,
        /// Source error.
        #[source]
        source: std::io::Error,
    },
    /// Failed to encode the journal payload.
    #[error("failed to encode query projection checkpoint journal: {0}")]
    Encode(#[source] norito::core::Error),
    /// Persisted journal uses an unsupported version.
    #[error("unsupported query projection checkpoint journal version {version} at {path}")]
    UnsupportedVersion {
        /// Path for the journal.
        path: PathBuf,
        /// Unsupported version encountered.
        version: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, norito::codec::Encode, norito::codec::Decode)]
struct PersistedQueryProjectionCheckpoint {
    version: u32,
    checkpoint: Option<QueryProjectionCheckpoint>,
}

/// Journal that records the latest durable query projection checkpoint descriptor.
#[derive(Debug, Clone)]
pub struct QueryProjectionCheckpointJournal {
    path: PathBuf,
    checkpoint: Option<QueryProjectionCheckpoint>,
}

impl QueryProjectionCheckpointJournal {
    /// Filename used to persist the latest query projection checkpoint next to the block store.
    pub const JOURNAL_FILE: &'static str = "query-projection-checkpoint.norito";
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

    /// Construct a fresh journal with no persisted checkpoint metadata.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            checkpoint: None,
        }
    }

    /// Load a journal from disk, preferring a valid temp file when present.
    ///
    /// Missing files are treated as an empty journal.
    ///
    /// # Errors
    ///
    /// Returns [`QueryProjectionCheckpointJournalError`] when persistence fails.
    pub fn load(path: impl Into<PathBuf>) -> Result<Self, QueryProjectionCheckpointJournalError> {
        let path = path.into();
        let mut journal = Self::new(path.clone());
        let tmp_path = Self::temp_path(&path);
        if path.as_os_str().is_empty() {
            return Ok(journal);
        }

        let main = if path.exists() {
            Some(Self::load_persisted(&path))
        } else {
            None
        };
        let tmp = if tmp_path.exists() {
            Some(Self::load_persisted(&tmp_path))
        } else {
            None
        };

        let (persisted, read_path) = match (tmp, main) {
            (None, None) => return Ok(journal),
            (Some(Ok(persisted)), _) => (persisted, tmp_path.clone()),
            (Some(Err(tmp_err)), None) => return Err(tmp_err),
            (Some(Err(_)) | None, Some(Ok(persisted))) => (persisted, path.clone()),
            (None | Some(Err(_)), Some(Err(err))) => return Err(err),
        };

        journal.checkpoint = persisted.checkpoint;
        if read_path != path {
            Self::promote_temp_journal(&read_path, &path);
        }
        Ok(journal)
    }

    fn load_persisted(
        path: &Path,
    ) -> Result<PersistedQueryProjectionCheckpoint, QueryProjectionCheckpointJournalError> {
        let bytes =
            fs::read(path).map_err(|source| QueryProjectionCheckpointJournalError::Read {
                path: path.to_path_buf(),
                source,
            })?;
        let persisted: PersistedQueryProjectionCheckpoint =
            decode_from_bytes(&bytes).map_err(|source| {
                QueryProjectionCheckpointJournalError::Decode {
                    path: path.to_path_buf(),
                    source,
                }
            })?;
        if persisted.version != Self::JOURNAL_VERSION {
            return Err(QueryProjectionCheckpointJournalError::UnsupportedVersion {
                path: path.to_path_buf(),
                version: persisted.version,
            });
        }
        Ok(persisted)
    }

    fn temp_path(path: &Path) -> PathBuf {
        path.with_extension("norito.tmp")
    }

    fn promote_temp_journal(from: &Path, to: &Path) {
        if let Err(err) = fs::rename(from, to) {
            if to.exists() {
                if let Err(remove_err) = fs::remove_file(to) {
                    warn!(
                        ?remove_err,
                        path = %to.display(),
                        "failed to remove query projection checkpoint journal before promotion"
                    );
                    return;
                }
                if let Err(err) = fs::rename(from, to) {
                    warn!(
                        ?err,
                        from = %from.display(),
                        to = %to.display(),
                        "failed to promote query projection checkpoint journal temp file after removal"
                    );
                    return;
                }
            } else {
                warn!(
                    ?err,
                    from = %from.display(),
                    to = %to.display(),
                    "failed to promote query projection checkpoint journal temp file"
                );
                return;
            }
        }

        if let Some(parent) = to.parent() {
            if let Ok(dir) = fs::File::open(parent) {
                let _ = dir.sync_all();
            }
        }
    }

    /// Return the current in-memory checkpoint descriptor snapshot.
    #[must_use]
    pub fn snapshot(&self) -> Option<QueryProjectionCheckpoint> {
        self.checkpoint.clone()
    }

    /// Update the latest checkpoint descriptor tracked by this journal.
    pub fn set_latest(&mut self, checkpoint: Option<QueryProjectionCheckpoint>) {
        self.checkpoint = checkpoint;
    }

    /// Persist the journal atomically to disk.
    ///
    /// # Errors
    ///
    /// Returns [`QueryProjectionCheckpointJournalError`] when encoding or writing fails.
    pub fn persist(&self) -> Result<(), QueryProjectionCheckpointJournalError> {
        if self.path.as_os_str().is_empty() {
            return Ok(());
        }

        let payload = PersistedQueryProjectionCheckpoint {
            version: Self::JOURNAL_VERSION,
            checkpoint: self.checkpoint.clone(),
        };
        let bytes = to_bytes(&payload).map_err(QueryProjectionCheckpointJournalError::Encode)?;
        let tmp_path = Self::temp_path(&self.path);

        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|source| {
                QueryProjectionCheckpointJournalError::Write {
                    path: self.path.clone(),
                    source,
                }
            })?;
        }

        {
            let mut file = fs::File::create(&tmp_path).map_err(|source| {
                QueryProjectionCheckpointJournalError::Write {
                    path: tmp_path.clone(),
                    source,
                }
            })?;
            file.write_all(&bytes).map_err(|source| {
                QueryProjectionCheckpointJournalError::Write {
                    path: tmp_path.clone(),
                    source,
                }
            })?;
            file.sync_all()
                .map_err(|source| QueryProjectionCheckpointJournalError::Write {
                    path: tmp_path.clone(),
                    source,
                })?;
        }

        fs::rename(&tmp_path, &self.path).map_err(|source| {
            QueryProjectionCheckpointJournalError::Write {
                path: self.path.clone(),
                source,
            }
        })?;

        if let Some(parent) = self.path.parent() {
            let dir = fs::File::open(parent).map_err(|source| {
                QueryProjectionCheckpointJournalError::Write {
                    path: parent.to_path_buf(),
                    source,
                }
            })?;
            dir.sync_all()
                .map_err(|source| QueryProjectionCheckpointJournalError::Write {
                    path: parent.to_path_buf(),
                    source,
                })?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{
        block::BlockHeader,
        da::types::{BlobDigest, StorageTicketId},
    };

    use super::*;
    use crate::query::projection_checkpoint::{
        QueryProjectionCheckpoint, QueryProjectionCheckpointShard, QueryProjectionResourceKind,
    };

    fn sample_hash(byte: u8) -> HashOf<BlockHeader> {
        HashOf::from_untyped_unchecked(Hash::new([byte; Hash::LENGTH]))
    }

    fn sample_digest(byte: u8) -> BlobDigest {
        BlobDigest::new([byte; 32])
    }

    fn sample_ticket(byte: u8) -> StorageTicketId {
        StorageTicketId::new([byte; 32])
    }

    fn sample_checkpoint() -> QueryProjectionCheckpoint {
        QueryProjectionCheckpoint {
            indexed_height: 17,
            indexed_block_hash: Some(sample_hash(0x41)),
            emitted_at_unix: 1_714_000_123,
            shards: vec![QueryProjectionCheckpointShard {
                resource: QueryProjectionResourceKind::AssetHolders,
                partition_id: 9,
                asset_definition_id: Some("pkr#paynet".to_string()),
                manifest_digest: sample_digest(0x11),
                storage_ticket: sample_ticket(0x22),
                blob_hash: sample_digest(0x33),
            }],
            ..QueryProjectionCheckpoint::default()
        }
    }

    #[test]
    fn load_missing_projection_checkpoint_journal_returns_empty_snapshot() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir
            .path()
            .join(QueryProjectionCheckpointJournal::JOURNAL_FILE);
        let journal = QueryProjectionCheckpointJournal::load(path).expect("load empty journal");
        assert!(journal.snapshot().is_none());
    }

    #[test]
    fn projection_checkpoint_journal_round_trips_latest_snapshot() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir
            .path()
            .join(QueryProjectionCheckpointJournal::JOURNAL_FILE);
        let mut journal = QueryProjectionCheckpointJournal::new(path.clone());
        journal.set_latest(Some(sample_checkpoint()));
        journal.persist().expect("persist journal");

        let loaded = QueryProjectionCheckpointJournal::load(path).expect("reload journal");
        assert_eq!(loaded.snapshot(), journal.snapshot());
    }

    #[test]
    fn projection_checkpoint_journal_promotes_temp_file_on_load() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir
            .path()
            .join(QueryProjectionCheckpointJournal::JOURNAL_FILE);
        let tmp_path = path.with_extension("norito.tmp");

        let payload = PersistedQueryProjectionCheckpoint {
            version: QueryProjectionCheckpointJournal::JOURNAL_VERSION,
            checkpoint: Some(sample_checkpoint()),
        };
        let bytes = to_bytes(&payload).expect("encode temp journal");
        fs::write(&tmp_path, bytes).expect("write temp journal");

        let loaded = QueryProjectionCheckpointJournal::load(path.clone()).expect("load journal");
        assert_eq!(loaded.snapshot(), Some(sample_checkpoint()));
        assert!(path.exists(), "temp journal should be promoted");
        assert!(!tmp_path.exists(), "temp journal should be consumed");
    }
}
