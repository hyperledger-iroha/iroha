//! Join the ordinary terminal approval to its actual State, candidate and body cells.
//!
//! This relation grants no Native capability and supplies no issuer or platform equation.
//! The enclosing terminal must consume the five exact original digests of the genuine
//! release-pinned ordinary Guard, the entire State/candidate proof and all carried histories.
//! Its candidate and body SHA outputs must be built from complete preparation/output openings.

use halo2_base::{
    AssignedValue, QuantumCell,
    gates::{GateInstructions as _, RangeInstructions as _, circuit::builder::BaseCircuitBuilder},
};
use iroha_data_model::kagemusha::KagemushaHardwareSelectionSigningLayoutV1 as S;

use super::{
    composite::assigned_uint_bytes_v1,
    ordinary_guard_data_binding::KagemushaOrdinaryGuardDataBindingV1,
};
use crate::{kagemusha_v1_poseidon::KagemushaPoseidonFieldV1, pasta_sha256::PastaSha256ByteV1};

/// Original assigned financial values and complete derived SHA outputs.
///
/// The future private producer borrows an exclusive ordinary financial owner for the
/// operation ID and reservation. They are never selected by a managed request frame.
/// These cells themselves have no authority, and no captured Bootstrap approval is accepted.
pub(super) struct KagemushaOrdinaryTerminalSubjectSourcesV1<F: KagemushaPoseidonFieldV1> {
    /// Exact operation cell of the recursively verified candidate (SendSplit 2 or RedeemSplit 4).
    pub(super) operation: AssignedValue<F>,
    /// Actual predecessor secure index; independent of the logical sequence.
    pub(super) secure_index_before: AssignedValue<F>,
    /// Actual successor secure index.
    pub(super) secure_index_after: AssignedValue<F>,
    /// Complete State transition statement SHA, derived from the same assigned State.
    pub(super) transition_digest: [PastaSha256ByteV1<F>; 32],
    /// Complete candidate projection SHA derived from the recursively verified column.
    pub(super) candidate_digest: [PastaSha256ByteV1<F>; 32],
    /// Complete terminal body SHA derived after opening the actual prepared intent and output.
    pub(super) terminal_body_digest: [PastaSha256ByteV1<F>; 32],
    /// Same Native-reserved operation ID that the original W signs.
    pub(super) native_operation_id: [PastaSha256ByteV1<F>; 32],
    /// Native reservation start, under its actual continuous/signed clock owner.
    pub(super) reservation_issued_at_ms: AssignedValue<F>,
    /// Exact immutable Native reservation expiry; this relation never widens it.
    pub(super) reservation_expires_at_ms: AssignedValue<F>,
}

/// Constrain the same original purpose1 W and full S to real terminal financial cells.
///
/// The interval constraints express containment, not current-time admission. Current FI,
/// credential, PI, descriptor prefix and clock custody remain separate mandatory Native checks.
pub(super) fn constrain_ordinary_terminal_subject_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    sources: KagemushaOrdinaryTerminalSubjectSourcesV1<F>,
    binding: &KagemushaOrdinaryGuardDataBindingV1<F>,
) -> Result<(), String> {
    let range = builder.range_chip();
    let gate = range.gate();
    let ctx = builder.main(0);
    let one = ctx.load_constant(F::ONE);
    ctx.constrain_equal(&binding.approval_purpose, &one);

    range.range_check(ctx, sources.operation, 8);
    let send = gate.is_equal(ctx, sources.operation, QuantumCell::Constant(F::from(2)));
    let redeem = gate.is_equal(ctx, sources.operation, QuantumCell::Constant(F::from(4)));
    let outgoing = gate.or(ctx, send, redeem);
    ctx.constrain_equal(&outgoing, &one);
    ctx.constrain_equal(
        &binding.canonical_subject[S::OPERATION_TAG.start],
        &sources.operation,
    );

    for (slot, actual) in [
        (S::SECURE_INDEX_BEFORE, sources.secure_index_before),
        (S::SECURE_INDEX_AFTER, sources.secure_index_after),
    ] {
        // Decomposition constrains the whole unsigned 128-bit value and every signed byte.
        for (signed, byte) in binding.canonical_subject[slot]
            .iter()
            .zip(assigned_uint_bytes_v1(ctx, gate, actual, 128))
        {
            ctx.constrain_equal(
                signed,
                &byte.assigned().ok_or("terminal index byte absent")?,
            );
        }
    }
    for (slot, actual) in [
        (S::TRANSITION_STATEMENT_DIGEST, sources.transition_digest),
        (S::CANDIDATE_ENVELOPE_DIGEST, sources.candidate_digest),
        (S::TERMINAL_BODY_COMMITMENT, sources.terminal_body_digest),
    ] {
        let mut nonzero = ctx.load_zero();
        for (signed, byte) in binding.canonical_subject[slot].iter().zip(actual) {
            let actual = byte.assigned().ok_or("terminal SHA byte absent")?;
            range.range_check(ctx, actual, 8);
            ctx.constrain_equal(signed, &actual);
            let zero = gate.is_zero(ctx, actual);
            let present = gate.not(ctx, zero);
            nonzero = gate.or(ctx, nonzero, present);
        }
        ctx.constrain_equal(&nonzero, &one);
    }
    let mut operation_nonzero = ctx.load_zero();
    for (approved, native) in binding
        .approval_operation_id
        .iter()
        .zip(sources.native_operation_id)
    {
        let approved = approved
            .assigned()
            .ok_or("signed W operation byte absent")?;
        let native = native.assigned().ok_or("Native operation byte absent")?;
        range.range_check(ctx, native, 8);
        ctx.constrain_equal(&approved, &native);
        let zero = gate.is_zero(ctx, native);
        let present = gate.not(ctx, zero);
        operation_nonzero = gate.or(ctx, operation_nonzero, present);
    }
    ctx.constrain_equal(&operation_nonzero, &one);

    // These are the same actual cells joined to every byte of W by the ordinary wrapper
    // relation. Assigning a second copy of issued/expires here would leave a detached interval.
    let reserved = sources.reservation_issued_at_ms;
    let expires = sources.reservation_expires_at_ms;
    let approved_at = binding.approval_issued_at_ms;
    let approved_until = binding.approval_expires_at_ms;
    for value in [reserved, expires, approved_at, approved_until] {
        range.range_check(ctx, value, 64);
    }
    range.check_less_than(ctx, reserved, expires, 64);
    range.check_less_than(ctx, approved_at, approved_until, 64);
    let starts_before_reservation = range.is_less_than(ctx, approved_at, reserved, 64);
    gate.assert_is_const(ctx, &starts_before_reservation, &F::ZERO);
    let ends_after_reservation = range.is_less_than(ctx, expires, approved_until, 64);
    gate.assert_is_const(ctx, &ends_after_reservation, &F::ZERO);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kagemusha_v1_poseidon::from_u128;
    use halo2_proofs::{
        dev::MockProver,
        halo2curves::pasta::{Fp, Fq},
    };

    // Modular subject tests exercise the actual helper equations only. These assigned
    // upstream cells are explicit mathematical fixtures, not authenticated C/W or Native owners.
    #[derive(Clone, Copy)]
    enum Mutation {
        SignedByte(usize),
        Purpose,
        NativeOperation,
        NativeOperationZero,
        CandidateZero,
        BodyZero,
        NotOutgoing,
        ReservationStart,
        ReservationEnd,
        ApprovalEmpty,
        ReservationEmpty,
        ActualIndex,
    }
    fn check<F: KagemushaPoseidonFieldV1>(operation: u64, mutation: Option<Mutation>) -> bool {
        const K: usize = 12;
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(K)
            .use_lookup_bits(K - 1)
            .use_instance_columns(0);
        let range = builder.range_chip();
        let ctx = builder.main(0);
        let index_before = (1_u128 << 100) + 41;
        let index_after = index_before + 1;
        let mut signed = [0_u8; S::TOTAL_BYTES];
        signed[S::OPERATION_TAG.start] = operation as u8;
        signed[S::SECURE_INDEX_BEFORE].copy_from_slice(&index_before.to_le_bytes());
        signed[S::SECURE_INDEX_AFTER].copy_from_slice(&index_after.to_le_bytes());
        signed[S::TRANSITION_STATEMENT_DIGEST].fill(0x31);
        signed[S::CANDIDATE_ENVELOPE_DIGEST].fill(0x32);
        signed[S::TERMINAL_BODY_COMMITMENT].fill(0x33);
        if let Some(Mutation::SignedByte(index)) = mutation {
            signed[index] ^= 1;
        }
        let canonical_subject = signed.map(|byte| ctx.load_witness(F::from(u64::from(byte))));
        let assigned_digest = |ctx: &mut halo2_base::Context<F>, value: u8| {
            core::array::from_fn(|_| {
                let cell = ctx.load_witness(F::from(u64::from(value)));
                PastaSha256ByteV1::range_checked(ctx, &range, cell)
            })
        };
        let binding = KagemushaOrdinaryGuardDataBindingV1 {
            digests: core::array::from_fn(|_| assigned_digest(ctx, 0x51)),
            account_binding: assigned_digest(ctx, 0x52),
            financial_authority_commitment:
                super::super::guard_bundle::device_authority_commitment_v1([0x41; 32]).map(|byte| {
                    let cell = ctx.load_witness(F::from(u64::from(byte)));
                    PastaSha256ByteV1::range_checked(ctx, &range, cell)
                }),
            canonical_subject,
            approval_purpose: ctx.load_witness(F::from(
                if matches!(mutation, Some(Mutation::Purpose)) {
                    2
                } else {
                    1
                },
            )),
            approval_operation_id: assigned_digest(ctx, 0x41),
            approval_nonce: assigned_digest(ctx, 0x42),
            approval_issued_at_ms: ctx.load_witness(F::from(200)),
            approval_expires_at_ms: ctx.load_witness(F::from(
                if matches!(mutation, Some(Mutation::ApprovalEmpty)) {
                    200
                } else {
                    300
                },
            )),
        };
        let sources = KagemushaOrdinaryTerminalSubjectSourcesV1 {
            operation: ctx.load_witness(F::from(
                if matches!(mutation, Some(Mutation::NotOutgoing)) {
                    1
                } else {
                    operation
                },
            )),
            secure_index_before: ctx.load_witness(from_u128::<F>(
                index_before + u128::from(matches!(mutation, Some(Mutation::ActualIndex))),
            )),
            secure_index_after: ctx.load_witness(from_u128::<F>(index_after)),
            transition_digest: assigned_digest(ctx, 0x31),
            candidate_digest: assigned_digest(
                ctx,
                if matches!(mutation, Some(Mutation::CandidateZero)) {
                    0
                } else {
                    0x32
                },
            ),
            terminal_body_digest: assigned_digest(
                ctx,
                if matches!(mutation, Some(Mutation::BodyZero)) {
                    0
                } else {
                    0x33
                },
            ),
            native_operation_id: assigned_digest(
                ctx,
                match mutation {
                    Some(Mutation::NativeOperation) => 0x42,
                    Some(Mutation::NativeOperationZero) => 0,
                    _ => 0x41,
                },
            ),
            reservation_issued_at_ms: ctx.load_witness(F::from(
                if matches!(mutation, Some(Mutation::ReservationStart)) {
                    201
                } else {
                    100
                },
            )),
            reservation_expires_at_ms: ctx.load_witness(F::from(match mutation {
                Some(Mutation::ReservationEnd) => 299,
                Some(Mutation::ReservationEmpty) => 100,
                _ => 400,
            })),
        };
        constrain_ordinary_terminal_subject_v1(&mut builder, sources, &binding).unwrap();
        builder.calculate_params(Some(9));
        MockProver::run(K as u32, &builder, vec![])
            .expect("modular ordinary terminal subject fits")
            .verify()
            .is_ok()
    }
    fn both(mutation: Option<Mutation>) {
        for operation in [2, 4] {
            assert_eq!(check::<Fp>(operation, mutation), mutation.is_none());
            assert_eq!(check::<Fq>(operation, mutation), mutation.is_none());
        }
    }
    #[test]
    fn ordinary_terminal_subject_both_fields_accept_actual_financial_indexes() {
        both(None);
    }
    #[test]
    fn ordinary_terminal_subject_rejects_preparation_bootstrap_or_substituted_original() {
        for mutation in [
            Mutation::Purpose,
            Mutation::NativeOperation,
            Mutation::NativeOperationZero,
            Mutation::CandidateZero,
            Mutation::BodyZero,
            Mutation::NotOutgoing,
            Mutation::ActualIndex,
        ] {
            both(Some(mutation));
        }
    }
    #[test]
    fn ordinary_terminal_subject_rejects_each_substituted_scope_byte() {
        for slot in [
            S::OPERATION_TAG,
            S::TRANSITION_STATEMENT_DIGEST,
            S::CANDIDATE_ENVELOPE_DIGEST,
            S::TERMINAL_BODY_COMMITMENT,
            S::SECURE_INDEX_BEFORE,
            S::SECURE_INDEX_AFTER,
        ] {
            for offset in [slot.start, slot.end - 1] {
                both(Some(Mutation::SignedByte(offset)));
            }
        }
    }
    #[test]
    fn ordinary_terminal_subject_same_signed_interval_must_fit_original_native_reservation() {
        for mutation in [
            Mutation::ReservationStart,
            Mutation::ReservationEnd,
            Mutation::ApprovalEmpty,
            Mutation::ReservationEmpty,
        ] {
            both(Some(mutation));
        }
    }
}
