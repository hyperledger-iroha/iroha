use std::{alloc::Layout, marker::PhantomData};

use concread::ebrcell::{EbrCellOwned, Untracked};

use crate::{
    BlockMode, PublicationPreparationError, PublicationPreparationResult, ReleaseGuard,
    ReleaseNotification, Value,
    publication::{CapturedPublication, NextPublication, Publication},
};
/// Multi-version storage for a single value.
///
/// Charged cells require both original allocation owners before cloning either
/// writer. The untracked convenience API is unavailable in that mode.
/// ```compile_fail
/// use mv::{allocation::AllocationCharge, cell::Cell};
/// fn cannot_skip_admission(cell: &Cell<u64, AllocationCharge>) {
///     let _block = cell.block();
/// }
/// ```
pub struct Cell<V: Value, Charge: Send + Sync + 'static = Untracked> {
    /// Process-local identity of the jointly published current/undo pair.
    pub(crate) publication: Publication,
    pub(crate) revert_released: ReleaseNotification,
    pub(crate) blocks_released: ReleaseNotification,
    /// Previous version of value, required to perform revert of the latest changes
    pub(crate) revert: EbrCell<Option<V>, Charge>,
    /// Value which represent aggregated changes of multiple blocks
    pub(crate) blocks: EbrCell<V, Charge>,
}
/// Prepaid ownership for one current and one undo EBR allocation.
///
/// Both owners must exist before either writer clones its payload. These charges
/// cover only the resources their caller actually admitted; the outer allocation
/// layouts do not include nested values, mutation growth, preimage/capture copies,
/// publication identities, release notifications, or epoch collector bookkeeping.
/// Each charge moves into its actual allocation and survives published readers.
pub struct CellAllocationCharges<Charge> {
    current: Charge,
    undo: Charge,
}

impl<Charge> CellAllocationCharges<Charge> {
    /// Bind the original prepaid owners in current/undo order, without cloning.
    pub fn new(current: Charge, undo: Charge) -> Self {
        Self { current, undo }
    }
}

impl CellAllocationCharges<Untracked> {
    fn untracked() -> Self {
        Self::new(Untracked, Untracked)
    }
}

impl<V: Value> Cell<V> {
    /// Construct an explicitly untracked cell.
    pub fn new(v: V) -> Self {
        Self::new_charged(v, CellAllocationCharges::untracked())
    }

    /// Replace current at the same logical cut, preserving retained undo.
    /// The caller owns cross-field visibility and replacement authorization.
    pub fn replace_current_preserving_predecessor(&self, value: V) {
        self.current_replacement().publish(value);
    }

    /// Acquire both original writers for an untracked same-cut replacement.
    /// Acquire before enclosing publication fences that another block can need.
    pub fn current_replacement(&self) -> CurrentReplacement<'_, V> {
        self.current_replacement_charged(CellAllocationCharges::untracked())
    }

    /// Create an untracked block to aggregate updates.
    pub fn block(&self) -> Block<'_, V> {
        self.block_charged(CellAllocationCharges::untracked())
    }

    /// Undo the published tip before staging an untracked replacement block.
    pub fn block_and_revert(&self) -> Block<'_, V> {
        self.block_and_revert_charged(CellAllocationCharges::untracked())
    }
}

impl<V: Value, Charge: Send + Sync + 'static> Cell<V, Charge> {
    /// Exact requested EBR allocation layouts, in current/undo order.
    ///
    /// These are concrete outer layouts, not estimates of nested or total heap
    /// memory. Reserve both before constructing either writer generation.
    pub fn allocation_layouts() -> [Layout; 2] {
        [
            EbrCell::<V, Charge>::allocation_layout(),
            EbrCell::<Option<V>, Charge>::allocation_layout(),
        ]
    }

    /// Construct current and empty undo with their already prepaid charges.
    /// The caller must separately admit any payload before constructing `v`.
    pub fn new_charged(v: V, charges: CellAllocationCharges<Charge>) -> Self {
        let CellAllocationCharges { current, undo } = charges;
        Self {
            publication: Publication::new(),
            revert_released: ReleaseNotification::default(),
            blocks_released: ReleaseNotification::default(),
            revert: EbrCell::new_charged(None, undo),
            blocks: EbrCell::new_charged(v, current),
        }
    }

    /// Create a persistent view of the current value without cloning it.
    pub fn view(&self) -> View<'_, V> {
        View {
            blocks: self.blocks.read(),
            _marker: PhantomData,
        }
    }

    /// Borrow the retained predecessor of the most recently committed block.
    ///
    /// `None` means that block did not change the value. This immutable EBR view
    /// performs no clone. Consumers joining current and predecessor must bind
    /// both reads to their own publication generation; this is not a joint view.
    pub fn predecessor_view(&self) -> View<'_, Option<V>> {
        View {
            blocks: self.revert.read(),
            _marker: PhantomData,
        }
    }

    /// Acquire the original current/undo pair for a same-cut replacement.
    ///
    /// Both charges are owned before either clone. Acquire before enclosing
    /// publication fences that another block can need. Abandonment changes no
    /// value or publication identity. The replacement payload needs separate
    /// admission; this method accounts for the requested EBR allocations only.
    pub fn current_replacement_charged(
        &self,
        charges: CellAllocationCharges<Charge>,
    ) -> CurrentReplacement<'_, V, Charge> {
        let (revert, blocks) = self.acquire_charged_writers(charges);
        CurrentReplacement {
            blocks,
            _revert: revert,
            publication: &self.publication,
        }
    }

    /// Create a block using a prepaid current/undo pair before either clone.
    /// Preimage copies and later payload growth require separate admission.
    pub fn block_charged(&self, charges: CellAllocationCharges<Charge>) -> Block<'_, V, Charge> {
        let (mut revert, blocks) = self.acquire_charged_writers(charges);
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

    /// Undo the published tip before staging a replacement with prepaid owners.
    /// The original undo/current semantics and writer order remain unchanged.
    pub fn block_and_revert_charged(
        &self,
        charges: CellAllocationCharges<Charge>,
    ) -> Block<'_, V, Charge> {
        let (mut revert, mut blocks) = self.acquire_charged_writers(charges);
        let predecessor = self.publication.capture();
        if let Some(revert) = core::mem::take(revert.get_mut()) {
            *blocks.get_mut() = revert;
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

    fn acquire_charged_writers(
        &self,
        charges: CellAllocationCharges<Charge>,
    ) -> (CellWriter<'_, Option<V>, Charge>, CellWriter<'_, V, Charge>) {
        let CellAllocationCharges { current, undo } = charges;
        let revert = self
            .revert_released
            .with_acquisition_unwind_notification(|| {
                self.revert
                    .write_charged(|_, _| Ok::<_, std::convert::Infallible>(undo))
                    .unwrap_or_else(|never| match never {})
            });
        let revert = self.revert_released.poisoning_guard(revert);
        let blocks = self
            .blocks_released
            .with_acquisition_unwind_notification(|| {
                self.blocks
                    .write_charged(|_, _| Ok::<_, std::convert::Infallible>(current))
                    .unwrap_or_else(|never| match never {})
            });
        let blocks = self.blocks_released.poisoning_guard(blocks);
        (revert, blocks)
    }
}

type CellWriter<'storage, V, Charge> =
    ReleaseGuard<'storage, concread::ebrcell::EbrCellWriteTxn<'storage, V, Charge>>;

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

/// Owned writers for a current-value replacement which preserves the exact undo.
/// This is physical custody only, not authorization for a new block or State.
#[must_use = "retain this owner until same-cut publication or abandonment"]
pub struct CurrentReplacement<'storage, V: Value, Charge: Send + Sync + 'static = Untracked> {
    // Drop the current writer before the outer undo writer on abandonment.
    blocks: CellWriter<'storage, V, Charge>,
    _revert: CellWriter<'storage, Option<V>, Charge>,
    publication: &'storage Publication,
}

impl<V: Value, Charge: Send + Sync + 'static> CurrentReplacement<'_, V, Charge> {
    /// Borrow the original current value while both writers remain held.
    pub fn get(&self) -> &V {
        &self.blocks
    }

    /// Publish the replacement once without changing or clearing retained undo.
    /// The caller must already hold its complete cross-field visibility boundary.
    pub fn publish(self, value: V) {
        let Self {
            _revert,
            mut blocks,
            publication,
        } = self;
        *blocks.get_mut() = value;
        publication.publish(|| blocks.release_with(|guard| guard.commit()));
        drop(_revert);
    }
}

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

/// Original current/undo successor allocations without their execution writers.
///
/// This move-only owner retains the exact payloads, allocation charges, original
/// predecessor identity and next publication identity. It holds no EBR pin or
/// storage lock. Reacquisition never reconstructs or clones a successor; only the
/// enclosing aggregate owner can supply cross-component publication authority.
pub struct Detached<V: Value, Admission, Charge: Send + Sync + 'static = Untracked> {
    revert: EbrCellOwned<Option<V>, Charge>,
    blocks: EbrCellOwned<V, Charge>,
    // Capacity outlives both original payload allocations on abandonment.
    metadata: DetachedMetadata<Admission>,
}

struct DetachedMetadata<Admission> {
    predecessor: CapturedPublication,
    mode: BlockMode,
    dirty: bool,
    next: NextPublication,
    admission: Admission,
}

impl<V: Value, Admission, Charge: Send + Sync + 'static> Detached<V, Admission, Charge> {
    /// Return how the original block acquired its predecessor.
    pub fn mode(&self) -> BlockMode {
        self.metadata.mode
    }

    /// Return the original block's current-value publication requirement.
    pub fn is_dirty(&self) -> bool {
        self.metadata.dirty
    }

    /// Borrow the exact retained preimage and successor; this never clones.
    pub fn touched_value(&self) -> Option<TouchedValue<'_, V>> {
        self.revert.as_ref().map(|before| TouchedValue {
            before,
            after: &self.blocks,
        })
    }

    /// Borrow the separate reservation returned before original-owner capture.
    /// Actual current/undo allocation charges remain inside their own allocations.
    pub fn admission(&self) -> &Admission {
        &self.metadata.admission
    }

    /// Observe whether this owner's original published pair still matches.
    /// This momentary observation grants no publication authority.
    pub fn matches_current(&self, cell: &Cell<V, Charge>) -> bool {
        self.metadata.predecessor.matches(&cell.publication)
    }

    /// Compare the original predecessor and mode of an already acquired block.
    pub fn matches_block_predecessor(&self, block: &Block<'_, V, Charge>) -> bool {
        self.metadata.mode == block.mode && self.metadata.predecessor.same_as(&block.predecessor)
    }

    /// Reacquire both original writers around the exact owned successors.
    ///
    /// No successor clone or generation allocation occurs. The original charges
    /// and prebuilt publication identity survive every refusal and abort. The
    /// callback may retain separately prepaid temporary installation resources;
    /// it must not refund or reacquire the actual generation charges.
    ///
    /// Original predecessor identity is checked before admission and again under
    /// both writers. Every refusal returns the same payload and allocation owners.
    /// An aggregate caller must join all components and external authorization
    /// before invoking any component's publish.
    pub fn try_prepare_publication<'target, Installation, E>(
        self,
        target: &'target Cell<V, Charge>,
        admit: impl FnOnce(&Self, &Cell<V, Charge>) -> Result<Installation, E>,
    ) -> PublicationPreparationResult<
        PreparedPublication<'target, V, Admission, Installation, Charge>,
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
            Err(blocks) => {
                let error = if target.blocks.is_poisoned() {
                    PublicationPreparationError::Poisoned
                } else {
                    PublicationPreparationError::after_failed_acquisition(wait)
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

/// Original successors held under both exact target writers.
/// Drop abandons them without publication; abort returns the same allocations.
#[must_use = "preparation must be published or aborted by its aggregate owner"]
pub struct PreparedPublication<
    'target,
    V: Value,
    Admission,
    Installation,
    Charge: Send + Sync + 'static = Untracked,
> {
    revert: CellWriter<'target, Option<V>, Charge>,
    blocks: CellWriter<'target, V, Charge>,
    publication: &'target Publication,
    metadata: DetachedMetadata<Admission>,
    // Release temporary resources after the original writers and payloads.
    installation: Installation,
}

impl<V: Value, Admission, Installation, Charge: Send + Sync + 'static>
    PreparedPublication<'_, V, Admission, Installation, Charge>
{
    /// Release both writer locks and return the exact original allocation owners.
    pub fn abort(self) -> Detached<V, Admission, Charge> {
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

    /// Publish once, returning capture and temporary installation reservations.
    ///
    /// Actual generation charges stay inside the published/retired allocations.
    /// The original next identity was allocated before detachment. Collector and
    /// other control bookkeeping still require their own admission; this method
    /// makes no complete heap-budget or allocation-free commit guarantee.
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
#[path = "cell/publication_tests.rs"]
mod publication_tests;

/// Module for [`Block`] and it's related impls
mod block {
    use super::*;
    use std::ops::{Deref, DerefMut};
    /// Batched update to the storage that can be reverted later
    pub struct Block<'storage, V: Value, Charge: Send + Sync + 'static = Untracked> {
        pub(crate) revert: CellWriter<'storage, Option<V>, Charge>,
        pub(crate) blocks: CellWriter<'storage, V, Charge>,
        pub(super) dirty: bool,
        pub(super) publication: &'storage Publication,
        pub(super) predecessor: CapturedPublication,
        pub(super) mode: BlockMode,
    }
    impl<'storage, V: Value, Charge: Send + Sync + 'static> Block<'storage, V, Charge> {
        pub(super) fn new(
            revert: CellWriter<'storage, Option<V>, Charge>,
            blocks: CellWriter<'storage, V, Charge>,
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
        pub fn transaction<'block>(&'block mut self) -> Transaction<'block, 'storage, V, Charge>
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
                    blocks.release_with(|guard| guard.commit());
                }
                revert.release_with(|guard| guard.commit());
            });
        }

        /// Admit metadata retention, then release writers around their original allocations.
        ///
        /// Both successor payloads and charges move into the detached owner
        /// without cloning. The next publication identity is allocated after
        /// admission and retained across installation retries. Payload allocation
        /// custody was already required before this block's original acquisition;
        /// this callback cannot retroactively fund execution or nested values.
        pub fn try_detach<Admission, E>(
            self,
            admit: impl FnOnce(&Self) -> Result<Admission, E>,
        ) -> Result<Detached<V, Admission, Charge>, E> {
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

        /// Observe this block's original owner, current/undo predecessor and mode.
        /// The opaque identity permits only local equality, never publication.
        pub fn publication_identity(&self) -> crate::BlockPublicationIdentity {
            crate::BlockPublicationIdentity::capture(&self.predecessor, self.mode)
        }

        /// Check the original cell owner without reading values or taking locks.
        /// This observation grants no mutation or publication authority.
        pub fn belongs_to(&self, cell: &Cell<V>) -> bool {
            self.predecessor.belongs_to(&cell.publication)
        }

        /// Return whether this block has staged a value mutation.
        pub fn is_dirty(&self) -> bool {
            self.dirty
        }
        /// Get mutable access to the value stored in
        pub fn get_mut(&mut self) -> &mut V {
            let value = self.blocks.get_mut();
            self.revert.get_or_insert_with(|| value.clone());
            self.dirty = true;
            value
        }
        /// Read entry from the storage up to certain version non-inclusive
        pub fn get(&self) -> &V {
            &self.blocks
        }
    }
    impl<V: Value, Charge: Send + Sync + 'static> Deref for Block<'_, V, Charge> {
        type Target = V;
        fn deref(&self) -> &Self::Target {
            self.get()
        }
    }
    impl<V: Value, Charge: Send + Sync + 'static> DerefMut for Block<'_, V, Charge> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            self.get_mut()
        }
    }
    /// Part of block's aggregated changes which applied or aborted at the same time
    pub struct Transaction<'block, 'storage, V: Value, Charge: Send + Sync + 'static = Untracked> {
        pub(crate) applied: bool,
        pub(crate) dirty_before: bool,
        pub(crate) revert: Option<V>,
        pub(crate) block: &'block mut Block<'storage, V, Charge>,
    }
    impl<'block, 'storage: 'block, V: Value, Charge: Send + Sync + 'static>
        Transaction<'block, 'storage, V, Charge>
    {
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
            self.revert.get_or_insert_with(|| value.clone());
            self.block.dirty = true;
            value
        }
        /// Read entry from the cell
        pub fn get(&self) -> &V {
            &self.block.blocks
        }
    }
    impl<'block, 'store: 'block, V: Value, Charge: Send + Sync + 'static> Drop
        for Transaction<'block, 'store, V, Charge>
    {
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
    impl<V: Value, Charge: Send + Sync + 'static> Deref for Transaction<'_, '_, V, Charge> {
        type Target = V;
        fn deref(&self) -> &Self::Target {
            self.get()
        }
    }
    impl<V: Value, Charge: Send + Sync + 'static> DerefMut for Transaction<'_, '_, V, Charge> {
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

#[cfg(test)]
#[path = "cell/charged_allocation_tests.rs"]
mod charged_allocation_tests;
