//! Owned single-phase IPA witness prefix, internal and not a proof entry point.
//!
//! The one key supplies every phase coordinate and remains inseparable from its completed
//! receipts, provider, proof RNG and transcript. The original producer and floor planner run
//! once; its final owner drops before instance commitments and tail/blind randomness. Errors
//! and unwinding destroy the partial continuation, including the owned RNG and transcript.
//!
//! Coordinate checks do not authenticate a circuit relation or parameter artifact. TODO: the
//! exact Core Claim Eq/Ep and recursive-state constructors must consume their authenticated,
//! role/parity/configuration-bound materials before a complete stored suffix is exposed. Never
//! promote this internal generic Circuit helper to arbitrary public producer admission. The
//! existing ordinary/consuming proof APIs remain unchanged. The stored quotient continuation
//! produces undivided numerator parts and the inverse continuation produces ordinary
//! coefficient pieces. Quotient commitments and openings remain required. No complete proof,
//! process-memory or latency claim follows.

use std::{
    marker::PhantomData,
    ptr,
    sync::atomic::{Ordering, compiler_fence},
};

use ff::{Field, WithSmallOrderMulGroup};
use group::Curve;
use rand_core::RngCore;

use crate::{
    arithmetic::CurveAffine,
    circuit::layouter::SyncDeps,
    plonk::{Circuit, ConstraintSystem, FloorPlanner, ProvingKey},
    poly::{
        EvaluationDomain, LagrangeCoeff, Polynomial,
        commitment::{Blind, Params},
        ipa::commitment::ParamsIPA,
        stored_advice::{
            StoredPolynomialBasisV1, StoredPolynomialErrorV1, StoredPolynomialProviderV1,
            StoredPolynomialRoleV1, StoredPolynomialWriterV1,
            assignment::StoredAssignmentFieldV1,
            phase::{
                CoefficientStoredAdviceV1, CompleteStoredAdviceV1, StoredPhaseErrorV1,
                admit_stored_phase_plan_v1,
                synthesis::{StoredSinglePhaseAssignmentV1, StoredSynthesisErrorV1},
            },
        },
    },
    transcript::{EncodedChallenge, TranscriptWrite},
};

/// Coarse prefix refusal; never embeds witness, backend or transcript text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StoredPrefixErrorV1 {
    /// Unsupported parameter geometry or single-phase configuration coordinate mismatch.
    Admission,
    /// Supplied instance columns, rows or compile-time commitment policy were invalid.
    Instances,
    /// A checked public or guarded scratch allocation failed.
    Allocation,
    /// An authenticated store refused an operation.
    Store(StoredPolynomialErrorV1),
    /// The consuming single-phase synthesis adapter refused the producer.
    Synthesis(StoredSynthesisErrorV1),
    /// Assignment finalization, advice commitment or phase absorption failed.
    Phase(StoredPhaseErrorV1),
    /// VK or instance prefix absorption failed; the owned transcript is discarded.
    Transcript,
}

impl From<StoredPolynomialErrorV1> for StoredPrefixErrorV1 {
    fn from(error: StoredPolynomialErrorV1) -> Self {
        Self::Store(error)
    }
}
impl From<StoredPhaseErrorV1> for StoredPrefixErrorV1 {
    fn from(error: StoredPhaseErrorV1) -> Self {
        Self::Phase(error)
    }
}
impl From<StoredSynthesisErrorV1> for StoredPrefixErrorV1 {
    fn from(error: StoredSynthesisErrorV1) -> Self {
        Self::Synthesis(error)
    }
}

/// Unfinished stored prover state; no detached-key pairing or proof-byte accessor exists.
///
/// Its only constructor runs the original producer against its own key-derived schedule.
/// Future suffix methods must derive graphs, fixed values and coset domains from this key,
/// preserve transcript/RNG ordering, and poison the complete owner on any error. Owning these
/// objects does not wipe every allocation owned by the unchanged key or caller-supplied R/T/P.
#[allow(dead_code)]
pub(crate) struct PendingStoredIpaProverV1<
    'params,
    'instances,
    C,
    P,
    R,
    T,
    E,
    const QUERY_INSTANCE: bool,
    const INSTANCE_MASK: u64,
> where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
    P: StoredPolynomialProviderV1,
{
    params: &'params ParamsIPA<C>,
    pk: ProvingKey<C>,
    advice: CompleteStoredAdviceV1<'params, C, <P::Writer as StoredPolynomialWriterV1>::Snapshot>,
    provider: P,
    rng: R,
    transcript: T,
    instances: &'instances [&'instances [C::Scalar]],
    _challenge: PhantomData<E>,
}

/// The original pending proof owner with coefficient copies staged for later cosets/openings.
///
/// Original Lagrange advice remains inside `advice` through the consuming lookup and copy
/// product stages. Staging performs no transcript or proof-RNG operation and grants no complete
/// argument token. There is no detached key/receipt constructor or mutable accessor.
/// The vanishing/y handoff moves original advice blinds into a coefficient-only owner.
#[allow(dead_code)]
pub(crate) struct CoefficientPendingStoredIpaProverV1<
    'params,
    'instances,
    C,
    P,
    R,
    T,
    E,
    const QUERY_INSTANCE: bool,
    const INSTANCE_MASK: u64,
> where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
    P: StoredPolynomialProviderV1,
{
    params: &'params ParamsIPA<C>,
    pk: ProvingKey<C>,
    advice:
        CoefficientStoredAdviceV1<'params, C, <P::Writer as StoredPolynomialWriterV1>::Snapshot>,
    provider: P,
    rng: R,
    transcript: T,
    instances: &'instances [&'instances [C::Scalar]],
    _challenge: PhantomData<E>,
}

impl<'params, 'instances, C, P, R, T, E, const QUERY_INSTANCE: bool, const INSTANCE_MASK: u64>
    PendingStoredIpaProverV1<'params, 'instances, C, P, R, T, E, QUERY_INSTANCE, INSTANCE_MASK>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    P: StoredPolynomialProviderV1,
{
    /// Consume this exact key/protocol owner into prerequisite coefficient staging.
    ///
    /// Domain and provider come only from this owner; no independent key, graph, receipt,
    /// challenge or completion token is accepted. Original Lagrange snapshots and sole blind
    /// guards remain retained for real arguments. Any error/unwind destroys all moved owners,
    /// including their original RNG/transcript; success preserves their current state exactly.
    pub(crate) fn stage_advice_coefficients(
        self,
    ) -> Result<
        CoefficientPendingStoredIpaProverV1<
            'params,
            'instances,
            C,
            P,
            R,
            T,
            E,
            QUERY_INSTANCE,
            INSTANCE_MASK,
        >,
        StoredPrefixErrorV1,
    > {
        let Self {
            params,
            pk,
            advice,
            mut provider,
            rng,
            transcript,
            instances,
            _challenge,
        } = self;
        let advice = advice.stage_coefficients(&pk.vk.domain, &mut provider)?;
        Ok(CoefficientPendingStoredIpaProverV1 {
            params,
            pk,
            advice,
            provider,
            rng,
            transcript,
            instances,
            _challenge,
        })
    }
}

fn reserved<T>(count: usize) -> Result<Vec<T>, StoredPrefixErrorV1> {
    let mut items = Vec::new();
    items
        .try_reserve_exact(count)
        .map_err(|_| StoredPrefixErrorV1::Allocation)?;
    Ok(items)
}

/// One zero-padded instance polynomial; each column drops before the next allocation.
struct InstanceScratch<F: StoredAssignmentFieldV1>(Polynomial<F, LagrangeCoeff>);
impl<F: StoredAssignmentFieldV1> InstanceScratch<F> {
    fn new(domain: &EvaluationDomain<F>, source: &[F]) -> Result<Self, StoredPrefixErrorV1>
    where
        F: WithSmallOrderMulGroup<3>,
    {
        let rows = 1_usize << domain.k();
        let mut values = reserved(rows)?;
        // Only zeros exist before the domain API transfers this allocation into the guard.
        values.resize(rows, F::ZERO);
        let mut owned = Self(domain.lagrange_from_vec(values));
        owned.0.values[..source.len()].copy_from_slice(source);
        Ok(owned)
    }
}
impl<F: StoredAssignmentFieldV1> Drop for InstanceScratch<F> {
    fn drop(&mut self) {
        for value in &mut self.0.values {
            // SAFETY: each initialized Copy Pasta scalar is exclusively borrowed; ZERO is valid.
            unsafe { ptr::write_volatile(value, F::ZERO) };
        }
        compiler_fence(Ordering::SeqCst);
    }
}

/// Check only coordinates preserved by selector optimization, never relation identity.
fn matching_coordinates<F: Field>(
    configured: &ConstraintSystem<F>,
    key: &ConstraintSystem<F>,
) -> bool {
    configured.num_advice_columns == key.num_advice_columns
        && configured.advice_column_phase == key.advice_column_phase
        && configured.num_instance_columns == key.num_instance_columns
        && configured.num_challenges == key.num_challenges
        && configured.challenge_phase == key.challenge_phase
        && configured.constants == key.constants
        && configured.permutation.columns == key.permutation.columns
        // Selector conversion appends fixed columns, so equality here would reject valid PKs.
        && configured.num_fixed_columns <= key.num_fixed_columns
}

/// Consume one original producer into its key-bound, single-phase stored witness prefix.
///
/// This crate-private primitive is not semantic producer admission. The production caller must
/// authenticate the concrete circuit configuration, key and ParamsIPA before construction.
/// QUERY_INSTANCE and INSTANCE_MASK are the exact constants of its selected IPA specialization.
/// No caller-supplied CS/domain, precompleted advice, graph or substitute key is accepted.
///
/// Success retains the exact owned key/provider/RNG/transcript; failure/unwind drops them. R/T/P
/// may themselves have caller-owned aliases: erasing those is outside the ownership guarantee.
/// The conditional SyncDeps bound preserves existing floor-planner feature requirements without
/// claiming that a single-threaded Rc store satisfies optional thread-safe-region mode.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_single_phase_stored_ipa_prefix_v1<
    'params,
    'instances,
    C,
    P,
    R,
    T,
    E,
    ConcreteCircuit,
    const QUERY_INSTANCE: bool,
    const INSTANCE_MASK: u64,
>(
    params: &'params ParamsIPA<C>,
    pk: ProvingKey<C>,
    producer: ConcreteCircuit,
    instances: &'instances [&'instances [C::Scalar]],
    mut provider: P,
    mut rng: R,
    mut transcript: T,
) -> Result<
    PendingStoredIpaProverV1<'params, 'instances, C, P, R, T, E, QUERY_INSTANCE, INSTANCE_MASK>,
    StoredPrefixErrorV1,
>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    P: StoredPolynomialProviderV1,
    R: RngCore,
    E: EncodedChallenge<C>,
    T: TranscriptWrite<C, E>,
    ConcreteCircuit: Circuit<C::Scalar>,
    StoredSinglePhaseAssignmentV1<'params, 'instances, C, P::Writer>: SyncDeps,
{
    let meta = &pk.vk.cs;
    if instances.len() != meta.num_instance_columns
        || (!QUERY_INSTANCE && INSTANCE_MASK != 0)
        || (meta.num_instance_columns < u64::BITS as usize
            && INSTANCE_MASK >> meta.num_instance_columns != 0)
    {
        return Err(StoredPrefixErrorV1::Instances);
    }
    let plan = admit_stored_phase_plan_v1(params, &pk.vk.domain, meta)?;
    // Single-phase means every configured advice column and challenge belongs to phase zero.
    // Zero-advice circuits still absorb the one empty phase, as in phase::into_complete.
    if meta
        .advice_column_phase
        .iter()
        .any(|phase| phase.to_u8() != 0)
        || meta.challenge_phase.iter().any(|phase| phase.to_u8() != 0)
    {
        return Err(StoredPrefixErrorV1::Admission);
    }
    let usable_rows = (params.n() as usize)
        .checked_sub(
            meta.blinding_factors()
                .checked_add(1)
                .ok_or(StoredPrefixErrorV1::Admission)?,
        )
        .ok_or(StoredPrefixErrorV1::Admission)?;
    if instances.iter().any(|column| column.len() > usable_rows) {
        return Err(StoredPrefixErrorV1::Instances);
    }
    let mut configured = ConstraintSystem::default();
    #[cfg(feature = "circuit-params")]
    let config = ConcreteCircuit::configure_with_params(&mut configured, producer.params());
    #[cfg(not(feature = "circuit-params"))]
    let config = ConcreteCircuit::configure(&mut configured);
    if !matching_coordinates(&configured, meta) {
        return Err(StoredPrefixErrorV1::Admission);
    }
    drop(configured);
    let mut constants = reserved(meta.constants.len())?;
    constants.extend_from_slice(&meta.constants);
    let mut writers = reserved(meta.num_advice_columns)?;
    // A valid proof hashes its VK before synthesis, exactly as the ordinary prover does.
    pk.vk
        .hash_into::<E, T>(&mut transcript)
        .map_err(|_| StoredPrefixErrorV1::Transcript)?;
    for column in 0..meta.num_advice_columns {
        writers.push(provider.create(
            C::Scalar::STORED_FIELD,
            StoredPolynomialBasisV1::Lagrange,
            params.k(),
            StoredPolynomialRoleV1::Advice {
                column: column as u32,
                phase: 0,
            },
        )?);
    }
    let mut assignments = StoredSinglePhaseAssignmentV1::new(plan, writers, instances)?;
    let result =
        ConcreteCircuit::FloorPlanner::synthesize(&mut assignments, &producer, config, constants);
    // No final-phase RNG or instance commitment is reachable while this owner lives.
    drop(producer);
    let assignments = assignments.into_assignments(result)?;

    if QUERY_INSTANCE {
        let mut projective = reserved(instances.len())?;
        let mut affine = reserved(instances.len())?;
        affine.resize(instances.len(), C::identity());
        for column in instances {
            let scratch = InstanceScratch::new(&pk.vk.domain, column)?;
            projective.push(params.commit_lagrange(&scratch.0, Blind::default()));
        }
        C::Curve::batch_normalize(&projective, &mut affine);
        drop(projective);
        for (column, point) in affine.into_iter().enumerate() {
            let result = if column < u64::BITS as usize && (INSTANCE_MASK >> column) & 1 != 0 {
                transcript.write_point(point)
            } else {
                transcript.common_point(point)
            };
            result.map_err(|_| StoredPrefixErrorV1::Transcript)?;
        }
    } else {
        for column in instances {
            for value in *column {
                transcript
                    .common_scalar(*value)
                    .map_err(|_| StoredPrefixErrorV1::Transcript)?;
            }
        }
    }
    let advice = assignments
        .finish(&mut rng)?
        .absorb::<E, T>(&mut transcript)?
        .into_complete()?;
    Ok(PendingStoredIpaProverV1 {
        params,
        pk,
        advice,
        provider,
        rng,
        transcript,
        instances,
        _challenge: PhantomData,
    })
}

#[cfg(test)]
mod tests;

mod auxiliary;
mod lookup;

mod lookup_membership;
mod lookup_permuted;
mod lookup_sort;

#[path = "stored/products.rs"]
mod products;

#[path = "stored/vanishing.rs"]
mod vanishing;

/// Complete original quotient numerator over authenticated bounded coset sources.
mod quotient;

/// Original-domain quotient division and inverse mixing before any quotient commitments.
mod quotient_inverse;

/// Original quotient commitments and ChallengeX over the closed coefficient owner.
mod quotient_commitments;
