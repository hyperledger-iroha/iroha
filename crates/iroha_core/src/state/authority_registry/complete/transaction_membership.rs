//! Same-original-writer capture of both declared membership tables and frontier.
//!
//! The table catalog captures this scoped pair through one closed group, retaining
//! its frontier, original surface and both roots together. Those roots cannot
//! substitute for the specialized membership root, complete State root, finalized
//! anchor or retained Kura store. The original publication surface is an identity
//! check, not authorization to publish. TODO: fund selection/schema metadata and
//! codec scratch, integrate complete table/cell publication and retain the original
//! durable membership store before complete-State admission.

use super::{
    CanonicalTablePairedSnapshot, LeafError, LeafLimits, State, is_stable_state_view_generation,
};
use crate::state::{
    authority_registry::leaf::{
        TypedPairedRowAllowance, TypedPairedRowError, TypedPairedTableBuilder,
    },
    storage_transactions::{
        TransactionsPublicationSurface, TransactionsStorage,
        authority::{
            TransactionMembershipAuthorityError, TransactionMembershipSide,
            TransactionMembershipVisitError,
        },
    },
};
use iroha_allocation::AllocationBudget;
use iroha_crypto::HashOf;
use iroha_data_model::prelude::TransactionEntrypoint;

const CURRENT: &str = "state.transactions.current";
const ROLLBACK: &str = "state.transactions.rollback";
type Builder<'budget> = TypedPairedTableBuilder<'budget, HashOf<TransactionEntrypoint>, u64>;

/// Local bounds for both outputs and all physical source-entry inspections.
#[derive(Clone, Copy)]
pub(super) struct MembershipCaptureLimits {
    tables: LeafLimits,
    max_row_visits: usize,
    max_total_rows: u64,
    max_total_streamed_bytes: u64,
    max_total_ordered_bytes: usize,
}

/// Failed capture returns no partial table and grants no validity verdict.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub(super) enum MembershipCaptureError {
    /// Preserve the original physical writer's local acquisition condition.
    #[error("membership writer acquisition failed: {0:?}")]
    Acquisition(crate::state::storage_transactions::MembershipAdmissionError),
    /// Complete physical source traversal was refused or malformed.
    #[error(transparent)]
    Authority(#[from] TransactionMembershipAuthorityError),
    /// Forward exact original-pool admission and deterministic codec failures.
    #[error(transparent)]
    Table(#[from] LeafError),
    /// Both tables together exceed the caller's admitted output/work bound.
    #[error("membership table pair exceeds aggregate capture bounds")]
    AggregateLimit,
}

/// Detached table nodes and frontier from one actual original membership cut.
pub(super) struct CapturedMembershipTables {
    frontier: u64,
    current: CanonicalTablePairedSnapshot,
    rollback: CanonicalTablePairedSnapshot,
    original_surface: TransactionsPublicationSurface,
}

/// Required source-work policy for the grouped catalog reader; there is no default.
#[derive(Clone, Copy)]
pub(super) struct MembershipWorkLimits {
    /// Every physical latest/history entry inspection, including filtered rows.
    pub(super) max_row_visits: usize,
    /// Both encoder passes across the two complete tables.
    pub(super) max_streamed_bytes: u64,
    /// Canonical key/digest framing and complete ordered nodes across both tables.
    pub(super) max_ordered_bytes: usize,
}

/// Same-cut cell and original identity inseparable from their retained table roots.
/// This local identity is neither finality nor permission to publish.
pub(super) struct CapturedMembershipCompanion {
    frontier: u64,
    original_surface: TransactionsPublicationSurface,
    current_root: iroha_crypto::Hash,
    rollback_root: iroha_crypto::Hash,
    first_position: usize,
}

impl CapturedMembershipCompanion {
    /// Verify the exact adjacent roots retained with this typed frontier.
    pub(super) fn matches_nodes(&self, nodes: &[CanonicalTablePairedSnapshot]) -> bool {
        let Some(pair) = nodes
            .get(self.first_position..)
            .and_then(|tail| tail.get(..2))
        else {
            return false;
        };
        pair[0].table_id() == CURRENT
            && pair[1].table_id() == ROLLBACK
            && pair[0].root() == self.current_root
            && pair[1].root() == self.rollback_root
    }

    /// Canonical u64 cell captured under the original membership writer.
    pub(super) fn frontier(&self) -> u64 {
        self.frontier
    }

    /// Borrow the original owner identity without reacquiring membership.
    pub(super) fn original_surface(&self) -> &TransactionsPublicationSurface {
        &self.original_surface
    }
}

impl CapturedMembershipTables {
    /// Move the one actual pair and its source cell into the caller's fixed slots.
    pub(super) fn into_group(
        self,
        first_position: usize,
    ) -> (
        CanonicalTablePairedSnapshot,
        CanonicalTablePairedSnapshot,
        CapturedMembershipCompanion,
    ) {
        let companion = CapturedMembershipCompanion {
            frontier: self.frontier,
            original_surface: self.original_surface,
            current_root: self.current.root(),
            rollback_root: self.rollback.root(),
            first_position,
        };
        (self.current, self.rollback, companion)
    }
}

/// Required group policy plus the aggregate's remaining row admission.
pub(super) fn capture_membership_group_once(
    state: &State,
    tables: LeafLimits,
    work: MembershipWorkLimits,
    remaining_rows: u64,
) -> Result<Option<CapturedMembershipTables>, MembershipCaptureError> {
    capture_transaction_membership_tables_once(
        state,
        MembershipCaptureLimits {
            tables,
            max_row_visits: work.max_row_visits,
            max_total_rows: remaining_rows,
            max_total_streamed_bytes: work.max_streamed_bytes,
            max_total_ordered_bytes: work.max_ordered_bytes,
        },
    )
}

fn remaining_allowance(
    current: &Builder<'_>,
    rollback: &Builder<'_>,
    limits: MembershipCaptureLimits,
) -> Result<TypedPairedRowAllowance, MembershipCaptureError> {
    let a = current.usage()?;
    let b = rollback.usage()?;
    let streamed_bytes =
        a.1.checked_add(b.1)
            .and_then(|n| limits.max_total_streamed_bytes.checked_sub(n))
            .ok_or(MembershipCaptureError::AggregateLimit)?;
    let ordered_bytes =
        a.2.checked_add(b.2)
            .and_then(|n| limits.max_total_ordered_bytes.checked_sub(n))
            .ok_or(MembershipCaptureError::AggregateLimit)?;
    Ok(TypedPairedRowAllowance {
        streamed_bytes,
        ordered_bytes,
    })
}

/// No writer or row borrow escapes the original pool's synchronous refund scope.
fn capture_from_storage(
    storage: &TransactionsStorage,
    budget: &AllocationBudget,
    limits: MembershipCaptureLimits,
) -> Result<CapturedMembershipTables, MembershipCaptureError> {
    capture_observed(storage, budget, limits, |_, _| {})
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CaptureProgress {
    EncodedRow,
    CurrentFinished,
}

// The private observation point allows custody tests to register a real pool
// waiter after an encoded row, or unwind there. Production supplies no action.
fn capture_observed(
    storage: &TransactionsStorage,
    budget: &AllocationBudget,
    limits: MembershipCaptureLimits,
    mut observe: impl FnMut(CaptureProgress, &AllocationBudget),
) -> Result<CapturedMembershipTables, MembershipCaptureError> {
    budget.with_deferred_refund_notifications(|_scope| {
        let owner = storage
            .try_membership_observation()
            .map_err(MembershipCaptureError::Acquisition)?;
        let cut = owner.membership_authority_cut(limits.max_row_visits)?;
        let frontier = cut.frontier_height();
        let original_surface = owner.publication_surface();
        // Source work is admitted before selection, encoding or allocation.
        let mut current = Some(Builder::new(CURRENT, limits.tables, budget)?);
        let mut rollback = Some(Builder::new(ROLLBACK, limits.tables, budget)?);
        let mut rows = 0_u64;
        cut.visit(|side, key, height| {
            rows = rows
                .checked_add(1)
                .filter(|n| *n <= limits.max_total_rows)
                .ok_or(MembershipCaptureError::AggregateLimit)?;
            let allowance = remaining_allowance(
                current.as_ref().expect("current builder"),
                rollback.as_ref().expect("rollback builder"),
                limits,
            )?;
            let target = match side {
                TransactionMembershipSide::Current => &mut current,
                TransactionMembershipSide::Rollback => &mut rollback,
            };
            // push consumes the old builder. An error cannot leave a
            // finishable prefix; height is encoded before this callback ends.
            *target = Some(
                target
                    .take()
                    .expect("one active table builder")
                    .push(key, &height, allowance)
                    .map_err(|error| match error {
                        TypedPairedRowError::Table(error) => MembershipCaptureError::Table(error),
                        TypedPairedRowError::StreamedAllowance
                        | TypedPairedRowError::OrderedAllowance => {
                            MembershipCaptureError::AggregateLimit
                        }
                    })?,
            );
            observe(CaptureProgress::EncodedRow, budget);
            Ok(())
        })
        .map_err(|error| match error {
            TransactionMembershipVisitError::Authority(error) => {
                MembershipCaptureError::Authority(error)
            }
            TransactionMembershipVisitError::Consumer(error) => error,
        })?;
        // Both source streams are complete and row borrows are gone. Only
        // funded canonical keys/digests remain, so release the physical writer
        // before sorting and final tree construction.
        drop(owner);
        let current = current.expect("complete current stream").finish()?;
        observe(CaptureProgress::CurrentFinished, budget);
        let rollback = rollback.expect("complete rollback stream").finish()?;
        // Inner codec/tree errors may already have refunded temporary credit.
        // All original-pool notifications are still deferred here. The actual
        // writer is gone before this scope returns or unwinds and dispatches them.
        Ok(CapturedMembershipTables {
            frontier,
            current,
            rollback,
            original_surface,
        })
    })
}

/// Capture scoped membership only; generation stability is not finality evidence.
fn capture_transaction_membership_tables_once(
    state: &State,
    limits: MembershipCaptureLimits,
) -> Result<Option<CapturedMembershipTables>, MembershipCaptureError> {
    let generation = state.state_view_generation();
    if generation & 1 != 0 {
        return Ok(None);
    }
    let captured =
        capture_from_storage(&state.transactions, &state.ivm_execution_budget(), limits)?;
    if !is_stable_state_view_generation(generation, state.state_view_generation()) {
        return Ok(None);
    }
    Ok(Some(captured))
}

#[cfg(test)]
#[path = "transaction_membership_tests.rs"]
mod tests;
