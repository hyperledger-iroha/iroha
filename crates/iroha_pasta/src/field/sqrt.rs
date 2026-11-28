//! Table-based square roots (Sarkar, IACR ePrint 2020/1407).
//!
//! This is the algorithm `pasta_curves` 0.6.1 uses with its `sqrt-table`
//! feature, with the same tables and perfect-hash parameters, so it returns the
//! same root as that crate for every input. Both Pasta fields have 2-adicity
//! `S = 32`, which the four 8-bit table levels cover.
//!
//! Timing posture: the computation has no value-dependent branches, but the
//! discrete-logarithm table lookups are indexed by values derived from the
//! input, as in `pasta_curves`. Use it on public data (point decompression,
//! hash-to-curve); it is not hardened against cache-timing observers.

use ff::{Field, PrimeField};
use subtle::Choice;

/// Field operations the square-root tables need.
pub trait SqrtField: PrimeField {
    /// Multiplier for the order-256 subgroup's perfect hash.
    const HASH_MULTIPLIER: u32;

    /// Raises `self` to `(t - 1) / 2`, where `m - 1 = t * 2^S` with `t` odd.
    fn pow_by_t_minus1_over2(&self) -> Self;
    /// The low 32 bits of the fully reduced Montgomery representation.
    fn montgomery_low_word(&self) -> u32;
}

/// Square a field element a fixed number of times.
#[inline]
pub(super) fn square_n<F: Field>(mut value: F, count: u32) -> F {
    for _ in 0..count {
        value = value.square();
    }
    value
}

const HASH_BITS: u32 = 10;

/// Discrete-log table index for an element of the order-256 subgroup.
fn subgroup_index<F: SqrtField>(x: &F) -> usize {
    (x.montgomery_low_word().wrapping_mul(F::HASH_MULTIPLIER) >> (32 - HASH_BITS)) as usize
}

/// Precomputed tables of powers of the 2^32-th root of unity.
#[derive(Debug)]
pub struct SqrtTables<F: SqrtField> {
    inv: Vec<u8>,
    g0: Vec<F>,
    g1: Vec<F>,
    g2: Vec<F>,
    g3: Vec<F>,
}

impl<F: SqrtField> SqrtTables<F> {
    /// Builds the tables for the given field.
    pub fn new() -> Self {
        let mut levels = Vec::with_capacity(4);
        let mut gi = F::ROOT_OF_UNITY;
        for _ in 0..4 {
            // Level i holds gi^j for j < 256, with gi = ROOT_OF_UNITY^(256^i).
            let mut level = Vec::with_capacity(256);
            let mut acc = F::ONE;
            for _ in 0..256 {
                level.push(acc);
                acc *= gi;
            }
            gi = level[255] * gi;
            levels.push(level);
        }
        let mut g3 = levels.pop().unwrap_or_default();
        let g2 = levels.pop().unwrap_or_default();
        let g1 = levels.pop().unwrap_or_default();
        let g0 = levels.pop().unwrap_or_default();

        let mut tables = Self {
            inv: vec![1u8; 1 << HASH_BITS],
            g0,
            g1,
            g2,
            g3: Vec::new(),
        };
        for (j, g3_j) in g3.iter().enumerate() {
            let hash = subgroup_index(g3_j);
            // Slot values are (256 - j) mod 256: 0 for j = 0 and 255..2 for
            // j = 1..254. The initial value 1 is assigned only at the last
            // index (j = 255), so a slot still holding 1 has not been used.
            assert_eq!(tables.inv[hash], 1, "perfect hash collision");
            tables.inv[hash] = u8::try_from((256 - j) & 0xFF).unwrap_or(0);
        }
        g3.truncate(129);
        tables.g3 = g3;
        tables
    }

    /// Returns `(is_square, root)` where `root` is a square root of `num / div`
    /// if it is a square, else of `ROOT_OF_UNITY * num / div`.
    ///
    /// Returns `(true, 0)` when `num = 0`, and `(false, 0)` when `num != 0` and
    /// `div = 0`.
    pub fn sqrt_ratio(&self, num: &F, div: &F) -> (Choice, F) {
        // s = div^(2^S - 1)
        let s = (0..5).fold(*div, |d: F, i| square_n(d, 1 << i) * d);
        // t = div^(2^(S+1) - 1)
        let t = s.square() * div;
        // w = (num * t)^((T-1)/2) * s
        let w = (t * num).pow_by_t_minus1_over2() * s;
        let v = w * div;
        let uv = w * num;
        let res = self.sqrt_common(&uv, &v);
        let sqdiv = res.square() * div;
        let is_square = (sqdiv - num).is_zero();
        let is_nonsquare = (sqdiv - F::ROOT_OF_UNITY * num).is_zero();
        debug_assert!(bool::from(
            num.is_zero() | div.is_zero() | (is_square ^ is_nonsquare)
        ));
        (is_square, res)
    }

    /// Same as `sqrt_ratio(u, 1)`.
    pub fn sqrt_alt(&self, u: &F) -> (Choice, F) {
        let v = u.pow_by_t_minus1_over2();
        let uv = *u * v;
        let res = self.sqrt_common(&uv, &v);
        let sq = res.square();
        let is_square = (sq - u).is_zero();
        let is_nonsquare = (sq - F::ROOT_OF_UNITY * u).is_zero();
        debug_assert!(bool::from(u.is_zero() | (is_square ^ is_nonsquare)));
        (is_square, res)
    }

    fn sqrt_common(&self, uv: &F, v: &F) -> F {
        // The discrete-log accumulator `t` reaches 0xFFFF_FFFF, so it is kept
        // in u64: `t + 1` below must not overflow on 32-bit targets.
        let inv = |x: F| u64::from(self.inv[subgroup_index(&x)]);

        let x3 = *uv * v;
        let x2 = square_n(x3, 8);
        let x1 = square_n(x2, 8);
        let x0 = square_n(x1, 8);

        let mut t = inv(x0);
        debug_assert!(t < 0x100);
        let alpha = x1 * self.g2[byte(t, 0)];

        t += inv(alpha) << 8;
        debug_assert!(t < 0x1_0000);
        let alpha = x2 * self.g1[byte(t, 0)] * self.g2[byte(t, 1)];

        t += inv(alpha) << 16;
        debug_assert!(t < 0x100_0000);
        let alpha = x3 * self.g0[byte(t, 0)] * self.g1[byte(t, 1)] * self.g2[byte(t, 2)];

        t += inv(alpha) << 24;
        let t = halve_round_up(t);
        debug_assert!(t <= 0x8000_0000);

        *uv * self.g0[byte(t, 0)] * self.g1[byte(t, 1)] * self.g2[byte(t, 2)] * self.g3[byte(t, 3)]
    }
}

/// Byte `index` of `value` as a table index (always below 256).
#[inline]
fn byte(value: u64, index: usize) -> usize {
    usize::from(value.to_le_bytes()[index])
}

/// `(t + 1) / 2`, the halving step of the reference.
///
/// `t` is odd for non-squares, so the `+ 1` rounds up exactly as
/// `pasta_curves` does (`((t as u64) + 1) >> 1`). `t <= 0xFFFF_FFFF`, so the
/// result is at most `2^31` and the u64 sum cannot overflow.
#[inline]
fn halve_round_up(t: u64) -> u64 {
    (t + 1) >> 1
}

#[cfg(test)]
mod tests {
    use crate::field::{Fp, Fq};
    use ff::Field;

    #[test]
    fn every_subgroup_element_has_a_distinct_inverse_log_slot() {
        fn check<F: super::SqrtField>() {
            let tables = super::SqrtTables::<F>::new();
            assert_eq!(
                [
                    tables.g0.len(),
                    tables.g1.len(),
                    tables.g2.len(),
                    tables.g3.len()
                ],
                [256, 256, 256, 129]
            );
            assert_eq!(tables.g0[1], F::ROOT_OF_UNITY);
            let generator = F::ROOT_OF_UNITY.pow_vartime([1u64 << 24]);
            let mut registered = [false; 1 << super::HASH_BITS];
            let mut x = F::ONE;
            for j in 0..=u8::MAX {
                let slot = super::subgroup_index(&x);
                // j=255 registers the value 1, which also fills unused slots.
                assert!(!registered[slot]);
                registered[slot] = true;
                assert_eq!(tables.inv[slot], j.wrapping_neg());
                x *= generator;
            }
            assert_eq!(x, F::ONE);
            assert_eq!(registered.iter().filter(|&&occupied| occupied).count(), 256);
            for (occupied, value) in registered.iter().zip(&tables.inv) {
                if !occupied {
                    assert_eq!(*value, 1);
                }
            }
        }
        check::<Fp>();
        check::<Fq>();
    }

    #[test]
    fn fixed_exponents_and_signed_roots_match_the_oracle() {
        use ff::PrimeField;
        use num_bigint::BigUint;

        fn check<F, Oracle>()
        where
            F: super::SqrtField + PrimeField<Repr = [u8; 32]>,
            Oracle: PrimeField<Repr = [u8; 32]>,
        {
            // In canonical integer form, -1 is exactly the modulus minus one.
            let exponent =
                (BigUint::from_bytes_le(&(-F::ONE).to_repr()) >> (F::S + 1)).to_u64_digits();
            let inputs = [
                F::ZERO,
                F::ONE,
                -F::ONE,
                F::MULTIPLICATIVE_GENERATOR,
                F::ROOT_OF_UNITY,
                F::from(37),
            ];
            for x in inputs {
                assert_eq!(x.pow_by_t_minus1_over2(), x.pow_vartime(&exponent));
                let ox = Oracle::from_repr(x.to_repr()).unwrap();
                let (square, root) = x.sqrt_alt();
                let (expected_square, expected_root) = ox.sqrt_alt();
                assert_eq!(bool::from(square), bool::from(expected_square));
                assert_eq!(root.to_repr(), expected_root.to_repr());
                for y in inputs {
                    let oy = Oracle::from_repr(y.to_repr()).unwrap();
                    let (square, root) = F::sqrt_ratio(&x, &y);
                    let (expected_square, expected_root) = Oracle::sqrt_ratio(&ox, &oy);
                    assert_eq!(bool::from(square), bool::from(expected_square));
                    assert_eq!(root.to_repr(), expected_root.to_repr());
                }
            }
        }
        check::<Fp, pasta_curves::Fp>();
        check::<Fq, pasta_curves::Fq>();
    }

    #[test]
    fn halving_and_byte_indexing_cover_the_full_range() {
        // The largest accumulator (every table lookup returned 255) must not
        // overflow, on any pointer width.
        assert_eq!(super::halve_round_up(0xFFFF_FFFF), 0x8000_0000);
        assert_eq!(super::halve_round_up(0), 0);
        assert_eq!(super::halve_round_up(5), 3);
        assert_eq!(super::halve_round_up(6), 3);
        let t = 0x8001_02FFu64;
        assert_eq!(
            [
                super::byte(t, 0),
                super::byte(t, 1),
                super::byte(t, 2),
                super::byte(t, 3)
            ],
            [0xFF, 0x02, 0x01, 0x80]
        );
    }

    #[test]
    fn non_square_roots_match_the_definition() {
        // Non-squares take the odd-accumulator path of the halving step.
        let t = super::SqrtTables::<Fq>::new();
        let root_of_unity = <Fq as ff::PrimeField>::ROOT_OF_UNITY;
        let mut non_squares = 0;
        for v in 2..200u64 {
            let u = Fq::from(v);
            let (is_square, r) = t.sqrt_alt(&u);
            if bool::from(is_square) {
                assert_eq!(r.square(), u);
            } else {
                non_squares += 1;
                assert_eq!(r.square(), root_of_unity * u);
            }
        }
        assert!(non_squares > 50);
    }

    #[test]
    fn sqrt_ratio_cases() {
        let t = super::SqrtTables::<Fp>::new();
        let num = Fp::from(9u64);
        let div = Fp::from(4u64);
        let (sq, r) = t.sqrt_ratio(&num, &div);
        assert!(bool::from(sq));
        assert_eq!(r.square() * div, num);
        let (sq, r) = t.sqrt_ratio(&Fp::ZERO, &div);
        assert!(bool::from(sq));
        assert_eq!(r, Fp::ZERO);
        let (sq, r) = t.sqrt_ratio(&num, &Fp::ZERO);
        assert!(!bool::from(sq));
        assert_eq!(r, Fp::ZERO);
        let (sq, r) = t.sqrt_alt(&Fp::from(5u64));
        assert!(!bool::from(sq));
        assert_eq!(
            r.square(),
            <Fp as ff::PrimeField>::ROOT_OF_UNITY * Fp::from(5u64)
        );
    }
}
