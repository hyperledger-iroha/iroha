//! Deterministic physical capacity for KAGEMUSHA's unchanged Base constraint graphs.

use halo2_base::{
    gates::circuit::{BaseCircuitParams, builder::BaseCircuitBuilder},
    utils::ScalarField,
};

/// Exact gate-column inventory for the vendor's four-row vertical assignment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct KagemushaBasePackingV1 {
    /// Complete evaluation domain, including reserved rows.
    pub(super) domain_rows: usize,
    /// Rows available under the circuit's existing reserve.
    pub(super) usable_rows: usize,
    /// Required physical advice columns, in stored phase order.
    pub(super) advice_columns: Vec<usize>,
    /// Largest number of assigned rows in any physical gate column.
    pub(super) maximum_advice_rows: usize,
}

/// Count physical cells without changing witnesses, selectors or copy constraints.
///
/// The Base estimator divides virtual cell counts by usable rows. Its assigner
/// additionally duplicates each boundary cell into the next column, including
/// a boundary on the final input. Replay those decisions in stored context order;
/// neither witness values nor hardware-dependent iteration affect this layout.
pub(super) fn base_packing_inventory_v1<F: ScalarField>(
    builder: &BaseCircuitBuilder<F>,
    unusable_rows: usize,
) -> Result<KagemushaBasePackingV1, String> {
    if builder.witness_gen_only() {
        return Err("Kagemusha Base packing requires the constraint graph".to_owned());
    }
    let exponent = u32::try_from(builder.config_params.k)
        .map_err(|_| "Kagemusha Base domain exponent overflow".to_owned())?;
    let domain_rows = 1_usize
        .checked_shl(exponent)
        .ok_or_else(|| "Kagemusha Base domain row count overflow".to_owned())?;
    let usable_rows = domain_rows
        .checked_sub(unusable_rows)
        .filter(|rows| *rows >= 4)
        .ok_or_else(|| "Kagemusha Base reserve leaves fewer than four gate rows".to_owned())?;
    let mut advice_columns = Vec::with_capacity(builder.core().phase_manager.len());
    let mut maximum_advice_rows = 0;
    for phase in &builder.core().phase_manager {
        let mut columns = 0_usize;
        let mut row = 0_usize;
        for context in &phase.threads {
            if context.selector.len() != context.advice_len() {
                return Err("Kagemusha Base selector and advice lengths differ".to_owned());
            }
            for &enabled in context.selector.iter() {
                columns = columns.max(1);
                maximum_advice_rows = maximum_advice_rows.max(row + 1);
                // Match halo2-base assign_with_constraints::<F, 4>. The old
                // column receives this input before it is duplicated at row zero.
                if (enabled && row > usable_rows - 4) || row >= usable_rows - 1 {
                    columns = columns
                        .checked_add(1)
                        .ok_or_else(|| "Kagemusha Base advice column count overflow".to_owned())?;
                    row = 0;
                }
                row += 1;
            }
        }
        advice_columns.push(columns);
    }
    Ok(KagemushaBasePackingV1 {
        domain_rows,
        usable_rows,
        advice_columns,
        maximum_advice_rows,
    })
}

/// Finalize physical capacity after the last constraint without changing the graph.
pub(super) fn finalize_base_params_v1<F: ScalarField>(
    builder: &mut BaseCircuitBuilder<F>,
    unusable_rows: usize,
) -> Result<(), String> {
    let packing = base_packing_inventory_v1(builder, unusable_rows)?;
    let statistics = builder.statistics();
    let params = BaseCircuitParams {
        k: builder.config_params.k,
        num_advice_per_phase: packing.advice_columns,
        // Constants are striped across columns; reserved rows are unavailable
        // for their fixed assignments and equality constraints too.
        num_fixed: statistics.gate.total_fixed.div_ceil(packing.usable_rows),
        num_lookup_advice_per_phase: statistics
            .total_lookup_advice_per_phase
            .iter()
            .map(|count| count.div_ceil(packing.usable_rows))
            .collect(),
        lookup_bits: builder.lookup_bits(),
        num_instance_columns: builder.config_params.num_instance_columns,
    };
    builder.set_params(params);
    Ok(())
}

/// Verify declared gate capacity and return the actual maximum assigned row count.
pub(super) fn validate_base_gate_capacity_v1<F: ScalarField>(
    builder: &BaseCircuitBuilder<F>,
    unusable_rows: usize,
) -> Result<KagemushaBasePackingV1, String> {
    let packing = base_packing_inventory_v1(builder, unusable_rows)?;
    let configured = &builder.config_params.num_advice_per_phase;
    if configured.len() != packing.advice_columns.len()
        || configured
            .iter()
            .zip(&packing.advice_columns)
            .any(|(actual, required)| actual < required)
    {
        return Err("Kagemusha Base gate columns do not fit the physical packing".to_owned());
    }
    Ok(packing)
}

#[cfg(test)]
#[path = "base_packing_tests.rs"]
mod tests;
