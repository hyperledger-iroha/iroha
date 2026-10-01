//! Join the ordinary State preparation subject to the same assigned financial transition.
//!
//! The enclosing State must verify the genuine ordinary Guard against the copied private
//! columns. This relation supplies no platform or issuer authorization by itself. Candidate
//! approval occurs in the separate post-candidate terminal phase so State audits remain complete.

use halo2_base::gates::{
    GateInstructions as _, RangeInstructions as _, circuit::builder::BaseCircuitBuilder,
};
use iroha_data_model::kagemusha::KagemushaHardwareSelectionSigningLayoutV1 as S;

use super::super::{
    ordinary_guard_data_binding::KagemushaOrdinaryGuardDataBindingV1,
    state_relation::{
        KagemushaAssignedStateRelationV1, KagemushaStateRelationWitnessV1,
        constrain_bootstrap_statement_digest_v1,
    },
};
use super::assigned_uint_bytes_v1;
use crate::{
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    pasta_sha256::{PastaSha256ByteV1, PastaSha256JobsV1},
};

pub(super) fn constrain_ordinary_state_subject_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    assigned: &KagemushaAssignedStateRelationV1<F>,
    witness: &KagemushaStateRelationWitnessV1,
    transition_digest: &[PastaSha256ByteV1<F>; 32],
    binding: &KagemushaOrdinaryGuardDataBindingV1<F>,
) -> Result<(), String> {
    // Both messages are constructed in every operation. The signed assigned operation,
    // rather than a host-side branch, chooses the corresponding complete canonical SHA.
    let bootstrap_digest =
        constrain_bootstrap_statement_digest_v1(builder, jobs, assigned, witness)?;
    let range = builder.range_chip();
    let gate = range.gate();
    let ctx = builder.main(0);
    let bootstrap = gate.is_zero(ctx, assigned.operation);
    // Bootstrap approves the initial publication. Every later State proof consumes a
    // pre-candidate PrepareTransition approval; terminal consumes a new full approval.
    let bootstrap_purpose = ctx.load_constant(F::ONE);
    let preparation_purpose = ctx.load_constant(F::from(2));
    let expected_purpose = gate.select(ctx, bootstrap_purpose, preparation_purpose, bootstrap);
    ctx.constrain_equal(&binding.approval_purpose, &expected_purpose);
    for ((signed, initial), transition) in binding.canonical_subject[S::TRANSITION_STATEMENT_DIGEST]
        .iter()
        .zip(bootstrap_digest)
        .zip(transition_digest)
    {
        let expected = gate.select(
            ctx,
            initial.assigned().ok_or("bootstrap SHA byte absent")?,
            transition.assigned().ok_or("transition SHA byte absent")?,
            bootstrap,
        );
        ctx.constrain_equal(signed, &expected);
    }
    // Bind the independent financial indexes explicitly, even if a particular State
    // policy currently keeps its logical sequence and secure index equal.
    for (slot, actual) in [
        (S::SECURE_INDEX_BEFORE, assigned.predecessor.secure_index),
        (S::SECURE_INDEX_AFTER, assigned.successor.secure_index),
    ] {
        let bytes = assigned_uint_bytes_v1(ctx, gate, actual, 128);
        for (signed, byte) in binding.canonical_subject[slot].iter().zip(bytes) {
            ctx.constrain_equal(
                signed,
                &byte.assigned().ok_or("financial index byte absent")?,
            );
        }
    }
    let zero = ctx.load_constant(F::ZERO);
    for byte in binding.canonical_subject[S::CANDIDATE_ENVELOPE_DIGEST]
        .iter()
        .chain(&binding.canonical_subject[S::TERMINAL_BODY_COMMITMENT])
    {
        ctx.constrain_equal(byte, &zero);
    }
    Ok(())
}
