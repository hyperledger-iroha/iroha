//! In-memory DA pin intent index mirrored from the on-chain registry.
//!
//! Pin intents are persisted into WSV during block application. This cache
//! provides fast lookups for Torii/query paths and can be rebuilt from the
//! block log or WSV indexes after restart.

use std::collections::{BTreeMap, BTreeSet};

use iroha_data_model::{
    da::{
        commitment::DaCommitmentLocation,
        pin_intent::{DaPinIntent, DaPinIntentWithLocation},
        types::StorageTicketId,
    },
    sorafs::pin_registry::ManifestDigest,
};

/// Simple index over DA pin intents with stable block locations.
#[derive(Debug, Default)]
pub struct DaPinStore {
    by_ticket: BTreeMap<StorageTicketId, DaPinIntentWithLocation>,
    by_alias: BTreeMap<String, StorageTicketId>,
    by_manifest: BTreeMap<ManifestDigest, DaPinIntentWithLocation>,
    by_lane_epoch: BTreeMap<(u32, u64, u64), DaPinIntentWithLocation>,
    by_location: BTreeMap<(u64, u32), DaPinIntentWithLocation>,
    seen_keys: BTreeSet<(u32, u64, u64)>,
}

impl DaPinStore {
    /// Build a store from an existing set of intents, preserving deterministic order.
    #[must_use]
    pub fn from_intents(intents: &[DaPinIntentWithLocation]) -> Self {
        let mut store = Self::default();
        for intent in intents.iter().cloned() {
            store.insert_with_location(intent);
        }
        store
    }

    /// Insert a pin intent with its originating block location, updating alias mapping if present.
    ///
    /// Returns `true` when the intent was newly inserted and `false` when it was
    /// dropped due to a duplicate `(lane, epoch, sequence)` key.
    pub fn insert(&mut self, intent: DaPinIntent, location: DaCommitmentLocation) -> bool {
        self.insert_with_location(DaPinIntentWithLocation { intent, location })
    }

    /// Insert a pin intent that already carries its location metadata.
    ///
    /// Returns `true` when the intent was newly inserted.
    pub fn insert_with_location(&mut self, entry: DaPinIntentWithLocation) -> bool {
        let DaPinIntentWithLocation { intent, location } = entry;
        let key = (intent.lane_id.as_u32(), intent.epoch, intent.sequence);
        if !self.seen_keys.insert(key) {
            return false;
        }

        let stored = DaPinIntentWithLocation {
            intent: intent.clone(),
            location,
        };

        if let Some(alias) = intent.alias.clone() {
            self.by_alias.insert(alias, intent.storage_ticket);
        }
        self.by_ticket.insert(intent.storage_ticket, stored.clone());
        self.by_manifest
            .insert(intent.manifest_hash, stored.clone());
        self.by_lane_epoch.insert(
            (intent.lane_id.as_u32(), intent.epoch, intent.sequence),
            stored.clone(),
        );
        self.by_location
            .insert((location.block_height, location.index_in_bundle), stored);

        true
    }

    /// Lookup by storage ticket.
    #[must_use]
    pub fn get_by_ticket(&self, ticket: &StorageTicketId) -> Option<&DaPinIntentWithLocation> {
        self.by_ticket.get(ticket)
    }

    /// Lookup by alias.
    #[must_use]
    pub fn get_by_alias(
        &self,
        alias: &str,
    ) -> Option<(&StorageTicketId, &DaPinIntentWithLocation)> {
        self.by_alias
            .get(alias)
            .and_then(|ticket| self.by_ticket.get_key_value(ticket))
    }

    /// Lookup by manifest hash.
    #[must_use]
    pub fn get_by_manifest(&self, digest: &ManifestDigest) -> Option<&DaPinIntentWithLocation> {
        self.by_manifest.get(digest)
    }

    /// Lookup by `(lane_id, epoch, sequence)`.
    #[must_use]
    pub fn get_by_lane_epoch_sequence(
        &self,
        lane_id: u32,
        epoch: u64,
        sequence: u64,
    ) -> Option<&DaPinIntentWithLocation> {
        self.by_lane_epoch.get(&(lane_id, epoch, sequence))
    }

    /// Return all intents ordered by `(block_height, index_in_bundle)`.
    pub fn all_sorted(&self) -> impl Iterator<Item = &DaPinIntentWithLocation> {
        self.by_location.values()
    }

    /// Drop pin intents belonging to retired lanes.
    pub fn prune_lanes(&mut self, retired: &BTreeSet<iroha_data_model::nexus::LaneId>) {
        if retired.is_empty() {
            return;
        }
        let mut rebuilt = DaPinStore::default();
        for entry in self.by_location.values() {
            if retired.contains(&entry.intent.lane_id) {
                continue;
            }
            rebuilt.insert_with_location(entry.clone());
        }
        *self = rebuilt;
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryFrom;

    use iroha_data_model::{nexus::LaneId, sorafs::pin_registry::ManifestDigest};

    use super::*;

    fn sample_intent(lane: u32, seq: u64, alias: Option<&str>) -> DaPinIntent {
        let lane_byte = u8::try_from(lane).expect("lane id fits in byte for test intent");
        let seq_byte = u8::try_from(seq).expect("sequence fits in byte for test intent");
        DaPinIntent {
            lane_id: LaneId::new(lane),
            epoch: 1,
            sequence: seq,
            storage_ticket: StorageTicketId::new([lane_byte; 32]),
            manifest_hash: ManifestDigest::new([seq_byte; 32]),
            alias: alias.map(ToOwned::to_owned),
            owner: None,
        }
    }

    fn located(intent: DaPinIntent, height: u64, idx: u32) -> DaPinIntentWithLocation {
        DaPinIntentWithLocation {
            intent,
            location: DaCommitmentLocation {
                block_height: height,
                index_in_bundle: idx,
            },
        }
    }

    #[test]
    fn inserts_and_overwrites_alias() {
        let mut store = DaPinStore::default();
        let a = located(sample_intent(1, 1, Some("alias-a")), 5, 0);
        let b = located(sample_intent(2, 2, Some("alias-b")), 5, 1);
        assert!(store.insert_with_location(a.clone()));
        assert!(store.insert_with_location(b.clone()));

        assert_eq!(store.all_sorted().count(), 2);
        assert_eq!(
            store
                .get_by_alias("alias-a")
                .map(|(ticket, record)| (ticket, &record.location)),
            Some((&a.intent.storage_ticket, &a.location))
        );
        assert_eq!(
            store
                .get_by_alias("alias-b")
                .map(|(ticket, record)| (ticket, &record.location)),
            Some((&b.intent.storage_ticket, &b.location))
        );
    }

    #[test]
    fn builds_from_intents() {
        let intents = vec![
            located(sample_intent(1, 1, Some("alias-a")), 1, 0),
            located(sample_intent(2, 0, None), 2, 3),
        ];
        let store = DaPinStore::from_intents(&intents);
        assert_eq!(store.all_sorted().count(), 2);
        assert!(
            store
                .get_by_ticket(&intents[1].intent.storage_ticket)
                .is_some()
        );
        let ordered: Vec<_> = store
            .all_sorted()
            .map(|rec| (rec.location.block_height, rec.location.index_in_bundle))
            .collect();
        assert_eq!(ordered, vec![(1, 0), (2, 3)]);
    }

    #[test]
    fn prunes_retired_lanes() {
        let mut store = DaPinStore::default();
        let lane0 = located(sample_intent(0, 1, Some("keep")), 1, 0);
        let lane1 = located(sample_intent(1, 2, Some("drop-me")), 1, 1);
        assert!(store.insert_with_location(lane0.clone()));
        assert!(store.insert_with_location(lane1.clone()));

        let retired = BTreeSet::from([LaneId::new(1)]);
        store.prune_lanes(&retired);

        assert!(store.get_by_alias("keep").is_some(), "lane 0 alias remains");
        assert!(
            store.get_by_alias("drop-me").is_none(),
            "retired lane alias removed"
        );
        let ordered: Vec<_> = store
            .all_sorted()
            .map(|rec| rec.intent.lane_id.as_u32())
            .collect();
        assert_eq!(ordered, vec![0]);
    }

    #[test]
    fn ignores_duplicates_by_key() {
        let mut store = DaPinStore::default();
        let first = located(sample_intent(3, 4, Some("alias-dup")), 9, 0);
        let mut second = first.clone();
        second.intent.manifest_hash = ManifestDigest::new([0xAA; 32]);
        second.intent.storage_ticket = StorageTicketId::new([0xBB; 32]);
        second.location.index_in_bundle = 1;

        assert!(store.insert_with_location(first.clone()));
        assert!(!store.insert_with_location(second.clone()));
        let stored = store
            .get_by_ticket(&first.intent.storage_ticket)
            .expect("stored intent present");
        assert_eq!(stored.intent.manifest_hash, first.intent.manifest_hash);
        assert_eq!(
            stored.location.index_in_bundle,
            first.location.index_in_bundle
        );
        assert!(
            store.get_by_ticket(&second.intent.storage_ticket).is_none(),
            "conflicting ticket should be ignored"
        );
    }
}
