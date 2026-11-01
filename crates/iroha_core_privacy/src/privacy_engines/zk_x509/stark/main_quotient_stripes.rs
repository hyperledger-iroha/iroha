//! Bounded interleaved quotient cosets with unchanged full-domain row order.

use super::*;
#[cfg(test)]
use crate::privacy_engines::transparent_stark::goldilocks_fft_v1;

/// Maximum live rows per trace column while evaluating one registration.
pub(super) const MAIN_QUOTIENT_STRIPE_LOG2_V1: u8 = 19;

/// One verifier-independent partition of the original quotient coset.
///
/// Stripe `s` contains full-domain indices `s + count * j`. Native translation
/// stays in the same stripe because the stripe domain contains the native one.
#[derive(Clone, Copy, Debug)]
pub(super) struct MainQuotientStripeV1 {
    pub(super) rows: usize,
    pub(super) count: usize,
    pub(super) ordinal: usize,
    pub(super) next_stride: usize,
    pub(super) root: F,
    pub(super) shift: F,
}

impl MainQuotientStripeV1 {
    pub(super) fn new_v1(
        native_log2: u8,
        evaluation_log2: u8,
        ordinal: usize,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        if native_log2 > MAIN_QUOTIENT_STRIPE_LOG2_V1
            || evaluation_log2 <= native_log2
            || evaluation_log2 > 32
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let stripe_log2 = evaluation_log2.min(MAIN_QUOTIENT_STRIPE_LOG2_V1);
        let rows = 1_usize
            .checked_shl(u32::from(stripe_log2))
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let count = 1_usize
            .checked_shl(u32::from(evaluation_log2 - stripe_log2))
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        if ordinal >= count {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let full_root =
            goldilocks_primitive_root_v1(evaluation_log2).map_err(map_transparent_error_v1)?;
        Ok(Self {
            rows,
            count,
            ordinal,
            next_stride: rows >> native_log2,
            root: full_root.pow(count as u128),
            shift: F(GOLDILOCKS_GENERATOR_V1).mul(full_root.pow(ordinal as u128)),
        })
    }

    /// Evaluate even when the masked polynomial degree exceeds the stripe size.
    ///
    /// Folding `a_k * shift^k` into lane `k mod rows` before the FFT evaluates
    /// the original polynomial exactly; truncating coefficients would be wrong.
    #[cfg(test)]
    pub(super) fn evaluate_v1(
        self,
        coefficients: &[F],
    ) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1> {
        let mut values = ZeroizingMainTraceColumnV1(Vec::new());
        values
            .0
            .try_reserve_exact(self.rows)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        values.0.resize(self.rows, F::ZERO);
        self.fold_into_v1(coefficients, &mut values)?;
        goldilocks_fft_v1(&mut values, self.root).map_err(map_transparent_error_v1)?;
        Ok(values)
    }
    /// Fold every shifted coefficient into its exact stripe lane, including
    /// masked coefficients above the stripe degree. No output allocation lives
    /// outside the caller's already admitted clearing matrix.
    pub(super) fn fold_into_v1(
        self,
        coefficients: &[F],
        values: &mut [F],
    ) -> Result<(), ZkX509StarkErrorV1> {
        const TILE_ROWS_V1: usize = 1 << 14;
        let parallel = self.rows >= 1 << 16 && rayon::current_num_threads() > 1;
        if self.rows < 2
            || !self.rows.is_power_of_two()
            || self.rows > 1 << MAIN_QUOTIENT_STRIPE_LOG2_V1
            || values.len() != self.rows
            || F::canonical(self.shift.0).is_none()
            || self.shift == F::ZERO
            || if parallel {
                coefficients
                    .par_chunks(TILE_ROWS_V1)
                    .any(|tile| tile.iter().any(|value| F::canonical(value.0).is_none()))
            } else {
                coefficients
                    .iter()
                    .any(|value| F::canonical(value.0).is_none())
            }
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        if parallel {
            // A tile owns disjoint output lanes. Every wrap visits those lanes
            // in the original coefficient order, including the masked suffix.
            // Only public shift powers are recomputed; no matrix or private
            // reduction scratch is allocated by this partition.
            values
                .par_chunks_mut(TILE_ROWS_V1)
                .enumerate()
                .for_each(|(tile, values)| {
                    values.fill(F::ZERO);
                    let first = tile * TILE_ROWS_V1;
                    for (wrap, coefficients) in coefficients.chunks(self.rows).enumerate() {
                        if first >= coefficients.len() {
                            break;
                        }
                        let mut power = self.shift.pow((wrap * self.rows + first) as u128);
                        for (target, coefficient) in values.iter_mut().zip(&coefficients[first..]) {
                            *target = target.add(coefficient.mul(power));
                            power = power.mul(self.shift);
                        }
                    }
                });
            return Ok(());
        }
        values.fill(F::ZERO);
        let mut shift_power = F::ONE;
        for (index, coefficient) in coefficients.iter().enumerate() {
            let lane = index % self.rows;
            values[lane] = values[lane].add(coefficient.mul(shift_power));
            shift_power = shift_power.mul(self.shift);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1;

    fn original_fold_v1(stripe: MainQuotientStripeV1, coefficients: &[F], values: &mut [F]) {
        assert!(
            coefficients
                .iter()
                .all(|value| F::canonical(value.0).is_some())
        );
        values.fill(F::ZERO);
        let mut power = F::ONE;
        for (index, coefficient) in coefficients.iter().enumerate() {
            let lane = index % stripe.rows;
            values[lane] = values[lane].add(coefficient.mul(power));
            power = power.mul(stripe.shift);
        }
    }

    #[test]
    fn tiled_fold_preserves_every_lane_wrap_and_original_allocation() {
        let rows = 1 << 16;
        for workers in [1, 4, 20] {
            rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap()
                .install(|| {
                    let stripe = MainQuotientStripeV1::new_v1(16, 17, 0).unwrap();
                    // Explicit public stripes also exercise more than two wraps
                    // and a partial last tile; production masked tails wrap once.
                    let stripe = MainQuotientStripeV1 { rows, ..stripe };
                    for length in [0, 1, rows - 1, rows, rows + 1816, 3 * rows + 71] {
                        let coefficients = (0..length)
                            .map(|i| F(GOLDILOCKS_MODULUS_V1 - 1 - (i as u64 % 7919)))
                            .collect::<Vec<_>>();
                        let mut expected = vec![F(19); rows];
                        original_fold_v1(stripe, &coefficients, &mut expected);
                        let mut actual = vec![F(19); rows];
                        let allocation = (actual.as_ptr(), actual.capacity());
                        stripe.fold_into_v1(&coefficients, &mut actual).unwrap();
                        assert_eq!(actual, expected, "workers={workers}, length={length}");
                        assert_eq!((actual.as_ptr(), actual.capacity()), allocation);
                    }
                    let mut coefficients = vec![F::ONE; rows + 1816];
                    *coefficients.last_mut().unwrap() = F(GOLDILOCKS_MODULUS_V1);
                    let mut actual = vec![F(19); rows];
                    assert!(stripe.fold_into_v1(&coefficients, &mut actual).is_err());
                    assert!(actual.iter().all(|value| *value == F(19)));
                });
        }
    }

    #[test]
    #[ignore = "native log19 quotient folding cost diagnostic"]
    fn native19_tiled_fold_cost_preserves_every_output() {
        let stripe = MainQuotientStripeV1::new_v1(19, 22, 7).unwrap();
        let coefficients = (0..stripe.rows + 1816)
            .map(|i| F(GOLDILOCKS_MODULUS_V1 - 1 - (i as u64 % 7919)))
            .collect::<Vec<_>>();
        let mut original = ZeroizingMainTraceColumnV1(vec![F::ZERO; stripe.rows]);
        let mut tiled = ZeroizingMainTraceColumnV1(vec![F::ZERO; stripe.rows]);
        for round in 0..3 {
            let mut times = [std::time::Duration::ZERO; 2];
            for mode in if round % 2 == 0 { [0, 1] } else { [1, 0] } {
                let start = std::time::Instant::now();
                for _ in 0..16 {
                    if mode == 0 {
                        original_fold_v1(stripe, &coefficients, &mut original);
                    } else {
                        stripe.fold_into_v1(&coefficients, &mut tiled).unwrap();
                    }
                }
                times[mode] = start.elapsed();
            }
            assert_eq!(&*original, &*tiled);
            eprintln!(
                "native19_fold round={round} original={:?} tiled={:?}",
                times[0], times[1]
            );
        }
    }

    #[test]
    fn folded_coefficient_adapter_rejects_bad_shapes_before_private_output_mutation() {
        let valid = MainQuotientStripeV1::new_v1(2, 4, 0).unwrap();
        for (stripe, coefficients, rows) in [
            (valid, vec![F(GOLDILOCKS_MODULUS_V1)], 16),
            (valid, vec![F::ONE], 15),
            (MainQuotientStripeV1 { rows: 0, ..valid }, vec![F::ONE], 16),
            (
                MainQuotientStripeV1 {
                    shift: F::ZERO,
                    ..valid
                },
                vec![F::ONE],
                16,
            ),
            (
                MainQuotientStripeV1 {
                    shift: F(GOLDILOCKS_MODULUS_V1),
                    ..valid
                },
                vec![F::ONE],
                16,
            ),
        ] {
            let mut values = vec![F(19); rows];
            assert!(stripe.fold_into_v1(&coefficients, &mut values).is_err());
            assert_eq!(values, vec![F(19); rows]);
        }
        let mut values = vec![F(19); 16];
        valid.fold_into_v1(&[], &mut values).unwrap();
        assert_eq!(values, vec![F::ZERO; 16]);
    }

    #[test]
    fn interleaved_stripes_preserve_full_domain_indices_and_native_translation() {
        let native_log = 18;
        let full_log = 21;
        let full_rows = 1 << full_log;
        let full_root = goldilocks_primitive_root_v1(full_log).unwrap();
        let native_root = goldilocks_primitive_root_v1(native_log).unwrap();
        for ordinal in 0..4 {
            let stripe = MainQuotientStripeV1::new_v1(native_log, full_log, ordinal).unwrap();
            assert_eq!(
                (stripe.rows, stripe.count, stripe.next_stride),
                (1 << 19, 4, 2)
            );
            for row in [0, 1, stripe.rows / 2, stripe.rows - 2, stripe.rows - 1] {
                let global = ordinal + stripe.count * row;
                let x = stripe.shift.mul(stripe.root.pow(row as u128));
                assert_eq!(
                    x,
                    F(GOLDILOCKS_GENERATOR_V1).mul(full_root.pow(global as u128))
                );
                let next = (row + stripe.next_stride) % stripe.rows;
                let global_next = (global + (full_rows >> native_log)) % full_rows;
                assert_eq!(ordinal + stripe.count * next, global_next);
                assert_eq!(
                    stripe.shift.mul(stripe.root.pow(next as u128)),
                    x.mul(native_root)
                );
            }
        }
    }

    #[test]
    fn shifted_folded_fft_matches_full_coset_and_independent_horner() {
        // Small explicit stripes exercise coefficients above the stripe degree,
        // including more than two wraps; production uses the same evaluator.
        let full_log = 7;
        let full_root = goldilocks_primitive_root_v1(full_log).unwrap();
        let coefficients = (0..97)
            .map(|i| F::reduce(u128::try_from(i * i + 7).expect("positive fixture coefficient")))
            .collect::<Vec<_>>();
        let full = goldilocks_evaluate_coset_v1(
            &coefficients,
            1 << full_log,
            full_root,
            F(GOLDILOCKS_GENERATOR_V1),
        )
        .unwrap();
        for ordinal in 0..8 {
            let stripe = MainQuotientStripeV1 {
                rows: 16,
                count: 8,
                ordinal,
                next_stride: 1,
                root: full_root.pow(8),
                shift: F(GOLDILOCKS_GENERATOR_V1).mul(full_root.pow(ordinal as u128)),
            };
            let actual = stripe.evaluate_v1(&coefficients).unwrap();
            for (row, value) in actual.iter().enumerate() {
                let x = stripe.shift.mul(stripe.root.pow(row as u128));
                let horner = coefficients
                    .iter()
                    .rev()
                    .fold(F::ZERO, |sum, a| sum.mul(x).add(*a));
                assert_eq!(*value, horner);
                assert_eq!(*value, full[ordinal + stripe.count * row]);
            }
        }
    }

    #[test]
    fn stripe_shape_and_noncanonical_coefficients_reject() {
        for (native, evaluation, ordinal) in [(20, 21, 0), (19, 19, 0), (19, 33, 0), (19, 22, 8)] {
            assert!(MainQuotientStripeV1::new_v1(native, evaluation, ordinal).is_err());
        }
        let stripe = MainQuotientStripeV1::new_v1(2, 4, 0).unwrap();
        assert_eq!(
            (
                stripe.rows,
                stripe.count,
                stripe.ordinal,
                stripe.next_stride
            ),
            (16, 1, 0, 4)
        );
        assert!(stripe.evaluate_v1(&[F(GOLDILOCKS_MODULUS_V1)]).is_err());
        assert_eq!(&*stripe.evaluate_v1(&[]).unwrap(), &[F::ZERO; 16]);
    }

    #[test]
    fn production_log19_stripes_retain_the_masked_high_degree_tail() {
        let n = 1_usize << 19;
        let mut coefficients = ZeroizingMainTraceColumnV1(vec![F::ZERO; n + 1816]);
        coefficients[0] = F(3);
        coefficients[1] = F(5);
        coefficients[n - 1] = F(7);
        coefficients[n] = F(11);
        coefficients[n + 1815] = F(13);
        for ordinal in [0, 7] {
            let stripe = MainQuotientStripeV1::new_v1(19, 22, ordinal).unwrap();
            let evaluated = stripe.evaluate_v1(&coefficients).unwrap();
            for row in [0, 1, 31, n / 2, n - 1] {
                let x = stripe.shift.mul(stripe.root.pow(row as u128));
                let expected = F(3)
                    .add(F(5).mul(x))
                    .add(F(7).mul(x.pow((n - 1) as u128)))
                    .add(F(11).mul(x.pow(n as u128)))
                    .add(F(13).mul(x.pow((n + 1815) as u128)));
                assert_eq!(evaluated[row], expected);
                let next = (row + stripe.next_stride) % stripe.rows;
                assert_eq!(
                    stripe.shift.mul(stripe.root.pow(next as u128)),
                    x.mul(goldilocks_primitive_root_v1(19).unwrap())
                );
            }
        }
    }
}
