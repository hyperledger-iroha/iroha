//! Original platform equations over the sole ordinary operation approval wrapper.
//!
//! The public ECDSA r,s values are signature witnesses, never the nonexportable private scalar.
//! All key cells must be copy-bound to the same complete Ed credential original and Native floor;
//! all wrapper/S cells must come from the actual financial State/Guard and durable nonce holder.
//! These equations alone grant no journal, lease or money. Full paired composition is mandatory.

use halo2_base::{
    AssignedValue,
    gates::{GateInstructions as _, RangeInstructions as _, circuit::builder::BaseCircuitBuilder},
};
use halo2_ecc::{bigint::ProperCrtUint, ecc::EcPoint, fields::fp::FpChip};
use halo2_proofs::halo2curves::secp256r1::Fp as P256Base;
use iroha_data_model::kagemusha::{
    KAGEMUSHA_APP_OPERATION_APPROVAL_DOMAIN_V1, KagemushaAppOperationApprovalSigningLayoutV1 as A,
};

use crate::{
    kagemusha_p256_curve_gadget::{
        P256_LIMB_BITS, P256_NUM_LIMBS, app_attest_der_gadget::constrain_p256_canonical_der_v1,
        assert_p256_ecdsa_digest,
    },
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    pasta_sha256::{PastaSha256ByteV1, PastaSha256JobsV1},
};

#[path = "ordinary_apple_original.rs"]
mod original_apple_cbor;

use super::{canonical_preimage::stream::KagemushaBoundedByteStreamV1, guard_bundle::hash};

/// Signature/key cells shared by exact original DER, canonical SEC1, SHA and the P-256 equation.
/// The Native financial secret is separate and cannot enter this structure as a platform scalar.
pub(super) struct OrdinaryPlatformSignatureCellsV1<'a, F: KagemushaPoseidonFieldV1> {
    pub(super) signature_public_key: &'a EcPoint<F, ProperCrtUint<F>>,
    pub(super) enrolled_public_key: &'a EcPoint<F, ProperCrtUint<F>>,
    pub(super) enrolled_public_key_sec1: &'a [AssignedValue<F>; 65],
    pub(super) r: &'a ProperCrtUint<F>,
    pub(super) s: &'a ProperCrtUint<F>,
    pub(super) z: &'a ProperCrtUint<F>,
    pub(super) digest_reduction_quotient: AssignedValue<F>,
}

/// Verify the unmodified Android DER and genuine P-256/SHA256 equation over the exact wrapper.
/// Both raw r,s are retained, including genuine high-S signatures; normalization cannot replace
/// the original digest stored by the Native operation owner.
pub(super) fn constrain_original_android_approval_stream_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    raw_der: &[u8],
    wrapper: &[PastaSha256ByteV1<F>; A::TOTAL_BYTES],
    signature: &OrdinaryPlatformSignatureCellsV1<'_, F>,
) -> Result<KagemushaBoundedByteStreamV1<F>, String> {
    constrain_original_android_signed_message_stream_v1(builder, jobs, raw_der, wrapper, signature)
}

/// Verify the same original DER equation for a separately framed model-owned signed message.
/// This equation admits no Native operation purpose; callers must bind every message byte.
pub(super) fn constrain_original_android_signed_message_stream_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    raw_der: &[u8],
    wrapper: &[PastaSha256ByteV1<F>],
    signature: &OrdinaryPlatformSignatureCellsV1<'_, F>,
) -> Result<KagemushaBoundedByteStreamV1<F>, String> {
    if !(8..=72).contains(&raw_der.len()) {
        return Err("ordinary Android original DER exceeds canonical bound".to_owned());
    }
    let range = builder.range_chip();
    let chip = FpChip::<F, P256Base>::new(&range, P256_LIMB_BITS, P256_NUM_LIMBS);
    let ctx = builder.main(0);
    let der = constrain_p256_canonical_der_v1(&chip, ctx, signature.r, signature.s);
    let raw_length = ctx.load_witness(F::from(raw_der.len() as u64));
    let original = (0..72)
        .map(|i| {
            let byte = ctx.load_witness(F::from(u64::from(raw_der.get(i).copied().unwrap_or(0))));
            PastaSha256ByteV1::range_checked(ctx, &range, byte)
        })
        .collect::<Vec<_>>();
    let original = KagemushaBoundedByteStreamV1::constrain(ctx, &range, original, raw_length)?;
    let canonical =
        KagemushaBoundedByteStreamV1::constrain(ctx, &range, der.bytes.to_vec(), der.len)?;
    ctx.constrain_equal(&original.actual_len(), &canonical.actual_len());
    for (raw, encoded) in original.bytes().iter().zip(canonical.bytes()) {
        let difference = range
            .gate()
            .sub(ctx, raw.quantum_cell(), encoded.quantum_cell());
        range.gate().assert_is_const(ctx, &difference, &F::ZERO);
    }
    let digest = hash(ctx, jobs, wrapper.to_vec())?;
    let digest = digest.map(|byte| {
        byte.assigned()
            .expect("computed wrapper SHA byte is assigned")
    });
    assert_p256_ecdsa_digest::<F, 256, false>(
        &chip,
        ctx,
        signature.signature_public_key,
        signature.enrolled_public_key,
        signature.enrolled_public_key_sec1,
        signature.r,
        signature.s,
        signature.z,
        &digest,
        signature.digest_reduction_quotient,
    );
    Ok(original)
}

pub(super) fn constrain_original_android_approval_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    raw_der: &[u8],
    wrapper: &[PastaSha256ByteV1<F>; A::TOTAL_BYTES],
    signature: &OrdinaryPlatformSignatureCellsV1<'_, F>,
) -> Result<(), String> {
    constrain_original_android_approval_stream_v1(builder, jobs, raw_der, wrapper, signature)
        .map(|_| ())
}

/// Verify actual original App Attest CBOR/DER/authenticatorData under the independently retained
/// RP hash and counter floor. The financial logical indexes remain solely in the signed S/State.
#[allow(clippy::too_many_arguments)]
pub(super) fn constrain_original_apple_approval_stream_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    raw_assertion: &[u8],
    wrapper: &[AssignedValue<F>; A::TOTAL_BYTES],
    authenticator_data: &[AssignedValue<F>; 37],
    governed_rp_hash: &[AssignedValue<F>; 32],
    expected_release_digest: [u8; 32],
    governed_release_digest: &[AssignedValue<F>; 32],
    retained_counter_floor: AssignedValue<F>,
    accepted_counter: AssignedValue<F>,
    signature: &OrdinaryPlatformSignatureCellsV1<'_, F>,
) -> Result<KagemushaBoundedByteStreamV1<F>, String> {
    constrain_original_apple_signed_message_stream_v1(
        builder,
        jobs,
        raw_assertion,
        wrapper,
        Some(wrapper),
        authenticator_data,
        governed_rp_hash,
        expected_release_digest,
        governed_release_digest,
        retained_counter_floor,
        accepted_counter,
        signature,
    )
}

/// Reuse the same full original CBOR/DER/RP/release/counter and platform equation for a
/// separate model-owned message. Only the operation-wrapper caller supplies its closed
/// wrapper shape; receiver requests retain their own exact model framing instead.
#[allow(clippy::too_many_arguments)]
pub(super) fn constrain_original_apple_signed_message_stream_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    raw_assertion: &[u8],
    wrapper: &[AssignedValue<F>],
    approval_wrapper: Option<&[AssignedValue<F>; A::TOTAL_BYTES]>,
    authenticator_data: &[AssignedValue<F>; 37],
    governed_rp_hash: &[AssignedValue<F>; 32],
    expected_release_digest: [u8; 32],
    governed_release_digest: &[AssignedValue<F>; 32],
    retained_counter_floor: AssignedValue<F>,
    accepted_counter: AssignedValue<F>,
    signature: &OrdinaryPlatformSignatureCellsV1<'_, F>,
) -> Result<KagemushaBoundedByteStreamV1<F>, String> {
    let range = builder.range_chip();
    let chip = FpChip::<F, P256Base>::new(&range, P256_LIMB_BITS, P256_NUM_LIMBS);
    let der = constrain_p256_canonical_der_v1(&chip, builder.main(0), signature.r, signature.s);
    let streams = original_apple_cbor::constrain_original_apple_assertion_stream_v1(
        builder,
        jobs,
        raw_assertion,
        authenticator_data,
        expected_release_digest,
        governed_release_digest,
        &der,
    )?;
    let ctx = builder.main(0);
    let gate = range.gate();
    if let Some(wrapper) = approval_wrapper {
        for (cell, byte) in wrapper.iter().zip(
            KAGEMUSHA_APP_OPERATION_APPROVAL_DOMAIN_V1
                .iter()
                .copied()
                .chain((A::BODY.len() as u64).to_le_bytes()),
        ) {
            gate.assert_is_const(ctx, cell, &F::from(u64::from(byte)));
        }
        for (cell, byte) in wrapper[A::VERSION].iter().zip([1_u8, 0]) {
            gate.assert_is_const(ctx, cell, &F::from(u64::from(byte)));
        }
        // The whole wrapper admits the two distinct Native purposes. The actual State consumer
        // requires PrepareTransition for its pre-candidate relation; the terminal money consumer
        // separately requires MonetaryTransition and its exact candidate/body.
        let purpose = wrapper[A::PURPOSE.start];
        let first = gate.sub(ctx, purpose, halo2_base::QuantumCell::Constant(F::ONE));
        let second = gate.sub(ctx, purpose, halo2_base::QuantumCell::Constant(F::from(2)));
        let invalid = gate.mul(ctx, first, second);
        gate.assert_is_const(ctx, &invalid, &F::ZERO);
    }
    let mut rp = Vec::with_capacity(32);
    for (actual, expected) in authenticator_data[..32].iter().zip(governed_rp_hash) {
        range.range_check(ctx, *expected, 8);
        ctx.constrain_equal(actual, expected);
        rp.push(*expected);
    }
    let rp_sum = gate.sum(ctx, rp);
    let rp_empty = gate.is_zero(ctx, rp_sum);
    gate.assert_is_const(ctx, &rp_empty, &F::ZERO);
    range.range_check(ctx, retained_counter_floor, 32);
    range.range_check(ctx, accepted_counter, 32);
    let advanced = range.is_less_than(ctx, retained_counter_floor, accepted_counter, 32);
    gate.assert_is_const(ctx, &advanced, &F::ONE);
    let counter = gate.inner_product(
        ctx,
        authenticator_data[33..37].iter().copied(),
        [24, 16, 8, 0].map(|bit| halo2_base::QuantumCell::Constant(F::from(1_u64 << bit))),
    );
    ctx.constrain_equal(&counter, &accepted_counter);
    let wrapper_bytes = wrapper
        .iter()
        .copied()
        .map(|b| PastaSha256ByteV1::range_checked(ctx, &range, b))
        .collect();
    let client_hash = hash(ctx, jobs, wrapper_bytes)?;
    let client_len = ctx.load_constant(F::from(32_u64));
    let client_stream =
        KagemushaBoundedByteStreamV1::constrain(ctx, &range, client_hash.to_vec(), client_len)?;
    let nonce_stream = streams
        .authenticator
        .concat(ctx, &range, &client_stream, 206 + 32)?;
    let nonce = original_apple_cbor::bounded_hash(ctx, &range, jobs, &nonce_stream)?;
    // The platform API signs nonce as an ECDSA-SHA256 message; the native equation hashes it.
    let digest = hash(ctx, jobs, nonce.to_vec())?
        .map(|b| b.assigned().expect("Apple final digest byte assigned"));
    assert_p256_ecdsa_digest::<F, 256, false>(
        &chip,
        ctx,
        signature.signature_public_key,
        signature.enrolled_public_key,
        signature.enrolled_public_key_sec1,
        signature.r,
        signature.s,
        signature.z,
        &digest,
        signature.digest_reduction_quotient,
    );
    Ok(streams.original)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn constrain_original_apple_approval_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    raw_assertion: &[u8],
    wrapper: &[AssignedValue<F>; A::TOTAL_BYTES],
    authenticator_data: &[AssignedValue<F>; 37],
    governed_rp_hash: &[AssignedValue<F>; 32],
    expected_release_digest: [u8; 32],
    governed_release_digest: &[AssignedValue<F>; 32],
    retained_counter_floor: AssignedValue<F>,
    accepted_counter: AssignedValue<F>,
    signature: &OrdinaryPlatformSignatureCellsV1<'_, F>,
) -> Result<(), String> {
    constrain_original_apple_approval_stream_v1(
        builder,
        jobs,
        raw_assertion,
        wrapper,
        authenticator_data,
        governed_rp_hash,
        expected_release_digest,
        governed_release_digest,
        retained_counter_floor,
        accepted_counter,
        signature,
    )
    .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use halo2_proofs::halo2curves::pasta::{Fp, Fq};

    #[test]
    fn original_platform_equations_have_full_width_both_parity_entry_points() {
        // Compile/type-bound evidence only. Genuine equation/mutation MockProver tests live in
        // the shared P-256, DER, CBOR and wrapper modules; this creates no credential or owner.
        let _ = constrain_original_android_approval_v1::<Fp>;
        let _ = constrain_original_android_approval_v1::<Fq>;
        let _ = constrain_original_apple_approval_v1::<Fp>;
        let _ = constrain_original_apple_approval_v1::<Fq>;
    }
}

#[cfg(test)]
#[path = "ordinary_signed_message_equation_tests.rs"]
mod signed_message_tests;
