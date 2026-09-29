//! Fixed original-pool backing and local retention custody for write logs.

use std::{
    fmt,
    ops::{Deref, DerefMut},
};

use super::{WriteLogEntry, allocation_error};
use crate::{
    VMError,
    cache_memory::{OwnedAllocation, OwnedVec, OwnedVecGrowthError},
    execution_memory::{ExecutionBuffer, ExecutionMemoryLease, ExecutionMemoryPlan},
};

pub(super) enum Bytes {
    Local(OwnedAllocation<u8>),
    Funded(ExecutionBuffer<u8>),
}

impl Bytes {
    pub(super) fn copy_funded(
        bytes: &[u8],
        lease: &mut ExecutionMemoryLease,
    ) -> Result<Self, VMError> {
        let mut copied = ExecutionBuffer::new(bytes.len(), lease).map_err(|_| unavailable())?;
        copied
            .append(bytes)
            .expect("exact prepaid payload capacity");
        Ok(Self::Funded(copied))
    }

    pub(super) fn try_retain(&self) -> bool {
        match self {
            Self::Local(bytes) => bytes.try_retain(),
            Self::Funded(bytes) => bytes.try_retain(),
        }
    }

    pub(super) fn make_active(&self) {
        match self {
            Self::Local(bytes) => bytes.make_active(),
            Self::Funded(bytes) => bytes.activate(),
        }
    }
}

impl Deref for Bytes {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Local(bytes) => bytes,
            Self::Funded(bytes) => bytes.as_slice(),
        }
    }
}

impl DerefMut for Bytes {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Self::Local(bytes) => bytes,
            Self::Funded(bytes) => bytes.as_mut_slice(),
        }
    }
}

impl fmt::Debug for Bytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, formatter)
    }
}

pub(super) enum Rows {
    Local(OwnedVec<WriteLogEntry>),
    Funded(Option<ExecutionBuffer<WriteLogEntry>>),
}

impl Default for Rows {
    fn default() -> Self {
        Self::Local(OwnedVec::default())
    }
}

impl Rows {
    #[cfg(test)]
    pub(super) fn capacity(&self) -> usize {
        match self {
            Self::Local(rows) => rows.capacity(),
            Self::Funded(rows) => rows.as_ref().map_or(0, ExecutionBuffer::capacity),
        }
    }

    pub(super) fn funded_growth_capacity(&self) -> Result<Option<usize>, VMError> {
        let Self::Funded(rows) = self else {
            return Err(unavailable());
        };
        let capacity = rows.as_ref().map_or(0, ExecutionBuffer::capacity);
        if self.len() < capacity {
            return Ok(None);
        }
        let next = capacity.checked_mul(2).ok_or_else(unavailable)?.max(4);
        ExecutionMemoryPlan::array::<WriteLogEntry>(next).map_err(VMError::AllocationDeferred)?;
        Ok(Some(next))
    }

    /// Construct replacement storage without moving a live row or refunding its owner.
    pub(super) fn prepare_growth(
        capacity: Option<usize>,
        lease: &mut ExecutionMemoryLease,
    ) -> Result<Option<ExecutionBuffer<WriteLogEntry>>, VMError> {
        capacity
            .map(|count| ExecutionBuffer::new(count, lease).map_err(|_| unavailable()))
            .transpose()
    }

    /// Install a fully allocated replacement before old-backing release can notify waiters.
    pub(super) fn install_growth(&mut self, replacement: Option<ExecutionBuffer<WriteLogEntry>>) {
        let Some(mut replacement) = replacement else {
            return;
        };
        let Self::Funded(current) = self else {
            panic!("original funded rows required");
        };
        if let Some(previous) = current.as_mut() {
            for row in previous.drain_all() {
                replacement.push_reserved(row);
            }
        }
        let previous = current.replace(replacement);
        drop(previous);
    }

    pub(super) fn prepare_local(&mut self, bytes: &[u8]) -> Result<Bytes, VMError> {
        let Self::Local(rows) = self else {
            return Err(unavailable());
        };
        rows.try_reserve_one().map_err(allocation_error)?;
        rows.try_copy_bytes(bytes).map(Bytes::Local)
    }

    pub(super) fn local_copy_capacity(&self, count: usize) -> Result<Self, VMError> {
        let Self::Local(rows) = self else {
            return Err(unavailable());
        };
        rows.try_copy_capacity(count)
            .map(Self::Local)
            .map_err(allocation_error)
    }

    pub(super) fn local_copy_bytes(&self, bytes: &[u8]) -> Result<Bytes, VMError> {
        let Self::Local(rows) = self else {
            return Err(unavailable());
        };
        rows.try_copy_bytes(bytes).map(Bytes::Local)
    }

    pub(super) fn funded_copy_capacity(
        count: usize,
        lease: &mut ExecutionMemoryLease,
    ) -> Result<Self, VMError> {
        ExecutionBuffer::new(count, lease)
            .map(|rows| Self::Funded(Some(rows)))
            .map_err(|_| unavailable())
    }

    pub(super) fn push_reserved(&mut self, row: WriteLogEntry) {
        match self {
            Self::Local(rows) => rows.insert_reserved(rows.len(), row),
            Self::Funded(rows) => rows
                .as_mut()
                .expect("prepaid row backing")
                .push_reserved(row),
        }
    }

    pub(super) fn clear(&mut self) {
        match self {
            Self::Local(rows) => rows.clear(),
            Self::Funded(rows) => {
                if let Some(rows) = rows {
                    rows.truncate(0);
                }
            }
        }
    }

    pub(super) fn try_retain(&self) -> bool {
        match self {
            Self::Local(rows) => rows.try_retain(),
            Self::Funded(rows) => rows.as_ref().is_none_or(ExecutionBuffer::try_retain),
        }
    }

    pub(super) fn make_active(&self) {
        match self {
            Self::Local(rows) => rows.make_active(),
            Self::Funded(rows) => {
                if let Some(rows) = rows {
                    rows.activate();
                }
            }
        }
    }
}

impl Deref for Rows {
    type Target = [WriteLogEntry];
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
    allocation_error(OwnedVecGrowthError::AllocationUnavailable)
}
