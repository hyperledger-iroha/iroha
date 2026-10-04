//! Exact domain-owner buckets over the original frozen StateBlock.
//!
//! The committed and frozen owners share one complete bounded both-image relation.
//! This retains the existing storage-key/bucket semantics without account existence,
//! embedded record-id or execution authority predicates. No fresh State view is needed.
//! TODO: consume this scoped encoder with every canonical table/cell and authenticated history
//! in the sole StatePublication owner. Whole predecessor coherence, custody and finality remain open.

use super::{CanonicalTableLeafSet, CanonicalTablePairedSnapshot, LeafError, LeafLimits};
use crate::state::{
    StateBlock, authority_registry::domain_ownership::validate_original_domain_ownership,
    block_field::AggregatePublication,
};
use iroha_allocation::AllocationBudget;
use iroha_data_model::{account::AccountId, domain::Domain};
use iroha_model_base::domain::DomainId;
use mv::storage::FrozenStorageImages;
use std::collections::BTreeSet;

// Only one actual execution owner can supply the rows, index and encoding pool.
struct Original<'frozen> {
    rows: FrozenStorageImages<'frozen, DomainId, Domain>,
    index: FrozenStorageImages<'frozen, AccountId, BTreeSet<DomainId>>,
    budget: &'frozen AllocationBudget,
}

impl<'frozen> Original<'frozen> {
    fn retain(block: &'frozen StateBlock<'_>) -> Option<Self> {
        let fields = block.fields.as_ref()?;
        if fields.world.publication != AggregatePublication::Frozen {
            return None;
        }
        let rows = fields.world.domains.frozen_images()?;
        let index = fields.world.domains_by_owner.frozen_images()?;
        let rows_owned = rows.belongs_to(&fields.state_ref.world.domains);
        let index_owned = index.belongs_to(&fields.state_ref.world.domains_by_owner);
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

/// Encode original canonical domains whose complete original owner buckets were checked.
///
/// `None` refuses incomplete, foreign, differently acquired or released sources.
/// Both original borrows and their State allocation pool survive validation and
/// paired encoding; every local refusal leaves the same StateBlock available for
/// retry. No State view, publication reacquisition or replacement pool is opened.
/// Success grants no complete State root, finality or private-row disclosure.
pub(in crate::state) fn capture(
    block: &StateBlock<'_>,
    limits: LeafLimits,
    max_work: u64,
) -> Result<Option<CanonicalTablePairedSnapshot>, LeafError> {
    let Some(original) = Original::retain(block) else {
        return Ok(None);
    };
    validate_original_domain_ownership(&original.rows, &original.index, max_work)?;
    CanonicalTableLeafSet::paired_table_from_rows(
        "world.domains",
        limits,
        original.budget,
        original.rows.current_entries(),
    )
    .map(Some)
}

#[cfg(test)]
#[path = "frozen_domain_ownership/tests.rs"]
mod tests;
