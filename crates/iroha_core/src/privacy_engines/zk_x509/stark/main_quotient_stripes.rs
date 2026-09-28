//! Bounded interleaved quotient cosets with unchanged full-domain row order.

use super::*;
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
        let full_root = goldilocks_primitive_root_v1(evaluation_log2)
            .map_err(map_transparent_error_v1)?;
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
    pub(super) fn evaluate_v1(
        self,
        coefficients: &[F],
    ) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1> {
        if coefficients.iter().any(|value| F::canonical(value.0).is_none()) {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let mut values = ZeroizingMainTraceColumnV1(Vec::new());
        values.0.try_reserve_exact(self.rows)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        values.0.resize(self.rows, F::ZERO);
        let mut shift_power = F::ONE;
        for (index, coefficient) in coefficients.iter().enumerate() {
            let lane = index % self.rows;
            values[lane] = values[lane].add(coefficient.mul(shift_power));
            shift_power = shift_power.mul(self.shift);
        }
        goldilocks_fft_v1(&mut values, self.root).map_err(map_transparent_error_v1)?;
        Ok(values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interleaved_stripes_preserve_full_domain_indices_and_native_translation() {
        let native_log = 18;
        let full_log = 21;
        let full_rows = 1 << full_log;
        let full_root = goldilocks_primitive_root_v1(full_log).unwrap();
        let native_root = goldilocks_primitive_root_v1(native_log).unwrap();
        for ordinal in 0..4 {
            let stripe = MainQuotientStripeV1::new_v1(native_log, full_log, ordinal).unwrap();
            assert_eq!((stripe.rows, stripe.count, stripe.next_stride), (1 << 19, 4, 2));
            for row in [0, 1, stripe.rows / 2, stripe.rows - 2, stripe.rows - 1] {
                let global = ordinal + stripe.count * row;
                let x = stripe.shift.mul(stripe.root.pow(row as u128));
                assert_eq!(x, F(GOLDILOCKS_GENERATOR_V1).mul(full_root.pow(global as u128)));
                let next = (row + stripe.next_stride) % stripe.rows;
                let global_next = (global + (full_rows >> native_log)) % full_rows;
                assert_eq!(ordinal + stripe.count * next, global_next);
                assert_eq!(stripe.shift.mul(stripe.root.pow(next as u128)), x.mul(native_root));
            }
        }
    }

    #[test]
    fn shifted_folded_fft_matches_full_coset_and_independent_horner() {
        // Small explicit stripes exercise coefficients above the stripe degree,
        // including more than two wraps; production uses the same evaluator.
        let full_log = 7;
        let full_root = goldilocks_primitive_root_v1(full_log).unwrap();
        let coefficients = (0..97).map(|i| F::reduce((i * i + 7) as u128)).collect::<Vec<_>>();
        let full = goldilocks_evaluate_coset_v1(
            &coefficients, 1 << full_log, full_root, F(GOLDILOCKS_GENERATOR_V1),
        ).unwrap();
        for ordinal in 0..8 {
            let stripe = MainQuotientStripeV1 {
                rows: 16, count: 8, ordinal, next_stride: 1,
                root: full_root.pow(8),
                shift: F(GOLDILOCKS_GENERATOR_V1).mul(full_root.pow(ordinal as u128)),
            };
            let actual = stripe.evaluate_v1(&coefficients).unwrap();
            for (row, value) in actual.iter().enumerate() {
                let x = stripe.shift.mul(stripe.root.pow(row as u128));
                let horner = coefficients.iter().rev().fold(F::ZERO, |sum, a| sum.mul(x).add(*a));
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
        assert_eq!((stripe.rows, stripe.count, stripe.ordinal, stripe.next_stride), (16, 1, 0, 4));
        assert!(stripe.evaluate_v1(&[F(GOLDILOCKS_MODULUS_V1)]).is_err());
        assert_eq!(&*stripe.evaluate_v1(&[]).unwrap(), &[F::ZERO; 16]);
    }
}
