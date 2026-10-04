//! Canonical quantity statement construction for AXT test fixtures.

/// Materialize full-domain fixture rows and their touched-tree roots before binding.
/// Non-transfer fixtures remain explicit unsupported semantic inputs.
pub fn materialize(batch: &mut fastpq_prover::TransitionBatch) {
    use fastpq_prover::gadgets::public_transfer_statement::{
        PublicTransferLimits, TransferSmtBuildLimits, materialize_quantity_public_transfers,
        public_claims_from_transcripts,
    };
    if !batch
        .transitions
        .iter()
        .all(|row| matches!(row.operation, fastpq_prover::OperationKind::Transfer))
    {
        return;
    }
    let transcripts = fastpq_prover::gadgets::transfer::decode_transcripts(&batch.metadata)
        .expect("decode fixture transcripts")
        .expect("transfer fixture transcripts");
    let limits = PublicTransferLimits::default();
    let claims =
        public_claims_from_transcripts(&transcripts, limits).expect("public fixture claims");
    let materialized = {
        // This test fixture owns its finite tree pool; production supplies its original owner.
        let tree_claims = &claims;
        let tree_limits =
            TransferSmtBuildLimits::for_update_limit(limits.max_rows).expect("bounded fixture SMT");
        let tree_updates = tree_claims
            .iter()
            .try_fold(0_usize, |count, claim| {
                count.checked_add(claim.deltas.len())
            })
            .expect("fixture effect count fits")
            .checked_mul(2)
            .expect("fixture row count fits");
        let tree_bytes = tree_limits
            .allocation_bytes(tree_updates, tree_updates)
            .expect("fixture tree allocation demand fits");
        let tree_budget = iroha_allocation::AllocationBudget::new(tree_bytes);
        let mut tree_reservation = tree_budget
            .try_reserve_bytes(tree_bytes)
            .expect("fixture owns complete tree credit");
        materialize_quantity_public_transfers(
            tree_claims,
            batch.public_inputs,
            fastpq_prover::ProofSemantics::AxtTransferClaim,
            limits,
            tree_limits,
            &tree_budget,
            &mut tree_reservation,
        )
    }
    .expect("canonical full-domain fixture");
    let (rows, inputs, _, _) = materialized.into_parts();
    batch.transitions = rows;
    batch.public_inputs = inputs;
}
