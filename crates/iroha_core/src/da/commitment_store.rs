//! In-memory DA commitment index used while WSV wiring lands.
//!
//! This store keeps a deterministic view of commitments keyed by manifest hash
//! and `(lane_id, epoch, sequence)` so Torii/query paths can be wired without
//! depending on persistent storage. Once WSV plumbing is in place this module
//! should be replaced by a durable column-family backed index.

use std::collections::{BTreeMap, BTreeSet};

use iroha_data_model::{
    da::commitment::{
        DaCommitmentBundle, DaCommitmentKey, DaCommitmentLocation, DaCommitmentRecord,
        DaCommitmentWithLocation,
    },
    nexus::LaneId,
    sorafs::pin_registry::ManifestDigest,
};

/// Simple index over DA commitments.
#[derive(Debug, Default)]
pub struct DaCommitmentStore {
    by_manifest: BTreeMap<ManifestDigest, DaCommitmentWithLocation>,
    by_lane_epoch: BTreeMap<(u32, u64, u64), DaCommitmentWithLocation>,
    by_block: BTreeMap<u64, DaCommitmentBundle>,
    seen_keys: BTreeSet<DaCommitmentKey>,
}

impl DaCommitmentStore {
    /// Build a store from an existing bundle, preserving deterministic order.
    #[must_use]
    pub fn from_bundle(bundle: &[DaCommitmentRecord]) -> Self {
        Self::from_bundle_at_height(bundle, 0)
    }

    /// Build a store from a bundle observed at the given block height.
    #[must_use]
    pub fn from_bundle_at_height(bundle: &[DaCommitmentRecord], block_height: u64) -> Self {
        let mut store = Self::default();
        let bundle = DaCommitmentBundle::new(bundle.to_vec());
        store.insert_bundle(block_height, bundle);
        store
    }

    /// Insert an entire bundle captured at `block_height`.
    pub fn insert_bundle(&mut self, block_height: u64, bundle: DaCommitmentBundle) {
        let mut pushed_any = false;
        for (idx, record) in bundle.commitments.iter().enumerate() {
            let location = DaCommitmentLocation {
                block_height,
                index_in_bundle: u32::try_from(idx)
                    .expect("DA commitment bundle size must fit within u32::MAX"),
            };
            pushed_any |= self.insert(record, location);
        }

        if pushed_any {
            self.by_block.insert(block_height, bundle);
        }
    }

    /// Insert a commitment if it has not been seen before.
    ///
    /// Returns `true` if the record was inserted into the index.
    pub fn insert(&mut self, record: &DaCommitmentRecord, location: DaCommitmentLocation) -> bool {
        let key = DaCommitmentKey::from_record(record);
        if !self.seen_keys.insert(key) {
            return false;
        }

        let with_location = DaCommitmentWithLocation {
            commitment: record.clone(),
            location,
        };
        self.by_manifest
            .insert(record.manifest_hash, with_location.clone());
        self.by_lane_epoch.insert(
            (record.lane_id.as_u32(), record.epoch, record.sequence),
            with_location,
        );
        true
    }

    /// Lookup by manifest hash.
    #[must_use]
    pub fn get_by_manifest(&self, digest: &ManifestDigest) -> Option<&DaCommitmentWithLocation> {
        self.by_manifest.get(digest)
    }

    /// Lookup by `(lane_id, epoch, sequence)`.
    #[must_use]
    pub fn get_by_lane_epoch_sequence(
        &self,
        lane_id: u32,
        epoch: u64,
        sequence: u64,
    ) -> Option<&DaCommitmentWithLocation> {
        self.by_lane_epoch.get(&(lane_id, epoch, sequence))
    }

    /// Return all commitments ordered by `(lane_id, epoch, sequence)`.
    pub fn all_sorted(&self) -> impl Iterator<Item = &DaCommitmentWithLocation> {
        self.by_lane_epoch.values()
    }

    /// Retrieve the stored bundle for a given block height.
    #[must_use]
    pub fn bundle_at(&self, block_height: u64) -> Option<&DaCommitmentBundle> {
        self.by_block.get(&block_height)
    }

    /// Iterate over stored bundles keyed by their originating block height.
    ///
    /// Bundles are ordered by block height.
    pub fn bundles(&self) -> impl Iterator<Item = (&u64, &DaCommitmentBundle)> {
        self.by_block.iter()
    }

    /// Drop commitments belonging to retired lanes.
    pub fn prune_lanes(&mut self, retired: &BTreeSet<LaneId>) {
        if retired.is_empty() {
            return;
        }
        self.by_manifest
            .retain(|_, entry| !retired.contains(&entry.commitment.lane_id));
        self.by_lane_epoch
            .retain(|(lane, _, _), _| !retired.contains(&LaneId::new(*lane)));
        self.by_block.retain(|_, bundle| {
            let filtered: Vec<_> = bundle
                .commitments
                .iter()
                .filter(|record| !retired.contains(&record.lane_id))
                .cloned()
                .collect();
            if filtered.is_empty() {
                return false;
            }
            if filtered.len() != bundle.commitments.len() {
                *bundle = DaCommitmentBundle::new(filtered);
            }
            true
        });
        self.seen_keys.retain(|key| !retired.contains(&key.lane_id));
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Hash, Signature};
    use iroha_data_model::{
        da::{
            commitment::{
                DaCommitmentBundle, DaCommitmentLocation, DaProofScheme, KzgCommitment,
                RetentionClass,
            },
            types::{BlobDigest, StorageTicketId},
        },
        nexus::LaneId,
    };

    use super::*;

    fn sample_record(id: u32, epoch: u64, seq: u64) -> DaCommitmentRecord {
        let id_u8 = u8::try_from(id).expect("lane id fits in u8 for test");
        let epoch_u8 = u8::try_from(epoch).unwrap_or(u8::MAX);
        let seq_u8 = u8::try_from(seq).unwrap_or(u8::MAX);
        DaCommitmentRecord::new(
            LaneId::new(id),
            epoch,
            seq,
            BlobDigest::new([id_u8; 32]),
            ManifestDigest::new([epoch_u8; 32]),
            DaProofScheme::MerkleSha256,
            Hash::prehashed([seq_u8; 32]),
            Some(KzgCommitment::new([0x11; 48])),
            None,
            RetentionClass::default(),
            StorageTicketId::new([0x22; 32]),
            Signature::from_bytes(&[0x33; 64]),
        )
    }

    #[test]
    fn inserts_and_deduplicates() {
        let mut store = DaCommitmentStore::default();
        let record = sample_record(1, 2, 3);
        let loc = DaCommitmentLocation {
            block_height: 7,
            index_in_bundle: 0,
        };
        let mut dup = record.clone();
        dup.storage_ticket = StorageTicketId::new([0x44; 32]);
        dup.manifest_hash = ManifestDigest::new([0x55; 32]);

        assert!(store.insert(&record, loc));
        assert!(!store.insert(&dup, loc));

        assert_eq!(store.all_sorted().count(), 1);
        let stored = store.get_by_manifest(&record.manifest_hash).unwrap();
        assert_eq!(stored.commitment, record);
        assert_eq!(stored.location.block_height, 7);
    }

    #[test]
    fn orders_by_lane_epoch_sequence() {
        let mut store = DaCommitmentStore::default();
        let a = sample_record(2, 1, 5);
        let b = sample_record(1, 1, 1);
        let c = sample_record(1, 2, 0);

        let base_loc = DaCommitmentLocation {
            block_height: 3,
            index_in_bundle: 0,
        };
        store.insert(
            &a,
            DaCommitmentLocation {
                index_in_bundle: 2,
                ..base_loc
            },
        );
        store.insert(
            &b,
            DaCommitmentLocation {
                index_in_bundle: 0,
                ..base_loc
            },
        );
        store.insert(
            &c,
            DaCommitmentLocation {
                index_in_bundle: 1,
                ..base_loc
            },
        );

        let ordered: Vec<_> = store
            .all_sorted()
            .map(|rec| {
                (
                    rec.commitment.lane_id.as_u32(),
                    rec.commitment.epoch,
                    rec.commitment.sequence,
                    rec.location.index_in_bundle,
                )
            })
            .collect();
        assert_eq!(ordered, vec![(1, 1, 1, 0), (1, 2, 0, 1), (2, 1, 5, 2)]);

        assert_eq!(
            store
                .get_by_lane_epoch_sequence(1, 2, 0)
                .map(|r| r.commitment.manifest_hash),
            Some(c.manifest_hash)
        );
    }

    #[test]
    fn builds_from_bundle() {
        let records = vec![sample_record(3, 4, 5), sample_record(2, 1, 0)];
        let store = DaCommitmentStore::from_bundle_at_height(&records, 11);
        assert_eq!(store.all_sorted().count(), 2);
        let fetched = store.get_by_lane_epoch_sequence(2, 1, 0).unwrap();
        assert_eq!(fetched.location.block_height, 11);
        assert_eq!(fetched.location.index_in_bundle, 1);
    }

    #[test]
    fn prunes_retired_lanes() {
        let mut store = DaCommitmentStore::default();
        let record_a = sample_record(0, 1, 1);
        let record_b = sample_record(1, 1, 2);
        store.insert_bundle(
            1,
            DaCommitmentBundle::new(vec![record_a.clone(), record_b.clone()]),
        );

        let retired = BTreeSet::from([LaneId::new(1)]);
        store.prune_lanes(&retired);

        assert!(
            store.get_by_lane_epoch_sequence(0, 1, 1).is_some(),
            "lane 0 entry kept"
        );
        assert!(
            store.get_by_lane_epoch_sequence(1, 1, 2).is_none(),
            "retired lane removed"
        );
        let bundle = store.bundle_at(1).expect("bundle retained");
        assert_eq!(bundle.commitments.len(), 1);
    }

    #[test]
    fn stores_bundles_per_block() {
        let mut store = DaCommitmentStore::default();
        let bundle = DaCommitmentBundle::new(vec![sample_record(1, 1, 1)]);
        store.insert_bundle(5, bundle.clone());

        let stored_bundle = store.bundle_at(5).expect("bundle present");
        assert_eq!(stored_bundle.commitments.len(), 1);
        assert_eq!(
            stored_bundle.commitments[0].manifest_hash,
            bundle.commitments[0].manifest_hash
        );
    }
}
