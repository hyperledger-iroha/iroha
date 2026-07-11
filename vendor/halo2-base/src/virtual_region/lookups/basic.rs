use std::iter::zip;

use crate::{
    halo2_proofs::{
        circuit::{Layouter, Region, Value},
        halo2curves::ff::Field,
        plonk::{Advice, Column, ConstraintSystem, Fixed, Phase},
        poly::Rotation,
    },
    utils::{
        halo2::{constrain_virtual_equals_external, raw_assign_advice, raw_assign_fixed},
        ScalarField,
    },
    virtual_region::copy_constraints::SharedCopyConstraintManager,
    AssignedValue,
};

/// A simple dynamic lookup table for when you want to verify some length `KEY_COL` key
/// is in a provided (dynamic) table of the same format.
///
/// Note that you can also use this to look up (key, out) pairs, where you consider the whole
/// pair as the new key.
///
/// We can have multiple sets of dedicated columns to be looked up: these can be specified
/// when calling `new`, but typically we just need 1 set.
///
/// The `table` consists of advice columns. Since this table may have poisoned rows (blinding factors),
/// we use a fixed column `table_selector` which is default 0 and only 1 on enabled rows of the table.
/// The dynamic lookup will check that for `(key, key_is_enabled)` in `to_lookup` we have `key` matches one of
/// the rows in `table` where `table_selector == key_is_enabled`.
/// Reminder: the Halo2 lookup argument will ignore the poisoned rows in `to_lookup`
/// (see [https://zcash.github.io/halo2/design/proving-system/lookup.html#zero-knowledge-adjustment]), but it will
/// not ignore the poisoned rows in `table`.
///
/// Part of this design consideration is to allow a key of `[F::ZERO; KEY_COL]` to still be used as a valid key
/// in the lookup argument. By default, unfilled rows in `to_lookup` will be all zeros; we require
/// at least one row in `table` where `table_is_enabled = 0` and the rest of the row in `table` are also 0s.
#[derive(Clone, Debug)]
pub struct BasicDynLookupConfig<const KEY_COL: usize> {
    /// Columns for cells to be looked up. Consists of `(key, key_is_enabled)`.
    pub to_lookup: Vec<([Column<Advice>; KEY_COL], Column<Fixed>)>,
    /// Table to look up against.
    pub table: [Column<Advice>; KEY_COL],
    /// Selector to enable a row in `table` to actually be part of the lookup table. This is to prevent
    /// blinding factors in `table` advice columns from being used in the lookup.
    pub table_is_enabled: Column<Fixed>,
}

impl<const KEY_COL: usize> BasicDynLookupConfig<KEY_COL> {
    /// Assumes all columns are in the same phase `P` to make life easier.
    /// We enable equality on all columns because we envision both the columns to lookup
    /// and the table will need to talk to halo2-lib.
    pub fn new<P: Phase, F: Field>(
        meta: &mut ConstraintSystem<F>,
        phase: impl Fn() -> P,
        num_lu_sets: usize,
    ) -> Self {
        let mut make_columns = || {
            let advices = [(); KEY_COL].map(|_| {
                let advice = meta.advice_column_in(phase());
                meta.enable_equality(advice);
                advice
            });
            let is_enabled = meta.fixed_column();
            (advices, is_enabled)
        };
        let (table, table_is_enabled) = make_columns();
        let to_lookup: Vec<_> = (0..num_lu_sets).map(|_| make_columns()).collect();

        for (key, key_is_enabled) in &to_lookup {
            meta.lookup_any("dynamic lookup table", |meta| {
                let table = table.map(|c| meta.query_advice(c, Rotation::cur()));
                let table_is_enabled = meta.query_fixed(table_is_enabled, Rotation::cur());
                let key = key.map(|c| meta.query_advice(c, Rotation::cur()));
                let key_is_enabled = meta.query_fixed(*key_is_enabled, Rotation::cur());
                zip(key, table).chain([(key_is_enabled, table_is_enabled)]).collect()
            });
        }

        Self { table_is_enabled, table, to_lookup }
    }

    /// Assign managed lookups. The `keys` must have already been raw assigned beforehand.
    ///
    /// `copy_manager` **must** be provided unless you are only doing witness generation
    /// without constraints.
    pub fn assign_virtual_to_lookup_to_raw<F: ScalarField>(
        &self,
        mut layouter: impl Layouter<F>,
        keys: impl IntoIterator<Item = [AssignedValue<F>; KEY_COL]>,
        copy_manager: Option<&SharedCopyConstraintManager<F>>,
    ) {
        #[cfg(not(feature = "halo2-axiom"))]
        let keys = keys.into_iter().collect::<Vec<_>>();
        layouter
            .assign_region(
                || "[BasicDynLookupConfig] Advice cells to lookup",
                |mut region| {
                    self.assign_virtual_to_lookup_to_raw_from_offset(
                        &mut region,
                        #[cfg(feature = "halo2-axiom")]
                        keys,
                        #[cfg(not(feature = "halo2-axiom"))]
                        keys.clone(),
                        0,
                        copy_manager,
                    );
                    Ok(())
                },
            )
            .unwrap();
    }

    /// Assign managed lookups. The `keys` must have already been raw assigned beforehand.
    ///
    /// `copy_manager` **must** be provided unless you are only doing witness generation
    /// without constraints.
    pub fn assign_virtual_to_lookup_to_raw_from_offset<F: ScalarField>(
        &self,
        region: &mut Region<F>,
        keys: impl IntoIterator<Item = [AssignedValue<F>; KEY_COL]>,
        mut offset: usize,
        copy_manager: Option<&SharedCopyConstraintManager<F>>,
    ) {
        let mut copy_manager = copy_manager.map(|c| c.lock().unwrap());
        // Copied from `LookupAnyManager::assign_raw` but modified to set `key_is_enabled` to 1.
        // Copy the cells to the config columns, going left to right, then top to bottom.
        // Will panic if out of rows
        let mut lookup_col = 0;
        for key in keys {
            if lookup_col >= self.to_lookup.len() {
                lookup_col = 0;
                offset += 1;
            }
            let (key_col, key_is_enabled_col) = self.to_lookup[lookup_col];
            // set key_is_enabled to 1
            raw_assign_fixed(region, key_is_enabled_col, offset, F::ONE);
            for (advice, column) in zip(key, key_col) {
                let bcell = raw_assign_advice(region, column, offset, Value::known(advice.value));
                if let Some(copy_manager) = copy_manager.as_mut() {
                    constrain_virtual_equals_external(region, advice, bcell.cell(), copy_manager);
                }
            }

            lookup_col += 1;
        }
    }

    /// Assign virtual table to raw. The `rows` must have already been raw assigned beforehand.
    ///
    /// `copy_manager` **must** be provided unless you are only doing witness generation
    /// without constraints.
    pub fn assign_virtual_table_to_raw<F: ScalarField>(
        &self,
        mut layouter: impl Layouter<F>,
        rows: impl IntoIterator<Item = [AssignedValue<F>; KEY_COL]>,
        copy_manager: Option<&SharedCopyConstraintManager<F>>,
    ) {
        #[cfg(not(feature = "halo2-axiom"))]
        let rows = rows.into_iter().collect::<Vec<_>>();
        layouter
            .assign_region(
                || "[BasicDynLookupConfig] Dynamic Lookup Table",
                |mut region| {
                    self.assign_virtual_table_to_raw_from_offset(
                        &mut region,
                        #[cfg(feature = "halo2-axiom")]
                        rows,
                        #[cfg(not(feature = "halo2-axiom"))]
                        rows.clone(),
                        0,
                        copy_manager,
                    );
                    Ok(())
                },
            )
            .unwrap();
    }

    /// Assign virtual table to raw. The `rows` must have already been raw assigned beforehand.
    ///
    /// `copy_manager` **must** be provided unless you are only doing witness generation
    /// without constraints.
    pub fn assign_virtual_table_to_raw_from_offset<F: ScalarField>(
        &self,
        region: &mut Region<F>,
        rows: impl IntoIterator<Item = [AssignedValue<F>; KEY_COL]>,
        mut offset: usize,
        copy_manager: Option<&SharedCopyConstraintManager<F>>,
    ) {
        let mut copy_manager = copy_manager.map(|c| c.lock().unwrap());
        for row in rows {
            // Enable this row in the table
            raw_assign_fixed(region, self.table_is_enabled, offset, F::ONE);
            for (advice, column) in zip(row, self.table) {
                let bcell = raw_assign_advice(region, column, offset, Value::known(advice.value));
                if let Some(copy_manager) = copy_manager.as_mut() {
                    constrain_virtual_equals_external(region, advice, bcell.cell(), copy_manager);
                }
            }
            offset += 1;
        }
        // always assign one disabled row with all 0s, so disabled to_lookup works for sure
        raw_assign_fixed(region, self.table_is_enabled, offset, F::ZERO);
        for col in self.table {
            raw_assign_advice(region, col, offset, Value::known(F::ZERO));
        }
    }
}
