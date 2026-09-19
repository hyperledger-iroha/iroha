//! Multi-version append-only storage for canonical carrier and replay identities.
#![allow(clippy::disallowed_types)]
use arc_swap::ArcSwapOption;
use dashmap::DashMap;
use iroha_crypto::HashOf;
use iroha_data_model::prelude::TransactionEntrypoint;
use mv::json::JsonKeyCodec;
use norito::json::{
    self, FastJsonWrite, JsonDeserialize as JsonDeserializeTrait,
    JsonSerialize as JsonSerializeTrait,
};
use parking_lot::{Mutex, RawMutex, lock_api::MutexGuard};
use std::{
    borrow::Borrow,
    collections::{BTreeMap, HashSet},
    hash::Hash,
    num::NonZeroUsize,
    sync::Arc,
};
type Key = HashOf<TransactionEntrypoint>;
type Value = NonZeroUsize;
/// Multi-version append-only key value storage for transaction replay identities.
/// This is analogue of [`mv::storage::Storage`] or `HashMap<Key, Value>`.
/// Contains canonical carrier hashes plus distinct sealed-reveal signed-execution
/// aliases mapped onto the block height where they are stored.
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
    /// Map with aggregated entrypoints of multiple blocks, EXCEPT for the latest block. Entries
    /// are retained only for finalised blocks (with heights strictly lower than the current latest
    /// block) so that stale transactions are discarded after rollbacks.
    blocks: DashMap<Key, Value>,
    // The opaque identity covers both the hot tip and the historical map.
    // It rotates while the writer is held, never from a caller-provided scalar.
    write_lock: Mutex<Arc<()>>,
    released: mv::ReleaseNotification,
}
#[derive(Clone, crate::json_macros::JsonSerialize, crate::json_macros::JsonDeserialize)]
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
            write_lock: Mutex::new(Arc::new(())),
            released: mv::ReleaseNotification::default(),
        }
    }
    /// Create persistent view of storage at certain point in time
    pub fn view(&self) -> TransactionsView<'_> {
        TransactionsView {
            latest_block: self.latest_block.load_full(),
            blocks: &self.blocks,
        }
    }
    /// Return the latest committed block height recorded by entrypoint storage.
    pub(crate) fn latest_height(&self) -> usize {
        self.latest_block
            .load()
            .as_ref()
            .map_or(0, |block| block.height.get())
    }
    /// Seed canonical entrypoint membership without constructing fixture blocks.
    #[cfg(test)]
    pub(crate) fn record_committed_entrypoint_membership_for_tests(
        &self,
        entrypoints: impl IntoIterator<Item = HashOf<TransactionEntrypoint>>,
        height: NonZeroUsize,
    ) {
        let mut guard = self.released.guard(self.write_lock.lock());
        let next_identity = Arc::new(());
        let entrypoints = entrypoints.into_iter().collect::<HashSet<_>>();
        let latest = self.latest_block.load_full();
        match latest.as_deref() {
            Some(block) if block.height > height => {
                for entrypoint in entrypoints {
                    self.blocks.insert(entrypoint, height);
                }
            }
            Some(block) if block.height == height => {
                let mut updated = block.clone();
                updated.transactions.extend(entrypoints);
                self.latest_block.store(Some(Arc::new(updated)));
            }
            Some(block) => {
                for &entrypoint in &block.transactions {
                    self.blocks.insert(entrypoint, block.height);
                }
                self.latest_block.store(Some(Arc::new(BlockInfo {
                    transactions: entrypoints,
                    height,
                })));
            }
            None => self.latest_block.store(Some(Arc::new(BlockInfo {
                transactions: entrypoints,
                height,
            }))),
        }
        **guard = next_identity;
    }
    /// Deliberately replace existing membership for a malformed-State recovery fixture.
    ///
    /// Unlike historical seeding, this removes a newer hot-set copy too, so the
    /// public reader observes the requested corruption. The canonical frontier
    /// and all unrelated memberships remain fixed; production never calls this.
    #[cfg(test)]
    pub(crate) fn overwrite_committed_entrypoint_membership_for_tests(
        &self,
        entrypoint: Key,
        height: Value,
    ) {
        let mut guard = self.released.guard(self.write_lock.lock());
        let next_identity = Arc::new(());
        assert!(
            self.view().get(&entrypoint).is_some(),
            "overwrite needs existing membership"
        );
        let latest = self
            .latest_block
            .load_full()
            .expect("existing membership has a frontier");
        assert!(
            height <= latest.height,
            "corruption must not advance the fixture frontier"
        );
        let mut updated = latest.as_ref().clone();
        updated.transactions.remove(&entrypoint);
        self.blocks.remove(&entrypoint);
        if height == updated.height {
            updated.transactions.insert(entrypoint);
        } else {
            self.blocks.insert(entrypoint, height);
        }
        self.latest_block.store(Some(Arc::new(updated)));
        **guard = next_identity;
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
        let guard = self.released.guard(self.write_lock.lock());
        TransactionsBlock {
            latest_block_ref: &self.latest_block,
            blocks_ref: &self.blocks,
            _guard: guard,
            revert,
            current_block: None,
        }
    }
}
/// Read the logical committed cut, excluding the abandoned tip during replacement.
fn membership_at_cut<Q>(
    latest: Option<&BlockInfo>,
    history: &DashMap<Key, Value>,
    undo_latest: bool,
    key: &Q,
) -> Option<Value>
where
    Key: Borrow<Q>,
    Q: Hash + Eq + ?Sized,
{
    let latest = latest?;
    if !undo_latest && latest.transactions.contains(key) {
        Some(latest.height)
    } else {
        history
            .get(key)
            .map(|height| *height)
            .filter(|height| *height < latest.height)
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
            membership_at_cut(self.latest_block.as_deref(), self.blocks, false, key)
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
    /// The block aggregates transaction hashes for a particular block height. Call [`insert_block`]
    /// exactly once to register the transactions and height. After all updates are collected,
    /// [`commit`](Self::commit) can be used to persist them in [`TransactionsStorage`].
    ///
    /// Committing a block without first calling [`insert_block`] is considered an error and will
    /// cause [`commit`](Self::commit) to fail. See the release-mode tests for examples. Errors that
    /// can occur when committing [`TransactionsBlock`]
    #[derive(thiserror::Error, Debug, displaydoc::Display)]
    #[ignore_extra_doc_attributes]
    pub enum TransactionsBlockError {
        /// `TransactionsBlock::insert_block()` was not called
        MissingInsertBlock,
        /// Block height `{actual_current_height}` does not match expected `{expected_current_height}`;
        /// callers should abort the block and retry against the latest state view.
        HeightMismatch {
            /// Height expected by the storage while committing the block.
            expected_current_height: usize,
            /// Height encoded in the block being committed.
            actual_current_height: usize,
        },
        /// The next transaction-membership height exceeds the storage representation
        HeightOverflow,
        /// Deterministic autoscale lane lifecycle failed while preparing block commit
        AutoscaleLaneLifecycle,
        /// Local lane geometry publication retains its original refusal: {0}
        LocalLaneGeometry(#[source] crate::state::LaneLifecycleError),
        /// Certified merge admission changed before the block could commit
        MergeAdmission,
        /// Finalized FASTPQ source ownership is invalid at block commit
        FastpqSourceInventory,
        /// Execution outputs require their canonical consuming publication owner
        ExecutionOutputCapacity,
        /// Frozen lane consensus metadata changed after its authenticated capture
        LaneConsensusContexts,
        /// Permanent AXT handle counter could not finalize its block transition
        AxtCounterRatchet,
        /// Live asset-definition incarnations are inconsistent with the registry
        AxtAssetIncarnation,
        /// Prepared World commit does not match the actual State or target height
        WorldCommitPreparation,
        /// The applying State was busy or changed during its complete snapshot observation
        SnapshotObservationChanged,
        /// A stable State snapshot projection or encoding is malformed
        SnapshotProjection,
    }
    impl From<crate::state::LaneLifecycleError> for TransactionsBlockError {
        fn from(error: crate::state::LaneLifecycleError) -> Self {
            use crate::state::LaneLifecycleError;
            match error {
                error @ (LaneLifecycleError::Storage(_)
                | LaneLifecycleError::GeometryStorage(_)
                | LaneLifecycleError::DrainObservation(_)
                | LaneLifecycleError::PublicationBusy { .. }) => Self::LocalLaneGeometry(error),
                _ => Self::AutoscaleLaneLifecycle,
            }
        }
    }
    /// Batched update to the storage that can be reverted later
    pub struct TransactionsBlock<'storage> {
        /// References to [`TransactionsStorage`] struct
        pub(super) latest_block_ref: &'storage ArcSwapOption<BlockInfo>,
        pub(super) blocks_ref: &'storage DashMap<Key, Value>,
        pub(super) _guard: mv::ReleaseGuard<'storage, MutexGuard<'storage, RawMutex, Arc<()>>>,
        /// Own fields
        pub(super) revert: bool,
        pub(super) current_block: Option<Arc<BlockInfo>>,
    }
    /// An admitted membership transition retaining its original exclusive writer.
    ///
    /// Dropping this owner leaves storage unchanged. Publication consumes the
    /// exact admitted action without consulting a new frontier or admitting a
    /// different payload. Shared reads remain available for State snapshots.
    pub(crate) struct PreparedTransactionsBlock<'storage> {
        block: TransactionsBlock<'storage>,
        publication: MembershipPublication,
        next_identity: Arc<()>,
    }

    /// An admitted transition whose original physical writer has been released.
    ///
    /// All payload sets are moved or share their original immutable allocation.
    /// This owner exposes no live-history reader. Publication preparation must
    /// reacquire its exact predecessor; the aggregate State publisher must
    /// retain every prepared component before publishing any of them.
    pub(crate) struct DetachedTransactionsBlock {
        predecessor_identity: Arc<()>,
        predecessor: Option<Arc<BlockInfo>>,
        current: Arc<BlockInfo>,
        revert: bool,
        publication: MembershipPublication,
        next_identity: Arc<()>,
    }

    /// Exact admitted membership with its writer reacquired for publication.
    ///
    /// The installation admission outlives the writer even when this owner is
    /// abandoned. Publication returns that admission to the aggregate owner.
    pub(crate) struct PreparedDetachedTransactionsBlock<'storage, Installation> {
        prepared: PreparedTransactionsBlock<'storage>,
        installation: Installation,
    }

    /// A short observation, never authorization to publish a detached journal.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum MembershipPredecessorStatus {
        /// Another writer owns the observation boundary.
        Busy,
        /// Both membership representations still have the captured identity.
        Current,
        /// Some committed membership changed after capture.
        Changed,
    }

    enum MembershipPublication {
        Repeated,
        Replace {
            current: Arc<BlockInfo>,
        },
        Advance {
            previous: Option<Arc<BlockInfo>>,
            current: Arc<BlockInfo>,
        },
    }

    impl<'storage> TransactionsBlock<'storage> {
        /// Return whether a canonical block membership update was staged.
        pub(crate) fn has_staged_block(&self) -> bool {
            self.current_block.is_some()
        }
        /// Return whether the staged canonical carrier has the exact height and
        /// entrypoint membership.
        pub(crate) fn has_exact_staged_block(
            &self,
            height: NonZeroUsize,
            transactions: &HashSet<Key>,
        ) -> bool {
            self.current_block
                .as_ref()
                .is_some_and(|block| block.height == height && &block.transactions == transactions)
        }
        /// Register transactions belonging to the block.
        ///
        /// This method **must** be called before [`commit`]. Calling it more than once with the
        /// same block payload is a no-op, while changing the height or the transaction set triggers
        /// a panic. Attempting to commit without inserting a block results in an error (see
        /// `commit_without_insert_block_fails`).
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
        /// Returns an error if [`insert_block`] was not called prior to committing or if the block
        /// height being committed does not match the expected height derived from the current
        /// storage state. These behaviours are illustrated in the release-mode tests.
        pub fn commit(self) -> Result<(), TransactionsBlockError> {
            self.prepare_commit()?.publish();
            Ok(())
        }
        /// Admit the exact staged transition and retain its exclusive writer.
        ///
        /// This performs all membership validation before publication. Failure
        /// drops the staging scope without modifying committed membership.
        pub(crate) fn prepare_commit(
            self,
        ) -> Result<PreparedTransactionsBlock<'storage>, TransactionsBlockError> {
            let publication = self.admit_publication()?;
            Ok(PreparedTransactionsBlock {
                block: self,
                publication,
                next_identity: Arc::new(()),
            })
        }
        /// Validate that this block can be committed without mutating the storage.
        ///
        /// This lets callers perform other fallible commit preparation after the transaction height
        /// has been proven acceptable, but before consuming the transaction block.
        pub(crate) fn validate_commit(&self) -> Result<(), TransactionsBlockError> {
            self.admit_publication().map(|_| ())
        }
        fn admit_publication(&self) -> Result<MembershipPublication, TransactionsBlockError> {
            let previous_block = self.latest_block_ref.load_full();
            let previous_height = previous_block.as_ref().map_or(0, |b| b.height.get());
            let Some(current_block) = self.current_block.as_ref() else {
                return Err(TransactionsBlockError::MissingInsertBlock);
            };
            if !self.revert
                && previous_block.as_ref().is_some_and(|previous_block| {
                    previous_block.height == current_block.height
                        && previous_block.transactions == current_block.transactions
                })
            {
                return Ok(MembershipPublication::Repeated);
            }
            let addition = usize::from(!self.revert);
            let expected_current_height = previous_height
                .checked_add(addition)
                .ok_or(TransactionsBlockError::HeightOverflow)?;
            let current_height = current_block.height.get();
            if expected_current_height != current_height {
                return Err(TransactionsBlockError::HeightMismatch {
                    expected_current_height,
                    actual_current_height: current_height,
                });
            }
            if self.revert {
                Ok(MembershipPublication::Replace {
                    current: Arc::clone(current_block),
                })
            } else {
                Ok(MembershipPublication::Advance {
                    previous: previous_block,
                    current: Arc::clone(current_block),
                })
            }
        }
    }
    impl<'storage> PreparedTransactionsBlock<'storage> {
        /// Move the admitted action and exact cut, then release the writer.
        ///
        /// This adds no collection allocation or copy. Snapshot/checkpoint
        /// projections must already have consumed the original locked reader.
        pub(crate) fn detach(self) -> DetachedTransactionsBlock {
            let Self {
                block,
                publication,
                next_identity,
            } = self;
            let detached = DetachedTransactionsBlock {
                predecessor_identity: Arc::clone(&block._guard),
                predecessor: block.latest_block_ref.load_full(),
                current: Arc::clone(block.current_block.as_ref().expect("admitted membership")),
                revert: block.revert,
                publication,
                next_identity,
            };
            drop(block);
            detached
        }

        /// Borrow immutable staged membership and its actual predecessor.
        pub(crate) fn as_block(&self) -> &TransactionsBlock<'storage> {
            &self.block
        }

        /// Publish the admitted transition once, retaining its writer throughout.
        ///
        /// All semantic refusal happened during preparation. The retained mutex
        /// prevents any other membership writer from changing the admitted cut.
        pub(crate) fn publish(self) {
            let Self {
                mut block,
                publication,
                next_identity,
            } = self;
            let changes_identity = !matches!(&publication, MembershipPublication::Repeated);
            match publication {
                MembershipPublication::Repeated => {
                    // Do not promote a repeated tip into history: replacement
                    // must still recover the actual older membership.
                }
                MembershipPublication::Replace { current } => {
                    block
                        .blocks_ref
                        .retain(|_, height| *height < current.height);
                    block.latest_block_ref.store(Some(current));
                }
                MembershipPublication::Advance { previous, current } => {
                    if let Some(previous) = previous {
                        for &transaction in &previous.transactions {
                            block.blocks_ref.insert(transaction, previous.height);
                        }
                    }
                    block.latest_block_ref.store(Some(current));
                }
            }
            if changes_identity {
                **block._guard = next_identity;
            }
            drop(block);
        }
    }
    impl DetachedTransactionsBlock {
        /// Admit installation and reacquire the original predecessor without waiting.
        ///
        /// No transaction set is rebuilt or admitted again. The callback must
        /// account for history-map insertion and reader retention before the
        /// writer is acquired. A refusal returns the original journal intact.
        /// This component does not establish aggregate State/finality authority.
        pub(crate) fn try_prepare_publication<'storage, Installation, E>(
            self,
            storage: &'storage TransactionsStorage,
            admit: impl FnOnce(&Self, &TransactionsStorage) -> Result<Installation, E>,
        ) -> Result<
            PreparedDetachedTransactionsBlock<'storage, Installation>,
            (Self, mv::PublicationPreparationError<E>),
        > {
            let wait = storage.released.observe();
            match self.observe_predecessor(storage) {
                MembershipPredecessorStatus::Busy => {
                    return Err((
                        self,
                        mv::PublicationPreparationError::after_failed_acquisition(wait),
                    ));
                }
                MembershipPredecessorStatus::Changed => {
                    return Err((self, mv::PublicationPreparationError::Changed));
                }
                MembershipPredecessorStatus::Current => {}
            }
            let installation = match admit(&self, storage) {
                Ok(installation) => installation,
                Err(error) => {
                    return Err((self, mv::PublicationPreparationError::Admission(error)));
                }
            };
            let wait = storage.released.observe();
            let Some(guard) = storage.write_lock.try_lock() else {
                return Err((
                    self,
                    mv::PublicationPreparationError::after_failed_acquisition(wait),
                ));
            };
            let guard = storage.released.guard(guard);
            if !Arc::ptr_eq(&guard, &self.predecessor_identity) {
                drop(guard);
                return Err((self, mv::PublicationPreparationError::Changed));
            }
            let Self {
                predecessor_identity: _,
                predecessor: _,
                current,
                revert,
                publication,
                next_identity,
            } = self;
            Ok(PreparedDetachedTransactionsBlock {
                prepared: PreparedTransactionsBlock {
                    block: TransactionsBlock {
                        latest_block_ref: &storage.latest_block,
                        blocks_ref: &storage.blocks,
                        _guard: guard,
                        revert,
                        current_block: Some(current),
                    },
                    publication,
                    next_identity,
                },
                installation,
            })
        }

        /// Borrow the exact admitted carrier height and immutable membership.
        pub(crate) fn staged_membership(&self) -> (Value, &HashSet<Key>) {
            (self.current.height, &self.current.transactions)
        }

        /// Whether admission reverted the original committed tip first.
        pub(crate) fn replaces_tip(&self) -> bool {
            self.revert
        }

        /// Height of the original committed cut, including a replaced tip.
        pub(crate) fn predecessor_height(&self) -> usize {
            self.predecessor
                .as_ref()
                .map_or(0, |block| block.height.get())
        }

        /// Observe a target's exact captured identity without blocking on its writer.
        /// A different storage, including one restored from identical bytes, differs.
        ///
        /// The result is advisory: an aggregate publisher must retain all
        /// journal writers while checking identities and publishing together.
        pub(crate) fn observe_predecessor(
            &self,
            storage: &TransactionsStorage,
        ) -> MembershipPredecessorStatus {
            let Some(guard) = storage.write_lock.try_lock() else {
                return MembershipPredecessorStatus::Busy;
            };
            let guard = storage.released.guard(guard);
            if Arc::ptr_eq(&guard, &self.predecessor_identity) {
                MembershipPredecessorStatus::Current
            } else {
                MembershipPredecessorStatus::Changed
            }
        }
    }
    impl<Installation> PreparedDetachedTransactionsBlock<'_, Installation> {
        /// Release the physical writer and recover the same admitted journal.
        pub(crate) fn abort(self) -> DetachedTransactionsBlock {
            let Self {
                prepared,
                installation,
            } = self;
            let journal = prepared.detach();
            drop(installation);
            journal
        }

        /// Consume the original admitted action and return its installation guard.
        ///
        /// The caller must already hold all other component writers and the
        /// exact aggregate publication authorization before calling this method.
        pub(crate) fn publish(self) -> Installation {
            let Self {
                prepared,
                installation,
            } = self;
            prepared.publish();
            installation
        }
    }
    impl TransactionsReadOnly for PreparedTransactionsBlock<'_> {
        fn get<Q>(&self, key: &Q) -> Option<Value>
        where
            Key: Borrow<Q>,
            Q: Hash + Eq + ?Sized,
        {
            self.block.get(key)
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
            let latest = self.latest_block_ref.load();
            membership_at_cut(latest.as_deref(), self.blocks_ref, self.revert, key)
        }
    }
}
pub(crate) use block::{
    DetachedTransactionsBlock, MembershipPredecessorStatus, PreparedDetachedTransactionsBlock,
    PreparedTransactionsBlock,
};
#[allow(unused_imports)]
pub use block::{TransactionsBlock, TransactionsBlockError};

/// Borrowed logical membership, independent of the latest/history representation.
mod membership_projection {
    use super::*;

    /// One validated transition borrowed from the actual membership owner.
    ///
    /// The block's existing write guard remains held for this borrow. No key set
    /// is copied and no caller-supplied inventory participates. Visits have
    /// unspecified order: use a history-independent map or canonicalize in the
    /// consumer rather than folding callback order into a commitment.
    /// This describes membership only, not carrier or sealed-alias admission.
    pub(in crate::state) struct TransactionsMembershipTransition<'block> {
        before: Snapshot<'block>,
        current: &'block BlockInfo,
    }

    struct Snapshot<'block> {
        latest: Option<Arc<BlockInfo>>,
        history: &'block DashMap<Key, Value>,
        revert: bool,
    }

    impl TransactionsBlock<'_> {
        fn membership_snapshot(&self) -> Snapshot<'_> {
            Snapshot {
                latest: self.latest_block_ref.load_full(),
                history: self.blocks_ref,
                revert: self.revert,
            }
        }

        /// Visit the exact currently committed logical hash-to-height map.
        ///
        /// This cold visit scans history and the latest set, borrowing each key
        /// while the block's write fence is held. It can run before staging.
        /// Callback order is unspecified and callbacks must not acquire a writer
        /// for this storage. Callback errors stop the visit without changing it.
        pub(in crate::state) fn visit_committed_membership<E>(
            &self,
            visit: impl FnMut(&Key, Value) -> Result<(), E>,
        ) -> Result<(), E> {
            self.membership_snapshot().visit(false, visit)
        }

        /// Visit the exact logical predecessor used by this block scope.
        ///
        /// For replacement this excludes the abandoned latest set and retains
        /// only earlier history, matching commit's undo cut. Ordinary scopes
        /// visit the committed tip. The same cold-visit rules as
        /// [`Self::visit_committed_membership`] apply.
        pub(in crate::state) fn visit_predecessor_membership<E>(
            &self,
            visit: impl FnMut(&Key, Value) -> Result<(), E>,
        ) -> Result<(), E> {
            let snapshot = self.membership_snapshot();
            snapshot.visit(snapshot.revert, visit)
        }

        /// Borrow an exact transition only after existing commit validation.
        ///
        /// Missing staging or an invalid height fails before any consumer can
        /// observe staged rows. The borrow prevents staging or committing a
        /// different payload while the transition is in use.
        pub(in crate::state) fn membership_transition(
            &self,
        ) -> Result<TransactionsMembershipTransition<'_>, TransactionsBlockError> {
            self.validate_commit()?;
            let current = self
                .current_block
                .as_deref()
                .ok_or(TransactionsBlockError::MissingInsertBlock)?;
            Ok(TransactionsMembershipTransition {
                before: self.membership_snapshot(),
                current,
            })
        }
    }

    impl Snapshot<'_> {
        fn history_before(&self, key: &Key, ceiling: Value) -> Option<Value> {
            self.history
                .get(key)
                .map(|entry| *entry)
                .filter(|height| *height < ceiling)
        }

        fn get(&self, undo_latest: bool, key: &Key) -> Option<Value> {
            membership_at_cut(self.latest.as_deref(), self.history, undo_latest, key)
        }

        fn visit<E>(
            &self,
            undo_latest: bool,
            mut visit: impl FnMut(&Key, Value) -> Result<(), E>,
        ) -> Result<(), E> {
            let Some(latest) = self.latest.as_ref() else {
                return Ok(());
            };
            if !undo_latest {
                for key in &latest.transactions {
                    visit(key, latest.height)?;
                }
            }
            for entry in self.history.iter() {
                if *entry.value() < latest.height
                    && (undo_latest || !latest.transactions.contains(entry.key()))
                {
                    visit(entry.key(), *entry.value())?;
                }
            }
            Ok(())
        }
    }

    impl TransactionsMembershipTransition<'_> {
        /// Height of the committed tip before this publication; zero if empty.
        pub(in crate::state) fn committed_height(&self) -> usize {
            self.before
                .latest
                .as_ref()
                .map_or(0, |block| block.height.get())
        }

        /// Height of the logical predecessor, after undo for replacement.
        pub(in crate::state) fn predecessor_height(&self) -> usize {
            let committed = self.committed_height();
            if self.before.revert {
                committed.saturating_sub(1)
            } else {
                committed
            }
        }

        /// Height whose exact membership has passed existing commit validation.
        pub(in crate::state) fn staged_height(&self) -> Value {
            self.current.height
        }

        /// Visit the exact post-commit logical map without publishing it.
        ///
        /// Cold traversal scans history plus the current/latest sets; duplicate
        /// physical keys are visited once with their actual lookup precedence.
        /// The underlying stores are borrowed and callback order is unspecified.
        pub(in crate::state) fn visit_staged_membership<E>(
            &self,
            mut visit: impl FnMut(&Key, Value) -> Result<(), E>,
        ) -> Result<(), E> {
            for key in &self.current.transactions {
                visit(key, self.current.height)?;
            }
            let promoted = (!self.before.revert)
                .then_some(self.before.latest.as_deref())
                .flatten();
            if let Some(previous) = promoted {
                for key in &previous.transactions {
                    if !self.current.transactions.contains(key) {
                        visit(key, previous.height)?;
                    }
                }
            }
            for entry in self.before.history.iter() {
                if *entry.value() < self.current.height
                    && !self.current.transactions.contains(entry.key())
                    && !promoted.is_some_and(|previous| previous.transactions.contains(entry.key()))
                {
                    visit(entry.key(), *entry.value())?;
                }
            }
            Ok(())
        }

        /// Visit net changes from the logical predecessor to the staged map.
        ///
        /// This visits only the actual staged set. Ordinary latest-set promotion
        /// changes representation, not logical values. Replacement starts from
        /// the earlier history cut, not the abandoned committed tip.
        pub(in crate::state) fn visit_predecessor_changes<E>(
            &self,
            mut visit: impl FnMut(&Key, Option<Value>, Option<Value>) -> Result<(), E>,
        ) -> Result<(), E> {
            for key in &self.current.transactions {
                let before = self.before.get(self.before.revert, key);
                let after = Some(self.current.height);
                if before != after {
                    visit(key, before, after)?;
                }
            }
            Ok(())
        }

        /// Visit net changes from the committed tip to the staged map.
        ///
        /// Unlike predecessor changes, replacement includes removal of abandoned
        /// tip-only keys and restoration of their earlier historical values.
        /// Work is bounded by the actual staged and latest sets, without scanning
        /// history. This relies on the private storage invariant: history has
        /// heights below the committed tip; exact repeated commits preserve it.
        pub(in crate::state) fn visit_committed_changes<E>(
            &self,
            mut visit: impl FnMut(&Key, Option<Value>, Option<Value>) -> Result<(), E>,
        ) -> Result<(), E> {
            for key in &self.current.transactions {
                let before = self.before.get(false, key);
                let after = Some(self.current.height);
                if before != after {
                    visit(key, before, after)?;
                }
            }
            if self.before.revert
                && let Some(previous) = &self.before.latest
            {
                for key in &previous.transactions {
                    if !self.current.transactions.contains(key) {
                        let after = self.before.history_before(key, self.current.height);
                        visit(key, Some(previous.height), after)?;
                    }
                }
            }
            Ok(())
        }
    }
}
pub(in crate::state) use membership_projection::TransactionsMembershipTransition;

#[cfg(test)]
#[path = "storage_transactions_projection_tests.rs"]
mod projection_tests;

#[cfg(test)]
#[path = "storage_transactions_preparation_tests.rs"]
mod preparation_tests;

#[cfg(test)]
#[path = "storage_transactions_publication_tests.rs"]
mod publication_tests;

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
    fn write_transactions_block_json(block: &TransactionsBlock<'_>, out: &mut String) {
        out.push('{');
        json::write_json_string("latest_block", out);
        out.push(':');
        match block.current_block.as_ref() {
            Some(current) => JsonSerializeTrait::json_serialize(current.as_ref(), out),
            None => out.push_str("null"),
        }
        out.push(',');
        json::write_json_string("blocks", out);
        out.push(':');
        let mut map = BTreeMap::new();
        #[allow(clippy::explicit_iter_loop)]
        for entry in block.blocks_ref.iter() {
            map.insert(*entry.key(), *entry.value());
        }
        if block.revert {
            if let Some(current) = block.current_block.as_ref() {
                map.retain(|_, height| *height < current.height);
            }
        } else {
            let previous = block.latest_block_ref.load();
            if let Some(previous) = previous.as_ref() {
                let repeated = block.current_block.as_ref().is_some_and(|current| {
                    current.height == previous.height
                        && current.transactions == previous.transactions
                });
                if !repeated {
                    for transaction in &previous.transactions {
                        map.insert(*transaction, previous.height);
                    }
                }
            }
        }
        JsonSerializeTrait::json_serialize(&map, out);
        out.push('}');
    }
    impl JsonSerializeTrait for TransactionsStorage {
        fn json_serialize(&self, out: &mut String) {
            write_transactions_view_json(&self.view(), out)
        }
    }
    impl JsonSerializeTrait for TransactionsBlock<'_> {
        fn json_serialize(&self, out: &mut String) {
            write_transactions_block_json(self, out)
        }
    }
    impl JsonSerializeTrait for PreparedTransactionsBlock<'_> {
        fn json_serialize(&self, out: &mut String) {
            write_transactions_block_json(self.as_block(), out)
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
            if let Some(field) = map.keys().next() {
                return Err(json::Error::unknown_field(field.as_str()));
            }
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
                write_lock: Mutex::new(Arc::new(())),
                released: mv::ReleaseNotification::default(),
            })
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_TEST_HASH: AtomicU64 = AtomicU64::new(1);
    fn random_hash() -> Key {
        let counter = NEXT_TEST_HASH.fetch_add(1, Ordering::Relaxed);
        let mut bytes = [0_u8; iroha_crypto::Hash::LENGTH];
        bytes[..8].copy_from_slice(&counter.to_le_bytes());
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
    fn fixture_membership_overwrite_reaches_public_reader_without_moving_frontier() {
        let [corrupted, untouched] = get_keys();
        let [historical, current] = get_values();
        let storage = TransactionsStorage::new();
        storage.record_committed_entrypoint_membership_for_tests([corrupted, untouched], current);
        storage.overwrite_committed_entrypoint_membership_for_tests(corrupted, historical);
        assert_eq!(storage.view().get(&corrupted), Some(historical));
        assert_eq!(storage.view().get(&untouched), Some(current));
        assert_eq!(storage.latest_height(), current.get());
        storage.overwrite_committed_entrypoint_membership_for_tests(corrupted, current);
        assert_eq!(storage.view().get(&corrupted), Some(current));
        assert_eq!(storage.view().get(&untouched), Some(current));
        assert!(storage.blocks.get(&corrupted).is_none());
        assert_eq!(storage.latest_height(), current.get());
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
    fn lane_geometry_commit_refusal_preserves_storage_source_and_deterministic_errors() {
        use crate::state::LaneLifecycleError;
        use std::error::Error as _;

        let path = std::path::PathBuf::from("retained/lane_geometry_journal.norito");
        let error = TransactionsBlockError::from(LaneLifecycleError::GeometryStorage(
            crate::kura::Error::IO(
                std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "original journal owner",
                ),
                path.clone(),
            ),
        ));
        let lifecycle = error
            .source()
            .and_then(|source| source.downcast_ref::<LaneLifecycleError>())
            .expect("commit refusal retains the typed lifecycle source");
        let Some(crate::kura::Error::IO(io, actual_path)) = lifecycle
            .source()
            .and_then(|source| source.downcast_ref::<crate::kura::Error>())
        else {
            panic!("commit refusal must retain the original Kura IO source");
        };
        assert_eq!(io.kind(), std::io::ErrorKind::PermissionDenied);
        assert_eq!(io.to_string(), "original journal owner");
        assert_eq!(actual_path, &path);
        let drain = TransactionsBlockError::from(LaneLifecycleError::DrainObservation(
            crate::state::MergeLedgerCommitError::Persistence(crate::kura::Error::IO(
                std::io::Error::new(std::io::ErrorKind::PermissionDenied, "original drain owner"),
                path.clone(),
            )),
        ));
        let lifecycle = drain
            .source()
            .and_then(|source| source.downcast_ref::<LaneLifecycleError>())
            .expect("drain refusal remains a local lifecycle source");
        let observation = lifecycle
            .source()
            .and_then(|source| source.downcast_ref::<crate::state::MergeLedgerCommitError>())
            .expect("drain refusal retains its exact observation error");
        assert!(
            matches!(observation, crate::state::MergeLedgerCommitError::Persistence(
            crate::kura::Error::IO(io, actual_path)
        ) if io.kind() == std::io::ErrorKind::PermissionDenied
            && io.to_string() == "original drain owner" && actual_path == &path)
        );
        assert!(matches!(
            TransactionsBlockError::from(LaneLifecycleError::Storage("tiered capture".to_owned())),
            TransactionsBlockError::LocalLaneGeometry(source)
                if matches!(&source, LaneLifecycleError::Storage(detail) if detail == "tiered capture")
        ));
        assert!(matches!(
            TransactionsBlockError::from(LaneLifecycleError::PhysicalPrimaryReplacement),
            TransactionsBlockError::AutoscaleLaneLifecycle
        ));
    }

    #[test]
    fn lane_geometry_commit_refusal_retains_original_release_observation() {
        use crate::state::LaneLifecycleError;
        use std::{
            future::Future as _,
            pin::Pin,
            task::{Context, Waker},
        };

        let release = mv::ReleaseNotification::default();
        let original_owner = release.guard(());
        let observation = release.observe();
        let error = TransactionsBlockError::from(LaneLifecycleError::PublicationBusy {
            field: "original geometry writer",
            wait: observation.clone(),
        });
        let TransactionsBlockError::LocalLaneGeometry(source) = error else {
            panic!("local publication refusal must retain its source");
        };
        let LaneLifecycleError::PublicationBusy { field, wait } = source else {
            panic!("original release observation must survive conversion");
        };
        assert_eq!(field, "original geometry writer");
        assert_eq!(wait, observation);
        let mut future = wait.wait_for_release();
        let mut context = Context::from_waker(Waker::noop());
        assert!(Pin::new(&mut future).poll(&mut context).is_pending());
        drop(original_owner);
        assert!(Pin::new(&mut future).poll(&mut context).is_ready());
    }

    #[test]
    fn commit_without_insert_block_fails() {
        let storage = TransactionsStorage::new();
        let block = storage.block();
        assert!(matches!(
            block.commit(),
            Err(TransactionsBlockError::MissingInsertBlock)
        ));
    }
    #[test]
    fn validate_commit_height_mismatch_does_not_mutate_storage() {
        let [key] = get_keys();
        let storage = TransactionsStorage::new();
        let mut block = storage.block();
        let wrong_height = NonZeroUsize::new(2).unwrap();
        block.insert_block(HashSet::from([key]), wrong_height);
        assert!(matches!(
            block.validate_commit(),
            Err(TransactionsBlockError::HeightMismatch {
                expected_current_height: 1,
                actual_current_height,
            }) if actual_current_height == wrong_height.get()
        ));
        assert_eq!(storage.latest_height(), 0);
        drop(block);
        assert_eq!(storage.latest_height(), 0);
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
        assert!(matches!(
            block.commit(),
            Err(TransactionsBlockError::HeightMismatch {
                expected_current_height,
                actual_current_height,
            }) if expected_current_height == expected_height
                && actual_current_height == wrong_height.get()
        ));
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
    fn latest_height_tracks_committed_block() {
        let [key] = get_keys();
        let [value] = get_values();
        let storage = TransactionsStorage::new();
        assert_eq!(storage.latest_height(), 0);
        let mut block = storage.block();
        insert_keys(&mut block, &[key], value);
        block.commit().unwrap();
        assert_eq!(storage.latest_height(), value.get());
    }
    #[test]
    fn carrier_membership_is_atomic_with_canonical_revert() {
        let [
            canonical_key,
            ordinary_carrier_key,
            merge_carrier_key,
            replacement_key,
        ] = get_keys();
        let [height1, height2] = get_values();
        let storage = TransactionsStorage::new();
        {
            let mut block = storage.block();
            insert_keys(&mut block, &[canonical_key], height1);
            block.commit().unwrap();
        }
        {
            let mut block = storage.block();
            insert_keys(
                &mut block,
                &[ordinary_carrier_key, merge_carrier_key],
                height2,
            );
            block.commit().unwrap();
        }
        {
            let mut block = storage.block_and_revert();
            insert_keys(&mut block, &[replacement_key], height2);
            block.commit().unwrap();
        }
        let view = storage.view();
        assert_eq!(view.get(&canonical_key), Some(height1));
        assert_eq!(view.get(&replacement_key), Some(height2));
        assert_eq!(view.get(&ordinary_carrier_key), None);
        assert_eq!(view.get(&merge_carrier_key), None);
    }
    #[test]
    fn retired_direct_membership_json_is_rejected() {
        let error = match norito::json::from_str::<TransactionsStorage>(
            r#"{"latest_block":null,"blocks":{},"direct_committed":{}}"#,
        ) {
            Ok(_) => panic!("retired membership field must not be accepted"),
            Err(error) => error,
        };
        assert!(
            matches!(error, json::Error::UnknownField { field } if field == "direct_committed")
        );
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
    fn commit_same_block_twice_is_idempotent() {
        let [key] = get_keys();
        let [value] = get_values();
        let storage = TransactionsStorage::new();
        {
            let mut block = storage.block();
            insert_keys(&mut block, &[key], value);
            block.commit().unwrap();
        }
        {
            let mut block = storage.block();
            insert_keys(&mut block, &[key], value);
            block.commit().unwrap();
        }
        assert_eq!(storage.view().get(&key), Some(value));
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
