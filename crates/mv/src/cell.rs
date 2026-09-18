use crate::{
    BlockMode, PublicationPreparationError, Value,
    publication::{CapturedPublication, NextPublication, Publication},
};
/// Multi-version storage for single value
pub struct Cell<V: Value> {
    /// Process-local identity of the jointly published current/undo pair.
    pub(crate) publication: Publication,
    /// Previous version of value, required to perform revert of the latest changes
    pub(crate) revert: EbrCell<Option<V>>,
    /// Value which represent aggregated changes of multiple blocks
    pub(crate) blocks: EbrCell<V>,
}
impl<V: Value> Cell<V> {
    /// Construct new [`Self`]
    pub fn new(v: V) -> Self {
        Self {
            publication: Publication::new(),
            revert: EbrCell::new(None),
            blocks: EbrCell::new(v),
        }
    }
    /// Create persistent view of storage at certain point in time
    pub fn view(&self) -> View<'_, V> {
        View {
            blocks: self.blocks.read(),
            _marker: core::marker::PhantomData,
        }
    }
    /// Borrow the retained predecessor of the most recently committed block.
    ///
    /// `None` means that block did not change the value. This immutable EBR
    /// view performs no clone of `V` and remains stable across later commits.
    /// Consumers combining current and predecessor views must bind both reads
    /// to their own publication generation; this method is not a joint snapshot.
    pub fn predecessor_view(&self) -> View<'_, Option<V>> {
        View {
            blocks: self.revert.read(),
            _marker: core::marker::PhantomData,
        }
    }
    /// Replace the current value at the same logical cut, preserving retained undo.
    ///
    /// This is for validated bootstrap, restore, or configuration projection at
    /// an existing cut. It must not publish a new block: use [`Self::block`] for
    /// a height transition. The caller owns any cross-field publication generation.
    /// Both writers are acquired in the same order as block scopes; no observer
    /// can mutate predecessor storage concurrently with this current replacement.
    pub fn replace_current_preserving_predecessor(&self, value: V) {
        let _revert = self.revert.write();
        let mut blocks = self.blocks.write();
        *blocks.get_mut() = value;
        self.publication.publish(|| blocks.commit());
    }
    /// Create block to aggregate updates
    pub fn block(&self) -> Block<'_, V> {
        let mut revert = self.revert.write();
        let blocks = self.blocks.write();
        let predecessor = self.publication.capture();
        *revert.get_mut() = None;
        Block::new(
            revert,
            blocks,
            false,
            &self.publication,
            predecessor,
            BlockMode::Ordinary,
        )
    }
    /// Create block to aggregate updates and revert changes made in latest block
    pub fn block_and_revert(&self) -> Block<'_, V> {
        let mut revert = self.revert.write();
        let mut blocks = self.blocks.write();
        let predecessor = self.publication.capture();
        {
            let revert = core::mem::take(revert.get_mut());
            if let Some(revert) = revert {
                *blocks.get_mut() = revert;
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
impl<V: Value + Default> Default for Cell<V> {
    fn default() -> Self {
        Self::new(V::default())
    }
}
/// Module for [`View`] and it's related impls
mod view {
    use super::*;
    use concread::ebrcell::EbrCellReadTxn;
    use std::ops::Deref;
    /// Consistent view of the storage at the certain version
    pub struct View<'storage, V: Value> {
        pub(crate) blocks: EbrCellReadTxn<V>,
        pub(crate) _marker: core::marker::PhantomData<&'storage V>,
    }
    impl<V: Value> View<'_, V> {
        /// Read entry from the list up to certain version non-inclusive
        pub fn get(&self) -> &V {
            &self.blocks
        }
    }
    impl<V: Value> Deref for View<'_, V> {
        type Target = V;
        fn deref(&self) -> &Self::Target {
            self.get()
        }
    }
}
use concread::EbrCell;
pub use view::View;
/// Borrowed before/after values for a cell touched by an overlay.
///
/// Equal values remain explicit: a mutable borrow is not proof of a semantic
/// change. The consumer owns any canonical encoding and equality policy.
pub struct TouchedValue<'a, V: Value> {
    /// Value before the overlay's first mutable access.
    pub before: &'a V,
    /// Current value, including applied child changes.
    pub after: &'a V,
}

/// Owned cell delta detached from its original writer after resource admission.
///
/// This move-only owner retains no EBR pin or storage lock. It identifies the
/// original published current/undo pair even when the candidate did not touch
/// the value. Replacement mode also retains the instruction to undo the tip;
/// that step is not a candidate touch. This is neither a complete read view nor
/// authority to publish. Installation reacquires the exact original writers;
/// the aggregate caller must admit and prepare every component before publishing.
pub struct Detached<V: Value, Admission> {
    predecessor: CapturedPublication,
    mode: BlockMode,
    dirty: bool,
    change: Option<(V, V)>,
    // Release the caller's reservation only after the retained values.
    admission: Admission,
}

impl<V: Value, Admission> Detached<V, Admission> {
    /// Return how the actual original block acquired its predecessor.
    pub fn mode(&self) -> BlockMode {
        self.mode
    }

    /// Return the original block's current-value publication requirement.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Borrow the exact candidate touch; absence still requires clearing undo.
    pub fn touched_value(&self) -> Option<TouchedValue<'_, V>> {
        self.change
            .as_ref()
            .map(|(before, after)| TouchedValue { before, after })
    }

    /// Borrow the resource reservation returned by the admission callback.
    pub fn admission(&self) -> &Admission {
        &self.admission
    }

    /// Observe whether this owner's published current/undo identity still matches.
    ///
    /// This is a momentary observation, not an exclusive publication lease or
    /// permission to reject a decided carrier. The aggregate State owner must
    /// reacquire all component writers and validate their identities together.
    pub fn matches_current(&self, cell: &Cell<V>) -> bool {
        self.predecessor.matches(&cell.publication)
    }

    /// Compare the captured predecessor and mode of an already acquired block.
    ///
    /// Candidate mutations in that block do not change its acquisition identity.
    /// Matching grants no publication capability.
    pub fn matches_block_predecessor(&self, block: &Block<'_, V>) -> bool {
        self.mode == block.mode && self.predecessor.same_as(&block.predecessor)
    }

    /// Prepare this exact delta for publication without making it visible.
    ///
    /// Admission runs before acquiring either writer, because acquiring an EBR
    /// writer itself clones its value. It must cover both current/undo COW,
    /// staging copies, the next identity and retained-reader installation peak.
    /// Both original writers are then acquired without waiting and the exact
    /// captured identity is checked again under those writers. Refusal returns
    /// this unchanged journal; the caller can retain it for a reachable retry.
    ///
    /// The result holds physical writers only for the final synchronous
    /// publication phase. An aggregate caller must prepare every component and
    /// join its external authorization before invoking any component's publish.
    pub fn try_prepare_publication<'target, Installation, E>(
        self,
        target: &'target Cell<V>,
        admit: impl FnOnce(&Self, &Cell<V>) -> Result<Installation, E>,
    ) -> Result<
        PreparedPublication<'target, V, Admission, Installation>,
        (Self, PublicationPreparationError<E>),
    > {
        if !self.predecessor.matches(&target.publication) {
            return Err((self, PublicationPreparationError::Changed));
        }
        let installation = match admit(&self, target) {
            Ok(installation) => installation,
            Err(error) => return Err((self, PublicationPreparationError::Admission(error))),
        };
        let Some(mut revert) = target.revert.try_write() else {
            return Err((self, PublicationPreparationError::Busy));
        };
        let Some(mut blocks) = target.blocks.try_write() else {
            return Err((self, PublicationPreparationError::Busy));
        };
        if !self.predecessor.matches(&target.publication) {
            return Err((self, PublicationPreparationError::Changed));
        }
        let next = NextPublication::new();
        let old_undo = revert.get_mut().take();
        if self.mode == BlockMode::Replace {
            if let Some(value) = old_undo {
                *blocks.get_mut() = value;
            }
        }
        if let Some((before, after)) = &self.change {
            *revert.get_mut() = Some(before.clone());
            *blocks.get_mut() = after.clone();
        }
        Ok(PreparedPublication {
            revert,
            blocks,
            publication: &target.publication,
            next,
            journal: self,
            installation,
        })
    }
}

/// Exact staged current/undo publication with both original writers retained.
/// Drop or [`Self::abort`] publishes nothing. There is no mutable block interface.
#[must_use = "preparation must be published or aborted by its aggregate owner"]
pub struct PreparedPublication<'target, V: Value, Admission, Installation> {
    revert: concread::ebrcell::EbrCellWriteTxn<'target, Option<V>>,
    blocks: concread::ebrcell::EbrCellWriteTxn<'target, V>,
    publication: &'target Publication,
    next: NextPublication,
    journal: Detached<V, Admission>,
    // Release resources only after staged and original values and their writers.
    installation: Installation,
}

impl<V: Value, Admission, Installation> PreparedPublication<'_, V, Admission, Installation> {
    /// Release the installation writers and return the exact original journal.
    pub fn abort(self) -> Detached<V, Admission> {
        let Self {
            revert,
            blocks,
            publication: _,
            next,
            journal,
            installation,
        } = self;
        drop(blocks);
        drop(revert);
        drop(next);
        drop(installation);
        journal
    }

    /// Publish once, returning both reservations to the aggregate owner.
    ///
    /// All fallible semantic preparation has finished. Concread's internal
    /// publication allocations and deferred EBR reclamation must already be
    /// covered by admission; this is not an allocation-free commit guarantee.
    /// The caller owns joint visibility and finality across distinct components.
    pub fn publish(self) -> (Admission, Installation) {
        let Self {
            revert,
            blocks,
            publication,
            next,
            journal,
            installation,
        } = self;
        let Detached {
            predecessor: _,
            mode: _,
            dirty,
            change,
            admission,
        } = journal;
        publication.publish_prepared(next, || {
            if dirty {
                blocks.commit();
            }
            revert.commit();
        });
        drop(change);
        (admission, installation)
    }
}

#[cfg(test)]
#[path = "cell/publication_tests.rs"]
mod publication_tests;

/// Module for [`Block`] and it's related impls
mod block {
    use super::*;
    use concread::ebrcell::EbrCellWriteTxn;
    use std::ops::{Deref, DerefMut};
    /// Batched update to the storage that can be reverted later
    pub struct Block<'storage, V: Value> {
        pub(crate) revert: EbrCellWriteTxn<'storage, Option<V>>,
        pub(crate) blocks: EbrCellWriteTxn<'storage, V>,
        pub(super) dirty: bool,
        pub(super) publication: &'storage Publication,
        pub(super) predecessor: CapturedPublication,
        pub(super) mode: BlockMode,
    }
    impl<'storage, V: Value> Block<'storage, V> {
        pub(super) fn new(
            revert: EbrCellWriteTxn<'storage, Option<V>>,
            blocks: EbrCellWriteTxn<'storage, V>,
            dirty: bool,
            publication: &'storage Publication,
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
        pub fn transaction<'block>(&'block mut self) -> Transaction<'block, 'storage, V>
        where
            'storage: 'block,
        {
            Transaction {
                applied: false,
                dirty_before: self.dirty,
                block: self,
                revert: None,
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
                    blocks.commit();
                }
                revert.commit();
            });
        }

        /// Admit retention, capture this exact delta, and release both writers.
        ///
        /// The callback inspects this immutable block before capture allocates
        /// or clones final values. Its returned reservation lives as long as the
        /// detached owner. Failure drops the original block without publishing.
        /// The preimage is moved; only a touched final value is cloned. Untouched
        /// cells retain identity and mode without copying the value.
        ///
        /// The caller must admit value memory, capture overlap and any future
        /// installation peak. This API does not promise allocation-free capture
        /// or installation and does not itself choose a byte-accounting policy.
        pub fn try_detach<Admission, E>(
            mut self,
            admit: impl FnOnce(&Self) -> Result<Admission, E>,
        ) -> Result<Detached<V, Admission>, E> {
            let admission = admit(&self)?;
            let change = self
                .revert
                .get_mut()
                .take()
                .map(|before| (before, (*self.blocks).clone()));
            let Self {
                revert,
                blocks,
                dirty,
                predecessor,
                mode,
                publication: _,
            } = self;
            drop(blocks);
            drop(revert);
            Ok(Detached {
                predecessor,
                mode,
                dirty,
                change,
                admission,
            })
        }
        /// Read the value before this block's first mutable access.
        ///
        /// Applied children preserve this first preimage; aborted children do
        /// not alter it. For `block_and_revert`, it is the value after undoing
        /// the prior block, not that discarded block's final value.
        pub fn get_before_block(&self) -> &V {
            self.revert.as_ref().unwrap_or_else(|| self.get())
        }

        /// Borrow the exact block preimage and current value, if touched.
        ///
        /// This performs no clone or allocation. An unchanged mutable borrow
        /// still returns `Some`; untouched and aborted-only overlays return
        /// `None`. Reverting a previous block happens before this overlay starts
        /// and does not by itself count as a touch.
        pub fn touched_value(&self) -> Option<TouchedValue<'_, V>> {
            self.revert.as_ref().map(|before| TouchedValue {
                before,
                after: self.get(),
            })
        }

        /// Return the actual acquisition mode, including an untouched replacement.
        pub fn mode(&self) -> BlockMode {
            self.mode
        }

        /// Return whether this block has staged a value mutation.
        pub fn is_dirty(&self) -> bool {
            self.dirty
        }
        /// Get mutable access to the value stored in
        pub fn get_mut(&mut self) -> &mut V {
            let value = self.blocks.get_mut();
            self.revert.get_or_insert(value.clone());
            self.dirty = true;
            value
        }
        /// Read entry from the storage up to certain version non-inclusive
        pub fn get(&self) -> &V {
            &self.blocks
        }
    }
    impl<V: Value> Deref for Block<'_, V> {
        type Target = V;
        fn deref(&self) -> &Self::Target {
            self.get()
        }
    }
    impl<V: Value> DerefMut for Block<'_, V> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            self.get_mut()
        }
    }
    /// Part of block's aggregated changes which applied or aborted at the same time
    pub struct Transaction<'block, 'storage, V: Value> {
        pub(crate) applied: bool,
        pub(crate) dirty_before: bool,
        pub(crate) revert: Option<V>,
        pub(crate) block: &'block mut Block<'storage, V>,
    }
    impl<'block, 'storage: 'block, V: Value> Transaction<'block, 'storage, V> {
        /// Read the value before the parent block's first mutable access.
        ///
        /// Both the block and open transaction journals participate, so a
        /// still-unapplied mutation cannot replace the block preimage.
        pub fn get_before_block(&self) -> &V {
            self.block
                .revert
                .as_ref()
                .or(self.revert.as_ref())
                .unwrap_or_else(|| self.get())
        }

        /// Read the value before this transaction's first mutable access.
        /// Earlier applied siblings are already part of this value.
        pub fn get_before_transaction(&self) -> &V {
            self.revert.as_ref().unwrap_or_else(|| self.get())
        }

        /// Borrow this transaction's preimage and current value, if touched.
        ///
        /// The record is independent of earlier sibling touches. It contains
        /// no copied value, and no-op mutable access is reported explicitly.
        /// Apply preserves the block's original preimage; drop restores this
        /// transaction's `before` value without adding it to the block journal.
        pub fn touched_value(&self) -> Option<TouchedValue<'_, V>> {
            self.revert.as_ref().map(|before| TouchedValue {
                before,
                after: self.get(),
            })
        }

        /// Apply aggregated changes of [`Transaction`] to the [`Block`]
        pub fn apply(mut self) {
            if let Some(prev_value) = core::mem::take(&mut self.revert) {
                self.block.revert.get_or_insert(prev_value);
            }
            self.applied = true;
        }
        /// Get mutable access to the value stored in cell
        pub fn get_mut(&mut self) -> &mut V {
            let value = self.block.blocks.get_mut();
            self.revert.get_or_insert(value.clone());
            self.block.dirty = true;
            value
        }
        /// Read entry from the cell
        pub fn get(&self) -> &V {
            &self.block.blocks
        }
    }
    impl<'block, 'store: 'block, V: Value> Drop for Transaction<'block, 'store, V> {
        fn drop(&mut self) {
            if self.applied {
                return;
            }
            // revert changes made so far by current transaction
            // if transaction was applied set would be empty
            if let Some(prev_value) = core::mem::take(&mut self.revert) {
                *self.block.blocks.get_mut() = prev_value;
            }
            self.block.dirty = self.dirty_before;
        }
    }
    impl<V: Value> Deref for Transaction<'_, '_, V> {
        type Target = V;
        fn deref(&self) -> &Self::Target {
            self.get()
        }
    }
    impl<V: Value> DerefMut for Transaction<'_, '_, V> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            self.get_mut()
        }
    }
}
pub use block::{Block, Transaction};
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn get() {
        let cell = Cell::new(0_u64);
        let view0 = cell.view();
        {
            let mut block = cell.block();
            *block.get_mut() = 1;
            block.commit()
        }
        let view1 = cell.view();
        {
            let mut block = cell.block();
            *block.get_mut() = 2;
            block.commit()
        }
        let view2 = cell.view();
        {
            let mut block = cell.block();
            *block.get_mut() = 3;
            block.commit()
        }
        let view3 = cell.view();
        assert_eq!(view0.get(), &0);
        assert_eq!(view1.get(), &1);
        assert_eq!(view2.get(), &2);
        assert_eq!(view3.get(), &3);
    }
    #[test]
    fn transaction_step() {
        let cell = Cell::new(0_u64);
        let mut block = cell.block();
        // Successful transaction
        {
            let mut transaction = block.transaction();
            *transaction.get_mut() = 1;
            transaction.apply();
        }
        // Aborted step
        {
            let mut transaction = block.transaction();
            *transaction.get_mut() = 2;
        }
        // Check that aborted transaction changes don't visible for subsequent transactions
        {
            let transaction = block.transaction();
            assert_eq!(transaction.get(), &1);
        }
        block.commit();
        // Check that effect of aborted step is not visible in the storage after committing transaction
        {
            let view = cell.view();
            assert_eq!(view.get(), &1);
        }
    }
    #[test]
    fn revert() {
        let cell = Cell::new(0_u64);
        {
            let mut block = cell.block();
            *block.get_mut() = 1;
            block.commit()
        }
        {
            let mut block = cell.block();
            *block.get_mut() = 2;
            block.commit()
        }
        let view1 = cell.view();
        {
            let block = cell.block_and_revert();
            block.commit();
        }
        let view2 = cell.view();
        // View is persistent so revert is not visible
        assert_eq!(view1.get(), &2);
        // Revert is visible in the view created after revert was applied
        assert_eq!(view2.get(), &1);
    }
    #[test]
    fn noop_commit_clears_revert_history() {
        let cell = Cell::new(0_u64);
        {
            let mut block = cell.block();
            *block.get_mut() = 1;
            block.commit();
        }
        {
            let block = cell.block();
            block.commit();
        }
        {
            let block = cell.block_and_revert();
            block.commit();
        }
        let view = cell.view();
        assert_eq!(view.get(), &1);
    }
    #[test]
    fn block_dirty_flag_tracks_staged_mutation() {
        let cell = Cell::new(0_u64);
        let mut block = cell.block();
        assert!(!block.is_dirty());
        assert_eq!(block.get(), &0);
        assert!(
            !block.is_dirty(),
            "read-only access must keep the block clean"
        );
        *block.get_mut() = 1;
        assert!(block.is_dirty());
    }
    #[test]
    fn aborted_transaction_dirty_commit_keeps_state_unchanged() {
        let cell = Cell::new(0_u64);
        {
            let mut block = cell.block();
            {
                let mut transaction = block.transaction();
                *transaction.get_mut() = 1;
            }
            assert!(!block.is_dirty());
            block.commit();
        }
        let view = cell.view();
        assert_eq!(view.get(), &0);
    }
    #[test]
    fn aborted_transaction_preserves_existing_dirty_state() {
        let cell = Cell::new(0_u64);
        {
            let mut block = cell.block();
            *block.get_mut() = 1;
            {
                let mut transaction = block.transaction();
                *transaction.get_mut() = 2;
            }
            assert!(block.is_dirty());
            block.commit();
        }
        let view = cell.view();
        assert_eq!(view.get(), &1);
    }
}

#[cfg(test)]
#[path = "cell/overlay_preimage_tests.rs"]
mod overlay_preimage_tests;

#[cfg(test)]
#[path = "cell/predecessor_view_tests.rs"]
mod predecessor_view_tests;

#[cfg(test)]
#[path = "cell/detached_tests.rs"]
mod detached_tests;
