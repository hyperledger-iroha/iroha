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
    let materialized = materialize_quantity_public_transfers(
        &claims,
        batch.public_inputs,
        fastpq_prover::ProofSemantics::AxtTransferClaim,
        limits,
        TransferSmtBuildLimits::for_update_limit(limits.max_rows).expect("bounded fixture SMT"),
    )
    .expect("canonical full-domain fixture");
    let (rows, inputs, _, _) = materialized.into_parts();
    batch.transitions = rows;
    batch.public_inputs = inputs;
}
