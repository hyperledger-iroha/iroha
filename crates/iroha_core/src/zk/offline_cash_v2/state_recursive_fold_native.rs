//! Native verifier for the provenance-bound Offline Cash V2 STATE fold relation.
//!
//! This private child verifies exactly one six-input BGH19 accumulation
//! relation for Eq and then one for Ep. It derives the canonical k=17 succinct
//! verification keys in process, parses every canonical input without modular
//! reduction, requires exact transcript consumption, and matches each
//! verifier-derived output to the ownership-bound claimed output. The result
//! remains a move-only seal around the complete provenance carrier.
//!
//! This is deliberately not an accumulator decision, recursive circuit,
//! ordinary STATE/GuardBundle proof verifier, artifact authority, persistence
//! boundary, readiness receipt, release decision, or production backend.
#![allow(
    dead_code,
    reason = "the native relation kernel remains unreachable from production until the ordinary child verifiers and recursive backend exist"
)]

use std::{
    io::Cursor,
    panic::{AssertUnwindSafe, catch_unwind},
};

use halo2_proofs::{
    halo2curves::{
        CurveAffine, CurveExt as _,
        ff::{FromUniformBytes, PrimeField},
        group::Curve as _,
        pasta::{Ep, EpAffine, Eq, EqAffine},
    },
    transcript::{Blake2bRead, Challenge255, TranscriptReadBuffer as _},
};
use snark_verifier::{
    loader::native::NativeLoader,
    pcs::{
        AccumulationScheme,
        ipa::{Bgh19, IpaAccumulator, IpaAs, IpaSuccinctVerifyingKey},
    },
    util::arithmetic::{Domain, root_of_unity},
};

use super::state_recursive_fold::{
    CanonicalStateAccumulatorV2, OpaqueStateBgh19ProofV2,
    ProvenanceBoundStateRecursiveFoldResultV2, STATE_RECURSIVE_FOLD_ACCUMULATOR_BYTES_V2,
    STATE_RECURSIVE_FOLD_ARTIFACTS_AUTHENTICATED_V2, STATE_RECURSIVE_FOLD_BACKEND_AVAILABLE_V2,
    STATE_RECURSIVE_FOLD_ECC_STRATEGY_GOVERNED_V2, STATE_RECURSIVE_FOLD_INPUT_ORDER_V2,
    STATE_RECURSIVE_FOLD_INPUTS_PER_PARITY_V2, STATE_RECURSIVE_FOLD_K_V2,
    STATE_RECURSIVE_FOLD_PRODUCTION_AVAILABLE_V2, STATE_RECURSIVE_FOLD_READINESS_AVAILABLE_V2,
    STATE_RECURSIVE_FOLD_RELEASE_ELIGIBLE_V2, StateRecursiveFoldParityV2,
};

/// The private native six-input Eq-then-Ep relation kernel is implemented.
///
/// This is an implementation fact only. It grants no child-proof, recursive
/// circuit, accumulator-decision, backend, readiness, or release authority.
pub(super) const STATE_RECURSIVE_FOLD_NATIVE_RELATION_KERNEL_IMPLEMENTED_V2: bool = true;

const HALO2_PARAMETERS_DOMAIN_V2: &str = "Halo2-Parameters";
const HALO2_G0_MESSAGE_V2: [u8; 5] = [0; 5];

const _: () = {
    assert!(STATE_RECURSIVE_FOLD_NATIVE_RELATION_KERNEL_IMPLEMENTED_V2);
    assert!(STATE_RECURSIVE_FOLD_K_V2 == 17);
    assert!(!STATE_RECURSIVE_FOLD_ECC_STRATEGY_GOVERNED_V2);
    assert!(!STATE_RECURSIVE_FOLD_ARTIFACTS_AUTHENTICATED_V2);
    assert!(!STATE_RECURSIVE_FOLD_BACKEND_AVAILABLE_V2);
    assert!(!STATE_RECURSIVE_FOLD_READINESS_AVAILABLE_V2);
    assert!(!STATE_RECURSIVE_FOLD_RELEASE_ELIGIBLE_V2);
    assert!(!STATE_RECURSIVE_FOLD_PRODUCTION_AVAILABLE_V2);
};

/// Stage at which one parity-local native relation was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StateRecursiveFoldNativeRelationStageV2 {
    InputDecode,
    TranscriptParse,
    TranscriptConsumption,
    RelationVerification,
    DerivedOutputEncoding,
    ClaimedOutputMatch,
}

/// Fail-closed native relation error. No library error or partial seal escapes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct StateRecursiveFoldNativeRelationErrorV2 {
    parity: StateRecursiveFoldParityV2,
    stage: StateRecursiveFoldNativeRelationStageV2,
    input_index: Option<usize>,
    panic_contained: bool,
}

impl StateRecursiveFoldNativeRelationErrorV2 {
    const fn rejected(
        parity: StateRecursiveFoldParityV2,
        stage: StateRecursiveFoldNativeRelationStageV2,
    ) -> Self {
        Self {
            parity,
            stage,
            input_index: None,
            panic_contained: false,
        }
    }

    const fn input(parity: StateRecursiveFoldParityV2, input_index: usize) -> Self {
        Self {
            parity,
            stage: StateRecursiveFoldNativeRelationStageV2::InputDecode,
            input_index: Some(input_index),
            panic_contained: false,
        }
    }

    const fn panic(
        parity: StateRecursiveFoldParityV2,
        stage: StateRecursiveFoldNativeRelationStageV2,
    ) -> Self {
        Self {
            parity,
            stage,
            input_index: None,
            panic_contained: true,
        }
    }

    pub(super) const fn parity(&self) -> StateRecursiveFoldParityV2 {
        self.parity
    }

    pub(super) const fn stage(&self) -> StateRecursiveFoldNativeRelationStageV2 {
        self.stage
    }

    pub(super) const fn input_index(&self) -> Option<usize> {
        self.input_index
    }

    pub(super) const fn panic_was_contained(&self) -> bool {
        self.panic_contained
    }
}

impl core::fmt::Display for StateRecursiveFoldNativeRelationErrorV2 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            formatter,
            "offline-cash V2 {:?} native fold relation rejected at {:?}",
            self.parity, self.stage
        )
    }
}

impl std::error::Error for StateRecursiveFoldNativeRelationErrorV2 {}

/// Move-only proof that both ownership-bound native fold relations verified.
///
/// The complete candidate remains retained. Only borrowed views of the exact
/// outputs are exposed, and this type grants no accumulator-decision or runtime
/// authority.
pub(super) struct StateRecursiveFoldNativeRelationSealV2 {
    candidate: ProvenanceBoundStateRecursiveFoldResultV2,
}

impl StateRecursiveFoldNativeRelationSealV2 {
    pub(super) const fn eq_output(&self) -> &CanonicalStateAccumulatorV2 {
        self.candidate.result().eq_claimed_output()
    }

    pub(super) const fn ep_output(&self) -> &CanonicalStateAccumulatorV2 {
        self.candidate.result().ep_claimed_output()
    }
}

fn eq_succinct_verifying_key_v2() -> IpaSuccinctVerifyingKey<EqAffine> {
    let hash_to_curve = Eq::hash_to_curve(HALO2_PARAMETERS_DOMAIN_V2);
    IpaSuccinctVerifyingKey::new(
        Domain::new(
            STATE_RECURSIVE_FOLD_K_V2 as usize,
            root_of_unity(STATE_RECURSIVE_FOLD_K_V2 as usize),
        ),
        hash_to_curve(&HALO2_G0_MESSAGE_V2).to_affine(),
        hash_to_curve(&[2]).to_affine(),
        Some(hash_to_curve(&[1]).to_affine()),
    )
}

fn ep_succinct_verifying_key_v2() -> IpaSuccinctVerifyingKey<EpAffine> {
    let hash_to_curve = Ep::hash_to_curve(HALO2_PARAMETERS_DOMAIN_V2);
    IpaSuccinctVerifyingKey::new(
        Domain::new(
            STATE_RECURSIVE_FOLD_K_V2 as usize,
            root_of_unity(STATE_RECURSIVE_FOLD_K_V2 as usize),
        ),
        hash_to_curve(&HALO2_G0_MESSAGE_V2).to_affine(),
        hash_to_curve(&[2]).to_affine(),
        Some(hash_to_curve(&[1]).to_affine()),
    )
}

fn decode_accumulator_v2<C>(
    accumulator: &CanonicalStateAccumulatorV2,
) -> Option<IpaAccumulator<C, NativeLoader>>
where
    C: CurveAffine,
    C::Scalar: PrimeField,
{
    let bytes = accumulator.as_bytes();
    let mut xi = Vec::with_capacity(STATE_RECURSIVE_FOLD_K_V2 as usize);
    for scalar_bytes in bytes[..STATE_RECURSIVE_FOLD_K_V2 as usize * 32].chunks_exact(32) {
        let mut repr = <C::Scalar as PrimeField>::Repr::default();
        if repr.as_ref().len() != 32 {
            return None;
        }
        repr.as_mut().copy_from_slice(scalar_bytes);
        xi.push(Option::<C::Scalar>::from(C::Scalar::from_repr(repr))?);
    }
    if xi.len() != STATE_RECURSIVE_FOLD_K_V2 as usize {
        return None;
    }

    let mut point_repr = C::Repr::default();
    if point_repr.as_ref().len() != 32 {
        return None;
    }
    point_repr
        .as_mut()
        .copy_from_slice(&bytes[STATE_RECURSIVE_FOLD_K_V2 as usize * 32..]);
    let point = Option::<C>::from(C::from_bytes(&point_repr))?;
    if bool::from(point.is_identity()) || point.to_bytes().as_ref() != point_repr.as_ref() {
        return None;
    }
    Some(IpaAccumulator::new(xi, point))
}

fn encode_accumulator_v2<C>(
    accumulator: &IpaAccumulator<C, NativeLoader>,
) -> Option<[u8; STATE_RECURSIVE_FOLD_ACCUMULATOR_BYTES_V2]>
where
    C: CurveAffine,
    C::Scalar: PrimeField,
{
    if accumulator.xi.len() != STATE_RECURSIVE_FOLD_K_V2 as usize {
        return None;
    }
    let mut bytes = [0_u8; STATE_RECURSIVE_FOLD_ACCUMULATOR_BYTES_V2];
    for (destination, scalar) in bytes[..STATE_RECURSIVE_FOLD_K_V2 as usize * 32]
        .chunks_exact_mut(32)
        .zip(&accumulator.xi)
    {
        let repr = scalar.to_repr();
        if repr.as_ref().len() != 32 {
            return None;
        }
        destination.copy_from_slice(repr.as_ref());
    }
    let point = accumulator.u.to_bytes();
    if point.as_ref().len() != 32 {
        return None;
    }
    bytes[STATE_RECURSIVE_FOLD_K_V2 as usize * 32..].copy_from_slice(point.as_ref());
    Some(bytes)
}

fn verify_parity_relation_v2<C, MOS>(
    parity: StateRecursiveFoldParityV2,
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    inputs: [&CanonicalStateAccumulatorV2; STATE_RECURSIVE_FOLD_INPUTS_PER_PARITY_V2],
    proof: &OpaqueStateBgh19ProofV2,
    claimed_output: &CanonicalStateAccumulatorV2,
) -> Result<(), StateRecursiveFoldNativeRelationErrorV2>
where
    C: CurveAffine,
    C::Scalar: PrimeField + FromUniformBytes<64>,
    MOS: Clone + core::fmt::Debug,
{
    if claimed_output.parity() != parity {
        return Err(StateRecursiveFoldNativeRelationErrorV2::rejected(
            parity,
            StateRecursiveFoldNativeRelationStageV2::ClaimedOutputMatch,
        ));
    }
    let inputs = inputs
        .into_iter()
        .enumerate()
        .map(|(index, input)| {
            if input.parity() != parity {
                return Err(StateRecursiveFoldNativeRelationErrorV2::input(
                    parity, index,
                ));
            }
            decode_accumulator_v2::<C>(input)
                .ok_or_else(|| StateRecursiveFoldNativeRelationErrorV2::input(parity, index))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut cursor = Cursor::new(proof.as_bytes().as_slice());
    let parsed = {
        let mut transcript = Blake2bRead::<_, C, Challenge255<C>>::init(&mut cursor);
        catch_unwind(AssertUnwindSafe(|| {
            <IpaAs<C, MOS> as AccumulationScheme<C, NativeLoader>>::read_proof(
                succinct_vk,
                &inputs,
                &mut transcript,
            )
        }))
        .map_err(|_| {
            StateRecursiveFoldNativeRelationErrorV2::panic(
                parity,
                StateRecursiveFoldNativeRelationStageV2::TranscriptParse,
            )
        })?
        .map_err(|_| {
            StateRecursiveFoldNativeRelationErrorV2::rejected(
                parity,
                StateRecursiveFoldNativeRelationStageV2::TranscriptParse,
            )
        })?
    };
    if cursor.position() != proof.as_bytes().len() as u64 {
        return Err(StateRecursiveFoldNativeRelationErrorV2::rejected(
            parity,
            StateRecursiveFoldNativeRelationStageV2::TranscriptConsumption,
        ));
    }

    let output = catch_unwind(AssertUnwindSafe(|| {
        <IpaAs<C, MOS> as AccumulationScheme<C, NativeLoader>>::verify(
            succinct_vk,
            &inputs,
            &parsed,
        )
    }))
    .map_err(|_| {
        StateRecursiveFoldNativeRelationErrorV2::panic(
            parity,
            StateRecursiveFoldNativeRelationStageV2::RelationVerification,
        )
    })?
    .map_err(|_| {
        StateRecursiveFoldNativeRelationErrorV2::rejected(
            parity,
            StateRecursiveFoldNativeRelationStageV2::RelationVerification,
        )
    })?;
    let output = encode_accumulator_v2(&output).ok_or_else(|| {
        StateRecursiveFoldNativeRelationErrorV2::rejected(
            parity,
            StateRecursiveFoldNativeRelationStageV2::DerivedOutputEncoding,
        )
    })?;
    if output != *claimed_output.as_bytes() {
        return Err(StateRecursiveFoldNativeRelationErrorV2::rejected(
            parity,
            StateRecursiveFoldNativeRelationStageV2::ClaimedOutputMatch,
        ));
    }
    Ok(())
}

fn verify_relation_pair_v2(
    eq_inputs: [&CanonicalStateAccumulatorV2; STATE_RECURSIVE_FOLD_INPUTS_PER_PARITY_V2],
    eq_proof: &OpaqueStateBgh19ProofV2,
    eq_claimed_output: &CanonicalStateAccumulatorV2,
    ep_inputs: [&CanonicalStateAccumulatorV2; STATE_RECURSIVE_FOLD_INPUTS_PER_PARITY_V2],
    ep_proof: &OpaqueStateBgh19ProofV2,
    ep_claimed_output: &CanonicalStateAccumulatorV2,
) -> Result<(), StateRecursiveFoldNativeRelationErrorV2> {
    verify_parity_relation_v2::<EqAffine, Bgh19>(
        StateRecursiveFoldParityV2::Eq,
        &eq_succinct_verifying_key_v2(),
        eq_inputs,
        eq_proof,
        eq_claimed_output,
    )?;
    verify_parity_relation_v2::<EpAffine, Bgh19>(
        StateRecursiveFoldParityV2::Ep,
        &ep_succinct_verifying_key_v2(),
        ep_inputs,
        ep_proof,
        ep_claimed_output,
    )
}

/// Verify the exact provenance-bound six-input fold pair and retain ownership.
///
/// Eq is verified first. No seal is constructed unless Ep also verifies and
/// both verifier-derived outputs exactly match their ownership-bound claims.
pub(super) fn verify_provenance_bound_state_recursive_fold_native_relation_v2(
    candidate: ProvenanceBoundStateRecursiveFoldResultV2,
) -> Result<StateRecursiveFoldNativeRelationSealV2, StateRecursiveFoldNativeRelationErrorV2> {
    let eq_views = candidate.eq_inputs();
    let ep_views = candidate.ep_inputs();
    for (index, (view, expected_role)) in eq_views
        .iter()
        .zip(STATE_RECURSIVE_FOLD_INPUT_ORDER_V2)
        .enumerate()
    {
        if view.role() != expected_role {
            return Err(StateRecursiveFoldNativeRelationErrorV2::input(
                StateRecursiveFoldParityV2::Eq,
                index,
            ));
        }
    }
    for (index, (view, expected_role)) in ep_views
        .iter()
        .zip(STATE_RECURSIVE_FOLD_INPUT_ORDER_V2)
        .enumerate()
    {
        if view.role() != expected_role {
            return Err(StateRecursiveFoldNativeRelationErrorV2::input(
                StateRecursiveFoldParityV2::Ep,
                index,
            ));
        }
    }
    let eq_inputs = eq_views.map(|input| input.accumulator());
    let ep_inputs = ep_views.map(|input| input.accumulator());
    let result = candidate.result();
    verify_relation_pair_v2(
        eq_inputs,
        result.eq_proof(),
        result.eq_claimed_output(),
        ep_inputs,
        result.ep_proof(),
        result.ep_claimed_output(),
    )?;
    Ok(StateRecursiveFoldNativeRelationSealV2 { candidate })
}

#[cfg(test)]
#[path = "state_recursive_fold_native_tests.rs"]
mod tests;
