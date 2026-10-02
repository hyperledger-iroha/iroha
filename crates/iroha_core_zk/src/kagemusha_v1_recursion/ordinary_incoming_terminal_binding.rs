//! Exact incoming purpose1 body openings in the fixed ordinary Guard graph.
//! The actual State/Source/Reserve capabilities are independently verified by the full Commit
//! consumer. These equations bind the complete body to the real platform-signed S and W.
use super::{
    composite::{assigned_digest_bytes_v1, assigned_uint_bytes_v1},
    guard_bundle::{
        KagemushaAssignedGuardBundleV1, assign_bytes, constant_bytes, digest_limbs_assigned, hash,
    },
};
use crate::{
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    pasta_sha256::{PastaSha256ByteV1, PastaSha256JobsV1},
};
use halo2_base::{
    AssignedValue, Context, QuantumCell,
    gates::{GateInstructions as _, RangeChip, RangeInstructions as _},
};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_ORDINARY_INCOMING_TERMINAL_INTENT_BYTES_V1,
    KAGEMUSHA_ORDINARY_TERMINAL_GUARD_COMMIT_DOMAIN_V1,
    KagemushaAppOperationApprovalSigningLayoutV1 as A,
    KagemushaHardwareSelectionSigningLayoutV1 as S, KagemushaOrdinaryIncomingTerminalBodyV1,
};

fn equal_if<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    enabled: AssignedValue<F>,
    left: &[PastaSha256ByteV1<F>],
    right: &[PastaSha256ByteV1<F>],
) -> Result<(), String> {
    if left.len() != right.len() {
        return Err("incoming W1 copy width differs".into());
    }
    for (a, b) in left.iter().zip(right) {
        let d = range.gate().sub(ctx, a.quantum_cell(), b.quantum_cell());
        let bad = range.gate().mul(ctx, enabled, d);
        range.gate().assert_is_const(ctx, &bad, &F::ZERO);
    }
    Ok(())
}
fn scalar<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    raw: &[PastaSha256ByteV1<F>],
) -> AssignedValue<F> {
    let value = range.gate().inner_product(
        ctx,
        raw.iter().map(|b| b.quantum_cell()),
        (0..raw.len()).map(|i| QuantumCell::Constant(F::from(256).pow_vartime([i as u64]))),
    );
    range.range_check(ctx, value, raw.len() * 8);
    value
}
fn nonzero_if<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    enabled: AssignedValue<F>,
    raw: &[PastaSha256ByteV1<F>],
) {
    let sum = range.gate().sum(ctx, raw.iter().map(|b| b.quantum_cell()));
    let zero = range.gate().is_zero(ctx, sum);
    let bad = range.gate().mul(ctx, enabled, zero);
    range.gate().assert_is_const(ctx, &bad, &F::ZERO);
}
fn bytes<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    raw: &[AssignedValue<F>],
) -> Vec<PastaSha256ByteV1<F>> {
    raw.iter()
        .map(|v| PastaSha256ByteV1::range_checked(ctx, range, *v))
        .collect()
}
/// Full 661/693-byte sole Model operands. None is private all-zero inactive graph padding,
/// never a fabricated signed body or a Native source. All operations execute the same SHA jobs.
/// Actual S/Wrapper cells come from their whole canonical originals, not caller digest fields.
pub(super) fn constrain_ordinary_incoming_terminal_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    g: &KagemushaAssignedGuardBundleV1<F>,
    subject: &[AssignedValue<F>; S::TOTAL_BYTES],
    wrapper: &[AssignedValue<F>; A::TOTAL_BYTES],
    original: Option<&KagemushaOrdinaryIncomingTerminalBodyV1>,
) -> Result<(), String> {
    let raw = match original {
        Some(b) => b.intent.binding_transcript()?,
        None => [0; KAGEMUSHA_ORDINARY_INCOMING_TERMINAL_INTENT_BYTES_V1],
    };
    let assigned = assign_bytes(ctx, range, &raw);
    constrain_assigned(ctx, range, jobs, g, subject, wrapper, &assigned)
}
fn constrain_assigned<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    g: &KagemushaAssignedGuardBundleV1<F>,
    subject: &[AssignedValue<F>; S::TOTAL_BYTES],
    wrapper: &[AssignedValue<F>; A::TOTAL_BYTES],
    raw: &[PastaSha256ByteV1<F>],
) -> Result<(), String> {
    if raw.len() != 661 {
        return Err("incoming W1 transcript inventory differs".into());
    }
    let gate = range.gate();
    let mint = gate.is_equal(ctx, g.operation, QuantumCell::Constant(F::ONE));
    let receive = gate.is_equal(ctx, g.operation, QuantumCell::Constant(F::from(3)));
    let incoming = gate.or(ctx, mint, receive);
    let monetary = gate.is_equal(
        ctx,
        wrapper[A::PURPOSE.start],
        QuantumCell::Constant(F::ONE),
    );
    let enabled = gate.and(ctx, incoming, monetary);
    let inactive = gate.not(ctx, enabled);
    let zero = constant_bytes(&[0; 661]);
    equal_if(ctx, range, inactive, raw, &zero)?;
    equal_if(
        ctx,
        range,
        enabled,
        &raw[..2],
        &constant_bytes(&1_u16.to_le_bytes()),
    )?;
    let operation = assigned_uint_bytes_v1(ctx, gate, g.operation, 8);
    equal_if(ctx, range, enabled, &raw[2..3], &operation)?;
    let digest = |n: usize| &raw[3 + n * 32..3 + (n + 1) * 32];
    for n in 0..15 {
        nonzero_if(ctx, range, enabled, digest(n))
    }
    let op = bytes(ctx, range, &wrapper[A::OPERATION_ID]);
    let nonce = bytes(ctx, range, &wrapper[A::NONCE]);
    equal_if(ctx, range, enabled, digest(0), &op)?;
    equal_if(ctx, range, enabled, digest(1), &nonce)?;
    let op_limbs =
        digest_limbs_assigned(ctx, &digest(0).try_into().map_err(|_| "operation width")?);
    let nonce_limbs = digest_limbs_assigned(ctx, &digest(1).try_into().map_err(|_| "nonce width")?);
    let same0 = gate.is_equal(ctx, op_limbs[0], nonce_limbs[0]);
    let same1 = gate.is_equal(ctx, op_limbs[1], nonce_limbs[1]);
    let same = gate.and(ctx, same0, same1);
    let bad = gate.mul(ctx, enabled, same);
    gate.assert_is_const(ctx, &bad, &F::ZERO);
    let candidate = bytes(ctx, range, &subject[S::CANDIDATE_ENVELOPE_DIGEST]);
    equal_if(ctx, range, enabled, digest(9), &candidate)?;
    // The first-release candidate identity is the SHA of this complete PUBLIC State original.
    equal_if(ctx, range, enabled, digest(6), digest(9))?;
    let effect = assigned_digest_bytes_v1(ctx, gate, g.transition_effect);
    equal_if(ctx, range, enabled, digest(3), &effect)?;
    // Signed clock DATA is opened completely; authentic signature/finality/nonce custody remains
    // the independent Clock capability. These bound interval checks do not certify local elapsed time.
    equal_if(
        ctx,
        range,
        enabled,
        &raw[483..485],
        &constant_bytes(&1_u16.to_le_bytes()),
    )?;
    for slot in [485..517, 517..549] {
        nonzero_if(ctx, range, enabled, &raw[slot])
    }
    let lower = scalar(ctx, range, &raw[549..557]);
    let upper = scalar(ctx, range, &raw[557..565]);
    let reversed = range.is_less_than(ctx, upper, lower, 64);
    let bad = gate.mul(ctx, enabled, reversed);
    gate.assert_is_const(ctx, &bad, &F::ZERO);
    // Financial indexes are full u128. They never use the independently maintained Apple counter.
    for (r, s) in [
        (565..581, S::SECURE_INDEX_BEFORE),
        (581..597, S::SECURE_INDEX_AFTER),
    ] {
        let signed = bytes(ctx, range, &subject[s]);
        equal_if(ctx, range, enabled, &raw[r], &signed)?;
    }
    for (r, v, bits) in [
        (597..613, g.predecessor_sequence, 128),
        (613..629, g.successor_sequence, 128),
        (629..637, g.journal_before, 64),
        (637..645, g.journal_after, 64),
    ] {
        let expected = assigned_uint_bytes_v1(ctx, gate, v, bits);
        equal_if(ctx, range, enabled, &raw[r], &expected)?;
    }
    for (before, after) in [
        (565..581, 581..597),
        (597..613, 613..629),
        (629..637, 637..645),
    ] {
        let b = scalar(ctx, range, &raw[before]);
        let a = scalar(ctx, range, &raw[after]);
        let next = gate.add(ctx, b, QuantumCell::Constant(F::ONE));
        let d = gate.sub(ctx, a, next);
        let bad = gate.mul(ctx, enabled, d);
        gate.assert_is_const(ctx, &bad, &F::ZERO);
    }
    for (r, w) in [(645..653, A::ISSUED_AT_MS), (653..661, A::EXPIRES_AT_MS)] {
        let signed = bytes(ctx, range, &wrapper[w]);
        equal_if(ctx, range, enabled, &raw[r], &signed)?;
    }
    let issued = scalar(ctx, range, &raw[645..653]);
    let expires = scalar(ctx, range, &raw[653..661]);
    let early = range.is_less_than(ctx, lower, issued, 64);
    let bad = gate.mul(ctx, enabled, early);
    gate.assert_is_const(ctx, &bad, &F::ZERO);
    let inside = range.is_less_than(ctx, upper, expires, 64);
    let outside = gate.not(ctx, inside);
    let bad = gate.mul(ctx, enabled, outside);
    gate.assert_is_const(ctx, &bad, &F::ZERO);
    let mut intent = constant_bytes(b"iroha:kagemusha:v1:ordinary-incoming-terminal-intent\0");
    intent.extend(constant_bytes(&661_u64.to_le_bytes()));
    intent.extend_from_slice(raw);
    let intent = hash(ctx, jobs, intent)?;
    let mut body = constant_bytes(b"iroha:kagemusha:v1:ordinary-incoming-terminal-body\0");
    body.extend(constant_bytes(&693_u64.to_le_bytes()));
    body.extend_from_slice(raw);
    body.extend_from_slice(&intent);
    let body = hash(ctx, jobs, body)?;
    let recovery = assigned_digest_bytes_v1(ctx, gate, g.recovery_record);
    equal_if(ctx, range, enabled, &intent, &recovery)?;
    let transition_intent = assigned_digest_bytes_v1(ctx, gate, g.transition_intent);
    equal_if(ctx, range, enabled, &body, &transition_intent)?;
    let terminal = bytes(ctx, range, &subject[S::TERMINAL_BODY_COMMITMENT]);
    equal_if(ctx, range, enabled, &body, &terminal)?;
    let mut commit = constant_bytes(KAGEMUSHA_ORDINARY_TERMINAL_GUARD_COMMIT_DOMAIN_V1);
    for d in [&body[..], digest(9), digest(6), digest(3)] {
        commit.extend_from_slice(d)
    }
    let commit = hash(ctx, jobs, commit)?;
    let expected = assigned_digest_bytes_v1(ctx, gate, g.terminal_commit_binding_digest);
    equal_if(ctx, range, enabled, &commit, &expected)?;
    // Incoming W2 must not smuggle a terminal claim. Outgoing semantics remain separately defined.
    let preparation = gate.not(ctx, monetary);
    let incoming_w2 = gate.and(ctx, incoming, preparation);
    equal_if(
        ctx,
        range,
        incoming_w2,
        &expected,
        &constant_bytes(&[0; 32]),
    )?;
    Ok(())
}
#[cfg(test)]
#[path = "ordinary_incoming_terminal_binding_tests.rs"]
mod tests;
