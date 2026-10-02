//! Full fresh incoming W2 preparation/recovery openings on a fixed all-operation graph.
//! Source funding FI/clock remain separate. These equations grant no Native custody or effect.

use super::{
    composite::{assigned_digest_bytes_v1, assigned_uint_bytes_v1},
    guard_bundle::{constant_bytes, digest_limbs_assigned, hash},
};
use crate::{
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    pasta_sha256::{PastaSha256ByteV1, PastaSha256JobsV1},
};
use halo2_base::{
    AssignedValue, Context, QuantumCell,
    gates::{GateInstructions as _, RangeChip, RangeInstructions as _},
};
use iroha_data_model::kagemusha::KAGEMUSHA_ORDINARY_INCOMING_PREPARATION_BYTES_V1;

type DigestCells<F> = [AssignedValue<F>; 2];
type DigestBytes<F> = [PastaSha256ByteV1<F>; 32];

/// Same assigned State edge, real W2 wrapper and source-envelope opening. The caller must
/// obtain FI/clock cells by opening its actual original; neither field is a time/FI capability.
pub(super) struct OrdinaryIncomingPreparationCellsV1<F: KagemushaPoseidonFieldV1> {
    pub(super) operation: AssignedValue<F>,
    pub(super) reservation_digest: DigestCells<F>,
    pub(super) operation_id: DigestBytes<F>,
    pub(super) approval_nonce: DigestBytes<F>,
    pub(super) transition_statement_digest: DigestBytes<F>,
    pub(super) predecessor_state: DigestCells<F>,
    pub(super) successor_state: DigestCells<F>,
    pub(super) financial_control_original_sha256: DigestBytes<F>,
    pub(super) clock_context_digest: DigestBytes<F>,
    pub(super) financial_index_before: AssignedValue<F>,
    pub(super) financial_index_after: AssignedValue<F>,
    pub(super) journal_before: AssignedValue<F>,
    pub(super) journal_after: AssignedValue<F>,
    pub(super) approval_purpose: AssignedValue<F>,
    pub(super) transition_effect: DigestCells<F>,
    pub(super) guard_intent: DigestCells<F>,
    pub(super) guard_recovery: DigestCells<F>,
}

fn require_nonzero_if<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    bytes: &[PastaSha256ByteV1<F>],
    enabled: AssignedValue<F>,
) {
    let sum = range
        .gate()
        .sum(ctx, bytes.iter().map(|b| b.quantum_cell()));
    let zero = range.gate().is_zero(ctx, sum);
    let invalid = range.gate().mul(ctx, enabled, zero);
    range.gate().assert_is_const(ctx, &invalid, &F::ZERO);
}
fn equal_if<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    enabled: AssignedValue<F>,
    left: AssignedValue<F>,
    right: AssignedValue<F>,
) {
    let difference = range.gate().sub(ctx, left, right);
    let invalid = range.gate().mul(ctx, enabled, difference);
    range.gate().assert_is_const(ctx, &invalid, &F::ZERO);
}

/// Derive the two exact sole Model29 digests from the assigned financial edge and the fresh
/// independently selected W2 operands. Mint approval/source FI or a sender clock cannot satisfy
/// these copies. Both SHA jobs are emitted for Bootstrap, outgoing, MintFold and ReceiveFold.
/// The operation cells select the equations; inactive digests never become source authority.
pub(super) fn constrain_ordinary_incoming_preparation_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    cells: OrdinaryIncomingPreparationCellsV1<F>,
) -> Result<(), String> {
    let gate = range.gate();
    range.range_check(ctx, cells.operation, 8);
    let mint = gate.is_equal(ctx, cells.operation, QuantumCell::Constant(F::ONE));
    let receive = gate.is_equal(ctx, cells.operation, QuantumCell::Constant(F::from(3)));
    let enabled = gate.or(ctx, mint, receive);
    gate.assert_bit(ctx, enabled);
    let two = ctx.load_constant(F::from(2));
    equal_if(ctx, range, enabled, cells.approval_purpose, two);
    for (before, after, bits) in [
        (
            cells.financial_index_before,
            cells.financial_index_after,
            128,
        ),
        (cells.journal_before, cells.journal_after, 64),
    ] {
        range.range_check(ctx, before, bits);
        range.range_check(ctx, after, bits);
        let next = gate.add(ctx, before, QuantumCell::Constant(F::ONE));
        equal_if(ctx, range, enabled, after, next);
    }
    for (actual, expected) in cells
        .transition_effect
        .into_iter()
        .zip(cells.reservation_digest)
    {
        equal_if(ctx, range, enabled, actual, expected);
    }
    let mut body = constant_bytes(&1_u16.to_le_bytes());
    let reservation = assigned_digest_bytes_v1(ctx, gate, cells.reservation_digest);
    let predecessor = assigned_digest_bytes_v1(ctx, gate, cells.predecessor_state);
    let successor = assigned_digest_bytes_v1(ctx, gate, cells.successor_state);
    for bytes in [
        reservation.as_slice(),
        cells.operation_id.as_slice(),
        cells.approval_nonce.as_slice(),
        cells.transition_statement_digest.as_slice(),
        predecessor.as_slice(),
        successor.as_slice(),
        cells.financial_control_original_sha256.as_slice(),
        cells.clock_context_digest.as_slice(),
    ] {
        require_nonzero_if(ctx, range, bytes, enabled);
        body.extend_from_slice(bytes);
    }
    let same_low = gate.is_equal(ctx, cells.predecessor_state[0], cells.successor_state[0]);
    let same_high = gate.is_equal(ctx, cells.predecessor_state[1], cells.successor_state[1]);
    let same_state = gate.and(ctx, same_low, same_high);
    let invalid_state = gate.mul(ctx, enabled, same_state);
    gate.assert_is_const(ctx, &invalid_state, &F::ZERO);
    for (value, bits) in [
        (cells.financial_index_before, 128),
        (cells.financial_index_after, 128),
        (cells.journal_before, 64),
        (cells.journal_after, 64),
    ] {
        body.extend(assigned_uint_bytes_v1(ctx, gate, value, bits));
    }
    if body.len() != KAGEMUSHA_ORDINARY_INCOMING_PREPARATION_BYTES_V1 {
        return Err("ordinary incoming full preparation transcript width differs".into());
    }
    let mut intent = constant_bytes(b"iroha:kagemusha:v1:ordinary-incoming-preparation\0");
    intent.extend(constant_bytes(&306_u64.to_le_bytes()));
    intent.extend_from_slice(&body);
    let intent = hash(ctx, jobs, intent)?;
    let mut recovery = constant_bytes(b"iroha:kagemusha:v1:ordinary-incoming-recovery\0");
    recovery.extend(constant_bytes(&338_u64.to_le_bytes()));
    recovery.extend(body);
    recovery.extend_from_slice(&intent);
    let recovery = hash(ctx, jobs, recovery)?;
    for (actual, expected) in [
        (digest_limbs_assigned(ctx, &intent), cells.guard_intent),
        (digest_limbs_assigned(ctx, &recovery), cells.guard_recovery),
    ] {
        for (actual, expected) in actual.into_iter().zip(expected) {
            equal_if(ctx, range, enabled, actual, expected);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "ordinary_incoming_preparation_binding_tests.rs"]
mod tests;
