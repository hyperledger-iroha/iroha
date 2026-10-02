//! Interval backing retains original active credit through growth and destruction.

use std::ops::{Deref, DerefMut, Range};

use crate::{
    VMError,
    cache_memory::OwnedVec,
    error::ExecutionDeferral,
    execution_memory::{ExecutionBuffer, ExecutionMemoryLease},
};

pub(super) enum Ranges {
    Local(OwnedVec<(u64, u64)>),
    Funded(Option<ExecutionBuffer<(u64, u64)>>),
}

impl Default for Ranges {
    fn default() -> Self {
        Self::Local(OwnedVec::default())
    }
}

impl Ranges {
    pub(super) fn capacity(&self) -> usize {
        match self {
            Self::Local(ranges) => ranges.capacity(),
            Self::Funded(ranges) => ranges.as_ref().map_or(0, ExecutionBuffer::capacity),
        }
    }

    /// Copy into independent backing, retaining the old owner through allocation.
    pub(super) fn try_copy(
        &self,
        capacity: usize,
        lease: Option<&mut ExecutionMemoryLease>,
    ) -> Result<Self, VMError> {
        assert!(capacity >= self.len());
        match (self, lease) {
            (Self::Local(ranges), None) => {
                let mut copied = ranges
                    .try_copy_capacity(capacity)
                    .map_err(|_| unavailable())?;
                for &range in &ranges[..] {
                    copied.insert_reserved(copied.len(), range);
                }
                Ok(Self::Local(copied))
            }
            (Self::Funded(_), Some(lease)) => {
                let mut copied =
                    ExecutionBuffer::new(capacity, lease).map_err(|_| unavailable())?;
                copied.append(self).expect("prepaid interval capacity");
                Ok(Self::Funded(Some(copied)))
            }
            _ => Err(unavailable()),
        }
    }

    pub(super) fn insert_reserved(&mut self, index: usize, value: (u64, u64)) {
        assert!(index <= self.len());
        match self {
            Self::Local(ranges) => ranges.insert_reserved(index, value),
            Self::Funded(ranges) => {
                let ranges = ranges.as_mut().expect("prepaid interval storage");
                let len = ranges.as_slice().len();
                ranges.push_reserved(value);
                let values = ranges.as_mut_slice();
                values.copy_within(index..len, index + 1);
                values[index] = value;
            }
        }
    }

    pub(super) fn remove_at(&mut self, index: usize) {
        self.remove_range(index..index + 1);
    }

    pub(super) fn remove_range(&mut self, range: Range<usize>) {
        assert!(range.start <= range.end && range.end <= self.len());
        match self {
            Self::Local(ranges) => ranges.remove_range(range),
            Self::Funded(ranges) => {
                if let Some(ranges) = ranges {
                    let len = ranges.as_slice().len();
                    ranges
                        .as_mut_slice()
                        .copy_within(range.end..len, range.start);
                    ranges.truncate(len - range.len());
                }
            }
        }
    }

    pub(super) fn clear(&mut self) {
        match self {
            Self::Local(ranges) => ranges.clear(),
            Self::Funded(ranges) => {
                if let Some(ranges) = ranges {
                    ranges.truncate(0);
                }
            }
        }
    }

    pub(super) fn try_retain(&self) -> bool {
        match self {
            Self::Local(ranges) => ranges.try_retain(),
            Self::Funded(ranges) => ranges.as_ref().is_none_or(ExecutionBuffer::try_retain),
        }
    }

    pub(super) fn make_active(&self) {
        match self {
            Self::Local(ranges) => ranges.make_active(),
            Self::Funded(ranges) => {
                if let Some(ranges) = ranges {
                    ranges.activate();
                }
            }
        }
    }
}

impl Deref for Ranges {
    type Target = [(u64, u64)];

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Local(ranges) => ranges,
            Self::Funded(ranges) => ranges.as_ref().map_or(&[], ExecutionBuffer::as_slice),
        }
    }
}

impl DerefMut for Ranges {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Self::Local(ranges) => ranges,
            Self::Funded(ranges) => ranges
                .as_mut()
                .map_or(&mut [], ExecutionBuffer::as_mut_slice),
        }
    }
}

impl std::fmt::Debug for Ranges {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&**self, formatter)
    }
}

pub(super) fn unavailable() -> VMError {
    VMError::ExecutionDeferred(ExecutionDeferral::AllocationUnavailable)
}
