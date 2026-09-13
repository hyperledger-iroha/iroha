//! Frozen pre-change assignment algorithms used only for regression comparison.

use crate::{
    Context, ContextCell,
    gates::flex_gate::{BasicGateConfig, ThreadBreakPoints},
    halo2_proofs::circuit::{Region, Value},
    utils::{
        ScalarField,
        halo2::{
            constrain_virtual_equals_external, raw_assign_advice,
            raw_assign_advice_discarding_value, raw_constrain_equal,
        },
    },
    virtual_region::{copy_constraints::CopyConstraintManager, lookups::LookupAnyManager},
};

pub(super) fn gate<F: ScalarField, const ROTATIONS: usize>(
    threads: &[Context<F>],
    basic_gates: &[BasicGateConfig<F>],
    region: &mut Region<F>,
    copy_manager: &mut CopyConstraintManager<F>,
    max_rows: usize,
    use_unknown: bool,
) -> ThreadBreakPoints {
    let mut break_points = vec![];
    let mut gate_index = 0;
    let mut row_offset = 0;
    for ctx in threads {
        if ctx.advice_len() == 0 {
            continue;
        }
        let mut basic_gate = basic_gates
                        .get(gate_index)
                        .unwrap_or_else(|| panic!("NOT ENOUGH ADVICE COLUMNS. Perhaps blinding factors were not taken into account. The max non-poisoned rows is {max_rows}"));
        assert_eq!(ctx.selector.len(), ctx.advice_len());

        for (i, (advice, &q)) in ctx.advice_values().zip(ctx.selector.iter()).enumerate() {
            let column = basic_gate.value;
            let value = if use_unknown {
                Value::unknown()
            } else {
                Value::known(advice)
            };
            #[cfg(feature = "halo2-axiom")]
            let cell = region.assign_advice(column, row_offset, value).cell();
            #[cfg(not(feature = "halo2-axiom"))]
            let cell = region
                .assign_advice(|| "", column, row_offset, || value)
                .unwrap()
                .cell();
            if let Some(old_cell) = copy_manager
                .assigned_advices
                .insert(ContextCell::new(ctx.type_id, ctx.context_id, i), cell)
            {
                assert!(
                    old_cell.row_offset == cell.row_offset && old_cell.column == cell.column,
                    "Trying to overwrite virtual cell with a different raw cell"
                );
            }

            // If selector enabled and row_offset is valid add break point, account for break point overlap, and enforce equality constraint for gate outputs.
            // ⚠️ This assumes overlap is of form: gate enabled at `i - delta` and `i`, where `delta = ROTATIONS - 1`. We currently do not support `delta < ROTATIONS - 1`.
            if (q && row_offset + ROTATIONS > max_rows) || row_offset >= max_rows - 1 {
                break_points.push(row_offset);
                row_offset = 0;
                gate_index += 1;

                // safety check: make sure selector is not enabled on `i - delta` for `0 < delta < ROTATIONS - 1`
                if ROTATIONS > 1 && i + 2 >= ROTATIONS {
                    for delta in 1..ROTATIONS - 1 {
                        assert!(
                            !ctx.selector[i - delta],
                            "We do not support overlaps with delta = {delta}"
                        );
                    }
                }
                // when there is a break point, because we may have two gates that overlap at the current cell, we must copy the current cell to the next column for safety
                basic_gate = basic_gates
                        .get(gate_index)
                        .unwrap_or_else(|| panic!("NOT ENOUGH ADVICE COLUMNS. Perhaps blinding factors were not taken into account. The max non-poisoned rows is {max_rows}"));
                let column = basic_gate.value;
                #[cfg(feature = "halo2-axiom")]
                let ncell = region.assign_advice(column, row_offset, value);
                #[cfg(not(feature = "halo2-axiom"))]
                let ncell = region
                    .assign_advice(|| "", column, row_offset, || value)
                    .unwrap();
                raw_constrain_equal(region, ncell.cell(), cell);
            }

            if q {
                basic_gate
                    .q_enable
                    .enable(region, row_offset)
                    .expect("enable selector should not fail");
            }

            row_offset += 1;
        }
    }
    break_points
}

pub(super) fn lookups<F: ScalarField, const ADVICE_COLS: usize>(
    manager: &LookupAnyManager<F, ADVICE_COLS>,
    config: &Vec<
        [crate::halo2_proofs::plonk::Column<crate::halo2_proofs::plonk::Advice>; ADVICE_COLS],
    >,
    region: &mut Region<F>,
) {
    let mut copy_manager =
        (!manager.witness_gen_only()).then(|| manager.copy_manager().lock().unwrap());
    let cells_to_lookup = manager.cells_to_lookup.lock().unwrap();
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
            if let Some(copy_manager) = copy_manager.as_mut() {
                let bcell =
                    raw_assign_advice(region, column, lookup_offset, Value::known(advice.value));
                constrain_virtual_equals_external(region, *advice, bcell.cell(), copy_manager);
            } else {
                raw_assign_advice_discarding_value(
                    region,
                    column,
                    lookup_offset,
                    Value::known(advice.value),
                );
            }
        }

        lookup_col += 1;
    }
    // We cannot clear `cells_to_lookup` because keygen_vk and keygen_pk both call this function
    let _ = manager.assigned.set(());
}
