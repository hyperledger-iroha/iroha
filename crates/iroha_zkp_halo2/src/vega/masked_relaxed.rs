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

pub(super) fn prove_masked_relaxed_v1<R: MaskedRelaxedRandomSourceV1>(
    domain: &'static [u8],
    context_frame: &[u8],
    commitment_key_label: &[u8],
    mut assignments: Vec<CircuitAssignment>,
    worker_count: usize,
    random: &mut R,
) -> Result<MaskedRelaxedProofWireV1, MaskedRelaxedErrorV1> {
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
    let spartan = RelaxedSpartanProof::prove(
        &shape,
        &key,
        &folded_instance,
        &folded_witness,
        &mut transcript,
    )
    .map_err(|_| MaskedRelaxedErrorV1::InvalidProfile)?;
    MaskedRelaxedProofWireV1::from_protocol(&mask_instance, &strict_instances, &folds, &spartan)
}

pub(super) fn verify_masked_relaxed_v1(
    domain: &'static [u8],
    context_frame: &[u8],
    commitment_key_label: &[u8],
    shape: &Shape,
    strict_public_inputs: &[Vec<Scalar>],
    proof: &MaskedRelaxedProofWireV1,
) -> Result<(), MaskedRelaxedErrorV1> {
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
        .map_err(|_| MaskedRelaxedErrorV1::VerificationFailed)
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
        self.0.fill(Scalar::zero());
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
        self.0.values.fill(Scalar::zero());
        self.0.blindings.fill(Scalar::zero());
    }
}

struct SecretRelaxedWitness(RelaxedWitness);

impl SecretRelaxedWitness {
    fn new(witness: RelaxedWitness) -> Self {
        Self(witness)
    }

    fn replace(&mut self, witness: RelaxedWitness) {
        self.0.values.fill(Scalar::zero());
        self.0.witness_blindings.fill(Scalar::zero());
        self.0.error.fill(Scalar::zero());
        self.0.error_blindings.fill(Scalar::zero());
        self.0 = witness;
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
        self.0.values.fill(Scalar::zero());
        self.0.witness_blindings.fill(Scalar::zero());
        self.0.error.fill(Scalar::zero());
        self.0.error_blindings.fill(Scalar::zero());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
