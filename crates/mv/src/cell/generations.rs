//! Exact original physical EBR backing before acquiring either Cell writer.

use super::*;
use crate::allocation::{AllocationBudget, AllocationCharge};
use concread::ebrcell::ReservedEbrCell;
use std::fmt;

/// Local refusal before any original Cell writer is acquired.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellGenerationBackingError {
    /// At least one original charge belongs to another finite pool.
    ForeignPool,
    /// One charge differs from the exact requested size or alignment.
    LayoutMismatch {
        /// Actual backing layout required by this Cell generation.
        expected: Layout,
        /// Layout retained by the unchanged refused charge.
        actual: Layout,
    },
    /// Physical allocation failed after the complete logical admission.
    Allocator {
        /// Exact original backing layout that was refused.
        layout: Layout,
    },
}
impl fmt::Display for CellGenerationBackingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Cell generation backing refused: {self:?}")
    }
}
impl std::error::Error for CellGenerationBackingError {}

/// The original pair of physical current/undo shells, with exact pool custody.
/// This never clones a value or obtains a writer. The enclosing aggregate must
/// finish every physical reservation before acquiring its first writer.
#[must_use = "retain both original physical generations through acquisition"]
pub struct CellGenerationBacking<V: Value> {
    current: ReservedEbrCell<V, AllocationCharge>,
    undo: ReservedEbrCell<Option<V>, AllocationCharge>,
    source: AllocationBudget,
}
impl<V: Value> CellGenerationBacking<V> {
    /// Consume the original complete pair admission without reserving new capacity.
    /// Wrong pool/layout or either physical refusal returns both unchanged charges.
    /// If the second allocation fails, the first unused shell is freed before its
    /// original charge is returned; no charge is refunded or replaced.
    pub fn try_from_charges(
        source: &AllocationBudget,
        charges: CellAllocationCharges<AllocationCharge>,
    ) -> Result<
        Self,
        (
            CellAllocationCharges<AllocationCharge>,
            CellGenerationBackingError,
        ),
    > {
        if !charges.current.belongs_to(source) || !charges.undo.belongs_to(source) {
            return Err((charges, CellGenerationBackingError::ForeignPool));
        }
        let [current, undo] = Cell::<V, AllocationCharge>::allocation_layouts();
        let mismatch = if charges.current.layout() != current {
            Some((current, charges.current.layout()))
        } else if charges.undo.layout() != undo {
            Some((undo, charges.undo.layout()))
        } else {
            None
        };
        if let Some((expected, actual)) = mismatch {
            return Err((
                charges,
                CellGenerationBackingError::LayoutMismatch { expected, actual },
            ));
        }
        let CellAllocationCharges { current, undo } = charges;
        let current = match ReservedEbrCell::try_new(current) {
            Ok(current) => current,
            Err((current, layout)) => {
                return Err((
                    CellAllocationCharges::new(current, undo),
                    CellGenerationBackingError::Allocator { layout },
                ));
            }
        };
        let undo = match ReservedEbrCell::try_new(undo) {
            Ok(undo) => undo,
            Err((undo, layout)) => {
                return Err((
                    CellAllocationCharges::new(current.into_charge(), undo),
                    CellGenerationBackingError::Allocator { layout },
                ));
            }
        };
        Ok(Self {
            current,
            undo,
            source: source.clone(),
        })
    }

    /// Compare exact original pools; equal limits or remaining bytes are insufficient.
    pub fn belongs_to(&self, source: &AllocationBudget) -> bool {
        self.source.same_pool(source)
    }

    pub(super) fn into_original(
        self,
    ) -> (
        ReservedEbrCell<V, AllocationCharge>,
        ReservedEbrCell<Option<V>, AllocationCharge>,
    ) {
        (self.current, self.undo)
    }
}
