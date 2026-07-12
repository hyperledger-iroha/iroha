use std::collections::{BTreeMap, HashMap};
use std::ops::DerefMut;
use std::sync::{Arc, Mutex, OnceLock};

use itertools::Itertools;
use rayon::slice::ParallelSliceMut;

use crate::halo2_proofs::{
    circuit::{Cell, Region},
    plonk::{Assigned, Column, Fixed},
};
use crate::utils::halo2::{raw_assign_fixed, raw_constrain_equal, Halo2AssignedCell};
use crate::AssignedValue;
use crate::{ff::Field, ContextCell};

use super::manager::VirtualRegionManager;

/// Type ID to distinguish external raw Halo2 cells. **This Type ID must be unique.**
pub const EXTERNAL_CELL_TYPE_ID: &str = "halo2-base:External Raw Halo2 Cell";

/// Thread-safe shared global manager for all copy constraints.
pub type SharedCopyConstraintManager<F> = Arc<Mutex<CopyConstraintManager<F>>>;

/// Global manager for all copy constraints. Thread-safe.
///
/// This will only be accessed during key generation, not proof generation, so it does not need to be optimized.
///
/// Implements [VirtualRegionManager], which should be assigned only after all cells have been assigned
/// by other managers.
#[derive(Clone, Default, Debug)]
pub struct CopyConstraintManager<F: Field + Ord> {
    /// A [Vec] tracking equality constraints between pairs of virtual advice cells, tagged by [ContextCell].
    /// These can be across different virtual regions.
    pub advice_equalities: Vec<(ContextCell, ContextCell)>,

    /// A [Vec] tracking equality constraints between virtual advice cell and fixed values.
    /// Fixed values will only be added once globally.
    pub constant_equalities: Vec<(F, ContextCell)>,

    external_cell_count: usize,

    // In circuit assignments
    /// Advice assignments, mapping from virtual [ContextCell] to assigned physical [Cell]
    pub assigned_advices: HashMap<ContextCell, Cell>,
    /// Constant assignments, (key = constant, value = [Cell])
    pub assigned_constants: BTreeMap<F, Cell>,
    /// Flag for whether `assign_raw` has been called, for safety only.
    assigned: OnceLock<()>,
}

impl<F: Field + Ord> CopyConstraintManager<F> {
    /// Returns the number of distinct constants used.
    pub fn num_distinct_constants(&self) -> usize {
        self.constant_equalities.iter().map(|(x, _)| x).sorted().dedup().count()
    }

    /// Adds external raw [Halo2AssignedCell] to `self.assigned_advices` and returns a new virtual [AssignedValue]
    /// that can be used in any virtual region. No copy constraint is imposed, as the virtual cell "points" to the
    /// raw assigned cell. The returned [ContextCell] will have `type_id` the `TypeId::of::<Cell>()`.
    pub fn load_external_assigned(
        &mut self,
        assigned_cell: Halo2AssignedCell<F>,
    ) -> AssignedValue<F> {
        let context_cell = self.load_external_cell(assigned_cell.cell());
        let mut value = Assigned::Trivial(F::ZERO);
        assigned_cell.value().map(|v| {
            #[cfg(feature = "halo2-axiom")]
            {
                value = **v;
            }
            #[cfg(not(feature = "halo2-axiom"))]
            {
                value = *v;
            }
        });
        AssignedValue { value, cell: Some(context_cell) }
    }

    /// Adds external raw Halo2 cell to `self.assigned_advices` and returns a new virtual cell that can be
    /// used as a tag (but will not be re-assigned). The returned [ContextCell] will have `type_id` the `TypeId::of::<Cell>()`.
    pub fn load_external_cell(&mut self, cell: Cell) -> ContextCell {
        self.load_external_cell_impl(Some(cell))
    }

    /// Mock to load an external cell for base circuit simulation. If any mock external cell is loaded, calling `assign_raw` will panic.
    pub fn mock_external_assigned(&mut self, v: F) -> AssignedValue<F> {
        let context_cell = self.load_external_cell_impl(None);
        AssignedValue { value: Assigned::Trivial(v), cell: Some(context_cell) }
    }

    fn load_external_cell_impl(&mut self, cell: Option<Cell>) -> ContextCell {
        let context_cell = ContextCell::new(EXTERNAL_CELL_TYPE_ID, 0, self.external_cell_count);
        self.external_cell_count += 1;
        if let Some(cell) = cell {
            if let Some(old_cell) = self.assigned_advices.insert(context_cell, cell) {
                assert!(
                    old_cell.row_offset == cell.row_offset && old_cell.column == cell.column,
                    "External cell already assigned"
                )
            }
        }
        context_cell
    }

    /// Clears state
    pub fn clear(&mut self) {
        self.advice_equalities.clear();
        self.constant_equalities.clear();
        self.assigned_advices.clear();
        self.assigned_constants.clear();
        self.external_cell_count = 0;
        self.assigned.take();
    }
}

impl<F: Field + Ord> Drop for CopyConstraintManager<F> {
    fn drop(&mut self) {
        if self.assigned.get().is_some() {
            return;
        }
        if !self.advice_equalities.is_empty() {
            log::warn!("WARNING: advice_equalities not empty");
        }
        if !self.constant_equalities.is_empty() {
            log::warn!("WARNING: constant_equalities not empty");
        }
    }
}

impl<F: Field + Ord> VirtualRegionManager<F> for SharedCopyConstraintManager<F> {
    // The fixed columns
    type Config = Vec<Column<Fixed>>;
    type Assignment = ();

    /// This should be the last manager to be assigned, after all other managers have assigned cells.
    fn assign_raw(&self, config: &Self::Config, region: &mut Region<F>) -> Self::Assignment {
        let mut guard = self.lock().unwrap();
        let manager = guard.deref_mut();
        // sort by constant so constant assignment order is deterministic
        // this is necessary because constants can be assigned by multiple CPU threads
        // We further sort by ContextCell because the backend implementation of `raw_constrain_equal` (permutation argument) seems to depend on the order you specify copy constraints...
        manager
            .constant_equalities
            .par_sort_unstable_by(|(c1, cell1), (c2, cell2)| c1.cmp(c2).then(cell1.cmp(cell2)));
        // Assign fixed cells, we go left to right, then top to bottom, to avoid needing to know number of rows here
        let mut fixed_col = 0;
        let mut fixed_offset = 0;
        for (c, _) in manager.constant_equalities.iter() {
            if !manager.assigned_constants.contains_key(c) {
                // this will panic if you run out of rows
                let cell = raw_assign_fixed(region, config[fixed_col], fixed_offset, *c);
                manager.assigned_constants.insert(*c, cell);
                fixed_col += 1;
                if fixed_col >= config.len() {
                    fixed_col = 0;
                    fixed_offset += 1;
                }
            }
        }

        // Just in case: we sort by ContextCell because the backend implementation of `raw_constrain_equal` (permutation argument) seems to depend on the order you specify copy constraints...
        manager.advice_equalities.par_sort_unstable();
        // Impose equality constraints between assigned advice cells
        // At this point we assume all cells have been assigned by other VirtualRegionManagers
        for (left, right) in &manager.advice_equalities {
            let left = manager.assigned_advices.get(left).expect("virtual cell not assigned");
            let right = manager.assigned_advices.get(right).expect("virtual cell not assigned");
            raw_constrain_equal(region, *left, *right);
        }
        for (left, right) in &manager.constant_equalities {
            let left = manager.assigned_constants[left];
            let right = manager.assigned_advices.get(right).expect("virtual cell not assigned");
            raw_constrain_equal(region, left, *right);
        }
        // We can't clear advice_equalities and constant_equalities because keygen_vk and keygen_pk will call this function twice
        let _ = manager.assigned.set(());
        // When keygen_vk and keygen_pk are both run, you need to clear assigned constants
        // so the second run still assigns constants in the pk
        manager.assigned_constants.clear();
    }
}
