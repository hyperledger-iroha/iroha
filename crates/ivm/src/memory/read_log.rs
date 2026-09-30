//! Read-range backing and detached snapshots retain their original allocation owners.
//!
//! Growth prepares an independent replacement before diagnostics or caller output
//! change. State-owned logs reserve from the same finite execution pool and copy
//! templates by partitioning their original prepaid parent reservation.

use std::{fmt, ops::Deref};

use iroha_allocation::AllocationBudget;

use super::AccessRange;
use crate::{
    VMError,
    cache_memory::OwnedVec,
    error::ExecutionDeferral,
    execution_memory::{ExecutionBuffer, ExecutionMemoryLease, ExecutionMemoryPlan},
};

/// Independent immutable read history with allocation-lifetime custody.
///
/// It holds no Memory lock. Later loads and clears cannot change the snapshot;
/// independent cloning is fallible and retains the original admission pool.
#[derive(Debug, PartialEq, Eq)]
pub struct ReadLogSnapshot {
    log: ReadLog,
}

impl ReadLogSnapshot {
    /// Copy this snapshot into independently owned exact row storage.
    ///
    /// # Errors
    /// Defers locally if original-pool capacity or physical allocation is
    /// unavailable. No other pool funds a refused copy.
    pub fn try_clone(&self) -> Result<Self, VMError> {
        self.log.try_snapshot()
    }
}

impl Deref for ReadLogSnapshot {
    type Target = [AccessRange];
    fn deref(&self) -> &Self::Target {
        &self.log.rows
    }
}

#[derive(Debug, Default)]
pub(super) struct ReadLog {
    rows: Rows,
    active_budget: Option<AllocationBudget>,
}

impl PartialEq for ReadLog {
    fn eq(&self, other: &Self) -> bool {
        self.rows[..] == other.rows[..]
    }
}
impl Eq for ReadLog {}

impl Deref for ReadLog {
    type Target = [AccessRange];
    fn deref(&self) -> &Self::Target {
        &self.rows
    }
}

impl ReadLog {
    pub(super) fn with_memory_budget(budget: &AllocationBudget) -> Self {
        Self {
            rows: Rows::Funded(None),
            active_budget: Some(budget.clone()),
        }
    }

    /// Prepare new backing without publishing it or changing existing rows.
    pub(super) fn prepare_growth(&self) -> Result<Option<Rows>, VMError> {
        if self.len() < self.rows.capacity() {
            return Ok(None);
        }
        let count = self
            .rows
            .capacity()
            .checked_mul(2)
            .ok_or_else(unavailable)?
            .max(4);
        let plan = ExecutionMemoryPlan::array::<AccessRange>(count)
            .map_err(VMError::AllocationDeferred)?;
        let rows = match &self.active_budget {
            Some(budget) => {
                // Both arrays remain charged through allocation, diagnostic
                // validation, publication and the old backing's deallocation.
                let mut lease = ExecutionMemoryLease::reserve(budget, plan)
                    .map_err(VMError::AllocationDeferred)?;
                Rows::funded_copy(&self.rows, count, &mut lease)?
            }
            None => self.rows.local_copy(count)?,
        };
        Ok(Some(rows))
    }

    /// Publish one prepared read after diagnostics accept the complete access.
    pub(super) fn record_prepared(&mut self, replacement: Option<Rows>, range: AccessRange) {
        if let Some(mut replacement) = replacement {
            replacement.push_reserved(range);
            let previous = std::mem::replace(&mut self.rows, replacement);
            // Publish the complete new history before old backing can notify
            // waiters. Memory defers those callbacks until its mutex releases.
            drop(previous);
        } else {
            self.rows.push_reserved(range);
        }
    }

    pub(super) fn clear(&mut self) {
        match &mut self.rows {
            Rows::Local(rows) => rows.clear(),
            Rows::Funded(rows) => {
                if let Some(rows) = rows {
                    rows.truncate(0);
                }
            }
        }
    }

    pub(super) fn memory_plan(&self) -> Result<ExecutionMemoryPlan, VMError> {
        ExecutionMemoryPlan::array::<AccessRange>(self.len()).map_err(VMError::AllocationDeferred)
    }

    pub(super) fn try_copy(
        &self,
        parent: Option<&mut ExecutionMemoryLease>,
    ) -> Result<Self, VMError> {
        let rows = if let Some(budget) = &self.active_budget {
            let plan = self.memory_plan()?;
            let mut lease = match parent {
                Some(parent) => {
                    if !parent.belongs_to(budget) {
                        return Err(unavailable());
                    }
                    parent.partition(plan).map_err(|_| unavailable())?
                }
                None => ExecutionMemoryLease::reserve(budget, plan)
                    .map_err(VMError::AllocationDeferred)?,
            };
            Rows::funded_copy(&self.rows, self.len(), &mut lease)?
        } else {
            if parent.is_some() {
                return Err(unavailable());
            }
            self.rows.local_copy(self.len())?
        };
        Ok(Self {
            rows,
            active_budget: self.active_budget.clone(),
        })
    }

    pub(super) fn try_snapshot(&self) -> Result<ReadLogSnapshot, VMError> {
        self.try_copy(None).map(|log| ReadLogSnapshot { log })
    }

    pub(super) fn try_retain(&self) -> bool {
        match &self.rows {
            Rows::Local(rows) => rows.try_retain(),
            Rows::Funded(rows) => rows.as_ref().is_none_or(ExecutionBuffer::try_retain),
        }
    }

    pub(super) fn make_active(&self) {
        match &self.rows {
            Rows::Local(rows) => rows.make_active(),
            Rows::Funded(rows) => {
                if let Some(rows) = rows {
                    rows.activate();
                }
            }
        }
    }

    #[cfg(test)]
    pub(super) fn capacity(&self) -> usize {
        self.rows.capacity()
    }
}

pub(super) enum Rows {
    Local(OwnedVec<AccessRange>),
    Funded(Option<ExecutionBuffer<AccessRange>>),
}

impl Default for Rows {
    fn default() -> Self {
        Self::Local(OwnedVec::default())
    }
}

impl Rows {
    fn capacity(&self) -> usize {
        match self {
            Self::Local(rows) => rows.capacity(),
            Self::Funded(rows) => rows.as_ref().map_or(0, ExecutionBuffer::capacity),
        }
    }

    fn local_copy(&self, count: usize) -> Result<Self, VMError> {
        let Self::Local(source) = self else {
            return Err(unavailable());
        };
        let mut rows = source.try_copy_capacity(count).map_err(|_| unavailable())?;
        for &row in source.iter() {
            rows.insert_reserved(rows.len(), row);
        }
        Ok(Self::Local(rows))
    }

    fn funded_copy(
        source: &[AccessRange],
        count: usize,
        lease: &mut ExecutionMemoryLease,
    ) -> Result<Self, VMError> {
        let mut rows = ExecutionBuffer::new(count, lease).map_err(|_| unavailable())?;
        rows.append(source)
            .expect("exact prepaid read-row capacity");
        Ok(Self::Funded(Some(rows)))
    }

    fn push_reserved(&mut self, row: AccessRange) {
        match self {
            Self::Local(rows) => rows.insert_reserved(rows.len(), row),
            Self::Funded(rows) => rows
                .as_mut()
                .expect("prepaid read-row backing")
                .push_reserved(row),
        }
    }
}

impl Deref for Rows {
    type Target = [AccessRange];
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Local(rows) => rows,
            Self::Funded(rows) => rows.as_ref().map_or(&[], ExecutionBuffer::as_slice),
        }
    }
}

impl fmt::Debug for Rows {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, formatter)
    }
}

fn unavailable() -> VMError {
    VMError::ExecutionDeferred(ExecutionDeferral::AllocationUnavailable)
}

#[cfg(test)]
mod tests;
