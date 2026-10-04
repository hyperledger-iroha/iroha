//! Explicit original test pools for producer calls; production never creates these pools.

use super::test_prover as prover;
use iroha_allocation::{AllocationBudget, AllocationReservation};
use iroha_data_model::{fastpq::FastpqPublicTransferStatementV1, nexus::AxtFastpqBinding};
use prover::offline_compact::{
    ExpectedAxtContext, ExpectedStatement, ProvingError, ProvingLimits, VerificationLimits,
};

/// Admit conservative tree backing for this test only.
pub fn funding(updates: usize) -> (AllocationBudget, AllocationReservation) {
    // An odd malformed row count still receives adequate test credit so the
    // original producer admission, not this helper, diagnoses the input.
    let updates = updates.checked_add(updates % 2).unwrap();
    let limits =
        prover::gadgets::public_transfer_statement::TransferSmtBuildLimits::for_update_limit(
            updates,
        )
        .unwrap();
    let bytes = limits.allocation_bytes(updates, updates).unwrap();
    let budget = AllocationBudget::new(bytes);
    let reservation = budget.try_reserve_bytes(bytes).unwrap();
    (budget, reservation)
}

/// Forward an AXT fixture with explicit test-owned credit.
#[allow(
    clippy::large_types_passed_by_value,
    reason = "test callers retain the public Copy policy interface"
)]
pub fn prove_quantity_axt_artifact(
    statement: &FastpqPublicTransferStatementV1,
    expected: ExpectedStatement,
    context: ExpectedAxtContext<'_>,
    proving: ProvingLimits,
    verification: VerificationLimits,
) -> Result<Vec<u8>, ProvingError> {
    let (budget, mut reservation) = funding(statement.transitions.len());
    let result = prover::offline_compact::prove_quantity_axt_artifact(
        statement,
        expected,
        context,
        proving,
        verification,
        &budget,
        &mut reservation,
    );
    drop(reservation);
    assert_eq!(
        budget.reserved_bytes(),
        0,
        "tree credits must follow private backing through success and refusal"
    );
    result
}

/// Forward a bound batch with explicit test-owned credit.
pub fn prove_axt_bound_batch(
    batch: &prover::TransitionBatch,
    binding: &AxtFastpqBinding,
) -> Result<Vec<u8>, prover::Error> {
    let (budget, mut reservation) = funding(batch.transitions.len());
    let result = prover::prove_axt_bound_batch(batch, binding, &budget, &mut reservation);
    drop(reservation);
    assert_eq!(
        budget.reserved_bytes(),
        0,
        "tree credits must follow private backing through success and refusal"
    );
    result
}
