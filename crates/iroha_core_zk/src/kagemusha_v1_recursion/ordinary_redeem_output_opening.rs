//! Fixed ordinary Redeem output, complete manifest and original beneficiary account openings.
//!
//! Both outgoing operations queue this same relation. The actual State operation selects the
//! public Redeem digest; inactive raw originals are empty and cannot claim a beneficiary,
//! release manifest or money. Native lends the original AccountId and authenticated manifest.
use super::{
    canonical_preimage::stream::KagemushaBoundedByteStreamV1,
    composite::assigned_uint_bytes_v1,
    guard_bundle::{assign_bytes, constant_bytes, hash},
    ordinary_cash_opening::{OrdinaryCashClockCellsV1, clock_payload, fill_transcript},
};
use crate::{
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    pasta_sha256::{PastaSha256BitV1, PastaSha256ByteV1, PastaSha256JobsV1},
};
use halo2_base::{
    AssignedValue, Context, QuantumCell,
    gates::{GateInstructions as _, RangeChip, RangeInstructions as _},
};
use iroha_data_model::{
    account::AccountId,
    kagemusha::{
        KAGEMUSHA_ORDINARY_CASH_CLOCK_DOMAIN_V1, KAGEMUSHA_ORDINARY_REDEEM_OUTPUT_DOMAIN_V1,
        KAGEMUSHA_RELEASE_MANIFEST_MAX_BYTES_V1, KagemushaOrdinaryCashClockContextV1,
        KagemushaOrdinaryRedemptionOutputV1,
    },
};
use norito::codec::Encode;
type Bytes<F> = [PastaSha256ByteV1<F>; 32];
/// Complete original AccountId encoding is bounded independently of offered vector lengths.
const ACCOUNT_MAX: usize = 4096;
/// Original data lent by Native; these are not a constructor for a beneficiary owner.
pub(super) struct OrdinaryRedeemOutputWitnessV1<'a> {
    pub(super) output: &'a KagemushaOrdinaryRedemptionOutputV1,
    pub(super) beneficiary: &'a AccountId,
    pub(super) manifest_original: &'a [u8],
}
pub(super) struct OrdinaryRedeemOutputSourcesV1<'a, F: KagemushaPoseidonFieldV1> {
    pub(super) operation: AssignedValue<F>,
    pub(super) amount: AssignedValue<F>,
    pub(super) release: Bytes<F>,
    pub(super) network: Bytes<F>,
    pub(super) asset: Bytes<F>,
    pub(super) incarnation: Bytes<F>,
    pub(super) scale: AssignedValue<F>,
    pub(super) pool: Bytes<F>,
    pub(super) beneficiary_account_binding: Bytes<F>,
    pub(super) before: Bytes<F>,
    pub(super) after: Bytes<F>,
    pub(super) nullifier: Bytes<F>,
    pub(super) lifecycle: Bytes<F>,
    pub(super) preparation_clock: &'a OrdinaryCashClockCellsV1<F>,
    pub(super) preparation_clock_specimen: &'a KagemushaOrdinaryCashClockContextV1,
    pub(super) expected_manifest_digest: Bytes<F>,
    pub(super) expected_semantic_digest: Bytes<F>,
}
pub(super) struct OrdinaryRedeemOutputOpeningV1<F: KagemushaPoseidonFieldV1> {
    pub(super) manifest_digest: Bytes<F>,
    pub(super) output_digest: Bytes<F>,
}
fn equal<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    a: Bytes<F>,
    b: Bytes<F>,
) {
    for (a, b) in a.into_iter().zip(b) {
        let delta = range.gate().sub(ctx, a.quantum_cell(), b.quantum_cell());
        range.gate().assert_is_const(ctx, &delta, &F::ZERO);
    }
}
fn selected<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    e: AssignedValue<F>,
    raw: Bytes<F>,
) -> Bytes<F> {
    raw.map(|b| {
        let value = range.gate().mul(ctx, b.quantum_cell(), e);
        PastaSha256ByteV1::range_checked(ctx, range, value)
    })
}
fn raw_original<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    raw: &[u8],
    maximum: usize,
    enabled: AssignedValue<F>,
) -> Result<KagemushaBoundedByteStreamV1<F>, String> {
    if raw.len() > maximum {
        return Err("ordinary Redeem original exceeds its fixed capacity".into());
    }
    let mut padding = vec![0; maximum];
    padding[..raw.len()].copy_from_slice(raw);
    let bytes = assign_bytes(ctx, range, &padding);
    let actual_len = ctx.load_witness(F::from(raw.len() as u64));
    let stream = KagemushaBoundedByteStreamV1::constrain(ctx, range, bytes, actual_len)?;
    let empty = range.gate().is_zero(ctx, stream.actual_len());
    let present = range.gate().not(ctx, empty);
    ctx.constrain_equal(&present, &enabled);
    Ok(stream)
}
/// Hash the entire actual original under the sole maintained domain+LE64(length) grammar.
fn whole_original_digest<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    domain: &[u8],
    original: &KagemushaBoundedByteStreamV1<F>,
) -> Result<Bytes<F>, String> {
    let mut prefix = constant_bytes(domain);
    prefix.extend(assigned_uint_bytes_v1(
        ctx,
        range.gate(),
        original.actual_len(),
        64,
    ));
    let prefix_len = prefix.len();
    let length = ctx.load_constant(F::from(prefix_len as u64));
    let prefix = KagemushaBoundedByteStreamV1::constrain(ctx, range, prefix, length)?;
    let capacity = prefix_len
        .checked_add(original.bytes().len())
        .ok_or("Redeem original capacity overflow")?;
    let stream = prefix.concat(ctx, range, original, capacity)?;
    let digest =
        jobs.digest_bounded_constrained(ctx, range, stream.bytes(), stream.actual_len())?;
    let mut bytes = Vec::with_capacity(32);
    for word in digest {
        let bits = PastaSha256BitV1::decompose(ctx, range.gate(), word, 32);
        for start in [24, 16, 8, 0] {
            bytes.push(PastaSha256ByteV1::from_bits_le(
                ctx,
                range.gate(),
                &bits[start..start + 8],
            ));
        }
    }
    bytes
        .try_into()
        .map_err(|_| "ordinary Redeem SHA width differs".into())
}
/// Same complete fixed topology for Send and Redeem, with selector-bound empty inactive originals.
pub(super) fn constrain_ordinary_redeem_output_opening_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    sources: OrdinaryRedeemOutputSourcesV1<'_, F>,
    native: Option<OrdinaryRedeemOutputWitnessV1<'_>>,
) -> Result<OrdinaryRedeemOutputOpeningV1<F>, String> {
    let redeem = range
        .gate()
        .is_equal(ctx, sources.operation, QuantumCell::Constant(F::from(4)));
    let present = ctx.load_witness(F::from(u64::from(native.is_some())));
    range.gate().assert_bit(ctx, present);
    ctx.constrain_equal(&present, &redeem);
    let empty_manifest: &[u8] = &[];
    let (beneficiary, manifest) = native.as_ref().map_or((Vec::new(), empty_manifest), |s| {
        (s.beneficiary.encode(), s.manifest_original)
    });
    if let Some(native) = &native {
        native
            .output
            .validate_against_clock(sources.preparation_clock_specimen)?;
    }
    let beneficiary = raw_original(ctx, range, &beneficiary, ACCOUNT_MAX, redeem)?;
    let manifest = raw_original(
        ctx,
        range,
        manifest,
        KAGEMUSHA_RELEASE_MANIFEST_MAX_BYTES_V1,
        redeem,
    )?;
    // These are the actual Model account-binding and admitted release manifest domains.
    let account = whole_original_digest(
        ctx,
        range,
        jobs,
        b"iroha:kagemusha:v1:app-approval-account\0",
        &beneficiary,
    )?;
    let manifest = whole_original_digest(
        ctx,
        range,
        jobs,
        b"iroha:kagemusha:v1:manifest\0",
        &manifest,
    )?;
    for (a, b) in account.into_iter().zip(sources.beneficiary_account_binding) {
        let delta = range.gate().sub(ctx, a.quantum_cell(), b.quantum_cell());
        let active = range.gate().mul(ctx, delta, redeem);
        range.gate().assert_is_const(ctx, &active, &F::ZERO);
    }
    let manifest = selected(ctx, range, redeem, manifest);
    equal(ctx, range, manifest, sources.expected_manifest_digest);
    let clock = clock_payload(
        ctx,
        range,
        sources.preparation_clock,
        sources.preparation_clock_specimen,
    )?;
    let mut clock_message = constant_bytes(KAGEMUSHA_ORDINARY_CASH_CLOCK_DOMAIN_V1);
    clock_message.extend(clock);
    let clock = hash(ctx, jobs, clock_message)?;
    let inactive = KagemushaOrdinaryRedemptionOutputV1 {
        version: 1,
        release_id: [0; 32],
        network_id: [0; 32],
        normalized_asset_id: [0; 32],
        asset_incarnation: [0; 32],
        scale: 0,
        reserve_pool_id: [0; 32],
        amount: 0,
        beneficiary_account_binding: [0; 32],
        sender_before_commitment: [0; 32],
        sender_after_commitment: [0; 32],
        transition_nullifier: [0; 32],
        lifecycle_digest: [0; 32],
        artifact_manifest_digest: [0; 32],
        clock_context_digest: [0; 32],
        prepared_at_ms: 0,
    };
    let specimen = native.as_ref().map_or(&inactive, |s| s.output);
    let (transcript, _) = fill_transcript(
        specimen.binding_transcript(),
        KAGEMUSHA_ORDINARY_REDEEM_OUTPUT_DOMAIN_V1,
        vec![
            ("version", constant_bytes(&1_u16.to_le_bytes())),
            ("release_id", sources.release.to_vec()),
            ("network_id", sources.network.to_vec()),
            ("normalized_asset_id", sources.asset.to_vec()),
            ("asset_incarnation", sources.incarnation.to_vec()),
            (
                "scale",
                assigned_uint_bytes_v1(ctx, range.gate(), sources.scale, 32),
            ),
            ("reserve_pool_id", sources.pool.to_vec()),
            (
                "amount",
                assigned_uint_bytes_v1(ctx, range.gate(), sources.amount, 128),
            ),
            ("beneficiary_account_binding", account.to_vec()),
            ("sender_before_commitment", sources.before.to_vec()),
            ("sender_after_commitment", sources.after.to_vec()),
            ("transition_nullifier", sources.nullifier.to_vec()),
            ("lifecycle_digest", sources.lifecycle.to_vec()),
            ("artifact_manifest_digest", manifest.to_vec()),
            ("clock_context_digest", clock.to_vec()),
            (
                "prepared_at_ms",
                assigned_uint_bytes_v1(
                    ctx,
                    range.gate(),
                    sources.preparation_clock.upper_at_ms,
                    64,
                ),
            ),
        ],
    )?;
    let output = hash(ctx, jobs, transcript)?;
    let output = selected(ctx, range, redeem, output);
    let expected = selected(ctx, range, redeem, sources.expected_semantic_digest);
    equal(ctx, range, output, expected);
    Ok(OrdinaryRedeemOutputOpeningV1 {
        manifest_digest: manifest,
        output_digest: output,
    })
}

#[cfg(test)]
#[path = "ordinary_redeem_output_opening_tests.rs"]
mod tests;
