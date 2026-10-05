//! Four exact frozen action projections from the one original State/Set/pool.
//! The complete action inverse is local consistency, not complete State or finality.
use super::{CanonicalTablePairedSnapshot, LeafError, LeafLimits};
use crate::smartcontracts::triggers::set::ActionTable;
use crate::state::{
    StateBlock, block_field::AggregatePublication, is_stable_state_view_generation,
};
/// Retain the real ten Set targets/common mode and original State pool through all outcomes.
pub(in crate::state) fn capture(
    block: &StateBlock<'_>,
    table: ActionTable,
    limits: LeafLimits,
    max_work: u64,
) -> Result<Option<CanonicalTablePairedSnapshot>, LeafError> {
    let Some(fields) = block.fields.as_ref() else {
        return Ok(None);
    };
    if fields.world.publication != AggregatePublication::Frozen {
        return Ok(None);
    }
    let generation = fields.state_ref.state_view_generation();
    if generation & 1 != 0 {
        return Ok(None);
    }
    let outcome = fields.world.triggers.capture_frozen_action_authority_table(
        &fields.state_ref.world.triggers,
        table,
        limits,
        &fields.state_ref.ivm_execution_budget,
        max_work,
    );
    if !is_stable_state_view_generation(generation, fields.state_ref.state_view_generation()) {
        return Ok(None);
    }
    outcome
}
#[cfg(test)]
#[path = "frozen_trigger_actions/tests.rs"]
mod tests;
