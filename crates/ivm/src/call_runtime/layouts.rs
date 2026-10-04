//! Once-per-loaded-image layouts with original-pool allocation custody.

use iroha_allocation::AllocationBudget;
use ivm_abi::call::CallNodeLayoutV1;

use crate::{
    VMError,
    cache_memory::{MemoryReservation, OwnedAllocation, strong_owner::StrongOwner},
    call_frame::CallFrameShape,
    error::ExecutionDeferral,
    execution_memory::{ExecutionBuffer, ExecutionMemoryLease, ExecutionMemoryPlan},
    metadata::EmbeddedContractInterfaceV1,
};

enum Storage<T> {
    Local(OwnedAllocation<T>),
    Funded(ExecutionBuffer<T>),
}

impl<T: Copy + Default> Storage<T> {
    fn new(len: usize, lease: Option<&mut ExecutionMemoryLease>) -> Result<Self, VMError> {
        if let Some(lease) = lease {
            let mut buffer = ExecutionBuffer::new(len, lease).map_err(|_| {
                VMError::ExecutionDeferred(ExecutionDeferral::AllocationUnavailable)
            })?;
            for _ in 0..len {
                buffer.push_reserved(T::default());
            }
            Ok(Self::Funded(buffer))
        } else {
            Ok(Self::Local(OwnedAllocation::try_filled_copy(
                len,
                T::default(),
            )?))
        }
    }
}

impl<T> Storage<T> {
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
pub(super) struct CallableLayout {
    pub(super) frame: CallFrameShape,
    pub(super) arguments: usize,
    pub(super) results: usize,
}

#[derive(Clone)]
pub(crate) struct CallLayouts(StrongOwner<CallLayoutsData>);

struct CallLayoutsData {
    callables: Storage<CallableLayout>,
    nodes: Storage<CallNodeLayoutV1>,
    reservation: MemoryReservation,
    // Original owner credit outlives both buffers and the StrongOwner allocation.
    _allocation_lease: Option<ExecutionMemoryLease>,
}

impl CallLayouts {
    pub(crate) fn prepare(
        interface: &EmbeddedContractInterfaceV1,
        budget: Option<&AllocationBudget>,
    ) -> Result<Self, VMError> {
        let mut count = 0_usize;
        for callable in &interface.callables {
            if !callable.validate() {
                return Err(VMError::InvalidMetadata);
            }
            count = count
                .checked_add(callable.arguments.nodes.len())
                .and_then(|count| count.checked_add(callable.results.nodes.len()))
                .ok_or(VMError::InvalidMetadata)?;
        }
        let mut plan = ExecutionMemoryPlan::array::<CallableLayout>(interface.callables.len())
            .map_err(VMError::AllocationDeferred)?;
        plan.include_child(
            ExecutionMemoryPlan::array::<CallNodeLayoutV1>(count)
                .map_err(VMError::AllocationDeferred)?,
        )
        .map_err(VMError::AllocationDeferred)?;
        let owner_bytes =
            norito::core::owned_arc_allocation_bytes::<CallLayoutsData>().map_err(|_| {
                VMError::AllocationDeferred(iroha_allocation::AllocationRefusal::DemandOverflow)
            })?;
        plan.include_child(
            ExecutionMemoryPlan::array::<u8>(owner_bytes).map_err(VMError::AllocationDeferred)?,
        )
        .map_err(VMError::AllocationDeferred)?;
        let mut lease = budget
            .map(|budget| {
                ExecutionMemoryLease::reserve(budget, plan).map_err(VMError::AllocationDeferred)
            })
            .transpose()?;
        let mut callables = Storage::new(interface.callables.len(), lease.as_mut())?;
        let mut nodes = Storage::new(count, lease.as_mut())?;
        let mut offset = 0;
        for (index, callable) in interface.callables.iter().enumerate() {
            let arguments = offset;
            offset += callable.arguments.nodes.len();
            let argument_words = callable
                .arguments
                .analyze_into(&mut nodes.values_mut()[arguments..offset])
                .ok_or(VMError::InvalidMetadata)?
                .word_count();
            let results = offset;
            offset += callable.results.nodes.len();
            let result_words = callable
                .results
                .analyze_into(&mut nodes.values_mut()[results..offset])
                .ok_or(VMError::InvalidMetadata)?
                .word_count();
            callables.values_mut()[index] = CallableLayout {
                frame: CallFrameShape {
                    entry_pc: callable.entry_pc,
                    frame_bytes: callable.frame_bytes,
                    argument_words,
                    result_words,
                },
                arguments,
                results,
            };
        }
        let reservation = MemoryReservation::active(owner_bytes);
        Ok(Self(StrongOwner::new(CallLayoutsData {
            callables,
            nodes,
            reservation,
            _allocation_lease: lease,
        })))
    }
    pub(super) fn callable(&self, index: usize) -> Result<CallableLayout, VMError> {
        self.0
            .callables
            .values()
            .get(index)
            .copied()
            .ok_or(VMError::DecodeError)
    }
    pub(super) fn node(&self, offset: usize, index: usize) -> Result<CallNodeLayoutV1, VMError> {
        self.0
            .nodes
            .values()
            .get(offset.checked_add(index).ok_or(VMError::DecodeError)?)
            .copied()
            .ok_or(VMError::DecodeError)
    }
    pub(crate) fn try_retain(&self) -> bool {
        self.0.reservation.try_retain()
            && self.0.callables.try_retain()
            && self.0.nodes.try_retain()
    }
}

#[cfg(test)]
#[path = "layouts/tests.rs"]
mod tests;
