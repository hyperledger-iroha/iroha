//! Fallibly owned, canonical private-byte intervals for one VM.
//!
//! Binary search keeps privacy queries logarithmic in the interval count.
//! Updates shift a contiguous range slice; the active execution owner funds its
//! backing before a guest store can mutate memory. The bounded linear update
//! work remains a performance qualification item for fragmented workloads.

use std::ops::Range;

use iroha_allocation::AllocationBudget;

use crate::{
    VMError,
    execution_memory::{ExecutionMemoryLease, ExecutionMemoryPlan},
};

mod storage;
use storage::Ranges;

#[cfg(test)]
thread_local! {
    static REFUSE_NEXT_GROWTH: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static REFUSE_NEXT_COPY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Sorted, disjoint half-open private ranges with allocation-owned capacity.
#[derive(Debug, Default)]
pub(super) struct PrivateMemoryRanges {
    ranges: Ranges,
    active_budget: Option<AllocationBudget>,
}

impl PartialEq for PrivateMemoryRanges {
    fn eq(&self, other: &Self) -> bool {
        self.ranges[..] == other.ranges[..]
    }
}

impl Eq for PrivateMemoryRanges {}

impl PrivateMemoryRanges {
    pub(super) fn with_memory_budget(budget: &AllocationBudget) -> Self {
        Self {
            ranges: Ranges::Funded(None),
            active_budget: Some(budget.clone()),
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    pub(super) fn clear(&mut self) {
        self.ranges.clear();
    }

    pub(super) fn runtime_template_memory_plan(&self) -> Result<ExecutionMemoryPlan, VMError> {
        ExecutionMemoryPlan::array::<(u64, u64)>(self.ranges.len())
            .map_err(VMError::AllocationDeferred)
    }

    #[cfg(test)]
    pub(super) fn try_clone(&self) -> Result<Self, VMError> {
        let mut lease = self
            .active_budget
            .as_ref()
            .map(|budget| {
                ExecutionMemoryLease::reserve(budget, self.runtime_template_memory_plan()?)
                    .map_err(VMError::AllocationDeferred)
            })
            .transpose()?;
        self.try_clone_for_runtime_template(lease.as_mut())
    }

    /// Copy from the existing snapshot plan without admitting a second pool reservation.
    pub(super) fn try_clone_for_runtime_template(
        &self,
        lease: Option<&mut ExecutionMemoryLease>,
    ) -> Result<Self, VMError> {
        match (&self.active_budget, &lease) {
            (Some(budget), Some(lease)) if lease.belongs_to(budget) => {}
            (None, None) => {}
            _ => return Err(storage::unavailable()),
        }
        #[cfg(test)]
        if !self.ranges.is_empty() && REFUSE_NEXT_COPY.with(|refuse| refuse.replace(false)) {
            return Err(storage::unavailable());
        }
        Ok(Self {
            ranges: self.ranges.try_copy(self.ranges.len(), lease)?,
            active_budget: self.active_budget.clone(),
        })
    }

    /// Prepare any missing interval capacity before reset changes guest state.
    pub(super) fn try_prepare_restore(&mut self, template: &Self) -> Result<(), VMError> {
        self.try_reserve_capacity(template.ranges.len())
    }

    /// Restore within the preflighted allocation, without a new copy owner.
    pub(super) fn restore_prepared(&mut self, template: &Self) {
        assert!(self.ranges.capacity() >= template.ranges.len());
        self.ranges.clear();
        for &range in &template.ranges[..] {
            self.ranges.insert_reserved(self.ranges.len(), range);
        }
    }

    pub(super) fn try_retain(&self) -> bool {
        self.ranges.try_retain()
    }

    pub(super) fn make_active(&self) {
        self.ranges.make_active();
    }

    fn try_reserve_capacity(&mut self, required: usize) -> Result<(), VMError> {
        if required <= self.ranges.capacity() {
            return Ok(());
        }
        let capacity = self
            .ranges
            .capacity()
            .checked_mul(2)
            .ok_or(VMError::AllocationDeferred(
                iroha_allocation::AllocationRefusal::DemandOverflow,
            ))?
            .max(4)
            .max(required);
        let plan = ExecutionMemoryPlan::array::<(u64, u64)>(capacity)
            .map_err(VMError::AllocationDeferred)?;
        let mut lease = self
            .active_budget
            .as_ref()
            .map(|budget| {
                ExecutionMemoryLease::reserve(budget, plan).map_err(VMError::AllocationDeferred)
            })
            .transpose()?;
        let replacement = self.ranges.try_copy(capacity, lease.as_mut())?;
        // Publish complete replacement storage before old backing release can
        // synchronously notify the original pool's waiting execution owner.
        let previous = std::mem::replace(&mut self.ranges, replacement);
        drop(previous);
        Ok(())
    }

    fn lower_bound(&self, start: u64) -> usize {
        self.ranges
            .partition_point(|(existing, _)| *existing < start)
    }

    /// Reserve the only possible new interval before the caller's memory store.
    pub(super) fn try_prepare_update(
        &mut self,
        range: &Range<u64>,
        private: bool,
    ) -> Result<(), VMError> {
        if range.start >= range.end {
            return Ok(());
        }
        let index = self.lower_bound(range.start);
        let needs_new = if private {
            let joins_left = index > 0 && self.ranges[index - 1].1 >= range.start;
            let joins_right = self
                .ranges
                .get(index)
                .is_some_and(|(start, _)| *start <= range.end);
            !joins_left && !joins_right
        } else {
            index > 0
                && self.ranges[index - 1].0 < range.start
                && self.ranges[index - 1].1 > range.end
        };
        if needs_new {
            #[cfg(test)]
            if self.ranges.len() == self.ranges.capacity()
                && REFUSE_NEXT_GROWTH.with(|refuse| refuse.replace(false))
            {
                return Err(storage::unavailable());
            }
            let required = self
                .ranges
                .len()
                .checked_add(1)
                .ok_or(VMError::AllocationDeferred(
                    iroha_allocation::AllocationRefusal::DemandOverflow,
                ))?;
            self.try_reserve_capacity(required)?;
        }
        Ok(())
    }

    /// Apply a preflighted tag update without allocation or failure.
    pub(super) fn apply_prepared_update(&mut self, range: Range<u64>, private: bool) {
        if private {
            self.insert_prepared(range);
        } else {
            self.remove_prepared(range);
        }
    }

    fn insert_prepared(&mut self, range: Range<u64>) {
        if range.start >= range.end {
            return;
        }
        let mut first = self.lower_bound(range.start);
        if first > 0 && self.ranges[first - 1].1 >= range.start {
            first -= 1;
        }
        let mut merged = (range.start, range.end);
        let mut tail = first;
        while let Some(&(start, end)) = self.ranges.get(tail) {
            if start > merged.1 {
                break;
            }
            merged.0 = merged.0.min(start);
            merged.1 = merged.1.max(end);
            tail += 1;
        }
        if first == tail {
            self.ranges.insert_reserved(first, merged);
        } else {
            self.ranges[first] = merged;
            self.ranges.remove_range(first + 1..tail);
        }
    }

    fn remove_prepared(&mut self, range: Range<u64>) {
        if range.start >= range.end || self.ranges.is_empty() {
            return;
        }
        let mut index = self.lower_bound(range.start);
        if index > 0 && self.ranges[index - 1].1 > range.start {
            index -= 1;
        }
        while let Some(&(start, end)) = self.ranges.get(index) {
            if start >= range.end {
                break;
            }
            if end <= range.start {
                index += 1;
                continue;
            }
            if start < range.start && end > range.end {
                self.ranges[index].1 = range.start;
                self.ranges.insert_reserved(index + 1, (range.end, end));
                break;
            }
            if start < range.start {
                self.ranges[index].1 = range.start;
                index += 1;
                continue;
            }
            if end > range.end {
                self.ranges[index].0 = range.end;
                break;
            }
            self.ranges.remove_at(index);
        }
    }

    pub(super) fn intersection_len(&self, range: Range<u64>) -> u64 {
        if range.start >= range.end {
            return 0;
        }
        let mut index = self.lower_bound(range.start);
        if index > 0 && self.ranges[index - 1].1 > range.start {
            index -= 1;
        }
        let mut count = 0_u64;
        for &(start, end) in &self.ranges[index..] {
            if start >= range.end {
                break;
            }
            count = count.saturating_add(end.min(range.end).saturating_sub(start.max(range.start)));
        }
        count
    }

    pub(super) fn intersects(&self, range: Range<u64>) -> bool {
        if range.start >= range.end {
            return false;
        }
        let mut index = self.lower_bound(range.start);
        if index > 0 && self.ranges[index - 1].1 > range.start {
            index -= 1;
        }
        self.ranges
            .get(index)
            .is_some_and(|(start, end)| *start < range.end && *end > range.start)
    }

    /// Return the next ordered range, avoiding an allocated drain snapshot.
    pub(super) fn next_after(&self, previous: Option<u64>) -> Option<(u64, u64)> {
        let index = previous.map_or(0, |start| {
            self.ranges
                .partition_point(|(existing, _)| *existing <= start)
        });
        self.ranges.get(index).copied()
    }

    #[cfg(test)]
    pub(super) fn pairs_for_testing(&self) -> &[(u64, u64)] {
        &self.ranges
    }

    #[cfg(test)]
    pub(super) fn refuse_next_growth_for_testing() {
        REFUSE_NEXT_GROWTH.with(|refuse| refuse.set(true));
    }

    #[cfg(test)]
    pub(super) fn refuse_next_copy_for_testing() {
        REFUSE_NEXT_COPY.with(|refuse| refuse.set(true));
    }

    #[cfg(test)]
    pub(super) fn try_insert(&mut self, range: Range<u64>) -> Result<(), VMError> {
        self.try_prepare_update(&range, true)?;
        self.apply_prepared_update(range, true);
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn try_remove(&mut self, range: Range<u64>) -> Result<(), VMError> {
        self.try_prepare_update(&range, false)?;
        self.apply_prepared_update(range, false);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
