//! Verifier-known periodic phase selectors without full-domain interpolation.
//!
//! For a trace subgroup of order `N` and period `B`, let `K = N/B` and let
//! `h_r = (g^K)^r` for the actual trace generator `g`. The selector for rows
//! congruent to `r mod B` is
//! `E_r(X) = h_r * (X^N - 1) / (B * (X^K - h_r))`.
//! The apparent singularities are removable: evaluation on the subgroup is
//! exactly one-hot. Each selector has degree `N-K < N`. A selector times a
//! quadratic relation in trace polynomials of degree below `N` has degree at
//! most `3N-3`; if it vanishes on the full subgroup, division by `X^N-1`
//! therefore gives a quotient of degree below `2N`.
//!
//! TODO: Bind the period, phase/slot mapping, padding and exceptional-edge masks
//! into the authenticated compact proof schema. Compile a stable residue ledger
//! that shares common equations; do not evaluate every phase's complete residue
//! vector at every LDE point. This helper does not constrain semantic carry across
//! hash padding/boundaries and does not replace mandatory witness replay.

use super::{
    GOLDILOCKS_MODULUS, field_inverse, field_pow, fixed_domain::FixedTraceDomain, mul_mod, sub_mod,
};
use crate::{Error, Result};
use fastpq_isi::StarkParameterSet;

/// Maximum fixed phase period supported by the compact hash schedule.
const MAX_PERIOD: usize = 512;

/// Small, immutable set of verifier-derived phase roots on one exact subgroup.
pub(super) struct PeriodicSelectors {
    phase_roots: Box<[u64]>,
    repetitions: u64,
    inverse_period: u64,
}

impl PeriodicSelectors {
    /// Validate the fixed geometry and retain only `period` phase roots.
    ///
    /// The caller derives `trace_rows` and `period` from the authenticated schema,
    /// never from unchecked witness phase labels. Period must be a power of two
    /// at most 512 and divide the supported power-of-two trace order. Validation
    /// checks canonical roots, their exact orders, agreement between the effective
    /// trace/LDE subgroups and a disjoint LDE coset before any allocation.
    pub(super) fn new(
        params: &StarkParameterSet,
        trace_rows: usize,
        period: usize,
    ) -> Result<Self> {
        if !trace_rows.is_power_of_two()
            || !period.is_power_of_two()
            || period > MAX_PERIOD
            || period > trace_rows
            || params.trace_log_size > 32
            || params.lde_log_size > 32
            || trace_rows.ilog2() > params.trace_log_size
        {
            return Err(shape_error(
                "periodic selector requires a supported trace and period",
            ));
        }
        let generator = FixedTraceDomain::new(params, trace_rows)?.generator;
        let repetitions = (trace_rows / period) as u64;
        let phase_generator = field_pow(generator, repetitions);
        let mut phase_root = 1;
        let phase_roots = (0..period)
            .map(|_| {
                let current = phase_root;
                phase_root = mul_mod(phase_root, phase_generator);
                current
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Ok(Self {
            phase_roots,
            repetitions,
            inverse_period: field_inverse(period as u64),
        })
    }

    /// Evaluate all fixed selectors in phase order at one canonical base point.
    ///
    /// Work and temporary storage are `O(period + log N)` and `O(period)`;
    /// there is no FFT, trace, or allocation proportional to `N`. Off-subgroup
    /// evaluation uses one batch inversion for all denominators. On-subgroup
    /// evaluation returns the exact one-hot vector without any inversion.
    pub(super) fn evaluate(&self, point: u64) -> Result<Vec<u64>> {
        validate_base(point, "fixed_schedule_evaluation_point")?;
        let reduced_point = field_pow(point, self.repetitions);
        let mut values = vec![0; self.phase_roots.len()];
        if let Some(phase) = self
            .phase_roots
            .iter()
            .position(|&root| root == reduced_point)
        {
            values[phase] = 1;
            return Ok(values);
        }
        let zerofier = sub_mod(field_pow(reduced_point, self.phase_roots.len() as u64), 1);
        let common = mul_mod(zerofier, self.inverse_period);
        let mut prefixes = Vec::with_capacity(self.phase_roots.len());
        let mut product = 1;
        for &root in &self.phase_roots {
            prefixes.push(product);
            product = mul_mod(product, sub_mod(reduced_point, root));
        }
        // The one-hot branch above excludes every zero denominator.
        let mut inverse_product = field_inverse(product);
        for phase in (0..self.phase_roots.len()).rev() {
            let root = self.phase_roots[phase];
            let inverse_denominator = mul_mod(inverse_product, prefixes[phase]);
            inverse_product = mul_mod(inverse_product, sub_mod(reduced_point, root));
            values[phase] = mul_mod(common, mul_mod(root, inverse_denominator));
        }
        Ok(values)
    }
}

fn validate_base(value: u64, context: &'static str) -> Result<()> {
    if value >= GOLDILOCKS_MODULUS {
        return Err(Error::NonCanonicalGoldilocksElement {
            context,
            indices: Vec::new(),
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
    use crate::{
        backend::{FriDomain, add_mod},
        fft::Planner,
    };
    use fastpq_isi::FASTPQ_FINAL_V1;

    fn selectors(rows: usize, period: usize) -> PeriodicSelectors {
        PeriodicSelectors::new(&FASTPQ_FINAL_V1, rows, period).unwrap()
    }

    fn trace_generator(rows: usize) -> u64 {
        field_pow(
            FASTPQ_FINAL_V1.trace_root,
            1_u64 << (FASTPQ_FINAL_V1.trace_log_size - rows.ilog2()),
        )
    }

    #[test]
    fn every_subgroup_position_has_exactly_its_periodic_phase() {
        for rows in [1_usize, 2, 4, 8, 16, 64, 512, 1024] {
            let generator = trace_generator(rows);
            for period_log in 0..=rows.ilog2().min(MAX_PERIOD.ilog2()) {
                let period = 1_usize << period_log;
                let schedule = selectors(rows, period);
                let mut point = 1;
                for row in 0..rows {
                    let values = schedule.evaluate(point).unwrap();
                    assert_eq!(values.len(), period);
                    for (phase, &value) in values.iter().enumerate() {
                        assert_eq!(
                            value,
                            u64::from(phase == row % period),
                            "N={rows}, B={period}, row={row}, phase={phase}"
                        );
                    }
                    point = mul_mod(point, generator);
                }
                assert_eq!(point, 1);
            }
        }
    }

    #[test]
    fn all_coset_selectors_match_independent_ifft_horner_and_degree() {
        let planner = Planner::new(&FASTPQ_FINAL_V1);
        for rows in [1_usize, 2, 4, 8, 16, 64] {
            for period_log in 0..=rows.ilog2() {
                let period = 1_usize << period_log;
                let schedule = selectors(rows, period);
                let mut columns: Vec<Vec<u64>> = (0..period)
                    .map(|phase| {
                        (0..rows)
                            .map(|row| u64::from(row % period == phase))
                            .collect()
                    })
                    .collect();
                planner.ifft_columns(&mut columns);
                let degree = rows - rows / period;
                for column in &columns {
                    assert_ne!(column[degree], 0, "N={rows}, B={period}");
                    assert!(
                        column[degree + 1..]
                            .iter()
                            .all(|&coefficient| coefficient == 0)
                    );
                }
                let lde_rows = rows * FASTPQ_FINAL_V1.fri.blowup_factor as usize;
                let domain = FriDomain::from_lde_parameters(
                    FASTPQ_FINAL_V1.lde_root,
                    FASTPQ_FINAL_V1.lde_log_size,
                    lde_rows,
                    FASTPQ_FINAL_V1.omega_coset,
                )
                .unwrap();
                for point in (0..lde_rows).map(|index| domain.point(index)).chain([
                    0,
                    1,
                    GOLDILOCKS_MODULUS - 1,
                ]) {
                    let values = schedule.evaluate(point).unwrap();
                    for (phase, column) in columns.iter().enumerate() {
                        let expected = column.iter().rev().fold(0, |sum, &coefficient| {
                            add_mod(mul_mod(sum, point), coefficient)
                        });
                        assert_eq!(
                            values[phase], expected,
                            "N={rows}, B={period}, phase={phase}, x={point}"
                        );
                    }
                    assert_eq!(values.into_iter().fold(0, add_mod), 1);
                }
            }
        }
    }

    #[test]
    fn exact_order_and_all_malformed_geometry_are_rejected() {
        for rows in [0, 3, usize::MAX, 1 << (FASTPQ_FINAL_V1.trace_log_size + 1)] {
            assert!(PeriodicSelectors::new(&FASTPQ_FINAL_V1, rows, 1).is_err());
        }
        for period in [0, 3, 1024, usize::MAX] {
            assert!(PeriodicSelectors::new(&FASTPQ_FINAL_V1, 1024, period).is_err());
        }
        assert!(PeriodicSelectors::new(&FASTPQ_FINAL_V1, 4, 8).is_err());
        for value in [0, 1, GOLDILOCKS_MODULUS, u64::MAX] {
            let mut params = FASTPQ_FINAL_V1;
            params.trace_root = value;
            assert!(PeriodicSelectors::new(&params, 8, 4).is_err());
            params = FASTPQ_FINAL_V1;
            params.lde_root = value;
            assert!(PeriodicSelectors::new(&params, 8, 4).is_err());
        }
        for value in [0, 1, GOLDILOCKS_MODULUS - 1, GOLDILOCKS_MODULUS, u64::MAX] {
            let mut params = FASTPQ_FINAL_V1;
            params.omega_coset = value;
            assert!(PeriodicSelectors::new(&params, 8, 4).is_err());
        }
        for log in [33, u32::MAX] {
            let mut params = FASTPQ_FINAL_V1;
            params.trace_log_size = log;
            assert!(PeriodicSelectors::new(&params, 8, 4).is_err());
            params = FASTPQ_FINAL_V1;
            params.lde_log_size = log;
            assert!(PeriodicSelectors::new(&params, 8, 4).is_err());
        }
        for blowup in [0, 3, u32::MAX] {
            let mut params = FASTPQ_FINAL_V1;
            params.fri.blowup_factor = blowup;
            assert!(PeriodicSelectors::new(&params, 8, 4).is_err());
        }
        let mut params = FASTPQ_FINAL_V1;
        params.fri.blowup_factor = 1 << FASTPQ_FINAL_V1.lde_log_size;
        assert!(PeriodicSelectors::new(&params, 8, 4).is_err());
    }

    #[test]
    fn trace_and_lde_roots_must_agree_at_the_actual_order() {
        let mut params = FASTPQ_FINAL_V1;
        params.trace_root = field_pow(params.trace_root, 3);
        assert!(PeriodicSelectors::new(&params, 16, 4).is_err());
        params = FASTPQ_FINAL_V1;
        params.lde_root = field_pow(params.lde_root, 3);
        assert!(PeriodicSelectors::new(&params, 16, 4).is_err());
        // A coherent alternative orientation still defines the same row order
        // in both domains and must yield one-hot selectors in that order.
        params.trace_root = field_pow(params.trace_root, 3);
        let schedule = PeriodicSelectors::new(&params, 16, 4).unwrap();
        let generator = field_pow(params.trace_root, 1 << (params.trace_log_size - 4));
        for row in 0..16 {
            let values = schedule.evaluate(field_pow(generator, row as u64)).unwrap();
            assert_eq!(values[row % 4], 1);
            assert_eq!(values.into_iter().fold(0, add_mod), 1);
        }
    }

    #[test]
    fn canonical_points_and_period_one_are_exact() {
        for rows in [1, 8, 512] {
            let schedule = selectors(rows, 1);
            for point in [0, 1, 7, FASTPQ_FINAL_V1.omega_coset, GOLDILOCKS_MODULUS - 1] {
                assert_eq!(schedule.evaluate(point).unwrap(), [1]);
            }
            for point in [GOLDILOCKS_MODULUS, u64::MAX] {
                assert!(matches!(
                    schedule.evaluate(point),
                    Err(Error::NonCanonicalGoldilocksElement {
                        context: "fixed_schedule_evaluation_point",
                        ..
                    })
                ));
            }
        }
        let schedule = selectors(1024, MAX_PERIOD);
        assert_eq!(
            schedule.evaluate(0).unwrap(),
            vec![field_inverse(MAX_PERIOD as u64); MAX_PERIOD]
        );
    }

    #[test]
    fn maximum_trace_storage_and_query_work_depend_only_on_period() {
        let rows = 1_usize << FASTPQ_FINAL_V1.trace_log_size;
        let schedule = selectors(rows, MAX_PERIOD);
        assert_eq!(schedule.phase_roots.len(), MAX_PERIOD);
        assert_eq!(
            core::mem::size_of_val(&*schedule.phase_roots),
            MAX_PERIOD * 8
        );
        assert_eq!(schedule.repetitions, (rows / MAX_PERIOD) as u64);
        let small = selectors(MAX_PERIOD, MAX_PERIOD);
        assert_eq!(schedule.phase_roots, small.phase_roots);
        let generator = trace_generator(rows);
        for row in [0, 407, 408, 511, 512, rows - 1] {
            let values = schedule.evaluate(field_pow(generator, row as u64)).unwrap();
            assert_eq!(values.len(), MAX_PERIOD);
            assert_eq!(values[row % MAX_PERIOD], 1);
            assert_eq!(values.into_iter().fold(0, add_mod), 1);
        }
        let values = schedule.evaluate(FASTPQ_FINAL_V1.omega_coset).unwrap();
        assert_eq!(values.len(), MAX_PERIOD);
        assert!(values.iter().all(|&value| value < GOLDILOCKS_MODULUS));
        assert_eq!(values.into_iter().fold(0, add_mod), 1);
    }
}
