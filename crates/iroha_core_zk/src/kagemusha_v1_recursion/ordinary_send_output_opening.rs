//! Complete ordinary Send output, actual encrypted stream and amount-bound credit opening.
//!
//! Sender custody/encryption and receiver decryption are private Native actions. This relation
//! authenticates their exact selected commitments and all semantic fields; it does not treat
//! ciphertext syntax, a plaintext opening or a clock projection as Native authority.
use super::{
    canonical_preimage::stream::KagemushaBoundedByteStreamV1,
    composite::assigned_uint_bytes_v1,
    guard_bundle::{assign_bytes, constant_bytes, hash},
    ordinary_cash_opening::{OrdinaryCashClockCellsV1, clock_payload, fill_transcript},
    ordinary_receiver_request_opening::OrdinaryReceiverRequestOpeningV1,
};
use crate::{
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    pasta_sha256::{PastaSha256BitV1, PastaSha256ByteV1, PastaSha256JobsV1},
};
use halo2_base::{
    AssignedValue, Context, QuantumCell,
    gates::{GateInstructions as _, RangeChip, RangeInstructions as _},
};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_ENCRYPTED_CREDIT_MAX_BYTES_V1, KAGEMUSHA_ORDINARY_CASH_CLOCK_DOMAIN_V1,
    KAGEMUSHA_ORDINARY_CREDIT_ID_DOMAIN_V1, KAGEMUSHA_ORDINARY_PAYMENT_OUTPUT_DOMAIN_V1,
    KAGEMUSHA_ORDINARY_TRANSITION_NULLIFIER_DOMAIN_V1,
    KAGEMUSHA_PEER_CREDIT_OPENING_COMMITMENT_DOMAIN_V1, KagemushaCreditOpeningV1,
    KagemushaOrdinaryCashClockContextV1, KagemushaOrdinaryPaymentOutputV1,
};
type Bytes<F> = [PastaSha256ByteV1<F>; 32];
/// Actual selected State and independently admitted receiver/Native preparation sources.
pub(super) struct OrdinarySendOutputSourcesV1<'a, F: KagemushaPoseidonFieldV1> {
    pub(super) operation: AssignedValue<F>,
    pub(super) amount: AssignedValue<F>,
    pub(super) before: Bytes<F>,
    pub(super) after: Bytes<F>,
    pub(super) predecessor_secure_index: AssignedValue<F>,
    pub(super) predecessor_epoch: Bytes<F>,
    pub(super) network: Bytes<F>,
    pub(super) sender_lane: Bytes<F>,
    pub(super) reserve_pool: Bytes<F>,
    pub(super) receiver: &'a OrdinaryReceiverRequestOpeningV1<F>,
    pub(super) selected_credit_id: Bytes<F>,
    pub(super) encrypted_credit: &'a KagemushaBoundedByteStreamV1<F>,
    pub(super) preparation_clock: &'a OrdinaryCashClockCellsV1<F>,
    pub(super) preparation_clock_specimen: &'a KagemushaOrdinaryCashClockContextV1,
    pub(super) expected_output_digest: Bytes<F>,
    pub(super) expected_encrypted_digest: Bytes<F>,
    pub(super) expected_nullifier: Bytes<F>,
    pub(super) expected_ciphertext_commitment: Bytes<F>,
}
pub(super) struct OrdinarySendOutputOpeningV1<F: KagemushaPoseidonFieldV1> {
    pub(super) output_digest: Bytes<F>,
    pub(super) encrypted_digest: Bytes<F>,
    pub(super) transition_nullifier: Bytes<F>,
    pub(super) credit_id: Bytes<F>,
    pub(super) ciphertext_commitment: Bytes<F>,
}
fn equal<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    a: Bytes<F>,
    b: Bytes<F>,
) {
    for (a, b) in a.into_iter().zip(b) {
        let d = range.gate().sub(ctx, a.quantum_cell(), b.quantum_cell());
        range.gate().assert_is_const(ctx, &d, &F::ZERO);
    }
}
fn select<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    enabled: AssignedValue<F>,
    bytes: Bytes<F>,
) -> Bytes<F> {
    bytes.map(|byte| {
        let value = range.gate().mul(ctx, byte.quantum_cell(), enabled);
        PastaSha256ByteV1::range_checked(ctx, range, value)
    })
}
fn positive_if<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    enabled: AssignedValue<F>,
    bytes: Bytes<F>,
) {
    let sum = range
        .gate()
        .sum(ctx, bytes.into_iter().map(|b| b.quantum_cell()));
    let zero = range.gate().is_zero(ctx, sum);
    let forbidden = range.gate().mul(ctx, zero, enabled);
    range.gate().assert_is_const(ctx, &forbidden, &F::ZERO);
}
/// Sole maintained full encrypted-credit domain+LE64(length)+all actual raw bytes.
/// Zero inactive streams are allowed only by the actual assigned non-Send operation selector.
fn encrypted_digest<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    stream: &KagemushaBoundedByteStreamV1<F>,
    send: AssignedValue<F>,
) -> Result<Bytes<F>, String> {
    if stream.bytes().len() != KAGEMUSHA_ENCRYPTED_CREDIT_MAX_BYTES_V1 {
        return Err("ordinary encrypted original capacity differs".into());
    }
    let gate = range.gate();
    let zero = gate.is_zero(ctx, stream.actual_len());
    let nonempty = gate.not(ctx, zero);
    ctx.constrain_equal(&nonempty, &send);
    let mut prefix = constant_bytes(b"iroha:kagemusha:v1:ciphertext\0");
    prefix.extend(assigned_uint_bytes_v1(ctx, gate, stream.actual_len(), 64));
    let n = prefix.len();
    let length = ctx.load_constant(F::from(n as u64));
    let prefix = KagemushaBoundedByteStreamV1::constrain(ctx, range, prefix, length)?;
    let full = prefix.concat(
        ctx,
        range,
        stream,
        n + KAGEMUSHA_ENCRYPTED_CREDIT_MAX_BYTES_V1,
    )?;
    let words = jobs.digest_bounded_constrained(ctx, range, full.bytes(), full.actual_len())?;
    let mut bytes = Vec::with_capacity(32);
    for word in words {
        let bits = PastaSha256BitV1::decompose(ctx, gate, word, 32);
        for start in [24, 16, 8, 0] {
            bytes.push(PastaSha256ByteV1::from_bits_le(
                ctx,
                gate,
                &bits[start..start + 8],
            ));
        }
    }
    bytes
        .try_into()
        .map_err(|_| "ordinary encrypted original SHA width differs".into())
}
/// Same fixed complete queue for Send and inactive Redeem. Every returned digest still needs
/// the complete typed SHA claim, whole histories and reciprocal audit equations in Terminal.
pub(super) fn constrain_ordinary_send_output_opening_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    sources: OrdinarySendOutputSourcesV1<'_, F>,
    native_output: &KagemushaOrdinaryPaymentOutputV1,
    native_credit_opening: Option<&KagemushaCreditOpeningV1>,
) -> Result<OrdinarySendOutputOpeningV1<F>, String> {
    if let Some(opening) = native_credit_opening {
        native_output.validate_shape()?;
        opening
            .validate_shape_against(native_output.credit_id, native_output.amount)
            .map_err(|e| e.to_string())?;
    }
    let gate = range.gate();
    let send = gate.is_equal(ctx, sources.operation, QuantumCell::Constant(F::from(2)));
    let present = ctx.load_witness(F::from(u64::from(native_credit_opening.is_some())));
    gate.assert_bit(ctx, present);
    ctx.constrain_equal(&present, &send);
    range.range_check(ctx, sources.amount, 128);
    let zero = gate.is_zero(ctx, sources.amount);
    let bad = gate.mul(ctx, zero, send);
    gate.assert_is_const(ctx, &bad, &F::ZERO);
    range.range_check(ctx, sources.predecessor_secure_index, 128);
    let amount = assigned_uint_bytes_v1(ctx, gate, sources.amount, 128);
    let mut nullifier = constant_bytes(KAGEMUSHA_ORDINARY_TRANSITION_NULLIFIER_DOMAIN_V1);
    nullifier.extend(sources.before);
    nullifier.extend(assigned_uint_bytes_v1(
        ctx,
        gate,
        sources.predecessor_secure_index,
        128,
    ));
    for digest in [
        sources.predecessor_epoch,
        sources.network,
        sources.sender_lane,
        sources.reserve_pool,
    ] {
        nullifier.extend(digest);
    }
    let nullifier = hash(ctx, jobs, nullifier)?;
    let mut credit_id = constant_bytes(KAGEMUSHA_ORDINARY_CREDIT_ID_DOMAIN_V1);
    credit_id.extend(nullifier);
    credit_id.extend(sources.receiver.request_digest);
    let credit_id = hash(ctx, jobs, credit_id)?;
    let opening = native_credit_opening
        .map(|o| {
            [
                o.credit_commitment_opening,
                o.recipient_binding_opening,
                o.recovery_nonce,
            ]
        })
        .unwrap_or([[0; 32]; 3]);
    let opening = opening.map(|d| {
        assign_bytes(ctx, range, &d)
            .try_into()
            .expect("fixed32 credit opening")
    });
    for digest in opening {
        positive_if(ctx, range, send, digest);
        for byte in digest {
            let forbidden = gate.mul_not(ctx, send, byte.quantum_cell());
            gate.assert_is_const(ctx, &forbidden, &F::ZERO);
        }
    }
    let opening_credit = assign_bytes(
        ctx,
        range,
        &native_credit_opening.map_or([0; 32], |o| o.credit_id),
    )
    .try_into()
    .map_err(|_| "credit ID width")?;
    let opening_amount = ctx.load_witness(crate::kagemusha_v1_poseidon::from_u128::<F>(
        native_credit_opening.map_or(0, |o| o.amount),
    ));
    range.range_check(ctx, opening_amount, 128);
    let expected_amount = gate.mul(ctx, sources.amount, send);
    ctx.constrain_equal(&opening_amount, &expected_amount);
    let selected_credit = select(ctx, range, send, credit_id);
    equal(ctx, range, opening_credit, selected_credit);
    equal(ctx, range, sources.selected_credit_id, selected_credit);
    let mut commitment = constant_bytes(KAGEMUSHA_PEER_CREDIT_OPENING_COMMITMENT_DOMAIN_V1);
    commitment.push(PastaSha256ByteV1::constant(0));
    commitment.extend(constant_bytes(&1_u16.to_le_bytes()));
    commitment.extend(sources.receiver.request_digest);
    commitment.extend(sources.receiver.encryption_key);
    commitment.extend_from_slice(&amount);
    for digest in opening {
        commitment.extend(digest);
    }
    let commitment = hash(ctx, jobs, commitment)?;
    let encrypted = encrypted_digest(ctx, range, jobs, sources.encrypted_credit, send)?;
    let clock_raw = clock_payload(
        ctx,
        range,
        sources.preparation_clock,
        sources.preparation_clock_specimen,
    )?;
    let mut clock = constant_bytes(KAGEMUSHA_ORDINARY_CASH_CLOCK_DOMAIN_V1);
    clock.extend(clock_raw);
    let clock = hash(ctx, jobs, clock)?;
    let prepared_at = assigned_uint_bytes_v1(ctx, gate, sources.preparation_clock.upper_at_ms, 64);
    let (output, _) = fill_transcript(
        native_output.binding_transcript(),
        KAGEMUSHA_ORDINARY_PAYMENT_OUTPUT_DOMAIN_V1,
        vec![
            ("version", constant_bytes(&1_u16.to_le_bytes())),
            ("request_digest", sources.receiver.request_digest.to_vec()),
            ("amount", amount),
            ("sender_before_commitment", sources.before.to_vec()),
            ("sender_after_commitment", sources.after.to_vec()),
            ("transition_nullifier", nullifier.to_vec()),
            ("credit_id", credit_id.to_vec()),
            ("ciphertext_commitment", commitment.to_vec()),
            ("encrypted_credit_digest", encrypted.to_vec()),
            ("clock_context_digest", clock.to_vec()),
            ("prepared_at_ms", prepared_at),
        ],
    )?;
    let output = hash(ctx, jobs, output)?;
    let output_digest = select(ctx, range, send, output);
    let encrypted_digest = select(ctx, range, send, encrypted);
    let commitment = select(ctx, range, send, commitment);
    equal(ctx, range, output_digest, sources.expected_output_digest);
    equal(
        ctx,
        range,
        encrypted_digest,
        sources.expected_encrypted_digest,
    );
    equal(ctx, range, nullifier, sources.expected_nullifier);
    equal(
        ctx,
        range,
        commitment,
        sources.expected_ciphertext_commitment,
    );
    Ok(OrdinarySendOutputOpeningV1 {
        output_digest,
        encrypted_digest,
        transition_nullifier: nullifier,
        credit_id: selected_credit,
        ciphertext_commitment: commitment,
    })
}

#[cfg(test)]
#[path = "ordinary_send_output_opening_tests.rs"]
mod tests;
