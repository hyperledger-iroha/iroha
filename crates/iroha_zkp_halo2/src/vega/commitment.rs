//! Hyrax row commitments over the canonical T256 group.

use thiserror::Error;

use super::{
    VegaCurveError, VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar, derive_t256_generators_v1,
};

const COMMITMENT_BEGIN: &[u8] = b"poly_commitment_begin";
const COMMITMENT_END: &[u8] = b"poly_commitment_end";

/// Failure while constructing or combining a Hyrax commitment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(super) enum CommitmentError {
    #[error("Vega commitment dimensions do not match")]
    InvalidDimension,
    #[error("Vega commitment key contains a duplicate or inverse point")]
    GeneratorCollision,
    #[error(transparent)]
    Curve(#[from] VegaCurveError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Commitment {
    points: Vec<Point>,
}

impl Commitment {
    pub(super) fn from_points(points: Vec<Point>) -> Result<Self, CommitmentError> {
        if points.is_empty() || points.iter().any(|point| point.is_identity()) {
            return Err(CommitmentError::InvalidDimension);
        }
        Ok(Self { points })
    }

    pub(super) fn points(&self) -> &[Point] {
        &self.points
    }

    pub(super) fn len(&self) -> usize {
        self.points.len()
    }

    pub(super) fn transcript_bytes(&self) -> Result<Vec<u8>, CommitmentError> {
        let point_bytes = self
            .points
            .len()
            .checked_mul(64)
            .ok_or(CommitmentError::InvalidDimension)?;
        let mut bytes =
            Vec::with_capacity(COMMITMENT_BEGIN.len() + point_bytes + COMMITMENT_END.len());
        bytes.extend_from_slice(COMMITMENT_BEGIN);
        for point in &self.points {
            bytes.extend_from_slice(&point.to_transcript_bytes()?);
        }
        bytes.extend_from_slice(COMMITMENT_END);
        Ok(bytes)
    }
}

#[derive(Clone, Debug)]
pub(super) struct CommitmentKey {
    generators: Vec<Point>,
    hiding_generator: Point,
}

impl CommitmentKey {
    pub(super) fn derive(label: &[u8], columns: usize) -> Result<Self, CommitmentError> {
        if columns == 0 {
            return Err(CommitmentError::InvalidDimension);
        }
        let mut points = derive_t256_generators_v1(
            label,
            columns
                .checked_add(1)
                .ok_or(CommitmentError::InvalidDimension)?,
        )?;
        let hiding_generator = points.pop().ok_or(CommitmentError::InvalidDimension)?;
        let key = Self {
            generators: points,
            hiding_generator,
        };
        key.validate_independence()?;
        Ok(key)
    }

    pub(super) fn columns(&self) -> usize {
        self.generators.len()
    }

    pub(super) fn generators(&self) -> &[Point] {
        &self.generators
    }

    pub(super) fn hiding_generator(&self) -> Point {
        self.hiding_generator
    }

    pub(super) fn commit(
        &self,
        values: &[Scalar],
        row_blindings: &[Scalar],
    ) -> Result<Commitment, CommitmentError> {
        if values.is_empty() {
            return Err(CommitmentError::InvalidDimension);
        }
        let row_count = values.len().div_ceil(self.columns());
        if row_blindings.len() != row_count {
            return Err(CommitmentError::InvalidDimension);
        }
        let mut points = Vec::with_capacity(row_count);
        for (row, blinding) in values
            .chunks(self.columns())
            .zip(row_blindings.iter().copied())
        {
            let committed = msm(row, &self.generators[..row.len()])?
                .add(self.hiding_generator.mul_scalar(blinding));
            if committed.is_identity() {
                return Err(CommitmentError::InvalidDimension);
            }
            points.push(committed);
        }
        Commitment::from_points(points)
    }

    fn validate_independence(&self) -> Result<(), CommitmentError> {
        let mut points = self.generators.clone();
        points.push(self.hiding_generator);
        for (index, point) in points.iter().copied().enumerate() {
            if point.is_identity() {
                return Err(CommitmentError::GeneratorCollision);
            }
            for other in points.iter().copied().skip(index + 1) {
                if point == other || point == other.negate() {
                    return Err(CommitmentError::GeneratorCollision);
                }
            }
        }
        Ok(())
    }
}

pub(super) fn msm(scalars: &[Scalar], points: &[Point]) -> Result<Point, CommitmentError> {
    if scalars.len() != points.len() {
        return Err(CommitmentError::InvalidDimension);
    }
    Ok(scalars
        .iter()
        .copied()
        .zip(points.iter().copied())
        .filter(|(scalar, _)| !scalar.is_zero())
        .fold(Point::identity(), |sum, (scalar, point)| {
            sum.add(point.mul_scalar(scalar))
        }))
}

pub(super) fn combine(commitments: &[&Commitment]) -> Result<Commitment, CommitmentError> {
    if commitments.is_empty() {
        return Err(CommitmentError::InvalidDimension);
    }
    let capacity = commitments.iter().try_fold(0_usize, |sum, commitment| {
        sum.checked_add(commitment.len())
            .ok_or(CommitmentError::InvalidDimension)
    })?;
    let mut points = Vec::with_capacity(capacity);
    for commitment in commitments {
        points.extend_from_slice(commitment.points());
    }
    Commitment::from_points(points)
}

pub(super) fn fold(
    commitments: &[&Commitment],
    weights: &[Scalar],
) -> Result<Commitment, CommitmentError> {
    let first = commitments
        .first()
        .ok_or(CommitmentError::InvalidDimension)?;
    if commitments.len() != weights.len()
        || commitments
            .iter()
            .any(|commitment| commitment.len() != first.len())
    {
        return Err(CommitmentError::InvalidDimension);
    }
    let mut output = Vec::with_capacity(first.len());
    for point_index in 0..first.len() {
        let point = commitments.iter().zip(weights.iter().copied()).fold(
            Point::identity(),
            |sum, (commitment, weight)| {
                sum.add(commitment.points()[point_index].mul_scalar(weight))
            },
        );
        if point.is_identity() {
            return Err(CommitmentError::InvalidDimension);
        }
        output.push(point);
    }
    Commitment::from_points(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(value: u64) -> Scalar {
        Scalar::from_u64(value)
    }

    #[test]
    fn canonical_ck_derivation_matches_independent_vector() {
        let key = CommitmentKey::derive(b"ck", 4).expect("canonical key");
        assert_eq!(
            hex::encode(
                key.generators()[0]
                    .to_non_identity_wire_bytes()
                    .expect("non-identity")
            ),
            "004154bdf3554e904fa3065f943102ddb108c8f6f62c1dea97b2f12029ef2f47ea"
        );
        assert_eq!(key.columns(), 4);
    }

    #[test]
    fn commitments_are_linear_and_combine_by_rows() {
        let key = CommitmentKey::derive(b"vega-commitment-test", 4).expect("canonical key");
        let left = key
            .commit(&[s(1), s(2), s(3), s(4)], &[s(9)])
            .expect("one row");
        let right = key
            .commit(&[s(5), s(6), s(7), s(8)], &[s(10)])
            .expect("one row");
        let folded = fold(&[&left, &right], &[s(3), s(7)]).expect("aligned rows");
        let expected_values = [s(38), s(48), s(58), s(68)];
        let expected = key
            .commit(&expected_values, &[s(97)])
            .expect("linear commitment");
        assert_eq!(folded, expected);

        let combined = combine(&[&left, &right]).expect("two nonempty commitments");
        assert_eq!(combined.len(), 2);
        assert_eq!(combined.points(), &[left.points()[0], right.points()[0]]);
    }

    #[test]
    fn commitment_dimensions_and_identity_results_fail_closed() {
        let key = CommitmentKey::derive(b"vega-commitment-negative", 2)
            .expect("canonical commitment key");
        assert!(key.commit(&[], &[]).is_err());
        assert!(key.commit(&[s(1)], &[]).is_err());
        assert!(msm(&[s(1)], &[]).is_err());
        let commitment = key.commit(&[s(1), s(2)], &[s(3)]).expect("valid");
        assert!(fold(&[&commitment], &[Scalar::zero()]).is_err());
        assert!(combine(&[]).is_err());
    }

    #[test]
    fn commitment_transcript_encoding_has_exact_markers_and_points() {
        let key = CommitmentKey::derive(b"vega-commitment-transcript", 2)
            .expect("canonical commitment key");
        let commitment = key.commit(&[s(1), s(2)], &[s(3)]).expect("valid");
        let bytes = commitment.transcript_bytes().expect("nonidentity points");
        assert!(bytes.starts_with(COMMITMENT_BEGIN));
        assert!(bytes.ends_with(COMMITMENT_END));
        assert_eq!(
            bytes.len(),
            COMMITMENT_BEGIN.len() + 64 + COMMITMENT_END.len()
        );
    }
}
