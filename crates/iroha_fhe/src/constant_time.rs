//! Branch-free selects and fixed-modulus Montgomery arithmetic.
//!
//! These kernels execute the same instruction sequence for every operand value:
//! they contain no data-dependent branch, table index or division. They are the
//! arithmetic for secret-dependent polynomials over a compiled modulus. The
//! modulus itself, transform lengths and exponents are public.
//!
//! [`FixedModulus`] keeps residues in the ordinary (non-Montgomery) domain:
//! each product performs two Montgomery reductions, entering through
//! `R^2 mod m` and leaving through the second reduction, so callers never see
//! a Montgomery representative.
use crate::modular::{ModularArithmetic, low_u64_from_u128};
use zeroize::Zeroizing;

/// Return `rhs` when `select_rhs_bit` is one and `lhs` when it is zero.
///
/// `select_rhs_bit` must be zero or one.
#[must_use]
pub const fn select_u64(lhs: u64, rhs: u64, select_rhs_bit: u64) -> u64 {
    let mask = 0_u64.wrapping_sub(select_rhs_bit);
    (lhs & !mask) | (rhs & mask)
}

/// Signed form of [`select_u64`].
#[must_use]
pub const fn select_i64(lhs: i64, rhs: i64, select_rhs_bit: u64) -> i64 {
    select_u64(lhs.cast_unsigned(), rhs.cast_unsigned(), select_rhs_bit).cast_signed()
}

/// Return one exactly when `lhs == rhs`, and zero otherwise.
#[must_use]
pub const fn equal_bit_u64(lhs: u64, rhs: u64) -> u64 {
    let difference = lhs ^ rhs;
    1 ^ ((difference | difference.wrapping_neg()) >> 63)
}

/// Return one exactly when `lhs > rhs`.
///
/// Both values must be below `2^63`, so the high bit of the wrapped reverse
/// subtraction is the borrow bit.
#[must_use]
pub const fn greater_than_bit_u64(lhs: u64, rhs: u64) -> u64 {
    rhs.wrapping_sub(lhs) >> 63
}

/// Subtract `modulus` once when `value >= modulus`.
///
/// The result is canonical for every `value < 2 * modulus`.
#[must_use]
pub fn reduce_once_u64(value: u64, modulus: u64) -> u64 {
    let (reduced, borrow) = value.overflowing_sub(modulus);
    select_u64(reduced, value, u64::from(borrow))
}

/// `(lhs + rhs) mod modulus` for canonical operands `lhs, rhs < modulus`.
///
/// Correct for every non-zero modulus, including those above `2^63`: the carry
/// out of the 64-bit sum is folded into the selection.
#[must_use]
pub fn add_mod_canonical_u64(lhs: u64, rhs: u64, modulus: u64) -> u64 {
    let (sum, carry) = lhs.overflowing_add(rhs);
    let (reduced, borrow) = sum.overflowing_sub(modulus);
    // The reduced value is the answer when the sum wrapped or reached the modulus.
    let keep_sum_bit = u64::from(borrow) & (1 ^ u64::from(carry));
    select_u64(reduced, sum, keep_sum_bit)
}

/// `(lhs - rhs) mod modulus` for canonical operands `lhs, rhs < modulus`.
#[must_use]
pub fn sub_mod_canonical_u64(lhs: u64, rhs: u64, modulus: u64) -> u64 {
    let (difference, borrow) = lhs.overflowing_sub(rhs);
    difference.wrapping_add(modulus & 0_u64.wrapping_sub(u64::from(borrow)))
}

/// One compiled odd modulus below `2^63` with its Montgomery constants for `R = 2^64`.
///
/// The fields are plain data so protocol tables can pin them as `const` values;
/// [`Self::is_consistent`] verifies a pinned triple and [`Self::derive`] computes one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FixedModulus {
    /// The modulus.
    pub modulus: u64,
    /// `-modulus^-1 mod 2^64`.
    pub montgomery_negative_inverse: u64,
    /// `2^128 mod modulus`, used to enter the Montgomery domain.
    pub montgomery_r_squared: u64,
}

impl FixedModulus {
    /// Compute the Montgomery constants of an odd modulus in `[3, 2^63)`.
    #[must_use]
    pub fn derive(modulus: u64) -> Option<Self> {
        if modulus < 3 || modulus.is_multiple_of(2) || modulus >= 1 << 63 {
            return None;
        }
        // Newton iteration doubles the number of correct low bits: 5 -> 10 -> ... -> 160.
        let mut inverse = modulus.wrapping_mul(3) ^ 2;
        for _ in 0..5 {
            inverse = inverse.wrapping_mul(2_u64.wrapping_sub(modulus.wrapping_mul(inverse)));
        }
        let r_mod = u64::try_from((1_u128 << 64) % u128::from(modulus)).ok()?;
        let r_squared =
            u64::try_from(u128::from(r_mod) * u128::from(r_mod) % u128::from(modulus)).ok()?;
        Some(Self {
            modulus,
            montgomery_negative_inverse: inverse.wrapping_neg(),
            montgomery_r_squared: r_squared,
        })
    }

    /// Whether the pinned constants are the Montgomery constants of the modulus.
    #[must_use]
    pub fn is_consistent(self) -> bool {
        Self::derive(self.modulus) == Some(self)
    }

    /// Subtract the modulus once when `value >= modulus`; exact for `value < 2 * modulus`.
    #[must_use]
    pub fn reduce_once(self, value: u64) -> u64 {
        reduce_once_u64(value, self.modulus)
    }

    /// Reduce any `u64` by bit-serial long division with one conditional subtraction per bit.
    #[must_use]
    pub fn reduce_u64(self, value: u64) -> u64 {
        let mut remainder = 0_u64;
        for bit_index in (0..u64::BITS).rev() {
            remainder = (remainder << 1) | ((value >> bit_index) & 1);
            remainder = self.reduce_once(remainder);
        }
        remainder
    }

    /// Least non-negative residue of a signed value without branching on its sign.
    #[must_use]
    pub fn canonicalize_i64(self, value: i64) -> u64 {
        let bits = value.cast_unsigned();
        let sign_bit = bits >> 63;
        let sign_mask = 0_u64.wrapping_sub(sign_bit);
        let magnitude = (bits ^ sign_mask).wrapping_add(sign_bit);
        let non_negative = self.reduce_u64(magnitude);
        let negative = self.sub_canonical(0, non_negative);
        select_u64(non_negative, negative, sign_bit)
    }

    /// `(lhs + rhs) mod modulus` for canonical operands.
    #[must_use]
    pub fn add_canonical(self, lhs: u64, rhs: u64) -> u64 {
        add_mod_canonical_u64(lhs, rhs, self.modulus)
    }

    /// `(lhs - rhs) mod modulus` for canonical operands.
    #[must_use]
    pub fn sub_canonical(self, lhs: u64, rhs: u64) -> u64 {
        sub_mod_canonical_u64(lhs, rhs, self.modulus)
    }

    /// Montgomery reduction `product * 2^-64 mod modulus` for `product < modulus * 2^64`.
    #[must_use]
    pub fn montgomery_reduce_u128(self, product: u128) -> u64 {
        let correction = low_u64_from_u128(product).wrapping_mul(self.montgomery_negative_inverse);
        let corrected_product = product + u128::from(correction) * u128::from(self.modulus);
        let quotient = low_u64_from_u128(corrected_product >> 64);
        self.reduce_once(quotient)
    }

    /// `(lhs * rhs) mod modulus` for `lhs, rhs < modulus`, in the ordinary domain.
    #[must_use]
    pub fn multiply(self, lhs: u64, rhs: u64) -> u64 {
        let lhs_montgomery =
            self.montgomery_reduce_u128(u128::from(lhs) * u128::from(self.montgomery_r_squared));
        self.montgomery_reduce_u128(u128::from(lhs_montgomery) * u128::from(rhs))
    }

    /// `base ^ exponent mod modulus`; the exponent is public.
    #[must_use]
    pub fn power(self, mut base: u64, mut exponent: u64) -> u64 {
        let mut output = 1_u64;
        while exponent != 0 {
            if exponent & 1 == 1 {
                output = self.multiply(output, base);
            }
            base = self.multiply(base, base);
            exponent >>= 1;
        }
        output
    }
}

impl ModularArithmetic for FixedModulus {
    fn modulus(&self) -> u64 {
        self.modulus
    }
    fn add(&self, lhs: u64, rhs: u64) -> u64 {
        self.add_canonical(lhs, rhs)
    }
    fn sub(&self, lhs: u64, rhs: u64) -> u64 {
        self.sub_canonical(lhs, rhs)
    }
    fn mul(&self, lhs: u64, rhs: u64) -> u64 {
        self.multiply(lhs, rhs)
    }
    fn pow(&self, base: u64, exponent: u64) -> u64 {
        self.power(base, exponent)
    }
}

/// Constants for centered three-prime CRT reconstruction into a target modulus.
///
/// With `P = p0 * p1 * p2`, a residue triple encodes the unique integer `x` in
/// `(-P/2, P/2)` and the kernel returns `x mod target`. `P` is odd, so no tie
/// exists. Every prime must be below the target, and `p0 < 2 * p1`,
/// `p0 < 2 * p2` and `p1 < 2 * p2`, so one conditional subtraction moves a
/// mixed-radix digit into the next prime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CenteredCrt3 {
    /// The three CRT primes in mixed-radix order.
    pub primes: [FixedModulus; 3],
    /// `p0^-1 mod p1` and `(p0 * p1)^-1 mod p2`.
    pub garner_inverses: [u64; 2],
    /// The modulus the centered integer is reduced into.
    pub target: FixedModulus,
    /// `p0 * p1 mod target`.
    pub first_two_product_mod_target: u64,
    /// `p0 * p1 * p2 mod target`.
    pub product_mod_target: u64,
}

impl CenteredCrt3 {
    /// Reconstruct the centered representative of `residues` and reduce it into the target.
    ///
    /// All temporaries live in a clearing scratch buffer.
    #[must_use]
    pub fn reconstruct_mod_target(&self, residues: [u64; 3]) -> u64 {
        const DIGIT_ZERO_INDEX: usize = 0;
        const DIGIT_ONE_INDEX: usize = 1;
        const DIGIT_TWO_INDEX: usize = 2;
        const LOWER_TWO_DIGITS_INDEX: usize = 3;
        const RECONSTRUCTED_INDEX: usize = 4;
        const NEGATIVE_INDEX: usize = 5;
        let residues = Zeroizing::new(residues);
        let mut scratch = Zeroizing::new([0_u64; 6]);
        let [prime_zero_modulus, prime_one_modulus, prime_two_modulus]: [FixedModulus; 3] =
            self.primes;
        let prime_zero = prime_zero_modulus.modulus;
        let prime_one = prime_one_modulus.modulus;
        let prime_two = prime_two_modulus.modulus;
        // Garner mixed-radix digits:
        // x = digit0 + p0 * digit1 + p0 * p1 * digit2, 0 <= x < P.
        scratch[DIGIT_ZERO_INDEX] = residues[0];
        scratch[DIGIT_ONE_INDEX] = prime_one_modulus.multiply(
            prime_one_modulus.sub_canonical(
                residues[1],
                prime_one_modulus.reduce_once(scratch[DIGIT_ZERO_INDEX]),
            ),
            self.garner_inverses[0],
        );
        scratch[LOWER_TWO_DIGITS_INDEX] = prime_two_modulus.add_canonical(
            prime_two_modulus.reduce_once(scratch[DIGIT_ZERO_INDEX]),
            prime_two_modulus.multiply(
                prime_two_modulus.reduce_once(prime_zero),
                prime_two_modulus.reduce_once(scratch[DIGIT_ONE_INDEX]),
            ),
        );
        scratch[DIGIT_TWO_INDEX] = prime_two_modulus.multiply(
            prime_two_modulus.sub_canonical(residues[2], scratch[LOWER_TWO_DIGITS_INDEX]),
            self.garner_inverses[1],
        );
        scratch[RECONSTRUCTED_INDEX] = self.target.add_canonical(
            self.target.add_canonical(
                scratch[DIGIT_ZERO_INDEX],
                self.target.multiply(prime_zero, scratch[DIGIT_ONE_INDEX]),
            ),
            self.target
                .multiply(self.first_two_product_mod_target, scratch[DIGIT_TWO_INDEX]),
        );
        // P is odd. Its floor-half has the mixed-radix digits
        // ((p0-1)/2, (p1-1)/2, (p2-1)/2), so a lexicographic comparison from
        // the most significant digit chooses the unique centered representative
        // without constructing the product P.
        let half_digits = [
            (prime_zero - 1) / 2,
            (prime_one - 1) / 2,
            (prime_two - 1) / 2,
        ];
        let is_negative = greater_than_bit_u64(scratch[DIGIT_TWO_INDEX], half_digits[2])
            | (equal_bit_u64(scratch[DIGIT_TWO_INDEX], half_digits[2])
                & (greater_than_bit_u64(scratch[DIGIT_ONE_INDEX], half_digits[1])
                    | (equal_bit_u64(scratch[DIGIT_ONE_INDEX], half_digits[1])
                        & greater_than_bit_u64(scratch[DIGIT_ZERO_INDEX], half_digits[0]))));
        scratch[NEGATIVE_INDEX] = self
            .target
            .sub_canonical(scratch[RECONSTRUCTED_INDEX], self.product_mod_target);
        select_u64(
            scratch[RECONSTRUCTED_INDEX],
            scratch[NEGATIVE_INDEX],
            is_negative,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODULI: [u64; 5] = [
        3,
        12_289,
        562_949_953_438_721,
        1_125_899_906_839_937,
        (1 << 63) - 25,
    ];

    fn splitmix64(state: &mut u64) -> u64 {
        *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = *state;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }
    fn reference_mul(lhs: u64, rhs: u64, modulus: u64) -> u64 {
        u64::try_from(u128::from(lhs) * u128::from(rhs) % u128::from(modulus)).unwrap()
    }
    fn reference_signed(value: i128, modulus: u64) -> u64 {
        u64::try_from(value.rem_euclid(i128::from(modulus))).unwrap()
    }

    #[test]
    fn selects_and_comparison_bits_cover_boundaries() {
        assert_eq!(select_u64(7, 9, 0), 7);
        assert_eq!(select_u64(7, 9, 1), 9);
        assert_eq!(select_i64(-7, 9, 0), -7);
        assert_eq!(select_i64(-7, i64::MIN, 1), i64::MIN);
        for (lhs, rhs) in [
            (0_u64, 0_u64),
            (1, 0),
            (0, 1),
            (u64::MAX, u64::MAX),
            (u64::MAX, 0),
        ] {
            assert_eq!(equal_bit_u64(lhs, rhs), u64::from(lhs == rhs));
        }
        let half = (1_u64 << 63) - 1;
        for (lhs, rhs) in [
            (0_u64, 0_u64),
            (1, 0),
            (0, 1),
            (half, half - 1),
            (half - 1, half),
            (half, 0),
            (0, half),
        ] {
            assert_eq!(greater_than_bit_u64(lhs, rhs), u64::from(lhs > rhs));
        }
    }

    #[test]
    fn canonical_add_sub_match_the_wide_reference_up_to_the_largest_modulus() {
        let mut state = 0x51_u64;
        for modulus in [1_u64, 2, 3, 17, (1 << 62) + 1, (1 << 63) + 3, u64::MAX] {
            let mut operands = vec![0, modulus - 1, modulus / 2, (modulus - 1) / 2];
            operands.extend((0..64).map(|_| splitmix64(&mut state) % modulus));
            for &lhs in &operands {
                for &rhs in &operands {
                    let sum =
                        u64::try_from((u128::from(lhs) + u128::from(rhs)) % u128::from(modulus))
                            .unwrap();
                    assert_eq!(add_mod_canonical_u64(lhs, rhs, modulus), sum);
                    assert_eq!(
                        sub_mod_canonical_u64(lhs, rhs, modulus),
                        reference_signed(i128::from(lhs) - i128::from(rhs), modulus)
                    );
                }
            }
            for value in [
                Some(0),
                Some(modulus - 1),
                Some(modulus),
                modulus.checked_add(modulus - 1),
            ]
            .into_iter()
            .flatten()
            {
                assert_eq!(reduce_once_u64(value, modulus), value % modulus);
            }
        }
    }

    #[test]
    fn derived_constants_satisfy_the_montgomery_identities() {
        for modulus in MODULI {
            let fixed = FixedModulus::derive(modulus).expect("odd modulus below 2^63");
            assert_eq!(fixed.modulus, modulus);
            assert_eq!(
                modulus.wrapping_mul(fixed.montgomery_negative_inverse),
                u64::MAX,
                "m * (-m^-1) = -1 mod 2^64"
            );
            let r = (1_u128 << 64) % u128::from(modulus);
            assert_eq!(
                u128::from(fixed.montgomery_r_squared),
                r * r % u128::from(modulus)
            );
            assert!(fixed.is_consistent());
            assert!(
                !FixedModulus {
                    modulus,
                    montgomery_negative_inverse: fixed.montgomery_negative_inverse ^ 2,
                    montgomery_r_squared: fixed.montgomery_r_squared
                }
                .is_consistent()
            );
        }
        for rejected in [0_u64, 1, 2, 4, 1 << 63, u64::MAX] {
            assert_eq!(FixedModulus::derive(rejected), None, "modulus {rejected}");
        }
    }

    #[test]
    fn fixed_modulus_arithmetic_matches_the_wide_reference() {
        let mut state = 0xB1_u64;
        for modulus in MODULI {
            let fixed = FixedModulus::derive(modulus).unwrap();
            let mut operands = vec![0, 1, modulus - 1, modulus / 2];
            operands.extend((0..96).map(|_| splitmix64(&mut state) % modulus));
            for &lhs in &operands {
                for &rhs in &operands {
                    assert_eq!(fixed.multiply(lhs, rhs), reference_mul(lhs, rhs, modulus));
                    assert_eq!(
                        ModularArithmetic::mul(&fixed, lhs, rhs),
                        reference_mul(lhs, rhs, modulus)
                    );
                    assert_eq!(
                        ModularArithmetic::add(&fixed, lhs, rhs),
                        reference_signed(i128::from(lhs) + i128::from(rhs), modulus)
                    );
                    assert_eq!(
                        ModularArithmetic::sub(&fixed, lhs, rhs),
                        reference_signed(i128::from(lhs) - i128::from(rhs), modulus)
                    );
                }
                let mut power = 1 % modulus;
                for exponent in 0..9_u64 {
                    assert_eq!(
                        fixed.power(lhs, exponent),
                        power,
                        "{lhs}^{exponent} mod {modulus}"
                    );
                    assert_eq!(ModularArithmetic::pow(&fixed, lhs, exponent), power);
                    power = reference_mul(power, lhs, modulus);
                }
            }
            assert_eq!(ModularArithmetic::modulus(&fixed), modulus);
        }
    }

    #[test]
    fn bit_serial_reduction_and_signed_canonicalization_cover_every_boundary() {
        let mut state = 0x77_u64;
        for modulus in MODULI {
            let fixed = FixedModulus::derive(modulus).unwrap();
            let mut values = vec![0, 1, modulus - 1, modulus, modulus + 1, u64::MAX, 1 << 63];
            values.extend((0..64).map(|_| splitmix64(&mut state)));
            for value in values {
                assert_eq!(fixed.reduce_u64(value), value % modulus);
                let signed = value.cast_signed();
                assert_eq!(
                    fixed.canonicalize_i64(signed),
                    reference_signed(i128::from(signed), modulus),
                    "value {signed} modulus {modulus}"
                );
            }
            for signed in [i64::MIN, i64::MIN + 1, -1, 0, 1, i64::MAX] {
                assert_eq!(
                    fixed.canonicalize_i64(signed),
                    reference_signed(i128::from(signed), modulus)
                );
            }
            assert_eq!(fixed.reduce_once(modulus), 0);
            assert_eq!(fixed.reduce_once(2 * modulus - 1), modulus - 1);
            assert_eq!(fixed.reduce_once(modulus - 1), modulus - 1);
        }
    }

    #[test]
    fn centered_three_prime_reconstruction_matches_signed_integers_at_every_boundary() {
        // The Bootle-Lantern internal CRT profile, the first consumer of this kernel.
        let primes = [
            1_125_899_906_840_833_u64,
            1_125_899_906_839_937,
            1_125_899_906_837_633,
        ];
        let target_modulus = 1_125_899_906_843_221_u64;
        assert!(
            primes
                .iter()
                .all(|&prime| crate::modular::is_prime_u64(prime))
        );
        let fixed = primes.map(|prime| FixedModulus::derive(prime).unwrap());
        let target = FixedModulus::derive(target_modulus).unwrap();
        // Pinned Montgomery constants of that profile.
        assert_eq!(
            target,
            FixedModulus {
                modulus: target_modulus,
                montgomery_negative_inverse: 4_655_614_974_089_172_227,
                montgomery_r_squared: 95_672_812_437_504
            }
        );
        assert_eq!(
            fixed,
            [
                FixedModulus {
                    modulus: primes[0],
                    montgomery_negative_inverse: 8_444_618_314_856_986_879,
                    montgomery_r_squared: 861_055_311_937_536
                },
                FixedModulus {
                    modulus: primes[1],
                    montgomery_negative_inverse: 2_456_608_423_348_909_439,
                    montgomery_r_squared: 812_195_763_980_927
                },
                FixedModulus {
                    modulus: primes[2],
                    montgomery_negative_inverse: 5_276_763_958_930_549_887,
                    montgomery_r_squared: 1_057_249_418_043_771
                },
            ]
        );
        let inverse =
            |value: u64, modulus: u64| crate::modular::mod_inv_prime_u64(value, modulus).unwrap();
        let first_two = u128::from(primes[0]) * u128::from(primes[1]);
        let crt = CenteredCrt3 {
            primes: fixed,
            garner_inverses: [
                inverse(primes[0] % primes[1], primes[1]),
                inverse(
                    u64::try_from(first_two % u128::from(primes[2])).unwrap(),
                    primes[2],
                ),
            ],
            target,
            first_two_product_mod_target: u64::try_from(first_two % u128::from(target_modulus))
                .unwrap(),
            product_mod_target: reference_mul(
                u64::try_from(first_two % u128::from(target_modulus)).unwrap(),
                primes[2] % target_modulus,
                target_modulus,
            ),
        };
        assert_eq!(
            crt.garner_inverses,
            [963_800_478_288_205, 296_975_494_591_860]
        );
        assert_eq!(crt.first_two_product_mod_target, 7_842_192);
        assert_eq!(crt.product_mod_target, 1_125_856_084_674_325);
        // Largest magnitudes representable in i128 stay far inside (-P/2, P/2) for P near 2^150.
        let mut state = 0xD3_u64;
        let mut values = vec![
            0_i128,
            1,
            -1,
            i128::from(u64::MAX),
            -i128::from(u64::MAX),
            i128::MAX,
            i128::MIN + 1,
        ];
        for _ in 0..128 {
            let high = i128::from(splitmix64(&mut state).cast_signed());
            values.push((high << 60) ^ i128::from(splitmix64(&mut state)));
        }
        for value in values {
            let residues = primes.map(|prime| reference_signed(value, prime));
            assert_eq!(
                crt.reconstruct_mod_target(residues),
                reference_signed(value, target_modulus),
                "value {value}"
            );
        }
        // The exact half boundary: digits ((p0-1)/2, (p1-1)/2, (p2-1)/2) encode floor(P/2), the
        // largest positive representative; one more is the most negative one.
        let half_digits = primes.map(|prime| (prime - 1) / 2);
        let mixed_to_residues = |digits: [u64; 3]| {
            primes.map(|prime| {
                let prime_wide = u128::from(prime);
                let lower = (u128::from(digits[0])
                    + u128::from(primes[0]) % prime_wide * u128::from(digits[1]))
                    % prime_wide;
                let upper =
                    first_two % prime_wide * (u128::from(digits[2]) % prime_wide) % prime_wide;
                u64::try_from((lower + upper) % prime_wide).unwrap()
            })
        };
        let mixed_mod_target = |digits: [u64; 3]| {
            let target_wide = u128::from(target_modulus);
            let lower = (u128::from(digits[0])
                + u128::from(primes[0]) % target_wide * u128::from(digits[1]) % target_wide)
                % target_wide;
            let upper =
                first_two % target_wide * (u128::from(digits[2]) % target_wide) % target_wide;
            u64::try_from((lower + upper) % target_wide).unwrap()
        };
        assert_eq!(
            crt.reconstruct_mod_target(mixed_to_residues(half_digits)),
            mixed_mod_target(half_digits),
            "floor(P/2) is positive"
        );
        let above_half = [half_digits[0] + 1, half_digits[1], half_digits[2]];
        assert_eq!(
            crt.reconstruct_mod_target(mixed_to_residues(above_half)),
            reference_signed(
                i128::from(mixed_mod_target(above_half)) - i128::from(crt.product_mod_target),
                target_modulus
            ),
            "floor(P/2) + 1 is negative"
        );
    }
}
