//! Fixed entrypoint indexes and bounded reusable reachability scratch.

use super::PreparedControlFlow;
use crate::{
    VMError,
    cache_memory::OwnedAllocation,
    error::ExecutionDeferral,
    execution_memory::{ExecutionBuffer, ExecutionMemoryLease, ExecutionMemoryPlan},
    instruction::wide,
    ivm_cache::DecodedOp,
    metadata::EmbeddedEntrypointDescriptor,
};
use iroha_allocation::{AllocationBudget, ChargedBufferError, PrepaidBufferError};

enum Fixed<T> {
    Local(OwnedAllocation<T>),
    Funded(ExecutionBuffer<T>),
}
impl<T: Copy + Default> Fixed<T> {
    fn new(len: usize, budget: Option<&AllocationBudget>) -> Result<Self, VMError> {
        let Some(budget) = budget else {
            return OwnedAllocation::try_filled_copy(len, T::default()).map(Self::Local);
        };
        let plan = ExecutionMemoryPlan::array::<T>(len).map_err(VMError::AllocationDeferred)?;
        let mut lease =
            ExecutionMemoryLease::reserve(budget, plan).map_err(VMError::AllocationDeferred)?;
        let mut values = ExecutionBuffer::new(len, &mut lease).map_err(|error| match error {
            PrepaidBufferError::Allocation(ChargedBufferError::Admission(error)) => {
                VMError::AllocationDeferred(error)
            }
            PrepaidBufferError::Allocation(ChargedBufferError::Allocator { .. })
            | PrepaidBufferError::Reservation(_) => {
                VMError::ExecutionDeferred(ExecutionDeferral::AllocationUnavailable)
            }
        })?;
        for _ in 0..len {
            values.push_reserved(T::default());
        }
        Ok(Self::Funded(values))
    }
}
impl<T> Fixed<T> {
    fn values(&self) -> &[T] {
        match self {
            Self::Local(values) => values,
            Self::Funded(values) => values.as_slice(),
        }
    }
    fn values_mut(&mut self) -> &mut [T] {
        match self {
            Self::Local(values) => values,
            Self::Funded(values) => values.as_mut_slice(),
        }
    }
    fn try_retain(&self) -> bool {
        match self {
            Self::Local(values) => values.try_retain(),
            Self::Funded(values) => values.try_retain(),
        }
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct EntrypointIndex {
    pub(super) descriptor_index: usize,
    pub(super) absolute_pc: u64,
    pub(super) requires_private_inputs: bool,
}

pub(super) struct Entrypoints(Fixed<EntrypointIndex>);
impl Entrypoints {
    pub(super) fn prepare(
        descriptors: &[EmbeddedEntrypointDescriptor],
        decoded: &[DecodedOp],
        graph: &PreparedControlFlow,
        instruction_entry_pc: u64,
        budget: Option<&AllocationBudget>,
    ) -> Result<Self, VMError> {
        let mut entries = Fixed::new(descriptors.len(), budget)?;
        let mut scratch = Reachability::new(
            if descriptors.is_empty() {
                0
            } else {
                decoded.len()
            },
            budget,
        )?;
        for (index, descriptor) in descriptors.iter().enumerate() {
            entries.values_mut()[index] = EntrypointIndex {
                descriptor_index: index,
                absolute_pc: instruction_entry_pc
                    .checked_add(descriptor.entry_pc)
                    .ok_or(VMError::DecodeError)?,
                requires_private_inputs: scratch.reaches_private_input(
                    decoded,
                    graph,
                    descriptor.entry_pc,
                )?,
            };
        }
        // Sorting moves inline indexes only. Names stay with their immutable
        // metadata owner; no String clone or sorting buffer is allocated.
        entries.values_mut().sort_unstable_by(|left, right| {
            descriptors[left.descriptor_index]
                .name
                .cmp(&descriptors[right.descriptor_index].name)
        });
        if entries.values().windows(2).any(|pair| {
            descriptors[pair[0].descriptor_index].name == descriptors[pair[1].descriptor_index].name
        }) {
            return Err(VMError::DecodeError);
        }
        Ok(Self(entries))
    }
    pub(super) fn get(
        &self,
        descriptors: &[EmbeddedEntrypointDescriptor],
        name: &str,
    ) -> Option<&EntrypointIndex> {
        let index = self
            .0
            .values()
            .binary_search_by(|entry| descriptors[entry.descriptor_index].name.as_str().cmp(name))
            .ok()?;
        self.0.values().get(index)
    }
    pub(super) fn len(&self) -> usize {
        self.0.values().len()
    }
    pub(super) fn try_retain(&self) -> bool {
        self.0.try_retain()
    }
}

#[derive(Clone, Copy, Default)]
struct Visit {
    discovered: bool,
    queued_index: usize,
}

struct Reachability(Fixed<Visit>);
impl Reachability {
    fn new(instructions: usize, budget: Option<&AllocationBudget>) -> Result<Self, VMError> {
        Fixed::new(instructions, budget).map(Self)
    }
    fn reaches_private_input(
        &mut self,
        decoded: &[DecodedOp],
        graph: &PreparedControlFlow,
        entry_pc: u64,
    ) -> Result<bool, VMError> {
        let slots = self.0.values_mut();
        slots.fill(Visit::default());
        let entry = decoded
            .binary_search_by_key(&entry_pc, |op| op.pc)
            .map_err(|_| VMError::DecodeError)?;
        slots.get_mut(entry).ok_or(VMError::DecodeError)?.discovered = true;
        slots[0].queued_index = entry;
        let (mut head, mut tail) = (0, 1);
        while head < tail {
            let index = slots[head].queued_index;
            head += 1;
            let op = decoded.get(index).ok_or(VMError::DecodeError)?;
            let syscall = match wide::opcode(op.inst) {
                wide::system::SCALL => Some(u32::from(wide::imm8(op.inst) as u8)),
                wide::system::SYSTEM => Some(crate::encoding::wide::decode_syscallx(op.inst)),
                _ => None,
            };
            if syscall == Some(crate::syscalls::SYSCALL_GET_PRIVATE_INPUT) {
                return Ok(true);
            }
            for pc in graph.node(op.pc).ok_or(VMError::DecodeError)?.successors() {
                let next = decoded
                    .binary_search_by_key(pc, |op| op.pc)
                    .map_err(|_| VMError::DecodeError)?;
                if !slots[next].discovered {
                    // Mark on enqueue: every instruction enters the queue once,
                    // even for loops, shared joins and duplicate successors.
                    slots[next].discovered = true;
                    slots
                        .get_mut(tail)
                        .ok_or(VMError::DecodeError)?
                        .queued_index = next;
                    tail += 1;
                }
            }
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests;
