use std::{borrow::Borrow, collections::BTreeMap, ops::RangeBounds};

use concread::{
    bptree::{BptreeMap, BptreeMapReadSnapshot, BptreeMapReadTxn, BptreeMapWriteTxn},
    ebrcell::{EbrCell, EbrCellWriteTxn},
};

use crate::{Key, Value};

/// Multi-version key value storage
pub struct Storage<K: Key, V: Value> {
    /// Previous version of values in the `blocks` map, required to perform revert of the latest changes
    pub(crate) revert: EbrCell<BTreeMap<K, Option<V>>>,
    /// Map which represent aggregated changes of multiple blocks
    pub(crate) blocks: BptreeMap<K, V>,
}

impl<K: Key, V: Value> Storage<K, V> {
    /// Construct new [`Self`]
    pub fn new() -> Self {
        Self {
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
        let mut revert = self.revert.write();
        let blocks = self.blocks.write();

        // Clear revert
        revert.get_mut().clear();

        Block { revert, blocks }
    }

    /// Insert a value directly into the latest committed state.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        let mut blocks = self.blocks.write();
        let prev_value = blocks.insert(key, value);
        blocks.commit();
        prev_value
    }

    /// Create block to aggregate updates and revert changes created in the latest block
    pub fn block_and_revert(&self) -> Block<'_, K, V> {
        let mut revert = self.revert.write();
        let mut blocks = self.blocks.write();

        {
            let revert = core::mem::take(revert.get_mut());
            for (key, value) in revert {
                match value {
                    None => blocks.remove(&key),
                    Some(value) => blocks.insert(key, value),
                };
            }
        }

        Block { revert, blocks }
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

/// Module for [`Block`] and it's related impls
mod block {
    use super::*;

    /// Batched update to the storage that can be reverted later
    pub struct Block<'store, K: Key, V: Value> {
        pub(crate) revert: EbrCellWriteTxn<'store, BTreeMap<K, Option<V>>>,
        pub(crate) blocks: BptreeMapWriteTxn<'store, K, V>,
    }

    impl<'store, K: Key, V: Value> Block<'store, K, V> {
        /// Create transaction for the block
        pub fn transaction<'block>(&'block mut self) -> Transaction<'block, 'store, K, V>
        where
            'store: 'block,
        {
            Transaction {
                block: self,
                revert: BTreeMap::new(),
            }
        }

        /// Apply aggregated changes to the storage
        pub fn commit(self) {
            // Commit fields in the inverse order
            self.blocks.commit();
            self.revert.commit();
        }

        /// Read-only access to the block revert map (keys touched in this block).
        pub fn revert_map(&self) -> &BTreeMap<K, Option<V>> {
            &self.revert
        }

        /// Get mutable access to the value stored in
        pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
            self.blocks.get_mut(key).inspect(|value| {
                self.revert
                    .entry(key.clone())
                    .or_insert_with(|| Some((*value).clone()));
            })
        }

        /// Insert key value into the storage
        pub fn insert(&mut self, key: K, value: V) -> Option<V> {
            let prev_value = self.blocks.insert(key.clone(), value);
            self.revert.entry(key).or_insert_with(|| prev_value.clone());
            prev_value
        }

        /// Remove key value from storage
        pub fn remove(&mut self, key: K) -> Option<V> {
            let prev_value = self.blocks.remove(&key);
            self.revert.entry(key).or_insert_with(|| prev_value.clone());
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

        fn len(&self) -> usize {
            self.blocks.len()
        }
    }

    /// Part of block's aggregated changes which applied or aborted at the same time
    pub struct Transaction<'block, 'store, K: Key, V: Value> {
        pub(crate) revert: BTreeMap<K, Option<V>>,
        pub(crate) block: &'block mut Block<'store, K, V>,
    }

    impl<'block, 'store: 'block, K: Key, V: Value> Transaction<'block, 'store, K, V> {
        /// Create read-only view into the current transaction state.
        pub fn view(&self) -> View<'_, K, V> {
            View::from_snapshot(self.block.blocks.to_snapshot())
        }

        /// Apply aggregated changes of [`Transaction`] to the [`Block`]
        pub fn apply(mut self) {
            for (key, value) in core::mem::take(&mut self.revert) {
                self.block.revert.entry(key).or_insert(value);
            }
        }

        /// Get mutable access to the value stored in
        pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
            self.block.blocks.get_mut(key).inspect(|value| {
                self.revert
                    .entry(key.clone())
                    .or_insert_with(|| Some((*value).clone()));
            })
        }

        /// Insert key value into the transaction temporary map
        pub fn insert(&mut self, key: K, value: V) -> Option<V> {
            let prev_value = self.block.blocks.insert(key.clone(), value);
            self.revert.entry(key).or_insert_with(|| prev_value.clone());
            prev_value
        }

        /// Remove key value from storage
        pub fn remove(&mut self, key: K) -> Option<V> {
            let prev_value = self.block.blocks.remove(&key);
            self.revert.entry(key).or_insert_with(|| prev_value.clone());
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

        fn len(&self) -> usize {
            self.block.len()
        }
    }

    impl<'block, 'store: 'block, K: Key, V: Value> Drop for Transaction<'block, 'store, K, V> {
        fn drop(&mut self) {
            // revert changes made so far by current transaction
            // if transaction was applied set would be empty
            for (key, value) in core::mem::take(&mut self.revert) {
                match value {
                    None => self.block.blocks.remove(&key),
                    Some(value) => self.block.blocks.insert(key, value),
                };
            }
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
        pub(crate) iter: Box<dyn Iterator<Item = (&'slf K, &'slf V)> + 'slf>,
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
}
pub use iter::{Iter, RangeIter};

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, ops::Bound};

    use super::*;

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
}
