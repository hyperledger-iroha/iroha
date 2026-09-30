//! Original finite custody for both initial EBR values and their shared controls.

use super::*;
use concread::ebrcell::ReservedEbrCell;
use iroha_allocation::{AllocationBudget, AllocationCharge, AllocationRefusal};
use std::fmt;

/// A local capacity or physical-allocation refusal before initial payload movement.
#[derive(Debug)]
pub enum CellInitializationError {
    /// The original finite pool cannot admit the complete initial overlap.
    Admission(AllocationRefusal),
    /// The physical allocator refused one prepaid backing allocation.
    Allocator {
        /// Exact layout of that original backing.
        layout: Layout,
    },
}
impl fmt::Display for CellInitializationError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admission(error) => write!(output, "initial Cell admission: {error}"),
            Self::Allocator { layout } => {
                write!(output, "initial Cell allocator refused {layout:?}")
            }
        }
    }
}
impl std::error::Error for CellInitializationError {}

/// All seven original initial allocations, acquired before moving either payload.
///
/// Each charge follows its actual EBR or shared allocation through readers and
/// observations. This covers requested outer layouts, not nested payloads,
/// platform mutex internals, waiter storage or epoch collector bookkeeping.
#[must_use = "initialize or abandon the exact original Cell allocations"]
pub struct CellInitialization<V: Value> {
    current: ReservedEbrCell<V, AllocationCharge>,
    undo: ReservedEbrCell<Option<V>, AllocationCharge>,
    publication: Publication,
    current_released: ReleaseNotification,
    undo_released: ReleaseNotification,
}
impl<V: Value> CellInitialization<V> {
    /// Exact EBR, initial identity and original notification backing layouts.
    pub fn allocation_layouts() -> [Layout; 7] {
        let [current, undo] = Cell::<V, AllocationCharge>::allocation_layouts();
        let [owner, version, identity_release] = Publication::initial_layouts();
        let release = ReleaseNotification::allocation_layout::<AllocationCharge>();
        [
            current,
            undo,
            owner,
            version,
            identity_release,
            release,
            release,
        ]
    }

    /// Reserve the complete overlap atomically from this original pool, then
    /// acquire every physical backing before a caller constructs either payload.
    /// Every partial refusal returns all unused capacity after physical cleanup.
    pub fn try_reserve(budget: &AllocationBudget) -> Result<Self, CellInitializationError> {
        let mut original = budget
            .try_reserve_layouts(Self::allocation_layouts())
            .map_err(CellInitializationError::Admission)?;
        let publication = Publication::try_from_original(&mut original).map_err(|error| {
            CellInitializationError::Allocator {
                layout: error.layout(),
            }
        })?;
        let release_layout = ReleaseNotification::allocation_layout::<AllocationCharge>();
        let current_released = ReleaseNotification::try_new_charged(
            original
                .try_split(release_layout)
                .expect("original current release demand"),
        )
        .map_err(|(charge, error)| {
            drop(charge);
            CellInitializationError::Allocator {
                layout: error.layout(),
            }
        })?;
        let undo_released = ReleaseNotification::try_new_charged(
            original
                .try_split(release_layout)
                .expect("original undo release demand"),
        )
        .map_err(|(charge, error)| {
            drop(charge);
            CellInitializationError::Allocator {
                layout: error.layout(),
            }
        })?;
        let [current_layout, undo_layout] = Cell::<V, AllocationCharge>::allocation_layouts();
        let current = ReservedEbrCell::try_new(
            original
                .try_split(current_layout)
                .expect("original current demand"),
        )
        .map_err(|(charge, layout)| {
            drop(charge);
            CellInitializationError::Allocator { layout }
        })?;
        let undo = ReservedEbrCell::try_new(
            original
                .try_split(undo_layout)
                .expect("original undo demand"),
        )
        .map_err(|(charge, layout)| {
            drop(charge);
            CellInitializationError::Allocator { layout }
        })?;
        assert_eq!(original.remaining_bytes(), 0);
        Ok(Self {
            current,
            undo,
            publication,
            current_released,
            undo_released,
        })
    }

    /// Move both payloads into their original allocations; this operation neither
    /// clones nor allocates, and grants no aggregate State publication authority.
    pub fn initialize(self, current_value: V, undo_value: Option<V>) -> Cell<V, AllocationCharge> {
        Cell {
            publication: self.publication,
            revert_released: self.undo_released,
            blocks_released: self.current_released,
            revert: self.undo.initialize(undo_value),
            blocks: self.current.initialize(current_value),
        }
    }
}

#[cfg(test)]
#[path = "initial_tests.rs"]
mod tests;
