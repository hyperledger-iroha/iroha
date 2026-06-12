//! DA commitment spool helpers.
//!
//! This module reads Torii-emitted `da-commitment-*.norito` files from the
//! configured spool directory and assembles a deterministic bundle ready to
//! embed into a block payload.

use std::path::{Path, PathBuf};

use iroha_data_model::{
    da::{
        commitment::{DaCommitmentBundle, DaCommitmentRecord},
        types::StorageTicketId,
    },
    nexus::LaneId,
};
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
    /// Commitment filename does not contain the expected lane/epoch/sequence/ticket/fingerprint tuple.
    #[error("malformed DA commitment filename at {path}")]
    MalformedFilename {
        /// Path that failed.
        path: PathBuf,
    },
    /// Commitment filename tuple does not match the decoded commitment body.
    #[error(
        "DA commitment filename tuple {filename_lane:?}/{filename_epoch}/{filename_sequence}/{filename_ticket:?} mismatches body {record_lane:?}/{record_epoch}/{record_sequence}/{record_ticket:?} at {path}"
    )]
    FilenameMismatch {
        /// Path that failed.
        path: PathBuf,
        /// Lane identifier parsed from the filename.
        filename_lane: LaneId,
        /// Epoch parsed from the filename.
        filename_epoch: u64,
        /// Sequence parsed from the filename.
        filename_sequence: u64,
        /// Storage ticket parsed from the filename.
        filename_ticket: StorageTicketId,
        /// Lane identifier decoded from the commitment body.
        record_lane: LaneId,
        /// Epoch decoded from the commitment body.
        record_epoch: u64,
        /// Sequence decoded from the commitment body.
        record_sequence: u64,
        /// Storage ticket decoded from the commitment body.
        record_ticket: StorageTicketId,
    },
}

/// Load all DA commitment records from the spool directory.
///
/// Files are filtered by filename (`da-commitment-*.norito`), checked against
/// their advertised lane/epoch/sequence/ticket tuple, decoded using Norito,
/// sorted deterministically, and wrapped into a [`DaCommitmentBundle`]. When
/// the directory is missing or no records are present, this returns `Ok(None)`.
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

        match decode_commitment_record(&bytes, &path) {
            Ok(record) => records.push(record),
            Err(err) => {
                warn!(
                    ?err,
                    path = %path.display(),
                    "failed to decode DA commitment file; skipping"
                );
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

#[derive(Clone, Copy)]
struct CommitmentFileKey {
    lane_id: LaneId,
    epoch: u64,
    sequence: u64,
    storage_ticket: StorageTicketId,
}

fn parse_commitment_file_key(path: &Path) -> Result<CommitmentFileKey, DaSpoolError> {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return Err(malformed_filename(path));
    };
    let Some(rest) = name
        .strip_prefix("da-commitment-")
        .and_then(|name| name.strip_suffix(".norito"))
    else {
        return Err(malformed_filename(path));
    };

    let mut fields = rest.split('-');
    let Some(lane_hex) = fields.next() else {
        return Err(malformed_filename(path));
    };
    let Some(epoch_hex) = fields.next() else {
        return Err(malformed_filename(path));
    };
    let Some(sequence_hex) = fields.next() else {
        return Err(malformed_filename(path));
    };
    let Some(ticket_hex) = fields.next() else {
        return Err(malformed_filename(path));
    };
    let Some(fingerprint_hex) = fields.next() else {
        return Err(malformed_filename(path));
    };
    if fields.next().is_some() {
        return Err(malformed_filename(path));
    }

    let lane_id = parse_fixed_hex_u32(lane_hex, 8, path).map(LaneId::new)?;
    let epoch = parse_fixed_hex_u64(epoch_hex, 16, path)?;
    let sequence = parse_fixed_hex_u64(sequence_hex, 16, path)?;
    let storage_ticket = StorageTicketId::new(parse_fixed_hex_32(ticket_hex, path)?);
    let _ = parse_fixed_hex_32(fingerprint_hex, path)?;

    Ok(CommitmentFileKey {
        lane_id,
        epoch,
        sequence,
        storage_ticket,
    })
}

fn parse_fixed_hex_u32(value: &str, width: usize, path: &Path) -> Result<u32, DaSpoolError> {
    if value.len() != width || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(malformed_filename(path));
    }
    u32::from_str_radix(value, 16).map_err(|_| malformed_filename(path))
}

fn parse_fixed_hex_u64(value: &str, width: usize, path: &Path) -> Result<u64, DaSpoolError> {
    if value.len() != width || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(malformed_filename(path));
    }
    u64::from_str_radix(value, 16).map_err(|_| malformed_filename(path))
}

fn parse_fixed_hex_32(value: &str, path: &Path) -> Result<[u8; 32], DaSpoolError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(malformed_filename(path));
    }
    let mut bytes = [0; 32];
    hex::decode_to_slice(value, &mut bytes).map_err(|_| malformed_filename(path))?;
    Ok(bytes)
}

fn malformed_filename(path: &Path) -> DaSpoolError {
    DaSpoolError::MalformedFilename {
        path: path.to_path_buf(),
    }
}

fn decode_commitment_record(data: &[u8], path: &Path) -> Result<DaCommitmentRecord, DaSpoolError> {
    let filename_key = parse_commitment_file_key(path)?;
    let record =
        decode_from_bytes::<DaCommitmentRecord>(data).map_err(|source| DaSpoolError::Decode {
            path: path.to_path_buf(),
            source,
        })?;
    if filename_key.lane_id != record.lane_id
        || filename_key.epoch != record.epoch
        || filename_key.sequence != record.sequence
        || filename_key.storage_ticket != record.storage_ticket
    {
        return Err(DaSpoolError::FilenameMismatch {
            path: path.to_path_buf(),
            filename_lane: filename_key.lane_id,
            filename_epoch: filename_key.epoch,
            filename_sequence: filename_key.sequence,
            filename_ticket: filename_key.storage_ticket,
            record_lane: record.lane_id,
            record_epoch: record.epoch,
            record_sequence: record.sequence,
            record_ticket: record.storage_ticket,
        });
    }

    Ok(record)
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

    fn commitment_file_name(record: &DaCommitmentRecord, fingerprint: [u8; 32]) -> String {
        format!(
            "da-commitment-{lane:08x}-{epoch:016x}-{sequence:016x}-{ticket}-{fingerprint}.norito",
            lane = record.lane_id.as_u32(),
            epoch = record.epoch,
            sequence = record.sequence,
            ticket = hex::encode(record.storage_ticket.as_ref()),
            fingerprint = hex::encode(fingerprint)
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

        let file_a = dir.path().join(commitment_file_name(&record_a, [0xaa; 32]));
        let file_b = dir.path().join(commitment_file_name(&record_b, [0xbb; 32]));

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
        let path = dir.path().join(commitment_file_name(&record, [0xcc; 32]));
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

        let valid_path = dir.path().join(commitment_file_name(&record, [0xdd; 32]));
        let mut corrupt_record = sample_record(1, 2);
        corrupt_record.storage_ticket = record.storage_ticket;
        let corrupt_path = dir
            .path()
            .join(commitment_file_name(&corrupt_record, [0xee; 32]));

        std::fs::write(valid_path, bytes).expect("write valid");
        std::fs::write(corrupt_path, b"corrupt").expect("write corrupt");

        let bundle = load_commitment_bundle(dir.path())
            .expect("load bundle")
            .expect("bundle present");
        assert_eq!(bundle.commitments.len(), 1);
        assert_eq!(bundle.commitments[0], record);
    }

    #[test]
    fn commitment_bundle_skips_malformed_filenames() {
        let dir = tempdir().expect("tempdir");
        let record = sample_record(1, 1);
        let bytes = to_bytes(&record).expect("encode record");
        let malformed_path = dir
            .path()
            .join("da-commitment-00000001-0000000000000001-0000000000000001.norito");

        std::fs::write(malformed_path, bytes).expect("write malformed filename record");

        assert!(
            load_commitment_bundle(dir.path())
                .expect("load bundle")
                .is_none()
        );
    }

    #[test]
    fn commitment_bundle_skips_filename_tuple_mismatches() {
        let dir = tempdir().expect("tempdir");
        let record = sample_record(1, 1);
        let bytes = to_bytes(&record).expect("encode record");
        let mut file_key = record.clone();
        file_key.sequence = 2;
        let mismatch_path = dir.path().join(commitment_file_name(&file_key, [0x99; 32]));

        std::fs::write(mismatch_path, bytes).expect("write mismatch record");

        assert!(
            load_commitment_bundle(dir.path())
                .expect("load bundle")
                .is_none()
        );
    }

    #[test]
    fn commitment_bundle_skips_filename_ticket_mismatches() {
        let dir = tempdir().expect("tempdir");
        let record = sample_record(1, 1);
        let bytes = to_bytes(&record).expect("encode record");
        let mut file_key = record.clone();
        file_key.storage_ticket = StorageTicketId::new([0x99; 32]);
        let mismatch_path = dir.path().join(commitment_file_name(&file_key, [0x88; 32]));

        std::fs::write(mismatch_path, bytes).expect("write ticket mismatch record");

        assert!(
            load_commitment_bundle(dir.path())
                .expect("load bundle")
                .is_none()
        );
    }
}
