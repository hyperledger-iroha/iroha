//! DA commitment spool helpers.
//!
//! This module reads Torii-emitted `da-commitment-*.norito` files from the
//! configured spool directory and assembles a deterministic bundle ready to
//! embed into a block payload.

use std::path::{Path, PathBuf};

use iroha_data_model::da::commitment::{DaCommitmentBundle, DaCommitmentRecord};
use iroha_logger::warn;
use norito::decode_from_bytes;
use thiserror::Error;

use crate::da::commitment_store::DaCommitmentStore;

/// Errors encountered while loading DA commitment artefacts from disk.
#[derive(Debug, Error)]
pub enum DaSpoolError {
    /// Directory does not exist or cannot be read.
    #[error("failed to read DA spool directory `{path}`: {source}")]
    ReadDir {
        /// Path that failed.
        path: PathBuf,
        /// Source error from the filesystem.
        #[source]
        source: std::io::Error,
    },
    /// Failed to read a commitment file.
    #[error("failed to read DA commitment `{path}`: {source}")]
    ReadFile {
        /// Path that failed.
        path: PathBuf,
        /// Source error from the filesystem.
        #[source]
        source: std::io::Error,
    },
    /// Failed to decode a commitment file.
    #[error("failed to decode DA commitment `{path}`: {source}")]
    Decode {
        /// Path that failed.
        path: PathBuf,
        /// Source decode error.
        #[source]
        source: norito::core::Error,
    },
}

/// Load all DA commitment records from the spool directory.
///
/// Files are filtered by filename (`da-commitment-*.norito`), decoded using
/// Norito, sorted deterministically, and wrapped into a [`DaCommitmentBundle`].
/// When the directory is missing or no records are present, this returns
/// `Ok(None)`.
///
/// # Errors
///
/// Returns a [`DaSpoolError`] if the spool directory cannot be read. Individual
/// commitment files that fail to read or decode are skipped with a warning.
pub fn load_commitment_bundle(
    spool_dir: &Path,
) -> Result<Option<DaCommitmentBundle>, DaSpoolError> {
    if !spool_dir.exists() {
        return Ok(None);
    }

    let mut records = Vec::new();
    let dir_entries = std::fs::read_dir(spool_dir).map_err(|source| DaSpoolError::ReadDir {
        path: spool_dir.to_path_buf(),
        source,
    })?;

    for entry in dir_entries {
        let entry = match entry {
            Ok(value) => value,
            Err(source) => {
                warn!(?source, "failed to read DA spool entry");
                continue;
            }
        };
        let path = entry.path();
        if !is_da_commitment_file(&path) {
            continue;
        }

        let bytes = match std::fs::read(&path) {
            Ok(buf) => buf,
            Err(source) => {
                warn!(
                    ?source,
                    path = %path.display(),
                    "failed to read DA commitment file; skipping"
                );
                continue;
            }
        };

        match decode_from_bytes::<DaCommitmentRecord>(&bytes) {
            Ok(record) => records.push(record),
            Err(source) => {
                warn!(
                    ?source,
                    path = %path.display(),
                    "failed to decode DA commitment file; skipping"
                );
                continue;
            }
        }
    }

    if records.is_empty() {
        return Ok(None);
    }

    records.sort();
    Ok(Some(DaCommitmentBundle::new(records)))
}

/// Load commitments from disk and build an in-memory index for query paths.
///
/// Returns an empty store if no commitments are present. See
/// [`load_commitment_bundle`] for error semantics.
///
/// # Errors
///
/// Propagates [`DaSpoolError`] when the spool directory cannot be read.
pub fn load_commitment_store(spool_dir: &Path) -> Result<DaCommitmentStore, DaSpoolError> {
    match load_commitment_bundle(spool_dir)? {
        Some(bundle) => Ok(DaCommitmentStore::from_bundle(&bundle.commitments)),
        None => Ok(DaCommitmentStore::default()),
    }
}

fn is_da_commitment_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("da-commitment-") && name.ends_with(".norito"))
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Hash, Signature};
    use iroha_data_model::{
        da::{
            commitment::{DaCommitmentRecord, DaProofScheme, KzgCommitment, RetentionClass},
            types::{BlobDigest, StorageTicketId},
        },
        nexus::LaneId,
        sorafs::pin_registry::ManifestDigest,
    };
    use norito::to_bytes;
    use tempfile::tempdir;

    use super::*;

    fn sample_record(lane: u32, seq: u64) -> DaCommitmentRecord {
        DaCommitmentRecord::new(
            LaneId::new(lane),
            1,
            seq,
            BlobDigest::new([0x11; 32]),
            ManifestDigest::new([0x22; 32]),
            DaProofScheme::MerkleSha256,
            Hash::prehashed([0x33; 32]),
            Some(KzgCommitment::new([0x44; 48])),
            Some(Hash::prehashed([0x55; 32])),
            RetentionClass::default(),
            StorageTicketId::new([0x66; 32]),
            Signature::from_bytes(&[0x77; 64]),
        )
    }

    #[test]
    fn returns_none_for_missing_dir() {
        let missing = PathBuf::from("this-path-should-not-exist-da-spool");
        assert!(load_commitment_bundle(&missing).unwrap().is_none());
    }

    #[test]
    fn loads_and_sorts_commitments() {
        let dir = tempdir().expect("tempdir");
        let record_a = sample_record(2, 5);
        let record_b = sample_record(1, 1);

        let bytes_a = to_bytes(&record_a).expect("encode record a");
        let bytes_b = to_bytes(&record_b).expect("encode record b");

        let file_a = dir
            .path()
            .join("da-commitment-00000002-0000000000000001-0000000000000005-a.norito");
        let file_b = dir
            .path()
            .join("da-commitment-00000001-0000000000000001-0000000000000001-b.norito");

        std::fs::write(file_a, bytes_a).expect("write a");
        std::fs::write(file_b, bytes_b).expect("write b");

        let bundle = load_commitment_bundle(dir.path())
            .expect("load bundle")
            .expect("bundle present");

        assert_eq!(bundle.commitments.len(), 2);
        // Sorted by lane then sequence, so record_b should come first.
        assert_eq!(bundle.commitments[0].lane_id, LaneId::new(1));
        assert_eq!(bundle.commitments[0].sequence, 1);
    }

    #[test]
    fn commitment_store_builds_from_spool() {
        let dir = tempdir().expect("tempdir");
        let record = sample_record(1, 1);
        let path = dir
            .path()
            .join("da-commitment-00000001-0000000000000001-0000000000000001.norito");
        let bytes = to_bytes(&record).expect("encode");
        std::fs::write(&path, bytes).expect("write");

        let store = load_commitment_store(dir.path()).expect("load store");
        let fetched = store
            .get_by_lane_epoch_sequence(1, 1, 1)
            .expect("commitment present");
        assert_eq!(fetched.commitment.storage_ticket, record.storage_ticket);
    }

    #[test]
    fn commitment_bundle_skips_corrupt_entries() {
        let dir = tempdir().expect("tempdir");
        let record = sample_record(1, 1);
        let bytes = to_bytes(&record).expect("encode record");

        let valid_path = dir
            .path()
            .join("da-commitment-00000001-0000000000000001-0000000000000001-ok.norito");
        let corrupt_path = dir
            .path()
            .join("da-commitment-00000001-0000000000000001-0000000000000002-bad.norito");

        std::fs::write(valid_path, bytes).expect("write valid");
        std::fs::write(corrupt_path, b"corrupt").expect("write corrupt");

        let bundle = load_commitment_bundle(dir.path())
            .expect("load bundle")
            .expect("bundle present");
        assert_eq!(bundle.commitments.len(), 1);
        assert_eq!(bundle.commitments[0], record);
    }
}
