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
    da::{commitment::DaCommitmentRecord, ingest::DaIngestReceipt},
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
    /// to the stored cursor for the `(lane, epoch)`.
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
    /// Returns [`DaReceiptCursorError`] when any record regresses relative to its cursor.
    pub fn record_bundle(
        &mut self,
        block_height: u64,
        records: &[DaCommitmentRecord],
    ) -> Result<Vec<(LaneEpoch, u64)>, DaReceiptCursorError> {
        let mut advanced = Vec::new();
        for record in records {
            let lane_epoch = LaneEpoch::new(record.lane_id, record.epoch);
            self.record(lane_epoch, record.sequence, block_height)?;
            advanced.push((lane_epoch, record.sequence));
        }
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
/// Returns a [`DaReceiptSpoolError`] if the directory cannot be read or a receipt fails to decode.
pub fn load_receipt_entries(spool_dir: &Path) -> Result<Vec<DaReceiptEntry>, DaReceiptSpoolError> {
    if !spool_dir.exists() {
        return Ok(Vec::new());
    }

    let mut receipts = Vec::new();
    let dir_entries =
        std::fs::read_dir(spool_dir).map_err(|source| DaReceiptSpoolError::ReadDir {
            path: spool_dir.to_path_buf(),
            source,
        })?;

    for entry in dir_entries {
        let entry = match entry {
            Ok(value) => value,
            Err(source) => {
                iroha_logger::warn!(?source, "failed to read DA receipt spool entry");
                continue;
            }
        };
        let path = entry.path();
        if !is_da_receipt_file(&path) {
            continue;
        }

        let data = match std::fs::read(&path) {
            Ok(buf) => buf,
            Err(source) => {
                iroha_logger::warn!(?source, path = %path.display(), "failed to read DA receipt file");
                return Err(DaReceiptSpoolError::ReadFile { path, source });
            }
        };

        match decode_receipt(&data, &path) {
            Ok(entry) => receipts.push(entry),
            Err(err) => return Err(err),
        }
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

fn is_da_receipt_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("da-receipt-") && name.ends_with(".norito"))
}

fn decode_receipt(data: &[u8], path: &Path) -> Result<DaReceiptEntry, DaReceiptSpoolError> {
    let stored = norito::decode_from_bytes::<StoredDaReceipt>(data).map_err(|source| {
        DaReceiptSpoolError::Decode {
            path: path.to_path_buf(),
            source,
        }
    })?;
    let StoredDaReceipt {
        sequence, receipt, ..
    } = stored;
    let lane_epoch = LaneEpoch::new(receipt.lane_id, receipt.epoch);
    Ok(DaReceiptEntry {
        lane_epoch,
        sequence,
        manifest_hash: ManifestDigest::new(*receipt.manifest_hash.as_bytes()),
        receipt,
    })
}

/// Canonicalize and filter receipts against the current cursor and sealed set.
///
/// This enforces monotonic sequencing per `(lane, epoch)`, drops stale or sealed
/// receipts, and returns the next contiguous slice that must be sealed before a
/// later sequence can appear.
///
/// # Errors
///
/// Returns a [`DaReceiptQueueError`] when lanes are unknown, manifests conflict,
/// or a sequence gap is detected after the current cursor.
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
            continue;
        }

        let key = iroha_data_model::da::commitment::DaCommitmentKey {
            lane_id: entry.lane_epoch.lane_id,
            epoch: entry.lane_epoch.epoch,
            sequence: entry.sequence,
            storage_ticket: entry.receipt.storage_ticket,
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

    let mut by_key: BTreeMap<(LaneEpoch, u64), &DaCommitmentRecord> = BTreeMap::new();
    for record in commitments {
        let lane_epoch = LaneEpoch::new(record.lane_id, record.epoch);
        let key = (lane_epoch, record.sequence);
        by_key.entry(key).or_insert(record);
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
        aligned.push((*record).clone());
    }

    Ok(aligned)
}

/// Remove stale receipts from the spool based on the committed cursor snapshot.
///
/// This is a best-effort cleanup used to keep the spool bounded; failures are
/// logged but do not abort callers.
pub fn prune_spool(spool_dir: &Path, cursors: &BTreeMap<LaneEpoch, u64>) {
    if !spool_dir.exists() {
        return;
    }

    let Ok(entries) = std::fs::read_dir(spool_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !is_da_receipt_file(&path) {
            continue;
        }
        let Ok(data) = std::fs::read(&path) else {
            continue;
        };
        let Ok(stored) = norito::decode_from_bytes::<StoredDaReceipt>(&data) else {
            continue;
        };
        let lane_epoch = LaneEpoch::new(stored.receipt.lane_id, stored.receipt.epoch);
        if let Some(highest) = cursors.get(&lane_epoch) {
            if stored.sequence <= *highest {
                if let Err(err) = std::fs::remove_file(&path) {
                    iroha_logger::debug!(
                        ?err,
                        path = %path.display(),
                        "failed to prune stale DA receipt file"
                    );
                }
            }
        }
    }
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
    use std::{collections::BTreeSet, num::NonZeroU32};

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
    fn load_receipt_entries_reads_spool() {
        let dir = tempdir().expect("tempdir");
        let receipt = sample_receipt(1, 2, 3);
        let stored = StoredDaReceipt {
            version: 1,
            sequence: 3,
            receipt: receipt.clone(),
        };
        let bytes = to_bytes(&stored).expect("encode");
        let path = dir
            .path()
            .join("da-receipt-00000001-0000000000000002-0000000000000003-test.norito");
        std::fs::write(&path, bytes).expect("write");

        let entries = load_receipt_entries(dir.path()).expect("load entries");
        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry.sequence, 3);
        assert_eq!(entry.lane_epoch, LaneEpoch::new(LaneId::new(1), 2));
        assert_eq!(entry.receipt, receipt);
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
}
