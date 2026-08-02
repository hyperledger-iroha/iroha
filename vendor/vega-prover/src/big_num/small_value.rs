// Copyright (c) Microsoft Corporation.
// SPDX-License-Identifier: MIT

//! Small-value optimization for sumcheck inner products.
//!
//! When polynomial values are small integers (e.g., 0/1 from boolean circuits),
//! we can compute inner products using integer arithmetic instead of field
//! multiplication, then multiply by the equality polynomial coefficient.
//!
//! This saves 2 field multiplications per element in the sumcheck inner loop:
//! instead of computing `(A*B - C)` as field arithmetic (2 field muls + 1 sub),
//! we compute it as `i64 * i64 - i64` (native integer ops), then accumulate
//! `eq[i] * small_val` using `mul_4_by_1` (4 limb muls instead of 16).

use super::{
    field_reduction_constants::FieldReductionConstants, limbs::reduce_8_mod_4,
    montgomery::MontgomeryLimbs,
};
use ff::PrimeField;

/// Maximum absolute value for "small" field elements stored as i64.
///
/// Chosen so that ALL i128 arithmetic in consumers is overflow-free:
///   - Products of originals:    a*b <= V^2   ~ 2^124         fits i128
///   - Cross-product sums:       a*b+c*d <= 2V^2 ~ 2^125      fits i128
///   - Products of differences:  (2V)^2 = 4V^2  ~ 2^126       fits i128
///   - Cross-product diff sums:  2*(2V)^2 = 8V^2 ~ 2^127      fits i128
///     Exact: 8*(2^62-1)^2 = 2^127-2^66+8 < 2^127-1 = i128::MAX
///
/// Practical impact: zero. Typical boolean-circuit witness values are well within this bound.
const SMALL_VALUE_MAX: u64 = (1u64 << 62) - 1;

/// Try to convert field elements to i64 values.
///
/// Convert field elements to i64 values, storing 0 for elements that don't fit.
///
/// This never fails -- large values are replaced with 0
/// and their positions are recorded for separate field-arithmetic correction.
///
/// Returns (i64 values, positions of large values that were zeroed).
#[inline(never)]
pub(crate) fn to_small_vec_or_zero<F: PrimeField + FieldReductionConstants>(
    poly: &[F],
) -> (Vec<i64>, Vec<usize>) {
    let mut result = Vec::with_capacity(poly.len());
    let mut large_positions = Vec::new();
    let p = &<F as FieldReductionConstants>::MODULUS;

    for (idx, f) in poly.iter().enumerate() {
        let repr = f.to_repr();
        let bytes = repr.as_ref();

        let l0 = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
        let l1 = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
        let l2 = u64::from_le_bytes(bytes[16..24].try_into().unwrap());
        let l3 = u64::from_le_bytes(bytes[24..32].try_into().unwrap());

        // Check small positive: value <= SMALL_VALUE_MAX
        if l1 == 0 && l2 == 0 && l3 == 0 && l0 <= SMALL_VALUE_MAX {
            result.push(l0 as i64);
            continue;
        }

        // Check small negative: p - value <= SMALL_VALUE_MAX
        let (d0, borrow0) = p[0].overflowing_sub(l0);
        let (d1a, borrow1a) = p[1].overflowing_sub(l1);
        let (d1, borrow1b) = d1a.overflowing_sub(borrow0 as u64);
        let borrow1 = borrow1a | borrow1b;
        let (d2a, borrow2a) = p[2].overflowing_sub(l2);
        let (d2, borrow2b) = d2a.overflowing_sub(borrow1 as u64);
        let borrow2 = borrow2a | borrow2b;
        let (d3a, _borrow3a) = p[3].overflowing_sub(l3);
        let (d3, _) = d3a.overflowing_sub(borrow2 as u64);

        if d1 == 0 && d2 == 0 && d3 == 0 && d0 > 0 && d0 <= SMALL_VALUE_MAX {
            result.push(-(d0 as i64));
            continue;
        }

        // Large value: store 0, record position for field correction
        result.push(0);
        large_positions.push(idx);
    }
    (result, large_positions)
}

/// Signed accumulator for sumcheck with small polynomial values.
///
/// Accumulates `field_element * small_integer` products using separate
/// positive and negative buckets, reducing only at the end.
///
/// Uses 7 limbs (448 bits) per bucket. Each product of
/// field (256 bits) * i128 (128 bits) = 384 bits, leaving 64 bits of
/// headroom -- sufficient for up to 2^64 accumulations per bucket.
#[derive(Clone, Copy)]
pub(crate) struct SmallAccumulator {
    pos: [u64; 7],
    neg: [u64; 7],
}

impl SmallAccumulator {
    #[inline(always)]
    pub fn zero() -> Self {
        Self {
            pos: [0u64; 7],
            neg: [0u64; 7],
        }
    }

    /// Accumulate: acc += field_limbs * val
    ///
    /// field_limbs is in Montgomery form (4 limbs). val is a small integer.
    /// The product is at most 5 limbs (256 + 64 = 320 bits for single-limb val)
    /// or 6 limbs (256 + 128 = 384 bits for two-limb val).
    #[inline(always)]
    pub fn accumulate(&mut self, field_limbs: &[u64; 4], val: i128) {
        if val == 0 {
            return;
        }

        let abs_val = val.unsigned_abs();
        let target = if val > 0 {
            &mut self.pos
        } else {
            &mut self.neg
        };

        let lo = abs_val as u64;
        let hi = (abs_val >> 64) as u64;

        // field_limbs * lo: 4 limb muls -> 5-limb result, accumulated into target
        let mut carry = 0u128;
        for j in 0..4 {
            let prod = (field_limbs[j] as u128) * (lo as u128) + (target[j] as u128) + carry;
            target[j] = prod as u64;
            carry = prod >> 64;
        }
        // Propagate carry through remaining limbs
        for item in target.iter_mut().take(7).skip(4) {
            let sum = (*item as u128) + carry;
            *item = sum as u64;
            carry = sum >> 64;
            if carry == 0 {
                break;
            }
        }
        debug_assert!(carry == 0, "SmallAccumulator lo-part carry overflow");

        // field_limbs * hi shifted by 1 limb (if hi != 0)
        if hi != 0 {
            carry = 0;
            for j in 0..4 {
                let prod =
                    (field_limbs[j] as u128) * (hi as u128) + (target[j + 1] as u128) + carry;
                target[j + 1] = prod as u64;
                carry = prod >> 64;
            }
            for item in target.iter_mut().take(7).skip(5) {
                let sum = (*item as u128) + carry;
                *item = sum as u64;
                carry = sum >> 64;
                if carry == 0 {
                    break;
                }
            }
            debug_assert!(carry == 0, "SmallAccumulator hi-part carry overflow");
        }
    }

    /// Reduce the accumulator to a field element: reduce(pos) - reduce(neg).
    #[inline]
    pub fn reduce<F: MontgomeryLimbs + FieldReductionConstants + PrimeField>(&self) -> F {
        let pos_f = reduce_7_to_field::<F>(&self.pos);
        let neg_f = reduce_7_to_field::<F>(&self.neg);
        pos_f - neg_f
    }
}

impl std::ops::AddAssign for SmallAccumulator {
    #[inline]
    fn add_assign(&mut self, other: Self) {
        let mut carry = 0u128;
        for i in 0..7 {
            let sum = (self.pos[i] as u128) + (other.pos[i] as u128) + carry;
            self.pos[i] = sum as u64;
            carry = sum >> 64;
        }
        debug_assert!(carry == 0, "SmallAccumulator pos AddAssign overflow");
        carry = 0;
        for i in 0..7 {
            let sum = (self.neg[i] as u128) + (other.neg[i] as u128) + carry;
            self.neg[i] = sum as u64;
            carry = sum >> 64;
        }
        debug_assert!(carry == 0, "SmallAccumulator neg AddAssign overflow");
    }
}

/// Reduce a 7-limb unsigned value to a field element.
///
/// The 7-limb value represents `R * sum(eq[i] * val[i])` in Montgomery form.
/// We compute `acc mod p` and interpret the result as Montgomery form,
/// which correctly represents `sum(eq[i] * val[i])` as a field element.
#[inline]
fn reduce_7_to_field<F: MontgomeryLimbs + FieldReductionConstants>(acc: &[u64; 7]) -> F {
    // Check if all limbs above 4 are zero (common fast path)
    if acc[4] == 0 && acc[5] == 0 && acc[6] == 0 {
        // Value is in [0, R). Reduce to [0, p) via up to MAX_REDC_SUB_CORRECTIONS subtractions.
        // For 254-bit primes like BN254 where R/p = 5, values up to 5p fit in 4 limbs.
        let mut base = [acc[0], acc[1], acc[2], acc[3]];
        for _ in 0..F::MAX_REDC_SUB_CORRECTIONS {
            if super::limbs::gte::<4>(&base, &F::MODULUS) {
                base = super::limbs::sub::<4>(&base, &F::MODULUS);
            }
        }
        return F::from_limbs(base);
    }

    // General case: use binary long division to reduce mod p
    let padded = [acc[0], acc[1], acc[2], acc[3], acc[4], acc[5], acc[6], 0];
    let reduced = reduce_8_mod_4(&padded, &F::MODULUS);
    F::from_limbs(reduced)
}

/// Generate tests for small-value optimization.
#[cfg(test)]
#[macro_export]
macro_rules! test_small_value {
    ($mod_name:ident, $field:ty) => {
        mod $mod_name {
            #[test]
            fn small_vec_or_zero() {
                $crate::big_num::small_value::tests::test_small_vec_or_zero_impl::<$field>();
            }
            #[test]
            fn small_accumulator() {
                $crate::big_num::small_value::tests::test_small_accumulator_impl::<$field>();
            }
            #[test]
            fn small_accumulator_i128() {
                $crate::big_num::small_value::tests::test_small_accumulator_i128_impl::<$field>();
            }
            #[test]
            fn small_accumulator_few_products() {
                $crate::big_num::small_value::tests::test_small_accumulator_few_products_impl::<
                    $field,
                >();
            }
        }
    };
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Test to_small_vec_or_zero with mixed small and large values
    pub(crate) fn test_small_vec_or_zero_impl<
        F: PrimeField + FieldReductionConstants + MontgomeryLimbs + Copy,
    >() {
        use rand::{SeedableRng, rngs::StdRng};
        let mut rng = StdRng::seed_from_u64(11111);

        // All small values: should have no large positions
        let vals: Vec<F> = (0..10).map(|i| F::from(i as u64)).collect();
        let (small, large) = to_small_vec_or_zero(&vals);
        assert!(large.is_empty());
        for (i, &v) in small.iter().enumerate() {
            assert_eq!(v, i as i64);
        }

        // Mix of small and large values
        let mixed = vec![
            F::from(5u64),
            F::random(&mut rng), // large
            -F::from(3u64),
            F::random(&mut rng), // large
            F::from(100u64),
        ];
        let (small, large) = to_small_vec_or_zero(&mixed);
        assert_eq!(small.len(), 5);
        assert_eq!(small[0], 5);
        assert_eq!(small[1], 0); // large -> 0
        assert_eq!(small[2], -3);
        assert_eq!(small[3], 0); // large -> 0
        assert_eq!(small[4], 100);
        assert_eq!(large, vec![1, 3]);

        // Threshold boundary: values at exactly SMALL_VALUE_MAX should be accepted
        let boundary = vec![F::from(SMALL_VALUE_MAX), -F::from(SMALL_VALUE_MAX)];
        let (small, large) = to_small_vec_or_zero(&boundary);
        assert!(large.is_empty(), "values at threshold should be small");
        assert_eq!(small[0], SMALL_VALUE_MAX as i64);
        assert_eq!(small[1], -(SMALL_VALUE_MAX as i64));

        // Values just above threshold should be large
        let above = vec![F::from(SMALL_VALUE_MAX + 1)];
        let (small, large) = to_small_vec_or_zero(&above);
        assert_eq!(large, vec![0], "values above threshold should be large");
        assert_eq!(small[0], 0);
    }

    /// Test SmallAccumulator matches field arithmetic
    pub(crate) fn test_small_accumulator_impl<
        F: PrimeField + FieldReductionConstants + MontgomeryLimbs + Copy,
    >() {
        use rand::{SeedableRng, rngs::StdRng};
        let mut rng = StdRng::seed_from_u64(12345);

        let n = 1000;
        let eq_vals: Vec<F> = (0..n).map(|_| F::random(&mut rng)).collect();
        let small_vals: Vec<i64> = (0..n)
            .map(|i| ((i as i64) % 201) - 100) // values in [-100, 100]
            .collect();

        // Compute expected result using field arithmetic
        let expected: F = eq_vals
            .iter()
            .zip(small_vals.iter())
            .map(|(&e, &s)| {
                if s >= 0 {
                    e * F::from(s as u64)
                } else {
                    -(e * F::from((-s) as u64))
                }
            })
            .sum();

        // Compute using SmallAccumulator
        let mut acc = SmallAccumulator::zero();
        for (&e, &s) in eq_vals.iter().zip(small_vals.iter()) {
            acc.accumulate(e.to_limbs(), s as i128);
        }
        let result = acc.reduce::<F>();

        assert_eq!(result, expected, "SmallAccumulator sum mismatch");
    }

    /// Test SmallAccumulator with i128 values
    pub(crate) fn test_small_accumulator_i128_impl<
        F: PrimeField + FieldReductionConstants + MontgomeryLimbs + Copy,
    >() {
        use rand::{SeedableRng, rngs::StdRng};
        let mut rng = StdRng::seed_from_u64(54321);

        let n = 100;
        let eq_vals: Vec<F> = (0..n).map(|_| F::random(&mut rng)).collect();
        // Use values that need i128 (product of two i64 values)
        let a_vals: Vec<i64> = (0..n).map(|i| (i as i64 + 1) * 1_000_000).collect();
        let b_vals: Vec<i64> = (0..n).map(|i| (i as i64 + 1) * 2_000_000).collect();

        // Compute expected: sum eq[i] * (a[i] * b[i])
        let expected: F = eq_vals
            .iter()
            .zip(a_vals.iter().zip(b_vals.iter()))
            .map(|(&e, (&a, &b))| {
                let prod = a as i128 * b as i128;
                if prod >= 0 {
                    e * F::from(prod as u64)
                } else {
                    -(e * F::from((-prod) as u64))
                }
            })
            .sum();

        let mut acc = SmallAccumulator::zero();
        for (&e, (&a, &b)) in eq_vals.iter().zip(a_vals.iter().zip(b_vals.iter())) {
            let prod = a as i128 * b as i128;
            acc.accumulate(e.to_limbs(), prod);
        }
        let result = acc.reduce::<F>();

        assert_eq!(result, expected, "SmallAccumulator i128 sum mismatch");
    }

    /// Test reduce_7_to_field fast path with few accumulations.
    ///
    /// For 254-bit primes (like BN254), R/p ~ 5, so values up to 5p fit in 4 limbs.
    /// The fast path must perform multiple subtractions, not just one.
    pub(crate) fn test_small_accumulator_few_products_impl<
        F: PrimeField + FieldReductionConstants + MontgomeryLimbs + Copy,
    >() {
        use rand::{SeedableRng, rngs::StdRng};
        let mut rng = StdRng::seed_from_u64(77777);

        // Test with 1..=MAX_REDC_SUB_CORRECTIONS+1 products to exercise the fast path
        // where the sum fits in 4 limbs but needs multiple subtractions.
        let max_n = F::MAX_REDC_SUB_CORRECTIONS + 2;
        for n in 1..=max_n {
            let eq_vals: Vec<F> = (0..n).map(|_| F::random(&mut rng)).collect();

            // val = 1: sum of field elements in Montgomery form, likely fits in 4 limbs for small n
            let expected: F = eq_vals.iter().copied().sum();

            let mut acc = SmallAccumulator::zero();
            for &e in &eq_vals {
                acc.accumulate(e.to_limbs(), 1i128);
            }
            let result = acc.reduce::<F>();

            assert_eq!(
                result, expected,
                "SmallAccumulator few-products mismatch (n={n})"
            );
        }
    }
}
