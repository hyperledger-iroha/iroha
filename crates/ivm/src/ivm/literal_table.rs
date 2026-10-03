//! Immutable literal indexes retaining their original allocation owner.

use crate::{
    SyscallPolicy, VMError,
    cache_memory::SharedAllocation,
    metadata::{LiteralDirectory, ParsedLiteralSection, ValidatedLiteral},
};
use iroha_allocation::AllocationBudget;

/// One admission-validated value in an indexed literal table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DecodedLiteral {
    /// Exact start address of a validated pointer-ABI TLV.
    Pointer(u64),
    /// Exact public scalar bits, which never confer pointer provenance.
    I64(u64),
}

/// Independent immutable indexes; absent indexes have no shared control allocation.
#[derive(Clone, Debug)]
pub(crate) struct DecodedLiteralTable {
    entries: Option<SharedAllocation<DecodedLiteral>>,
    pointer_starts: Option<SharedAllocation<u64>>,
}

impl DecodedLiteralTable {
    pub(super) const fn empty() -> Self {
        Self {
            entries: None,
            pointer_starts: None,
        }
    }

    /// Return values in their authenticated descriptor order.
    pub(crate) fn entries(&self) -> &[DecodedLiteral] {
        self.entries.as_deref().unwrap_or(&[])
    }

    pub(super) fn pointer_starts(&self) -> &[u64] {
        self.pointer_starts.as_deref().unwrap_or(&[])
    }

    pub(crate) fn try_retain(&self) -> bool {
        self.entries
            .as_ref()
            .is_none_or(SharedAllocation::try_retain)
            && self
                .pointer_starts
                .as_ref()
                .is_none_or(SharedAllocation::try_retain)
    }

    /// Rebind only foreign or diagnostic storage before installing any guest state.
    pub(super) fn for_budget(&self, budget: &AllocationBudget) -> Result<Self, VMError> {
        fn bind<T: Copy>(
            values: &Option<SharedAllocation<T>>,
            budget: &AllocationBudget,
        ) -> Result<Option<SharedAllocation<T>>, VMError> {
            values
                .as_ref()
                .map(|values| {
                    if values.belongs_to(budget) {
                        Ok(values.clone())
                    } else {
                        SharedAllocation::try_copy_from_slice_with_memory_budget(values, budget)
                    }
                })
                .transpose()
        }
        Ok(Self {
            entries: bind(&self.entries, budget)?,
            pointer_starts: bind(&self.pointer_starts, budget)?,
        })
    }
}

/// Decode exact native values only after the canonical borrowed validator succeeds.
pub(crate) fn decode_literal_table(
    program: &[u8],
    header_len: usize,
    section: Option<ParsedLiteralSection>,
    policy: SyscallPolicy,
    budget: Option<&AllocationBudget>,
) -> Result<DecodedLiteralTable, VMError> {
    let directory = LiteralDirectory::validate(program, header_len, section, policy)?;
    let entries = collect(
        directory.iter().map(|value| {
            Ok(match value {
                ValidatedLiteral::Pointer { address, .. } => DecodedLiteral::Pointer(address),
                ValidatedLiteral::I64(bits) => DecodedLiteral::I64(bits),
            })
        }),
        budget,
    )?;
    let pointer_starts = collect(directory.pointer_addresses().map(Ok), budget)?;
    Ok(DecodedLiteralTable {
        entries,
        pointer_starts,
    })
}

fn collect<T: Copy>(
    values: impl ExactSizeIterator<Item = Result<T, VMError>>,
    budget: Option<&AllocationBudget>,
) -> Result<Option<SharedAllocation<T>>, VMError> {
    if values.len() == 0 {
        return Ok(None);
    }
    match budget {
        Some(budget) => SharedAllocation::try_from_iter_with_memory_budget(values, budget),
        None => SharedAllocation::try_from_iter(values),
    }
    .map(Some)
}

#[cfg(test)]
mod tests;
