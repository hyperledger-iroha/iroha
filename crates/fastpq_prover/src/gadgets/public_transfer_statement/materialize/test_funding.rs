//! Explicit fixture-only admission for existing tree and materialization controls.

use super::*;

pub(super) fn derive_two_update_smt<T: CheckedUpdateTable + ?Sized>(
    table: &T,
    limits: TransferSmtBuildLimits,
) -> Result<DerivedTransferSmtWitnesses> {
    let updates = table
        .pair_count()
        .checked_mul(2)
        .ok_or_else(|| invariant("SMT update count overflows"))?;
    let bytes = limits.allocation_bytes(updates, table.keys().len())?;
    let budget = AllocationBudget::new(bytes);
    let mut reservation = budget.try_reserve_bytes(bytes)?;
    super::derive_two_update_smt(table, limits, &budget, &mut reservation)
}

pub(super) fn derive<V>(
    table: &PreparedPublicTransfers<'_, V>,
    limits: TransferSmtBuildLimits,
) -> Result<DerivedTransferSmtWitnesses> {
    derive_two_update_smt(table, limits)
}

pub(super) fn funded_build<V>(
    table: &PreparedPublicTransfers<'_, V>,
    limits: TransferSmtBuildLimits,
) -> Result<DerivedTransferSmtWitnesses> {
    let bytes = limits.allocation_bytes(table.row_count(), table.keys().len())?;
    let budget = AllocationBudget::new(bytes);
    let mut reservation = budget.try_reserve_bytes(bytes)?;
    table.build_smt_witnesses(limits, &budget, &mut reservation)
}

pub(super) fn materialize_quantity_public_transfers(
    claims: &[PublicTransferTranscript],
    inputs: PublicInputs,
    semantics: ProofSemantics,
    public_limits: PublicTransferLimits,
    tree_limits: TransferSmtBuildLimits,
) -> Result<QuantityTransferMaterialization> {
    // This fixture wrapper explicitly reserves the supplied worst-case count
    // bounds. Production accepts its original owner's reservation instead.
    let bytes =
        tree_limits.allocation_bytes(tree_limits.max_updates, tree_limits.max_unique_keys)?;
    let budget = AllocationBudget::new(bytes);
    let mut reservation = budget.try_reserve_bytes(bytes)?;
    super::materialize_quantity_public_transfers(
        claims,
        inputs,
        semantics,
        public_limits,
        tree_limits,
        &budget,
        &mut reservation,
    )
}
