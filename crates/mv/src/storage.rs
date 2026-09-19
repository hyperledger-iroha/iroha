use crate::{
    BlockMode, Key, PublicationPreparationError, PublicationPreparationResult, ReleaseGuard,
    ReleaseNotification, Value,
    publication::{CapturedPublication, NextPublication, Publication},
};
use concread::{
    bptree::{
        BptreeMap, BptreeMapOwned, BptreeMapReadSnapshot, BptreeMapReadTxn, BptreeMapWriteTxn,
        OwnedWriteError,
    },
    ebrcell::{EbrCell, EbrCellOwned, EbrCellWriteTxn},
};
use std::{borrow::Borrow, collections::BTreeMap, ops::RangeBounds};
/// Multi-version key value storage
pub struct Storage<K: Key, V: Value> {
    /// Process-local identity of the jointly published current/undo pair.
    pub(crate) publication: Publication,
    pub(crate) revert_released: ReleaseNotification,
    pub(crate) blocks_released: ReleaseNotification,
    /// Previous version of values in the `blocks` map, required to perform revert of the latest changes
    pub(crate) revert: EbrCell<BTreeMap<K, Option<V>>>,
    /// Map which represent aggregated changes of multiple blocks
    pub(crate) blocks: BptreeMap<K, V>,
}
impl<K: Key, V: Value> Storage<K, V> {
    /// Construct new [`Self`]
    pub fn new() -> Self {
        Self {
            publication: Publication::new(),
            revert_released: ReleaseNotification::default(),
            blocks_released: ReleaseNotification::default(),
            revert: EbrCell::new(BTreeMap::new()),
            blocks: BptreeMap::new(),
        }
    }
    /// Create persistent view of storage at certain point in time
    pub fn view(&self) -> View<'_, K, V> {
        let read = self.blocks.read();
        View::from_read_txn(read)
    }
    /// Create block to aggregate updates
    pub fn block(&self) -> Block<'_, K, V> {
        let mut revert = self.revert_released.poisoning_guard(
            self.revert_released
                .with_acquisition_unwind_notification(|| self.revert.write()),
        );
        let blocks = self.blocks_released.poisoning_guard(
            self.blocks_released
                .with_acquisition_unwind_notification(|| self.blocks.write()),
        );
        let predecessor = self.publication.capture();
        // Clear revert
        revert.get_mut().clear();
        Block::new(
            revert,
            blocks,
            false,
            &self.publication,
            predecessor,
            BlockMode::Ordinary,
        )
    }
    /// Insert a value directly into the latest committed state.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        let mut blocks = self.blocks_released.poisoning_guard(
            self.blocks_released
                .with_acquisition_unwind_notification(|| self.blocks.write()),
        );
        let prev_value = blocks.insert(key, value);
        self.publication
            .publish(|| blocks.release_with(|guard| guard.commit()));
        prev_value
    }
    /// Create block to aggregate updates and revert changes created in the latest block
    pub fn block_and_revert(&self) -> Block<'_, K, V> {
        let mut revert = self.revert_released.poisoning_guard(
            self.revert_released
                .with_acquisition_unwind_notification(|| self.revert.write()),
        );
        let mut blocks = self.blocks_released.poisoning_guard(
            self.blocks_released
                .with_acquisition_unwind_notification(|| self.blocks.write()),
        );
        let predecessor = self.publication.capture();
        {
            let revert = core::mem::take(revert.get_mut());
            for (key, value) in revert {
                match value {
                    None => blocks.remove(&key),
                    Some(value) => blocks.insert(key, value),
                };
            }
        }
        Block::new(
            revert,
            blocks,
            true,
            &self.publication,
            predecessor,
            BlockMode::Replace,
        )
    }
}
impl<K: Key, V: Value> Default for Storage<K, V> {
    fn default() -> Self {
        Self::new()
    }
}
impl<K: Key, V: Value> FromIterator<(K, V)> for Storage<K, V> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        Self {
            publication: Publication::new(),
            revert_released: ReleaseNotification::default(),
            blocks_released: ReleaseNotification::default(),
            revert: EbrCell::new(BTreeMap::new()),
            blocks: iter.into_iter().collect(),
        }
    }
}
pub trait StorageReadOnly<K: Key, V: Value> {
    /// Read entry from the storage
    fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Ord + Borrow<Q>,
        Q: Ord + ?Sized;
    /// Read entry from the storage together with the canonical key reference.
    ///
    /// This helper is useful when callers need the key reference borrowed from
    /// the storage (for example to build a `Ref<'_, K, V>` pair) rather than a
    /// cloned key value.
    fn get_key_value(&self, key: &K) -> Option<(&K, &V)> {
        self.range(key..=key)
            .next()
            .filter(|(candidate, _)| *candidate == key)
    }
    /// Iterate over all entries in the storage
    fn iter(&self) -> Iter<'_, K, V>;
    /// Iterate over range of entries in the storage
    fn range<Q>(&self, bounds: impl RangeBounds<Q>) -> RangeIter<'_, K, V>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized;
    /// Read the entry with the smallest canonical key without walking the map.
    fn first_key_value(&self) -> Option<(&K, &V)>;
    /// Read the entry with the largest canonical key without walking the map.
    fn last_key_value(&self) -> Option<(&K, &V)>;
    /// Get amount of entries in the storage
    fn len(&self) -> usize;
    /// Check whether the storage contains no entries
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
/// Module for [`View`] and it's related impls
mod view {
    use super::*;
    enum ViewInner<'storage, K: Key, V: Value> {
        Txn(BptreeMapReadTxn<'storage, K, V>),
        Snapshot(BptreeMapReadSnapshot<'storage, K, V>),
    }
    /// Consistent view of the storage at the certain version
    pub struct View<'storage, K: Key, V: Value> {
        blocks: ViewInner<'storage, K, V>,
    }
    impl<'storage, K: Key, V: Value> View<'storage, K, V> {
        pub(crate) fn from_read_txn(read: BptreeMapReadTxn<'storage, K, V>) -> Self {
            Self {
                blocks: ViewInner::Txn(read),
            }
        }
        pub(crate) fn from_snapshot(snapshot: BptreeMapReadSnapshot<'storage, K, V>) -> Self {
            Self {
                blocks: ViewInner::Snapshot(snapshot),
            }
        }
    }
    impl<K: Key, V: Value> StorageReadOnly<K, V> for View<'_, K, V> {
        fn get<Q>(&self, key: &Q) -> Option<&V>
        where
            K: Ord + Borrow<Q>,
            Q: Ord + ?Sized,
        {
            match &self.blocks {
                ViewInner::Txn(txn) => txn.get(key),
                ViewInner::Snapshot(snapshot) => snapshot.get(key),
            }
        }
        fn iter(&self) -> Iter<'_, K, V> {
            Iter {
                iter: match &self.blocks {
                    ViewInner::Txn(txn) => Box::new(txn.iter()),
                    ViewInner::Snapshot(snapshot) => Box::new(snapshot.iter()),
                },
            }
        }
        fn range<Q>(&self, bounds: impl RangeBounds<Q>) -> RangeIter<'_, K, V>
        where
            K: Borrow<Q>,
            Q: Ord + ?Sized,
        {
            RangeIter {
                iter: match &self.blocks {
                    ViewInner::Txn(txn) => Box::new(txn.range(bounds)),
                    ViewInner::Snapshot(snapshot) => Box::new(snapshot.range(bounds)),
                },
            }
        }
        fn first_key_value(&self) -> Option<(&K, &V)> {
            match &self.blocks {
                ViewInner::Txn(txn) => txn.first_key_value(),
                ViewInner::Snapshot(snapshot) => snapshot.iter().next(),
            }
        }
        fn last_key_value(&self) -> Option<(&K, &V)> {
            match &self.blocks {
                ViewInner::Txn(txn) => txn.last_key_value(),
                // `concread` does not expose endpoint lookup on a write-backed
                // snapshot. Such snapshots only exist inside an in-flight state
                // transaction; persistent read views take the indexed branch.
                ViewInner::Snapshot(snapshot) => snapshot.iter().last(),
            }
        }
        /// Get amount of entries in the storage
        fn len(&self) -> usize {
            match &self.blocks {
                ViewInner::Txn(txn) => txn.len(),
                ViewInner::Snapshot(snapshot) => snapshot.len(),
            }
        }
    }
}
pub use view::View;
/// Borrowed before/after values for one key touched by an overlay.
///
/// `None` represents absence, including removal of a missing key. A touched key
/// may have equal before/after values. Consumers which commit semantic changes
/// must compare their canonical value projections instead of hashing the fact
/// that a mutable accessor was used. Borrowing this record prevents further
/// mutation of its originating overlay until the borrow ends.
pub struct TouchedEntry<'a, K: Key, V: Value> {
    /// Key in the storage's canonical order.
    pub key: &'a K,
    /// Value before the overlay's first mutation of this key.
    pub before: Option<&'a V>,
    /// Current value, including changes from applied children.
    pub after: Option<&'a V>,
}

/// Exact original map and undo successors, without physical writer locks.
///
/// This move-only journal preserves the original nodes, keys and values, including
/// no-op touches. The map owner also retains its original root and base reader
/// generation so untouched shared nodes remain alive even if Storage is dropped.
/// Replacement semantics are already staged by the original block. Publication
/// reacquires and authenticates that same current/undo pair without rebuilding it.
pub struct Detached<K: Key, V: Value, Admission> {
    revert: EbrCellOwned<BTreeMap<K, Option<V>>>,
    blocks: BptreeMapOwned<K, V>,
    // Release metadata admission after the original successors and their pins.
    metadata: DetachedMetadata<Admission>,
}

struct DetachedMetadata<Admission> {
    predecessor: CapturedPublication,
    mode: BlockMode,
    dirty: bool,
    next: NextPublication,
    admission: Admission,
}

impl<K: Key, V: Value, Admission> Detached<K, V, Admission> {
    /// Return the actual original block's acquisition mode.
    pub fn mode(&self) -> BlockMode {
        self.metadata.mode
    }

    /// Return whether the original block requires current-map publication.
    pub fn is_dirty(&self) -> bool {
        self.metadata.dirty
    }

    /// Borrow original preimages and successors, including no-ops, in key order.
    pub fn touched_entries(
        &self,
    ) -> impl DoubleEndedIterator<Item = TouchedEntry<'_, K, V>> + ExactSizeIterator {
        self.revert.iter().map(|(key, before)| TouchedEntry {
            key,
            before: before.as_ref(),
            after: self.blocks.get(key),
        })
    }

    /// Borrow the caller's separate capture reservation.
    /// Actual node, nested payload and reader-chain custody needs its own admission.
    pub fn admission(&self) -> &Admission {
        &self.metadata.admission
    }

    /// Observe equality with the original published current/undo pair.
    /// This momentary observation grants no publication authority.
    pub fn matches_current(&self, storage: &Storage<K, V>) -> bool {
        self.metadata.predecessor.matches(&storage.publication)
    }

    /// Compare exact original owner/version and mode with an acquired block.
    pub fn matches_block_predecessor(&self, block: &Block<'_, K, V>) -> bool {
        self.metadata.mode == block.mode && self.metadata.predecessor.same_as(&block.predecessor)
    }

    /// Reacquire both original writers around the exact retained successors.
    ///
    /// No key/value clone, delta reconstruction or successor allocation occurs.
    /// The original next identity survives every refusal and abort. The callback
    /// may retain separately prepaid temporary installation resources; it cannot
    /// retroactively admit the original execution or nested allocations.
    ///
    /// Both MV pair identity and the map's exact root/base generation are checked.
    /// Acquisition never waits and every refusal returns the original journal.
    /// Aggregate publication and complete resource admission remain the caller's
    /// responsibility; prepare every component before publishing the first one.
    pub fn try_prepare_publication<'target, Installation, E>(
        self,
        target: &'target Storage<K, V>,
        admit: impl FnOnce(&Self, &Storage<K, V>) -> Result<Installation, E>,
    ) -> PublicationPreparationResult<
        PreparedPublication<'target, K, V, Admission, Installation>,
        Self,
        E,
    > {
        if let Err(error) = self
            .metadata
            .predecessor
            .try_check_current(&target.publication)
        {
            return Err((self, error));
        }
        let installation = match admit(&self, target) {
            Ok(installation) => installation,
            Err(error) => return Err((self, PublicationPreparationError::Admission(error))),
        };
        let Self {
            revert,
            blocks,
            metadata,
        } = self;
        let wait = target.revert_released.observe();
        let revert = match target.revert.try_write_owned(revert) {
            Ok(writer) => target.revert_released.poisoning_guard(writer),
            Err(revert) => {
                let error = if target.revert.is_poisoned() {
                    PublicationPreparationError::Poisoned
                } else {
                    PublicationPreparationError::after_failed_acquisition(wait)
                };
                return Err((
                    Self {
                        revert,
                        blocks,
                        metadata,
                    },
                    error,
                ));
            }
        };
        let wait = target.blocks_released.observe();
        let blocks = match target.blocks.try_write_owned(blocks) {
            Ok(writer) => target.blocks_released.poisoning_guard(writer),
            Err((blocks, error)) => {
                let error = match error {
                    OwnedWriteError::Busy => {
                        PublicationPreparationError::after_failed_acquisition(wait)
                    }
                    OwnedWriteError::Poisoned => PublicationPreparationError::Poisoned,
                    OwnedWriteError::Changed => PublicationPreparationError::Changed,
                };
                let revert = revert.release_with(|writer| writer.detach());
                return Err((
                    Self {
                        revert,
                        blocks,
                        metadata,
                    },
                    error,
                ));
            }
        };
        let prepared = PreparedPublication {
            revert,
            blocks,
            publication: &target.publication,
            metadata,
            installation,
        };
        if let Err(error) = prepared
            .metadata
            .predecessor
            .try_check_current(&target.publication)
        {
            return Err((prepared.abort(), error));
        }
        Ok(prepared)
    }
}

/// Original map and undo successors held under both exact target writers.
/// Drop abandons them without publication; abort returns the original owners.
#[must_use = "preparation must be published or aborted by its aggregate owner"]
pub struct PreparedPublication<'target, K: Key, V: Value, Admission, Installation> {
    revert: ReleaseGuard<'target, EbrCellWriteTxn<'target, BTreeMap<K, Option<V>>>>,
    blocks: ReleaseGuard<'target, BptreeMapWriteTxn<'target, K, V>>,
    publication: &'target Publication,
    metadata: DetachedMetadata<Admission>,
    // Release temporary resources after every retained successor and writer.
    installation: Installation,
}

impl<K: Key, V: Value, Admission, Installation>
    PreparedPublication<'_, K, V, Admission, Installation>
{
    /// Release physical writers and return the exact original successors.
    pub fn abort(self) -> Detached<K, V, Admission> {
        let Self {
            revert,
            blocks,
            publication: _,
            metadata,
            installation,
        } = self;
        let blocks = blocks.release_with(|writer| writer.detach());
        let revert = revert.release_with(|writer| writer.detach());
        drop(installation);
        Detached {
            revert,
            blocks,
            metadata,
        }
    }

    /// Publish the original prepared pair and return separate reservations.
    ///
    /// Original map successors and the next identities were prepared before
    /// detachment. This does not establish complete heap admission: nested data,
    /// cursor/node custody and collector/control bookkeeping still need their own
    /// policy. The aggregate owner supplies joint visibility and finality.
    pub fn publish(self) -> (Admission, Installation) {
        let Self {
            revert,
            blocks,
            publication,
            metadata,
            installation,
        } = self;
        let DetachedMetadata {
            predecessor: _,
            mode: _,
            dirty,
            next,
            admission,
        } = metadata;
        publication.publish_prepared(next, || {
            if dirty {
                blocks.release_with(|guard| guard.commit());
            }
            revert.release_with(|guard| guard.commit());
        });
        (admission, installation)
    }
}

#[cfg(test)]
#[path = "storage/publication_tests.rs"]
mod publication_tests;

/// Module for [`Block`] and it's related impls
mod block {
    use super::*;
    /// Batched update to the storage that can be reverted later
    pub struct Block<'store, K: Key, V: Value> {
        pub(crate) revert: ReleaseGuard<'store, EbrCellWriteTxn<'store, BTreeMap<K, Option<V>>>>,
        pub(crate) blocks: ReleaseGuard<'store, BptreeMapWriteTxn<'store, K, V>>,
        pub(super) dirty: bool,
        pub(super) publication: &'store Publication,
        pub(super) predecessor: CapturedPublication,
        pub(super) mode: BlockMode,
    }
    impl<'store, K: Key, V: Value> Block<'store, K, V> {
        /// Observe this block's original owner, current/undo predecessor and mode.
        /// The opaque identity permits only local equality, never publication.
        pub fn publication_identity(&self) -> crate::BlockPublicationIdentity {
            crate::BlockPublicationIdentity::capture(&self.predecessor, self.mode)
        }

        /// Check the original storage owner without reading values or taking locks.
        /// This observation grants no mutation or publication authority.
        pub fn belongs_to(&self, storage: &Storage<K, V>) -> bool {
            self.predecessor.belongs_to(&storage.publication)
        }

        pub(super) fn new(
            revert: ReleaseGuard<'store, EbrCellWriteTxn<'store, BTreeMap<K, Option<V>>>>,
            blocks: ReleaseGuard<'store, BptreeMapWriteTxn<'store, K, V>>,
            dirty: bool,
            publication: &'store Publication,
            predecessor: CapturedPublication,
            mode: BlockMode,
        ) -> Self {
            Self {
                revert,
                blocks,
                dirty,
                publication,
                predecessor,
                mode,
            }
        }
        /// Create transaction for the block
        pub fn transaction<'block>(&'block mut self) -> Transaction<'block, 'store, K, V>
        where
            'store: 'block,
        {
            Transaction {
                applied: false,
                dirty_before: self.dirty,
                block: self,
                revert: BTreeMap::new(),
            }
        }
        /// Apply aggregated changes to the storage
        pub fn commit(self) {
            let Self {
                revert,
                blocks,
                dirty,
                publication,
                predecessor: _,
                mode: _,
            } = self;
            publication.publish(|| {
                // Commit fields in the inverse order. Even an untouched block
                // publishes its clear-undo transition and changes pair identity.
                if dirty {
                    blocks.release_with(|guard| guard.commit());
                }
                revert.release_with(|guard| guard.commit());
            });
        }

        /// Admit capture metadata, then retain the exact original successors.
        ///
        /// The callback runs before next-identity allocation. No key/value clone
        /// or delta vector is needed: current and undo move with their original
        /// allocation owners. The map retains its original root/base generation
        /// to protect untouched shared nodes after releasing the physical writer.
        /// Original execution, nested payload growth and retained-reader resources
        /// require their own earlier admission; this callback cannot fund them
        /// retroactively. Rejection abandons the block without publication.
        pub fn try_detach<Admission, E>(
            self,
            admit: impl FnOnce(&Self) -> Result<Admission, E>,
        ) -> Result<Detached<K, V, Admission>, E> {
            let admission = admit(&self)?;
            let next = NextPublication::new();
            let Self {
                revert,
                blocks,
                dirty,
                predecessor,
                mode,
                publication: _,
            } = self;
            let blocks = blocks.release_with(|writer| writer.detach());
            let revert = revert.release_with(|writer| writer.detach());
            Ok(Detached {
                revert,
                blocks,
                metadata: DetachedMetadata {
                    predecessor,
                    mode,
                    dirty,
                    next,
                    admission,
                },
            })
        }
        /// Read-only access to the block revert map (keys touched in this block).
        pub fn revert_map(&self) -> &BTreeMap<K, Option<V>> {
            &self.revert
        }
        /// Read the value that existed before this block's first mutation of `key`.
        ///
        /// The block undo log retains the first pre-block value across direct
        /// mutations and applied child transactions. An undo entry containing
        /// `None` means the key was absent before the block; an untouched key is
        /// read from the current map.
        pub fn get_before_block(&self, key: &K) -> Option<&V> {
            match self.revert_map().get(key) {
                Some(previous) => previous.as_ref(),
                None => self.get(key),
            }
        }
        /// Visit this block's touched keys with their exact pre-block and current values.
        ///
        /// Entries are ordered by `K::Ord`, independent of mutation order. The
        /// iterator borrows the undo journal and current storage without cloning
        /// keys/values or allocating another change list. Aborted transactions
        /// contribute no entries; applied transactions retain the first block
        /// preimage. No-op touches remain visible, including absent-to-absent.
        /// For `block_and_revert`, the preimage starts after the prior block was
        /// reverted, just like [`Self::get_before_block`].
        pub fn touched_entries(
            &self,
        ) -> impl DoubleEndedIterator<Item = TouchedEntry<'_, K, V>> + ExactSizeIterator {
            self.revert.iter().map(|(key, before)| TouchedEntry {
                key,
                before: before.as_ref(),
                after: self.get(key),
            })
        }

        /// Return the actual acquisition mode, including an untouched replacement.
        pub fn mode(&self) -> BlockMode {
            self.mode
        }

        /// Return whether this block has staged any storage mutation.
        pub fn is_dirty(&self) -> bool {
            self.dirty
        }
        /// Get mutable access to the value stored in
        pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
            let dirty = &mut self.dirty;
            let revert = &mut self.revert;
            self.blocks.get_mut(key).inspect(|value| {
                *dirty = true;
                revert
                    .entry(key.clone())
                    .or_insert_with(|| Some((*value).clone()));
            })
        }
        /// Insert key value into the storage
        pub fn insert(&mut self, key: K, value: V) -> Option<V> {
            let prev_value = self.blocks.insert(key.clone(), value);
            self.revert.entry(key).or_insert_with(|| prev_value.clone());
            self.dirty = true;
            prev_value
        }
        /// Remove key value from storage
        pub fn remove(&mut self, key: K) -> Option<V> {
            let prev_value = self.blocks.remove(&key);
            self.revert.entry(key).or_insert_with(|| prev_value.clone());
            if prev_value.is_some() {
                self.dirty = true;
            }
            prev_value
        }
    }
    impl<K: Key, V: Value> StorageReadOnly<K, V> for Block<'_, K, V> {
        fn get<Q>(&self, key: &Q) -> Option<&V>
        where
            K: Borrow<Q>,
            Q: Ord + ?Sized,
        {
            self.blocks.get(key)
        }
        fn iter(&self) -> Iter<'_, K, V> {
            Iter {
                iter: Box::new(self.blocks.iter()),
            }
        }
        fn range<Q>(&self, bounds: impl RangeBounds<Q>) -> RangeIter<'_, K, V>
        where
            K: Borrow<Q>,
            Q: Ord + ?Sized,
        {
            RangeIter {
                iter: Box::new(self.blocks.range(bounds)),
            }
        }
        fn first_key_value(&self) -> Option<(&K, &V)> {
            self.blocks.first_key_value()
        }
        fn last_key_value(&self) -> Option<(&K, &V)> {
            self.blocks.last_key_value()
        }
        fn len(&self) -> usize {
            self.blocks.len()
        }
    }
    /// Part of block's aggregated changes which applied or aborted at the same time
    pub struct Transaction<'block, 'store, K: Key, V: Value> {
        pub(crate) applied: bool,
        pub(crate) dirty_before: bool,
        pub(crate) revert: BTreeMap<K, Option<V>>,
        pub(crate) block: &'block mut Block<'store, K, V>,
    }
    impl<'block, 'store: 'block, K: Key, V: Value> Transaction<'block, 'store, K, V> {
        /// Create read-only view into the current transaction state.
        pub fn view(&self) -> View<'_, K, V> {
            View::from_snapshot(self.block.blocks.to_snapshot())
        }
        /// Read the value that existed before this block's first mutation of `key`.
        ///
        /// An applied earlier transaction contributes to the parent block undo
        /// log. A mutation in this still-open transaction contributes to its
        /// local undo log. Consulting both preserves the exact block-start
        /// value without exposing an aborted candidate write.
        pub fn get_before_block(&self, key: &K) -> Option<&V> {
            if let Some(previous) = self.block.revert_map().get(key) {
                return previous.as_ref();
            }
            match self.revert.get(key) {
                Some(previous) => previous.as_ref(),
                None => self.get(key),
            }
        }
        /// Read the value before this transaction's first mutation of `key`.
        ///
        /// Earlier applied transactions are part of this preimage. Use
        /// [`Self::get_before_block`] for the parent block's original value.
        pub fn get_before_transaction(&self, key: &K) -> Option<&V> {
            match self.revert.get(key) {
                Some(previous) => previous.as_ref(),
                None => self.get(key),
            }
        }

        /// Visit this transaction's touched keys before apply or rollback.
        ///
        /// The borrowed records contain transaction-start and current values,
        /// ordered by `K::Ord`, without cloning or allocating another change
        /// list. Earlier applied siblings are included in `before`; untouched
        /// sibling keys are not returned. No-op touches remain explicit. On
        /// apply, the block journal retains its earlier preimage instead.
        pub fn touched_entries(
            &self,
        ) -> impl DoubleEndedIterator<Item = TouchedEntry<'_, K, V>> + ExactSizeIterator {
            self.revert.iter().map(|(key, before)| TouchedEntry {
                key,
                before: before.as_ref(),
                after: self.get(key),
            })
        }

        /// Apply aggregated changes of [`Transaction`] to the [`Block`]
        pub fn apply(mut self) {
            for (key, value) in core::mem::take(&mut self.revert) {
                self.block.revert.entry(key).or_insert(value);
            }
            self.applied = true;
        }
        /// Get mutable access to the value stored in
        pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
            self.block.blocks.get_mut(key).inspect(|value| {
                self.block.dirty = true;
                self.revert
                    .entry(key.clone())
                    .or_insert_with(|| Some((*value).clone()));
            })
        }
        /// Insert key value into the transaction temporary map
        pub fn insert(&mut self, key: K, value: V) -> Option<V> {
            let prev_value = self.block.blocks.insert(key.clone(), value);
            self.revert.entry(key).or_insert_with(|| prev_value.clone());
            self.block.dirty = true;
            prev_value
        }
        /// Remove key value from storage
        pub fn remove(&mut self, key: K) -> Option<V> {
            let prev_value = self.block.blocks.remove(&key);
            self.revert.entry(key).or_insert_with(|| prev_value.clone());
            if prev_value.is_some() {
                self.block.dirty = true;
            }
            prev_value
        }
    }
    impl<K: Key, V: Value> StorageReadOnly<K, V> for Transaction<'_, '_, K, V> {
        fn get<Q>(&self, key: &Q) -> Option<&V>
        where
            K: Borrow<Q>,
            Q: Ord + ?Sized,
        {
            self.block.get(key)
        }
        fn iter(&self) -> Iter<'_, K, V> {
            self.block.iter()
        }
        fn range<Q>(&self, bounds: impl RangeBounds<Q>) -> RangeIter<'_, K, V>
        where
            K: Borrow<Q>,
            Q: Ord + ?Sized,
        {
            self.block.range(bounds)
        }
        fn first_key_value(&self) -> Option<(&K, &V)> {
            self.block.first_key_value()
        }
        fn last_key_value(&self) -> Option<(&K, &V)> {
            self.block.last_key_value()
        }
        fn len(&self) -> usize {
            self.block.len()
        }
    }
    impl<'block, 'store: 'block, K: Key, V: Value> Drop for Transaction<'block, 'store, K, V> {
        fn drop(&mut self) {
            if self.applied {
                return;
            }
            // revert changes made so far by current transaction
            // if transaction was applied set would be empty
            for (key, value) in core::mem::take(&mut self.revert) {
                match value {
                    None => self.block.blocks.remove(&key),
                    Some(value) => self.block.blocks.insert(key, value),
                };
            }
            self.block.dirty = self.dirty_before;
        }
    }
}
pub use block::{Block, Transaction};
mod iter {
    use super::*;
    /// Iterate over entries in block, view or transaction
    pub struct Iter<'slf, K: Key, V: Value> {
        pub(crate) iter: Box<dyn Iterator<Item = (&'slf K, &'slf V)> + 'slf>,
    }
    /// Iterate over range of entries in block, view or transaction
    pub struct RangeIter<'slf, K: Key, V: Value> {
        pub(crate) iter: Box<dyn DoubleEndedIterator<Item = (&'slf K, &'slf V)> + 'slf>,
    }
    impl<'slf, K: Key, V: Value> Iterator for Iter<'slf, K, V> {
        type Item = (&'slf K, &'slf V);
        fn next(&mut self) -> Option<Self::Item> {
            self.iter.next()
        }
    }
    impl<'slf, K: Key, V: Value> Iterator for RangeIter<'slf, K, V> {
        type Item = (&'slf K, &'slf V);
        fn next(&mut self) -> Option<Self::Item> {
            self.iter.next()
        }
    }
    impl<'slf, K: Key, V: Value> DoubleEndedIterator for RangeIter<'slf, K, V> {
        fn next_back(&mut self) -> Option<Self::Item> {
            self.iter.next_back()
        }
    }
}
pub use iter::{Iter, RangeIter};
#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::BTreeMap, ops::Bound};
    #[test]
    fn get() {
        let storage = Storage::<u64, u64>::new();
        let view0 = storage.view();
        {
            let mut block = storage.block();
            for (key, value) in [(0, 0), (1, 0), (2, 0)] {
                block.insert(key, value);
            }
            block.commit()
        }
        let view1 = storage.view();
        {
            let mut block = storage.block();
            for (key, value) in [(0, 1), (1, 1), (3, 1)] {
                block.insert(key, value);
            }
            block.commit()
        }
        let view2 = storage.view();
        {
            let mut block = storage.block();
            for (key, value) in [(1, 2), (4, 2)] {
                block.insert(key, value);
            }
            block.commit()
        }
        let view3 = storage.view();
        assert_eq!(view0.get(&0), None);
        assert_eq!(view0.get(&1), None);
        assert_eq!(view0.get(&2), None);
        assert_eq!(view0.get(&3), None);
        assert_eq!(view1.get(&0), Some(&0));
        assert_eq!(view1.get(&1), Some(&0));
        assert_eq!(view1.get(&2), Some(&0));
        assert_eq!(view1.get(&3), None);
        assert_eq!(view2.get(&0), Some(&1));
        assert_eq!(view2.get(&1), Some(&1));
        assert_eq!(view2.get(&2), Some(&0));
        assert_eq!(view2.get(&3), Some(&1));
        assert_eq!(view2.get(&4), None);
        assert_eq!(view3.get(&0), Some(&1));
        assert_eq!(view3.get(&1), Some(&2));
        assert_eq!(view3.get(&2), Some(&0));
        assert_eq!(view3.get(&3), Some(&1));
        assert_eq!(view3.get(&4), Some(&2));
    }
    #[test]
    fn endpoint_lookups_follow_canonical_key_order() {
        let storage = Storage::<u64, u64>::from_iter([(7, 70), (2, 20), (11, 110)]);
        let view = storage.view();
        assert_eq!(view.first_key_value(), Some((&2, &20)));
        assert_eq!(view.last_key_value(), Some((&11, &110)));
        let mut block = storage.block();
        assert_eq!(block.first_key_value(), Some((&2, &20)));
        assert_eq!(block.last_key_value(), Some((&11, &110)));
        {
            let mut transaction = block.transaction();
            transaction.insert(1, 10);
            transaction.insert(12, 120);
            assert_eq!(transaction.first_key_value(), Some((&1, &10)));
            assert_eq!(transaction.last_key_value(), Some((&12, &120)));
        }
    }
    #[test]
    fn transaction_step() {
        let storage = Storage::<u64, u64>::new();
        let mut block = storage.block();
        // Successful transaction
        {
            let mut transaction = block.transaction();
            transaction.insert(0, 0);
            transaction.apply();
        }
        // Aborted step
        {
            let mut transaction = block.transaction();
            transaction.insert(0, 1);
            transaction.insert(1, 1);
        }
        // Check that aborted transaction changes don't visible for subsequent transactions
        {
            let transaction = block.transaction();
            assert_eq!(transaction.get(&0), Some(&0));
            assert_eq!(transaction.get(&1), None);
        }
        block.commit();
        // Check that effect of aborted step is not visible in the storage after committing transaction
        {
            let view = storage.view();
            assert_eq!(view.get(&0), Some(&0));
            assert_eq!(view.get(&1), None);
        }
    }
    #[test]
    fn iter() {
        let storage = Storage::<u64, u64>::new();
        {
            let mut block = storage.block();
            for (key, value) in [(0, 0), (1, 0), (2, 0)] {
                block.insert(key, value);
            }
            block.commit()
        }
        {
            let mut block = storage.block();
            for (key, value) in [(0, 1), (1, 1), (3, 1)] {
                block.insert(key, value);
            }
            block.commit()
        }
        {
            let mut block = storage.block();
            for (key, value) in [(1, 2), (4, 2)] {
                block.insert(key, value);
            }
            block.commit()
        }
        let view = storage.view();
        for (kv_actual, kv_expected) in
            view.iter()
                .zip([(&0, &1), (&1, &2), (&2, &0), (&3, &1), (&4, &2)])
        {
            assert_eq!(kv_actual, kv_expected);
        }
        let mut block = storage.block();
        block.insert(0, 3);
        block.insert(5, 3);
        let mut transaction = block.transaction();
        transaction.insert(1, 4);
        transaction.insert(6, 4);
        for (kv_actual, kv_expected) in transaction.iter().zip([
            (&0, &3),
            (&1, &4),
            (&2, &0),
            (&3, &1),
            (&4, &2),
            (&5, &3),
            (&6, &4),
        ]) {
            assert_eq!(kv_actual, kv_expected);
        }
    }
    #[test]
    fn range() {
        let storage = Storage::<u64, u64>::new();
        {
            let mut block = storage.block();
            for (key, value) in [(0, 0), (1, 0), (2, 0)] {
                block.insert(key, value);
            }
            block.commit()
        }
        {
            let mut block = storage.block();
            for (key, value) in [(0, 1), (1, 1), (3, 1)] {
                block.insert(key, value);
            }
            block.commit()
        }
        {
            let mut block = storage.block();
            for (key, value) in [(1, 2), (4, 2)] {
                block.insert(key, value);
            }
            block.commit()
        }
        let view = storage.view();
        for (kv_actual, kv_expected) in view
            .range((Bound::<u64>::Unbounded, Bound::Unbounded))
            .zip([(&0, &1), (&1, &2), (&2, &0), (&3, &1), (&4, &2)])
        {
            assert_eq!(kv_actual, kv_expected);
        }
        for (kv_actual, kv_expected) in view
            .range((Bound::Included(&1), Bound::Included(&3)))
            .zip([(&1, &2), (&2, &0), (&3, &1)])
        {
            assert_eq!(kv_actual, kv_expected);
        }
        for (kv_actual, kv_expected) in view
            .range((Bound::Excluded(&1), Bound::Excluded(&3)))
            .zip([(&2, &0)])
        {
            assert_eq!(kv_actual, kv_expected);
        }
        assert_eq!(view.range(..=3).next_back(), Some((&3, &1)));
        let mut block = storage.block();
        block.insert(0, 3);
        block.insert(5, 3);
        let mut transaction = block.transaction();
        transaction.insert(1, 4);
        transaction.insert(6, 4);
        for (kv_actual, kv_expected) in transaction
            .range((Bound::<u64>::Unbounded, Bound::Unbounded))
            .zip([
                (&0, &3),
                (&1, &4),
                (&2, &0),
                (&3, &1),
                (&4, &2),
                (&5, &3),
                (&6, &4),
            ])
        {
            assert_eq!(kv_actual, kv_expected);
        }
        assert_eq!(transaction.range(..=5).next_back(), Some((&5, &3)));
        for (kv_actual, kv_expected) in transaction
            .range((Bound::Included(&1), Bound::Included(&3)))
            .zip([(&1, &4), (&2, &0), (&3, &1)])
        {
            assert_eq!(kv_actual, kv_expected);
        }
        for (kv_actual, kv_expected) in transaction
            .range((Bound::Excluded(&1), Bound::Excluded(&3)))
            .zip([(&2, &0)])
        {
            assert_eq!(kv_actual, kv_expected);
        }
    }
    #[test]
    fn revert() {
        let storage = Storage::<u64, u64>::new();
        {
            let mut block = storage.block();
            block.insert(0, 0);
            block.commit()
        }
        {
            let mut block = storage.block();
            block.insert(0, 1);
            block.commit()
        }
        let view1 = storage.view();
        {
            let block = storage.block_and_revert();
            block.commit();
        }
        let view2 = storage.view();
        // View is persistent so revert is not visible
        assert_eq!(view1.get(&0), Some(&1));
        // Revert is visible in the view created after revert was applied
        assert_eq!(view2.get(&0), Some(&0));
    }
    #[test]
    fn noop_commit_clears_revert_history() {
        let storage = Storage::<u64, u64>::new();
        {
            let mut block = storage.block();
            block.insert(0, 1);
            block.commit();
        }
        {
            let block = storage.block();
            block.commit();
        }
        {
            let block = storage.block_and_revert();
            block.commit();
        }
        let view = storage.view();
        assert_eq!(view.get(&0), Some(&1));
    }
    #[test]
    fn aborted_transaction_dirty_commit_keeps_state_unchanged() {
        let storage = Storage::<u64, u64>::new();
        {
            let mut block = storage.block();
            {
                let mut transaction = block.transaction();
                transaction.insert(0, 1);
            }
            assert!(!block.dirty);
            block.commit();
        }
        let view = storage.view();
        assert_eq!(view.get(&0), None);
    }
    #[test]
    fn aborted_transaction_preserves_existing_dirty_state() {
        let storage = Storage::<u64, u64>::new();
        {
            let mut block = storage.block();
            block.insert(0, 1);
            {
                let mut transaction = block.transaction();
                transaction.insert(1, 2);
            }
            assert!(block.dirty);
            block.commit();
        }
        let view = storage.view();
        assert_eq!(view.get(&0), Some(&1));
        assert_eq!(view.get(&1), None);
    }
    #[test]
    fn remove_missing_key_is_noop_commit() {
        let storage = Storage::<u64, u64>::new();
        {
            let mut block = storage.block();
            block.insert(0, 1);
            block.commit();
        }
        {
            let mut block = storage.block();
            assert_eq!(block.remove(1), None);
            assert!(!block.dirty);
            block.commit();
        }
        {
            let block = storage.block_and_revert();
            block.commit();
        }
        let view = storage.view();
        assert_eq!(view.get(&0), Some(&1));
        assert_eq!(view.get(&1), None);
    }
    #[test]
    fn transaction_remove_missing_key_keeps_block_clean() {
        let storage = Storage::<u64, u64>::new();
        {
            let mut block = storage.block();
            {
                let mut transaction = block.transaction();
                assert_eq!(transaction.remove(0), None);
                transaction.apply();
            }
            assert!(!block.dirty);
            block.commit();
        }
        let view = storage.view();
        assert_eq!(view.get(&0), None);
    }
    #[test]
    fn len() {
        let storage = Storage::<u64, u64>::new();
        // Newly created storage should have no entries
        assert!(storage.view().is_empty());
        {
            let mut block = storage.block();
            for (key, value) in [(0, 0), (1, 0), (2, 0)] {
                block.insert(key, value);
            }
            block.commit()
        }
        {
            let mut block = storage.block();
            for (key, value) in [(0, 1), (1, 1), (3, 1)] {
                block.insert(key, value);
            }
            block.commit()
        }
        {
            let mut block = storage.block();
            for (key, value) in [(1, 2), (4, 2)] {
                block.insert(key, value);
            }
            block.commit()
        }
        let view = storage.view();
        assert_eq!(view.len(), 5);
        assert!(!view.is_empty());
    }
    #[test]
    fn consistent_with_btreemap() {
        let storage = Storage::<u64, u64>::new();
        let mut map = BTreeMap::new();
        let txs = vec![
            (true, vec![(0, Some(10)), (1, Some(20)), (0, None)]),
            (true, vec![(2, Some(30))]),
            (false, vec![(1, Some(40))]),
        ];
        for (committed, tx) in txs {
            let mut block = storage.block();
            for (key, value) in tx {
                match value {
                    Some(v) => {
                        if committed {
                            map.insert(key, v);
                        }
                        block.insert(key, v);
                    }
                    None => {
                        if committed {
                            map.remove(&key);
                        }
                        block.remove(key);
                    }
                }
            }
            if committed {
                block.commit();
            }
        }
        let view = storage.view();
        for (k, v) in map.iter() {
            assert_eq!(view.get(k), Some(v));
        }
    }
    #[test]
    fn revert_map_tracks_changed_keys() {
        let storage = Storage::<u64, u64>::new();
        let mut block = storage.block();
        block.insert(1, 10);
        block.insert(2, 20);
        block.remove(1);
        let revert = block.revert_map();
        assert!(revert.contains_key(&1));
        assert!(revert.contains_key(&2));
    }
    #[test]
    fn get_before_block_tracks_first_value_across_direct_mutations() {
        let storage = Storage::from_iter([(1_u64, 10_u64), (2, 20), (4, 40)]);
        let mut block = storage.block();

        assert_eq!(block.get_before_block(&1), Some(&10), "untouched value");
        assert_eq!(block.get_before_block(&3), None, "untouched absence");

        block.insert(1, 11);
        block.insert(1, 12);
        assert_eq!(block.get(&1), Some(&12));
        assert_eq!(block.get_before_block(&1), Some(&10));

        block.insert(3, 30);
        block.insert(3, 31);
        assert_eq!(block.get(&3), Some(&31));
        assert_eq!(block.get_before_block(&3), None);

        assert_eq!(block.remove(2), Some(20));
        assert_eq!(block.get(&2), None);
        assert_eq!(block.get_before_block(&2), Some(&20));

        *block.get_mut(&4).expect("existing fixture value") += 1;
        assert_eq!(block.get_before_block(&4), Some(&40));
    }
    #[test]
    fn get_before_block_observes_only_applied_transaction_mutations() {
        let storage = Storage::from_iter([(1_u64, 10_u64)]);
        let mut block = storage.block();

        {
            let mut transaction = block.transaction();
            transaction.insert(1, 11);
            transaction.insert(2, 20);
        }
        assert_eq!(block.get(&1), Some(&10));
        assert_eq!(block.get(&2), None);
        assert_eq!(block.get_before_block(&1), Some(&10));
        assert_eq!(block.get_before_block(&2), None);

        {
            let mut transaction = block.transaction();
            transaction.insert(1, 12);
            transaction.insert(2, 21);
            transaction.apply();
        }
        assert_eq!(block.get(&1), Some(&12));
        assert_eq!(block.get(&2), Some(&21));
        assert_eq!(block.get_before_block(&1), Some(&10));
        assert_eq!(block.get_before_block(&2), None);

        {
            let mut transaction = block.transaction();
            transaction.insert(1, 13);
            transaction.apply();
        }
        assert_eq!(block.get_before_block(&1), Some(&10));
    }
    #[test]
    fn transaction_get_before_block_includes_its_local_undo_log() {
        let storage = Storage::from_iter([(1_u64, 10_u64), (3, 30)]);
        let mut block = storage.block();
        block.insert(1, 11);

        {
            let mut transaction = block.transaction();
            transaction.insert(1, 12);
            transaction.insert(2, 20);
            transaction.remove(3);
            assert_eq!(transaction.get_before_block(&1), Some(&10));
            assert_eq!(transaction.get_before_block(&2), None);
            assert_eq!(transaction.get_before_block(&3), Some(&30));
        }

        assert_eq!(block.get(&1), Some(&11));
        assert_eq!(block.get(&2), None);
        assert_eq!(block.get(&3), Some(&30));
        assert_eq!(block.get_before_block(&1), Some(&10));
    }
    #[test]
    fn get_before_block_uses_reverted_state_as_new_block_baseline() {
        let storage = Storage::from_iter([(1_u64, 10_u64)]);
        {
            let mut block = storage.block();
            block.insert(1, 11);
            block.insert(2, 20);
            block.commit();
        }

        let mut reverted = storage.block_and_revert();
        assert_eq!(reverted.get(&1), Some(&10));
        assert_eq!(reverted.get(&2), None);
        assert_eq!(reverted.get_before_block(&1), Some(&10));
        assert_eq!(reverted.get_before_block(&2), None);

        reverted.insert(1, 12);
        reverted.insert(2, 21);
        assert_eq!(reverted.get_before_block(&1), Some(&10));
        assert_eq!(reverted.get_before_block(&2), None);
    }
}

#[path = "storage/history.rs"]
mod history;
pub use history::History;
mod snapshot;
pub use snapshot::Snapshot;

#[cfg(test)]
#[path = "storage/overlay_preimage_tests.rs"]
mod overlay_preimage_tests;

#[cfg(test)]
#[path = "storage/detached_tests.rs"]
mod detached_tests;
