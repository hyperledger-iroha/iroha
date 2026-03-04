//! Multi-version append-only key value storage for transactions

#![allow(clippy::disallowed_types)]

use std::{
    borrow::Borrow,
    collections::{BTreeMap, HashSet},
    hash::Hash,
    num::NonZeroUsize,
    sync::Arc,
};

use arc_swap::ArcSwapOption;
use dashmap::DashMap;
use iroha_crypto::HashOf;
use iroha_data_model::prelude::SignedTransaction;
use mv::json::JsonKeyCodec;
use norito::json::{
    self, FastJsonWrite, JsonDeserialize as JsonDeserializeTrait,
    JsonSerialize as JsonSerializeTrait,
};
use parking_lot::{Mutex, RawMutex, lock_api::MutexGuard};

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
    /// Entries are retained only for finalised blocks (with heights strictly
    /// lower than the current latest block) so that stale transactions are
    /// discarded after rollbacks.
    blocks: DashMap<Key, Value>,
    write_lock: Mutex<()>,
}

#[derive(crate::json_macros::JsonSerialize, crate::json_macros::JsonDeserialize)]
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
    pub fn view(&self) -> TransactionsView<'_> {
        TransactionsView {
            latest_block: self.latest_block.load_full(),
            blocks: &self.blocks,
        }
    }

    /// Create block to aggregate updates
    pub fn block(&self) -> TransactionsBlock<'_> {
        self.block_impl(false)
    }

    /// Create block to aggregate updates and revert changes created in the latest block
    pub fn block_and_revert(&self) -> TransactionsBlock<'_> {
        self.block_impl(true)
    }

    fn block_impl(&self, revert: bool) -> TransactionsBlock<'_> {
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
    #[derive(Clone)]
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

    #[cfg(any(test, feature = "iroha-core-tests"))]
    impl TransactionsView<'_> {
        /// Test helper: return the latest committed block height (0 when empty).
        pub fn latest_height_for_tests(&self) -> usize {
            self.latest_block
                .as_ref()
                .map_or(0, |block| block.height.get())
        }
    }
}
pub use view::TransactionsView;

/// Module for [`TransactionsBlock`] and it's related impls
mod block {
    use super::*;

    /// Batched update to the storage that can be reverted later.
    ///
    /// The block aggregates transaction hashes for a particular block height.
    /// Call [`insert_block`] exactly once to register the transactions and
    /// height. After all updates are collected, [`commit`](Self::commit) can be
    /// used to persist them in [`TransactionsStorage`].
    ///
    /// Committing a block without first calling [`insert_block`] is considered
    /// an error and will cause [`commit`](Self::commit) to fail. See the
    /// release-mode tests for examples.
    /// Errors that can occur when committing [`TransactionsBlock`]
    #[derive(thiserror::Error, Debug, displaydoc::Display, Clone, Copy, PartialEq, Eq)]
    #[ignore_extra_doc_attributes]
    pub enum TransactionsBlockError {
        /// `TransactionsBlock::insert_block()` was not called
        MissingInsertBlock,
        /// Consensus mode staging invariant violated: `next_mode` and `mode_activation_height` must be set together
        ModeStagingInvariant,
        /// Block height `{actual_current_height}` does not match expected `{expected_current_height}`;
        /// callers should abort the block and retry against the latest state view.
        HeightMismatch {
            /// Height expected by the storage while committing the block.
            expected_current_height: usize,
            /// Height encoded in the block being committed.
            actual_current_height: usize,
        },
        /// Block contained no executed effects; empty blocks cannot be committed
        EmptyBlock,
    }

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
        /// Register transactions belonging to the block.
        ///
        /// This method **must** be called before [`commit`].
        /// Calling it more than once with the same block payload is a no-op,
        /// while changing the height or the transaction set triggers a panic.
        /// Attempting to commit without inserting a block results in an error
        /// (see `commit_without_insert_block_fails`).
        pub fn insert_block(&mut self, transactions: HashSet<Key>, height: Value) {
            if let Some(current_block) = &self.current_block {
                assert_eq!(
                    current_block.height, height,
                    "`TransactionsBlock::insert_block()` called multiple times with different height"
                );
                assert_eq!(
                    current_block.transactions, transactions,
                    "`TransactionsBlock::insert_block()` called multiple times with different transactions"
                );
                return;
            }

            let block_info = BlockInfo {
                transactions,
                height,
            };
            self.current_block = Some(Arc::new(block_info));
        }

        #[cfg(test)]
        pub fn insert_block_with_single_tx(&mut self, tx: Key, height: Value) {
            let transactions = [tx].into_iter().collect();
            self.insert_block(transactions, height);
        }

        /// Apply aggregated changes to the storage.
        ///
        /// # Errors
        /// Returns an error if [`insert_block`] was not called prior to
        /// committing or if the block height being committed does not match
        /// the expected height derived from the current storage state. These
        /// behaviours are illustrated in the release-mode tests.
        pub fn commit(self) -> Result<(), TransactionsBlockError> {
            let previous_block = &self.latest_block_ref.load();
            let previous_block = previous_block.as_ref();

            let previous_height = previous_block.map_or(0, |b| b.height.get());
            let addition = usize::from(!self.revert);
            let expected_current_height = previous_height + addition;

            let Some(current_block) = self.current_block else {
                return Err(TransactionsBlockError::MissingInsertBlock);
            };
            let current_height = current_block.height.get();
            if expected_current_height != current_height {
                return Err(TransactionsBlockError::HeightMismatch {
                    expected_current_height,
                    actual_current_height: current_height,
                });
            }

            if self.revert {
                // Rolling back the latest block must not drop historical entries from
                // earlier heights; we only prune versions that no longer fall strictly
                // below the height being re-executed.
                self.blocks_ref
                    .retain(|_, height| *height < current_block.height);
            } else if let Some(previous_block) = previous_block {
                for &transaction in &previous_block.transactions {
                    self.blocks_ref.insert(transaction, previous_block.height);
                }
            }

            self.latest_block_ref.store(Some(current_block));
            Ok(())
        }
    }

    impl TransactionsReadOnly for TransactionsBlock<'_> {
        fn get<Q>(&self, key: &Q) -> Option<Value>
        where
            Key: Borrow<Q>,
            Q: Hash + Eq + ?Sized,
        {
            if let Some(height) = self
                .current_block
                .as_ref()
                .and_then(|block| block.transactions.contains(key).then_some(block.height))
            {
                return Some(height);
            }

            let latest_block = self.latest_block_ref.load();
            if let Some(block) = latest_block.as_ref().as_ref()
                && block.transactions.contains(key)
            {
                return Some(block.height);
            }

            self.blocks_ref.get(key).map(|height| *height)
        }
    }
}
#[allow(unused_imports)]
pub use block::{TransactionsBlock, TransactionsBlockError};

/// Module with serialization and deserialization of [`TransactionsStorage`]
mod serialization {
    use super::*;

    fn write_transactions_view_json(view: &TransactionsView<'_>, out: &mut String) {
        out.push('{');
        json::write_json_string("latest_block", out);
        out.push(':');
        match view.latest_block.as_ref() {
            Some(block) => JsonSerializeTrait::json_serialize(block.as_ref(), out),
            None => out.push_str("null"),
        }
        out.push(',');
        json::write_json_string("blocks", out);
        out.push(':');
        let mut map = BTreeMap::new();
        #[allow(clippy::explicit_iter_loop)]
        for entry in view.blocks.iter() {
            map.insert(*entry.key(), *entry.value());
        }
        JsonSerializeTrait::json_serialize(&map, out);
        out.push('}');
    }

    impl JsonSerializeTrait for TransactionsStorage {
        fn json_serialize(&self, out: &mut String) {
            write_transactions_view_json(&self.view(), out)
        }
    }

    impl FastJsonWrite for TransactionsView<'_> {
        fn write_json(&self, out: &mut String) {
            write_transactions_view_json(self, out)
        }
    }

    impl JsonDeserializeTrait for TransactionsStorage {
        fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
            let json::Value::Object(mut map) = json::Value::json_deserialize(parser)? else {
                return Err(json::Error::InvalidField {
                    field: "transactions_storage".into(),
                    message: "expected object".into(),
                });
            };

            let latest_block_value = map
                .remove("latest_block")
                .ok_or_else(|| json::Error::missing_field("latest_block"))?;
            let blocks_value = map
                .remove("blocks")
                .ok_or_else(|| json::Error::missing_field("blocks"))?;

            let latest_block = match latest_block_value {
                json::Value::Null => None,
                other => {
                    let block: BlockInfo = json::value::from_value(other)?;
                    Some(Arc::new(block))
                }
            };

            let dash = DashMap::new();
            let json::Value::Object(entries) = blocks_value else {
                return Err(json::Error::InvalidField {
                    field: "blocks".into(),
                    message: "expected object".into(),
                });
            };
            for (key_str, value_value) in entries {
                let key = Key::decode_json_key(&key_str).map_err(|err| {
                    json::Error::Message(format!("invalid transaction hash `{key_str}`: {err}"))
                })?;
                let value: Value = json::value::from_value(value_value)?;
                dash.insert(key, value);
            }

            if let Some(block) = &latest_block {
                let latest_height = block.height;
                dash.retain(|_, height| *height < latest_height);
            } else {
                dash.clear();
            }

            Ok(TransactionsStorage {
                latest_block: ArcSwapOption::from(latest_block),
                blocks: dash,
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
        let mut rng = rand::rng();
        let bytes = rng.random();
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
            block.commit().unwrap()
        }
        let view1 = storage.view();

        {
            let mut block = storage.block();
            insert_keys(&mut block, &[k0, k1, k3], v2);
            block.commit().unwrap()
        }
        let view2 = storage.view();

        {
            let mut block = storage.block();
            insert_keys(&mut block, &[k1, k4], v3);
            block.commit().unwrap()
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
            block.commit().unwrap()
        }

        {
            let mut block = storage.block();
            insert_keys(&mut block, &[k0], v2);
            block.commit().unwrap()
        }
        let view1 = storage.view();

        {
            let mut block = storage.block_and_revert();
            block.insert_block(HashSet::new(), v2);
            block.commit().unwrap();
        }
        let view2 = storage.view();

        // View is persistent so revert is not visible
        assert_eq!(view1.get(&k0), Some(v2));
        // Revert is visible in the view created after revert was applied
        assert_eq!(view2.get(&k0), Some(v1));
    }

    #[test]
    fn revert_drops_discarded_transactions() {
        let [tx_a, tx_b, tx_b_prime] = get_keys();
        let [height_a, height_b] = get_values();

        let storage = TransactionsStorage::new();

        {
            let mut block = storage.block();
            insert_keys(&mut block, &[tx_a], height_a);
            block.commit().unwrap();
        }

        {
            let mut block = storage.block();
            insert_keys(&mut block, &[tx_b], height_b);
            block.commit().unwrap();
        }

        let view_before_revert = storage.view();
        assert_eq!(view_before_revert.get(&tx_b), Some(height_b));

        {
            let mut block = storage.block_and_revert();
            block.insert_block(HashSet::from([tx_b_prime]), height_b);
            block.commit().unwrap();
        }

        let view_after_revert = storage.view();
        assert_eq!(view_after_revert.get(&tx_a), Some(height_a));
        assert_eq!(view_after_revert.get(&tx_b_prime), Some(height_b));
        assert_eq!(view_after_revert.get(&tx_b), None);
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn commit_succeeds_in_release() {
        let [key] = get_keys();
        let [height] = get_values();
        let storage = TransactionsStorage::new();
        let mut block = storage.block();
        insert_keys(&mut block, &[key], height);
        assert!(block.commit().is_ok());
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
            let json = norito::json::to_json(&view1).unwrap();
            let storage2: TransactionsStorage = norito::json::from_str(&json).unwrap();
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
            block.commit().unwrap()
        }
        views.push(storage.view());
        check_views(&views, &keys);

        {
            let mut block = storage.block();
            insert_keys(&mut block, &[k0, k1, k3], v2);
            block.commit().unwrap()
        }
        views.push(storage.view());
        check_views(&views, &keys);

        {
            let mut block = storage.block();
            insert_keys(&mut block, &[k1, k4], v3);
            block.commit().unwrap()
        }
        views.push(storage.view());
        check_views(&views, &keys);

        {
            let mut block = storage.block_and_revert();
            insert_keys(&mut block, &[k2], v3);
            block.commit().unwrap()
        }
        views.push(storage.view());
        check_views(&views, &keys);
    }

    #[test]
    fn commit_without_insert_block_fails() {
        let storage = TransactionsStorage::new();
        let block = storage.block();
        assert_eq!(
            block.commit(),
            Err(TransactionsBlockError::MissingInsertBlock)
        );
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn commit_height_mismatch_fails_in_release() {
        let [first_key, second_key] = get_keys();
        let [first_height, _] = get_values();
        let storage = TransactionsStorage::new();

        {
            let mut block = storage.block();
            insert_keys(&mut block, &[first_key], first_height);
            block.commit().unwrap();
        }

        let mut block = storage.block();
        let transactions = HashSet::from([second_key]);
        let wrong_height = NonZeroUsize::new(first_height.get() + 2).unwrap();
        let expected_height = first_height.get() + 1;
        block.insert_block(transactions, wrong_height);
        assert_eq!(
            block.commit(),
            Err(TransactionsBlockError::HeightMismatch {
                expected_current_height: expected_height,
                actual_current_height: wrong_height.get(),
            })
        );
    }

    #[test]
    fn commit_with_insert_block_succeeds() {
        let [key] = get_keys();
        let [value] = get_values();
        let storage = TransactionsStorage::new();
        let mut block = storage.block();
        insert_keys(&mut block, &[key], value);
        block.commit().unwrap();
    }

    #[test]
    fn insert_block_allowed_twice_for_same_payload() {
        let [key] = get_keys();
        let [value] = get_values();
        let storage = TransactionsStorage::new();
        let mut block = storage.block();
        let payload: HashSet<_> = HashSet::from([key]);

        block.insert_block(payload.clone(), value);
        block.insert_block(payload, value);

        block.commit().unwrap();
    }

    #[test]
    #[should_panic(
        expected = "`TransactionsBlock::insert_block()` called multiple times with different height"
    )]
    fn insert_block_rejects_height_mismatch() {
        let [key] = get_keys();
        let [value1, value2] = get_values();
        let storage = TransactionsStorage::new();
        let mut block = storage.block();
        let payload = HashSet::from([key]);

        block.insert_block(payload.clone(), value1);
        block.insert_block(payload, value2);
    }

    #[test]
    #[should_panic(
        expected = "`TransactionsBlock::insert_block()` called multiple times with different transactions"
    )]
    fn insert_block_rejects_transaction_mismatch() {
        let [key1, key2] = get_keys();
        let [value] = get_values();
        let storage = TransactionsStorage::new();
        let mut block = storage.block();

        block.insert_block(HashSet::from([key1]), value);
        block.insert_block(HashSet::from([key2]), value);
    }
}
