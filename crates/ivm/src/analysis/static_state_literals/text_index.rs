//! Original-budget borrowed text index and separately admitted NFC scratch.

use super::*;
use crate::{
    VMError,
    error::ExecutionDeferral,
    execution_memory::{ExecutionBuffer, ExecutionMemoryLease, ExecutionMemoryPlan},
};
use iroha_allocation::{AllocationBudget, ChargedBufferError, PrepaidBufferError};

#[derive(Clone, Copy)]
enum Text<'a> {
    Absent,
    Name(&'a str),
    Path(&'a str),
}
impl Text<'_> {
    fn scratch_bytes(self) -> usize {
        match self {
            Self::Absent => 0,
            Self::Name(text) => Name::canonical_validation_scratch_bytes(text),
            Self::Path(text) => StatePath::canonical_validation_scratch_bytes(text),
        }
    }

    fn validated(self) -> Self {
        let valid = match self {
            Self::Absent => true,
            Self::Name(text) => Name::validate_canonical(text).is_ok(),
            Self::Path(text) => StatePath::validate_canonical(text).is_ok(),
        };
        if valid { self } else { Self::Absent }
    }
}

/// Borrowed text does not outlive the original prepared artifact. Only this
/// exact transient index allocation is owned; published keys have their own
/// immutable final-owner allocation. No Name/StatePath or text clone is stored.
pub(super) struct TextIndex<'a> {
    entries: ExecutionBuffer<Text<'a>>,
}

impl<'a> TextIndex<'a> {
    pub(super) fn new(
        contract: &'a PreparedContract,
        budget: &AllocationBudget,
    ) -> Result<Self, VMError> {
        Self::from_candidates(
            contract
                .literal_table()
                .entries()
                .iter()
                .map(|literal| candidate(contract, literal)),
            budget,
        )
    }

    fn from_candidates(
        candidates: impl ExactSizeIterator<Item = Text<'a>> + Clone,
        budget: &AllocationBudget,
    ) -> Result<Self, VMError> {
        let count = candidates.len();
        // Both passes borrow one unchanged original literal slice.
        // This pass uses only original bytes, typed canonical framing and
        // preliminary syntax. It performs no ICU call or heap allocation.
        let scratch_bytes = candidates
            .clone()
            .map(Text::scratch_bytes)
            .max()
            .unwrap_or(0);
        let mut plan =
            ExecutionMemoryPlan::array::<Text<'a>>(count).map_err(VMError::AllocationDeferred)?;
        plan.include_child(
            ExecutionMemoryPlan::array::<u8>(scratch_bytes).map_err(VMError::AllocationDeferred)?,
        )
        .map_err(VMError::AllocationDeferred)?;
        let mut lease =
            ExecutionMemoryLease::reserve(budget, plan).map_err(VMError::AllocationDeferred)?;
        let mut entries = ExecutionBuffer::new(count, &mut lease).map_err(buffer_error)?;
        // The unpartitioned lease covers the audited cumulative ICU/sort
        // layouts, not another physical byte buffer. Sequential validations
        // reuse its maximum only after each previous ICU owner has dropped.
        for candidate in candidates {
            entries.push_reserved(candidate.validated());
        }
        Ok(Self { entries })
    }

    pub(super) fn name(&self, index: usize) -> Option<&str> {
        match self.entries.as_slice().get(index)? {
            Text::Name(text) => Some(text),
            _ => None,
        }
    }

    pub(super) fn path(&self, index: usize) -> Option<&str> {
        match self.entries.as_slice().get(index)? {
            Text::Path(text) => Some(text),
            _ => None,
        }
    }
}

fn candidate<'a>(contract: &'a PreparedContract, literal: &DecodedLiteral) -> Text<'a> {
    let DecodedLiteral::Pointer(pointer) = literal else {
        return Text::Absent;
    };
    let Some(bytes) = authenticated_literal_tlv_bytes(contract, *pointer) else {
        return Text::Absent;
    };
    let Ok(tlv) = validate_tlv_bytes(bytes) else {
        return Text::Absent;
    };
    match tlv.type_id {
        PointerType::Name
            if tlv.payload.len() <= crate::syscalls::STATE_MAP_MAX_BASE_FRAME_BYTES =>
        {
            norito::core::borrow_canonical_text::<Name>(tlv.payload)
                .map(Text::Name)
                .unwrap_or(Text::Absent)
        }
        PointerType::NoritoBytes
            if tlv.payload.len() <= crate::syscalls::STATE_MAX_PATH_FRAME_BYTES =>
        {
            norito::core::borrow_canonical_text::<StatePath>(tlv.payload)
                .ok()
                .filter(|text| text.len() <= crate::syscalls::STATE_MAX_PATH_BYTES)
                .map(Text::Path)
                .unwrap_or(Text::Absent)
        }
        _ => Text::Absent,
    }
}

fn buffer_error(error: PrepaidBufferError) -> VMError {
    match error {
        PrepaidBufferError::Allocation(ChargedBufferError::Admission(error)) => {
            VMError::AllocationDeferred(error)
        }
        PrepaidBufferError::Allocation(ChargedBufferError::Allocator { .. })
        | PrepaidBufferError::Reservation(_) => {
            VMError::ExecutionDeferred(ExecutionDeferral::AllocationUnavailable)
        }
    }
}

#[cfg(test)]
mod tests;
