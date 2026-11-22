//! Root-addressed IMT witnesses for transfers. A staged map descriptor has no monetary
//! authority. Only a root in the proved wallet state can select its contents.
use super::custody_archive::{Checkpoint, Objects, storage_error};
use super::*;
use crate::kagemusha_wallet_advance_v1::{
    KagemushaWalletSlotIdV1 as Slot, kagemusha_wallet_archive_object_digest_v1 as digest,
};
use crate::kagemusha_wallet_state_v1::{
    self as state, IndexRoot, ObjectWorkspace, PersistentMapV1,
};
use iroha_data_model::kagemusha::{
    KagemushaWalletIndexedInsertV1, KagemushaWalletIndexedLeafV1, KagemushaWalletIndexedOpeningV1,
    KagemushaWalletIndexedRemoveV1, kagemusha_wallet_indexed_verify_non_membership_v1 as absent,
};
use native::{SpartanWalletMap as Role, SpartanWalletMaps};

const DESCRIPTOR_LIMIT: usize = 1024;
/// Wallet wire v1 removes keys only from the pending-outgoing map (ArchiveSent).
const REMOVAL_ROLE: &str = "wallet transfer map removal role";
/// The selected root's authenticated low leaf proves the key absent: never sent from this
/// lineage, or already removed at this root. Not custody loss.
const REMOVAL_ABSENT: &str = "wallet pending-outgoing entry absent at the selected root";

struct Maps<'a, W> {
    store: &'a mut W,
    root: IndexRoot,
    failure: Option<state::Error>,
}

fn key(role: Role, root: [u8; 32]) -> [u8; 32] {
    let tag = match role {
        Role::Pending => 1,
        Role::Fee => 2,
        Role::Consumed => 3,
        Role::Credit => 4,
        Role::Recovery => 5,
    };
    digest(
        &[
            b"future-transfer-map-descriptor-v1".as_slice(),
            &[tag],
            &root,
        ]
        .concat(),
    )
}

impl<W: ObjectWorkspace> Maps<'_, W> {
    fn descriptor(&mut self, role: Role, root: [u8; 32]) -> Result<PersistentMapV1, state::Error> {
        let empty = PersistentMapV1::default();
        if empty.root() == root {
            return Ok(empty);
        }
        let address = self
            .root
            .get(self.store, &key(role, root))?
            .ok_or(state::Error::WitnessLost("future transfer map descriptor"))?;
        let address: [u8; 32] = address
            .try_into()
            .map_err(|_| state::Error::WitnessLost("future transfer map descriptor address"))?;
        let encoded = self.store.read_staged_object(&address, DESCRIPTOR_LIMIT)?;
        let map: PersistentMapV1 = norito::core::classify_decode_attempt(|| {
            norito::decode_canonical_with_limits(
                &encoded,
                norito::canonical_decode_limits(encoded.len()),
            )
        })
        .map_err(|e| {
            state::decode_attempt_error(
                e,
                state::Error::WitnessLost("future transfer map descriptor"),
            )
        })?;
        map.validate()?;
        if map.root() != root {
            return Err(state::Error::WitnessLost("future transfer map root"));
        }
        Ok(map)
    }

    fn retain(&mut self, role: Role, map: &PersistentMapV1) -> Result<(), state::Error> {
        let bytes = norito::encode_canonical(map)
            .map_err(|_| state::Error::Invalid("future transfer descriptor encoding"))?;
        let address = self.store.stage_object(&bytes, DESCRIPTOR_LIMIT)?;
        let key = key(role, map.root());
        if let Some(existing) = self.root.get(self.store, &key)? {
            if existing != address {
                return Err(state::Error::WitnessLost(
                    "future transfer descriptor conflict",
                ));
            }
        } else {
            self.root = self.root.set(self.store, key, &address)?;
        }
        Ok(())
    }

    /// One authenticated opening of `credit` under exactly `root`: `Some` member, `None` an
    /// authenticated absence. The descriptor's openings are checked against `root` first.
    fn open(
        &mut self,
        map: &PersistentMapV1,
        root: [u8; 32],
        credit: &[u8; 32],
    ) -> Result<Option<KagemushaWalletIndexedLeafV1>, state::Error> {
        let (leaf, opening) = map.member_or_low(self.store, credit)?;
        if leaf.key == *credit {
            return Ok(Some(leaf));
        }
        absent(&root, credit, &leaf, &opening)
            .map_err(|_| state::Error::WitnessLost("future transfer map absence"))?;
        Ok(None)
    }

    /// Removal from the descriptor retained for exactly `root`. The stock persistent map
    /// relinks the authenticated predecessor and clears the leaf in a copy; the witness is
    /// verified again from `root`. The source descriptor and every older root are unchanged.
    fn stage_removal(
        &mut self,
        role: Role,
        root: [u8; 32],
        credit: &[u8; 32],
    ) -> Result<Option<KagemushaWalletIndexedRemoveV1>, state::Error> {
        let source = self.descriptor(role, root)?;
        let Some(leaf) = self.open(&source, root, credit)? else {
            return Ok(None);
        };
        let mut next = source.clone();
        let removed = next.remove(self.store, credit)?;
        let after = removed
            .verify(&root, credit)
            .map_err(|_| state::Error::WitnessLost("future transfer map removal"))?;
        if after != next.root() || after == root || removed.leaf != leaf {
            return Err(state::Error::WitnessLost("future transfer map removal"));
        }
        self.retain_removed(role, &next, credit)?;
        Ok(Some(removed))
    }

    /// Retain a removal result under its root. A removal may return to a root this index
    /// already names (the map before the matching insertion). A root commits to every slot,
    /// so the descriptor already retained there opens identically: it stays selected, after
    /// an authenticated absence of the removed key, and no index entry is ever replaced.
    /// The empty root is always answered by the default descriptor and is not retained.
    fn retain_removed(
        &mut self,
        role: Role,
        map: &PersistentMapV1,
        credit: &[u8; 32],
    ) -> Result<(), state::Error> {
        let root = map.root();
        if root == PersistentMapV1::default().root() {
            return Ok(());
        }
        let index = key(role, root);
        match self.root.get(self.store, &index)? {
            None => {
                let bytes = norito::encode_canonical(map)
                    .map_err(|_| state::Error::Invalid("future transfer descriptor encoding"))?;
                let address = self.store.stage_object(&bytes, DESCRIPTOR_LIMIT)?;
                self.root = self.root.set(self.store, index, &address)?;
            }
            Some(_) => {
                let held = self.descriptor(role, root)?;
                if self.open(&held, root, credit)?.is_some() {
                    return Err(state::Error::WitnessLost(
                        "future transfer descriptor conflict",
                    ));
                }
            }
        }
        Ok(())
    }

    fn map_result<T>(&mut self, result: Result<T, state::Error>) -> native::Result<T> {
        result.map_err(|err| {
            self.failure = Some(err);
            native::Error::Unavailable("protected transfer map")
        })
    }
}
impl<W: ObjectWorkspace> SpartanWalletMaps for Maps<'_, W> {
    fn member_or_low(
        &mut self,
        role: Role,
        root: [u8; 32],
        key: [u8; 32],
    ) -> native::Result<(
        KagemushaWalletIndexedLeafV1,
        KagemushaWalletIndexedOpeningV1,
    )> {
        let result = self
            .descriptor(role, root)
            .and_then(|map| map.member_or_low(self.store, &key));
        self.map_result(result)
    }
    fn insert(
        &mut self,
        role: Role,
        root: [u8; 32],
        key: [u8; 32],
        value: [u8; 32],
    ) -> native::Result<KagemushaWalletIndexedInsertV1> {
        let result = self.descriptor(role, root).and_then(|mut map| {
            let inserted = map.insert(self.store, key, value)?;
            self.retain(role, &map)?;
            Ok(inserted)
        });
        self.map_result(result)
    }
    /// ArchiveSent: stage the removal of one present pending-outgoing key from the
    /// descriptor retained for `root`, as an insertion stages its successor. The successor
    /// root gains no authority until a released successor selects it. An authenticated
    /// absence and another map are typed refusals; lost or inconsistent retained bytes are
    /// custody failures that stop the whole draft.
    fn remove(
        &mut self,
        role: Role,
        root: [u8; 32],
        key: [u8; 32],
    ) -> native::Result<KagemushaWalletIndexedRemoveV1> {
        if role != Role::Pending {
            return Err(native::Error::AuthorityMismatch(REMOVAL_ROLE));
        }
        let result = self.stage_removal(role, root, &key);
        self.map_result(result)?
            .ok_or(native::Error::AuthorityMismatch(REMOVAL_ABSENT))
    }
}

/// The exclusive provider's transfer-map staging, shared by every retail operation. The
/// derivation runs in an unpublished draft over the descriptors selected by `checkpoint`;
/// a refusal publishes nothing. Only when the root-addressed descriptor index changed are
/// the immutable objects staged in one archive call and published with the checkpoint
/// (compare-and-publish against `expected`). Neither path selects a head or rewrites a
/// retained original, and an exact retry re-derives the same content-addressed objects.
fn stage_transfer_maps<F, P, C, R, T>(
    provider: &mut KagemushaWalletProviderV1<F, P, C, R>,
    slot: &Slot,
    expected: [u8; 32],
    mut checkpoint: Checkpoint,
    build: impl FnOnce(&mut dyn SpartanWalletMaps) -> native::Result<T>,
) -> Result<T, ProviderError>
where
    F: KagemushaWalletFsV1,
    P: KagemushaWalletPlatformV1,
    C: KagemushaWalletAdvanceCapsuleV1,
    R: KagemushaWalletCompletionFrameV1,
{
    let ((built, next), objects) = provider.with_archive(slot, |archive| {
        state::with_transfer_draft(&mut Objects(archive), |store| {
            let mut maps = Maps {
                store,
                root: IndexRoot(checkpoint.transfer_descriptors),
                failure: None,
            };
            let built = build(&mut maps);
            if let Some(err) = maps.failure {
                return Err(err);
            }
            Ok((
                built.map_err(|e| state::Error::Provider(provider_error(e)))?,
                maps.root.0,
            ))
        })
        .map_err(storage_error)
    })?;
    if next != checkpoint.transfer_descriptors {
        let payload = objects.iter().map(|(_, bytes)| bytes.len() as u64).sum();
        let required = state::transfer_storage_required(payload, objects.len() as u64);
        if provider.runtime_resources()?.storage_bytes < required {
            return Err(ProviderError::NoSpace);
        }
        checkpoint.transfer_descriptors = next;
        provider.open_archive_call(slot);
        let published = (|| {
            for (key, bytes) in objects {
                provider.stage_archive_record(slot, &key, &bytes, None)?;
            }
            let bytes =
                norito::encode_canonical(&checkpoint).map_err(|_| ProviderError::Invalid {
                    field: "future custody checkpoint encoding",
                })?;
            provider.publish_archive_checkpoint(slot, expected, &bytes)?;
            Ok::<_, ProviderError>(())
        })();
        let closed = provider.close_archive_call(slot, published.is_ok());
        published?;
        closed?;
    }
    Ok(built)
}

impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1 + 'static, S> FutureRuntime<F, P, S> {
    pub(super) fn with_transfer_maps<T>(
        &mut self,
        slot: &Slot,
        build: impl FnOnce(
            &native::NativeEngine,
            &AuthenticatedParameters,
            &mut dyn SpartanWalletMaps,
        ) -> native::Result<T>,
    ) -> Result<T, ProviderError> {
        let wallet = self.authorized_wallet_id(slot)?;
        let (expected, checkpoint) = self.custody_checkpoint(slot, wallet)?;
        let engine = &self.engine;
        let parameters: &AuthenticatedParameters = &self.parameters;
        stage_transfer_maps(&mut self.provider, slot, expected, checkpoint, |maps| {
            build(engine, parameters, maps)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kagemusha_wallet_advance_v1::{
        KagemushaWalletAnchorPolicyV1, KagemushaWalletStdFsV1,
        test_support::{FakePlatformV1, bootstrapped_provider},
    };
    use crate::kagemusha_wallet_archive_bytes_v1::PrivateArchiveBytes;
    use iroha_data_model::kagemusha::KagemushaWalletIndexedTreeV1;
    use state::ObjectStore;
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::{Path, PathBuf};
    /// Content-addressed objects, and every key a reader asked for.
    #[derive(Default)]
    struct Memory(BTreeMap<[u8; 32], Vec<u8>>, BTreeSet<[u8; 32]>);
    impl ObjectStore for Memory {
        fn read_object(&mut self, key: &[u8; 32], limit: usize) -> Result<Vec<u8>, state::Error> {
            self.1.insert(*key);
            let value = self
                .0
                .get(key)
                .ok_or(state::Error::WitnessLost("map object"))?;
            if value.len() > limit || digest(value) != *key {
                return Err(state::Error::WitnessLost("map object content"));
            }
            Ok(value.clone())
        }
        fn write_object(&mut self, value: &[u8], limit: usize) -> Result<[u8; 32], state::Error> {
            assert!(value.len() <= limit);
            let address = digest(value);
            self.0.insert(address, value.to_vec());
            Ok(address)
        }
    }
    fn word(n: u8) -> [u8; 32] {
        let mut w = [0; 32];
        w[0] = n;
        w
    }
    #[test]
    fn unpublished_map_drafts_leave_no_writes_and_distinct_roots_remain_openable_after_reopen() {
        let mut store = Memory::default();
        let empty = PersistentMapV1::default().root();
        let failed = state::with_transfer_draft(&mut store, |draft| {
            let mut maps = Maps {
                store: draft,
                root: IndexRoot::default(),
                failure: None,
            };
            maps.insert(Role::Credit, empty, word(4), word(12)).unwrap();
            Err::<(), _>(state::Error::Invalid("later proof validation refused"))
        });
        assert!(failed.is_err() && store.0.is_empty());
        let ((first, second, index), objects) = state::with_transfer_draft(&mut store, |draft| {
            let mut maps = Maps {
                store: draft,
                root: IndexRoot::default(),
                failure: None,
            };
            let one = maps
                .insert(Role::Credit, empty, word(4), word(12))
                .unwrap()
                .verify(&empty, &word(4), &word(12))
                .unwrap();
            let two = maps
                .insert(Role::Credit, one, word(7), word(21))
                .unwrap()
                .verify(&one, &word(7), &word(21))
                .unwrap();
            Ok((one, two, maps.root))
        })
        .unwrap();
        assert!(store.0.is_empty());
        for (key, bytes) in objects {
            assert_eq!(store.write_object(&bytes, bytes.len()).unwrap(), key);
        }
        let mut reopened = Maps {
            store: &mut store,
            root: index,
            failure: None,
        };
        assert_eq!(
            reopened
                .member_or_low(Role::Credit, first, word(4))
                .unwrap()
                .0
                .value,
            word(12)
        );
        assert_ne!(
            reopened
                .member_or_low(Role::Credit, first, word(7))
                .unwrap()
                .0
                .key,
            word(7)
        );
        assert_eq!(
            reopened
                .member_or_low(Role::Credit, second, word(7))
                .unwrap()
                .0
                .value,
            word(21)
        );
        assert!(
            reopened
                .member_or_low(Role::Consumed, second, word(7))
                .is_err()
        );
        let address: [u8; 32] = index
            .get(reopened.store, &key(Role::Credit, first))
            .unwrap()
            .unwrap()
            .try_into()
            .unwrap();
        reopened.store.0.remove(&address);
        assert!(
            reopened
                .member_or_low(Role::Credit, first, word(4))
                .is_err()
        );
    }


    /// Three pending-outgoing descriptors, each inserted from the previous root as Send does.
    const HISTORY: [(u8, u8); 3] = [(4, 40), (9, 90), (27, 70)];

    fn materialize(store: &mut Memory, objects: Vec<([u8; 32], PrivateArchiveBytes)>) {
        for (key, bytes) in objects {
            assert_eq!(store.write_object(&bytes, bytes.len()).unwrap(), key);
        }
    }

    /// Send-shaped history staged through the adapter, then materialized.
    fn pending_history(store: &mut Memory, entries: &[(u8, u8)]) -> (Vec<[u8; 32]>, IndexRoot) {
        let empty = PersistentMapV1::default().root();
        let ((roots, index), objects) = state::with_transfer_draft(&mut *store, |draft| {
            let mut maps = Maps {
                store: draft,
                root: IndexRoot::default(),
                failure: None,
            };
            let mut roots = vec![empty];
            for &(k, v) in entries {
                let root = *roots.last().unwrap();
                let plan = maps.insert(Role::Pending, root, word(k), word(v)).unwrap();
                roots.push(plan.verify(&root, &word(k), &word(v)).unwrap());
            }
            Ok((roots, maps.root))
        })
        .unwrap();
        materialize(store, objects);
        (roots, index)
    }

    fn stock(entries: &[(u8, u8)]) -> KagemushaWalletIndexedTreeV1 {
        let mut tree = KagemushaWalletIndexedTreeV1::new();
        for &(k, v) in entries {
            tree.insert(word(k), word(v)).unwrap();
        }
        tree
    }

    /// One removal in its own draft. The durable store must be unchanged by any outcome.
    fn attempt(
        store: &mut Memory,
        root: IndexRoot,
        role: Role,
        at: [u8; 32],
        credit: [u8; 32],
    ) -> (
        native::Result<KagemushaWalletIndexedRemoveV1>,
        Option<state::Error>,
        bool,
    ) {
        let before = store.0.clone();
        let ((result, failure, changed), _) = state::with_transfer_draft(&mut *store, |draft| {
            let mut maps = Maps {
                store: draft,
                root,
                failure: None,
            };
            let result = maps.remove(role, at, credit);
            Ok((result, maps.failure.take(), maps.root.0 != root.0))
        })
        .unwrap();
        assert_eq!(store.0, before, "a draft removal writes nothing");
        (result, failure, changed)
    }

    #[test]
    fn pending_removal_stages_the_stock_witness_without_touching_selected_roots_or_bytes() {
        let mut store = Memory::default();
        let (roots, index) = pending_history(&mut store, &HISTORY);
        let full = roots[3];
        let source = store.0.clone();
        // A later refusal in the same derivation publishes nothing.
        let refused = state::with_transfer_draft(&mut store, |draft| {
            let mut maps = Maps {
                store: draft,
                root: index,
                failure: None,
            };
            maps.remove(Role::Pending, full, word(9)).unwrap();
            Err::<(), _>(state::Error::Invalid("later witness refusal"))
        });
        assert!(refused.is_err());
        assert_eq!(store.0, source);
        let stage = |store: &mut Memory| {
            state::with_transfer_draft(&mut *store, |draft| {
                let mut maps = Maps {
                    store: draft,
                    root: index,
                    failure: None,
                };
                // ArchiveSent asks for the committed and the lineage-adjusted root; here equal.
                let core = maps.remove(Role::Pending, full, word(9)).unwrap();
                let adjusted = maps.remove(Role::Pending, full, word(9)).unwrap();
                assert_eq!(core, adjusted);
                let (held, _) = maps.member_or_low(Role::Pending, full, word(9)).unwrap();
                assert_eq!(held.value, word(90), "the selected source root is unchanged");
                assert!(maps.failure.is_none());
                Ok((core, maps.root))
            })
            .unwrap()
        };
        let ((removed, successor), objects) = stage(&mut store);
        assert_eq!(store.0, source, "an unpublished removal writes nothing");
        assert_ne!(successor.0, index.0, "the successor descriptor is retained");
        let after = removed.verify(&full, &word(9)).unwrap();
        assert_eq!(
            removed.leaf,
            KagemushaWalletIndexedLeafV1 {
                key: word(9),
                value: word(90),
                next_key: word(27),
            },
            "the exact stored descriptor"
        );
        assert_eq!(removed.predecessor.key, word(4));
        let mut tree = stock(&HISTORY);
        assert_eq!(tree.root(), full);
        assert_eq!(tree.remove(&word(9)).unwrap(), removed, "stock indexed tree agrees");
        assert_eq!(tree.root(), after);
        // A retry after a lost draft derives the identical witness, index and objects.
        let ((retried, retried_index), again) = stage(&mut store);
        assert_eq!(retried, removed);
        assert_eq!(retried_index.0, successor.0);
        let names = |objects: &[([u8; 32], PrivateArchiveBytes)]| {
            objects.iter().map(|(key, _)| *key).collect::<Vec<_>>()
        };
        assert_eq!(names(&again[..]), names(&objects[..]));
        materialize(&mut store, objects);
        for (key, bytes) in &source {
            assert_eq!(store.0.get(key), Some(bytes), "retained bytes are never rewritten");
        }
        // Reopen the published index.
        let mut reopened = Maps {
            store: &mut store,
            root: successor,
            failure: None,
        };
        let (low, path) = reopened.member_or_low(Role::Pending, after, word(9)).unwrap();
        assert_eq!((low.key, low.next_key), (word(4), word(27)));
        absent(&after, &word(9), &low, &path).unwrap();
        assert_eq!(
            reopened
                .member_or_low(Role::Pending, full, word(9))
                .unwrap()
                .0
                .value,
            word(90),
            "the older root stays openable"
        );
        assert!(
            matches!(
                reopened.remove(Role::Pending, after, word(9)),
                Err(native::Error::AuthorityMismatch(m)) if m == REMOVAL_ABSENT
            ),
            "an already removed entry is an authenticated absence"
        );
        assert!(reopened.failure.is_none());
        let next = reopened.remove(Role::Pending, after, word(27)).unwrap();
        assert_eq!(next, tree.remove(&word(27)).unwrap());
    }

    #[test]
    fn pending_removal_refusals_are_typed_and_lost_bytes_are_never_absence() {
        let mut store = Memory::default();
        let (roots, index) = pending_history(&mut store, &HISTORY);
        let full = roots[3];
        let (result, failure, changed) = attempt(&mut store, index, Role::Credit, full, word(9));
        assert!(matches!(result, Err(native::Error::AuthorityMismatch(m)) if m == REMOVAL_ROLE));
        assert!(failure.is_none() && !changed);
        for (at, credit) in [(full, word(5)), (roots[0], word(9)), (roots[1], word(9))] {
            let (result, failure, changed) = attempt(&mut store, index, Role::Pending, at, credit);
            assert!(
                matches!(result, Err(native::Error::AuthorityMismatch(m)) if m == REMOVAL_ABSENT),
                "{}",
                hex::encode(at)
            );
            assert!(failure.is_none() && !changed);
        }
        // A root this wallet never retained is lost custody, not absence.
        let (result, failure, _) = attempt(&mut store, index, Role::Pending, word(77), word(9));
        assert!(matches!(result, Err(native::Error::Unavailable(_))));
        assert!(matches!(
            failure,
            Some(state::Error::WitnessLost("future transfer map descriptor"))
        ));
        let (_, failure, _) = attempt(&mut store, index, Role::Pending, full, [0; 32]);
        assert!(matches!(failure, Some(state::Error::Invalid(_))));
        // An index entry naming another root's descriptor is refused before any leaf is used.
        let other: [u8; 32] = index
            .get(&mut store, &key(Role::Pending, full))
            .unwrap()
            .unwrap()
            .try_into()
            .unwrap();
        let tampered = index
            .set(&mut store, key(Role::Pending, roots[2]), &other)
            .unwrap();
        let (_, failure, _) = attempt(&mut store, tampered, Role::Pending, roots[2], word(9));
        assert!(matches!(
            failure,
            Some(state::Error::WitnessLost("future transfer map root"))
        ));
        // Deleting a retained object this removal reads is lost custody; deleting any other
        // leaves the witness unchanged. Never another witness or an absence. A spread sample
        // of both sets keeps the debug-build sweep bounded.
        let mut probe = Memory(store.0.clone(), BTreeSet::new());
        let canonical = attempt(&mut probe, index, Role::Pending, full, word(9))
            .0
            .unwrap();
        let read: Vec<_> = probe.1.iter().copied().collect();
        let unread: Vec<_> = store
            .0
            .keys()
            .filter(|object| !probe.1.contains(*object))
            .copied()
            .collect();
        assert!(!read.is_empty() && !unread.is_empty());
        let spread = |all: &[[u8; 32]], n: usize| {
            (0..n)
                .map(|i| all[i * (all.len() - 1) / (n - 1)])
                .collect::<BTreeSet<_>>()
        };
        for object in spread(&read, 10).into_iter().chain(spread(&unread, 3)) {
            let used = probe.1.contains(&object);
            let mut damaged = Memory(store.0.clone(), BTreeSet::new());
            damaged.0.remove(&object);
            match attempt(&mut damaged, index, Role::Pending, full, word(9)) {
                (Ok(witness), None, _) if !used => assert_eq!(witness, canonical),
                (Err(native::Error::Unavailable(_)), Some(state::Error::WitnessLost(_)), _)
                    if used => {}
                (result, failure, _) => panic!(
                    "deleting {} (read {used}) gave {result:?} / {failure:?}",
                    hex::encode(object)
                ),
            }
        }
        eprintln!("removal reads {} of {} retained objects", read.len(), store.0.len());
    }

    #[test]
    fn repeated_removal_requires_readable_successor_custody() {
        let mut store = Memory::default();
        let (roots, index) = pending_history(&mut store, &HISTORY);
        let source = roots[3];
        let ((plan, next_index), objects) = state::with_transfer_draft(&mut store, |draft| {
            let mut maps = Maps { store: draft, root: index, failure: None };
            let plan = maps.remove(Role::Pending, source, word(9)).unwrap();
            Ok((plan, maps.root))
        })
        .unwrap();
        materialize(&mut store, objects);
        let after = plan.verify(&source, &word(9)).unwrap();
        let address: [u8; 32] = next_index.get(&mut store, &key(Role::Pending, after))
            .unwrap().unwrap().try_into().unwrap();
        let (retried, failure, changed) =
            attempt(&mut store, next_index, Role::Pending, source, word(9));
        assert_eq!(retried.unwrap(), plan);
        assert!(failure.is_none() && !changed);

        // Keep the checkpoint and source map. Only the previously saved
        // successor descriptor disappears between attempts.
        assert!(store.0.remove(&address).is_some());
        let (retried, failure, changed) =
            attempt(&mut store, next_index, Role::Pending, source, word(9));
        assert!(matches!(retried, Err(native::Error::Unavailable(_))));
        assert!(matches!(failure, Some(state::Error::WitnessLost(_))));
        assert!(!changed);
    }

    #[test]
    fn removal_back_to_a_retained_root_keeps_its_descriptor_and_the_empty_root_is_implicit() {
        let mut store = Memory::default();
        let (roots, index) = pending_history(&mut store, &HISTORY[..2]);
        let ((removed, restored, inserted), objects) =
            state::with_transfer_draft(&mut store, |draft| {
                let mut maps = Maps {
                    store: draft,
                    root: index,
                    failure: None,
                };
                let removed = maps.remove(Role::Pending, roots[2], word(9)).unwrap();
                let restored = maps.root;
                // A later Send from the restored root opens its retained descriptor.
                let plan = maps.insert(Role::Pending, roots[1], word(27), word(70)).unwrap();
                let inserted = plan.verify(&roots[1], &word(27), &word(70)).unwrap();
                assert!(maps.failure.is_none());
                Ok((removed, restored, (inserted, maps.root)))
            })
            .unwrap();
        assert_eq!(
            removed.verify(&roots[2], &word(9)).unwrap(),
            roots[1],
            "the removal restores the earlier root"
        );
        assert_eq!(restored.0, index.0, "no index entry is replaced or added");
        assert_eq!(stock(&HISTORY[..2]).remove(&word(9)).unwrap(), removed);
        materialize(&mut store, objects);
        let mut reopened = Maps {
            store: &mut store,
            root: inserted.1,
            failure: None,
        };
        assert_eq!(
            reopened
                .member_or_low(Role::Pending, inserted.0, word(27))
                .unwrap()
                .0
                .value,
            word(70)
        );
        // Removing the only descriptor reaches the implicit empty root; nothing is retained.
        let mut single = Memory::default();
        let (roots, index) = pending_history(&mut single, &HISTORY[..1]);
        let (result, failure, changed) = attempt(&mut single, index, Role::Pending, roots[1], word(4));
        assert_eq!(result.unwrap().verify(&roots[1], &word(4)).unwrap(), roots[0]);
        assert!(failure.is_none() && !changed);
    }

    /// Exact descriptor and record bytes, skipping the lock and the NOREPLACE probe.
    fn snapshot(root: &Path) -> BTreeMap<PathBuf, [u8; 32]> {
        fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, [u8; 32]>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                let relative = path.strip_prefix(root).unwrap().to_path_buf();
                let top = relative.components().next().unwrap().as_os_str();
                if top == "lock" || top == "probe" || top == "canary" {
                    continue;
                }
                if entry.file_type().unwrap().is_dir() {
                    walk(root, &path, out);
                } else {
                    out.insert(relative, digest(&std::fs::read(&path).unwrap()));
                }
            }
        }
        let mut out = BTreeMap::new();
        walk(root, root, &mut out);
        out
    }

    /// Native custody admits only an owner-only root, whatever the umask.
    fn owner_only(path: &Path) {
        std::fs::create_dir(path).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
    }

    /// The custody checkpoint the current marker selects, as `custody_checkpoint` opens it.
    fn selected<F, P, C, R>(
        provider: &mut KagemushaWalletProviderV1<F, P, C, R>,
        slot: &Slot,
        parameters: [u8; 32],
        wallet: [u8; 32],
    ) -> ([u8; 32], Checkpoint)
    where
        F: KagemushaWalletFsV1,
        P: KagemushaWalletPlatformV1,
        C: KagemushaWalletAdvanceCapsuleV1,
        R: KagemushaWalletCompletionFrameV1,
    {
        let scheme = *provider.scheme_id();
        Checkpoint::open(
            provider.archive_checkpoint(slot).unwrap(),
            parameters,
            scheme,
            wallet,
        )
        .unwrap()
    }

    fn is_original(path: &Path) -> bool {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("w-"))
            || path
                .components()
                .any(|c| c.as_os_str() == "capsules" || c.as_os_str() == "completion")
    }

    #[test]
    fn durable_pending_removal_publishes_only_new_objects_and_survives_cold_reopen() {
        let scratch = tempfile::tempdir().unwrap();
        let root = scratch.path().join("custody");
        owner_only(&root);
        let (mut provider, platform, slot, capsule) = bootstrapped_provider(
            KagemushaWalletStdFsV1::open(&root).unwrap(),
            KagemushaWalletAnchorPolicyV1::NotRequired,
            0x51,
        );
        let wallet = capsule.body().successor_state.core.wallet_id;
        let scheme = *provider.scheme_id();
        let options = *provider.options();
        let parameters = [0x43; 32];
        let reopen = |platform: &FakePlatformV1| {
            let provider: KagemushaWalletProviderV1<KagemushaWalletStdFsV1, FakePlatformV1> =
                KagemushaWalletProviderV1::open(
                    KagemushaWalletStdFsV1::open(&root).unwrap(),
                    platform.clone(),
                    scheme,
                    options,
                )
                .unwrap();
            provider
        };
        // Send-shaped pending history through the shared retail staging and publication.
        let (expected, checkpoint) = selected(&mut provider, &slot, parameters, wallet);
        let empty = PersistentMapV1::default().root();
        let roots = stage_transfer_maps(&mut provider, &slot, expected, checkpoint, |maps| {
            let mut roots = vec![empty];
            for (k, v) in HISTORY {
                let root = *roots.last().unwrap();
                let plan = maps.insert(Role::Pending, root, word(k), word(v))?;
                roots.push(plan.verify(&root, &word(k), &word(v)).unwrap());
            }
            Ok(roots)
        })
        .unwrap();
        let full = roots[3];
        assert_ne!(selected(&mut provider, &slot, parameters, wallet).0, expected);
        drop(provider);

        // Cold reopen. Every refusal leaves every durable byte and the selection unchanged.
        let mut provider = reopen(&platform);
        let source = snapshot(&root);
        let (expected, checkpoint) = selected(&mut provider, &slot, parameters, wallet);
        let refused = stage_transfer_maps(&mut provider, &slot, expected, checkpoint, |maps| {
            maps.remove(Role::Pending, full, word(9))?;
            Err::<(), _>(native::Error::Rejected("later witness refusal".into()))
        });
        assert!(matches!(refused, Err(ProviderError::Invalid { .. })));
        let (expected, checkpoint) = selected(&mut provider, &slot, parameters, wallet);
        let missing = stage_transfer_maps(&mut provider, &slot, expected, checkpoint, |maps| {
            let refused = maps.remove(Role::Pending, full, word(5));
            assert!(
                matches!(refused, Err(native::Error::AuthorityMismatch(m)) if m == REMOVAL_ABSENT)
            );
            refused
        });
        assert!(matches!(
            missing,
            Err(ProviderError::Invalid {
                field: "future wallet proof authority"
            })
        ));
        let (expected, checkpoint) = selected(&mut provider, &slot, parameters, wallet);
        let unknown = stage_transfer_maps(&mut provider, &slot, expected, checkpoint, |maps| {
            maps.remove(Role::Pending, word(77), word(9))
        });
        assert!(matches!(
            unknown,
            Err(ProviderError::UnavailableCustodyData {
                object: "future transfer map descriptor"
            })
        ));
        let (expected, checkpoint) = selected(&mut provider, &slot, parameters, wallet);
        let other = stage_transfer_maps(&mut provider, &slot, expected, checkpoint, |maps| {
            maps.remove(Role::Fee, full, word(9))
        });
        assert!(matches!(other, Err(ProviderError::Invalid { .. })));
        assert_eq!(snapshot(&root), source, "refused staging writes nothing");
        assert_eq!(selected(&mut provider, &slot, parameters, wallet).0, expected);

        // Staging: the committed and lineage-adjusted removals from the same retained root.
        let (expected, checkpoint) = selected(&mut provider, &slot, parameters, wallet);
        let removed = stage_transfer_maps(&mut provider, &slot, expected, checkpoint, |maps| {
            let core = maps.remove(Role::Pending, full, word(9))?;
            let adjusted = maps.remove(Role::Pending, full, word(9))?;
            assert_eq!(core, adjusted);
            Ok(core)
        })
        .unwrap();
        let after = removed.verify(&full, &word(9)).unwrap();
        let mut tree = stock(&HISTORY);
        assert_eq!(tree.remove(&word(9)).unwrap(), removed, "stock indexed tree agrees");
        assert_eq!(tree.root(), after);
        let (published, _) = selected(&mut provider, &slot, parameters, wallet);
        assert_ne!(published, expected, "the successor descriptor is published");
        let staged = snapshot(&root);
        for (path, bytes) in source.iter().filter(|(path, _)| is_original(path)) {
            assert_eq!(staged.get(path), Some(bytes), "{} unchanged", path.display());
        }
        let added = staged
            .keys()
            .filter(|path| !source.contains_key(*path) && is_original(path))
            .count();
        assert!(added > 0, "new content-addressed records only");

        // Exact retry: the identical witness, and nothing new to publish.
        let (expected, checkpoint) = selected(&mut provider, &slot, parameters, wallet);
        let retried = stage_transfer_maps(&mut provider, &slot, expected, checkpoint, |maps| {
            maps.remove(Role::Pending, full, word(9))
        })
        .unwrap();
        assert_eq!(retried, removed);
        assert_eq!(selected(&mut provider, &slot, parameters, wallet).0, published);
        assert_eq!(snapshot(&root), staged);
        drop(provider);

        // Cold reopen reads both roots; the removed entry is an authenticated absence.
        let mut provider = reopen(&platform);
        let (expected, checkpoint) = selected(&mut provider, &slot, parameters, wallet);
        let descriptors = checkpoint.transfer_descriptors;
        stage_transfer_maps(&mut provider, &slot, expected, checkpoint, |maps| {
            let (low, path) = maps.member_or_low(Role::Pending, after, word(9))?;
            assert_eq!((low.key, low.next_key), (word(4), word(27)));
            absent(&after, &word(9), &low, &path).unwrap();
            assert_eq!(maps.member_or_low(Role::Pending, full, word(9))?.0.value, word(90));
            let again = maps.remove(Role::Pending, after, word(9));
            assert!(
                matches!(again, Err(native::Error::AuthorityMismatch(m)) if m == REMOVAL_ABSENT)
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(snapshot(&root), staged);

        // Both copies of the successor descriptor lost: custody loss, never absence.
        let address: [u8; 32] = provider
            .with_archive(&slot, |archive| {
                IndexRoot(descriptors)
                    .get(&mut Objects(archive), &key(Role::Pending, after))
                    .map_err(storage_error)
            })
            .unwrap()
            .unwrap()
            .try_into()
            .unwrap();
        drop(provider);
        let primary = format!("w-{}.rec", hex::encode(address));
        let copy = format!("{primary}.r");
        let mut deleted = 0;
        for path in staged.keys() {
            let name = path.file_name().unwrap().to_str().unwrap();
            if name == primary || name == copy {
                std::fs::remove_file(root.join(path)).unwrap();
                deleted += 1;
            }
        }
        assert_eq!(deleted, 2, "both durable copies of the successor descriptor");
        let damaged = snapshot(&root);
        let mut provider = reopen(&platform);
        let (expected, checkpoint) = selected(&mut provider, &slot, parameters, wallet);
        let lost = stage_transfer_maps(&mut provider, &slot, expected, checkpoint, |maps| {
            maps.remove(Role::Pending, after, word(27))
        });
        assert!(
            matches!(lost, Err(ProviderError::UnavailableCustodyData { .. })),
            "{lost:?}"
        );
        assert_eq!(snapshot(&root), damaged, "a lost-custody refusal writes nothing");
        // The source root reads only its own retained objects.
        let (expected, checkpoint) = selected(&mut provider, &slot, parameters, wallet);
        let source_again = stage_transfer_maps(&mut provider, &slot, expected, checkpoint, |maps| {
            maps.remove(Role::Pending, full, word(9))
        })
        .unwrap();
        assert_eq!(source_again, removed);
    }

    /// Exact copy of a retained custody tree into an owner-only scratch root.
    fn copy_tree(from: &Path, to: &Path) {
        owner_only(to);
        for entry in std::fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let target = to.join(entry.file_name());
            let kind = entry.file_type().unwrap();
            if kind.is_dir() {
                copy_tree(&entry.path(), &target);
            } else if kind.is_file() {
                std::fs::copy(entry.path(), &target).unwrap();
            } else {
                panic!("unexpected custody entry {}", entry.path().display());
            }
        }
    }

    /// The retained three-launch host wallet: its own custody (copied; the fixture is only
    /// read), its exact released Send, Request, Credited status and later Receive head.
    ///
    /// Host-local test custody only: the public fixture scheme root and finality fixture
    /// that signed the bundle, and the canonical software platform of the host run. No
    /// hardware key, attestation or production trust root is involved or implied.
    #[test]
    #[ignore = "requires the retained three-launch host custody, its exact Send and Receive originals and the signed fixture bundle"]
    fn retained_host_custody_stages_archive_sent_from_the_retail_pending_store() {
        use crate::kagemusha_wallet_advance_v1::{
            KagemushaWalletKeyGenerationRequestV1, KagemushaWalletKeyGenerationV1,
            KagemushaWalletKeyProfileV1, KagemushaWalletLookupV1 as Lookup,
            KagemushaWalletProviderOptionsV1, KagemushaWalletSlotStatusV1 as Status,
        };
        use crate::kagemusha_wallet_state_v1::NativeProviderV1;
        use ff::PrimeField;
        use iroha_data_model::{
            kagemusha::{
                KagemushaDevicePublicKeyV1, KagemushaWalletSchemeV1,
                kagemusha_wallet_provider_contract_v1,
            },
            sumeragi_finality::{SumeragiCommitVerifierV1, test_fixtures::NativeFinalityFixture},
        };
        use std::time::Instant;

        let input = |name: &str| {
            PathBuf::from(
                std::env::var_os(name).unwrap_or_else(|| panic!("missing real input {name}")),
            )
        };
        let read = |path: PathBuf| {
            std::fs::read(&path)
                .unwrap_or_else(|e| panic!("missing real input {}: {e}", path.display()))
        };
        let originals = input("KAGEMUSHA_SPARTAN_ARCHIVE_RAW_ORIGINALS");
        let device = input("KAGEMUSHA_SPARTAN_DEVICE_ROOT");
        let bundle = input("KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS");

        // The public fixture scheme (root secret 0x11.., relation 0x33..). Installation
        // authenticates it against the signed bundle; a different scheme cannot install.
        let chain = NativeFinalityFixture::start_with_explicit_parameters("portable-native-fixture");
        let root_key = p256::ecdsa::SigningKey::from_slice(&[0x11; 32]).unwrap();
        let scheme = KagemushaWalletSchemeV1 {
            version: 1,
            network_id: *chain.network_id().as_bytes(),
            scheme_root_key: KagemushaDevicePublicKeyV1::from_sec1_bytes(
                root_key.verifying_key().to_sec1_point(false).as_bytes(),
            )
            .unwrap(),
            relation_id: [0x33; 32],
            provider_contract: kagemusha_wallet_provider_contract_v1(),
        };
        let finality = SumeragiCommitVerifierV1::new(
            &NativeFinalityFixture::new_with_explicit_parameters().verifier(),
        )
        .unwrap();
        let parameters = AuthenticatedParameters::install(
            &read(bundle.join("program-bundle.norito")),
            &read(bundle.join("parameter-manifest.norito")),
            read(bundle.join("parameter-signature.bin")).try_into().unwrap(),
            &scheme,
            finality,
        )
        .unwrap();
        let scheme_id = scheme.scheme_id();
        let engine = native::NativeEngine::default();

        // Exact retained Send and later Receive frames, released again natively.
        let mut operations = Vec::new();
        for path in std::fs::read_dir(&originals)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|e| e == "cap"))
        {
            let capsule_bytes = read(path.clone());
            let completion_bytes = read(path.with_extension("cr"));
            let capsule = RecoveryCapsule::decode(&capsule_bytes, &scheme_id).unwrap();
            let record = CompletionRecord::decode(&completion_bytes, capsule.wallet()).unwrap();
            operations.push((capsule, record, capsule_bytes, completion_bytes));
        }
        let pick = |kind: u8| {
            operations
                .iter()
                .find(|(capsule, ..)| capsule.kind() == kind)
                .unwrap_or_else(|| panic!("missing real input: retained kind {kind} frames"))
        };
        let (send_frame, send_record, send_capsule, send_completion) = pick(3);
        let (receive_frame, receive_record, _, _) = pick(4);
        let started = Instant::now();
        let (send, _) = engine
            .verify_released_operation(send_frame, send_record, &parameters)
            .unwrap();
        let (receive, later) = engine
            .verify_released_operation(receive_frame, receive_record, &parameters)
            .unwrap();
        eprintln!(
            "released Send and later Receive verified {:.2} s",
            started.elapsed().as_secs_f64()
        );
        assert!(receive.capsule().sequence() > send.capsule().sequence());
        let payment = engine
            .verify_payment(&read(device.join("complete-payment.bin")), &parameters)
            .unwrap();
        let open_request = std::fs::read_dir(&originals)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|e| e == "ipm1"))
            .map(|path| read(path))
            .find(|bytes| {
                native::decode_open_request_peer(bytes)
                    .is_ok_and(|open| open.request.ends_with(&payment.request().signature))
            })
            .expect("missing real input: the recipient-signed open Request");
        let status = read(device.join("outgoing-credit-status.bin"));
        let fields = payment.statement();
        let credit = fields[17].to_repr();
        let descriptor = iroha_pasta::poseidon::hash_with_domain(
            u64::from_le_bytes(*b"kgwpout1"),
            &fields[17..24],
        )
        .to_repr();
        let mut tree = KagemushaWalletIndexedTreeV1::new();
        tree.insert(credit, descriptor).unwrap();
        let pending = tree.root();
        let stock_removal = tree.clone().remove(&credit).unwrap();
        let empty = PersistentMapV1::default().root();

        // Scratch copy of the retained custody, under the host software platform.
        let scratch = tempfile::tempdir().unwrap();
        let custody = scratch.path().join("custody");
        copy_tree(&device.join("custody"), &custody);
        let platform = FakePlatformV1::new(KagemushaWalletAnchorPolicyV1::NotRequired, 0x5a);
        for entry in read(device.join("host-software-keys.bin")).chunks(65) {
            let slot = Slot(entry[..32].try_into().unwrap());
            let request = KagemushaWalletKeyGenerationRequestV1 {
                challenge_digest: entry[32..64].try_into().unwrap(),
                profile: KagemushaWalletKeyProfileV1::from_tag(entry[64]).unwrap(),
            };
            assert!(matches!(
                platform.key_generate(&slot, &request),
                KagemushaWalletKeyGenerationV1::Generated(_)
            ));
        }
        let open = |platform: &FakePlatformV1| {
            let provider: NativeProviderV1<_, _> = KagemushaWalletProviderV1::open(
                KagemushaWalletStdFsV1::open(&custody).unwrap(),
                platform.clone(),
                scheme_id,
                KagemushaWalletProviderOptionsV1::default(),
            )
            .unwrap();
            into_future_frames(provider, &parameters)
        };
        let mut provider = open(&platform);
        let slots = provider.slots().unwrap();
        assert_eq!(slots.len(), 1, "one retained custody slot");
        let slot = slots.into_iter().next().unwrap();
        let Status::Released(marker) = provider.status(&slot).unwrap() else {
            panic!("missing real input: a released retained head");
        };
        let wallet = marker.marker().wallet_id;
        assert_eq!(receive.capsule().wallet(), &wallet);
        eprintln!(
            "retained head {:?}, Receive capsule {}",
            marker.head().map(|(sequence, _, capsule)| (sequence, hex::encode(capsule))),
            hex::encode(RecoveryCapsule::frame_digest(&receive_frame.encode().unwrap()))
        );
        // The exact Send originals are this custody's own retained Send.
        let Lookup::Retained(row) = provider
            .lookup(&slot, send.capsule().operation_id())
            .unwrap()
        else {
            panic!("missing real input: the custody no longer retains the Send completion");
        };
        assert_eq!(row.capsule_digest, RecoveryCapsule::frame_digest(send_capsule));
        assert_eq!(&row.frame, send_completion);
        let manifest = *parameters.manifest_digest();

        // The Send's pending descriptor as the retail store retained it.
        let (expected, checkpoint) = selected(&mut provider, &slot, manifest, wallet);
        let retained = stage_transfer_maps(&mut provider, &slot, expected, checkpoint, |maps| {
            let refused = maps.remove(Role::Credit, pending, credit);
            assert!(matches!(refused, Err(native::Error::AuthorityMismatch(m)) if m == REMOVAL_ROLE));
            let refused = maps.remove(Role::Pending, empty, credit);
            assert!(matches!(refused, Err(native::Error::AuthorityMismatch(m)) if m == REMOVAL_ABSENT));
            maps.member_or_low(Role::Pending, pending, credit)
        });
        match &retained {
            Ok((leaf, _)) if leaf.key == credit && leaf.value == descriptor => eprintln!(
                "retained pending descriptor present at root {}",
                hex::encode(pending)
            ),
            other => {
                eprintln!(
                    "DIAGNOSTIC missing real input: no retained pending descriptor at root {}: {}",
                    hex::encode(pending),
                    match other {
                        Ok((leaf, _)) => format!("low leaf {}", hex::encode(leaf.key)),
                        Err(error) => format!("{error:?}"),
                    }
                );
                // Seed only the scratch copy through the Send's own production insertion.
                let (expected, checkpoint) = selected(&mut provider, &slot, manifest, wallet);
                let plan = stage_transfer_maps(&mut provider, &slot, expected, checkpoint, |maps| {
                    maps.insert(Role::Pending, empty, credit, descriptor)
                })
                .unwrap();
                assert_eq!(plan.verify(&empty, &credit, &descriptor).unwrap(), pending);
                eprintln!("seeded the scratch copy with the Send's pending insertion");
            }
        }
        drop(provider);

        // Cold reopen, then stage the ArchiveSent witness through the retail store.
        let mut provider = open(&platform);
        let source = snapshot(&custody);
        let (expected, checkpoint) = selected(&mut provider, &slot, manifest, wallet);
        let Status::Released(marker) = provider.status(&slot).unwrap() else {
            panic!("released head");
        };
        let head = marker.head();
        let build = |provider: &mut FutureProvider<KagemushaWalletStdFsV1, FakePlatformV1>,
                     expected: [u8; 32],
                     checkpoint: Checkpoint| {
            let started = Instant::now();
            let witness = stage_transfer_maps(provider, &slot, expected, checkpoint, |maps| {
                engine.wallet_archive_sent_witness(
                    &receive,
                    later.head(),
                    send_capsule,
                    send_completion,
                    &open_request,
                    &status,
                    maps,
                    &parameters,
                )
            })
            .unwrap();
            eprintln!(
                "ArchiveSent witness from the retail pending store {:.2} s",
                started.elapsed().as_secs_f64()
            );
            witness
        };
        let witness = build(&mut provider, expected, checkpoint);
        let native::ExactOperationWitness::ArchiveSent {
            step,
            send_capsule: kept_capsule,
            send_completion: kept_completion,
            open_request: kept_open,
            credit_status: kept_status,
        } = &witness
        else {
            panic!("ArchiveSent witness");
        };
        assert_eq!(step.core_removal, stock_removal, "stock indexed tree agrees");
        assert_eq!(step.adjusted_removal, stock_removal);
        assert_eq!(step.core_removal.verify(&pending, &credit).unwrap(), empty);
        assert_eq!(step.core_removal.leaf.value, descriptor, "the exact Send descriptor");
        assert_eq!(&kept_capsule[..], &send_capsule[..]);
        assert_eq!(&kept_completion[..], &send_completion[..]);
        assert_eq!(&kept_open[..], &open_request[..]);
        assert_eq!(&kept_status[..], &status[..]);
        assert_eq!(snapshot(&custody), source, "staging wrote no durable byte");
        assert_eq!(selected(&mut provider, &slot, manifest, wallet).0, expected);
        let Status::Released(now) = provider.status(&slot).unwrap() else {
            panic!("released head");
        };
        assert_eq!(now.head(), head, "the selected head is unchanged");
        let bytes = witness.encode().unwrap();
        assert_eq!(
            native::ExactOperationWitness::decode(&bytes)
                .unwrap()
                .encode()
                .unwrap(),
            bytes
        );
        let started = Instant::now();
        let refused = engine.prepare(&witness, later.head(), &parameters);
        eprintln!(
            "ArchiveSent prepare recheck {:.2} s, witness {} B",
            started.elapsed().as_secs_f64(),
            bytes.len()
        );
        assert!(
            matches!(
                &refused,
                Err(native::Error::AuthorityMismatch(m))
                    if *m == "signed generation has no ArchiveSent program"
            ),
            "{:?}",
            refused.err()
        );
        drop(provider);

        // Exact retry after a process loss: identical removals and operation identity.
        let mut provider = open(&platform);
        let (expected, checkpoint) = selected(&mut provider, &slot, manifest, wallet);
        let retry = build(&mut provider, expected, checkpoint);
        let native::ExactOperationWitness::ArchiveSent { step: again, .. } = &retry else {
            panic!("ArchiveSent witness");
        };
        assert_eq!(again.core_removal, step.core_removal);
        assert_eq!(again.adjusted_removal, step.adjusted_removal);
        assert_eq!(again.receipt.body[114..146], step.receipt.body[114..146]);
        assert_eq!(again.statement[14], step.statement[14]);
        assert_eq!(again.statement[17..19], step.statement[17..19]);
        assert_ne!(again.after_core[29], step.after_core[29], "a fresh state nonce");
        assert_eq!(snapshot(&custody), source);
        eprintln!(
            "ArchiveSent operation {} credit {}",
            hex::encode(&step.receipt.body[114..146]),
            hex::encode(credit)
        );
    }
}
