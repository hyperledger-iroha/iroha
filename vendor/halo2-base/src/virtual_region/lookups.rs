use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};

use getset::{CopyGetters, Getters, Setters};

use crate::ff::Field;
use crate::halo2_proofs::{
    circuit::{Region, Value},
    plonk::{Advice, Column},
};
use crate::utils::halo2::{constrain_virtual_equals_external, raw_assign_advice};
use crate::{AssignedValue, ContextTag};

use super::copy_constraints::SharedCopyConstraintManager;
use super::manager::VirtualRegionManager;

/// Basic dynamic lookup table gadget.
pub mod basic;

/// A manager that can be used for any lookup argument. This manager automates
/// the process of copying cells to designed advice columns with lookup enabled.
/// It also manages how many such advice columns are necessary.
///
/// ## Detailed explanation
/// If we have a lookup argument that uses `ADVICE_COLS` advice columns and `TABLE_COLS` table columns, where
/// the table is either fixed or dynamic (advice), then we want to dynamically allocate chunks of `ADVICE_COLS` columns
/// that have the lookup into the table **always on** so that:
/// - every time we want to lookup [_; ADVICE_COLS] values, we copy them over to a row in the special
///
/// lookup-enabled advice columns.
/// - note that just for assignment, we don't need to know anything about the table itself.
///
/// Note: the manager does not need to know the value of `TABLE_COLS`.
///
/// We want this manager to be CPU thread safe, while ensuring that the resulting circuit is
/// deterministic -- the order in which the cells to lookup are added matters.
/// The current solution is to tag the cells to lookup with the context id from the [`Context`](crate::Context) in which
/// it was called, and add virtual cells sequentially to buckets labelled by id.
/// The virtual cells will be assigned to physical cells sequentially by id.
/// We use a `BTreeMap` for the buckets instead of sorting to cells, to ensure that the order of the cells
/// within a bucket is deterministic.
/// The assumption is that the [`Context`](crate::Context) is thread-local.
///
/// Cheap to clone across threads because everything is in [Arc].
#[derive(Clone, Debug, Getters, CopyGetters, Setters)]
pub struct LookupAnyManager<F: Field + Ord, const ADVICE_COLS: usize> {
    /// Shared cells to lookup, tagged by (type id, context id).
    #[allow(clippy::type_complexity)]
    pub cells_to_lookup: Arc<Mutex<BTreeMap<ContextTag, Vec<[AssignedValue<F>; ADVICE_COLS]>>>>,
    /// Global shared copy manager
    #[getset(get = "pub", set = "pub")]
    copy_manager: SharedCopyConstraintManager<F>,
    /// Specify whether constraints should be imposed for additional safety.
    #[getset(get_copy = "pub")]
    witness_gen_only: bool,
    /// Flag for whether `assign_raw` has been called, for safety only.
    pub(crate) assigned: Arc<OnceLock<()>>,
}

impl<F: Field + Ord, const ADVICE_COLS: usize> LookupAnyManager<F, ADVICE_COLS> {
    /// Creates a new [LookupAnyManager] with a given copy manager.
    pub fn new(witness_gen_only: bool, copy_manager: SharedCopyConstraintManager<F>) -> Self {
        Self {
            witness_gen_only,
            cells_to_lookup: Default::default(),
            copy_manager,
            assigned: Default::default(),
        }
    }

    /// Add a lookup argument to the manager.
    pub fn add_lookup(&self, tag: ContextTag, cells: [AssignedValue<F>; ADVICE_COLS]) {
        self.cells_to_lookup
            .lock()
            .unwrap()
            .entry(tag)
            .and_modify(|thread| thread.push(cells))
            .or_insert(vec![cells]);
    }

    /// The total number of virtual rows needed to special lookups
    pub fn total_rows(&self) -> usize {
        self.cells_to_lookup.lock().unwrap().iter().flat_map(|(_, advices)| advices).count()
    }

    /// The optimal number of `ADVICE_COLS` chunks of advice columns with lookup enabled for this
    /// particular lookup argument that we should allocate.
    pub fn num_advice_chunks(&self, usable_rows: usize) -> usize {
        let total = self.total_rows();
        total.div_ceil(usable_rows)
    }

    /// Clears state
    pub fn clear(&mut self) {
        self.cells_to_lookup.lock().unwrap().clear();
        self.copy_manager.lock().unwrap().clear();
        self.assigned = Arc::new(OnceLock::new());
    }

    /// Deep clone with the specified copy manager. Unsets `assigned`.
    pub fn deep_clone(&self, copy_manager: SharedCopyConstraintManager<F>) -> Self {
        Self {
            witness_gen_only: self.witness_gen_only,
            cells_to_lookup: Arc::new(Mutex::new(self.cells_to_lookup.lock().unwrap().clone())),
            copy_manager,
            assigned: Default::default(),
        }
    }
}

impl<F: Field + Ord, const ADVICE_COLS: usize> Drop for LookupAnyManager<F, ADVICE_COLS> {
    /// Sanity checks whether the manager has assigned cells to lookup,
    /// to prevent user error.
    fn drop(&mut self) {
        if Arc::strong_count(&self.cells_to_lookup) > 1 {
            return;
        }
        if self.total_rows() > 0 && self.assigned.get().is_none() {
            log::warn!("WARNING: LookupAnyManager was not assigned!");
        }
    }
}

impl<F: Field + Ord, const ADVICE_COLS: usize> VirtualRegionManager<F>
    for LookupAnyManager<F, ADVICE_COLS>
{
    type Config = Vec<[Column<Advice>; ADVICE_COLS]>;
    type Assignment = ();

    fn assign_raw(&self, config: &Self::Config, region: &mut Region<F>) {
        let mut copy_manager =
            (!self.witness_gen_only).then(|| self.copy_manager().lock().unwrap());
        let cells_to_lookup = self.cells_to_lookup.lock().unwrap();
        // Copy the cells to the config columns, going left to right, then top to bottom.
        // Will panic if out of rows
        let mut lookup_offset = 0;
        let mut lookup_col = 0;
        for advices in cells_to_lookup.iter().flat_map(|(_, advices)| advices) {
            if lookup_col >= config.len() {
                lookup_col = 0;
                lookup_offset += 1;
            }
            for (advice, &column) in advices.iter().zip(config[lookup_col].iter()) {
                let bcell =
                    raw_assign_advice(region, column, lookup_offset, Value::known(advice.value));
                if let Some(copy_manager) = copy_manager.as_mut() {
                    constrain_virtual_equals_external(region, *advice, bcell.cell(), copy_manager);
                }
            }

            lookup_col += 1;
        }
        // We cannot clear `cells_to_lookup` because keygen_vk and keygen_pk both call this function
        let _ = self.assigned.set(());
    }
}
