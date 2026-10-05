//! Scalar arithmetic modulo a word-sized modulus.
//!
//! Every function is total: it returns the same value on every target and never
//! panics. A zero modulus has no residues, so the arithmetic functions return
//! zero for it and the fallible ones return `None`.
//!
//! [`add_mod_u64`], [`sub_mod_u64`] and [`mul_mod_u64`] accept unreduced
//! operands and reduce them exactly. When both operands of an addition or a
//! subtraction are already canonical the result is produced by the branch-free
//! kernels of [`crate::constant_time`]; products go through a `u128` remainder
//! and are not constant-time. Secret-dependent fixed-modulus code uses
//! [`crate::constant_time::FixedModulus`] instead.
use crate::constant_time::{add_mod_canonical_u64, sub_mod_canonical_u64};

/// Arithmetic modulo one modulus, as used by the generic transforms.
///
/// Implementations return canonical residues in `[0, modulus)` for canonical
/// operands. [`WordModulus`] is the variable-time `u128` reference;
/// [`crate::constant_time::FixedModulus`] is the branch-free Montgomery form.
pub trait ModularArithmetic {
    /// The modulus.
    fn modulus(&self) -> u64;
    /// `lhs + rhs` modulo the modulus.
    fn add(&self, lhs: u64, rhs: u64) -> u64;
    /// `lhs - rhs` modulo the modulus.
    fn sub(&self, lhs: u64, rhs: u64) -> u64;
    /// `lhs * rhs` modulo the modulus.
    fn mul(&self, lhs: u64, rhs: u64) -> u64;
    /// `base ^ exponent` modulo the modulus by square-and-multiply.
    ///
    /// The exponent is treated as public: the loop branches on its bits.
    fn pow(&self, mut base: u64, mut exponent: u64) -> u64 {
        let mut result = 1_u64;
        while exponent != 0 {
            if exponent & 1 == 1 {
                result = self.mul(result, base);
            }
            base = self.mul(base, base);
            exponent >>= 1;
        }
        result
    }
}

/// Variable-time arithmetic modulo an arbitrary word modulus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WordModulus(pub u64);

impl ModularArithmetic for WordModulus {
    fn modulus(&self) -> u64 {
        self.0
    }
    fn add(&self, lhs: u64, rhs: u64) -> u64 {
        add_mod_u64(lhs, rhs, self.0)
    }
    fn sub(&self, lhs: u64, rhs: u64) -> u64 {
        sub_mod_u64(lhs, rhs, self.0)
    }
    fn mul(&self, lhs: u64, rhs: u64) -> u64 {
        mul_mod_u64(lhs, rhs, self.0)
    }
}

/// Return the low 64 bits of an unsigned 128-bit value.
#[must_use]
pub const fn low_u64_from_u128(value: u128) -> u64 {
    let [b0, b1, b2, b3, b4, b5, b6, b7, _, _, _, _, _, _, _, _] = value.to_le_bytes();
    u64::from_le_bytes([b0, b1, b2, b3, b4, b5, b6, b7])
}

/// Return the low 64 bits of the two's-complement encoding of a signed 128-bit value.
#[must_use]
pub const fn low_u64_from_i128(value: i128) -> u64 {
    let [b0, b1, b2, b3, b4, b5, b6, b7, _, _, _, _, _, _, _, _] = value.to_le_bytes();
    u64::from_le_bytes([b0, b1, b2, b3, b4, b5, b6, b7])
}

/// `(lhs + rhs) mod modulus` for any operands; zero when `modulus` is zero.
#[must_use]
pub fn add_mod_u64(lhs: u64, rhs: u64, modulus: u64) -> u64 {
    if modulus == 0 {
        return 0;
    }
    if lhs < modulus && rhs < modulus {
        return add_mod_canonical_u64(lhs, rhs, modulus);
    }
    low_u64_from_u128((u128::from(lhs) + u128::from(rhs)) % u128::from(modulus))
}

/// `(lhs - rhs) mod modulus` as the least non-negative residue; zero when `modulus` is zero.
#[must_use]
pub fn sub_mod_u64(lhs: u64, rhs: u64, modulus: u64) -> u64 {
    if modulus == 0 {
        return 0;
    }
    if lhs < modulus && rhs < modulus {
        return sub_mod_canonical_u64(lhs, rhs, modulus);
    }
    reduce_i128_to_u64_mod(i128::from(lhs) - i128::from(rhs), modulus)
}

/// `(lhs * rhs) mod modulus` for any operands; zero when `modulus` is zero.
#[must_use]
pub fn mul_mod_u64(lhs: u64, rhs: u64, modulus: u64) -> u64 {
    if modulus == 0 {
        return 0;
    }
    if (lhs | rhs) >> 32 == 0 {
        // Both operands fit 32 bits, so the product fits one word.
        return lhs * rhs % modulus;
    }
    low_u64_from_u128((u128::from(lhs) * u128::from(rhs)) % u128::from(modulus))
}

/// Least non-negative residue of a signed value; zero when `modulus` is zero.
///
/// This is Euclidean reduction: `-1` maps to `modulus - 1`.
#[must_use]
pub fn reduce_i128_to_u64_mod(value: i128, modulus: u64) -> u64 {
    if modulus == 0 {
        return 0;
    }
    low_u64_from_i128(value.rem_euclid(i128::from(modulus)))
}

/// Least non-negative residue of an unsigned 128-bit value.
///
/// Returns `None` only when `modulus` is zero.
#[must_use]
pub fn reduce_u128_to_u64_mod(value: u128, modulus: u64) -> Option<u64> {
    if modulus == 0 {
        return None;
    }
    u64::try_from(value % u128::from(modulus)).ok()
}

/// `base ^ exponent mod modulus` by square-and-multiply.
///
/// A zero exponent returns one for every modulus, including one and zero; the
/// loop branches on the exponent bits, which callers treat as public.
#[must_use]
pub fn mod_pow_u64(base: u64, exponent: u64, modulus: u64) -> u64 {
    WordModulus(modulus).pow(base, exponent)
}

/// Inverse of `value` modulo an odd prime by Fermat's little theorem.
///
/// Returns `None` when `modulus <= 2` or `value` is a multiple of `modulus`.
/// The result is an inverse only when `modulus` is prime.
#[must_use]
pub fn mod_inv_prime_u64(value: u64, modulus: u64) -> Option<u64> {
    if modulus <= 2 || value.is_multiple_of(modulus) {
        return None;
    }
    Some(mod_pow_u64(value, modulus - 2, modulus))
}

/// Greatest common divisor by the Euclidean algorithm; `gcd(x, 0) = x`.
#[must_use]
pub const fn gcd_u64(mut lhs: u64, mut rhs: u64) -> u64 {
    while rhs != 0 {
        let remainder = lhs % rhs;
        lhs = rhs;
        rhs = remainder;
    }
    lhs
}

/// Deterministic Miller-Rabin primality for every `u64`.
///
/// The seven bases `2, 325, 9375, 28178, 450775, 9780504, 1795265022` decide
/// primality for all 64-bit integers.
#[must_use]
pub fn is_prime_u64(candidate: u64) -> bool {
    const SMALL_PRIMES: [u64; 12] = [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37];
    const MILLER_RABIN_BASES: [u64; 7] = [2, 325, 9_375, 28_178, 450_775, 9_780_504, 1_795_265_022];
    if candidate < 2 {
        return false;
    }
    for prime in SMALL_PRIMES {
        if candidate == prime {
            return true;
        }
        if candidate.is_multiple_of(prime) {
            return false;
        }
    }
    let mut odd_factor = candidate - 1;
    let mut power_of_two = 0_u32;
    while odd_factor.is_multiple_of(2) {
        odd_factor /= 2;
        power_of_two = power_of_two.saturating_add(1);
    }
    'base: for base in MILLER_RABIN_BASES {
        let base = base % candidate;
        if base == 0 {
            continue;
        }
        let mut witness = mod_pow_u64(base, odd_factor, candidate);
        if witness == 1 || witness == candidate - 1 {
            continue;
        }
        for _ in 1..power_of_two {
            witness = mul_mod_u64(witness, witness, candidate);
            if witness == candidate - 1 {
                continue 'base;
            }
        }
        return false;
    }
    true
}

/// Whether `root` is a primitive `order`-th root of unity modulo `modulus`.
///
/// `order` must be even and greater than one. The test is `root^order = 1`
/// and `root^(order/2) = -1`, which is exact when `order` is a power of two.
#[must_use]
pub fn is_primitive_root_of_order(modulus: u64, root: u64, order: u64) -> bool {
    order > 1
        && order.is_multiple_of(2)
        && modulus > 2
        && root > 1
        && root < modulus
        && mod_pow_u64(root, order, modulus) == 1
        && mod_pow_u64(root, order / 2, modulus) == modulus - 1
}

/// Find a primitive `order`-th root of unity by trying generators `2..=max_candidate`.
///
/// Candidate `g` yields `g^((modulus - 1) / order)`; the first one that passes
/// the primitivity test is returned, so the result is the same on every host.
/// Returns `None` when `order` does not divide `modulus - 1` or no candidate
/// within the limit works.
#[must_use]
pub fn primitive_root_of_order_with_candidate_limit(
    modulus: u64,
    order: u64,
    max_candidate: u64,
) -> Option<u64> {
    if order <= 1 || modulus <= 2 || max_candidate < 2 || !(modulus - 1).is_multiple_of(order) {
        return None;
    }
    let exponent = (modulus - 1) / order;
    let half_order = order / 2;
    for candidate in 2..=max_candidate.min(modulus - 1) {
        let root = mod_pow_u64(candidate, exponent, modulus);
        if root != 1
            && mod_pow_u64(root, order, modulus) == 1
            && mod_pow_u64(root, half_order, modulus) == modulus - 1
        {
            return Some(root);
        }
    }
    None
}

/// Residue of a big-endian byte string modulo a word modulus by Horner's rule.
///
/// The accumulator stays canonical, so each step is a branch-free canonical
/// addition of a `u128`-reduced product. Returns zero when `modulus` is zero.
#[must_use]
pub fn reduce_be_bytes_mod_u64(bytes: &[u8], modulus: u64) -> u64 {
    if modulus == 0 {
        return 0;
    }
    bytes.iter().fold(0_u64, |accumulator, byte| {
        add_mod_canonical_u64(
            mul_mod_u64(accumulator, 256 % modulus, modulus),
            u64::from(*byte) % modulus,
            modulus,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference_add(lhs: u64, rhs: u64, modulus: u64) -> u64 {
        u64::try_from((u128::from(lhs) + u128::from(rhs)) % u128::from(modulus)).unwrap()
    }
    fn reference_sub(lhs: u64, rhs: u64, modulus: u64) -> u64 {
        let modulus = i128::from(modulus);
        u64::try_from((i128::from(lhs) - i128::from(rhs)).rem_euclid(modulus)).unwrap()
    }
    fn splitmix64(state: &mut u64) -> u64 {
        *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = *state;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }

    #[test]
    fn low_word_extraction_matches_truncation() {
        assert_eq!(low_u64_from_u128(u128::MAX), u64::MAX);
        assert_eq!(low_u64_from_u128((7_u128 << 64) | 9), 9);
        assert_eq!(low_u64_from_i128(-1), u64::MAX);
        assert_eq!(low_u64_from_i128(5), 5);
    }

    #[test]
    fn add_sub_match_the_wide_reference_for_reduced_and_unreduced_operands() {
        let mut state = 0xC2_u64;
        for modulus in [
            1,
            2,
            3,
            17,
            30_593,
            4_293_918_721,
            (1 << 62) + 1,
            u64::MAX - 58,
            u64::MAX,
        ] {
            let boundary = [
                0,
                1,
                modulus - 1,
                modulus,
                modulus.wrapping_add(1),
                u64::MAX,
            ];
            for &lhs in &boundary {
                for &rhs in &boundary {
                    assert_eq!(
                        add_mod_u64(lhs, rhs, modulus),
                        reference_add(lhs, rhs, modulus)
                    );
                    assert_eq!(
                        sub_mod_u64(lhs, rhs, modulus),
                        reference_sub(lhs, rhs, modulus)
                    );
                }
            }
            for _ in 0..512 {
                let lhs = splitmix64(&mut state);
                let rhs = splitmix64(&mut state);
                for (lhs, rhs) in [(lhs, rhs), (lhs % modulus, rhs % modulus)] {
                    assert_eq!(
                        add_mod_u64(lhs, rhs, modulus),
                        reference_add(lhs, rhs, modulus)
                    );
                    assert_eq!(
                        sub_mod_u64(lhs, rhs, modulus),
                        reference_sub(lhs, rhs, modulus)
                    );
                }
            }
        }
    }

    /// Assertions carried over from the retired `bfv-accel` `add_mod_prime` helper.
    #[test]
    fn add_handles_large_modulus_without_overflow() {
        let modulus = u64::MAX - 58;
        assert_eq!(add_mod_u64(modulus - 2, 10, modulus), 8);
        assert_eq!(add_mod_u64(u64::MAX, u64::MAX, modulus), 116);
        assert_eq!(add_mod_u64(1, 1, 0), 0);
    }

    /// Assertions carried over from the retired `bfv-accel` `sub_mod_prime` helper.
    #[test]
    fn sub_handles_large_modulus_and_unreduced_rhs() {
        let modulus = u64::MAX - 58;
        assert_eq!(sub_mod_u64(7, modulus - 11, modulus), 18);
        assert_eq!(sub_mod_u64(1, 1, 0), 0);
        assert_eq!(sub_mod_u64(3, 41, 17), 13);
    }

    #[test]
    fn scalar_helpers_handle_max_width_values() {
        let modulus = u64::MAX;
        assert_eq!(add_mod_u64(modulus - 1, modulus - 2, modulus), modulus - 3);
        assert_eq!(mul_mod_u64(modulus - 1, modulus - 2, modulus), 2);
        assert_eq!(sub_mod_u64(1, modulus - 1, modulus), 2);
        assert_eq!(reduce_i128_to_u64_mod(-1, modulus), modulus - 1);
    }

    #[test]
    fn zero_modulus_is_total() {
        assert_eq!(mul_mod_u64(3, 5, 0), 0);
        assert_eq!(reduce_i128_to_u64_mod(-9, 0), 0);
        assert_eq!(reduce_u128_to_u64_mod(9, 0), None);
        assert_eq!(mod_pow_u64(3, 0, 0), 1);
        assert_eq!(mod_pow_u64(3, 2, 0), 0);
        assert_eq!(mod_inv_prime_u64(3, 0), None);
        assert_eq!(reduce_be_bytes_mod_u64(&[1, 2, 3], 0), 0);
        assert!(!is_primitive_root_of_order(0, 3, 4));
        assert_eq!(primitive_root_of_order_with_candidate_limit(0, 4, 8), None);
    }

    #[test]
    fn signed_and_wide_reduction_use_least_non_negative_residues() {
        assert_eq!(reduce_i128_to_u64_mod(-1, 17), 16);
        assert_eq!(reduce_i128_to_u64_mod(-17, 17), 0);
        assert_eq!(reduce_i128_to_u64_mod(i128::MIN, 17), {
            // -2^127 mod 17: 2^4 = -1, so 2^127 = 2^3 * (2^4)^31 = -8, hence -2^127 = 8.
            8
        });
        assert_eq!(reduce_i128_to_u64_mod(i128::MAX, u64::MAX), {
            // 2^127 - 1 with 2^64 = 1 mod (2^64 - 1): 2^127 = 2^63.
            (1 << 63) - 1
        });
        assert_eq!(reduce_u128_to_u64_mod(u128::MAX, u64::MAX), Some(0));
        assert_eq!(reduce_u128_to_u64_mod(35, 17), Some(1));
    }

    #[test]
    fn mul_and_pow_match_small_exhaustive_reference() {
        for modulus in 1_u64..=40 {
            for lhs in 0..modulus + 3 {
                for rhs in 0..modulus + 3 {
                    assert_eq!(mul_mod_u64(lhs, rhs, modulus), lhs * rhs % modulus);
                }
                let mut power = 1 % modulus;
                for exponent in 1..12_u64 {
                    power = power * (lhs % modulus) % modulus;
                    assert_eq!(mod_pow_u64(lhs, exponent, modulus), power);
                }
            }
        }
        assert_eq!(
            mod_pow_u64(5, 0, 1),
            1,
            "zero exponent returns one unreduced"
        );
    }

    #[test]
    fn prime_inverse_is_an_inverse_and_rejects_degenerate_inputs() {
        for modulus in [3_u64, 17, 30_593, 4_293_918_721] {
            for value in [1, 2, modulus - 1, modulus + 2] {
                let inverse = mod_inv_prime_u64(value, modulus).expect("invertible");
                assert_eq!(mul_mod_u64(value, inverse, modulus), 1);
            }
            assert_eq!(mod_inv_prime_u64(0, modulus), None);
            assert_eq!(mod_inv_prime_u64(modulus, modulus), None);
        }
        assert_eq!(mod_inv_prime_u64(1, 2), None);
        assert_eq!(mod_inv_prime_u64(1, 1), None);
    }

    #[test]
    fn gcd_matches_known_values() {
        assert_eq!(gcd_u64(0, 0), 0);
        assert_eq!(gcd_u64(12, 0), 12);
        assert_eq!(gcd_u64(0, 12), 12);
        assert_eq!(gcd_u64(30_593, 30_977), 1);
        assert_eq!(gcd_u64(2 * 3 * 5 * 7, 3 * 7 * 11), 21);
        assert_eq!(gcd_u64(u64::MAX, u64::MAX - 1), 1);
    }

    #[test]
    fn primality_matches_trial_division_and_rejects_strong_pseudoprimes() {
        let trial = |candidate: u64| {
            candidate >= 2
                && (2..candidate)
                    .take_while(|d| d * d <= candidate)
                    .all(|d| !candidate.is_multiple_of(d))
        };
        for candidate in 0..5_000_u64 {
            assert_eq!(
                is_prime_u64(candidate),
                trial(candidate),
                "candidate {candidate}"
            );
        }
        // Strong pseudoprimes to small bases, a Carmichael number and a square of a prime.
        for composite in [
            2_047_u64,
            3_215_031_751,
            3_825_123_056_546_413_051,
            561,
            4_293_918_721 * 3,
            30_593 * 30_593,
            u64::MAX,
        ] {
            assert!(!is_prime_u64(composite), "composite {composite}");
        }
        for prime in [
            30_593_u64,
            35_969,
            4_293_918_721,
            4_292_018_177,
            2_013_265_921,
            70_368_744_067_073,
            18_446_744_073_709_551_557,
        ] {
            assert!(is_prime_u64(prime), "prime {prime}");
        }
    }

    #[test]
    fn primitive_root_search_is_bounded_and_returns_primitive_roots() {
        // 2689 = 1 + 128 * 21 supports order 128; the first working generator is 13.
        assert_eq!(
            primitive_root_of_order_with_candidate_limit(2_689, 128, 12),
            None
        );
        assert_eq!(
            primitive_root_of_order_with_candidate_limit(2_689, 128, 13),
            Some(491)
        );
        assert!(is_primitive_root_of_order(2_689, 491, 128));
        assert!(!is_primitive_root_of_order(2_689, 491, 64));
        assert!(!is_primitive_root_of_order(2_689, 1, 128));
        assert!(!is_primitive_root_of_order(2_689, 2_689, 128));
        assert!(!is_primitive_root_of_order(2_689, 491, 127));
        assert_eq!(
            primitive_root_of_order_with_candidate_limit(2_689, 1, 13),
            None
        );
        assert_eq!(
            primitive_root_of_order_with_candidate_limit(2_689, 128, 1),
            None
        );
        assert_eq!(
            primitive_root_of_order_with_candidate_limit(2_689, 256, 4_096),
            None,
            "order must divide modulus - 1"
        );
        for modulus in [
            30_593_u64, 30_977, 31_489, 31_873, 32_257, 33_409, 35_201, 35_969,
        ] {
            let root = primitive_root_of_order_with_candidate_limit(modulus, 128, 4_096)
                .expect("registered limb supports order 128");
            assert!(is_primitive_root_of_order(modulus, root, 128));
        }
    }

    #[test]
    fn big_endian_bytes_reduce_like_the_integer_they_encode() {
        let bytes = [
            0x12_u8, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0x0F, 0x1E, 0x2D, 0x3C, 0x4B,
        ];
        let mut wide = 0_u128;
        for byte in bytes {
            wide = (wide << 8) | u128::from(byte);
        }
        for modulus in [1_u64, 2, 17, 2_013_265_921, (1 << 62) - 57, u64::MAX] {
            assert_eq!(
                u128::from(reduce_be_bytes_mod_u64(&bytes, modulus)),
                wide % u128::from(modulus)
            );
        }
        assert_eq!(reduce_be_bytes_mod_u64(&[], 17), 0);
    }

    #[test]
    fn word_modulus_arithmetic_delegates_to_the_scalar_functions() {
        let arithmetic = WordModulus(30_593);
        assert_eq!(arithmetic.modulus(), 30_593);
        assert_eq!(arithmetic.add(30_592, 2), 1);
        assert_eq!(arithmetic.sub(1, 2), 30_592);
        assert_eq!(arithmetic.mul(30_592, 30_592), 1);
        assert_eq!(arithmetic.pow(3, 30_592), 1);
    }
}
