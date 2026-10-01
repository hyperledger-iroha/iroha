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
use iroha_data_model::kagemusha::KagemushaAppOperationApprovalSigningLayoutV1 as A;

use crate::{
    kagemusha_p256_curve_gadget::{
        P256_LIMB_BITS, P256_NUM_LIMBS, app_attest_der_gadget::constrain_p256_canonical_der_v1,
        assert_apple_app_operation_approval_ecdsa, assert_p256_ecdsa_digest,
    },
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    pasta_sha256::{PastaSha256ByteV1, PastaSha256JobsV1},
};

#[path = "app_attest_assertion_cbor.rs"]
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
    retained_counter_floor: AssignedValue<F>,
    accepted_counter: AssignedValue<F>,
    signature: &OrdinaryPlatformSignatureCellsV1<'_, F>,
) -> Result<KagemushaBoundedByteStreamV1<F>, String> {
    let range = builder.range_chip();
    let chip = FpChip::<F, P256Base>::new(&range, P256_LIMB_BITS, P256_NUM_LIMBS);
    let der = constrain_p256_canonical_der_v1(&chip, builder.main(0), signature.r, signature.s);
    let original = original_apple_cbor::constrain_original_apple_assertion_stream_37_v1(
        builder,
        raw_assertion,
        authenticator_data,
        &der,
    )?;
    assert_apple_app_operation_approval_ecdsa::<F, 256>(
        &chip,
        builder.main(0),
        jobs,
        wrapper,
        authenticator_data,
        governed_rp_hash,
        retained_counter_floor,
        accepted_counter,
        signature.signature_public_key,
        signature.enrolled_public_key,
        signature.enrolled_public_key_sec1,
        signature.r,
        signature.s,
        signature.z,
        signature.digest_reduction_quotient,
    )?;
    Ok(original)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn constrain_original_apple_approval_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    raw_assertion: &[u8],
    wrapper: &[AssignedValue<F>; A::TOTAL_BYTES],
    authenticator_data: &[AssignedValue<F>; 37],
    governed_rp_hash: &[AssignedValue<F>; 32],
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
