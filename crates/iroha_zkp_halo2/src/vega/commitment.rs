//! Hyrax row commitments over the canonical T256 group.
use super::{
    VegaCurveError, VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar, derive_t256_generators_v1,
};
use halo2curves::{
    group::{Curve as _, prime::PrimeCurveAffine as _},
    msm::msm_best,
    t256::{T256, T256Affine},
};
use thiserror::Error;
const COMMITMENT_BEGIN: &[u8] = b"poly_commitment_begin";
const COMMITMENT_END: &[u8] = b"poly_commitment_end";
pub(super) const MAX_COMMITMENT_WORKERS: usize = 20;
pub(super) const COMMITMENT_WORKER_STACK_BYTES: usize = 512 * 1024;
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
    pub(super) fn into_points(self) -> Vec<Point> {
        self.points
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
    generator_affines: Vec<T256Affine>,
    hiding_generator: Point,
    worker_count: usize,
    #[cfg(test)]
    panic_worker: Option<usize>,
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
        let generator_affines = batch_normalize(&points);
        let key = Self {
            generators: points,
            generator_affines,
            hiding_generator,
            worker_count: 1,
            #[cfg(test)]
            panic_worker: None,
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
    pub(super) fn with_worker_count(
        mut self,
        worker_count: usize,
    ) -> Result<Self, CommitmentError> {
        if worker_count == 0 || worker_count > MAX_COMMITMENT_WORKERS {
            return Err(CommitmentError::InvalidDimension);
        }
        self.worker_count = worker_count;
        Ok(self)
    }
    #[cfg(test)]
    fn with_test_worker_panic(mut self, worker_index: usize) -> Self {
        self.panic_worker = Some(worker_index);
        self
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
        let worker_count = self.worker_count;
        if worker_count > row_count {
            return Err(CommitmentError::InvalidDimension);
        }
        let points = std::thread::scope(|scope| {
            let mut workers = Vec::with_capacity(worker_count);
            for worker_index in 0..worker_count {
                let row_start = worker_index
                    .checked_mul(row_count)
                    .map(|value| value / worker_count)
                    .ok_or(CommitmentError::InvalidDimension)?;
                let row_end = worker_index
                    .checked_add(1)
                    .and_then(|index| index.checked_mul(row_count))
                    .map(|value| value / worker_count)
                    .ok_or(CommitmentError::InvalidDimension)?;
                let value_start = row_start
                    .checked_mul(self.columns())
                    .ok_or(CommitmentError::InvalidDimension)?;
                let value_end = row_end
                    .checked_mul(self.columns())
                    .map(|end| end.min(values.len()))
                    .ok_or(CommitmentError::InvalidDimension)?;
                let value_rows = &values[value_start..value_end];
                let blinding_rows = &row_blindings[row_start..row_end];
                let worker = std::thread::Builder::new()
                    .name(format!("vega-msm-{worker_index}"))
                    .stack_size(COMMITMENT_WORKER_STACK_BYTES)
                    .spawn_scoped(scope, move || {
                        #[cfg(test)]
                        if self.panic_worker == Some(worker_index) {
                            panic!("injected Vega commitment worker panic");
                        }
                        self.commit_rows(value_rows, blinding_rows)
                    });
                match worker {
                    Ok(worker) => workers.push(worker),
                    Err(_) => {
                        for worker in workers {
                            let _ = worker.join();
                        }
                        return Err(CommitmentError::InvalidDimension);
                    }
                }
            }
            let mut points = Vec::with_capacity(row_count);
            let mut worker_failed = false;
            for worker in workers {
                match worker.join() {
                    Ok(Ok(worker_points)) if !worker_failed => points.extend(worker_points),
                    Ok(Ok(_)) => {}
                    Ok(Err(_)) | Err(_) => worker_failed = true,
                }
            }
            if worker_failed {
                return Err(CommitmentError::InvalidDimension);
            }
            Ok::<_, CommitmentError>(points)
        })?;
        Commitment::from_points(points)
    }

    /// Commit one exactly padded segment without materializing its zero suffix.
    ///
    /// `values` is the unpadded prefix and `padded_len` is the governed segment
    /// length.  One blinding is required for every complete commitment row.
    /// This is the Figure 9 application path's bounded alternative to allocating
    /// hundreds of thousands of explicit zero scalars per split instance.
    pub(super) fn commit_padded_prefix(
        &self,
        values: &[Scalar],
        padded_len: usize,
        row_blindings: &[Scalar],
    ) -> Result<Commitment, CommitmentError> {
        if padded_len == 0
            || values.len() > padded_len
            || !padded_len.is_multiple_of(self.columns())
        {
            return Err(CommitmentError::InvalidDimension);
        }
        let row_count = padded_len / self.columns();
        if row_blindings.len() != row_count {
            return Err(CommitmentError::InvalidDimension);
        }
        // Small sections (notably one-row precommitments) remain valid even
        // when the proof-wide worker setting is larger than their row count.
        let worker_count = self.worker_count.min(row_count);
        let points = std::thread::scope(|scope| {
            let mut workers = Vec::with_capacity(worker_count);
            for worker_index in 0..worker_count {
                let row_start = worker_index
                    .checked_mul(row_count)
                    .map(|value| value / worker_count)
                    .ok_or(CommitmentError::InvalidDimension)?;
                let row_end = worker_index
                    .checked_add(1)
                    .and_then(|index| index.checked_mul(row_count))
                    .map(|value| value / worker_count)
                    .ok_or(CommitmentError::InvalidDimension)?;
                let worker = std::thread::Builder::new()
                    .name(format!("vega-padded-msm-{worker_index}"))
                    .stack_size(COMMITMENT_WORKER_STACK_BYTES)
                    .spawn_scoped(scope, move || {
                        #[cfg(test)]
                        if self.panic_worker == Some(worker_index) {
                            panic!("injected Vega commitment worker panic");
                        }
                        self.commit_padded_rows(
                            values,
                            &row_blindings[row_start..row_end],
                            row_start,
                        )
                    });
                match worker {
                    Ok(worker) => workers.push(worker),
                    Err(_) => {
                        for worker in workers {
                            let _ = worker.join();
                        }
                        return Err(CommitmentError::InvalidDimension);
                    }
                }
            }
            let mut points = Vec::with_capacity(row_count);
            let mut worker_failed = false;
            for worker in workers {
                match worker.join() {
                    Ok(Ok(worker_points)) if !worker_failed => points.extend(worker_points),
                    Ok(Ok(_)) => {}
                    Ok(Err(_)) | Err(_) => worker_failed = true,
                }
            }
            if worker_failed {
                return Err(CommitmentError::InvalidDimension);
            }
            Ok::<_, CommitmentError>(points)
        })?;
        Commitment::from_points(points)
    }

    fn commit_padded_rows(
        &self,
        values: &[Scalar],
        row_blindings: &[Scalar],
        first_row: usize,
    ) -> Result<Vec<Point>, CommitmentError> {
        let mut points = Vec::with_capacity(row_blindings.len());
        for (offset, blinding) in row_blindings.iter().copied().enumerate() {
            let row = first_row
                .checked_add(offset)
                .ok_or(CommitmentError::InvalidDimension)?;
            let value_start = row
                .checked_mul(self.columns())
                .ok_or(CommitmentError::InvalidDimension)?;
            let value_end = value_start
                .checked_add(self.columns())
                .map(|end| end.min(values.len()))
                .ok_or(CommitmentError::InvalidDimension)?;
            let populated = if value_start < values.len() {
                &values[value_start..value_end]
            } else {
                &[]
            };
            let populated = populated
                .iter()
                .rposition(|value| !value.is_zero())
                .map_or(&[][..], |last| &populated[..=last]);
            let hiding = self.hiding_generator.mul_scalar(blinding);
            let committed = if populated.is_empty() {
                hiding
            } else {
                Point(msm_best(
                    &populated.iter().map(|scalar| scalar.0).collect::<Vec<_>>(),
                    &self.generator_affines[..populated.len()],
                )) + hiding
            };
            if committed.is_identity() {
                return Err(CommitmentError::InvalidDimension);
            }
            points.push(committed);
        }
        Ok(points)
    }

    fn commit_rows(
        &self,
        values: &[Scalar],
        row_blindings: &[Scalar],
    ) -> Result<Vec<Point>, CommitmentError> {
        let mut points = Vec::with_capacity(row_blindings.len());
        for (row, blinding) in values
            .chunks(self.columns())
            .zip(row_blindings.iter().copied())
        {
            let committed = Point(msm_best(
                &row.iter().map(|scalar| scalar.0).collect::<Vec<_>>(),
                &self.generator_affines[..row.len()],
            )) + self.hiding_generator.mul_scalar(blinding);
            if committed.is_identity() {
                return Err(CommitmentError::InvalidDimension);
            }
            points.push(committed);
        }
        Ok(points)
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
    Ok(Point(msm_best(
        &scalars.iter().map(|scalar| scalar.0).collect::<Vec<_>>(),
        &batch_normalize(points),
    )))
}
fn batch_normalize(points: &[Point]) -> Vec<T256Affine> {
    let projective = points.iter().map(|point| point.0).collect::<Vec<T256>>();
    let mut affine = vec![T256Affine::identity(); projective.len()];
    T256::batch_normalize(&projective, &mut affine);
    affine
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
        let point = commitments
            .iter()
            .zip(weights.iter().copied())
            .fold(Point::identity(), |sum, (commitment, weight)| {
                sum + commitment.points()[point_index].mul_scalar(weight)
            });
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
    fn commitments_are_linear_and_fold_by_rows() {
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
    }
    #[test]
    fn invalid_worker_counts_and_worker_panics_fail_closed() {
        let key = CommitmentKey::derive(b"vega-commitment-worker-negative", 2)
            .expect("canonical commitment key");
        assert!(key.clone().with_worker_count(0).is_err());
        assert!(
            key.clone()
                .with_worker_count(MAX_COMMITMENT_WORKERS + 1)
                .is_err()
        );
        assert!(
            key.clone()
                .with_worker_count(2)
                .expect("two workers")
                .commit(&[s(1), s(2)], &[s(3)])
                .is_err(),
            "worker count greater than the row count must fail"
        );
        assert_eq!(
            key.with_worker_count(2)
                .expect("two workers")
                .with_test_worker_panic(1)
                .commit(&[s(1), s(2), s(3), s(4)], &[s(5), s(6)]),
            Err(CommitmentError::InvalidDimension)
        );
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
