//! Durable query-index status persisted alongside the block store.
//!
//! The aggregate DSL currently executes against live state, but Torii still
//! needs a durable notion of the latest query snapshot that survived restarts.
//! This journal stores the latest indexed block height and hash under the Kura
//! root so aggregate responses can report a stable snapshot marker.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use iroha_crypto::HashOf;
use iroha_data_model::block::BlockHeader;
use iroha_logger::warn;
use norito::{
    codec::{Decode, Encode},
    decode_from_bytes, to_bytes,
};
use thiserror::Error;

/// Snapshot of the latest durable query index state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Encode, Decode)]
pub struct QueryIndexStatus {
    /// Latest block height covered by the query index.
    pub indexed_height: u64,
    /// Latest block hash covered by the query index.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub indexed_block_hash: Option<HashOf<BlockHeader>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
struct PersistedQueryIndexStatus {
    version: u32,
    status: QueryIndexStatus,
}

/// Errors returned when loading or persisting the query-index status journal.
#[derive(Debug, Error)]
pub enum QueryIndexJournalError {
    /// Failed to read the persisted journal.
    #[error("failed to read query index journal {path}: {source}")]
    Read {
        /// Path that failed.
        path: PathBuf,
        /// Source error.
        #[source]
        source: std::io::Error,
    },
    /// Failed to decode the persisted journal.
    #[error("failed to decode query index journal {path}: {source}")]
    Decode {
        /// Path that failed.
        path: PathBuf,
        /// Source decode error.
        #[source]
        source: norito::core::Error,
    },
    /// Failed to write the journal to disk.
    #[error("failed to persist query index journal {path}: {source}")]
    Write {
        /// Path that failed.
        path: PathBuf,
        /// Source error.
        #[source]
        source: std::io::Error,
    },
    /// Failed to encode the journal payload.
    #[error("failed to encode query index journal: {0}")]
    Encode(#[source] norito::core::Error),
    /// Persisted journal uses an unsupported version.
    #[error("unsupported query index journal version {version} at {path}")]
    UnsupportedVersion {
        /// Path for the journal.
        path: PathBuf,
        /// Unsupported version encountered.
        version: u32,
    },
}

/// Journal that records the latest query-index snapshot marker.
#[derive(Debug, Clone)]
pub struct QueryIndexJournal {
    path: PathBuf,
    status: QueryIndexStatus,
}

impl QueryIndexJournal {
    /// Filename used to persist query index status next to the block store.
    pub const JOURNAL_FILE: &'static str = "query-index-status.norito";
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

    /// Construct a fresh journal with no indexed state.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            status: QueryIndexStatus::default(),
        }
    }

    /// Load a journal from disk, preferring a valid temp file when present.
    ///
    /// Missing files are treated as an empty journal.
    ///
    /// # Errors
    ///
    /// Returns [`QueryIndexJournalError`] when persistence fails.
    pub fn load(path: impl Into<PathBuf>) -> Result<Self, QueryIndexJournalError> {
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

        journal.status = persisted.status;
        if read_path != path {
            Self::promote_temp_journal(&read_path, &path);
        }
        Ok(journal)
    }

    fn load_persisted(path: &Path) -> Result<PersistedQueryIndexStatus, QueryIndexJournalError> {
        let bytes = fs::read(path).map_err(|source| QueryIndexJournalError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let persisted: PersistedQueryIndexStatus =
            decode_from_bytes(&bytes).map_err(|source| QueryIndexJournalError::Decode {
                path: path.to_path_buf(),
                source,
            })?;
        if persisted.version != Self::JOURNAL_VERSION {
            return Err(QueryIndexJournalError::UnsupportedVersion {
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
                        "failed to remove query index journal before promotion"
                    );
                    return;
                }
                if let Err(err) = fs::rename(from, to) {
                    warn!(
                        ?err,
                        from = %from.display(),
                        to = %to.display(),
                        "failed to promote query index journal temp file after removal"
                    );
                    return;
                }
            } else {
                warn!(
                    ?err,
                    from = %from.display(),
                    to = %to.display(),
                    "failed to promote query index journal temp file"
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

    /// Return the current in-memory snapshot.
    #[must_use]
    pub fn snapshot(&self) -> QueryIndexStatus {
        self.status.clone()
    }

    /// Update the latest indexed height/hash tracked by this journal.
    pub fn set_latest(
        &mut self,
        indexed_height: u64,
        indexed_block_hash: Option<HashOf<BlockHeader>>,
    ) {
        self.status.indexed_height = indexed_height;
        self.status.indexed_block_hash = indexed_block_hash;
    }

    /// Persist the journal atomically to disk.
    ///
    /// # Errors
    ///
    /// Returns [`QueryIndexJournalError`] when encoding or writing fails.
    pub fn persist(&self) -> Result<(), QueryIndexJournalError> {
        if self.path.as_os_str().is_empty() {
            return Ok(());
        }

        let payload = PersistedQueryIndexStatus {
            version: Self::JOURNAL_VERSION,
            status: self.status.clone(),
        };
        let bytes = to_bytes(&payload).map_err(QueryIndexJournalError::Encode)?;
        let tmp_path = Self::temp_path(&self.path);

        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|source| QueryIndexJournalError::Write {
                path: self.path.clone(),
                source,
            })?;
        }

        {
            let mut file =
                fs::File::create(&tmp_path).map_err(|source| QueryIndexJournalError::Write {
                    path: tmp_path.clone(),
                    source,
                })?;
            file.write_all(&bytes)
                .map_err(|source| QueryIndexJournalError::Write {
                    path: tmp_path.clone(),
                    source,
                })?;
            file.sync_all()
                .map_err(|source| QueryIndexJournalError::Write {
                    path: tmp_path.clone(),
                    source,
                })?;
        }

        fs::rename(&tmp_path, &self.path).map_err(|source| QueryIndexJournalError::Write {
            path: self.path.clone(),
            source,
        })?;

        if let Some(parent) = self.path.parent() {
            let dir = fs::File::open(parent).map_err(|source| QueryIndexJournalError::Write {
                path: parent.to_path_buf(),
                source,
            })?;
            dir.sync_all()
                .map_err(|source| QueryIndexJournalError::Write {
                    path: parent.to_path_buf(),
                    source,
                })?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::Hash;

    fn sample_hash(byte: u8) -> HashOf<BlockHeader> {
        HashOf::from_untyped_unchecked(Hash::new([byte; Hash::LENGTH]))
    }

    #[test]
    fn load_missing_query_index_journal_returns_empty_snapshot() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join(QueryIndexJournal::JOURNAL_FILE);
        let journal = QueryIndexJournal::load(path).expect("load empty journal");
        assert_eq!(journal.snapshot(), QueryIndexStatus::default());
    }

    #[test]
    fn query_index_journal_round_trips_latest_snapshot() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join(QueryIndexJournal::JOURNAL_FILE);
        let mut journal = QueryIndexJournal::new(path.clone());
        journal.set_latest(42, Some(sample_hash(0xAB)));
        journal.persist().expect("persist journal");

        let loaded = QueryIndexJournal::load(path).expect("reload journal");
        assert_eq!(loaded.snapshot(), journal.snapshot());
    }

    #[test]
    fn query_index_journal_promotes_temp_file_on_load() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join(QueryIndexJournal::JOURNAL_FILE);
        let tmp_path = path.with_extension("norito.tmp");

        let payload = PersistedQueryIndexStatus {
            version: QueryIndexJournal::JOURNAL_VERSION,
            status: QueryIndexStatus {
                indexed_height: 7,
                indexed_block_hash: Some(sample_hash(0x11)),
            },
        };
        let bytes = to_bytes(&payload).expect("encode temp journal");
        fs::write(&tmp_path, bytes).expect("write temp journal");

        let loaded = QueryIndexJournal::load(path.clone()).expect("load journal");
        assert_eq!(loaded.snapshot().indexed_height, 7);
        assert!(path.exists(), "temp journal should be promoted");
        assert!(!tmp_path.exists(), "temp journal should be consumed");
    }
}
