//! Ledger-derived in-memory DA commitment index.
//!
//! This store keeps a deterministic view of commitments keyed by manifest hash
//! and `(lane_id, epoch, sequence)` for Torii/query paths, plus retained
//! committed-identity indexes for consensus validation. Committed block bodies in
//! Kura are the recovery source of truth; [`crate::state::State`] hydrates this
//! projection from those DA commitment bundles during access or rewind.

use std::collections::{BTreeMap, BTreeSet};

use iroha_data_model::{
    da::commitment::{
        DaCommitmentBundle, DaCommitmentKey, DaCommitmentLocation, DaCommitmentRecord,
        DaCommitmentWithLocation,
    },
    da::types::StorageTicketId,
    nexus::LaneId,
    sorafs::pin_registry::ManifestDigest,
};
use tracing::warn;

/// Simple index over DA commitments.
#[derive(Debug, Default)]
pub struct DaCommitmentStore {
    by_manifest: BTreeMap<ManifestDigest, DaCommitmentWithLocation>,
    by_ticket: BTreeMap<StorageTicketId, DaCommitmentWithLocation>,
    by_lane_epoch: BTreeMap<(u32, u64, u64), DaCommitmentWithLocation>,
    by_block: BTreeMap<u64, DaCommitmentBundle>,
    committed_by_key: BTreeMap<DaCommitmentKey, DaCommitmentWithLocation>,
    committed_by_manifest: BTreeMap<ManifestDigest, DaCommitmentWithLocation>,
    committed_by_ticket: BTreeMap<StorageTicketId, DaCommitmentWithLocation>,
}

impl DaCommitmentStore {
    /// Build a store from existing records, preserving canonical deterministic order.
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
        for (idx, record) in bundle.commitments.iter().enumerate() {
            let Some(index_in_bundle) = crate::da::da_bundle_location_index(idx) else {
                warn!(
                    block_height,
                    index = idx,
                    "skipping DA commitment query index with unrepresentable bundle location"
                );
                continue;
            };
            let location = DaCommitmentLocation {
                block_height,
                index_in_bundle,
            };
            let _ = self.insert(record, location);
        }

        if !bundle.commitments.is_empty() {
            self.by_block.insert(block_height, bundle);
        }
    }

    /// Insert a commitment if none of its committed identities have been seen before.
    ///
    /// Returns `true` if the record was inserted into the index.
    pub fn insert(&mut self, record: &DaCommitmentRecord, location: DaCommitmentLocation) -> bool {
        let key = DaCommitmentKey::from_record(record);
        if self.committed_by_key.contains_key(&key)
            || self
                .committed_by_manifest
                .contains_key(&record.manifest_hash)
            || self
                .committed_by_ticket
                .contains_key(&record.storage_ticket)
        {
            return false;
        }

        let with_location = DaCommitmentWithLocation {
            commitment: record.clone(),
            location,
        };
        self.committed_by_key.insert(key, with_location.clone());
        self.committed_by_manifest
            .insert(record.manifest_hash, with_location.clone());
        self.committed_by_ticket
            .insert(record.storage_ticket, with_location.clone());
        self.by_manifest
            .insert(record.manifest_hash, with_location.clone());
        self.by_ticket
            .insert(record.storage_ticket, with_location.clone());
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

    /// Lookup by storage ticket.
    #[must_use]
    pub fn get_by_storage_ticket(
        &self,
        ticket: &StorageTicketId,
    ) -> Option<&DaCommitmentWithLocation> {
        self.by_ticket.get(ticket)
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

    /// Lookup any committed record by `(lane_id, epoch, sequence)`, including retired lanes.
    #[must_use]
    pub fn get_committed_by_key(&self, key: &DaCommitmentKey) -> Option<&DaCommitmentWithLocation> {
        self.committed_by_key.get(key)
    }

    /// Lookup any committed record by manifest hash, including retired lanes.
    #[must_use]
    pub fn get_committed_by_manifest(
        &self,
        digest: &ManifestDigest,
    ) -> Option<&DaCommitmentWithLocation> {
        self.committed_by_manifest.get(digest)
    }

    /// Lookup any committed record by storage ticket, including retired lanes.
    #[must_use]
    pub fn get_committed_by_storage_ticket(
        &self,
        ticket: &StorageTicketId,
    ) -> Option<&DaCommitmentWithLocation> {
        self.committed_by_ticket.get(ticket)
    }

    /// Return whether a record collides with any committed commitment identity.
    #[must_use]
    pub fn contains_record_identity(&self, record: &DaCommitmentRecord) -> bool {
        let key = DaCommitmentKey::from_record(record);
        self.get_committed_by_key(&key).is_some()
            || self
                .get_committed_by_manifest(&record.manifest_hash)
                .is_some()
            || self
                .get_committed_by_storage_ticket(&record.storage_ticket)
                .is_some()
    }

    /// Return all commitments ordered by `(lane_id, epoch, sequence)`.
    pub fn all_sorted(&self) -> impl Iterator<Item = &DaCommitmentWithLocation> {
        self.by_lane_epoch.values()
    }

    /// Number of currently queryable commitment records.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_lane_epoch.len()
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

    /// Drop query indexes belonging to retired lanes.
    ///
    /// Stored block bundles remain byte-for-byte committed bundle snapshots so
    /// proof construction continues to match the block header commitment hash.
    /// Committed identity indexes are retained so validation continues to reject
    /// key, manifest, and storage-ticket reuse after lane retirement.
    pub fn prune_lanes(&mut self, retired: &BTreeSet<LaneId>) {
        if retired.is_empty() {
            return;
        }
        self.by_manifest
            .retain(|_, entry| !retired.contains(&entry.commitment.lane_id));
        self.by_ticket
            .retain(|_, entry| !retired.contains(&entry.commitment.lane_id));
        self.by_lane_epoch
            .retain(|(lane, _, _), _| !retired.contains(&LaneId::new(*lane)));
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
        let mut manifest_hash = [0x44; 32];
        manifest_hash[0] = id_u8;
        manifest_hash[1] = epoch_u8;
        manifest_hash[2] = seq_u8;
        let mut storage_ticket = [0x22; 32];
        storage_ticket[0] = id_u8;
        storage_ticket[1] = epoch_u8;
        storage_ticket[2] = seq_u8;
        DaCommitmentRecord::new(
            LaneId::new(id),
            epoch,
            seq,
            BlobDigest::new([id_u8; 32]),
            ManifestDigest::new(manifest_hash),
            DaProofScheme::MerkleSha256,
            Hash::prehashed([seq_u8; 32]),
            Some(KzgCommitment::new([0x11; 48])),
            None,
            RetentionClass::default(),
            StorageTicketId::new(storage_ticket),
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
        assert_eq!(store.len(), 1);
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
        assert_eq!(store.len(), 2);
        let fetched = store.get_by_lane_epoch_sequence(2, 1, 0).unwrap();
        assert_eq!(fetched.location.block_height, 11);
        assert_eq!(fetched.location.index_in_bundle, 0);
    }

    #[test]
    fn da_bundle_location_index_rejects_unrepresentable_indexes() {
        assert_eq!(crate::da::da_bundle_location_index(0), Some(0));
        assert_eq!(
            crate::da::da_bundle_location_index((u32::MAX as usize) - 1),
            Some(u32::MAX - 1)
        );
        assert_eq!(crate::da::da_bundle_location_index(u32::MAX as usize), None);
        if let Some(index) = (u32::MAX as usize).checked_add(1) {
            assert_eq!(crate::da::da_bundle_location_index(index), None);
        }
    }

    #[test]
    fn prunes_retired_lanes() {
        let mut store = DaCommitmentStore::default();
        let mut record_a = sample_record(0, 1, 1);
        record_a.manifest_hash = ManifestDigest::new([0xA0; 32]);
        record_a.storage_ticket = StorageTicketId::new([0xA1; 32]);
        let mut record_b = sample_record(1, 1, 2);
        record_b.manifest_hash = ManifestDigest::new([0xB0; 32]);
        record_b.storage_ticket = StorageTicketId::new([0xB1; 32]);
        let retired_key = DaCommitmentKey::from_record(&record_b);
        store.insert_bundle(
            1,
            DaCommitmentBundle::new(vec![record_b.clone(), record_a.clone()]),
        );

        let retired = BTreeSet::from([LaneId::new(1)]);
        store.prune_lanes(&retired);

        assert_eq!(store.len(), 1);
        let kept = store
            .get_by_lane_epoch_sequence(0, 1, 1)
            .expect("lane 0 entry kept");
        assert_eq!(kept.location.block_height, 1);
        assert_eq!(kept.location.index_in_bundle, 0);
        assert!(
            store.get_by_lane_epoch_sequence(1, 1, 2).is_none(),
            "retired lane removed"
        );
        assert!(
            store.get_by_manifest(&record_b.manifest_hash).is_none(),
            "retired lane manifest query entry removed"
        );
        assert!(
            store
                .get_by_storage_ticket(&record_b.storage_ticket)
                .is_none(),
            "retired lane ticket query entry removed"
        );
        assert!(
            store.get_committed_by_key(&retired_key).is_some(),
            "retired lane key history retained"
        );
        assert!(
            store
                .get_committed_by_manifest(&record_b.manifest_hash)
                .is_some(),
            "retired lane manifest history retained"
        );
        assert!(
            store
                .get_committed_by_storage_ticket(&record_b.storage_ticket)
                .is_some(),
            "retired lane ticket history retained"
        );
        let mut duplicate_key = record_b.clone();
        duplicate_key.manifest_hash = ManifestDigest::new([0xC0; 32]);
        duplicate_key.storage_ticket = StorageTicketId::new([0xC1; 32]);
        assert!(store.contains_record_identity(&duplicate_key));

        let mut duplicate_manifest = sample_record(2, 2, 0);
        duplicate_manifest.manifest_hash = record_b.manifest_hash;
        duplicate_manifest.storage_ticket = StorageTicketId::new([0xC2; 32]);
        assert!(store.contains_record_identity(&duplicate_manifest));

        let mut duplicate_ticket = sample_record(2, 2, 1);
        duplicate_ticket.manifest_hash = ManifestDigest::new([0xC3; 32]);
        duplicate_ticket.storage_ticket = record_b.storage_ticket;
        assert!(store.contains_record_identity(&duplicate_ticket));

        let bundle = store.bundle_at(1).expect("committed bundle retained");
        assert_eq!(bundle.commitments.as_slice(), &[record_a, record_b]);
    }

    #[test]
    fn insert_bundle_filters_stale_duplicates_from_indexes_but_preserves_bundle() {
        let mut store = DaCommitmentStore::default();
        let first = sample_record(1, 1, 1);
        let mut stale_duplicate = first.clone();
        stale_duplicate.manifest_hash = ManifestDigest::new([0x55; 32]);
        stale_duplicate.storage_ticket = StorageTicketId::new([0x66; 32]);
        let later = sample_record(2, 1, 0);

        store.insert_bundle(7, DaCommitmentBundle::new(vec![first.clone()]));
        store.insert_bundle(
            8,
            DaCommitmentBundle::new(vec![stale_duplicate.clone(), later.clone()]),
        );

        assert!(
            store
                .get_by_manifest(&stale_duplicate.manifest_hash)
                .is_none(),
            "stale duplicate manifest must not become queryable"
        );
        let fetched = store
            .get_by_lane_epoch_sequence(2, 1, 0)
            .expect("later record indexed");
        assert_eq!(fetched.location.block_height, 8);
        assert_eq!(fetched.location.index_in_bundle, 1);

        let bundle = store.bundle_at(8).expect("committed block bundle retained");
        assert_eq!(
            bundle.commitments.as_slice(),
            &[stale_duplicate, later.clone()]
        );
        assert_eq!(
            bundle.commitments[usize::try_from(fetched.location.index_in_bundle).unwrap()],
            fetched.commitment
        );
    }

    #[test]
    fn insert_bundle_filters_identity_collisions_from_indexes_but_preserves_bundle() {
        let mut store = DaCommitmentStore::default();
        let first = sample_record(1, 1, 1);
        let mut duplicate_manifest = sample_record(2, 1, 0);
        duplicate_manifest.manifest_hash = first.manifest_hash;
        let mut duplicate_ticket = sample_record(3, 1, 0);
        duplicate_ticket.storage_ticket = first.storage_ticket;
        let later = sample_record(4, 1, 0);

        store.insert_bundle(7, DaCommitmentBundle::new(vec![first.clone()]));
        store.insert_bundle(
            8,
            DaCommitmentBundle::new(vec![
                duplicate_manifest.clone(),
                duplicate_ticket.clone(),
                later.clone(),
            ]),
        );

        assert!(
            store.get_by_lane_epoch_sequence(2, 1, 0).is_none(),
            "duplicate-manifest record must not become queryable by lane"
        );
        assert!(
            store.get_by_lane_epoch_sequence(3, 1, 0).is_none(),
            "duplicate-ticket record must not become queryable by lane"
        );
        let fetched = store
            .get_by_lane_epoch_sequence(4, 1, 0)
            .expect("later record indexed");
        assert_eq!(fetched.location.block_height, 8);
        assert_eq!(fetched.location.index_in_bundle, 2);

        let bundle = store.bundle_at(8).expect("committed block bundle retained");
        assert_eq!(
            bundle.commitments.as_slice(),
            &[duplicate_manifest, duplicate_ticket, later.clone()]
        );
        assert_eq!(
            bundle.commitments[usize::try_from(fetched.location.index_in_bundle).unwrap()],
            fetched.commitment
        );
    }

    #[test]
    fn insert_bundle_preserves_all_duplicate_bundle() {
        let mut store = DaCommitmentStore::default();
        let first = sample_record(1, 1, 1);
        let mut duplicate_key = first.clone();
        duplicate_key.manifest_hash = ManifestDigest::new([0x77; 32]);
        duplicate_key.storage_ticket = StorageTicketId::new([0x78; 32]);

        store.insert_bundle(7, DaCommitmentBundle::new(vec![first.clone()]));
        store.insert_bundle(8, DaCommitmentBundle::new(vec![duplicate_key.clone()]));

        assert!(
            store
                .get_by_manifest(&duplicate_key.manifest_hash)
                .is_none(),
            "duplicate-key record must not become queryable by manifest"
        );
        let bundle = store
            .bundle_at(8)
            .expect("duplicate-only committed block bundle retained");
        assert_eq!(bundle.commitments.as_slice(), &[duplicate_key]);
    }

    #[test]
    fn duplicate_manifest_is_rejected_from_indexes() {
        let mut store = DaCommitmentStore::default();
        let first = sample_record(1, 1, 1);
        let mut duplicate_manifest = sample_record(2, 2, 0);
        duplicate_manifest.manifest_hash = first.manifest_hash;

        assert!(store.insert(
            &first,
            DaCommitmentLocation {
                block_height: 1,
                index_in_bundle: 0,
            }
        ));
        assert!(!store.insert(
            &duplicate_manifest,
            DaCommitmentLocation {
                block_height: 2,
                index_in_bundle: 0,
            }
        ));

        let manifest_lookup = store
            .get_by_manifest(&first.manifest_hash)
            .expect("manifest lookup retained");
        assert_eq!(manifest_lookup.commitment, first);
        assert!(
            store.get_by_lane_epoch_sequence(2, 2, 0).is_none(),
            "duplicate-manifest record must not be indexed by lane"
        );
    }

    #[test]
    fn duplicate_ticket_is_rejected_from_indexes() {
        let mut store = DaCommitmentStore::default();
        let first = sample_record(1, 1, 1);
        let mut duplicate_ticket = sample_record(2, 2, 0);
        duplicate_ticket.storage_ticket = first.storage_ticket;

        assert!(store.insert(
            &first,
            DaCommitmentLocation {
                block_height: 1,
                index_in_bundle: 0,
            }
        ));
        assert!(!store.insert(
            &duplicate_ticket,
            DaCommitmentLocation {
                block_height: 2,
                index_in_bundle: 0,
            }
        ));

        let ticket_lookup = store
            .get_by_storage_ticket(&first.storage_ticket)
            .expect("ticket lookup retained");
        assert_eq!(ticket_lookup.commitment, first);
        assert!(
            store.get_by_lane_epoch_sequence(2, 2, 0).is_none(),
            "duplicate-ticket record must not be indexed by lane"
        );
    }

    #[test]
    fn contains_record_identity_detects_key_manifest_and_ticket_collisions() {
        let mut store = DaCommitmentStore::default();
        let first = sample_record(1, 1, 1);
        assert!(store.insert(
            &first,
            DaCommitmentLocation {
                block_height: 1,
                index_in_bundle: 0,
            }
        ));

        let mut duplicate_key = first.clone();
        duplicate_key.manifest_hash = ManifestDigest::new([0x90; 32]);
        duplicate_key.storage_ticket = StorageTicketId::new([0x91; 32]);
        assert!(store.contains_record_identity(&duplicate_key));

        let mut duplicate_manifest = sample_record(2, 2, 0);
        duplicate_manifest.manifest_hash = first.manifest_hash;
        duplicate_manifest.storage_ticket = StorageTicketId::new([0x92; 32]);
        assert!(store.contains_record_identity(&duplicate_manifest));

        let mut duplicate_ticket = sample_record(3, 3, 0);
        duplicate_ticket.manifest_hash = ManifestDigest::new([0x93; 32]);
        duplicate_ticket.storage_ticket = first.storage_ticket;
        assert!(store.contains_record_identity(&duplicate_ticket));

        let mut fresh = sample_record(4, 4, 0);
        fresh.manifest_hash = ManifestDigest::new([0x94; 32]);
        fresh.storage_ticket = StorageTicketId::new([0x95; 32]);
        assert!(!store.contains_record_identity(&fresh));
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
