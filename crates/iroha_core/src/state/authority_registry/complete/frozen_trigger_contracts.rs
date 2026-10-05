//! One exact frozen trigger-contract output from the original State/World/Set and pool.
//! The remaining action relations, cells, joint State publication and Kura remain open.
use super::{CanonicalTablePairedSnapshot, LeafError, LeafLimits};
use crate::state::{
    StateBlock, block_field::AggregatePublication, is_stable_state_view_generation,
};
/// Preserve actual ten Set targets/modes and original State pool through both-image checks.
pub(in crate::state) fn capture(
    block: &StateBlock<'_>,
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
    let outcome = fields
        .world
        .triggers
        .capture_frozen_contracts_authority_table(
            &fields.state_ref.world.triggers,
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
#[path = "frozen_trigger_contracts/tests.rs"]
mod tests;
