//! In-memory index for confidential-compute DA receipts.
//!
//! Each entry records the storage ticket and digest information for a
//! confidential lane alongside the resolved policy version. Payload bytes are
//! never kept here; only deterministic metadata derived from commitments.

use std::collections::{BTreeMap, BTreeSet};

use iroha_data_model::{
    da::{
        commitment::{DaCommitmentKey, DaCommitmentLocation, DaCommitmentRecord},
        confidential_compute::{ConfidentialComputeMechanism, ConfidentialComputePolicy},
        types::{BlobDigest, StorageTicketId},
    },
    nexus::LaneId,
    sorafs::pin_registry::ManifestDigest,
};

/// Deterministic record for a confidential-compute commitment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfidentialComputeReceipt {
    /// Lane identifier.
    pub lane_id: LaneId,
    /// Epoch associated with the receipt.
    pub epoch: u64,
    /// Sequence number associated with the receipt.
    pub sequence: u64,
    /// Digest of the encrypted payload.
    pub payload_digest: BlobDigest,
    /// Manifest digest anchoring the payload.
    pub manifest_digest: ManifestDigest,
    /// Ticket referencing the sealed payload in `SoraFS`.
    pub storage_ticket: StorageTicketId,
    /// Policy mechanism used for the lane.
    pub mechanism: ConfidentialComputeMechanism,
    /// Policy key/share version expected for the lane.
    pub key_version: u32,
    /// Audience labels allowed to fetch the payload.
    pub allowed_audiences: Vec<String>,
}

/// Receipt paired with its location in the block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfidentialComputeWithLocation {
    /// Receipt contents.
    pub receipt: ConfidentialComputeReceipt,
    /// Location in the block bundle.
    pub location: DaCommitmentLocation,
}

/// In-memory index over confidential-compute receipts.
#[derive(Debug, Default)]
pub struct ConfidentialComputeStore {
    by_lane_epoch: BTreeMap<(u32, u64, u64), ConfidentialComputeWithLocation>,
    by_block: BTreeMap<u64, Vec<ConfidentialComputeWithLocation>>,
    seen_keys: BTreeSet<DaCommitmentKey>,
}

impl ConfidentialComputeStore {
    /// Insert a record derived from a DA commitment.
    pub fn insert(
        &mut self,
        record: &DaCommitmentRecord,
        location: DaCommitmentLocation,
        policy: &ConfidentialComputePolicy,
    ) -> bool {
        let key = DaCommitmentKey::from_record(record);
        if !self.seen_keys.insert(key) {
            return false;
        }

        let receipt = ConfidentialComputeReceipt {
            lane_id: record.lane_id,
            epoch: record.epoch,
            sequence: record.sequence,
            payload_digest: record.client_blob_id,
            manifest_digest: record.manifest_hash,
            storage_ticket: record.storage_ticket,
            mechanism: policy.mechanism,
            key_version: policy.key_version,
            allowed_audiences: policy.allowed_audiences.clone(),
        };
        let located = ConfidentialComputeWithLocation { receipt, location };
        self.by_lane_epoch.insert(
            (record.lane_id.as_u32(), record.epoch, record.sequence),
            located.clone(),
        );
        self.by_block
            .entry(location.block_height)
            .or_default()
            .push(located);
        true
    }

    /// Retrieve a receipt by `(lane, epoch, sequence)`.
    #[must_use]
    pub fn get_by_lane_epoch_sequence(
        &self,
        lane_id: u32,
        epoch: u64,
        sequence: u64,
    ) -> Option<&ConfidentialComputeWithLocation> {
        self.by_lane_epoch.get(&(lane_id, epoch, sequence))
    }

    /// Receipts stored for a specific block height.
    #[must_use]
    pub fn receipts_at(&self, block_height: u64) -> Option<&[ConfidentialComputeWithLocation]> {
        self.by_block.get(&block_height).map(Vec::as_slice)
    }

    /// Return all receipts ordered by `(lane, epoch, sequence)`.
    pub fn all_sorted(&self) -> impl Iterator<Item = &ConfidentialComputeWithLocation> {
        self.by_lane_epoch.values()
    }

    /// Returns true when no receipts are stored.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_lane_epoch.is_empty()
    }

    /// Drop cached receipts belonging to retired lanes.
    pub fn prune_lanes(&mut self, retired: &BTreeSet<LaneId>) {
        if retired.is_empty() {
            return;
        }
        self.by_lane_epoch
            .retain(|(lane, _, _), _| !retired.contains(&LaneId::new(*lane)));
        self.by_block.retain(|_, receipts| {
            receipts.retain(|entry| !retired.contains(&entry.receipt.lane_id));
            !receipts.is_empty()
        });
        self.seen_keys.retain(|key| !retired.contains(&key.lane_id));
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Hash, Signature};
    use iroha_data_model::{
        da::{
            commitment::{DaProofScheme, KzgCommitment, RetentionClass},
            types::{BlobDigest, StorageTicketId},
        },
        nexus::LaneId,
    };

    use super::*;

    fn policy(version: u32) -> ConfidentialComputePolicy {
        ConfidentialComputePolicy::new(
            ConfidentialComputeMechanism::Encryption,
            version,
            vec!["ops".to_string()],
        )
    }

    fn record(lane: u32, epoch: u64, sequence: u64) -> DaCommitmentRecord {
        let lane_byte = u8::try_from(lane).expect("lane fits in u8 for test fixture");
        let seq_byte = u8::try_from(sequence).expect("sequence fits in u8 for test fixture");
        let epoch_byte = u8::try_from(epoch).expect("epoch fits in u8 for test fixture");
        DaCommitmentRecord::new(
            LaneId::new(lane),
            epoch,
            sequence,
            BlobDigest::new([lane_byte; 32]),
            ManifestDigest::new([seq_byte; 32]),
            DaProofScheme::MerkleSha256,
            Hash::prehashed([epoch_byte; 32]),
            Some(KzgCommitment::new([0x11; 48])),
            None,
            RetentionClass::default(),
            StorageTicketId::new([0x22; 32]),
            Signature::from_bytes(&[0x33; 64]),
        )
    }

    #[test]
    fn inserts_and_deduplicates() {
        let mut store = ConfidentialComputeStore::default();
        let policy = policy(1);
        let rec = record(0, 1, 2);
        let mut conflict = rec.clone();
        conflict.storage_ticket = StorageTicketId::new([0x44; 32]);
        let loc = DaCommitmentLocation {
            block_height: 5,
            index_in_bundle: 0,
        };

        assert!(store.insert(&rec, loc, &policy));
        assert!(!store.insert(&conflict, loc, &policy));
        assert!(!store.insert(&rec, loc, &policy));
        assert_eq!(store.all_sorted().count(), 1);
        let fetched = store
            .get_by_lane_epoch_sequence(0, 1, 2)
            .expect("receipt present");
        assert_eq!(fetched.receipt.key_version, 1);
        assert_eq!(fetched.location.block_height, 5);
    }

    #[test]
    fn orders_by_lane_epoch_sequence() {
        let mut store = ConfidentialComputeStore::default();
        let policy = policy(2);

        let records = [record(1, 2, 0), record(1, 1, 1), record(0, 3, 5)];

        for (idx, rec) in records.iter().enumerate() {
            let loc = DaCommitmentLocation {
                block_height: 9,
                index_in_bundle: u32::try_from(idx).expect("test index fits in u32"),
            };
            store.insert(rec, loc, &policy);
        }

        let ordered: Vec<_> = store
            .all_sorted()
            .map(|rec| {
                (
                    rec.receipt.lane_id.as_u32(),
                    rec.receipt.epoch,
                    rec.receipt.sequence,
                )
            })
            .collect();
        assert_eq!(ordered, vec![(0, 3, 5), (1, 1, 1), (1, 2, 0)]);
    }

    #[test]
    fn prunes_retired_lanes() {
        let mut store = ConfidentialComputeStore::default();
        let policy = policy(3);
        let rec_keep = record(0, 1, 1);
        let rec_drop = record(2, 1, 2);

        store.insert(
            &rec_keep,
            DaCommitmentLocation {
                block_height: 4,
                index_in_bundle: 0,
            },
            &policy,
        );
        store.insert(
            &rec_drop,
            DaCommitmentLocation {
                block_height: 4,
                index_in_bundle: 1,
            },
            &policy,
        );

        let retired = BTreeSet::from([LaneId::new(2)]);
        store.prune_lanes(&retired);

        assert!(
            store.get_by_lane_epoch_sequence(0, 1, 1).is_some(),
            "kept lane should remain"
        );
        assert!(
            store.get_by_lane_epoch_sequence(2, 1, 2).is_none(),
            "retired lane should be dropped"
        );
        assert_eq!(store.by_block.len(), 1);
        assert_eq!(
            store.by_block.get(&4).map(Vec::len),
            Some(1),
            "block index retains surviving receipts"
        );
    }
}
