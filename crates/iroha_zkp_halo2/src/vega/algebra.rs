//! Deterministic polynomial algebra used by Vega's sum-check protocols.
use super::VegaT256ScalarV1 as Scalar;
use thiserror::Error;
/// Failure while evaluating a bounded Vega polynomial.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(super) enum AlgebraError {
    #[error("Vega polynomial dimension is invalid")]
    InvalidDimension,
    #[error("Vega polynomial evaluation table is too large")]
    EvaluationTableTooLarge,
}
const MAX_EVALUATION_TABLE_ITEMS: usize = 1 << 20;
pub(super) fn evaluation_table_size(variable_count: usize) -> Result<usize, AlgebraError> {
    let size = 1_usize
        .checked_shl(
            u32::try_from(variable_count).map_err(|_| AlgebraError::EvaluationTableTooLarge)?,
        )
        .ok_or(AlgebraError::EvaluationTableTooLarge)?;
    if size > MAX_EVALUATION_TABLE_ITEMS {
        return Err(AlgebraError::EvaluationTableTooLarge);
    }
    Ok(size)
}
pub(super) fn log2_exact(value: usize) -> Result<usize, AlgebraError> {
    if value == 0 || !value.is_power_of_two() {
        return Err(AlgebraError::InvalidDimension);
    }
    Ok(value.trailing_zeros() as usize)
}
pub(super) fn eq_evals(point: &[Scalar]) -> Result<Vec<Scalar>, AlgebraError> {
    let size = evaluation_table_size(point.len())?;
    let mut evaluations = vec![Scalar::zero(); size];
    evaluations[0] = Scalar::one();
    let mut populated = 1;
    for coordinate in point.iter().rev().copied() {
        for index in 0..populated {
            let selected = evaluations[index] * coordinate;
            evaluations[populated + index] = selected;
            evaluations[index] -= selected;
        }
        populated *= 2;
    }
    Ok(evaluations)
}
pub(super) fn eq_evaluate(left: &[Scalar], right: &[Scalar]) -> Result<Scalar, AlgebraError> {
    if left.len() != right.len() {
        return Err(AlgebraError::InvalidDimension);
    }
    let mut result = Scalar::one();
    for (left, right) in left.iter().copied().zip(right.iter().copied()) {
        result *= right * left + (Scalar::one() - right) * (Scalar::one() - left);
    }
    Ok(result)
}
pub(super) fn decompress_univariate(
    coefficients_except_linear: &[Scalar],
    sum_at_boolean_points: Scalar,
) -> Result<Vec<Scalar>, AlgebraError> {
    let (constant, higher) = coefficients_except_linear
        .split_first()
        .ok_or(AlgebraError::InvalidDimension)?;
    let linear = higher.iter().copied().fold(
        sum_at_boolean_points - *constant - *constant,
        |value, coefficient| value - coefficient,
    );
    let mut coefficients = Vec::with_capacity(coefficients_except_linear.len() + 1);
    coefficients.push(*constant);
    coefficients.push(linear);
    coefficients.extend_from_slice(higher);
    Ok(coefficients)
}
pub(super) fn evaluate_univariate(
    coefficients: &[Scalar],
    point: Scalar,
) -> Result<Scalar, AlgebraError> {
    let (constant, tail) = coefficients
        .split_first()
        .ok_or(AlgebraError::InvalidDimension)?;
    let mut result = *constant;
    let mut power = point;
    for coefficient in tail {
        result += power * *coefficient;
        power *= point;
    }
    Ok(result)
}
pub(super) fn inner_product(left: &[Scalar], right: &[Scalar]) -> Result<Scalar, AlgebraError> {
    if left.len() != right.len() {
        return Err(AlgebraError::InvalidDimension);
    }
    Ok(left
        .iter()
        .copied()
        .zip(right.iter().copied())
        .fold(Scalar::zero(), |sum, (left, right)| sum + left * right))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn s(value: u64) -> Scalar {
        Scalar::from_u64(value)
    }
    #[test]
    fn eq_table_and_direct_evaluation_agree_at_boolean_points() {
        let point = [s(3), s(5), s(7)];
        let table = eq_evals(&point).expect("small table");
        assert_eq!(table.len(), 8);
        for (index, expected) in table.iter().copied().enumerate() {
            let boolean = [
                s(u64::from((index & 4) != 0)),
                s(u64::from((index & 2) != 0)),
                s(u64::from((index & 1) != 0)),
            ];
            assert_eq!(
                eq_evaluate(&point, &boolean).expect("same length"),
                expected
            );
        }
        assert_eq!(
            table
                .into_iter()
                .fold(Scalar::zero(), |sum, value| sum + value),
            Scalar::one()
        );
    }
    #[test]
    fn compressed_univariate_recovers_boolean_sum_invariant() {
        let compressed = [s(11), s(13), s(17)];
        let hint = s(101);
        let polynomial = decompress_univariate(&compressed, hint).expect("degree three");
        assert_eq!(
            evaluate_univariate(&polynomial, Scalar::zero()).expect("nonempty")
                + evaluate_univariate(&polynomial, Scalar::one()).expect("nonempty"),
            hint
        );
        assert_eq!(polynomial[0], compressed[0]);
        assert_eq!(polynomial[2..], compressed[1..]);
    }
    #[test]
    fn evaluation_tables_reject_oversized_dimensions() {
        assert_eq!(
            evaluation_table_size(21),
            Err(AlgebraError::EvaluationTableTooLarge)
        );
        assert!(eq_evals(&vec![Scalar::zero(); 21]).is_err());
    }
}
