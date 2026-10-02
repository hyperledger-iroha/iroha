//! Complete carrier commitments and canonical two-field binding.
//!
//! Protocol-owned topology supplies a domain, version, fixed carrier width and semantic prefix.
//! Every challenge follows all four authentic instance commitments. The complete ordered u128
//! polynomial retains remainder and ternary quotient information; it never aliases zero and M.
//! These helpers alone confer no proof authority: native and recursive consumers must check the
//! exact hybrid proof-supplied commitments and decide its carried accumulator.

use super::{
    KAGEMUSHA_RECURSION_IPA_K_V1, KagemushaPastaParityV1, carrier_rlc::*,
    deferred_parent::DeferredLoader,
};
use crate::kagemusha_v1_poseidon::{
    KagemushaPoseidonChipV1, KagemushaPoseidonFieldV1, encode, hash,
};
use ff::{Field as _, PrimeField as _};
use halo2_base::{
    AssignedValue,
    QuantumCell::{Constant, Existing},
    gates::{GateInstructions as _, RangeInstructions as _},
    utils::{BigPrimeField, CurveAffineExt},
};
use halo2_proofs::{
    arithmetic::best_multiexp,
    halo2curves::{
        CurveAffine,
        group::{Curve as _, prime::PrimeCurveAffine as _},
        pasta::{EpAffine, EqAffine, Fp, Fq},
    },
    poly::{
        commitment::{Params as _, ParamsProver as _},
        ipa::commitment::ParamsIPA,
    },
};
use p256::elliptic_curve::bigint::{Encoding as _, NonZero, U256};

/// Closed protocol dimensions, supplied only by the owning circuit implementation.
#[derive(Clone, Copy)]
pub(super) struct KagemushaCarrierBindingLayoutV1 {
    pub(super) domain: u64,
    pub(super) version: u64,
    pub(super) capacity: usize,
    pub(super) semantic_prefix: usize,
}

#[derive(Clone, Copy)]
pub(super) struct KagemushaCarrierCommitmentsV1 {
    pub(super) eq_proof_eq_carrier: EqAffine,
    pub(super) eq_proof_ep_carrier: EqAffine,
    pub(super) ep_proof_eq_carrier: EpAffine,
    pub(super) ep_proof_ep_carrier: EpAffine,
}

#[derive(Clone, Copy)]
pub(super) struct KagemushaCarrierBindingV1 {
    pub(super) commitments: KagemushaCarrierCommitmentsV1,
    pub(super) eq_challenge: u128,
    pub(super) ep_challenge: u128,
    pub(super) eq_at_eq_challenge: u128,
    pub(super) eq_at_ep_challenge: u128,
    pub(super) ep_at_eq_challenge: u128,
    pub(super) ep_at_ep_challenge: u128,
}

pub(super) fn placeholder_carrier_binding_v1() -> KagemushaCarrierBindingV1 {
    KagemushaCarrierBindingV1 {
        commitments: KagemushaCarrierCommitmentsV1 {
            eq_proof_eq_carrier: EqAffine::generator(),
            eq_proof_ep_carrier: EqAffine::generator(),
            ep_proof_eq_carrier: EpAffine::generator(),
            ep_proof_ep_carrier: EpAffine::generator(),
        },
        eq_challenge: 1,
        ep_challenge: 1,
        eq_at_eq_challenge: 0,
        eq_at_ep_challenge: 0,
        ep_at_eq_challenge: 0,
        ep_at_ep_challenge: 0,
    }
}

pub(super) fn canonical_carrier_commitment_v1<C>(
    parameters: &ParamsIPA<C>,
    values: &[u128],
    layout: KagemushaCarrierBindingLayoutV1,
) -> Result<C, String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    if values.len() != layout.capacity {
        return Err("mint-hash claim padded carrier has the wrong shape".to_owned());
    }
    let bases = parameters
        .get_g_lagrange()
        .get(..values.len())
        .ok_or_else(|| "mint-hash claim carrier exceeds the IPA domain".to_owned())?;
    let scalars = values
        .iter()
        .copied()
        .map(C::ScalarExt::from_u128)
        .collect::<Vec<_>>();
    let commitment =
        (best_multiexp::<C>(&scalars, bases) + parameters.get_blind_base().to_curve()).to_affine();
    if bool::from(commitment.is_identity()) {
        return Err("mint-hash claim carrier commitment is the identity".to_owned());
    }
    Ok(commitment)
}

pub(super) fn point_u128_limbs_v1<C: CurveAffine>(point: C) -> [u128; 2] {
    let bytes = point.to_bytes();
    let bytes = bytes.as_ref();
    std::array::from_fn(|half| {
        u128::from_le_bytes(
            bytes[half * 16..(half + 1) * 16]
                .try_into()
                .expect("Pasta compressed point half has sixteen bytes"),
        )
    })
}

pub(super) fn carrier_commitment_limbs_v1(commitments: KagemushaCarrierCommitmentsV1) -> [u128; 8] {
    let [eq_eq_0, eq_eq_1] = point_u128_limbs_v1(commitments.eq_proof_eq_carrier);
    let [eq_ep_0, eq_ep_1] = point_u128_limbs_v1(commitments.eq_proof_ep_carrier);
    let [ep_eq_0, ep_eq_1] = point_u128_limbs_v1(commitments.ep_proof_eq_carrier);
    let [ep_ep_0, ep_ep_1] = point_u128_limbs_v1(commitments.ep_proof_ep_carrier);
    [
        eq_eq_0, eq_eq_1, eq_ep_0, eq_ep_1, ep_eq_0, ep_eq_1, ep_ep_0, ep_ep_1,
    ]
}

pub(super) fn carrier_binding_values_v1(binding: KagemushaCarrierBindingV1) -> [u128; 14] {
    let commitments = carrier_commitment_limbs_v1(binding.commitments);
    [
        commitments[0],
        commitments[1],
        commitments[2],
        commitments[3],
        commitments[4],
        commitments[5],
        commitments[6],
        commitments[7],
        binding.eq_challenge,
        binding.ep_challenge,
        binding.eq_at_eq_challenge,
        binding.eq_at_ep_challenge,
        binding.ep_at_eq_challenge,
        binding.ep_at_ep_challenge,
    ]
}

pub(super) fn native_carrier_challenge_v1<F: KagemushaPoseidonFieldV1>(
    commitments: KagemushaCarrierCommitmentsV1,
    parity: KagemushaPastaParityV1,
    layout: KagemushaCarrierBindingLayoutV1,
) -> u128 {
    let mut inputs = Vec::with_capacity(13);
    inputs.extend([
        F::from(layout.version),
        F::from(match parity {
            KagemushaPastaParityV1::Eq => 1,
            KagemushaPastaParityV1::Ep => 2,
        }),
        F::from(u64::from(KAGEMUSHA_RECURSION_IPA_K_V1)),
        F::from(u64::try_from(layout.capacity).expect("fixed carrier length fits u64")),
        F::from(2),
    ]);
    inputs.extend(
        carrier_commitment_limbs_v1(commitments)
            .into_iter()
            .map(F::from_u128),
    );
    let digest = encode(hash::<F>(layout.domain, &inputs));
    let low = u128::from_le_bytes(
        digest[..16]
            .try_into()
            .expect("Pasta scalar low half has sixteen bytes"),
    );
    (low & ((1_u128 << CARRIER_RLC_CHALLENGE_BITS_V1) - 1)) + 1
}

pub(super) fn native_carrier_rlc_v1(
    values: &[u128],
    challenge: u128,
    layout: KagemushaCarrierBindingLayoutV1,
) -> Result<u128, String> {
    if values.len() != layout.capacity
        || challenge == 0
        || challenge > (1_u128 << CARRIER_RLC_CHALLENGE_BITS_V1)
    {
        return Err("mint-hash carrier RLC input shape is invalid".to_owned());
    }
    let modulus = U256::from_u128(CARRIER_RLC_MODULUS_V1);
    let divisor = Option::<NonZero<U256>>::from(NonZero::new(modulus))
        .expect("fixed Mersenne RLC modulus is nonzero");
    let challenge = U256::from_u128(challenge);
    let mut accumulator = U256::ZERO;
    for coefficient in native_carrier_coefficients_v1(values, layout)? {
        let product = accumulator.wrapping_mul(&challenge);
        let numerator = product.wrapping_add(&U256::from_u128(coefficient));
        accumulator = numerator.div_rem(&divisor).1;
    }
    let bytes: [u8; 32] = accumulator.to_le_bytes();
    if bytes[16..].iter().any(|byte| *byte != 0) {
        return Err("mint-hash carrier RLC result exceeds u128".to_owned());
    }
    Ok(u128::from_le_bytes(
        bytes[..16]
            .try_into()
            .expect("RLC low half has sixteen bytes"),
    ))
}

pub(super) fn native_carrier_coefficients_v1(
    values: &[u128],
    layout: KagemushaCarrierBindingLayoutV1,
) -> Result<Vec<u128>, String> {
    if values.len() != layout.capacity {
        return Err("mint-hash carrier coefficient input shape is invalid".to_owned());
    }
    let mut coefficients = Vec::with_capacity(
        values.len()
            + values
                .len()
                .div_ceil(CARRIER_RLC_QUOTIENTS_PER_COEFFICIENT_V1),
    );
    let mut quotients = Vec::with_capacity(values.len());
    for value in values.iter().copied() {
        coefficients.push(value % CARRIER_RLC_MODULUS_V1);
        let quotient = value / CARRIER_RLC_MODULUS_V1;
        if quotient >= CARRIER_RLC_QUOTIENT_RADIX_V1 {
            return Err("mint-hash carrier quotient is not a ternary digit".to_owned());
        }
        quotients.push(quotient);
    }
    for chunk in quotients.chunks(CARRIER_RLC_QUOTIENTS_PER_COEFFICIENT_V1) {
        let mut packed = 0_u128;
        let mut power = 1_u128;
        for (index, quotient) in chunk.iter().copied().enumerate() {
            packed = packed
                .checked_add(
                    quotient
                        .checked_mul(power)
                        .ok_or_else(|| "mint-hash quotient pack overflowed".to_owned())?,
                )
                .ok_or_else(|| "mint-hash quotient pack overflowed".to_owned())?;
            if index + 1 != chunk.len() {
                power = power
                    .checked_mul(CARRIER_RLC_QUOTIENT_RADIX_V1)
                    .ok_or_else(|| "mint-hash quotient radix overflowed".to_owned())?;
            }
        }
        if packed >= CARRIER_RLC_MODULUS_V1 {
            return Err("mint-hash quotient pack exceeds the RLC modulus".to_owned());
        }
        coefficients.push(packed);
    }
    Ok(coefficients)
}

pub(super) fn derive_carrier_binding_v1(
    eq_parameters: &ParamsIPA<EqAffine>,
    ep_parameters: &ParamsIPA<EpAffine>,
    eq_carrier: &[u128],
    ep_carrier: &[u128],
    layout: KagemushaCarrierBindingLayoutV1,
) -> Result<KagemushaCarrierBindingV1, String> {
    let commitments = KagemushaCarrierCommitmentsV1 {
        eq_proof_eq_carrier: canonical_carrier_commitment_v1(eq_parameters, eq_carrier, layout)?,
        eq_proof_ep_carrier: canonical_carrier_commitment_v1(eq_parameters, ep_carrier, layout)?,
        ep_proof_eq_carrier: canonical_carrier_commitment_v1(ep_parameters, eq_carrier, layout)?,
        ep_proof_ep_carrier: canonical_carrier_commitment_v1(ep_parameters, ep_carrier, layout)?,
    };
    let eq_challenge =
        native_carrier_challenge_v1::<Fp>(commitments, KagemushaPastaParityV1::Eq, layout);
    let ep_challenge =
        native_carrier_challenge_v1::<Fq>(commitments, KagemushaPastaParityV1::Ep, layout);
    Ok(KagemushaCarrierBindingV1 {
        commitments,
        eq_challenge,
        ep_challenge,
        eq_at_eq_challenge: native_carrier_rlc_v1(eq_carrier, eq_challenge, layout)?,
        eq_at_ep_challenge: native_carrier_rlc_v1(eq_carrier, ep_challenge, layout)?,
        ep_at_eq_challenge: native_carrier_rlc_v1(ep_carrier, eq_challenge, layout)?,
        ep_at_ep_challenge: native_carrier_rlc_v1(ep_carrier, ep_challenge, layout)?,
    })
}

pub(super) fn constrain_carrier_challenge_v1<C>(
    loader: &DeferredLoader<'_, C>,
    public: &[AssignedValue<C::ScalarExt>],
    parity: KagemushaPastaParityV1,
    layout: KagemushaCarrierBindingLayoutV1,
) -> Result<AssignedValue<C::ScalarExt>, String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    let commitment_cells = public
        .get(layout.semantic_prefix..layout.semantic_prefix + 8)
        .ok_or_else(|| "mint-hash carrier commitment binding is truncated".to_owned())?;
    if commitment_cells.len() != 8 {
        return Err("mint-hash carrier commitment binding shape drifted".to_owned());
    }
    let chip = loader.ecc_chip();
    let range = chip.range();
    let mut ctx = loader.ctx_mut();
    let mut inputs = Vec::with_capacity(13);
    inputs.extend([
        ctx.main().load_constant(C::ScalarExt::from(layout.version)),
        ctx.main().load_constant(C::ScalarExt::from(match parity {
            KagemushaPastaParityV1::Eq => 1,
            KagemushaPastaParityV1::Ep => 2,
        })),
        ctx.main()
            .load_constant(C::ScalarExt::from(u64::from(KAGEMUSHA_RECURSION_IPA_K_V1))),
        ctx.main().load_constant(C::ScalarExt::from(
            u64::try_from(layout.capacity).expect("fixed carrier length fits u64"),
        )),
        ctx.main().load_constant(C::ScalarExt::from(2)),
    ]);
    inputs.extend_from_slice(commitment_cells);
    let poseidon = KagemushaPoseidonChipV1::new(ctx.main(), range);
    let digest = poseidon.hash(ctx.main(), range, layout.domain, &inputs);
    let low_limb = chip.assigned_scalar_u128_limbs(&mut ctx, digest)[0];
    let (_, low_125) = range.div_mod(
        ctx.main(),
        Existing(low_limb),
        1_u128 << CARRIER_RLC_CHALLENGE_BITS_V1,
        128,
    );
    let challenge = range
        .gate()
        .add(ctx.main(), Existing(low_125), Constant(C::ScalarExt::ONE));
    let expected_offset = match parity {
        KagemushaPastaParityV1::Eq => layout.semantic_prefix + 8,
        KagemushaPastaParityV1::Ep => layout.semantic_prefix + 9,
    };
    ctx.main()
        .constrain_equal(&challenge, &public[expected_offset]);
    Ok(challenge)
}
