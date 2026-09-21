//! Original finite credits for the fixed-width canonical hash journal.

use super::*;
#[cfg(test)]
#[path = "block_hashes_admission_tests.rs"]
mod tests;
use concread::bptree::{
    AllocationDemand, ClonePlanning, MapAdmissionError, NodeCloning, NodeFunding, PlanningError,
};

/// Separate local acquisition refusal from the caller's deterministic start stage.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum StateBlockStartError<E: std::fmt::Debug> {
    /// No World owner or start effect was acquired before this local refusal.
    #[error(transparent)]
    History(#[from] BlockHashAdmissionError),
    /// The caller's original pristine/after-start failure.
    #[error("block start stage failed: {0:?}")]
    Stage(E),
}
impl From<StateBlockStartError<MergeLedgerCommitError>> for MergeLedgerCommitError {
    fn from(error: StateBlockStartError<MergeLedgerCommitError>) -> Self {
        match error {
            StateBlockStartError::History(error) => Self::BlockHashAdmission(error),
            StateBlockStartError::Stage(error) => error,
        }
    }
}

/// Local history acquisition failure; never a verdict on a block or transaction.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum BlockHashAdmissionError {
    /// Another physical owner must release the original history lock.
    #[error("canonical hash history is busy")]
    Busy(mv::ReleaseWait),
    /// The configured finite pool refused the complete operation.
    #[error("canonical hash history capacity: {0}")]
    Capacity(mv::allocation::AllocationRefusal),
    /// A checked layout or generation cannot be represented.
    #[error("canonical hash history planning failed: {0:?}")]
    Planning(PlanningError),
    /// An earlier failed edit requires Strict restart/recovery.
    #[error("canonical hash history is poisoned")]
    Poisoned,
    /// The observed original generation was replaced before acquisition.
    #[error("canonical hash history predecessor changed")]
    Changed(mv::ReleaseWait),
    /// Emergency Fast startup does not permit a successor.
    #[error("emergency Fast history is read-only; restart in Strict mode")]
    ReadOnly,
}

impl BlockHashAdmissionError {
    /// The original release observation, only when releasing another owner can help.
    pub fn release_wait(&self) -> Option<&mv::ReleaseWait> {
        match self {
            Self::Busy(wait) | Self::Changed(wait) => Some(wait),
            Self::Capacity(mv::allocation::AllocationRefusal::Capacity { release, .. }) => {
                Some(release)
            }
            _ => None,
        }
    }
}
impl<E: std::fmt::Debug> StateBlockStartError<E> {
    /// Preserve the original history release without retrying a deterministic stage failure.
    pub fn release_wait(&self) -> Option<&mv::ReleaseWait> {
        match self {
            Self::History(error) => error.release_wait(),
            Self::Stage(_) => None,
        }
    }
}

/// Concrete payloads contain no nested allocation or destructor-owned storage.
pub(super) struct BlockHashPolicy(mv::allocation::AllocationReservation);
impl NodeFunding for BlockHashPolicy {
    type Charge = mv::allocation::AllocationCharge;
    fn take_node_charge(&mut self, layout: std::alloc::Layout) -> Self::Charge {
        self.0
            .try_split(layout)
            .expect("complete concrete hash edit plan")
    }
}
impl NodeCloning<usize, HashOf<BlockHeader>> for BlockHashPolicy {
    fn clone_key(&mut self, key: &usize) -> usize {
        *key
    }
    fn clone_value(&mut self, value: &HashOf<BlockHeader>) -> HashOf<BlockHeader> {
        *value
    }
}
impl ClonePlanning<usize, HashOf<BlockHeader>> for BlockHashPolicy {
    fn plan_key(_: &usize, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Ok(())
    }
    fn plan_value(_: &HashOf<BlockHeader>, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Ok(())
    }
}
impl BlockHashes {
    fn admit_successor(
        &self,
        existing: AllocationDemand,
        additional: AllocationDemand,
    ) -> Result<BlockHashPolicy, mv::allocation::AllocationRefusal> {
        let required = existing
            .bytes()
            .checked_add(additional.bytes())
            .ok_or(mv::allocation::AllocationRefusal::DemandOverflow)?;
        if required > self.budget.limit_bytes() {
            return Err(mv::allocation::AllocationRefusal::ExceedsLimit {
                requested_bytes: required,
                limit_bytes: self.budget.limit_bytes(),
            });
        }
        self.admit(additional)
    }
    pub(super) fn admit(
        &self,
        demand: AllocationDemand,
    ) -> Result<BlockHashPolicy, mv::allocation::AllocationRefusal> {
        self.budget
            .try_reserve_bytes(demand.bytes())
            .map(BlockHashPolicy)
    }
    fn admission_error(
        &self,
        error: MapAdmissionError<mv::allocation::AllocationRefusal>,
        wait: mv::ReleaseWait,
    ) -> BlockHashAdmissionError {
        match error {
            MapAdmissionError::Busy => BlockHashAdmissionError::Busy(wait),
            MapAdmissionError::Poisoned => BlockHashAdmissionError::Poisoned,
            MapAdmissionError::Changed => BlockHashAdmissionError::Changed(wait),
            MapAdmissionError::Planning(error) => BlockHashAdmissionError::Planning(error),
            MapAdmissionError::Refused(error) => BlockHashAdmissionError::Capacity(error),
        }
    }
    /// Restore exact ordered history with every tree allocation charged to this pool.
    /// The input sequence is owned by the caller's authenticated snapshot/replay scope.
    pub(crate) fn try_new(
        initial: impl IntoIterator<Item = HashOf<BlockHeader>>,
        budget: mv::allocation::AllocationBudget,
    ) -> Result<Self, BlockHashAdmissionError> {
        budget.with_deferred_refund_notifications(|| {
            let map = BlockHashMap::try_new_with_node_custody(|demand| {
                budget
                    .try_reserve_bytes(demand.bytes())
                    .map(BlockHashPolicy)
            })
            .map_err(BlockHashAdmissionError::Capacity)?;
            let owner = Self {
                inner: BlockHashStorage::Owned(map),
                budget: budget.clone(),
                released: mv::ReleaseNotification::default(),
                committed_height: AtomicUsize::new(0),
            };
            for (index, hash) in initial.into_iter().enumerate() {
                let map = owner.map().expect("new mutable history");
                let wait = owner.released.observe();
                let (work, old) = map
                    .try_insert_admitted_with_footprint(index, hash, |existing, additional| {
                        owner.admit_successor(existing, additional)
                    })
                    .map_err(|(_, error)| owner.admission_error(error, wait))?;
                debug_assert!(old.is_none());
                let writer = map
                    .try_write_owned(work)
                    .unwrap_or_else(|_| unreachable!("exclusive cold history"));
                drop(writer.prepare_commit().publish().release());
                owner.committed_height.store(index + 1, Ordering::Release);
            }
            Ok(owner)
        })
    }

    /// Prepay and create the entire successor before any World execution begins.
    /// The private tip remains hidden until `push` installs the exact final hash.
    pub(crate) fn try_next_block(
        &self,
        replacement: bool,
    ) -> Result<BlockHashesBlock<'_>, BlockHashAdmissionError> {
        self.budget.with_deferred_refund_notifications(|| {
            let map = self.map().ok_or(BlockHashAdmissionError::ReadOnly)?;
            let wait = self.released.observe();
            let view = self.try_view().map_err(|error| match error {
                concread::bptree::OwnedWriteError::Busy => {
                    BlockHashAdmissionError::Busy(wait.clone())
                }
                concread::bptree::OwnedWriteError::Poisoned => BlockHashAdmissionError::Poisoned,
                concread::bptree::OwnedWriteError::Changed => {
                    BlockHashAdmissionError::Changed(wait.clone())
                }
            })?;
            let prefix = if replacement {
                view.len().saturating_sub(1)
            } else {
                view.len()
            };
            let predecessor = match &view.inner {
                BlockHashesViewInner::Owned(view) => view.predecessor().retain(),
                BlockHashesViewInner::Mapped(_) => unreachable!("mutable map checked above"),
            };
            drop(view);
            let wait = self.released.observe();
            let result = map.try_insert_admitted_with_footprint(
                prefix,
                HashOf::from_untyped_unchecked(Hash::prehashed([0; 32])),
                |existing, additional| self.admit_successor(existing, additional),
            );
            if !matches!(
                result,
                Err((_, MapAdmissionError::Busy | MapAdmissionError::Poisoned))
            ) {
                drop(self.released.guard(()));
            }
            let (work, _) =
                result.map_err(|(_, error)| self.admission_error(error, wait.clone()))?;
            if !predecessor.matches(&work.predecessor()) {
                return Err(BlockHashAdmissionError::Changed(wait));
            }
            Ok(BlockHashesBlock {
                inner: self,
                work,
                visible_len: prefix,
                reserved_tip: Some(prefix),
                fixture_edits: false,
                mode: if replacement {
                    mv::BlockMode::Replace
                } else {
                    mv::BlockMode::Ordinary
                },
            })
        })
    }
}
