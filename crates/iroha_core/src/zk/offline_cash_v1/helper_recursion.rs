//! Offline Cash V1 recursive-lineage boundary.
//!
//! Helper and final-State wrappers verify canonical ordinary Poseidon child
//! proofs in-circuit, fold their child accumulators to exactly one parity-local
//! lineage, and constrain that lineage to a dedicated public instance column.
//! The reciprocal Pasta proof enforces every deferred verifier/fold equation.
//! Terminal native verification then decides both the wrapper proof's outer
//! accumulator and the constrained carried lineage against the authenticated
//! transparent parameter generator vector. No delayed-history suffix and no
//! native-only child-proof acceptance exists in this profile.

use ff::PrimeField as _;
use halo2_proofs::{
    halo2curves::{
        CurveAffine,
        group::{GroupEncoding as _, prime::PrimeCurveAffine as _},
        pasta::{EpAffine, EqAffine, Fp, Fq},
    },
    plonk::VerifyingKey,
    poly::ipa::commitment::ParamsIPA,
};
use iroha_data_model::offline::{
    OFFLINE_CASH_IPA_LINEAGE_CHALLENGES_V1, OFFLINE_CASH_IPA_LINEAGE_INSTANCE_CELLS_V1,
    OfflineCashIpaLineageV1,
};
use snark_verifier::{loader::native::NativeLoader, pcs::ipa::IpaAccumulator};

use crate::zk::kagemusha_recursion_adapter::{
    decide_poseidon_accumulator_native_v1, verify_poseidon_child_proof_native_v1,
};

/// Failure at the curve-aware carried-lineage or terminal-decision boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OfflineCashRecursiveLineageErrorV1 {
    InvalidWireShape,
    NonCanonicalScalar,
    NonCanonicalOrIdentityPoint,
    InvalidOuterProof,
    InvalidOuterDecision,
    InvalidCarriedDecision,
}

fn encode_lineage<C>(
    accumulator: &IpaAccumulator<C, NativeLoader>,
) -> Result<OfflineCashIpaLineageV1, OfflineCashRecursiveLineageErrorV1>
where
    C: CurveAffine,
    C::ScalarExt: ff::PrimeField,
    <C::ScalarExt as ff::PrimeField>::Repr: AsRef<[u8]>,
    C::Repr: AsRef<[u8]>,
{
    if accumulator.xi.len() != OFFLINE_CASH_IPA_LINEAGE_CHALLENGES_V1
        || bool::from(accumulator.u.is_identity())
    {
        return Err(OfflineCashRecursiveLineageErrorV1::InvalidWireShape);
    }
    let mut round_challenges = [[0_u8; 32]; OFFLINE_CASH_IPA_LINEAGE_CHALLENGES_V1];
    for (target, challenge) in round_challenges.iter_mut().zip(&accumulator.xi) {
        let encoded = challenge.to_repr();
        if encoded.as_ref().len() != target.len() {
            return Err(OfflineCashRecursiveLineageErrorV1::InvalidWireShape);
        }
        target.copy_from_slice(encoded.as_ref());
    }
    let encoded_point = accumulator.u.to_bytes();
    if encoded_point.as_ref().len() != 32 {
        return Err(OfflineCashRecursiveLineageErrorV1::InvalidWireShape);
    }
    let mut folded_generator = [0_u8; 32];
    folded_generator.copy_from_slice(encoded_point.as_ref());
    let lineage = OfflineCashIpaLineageV1::new(round_challenges, folded_generator)
        .map_err(|_| OfflineCashRecursiveLineageErrorV1::InvalidWireShape)?;
    Ok(lineage)
}

fn decode_scalars<F>(
    lineage: &OfflineCashIpaLineageV1,
) -> Result<Vec<F>, OfflineCashRecursiveLineageErrorV1>
where
    F: ff::PrimeField,
    F::Repr: From<[u8; 32]>,
{
    lineage
        .validate()
        .map_err(|_| OfflineCashRecursiveLineageErrorV1::InvalidWireShape)?;
    lineage
        .round_challenges
        .chunks_exact(32)
        .map(|bytes| {
            let bytes: [u8; 32] = bytes
                .try_into()
                .expect("fixed Offline Cash scalar encoding width");
            Option::<F>::from(F::from_repr(bytes.into()))
                .ok_or(OfflineCashRecursiveLineageErrorV1::NonCanonicalScalar)
        })
        .collect()
}

/// Encode one Eq/Fp carried accumulator without reduction.
pub(super) fn offline_cash_lineage_from_eq_v1(
    accumulator: &IpaAccumulator<EqAffine, NativeLoader>,
) -> Result<OfflineCashIpaLineageV1, OfflineCashRecursiveLineageErrorV1> {
    encode_lineage(accumulator)
}

/// Encode one Ep/Fq carried accumulator without reduction.
pub(super) fn offline_cash_lineage_from_ep_v1(
    accumulator: &IpaAccumulator<EpAffine, NativeLoader>,
) -> Result<OfflineCashIpaLineageV1, OfflineCashRecursiveLineageErrorV1> {
    encode_lineage(accumulator)
}

/// Strictly parse one Eq/Fp carried accumulator.
pub(super) fn offline_cash_lineage_to_eq_v1(
    lineage: &OfflineCashIpaLineageV1,
) -> Result<IpaAccumulator<EqAffine, NativeLoader>, OfflineCashRecursiveLineageErrorV1> {
    let xi = decode_scalars::<Fp>(lineage)?;
    let u = Option::<EqAffine>::from(EqAffine::from_bytes(&lineage.folded_generator.into()))
        .ok_or(OfflineCashRecursiveLineageErrorV1::NonCanonicalOrIdentityPoint)?;
    if bool::from(u.is_identity()) {
        return Err(OfflineCashRecursiveLineageErrorV1::NonCanonicalOrIdentityPoint);
    }
    Ok(IpaAccumulator::new(xi, u))
}

/// Strictly parse one Ep/Fq carried accumulator.
pub(super) fn offline_cash_lineage_to_ep_v1(
    lineage: &OfflineCashIpaLineageV1,
) -> Result<IpaAccumulator<EpAffine, NativeLoader>, OfflineCashRecursiveLineageErrorV1> {
    let xi = decode_scalars::<Fq>(lineage)?;
    let u = Option::<EpAffine>::from(EpAffine::from_bytes(&lineage.folded_generator.into()))
        .ok_or(OfflineCashRecursiveLineageErrorV1::NonCanonicalOrIdentityPoint)?;
    if bool::from(u.is_identity()) {
        return Err(OfflineCashRecursiveLineageErrorV1::NonCanonicalOrIdentityPoint);
    }
    Ok(IpaAccumulator::new(xi, u))
}

fn lineage_instance_column<F>(
    lineage: &OfflineCashIpaLineageV1,
) -> Result<[F; OFFLINE_CASH_IPA_LINEAGE_INSTANCE_CELLS_V1], OfflineCashRecursiveLineageErrorV1>
where
    F: ff::PrimeField,
{
    let limbs = lineage
        .instance_limbs()
        .map_err(|_| OfflineCashRecursiveLineageErrorV1::InvalidWireShape)?;
    Ok(limbs.map(F::from_u128))
}

/// Exact 36-cell Eq/Fp carried-lineage column.
pub(super) fn offline_cash_eq_lineage_instance_column_v1(
    lineage: &OfflineCashIpaLineageV1,
) -> Result<[Fp; OFFLINE_CASH_IPA_LINEAGE_INSTANCE_CELLS_V1], OfflineCashRecursiveLineageErrorV1> {
    lineage_instance_column(lineage)
}

/// Exact 36-cell Ep/Fq carried-lineage column.
pub(super) fn offline_cash_ep_lineage_instance_column_v1(
    lineage: &OfflineCashIpaLineageV1,
) -> Result<[Fq; OFFLINE_CASH_IPA_LINEAGE_INSTANCE_CELLS_V1], OfflineCashRecursiveLineageErrorV1> {
    lineage_instance_column(lineage)
}

/// Verify and terminally decide a final Eq proof and its circuit-bound carried
/// lineage. `instances` must end with the exact 36-cell lineage column.
pub(super) fn terminal_verify_eq_outer_and_carried_v1(
    params: &ParamsIPA<EqAffine>,
    verifier: &VerifyingKey<EqAffine>,
    instances: &[Vec<Fp>],
    ordinary_proof: &[u8],
    max_proof_bytes: usize,
    lineage: &OfflineCashIpaLineageV1,
) -> Result<(), OfflineCashRecursiveLineageErrorV1> {
    let expected_lineage = offline_cash_eq_lineage_instance_column_v1(lineage)?;
    if instances.last().map(Vec::as_slice) != Some(expected_lineage.as_slice()) {
        return Err(OfflineCashRecursiveLineageErrorV1::InvalidWireShape);
    }
    let outer = verify_poseidon_child_proof_native_v1(
        params,
        verifier,
        instances,
        ordinary_proof,
        max_proof_bytes,
    )
    .map_err(|_| OfflineCashRecursiveLineageErrorV1::InvalidOuterProof)?;
    decide_poseidon_accumulator_native_v1(params, outer)
        .map_err(|_| OfflineCashRecursiveLineageErrorV1::InvalidOuterDecision)?;
    decide_poseidon_accumulator_native_v1(params, offline_cash_lineage_to_eq_v1(lineage)?)
        .map_err(|_| OfflineCashRecursiveLineageErrorV1::InvalidCarriedDecision)
}

/// Verify and terminally decide a final Ep proof and its circuit-bound carried
/// lineage. `instances` must end with the exact 36-cell lineage column.
pub(super) fn terminal_verify_ep_outer_and_carried_v1(
    params: &ParamsIPA<EpAffine>,
    verifier: &VerifyingKey<EpAffine>,
    instances: &[Vec<Fq>],
    ordinary_proof: &[u8],
    max_proof_bytes: usize,
    lineage: &OfflineCashIpaLineageV1,
) -> Result<(), OfflineCashRecursiveLineageErrorV1> {
    let expected_lineage = offline_cash_ep_lineage_instance_column_v1(lineage)?;
    if instances.last().map(Vec::as_slice) != Some(expected_lineage.as_slice()) {
        return Err(OfflineCashRecursiveLineageErrorV1::InvalidWireShape);
    }
    let outer = verify_poseidon_child_proof_native_v1(
        params,
        verifier,
        instances,
        ordinary_proof,
        max_proof_bytes,
    )
    .map_err(|_| OfflineCashRecursiveLineageErrorV1::InvalidOuterProof)?;
    decide_poseidon_accumulator_native_v1(params, outer)
        .map_err(|_| OfflineCashRecursiveLineageErrorV1::InvalidOuterDecision)?;
    decide_poseidon_accumulator_native_v1(params, offline_cash_lineage_to_ep_v1(lineage)?)
        .map_err(|_| OfflineCashRecursiveLineageErrorV1::InvalidCarriedDecision)
}

// Compile-time parity contract: Eq proofs/circuits use Fp; Ep use Fq. Keeping
// these aliases concrete prevents a future cross-parity child-verifier swap.
const _: fn(Fp) -> Fp = core::convert::identity;
const _: fn(Fq) -> Fq = core::convert::identity;
