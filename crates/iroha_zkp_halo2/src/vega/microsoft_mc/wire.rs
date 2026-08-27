//! Exact bounded Microsoft Vega-MC proof codec.
use super::super::{
    MAX_VEGA_PROOF_BYTES_V1, VegaCurveError, VegaMdlProofDimensionsV1, VegaT256PointV1 as Point,
    VegaT256ScalarV1 as Scalar, commitment::Commitment,
};
use thiserror::Error;

const LENGTH_BYTES: usize = 8;
const POINT_BYTES: usize = 33;
const SCALAR_BYTES: usize = 32;
/// Failure while decoding or encoding the fixed Microsoft proof representation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(in crate::vega) enum McCodecError {
    /// The payload is truncated, trailing, noncanonical, or dimensionally wrong.
    #[error("invalid canonical Microsoft Vega-MC encoding")]
    InvalidEncoding,
    /// A proof point is not a canonical non-identity T256 point.
    #[error(transparent)]
    Curve(#[from] VegaCurveError),
    /// A bounded canonical owner could not reserve its exact storage.
    #[error("Microsoft Vega-MC codec resource exhausted")]
    ResourceExhausted,
}

pub(super) fn try_vec_with_capacity<T>(capacity: usize) -> Result<Vec<T>, McCodecError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| McCodecError::ResourceExhausted)?;
    Ok(values)
}
/// One row-vector Hyrax commitment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct McCommitment {
    pub(super) points: Vec<Point>,
}
impl McCommitment {
    pub(super) fn to_local(&self) -> Result<Commitment, McCodecError> {
        Commitment::from_points(self.points.clone()).map_err(|_| McCodecError::InvalidEncoding)
    }
}
/// One split application-circuit instance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SplitInstanceWire {
    pub(super) shared: Option<McCommitment>,
    pub(super) precommitted: Option<McCommitment>,
    pub(super) rest: McCommitment,
    pub(super) public_values: Vec<Scalar>,
    pub(super) challenges: Vec<Scalar>,
}
/// Multi-round verifier-circuit instance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct MultiRoundInstanceWire {
    pub(super) commitments: Vec<McCommitment>,
    pub(super) public_values: Vec<Scalar>,
    pub(super) challenges_per_round: Vec<Vec<Scalar>>,
}
/// Relaxed verifier-circuit instance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RelaxedInstanceWire {
    pub(super) witness_commitment: McCommitment,
    pub(super) error_commitment: McCommitment,
    pub(super) public_values: Vec<Scalar>,
    pub(super) relaxation: Scalar,
}
/// Linear inner-product response used by the final Hyrax opening.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LinearIpaWire {
    pub(super) delta: Point,
    pub(super) beta: Point,
    pub(super) responses: Vec<Scalar>,
    pub(super) delta_response: Scalar,
    pub(super) beta_response: Scalar,
}
/// One compressed sum-check polynomial.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CompressedPolynomialWire {
    pub(super) coefficients_except_linear: Vec<Scalar>,
}
/// One complete sum-check transcript.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SumcheckWire {
    pub(super) rounds: Vec<CompressedPolynomialWire>,
}
/// Relaxed-Spartan proof nested in the Microsoft MC proof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RelaxedSpartanWire {
    pub(super) outer_sumcheck: SumcheckWire,
    pub(super) outer_claims: [Scalar; 3],
    pub(super) inner_sumcheck: SumcheckWire,
    pub(super) witness_opening: Vec<Scalar>,
    pub(super) witness_blinding: Scalar,
    pub(super) error_opening: Vec<Scalar>,
    pub(super) error_blinding: Scalar,
}
/// Exact Microsoft `VegaMcZkSNARK` proof object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct McProofWire {
    pub(super) shared_commitment: Option<McCommitment>,
    pub(super) step_instances: Vec<SplitInstanceWire>,
    pub(super) core_instance: SplitInstanceWire,
    pub(super) evaluation_argument: LinearIpaWire,
    pub(super) verifier_instance: MultiRoundInstanceWire,
    pub(super) nova_cross_term: McCommitment,
    pub(super) random_instance: RelaxedInstanceWire,
    pub(super) relaxed_spartan: RelaxedSpartanWire,
}
impl McProofWire {
    /// Decode one proof after validating every key-derived length before allocation.
    pub(super) fn decode(
        bytes: &[u8],
        dimensions: &VegaMdlProofDimensionsV1,
    ) -> Result<Self, McCodecError> {
        if bytes.len() > MAX_VEGA_PROOF_BYTES_V1 || proof_encoded_len(dimensions)? != bytes.len() {
            return Err(McCodecError::InvalidEncoding);
        }
        let mut reader = Reader::new(bytes);
        let shared_commitment = reader.option_commitment(
            (dimensions.shared_commitment_points != 0)
                .then_some(dimensions.shared_commitment_points),
        )?;
        reader.expect_len(dimensions.num_steps)?;
        let mut step_instances = try_vec_with_capacity(dimensions.num_steps)?;
        for _ in 0..dimensions.num_steps {
            step_instances.push(read_split_instance(
                &mut reader,
                dimensions.step_precommitted_points,
                dimensions.step_rest_points,
                dimensions.step_public_values,
                dimensions.step_challenges,
            )?);
        }
        let core_instance = read_split_instance(
            &mut reader,
            dimensions.core_precommitted_points,
            dimensions.core_rest_points,
            dimensions.core_public_values,
            dimensions.core_challenges,
        )?;
        let evaluation_argument = LinearIpaWire {
            delta: reader.point()?,
            beta: reader.point()?,
            responses: reader.scalar_vec(dimensions.evaluation_response_scalars)?,
            delta_response: reader.scalar()?,
            beta_response: reader.scalar()?,
        };
        reader.expect_len(dimensions.verifier_round_commitment_points.len())?;
        let mut commitments =
            try_vec_with_capacity(dimensions.verifier_round_commitment_points.len())?;
        for points in &dimensions.verifier_round_commitment_points {
            commitments.push(reader.commitment(*points)?);
        }
        let public_values = reader.scalar_vec(dimensions.verifier_public_values)?;
        reader.expect_len(dimensions.verifier_challenges_per_round.len())?;
        let mut challenges_per_round =
            try_vec_with_capacity(dimensions.verifier_challenges_per_round.len())?;
        for challenges in &dimensions.verifier_challenges_per_round {
            challenges_per_round.push(reader.scalar_vec(*challenges)?);
        }
        let verifier_instance = MultiRoundInstanceWire {
            commitments,
            public_values,
            challenges_per_round,
        };
        let nova_cross_term = reader.commitment(dimensions.nova_cross_term_points)?;
        let random_instance = RelaxedInstanceWire {
            witness_commitment: reader.commitment(dimensions.random_witness_commitment_points)?,
            error_commitment: reader.commitment(dimensions.random_error_commitment_points)?,
            public_values: reader.scalar_vec(dimensions.random_public_values)?,
            relaxation: reader.scalar()?,
        };
        let relaxed_spartan = RelaxedSpartanWire {
            outer_sumcheck: reader.sumcheck(
                dimensions.relaxed_outer_rounds,
                dimensions.relaxed_outer_coefficients,
            )?,
            outer_claims: [reader.scalar()?, reader.scalar()?, reader.scalar()?],
            inner_sumcheck: reader.sumcheck(
                dimensions.relaxed_inner_rounds,
                dimensions.relaxed_inner_coefficients,
            )?,
            witness_opening: reader.scalar_vec(dimensions.relaxed_opening_scalars)?,
            witness_blinding: reader.scalar()?,
            error_opening: reader.scalar_vec(dimensions.relaxed_opening_scalars)?,
            error_blinding: reader.scalar()?,
        };
        reader.finish()?;
        Ok(Self {
            shared_commitment,
            step_instances,
            core_instance,
            evaluation_argument,
            verifier_instance,
            nova_cross_term,
            random_instance,
            relaxed_spartan,
        })
    }
    /// Encode the exact fixed-little-endian compatibility representation.
    pub(super) fn encode(&self) -> Result<Vec<u8>, McCodecError> {
        let encoded_len = self.encoded_len()?;
        if encoded_len > MAX_VEGA_PROOF_BYTES_V1 {
            return Err(McCodecError::InvalidEncoding);
        }
        let mut output = try_vec_with_capacity(encoded_len)?;
        write_option_commitment(&mut output, self.shared_commitment.as_ref())?;
        write_len(&mut output, self.step_instances.len())?;
        for instance in &self.step_instances {
            write_split_instance(&mut output, instance)?;
        }
        write_split_instance(&mut output, &self.core_instance)?;
        write_point(&mut output, self.evaluation_argument.delta)?;
        write_point(&mut output, self.evaluation_argument.beta)?;
        write_scalars(&mut output, &self.evaluation_argument.responses)?;
        write_scalar(&mut output, self.evaluation_argument.delta_response);
        write_scalar(&mut output, self.evaluation_argument.beta_response);
        write_len(&mut output, self.verifier_instance.commitments.len())?;
        for commitment in &self.verifier_instance.commitments {
            write_commitment(&mut output, commitment)?;
        }
        write_scalars(&mut output, &self.verifier_instance.public_values)?;
        write_len(
            &mut output,
            self.verifier_instance.challenges_per_round.len(),
        )?;
        for challenges in &self.verifier_instance.challenges_per_round {
            write_scalars(&mut output, challenges)?;
        }
        write_commitment(&mut output, &self.nova_cross_term)?;
        write_commitment(&mut output, &self.random_instance.witness_commitment)?;
        write_commitment(&mut output, &self.random_instance.error_commitment)?;
        write_scalars(&mut output, &self.random_instance.public_values)?;
        write_scalar(&mut output, self.random_instance.relaxation);
        write_sumcheck(&mut output, &self.relaxed_spartan.outer_sumcheck)?;
        for claim in self.relaxed_spartan.outer_claims {
            write_scalar(&mut output, claim);
        }
        write_sumcheck(&mut output, &self.relaxed_spartan.inner_sumcheck)?;
        write_scalars(&mut output, &self.relaxed_spartan.witness_opening)?;
        write_scalar(&mut output, self.relaxed_spartan.witness_blinding);
        write_scalars(&mut output, &self.relaxed_spartan.error_opening)?;
        write_scalar(&mut output, self.relaxed_spartan.error_blinding);
        if output.len() != encoded_len {
            return Err(McCodecError::InvalidEncoding);
        }
        Ok(output)
    }

    fn encoded_len(&self) -> Result<usize, McCodecError> {
        let step_instances = self
            .step_instances
            .iter()
            .try_fold(LENGTH_BYTES, |length, instance| {
                checked_add(length, split_instance_encoded_len(instance)?)
            })?;
        let verifier_commitments = self.verifier_instance.commitments.iter().try_fold(
            LENGTH_BYTES,
            |length, commitment| {
                checked_add(length, commitment_encoded_len(commitment.points.len())?)
            },
        )?;
        let verifier_challenges = self
            .verifier_instance
            .challenges_per_round
            .iter()
            .try_fold(LENGTH_BYTES, |length, challenges| {
                checked_add(length, scalar_vec_encoded_len(challenges.len())?)
            })?;
        checked_sum(&[
            option_commitment_encoded_len(
                self.shared_commitment
                    .as_ref()
                    .map(|commitment| commitment.points.len()),
            )?,
            step_instances,
            split_instance_encoded_len(&self.core_instance)?,
            POINT_BYTES,
            POINT_BYTES,
            scalar_vec_encoded_len(self.evaluation_argument.responses.len())?,
            SCALAR_BYTES,
            SCALAR_BYTES,
            verifier_commitments,
            scalar_vec_encoded_len(self.verifier_instance.public_values.len())?,
            verifier_challenges,
            commitment_encoded_len(self.nova_cross_term.points.len())?,
            commitment_encoded_len(self.random_instance.witness_commitment.points.len())?,
            commitment_encoded_len(self.random_instance.error_commitment.points.len())?,
            scalar_vec_encoded_len(self.random_instance.public_values.len())?,
            SCALAR_BYTES,
            sumcheck_encoded_len(&self.relaxed_spartan.outer_sumcheck)?,
            3 * SCALAR_BYTES,
            sumcheck_encoded_len(&self.relaxed_spartan.inner_sumcheck)?,
            scalar_vec_encoded_len(self.relaxed_spartan.witness_opening.len())?,
            SCALAR_BYTES,
            scalar_vec_encoded_len(self.relaxed_spartan.error_opening.len())?,
            SCALAR_BYTES,
        ])
    }
}

fn proof_encoded_len(dimensions: &VegaMdlProofDimensionsV1) -> Result<usize, McCodecError> {
    let step_instance = split_instance_dimensions_encoded_len(
        dimensions.step_precommitted_points,
        dimensions.step_rest_points,
        dimensions.step_public_values,
        dimensions.step_challenges,
    )?;
    let step_instances = checked_add(
        LENGTH_BYTES,
        checked_mul(dimensions.num_steps, step_instance)?,
    )?;
    let verifier_commitments = dimensions
        .verifier_round_commitment_points
        .iter()
        .try_fold(LENGTH_BYTES, |length, points| {
            checked_add(length, commitment_encoded_len(*points)?)
        })?;
    let verifier_challenges = dimensions
        .verifier_challenges_per_round
        .iter()
        .try_fold(LENGTH_BYTES, |length, challenges| {
            checked_add(length, scalar_vec_encoded_len(*challenges)?)
        })?;
    let outer_sumcheck = sumcheck_dimensions_encoded_len(
        dimensions.relaxed_outer_rounds,
        dimensions.relaxed_outer_coefficients,
    )?;
    let inner_sumcheck = sumcheck_dimensions_encoded_len(
        dimensions.relaxed_inner_rounds,
        dimensions.relaxed_inner_coefficients,
    )?;
    let encoded_len = checked_sum(&[
        option_commitment_encoded_len(
            (dimensions.shared_commitment_points != 0)
                .then_some(dimensions.shared_commitment_points),
        )?,
        step_instances,
        split_instance_dimensions_encoded_len(
            dimensions.core_precommitted_points,
            dimensions.core_rest_points,
            dimensions.core_public_values,
            dimensions.core_challenges,
        )?,
        POINT_BYTES,
        POINT_BYTES,
        scalar_vec_encoded_len(dimensions.evaluation_response_scalars)?,
        SCALAR_BYTES,
        SCALAR_BYTES,
        verifier_commitments,
        scalar_vec_encoded_len(dimensions.verifier_public_values)?,
        verifier_challenges,
        commitment_encoded_len(dimensions.nova_cross_term_points)?,
        commitment_encoded_len(dimensions.random_witness_commitment_points)?,
        commitment_encoded_len(dimensions.random_error_commitment_points)?,
        scalar_vec_encoded_len(dimensions.random_public_values)?,
        SCALAR_BYTES,
        outer_sumcheck,
        3 * SCALAR_BYTES,
        inner_sumcheck,
        scalar_vec_encoded_len(dimensions.relaxed_opening_scalars)?,
        SCALAR_BYTES,
        scalar_vec_encoded_len(dimensions.relaxed_opening_scalars)?,
        SCALAR_BYTES,
    ])?;
    if encoded_len > MAX_VEGA_PROOF_BYTES_V1 {
        return Err(McCodecError::InvalidEncoding);
    }
    Ok(encoded_len)
}

fn split_instance_dimensions_encoded_len(
    precommitted_points: usize,
    rest_points: usize,
    public_values: usize,
    challenges: usize,
) -> Result<usize, McCodecError> {
    checked_sum(&[
        1,
        option_commitment_encoded_len((precommitted_points != 0).then_some(precommitted_points))?,
        commitment_encoded_len(rest_points)?,
        scalar_vec_encoded_len(public_values)?,
        scalar_vec_encoded_len(challenges)?,
    ])
}

fn split_instance_encoded_len(instance: &SplitInstanceWire) -> Result<usize, McCodecError> {
    checked_sum(&[
        option_commitment_encoded_len(
            instance
                .shared
                .as_ref()
                .map(|commitment| commitment.points.len()),
        )?,
        option_commitment_encoded_len(
            instance
                .precommitted
                .as_ref()
                .map(|commitment| commitment.points.len()),
        )?,
        commitment_encoded_len(instance.rest.points.len())?,
        scalar_vec_encoded_len(instance.public_values.len())?,
        scalar_vec_encoded_len(instance.challenges.len())?,
    ])
}

fn option_commitment_encoded_len(points: Option<usize>) -> Result<usize, McCodecError> {
    points.map_or(Ok(1), |points| {
        checked_add(1, commitment_encoded_len(points)?)
    })
}

fn commitment_encoded_len(points: usize) -> Result<usize, McCodecError> {
    checked_add(LENGTH_BYTES, checked_mul(points, POINT_BYTES)?)
}

fn scalar_vec_encoded_len(scalars: usize) -> Result<usize, McCodecError> {
    checked_add(LENGTH_BYTES, checked_mul(scalars, SCALAR_BYTES)?)
}

fn sumcheck_dimensions_encoded_len(
    rounds: usize,
    coefficients: usize,
) -> Result<usize, McCodecError> {
    checked_add(
        LENGTH_BYTES,
        checked_mul(rounds, scalar_vec_encoded_len(coefficients)?)?,
    )
}

fn sumcheck_encoded_len(sumcheck: &SumcheckWire) -> Result<usize, McCodecError> {
    sumcheck
        .rounds
        .iter()
        .try_fold(LENGTH_BYTES, |length, round| {
            checked_add(
                length,
                scalar_vec_encoded_len(round.coefficients_except_linear.len())?,
            )
        })
}

fn checked_sum(lengths: &[usize]) -> Result<usize, McCodecError> {
    lengths
        .iter()
        .try_fold(0_usize, |total, length| checked_add(total, *length))
}

fn checked_add(left: usize, right: usize) -> Result<usize, McCodecError> {
    left.checked_add(right).ok_or(McCodecError::InvalidEncoding)
}

fn checked_mul(left: usize, right: usize) -> Result<usize, McCodecError> {
    left.checked_mul(right).ok_or(McCodecError::InvalidEncoding)
}
fn read_split_instance(
    reader: &mut Reader<'_>,
    precommitted_points: usize,
    rest_points: usize,
    public_values: usize,
    challenges: usize,
) -> Result<SplitInstanceWire, McCodecError> {
    Ok(SplitInstanceWire {
        shared: reader.option_commitment(None)?,
        precommitted: reader
            .option_commitment((precommitted_points != 0).then_some(precommitted_points))?,
        rest: reader.commitment(rest_points)?,
        public_values: reader.scalar_vec(public_values)?,
        challenges: reader.scalar_vec(challenges)?,
    })
}
fn write_split_instance(
    output: &mut Vec<u8>,
    instance: &SplitInstanceWire,
) -> Result<(), McCodecError> {
    write_option_commitment(output, instance.shared.as_ref())?;
    write_option_commitment(output, instance.precommitted.as_ref())?;
    write_commitment(output, &instance.rest)?;
    write_scalars(output, &instance.public_values)?;
    write_scalars(output, &instance.challenges)
}
pub(super) struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Reader<'a> {
    pub(super) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    pub(super) fn finish(self) -> Result<(), McCodecError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(McCodecError::InvalidEncoding)
        }
    }
    pub(super) fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }
    pub(super) fn require_remaining_elements(
        &self,
        count: usize,
        element_bytes: usize,
    ) -> Result<(), McCodecError> {
        if count
            .checked_mul(element_bytes)
            .is_none_or(|bytes| bytes > self.remaining())
        {
            return Err(McCodecError::InvalidEncoding);
        }
        Ok(())
    }
    pub(super) fn take(&mut self, count: usize) -> Result<&'a [u8], McCodecError> {
        let end = self
            .offset
            .checked_add(count)
            .filter(|end| *end <= self.bytes.len())
            .ok_or(McCodecError::InvalidEncoding)?;
        let bytes = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(bytes)
    }
    pub(super) fn encoded_len(&mut self) -> Result<usize, McCodecError> {
        usize::try_from(u64::from_le_bytes(
            self.take(8)?
                .try_into()
                .map_err(|_| McCodecError::InvalidEncoding)?,
        ))
        .map_err(|_| McCodecError::InvalidEncoding)
    }
    fn expect_len(&mut self, expected: usize) -> Result<(), McCodecError> {
        if self.encoded_len()? == expected {
            Ok(())
        } else {
            Err(McCodecError::InvalidEncoding)
        }
    }
    pub(super) fn scalar(&mut self) -> Result<Scalar, McCodecError> {
        let bytes = self
            .take(SCALAR_BYTES)?
            .try_into()
            .map_err(|_| McCodecError::InvalidEncoding)?;
        Scalar::from_le_bytes_exact(bytes).map_err(|_| McCodecError::InvalidEncoding)
    }
    pub(super) fn point(&mut self) -> Result<Point, McCodecError> {
        Point::from_non_identity_wire_bytes_exact(self.take(POINT_BYTES)?).map_err(Into::into)
    }
    fn commitment(&mut self, points: usize) -> Result<McCommitment, McCodecError> {
        self.expect_len(points)?;
        self.require_remaining_elements(points, POINT_BYTES)?;
        let mut decoded = try_vec_with_capacity(points)?;
        for _ in 0..points {
            decoded.push(self.point()?);
        }
        Ok(McCommitment { points: decoded })
    }
    fn option_commitment(
        &mut self,
        expected: Option<usize>,
    ) -> Result<Option<McCommitment>, McCodecError> {
        match (self.take(1)?[0], expected) {
            (0, None) => Ok(None),
            (1, Some(points)) => self.commitment(points).map(Some),
            _ => Err(McCodecError::InvalidEncoding),
        }
    }
    fn scalar_vec(&mut self, scalars: usize) -> Result<Vec<Scalar>, McCodecError> {
        self.expect_len(scalars)?;
        self.require_remaining_elements(scalars, SCALAR_BYTES)?;
        let mut decoded = try_vec_with_capacity(scalars)?;
        for _ in 0..scalars {
            decoded.push(self.scalar()?);
        }
        Ok(decoded)
    }
    fn sumcheck(
        &mut self,
        rounds: usize,
        coefficients: usize,
    ) -> Result<SumcheckWire, McCodecError> {
        self.expect_len(rounds)?;
        let round_bytes = scalar_vec_encoded_len(coefficients)?;
        self.require_remaining_elements(rounds, round_bytes)?;
        let mut decoded = try_vec_with_capacity(rounds)?;
        for _ in 0..rounds {
            decoded.push(CompressedPolynomialWire {
                coefficients_except_linear: self.scalar_vec(coefficients)?,
            });
        }
        Ok(SumcheckWire { rounds: decoded })
    }
}
pub(super) fn write_len(output: &mut Vec<u8>, length: usize) -> Result<(), McCodecError> {
    output.extend_from_slice(
        &u64::try_from(length)
            .map_err(|_| McCodecError::InvalidEncoding)?
            .to_le_bytes(),
    );
    Ok(())
}
pub(super) fn write_scalar(output: &mut Vec<u8>, scalar: Scalar) {
    output.extend_from_slice(&scalar.to_le_bytes());
}
fn write_scalars(output: &mut Vec<u8>, scalars: &[Scalar]) -> Result<(), McCodecError> {
    write_len(output, scalars.len())?;
    for scalar in scalars {
        write_scalar(output, *scalar);
    }
    Ok(())
}
pub(super) fn write_point(output: &mut Vec<u8>, point: Point) -> Result<(), McCodecError> {
    output.extend_from_slice(&point.to_non_identity_wire_bytes()?);
    Ok(())
}
fn write_commitment(output: &mut Vec<u8>, commitment: &McCommitment) -> Result<(), McCodecError> {
    write_len(output, commitment.points.len())?;
    for point in &commitment.points {
        write_point(output, *point)?;
    }
    Ok(())
}
fn write_option_commitment(
    output: &mut Vec<u8>,
    commitment: Option<&McCommitment>,
) -> Result<(), McCodecError> {
    match commitment {
        None => output.push(0),
        Some(commitment) => {
            output.push(1);
            write_commitment(output, commitment)?;
        }
    }
    Ok(())
}
fn write_sumcheck(output: &mut Vec<u8>, proof: &SumcheckWire) -> Result<(), McCodecError> {
    write_len(output, proof.rounds.len())?;
    for round in &proof.rounds {
        write_scalars(output, &round.coefficients_except_linear)?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn dimensions() -> VegaMdlProofDimensionsV1 {
        VegaMdlProofDimensionsV1 {
            num_steps: 1,
            shared_variables: 0,
            step_precommitted_variables: 0,
            step_rest_variables: 1,
            core_precommitted_variables: 0,
            core_rest_variables: 1,
            step_constraints: 1,
            step_variables: 1,
            core_constraints: 1,
            core_variables: 1,
            shared_commitment_points: 0,
            step_precommitted_points: 0,
            step_rest_points: 1,
            step_public_values: 1,
            step_challenges: 0,
            core_precommitted_points: 0,
            core_rest_points: 1,
            core_public_values: 1,
            core_challenges: 0,
            evaluation_response_scalars: 1,
            verifier_round_commitment_points: vec![1],
            verifier_public_values: 1,
            verifier_challenges_per_round: vec![0],
            nova_cross_term_points: 1,
            random_witness_commitment_points: 1,
            random_error_commitment_points: 1,
            random_public_values: 1,
            verifier_constraints: 1,
            verifier_variables: 1,
            relaxed_outer_rounds: 1,
            relaxed_outer_coefficients: 3,
            relaxed_inner_rounds: 1,
            relaxed_inner_coefficients: 2,
            relaxed_opening_scalars: 1,
        }
    }
    #[test]
    fn decoder_rejects_length_bombs_before_allocating() {
        let mut proof = vec![0_u8];
        proof.extend_from_slice(&u64::MAX.to_le_bytes());
        assert_eq!(
            McProofWire::decode(&proof, &dimensions()),
            Err(McCodecError::InvalidEncoding)
        );
    }

    #[test]
    fn exact_length_preflight_rejects_overflowing_dimensions() {
        let mut dimensions = dimensions();
        dimensions.num_steps = usize::MAX;
        assert_eq!(
            proof_encoded_len(&dimensions),
            Err(McCodecError::InvalidEncoding)
        );
        assert_eq!(
            McProofWire::decode(&[], &dimensions),
            Err(McCodecError::InvalidEncoding)
        );
    }

    #[test]
    fn fallible_vector_reservation_reports_resource_exhaustion() {
        assert_eq!(
            try_vec_with_capacity::<u8>(usize::MAX),
            Err(McCodecError::ResourceExhausted)
        );
    }
}
