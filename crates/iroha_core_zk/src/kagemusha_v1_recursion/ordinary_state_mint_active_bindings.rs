//! Genuine active Mint113-to-State scope and private credit-opening equations.
//!
//! The enclosing ordinary State must additionally consume the actual Mint113 current proof,
//! complete history and the independently finalized MintAuthority current proof/history, and
//! open the exact full authorization/finalized-credit originals to its replay envelope. This
//! component supplies no finalized debit, Native opening loan, global reservation or State grant.
use super::super::{
    canonical_preimage::{
        assemble_canonical_preimage_v1, field_stream::framed_hash_v1,
        stream::KagemushaBoundedByteStreamV1,
    },
    composite::{assigned_digest_bytes_v1, assigned_uint_bytes_v1},
    guard_bundle::{assign_bytes, constant_bytes, digest_limbs_assigned, hash},
    ordinary_mint_circuit::ORDINARY_MINT_PUBLIC_INSTANCE_COUNT_V1,
    ordinary_mint_public::ORDINARY_MINT_PUBLIC_PREFIX_V1,
};
use crate::{
    kagemusha_v1_poseidon::{KagemushaPoseidonFieldV1, from_u128},
    pasta_sha256::{PastaSha256ByteV1, PastaSha256JobsV1},
};
use halo2_base::{
    AssignedValue, Context,
    gates::{GateInstructions as _, RangeChip, RangeInstructions as _},
};
use iroha_data_model::kagemusha::*;

type Bytes<F> = [PastaSha256ByteV1<F>; 32];
type DigestCells<F> = [AssignedValue<F>; 2];

/// Cells copied from the enclosing actual assigned State and issuer-bound ordinary Guard.
/// Copying these mathematical cells is not a public authority constructor or a Native loan.
pub(in crate::kagemusha_v1_recursion) struct OrdinaryMintStateBindingCellsV1<
    F: KagemushaPoseidonFieldV1,
> {
    pub(in crate::kagemusha_v1_recursion) operation: AssignedValue<F>,
    pub(in crate::kagemusha_v1_recursion) amount: AssignedValue<F>,
    pub(in crate::kagemusha_v1_recursion) predecessor_outer: DigestCells<F>,
    pub(in crate::kagemusha_v1_recursion) predecessor_sequence: AssignedValue<F>,
    pub(in crate::kagemusha_v1_recursion) financial_epoch: DigestCells<F>,
    pub(in crate::kagemusha_v1_recursion) release: DigestCells<F>,
    pub(in crate::kagemusha_v1_recursion) suite: DigestCells<F>,
    pub(in crate::kagemusha_v1_recursion) vk: DigestCells<F>,
    pub(in crate::kagemusha_v1_recursion) network: DigestCells<F>,
    pub(in crate::kagemusha_v1_recursion) asset: DigestCells<F>,
    pub(in crate::kagemusha_v1_recursion) incarnation: DigestCells<F>,
    pub(in crate::kagemusha_v1_recursion) pool: DigestCells<F>,
    pub(in crate::kagemusha_v1_recursion) lane: DigestCells<F>,
    pub(in crate::kagemusha_v1_recursion) profile: DigestCells<F>,
    pub(in crate::kagemusha_v1_recursion) scale: AssignedValue<F>,
    pub(in crate::kagemusha_v1_recursion) policy_epoch: AssignedValue<F>,
    pub(in crate::kagemusha_v1_recursion) replay_credit_id: DigestCells<F>,
    pub(in crate::kagemusha_v1_recursion) credential: Bytes<F>,
    pub(in crate::kagemusha_v1_recursion) account_binding: Bytes<F>,
    pub(in crate::kagemusha_v1_recursion) financial_authority: Bytes<F>,
    pub(in crate::kagemusha_v1_recursion) provider_root: Bytes<F>,
}

/// Private mathematical openings borrowed by a genuine Native source producer. No decoder or
/// exported SDK value can construct the actual source loan required by that producer.
pub(in crate::kagemusha_v1_recursion) struct OrdinaryMintStateOpeningV1<'a> {
    pub(in crate::kagemusha_v1_recursion) credit: &'a KagemushaCreditOpeningV1,
    pub(in crate::kagemusha_v1_recursion) encrypted_credit: &'a [u8],
}

fn equal_if<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    actual: AssignedValue<F>,
    expected: AssignedValue<F>,
    enabled: AssignedValue<F>,
) {
    let difference = range.gate().sub(ctx, actual, expected);
    let selected = range.gate().mul(ctx, difference, enabled);
    range.gate().assert_is_const(ctx, &selected, &F::ZERO);
}
fn digest_if<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    actual: DigestCells<F>,
    expected: DigestCells<F>,
    enabled: AssignedValue<F>,
) {
    for (actual, expected) in actual.into_iter().zip(expected) {
        equal_if(ctx, range, actual, expected, enabled);
    }
}
fn bytes_if<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    actual: &[PastaSha256ByteV1<F>],
    expected: DigestCells<F>,
    enabled: AssignedValue<F>,
) -> Result<(), String> {
    let bytes: &Bytes<F> = actual
        .try_into()
        .map_err(|_| "ordinary Mint digest width")?;
    let actual = digest_limbs_assigned(ctx, bytes);
    digest_if(ctx, range, actual, expected, enabled);
    Ok(())
}
fn nonzero_if<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    bytes: &[PastaSha256ByteV1<F>],
    enabled: AssignedValue<F>,
) {
    let sum = range
        .gate()
        .sum(ctx, bytes.iter().map(|b| b.quantum_cell()));
    let zero = range.gate().is_zero(ctx, sum);
    let forbidden = range.gate().mul(ctx, zero, enabled);
    range.gate().assert_is_const(ctx, &forbidden, &F::ZERO);
}
fn framed_hash<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    domain: &[u8],
    bytes: Vec<PastaSha256ByteV1<F>>,
) -> Result<Bytes<F>, String> {
    let mut message = constant_bytes(domain);
    message.extend(constant_bytes(&(bytes.len() as u64).to_le_bytes()));
    message.extend(bytes);
    hash(ctx, jobs, message)
}

/// Join the true113 semantic offsets to the actual State and the same private credit openings.
/// Both active and inactive branches emit the same 384-byte capacity and 32-byte opening
/// graphs. The active canonical original is 327 bytes; inactive buffers, length and the entire
/// 79-cell semantic prefix are explicitly zero. Transport padding never enters the digest.
/// The generic cash/Bootstrap approval cannot satisfy this dedicated pre-debit Mint family.
pub(in crate::kagemusha_v1_recursion) fn constrain_ordinary_mint_state_bindings_v1<
    F: KagemushaPoseidonFieldV1,
>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    column: &[AssignedValue<F>],
    state: &OrdinaryMintStateBindingCellsV1<F>,
    opening: Option<OrdinaryMintStateOpeningV1<'_>>,
) -> Result<(), String> {
    if column.len() != ORDINARY_MINT_PUBLIC_INSTANCE_COUNT_V1
        || opening.as_ref().is_some_and(|o| {
            o.encrypted_credit.len() != KAGEMUSHA_ENCRYPTED_CREDIT_CANONICAL_BYTES_V1
        })
    {
        return Err("ordinary Mint State full column/canonical cipher length differs".into());
    }
    let gate = range.gate();
    let enabled = gate.is_equal(
        ctx,
        state.operation,
        halo2_base::QuantumCell::Constant(F::ONE),
    );
    gate.assert_bit(ctx, enabled);
    let present = ctx.load_witness(F::from(u64::from(opening.is_some())));
    ctx.constrain_equal(&present, &enabled);
    let inactive = gate.not(ctx, enabled);
    for cell in &column[..ORDINARY_MINT_PUBLIC_PREFIX_V1] {
        let selected = gate.mul(ctx, *cell, inactive);
        gate.assert_is_const(ctx, &selected, &F::ZERO);
    }
    let digest = |i: usize| [column[2 * i], column[2 * i + 1]];
    let scalar = |i: usize| column[68 + i];
    let one = ctx.load_constant(F::ONE);
    equal_if(ctx, range, scalar(0), one, enabled);
    for (index, expected) in [
        (7, state.release),
        (8, state.suite),
        (9, state.vk),
        (11, state.network),
        (12, state.asset),
        (13, state.incarnation),
        (14, state.pool),
        (15, state.lane),
        (18, state.financial_epoch),
        (20, state.profile),
        (24, state.replay_credit_id),
        (31, state.predecessor_outer),
    ] {
        digest_if(ctx, range, digest(index), expected, enabled);
    }
    for (index, expected) in [
        (2, &state.credential),
        (16, &state.account_binding),
        (19, &state.financial_authority),
        (33, &state.provider_root),
    ] {
        let limbs = digest_limbs_assigned(ctx, expected);
        digest_if(ctx, range, digest(index), limbs, enabled);
    }
    for (index, expected) in [
        (1, state.amount),
        (2, state.scale),
        (3, state.policy_epoch),
        (4, state.predecessor_sequence),
    ] {
        equal_if(ctx, range, scalar(index), expected, enabled);
    }
    // Historical Mint PI need not equal the newly refreshed W2 lease. The actual Mint113 proof
    // separately authenticates its selected original PI and its original signing interval.
    let credit = opening.as_ref().map(|o| o.credit);
    let private = |select: fn(&KagemushaCreditOpeningV1) -> &[u8; 32]| {
        credit.map(select).copied().unwrap_or([0; 32])
    };
    let recipient = assign_bytes(ctx, range, &private(|o| &o.recipient_binding_opening));
    let commitment = assign_bytes(ctx, range, &private(|o| &o.credit_commitment_opening));
    let recovery = assign_bytes(ctx, range, &private(|o| &o.recovery_nonce));
    for bytes in [&recipient, &commitment, &recovery] {
        nonzero_if(ctx, range, bytes, enabled);
        for byte in bytes {
            let selected = gate.mul(ctx, byte.quantum_cell(), inactive);
            gate.assert_is_const(ctx, &selected, &F::ZERO);
        }
    }
    let actual_version = ctx.load_witness(F::from(u64::from(credit.map_or(0, |o| o.version))));
    let actual_amount = ctx.load_witness(from_u128::<F>(credit.map_or(0, |o| o.amount)));
    equal_if(ctx, range, actual_version, scalar(0), enabled);
    equal_if(ctx, range, actual_amount, state.amount, enabled);
    for cell in [actual_version, actual_amount] {
        let selected = gate.mul(ctx, cell, inactive);
        gate.assert_is_const(ctx, &selected, &F::ZERO);
    }
    let actual_credit_id = assign_bytes(ctx, range, &private(|o| &o.credit_id));
    bytes_if(
        ctx,
        range,
        &actual_credit_id,
        state.replay_credit_id,
        enabled,
    )?;
    for byte in &actual_credit_id {
        let selected = gate.mul(ctx, byte.quantum_cell(), inactive);
        gate.assert_is_const(ctx, &selected, &F::ZERO);
    }
    let operation = assigned_digest_bytes_v1(ctx, gate, digest(6));
    let credential = assigned_digest_bytes_v1(ctx, gate, digest(2));
    let recipient_original = assemble_canonical_preimage_v1(
        ctx,
        range,
        &kagemusha_recipient_credential_commitment_preimage_layout_v1()
            .map_err(|e| e.to_string())?,
        &KAGEMUSHA_RECIPIENT_CREDENTIAL_COMMITMENT_PREIMAGE_FIELD_RANGES_V1,
        &[&operation, &credential, &recipient],
    )?;
    let recipient_digest = framed_hash(
        ctx,
        jobs,
        b"iroha:kagemusha:v1:recipient-credential-commitment\0",
        recipient_original,
    )?;
    bytes_if(ctx, range, &recipient_digest, digest(21), enabled)?;
    let version = constant_bytes(&1_u16.to_le_bytes());
    let network = assigned_digest_bytes_v1(ctx, gate, digest(11));
    let asset = assigned_digest_bytes_v1(ctx, gate, digest(12));
    let incarnation = assigned_digest_bytes_v1(ctx, gate, digest(13));
    let scale = assigned_uint_bytes_v1(ctx, gate, scalar(2), 32);
    let pool = assigned_digest_bytes_v1(ctx, gate, digest(14));
    let amount = assigned_uint_bytes_v1(ctx, gate, scalar(1), 128);
    let account = assigned_digest_bytes_v1(ctx, gate, digest(17));
    let key = assigned_digest_bytes_v1(ctx, gate, digest(23));
    let credit_original = assemble_canonical_preimage_v1(
        ctx,
        range,
        &kagemusha_mint_credit_opening_commitment_preimage_layout_v1()
            .map_err(|e| e.to_string())?,
        &KAGEMUSHA_MINT_CREDIT_OPENING_COMMITMENT_PREIMAGE_FIELD_RANGES_V1,
        &[
            &version,
            &network,
            &asset,
            &incarnation,
            &scale,
            &pool,
            &amount,
            &account,
            &key,
            &commitment,
        ],
    )?;
    let credit_digest = framed_hash(
        ctx,
        jobs,
        b"iroha:kagemusha:v1:mint-credit-opening-commitment\0",
        credit_original,
    )?;
    bytes_if(ctx, range, &credit_digest, digest(22), enabled)?;
    let actual = opening.as_ref().map_or(&[][..], |o| o.encrypted_credit);
    let mut ciphertext = vec![0; KAGEMUSHA_ENCRYPTED_CREDIT_MAX_BYTES_V1];
    ciphertext[..actual.len()].copy_from_slice(actual);
    let ciphertext = assign_bytes(ctx, range, &ciphertext);
    for byte in &ciphertext {
        let selected = gate.mul(ctx, byte.quantum_cell(), inactive);
        gate.assert_is_const(ctx, &selected, &F::ZERO);
    }
    let length = ctx.load_witness(F::from(actual.len() as u64));
    let selected_length = gate.mul(
        ctx,
        enabled,
        halo2_base::QuantumCell::Constant(F::from(
            KAGEMUSHA_ENCRYPTED_CREDIT_CANONICAL_BYTES_V1 as u64,
        )),
    );
    ctx.constrain_equal(&length, &selected_length);
    let ciphertext = KagemushaBoundedByteStreamV1::constrain(ctx, range, ciphertext, length)?;
    let ciphertext_digest = framed_hash_v1(
        ctx,
        range,
        jobs,
        b"iroha:kagemusha:v1:ciphertext\0",
        &ciphertext,
    )?;
    bytes_if(ctx, range, &ciphertext_digest, digest(25), enabled)?;
    Ok(())
}

#[cfg(test)]
#[path = "ordinary_state_mint_active_bindings_tests.rs"]
mod tests;
