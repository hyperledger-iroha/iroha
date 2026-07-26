//! Deterministic polynomial algebra used by Vega's sum-check protocols.

use thiserror::Error;

use super::VegaT256ScalarV1 as Scalar;

/// Failure while evaluating a bounded Vega polynomial.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(super) enum AlgebraError {
    #[error("Vega polynomial dimension is invalid")]
    InvalidDimension,
    #[error("Vega polynomial evaluation table is too large")]
    EvaluationTableTooLarge,
}

const MAX_EVALUATION_TABLE_ITEMS: usize = 1 << 20;

pub(super) fn log2_ceil(value: usize) -> Result<usize, AlgebraError> {
    if value == 0 {
        return Err(AlgebraError::InvalidDimension);
    }
    Ok(usize::BITS as usize - (value - 1).leading_zeros() as usize)
}

pub(super) fn log2_exact(value: usize) -> Result<usize, AlgebraError> {
    if value == 0 || !value.is_power_of_two() {
        return Err(AlgebraError::InvalidDimension);
    }
    Ok(value.trailing_zeros() as usize)
}

pub(super) fn eq_evals(point: &[Scalar]) -> Result<Vec<Scalar>, AlgebraError> {
    let size = 1_usize
        .checked_shl(u32::try_from(point.len()).map_err(|_| AlgebraError::EvaluationTableTooLarge)?)
        .ok_or(AlgebraError::EvaluationTableTooLarge)?;
    if size > MAX_EVALUATION_TABLE_ITEMS {
        return Err(AlgebraError::EvaluationTableTooLarge);
    }
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

pub(super) fn power_evaluate(
    base: Scalar,
    variable_count: usize,
    point: &[Scalar],
) -> Result<Scalar, AlgebraError> {
    if point.len() != variable_count {
        return Err(AlgebraError::InvalidDimension);
    }
    let mut powers = Vec::with_capacity(variable_count);
    let mut power = base;
    for _ in 0..variable_count {
        powers.push(power);
        power = power.square();
    }
    let mut result = Scalar::one();
    for (power, coordinate) in powers.into_iter().zip(point.iter().rev().copied()) {
        result *= Scalar::one() + (power - Scalar::one()) * coordinate;
    }
    Ok(result)
}

pub(super) fn sparse_polynomial_evaluate(
    variable_count: usize,
    values: &[Scalar],
    point: &[Scalar],
) -> Result<Scalar, AlgebraError> {
    if values.is_empty() || point.len() != variable_count || variable_count == 0 {
        return Err(AlgebraError::InvalidDimension);
    }
    let value_variables = log2_ceil(values.len().next_power_of_two())?;
    let suffix_start = variable_count
        .checked_sub(1 + value_variables)
        .ok_or(AlgebraError::InvalidDimension)?;
    let weights = eq_evals(&point[suffix_start..])?;
    if weights.len() < values.len() {
        return Err(AlgebraError::InvalidDimension);
    }
    let partial = inner_product(values, &weights[..values.len()])?;
    let prefix = point[..suffix_start]
        .iter()
        .copied()
        .fold(Scalar::one(), |accumulator, coordinate| {
            accumulator * (Scalar::one() - coordinate)
        });
    Ok(prefix * partial)
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
        for index in 0..8 {
            let boolean = [
                s(u64::from((index & 4) != 0)),
                s(u64::from((index & 2) != 0)),
                s(u64::from((index & 1) != 0)),
            ];
            assert_eq!(
                eq_evaluate(&point, &boolean).expect("same length"),
                table[index]
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
    fn power_and_sparse_polynomial_boundaries_are_strict() {
        let point = [s(2), s(3)];
        assert_eq!(
            power_evaluate(s(5), 2, &point).expect("matching dimension"),
            (Scalar::one() + (s(5) - Scalar::one()) * s(3))
                * (Scalar::one() + (s(25) - Scalar::one()) * s(2))
        );
        assert!(power_evaluate(s(5), 3, &point).is_err());
        assert!(sparse_polynomial_evaluate(0, &[s(1)], &[]).is_err());
        assert!(eq_evals(&vec![Scalar::zero(); 21]).is_err());
    }
}
