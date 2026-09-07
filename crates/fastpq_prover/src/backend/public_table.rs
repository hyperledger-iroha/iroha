//! Sparse public-table interpolation without private trace reconstruction.
//!
//! Public rows occupy an explicit ordered subset of subgroup points; every other
//! row is exactly zero. Evaluation uses the subgroup Lagrange formula and one batch inversion,
//! allocating only in proportion to public rows and columns, never the padded
//! trace or LDE domain. These are public-input operations, not witness checks.
//!
//! TODO: Bind the canonical public bytes, count, schema and geometry before phase
//! challenges, and use these evaluations in the complete pair/binding AIR. This
//! helper does not authenticate caller-provided rows or replace mandatory replay.

use super::{
    GOLDILOCKS_MODULUS, GoldilocksFp4V1, add_mod, field_inverse, field_pow,
    fixed_domain::FixedTraceDomain, mul_mod, sub_mod,
};
use crate::{Error, Result, trace::DEFAULT_MAX_TRACE_COLUMNS};
use fastpq_isi::StarkParameterSet;

/// Borrowed public rows at fixed positions, with exact implicit zeros elsewhere.
pub(super) struct PublicTablePolynomial<'a> {
    rows: &'a [Vec<u64>],
    width: usize,
    trace_rows: usize,
    points: Vec<u64>,
    inverse_order: u64,
}

impl<'a> PublicTablePolynomial<'a> {
    /// Validate canonical public rows and fixed geometry before allocating points.
    ///
    /// `max_public_rows` must come from the verifier's authenticated schema and
    /// resource policy, not an untrusted proof field. The caller must additionally
    /// enforce combined public-input byte limits before constructing this view.
    pub(super) fn new(
        params: &StarkParameterSet,
        trace_rows: usize,
        width: usize,
        rows: &'a [Vec<u64>],
        max_public_rows: usize,
    ) -> Result<Self> {
        Self::new_at_rows(params, trace_rows, width, rows, None, max_public_rows)
    }

    /// Place authenticated public rows at exact strictly increasing subgroup indices.
    ///
    /// This supports sparse public leaf/root/path ports in a fixed hash program
    /// without allocating its full trace. Positions must be verifier-derived or
    /// authenticated with the statement, never selected by an unchecked proof.
    pub(super) fn new_sparse(
        params: &StarkParameterSet,
        trace_rows: usize,
        width: usize,
        rows: &'a [Vec<u64>],
        positions: &[usize],
        max_public_rows: usize,
    ) -> Result<Self> {
        Self::new_at_rows(
            params,
            trace_rows,
            width,
            rows,
            Some(positions),
            max_public_rows,
        )
    }

    fn new_at_rows(
        params: &StarkParameterSet,
        trace_rows: usize,
        width: usize,
        rows: &'a [Vec<u64>],
        positions: Option<&[usize]>,
        max_public_rows: usize,
    ) -> Result<Self> {
        if !trace_rows.is_power_of_two() || trace_rows.ilog2() > params.trace_log_size {
            return Err(shape_error(
                "public table requires a supported trace subgroup",
            ));
        }
        if rows.len() > max_public_rows || rows.len() > trace_rows {
            return Err(Error::VerifierLimitExceeded {
                limit: "max_public_table_rows",
                actual: rows.len(),
                max: max_public_rows.min(trace_rows),
            });
        }
        if width == 0 || width > DEFAULT_MAX_TRACE_COLUMNS {
            return Err(shape_error(
                "public table width is outside the supported schema",
            ));
        }
        if let Some(positions) = positions {
            if positions.len() != rows.len()
                || positions.iter().any(|&row| row >= trace_rows)
                || positions.windows(2).any(|pair| pair[0] >= pair[1])
            {
                return Err(shape_error(
                    "public row positions must be exact, ordered and unique",
                ));
            }
        }
        for (row_index, row) in rows.iter().enumerate() {
            if row.len() != width {
                return Err(shape_error(
                    "public table rows must have the exact schema width",
                ));
            }
            for (column, &value) in row.iter().enumerate() {
                validate_base(value, "public_table_row", &[row_index, column])?;
            }
        }
        let generator = FixedTraceDomain::new(params, trace_rows)?.generator;
        let points = if let Some(positions) = positions {
            positions
                .iter()
                .map(|&row| field_pow(generator, row as u64))
                .collect()
        } else {
            let mut point = 1;
            (0..rows.len())
                .map(|_| {
                    let current = point;
                    point = mul_mod(point, generator);
                    current
                })
                .collect()
        };
        Ok(Self {
            rows,
            width,
            trace_rows,
            points,
            inverse_order: field_inverse(trace_rows as u64),
        })
    }

    /// Evaluate every public column at a canonical base-field point.
    ///
    /// Work is O(public rows × width + log(trace rows)); temporary storage is
    /// O(public rows + width). Subgroup points, including every implicit zero, are
    /// handled exactly without attempting to invert zero.
    pub(super) fn evaluate(&self, point: u64) -> Result<Vec<u64>> {
        validate_base(point, "public_table_evaluation_point", &[])?;
        if let Some(index) = self.points.iter().position(|&known| known == point) {
            return Ok(self.rows[index].clone());
        }
        let mut values = vec![0; self.width];
        let zerofier = sub_mod(field_pow(point, self.trace_rows as u64), 1);
        if zerofier == 0 || self.rows.is_empty() {
            return Ok(values);
        }
        let mut prefixes = Vec::with_capacity(self.rows.len());
        let mut product = 1;
        for &known in &self.points {
            prefixes.push(product);
            product = mul_mod(product, sub_mod(point, known));
        }
        // Known subgroup points were handled above, so every denominator is nonzero.
        let mut inverse_product = field_inverse(product);
        let common = mul_mod(zerofier, self.inverse_order);
        for row in (0..self.rows.len()).rev() {
            let inverse_denominator = mul_mod(inverse_product, prefixes[row]);
            inverse_product = mul_mod(inverse_product, sub_mod(point, self.points[row]));
            let weight = mul_mod(common, mul_mod(self.points[row], inverse_denominator));
            for (value, &entry) in values.iter_mut().zip(&self.rows[row]) {
                *value = add_mod(*value, mul_mod(weight, entry));
            }
        }
        Ok(values)
    }

    /// Evaluate a full-Fp4 linear combination of the known public columns.
    ///
    /// Coefficients must be derived after binding the public statement. This is
    /// also the public-table side of an affine tuple compression; add its fixed
    /// shift separately in the surrounding pair/bus relation.
    pub(super) fn evaluate_mixed(
        &self,
        point: u64,
        coefficients: &[GoldilocksFp4V1],
    ) -> Result<GoldilocksFp4V1> {
        if coefficients.len() != self.width {
            return Err(shape_error(
                "public table mixture requires one coefficient per column",
            ));
        }
        for (column, coefficient) in coefficients.iter().enumerate() {
            for (lane, value) in coefficient.coefficients().into_iter().enumerate() {
                validate_base(value, "public_table_mix", &[column, lane])?;
            }
        }
        Ok(self
            .evaluate(point)?
            .into_iter()
            .zip(coefficients)
            .fold(GoldilocksFp4V1::ZERO, |sum, (value, coefficient)| {
                sum.add(coefficient.mul_base(value))
            }))
    }
}

fn validate_base(value: u64, context: &'static str, indices: &[usize]) -> Result<()> {
    if value >= GOLDILOCKS_MODULUS {
        return Err(Error::NonCanonicalGoldilocksElement {
            context,
            indices: indices.to_vec(),
        });
    }
    Ok(())
}

fn shape_error(details: &'static str) -> Error {
    Error::InvalidTraceShape {
        details: details.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fft::Planner;
    use fastpq_isi::FASTPQ_FINAL_V1;

    fn table(rows: &[Vec<u64>], order: usize, width: usize) -> PublicTablePolynomial<'_> {
        PublicTablePolynomial::new(&FASTPQ_FINAL_V1, order, width, rows, 256).unwrap()
    }

    #[test]
    fn sparse_evaluation_matches_independent_ifft_and_horner() {
        let planner = Planner::new(&FASTPQ_FINAL_V1);
        for order in [1_usize, 2, 4, 8, 16, 64] {
            for count in [0, 1, order / 2, order] {
                let rows: Vec<_> = (0..count)
                    .map(|row| vec![row as u64 * 91 + 7, GOLDILOCKS_MODULUS - 1 - row as u64])
                    .collect();
                let public = table(&rows, order, 2);
                let mut coefficients: Vec<Vec<u64>> = (0..2)
                    .map(|column| {
                        (0..order)
                            .map(|row| rows.get(row).map_or(0, |values| values[column]))
                            .collect()
                    })
                    .collect();
                planner.ifft_columns(&mut coefficients);
                for point in [
                    0,
                    1,
                    7,
                    19,
                    FASTPQ_FINAL_V1.omega_coset,
                    GOLDILOCKS_MODULUS - 1,
                ] {
                    let expected: Vec<_> = coefficients
                        .iter()
                        .map(|column| {
                            column
                                .iter()
                                .rev()
                                .fold(0, |sum, &value| add_mod(mul_mod(sum, point), value))
                        })
                        .collect();
                    assert_eq!(
                        public.evaluate(point).unwrap(),
                        expected,
                        "N={order}, m={count}"
                    );
                }
            }
        }
    }

    #[test]
    fn every_subgroup_position_returns_exact_public_or_zero_row() {
        let rows = vec![vec![0, 11], vec![13, 0], vec![17, 19]];
        for order in [4_usize, 8, 16, 64] {
            let public = table(&rows, order, 2);
            let generator = field_pow(
                FASTPQ_FINAL_V1.trace_root,
                1_u64 << (FASTPQ_FINAL_V1.trace_log_size - order.ilog2()),
            );
            let mut point = 1;
            for row in 0..order {
                assert_eq!(
                    public.evaluate(point).unwrap(),
                    rows.get(row).cloned().unwrap_or(vec![0; 2])
                );
                point = mul_mod(point, generator);
            }
        }
    }

    #[test]
    fn full_extension_mix_preserves_all_coefficients_and_public_order() {
        let rows = vec![vec![7, 11], vec![13, 17], vec![19, 23]];
        let public = table(&rows, 8, 2);
        let mix = [
            GoldilocksFp4V1::new([1, 2, 3, 4]).unwrap(),
            GoldilocksFp4V1::new([5, 6, 7, 8]).unwrap(),
        ];
        for point in [0, 1, 7, 9] {
            let values = public.evaluate(point).unwrap();
            let expected = mix[0].mul_base(values[0]).add(mix[1].mul_base(values[1]));
            assert_eq!(public.evaluate_mixed(point, &mix).unwrap(), expected);
            let swapped_rows = [rows[1].clone(), rows[0].clone(), rows[2].clone()];
            let swapped = table(&swapped_rows, 8, 2);
            if point == 1 {
                assert_ne!(swapped.evaluate_mixed(point, &mix).unwrap(), expected);
            }
        }
    }

    #[test]
    fn geometry_shape_and_every_noncanonical_input_are_rejected() {
        let valid = vec![vec![1, 2], vec![3, 4]];
        for order in [0, 3, 1 << (FASTPQ_FINAL_V1.trace_log_size + 1)] {
            assert!(PublicTablePolynomial::new(&FASTPQ_FINAL_V1, order, 2, &valid, 256).is_err());
        }
        for width in [0, 1, DEFAULT_MAX_TRACE_COLUMNS + 1] {
            assert!(PublicTablePolynomial::new(&FASTPQ_FINAL_V1, 4, width, &valid, 256).is_err());
        }
        assert!(PublicTablePolynomial::new(&FASTPQ_FINAL_V1, 1, 2, &valid, 256).is_err());
        assert!(PublicTablePolynomial::new(&FASTPQ_FINAL_V1, 4, 2, &valid, 1).is_err());
        for trace_root in [0, 1, GOLDILOCKS_MODULUS, u64::MAX] {
            let mut bad_params = FASTPQ_FINAL_V1;
            bad_params.trace_root = trace_root;
            assert!(PublicTablePolynomial::new(&bad_params, 4, 2, &valid, 256).is_err());
        }
        for row in 0..valid.len() {
            for column in 0..2 {
                for value in [GOLDILOCKS_MODULUS, u64::MAX] {
                    let mut bad = valid.clone();
                    bad[row][column] = value;
                    assert!(PublicTablePolynomial::new(&FASTPQ_FINAL_V1, 4, 2, &bad, 256).is_err());
                }
            }
        }
        let public = table(&valid, 4, 2);
        assert!(public.evaluate(GOLDILOCKS_MODULUS).is_err());
        assert!(public.evaluate(u64::MAX).is_err());
        assert!(public.evaluate_mixed(7, &[GoldilocksFp4V1::ONE]).is_err());
        assert!(
            public
                .evaluate_mixed(GOLDILOCKS_MODULUS, &[GoldilocksFp4V1::ONE; 2])
                .is_err()
        );
        for column in 0..2 {
            for lane in 0..4 {
                let mut mix = [GoldilocksFp4V1::ONE; 2];
                let mut coefficients = [0; 4];
                coefficients[lane] = GOLDILOCKS_MODULUS;
                mix[column] = GoldilocksFp4V1::from_coefficients_unchecked_for_test(coefficients);
                assert!(public.evaluate_mixed(7, &mix).is_err());
            }
        }
    }

    #[test]
    fn sparse_positions_match_horner_and_reject_duplicate_or_relabelled_rows() {
        let rows = vec![vec![5, 9], vec![11, 15], vec![17, 21]];
        let positions = [0, 7, 15];
        let public =
            PublicTablePolynomial::new_sparse(&FASTPQ_FINAL_V1, 16, 2, &rows, &positions, 256)
                .unwrap();
        let mut coefficients = vec![vec![0; 16]; 2];
        for (&position, values) in positions.iter().zip(&rows) {
            for (column, &value) in values.iter().enumerate() {
                coefficients[column][position] = value;
            }
        }
        Planner::new(&FASTPQ_FINAL_V1).ifft_columns(&mut coefficients);
        for point in [
            0,
            1,
            7,
            11,
            FASTPQ_FINAL_V1.omega_coset,
            GOLDILOCKS_MODULUS - 1,
        ] {
            let expected: Vec<_> = coefficients
                .iter()
                .map(|column| {
                    column
                        .iter()
                        .rev()
                        .fold(0, |sum, &value| add_mod(mul_mod(sum, point), value))
                })
                .collect();
            assert_eq!(public.evaluate(point).unwrap(), expected);
        }
        for bad in [&[0, 7][..], &[0, 7, 7], &[0, 15, 7], &[0, 7, 16]] {
            assert!(
                PublicTablePolynomial::new_sparse(&FASTPQ_FINAL_V1, 16, 2, &rows, bad, 256)
                    .is_err()
            );
        }
        let moved =
            PublicTablePolynomial::new_sparse(&FASTPQ_FINAL_V1, 16, 2, &rows, &[1, 7, 15], 256)
                .unwrap();
        assert_ne!(public.evaluate(1).unwrap(), moved.evaluate(1).unwrap());
        assert_eq!(moved.evaluate(1).unwrap(), [0, 0]);
    }

    #[test]
    fn maximum_trace_domain_retains_only_the_public_prefix() {
        let rows = vec![vec![5, 9], vec![11, 15]];
        let public = table(&rows, 1 << FASTPQ_FINAL_V1.trace_log_size, 2);
        assert_eq!(public.points.len(), rows.len());
        assert_eq!(public.points.capacity(), rows.len());
        assert!(core::ptr::eq(public.rows, rows.as_slice()));
        assert_eq!(public.evaluate(1).unwrap(), rows[0]);
        assert_eq!(
            public.evaluate(FASTPQ_FINAL_V1.omega_coset).unwrap().len(),
            2
        );
    }
}
