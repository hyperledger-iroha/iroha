//! Sparse public-table interpolation without private trace reconstruction.
//!
//! Public rows occupy an explicit ordered subset of subgroup points; every other
//! row is exactly zero. Evaluation uses the subgroup Lagrange formula and one batch inversion,
//! allocating only in proportion to public rows and columns, never the padded
//! trace or LDE domain. These are public-input operations, not witness checks.
//!
//! The typed transfer AIR and its profile own statement/geometry binding. This
//! helper validates the bounded supplied table and evaluates its exact polynomial;
//! it does not authenticate the table's source or establish a proof opening.

#[cfg(test)]
use super::GoldilocksFp4V1;
#[cfg(test)]
use super::add_mod;
use super::{
    GOLDILOCKS_MODULUS, field_inverse, field_pow, fixed_domain::FixedTraceDomain, mul_mod,
    polynomial_field::PolynomialField,
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
    #[cfg(test)]
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

    /// Evaluate every public column at a canonical base or extension field point.
    ///
    /// Work is O(public rows × width + log(trace rows)); temporary storage is
    /// O(public rows + width). Subgroup points, including every implicit zero, are
    /// handled exactly without attempting to invert zero.
    pub(super) fn evaluate<F: PolynomialField>(&self, point: F) -> Result<Vec<F>> {
        point.validate("public_table_evaluation_point", &[])?;
        if let Some(index) = self
            .points
            .iter()
            .position(|&known| F::embed_base(known) == point)
        {
            return Ok(self.rows[index]
                .iter()
                .copied()
                .map(F::embed_base)
                .collect());
        }
        let mut values = vec![F::ZERO; self.width];
        let zerofier = point.power(self.trace_rows as u64).sub(F::ONE);
        if zerofier == F::ZERO || self.rows.is_empty() {
            return Ok(values);
        }
        let mut prefixes = Vec::with_capacity(self.rows.len());
        let mut product = F::ONE;
        for &known in &self.points {
            prefixes.push(product);
            product = product.mul(point.sub(F::embed_base(known)));
        }
        // Known subgroup points were handled above, so no denominator is zero.
        let mut inverse_product = product
            .inverse()
            .ok_or_else(|| shape_error("public table denominator is not invertible"))?;
        let common = zerofier.scale_base(self.inverse_order);
        for row in (0..self.rows.len()).rev() {
            let inverse_denominator = inverse_product.mul(prefixes[row]);
            inverse_product = inverse_product.mul(point.sub(F::embed_base(self.points[row])));
            let weight = common.mul(inverse_denominator).scale_base(self.points[row]);
            for (value, &entry) in values.iter_mut().zip(&self.rows[row]) {
                *value = value.add(weight.scale_base(entry));
            }
        }
        Ok(values)
    }

    /// Evaluate a full-Fp4 linear combination of the known public columns.
    ///
    /// Coefficients must be derived after binding the public statement. This is
    /// also the public-table side of an affine tuple compression; add its fixed
    /// shift separately in the surrounding pair/bus relation.
    #[cfg(test)]
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
        assert_ne!(
            public.evaluate(1_u64).unwrap(),
            moved.evaluate(1_u64).unwrap()
        );
        assert_eq!(moved.evaluate(1_u64).unwrap(), [0, 0]);
    }

    #[test]
    fn maximum_trace_domain_retains_only_the_public_prefix() {
        let rows = vec![vec![5, 9], vec![11, 15]];
        let public = table(&rows, 1 << FASTPQ_FINAL_V1.trace_log_size, 2);
        assert_eq!(public.points.len(), rows.len());
        assert_eq!(public.points.capacity(), rows.len());
        assert!(core::ptr::eq(public.rows, rows.as_slice()));
        assert_eq!(public.evaluate(1_u64).unwrap(), rows[0]);
        assert_eq!(
            public.evaluate(FASTPQ_FINAL_V1.omega_coset).unwrap().len(),
            2
        );
    }

    #[test]
    fn extension_public_tables_match_independent_horner_sparse_and_full_domain() {
        use super::super::polynomial_reference as reference;
        type F = GoldilocksFp4V1;
        let rows = vec![vec![7, 11], vec![19, 31], vec![0, GOLDILOCKS_MODULUS - 1]];
        for order in [4, 8, 16, 65_536] {
            let positions = [0, order / 2, order - 1];
            let public =
                PublicTablePolynomial::new_sparse(&FASTPQ_FINAL_V1, order, 2, &rows, &positions, 3)
                    .unwrap();
            let mut points = reference::points();
            let generator = FixedTraceDomain::new(&FASTPQ_FINAL_V1, order)
                .unwrap()
                .generator;
            for row in [0, 1, order / 2, order - 1] {
                points.push(F::embed_base(field_pow(generator, row as u64)));
            }
            for point in points {
                let actual = public.evaluate(point).unwrap();
                for column in 0..2 {
                    let expected =
                        positions
                            .iter()
                            .zip(&rows)
                            .fold(F::ZERO, |sum, (&row, values)| {
                                sum.add(
                                    reference::lagrange(order, row, point).mul_base(values[column]),
                                )
                            });
                    assert_eq!(actual[column], expected);
                    if order <= 16 {
                        let mut values = vec![0; order];
                        for (&row, values_at_row) in positions.iter().zip(&rows) {
                            values[row] = values_at_row[column];
                        }
                        assert_eq!(
                            expected,
                            reference::horner(&reference::interpolate(&values), point)
                        );
                    }
                }
            }
        }
        let empty =
            PublicTablePolynomial::new_sparse(&FASTPQ_FINAL_V1, 65_536, 2, &[], &[], 0).unwrap();
        for point in reference::points() {
            assert_eq!(empty.evaluate(point).unwrap(), vec![F::ZERO; 2]);
        }
    }

    #[test]
    fn extension_public_tables_bind_sparse_positions_values_and_full_point() {
        use super::super::polynomial_reference as reference;
        let rows = vec![vec![3, 7], vec![11, 13]];
        let original =
            PublicTablePolynomial::new_sparse(&FASTPQ_FINAL_V1, 16, 2, &rows, &[0, 15], 2).unwrap();
        let moved =
            PublicTablePolynomial::new_sparse(&FASTPQ_FINAL_V1, 16, 2, &rows, &[1, 15], 2).unwrap();
        let changed_rows = vec![vec![5, 7], vec![11, 13]];
        let changed =
            PublicTablePolynomial::new_sparse(&FASTPQ_FINAL_V1, 16, 2, &changed_rows, &[0, 15], 2)
                .unwrap();
        let point = *reference::points().last().unwrap();
        assert_ne!(
            original.evaluate(point).unwrap(),
            moved.evaluate(point).unwrap()
        );
        assert_ne!(
            original.evaluate(point).unwrap(),
            changed.evaluate(point).unwrap()
        );
        assert_ne!(
            original.evaluate(point).unwrap(),
            original
                .evaluate(GoldilocksFp4V1::embed_base(point.coefficients()[0]))
                .unwrap()
        );
        for lane in 0..4 {
            let mut words = [0; 4];
            words[lane] = GOLDILOCKS_MODULUS;
            assert!(
                matches!(original.evaluate(GoldilocksFp4V1::from_coefficients_unchecked_for_test(words)), Err(Error::NonCanonicalGoldilocksElement { context: "public_table_evaluation_point", indices }) if indices == [lane])
            );
        }
    }
}
