//! Canonical Hyrax row-commitment direct openings for Vega V1.
//!
//! The crate-private V1 Relaxed Spartan proof intentionally opens the
//! row-bound vector and its combined blinding directly. Its proof wire does
//! not carry a separate evaluation commitment or inner-product argument.

use thiserror::Error;

use super::{
    VegaT256ScalarV1 as Scalar,
    algebra::{AlgebraError, eq_evals, inner_product, log2_exact},
    commitment::{Commitment, CommitmentError, CommitmentKey, msm},
};

/// Failure while proving or verifying a canonical Hyrax direct opening.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(super) enum HyraxError {
    #[error("Vega Hyrax dimensions do not match the fixed direct-opening shape")]
    InvalidDimension,
    #[error("Vega Hyrax direct opening does not match its commitment")]
    InvalidDirectOpening,
    #[error(transparent)]
    Algebra(#[from] AlgebraError),
    #[error(transparent)]
    Commitment(#[from] CommitmentError),
}

pub(super) fn prove_direct(
    key: &CommitmentKey,
    polynomial: &[Scalar],
    blindings: &[Scalar],
    point: &[Scalar],
) -> Result<(Vec<Scalar>, Scalar), HyraxError> {
    let padded_len = 1_usize
        .checked_shl(u32::try_from(point.len()).map_err(|_| HyraxError::InvalidDimension)?)
        .ok_or(HyraxError::InvalidDimension)?;
    if polynomial.is_empty() || polynomial.len() > padded_len || !key.columns().is_power_of_two() {
        return Err(HyraxError::InvalidDimension);
    }
    let row_count = padded_len.div_ceil(key.columns());
    if blindings.len() != polynomial.len().div_ceil(key.columns()) || !row_count.is_power_of_two() {
        return Err(HyraxError::InvalidDimension);
    }
    if row_count == 1 {
        let mut values = polynomial.to_vec();
        values.resize(key.columns(), Scalar::zero());
        return Ok((values, blindings[0]));
    }
    let row_variables = log2_exact(row_count)?;
    let left_weights = eq_evals(&point[..row_variables])?;
    let mut padded = polynomial.to_vec();
    padded.resize(padded_len, Scalar::zero());
    let mut values = vec![Scalar::zero(); key.columns()];
    for (row, weight) in padded
        .chunks_exact(key.columns())
        .zip(left_weights.iter().copied())
    {
        for (output, value) in values.iter_mut().zip(row.iter().copied()) {
            *output += weight * value;
        }
    }
    let combined_blinding = inner_product(&left_weights[..blindings.len()], blindings)?;
    Ok((values, combined_blinding))
}

pub(super) fn verify_direct(
    key: &CommitmentKey,
    commitment: &Commitment,
    values: &[Scalar],
    combined_blinding: Scalar,
    point: &[Scalar],
) -> Result<Scalar, HyraxError> {
    if values.len() != key.columns() || commitment.len() == 0 {
        return Err(HyraxError::InvalidDimension);
    }
    let padded_len = 1_usize
        .checked_shl(u32::try_from(point.len()).map_err(|_| HyraxError::InvalidDimension)?)
        .ok_or(HyraxError::InvalidDimension)?;
    let row_count = padded_len.div_ceil(key.columns());
    if commitment.len() > row_count
        || !row_count.is_power_of_two()
        || !key.columns().is_power_of_two()
    {
        return Err(HyraxError::InvalidDimension);
    }
    let row_variables = log2_exact(row_count)?;
    let comm_lz = if row_variables == 0 {
        commitment.points()[0]
    } else {
        let left_weights = eq_evals(&point[..row_variables])?;
        msm(&left_weights[..commitment.len()], commitment.points())?
    };
    let expected =
        msm(values, key.generators())? + key.hiding_generator().mul_scalar(combined_blinding);
    if comm_lz != expected {
        return Err(HyraxError::InvalidDirectOpening);
    }
    let right_weights = eq_evals(&point[row_variables..])?;
    Ok(inner_product(values, &right_weights)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(value: u64) -> Scalar {
        Scalar::from_u64(value)
    }

    fn setup(columns: usize) -> CommitmentKey {
        CommitmentKey::derive(b"hyrax-direct-opening-test", columns).expect("commitment key")
    }

    #[test]
    fn one_row_direct_opening_roundtrips_and_rejects_mutation() {
        let key = setup(4);
        let polynomial = [s(2), s(3), s(5), s(7)];
        let point = [s(11), s(13)];
        let expected =
            inner_product(&polynomial, &eq_evals(&point).expect("small table")).expect("aligned");
        let polynomial_blinding = s(17);
        let commitment = key
            .commit(&polynomial, &[polynomial_blinding])
            .expect("one row");

        let (values, blind) =
            prove_direct(&key, &polynomial, &[polynomial_blinding], &point).expect("direct");
        assert_eq!(values, polynomial);
        assert_eq!(blind, polynomial_blinding);
        assert_eq!(
            verify_direct(&key, &commitment, &values, blind, &point).expect("valid direct"),
            expected
        );

        let mut bad_values = values;
        bad_values[0] += Scalar::one();
        assert_eq!(
            verify_direct(&key, &commitment, &bad_values, blind, &point),
            Err(HyraxError::InvalidDirectOpening)
        );
        assert_eq!(
            verify_direct(
                &key,
                &commitment,
                &polynomial,
                blind + Scalar::one(),
                &point
            ),
            Err(HyraxError::InvalidDirectOpening)
        );
    }

    #[test]
    fn multi_row_direct_opening_binds_rows_in_transcript_order() {
        let key = setup(2);
        let polynomial = [s(2), s(3), s(5), s(7)];
        let blindings = [s(11), s(13)];
        let point = [s(17), s(19)];
        let commitment = key.commit(&polynomial, &blindings).expect("two rows");

        let (values, combined_blinding) =
            prove_direct(&key, &polynomial, &blindings, &point).expect("direct");
        let unselected = Scalar::one() - point[0];
        assert_eq!(
            values,
            vec![
                unselected * polynomial[0] + point[0] * polynomial[2],
                unselected * polynomial[1] + point[0] * polynomial[3],
            ]
        );
        assert_eq!(
            combined_blinding,
            unselected * blindings[0] + point[0] * blindings[1]
        );
        assert_eq!(
            verify_direct(&key, &commitment, &values, combined_blinding, &point)
                .expect("valid direct"),
            inner_product(&polynomial, &eq_evals(&point).expect("small table")).expect("aligned")
        );
    }

    #[test]
    fn direct_opening_rejects_inconsistent_shapes() {
        let key = setup(2);
        let polynomial = [s(2), s(3), s(5), s(7)];
        let point = [s(11), s(13)];
        let commitment = key.commit(&polynomial, &[s(17), s(19)]).expect("two rows");

        assert!(prove_direct(&key, &[], &[], &point).is_err());
        assert!(prove_direct(&key, &polynomial, &[s(17)], &point).is_err());
        assert!(prove_direct(&key, &polynomial, &[s(17), s(19)], &point[..1]).is_err());
        assert!(verify_direct(&key, &commitment, &[s(1)], s(2), &point).is_err());
        assert!(verify_direct(&key, &commitment, &[s(1), s(2)], s(3), &point[..0]).is_err());
    }
}
