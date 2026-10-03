//! One shared prepared shell, prepaid and reclaimed with its original owner.

use super::PreparedContractInner;
use crate::{
    VMError,
    cache_memory::{MemoryReservation, strong_owner::StrongOwner},
    error::ExecutionDeferral,
};
use iroha_allocation::{AllocationBudget, ChargedShared, ReservedChargedShared};
use std::ops::Deref;

#[derive(Clone)]
pub(super) enum PreparedOwner {
    Local(StrongOwner<PreparedContractInner>),
    Funded(ChargedShared<PreparedContractInner>),
}
pub(super) enum PreparedShell {
    Local,
    Funded(ReservedChargedShared<PreparedContractInner>),
}
impl PreparedShell {
    pub(super) fn reserve(
        budget: Option<&AllocationBudget>,
    ) -> Result<(Self, MemoryReservation), VMError> {
        let Some(budget) = budget else {
            let bytes = std::alloc::Layout::new::<(usize, usize)>()
                .extend(std::alloc::Layout::new::<PreparedContractInner>())
                .expect("fixed prepared Arc layout fits host address space")
                .0
                .pad_to_align()
                .size();
            return Ok((Self::Local, MemoryReservation::active(bytes)));
        };
        let layout = ChargedShared::<PreparedContractInner>::allocation_layout();
        let mut admission = budget
            .try_reserve(layout)
            .map_err(VMError::AllocationDeferred)?;
        let retention = MemoryReservation::active(layout.size());
        let shell = ChargedShared::reserve_from(&mut admission)
            .map_err(|_| VMError::ExecutionDeferred(ExecutionDeferral::AllocationUnavailable))?;
        Ok((Self::Funded(shell), retention))
    }
    pub(super) fn initialize(self, value: PreparedContractInner) -> PreparedOwner {
        match self {
            Self::Local => PreparedOwner::Local(StrongOwner::new(value)),
            Self::Funded(shell) => PreparedOwner::Funded(shell.initialize(value)),
        }
    }
}
impl PreparedOwner {
    pub(super) fn ptr_eq(left: &Self, right: &Self) -> bool {
        match (left, right) {
            (Self::Local(left), Self::Local(right)) => StrongOwner::ptr_eq(left, right),
            (Self::Funded(left), Self::Funded(right)) => ChargedShared::ptr_eq(left, right),
            _ => false,
        }
    }
}
impl Deref for PreparedOwner {
    type Target = PreparedContractInner;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Local(value) => value,
            Self::Funded(value) => value,
        }
    }
}

#[cfg(test)]
mod tests;
