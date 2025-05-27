//! Multi-version append-only key value storage for transactions

#![allow(clippy::disallowed_types)]

use std::{borrow::Borrow, collections::HashSet, hash::Hash, num::NonZeroUsize, sync::Arc};

use arc_swap::ArcSwapOption;
use dashmap::DashMap;
use iroha_crypto::HashOf;
use iroha_data_model::prelude::SignedTransaction;
use parking_lot::{lock_api::MutexGuard, Mutex, RawMutex};
use serde::{Deserialize, Serialize};

type Key = HashOf<SignedTransaction>;
type Value = NonZeroUsize;

/// Multi-version append-only key value storage for transactions
/// This is analogue of [`mv::storage::Storage`] or `HashMap<Key, Value>`.
/// Contains hashes of transactions mapped onto block height where they are stored.
///
/// * Q: Why don't we use `HashMap`/`BTreeMap`?
///   A: Because we need multi-version storage with transactional behaviour
///   (`.view()`, `.block()`, `.block_and_revert()`, `.commit()`)
/// * Q: Why don't we use [`mv::storage::Storage`]?
///   A: Because transactions map consumes 80%+ RAM of iroha.
///   This storage is memory-optimized and consumes ~3x less memory.
pub struct TransactionsStorage {
    /// Latest block. Stored separately because of reverts.
    /// `None` when there are no blocks yet, otherwise must be not `None`.
    latest_block: ArcSwapOption<BlockInfo>,
    /// Map with aggregated transactions of multiple blocks, EXCEPT for the latest block.
    blocks: DashMap<Key, Value>,
    write_lock: Mutex<()>,
}

#[derive(Serialize, Deserialize)]
struct BlockInfo {
    /// Transactions added in the block
    transactions: HashSet<Key>,
    /// Height of the block.
    height: NonZeroUsize,
}

impl TransactionsStorage {
    /// Construct new [`Self`]
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            latest_block: ArcSwapOption::empty(),
            blocks: DashMap::new(),
            write_lock: Mutex::new(()),
        }
    }

    /// Create persistent view of storage at certain point in time
    pub fn view(&self) -> TransactionsView {
        TransactionsView {
            latest_block: self.latest_block.load_full(),
            blocks: &self.blocks,
        }
    }

    /// Create block to aggregate updates
    pub fn block(&self) -> TransactionsBlock {
        self.block_impl(false)
    }

    /// Create block to aggregate updates and revert changes created in the latest block
    pub fn block_and_revert(&self) -> TransactionsBlock {
        self.block_impl(true)
    }

    fn block_impl(&self, revert: bool) -> TransactionsBlock {
        let guard = self.write_lock.lock();
        TransactionsBlock {
            latest_block_ref: &self.latest_block,
            blocks_ref: &self.blocks,
            _guard: guard,
            revert,
            current_block: None,
        }
    }
}

/// Persistent view of storage at certain point in time
pub trait TransactionsReadOnly {
    /// Read entry from the storage
    fn get<Q>(&self, key: &Q) -> Option<Value>
    where
        Key: Borrow<Q>,
        Q: Hash + Eq + ?Sized;
}

/// Module for [`TransactionsView`] and it's related impls
mod view {
    use super::*;

    /// Consistent view of the storage at the certain version
    #[derive(Serialize)]
    pub struct TransactionsView<'storage> {
        pub(super) latest_block: Option<Arc<BlockInfo>>,
        /// Some transactions may be added to the map after `Self` is created,
        /// but for us exists only transactions with `height < latest_block.height`
        pub(super) blocks: &'storage DashMap<Key, Value>,
    }

    impl TransactionsReadOnly for TransactionsView<'_> {
        fn get<Q>(&self, key: &Q) -> Option<Value>
        where
            Key: Borrow<Q>,
            Q: Hash + Eq + ?Sized,
        {
            let Some(block) = &self.latest_block else {
                return None;
            };
            let block_height = block.height;
            if block.transactions.contains(key) {
                return Some(block_height);
            }

            self.blocks
                .get(key)
                .map(|h| *h)
                .filter(|&h| h < block_height)
        }
    }
}
pub use view::TransactionsView;

/// Module for [`TransactionsBlock`] and it's related impls
mod block {
    use std::ops::Deref;

    use super::*;

    /// Batched update to the storage that can be reverted later
    pub struct TransactionsBlock<'storage> {
        /// References to [`TransactionsStorage`] struct
        pub(super) latest_block_ref: &'storage ArcSwapOption<BlockInfo>,
        pub(super) blocks_ref: &'storage DashMap<Key, Value>,
        pub(super) _guard: MutexGuard<'storage, RawMutex, ()>,

        /// Own fields
        pub(super) revert: bool,
        pub(super) current_block: Option<Arc<BlockInfo>>,
    }

    impl TransactionsBlock<'_> {
        /// Add transactions of the block
        pub fn insert_block(&mut self, transactions: HashSet<Key>, height: Value) {
            let block_info = BlockInfo {
                transactions,
                height,
            };
            assert!(
                self.current_block.is_none(),
                "`TransactionsBlock::insert_block()` must be called only once"
            );
            self.current_block = Some(Arc::new(block_info));
        }

        #[cfg(test)]
        pub fn insert_block_with_single_tx(&mut self, tx: Key, height: Value) {
            let transactions = [tx].into_iter().collect();
            self.insert_block(transactions, height);
        }

        /// Apply aggregated changes to the storage
        pub fn commit(self) {
            let previous_block = &self.latest_block_ref.load();
            let previous_block = previous_block.as_ref();

            let previous_height = previous_block.map_or(0, |b| b.height.get());
            let addition = usize::from(!self.revert);
            let expected_current_height = previous_height + addition;

            #[allow(clippy::option_if_let_else)]
            let current_block = match self.current_block {
                Some(current_block) => {
                    let current_height = current_block.height.get();
                    debug_assert_eq!(expected_current_height, current_height);
                    current_block
                }
                None => {
                    #[cfg(not(debug_assertions))]
                    panic!("`TransactionsBlock::insert_block()` was not called");

                    // Note that in production `current_block` must be not `None`.
                    // We support `None` case here for tests,
                    // in which `.block()` + `.commit()` is used to populate store,
                    // without actually adding block with transactions.
                    #[cfg(debug_assertions)]
                    Arc::new(BlockInfo {
                        transactions: HashSet::new(),
                        height: NonZeroUsize::new(expected_current_height).unwrap(),
                    })
                }
            };

            if !self.revert {
                if let Some(previous_block) = previous_block {
                    for &transaction in &previous_block.transactions {
                        self.blocks_ref.insert(transaction, previous_block.height);
                    }
                }
            }

            self.latest_block_ref.store(Some(current_block));
        }
    }

    impl TransactionsReadOnly for TransactionsBlock<'_> {
        fn get<Q>(&self, key: &Q) -> Option<Value>
        where
            Key: Borrow<Q>,
            Q: Hash + Eq + ?Sized,
        {
            if let Some(current_block) = &self.current_block {
                if current_block.transactions.contains(key) {
                    return Some(current_block.height);
                }
            }

            #[allow(clippy::explicit_deref_methods)]
            if let Some(latest_block) = self.latest_block_ref.load().deref() {
                if latest_block.transactions.contains(key) {
                    return Some(latest_block.height);
                }
            }

            self.blocks_ref.get(key).map(|h| *h)
        }
    }
}
pub use block::TransactionsBlock;

/// Module with serialization and deserialization of [`TransactionsStorage`]
mod serialization {
    use super::*;

    impl Serialize for TransactionsStorage {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            let view = self.view();

            // Note that some new entries may be added to `blocks` during serialization.
            // We will filter such entries later during deserialization.
            // Potential alternative implementations:
            // * Clone `blocks`, filter, then serialize
            //   Bad approach, will have 2x peak memory usage
            // * Write custom serialization with filtration
            //   (using `serializer.serialize_map()`)
            view.serialize(serializer)
        }
    }

    impl<'de> Deserialize<'de> for TransactionsStorage {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            #[derive(Deserialize)]
            struct Storage {
                latest_block: Option<Arc<BlockInfo>>,
                blocks: DashMap<Key, Value>,
            }

            let storage = Storage::deserialize(deserializer)?;
            // See `serialize` implementation why filter is needed here
            if let Some(latest_block) = &storage.latest_block {
                let latest_block_height = latest_block.height;
                storage
                    .blocks
                    .retain(|_, &mut height| height < latest_block_height);
            } else {
                storage.blocks.clear();
            }

            Ok(TransactionsStorage {
                latest_block: ArcSwapOption::from(storage.latest_block),
                blocks: storage.blocks,
                write_lock: Mutex::new(()),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn random_hash() -> Key {
        use rand::Rng;
        let bytes = rand::thread_rng().gen();
        let hash = iroha_crypto::Hash::prehashed(bytes);
        HashOf::from_untyped_unchecked(hash)
    }

    fn get_keys<const N: usize>() -> [Key; N] {
        [(); N].map(|()| random_hash())
    }

    fn get_values<const N: usize>() -> [Value; N] {
        let mut i = 0;
        [(); N].map(|()| {
            i += 1;
            NonZeroUsize::new(i).unwrap()
        })
    }

    fn insert_keys(block: &mut TransactionsBlock, keys: &[Key], value: Value) {
        let keys = keys.iter().copied().collect();
        block.insert_block(keys, value);
    }

    #[test]
    fn get() {
        let [k0, k1, k2, k3, k4] = get_keys();
        let [v1, v2, v3] = get_values();

        let storage = TransactionsStorage::new();
        let view0 = storage.view();

        {
            let mut block = storage.block();
            insert_keys(&mut block, &[k0, k1, k2], v1);
            block.commit()
        }
        let view1 = storage.view();

        {
            let mut block = storage.block();
            insert_keys(&mut block, &[k0, k1, k3], v2);
            block.commit()
        }
        let view2 = storage.view();

        {
            let mut block = storage.block();
            insert_keys(&mut block, &[k1, k4], v3);
            block.commit()
        }
        let view3 = storage.view();

        assert_eq!(view0.get(&k0), None);
        assert_eq!(view0.get(&k1), None);
        assert_eq!(view0.get(&k2), None);
        assert_eq!(view0.get(&k3), None);

        assert_eq!(view1.get(&k0), Some(v1));
        assert_eq!(view1.get(&k1), Some(v1));
        assert_eq!(view1.get(&k2), Some(v1));
        assert_eq!(view1.get(&k3), None);

        assert_eq!(view2.get(&k0), Some(v2));
        assert_eq!(view2.get(&k1), Some(v2));
        assert_eq!(view2.get(&k2), Some(v1));
        assert_eq!(view2.get(&k3), Some(v2));
        assert_eq!(view2.get(&k4), None);

        assert_eq!(view3.get(&k0), Some(v2));
        assert_eq!(view3.get(&k1), Some(v3));
        assert_eq!(view3.get(&k2), Some(v1));
        assert_eq!(view3.get(&k3), Some(v2));
        assert_eq!(view3.get(&k4), Some(v3));
    }

    #[test]
    fn revert() {
        let [k0] = get_keys();
        let [v1, v2] = get_values();

        let storage = TransactionsStorage::new();

        {
            let mut block = storage.block();
            insert_keys(&mut block, &[k0], v1);
            block.commit()
        }

        {
            let mut block = storage.block();
            insert_keys(&mut block, &[k0], v2);
            block.commit()
        }
        let view1 = storage.view();

        {
            let block = storage.block_and_revert();
            block.commit();
        }
        let view2 = storage.view();

        // View is persistent so revert is not visible
        assert_eq!(view1.get(&k0), Some(v2));
        // Revert is visible in the view created after revert was applied
        assert_eq!(view2.get(&k0), Some(v1));
    }

    #[test]
    fn serialization() {
        fn assert_views_equal(view1: &TransactionsView, view2: &TransactionsView, keys: &[Key]) {
            for key in keys {
                let value1 = view1.get(key);
                let value2 = view2.get(key);
                assert_eq!(value1, value2);
            }
        }
        fn check_view(view1: &TransactionsView, keys: &[Key]) {
            let json = serde_json::to_string(&view1).unwrap();
            let storage2: TransactionsStorage = serde_json::from_str(&json).unwrap();
            let view2 = storage2.view();
            assert_views_equal(view1, &view2, keys);
        }
        fn check_views(views: &[TransactionsView], keys: &[Key]) {
            for view in views {
                check_view(view, keys);
            }
        }

        let keys = get_keys();
        let [k0, k1, k2, k3, k4] = keys;
        let [v1, v2, v3] = get_values();

        let mut views = Vec::new();

        let storage = TransactionsStorage::new();
        views.push(storage.view());
        check_views(&views, &keys);

        {
            let mut block = storage.block();
            insert_keys(&mut block, &[k0, k1, k2], v1);
            block.commit()
        }
        views.push(storage.view());
        check_views(&views, &keys);

        {
            let mut block = storage.block();
            insert_keys(&mut block, &[k0, k1, k3], v2);
            block.commit()
        }
        views.push(storage.view());
        check_views(&views, &keys);

        {
            let mut block = storage.block();
            insert_keys(&mut block, &[k1, k4], v3);
            block.commit()
        }
        views.push(storage.view());
        check_views(&views, &keys);

        {
            let mut block = storage.block_and_revert();
            insert_keys(&mut block, &[k2], v3);
            block.commit()
        }
        views.push(storage.view());
        check_views(&views, &keys);
    }
}
