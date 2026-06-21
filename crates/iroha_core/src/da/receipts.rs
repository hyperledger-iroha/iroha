//! DA receipt spool helpers and cursor tracking.
//!
//! Torii emits `da-receipt-*.norito` artefacts alongside commitment records.
//! These helpers load and canonicalize those receipts, enforce monotonic
//! sequencing per `(lane, epoch)`, and map them onto the sanitized commitment
//! bundle that block assembly embeds.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use blake3::Hasher as Blake3Hasher;
use iroha_config::parameters::actual::LaneConfig;
use iroha_data_model::{
    da::{commitment::DaCommitmentRecord, ingest::DaIngestReceipt, types::StorageTicketId},
    nexus::LaneId,
    sorafs::pin_registry::ManifestDigest,
};
use thiserror::Error;

use crate::da::{LaneEpoch, ReplayFingerprint};

#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize,
)]
struct StoredDaReceipt {
    version: u16,
    sequence: u64,
    receipt: DaIngestReceipt,
}

const STORED_RECEIPT_VERSION: u16 = 1;

/// Encode a DA receipt spool record using the production wrapper schema.
#[cfg(all(test, feature = "sumeragi-main-loop-tests"))]
pub(crate) fn encode_receipt_for_spool_test(
    sequence: u64,
    receipt: DaIngestReceipt,
) -> Result<Vec<u8>, norito::core::Error> {
    let stored = StoredDaReceipt {
        version: STORED_RECEIPT_VERSION,
        sequence,
        receipt,
    };
    norito::to_bytes(&stored)
}

/// Receipt entry captured from the spool.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DaReceiptEntry {
    /// Lane/epoch the receipt belongs to.
    pub lane_epoch: LaneEpoch,
    /// Sequence number scoped to the lane/epoch.
    pub sequence: u64,
    /// Manifest hash referenced by the receipt.
    pub manifest_hash: ManifestDigest,
    /// Full receipt payload.
    pub receipt: DaIngestReceipt,
}

/// Errors encountered while loading DA receipts from disk.
#[derive(Debug, Error)]
pub enum DaReceiptSpoolError {
    /// Directory does not exist or cannot be read.
    #[error("failed to read DA receipt spool `{path}`: {source}")]
    ReadDir {
        /// Path that failed.
        path: PathBuf,
        /// Source error from the filesystem.
        #[source]
        source: std::io::Error,
    },
    /// Failed to read a directory entry while scanning the spool.
    #[error("failed to read DA receipt spool entry in `{path}`: {source}")]
    ReadEntry {
        /// Spool path being scanned.
        path: PathBuf,
        /// Source error from the filesystem.
        #[source]
        source: std::io::Error,
    },
    /// Failed to read a receipt file.
    #[error("failed to read DA receipt `{path}`: {source}")]
    ReadFile {
        /// Path that failed.
        path: PathBuf,
        /// Source error from the filesystem.
        #[source]
        source: std::io::Error,
    },
    /// Failed to decode a receipt file.
    #[error("failed to decode DA receipt `{path}`: {source}")]
    Decode {
        /// Path that failed.
        path: PathBuf,
        /// Source decode error.
        #[source]
        source: norito::core::Error,
    },
    /// Receipt version is not supported.
    #[error("unsupported DA receipt version {version} at {path}")]
    UnsupportedVersion {
        /// Path that failed.
        path: PathBuf,
        /// Unsupported version encountered.
        version: u16,
    },
    /// Receipt filename does not contain the expected lane/epoch/sequence/ticket/fingerprint tuple.
    #[error("malformed DA receipt filename at {path}")]
    MalformedFilename {
        /// Path that failed.
        path: PathBuf,
    },
    /// Receipt filename tuple does not match the decoded receipt body.
    #[error(
        "DA receipt filename tuple {filename_lane:?}/{filename_epoch}/{filename_sequence}/{filename_ticket:?} mismatches body {receipt_lane:?}/{receipt_epoch}/{receipt_sequence}/{receipt_ticket:?} at {path}"
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
        /// Lane identifier decoded from the receipt.
        receipt_lane: LaneId,
        /// Epoch decoded from the receipt.
        receipt_epoch: u64,
        /// Sequence decoded from the receipt wrapper.
        receipt_sequence: u64,
        /// Storage ticket decoded from the receipt.
        receipt_ticket: StorageTicketId,
    },
    /// The same signed receipt body appeared under multiple replay fingerprints.
    #[error(
        "duplicate DA receipt fingerprint conflict for lane {lane:?} epoch {epoch} sequence {sequence}"
    )]
    DuplicateFingerprintConflict {
        /// Lane identifier that conflicted.
        lane: LaneId,
        /// Epoch that conflicted.
        epoch: u64,
        /// Sequence that conflicted.
        sequence: u64,
        /// Replay fingerprint first observed for this receipt body.
        expected: ReplayFingerprint,
        /// Replay fingerprint observed on the duplicate filename.
        observed: ReplayFingerprint,
    },
    /// Multiple receipt bodies claimed the same lane/epoch/sequence key.
    #[error("duplicate DA receipt key for lane {lane:?} epoch {epoch} sequence {sequence}")]
    DuplicateReceiptKey {
        /// Lane identifier that conflicted.
        lane: LaneId,
        /// Epoch that conflicted.
        epoch: u64,
        /// Sequence that conflicted.
        sequence: u64,
    },
}

/// Errors returned when the receipt queue violates ordering or bundle mapping.
#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum DaReceiptQueueError {
    /// A receipt referenced a lane that is not present in the configured catalog.
    #[error("lane {lane} not present in the configured lane catalog")]
    UnknownLane {
        /// Missing lane identifier.
        lane: LaneId,
    },
    /// Receipt reused a sequence number with a different manifest hash.
    #[error("receipt conflict for lane {lane:?} epoch {epoch} sequence {sequence}")]
    ManifestConflict {
        /// Lane identifier that conflicted.
        lane: LaneId,
        /// Epoch that conflicted.
        epoch: u64,
        /// Sequence that conflicted.
        sequence: u64,
        /// Manifest hash already recorded.
        expected: ManifestDigest,
        /// Manifest hash observed in the new receipt.
        observed: ManifestDigest,
    },
    /// Receipt reused a sequence number with a different storage ticket.
    #[error("receipt storage ticket conflict for lane {lane:?} epoch {epoch} sequence {sequence}")]
    StorageTicketConflict {
        /// Lane identifier that conflicted.
        lane: LaneId,
        /// Epoch that conflicted.
        epoch: u64,
        /// Sequence that conflicted.
        sequence: u64,
        /// Storage ticket already recorded.
        expected: StorageTicketId,
        /// Storage ticket observed in the new receipt.
        observed: StorageTicketId,
    },
    /// Receipt reused a sequence number with different signed receipt evidence.
    #[error("receipt evidence conflict for lane {lane:?} epoch {epoch} sequence {sequence}")]
    ReceiptEvidenceConflict {
        /// Lane identifier that conflicted.
        lane: LaneId,
        /// Epoch that conflicted.
        epoch: u64,
        /// Sequence that conflicted.
        sequence: u64,
    },
    /// Receipt sequence regressed relative to the cursor.
    #[error(
        "receipt for lane {lane:?} epoch {epoch} sequence {sequence} fell behind cursor {highest}"
    )]
    Stale {
        /// Lane identifier for the stale receipt.
        lane: LaneId,
        /// Epoch for the stale receipt.
        epoch: u64,
        /// Sequence observed in the receipt.
        sequence: u64,
        /// Highest sequence already recorded for the lane/epoch.
        highest: u64,
    },
    /// A gap was detected between the cursor and the next committable receipt.
    #[error(
        "missing receipt for lane {lane:?} epoch {epoch}: expected sequence {expected} but saw {observed}"
    )]
    MissingSequence {
        /// Lane identifier that triggered the gap.
        lane: LaneId,
        /// Epoch that triggered the gap.
        epoch: u64,
        /// Sequence number expected after the cursor.
        expected: u64,
        /// First sequence observed after the gap.
        observed: u64,
    },
    /// Commitment bundle contained more than one record for the same lane/epoch/sequence.
    #[error("duplicate DA commitment for lane {lane:?} epoch {epoch} sequence {sequence}")]
    DuplicateCommitment {
        /// Lane identifier that failed validation.
        lane: LaneId,
        /// Epoch that failed validation.
        epoch: u64,
        /// Sequence number that failed validation.
        sequence: u64,
    },
    /// Receipt was present but no matching commitment record was found in the bundle.
    #[error(
        "missing DA commitment for lane {lane:?} epoch {epoch} sequence {sequence} referenced by receipt"
    )]
    MissingCommitment {
        /// Lane identifier that failed lookup.
        lane: LaneId,
        /// Epoch that failed lookup.
        epoch: u64,
        /// Sequence number that failed lookup.
        sequence: u64,
    },
    /// Commitment manifest did not match the receipt manifest hash.
    #[error(
        "commitment manifest hash {commitment:?} mismatches receipt manifest {receipt:?} for lane {lane:?} epoch {epoch} sequence {sequence}"
    )]
    CommitmentManifestMismatch {
        /// Lane identifier that failed validation.
        lane: LaneId,
        /// Epoch that failed validation.
        epoch: u64,
        /// Sequence number that failed validation.
        sequence: u64,
        /// Manifest hash recorded in the commitment.
        commitment: ManifestDigest,
        /// Manifest hash referenced by the receipt.
        receipt: ManifestDigest,
    },
    /// Commitment storage ticket did not match the receipt ticket.
    #[error(
        "commitment storage ticket {commitment:?} mismatches receipt ticket {receipt:?} for lane {lane:?} epoch {epoch} sequence {sequence}"
    )]
    CommitmentTicketMismatch {
        /// Lane identifier that failed validation.
        lane: LaneId,
        /// Epoch that failed validation.
        epoch: u64,
        /// Sequence number that failed validation.
        sequence: u64,
        /// Storage ticket recorded in the commitment.
        commitment: StorageTicketId,
        /// Storage ticket referenced by the receipt.
        receipt: StorageTicketId,
    },
}

/// Summary of a DA receipt spool cleanup pass.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DaReceiptPruneReport {
    /// Receipt-shaped files considered by the cleanup pass.
    pub scanned_receipts: usize,
    /// Stale receipt files removed successfully.
    pub removed_stale: usize,
    /// Receipt-shaped files skipped because their filename or body was invalid.
    pub skipped_invalid: usize,
    /// Directory entries that could not be read.
    pub entry_failures: usize,
    /// Receipt-shaped files that could not be read.
    pub read_failures: usize,
    /// Stale receipt files that could not be removed.
    pub remove_failures: usize,
    /// True when the spool directory itself could not be opened.
    pub read_dir_failed: bool,
}

impl DaReceiptPruneReport {
    /// Return true when cleanup encountered filesystem failures.
    #[must_use]
    pub const fn has_failures(self) -> bool {
        self.read_dir_failed
            || self.entry_failures > 0
            || self.read_failures > 0
            || self.remove_failures > 0
    }
}

/// Error encountered while advancing the receipt cursor index.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum DaReceiptCursorError {
    /// Sequence regressed for a lane/epoch.
    #[error(
        "receipt cursor regression for lane {lane:?} epoch {epoch}: observed {observed} below recorded {recorded}"
    )]
    Regression {
        /// Lane identifier that regressed.
        lane: LaneId,
        /// Epoch that regressed.
        epoch: u64,
        /// Sequence observed.
        observed: u64,
        /// Sequence already recorded.
        recorded: u64,
    },
    /// Sequence skipped the next expected value for a lane/epoch.
    #[error(
        "receipt cursor gap for lane {lane:?} epoch {epoch}: expected sequence {expected} but observed {observed}"
    )]
    MissingSequence {
        /// Lane identifier that skipped a sequence.
        lane: LaneId,
        /// Epoch that skipped a sequence.
        epoch: u64,
        /// Next sequence expected by the cursor.
        expected: u64,
        /// Sequence observed.
        observed: u64,
    },
}

/// Snapshot of the highest receipt sequence per `(lane, epoch)` observed in committed blocks.
#[derive(Clone, Debug, Default)]
pub struct DaReceiptCursorIndex {
    by_lane_epoch: BTreeMap<LaneEpoch, DaReceiptCursor>,
}

/// Cursor tracking the highest DA receipt committed for a lane/epoch pair.
#[derive(Clone, Copy, Debug, Default)]
pub struct DaReceiptCursor {
    /// Epoch associated with this cursor.
    pub epoch: u64,
    /// Highest committed sequence recorded.
    pub sequence: u64,
    /// Block height that advanced the cursor.
    pub last_block_height: u64,
}

impl DaReceiptCursorIndex {
    /// Record a single cursor advancement.
    ///
    /// # Errors
    ///
    /// Returns [`DaReceiptCursorError::Regression`] when the supplied sequence regresses relative
    /// to the stored cursor for the `(lane, epoch)`, or
    /// [`DaReceiptCursorError::MissingSequence`] when it skips the next expected sequence.
    pub fn record(
        &mut self,
        lane_epoch: LaneEpoch,
        sequence: u64,
        block_height: u64,
    ) -> Result<(), DaReceiptCursorError> {
        match self.by_lane_epoch.get_mut(&lane_epoch) {
            None => {
                self.by_lane_epoch.insert(
                    lane_epoch,
                    DaReceiptCursor {
                        epoch: lane_epoch.epoch,
                        sequence,
                        last_block_height: block_height,
                    },
                );
                Ok(())
            }
            Some(cursor) => {
                if sequence < cursor.sequence {
                    return Err(DaReceiptCursorError::Regression {
                        lane: lane_epoch.lane_id,
                        epoch: lane_epoch.epoch,
                        observed: sequence,
                        recorded: cursor.sequence,
                    });
                }
                if sequence == cursor.sequence {
                    return Ok(());
                }
                let expected = cursor.sequence.saturating_add(1);
                if sequence != expected {
                    return Err(DaReceiptCursorError::MissingSequence {
                        lane: lane_epoch.lane_id,
                        epoch: lane_epoch.epoch,
                        expected,
                        observed: sequence,
                    });
                }
                *cursor = DaReceiptCursor {
                    epoch: lane_epoch.epoch,
                    sequence,
                    last_block_height: block_height,
                };
                Ok(())
            }
        }
    }

    /// Record all cursors present in the commitment bundle.
    ///
    /// # Errors
    ///
    /// Returns [`DaReceiptCursorError`] when any record regresses or skips the next expected
    /// sequence relative to its cursor.
    pub fn record_bundle(
        &mut self,
        block_height: u64,
        records: &[DaCommitmentRecord],
    ) -> Result<Vec<(LaneEpoch, u64)>, DaReceiptCursorError> {
        let mut candidate = self.clone();
        let mut advanced = Vec::new();
        for record in records {
            let lane_epoch = LaneEpoch::new(record.lane_id, record.epoch);
            candidate.record(lane_epoch, record.sequence, block_height)?;
            advanced.push((lane_epoch, record.sequence));
        }
        *self = candidate;
        Ok(advanced)
    }

    /// Return the highest recorded sequence for a `(lane, epoch)` pair.
    #[must_use]
    pub fn highest(&self, lane_epoch: LaneEpoch) -> Option<u64> {
        self.by_lane_epoch
            .get(&lane_epoch)
            .map(|cursor| cursor.sequence)
    }

    /// Snapshot the internal map for downstream planning.
    #[must_use]
    pub fn snapshot(&self) -> BTreeMap<LaneEpoch, u64> {
        self.by_lane_epoch
            .iter()
            .map(|(lane_epoch, cursor)| (*lane_epoch, cursor.sequence))
            .collect()
    }
}

/// Load raw receipt entries from the spool directory, retaining deterministic order.
///
/// # Errors
///
/// Returns a [`DaReceiptSpoolError`] if the spool directory or any matching
/// receipt file cannot be read, decoded, or matched against its advertised
/// filename tuple.
pub fn load_receipt_entries(spool_dir: &Path) -> Result<Vec<DaReceiptEntry>, DaReceiptSpoolError> {
    let Some(dir_entries) =
        open_receipt_spool_dir(spool_dir).map_err(|source| DaReceiptSpoolError::ReadDir {
            path: spool_dir.to_path_buf(),
            source,
        })?
    else {
        return Ok(Vec::new());
    };

    let mut receipts = Vec::new();
    let mut seen: BTreeMap<(LaneEpoch, u64), Vec<(DaIngestReceipt, ReplayFingerprint)>> =
        BTreeMap::new();
    let mut seen_keys: BTreeMap<(LaneEpoch, u64), DaIngestReceipt> = BTreeMap::new();

    let mut paths = Vec::new();
    for entry in dir_entries {
        let entry = entry.map_err(|source| DaReceiptSpoolError::ReadEntry {
            path: spool_dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if !is_da_receipt_file(&path)? {
            continue;
        }
        paths.push(path);
    }
    paths.sort();

    for path in paths {
        let data = read_regular_receipt_file(&path)?;
        let (filename_key, receipt_entry) = decode_receipt(&data, &path)?;
        let seen_key = (receipt_entry.lane_epoch, receipt_entry.sequence);
        let duplicate_receipts = seen.entry(seen_key).or_default();
        if let Some((_, expected)) = duplicate_receipts
            .iter()
            .find(|(receipt, _)| receipt == &receipt_entry.receipt)
        {
            if *expected != filename_key.fingerprint {
                return Err(DaReceiptSpoolError::DuplicateFingerprintConflict {
                    lane: receipt_entry.lane_epoch.lane_id,
                    epoch: receipt_entry.lane_epoch.epoch,
                    sequence: receipt_entry.sequence,
                    expected: *expected,
                    observed: filename_key.fingerprint,
                });
            }
        }
        duplicate_receipts.push((receipt_entry.receipt.clone(), filename_key.fingerprint));
        if seen_keys
            .insert(seen_key, receipt_entry.receipt.clone())
            .is_some_and(|previous| previous != receipt_entry.receipt)
        {
            return Err(DaReceiptSpoolError::DuplicateReceiptKey {
                lane: receipt_entry.lane_epoch.lane_id,
                epoch: receipt_entry.lane_epoch.epoch,
                sequence: receipt_entry.sequence,
            });
        }
        receipts.push(receipt_entry);
    }

    receipts.sort_by(|a, b| {
        (
            a.lane_epoch.lane_id.as_u32(),
            a.lane_epoch.epoch,
            a.sequence,
            a.manifest_hash.as_bytes(),
        )
            .cmp(&(
                b.lane_epoch.lane_id.as_u32(),
                b.lane_epoch.epoch,
                b.sequence,
                b.manifest_hash.as_bytes(),
            ))
    });
    Ok(receipts)
}

fn open_receipt_spool_dir(spool_dir: &Path) -> std::io::Result<Option<std::fs::ReadDir>> {
    let metadata = match std::fs::symlink_metadata(spool_dir) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(source),
    };
    if !metadata.file_type().is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "DA receipt spool path is not a directory",
        ));
    }
    std::fs::read_dir(spool_dir).map(Some)
}

fn read_regular_receipt_file(path: &Path) -> Result<Vec<u8>, DaReceiptSpoolError> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|source| DaReceiptSpoolError::ReadFile {
            path: path.to_path_buf(),
            source,
        })?;
    if !metadata.file_type().is_file() {
        return Err(DaReceiptSpoolError::ReadFile {
            path: path.to_path_buf(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "DA receipt artifact is not a regular file",
            ),
        });
    }
    let bytes = std::fs::read(path).map_err(|source| DaReceiptSpoolError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;
    revalidate_regular_receipt_file(path, &metadata, bytes.len())?;
    Ok(bytes)
}

fn revalidate_regular_receipt_file(
    path: &Path,
    metadata: &std::fs::Metadata,
    bytes_len: usize,
) -> Result<(), DaReceiptSpoolError> {
    let current_metadata =
        std::fs::symlink_metadata(path).map_err(|source| DaReceiptSpoolError::ReadFile {
            path: path.to_path_buf(),
            source,
        })?;
    if !current_metadata.file_type().is_file() {
        return Err(DaReceiptSpoolError::ReadFile {
            path: path.to_path_buf(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "DA receipt artifact changed to a non-regular file while reading",
            ),
        });
    }
    if current_metadata.len() != metadata.len()
        || u64::try_from(bytes_len).ok() != Some(metadata.len())
    {
        return Err(DaReceiptSpoolError::ReadFile {
            path: path.to_path_buf(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "DA receipt artifact changed while reading",
            ),
        });
    }
    Ok(())
}

fn is_da_receipt_file(path: &Path) -> Result<bool, DaReceiptSpoolError> {
    let Some(name) = path.file_name() else {
        return Ok(false);
    };
    if let Some(name) = name.to_str() {
        return Ok(name.starts_with("da-receipt-") && name.ends_with(".norito"));
    }
    if non_utf8_artifact_name_matches(name, b"da-receipt-", b".norito") {
        return Err(malformed_receipt_filename(path));
    }
    Ok(false)
}

#[cfg(unix)]
fn non_utf8_artifact_name_matches(name: &std::ffi::OsStr, prefix: &[u8], suffix: &[u8]) -> bool {
    use std::os::unix::ffi::OsStrExt;

    let bytes = name.as_bytes();
    bytes.starts_with(prefix) && bytes.ends_with(suffix)
}

#[cfg(not(unix))]
fn non_utf8_artifact_name_matches(_name: &std::ffi::OsStr, _prefix: &[u8], _suffix: &[u8]) -> bool {
    false
}

#[derive(Clone, Copy)]
struct ReceiptFileKey {
    lane_id: LaneId,
    epoch: u64,
    sequence: u64,
    storage_ticket: StorageTicketId,
    fingerprint: ReplayFingerprint,
}

fn parse_receipt_file_key(path: &Path) -> Result<ReceiptFileKey, DaReceiptSpoolError> {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return Err(malformed_receipt_filename(path));
    };
    let Some(rest) = name
        .strip_prefix("da-receipt-")
        .and_then(|name| name.strip_suffix(".norito"))
    else {
        return Err(malformed_receipt_filename(path));
    };

    let mut fields = rest.split('-');
    let Some(lane_hex) = fields.next() else {
        return Err(malformed_receipt_filename(path));
    };
    let Some(epoch_hex) = fields.next() else {
        return Err(malformed_receipt_filename(path));
    };
    let Some(sequence_hex) = fields.next() else {
        return Err(malformed_receipt_filename(path));
    };
    let Some(ticket_hex) = fields.next() else {
        return Err(malformed_receipt_filename(path));
    };
    let Some(fingerprint_hex) = fields.next() else {
        return Err(malformed_receipt_filename(path));
    };
    if fields.next().is_some() {
        return Err(malformed_receipt_filename(path));
    }

    let lane_id = parse_fixed_hex_u32(lane_hex, 8, path).map(LaneId::new)?;
    let epoch = parse_fixed_hex_u64(epoch_hex, 16, path)?;
    let sequence = parse_fixed_hex_u64(sequence_hex, 16, path)?;
    let storage_ticket = StorageTicketId::new(parse_fixed_hex_32(ticket_hex, path)?);
    let fingerprint = ReplayFingerprint::from(parse_fixed_hex_32(fingerprint_hex, path)?);

    Ok(ReceiptFileKey {
        lane_id,
        epoch,
        sequence,
        storage_ticket,
        fingerprint,
    })
}

fn malformed_receipt_filename(path: &Path) -> DaReceiptSpoolError {
    DaReceiptSpoolError::MalformedFilename {
        path: path.to_path_buf(),
    }
}

fn parse_fixed_hex_u32(value: &str, width: usize, path: &Path) -> Result<u32, DaReceiptSpoolError> {
    if value.len() != width || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(malformed_receipt_filename(path));
    }
    u32::from_str_radix(value, 16).map_err(|_| malformed_receipt_filename(path))
}

fn parse_fixed_hex_u64(value: &str, width: usize, path: &Path) -> Result<u64, DaReceiptSpoolError> {
    if value.len() != width || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(malformed_receipt_filename(path));
    }
    u64::from_str_radix(value, 16).map_err(|_| malformed_receipt_filename(path))
}

fn parse_fixed_hex_32(value: &str, path: &Path) -> Result<[u8; 32], DaReceiptSpoolError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(malformed_receipt_filename(path));
    }
    let mut bytes = [0_u8; 32];
    hex::decode_to_slice(value, &mut bytes).map_err(|_| malformed_receipt_filename(path))?;
    Ok(bytes)
}

fn decode_receipt(
    data: &[u8],
    path: &Path,
) -> Result<(ReceiptFileKey, DaReceiptEntry), DaReceiptSpoolError> {
    let filename_key = parse_receipt_file_key(path)?;
    let stored = norito::decode_from_bytes::<StoredDaReceipt>(data).map_err(|source| {
        DaReceiptSpoolError::Decode {
            path: path.to_path_buf(),
            source,
        }
    })?;
    if stored.version != STORED_RECEIPT_VERSION {
        return Err(DaReceiptSpoolError::UnsupportedVersion {
            path: path.to_path_buf(),
            version: stored.version,
        });
    }
    let StoredDaReceipt {
        sequence, receipt, ..
    } = stored;
    if filename_key.lane_id != receipt.lane_id
        || filename_key.epoch != receipt.epoch
        || filename_key.sequence != sequence
        || filename_key.storage_ticket != receipt.storage_ticket
    {
        return Err(DaReceiptSpoolError::FilenameMismatch {
            path: path.to_path_buf(),
            filename_lane: filename_key.lane_id,
            filename_epoch: filename_key.epoch,
            filename_sequence: filename_key.sequence,
            filename_ticket: filename_key.storage_ticket,
            receipt_lane: receipt.lane_id,
            receipt_epoch: receipt.epoch,
            receipt_sequence: sequence,
            receipt_ticket: receipt.storage_ticket,
        });
    }
    let lane_epoch = LaneEpoch::new(receipt.lane_id, receipt.epoch);
    Ok((
        filename_key,
        DaReceiptEntry {
            lane_epoch,
            sequence,
            manifest_hash: ManifestDigest::new(*receipt.manifest_hash.as_bytes()),
            receipt,
        },
    ))
}

/// Canonicalize and filter receipts against the current cursor and sealed set.
///
/// This enforces monotonic sequencing per `(lane, epoch)`, drops stale or sealed
/// receipts, and returns the next contiguous slice that must be sealed before a
/// later sequence can appear.
///
/// # Errors
///
/// Returns a [`DaReceiptQueueError`] when manifests conflict or a sequence gap
/// is detected after the current cursor. Receipts for lanes no longer present in
/// the configured catalog are treated as stale local spool data and skipped.
pub fn plan_committable_receipts(
    lane_config: &LaneConfig,
    cursor_snapshot: &BTreeMap<LaneEpoch, u64>,
    sealed: &BTreeSet<iroha_data_model::da::commitment::DaCommitmentKey>,
    receipts: Vec<DaReceiptEntry>,
) -> Result<Vec<DaReceiptEntry>, DaReceiptQueueError> {
    let mut sealed_highest: BTreeMap<LaneEpoch, u64> = BTreeMap::new();
    for key in sealed {
        let lane_epoch = LaneEpoch::new(key.lane_id, key.epoch);
        sealed_highest
            .entry(lane_epoch)
            .and_modify(|seq| *seq = (*seq).max(key.sequence))
            .or_insert(key.sequence);
    }

    let mut grouped: BTreeMap<LaneEpoch, BTreeMap<u64, DaReceiptEntry>> = BTreeMap::new();
    for entry in receipts {
        if lane_config.entry(entry.lane_epoch.lane_id).is_none() {
            iroha_logger::warn!(
                lane = entry.lane_epoch.lane_id.as_u32(),
                epoch = entry.lane_epoch.epoch,
                sequence = entry.sequence,
                "skipping stale DA receipt for lane not present in the configured catalog"
            );
            continue;
        }

        let key = iroha_data_model::da::commitment::DaCommitmentKey {
            lane_id: entry.lane_epoch.lane_id,
            epoch: entry.lane_epoch.epoch,
            sequence: entry.sequence,
        };
        if sealed.contains(&key) {
            continue;
        }

        let lane_map = grouped.entry(entry.lane_epoch).or_default();
        if let Some(existing) = lane_map.get(&entry.sequence) {
            if existing.manifest_hash != entry.manifest_hash {
                return Err(DaReceiptQueueError::ManifestConflict {
                    lane: entry.lane_epoch.lane_id,
                    epoch: entry.lane_epoch.epoch,
                    sequence: entry.sequence,
                    expected: existing.manifest_hash,
                    observed: entry.manifest_hash,
                });
            }
            if existing.receipt.storage_ticket != entry.receipt.storage_ticket {
                return Err(DaReceiptQueueError::StorageTicketConflict {
                    lane: entry.lane_epoch.lane_id,
                    epoch: entry.lane_epoch.epoch,
                    sequence: entry.sequence,
                    expected: existing.receipt.storage_ticket,
                    observed: entry.receipt.storage_ticket,
                });
            }
            if existing.receipt != entry.receipt {
                return Err(DaReceiptQueueError::ReceiptEvidenceConflict {
                    lane: entry.lane_epoch.lane_id,
                    epoch: entry.lane_epoch.epoch,
                    sequence: entry.sequence,
                });
            }
            continue;
        }
        lane_map.insert(entry.sequence, entry);
    }

    let mut planned = Vec::new();
    for (lane_epoch, entries) in grouped {
        let base_floor = match (
            cursor_snapshot.get(&lane_epoch),
            sealed_highest.get(&lane_epoch),
        ) {
            (Some(committed), Some(sealed_seq)) => Some((*committed).max(*sealed_seq)),
            (Some(committed), None) => Some(*committed),
            (None, Some(sealed_seq)) => Some(*sealed_seq),
            (None, None) => None,
        };
        let mut expected = base_floor.map_or_else(
            || *entries.keys().next().unwrap_or(&0),
            |highest| highest.saturating_add(1),
        );
        for (sequence, entry) in entries {
            if let Some(highest) = base_floor {
                if sequence <= highest {
                    continue;
                }
            }
            if sequence != expected {
                return Err(DaReceiptQueueError::MissingSequence {
                    lane: lane_epoch.lane_id,
                    epoch: lane_epoch.epoch,
                    expected,
                    observed: sequence,
                });
            }
            planned.push(entry);
            expected = expected.saturating_add(1);
        }
    }

    planned.sort_by(|a, b| {
        (
            a.lane_epoch.lane_id.as_u32(),
            a.lane_epoch.epoch,
            a.sequence,
        )
            .cmp(&(
                b.lane_epoch.lane_id.as_u32(),
                b.lane_epoch.epoch,
                b.sequence,
            ))
    });
    Ok(planned)
}

/// Align commitment records with the planned receipt queue.
///
/// # Errors
///
/// Returns a [`DaReceiptQueueError`] when a receipt lacks a matching commitment
/// or when manifests diverge.
pub fn align_commitments_for_receipts(
    receipts: &[DaReceiptEntry],
    commitments: &[DaCommitmentRecord],
) -> Result<Vec<DaCommitmentRecord>, DaReceiptQueueError> {
    if receipts.is_empty() {
        return Ok(Vec::new());
    }

    let receipt_keys: BTreeSet<_> = receipts
        .iter()
        .map(|receipt| (receipt.lane_epoch, receipt.sequence))
        .collect();
    let mut by_key: BTreeMap<(LaneEpoch, u64), &DaCommitmentRecord> = BTreeMap::new();
    for record in commitments {
        let lane_epoch = LaneEpoch::new(record.lane_id, record.epoch);
        let key = (lane_epoch, record.sequence);
        if !receipt_keys.contains(&key) {
            continue;
        }
        if by_key.insert(key, record).is_some() {
            return Err(DaReceiptQueueError::DuplicateCommitment {
                lane: record.lane_id,
                epoch: record.epoch,
                sequence: record.sequence,
            });
        }
    }

    let mut aligned = Vec::with_capacity(receipts.len());
    for receipt in receipts {
        let key = (receipt.lane_epoch, receipt.sequence);
        let Some(record) = by_key.get(&key) else {
            return Err(DaReceiptQueueError::MissingCommitment {
                lane: receipt.lane_epoch.lane_id,
                epoch: receipt.lane_epoch.epoch,
                sequence: receipt.sequence,
            });
        };
        if record.manifest_hash != receipt.manifest_hash {
            return Err(DaReceiptQueueError::CommitmentManifestMismatch {
                lane: receipt.lane_epoch.lane_id,
                epoch: receipt.lane_epoch.epoch,
                sequence: receipt.sequence,
                commitment: record.manifest_hash,
                receipt: receipt.manifest_hash,
            });
        }
        if record.storage_ticket != receipt.receipt.storage_ticket {
            return Err(DaReceiptQueueError::CommitmentTicketMismatch {
                lane: receipt.lane_epoch.lane_id,
                epoch: receipt.lane_epoch.epoch,
                sequence: receipt.sequence,
                commitment: record.storage_ticket,
                receipt: receipt.receipt.storage_ticket,
            });
        }
        aligned.push((*record).clone());
    }

    Ok(aligned)
}

/// Remove stale receipts from the spool based on the committed cursor snapshot.
///
/// Cleanup failures are reported and logged, but they do not abort callers.
/// Proposal assembly must continue to rely on validated receipt loading and
/// cursor checks rather than on cleanup success.
pub fn prune_spool(spool_dir: &Path, cursors: &BTreeMap<LaneEpoch, u64>) -> DaReceiptPruneReport {
    let mut report = DaReceiptPruneReport::default();
    let entries = match open_receipt_spool_dir(spool_dir) {
        Ok(Some(entries)) => entries,
        Ok(None) => return report,
        Err(err) => {
            report.read_dir_failed = true;
            iroha_logger::warn!(
                ?err,
                path = %spool_dir.display(),
                "failed to open DA receipt spool for stale cleanup"
            );
            return report;
        }
    };

    let mut paths = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                report.entry_failures = report.entry_failures.saturating_add(1);
                iroha_logger::warn!(
                    ?err,
                    path = %spool_dir.display(),
                    "failed to read DA receipt spool entry during stale cleanup"
                );
                continue;
            }
        };
        paths.push(entry.path());
    }
    paths.sort();

    for path in paths {
        match is_da_receipt_file(&path) {
            Ok(true) => {}
            Ok(false) => continue,
            Err(err) => {
                report.skipped_invalid = report.skipped_invalid.saturating_add(1);
                iroha_logger::warn!(
                    ?err,
                    path = %path.display(),
                    "skipping invalid DA receipt filename during stale cleanup"
                );
                continue;
            }
        }
        report.scanned_receipts = report.scanned_receipts.saturating_add(1);
        let data = match read_regular_receipt_file(&path) {
            Ok(data) => data,
            Err(err) => {
                report.read_failures = report.read_failures.saturating_add(1);
                iroha_logger::warn!(
                    ?err,
                    path = %path.display(),
                    "failed to read DA receipt during stale cleanup"
                );
                continue;
            }
        };
        let entry = match decode_receipt(&data, &path) {
            Ok((_, entry)) => entry,
            Err(err) => {
                report.skipped_invalid = report.skipped_invalid.saturating_add(1);
                iroha_logger::warn!(
                    ?err,
                    path = %path.display(),
                    "skipping invalid DA receipt during stale cleanup"
                );
                continue;
            }
        };
        if let Some(highest) = cursors.get(&entry.lane_epoch) {
            if entry.sequence <= *highest {
                if let Err(err) = std::fs::remove_file(&path) {
                    report.remove_failures = report.remove_failures.saturating_add(1);
                    iroha_logger::warn!(
                        ?err,
                        path = %path.display(),
                        "failed to prune stale DA receipt file"
                    );
                } else {
                    report.removed_stale = report.removed_stale.saturating_add(1);
                }
            }
        }
    }
    report
}

/// Extract a deterministic fingerprint for replay cache usage.
#[must_use]
pub fn receipt_fingerprint(receipt: &DaIngestReceipt) -> ReplayFingerprint {
    let mut hasher = Blake3Hasher::new();
    hasher.update(receipt.chunk_root.as_ref());
    hasher.update(receipt.manifest_hash.as_ref());
    ReplayFingerprint::from_hash(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        num::NonZeroU32,
    };

    use iroha_config::parameters::actual::LaneConfig as ConfigLaneConfig;
    use iroha_crypto::{Hash, Signature};
    use iroha_data_model::{
        da::{
            commitment::{DaCommitmentRecord, DaProofScheme, KzgCommitment, RetentionClass},
            ingest::DaStripeLayout,
            types::{BlobDigest, DaRentQuote, StorageTicketId},
        },
        nexus::{LaneCatalog, LaneConfig as ModelLaneConfig, LaneId},
        sorafs::pin_registry::ManifestDigest,
    };
    use norito::to_bytes;
    use tempfile::tempdir;

    use super::*;

    fn sample_receipt(lane: u32, epoch: u64, sequence: u64) -> DaIngestReceipt {
        let lane_id = LaneId::new(lane);
        let seq_byte = u8::try_from(sequence).unwrap_or(0);
        DaIngestReceipt {
            client_blob_id: BlobDigest::new([0xAA; 32]),
            lane_id,
            epoch,
            blob_hash: BlobDigest::new([0xBB; 32]),
            chunk_root: BlobDigest::new([seq_byte; 32]),
            manifest_hash: BlobDigest::new([0xDD; 32]),
            storage_ticket: StorageTicketId::new([0xEE; 32]),
            pdp_commitment: None,
            stripe_layout: DaStripeLayout::default(),
            queued_at_unix: 0,
            rent_quote: DaRentQuote::default(),
            operator_signature: Signature::from_bytes(&[0x11; 64]),
        }
    }

    fn sample_record(receipt: &DaIngestReceipt, sequence: u64) -> DaCommitmentRecord {
        DaCommitmentRecord::new(
            receipt.lane_id,
            receipt.epoch,
            sequence,
            receipt.client_blob_id,
            ManifestDigest::new(*receipt.manifest_hash.as_bytes()),
            DaProofScheme::MerkleSha256,
            Hash::prehashed(*receipt.chunk_root.as_bytes()),
            Some(KzgCommitment::new([0x44; 48])),
            None,
            RetentionClass::default(),
            receipt.storage_ticket,
            Signature::from_bytes(&[0x33; 64]),
        )
    }

    fn receipt_file_name(
        receipt: &DaIngestReceipt,
        sequence: u64,
        fingerprint: [u8; 32],
    ) -> String {
        format!(
            "da-receipt-{lane:08x}-{epoch:016x}-{sequence:016x}-{ticket}-{fingerprint}.norito",
            lane = receipt.lane_id.as_u32(),
            epoch = receipt.epoch,
            ticket = hex::encode(receipt.storage_ticket.as_ref()),
            fingerprint = hex::encode(fingerprint)
        )
    }

    fn cursor_snapshot(lane: LaneId, epoch: u64, sequence: u64) -> BTreeMap<LaneEpoch, u64> {
        let mut map = BTreeMap::new();
        map.insert(LaneEpoch::new(lane, epoch), sequence);
        map
    }

    fn lane_config_for(lane: LaneId) -> ConfigLaneConfig {
        let lane_count =
            NonZeroU32::new(lane.as_u32().saturating_add(1)).expect("lane count is non-zero");
        let metadata = ModelLaneConfig {
            id: lane,
            alias: format!("lane-{}", lane.as_u32()),
            ..ModelLaneConfig::default()
        };
        let catalog = LaneCatalog::new(lane_count, vec![metadata]).expect("lane catalog");
        ConfigLaneConfig::from_catalog(&catalog)
    }

    #[test]
    fn receipt_cursor_record_bundle_rolls_back_when_later_record_regresses() {
        let lane0_epoch = LaneEpoch::new(LaneId::new(0), 1);
        let lane1_epoch = LaneEpoch::new(LaneId::new(1), 1);
        let mut index = DaReceiptCursorIndex::default();
        let initial_lane0 = sample_record(&sample_receipt(0, 1, 1), 1);
        let initial_lane1 = sample_record(&sample_receipt(1, 1, 3), 3);
        index
            .record_bundle(1, &[initial_lane0, initial_lane1])
            .expect("initial receipt cursor records");

        let advancing_lane0 = sample_record(&sample_receipt(0, 1, 2), 2);
        let regressing_lane1 = sample_record(&sample_receipt(1, 1, 2), 2);
        let err = index
            .record_bundle(2, &[advancing_lane0, regressing_lane1])
            .expect_err("later receipt cursor regression must fail");
        assert!(matches!(
            err,
            DaReceiptCursorError::Regression {
                lane,
                observed: 2,
                recorded: 3,
                ..
            } if lane == LaneId::new(1)
        ));
        assert_eq!(index.highest(lane0_epoch), Some(1));
        assert_eq!(index.highest(lane1_epoch), Some(3));
        assert_eq!(
            index
                .by_lane_epoch
                .get(&lane0_epoch)
                .expect("lane0 cursor")
                .last_block_height,
            1
        );
    }

    #[test]
    fn receipt_cursor_record_bundle_rejects_sequence_gap_and_rolls_back() {
        let lane_epoch = LaneEpoch::new(LaneId::new(0), 1);
        let mut index = DaReceiptCursorIndex::default();
        let initial = sample_record(&sample_receipt(0, 1, 1), 1);
        index
            .record_bundle(1, &[initial])
            .expect("initial receipt cursor record");

        let advancing = sample_record(&sample_receipt(0, 1, 2), 2);
        let skipping = sample_record(&sample_receipt(0, 1, 4), 4);
        let err = index
            .record_bundle(2, &[advancing, skipping])
            .expect_err("later receipt cursor sequence gap must fail");
        assert!(matches!(
            err,
            DaReceiptCursorError::MissingSequence {
                lane,
                epoch: 1,
                expected: 3,
                observed: 4
            } if lane == LaneId::new(0)
        ));
        assert_eq!(index.highest(lane_epoch), Some(1));
        assert_eq!(
            index
                .by_lane_epoch
                .get(&lane_epoch)
                .expect("cursor")
                .last_block_height,
            1
        );
    }

    #[test]
    fn load_receipt_entries_reads_spool() {
        let dir = tempdir().expect("tempdir");
        let receipt = sample_receipt(1, 2, 3);
        let stored = StoredDaReceipt {
            version: STORED_RECEIPT_VERSION,
            sequence: 3,
            receipt: receipt.clone(),
        };
        let bytes = to_bytes(&stored).expect("encode");
        let path = dir.path().join(receipt_file_name(&receipt, 3, [0x99; 32]));
        std::fs::write(&path, bytes).expect("write");

        let entries = load_receipt_entries(dir.path()).expect("load entries");
        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry.sequence, 3);
        assert_eq!(entry.lane_epoch, LaneEpoch::new(LaneId::new(1), 2));
        assert_eq!(entry.receipt, receipt);
    }

    #[test]
    fn load_receipt_entries_rejects_corrupt_files() {
        let dir = tempdir().expect("tempdir");
        let receipt = sample_receipt(1, 2, 3);
        let stored = StoredDaReceipt {
            version: STORED_RECEIPT_VERSION,
            sequence: 3,
            receipt: receipt.clone(),
        };
        let bytes = to_bytes(&stored).expect("encode");
        let ok_path = dir.path().join(receipt_file_name(&receipt, 3, [0x99; 32]));
        let corrupt_receipt = sample_receipt(1, 2, 4);
        let bad_path = dir
            .path()
            .join(receipt_file_name(&corrupt_receipt, 4, [0x88; 32]));
        std::fs::write(&ok_path, bytes).expect("write ok");
        std::fs::write(&bad_path, b"corrupt").expect("write corrupt");

        assert!(
            matches!(
                load_receipt_entries(dir.path()),
                Err(DaReceiptSpoolError::Decode { .. })
            ),
            "corrupt receipt artifacts must reject the whole spool load"
        );
    }

    #[test]
    fn load_receipt_entries_rejects_receipt_shaped_directory() {
        let dir = tempdir().expect("tempdir");
        let receipt = sample_receipt(1, 2, 3);
        let path = dir.path().join(receipt_file_name(&receipt, 3, [0x7c; 32]));
        std::fs::create_dir(&path).expect("create receipt-shaped directory");

        assert!(
            matches!(
                load_receipt_entries(dir.path()),
                Err(DaReceiptSpoolError::ReadFile { path: observed, .. }) if observed == path
            ),
            "receipt-shaped non-files must reject the whole spool load"
        );
    }

    #[cfg(unix)]
    #[test]
    fn load_receipt_entries_rejects_receipt_shaped_symlink() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().expect("tempdir");
        let receipt = sample_receipt(1, 2, 3);
        let stored = StoredDaReceipt {
            version: STORED_RECEIPT_VERSION,
            sequence: 3,
            receipt: receipt.clone(),
        };
        let target = dir.path().join("receipt-target.bin");
        std::fs::write(&target, to_bytes(&stored).expect("encode receipt"))
            .expect("write target receipt");
        let path = dir.path().join(receipt_file_name(&receipt, 3, [0x7c; 32]));
        symlink(&target, &path).expect("create receipt-shaped symlink");

        assert!(
            matches!(
                load_receipt_entries(dir.path()),
                Err(DaReceiptSpoolError::ReadFile { path: observed, .. }) if observed == path
            ),
            "receipt-shaped symlinks must reject the whole spool load"
        );
    }

    #[cfg(unix)]
    #[test]
    fn load_receipt_entries_rejects_spool_dir_symlink() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().expect("tempdir");
        let target = dir.path().join("receipt-spool-target");
        std::fs::create_dir(&target).expect("create target directory");
        let spool = dir.path().join("receipt-spool-link");
        symlink(&target, &spool).expect("create receipt spool symlink");

        let err =
            load_receipt_entries(&spool).expect_err("symlinked receipt spool must reject load");

        match err {
            DaReceiptSpoolError::ReadDir {
                path: observed,
                source,
            } => {
                assert_eq!(observed, spool);
                assert_eq!(source.kind(), std::io::ErrorKind::InvalidData);
                assert!(
                    source.to_string().contains("spool path is not a directory"),
                    "unexpected error: {source}"
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
        assert!(
            std::fs::symlink_metadata(&spool)
                .expect("inspect spool symlink")
                .file_type()
                .is_symlink(),
            "failed load should leave spool symlink visible"
        );
        assert!(
            target.exists(),
            "spool symlink target should not be removed"
        );
    }

    #[test]
    fn receipt_read_revalidation_rejects_length_change() {
        let dir = tempdir().expect("tempdir");
        let receipt = sample_receipt(1, 2, 3);
        let path = dir.path().join(receipt_file_name(&receipt, 3, [0x7d; 32]));
        std::fs::write(&path, b"old").expect("write initial receipt");
        let metadata = std::fs::symlink_metadata(&path).expect("inspect initial receipt");
        std::fs::write(&path, b"new-longer").expect("replace receipt bytes");

        let err = revalidate_regular_receipt_file(&path, &metadata, 3)
            .expect_err("post-read length changes must reject DA receipt artifacts");

        match err {
            DaReceiptSpoolError::ReadFile {
                path: observed,
                source,
            } => {
                assert_eq!(observed, path);
                assert_eq!(source.kind(), std::io::ErrorKind::InvalidData);
                assert!(
                    source.to_string().contains("changed while reading"),
                    "unexpected revalidation error: {source}"
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn receipt_file_matcher_rejects_non_utf8_receipt_shaped_filename() {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};

        let path = PathBuf::from(OsString::from_vec(b"da-receipt-\xFF.norito".to_vec()));

        let err = is_da_receipt_file(&path).expect_err("non-UTF8 shaped artifact rejects");
        match err {
            DaReceiptSpoolError::MalformedFilename { path: seen } => assert_eq!(seen, path),
            _ => panic!("expected malformed filename for non-UTF8 DA artifact, got {err:?}"),
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn load_receipt_entries_rejects_non_utf8_receipt_shaped_filename() {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};

        let dir = tempdir().expect("tempdir");
        let path = dir.path().join(PathBuf::from(OsString::from_vec(
            b"da-receipt-\xFF.norito".to_vec(),
        )));
        std::fs::write(&path, b"ignored").expect("write invalid utf8 filename");

        let err = load_receipt_entries(dir.path()).expect_err("non-UTF8 DA artifact rejects");
        match err {
            DaReceiptSpoolError::MalformedFilename { path: seen } => assert_eq!(seen, path),
            _ => panic!("expected malformed filename for non-UTF8 DA artifact, got {err:?}"),
        }
    }

    #[test]
    fn load_receipt_entries_rejects_unsupported_versions() {
        let dir = tempdir().expect("tempdir");
        let receipt = sample_receipt(1, 2, 3);
        let stored = StoredDaReceipt {
            version: STORED_RECEIPT_VERSION + 1,
            sequence: 3,
            receipt: receipt.clone(),
        };
        let bytes = to_bytes(&stored).expect("encode");
        let path = dir.path().join(receipt_file_name(&receipt, 3, [0x99; 32]));
        std::fs::write(&path, bytes).expect("write");

        assert!(
            matches!(
                load_receipt_entries(dir.path()),
                Err(DaReceiptSpoolError::UnsupportedVersion { version, .. })
                    if version == STORED_RECEIPT_VERSION + 1
            ),
            "unsupported receipt versions must reject the whole spool load"
        );
    }

    #[test]
    fn load_receipt_entries_rejects_malformed_filenames() {
        let dir = tempdir().expect("tempdir");
        let receipt = sample_receipt(1, 2, 3);
        let stored = StoredDaReceipt {
            version: STORED_RECEIPT_VERSION,
            sequence: 3,
            receipt: receipt.clone(),
        };
        let bytes = to_bytes(&stored).expect("encode");
        let path = dir.path().join("da-receipt-malformed.norito");
        std::fs::write(&path, bytes).expect("write malformed filename");

        assert!(
            matches!(
                load_receipt_entries(dir.path()),
                Err(DaReceiptSpoolError::MalformedFilename { .. })
            ),
            "malformed receipt filenames must reject the whole spool load"
        );
    }

    #[test]
    fn load_receipt_entries_rejects_filename_body_tuple_mismatch() {
        let dir = tempdir().expect("tempdir");
        let receipt = sample_receipt(1, 2, 3);
        let stored = StoredDaReceipt {
            version: STORED_RECEIPT_VERSION,
            sequence: 3,
            receipt: receipt.clone(),
        };
        let bytes = to_bytes(&stored).expect("encode");
        let path = dir.path().join(receipt_file_name(&receipt, 4, [0x99; 32]));
        std::fs::write(&path, bytes).expect("write mismatched receipt");

        assert!(
            matches!(
                load_receipt_entries(dir.path()),
                Err(DaReceiptSpoolError::FilenameMismatch { .. })
            ),
            "receipt filename/body tuple mismatches must reject the whole spool load"
        );
    }

    #[test]
    fn load_receipt_entries_rejects_filename_ticket_mismatch() {
        let dir = tempdir().expect("tempdir");
        let receipt = sample_receipt(1, 2, 3);
        let stored = StoredDaReceipt {
            version: STORED_RECEIPT_VERSION,
            sequence: 3,
            receipt: receipt.clone(),
        };
        let bytes = to_bytes(&stored).expect("encode");
        let mut filename_receipt = receipt;
        filename_receipt.storage_ticket = StorageTicketId::new([0x99; 32]);
        let path = dir
            .path()
            .join(receipt_file_name(&filename_receipt, 3, [0x88; 32]));
        std::fs::write(&path, bytes).expect("write mismatched receipt");

        assert!(
            matches!(
                load_receipt_entries(dir.path()),
                Err(DaReceiptSpoolError::FilenameMismatch { .. })
            ),
            "receipt filename/body ticket mismatches must reject the whole spool load"
        );
    }

    #[test]
    fn load_receipt_entries_rejects_same_receipt_under_different_fingerprint() {
        let dir = tempdir().expect("tempdir");
        let receipt = sample_receipt(1, 2, 3);
        let stored = StoredDaReceipt {
            version: STORED_RECEIPT_VERSION,
            sequence: 3,
            receipt: receipt.clone(),
        };
        let bytes = to_bytes(&stored).expect("encode");

        for fingerprint in [[0x78; 32], [0x77; 32]] {
            let path = dir.path().join(receipt_file_name(&receipt, 3, fingerprint));
            std::fs::write(&path, &bytes).expect("write duplicate receipt");
        }

        let err = load_receipt_entries(dir.path())
            .expect_err("same signed receipt under different replay fingerprints must reject");
        match err {
            DaReceiptSpoolError::DuplicateFingerprintConflict {
                lane,
                epoch,
                sequence,
                expected,
                observed,
            } => {
                assert_eq!(lane, LaneId::new(1));
                assert_eq!(epoch, 2);
                assert_eq!(sequence, 3);
                assert_eq!(expected, ReplayFingerprint::from([0x77; 32]));
                assert_eq!(observed, ReplayFingerprint::from([0x78; 32]));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn load_receipt_entries_rejects_duplicate_key_with_different_receipt() {
        let dir = tempdir().expect("tempdir");
        let first = sample_receipt(1, 2, 3);
        let mut second = sample_receipt(1, 2, 3);
        second.manifest_hash = BlobDigest::new([0xD1; 32]);
        second.storage_ticket = StorageTicketId::new([0xE1; 32]);

        for (receipt, fingerprint) in [(&first, [0x77; 32]), (&second, [0x78; 32])] {
            let stored = StoredDaReceipt {
                version: STORED_RECEIPT_VERSION,
                sequence: 3,
                receipt: receipt.clone(),
            };
            let path = dir.path().join(receipt_file_name(receipt, 3, fingerprint));
            std::fs::write(&path, to_bytes(&stored).expect("encode duplicate receipt"))
                .expect("write duplicate receipt");
        }

        let err = load_receipt_entries(dir.path())
            .expect_err("different receipts with the same key must reject");
        assert!(matches!(
            err,
            DaReceiptSpoolError::DuplicateReceiptKey {
                lane,
                epoch: 2,
                sequence: 3
            } if lane == LaneId::new(1)
        ));
    }

    #[test]
    fn prune_spool_removes_valid_stale_receipts() {
        let dir = tempdir().expect("tempdir");
        let receipt = sample_receipt(1, 2, 3);
        let stored = StoredDaReceipt {
            version: STORED_RECEIPT_VERSION,
            sequence: 3,
            receipt: receipt.clone(),
        };
        let path = dir.path().join(receipt_file_name(&receipt, 3, [0x99; 32]));
        std::fs::write(&path, to_bytes(&stored).expect("encode receipt")).expect("write receipt");
        let cursors = cursor_snapshot(LaneId::new(1), 2, 3);

        let report = prune_spool(dir.path(), &cursors);

        assert!(!path.exists(), "valid stale receipt should be removed");
        assert_eq!(
            report,
            DaReceiptPruneReport {
                scanned_receipts: 1,
                removed_stale: 1,
                ..DaReceiptPruneReport::default()
            }
        );
    }

    #[test]
    fn prune_spool_skips_filename_body_mismatch() {
        let dir = tempdir().expect("tempdir");
        let receipt = sample_receipt(1, 2, 3);
        let stored = StoredDaReceipt {
            version: STORED_RECEIPT_VERSION,
            sequence: 3,
            receipt: receipt.clone(),
        };
        let path = dir.path().join(receipt_file_name(&receipt, 4, [0x99; 32]));
        std::fs::write(&path, to_bytes(&stored).expect("encode receipt"))
            .expect("write mismatched receipt");
        let cursors = cursor_snapshot(LaneId::new(1), 2, 3);

        let report = prune_spool(dir.path(), &cursors);

        assert!(
            path.exists(),
            "mismatched receipt filename/body must not be trusted during prune"
        );
        assert_eq!(
            report,
            DaReceiptPruneReport {
                scanned_receipts: 1,
                skipped_invalid: 1,
                ..DaReceiptPruneReport::default()
            }
        );
        assert!(!report.has_failures());
    }

    #[test]
    fn prune_spool_reports_receipt_shaped_read_failures() {
        let dir = tempdir().expect("tempdir");
        let receipt = sample_receipt(1, 2, 3);
        let path = dir.path().join(receipt_file_name(&receipt, 3, [0x99; 32]));
        std::fs::create_dir(&path).expect("create unreadable receipt-shaped directory");
        let cursors = cursor_snapshot(LaneId::new(1), 2, 3);

        let report = prune_spool(dir.path(), &cursors);

        assert_eq!(
            report,
            DaReceiptPruneReport {
                scanned_receipts: 1,
                read_failures: 1,
                ..DaReceiptPruneReport::default()
            }
        );
        assert!(report.has_failures());
    }

    #[cfg(unix)]
    #[test]
    fn prune_spool_rejects_receipt_shaped_symlink() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().expect("tempdir");
        let receipt = sample_receipt(1, 2, 3);
        let stored = StoredDaReceipt {
            version: STORED_RECEIPT_VERSION,
            sequence: 3,
            receipt: receipt.clone(),
        };
        let target = dir.path().join("receipt-target.norito");
        std::fs::write(&target, to_bytes(&stored).expect("encode receipt"))
            .expect("write symlink target receipt");
        let path = dir.path().join(receipt_file_name(&receipt, 3, [0x99; 32]));
        symlink(&target, &path).expect("create receipt-shaped symlink");
        let cursors = cursor_snapshot(LaneId::new(1), 2, 3);

        let report = prune_spool(dir.path(), &cursors);

        assert!(
            path.exists(),
            "receipt-shaped symlink must not be removed as stale"
        );
        assert!(
            target.exists(),
            "receipt-shaped symlink target must not be removed"
        );
        assert_eq!(
            report,
            DaReceiptPruneReport {
                scanned_receipts: 1,
                read_failures: 1,
                ..DaReceiptPruneReport::default()
            }
        );
        assert!(report.has_failures());
    }

    #[cfg(unix)]
    #[test]
    fn prune_spool_rejects_spool_dir_symlink() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().expect("tempdir");
        let target = dir.path().join("receipt-prune-target");
        std::fs::create_dir(&target).expect("create target directory");
        let spool = dir.path().join("receipt-prune-link");
        symlink(&target, &spool).expect("create receipt prune spool symlink");
        let cursors = BTreeMap::new();

        let report = prune_spool(&spool, &cursors);

        assert_eq!(
            report,
            DaReceiptPruneReport {
                read_dir_failed: true,
                ..DaReceiptPruneReport::default()
            }
        );
        assert!(report.has_failures());
        assert!(
            std::fs::symlink_metadata(&spool)
                .expect("inspect spool symlink")
                .file_type()
                .is_symlink(),
            "failed prune should leave spool symlink visible"
        );
        assert!(
            target.exists(),
            "spool symlink target should not be removed"
        );
    }

    #[test]
    fn prune_spool_reports_read_dir_failures() {
        let dir = tempdir().expect("tempdir");
        let spool_path = dir.path().join("not-a-directory");
        std::fs::write(&spool_path, b"file").expect("write non-directory spool path");
        let cursors = BTreeMap::new();

        let report = prune_spool(&spool_path, &cursors);

        assert_eq!(
            report,
            DaReceiptPruneReport {
                read_dir_failed: true,
                ..DaReceiptPruneReport::default()
            }
        );
        assert!(report.has_failures());
    }

    #[test]
    fn plan_committable_receipts_detects_gaps() {
        let lane = LaneId::new(7);
        let receipt = DaReceiptEntry {
            lane_epoch: LaneEpoch::new(lane, 1),
            sequence: 5,
            manifest_hash: ManifestDigest::new([0x11; 32]),
            receipt: sample_receipt(lane.as_u32(), 1, 5),
        };
        let lane_config = lane_config_for(lane);
        let sealed = BTreeSet::new();
        let cursors = cursor_snapshot(lane, 1, 1);

        let result = plan_committable_receipts(&lane_config, &cursors, &sealed, vec![receipt]);
        assert!(matches!(
            result,
            Err(DaReceiptQueueError::MissingSequence { expected: 2, .. })
        ));
    }

    #[test]
    fn plan_committable_receipts_skips_unknown_lane() {
        let known_lane = LaneId::new(0);
        let unknown_lane = LaneId::new(7);
        let unknown_receipt = sample_receipt(unknown_lane.as_u32(), 1, 1);
        let known_receipt = sample_receipt(known_lane.as_u32(), 1, 1);
        let unknown_entry = DaReceiptEntry {
            lane_epoch: LaneEpoch::new(unknown_lane, 1),
            sequence: 1,
            manifest_hash: ManifestDigest::new(*unknown_receipt.manifest_hash.as_bytes()),
            receipt: unknown_receipt,
        };
        let known_entry = DaReceiptEntry {
            lane_epoch: LaneEpoch::new(known_lane, 1),
            sequence: 1,
            manifest_hash: ManifestDigest::new(*known_receipt.manifest_hash.as_bytes()),
            receipt: known_receipt,
        };
        let lane_config = lane_config_for(known_lane);

        let result = plan_committable_receipts(
            &lane_config,
            &BTreeMap::new(),
            &BTreeSet::new(),
            vec![unknown_entry, known_entry],
        )
        .expect("unknown lane receipts should be skipped");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].lane_epoch.lane_id, known_lane);
    }

    #[test]
    fn plan_committable_receipts_skips_sealed_and_stale() {
        let lane = LaneId::new(3);
        let base_receipt = sample_receipt(lane.as_u32(), 9, 1);
        let mut sealed = BTreeSet::new();
        sealed.insert(
            iroha_data_model::da::commitment::DaCommitmentKey::from_record(&sample_record(
                &base_receipt,
                1,
            )),
        );
        let receipts = vec![
            DaReceiptEntry {
                lane_epoch: LaneEpoch::new(lane, 9),
                sequence: 1,
                manifest_hash: ManifestDigest::new(*base_receipt.manifest_hash.as_bytes()),
                receipt: base_receipt.clone(),
            },
            DaReceiptEntry {
                lane_epoch: LaneEpoch::new(lane, 9),
                sequence: 2,
                manifest_hash: ManifestDigest::new(*base_receipt.manifest_hash.as_bytes()),
                receipt: sample_receipt(lane.as_u32(), 9, 2),
            },
        ];

        let lane_config = lane_config_for(lane);
        let cursors = cursor_snapshot(lane, 9, 1);
        let planned = plan_committable_receipts(&lane_config, &cursors, &sealed, receipts).unwrap();
        assert_eq!(planned.len(), 1);
        assert_eq!(planned[0].sequence, 2);
    }

    #[test]
    fn plan_committable_receipts_skips_sealed_sequence_with_ticket_mismatch() {
        let lane = LaneId::new(5);
        let receipt = sample_receipt(lane.as_u32(), 2, 1);
        let mut sealed_record = sample_record(&receipt, 1);
        sealed_record.storage_ticket = StorageTicketId::new([0x99; 32]);
        let mut sealed = BTreeSet::new();
        sealed
            .insert(iroha_data_model::da::commitment::DaCommitmentKey::from_record(&sealed_record));

        let lane_config = lane_config_for(lane);
        let planned = plan_committable_receipts(
            &lane_config,
            &BTreeMap::new(),
            &sealed,
            vec![DaReceiptEntry {
                lane_epoch: LaneEpoch::new(lane, 2),
                sequence: 1,
                manifest_hash: ManifestDigest::new(*receipt.manifest_hash.as_bytes()),
                receipt,
            }],
        )
        .expect("plan receipts");

        assert!(planned.is_empty());
    }

    #[test]
    fn align_commitments_enforces_manifest_match() {
        let lane = LaneId::new(4);
        let receipt = sample_receipt(lane.as_u32(), 2, 1);
        let mut bad_record = sample_record(&receipt, 1);
        bad_record.manifest_hash = ManifestDigest::new([0x99; 32]);

        let entries = vec![DaReceiptEntry {
            lane_epoch: LaneEpoch::new(lane, 2),
            sequence: 1,
            manifest_hash: ManifestDigest::new(*receipt.manifest_hash.as_bytes()),
            receipt,
        }];
        let result = align_commitments_for_receipts(&entries, &[bad_record]);
        assert!(matches!(
            result,
            Err(DaReceiptQueueError::CommitmentManifestMismatch { .. })
        ));
    }

    #[test]
    fn align_commitments_enforces_storage_ticket_match() {
        let lane = LaneId::new(4);
        let receipt = sample_receipt(lane.as_u32(), 2, 1);
        let mut bad_record = sample_record(&receipt, 1);
        bad_record.storage_ticket = StorageTicketId::new([0x99; 32]);

        let entries = vec![DaReceiptEntry {
            lane_epoch: LaneEpoch::new(lane, 2),
            sequence: 1,
            manifest_hash: ManifestDigest::new(*receipt.manifest_hash.as_bytes()),
            receipt,
        }];
        let result = align_commitments_for_receipts(&entries, &[bad_record]);
        assert!(matches!(
            result,
            Err(DaReceiptQueueError::CommitmentTicketMismatch { .. })
        ));
    }

    #[test]
    fn align_commitments_rejects_duplicate_commitment_key() {
        let lane = LaneId::new(4);
        let receipt = sample_receipt(lane.as_u32(), 2, 1);
        let record = sample_record(&receipt, 1);
        let entries = vec![DaReceiptEntry {
            lane_epoch: LaneEpoch::new(lane, 2),
            sequence: 1,
            manifest_hash: ManifestDigest::new(*receipt.manifest_hash.as_bytes()),
            receipt,
        }];

        let result = align_commitments_for_receipts(&entries, &[record.clone(), record]);

        assert!(matches!(
            result,
            Err(DaReceiptQueueError::DuplicateCommitment { lane: dup_lane, sequence: 1, .. })
                if dup_lane == lane
        ));
    }

    #[test]
    fn align_commitments_ignores_duplicate_unplanned_commitment_key() {
        let lane = LaneId::new(4);
        let receipt = sample_receipt(lane.as_u32(), 2, 1);
        let record = sample_record(&receipt, 1);
        let unplanned_receipt = sample_receipt(8, 3, 9);
        let unplanned_record = sample_record(&unplanned_receipt, 9);
        let entries = vec![DaReceiptEntry {
            lane_epoch: LaneEpoch::new(lane, 2),
            sequence: 1,
            manifest_hash: ManifestDigest::new(*receipt.manifest_hash.as_bytes()),
            receipt,
        }];

        let aligned = align_commitments_for_receipts(
            &entries,
            &[unplanned_record.clone(), record.clone(), unplanned_record],
        )
        .expect("unplanned duplicate commitments should not block receipt alignment");

        assert_eq!(aligned, vec![record]);
    }

    #[test]
    fn align_commitments_detects_missing_record() {
        let lane = LaneId::new(5);
        let receipt = sample_receipt(lane.as_u32(), 3, 7);
        let entries = vec![DaReceiptEntry {
            lane_epoch: LaneEpoch::new(lane, 3),
            sequence: 7,
            manifest_hash: ManifestDigest::new(*receipt.manifest_hash.as_bytes()),
            receipt,
        }];
        let result = align_commitments_for_receipts(&entries, &[]);
        assert!(matches!(
            result,
            Err(DaReceiptQueueError::MissingCommitment { lane: missing_lane, sequence: 7, .. })
                if missing_lane == lane
        ));
    }

    #[test]
    fn plan_committable_receipts_flags_manifest_conflict() {
        let lane = LaneId::new(6);
        let mut receipt_a = sample_receipt(lane.as_u32(), 1, 1);
        receipt_a.manifest_hash = BlobDigest::new([0x01; 32]);
        let mut receipt_b = sample_receipt(lane.as_u32(), 1, 1);
        receipt_b.manifest_hash = BlobDigest::new([0x02; 32]);

        let entries = vec![
            DaReceiptEntry {
                lane_epoch: LaneEpoch::new(lane, 1),
                sequence: 1,
                manifest_hash: ManifestDigest::new(*receipt_a.manifest_hash.as_bytes()),
                receipt: receipt_a,
            },
            DaReceiptEntry {
                lane_epoch: LaneEpoch::new(lane, 1),
                sequence: 1,
                manifest_hash: ManifestDigest::new(*receipt_b.manifest_hash.as_bytes()),
                receipt: receipt_b,
            },
        ];
        let lane_config = lane_config_for(lane);
        let sealed = BTreeSet::new();
        let cursors = BTreeMap::new();

        let result = plan_committable_receipts(&lane_config, &cursors, &sealed, entries);
        assert!(matches!(
            result,
            Err(DaReceiptQueueError::ManifestConflict { sequence: 1, .. })
        ));
    }

    #[test]
    fn plan_committable_receipts_flags_ticket_conflict() {
        let lane = LaneId::new(6);
        let receipt_a = sample_receipt(lane.as_u32(), 1, 1);
        let mut receipt_b = sample_receipt(lane.as_u32(), 1, 1);
        receipt_b.storage_ticket = StorageTicketId::new([0xAA; 32]);

        let entries = vec![
            DaReceiptEntry {
                lane_epoch: LaneEpoch::new(lane, 1),
                sequence: 1,
                manifest_hash: ManifestDigest::new(*receipt_a.manifest_hash.as_bytes()),
                receipt: receipt_a,
            },
            DaReceiptEntry {
                lane_epoch: LaneEpoch::new(lane, 1),
                sequence: 1,
                manifest_hash: ManifestDigest::new(*receipt_b.manifest_hash.as_bytes()),
                receipt: receipt_b,
            },
        ];
        let lane_config = lane_config_for(lane);
        let sealed = BTreeSet::new();
        let cursors = BTreeMap::new();

        let result = plan_committable_receipts(&lane_config, &cursors, &sealed, entries);
        assert!(matches!(
            result,
            Err(DaReceiptQueueError::StorageTicketConflict { sequence: 1, .. })
        ));
    }

    #[test]
    fn plan_committable_receipts_flags_same_manifest_receipt_evidence_conflict() {
        let lane = LaneId::new(6);
        let receipt_a = sample_receipt(lane.as_u32(), 1, 1);
        let mut receipt_b = sample_receipt(lane.as_u32(), 1, 1);
        receipt_b.manifest_hash = receipt_a.manifest_hash;
        receipt_b.storage_ticket = receipt_a.storage_ticket;
        receipt_b.blob_hash = BlobDigest::new([0x42; 32]);

        let entries = vec![
            DaReceiptEntry {
                lane_epoch: LaneEpoch::new(lane, 1),
                sequence: 1,
                manifest_hash: ManifestDigest::new(*receipt_a.manifest_hash.as_bytes()),
                receipt: receipt_a,
            },
            DaReceiptEntry {
                lane_epoch: LaneEpoch::new(lane, 1),
                sequence: 1,
                manifest_hash: ManifestDigest::new(*receipt_b.manifest_hash.as_bytes()),
                receipt: receipt_b,
            },
        ];
        let lane_config = lane_config_for(lane);
        let sealed = BTreeSet::new();
        let cursors = BTreeMap::new();

        let result = plan_committable_receipts(&lane_config, &cursors, &sealed, entries);
        assert!(matches!(
            result,
            Err(DaReceiptQueueError::ReceiptEvidenceConflict { sequence: 1, .. })
        ));
    }
}
