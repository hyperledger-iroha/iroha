//! Original-pool custody for the fixed per-cycle register/memory root rows.
//!
//! This owner covers only `StepEntry` backing. Register events, Merkle paths,
//! delta traces and host scratch have independent allocation boundaries.

use super::StepEntry;
use crate::{
    VMError,
    cache_memory::{OwnedVec, OwnedVecGrowthError},
    error::ExecutionDeferral,
    execution_memory::{ExecutionBuffer, ExecutionMemoryLease, ExecutionMemoryPlan},
};
use iroha_allocation::{
    AllocationBudget, AllocationRefusal, ChargedBufferError, PrepaidBufferError,
};
use iroha_crypto::{HashOf, MerkleTree, zeroize_value_for_confidential_discard as erase};

/// A cycle-root buffer bound to the VM's original pool. The explicit None case
/// is the existing untracked standalone boundary, never a replacement pool.
pub struct StepLog {
    rows: Rows,
    original: Option<AllocationBudget>,
}

enum Rows {
    Funded(Option<ExecutionBuffer<StepEntry>>),
    Standalone(OwnedVec<StepEntry>),
}

impl Rows {
    fn empty(funded: bool) -> Self {
        if funded {
            Self::Funded(None)
        } else {
            Self::Standalone(OwnedVec::default())
        }
    }
    fn as_slice(&self) -> &[StepEntry] {
        match self {
            Self::Funded(rows) => rows.as_ref().map_or(&[], ExecutionBuffer::as_slice),
            Self::Standalone(rows) => rows,
        }
    }
    fn as_mut_slice(&mut self) -> &mut [StepEntry] {
        match self {
            Self::Funded(rows) => rows.as_mut().map_or(&mut [], ExecutionBuffer::as_mut_slice),
            Self::Standalone(rows) => rows,
        }
    }
    fn capacity(&self) -> usize {
        match self {
            Self::Funded(rows) => rows.as_ref().map_or(0, ExecutionBuffer::capacity),
            Self::Standalone(rows) => rows.capacity(),
        }
    }
    fn push_reserved(&mut self, row: StepEntry) {
        match self {
            Self::Funded(rows) => rows
                .as_mut()
                .expect("prepaid cycle root rows")
                .push_reserved(row),
            Self::Standalone(rows) => rows.insert_reserved(rows.len(), row),
        }
    }
    fn scrub(&mut self) {
        for row in self.as_mut_slice() {
            erase(&mut row.pc);
            // HashOf dereferences to its original inline Hash, which supports
            // volatile erasure. The discarded value is never read afterwards.
            erase(&mut *row.reg_root);
            erase(&mut *row.mem_root);
        }
    }
    fn clear(&mut self) {
        self.scrub();
        match self {
            Self::Funded(rows) => {
                if let Some(rows) = rows {
                    rows.truncate(0);
                }
            }
            Self::Standalone(rows) => rows.clear(),
        }
    }
}
impl Drop for Rows {
    fn drop(&mut self) {
        self.scrub();
    }
}

impl StepLog {
    /// Bind an empty log to the existing VM allocation owner.
    pub fn new(original: Option<&AllocationBudget>) -> Self {
        Self {
            rows: Rows::empty(original.is_some()),
            original: original.cloned(),
        }
    }

    /// Admit complete additional cycle rows before instruction/padding effects.
    /// Old backing remains charged throughout allocation and copying. Publication
    /// precedes retirement, so any refund callback sees the intact new owner.
    pub fn prepare_cycles(&mut self, cycles: u64) -> Result<(), VMError> {
        let additional = usize::try_from(cycles).map_err(|_| overflow())?;
        let needed = self
            .as_slice()
            .len()
            .checked_add(additional)
            .ok_or_else(overflow)?;
        if needed <= self.rows.capacity() {
            return Ok(());
        }
        let capacity = self
            .rows
            .capacity()
            .checked_mul(2)
            .ok_or_else(overflow)?
            .max(needed)
            .max(4);
        let plan = ExecutionMemoryPlan::array::<StepEntry>(capacity)
            .map_err(VMError::AllocationDeferred)?;
        let mut replacement = if let Some(original) = &self.original {
            let mut lease = ExecutionMemoryLease::reserve(original, plan)
                .map_err(VMError::AllocationDeferred)?;
            #[cfg(test)]
            if REFUSE_NEXT_ALLOCATION.replace(false) {
                return Err(unavailable());
            }
            Rows::Funded(Some(
                ExecutionBuffer::new(capacity, &mut lease).map_err(prepaid_error)?,
            ))
        } else {
            let Rows::Standalone(rows) = &self.rows else {
                unreachable!("original backing identity");
            };
            Rows::Standalone(rows.try_copy_capacity(capacity).map_err(local_error)?)
        };
        for row in self.as_slice() {
            replacement.push_reserved(row.clone());
        }
        let retired = std::mem::replace(&mut self.rows, replacement);
        drop(retired);
        Ok(())
    }

    /// Append only after the instruction/padding owner admitted its full cycle count.
    pub fn record_reserved(
        &mut self,
        pc: u64,
        reg_root: HashOf<MerkleTree<[u8; 32]>>,
        mem_root: HashOf<MerkleTree<[u8; 32]>>,
    ) {
        self.rows.push_reserved(StepEntry {
            pc,
            reg_root,
            mem_root,
        });
    }

    /// Borrow initialized rows while their original owner remains live.
    pub fn as_slice(&self) -> &[StepEntry] {
        self.rows.as_slice()
    }

    /// Exact retained capacity, including spare prepaid rows.
    pub fn allocated_bytes(&self) -> Result<usize, VMError> {
        ExecutionMemoryPlan::array::<StepEntry>(self.rows.capacity())
            .map(|plan| plan.requested_bytes())
            .map_err(VMError::AllocationDeferred)
    }

    /// Erase initialized rows while preserving their original admitted backing.
    pub fn clear(&mut self) {
        self.rows.clear();
    }

    /// Retire a runtime's old row storage while retaining its original budget
    /// for the next run. No reset can silently revert a State VM to standalone.
    pub fn reset(&mut self) {
        let retired = std::mem::replace(&mut self.rows, Rows::empty(self.original.is_some()));
        drop(retired);
    }

    /// Test-only independent copy, admitted from the same original pool.
    #[cfg(test)]
    pub fn try_clone_allocation(&self) -> Result<Self, VMError> {
        let mut copy = Self::new(self.original.as_ref());
        copy.prepare_cycles(self.as_slice().len() as u64)?;
        for row in self.as_slice() {
            copy.rows.push_reserved(row.clone());
        }
        Ok(copy)
    }
}

fn overflow() -> VMError {
    VMError::AllocationDeferred(AllocationRefusal::DemandOverflow)
}
fn unavailable() -> VMError {
    VMError::ExecutionDeferred(ExecutionDeferral::AllocationUnavailable)
}
fn local_error(error: OwnedVecGrowthError) -> VMError {
    match error {
        OwnedVecGrowthError::CapacityOverflow => overflow(),
        OwnedVecGrowthError::AllocationUnavailable => unavailable(),
    }
}
fn prepaid_error(error: PrepaidBufferError) -> VMError {
    match error {
        PrepaidBufferError::Allocation(ChargedBufferError::Admission(refusal)) => {
            VMError::AllocationDeferred(refusal)
        }
        PrepaidBufferError::Allocation(ChargedBufferError::Allocator { .. })
        | PrepaidBufferError::Reservation(_) => unavailable(),
    }
}

#[cfg(test)]
thread_local! { static REFUSE_NEXT_ALLOCATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }

#[cfg(test)]
mod tests;
