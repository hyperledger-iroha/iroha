//! Geometry-sized dirty-leaf sets with allocation-lifetime custody.
//!
//! Both sets are reserved with the memory image, before execution. Stores,
//! commits and warm resets only change initialized words and cannot grow them.

use crate::{
    cache_memory::OwnedAllocation,
    error::VMError,
    execution_memory::{ExecutionBuffer, ExecutionMemoryLease, ExecutionMemoryPlan},
};

enum Words {
    Local(OwnedAllocation<u64>),
    Funded(ExecutionBuffer<u64>),
}

impl Words {
    fn as_slice(&self) -> &[u64] {
        match self {
            Self::Local(words) => words,
            Self::Funded(words) => words.as_slice(),
        }
    }

    fn as_mut_slice(&mut self) -> &mut [u64] {
        match self {
            Self::Local(words) => words,
            Self::Funded(words) => words.as_mut_slice(),
        }
    }
}

/// One bit for each exact memory leaf, with no dynamically growing index set.
pub(crate) struct DirtyChunks {
    words: Words,
    chunks: usize,
    count: usize,
}

impl DirtyChunks {
    pub(crate) fn memory_plan(chunks: usize) -> Result<ExecutionMemoryPlan, VMError> {
        ExecutionMemoryPlan::array::<u64>(chunks.div_ceil(64)).map_err(VMError::AllocationDeferred)
    }

    pub(crate) fn new(
        chunks: usize,
        lease: Option<&mut ExecutionMemoryLease>,
    ) -> Result<Self, VMError> {
        let count = chunks.div_ceil(64);
        let words = if let Some(lease) = lease {
            let mut words = ExecutionBuffer::new(count, lease).map_err(|_| {
                VMError::ExecutionDeferred(crate::error::ExecutionDeferral::AllocationUnavailable)
            })?;
            for _ in 0..count {
                words.push_reserved(0);
            }
            Words::Funded(words)
        } else {
            Words::Local(OwnedAllocation::try_filled_copy(count, 0)?)
        };
        Ok(Self {
            words,
            chunks,
            count: 0,
        })
    }

    pub(crate) fn try_copy(
        &self,
        lease: Option<&mut ExecutionMemoryLease>,
    ) -> Result<Self, VMError> {
        let mut copy = Self::new(self.chunks, lease)?;
        copy.copy_from(self);
        Ok(copy)
    }

    pub(crate) fn copy_from(&mut self, other: &Self) {
        assert_eq!(self.chunks, other.chunks, "dirty-leaf geometry mismatch");
        self.words.as_mut_slice().copy_from_slice(other.words());
        self.count = other.count;
    }

    pub(crate) fn insert(&mut self, index: usize) {
        assert!(index < self.chunks, "dirty leaf outside memory geometry");
        let mask = 1_u64 << (index % 64);
        let word = &mut self.words.as_mut_slice()[index / 64];
        self.count += usize::from(*word & mask == 0);
        *word |= mask;
    }

    #[cfg(test)]
    pub(crate) fn extend(&mut self, indices: impl IntoIterator<Item = usize>) {
        for index in indices {
            self.insert(index);
        }
    }

    pub(crate) fn clear(&mut self) {
        self.words.as_mut_slice().fill(0);
        self.count = 0;
    }

    pub(crate) fn len(&self) -> usize {
        self.count
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub(crate) fn chunks(&self) -> usize {
        self.chunks
    }

    pub(crate) fn words(&self) -> &[u64] {
        self.words.as_slice()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.words().iter().enumerate().flat_map(|(index, word)| {
            let mut remaining = *word;
            std::iter::from_fn(move || {
                if remaining == 0 {
                    return None;
                }
                let offset = remaining.trailing_zeros() as usize;
                remaining &= remaining - 1;
                Some(index * 64 + offset)
            })
        })
    }

    pub(crate) fn try_retain(&self) -> bool {
        match &self.words {
            Words::Local(words) => words.try_retain(),
            Words::Funded(words) => words.try_retain(),
        }
    }

    pub(crate) fn make_active(&self) {
        match &self.words {
            Words::Local(words) => words.make_active(),
            Words::Funded(words) => words.activate(),
        }
    }
}

impl std::fmt::Debug for DirtyChunks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DirtyChunks")
            .field("chunks", &self.chunks)
            .field("count", &self.count)
            .field("words", &self.words())
            .finish()
    }
}

impl PartialEq for DirtyChunks {
    fn eq(&self, other: &Self) -> bool {
        self.chunks == other.chunks && self.words() == other.words()
    }
}

impl Eq for DirtyChunks {}

#[cfg(test)]
mod tests;
