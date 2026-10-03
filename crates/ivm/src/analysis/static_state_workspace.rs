//! Original-pool facts and a bounded FIFO for optional static-state dataflow.
//!
//! One fixed row belongs to each admitted instruction. A changed row is queued
//! at most once until popped; merging while queued updates that same row before
//! it is read. No root vector, PC map or growing work queue is needed.
//! Borrowed literal indexes, canonical NFC scratch, symbolic key scratch and
//! immutable result keys use their respective original funded owners.

use super::StaticStateFacts;
use crate::{
    PreparedContract, VMError,
    error::ExecutionDeferral,
    execution_memory::{ExecutionBuffer, ExecutionMemoryLease, ExecutionMemoryPlan},
    metadata::EmbeddedEntrypointDescriptor,
};
use iroha_allocation::{AllocationBudget, ChargedBufferError, PrepaidBufferError};

/// Original metadata supplies either one selected root or all declared roots.
pub(super) struct Roots<'a> {
    descriptors: &'a [EmbeddedEntrypointDescriptor],
}
impl<'a> Roots<'a> {
    pub(super) fn new(contract: &'a PreparedContract, name: Option<&str>) -> Option<Self> {
        let descriptors = match name {
            Some(name) => std::slice::from_ref(contract.entrypoint_descriptor(name)?),
            None => contract.contract_interface().entrypoints.as_slice(),
        };
        (!descriptors.is_empty()).then_some(Self { descriptors })
    }

    pub(super) fn iter(&self) -> impl ExactSizeIterator<Item = u64> + '_ {
        self.descriptors
            .iter()
            .map(|descriptor| descriptor.entry_pc)
    }

    pub(super) fn contains(&self, pc: u64) -> bool {
        self.iter().any(|root| root == pc)
    }
}

struct Row {
    facts: Option<StaticStateFacts>,
    queued: bool,
}

/// Fixed instruction-indexed incoming states plus a non-growing ring queue.
/// The two backing allocations keep their original charges through final drop.
pub(super) struct Workspace {
    rows: ExecutionBuffer<Row>,
    queue: ExecutionBuffer<usize>,
    head: usize,
    pending: usize,
}
impl Workspace {
    pub(super) fn new(instructions: usize, budget: &AllocationBudget) -> Result<Self, VMError> {
        let mut plan =
            ExecutionMemoryPlan::array::<Row>(instructions).map_err(VMError::AllocationDeferred)?;
        plan.include_child(
            ExecutionMemoryPlan::array::<usize>(instructions)
                .map_err(VMError::AllocationDeferred)?,
        )
        .map_err(VMError::AllocationDeferred)?;
        // Admit the combined exact layouts before allocating either backing.
        let mut lease =
            ExecutionMemoryLease::reserve(budget, plan).map_err(VMError::AllocationDeferred)?;
        let mut rows = ExecutionBuffer::new(instructions, &mut lease).map_err(buffer_error)?;
        let mut queue = ExecutionBuffer::new(instructions, &mut lease).map_err(buffer_error)?;
        for _ in 0..instructions {
            rows.push_reserved(Row {
                facts: None,
                queued: false,
            });
            queue.push_reserved(0);
        }
        Ok(Self {
            rows,
            queue,
            head: 0,
            pending: 0,
        })
    }

    /// Preserve the existing fixed-point merge and FIFO discovery order.
    /// Repeated changes to a pending row need no duplicate queue entries: its
    /// latest merged value is read when that original entry is popped.
    pub(super) fn merge(
        &mut self,
        index: usize,
        incoming: &StaticStateFacts,
    ) -> Result<bool, VMError> {
        let row = self
            .rows
            .as_mut_slice()
            .get_mut(index)
            .ok_or(VMError::DecodeError)?;
        let changed = match &mut row.facts {
            Some(facts) => facts.merge_from(incoming),
            None => {
                row.facts = Some(incoming.clone());
                true
            }
        };
        if changed && !row.queued {
            let capacity = self.queue.as_slice().len();
            // Every pending entry has a distinct marked row. An unmarked row
            // therefore guarantees a free queue slot, including after wrap.
            if self.pending == capacity {
                return Err(VMError::DecodeError);
            }
            let tail = (self.head + self.pending) % capacity;
            self.queue.as_mut_slice()[tail] = index;
            self.pending += 1;
            row.queued = true;
        }
        Ok(changed)
    }

    /// Take one current merged state; clearing its flag permits a later change
    /// (including a back edge) to schedule the same instruction again.
    pub(super) fn pop(&mut self) -> Option<(usize, StaticStateFacts)> {
        if self.pending == 0 {
            return None;
        }
        let index = self.queue.as_slice()[self.head];
        self.head = (self.head + 1) % self.queue.as_slice().len();
        self.pending -= 1;
        let row = &mut self.rows.as_mut_slice()[index];
        row.queued = false;
        Some((
            index,
            row.facts
                .as_ref()
                .expect("queued row owns incoming facts")
                .clone(),
        ))
    }
}

fn buffer_error(error: PrepaidBufferError) -> VMError {
    match error {
        PrepaidBufferError::Allocation(ChargedBufferError::Admission(error)) => {
            VMError::AllocationDeferred(error)
        }
        PrepaidBufferError::Allocation(ChargedBufferError::Allocator { .. })
        | PrepaidBufferError::Reservation(_) => {
            VMError::ExecutionDeferred(ExecutionDeferral::AllocationUnavailable)
        }
    }
}

#[cfg(test)]
mod tests;
