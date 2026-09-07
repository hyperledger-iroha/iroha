//! Deterministic interpolation and LDE of postchallenge extension-field columns.
//!
//! Every Fp4 column is split into its four polynomial-basis coefficient columns,
//! transformed with the existing CPU planner, then reassembled in the original
//! column and row order. No challenge, commitment or admission rule is added here.
//!
//! TODO: Wire authenticated second-phase columns, transcript commitments, sampled
//! openings and complete degree/quotient relations before enabling this helper in
//! the production proof pipeline. The caller must enforce the combined base and
//! auxiliary schema budget; the local width check is not that combined budget.

use super::AirQuotientDomain;
use crate::{Error, GoldilocksFp4V1, Result, fft::Planner, trace::DEFAULT_MAX_TRACE_COLUMNS};

/// Polynomial coefficients and coset evaluations for one fixed auxiliary schema.
pub(super) struct AuxiliaryPolynomialData {
    trace_rows: usize,
    lde_rows: usize,
    coefficients: Vec<Vec<GoldilocksFp4V1>>,
    lde_columns: Vec<Vec<GoldilocksFp4V1>>,
}

impl AuxiliaryPolynomialData {
    /// Interpolate exact subgroup evaluations and extend every Fp4 coefficient.
    ///
    /// `trace_rows` must match the already committed base trace. Empty auxiliary
    /// schemas are supported with explicit geometry; populated columns must all
    /// have exactly this length. Inputs are inspected before splitting/allocating.
    ///
    /// # Errors
    /// Rejects zero, non-power-of-two, oversized or overlapping domains, mismatched
    /// column lengths, oversized schemas and noncanonical field coefficients.
    pub(super) fn from_evaluations(
        planner: &Planner,
        trace_rows: usize,
        columns: &[Vec<GoldilocksFp4V1>],
    ) -> Result<Self> {
        let params = planner.params();
        if !trace_rows.is_power_of_two() || trace_rows.ilog2() > params.trace_log_size {
            return Err(shape_error(
                "auxiliary trace rows must match a supported power-of-two subgroup",
            ));
        }
        let blowup = usize::try_from(params.fri.blowup_factor)
            .map_err(|_| shape_error("auxiliary blowup does not fit this platform"))?;
        let lde_rows = trace_rows
            .checked_mul(blowup)
            .ok_or(Error::TraceLengthOverflow { rows: trace_rows })?;
        AirQuotientDomain::new(params, lde_rows)?;
        if columns.len() > DEFAULT_MAX_TRACE_COLUMNS {
            return Err(Error::VerifierLimitExceeded {
                limit: "max_air_row_values",
                actual: columns.len(),
                max: DEFAULT_MAX_TRACE_COLUMNS,
            });
        }
        for (column_index, column) in columns.iter().enumerate() {
            if column.len() != trace_rows {
                return Err(shape_error(
                    "auxiliary columns must share the exact committed trace length",
                ));
            }
            for (row_index, &value) in column.iter().enumerate() {
                validate_element(
                    value,
                    "auxiliary_trace_evaluations",
                    &[column_index, row_index],
                )?;
            }
        }
        // Indexed Rayon collection in the planner preserves this explicit order:
        // column 0 coefficients 0..4, column 1 coefficients 0..4, and so on.
        let mut coefficient_lanes = columns
            .iter()
            .flat_map(|column| {
                (0..4).map(move |lane| {
                    column
                        .iter()
                        .map(|value| value.coefficients()[lane])
                        .collect::<Vec<_>>()
                })
            })
            .collect::<Vec<_>>();
        planner.ifft_columns(&mut coefficient_lanes);
        let lde_lanes = planner.lde_columns(&coefficient_lanes);
        Ok(Self {
            trace_rows,
            lde_rows,
            coefficients: join_lanes(&coefficient_lanes, trace_rows),
            lde_columns: join_lanes(&lde_lanes, lde_rows),
        })
    }

    /// Exact subgroup size used to interpolate every auxiliary column.
    pub(super) fn trace_rows(&self) -> usize {
        self.trace_rows
    }

    /// Exact number of coset evaluations in every auxiliary column.
    pub(super) fn lde_rows(&self) -> usize {
        self.lde_rows
    }

    /// Fp4 polynomial coefficients in caller-supplied column order.
    pub(super) fn coefficients(&self) -> &[Vec<GoldilocksFp4V1>] {
        &self.coefficients
    }

    /// Fp4 coset evaluations in caller-supplied column order.
    pub(super) fn lde_columns(&self) -> &[Vec<GoldilocksFp4V1>] {
        &self.lde_columns
    }

    /// Mix every auxiliary evaluation using complete extension-field products.
    ///
    /// The caller must derive these coefficients after the appropriate oracle
    /// commitments. A zero-column schema produces the zero oracle on its domain.
    ///
    /// # Errors
    /// Rejects a mismatched coefficient count or a noncanonical coefficient.
    pub(super) fn mixed_evaluations(
        &self,
        coefficients: &[GoldilocksFp4V1],
    ) -> Result<Vec<GoldilocksFp4V1>> {
        if coefficients.len() != self.lde_columns.len() {
            return Err(shape_error(
                "auxiliary mixture requires one coefficient per committed column",
            ));
        }
        for (index, &coefficient) in coefficients.iter().enumerate() {
            validate_element(coefficient, "auxiliary_column_mix", &[index])?;
        }
        Ok((0..self.lde_rows)
            .map(|row| {
                self.lde_columns
                    .iter()
                    .zip(coefficients)
                    .fold(GoldilocksFp4V1::ZERO, |sum, (column, coefficient)| {
                        sum.add(column[row].mul(*coefficient))
                    })
            })
            .collect())
    }
}

fn validate_element(value: GoldilocksFp4V1, context: &'static str, prefix: &[usize]) -> Result<()> {
    for (lane, coefficient) in value.coefficients().into_iter().enumerate() {
        if coefficient >= crate::GOLDILOCKS_MODULUS_V1 {
            let mut indices = prefix.to_vec();
            indices.push(lane);
            return Err(Error::NonCanonicalGoldilocksElement { context, indices });
        }
    }
    Ok(())
}

fn join_lanes(lanes: &[Vec<u64>], rows: usize) -> Vec<Vec<GoldilocksFp4V1>> {
    debug_assert!(lanes.len().is_multiple_of(4));
    lanes
        .chunks_exact(4)
        .map(|column| {
            (0..rows)
                .map(|row| {
                    GoldilocksFp4V1::new(core::array::from_fn(|lane| column[lane][row]))
                        .expect("base-field FFT results are canonical")
                })
                .collect()
        })
        .collect()
}

fn shape_error(details: &str) -> Error {
    Error::InvalidTraceShape {
        details: details.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::FriDomain;
    use fastpq_isi::FASTPQ_FINAL_V1;

    fn value(coefficients: [u64; 4]) -> GoldilocksFp4V1 {
        GoldilocksFp4V1::new(coefficients).unwrap()
    }

    fn evaluate(coefficients: &[GoldilocksFp4V1], point: u64) -> GoldilocksFp4V1 {
        coefficients
            .iter()
            .rev()
            .fold(GoldilocksFp4V1::ZERO, |sum, coefficient| {
                sum.mul_base(point).add(*coefficient)
            })
    }

    #[test]
    fn all_extension_coordinates_interpolate_and_extend_on_the_exact_coset() {
        let params = FASTPQ_FINAL_V1;
        let planner = Planner::new(&params);
        for rows in [1_usize, 2, 4, 8, 16] {
            let coefficients = (0..3)
                .map(|column| {
                    (0..rows)
                        .map(|row| {
                            value(core::array::from_fn(|lane| {
                                1 + (column * rows * 4 + row * 4 + lane) as u64
                            }))
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let subgroup =
                FriDomain::from_lde_parameters(params.trace_root, params.trace_log_size, rows, 1)
                    .unwrap();
            let evaluations = coefficients
                .iter()
                .map(|column| {
                    (0..rows)
                        .map(|row| evaluate(column, subgroup.point(row)))
                        .collect()
                })
                .collect::<Vec<_>>();
            let data =
                AuxiliaryPolynomialData::from_evaluations(&planner, rows, &evaluations).unwrap();
            assert_eq!(data.trace_rows(), rows);
            assert_eq!(data.lde_rows(), rows * params.fri.blowup_factor as usize);
            assert_eq!(data.coefficients(), coefficients);
            let coset = FriDomain::from_lde_parameters(
                params.lde_root,
                params.lde_log_size,
                data.lde_rows(),
                params.omega_coset,
            )
            .unwrap();
            for (column, expected) in data.lde_columns().iter().zip(&coefficients) {
                for (row, &actual) in column.iter().enumerate() {
                    assert_eq!(actual, evaluate(expected, coset.point(row)));
                    assert!(
                        actual
                            .coefficients()
                            .iter()
                            .all(|value| *value < crate::GOLDILOCKS_MODULUS_V1)
                    );
                }
            }
            let repeated =
                AuxiliaryPolynomialData::from_evaluations(&planner, rows, &evaluations).unwrap();
            assert_eq!(data.coefficients(), repeated.coefficients());
            assert_eq!(data.lde_columns(), repeated.lde_columns());
            let mut reversed = evaluations.clone();
            reversed.reverse();
            let reordered =
                AuxiliaryPolynomialData::from_evaluations(&planner, rows, &reversed).unwrap();
            assert_eq!(
                data.lde_columns().iter().rev().collect::<Vec<_>>(),
                reordered.lde_columns().iter().collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn base_embedded_columns_match_existing_planner_outputs() {
        let planner = Planner::new(&FASTPQ_FINAL_V1);
        let base = vec![
            vec![1, 2, 3, 4],
            vec![crate::GOLDILOCKS_MODULUS_V1 - 1, 5, 7, 11],
        ];
        let embedded = base
            .iter()
            .map(|column| {
                column
                    .iter()
                    .map(|&word| GoldilocksFp4V1::from_base(word).unwrap())
                    .collect()
            })
            .collect::<Vec<_>>();
        let mut coefficients = base.clone();
        planner.ifft_columns(&mut coefficients);
        let extended = planner.lde_columns(&coefficients);
        let data = AuxiliaryPolynomialData::from_evaluations(&planner, 4, &embedded).unwrap();
        for (actual, expected) in [
            (data.coefficients(), coefficients.as_slice()),
            (data.lde_columns(), extended.as_slice()),
        ] {
            for (column, expected) in actual.iter().zip(expected) {
                for (value, &word) in column.iter().zip(expected) {
                    assert_eq!(value.coefficients(), [word, 0, 0, 0]);
                }
            }
        }
    }

    #[test]
    fn mixed_evaluations_use_extension_multiplication_and_exact_column_count() {
        let planner = Planner::new(&FASTPQ_FINAL_V1);
        let columns = vec![vec![value([0, 0, 0, 1]); 2], vec![value([2, 3, 5, 7]); 2]];
        let data = AuxiliaryPolynomialData::from_evaluations(&planner, 2, &columns).unwrap();
        let weights = [value([0, 1, 0, 0]), value([11, 13, 17, 19])];
        let expected = GoldilocksFp4V1::from_base(7)
            .unwrap()
            .add(columns[1][0].mul(weights[1]));
        assert_eq!(
            data.mixed_evaluations(&weights).unwrap(),
            vec![expected; data.lde_rows()]
        );
        assert!(data.mixed_evaluations(&weights[..1]).is_err());
        assert!(data.mixed_evaluations(&[GoldilocksFp4V1::ONE; 3]).is_err());
        let invalid = GoldilocksFp4V1::from_coefficients_unchecked_for_test([
            0,
            crate::GOLDILOCKS_MODULUS_V1,
            0,
            0,
        ]);
        assert!(matches!(data.mixed_evaluations(&[invalid, weights[1]]),
            Err(Error::NonCanonicalGoldilocksElement { context: "auxiliary_column_mix", indices }) if indices == [0, 1]));
    }

    #[test]
    fn geometry_and_canonicality_fail_before_fft_or_allocation() {
        let params = FASTPQ_FINAL_V1;
        let planner = Planner::new(&params);
        for rows in [0, 3, 6, 1_usize << (params.trace_log_size + 1)] {
            assert!(AuxiliaryPolynomialData::from_evaluations(&planner, rows, &[]).is_err());
        }
        for columns in [
            vec![vec![]],
            vec![vec![GoldilocksFp4V1::ZERO; 1]],
            vec![
                vec![GoldilocksFp4V1::ZERO; 2],
                vec![GoldilocksFp4V1::ZERO; 4],
            ],
        ] {
            assert!(AuxiliaryPolynomialData::from_evaluations(&planner, 2, &columns).is_err());
        }
        let excessive = vec![vec![]; DEFAULT_MAX_TRACE_COLUMNS + 1];
        assert!(matches!(
            AuxiliaryPolynomialData::from_evaluations(&planner, 2, &excessive),
            Err(Error::VerifierLimitExceeded {
                limit: "max_air_row_values",
                ..
            })
        ));
        for lane in 0..4 {
            let mut coefficients = [0; 4];
            coefficients[lane] = crate::GOLDILOCKS_MODULUS_V1;
            let bad = GoldilocksFp4V1::from_coefficients_unchecked_for_test(coefficients);
            assert!(
                matches!(AuxiliaryPolynomialData::from_evaluations(&planner, 2, &[vec![GoldilocksFp4V1::ZERO, bad]]),
                Err(Error::NonCanonicalGoldilocksElement { context: "auxiliary_trace_evaluations", indices }) if indices == [0, 1, lane])
            );
        }
        for offset in [0, 1, crate::GOLDILOCKS_MODULUS_V1] {
            let mut altered = params;
            altered.omega_coset = offset;
            let planner = Planner::new(&altered);
            assert!(AuxiliaryPolynomialData::from_evaluations(&planner, 2, &[]).is_err());
        }
    }

    #[test]
    fn empty_auxiliary_schema_preserves_explicit_geometry_and_zero_mixture() {
        let planner = Planner::new(&FASTPQ_FINAL_V1);
        let data = AuxiliaryPolynomialData::from_evaluations(&planner, 4, &[]).unwrap();
        assert_eq!(data.trace_rows(), 4);
        assert_eq!(data.lde_rows(), 32);
        assert!(data.coefficients().is_empty());
        assert!(data.lde_columns().is_empty());
        assert_eq!(
            data.mixed_evaluations(&[]).unwrap(),
            vec![GoldilocksFp4V1::ZERO; 32]
        );
    }
}
