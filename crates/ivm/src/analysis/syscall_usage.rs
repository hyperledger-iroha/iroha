//! Exact syscall histogram storage and original-pool scratch custody.

use super::SyscallUsage;
use crate::{
    VMError,
    cache_memory::{OwnedAllocation, SharedAllocation},
    error::ExecutionDeferral,
    execution_memory::{ExecutionBuffer, ExecutionMemoryLease, ExecutionMemoryPlan},
};
use iroha_allocation::{AllocationBudget, ChargedBufferError, PrepaidBufferError};
use std::ops::Deref;

/// Immutable syscall histogram whose backing stays charged through its final borrower.
/// Empty histograms need no shared shell. Cloning never copies the histogram.
#[derive(Clone, Debug, Default)]
pub struct SyscallUsages(Option<SharedAllocation<SyscallUsage>>);

impl SyscallUsages {
    /// Attempt nonblocking retention of the original shared allocation.
    pub fn try_retain(&self) -> bool {
        self.0.as_ref().is_none_or(SharedAllocation::try_retain)
    }

    /// Transfer this handle into a cache without copying or charging it twice.
    #[must_use]
    pub fn into_cache_owner(self) -> Self {
        Self(self.0.map(SharedAllocation::into_cache_owner))
    }
}

impl Deref for SyscallUsages {
    type Target = [SyscallUsage];
    fn deref(&self) -> &Self::Target {
        self.0.as_deref().unwrap_or(&[])
    }
}

impl PartialEq for SyscallUsages {
    fn eq(&self, other: &Self) -> bool {
        self.deref() == other.deref()
    }
}
impl Eq for SyscallUsages {}

impl<'a> IntoIterator for &'a SyscallUsages {
    type Item = &'a SyscallUsage;
    type IntoIter = std::slice::Iter<'a, SyscallUsage>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

enum Scratch {
    Local(OwnedAllocation<u32>),
    Funded(ExecutionBuffer<u32>),
}

/// One slot per syscall occurrence, funded before any instruction is accumulated.
pub(super) struct UsageScratch {
    storage: Scratch,
    used: usize,
    capacity: usize,
    budget: Option<AllocationBudget>,
}

impl UsageScratch {
    pub(super) fn new(
        occurrences: usize,
        budget: Option<&AllocationBudget>,
    ) -> Result<Self, VMError> {
        let storage = match budget {
            Some(budget) => {
                let plan = ExecutionMemoryPlan::array::<u32>(occurrences)
                    .map_err(VMError::AllocationDeferred)?;
                let mut lease = ExecutionMemoryLease::reserve(budget, plan)
                    .map_err(VMError::AllocationDeferred)?;
                let buffer =
                    ExecutionBuffer::new(occurrences, &mut lease).map_err(|error| match error {
                        PrepaidBufferError::Allocation(ChargedBufferError::Admission(error)) => {
                            VMError::AllocationDeferred(error)
                        }
                        PrepaidBufferError::Allocation(ChargedBufferError::Allocator {
                            ..
                        })
                        | PrepaidBufferError::Reservation(_) => {
                            VMError::ExecutionDeferred(ExecutionDeferral::AllocationUnavailable)
                        }
                    })?;
                Scratch::Funded(buffer)
            }
            None => Scratch::Local(OwnedAllocation::try_filled_copy(occurrences, 0)?),
        };
        Ok(Self {
            storage,
            used: 0,
            capacity: occurrences,
            budget: budget.cloned(),
        })
    }

    pub(super) fn push(&mut self, number: u32) -> Result<(), VMError> {
        if self.used == self.capacity {
            return Err(VMError::DecodeError);
        }
        match &mut self.storage {
            Scratch::Local(values) => values[self.used] = number,
            Scratch::Funded(values) => values.push_reserved(number),
        }
        self.used += 1;
        Ok(())
    }

    pub(super) fn finish(mut self) -> Result<SyscallUsages, VMError> {
        if self.used != self.capacity {
            return Err(VMError::DecodeError);
        }
        if self.used == 0 {
            return Ok(SyscallUsages::default());
        }
        let numbers = match &mut self.storage {
            Scratch::Local(values) => &mut values[..],
            Scratch::Funded(values) => values.as_mut_slice(),
        };
        numbers.sort_unstable();
        let remaining = 1 + numbers.windows(2).filter(|pair| pair[0] != pair[1]).count();
        let runs = Runs { numbers, remaining };
        // The exact output buffer and control block are admitted while scratch
        // is still live. Scratch then drops before this immutable result escapes.
        let values = match self.budget.as_ref() {
            Some(budget) => SharedAllocation::try_from_iter_with_memory_budget(runs, budget)?,
            None => SharedAllocation::try_from_iter(runs)?,
        };
        Ok(SyscallUsages(Some(values)))
    }
}

struct Runs<'a> {
    numbers: &'a [u32],
    remaining: usize,
}
impl Iterator for Runs<'_> {
    type Item = Result<SyscallUsage, VMError>;
    fn next(&mut self) -> Option<Self::Item> {
        let number = *self.numbers.first()?;
        let count = self
            .numbers
            .iter()
            .take_while(|value| **value == number)
            .count();
        self.numbers = &self.numbers[count..];
        self.remaining -= 1;
        Some(Ok(SyscallUsage {
            number,
            count: count as u64,
        }))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
impl ExactSizeIterator for Runs<'_> {}

#[cfg(test)]
mod tests;
