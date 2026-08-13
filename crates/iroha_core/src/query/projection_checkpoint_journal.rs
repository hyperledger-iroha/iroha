//! Durable journal for the latest query projection checkpoint descriptor.
//!
//! Query aggregate responses already expose a durable indexed-height marker.
//! This companion journal stores the latest DA-backed projection checkpoint
//! metadata so future rebuild workers can recover the most recent checkpoint
//! without scanning external DA manifests ad hoc. Recovery handles temp and
//! main candidates sequentially and applies fixed byte/allocation/shard limits
//! before retaining a decoded checkpoint.
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};
use iroha_logger::warn;
use norito::{DecodeLimits, decode_from_bytes_with_limits, to_bytes};
use thiserror::Error;
use crate::query::projection_checkpoint::{
    QUERY_PROJECTION_CHECKPOINT_MAX_ASSET_DEFINITION_ID_BYTES,
    QUERY_PROJECTION_CHECKPOINT_MAX_SHARDS,
    QUERY_PROJECTION_CHECKPOINT_MAX_TOTAL_ASSET_DEFINITION_ID_BYTES, QueryProjectionCheckpoint,
};
/// Maximum encoded bytes retained or decoded for one first-release checkpoint journal.
const QUERY_PROJECTION_CHECKPOINT_JOURNAL_MAX_BYTES: usize = 16 * 1024 * 1024;
/// Maximum aggregate allocation permitted while decoding one checkpoint journal.
const QUERY_PROJECTION_CHECKPOINT_JOURNAL_MAX_DECODE_ALLOCATED_BYTES: usize = 32 * 1024 * 1024;
const QUERY_PROJECTION_CHECKPOINT_JOURNAL_MAX_DECODE_DEPTH: usize = 32;
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
    /// The journal or its checkpoint exceeds a first-release resource ceiling.
    #[error("query projection checkpoint journal {path} exceeds resource limits: {reason}")]
    ResourceLimit {
        /// Path for the journal.
        path: PathBuf,
        /// Stable resource-limit failure.
        reason: String,
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
        // Recover candidates sequentially. A valid temp file wins without opening or decoding the
        // main file, and an invalid temp value is dropped before the main candidate is loaded.
        // This prevents recovery from retaining two maximum-size decoded checkpoints at once.
        let (persisted, read_path) = match Self::load_persisted_if_present(&tmp_path) {
            Ok(Some(persisted)) => (persisted, tmp_path.clone()),
            Ok(None) => match Self::load_persisted_if_present(&path)? {
                Some(persisted) => (persisted, path.clone()),
                None => return Ok(journal),
            },
            Err(tmp_error) => match Self::load_persisted_if_present(&path) {
                Ok(Some(persisted)) => (persisted, path.clone()),
                Ok(None) => return Err(tmp_error),
                Err(main_error) => return Err(main_error),
            },
        };
        journal.checkpoint = persisted.checkpoint;
        if read_path != path {
            Self::promote_temp_journal(&read_path, &path);
        }
        Ok(journal)
    }
    fn load_persisted_if_present(
        path: &Path,
    ) -> Result<Option<PersistedQueryProjectionCheckpoint>, QueryProjectionCheckpointJournalError>
    {
        let Some(bytes) =
            read_bounded_journal_file(path, QUERY_PROJECTION_CHECKPOINT_JOURNAL_MAX_BYTES)
                .map_err(|source| QueryProjectionCheckpointJournalError::Read {
                    path: path.to_path_buf(),
                    source,
                })?
        else {
            return Ok(None);
        };
        let persisted: PersistedQueryProjectionCheckpoint =
            decode_from_bytes_with_limits(&bytes, checkpoint_journal_decode_limits()).map_err(
                |source| QueryProjectionCheckpointJournalError::Decode {
                    path: path.to_path_buf(),
                    source,
                },
            )?;
        if persisted.version != Self::JOURNAL_VERSION {
            return Err(QueryProjectionCheckpointJournalError::UnsupportedVersion {
                path: path.to_path_buf(),
                version: persisted.version,
            });
        }
        validate_checkpoint_resource_bounds(persisted.checkpoint.as_ref()).map_err(|reason| {
            QueryProjectionCheckpointJournalError::ResourceLimit {
                path: path.to_path_buf(),
                reason,
            }
        })?;
        Ok(Some(persisted))
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
        validate_checkpoint_resource_bounds(self.checkpoint.as_ref()).map_err(|reason| {
            QueryProjectionCheckpointJournalError::ResourceLimit {
                path: self.path.clone(),
                reason,
            }
        })?;
        let payload = PersistedQueryProjectionCheckpoint {
            version: Self::JOURNAL_VERSION,
            checkpoint: self.checkpoint.clone(),
        };
        let bytes = to_bytes(&payload).map_err(QueryProjectionCheckpointJournalError::Encode)?;
        if bytes.len() > QUERY_PROJECTION_CHECKPOINT_JOURNAL_MAX_BYTES {
            return Err(QueryProjectionCheckpointJournalError::ResourceLimit {
                path: self.path.clone(),
                reason: format!(
                    "encoded journal is {} bytes (maximum {})",
                    bytes.len(),
                    QUERY_PROJECTION_CHECKPOINT_JOURNAL_MAX_BYTES
                ),
            });
        }
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
fn checkpoint_journal_decode_limits() -> DecodeLimits {
    DecodeLimits::new(
        QUERY_PROJECTION_CHECKPOINT_MAX_SHARDS,
        QUERY_PROJECTION_CHECKPOINT_JOURNAL_MAX_BYTES,
        QUERY_PROJECTION_CHECKPOINT_JOURNAL_MAX_BYTES,
        QUERY_PROJECTION_CHECKPOINT_JOURNAL_MAX_DECODE_ALLOCATED_BYTES,
        QUERY_PROJECTION_CHECKPOINT_JOURNAL_MAX_DECODE_DEPTH,
    )
}
fn validate_checkpoint_resource_bounds(
    checkpoint: Option<&QueryProjectionCheckpoint>,
) -> Result<(), String> {
    let Some(checkpoint) = checkpoint else {
        return Ok(());
    };
    if checkpoint.shards.len() > QUERY_PROJECTION_CHECKPOINT_MAX_SHARDS {
        return Err(format!(
            "checkpoint contains {} shards (maximum {})",
            checkpoint.shards.len(),
            QUERY_PROJECTION_CHECKPOINT_MAX_SHARDS
        ));
    }
    if checkpoint.codec.0.len() > QUERY_PROJECTION_CHECKPOINT_MAX_ASSET_DEFINITION_ID_BYTES {
        return Err(format!(
            "checkpoint codec label is {} bytes (maximum {})",
            checkpoint.codec.0.len(),
            QUERY_PROJECTION_CHECKPOINT_MAX_ASSET_DEFINITION_ID_BYTES
        ));
    }
    let mut total_asset_definition_id_bytes = 0_usize;
    for shard in &checkpoint.shards {
        let bytes = shard.asset_definition_id.as_ref().map_or(0, String::len);
        if bytes > QUERY_PROJECTION_CHECKPOINT_MAX_ASSET_DEFINITION_ID_BYTES {
            return Err(format!(
                "asset-definition discriminator is {bytes} bytes (maximum {})",
                QUERY_PROJECTION_CHECKPOINT_MAX_ASSET_DEFINITION_ID_BYTES
            ));
        }
        total_asset_definition_id_bytes = total_asset_definition_id_bytes
            .checked_add(bytes)
            .ok_or_else(|| "asset-definition discriminator byte count overflowed".to_owned())?;
        if total_asset_definition_id_bytes
            > QUERY_PROJECTION_CHECKPOINT_MAX_TOTAL_ASSET_DEFINITION_ID_BYTES
        {
            return Err(format!(
                "asset-definition discriminators require {total_asset_definition_id_bytes} bytes (maximum {})",
                QUERY_PROJECTION_CHECKPOINT_MAX_TOTAL_ASSET_DEFINITION_ID_BYTES
            ));
        }
    }
    Ok(())
}
fn read_bounded_journal_file(path: &Path, max_bytes: usize) -> io::Result<Option<Vec<u8>>> {
    let path_before = match direct_journal_file_metadata(path, max_bytes) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        let nofollow = i32::try_from(rustix::fs::OFlags::NOFOLLOW.bits())
            .expect("NOFOLLOW flag bits fit the platform custom-flags type");
        options.custom_flags(nofollow);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let mut file = options.open(path)?;
    let opened_before = file.metadata()?;
    if !journal_file_metadata_unchanged(&path_before, &opened_before) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "query projection checkpoint journal identity changed while opening",
        ));
    }
    let mut bytes = Vec::with_capacity(
        usize::try_from(opened_before.len())
            .unwrap_or(max_bytes)
            .min(max_bytes),
    );
    Read::by_ref(&mut file)
        .take(
            u64::try_from(max_bytes)
                .unwrap_or(u64::MAX)
                .saturating_add(1),
        )
        .read_to_end(&mut bytes)?;
    let opened_after = file.metadata()?;
    let path_after = fs::symlink_metadata(path)?;
    if bytes.len() > max_bytes
        || !journal_file_metadata_unchanged(&opened_before, &opened_after)
        || !journal_file_metadata_unchanged(&opened_before, &path_after)
        || opened_after.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "query projection checkpoint journal changed or exceeded its hard byte limit while reading",
        ));
    }
    Ok(Some(bytes))
}
fn direct_journal_file_metadata(path: &Path, max_bytes: usize) -> io::Result<fs::Metadata> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || !journal_file_is_single_link(&metadata)
        || metadata.len() > u64::try_from(max_bytes).unwrap_or(u64::MAX)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "query projection checkpoint journal must be a bounded direct single-link regular file",
        ));
    }
    Ok(metadata)
}
fn journal_file_is_single_link(metadata: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        metadata.nlink() == 1
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        metadata.number_of_links() == Some(1)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = metadata;
        false
    }
}
#[cfg(unix)]
fn journal_file_metadata_unchanged(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.nlink() == 1
        && right.nlink() == 1
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}
#[cfg(windows)]
fn journal_file_metadata_unchanged(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    left.volume_serial_number().is_some()
        && left.file_index().is_some()
        && left.volume_serial_number() == right.volume_serial_number()
        && left.file_index() == right.file_index()
        && left.number_of_links() == Some(1)
        && right.number_of_links() == Some(1)
        && left.file_size() == right.file_size()
        && left.last_write_time() == right.last_write_time()
        && left.creation_time() == right.creation_time()
}
#[cfg(not(any(unix, windows)))]
fn journal_file_metadata_unchanged(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    false
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
        // A valid temp file must win without opening or allocating the oversized main candidate.
        fs::File::create(&path)
            .and_then(|file| {
                file.set_len(
                    u64::try_from(QUERY_PROJECTION_CHECKPOINT_JOURNAL_MAX_BYTES + 1)
                        .expect("journal limit fits u64"),
                )
            })
            .expect("write oversized main journal");
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
    #[test]
    fn projection_checkpoint_journal_rejects_oversized_file_before_decode() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir
            .path()
            .join(QueryProjectionCheckpointJournal::JOURNAL_FILE);
        fs::File::create(&path)
            .and_then(|file| {
                file.set_len(
                    u64::try_from(QUERY_PROJECTION_CHECKPOINT_JOURNAL_MAX_BYTES + 1)
                        .expect("journal limit fits u64"),
                )
            })
            .expect("write oversized journal");
        assert!(matches!(
            QueryProjectionCheckpointJournal::load(&path),
            Err(QueryProjectionCheckpointJournalError::Read { path: failed_path, source })
                if failed_path == path && source.kind() == io::ErrorKind::InvalidData
        ));
    }
    #[test]
    fn projection_checkpoint_journal_rejects_checkpoint_bounds_before_write() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir
            .path()
            .join(QueryProjectionCheckpointJournal::JOURNAL_FILE);
        let mut checkpoint = sample_checkpoint();
        checkpoint.shards[0].asset_definition_id =
            Some("x".repeat(QUERY_PROJECTION_CHECKPOINT_MAX_ASSET_DEFINITION_ID_BYTES + 1));
        let mut journal = QueryProjectionCheckpointJournal::new(path.clone());
        journal.set_latest(Some(checkpoint));
        assert!(matches!(
            journal.persist(),
            Err(QueryProjectionCheckpointJournalError::ResourceLimit { path: failed_path, .. })
                if failed_path == path
        ));
        assert!(!path.exists());
        assert!(!path.with_extension("norito.tmp").exists());
    }
}
