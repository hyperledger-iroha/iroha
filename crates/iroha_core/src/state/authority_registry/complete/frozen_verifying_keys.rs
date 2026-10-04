//! Exact frozen verifier group borrowed from the original State execution owner.
//!
//! Both images use the same relation as committed capture and snapshot validation.
//! Only current canonical registry rows enter the existing paired table encoder;
//! the inverse is checked, never independent authority. Physical work and encoding
//! limits are caller-admitted local bounds, not new registry or consensus limits.
//!
//! TODO: consume this group with every canonical table/cell in StatePublication
//! after deterministic tail writes and complete freeze, before original publication
//! reacquisition and the generation fence. That integration must retain its actual
//! admitted policy, classify every local refusal, and retain all node custody through
//! publication/recovery. No default policy, automatic commit gate or finalized root
//! is introduced here. Existing schema-name/codec scratch funding remains open.

use super::{CanonicalTableLeafSet, CanonicalTablePairedSnapshot, LeafError, LeafLimits};
use crate::state::{
    StateBlock,
    authority_registry::grouped_ownership::GroupedOwnershipError,
    block_field::AggregatePublication,
    verifying_key_index_validation::{self as relation, Work},
};
use iroha_allocation::AllocationBudget;
use iroha_data_model::proof::{VerifyingKeyId, VerifyingKeyRecord};
use mv::storage::FrozenStorageImages;

// The private constructor accepts one actual StateBlock. Callers cannot combine
// rows from another World, swap the original pool, or bypass complete World freeze.
struct Original<'frozen> {
    rows: FrozenStorageImages<'frozen, VerifyingKeyId, VerifyingKeyRecord>,
    index: FrozenStorageImages<'frozen, (String, u32), VerifyingKeyId>,
    budget: &'frozen AllocationBudget,
}
impl<'frozen> Original<'frozen> {
    fn retain(block: &'frozen StateBlock<'_>) -> Option<Self> {
        let fields = block.fields.as_ref()?;
        if fields.world.publication != AggregatePublication::Frozen {
            return None;
        }
        let rows = fields.world.verifying_keys.frozen_images()?;
        let index = fields.world.verifying_keys_by_circuit.frozen_images()?;
        let rows_owned = rows.belongs_to(&fields.state_ref.world.verifying_keys);
        let index_owned = index.belongs_to(&fields.state_ref.world.verifying_keys_by_circuit);
        if !rows_owned || !index_owned || rows.mode() != index.mode() {
            return None;
        }
        Some(Self {
            rows,
            index,
            budget: &fields.state_ref.ivm_execution_budget,
        })
    }
}

/// Prepare a scoped pair from the original frozen registry and checked inverse.
///
/// `None` means this owner is not completely frozen or does not belong to its State.
/// No rows are copied/reacquired and no State/configuration view is consulted. Both
/// borrows survive validation and encoding; every refusal leaves the caller's same
/// StateBlock untouched for retry. Local work/row/byte/pool limits are operational
/// refusal only, never a transaction verdict. The eventual State publisher still
/// must check every original target predecessor before consuming these nodes.
pub(in crate::state) fn capture(
    block: &StateBlock<'_>,
    limits: LeafLimits,
    max_work: u64,
) -> Result<Option<CanonicalTablePairedSnapshot>, LeafError> {
    let Some(original) = Original::retain(block) else {
        return Ok(None);
    };
    relation::validate(
        &original.rows,
        &original.index,
        &mut Work::bounded(max_work),
    )
    .map_err(GroupedOwnershipError::from_verifying_key_relation)?;
    let snapshot = CanonicalTableLeafSet::paired_table_from_rows(
        "world.verifying_keys",
        limits,
        original.budget,
        original.rows.current_entries(),
    );
    // The caller cannot move or mutate the frozen StateBlock while either original
    // borrow is live. Committed target changes never refresh this private image;
    // whole-publication currentness remains the existing original publisher's job.
    snapshot.map(Some)
}

#[cfg(test)]
#[path = "frozen_verifying_keys/tests.rs"]
mod tests;
