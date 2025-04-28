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
///      This storage is memory-optimized and consumes ~3x less memory.
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
                "Should add block transactions only once"
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

            // Check previous and current block height consistency
            //
            // Note that in production `current_block` must be not `None`.
            // We support `None` case here for tests,
            // in which `.block()` + `.commit()` is used to populate store,
            // without actually adding block with transactions.
            #[cfg(debug_assertions)]
            if let Some(current_block) = &self.current_block {
                let current_height = current_block.height.get();
                let previous_height = previous_block.map_or(0, |b| b.height.get());
                let addition = usize::from(!self.revert);
                debug_assert_eq!(previous_height + addition, current_height);
            }

            if !self.revert {
                if let Some(previous_block) = previous_block {
                    for &transaction in &previous_block.transactions {
                        self.blocks_ref.insert(transaction, previous_block.height);
                    }
                }
            }

            self.latest_block_ref.store(self.current_block);
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
            #[derive(Serialize)]
            struct Storage<'a> {
                latest_block: Option<Arc<BlockInfo>>,
                blocks: &'a DashMap<Key, Value>,
            }

            // Note that some new entries may be added to `blocks` during serialization.
            // We will filter such entries later during deserialization.
            // Potential alternative implementations:
            // * Clone `blocks`, filter, then serialize
            //   Bad approach, will have 2x peak memory usage
            // * Write custom serialization with filtration
            //   (using `serializer.serialize_map()`
            let storage = Storage {
                latest_block: self.latest_block.load_full(),
                blocks: &self.blocks,
            };
            storage.serialize(serializer)
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
