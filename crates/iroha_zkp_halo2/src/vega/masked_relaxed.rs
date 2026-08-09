//! Bounded setup-free masked relaxed-R1CS composition.
//!
//! This is the shared protocol boundary for native relations that use the
//! T256 Hyrax commitment, sequential Nova folding, and one terminal Relaxed
//! Spartan proof. A fresh full relaxed assignment masks every strict witness
//! coordinate before Spartan's direct openings are released.

#![allow(unexpected_cfgs)]

use core::ops::{Deref, DerefMut};

use thiserror::Error;

use super::{
    VegaPointWireV1, VegaScalarWireV1, VegaT256ScalarV1 as Scalar,
    circuit::{CircuitAssignment, MAX_CIRCUIT_ROWS},
    commitment::{Commitment, CommitmentKey, MAX_COMMITMENT_WORKERS},
    nifs::{NovaNifs, NovaNifsProverInput},
    r1cs::{Instance, RelaxedInstance, RelaxedWitness, Shape, Witness},
    spartan::RelaxedSpartanProof,
    sumcheck::{CompressedUnivariate, SumcheckProof},
    transcript::VegaTranscriptV1,
};

pub(super) const MASKED_RELAXED_COMMITMENT_COLUMNS_V1: usize = 1024;
pub(super) const MAX_MASKED_RELAXED_STRICT_INSTANCES_V1: usize = 8;
const PROOF_VERSION_V1: u8 = 1;
const RANDOM_HEALTH_RETRIES: usize = 16;

/// Failure explicitly reported by an injected cryptographic random source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum MaskedRelaxedRandomErrorV1 {
    /// The operating-system or hardware random source was unavailable.
    #[error("masked relaxed-R1CS cryptographic random source is unavailable")]
    Unavailable,
}

/// Fallible cryptographic byte source used by the masked composer.
pub trait MaskedRelaxedRandomSourceV1 {
    /// Fill the complete destination or report unavailability.
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum MaskedRelaxedErrorV1 {
    #[error("masked relaxed-R1CS strict-instance count {actual} is outside 1..={max}")]
    InvalidInstanceCount { actual: usize, max: usize },
    #[error("masked relaxed-R1CS worker count {actual} is outside 1..={max}")]
    InvalidWorkerCount { actual: usize, max: usize },
    #[error("masked relaxed-R1CS compiled shape or profile is invalid")]
    InvalidProfile,
    #[error("masked relaxed-R1CS strict witness is unsatisfied")]
    UnsatisfiedWitness,
    #[error("masked relaxed-R1CS proof encoding is invalid")]
    InvalidProofEncoding,
    #[error("masked relaxed-R1CS verification failed")]
    VerificationFailed,
    #[error("masked relaxed-R1CS randomness is degenerate")]
    DegenerateRandomness,
    #[error(transparent)]
    Random(#[from] MaskedRelaxedRandomErrorV1),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MaskedRelaxedDimensionsV1 {
    pub(super) variable_count: usize,
    pub(super) constraint_count: usize,
    pub(super) public_input_count: usize,
    pub(super) witness_commitment_points: usize,
    pub(super) error_commitment_points: usize,
    pub(super) outer_sumcheck_rounds: usize,
    pub(super) inner_sumcheck_rounds: usize,
}

impl MaskedRelaxedDimensionsV1 {
    pub(super) fn from_shape(shape: &Shape) -> Result<Self, MaskedRelaxedErrorV1> {
        if shape.public_input_count() == 0
            || shape.variable_count() > MAX_CIRCUIT_ROWS
            || shape.constraint_count() > MAX_CIRCUIT_ROWS
            || !shape.variable_count().is_power_of_two()
            || !shape.constraint_count().is_power_of_two()
            || MASKED_RELAXED_COMMITMENT_COLUMNS_V1 > shape.variable_count()
            || MASKED_RELAXED_COMMITMENT_COLUMNS_V1 > shape.constraint_count()
        {
            return Err(MaskedRelaxedErrorV1::InvalidProfile);
        }
        let witness_commitment_points = shape
            .variable_count()
            .div_ceil(MASKED_RELAXED_COMMITMENT_COLUMNS_V1);
        let error_commitment_points = shape
            .constraint_count()
            .div_ceil(MASKED_RELAXED_COMMITMENT_COLUMNS_V1);
        if !witness_commitment_points.is_power_of_two()
            || !error_commitment_points.is_power_of_two()
        {
            return Err(MaskedRelaxedErrorV1::InvalidProfile);
        }
        Ok(Self {
            variable_count: shape.variable_count(),
            constraint_count: shape.constraint_count(),
            public_input_count: shape.public_input_count(),
            witness_commitment_points,
            error_commitment_points,
            outer_sumcheck_rounds: usize::try_from(shape.constraint_count().ilog2())
                .map_err(|_| MaskedRelaxedErrorV1::InvalidProfile)?,
            inner_sumcheck_rounds: usize::try_from(shape.variable_count().ilog2())
                .map_err(|_| MaskedRelaxedErrorV1::InvalidProfile)?
                .checked_add(1)
                .ok_or(MaskedRelaxedErrorV1::InvalidProfile)?,
        })
    }

    pub(super) fn proof_decode_limits(
        self,
        expected_instances: usize,
        payload_len: usize,
        max_proof_bytes: usize,
    ) -> Result<norito::DecodeLimits, MaskedRelaxedErrorV1> {
        validate_count(expected_instances)?;
        let max_sequence_elements = [
            self.witness_commitment_points,
            self.error_commitment_points,
            self.public_input_count,
            expected_instances,
            self.outer_sumcheck_rounds,
            self.inner_sumcheck_rounds,
            MASKED_RELAXED_COMMITMENT_COLUMNS_V1,
        ]
        .into_iter()
        .max()
        .ok_or(MaskedRelaxedErrorV1::InvalidProfile)?;
        let nested_commitment_elements = self
            .witness_commitment_points
            .checked_add(self.error_commitment_points)
            .and_then(|points| points.checked_mul(expected_instances))
            .ok_or(MaskedRelaxedErrorV1::InvalidProfile)?;
        let max_total_elements = self
            .witness_commitment_points
            .checked_add(self.error_commitment_points)
            .and_then(|total| total.checked_add(self.public_input_count))
            .and_then(|total| {
                expected_instances
                    .checked_mul(2)
                    .and_then(|outer_vectors| total.checked_add(outer_vectors))
            })
            .and_then(|total| total.checked_add(nested_commitment_elements))
            .and_then(|total| total.checked_add(self.outer_sumcheck_rounds))
            .and_then(|total| total.checked_add(self.inner_sumcheck_rounds))
            .and_then(|total| {
                MASKED_RELAXED_COMMITMENT_COLUMNS_V1
                    .checked_mul(2)
                    .and_then(|openings| total.checked_add(openings))
            })
            .ok_or(MaskedRelaxedErrorV1::InvalidProfile)?;
        Ok(norito::DecodeLimits::new(
            max_sequence_elements,
            payload_len,
            max_total_elements,
            max_proof_bytes.saturating_mul(8),
            24,
        ))
    }
}

#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize,
)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
#[norito(decode_from_slice)]
pub(super) struct MaskedRelaxedCommitmentWireV1 {
    pub(super) points: Vec<VegaPointWireV1>,
}

#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize,
)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
#[norito(decode_from_slice)]
pub(super) struct MaskedRelaxedProofWireV1 {
    pub(super) version: u8,
    pub(super) strict_instance_count: u8,
    pub(super) mask_witness_commitment: MaskedRelaxedCommitmentWireV1,
    pub(super) mask_error_commitment: MaskedRelaxedCommitmentWireV1,
    pub(super) mask_relaxation: VegaScalarWireV1,
    pub(super) mask_public_inputs: Vec<VegaScalarWireV1>,
    pub(super) strict_witness_commitments: Vec<MaskedRelaxedCommitmentWireV1>,
    pub(super) cross_term_commitments: Vec<MaskedRelaxedCommitmentWireV1>,
    pub(super) outer_sumcheck_rounds: Vec<[VegaScalarWireV1; 3]>,
    pub(super) outer_claims: [VegaScalarWireV1; 3],
    pub(super) inner_sumcheck_rounds: Vec<[VegaScalarWireV1; 2]>,
    pub(super) witness_opening: Vec<VegaScalarWireV1>,
    pub(super) witness_opening_blinding: VegaScalarWireV1,
    pub(super) error_opening: Vec<VegaScalarWireV1>,
    pub(super) error_opening_blinding: VegaScalarWireV1,
}

/// Producer-side masked Nova history before the terminal Spartan proof.
///
/// The final folded witness remains inside a zeroizing wrapper. Public
/// consumers may use the instances/folds to build an audit transcript, but
/// settlement must derive the terminal instance again with
/// [`verify_and_replay_masked_relaxed_v1`].
pub(super) struct MaskedRelaxedPrecomputationV1 {
    pub(super) shape: Shape,
    pub(super) mask_instance: RelaxedInstance,
    pub(super) strict_instances: Vec<Instance>,
    pub(super) folds: Vec<NovaNifs>,
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) folded_instance: RelaxedInstance,
    folded_witness: SecretRelaxedWitness,
}

impl MaskedRelaxedPrecomputationV1 {
    pub(super) fn folded_witness(&self) -> &RelaxedWitness {
        &self.folded_witness
    }
}

pub(super) fn prove_masked_relaxed_v1<R: MaskedRelaxedRandomSourceV1>(
    domain: &'static [u8],
    context_frame: &[u8],
    commitment_key_label: &[u8],
    assignments: Vec<CircuitAssignment>,
    worker_count: usize,
    random: &mut R,
) -> Result<MaskedRelaxedProofWireV1, MaskedRelaxedErrorV1> {
    let precomputation = precompute_masked_relaxed_v1(
        domain,
        context_frame,
        commitment_key_label,
        assignments,
        worker_count,
        random,
    )?;
    prove_masked_relaxed_precomputation_v1(
        domain,
        context_frame,
        commitment_key_label,
        &precomputation,
        worker_count,
    )
}

/// Build the complete producer-side masked Nova history from strict circuit
/// assignments. This is the canonical source of the transcript schedule used
/// by plaintext KATs and by the encrypted Phase-II/III conformance oracle.
pub(super) fn precompute_masked_relaxed_v1<R: MaskedRelaxedRandomSourceV1>(
    domain: &'static [u8],
    context_frame: &[u8],
    commitment_key_label: &[u8],
    mut assignments: Vec<CircuitAssignment>,
    worker_count: usize,
    random: &mut R,
) -> Result<MaskedRelaxedPrecomputationV1, MaskedRelaxedErrorV1> {
    validate_count(assignments.len())?;
    validate_worker_count(worker_count)?;
    if domain.is_empty() || context_frame.is_empty() || commitment_key_label.is_empty() {
        return Err(MaskedRelaxedErrorV1::InvalidProfile);
    }
    let shape = assignments
        .first()
        .map(|assignment| assignment.shape.clone())
        .ok_or(MaskedRelaxedErrorV1::InvalidProfile)?;
    let dimensions = MaskedRelaxedDimensionsV1::from_shape(&shape)?;
    if assignments.iter().any(|assignment| {
        assignment.shape != shape || assignment.public_inputs.len() != dimensions.public_input_count
    }) {
        return Err(MaskedRelaxedErrorV1::InvalidProfile);
    }
    for assignment in &assignments {
        shape
            .validate_relaxed_assignment(
                &assignment.witness,
                Scalar::one(),
                &assignment.public_inputs,
                &vec![Scalar::zero(); shape.constraint_count()],
            )
            .map_err(|_| MaskedRelaxedErrorV1::UnsatisfiedWitness)?;
    }
    let key = CommitmentKey::derive(commitment_key_label, MASKED_RELAXED_COMMITMENT_COLUMNS_V1)
        .and_then(|key| key.with_worker_count(worker_count))
        .map_err(|_| MaskedRelaxedErrorV1::InvalidProfile)?;

    validate_random_health(random)?;
    let (mut folded_instance, folded_witness) =
        sample_relaxed_mask(random, &shape, &key, dimensions)?;
    let mut folded_witness = SecretRelaxedWitness::new(folded_witness);
    let mask_instance = folded_instance.clone();
    let public_inputs = assignments
        .iter()
        .map(|assignment| assignment.public_inputs.clone())
        .collect::<Vec<_>>();
    let mut transcript =
        composition_transcript(domain, context_frame, &shape, &public_inputs, dimensions)?;
    let mut strict_instances = Vec::with_capacity(assignments.len());
    let mut folds = Vec::with_capacity(assignments.len());

    for assignment in &mut assignments {
        let values = core::mem::take(&mut assignment.witness);
        let blindings = SecretScalars::new(sample_nonzero_scalars(
            random,
            dimensions.witness_commitment_points,
        )?);
        let regular_witness = SecretWitness::new(Witness {
            values,
            blindings: blindings.to_vec(),
        });
        let regular_instance = Instance {
            witness_commitment: key
                .commit(&regular_witness.values, &regular_witness.blindings)
                .map_err(|_| MaskedRelaxedErrorV1::DegenerateRandomness)?,
            public_inputs: assignment.public_inputs.clone(),
        };
        let cross_blindings = SecretScalars::new(sample_nonzero_scalars(
            random,
            dimensions.error_commitment_points,
        )?);
        let (fold, next_instance, next_witness) = NovaNifs::prove(
            NovaNifsProverInput {
                key: &key,
                shape: &shape,
                relaxed_instance: &folded_instance,
                relaxed_witness: &folded_witness,
                regular_instance: &regular_instance,
                regular_witness: &regular_witness,
                cross_term_blindings: &cross_blindings,
            },
            &mut transcript,
        )
        .map_err(|_| MaskedRelaxedErrorV1::InvalidProfile)?;
        folded_instance = next_instance;
        folded_witness.replace(next_witness);
        strict_instances.push(regular_instance);
        folds.push(fold);
    }
    Ok(MaskedRelaxedPrecomputationV1 {
        shape,
        mask_instance,
        strict_instances,
        folds,
        folded_instance,
        folded_witness,
    })
}

/// Finish one producer-side precomputation without exposing or copying its
/// zeroized final folded witness.
pub(super) fn prove_masked_relaxed_precomputation_v1(
    domain: &'static [u8],
    context_frame: &[u8],
    commitment_key_label: &[u8],
    precomputation: &MaskedRelaxedPrecomputationV1,
    worker_count: usize,
) -> Result<MaskedRelaxedProofWireV1, MaskedRelaxedErrorV1> {
    prove_precomputed_masked_relaxed_inner_v1(
        domain,
        context_frame,
        commitment_key_label,
        &precomputation.shape,
        &precomputation.mask_instance,
        &precomputation.strict_instances,
        &precomputation.folds,
        precomputation.folded_witness(),
        worker_count,
    )
}

/// Finish a masked relaxed-R1CS proof from a publicly replayable fold history
/// and the one final folded witness reconstructed by the PBS.
///
/// The caller cannot nominate the terminal relaxed instance. This function
/// replays every Nova challenge from the mask, the ordered strict public
/// inputs and witness commitments, and the ordered cross-term commitments.
/// Only the resulting instance is accepted by the terminal Spartan prover.
/// That hard boundary prevents a malicious PBS from replacing the encrypted
/// fold history with an independently satisfiable relaxed assignment.
#[allow(clippy::too_many_arguments)]
pub(super) fn prove_precomputed_masked_relaxed_v1(
    domain: &'static [u8],
    context_frame: &[u8],
    commitment_key_label: &[u8],
    shape: &Shape,
    mask_instance: &RelaxedInstance,
    strict_instances: &[Instance],
    folds: &[NovaNifs],
    folded_witness: RelaxedWitness,
    worker_count: usize,
) -> Result<MaskedRelaxedProofWireV1, MaskedRelaxedErrorV1> {
    // Take ownership immediately so every success and error path scrubs the
    // materialized folded witness on return.
    let folded_witness = SecretRelaxedWitness::new(folded_witness);
    #[cfg(test)]
    maybe_panic_after_precomputed_witness_handoff_v1();
    prove_precomputed_masked_relaxed_inner_v1(
        domain,
        context_frame,
        commitment_key_label,
        shape,
        mask_instance,
        strict_instances,
        folds,
        &folded_witness,
        worker_count,
    )
}

#[allow(clippy::too_many_arguments)]
fn prove_precomputed_masked_relaxed_inner_v1(
    domain: &'static [u8],
    context_frame: &[u8],
    commitment_key_label: &[u8],
    shape: &Shape,
    mask_instance: &RelaxedInstance,
    strict_instances: &[Instance],
    folds: &[NovaNifs],
    folded_witness: &RelaxedWitness,
    worker_count: usize,
) -> Result<MaskedRelaxedProofWireV1, MaskedRelaxedErrorV1> {
    validate_count(strict_instances.len())?;
    validate_worker_count(worker_count)?;
    if domain.is_empty()
        || context_frame.is_empty()
        || commitment_key_label.is_empty()
        || folds.len() != strict_instances.len()
    {
        return Err(MaskedRelaxedErrorV1::InvalidProfile);
    }
    let dimensions = MaskedRelaxedDimensionsV1::from_shape(shape)?;
    if strict_instances
        .iter()
        .any(|instance| instance.public_inputs.len() != dimensions.public_input_count)
    {
        return Err(MaskedRelaxedErrorV1::InvalidProfile);
    }
    let strict_public_inputs = strict_instances
        .iter()
        .map(|instance| instance.public_inputs.clone())
        .collect::<Vec<_>>();
    let key = CommitmentKey::derive(commitment_key_label, MASKED_RELAXED_COMMITMENT_COLUMNS_V1)
        .and_then(|key| key.with_worker_count(worker_count))
        .map_err(|_| MaskedRelaxedErrorV1::InvalidProfile)?;
    let mut transcript = composition_transcript(
        domain,
        context_frame,
        shape,
        &strict_public_inputs,
        dimensions,
    )?;
    let mut folded_instance = mask_instance.clone();
    for (strict_instance, fold) in strict_instances.iter().zip(folds) {
        folded_instance = fold
            .verify(
                &key,
                shape,
                &mut transcript,
                &folded_instance,
                strict_instance,
            )
            .map_err(|_| MaskedRelaxedErrorV1::VerificationFailed)?;
    }

    shape
        .validate_relaxed_assignment(
            &folded_witness.values,
            folded_instance.relaxation,
            &folded_instance.public_inputs,
            &folded_witness.error,
        )
        .map_err(|_| MaskedRelaxedErrorV1::UnsatisfiedWitness)?;
    if key
        .commit(&folded_witness.values, &folded_witness.witness_blindings)
        .map_err(|_| MaskedRelaxedErrorV1::InvalidProfile)?
        != folded_instance.witness_commitment
        || key
            .commit(&folded_witness.error, &folded_witness.error_blindings)
            .map_err(|_| MaskedRelaxedErrorV1::InvalidProfile)?
            != folded_instance.error_commitment
    {
        return Err(MaskedRelaxedErrorV1::VerificationFailed);
    }

    let spartan = RelaxedSpartanProof::prove(
        shape,
        &key,
        &folded_instance,
        folded_witness,
        &mut transcript,
    )
    .map_err(|_| MaskedRelaxedErrorV1::InvalidProfile)?;
    let proof =
        MaskedRelaxedProofWireV1::from_protocol(mask_instance, strict_instances, folds, &spartan)?;

    // A native prover bug must never escape as a settlement-acceptable wire.
    verify_masked_relaxed_v1(
        domain,
        context_frame,
        commitment_key_label,
        shape,
        &strict_public_inputs,
        &proof,
    )?;
    Ok(proof)
}

pub(super) fn verify_masked_relaxed_v1(
    domain: &'static [u8],
    context_frame: &[u8],
    commitment_key_label: &[u8],
    shape: &Shape,
    strict_public_inputs: &[Vec<Scalar>],
    proof: &MaskedRelaxedProofWireV1,
) -> Result<(), MaskedRelaxedErrorV1> {
    verify_and_replay_masked_relaxed_v1(
        domain,
        context_frame,
        commitment_key_label,
        shape,
        strict_public_inputs,
        proof,
    )
    .map(|_| ())
}

/// Verify the complete masked fold proof and return the terminal relaxed
/// instance derived by public replay.
///
/// Consumers with a separately transported terminal anchor must compare it
/// exactly to this value. Returning the replay result avoids circularly
/// binding that anchor into the Fiat--Shamir context that produced it.
pub(super) fn verify_and_replay_masked_relaxed_v1(
    domain: &'static [u8],
    context_frame: &[u8],
    commitment_key_label: &[u8],
    shape: &Shape,
    strict_public_inputs: &[Vec<Scalar>],
    proof: &MaskedRelaxedProofWireV1,
) -> Result<RelaxedInstance, MaskedRelaxedErrorV1> {
    validate_count(strict_public_inputs.len())?;
    if domain.is_empty() || context_frame.is_empty() || commitment_key_label.is_empty() {
        return Err(MaskedRelaxedErrorV1::InvalidProfile);
    }
    let dimensions = MaskedRelaxedDimensionsV1::from_shape(shape)?;
    proof.validate_shape(dimensions, strict_public_inputs.len())?;
    if strict_public_inputs
        .iter()
        .any(|inputs| inputs.len() != dimensions.public_input_count)
    {
        return Err(MaskedRelaxedErrorV1::InvalidProfile);
    }
    let key = CommitmentKey::derive(commitment_key_label, MASKED_RELAXED_COMMITMENT_COLUMNS_V1)
        .map_err(|_| MaskedRelaxedErrorV1::InvalidProfile)?;
    let (mask, strict, folds, spartan) = proof.to_protocol(strict_public_inputs)?;
    let mut transcript = composition_transcript(
        domain,
        context_frame,
        shape,
        strict_public_inputs,
        dimensions,
    )?;
    let mut folded = mask;
    for ((instance, fold), _) in strict.iter().zip(&folds).zip(strict_public_inputs) {
        folded = fold
            .verify(&key, shape, &mut transcript, &folded, instance)
            .map_err(|_| MaskedRelaxedErrorV1::VerificationFailed)?;
    }
    spartan
        .verify(shape, &key, &folded, &mut transcript)
        .map_err(|_| MaskedRelaxedErrorV1::VerificationFailed)?;
    Ok(folded)
}

impl MaskedRelaxedProofWireV1 {
    fn from_protocol(
        mask: &RelaxedInstance,
        strict: &[Instance],
        folds: &[NovaNifs],
        spartan: &RelaxedSpartanProof,
    ) -> Result<Self, MaskedRelaxedErrorV1> {
        if strict.len() != folds.len() {
            return Err(MaskedRelaxedErrorV1::InvalidProfile);
        }
        Ok(Self {
            version: PROOF_VERSION_V1,
            strict_instance_count: u8::try_from(strict.len())
                .map_err(|_| MaskedRelaxedErrorV1::InvalidProfile)?,
            mask_witness_commitment: MaskedRelaxedCommitmentWireV1::from_commitment(
                &mask.witness_commitment,
            )?,
            mask_error_commitment: MaskedRelaxedCommitmentWireV1::from_commitment(
                &mask.error_commitment,
            )?,
            mask_relaxation: VegaScalarWireV1::from_scalar(mask.relaxation),
            mask_public_inputs: mask
                .public_inputs
                .iter()
                .copied()
                .map(VegaScalarWireV1::from_scalar)
                .collect(),
            strict_witness_commitments: strict
                .iter()
                .map(|instance| {
                    MaskedRelaxedCommitmentWireV1::from_commitment(&instance.witness_commitment)
                })
                .collect::<Result<_, _>>()?,
            cross_term_commitments: folds
                .iter()
                .map(|fold| {
                    MaskedRelaxedCommitmentWireV1::from_commitment(&fold.cross_term_commitment)
                })
                .collect::<Result<_, _>>()?,
            outer_sumcheck_rounds: spartan
                .outer_sumcheck
                .rounds
                .iter()
                .map(|round| scalar_array_to_wire(round.coefficients()))
                .collect::<Result<_, _>>()?,
            outer_claims: spartan.outer_claims.map(VegaScalarWireV1::from_scalar),
            inner_sumcheck_rounds: spartan
                .inner_sumcheck
                .rounds
                .iter()
                .map(|round| scalar_array_to_wire(round.coefficients()))
                .collect::<Result<_, _>>()?,
            witness_opening: spartan
                .witness_opening
                .iter()
                .copied()
                .map(VegaScalarWireV1::from_scalar)
                .collect(),
            witness_opening_blinding: VegaScalarWireV1::from_scalar(
                spartan.witness_opening_blinding,
            ),
            error_opening: spartan
                .error_opening
                .iter()
                .copied()
                .map(VegaScalarWireV1::from_scalar)
                .collect(),
            error_opening_blinding: VegaScalarWireV1::from_scalar(spartan.error_opening_blinding),
        })
    }

    pub(super) fn validate_shape(
        &self,
        dimensions: MaskedRelaxedDimensionsV1,
        expected_instances: usize,
    ) -> Result<(), MaskedRelaxedErrorV1> {
        if self.version != PROOF_VERSION_V1
            || usize::from(self.strict_instance_count) != expected_instances
            || self.mask_witness_commitment.points.len() != dimensions.witness_commitment_points
            || self.mask_error_commitment.points.len() != dimensions.error_commitment_points
            || self.mask_public_inputs.len() != dimensions.public_input_count
            || self.strict_witness_commitments.len() != expected_instances
            || self.cross_term_commitments.len() != expected_instances
            || self
                .strict_witness_commitments
                .iter()
                .any(|commitment| commitment.points.len() != dimensions.witness_commitment_points)
            || self
                .cross_term_commitments
                .iter()
                .any(|commitment| commitment.points.len() != dimensions.error_commitment_points)
            || self.outer_sumcheck_rounds.len() != dimensions.outer_sumcheck_rounds
            || self.inner_sumcheck_rounds.len() != dimensions.inner_sumcheck_rounds
            || self.witness_opening.len() != MASKED_RELAXED_COMMITMENT_COLUMNS_V1
            || self.error_opening.len() != MASKED_RELAXED_COMMITMENT_COLUMNS_V1
        {
            return Err(MaskedRelaxedErrorV1::InvalidProofEncoding);
        }
        Ok(())
    }

    fn to_protocol(
        &self,
        strict_public_inputs: &[Vec<Scalar>],
    ) -> Result<
        (
            RelaxedInstance,
            Vec<Instance>,
            Vec<NovaNifs>,
            RelaxedSpartanProof,
        ),
        MaskedRelaxedErrorV1,
    > {
        let mask = RelaxedInstance {
            witness_commitment: self.mask_witness_commitment.to_commitment()?,
            error_commitment: self.mask_error_commitment.to_commitment()?,
            public_inputs: wire_to_scalars(&self.mask_public_inputs)?,
            relaxation: wire_to_scalar(self.mask_relaxation)?,
        };
        let strict = self
            .strict_witness_commitments
            .iter()
            .zip(strict_public_inputs)
            .map(|(commitment, public_inputs)| {
                Ok(Instance {
                    witness_commitment: commitment.to_commitment()?,
                    public_inputs: public_inputs.clone(),
                })
            })
            .collect::<Result<Vec<_>, MaskedRelaxedErrorV1>>()?;
        let folds = self
            .cross_term_commitments
            .iter()
            .map(|commitment| {
                Ok(NovaNifs {
                    cross_term_commitment: commitment.to_commitment()?,
                })
            })
            .collect::<Result<Vec<_>, MaskedRelaxedErrorV1>>()?;
        let spartan = RelaxedSpartanProof {
            outer_sumcheck: SumcheckProof::new(
                self.outer_sumcheck_rounds
                    .iter()
                    .map(|round| {
                        CompressedUnivariate::new(wire_to_scalars(round)?, 3)
                            .map_err(|_| MaskedRelaxedErrorV1::InvalidProofEncoding)
                    })
                    .collect::<Result<_, _>>()?,
            ),
            outer_claims: wire_to_scalar_array(&self.outer_claims)?,
            inner_sumcheck: SumcheckProof::new(
                self.inner_sumcheck_rounds
                    .iter()
                    .map(|round| {
                        CompressedUnivariate::new(wire_to_scalars(round)?, 2)
                            .map_err(|_| MaskedRelaxedErrorV1::InvalidProofEncoding)
                    })
                    .collect::<Result<_, _>>()?,
            ),
            witness_opening: wire_to_scalars(&self.witness_opening)?,
            witness_opening_blinding: wire_to_scalar(self.witness_opening_blinding)?,
            error_opening: wire_to_scalars(&self.error_opening)?,
            error_opening_blinding: wire_to_scalar(self.error_opening_blinding)?,
        };
        Ok((mask, strict, folds, spartan))
    }
}

impl MaskedRelaxedCommitmentWireV1 {
    fn from_commitment(commitment: &Commitment) -> Result<Self, MaskedRelaxedErrorV1> {
        Ok(Self {
            points: commitment
                .points()
                .iter()
                .copied()
                .map(VegaPointWireV1::from_point)
                .collect::<Result<_, _>>()
                .map_err(|_| MaskedRelaxedErrorV1::InvalidProfile)?,
        })
    }

    fn to_commitment(&self) -> Result<Commitment, MaskedRelaxedErrorV1> {
        Commitment::from_points(
            self.points
                .iter()
                .copied()
                .map(VegaPointWireV1::to_point)
                .collect::<Result<_, _>>()
                .map_err(|_| MaskedRelaxedErrorV1::InvalidProofEncoding)?,
        )
        .map_err(|_| MaskedRelaxedErrorV1::InvalidProofEncoding)
    }
}

fn sample_relaxed_mask<R: MaskedRelaxedRandomSourceV1>(
    random: &mut R,
    shape: &Shape,
    key: &CommitmentKey,
    dimensions: MaskedRelaxedDimensionsV1,
) -> Result<(RelaxedInstance, RelaxedWitness), MaskedRelaxedErrorV1> {
    let values = SecretScalars::new(sample_scalars(random, shape.variable_count())?);
    let relaxation = SecretScalar::new(sample_scalar(random)?);
    let public_inputs = SecretScalars::new(sample_scalars(random, shape.public_input_count())?);
    if relaxation.is_zero()
        && values.iter().all(|value| value.is_zero())
        && public_inputs.iter().all(|value| value.is_zero())
    {
        return Err(MaskedRelaxedErrorV1::DegenerateRandomness);
    }
    let mut assignment = SecretScalars::new(Vec::with_capacity(shape.columns()));
    assignment.extend_from_slice(&values);
    assignment.push(*relaxation);
    assignment.extend_from_slice(&public_inputs);
    let products = shape
        .multiply(&assignment)
        .map_err(|_| MaskedRelaxedErrorV1::InvalidProfile)?;
    let error = SecretScalars::new(
        products
            .a
            .into_iter()
            .zip(products.b)
            .zip(products.c)
            .map(|((a, b), c)| a * b - *relaxation * c)
            .collect(),
    );
    let witness_blindings = SecretScalars::new(sample_nonzero_scalars(
        random,
        dimensions.witness_commitment_points,
    )?);
    let error_blindings = SecretScalars::new(sample_nonzero_scalars(
        random,
        dimensions.error_commitment_points,
    )?);
    let witness_commitment = key
        .commit(&values, &witness_blindings)
        .map_err(|_| MaskedRelaxedErrorV1::DegenerateRandomness)?;
    let error_commitment = key
        .commit(&error, &error_blindings)
        .map_err(|_| MaskedRelaxedErrorV1::DegenerateRandomness)?;
    Ok((
        RelaxedInstance {
            witness_commitment,
            error_commitment,
            public_inputs: public_inputs.to_vec(),
            relaxation: *relaxation,
        },
        RelaxedWitness {
            values: values.to_vec(),
            witness_blindings: witness_blindings.to_vec(),
            error: error.to_vec(),
            error_blindings: error_blindings.to_vec(),
        },
    ))
}

/// Construct the sole masked-Nova composition transcript used by plaintext,
/// encrypted, prover, and verifier paths.
pub(super) fn masked_relaxed_composition_transcript_v1(
    domain: &'static [u8],
    context_frame: &[u8],
    shape: &Shape,
    strict_public_inputs: &[Vec<Scalar>],
) -> Result<VegaTranscriptV1, MaskedRelaxedErrorV1> {
    validate_count(strict_public_inputs.len())?;
    if domain.is_empty() || context_frame.is_empty() {
        return Err(MaskedRelaxedErrorV1::InvalidProfile);
    }
    let dimensions = MaskedRelaxedDimensionsV1::from_shape(shape)?;
    if strict_public_inputs
        .iter()
        .any(|inputs| inputs.len() != dimensions.public_input_count)
    {
        return Err(MaskedRelaxedErrorV1::InvalidProfile);
    }
    composition_transcript(
        domain,
        context_frame,
        shape,
        strict_public_inputs,
        dimensions,
    )
}

fn composition_transcript(
    domain: &'static [u8],
    context_frame: &[u8],
    shape: &Shape,
    strict_public_inputs: &[Vec<Scalar>],
    dimensions: MaskedRelaxedDimensionsV1,
) -> Result<VegaTranscriptV1, MaskedRelaxedErrorV1> {
    let mut frame = Vec::with_capacity(
        context_frame
            .len()
            .saturating_add(strict_public_inputs.len() * dimensions.public_input_count * 32)
            .saturating_add(128),
    );
    push_frame(&mut frame, 0, context_frame)?;
    for (tag, value) in [
        (1, shape.variable_count()),
        (2, shape.constraint_count()),
        (3, shape.public_input_count()),
        (4, MASKED_RELAXED_COMMITMENT_COLUMNS_V1),
        (5, strict_public_inputs.len()),
    ] {
        push_frame(
            &mut frame,
            tag,
            &u64::try_from(value)
                .map_err(|_| MaskedRelaxedErrorV1::InvalidProfile)?
                .to_be_bytes(),
        )?;
    }
    for (index, inputs) in strict_public_inputs.iter().enumerate() {
        let mut encoded = Vec::with_capacity(4 + inputs.len() * 32);
        encoded.extend_from_slice(
            &u32::try_from(index)
                .map_err(|_| MaskedRelaxedErrorV1::InvalidProfile)?
                .to_be_bytes(),
        );
        for input in inputs {
            encoded.extend_from_slice(&input.to_be_bytes());
        }
        push_frame(&mut frame, 6, &encoded)?;
    }
    let mut transcript = VegaTranscriptV1::new_neutron_nova();
    transcript
        .domain_separator(domain)
        .map_err(|_| MaskedRelaxedErrorV1::InvalidProfile)?;
    transcript
        .absorb_raw(b"masked_relaxed_r1cs_release_v1", &frame)
        .map_err(|_| MaskedRelaxedErrorV1::InvalidProfile)?;
    Ok(transcript)
}

fn push_frame(output: &mut Vec<u8>, tag: u8, value: &[u8]) -> Result<(), MaskedRelaxedErrorV1> {
    output.push(tag);
    output.extend_from_slice(
        &u32::try_from(value.len())
            .map_err(|_| MaskedRelaxedErrorV1::InvalidProfile)?
            .to_be_bytes(),
    );
    output.extend_from_slice(value);
    Ok(())
}

fn sample_scalar<R: MaskedRelaxedRandomSourceV1>(
    random: &mut R,
) -> Result<Scalar, MaskedRelaxedErrorV1> {
    let mut wide = [0_u8; 64];
    let result = random.fill_bytes(&mut wide);
    if let Err(error) = result {
        wide.fill(0);
        return Err(error.into());
    }
    let scalar = Scalar::from_uniform_le_bytes(wide);
    wide.fill(0);
    Ok(scalar)
}

fn sample_nonzero_scalar<R: MaskedRelaxedRandomSourceV1>(
    random: &mut R,
) -> Result<Scalar, MaskedRelaxedErrorV1> {
    for _ in 0..RANDOM_HEALTH_RETRIES {
        let scalar = sample_scalar(random)?;
        if !scalar.is_zero() {
            return Ok(scalar);
        }
    }
    Err(MaskedRelaxedErrorV1::DegenerateRandomness)
}

fn validate_random_health<R: MaskedRelaxedRandomSourceV1>(
    random: &mut R,
) -> Result<(), MaskedRelaxedErrorV1> {
    let first = SecretScalar::new(sample_nonzero_scalar(random)?);
    for _ in 0..RANDOM_HEALTH_RETRIES {
        let candidate = SecretScalar::new(sample_nonzero_scalar(random)?);
        if *candidate != *first {
            return Ok(());
        }
    }
    Err(MaskedRelaxedErrorV1::DegenerateRandomness)
}

fn sample_scalars<R: MaskedRelaxedRandomSourceV1>(
    random: &mut R,
    count: usize,
) -> Result<Vec<Scalar>, MaskedRelaxedErrorV1> {
    let mut values = SecretScalars::new(Vec::with_capacity(count));
    for _ in 0..count {
        values.push(sample_scalar(random)?);
    }
    Ok(values.to_vec())
}

fn sample_nonzero_scalars<R: MaskedRelaxedRandomSourceV1>(
    random: &mut R,
    count: usize,
) -> Result<Vec<Scalar>, MaskedRelaxedErrorV1> {
    let mut values = SecretScalars::new(Vec::with_capacity(count));
    for _ in 0..count {
        values.push(sample_nonzero_scalar(random)?);
    }
    Ok(values.to_vec())
}

fn validate_count(count: usize) -> Result<(), MaskedRelaxedErrorV1> {
    if count == 0 || count > MAX_MASKED_RELAXED_STRICT_INSTANCES_V1 {
        return Err(MaskedRelaxedErrorV1::InvalidInstanceCount {
            actual: count,
            max: MAX_MASKED_RELAXED_STRICT_INSTANCES_V1,
        });
    }
    Ok(())
}

fn validate_worker_count(worker_count: usize) -> Result<(), MaskedRelaxedErrorV1> {
    if worker_count == 0 || worker_count > MAX_COMMITMENT_WORKERS {
        return Err(MaskedRelaxedErrorV1::InvalidWorkerCount {
            actual: worker_count,
            max: MAX_COMMITMENT_WORKERS,
        });
    }
    Ok(())
}

fn scalar_array_to_wire<const N: usize>(
    scalars: &[Scalar],
) -> Result<[VegaScalarWireV1; N], MaskedRelaxedErrorV1> {
    scalars
        .iter()
        .copied()
        .map(VegaScalarWireV1::from_scalar)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| MaskedRelaxedErrorV1::InvalidProfile)
}

fn wire_to_scalar(scalar: VegaScalarWireV1) -> Result<Scalar, MaskedRelaxedErrorV1> {
    scalar
        .to_scalar()
        .map_err(|_| MaskedRelaxedErrorV1::InvalidProofEncoding)
}

fn wire_to_scalars(scalars: &[VegaScalarWireV1]) -> Result<Vec<Scalar>, MaskedRelaxedErrorV1> {
    scalars.iter().copied().map(wire_to_scalar).collect()
}

fn wire_to_scalar_array<const N: usize>(
    scalars: &[VegaScalarWireV1; N],
) -> Result<[Scalar; N], MaskedRelaxedErrorV1> {
    wire_to_scalars(scalars)?
        .try_into()
        .map_err(|_| MaskedRelaxedErrorV1::InvalidProofEncoding)
}

struct SecretScalar(Scalar);

impl SecretScalar {
    fn new(value: Scalar) -> Self {
        Self(value)
    }
}

impl Deref for SecretScalar {
    type Target = Scalar;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for SecretScalar {
    fn drop(&mut self) {
        self.0 = Scalar::zero();
    }
}

struct SecretScalars(Vec<Scalar>);

fn clear_secret_scalar_slice_v1(values: &mut [Scalar]) {
    let values = core::hint::black_box(values);
    for value in values.iter_mut() {
        value.clear_secret();
    }
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    let _ = core::hint::black_box(&mut *values);
}

#[cfg(test)]
std::thread_local! {
    static SECRET_SCALARS_ZEROIZED_DROPS_V1: core::cell::Cell<usize> = const {
        core::cell::Cell::new(0)
    };
    static SECRET_WITNESS_ZEROIZED_DROPS_V1: core::cell::Cell<usize> = const {
        core::cell::Cell::new(0)
    };
    static SECRET_RELAXED_WITNESS_ZEROIZED_CLEARS_V1: core::cell::Cell<usize> = const {
        core::cell::Cell::new(0)
    };
    static PANIC_AFTER_PRECOMPUTED_WITNESS_HANDOFF_V1: core::cell::Cell<bool> = const {
        core::cell::Cell::new(false)
    };
}

#[cfg(test)]
fn secret_scalars_zeroized_drop_count_v1() -> usize {
    SECRET_SCALARS_ZEROIZED_DROPS_V1
        .try_with(core::cell::Cell::get)
        .unwrap_or(0)
}

#[cfg(test)]
fn secret_witness_zeroized_drop_count_v1() -> usize {
    SECRET_WITNESS_ZEROIZED_DROPS_V1
        .try_with(core::cell::Cell::get)
        .unwrap_or(0)
}

#[cfg(test)]
fn secret_relaxed_witness_zeroized_clear_count_v1() -> usize {
    SECRET_RELAXED_WITNESS_ZEROIZED_CLEARS_V1
        .try_with(core::cell::Cell::get)
        .unwrap_or(0)
}

#[cfg(test)]
fn arm_panic_after_precomputed_witness_handoff_v1() {
    let _ = PANIC_AFTER_PRECOMPUTED_WITNESS_HANDOFF_V1
        .try_with(|armed| armed.set(true));
}

#[cfg(test)]
fn maybe_panic_after_precomputed_witness_handoff_v1() {
    let should_panic = PANIC_AFTER_PRECOMPUTED_WITNESS_HANDOFF_V1
        .try_with(|armed| armed.replace(false))
        .unwrap_or(false);
    assert!(
        !should_panic,
        "injected panic after precomputed folded-witness handoff"
    );
}

impl SecretScalars {
    fn new(values: Vec<Scalar>) -> Self {
        Self(values)
    }
}

impl Deref for SecretScalars {
    type Target = Vec<Scalar>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SecretScalars {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for SecretScalars {
    fn drop(&mut self) {
        let values = core::hint::black_box(&mut self.0);
        clear_secret_scalar_slice_v1(values);
        #[cfg(test)]
        if values.iter().all(|value| value.is_zero()) {
            let _ = SECRET_SCALARS_ZEROIZED_DROPS_V1
                .try_with(|drops| drops.set(drops.get().saturating_add(1)));
        }
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut *values);
    }
}

struct SecretWitness(Witness);

impl SecretWitness {
    fn new(witness: Witness) -> Self {
        Self(witness)
    }
}

impl Deref for SecretWitness {
    type Target = Witness;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for SecretWitness {
    fn drop(&mut self) {
        let witness = core::hint::black_box(&mut self.0);
        clear_secret_scalar_slice_v1(&mut witness.values);
        clear_secret_scalar_slice_v1(&mut witness.blindings);
        #[cfg(test)]
        if witness.values.iter().all(|value| value.is_zero())
            && witness.blindings.iter().all(|value| value.is_zero())
        {
            let _ = SECRET_WITNESS_ZEROIZED_DROPS_V1
                .try_with(|drops| drops.set(drops.get().saturating_add(1)));
        }
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut *witness);
    }
}

struct SecretRelaxedWitness(RelaxedWitness);

impl SecretRelaxedWitness {
    fn new(witness: RelaxedWitness) -> Self {
        Self(witness)
    }

    fn replace(&mut self, witness: RelaxedWitness) {
        self.clear_secret();
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        self.0 = witness;
        let _ = core::hint::black_box(&mut self.0);
    }

    fn clear_secret(&mut self) {
        let witness = core::hint::black_box(&mut self.0);
        clear_secret_scalar_slice_v1(&mut witness.values);
        clear_secret_scalar_slice_v1(&mut witness.witness_blindings);
        clear_secret_scalar_slice_v1(&mut witness.error);
        clear_secret_scalar_slice_v1(&mut witness.error_blindings);
        #[cfg(test)]
        if witness.values.iter().all(|value| value.is_zero())
            && witness
                .witness_blindings
                .iter()
                .all(|value| value.is_zero())
            && witness.error.iter().all(|value| value.is_zero())
            && witness
                .error_blindings
                .iter()
                .all(|value| value.is_zero())
        {
            let _ = SECRET_RELAXED_WITNESS_ZEROIZED_CLEARS_V1
                .try_with(|clears| clears.set(clears.get().saturating_add(1)));
        }
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut *witness);
    }
}

impl Deref for SecretRelaxedWitness {
    type Target = RelaxedWitness;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for SecretRelaxedWitness {
    fn drop(&mut self) {
        self.clear_secret();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vega::r1cs::SparseMatrix;

    const TEST_DOMAIN: &[u8] = b"iroha.test.masked-relaxed.precomputed";
    const TEST_CONTEXT: &[u8] = b"ordered-batch-context-v1";
    const TEST_KEY_LABEL: &[u8] = b"iroha.test.masked-relaxed.precomputed.key";

    struct ConstantRandom(u8);

    impl MaskedRelaxedRandomSourceV1 for ConstantRandom {
        fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
            destination.fill(self.0);
            Ok(())
        }
    }

    struct FailureRandom;

    impl MaskedRelaxedRandomSourceV1 for FailureRandom {
        fn fill_bytes(
            &mut self,
            _destination: &mut [u8],
        ) -> Result<(), MaskedRelaxedRandomErrorV1> {
            Err(MaskedRelaxedRandomErrorV1::Unavailable)
        }
    }

    struct CounterRandom(u64);

    impl MaskedRelaxedRandomSourceV1 for CounterRandom {
        fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
            for chunk in destination.chunks_mut(8) {
                self.0 = self
                    .0
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                chunk.copy_from_slice(&self.0.to_le_bytes()[..chunk.len()]);
            }
            Ok(())
        }
    }

    struct PrecomputedFixture {
        shape: Shape,
        mask: RelaxedInstance,
        strict: Vec<Instance>,
        folds: Vec<NovaNifs>,
        folded_instance: RelaxedInstance,
        folded_witness: RelaxedWitness,
    }

    fn s(value: u64) -> Scalar {
        Scalar::from_u64(value)
    }

    fn precomputed_test_shape() -> Shape {
        let variables = MASKED_RELAXED_COMMITMENT_COLUMNS_V1;
        let constraints = MASKED_RELAXED_COMMITMENT_COLUMNS_V1;
        let columns = variables + 2;
        // The only non-empty row enforces W[0] * u = x[0]. Empty rows
        // canonicalize to 0 * 0 = 0, preserving the released power-of-two
        // commitment geometry without adding irrelevant test constraints.
        let a = SparseMatrix::new(constraints, columns, &[(0, 0, s(1))]).expect("canonical A");
        let b =
            SparseMatrix::new(constraints, columns, &[(0, variables, s(1))]).expect("canonical B");
        let c = SparseMatrix::new(constraints, columns, &[(0, variables + 1, s(1))])
            .expect("canonical C");
        Shape::new(constraints, variables, 1, a, b, c).expect("valid test shape")
    }

    fn precomputed_fixture(values: &[u64]) -> PrecomputedFixture {
        let shape = precomputed_test_shape();
        let dimensions = MaskedRelaxedDimensionsV1::from_shape(&shape).expect("dimensions");
        let key = CommitmentKey::derive(TEST_KEY_LABEL, MASKED_RELAXED_COMMITMENT_COLUMNS_V1)
            .expect("commitment key");
        let mut random = CounterRandom(0x6a09_e667_f3bc_c909);
        let (mask, mut folded_witness) =
            sample_relaxed_mask(&mut random, &shape, &key, dimensions).expect("fresh mask");
        let public_inputs = values
            .iter()
            .map(|value| vec![s(*value)])
            .collect::<Vec<_>>();
        let mut transcript = composition_transcript(
            TEST_DOMAIN,
            TEST_CONTEXT,
            &shape,
            &public_inputs,
            dimensions,
        )
        .expect("composition transcript");
        let mut folded_instance = mask.clone();
        let mut strict = Vec::with_capacity(values.len());
        let mut folds = Vec::with_capacity(values.len());
        for value in values {
            let mut witness_values = vec![Scalar::zero(); shape.variable_count()];
            witness_values[0] = s(*value);
            let witness = Witness {
                values: witness_values,
                blindings: sample_nonzero_scalars(
                    &mut random,
                    dimensions.witness_commitment_points,
                )
                .expect("strict blindings"),
            };
            let instance = Instance {
                witness_commitment: key
                    .commit(&witness.values, &witness.blindings)
                    .expect("strict commitment"),
                public_inputs: vec![s(*value)],
            };
            let cross_term_blindings =
                sample_nonzero_scalars(&mut random, dimensions.error_commitment_points)
                    .expect("cross-term blindings");
            let (fold, next_instance, next_witness) = NovaNifs::prove(
                NovaNifsProverInput {
                    key: &key,
                    shape: &shape,
                    relaxed_instance: &folded_instance,
                    relaxed_witness: &folded_witness,
                    regular_instance: &instance,
                    regular_witness: &witness,
                    cross_term_blindings: &cross_term_blindings,
                },
                &mut transcript,
            )
            .expect("valid precomputed fold");
            folded_instance = next_instance;
            folded_witness = next_witness;
            strict.push(instance);
            folds.push(fold);
        }
        PrecomputedFixture {
            shape,
            mask,
            strict,
            folds,
            folded_instance,
            folded_witness,
        }
    }

    #[test]
    fn random_health_rejects_unavailable_zero_and_constant_sources() {
        assert_eq!(
            validate_random_health(&mut FailureRandom),
            Err(MaskedRelaxedErrorV1::Random(
                MaskedRelaxedRandomErrorV1::Unavailable
            ))
        );
        assert_eq!(
            validate_random_health(&mut ConstantRandom(0)),
            Err(MaskedRelaxedErrorV1::DegenerateRandomness)
        );
        assert_eq!(
            validate_random_health(&mut ConstantRandom(1)),
            Err(MaskedRelaxedErrorV1::DegenerateRandomness)
        );
    }

    #[test]
    fn precomputed_history_round_trips_and_binds_ordered_public_inputs() {
        let fixture = precomputed_fixture(&[3, 5]);
        let proof = prove_precomputed_masked_relaxed_v1(
            TEST_DOMAIN,
            TEST_CONTEXT,
            TEST_KEY_LABEL,
            &fixture.shape,
            &fixture.mask,
            &fixture.strict,
            &fixture.folds,
            fixture.folded_witness.clone(),
            1,
        )
        .expect("full history produces terminal proof");
        let public_inputs = fixture
            .strict
            .iter()
            .map(|instance| instance.public_inputs.clone())
            .collect::<Vec<_>>();
        verify_masked_relaxed_v1(
            TEST_DOMAIN,
            TEST_CONTEXT,
            TEST_KEY_LABEL,
            &fixture.shape,
            &public_inputs,
            &proof,
        )
        .expect("canonical history verifies");
        assert_eq!(
            verify_and_replay_masked_relaxed_v1(
                TEST_DOMAIN,
                TEST_CONTEXT,
                TEST_KEY_LABEL,
                &fixture.shape,
                &public_inputs,
                &proof,
            )
            .expect("canonical history replays"),
            fixture.folded_instance
        );

        let mut reordered = public_inputs.clone();
        reordered.swap(0, 1);
        assert_eq!(
            verify_masked_relaxed_v1(
                TEST_DOMAIN,
                TEST_CONTEXT,
                TEST_KEY_LABEL,
                &fixture.shape,
                &reordered,
                &proof,
            ),
            Err(MaskedRelaxedErrorV1::VerificationFailed)
        );
        let mut changed_context = TEST_CONTEXT.to_vec();
        changed_context.push(0);
        assert_eq!(
            verify_masked_relaxed_v1(
                TEST_DOMAIN,
                &changed_context,
                TEST_KEY_LABEL,
                &fixture.shape,
                &public_inputs,
                &proof,
            ),
            Err(MaskedRelaxedErrorV1::VerificationFailed)
        );
    }

    #[test]
    fn precomputed_history_rejects_splicing_and_final_witness_forgery() {
        let fixture = precomputed_fixture(&[7, 11]);

        let mut reordered_strict = fixture.strict.clone();
        reordered_strict.swap(0, 1);
        assert!(
            prove_precomputed_masked_relaxed_v1(
                TEST_DOMAIN,
                TEST_CONTEXT,
                TEST_KEY_LABEL,
                &fixture.shape,
                &fixture.mask,
                &reordered_strict,
                &fixture.folds,
                fixture.folded_witness.clone(),
                1,
            )
            .is_err()
        );

        let mut spliced_folds = fixture.folds.clone();
        spliced_folds.swap(0, 1);
        assert!(
            prove_precomputed_masked_relaxed_v1(
                TEST_DOMAIN,
                TEST_CONTEXT,
                TEST_KEY_LABEL,
                &fixture.shape,
                &fixture.mask,
                &fixture.strict,
                &spliced_folds,
                fixture.folded_witness.clone(),
                1,
            )
            .is_err()
        );

        assert_eq!(
            prove_precomputed_masked_relaxed_v1(
                TEST_DOMAIN,
                TEST_CONTEXT,
                TEST_KEY_LABEL,
                &fixture.shape,
                &fixture.mask,
                &fixture.strict,
                &fixture.folds[..1],
                fixture.folded_witness.clone(),
                1,
            ),
            Err(MaskedRelaxedErrorV1::InvalidProfile)
        );

        let mut forged_relation = fixture.folded_witness.clone();
        forged_relation.error[0] += Scalar::one();
        assert_eq!(
            prove_precomputed_masked_relaxed_v1(
                TEST_DOMAIN,
                TEST_CONTEXT,
                TEST_KEY_LABEL,
                &fixture.shape,
                &fixture.mask,
                &fixture.strict,
                &fixture.folds,
                forged_relation,
                1,
            ),
            Err(MaskedRelaxedErrorV1::UnsatisfiedWitness)
        );

        let mut forged_blinding = fixture.folded_witness.clone();
        forged_blinding.witness_blindings[0] += Scalar::one();
        assert_eq!(
            prove_precomputed_masked_relaxed_v1(
                TEST_DOMAIN,
                TEST_CONTEXT,
                TEST_KEY_LABEL,
                &fixture.shape,
                &fixture.mask,
                &fixture.strict,
                &fixture.folds,
                forged_blinding,
                1,
            ),
            Err(MaskedRelaxedErrorV1::VerificationFailed)
        );
    }
}
