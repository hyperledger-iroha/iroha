//! Write-log rows, payloads and detached snapshots retain their own allocation charges.
//!
//! Preparation finishes every allocation before a store changes guest state.
//! State-owned logs reserve exact row and payload layouts from their original
//! finite execution pool; independent snapshots retain that same admission owner.
//! Runtime-template copies split the already prepaid parent lease once.

use std::ops::Deref;

use crate::{
    VMError,
    cache_memory::OwnedVecGrowthError,
    error::ExecutionDeferral,
    execution_memory::{ExecutionMemoryLease, ExecutionMemoryPlan},
};
use mv::allocation::AllocationBudget;

mod storage;
use storage::{Bytes, Rows};

/// One immutable recorded write with independently owned, scrubbed byte storage.
#[derive(Debug)]
pub struct WriteLogEntry {
    addr: u64,
    bytes: Bytes,
}

impl WriteLogEntry {
    /// Guest address of the first recorded byte.
    pub const fn address(&self) -> u64 {
        self.addr
    }

    /// Exact bytes written, borrowed without allocating or exposing capacity.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl PartialEq for WriteLogEntry {
    fn eq(&self, other: &Self) -> bool {
        self.addr == other.addr && *self.bytes == *other.bytes
    }
}
impl Eq for WriteLogEntry {}

impl Drop for WriteLogEntry {
    fn drop(&mut self) {
        iroha_crypto::zeroize_value_for_confidential_discard(&mut self.bytes[..]);
        #[cfg(test)]
        {
            assert!(self.bytes.iter().all(|byte| *byte == 0));
            SCRUBBED_ENTRIES.set(SCRUBBED_ENTRIES.get() + 1);
        }
        // Both local and funded storage drop backing before returning its charges.
    }
}

/// Independent immutable write history with allocation-lifetime custody.
///
/// It holds no Memory lock; later stores and clears cannot change this snapshot.
/// Cloning is explicitly fallible and creates independently accounted storage.
#[derive(Debug, PartialEq, Eq)]
pub struct WriteLogSnapshot {
    log: WriteLog,
}

impl WriteLogSnapshot {
    /// Copy this snapshot after reserving each row and payload allocation.
    ///
    /// # Errors
    /// Returns a local allocation deferral if original-pool credit or physical
    /// allocation is unavailable. No alternate budget funds a refused copy.
    pub fn try_clone(&self) -> Result<Self, VMError> {
        self.log.try_snapshot()
    }
}

impl Deref for WriteLogSnapshot {
    type Target = [WriteLogEntry];
    fn deref(&self) -> &Self::Target {
        &self.log.rows
    }
}

#[derive(Debug, Default)]
pub(super) struct WriteLog {
    rows: Rows,
    active_budget: Option<AllocationBudget>,
}

impl PartialEq for WriteLog {
    fn eq(&self, other: &Self) -> bool {
        self.rows[..] == other.rows[..]
    }
}
impl Eq for WriteLog {}

impl WriteLog {
    pub(super) fn with_memory_budget(budget: &AllocationBudget) -> Self {
        Self {
            rows: Rows::Funded(None),
            active_budget: Some(budget.clone()),
        }
    }

    #[cfg(test)]
    pub(super) fn with_test_budget(budget: &crate::cache_memory::TestMemoryBudget) -> Self {
        Self {
            rows: Rows::Local(budget.empty_rows()),
            active_budget: None,
        }
    }

    /// Reserve a row and its payload before a caller mutates guest state.
    pub(super) fn prepare(&mut self, addr: u64, bytes: &[u8]) -> Result<WriteLogEntry, VMError> {
        if let Some(budget) = &self.active_budget {
            let capacity = self.rows.funded_growth_capacity()?;
            let mut plan = ExecutionMemoryPlan::array::<u8>(bytes.len())
                .map_err(VMError::AllocationDeferred)?;
            if let Some(capacity) = capacity {
                plan.include_child(
                    ExecutionMemoryPlan::array::<WriteLogEntry>(capacity)
                        .map_err(VMError::AllocationDeferred)?,
                )
                .map_err(VMError::AllocationDeferred)?;
            }
            // Old row backing stays charged throughout admission and physical
            // allocation of both replacements. Refusal cannot change guest bytes.
            let mut lease =
                ExecutionMemoryLease::reserve(budget, plan).map_err(VMError::AllocationDeferred)?;
            let replacement = Rows::prepare_growth(capacity, &mut lease)?;
            let copied = Bytes::copy_funded(bytes, &mut lease)?;
            let entry = WriteLogEntry {
                addr,
                bytes: copied,
            };
            // Publish valid row storage before dropping old backing can notify
            // resource waiters. A callback unwind still leaves prior rows intact.
            self.rows.install_growth(replacement);
            Ok(entry)
        } else {
            self.rows
                .prepare_local(bytes)
                .map(|bytes| WriteLogEntry { addr, bytes })
        }
    }

    /// Publish exactly one prepared entry without another allocation or refusal.
    pub(super) fn record_prepared(&mut self, entry: WriteLogEntry) {
        self.rows.push_reserved(entry);
    }

    pub(super) fn clear(&mut self) {
        self.rows.clear();
    }

    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub(super) fn memory_plan(&self) -> Result<ExecutionMemoryPlan, VMError> {
        let mut plan = ExecutionMemoryPlan::array::<WriteLogEntry>(self.rows.len())
            .map_err(VMError::AllocationDeferred)?;
        for entry in self.rows.iter() {
            plan.include_child(
                ExecutionMemoryPlan::array::<u8>(entry.bytes.len())
                    .map_err(VMError::AllocationDeferred)?,
            )
            .map_err(VMError::AllocationDeferred)?;
        }
        Ok(plan)
    }

    pub(super) fn try_copy(
        &self,
        parent: Option<&mut ExecutionMemoryLease>,
    ) -> Result<Self, VMError> {
        let mut lease = if let Some(budget) = &self.active_budget {
            let plan = self.memory_plan()?;
            Some(match parent {
                Some(parent) => {
                    if !parent.belongs_to(budget) {
                        return Err(allocation_error(OwnedVecGrowthError::AllocationUnavailable));
                    }
                    parent
                        .partition(plan)
                        .map_err(|_| allocation_error(OwnedVecGrowthError::AllocationUnavailable))?
                }
                None => ExecutionMemoryLease::reserve(budget, plan)
                    .map_err(VMError::AllocationDeferred)?,
            })
        } else {
            // A local owner cannot silently accept a foreign funded template.
            if parent.is_some() {
                return Err(allocation_error(OwnedVecGrowthError::AllocationUnavailable));
            }
            None
        };
        let mut rows = match lease.as_mut() {
            Some(lease) => Rows::funded_copy_capacity(self.rows.len(), lease)?,
            None => self.rows.local_copy_capacity(self.rows.len())?,
        };
        for entry in self.rows.iter() {
            let bytes = match lease.as_mut() {
                Some(lease) => Bytes::copy_funded(entry.bytes(), lease)?,
                None => rows.local_copy_bytes(entry.bytes())?,
            };
            rows.push_reserved(WriteLogEntry {
                addr: entry.addr,
                bytes,
            });
        }
        Ok(Self {
            rows,
            active_budget: self.active_budget.clone(),
        })
    }

    pub(super) fn try_snapshot(&self) -> Result<WriteLogSnapshot, VMError> {
        self.try_copy(None).map(|log| WriteLogSnapshot { log })
    }

    pub(super) fn try_retain(&self) -> bool {
        self.rows.try_retain() && self.rows.iter().all(|entry| entry.bytes.try_retain())
    }

    pub(super) fn make_active(&self) {
        self.rows.make_active();
        for entry in self.rows.iter() {
            entry.bytes.make_active();
        }
    }
}

fn allocation_error(_: OwnedVecGrowthError) -> VMError {
    VMError::ExecutionDeferred(ExecutionDeferral::AllocationUnavailable)
}

#[cfg(test)]
thread_local! {
    static SCRUBBED_ENTRIES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod funded_tests;
