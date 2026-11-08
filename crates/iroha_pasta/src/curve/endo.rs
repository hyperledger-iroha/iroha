//! GLV scalar decomposition and variable-time scalar multiplication.
//!
//! Both Pasta curves have the endomorphism `phi(x, y) = (beta * x, y)`, which
//! acts as multiplication by the scalar cube root of unity `lambda = ZETA`.
//! A scalar `k` splits as `k = k1 + k2 * lambda (mod r)` with
//! `|k1|, |k2| < 2^128`, halving the number of doublings.
//!
//! The decomposition uses Babai rounding against the short lattice basis
//! `(b1, b2)` with the precomputed truncation constants `gamma1, gamma2`
//! (`floor(2^256 * b / r)`), the same constants the vendored
//! `halo2curves-axiom` uses. Each coefficient `k * gamma / 2^256` is rounded
//! to the nearest integer, so it is off from the exact `k * b / r` by at most
//! `1/2` plus the constants' truncation (under `1/4`, as `k < r < 2^255`).
//! That bounds `|k1| < 0.87 * 2^127` and `|k2| < 1.3 * 2^127` (exact corner
//! analysis of both Pasta lattices); `decompose` checks `|k_i| < 2^128` and the
//! tests exercise the tighter bounds on random and edge scalars.
//!
//! Rounding to nearest also recovers short representations: for
//! `k = a + b * lambda` with `|a|, |b|` far below the lattice minimum (for
//! example `2^120`), the exact coefficients sit next to integers, rounding
//! lands on them, and the decomposition returns `(a, b)` itself. Endoscalar
//! challenges built that way from 64-bit halves therefore fold with 64-bit
//! wNAF digits instead of 128-bit ones. Flooring the coefficients, as before
//! 2026-11-08, lost this whenever an exact coefficient fell just below an
//! integer, which the truncated constants made the usual case.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]

use ff::WithSmallOrderMulGroup;

use crate::curve::PastaCurve;
use crate::field::PastaField;
use crate::field::cios::mac;

/// Lattice constants for the GLV decomposition of one curve's scalars.
pub trait GlvParams: PastaCurve {
    /// `floor(2^256 * Self::B1 / r)`.
    const GAMMA1: [u64; 4];
    /// `floor(2^256 * Self::B2 / r)`.
    const GAMMA2: [u64; 4];
    /// First short basis component.
    const B1: [u64; 4];
    /// Second short basis component.
    const B2: [u64; 4];
}

/// A scalar split as `k = s1 * k1 + s2 * k2 * ZETA`, `s_i = -1` when `k_i_neg`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlvDecomposition {
    /// Magnitude of the first component.
    pub k1: u128,
    /// Whether the first component is negative.
    pub k1_neg: bool,
    /// Magnitude of the second component.
    pub k2: u128,
    /// Whether the second component is negative.
    pub k2_neg: bool,
}

/// Full 512-bit product of two 256-bit integers.
fn mul_512(a: &[u64; 4], b: &[u64; 4]) -> [u64; 8] {
    let mut out = [0u64; 8];
    for i in 0..4 {
        let mut carry = 0u64;
        for j in 0..4 {
            let (lo, hi) = mac(out[i + j], a[i], b[j], carry);
            out[i + j] = lo;
            carry = hi;
        }
        out[i + 4] = carry;
    }
    out
}

/// `floor((a * b + 2^255) / 2^256)`: the high half of the 512-bit product,
/// rounded to the nearest integer.
fn mul_high_rounded(a: &[u64; 4], b: &[u64; 4]) -> [u64; 4] {
    let mut product = mul_512(a, b);
    // Add 2^255, the top bit of limb 3, and carry into the high half.
    let (limb, mut carry) = product[3].overflowing_add(1 << 63);
    product[3] = limb;
    for limb in &mut product[4..] {
        let (sum, overflow) = limb.overflowing_add(u64::from(carry));
        *limb = sum;
        carry = overflow;
    }
    [product[4], product[5], product[6], product[7]]
}

/// Returns `(negative, magnitude)` of a field element read as a signed value
/// in `(-r/2, r/2]` when its magnitude is below `2^128`.
fn signed_small<F: PastaField>(v: &F) -> Option<(bool, u128)> {
    let limbs = v.to_canonical_limbs();
    if limbs[2] == 0 && limbs[3] == 0 {
        return Some((false, u128::from(limbs[0]) | (u128::from(limbs[1]) << 64)));
    }
    let neg = (-*v).to_canonical_limbs();
    if neg[2] == 0 && neg[3] == 0 {
        return Some((true, u128::from(neg[0]) | (u128::from(neg[1]) << 64)));
    }
    None
}

/// Splits a scalar for the GLV method on curve `C`.
///
/// Returns `None` only if the lattice bound fails, which cannot happen for the
/// Pasta constants (tested); callers then fall back to plain double-and-add.
pub fn decompose<C: GlvParams>(k: &C::ScalarExt) -> Option<GlvDecomposition> {
    let input = k.to_canonical_limbs();
    let c1 = mul_high_rounded(&C::GAMMA2, &input);
    let c2 = mul_high_rounded(&C::GAMMA1, &input);
    let q1 = mul_512(&c1, &C::B1);
    let q2 = mul_512(&c2, &C::B2);
    let q1 = C::ScalarExt::from_raw_reduced([q1[0], q1[1], q1[2], q1[3]]);
    let q2 = C::ScalarExt::from_raw_reduced([q2[0], q2[1], q2[2], q2[3]]);
    // k = k1 + k2 * ZETA with k2 = q1 - q2 and k1 = k - k2 * ZETA.
    let k2 = q1 - q2;
    let k1 = *k - k2 * C::ScalarExt::ZETA;
    let (k1_neg, k1) = signed_small(&k1)?;
    let (k2_neg, k2) = signed_small(&k2)?;
    Some(GlvDecomposition {
        k1,
        k1_neg,
        k2,
        k2_neg,
    })
}

/// Width-`w` non-adjacent form of a 128-bit magnitude, least significant digit
/// first. Digits are odd values in `(-2^(w-1), 2^(w-1))` or zero.
pub fn wnaf_u128(a: u128, w: u32) -> Vec<i8> {
    // The value is `lo + hi * 2^128`; `hi` absorbs the carry of a final
    // rounding step near 2^128, so every 128-bit input is handled.
    let mut lo = a;
    let mut hi = 0u8;
    let mut out = Vec::with_capacity(130);
    let full = 1i32 << w;
    let half = 1i32 << (w - 1);
    let mask = (1u128 << w) - 1;
    while lo != 0 || hi != 0 {
        if lo & 1 == 1 {
            let mut d = (lo & mask) as i32;
            if d >= half {
                d -= full;
            }
            if d >= 0 {
                lo -= d as u128;
            } else {
                let (sum, overflow) = lo.overflowing_add((-d) as u128);
                lo = sum;
                hi += u8::from(overflow);
            }
            out.push(d as i8);
        } else {
            out.push(0);
        }
        lo = (lo >> 1) | (u128::from(hi & 1) << 127);
        hi >>= 1;
    }
    out
}

/// Window width of the variable-time multiplication.
const WNAF_WIDTH: u32 = 5;

/// Variable-time `k * P` with GLV and interleaved width-5 wNAF.
///
/// Public scalars only: the sequence of additions depends on `k`.
pub fn mul_vartime<C: PastaCurve>(p: &C, k: &C::ScalarExt) -> C {
    let Some(d) = C::glv_decompose(k) else {
        // Unreachable for the Pasta constants; keep a correct fallback.
        return *p * *k;
    };
    let p1 = if d.k1_neg { -*p } else { *p };
    let phi = p.endo();
    let p2 = if d.k2_neg { -phi } else { phi };
    let table = |base: C| {
        let two = base.double();
        let mut t = [base; 8];
        for i in 1..8 {
            t[i] = t[i - 1] + two;
        }
        t
    };
    let t1 = table(p1);
    let t2 = table(p2);
    let n1 = wnaf_u128(d.k1, WNAF_WIDTH);
    let n2 = wnaf_u128(d.k2, WNAF_WIDTH);
    let len = n1.len().max(n2.len());
    let mut acc = C::identity();
    for i in (0..len).rev() {
        acc = acc.double();
        for (digits, t) in [(&n1, &t1), (&n2, &t2)] {
            let digit = digits.get(i).copied().unwrap_or(0);
            if digit > 0 {
                acc += t[(digit as usize) / 2];
            } else if digit < 0 {
                acc -= t[((-digit) as usize) / 2];
            }
        }
    }
    acc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curve::{Ep, Eq};
    use crate::field::{Fp, Fq};
    use ff::{Field, PrimeField};
    use group::Group;
    use rand_chacha::ChaCha20Rng;
    use rand_core::{Rng, SeedableRng};

    fn check_decomposition<C: PastaCurve>(k: C::ScalarExt) {
        let d = C::glv_decompose(&k).expect("lattice bound");
        let k1 = C::ScalarExt::from_u128(d.k1);
        let k2 = C::ScalarExt::from_u128(d.k2);
        let k1 = if d.k1_neg { -k1 } else { k1 };
        let k2 = if d.k2_neg { -k2 } else { k2 };
        assert_eq!(k1 + k2 * C::ScalarExt::ZETA, k);
        // 0.875 * 2^127 and 1.3125 * 2^127 sit above the exact worst cases
        // (0.866, 1.299) and below the 1.5 * 2^127 that iroha_plonk_gadgets'
        // offset GLV witness needs.
        assert!(d.k1 < 7u128 << 124, "{d:?}");
        assert!(
            d.k2 < (1u128 << 127) + (1u128 << 125) + (1u128 << 123),
            "{d:?}"
        );
    }

    /// `a + b * ZETA` with `|a|, |b|` far below the lattice minimum decomposes
    /// to `(a, b)` itself, so endoscalar challenges keep their short halves.
    fn recovers_short<C: PastaCurve>(seed: u64) {
        let mut rng = ChaCha20Rng::seed_from_u64(seed);
        let mut halves = vec![
            (0u128, false, 0u128, false),
            (1, false, 0, false),
            (0, false, 1, true),
        ];
        for bits in [1u32, 32, 64, 65, 100, 120] {
            for _ in 0..200 {
                let mask = (1u128 << bits) - 1;
                let a = (u128::from(rng.next_u64()) | (u128::from(rng.next_u64()) << 64)) & mask;
                let b = (u128::from(rng.next_u64()) | (u128::from(rng.next_u64()) << 64)) & mask;
                let signs = rng.next_u64();
                halves.push((a, signs & 1 == 1 && a != 0, b, signs & 2 == 2 && b != 0));
            }
        }
        for (a, a_neg, b, b_neg) in halves {
            let signed = |magnitude: u128, negative: bool| {
                let v = C::ScalarExt::from_u128(magnitude);
                if negative { -v } else { v }
            };
            let k = signed(a, a_neg) + signed(b, b_neg) * C::ScalarExt::ZETA;
            let expected = GlvDecomposition {
                k1: a,
                k1_neg: a_neg,
                k2: b,
                k2_neg: b_neg,
            };
            assert_eq!(C::glv_decompose(&k), Some(expected));
        }
    }

    #[test]
    fn decomposition_recovers_short_representations() {
        recovers_short::<Ep>(11);
        recovers_short::<Eq>(12);
    }

    #[test]
    fn mul_high_rounded_rounds_half_up() {
        // 2^255 * 1 rounds to 1; 2^255 - 1 rounds to 0; 3 * 2^255 rounds to 2.
        assert_eq!(
            mul_high_rounded(&[0, 0, 0, 1 << 63], &[1, 0, 0, 0]),
            [1, 0, 0, 0]
        );
        assert_eq!(
            mul_high_rounded(
                &[u64::MAX, u64::MAX, u64::MAX, (1 << 63) - 1],
                &[1, 0, 0, 0]
            ),
            [0, 0, 0, 0]
        );
        assert_eq!(
            mul_high_rounded(&[0, 0, 0, 1 << 63], &[3, 0, 0, 0]),
            [2, 0, 0, 0]
        );
        // 2^255 * (2^192 - 1) + 2^255 = 2^447: the rounding carry runs through
        // two all-ones high limbs.
        assert_eq!(
            mul_high_rounded(&[0, 0, 0, 1 << 63], &[u64::MAX, u64::MAX, u64::MAX, 0]),
            [0, 0, 1 << 63, 0]
        );
    }

    #[test]
    fn decomposition_random_and_edges() {
        let mut rng = ChaCha20Rng::seed_from_u64(7);
        for _ in 0..2000 {
            check_decomposition::<Ep>(Fq::random(&mut rng));
            check_decomposition::<Eq>(Fp::random(&mut rng));
        }
        for k in [
            Fq::ZERO,
            Fq::ONE,
            -Fq::ONE,
            Fq::ZETA,
            -Fq::ZETA,
            Fq::TWO_INV,
        ] {
            check_decomposition::<Ep>(k);
        }
        for k in [
            Fp::ZERO,
            Fp::ONE,
            -Fp::ONE,
            Fp::ZETA,
            Fp::from_u128(u128::MAX),
        ] {
            check_decomposition::<Eq>(k);
        }
    }

    #[test]
    fn wnaf_reconstructs() {
        for a in [0u128, 1, 15, 16, 31, 0xdead_beef, u128::MAX >> 1, u128::MAX] {
            let digits = wnaf_u128(a, 5);
            assert!(digits.len() <= 129);
            // Reconstruct modulo 2^128 with wrapping arithmetic.
            let mut acc: u128 = 0;
            for d in digits.iter().rev() {
                assert!(*d == 0 || (*d % 2 != 0 && d.abs() < 16));
                acc = acc.wrapping_mul(2);
                acc = if *d >= 0 {
                    acc.wrapping_add(u128::from(d.unsigned_abs()))
                } else {
                    acc.wrapping_sub(u128::from(d.unsigned_abs()))
                };
            }
            assert_eq!(acc, a);
        }
    }

    #[test]
    fn mul_vartime_matches_constant_time() {
        let mut rng = ChaCha20Rng::seed_from_u64(9);
        for _ in 0..64 {
            let p = Ep::random(&mut rng);
            let k = Fq::random(&mut rng);
            assert_eq!(mul_vartime(&p, &k), p * k);
            let q = Eq::random(&mut rng);
            let s = Fp::random(&mut rng);
            assert_eq!(mul_vartime(&q, &s), q * s);
        }
        assert_eq!(mul_vartime(&Ep::generator(), &Fq::ZERO), Ep::identity());
        assert_eq!(mul_vartime(&Ep::identity(), &Fq::ONE), Ep::identity());
    }

    #[test]
    fn mul_512_and_signed_small() {
        let p = mul_512(&[u64::MAX, 0, 0, 0], &[u64::MAX, 0, 0, 0]);
        assert_eq!(p, [1, u64::MAX - 1, 0, 0, 0, 0, 0, 0]);
        assert_eq!(signed_small(&-Fp::from(5u64)), Some((true, 5)));
        assert_eq!(signed_small(&Fp::from(5u64)), Some((false, 5)));
        assert_eq!(signed_small(&Fp::TWO_INV), None);
        assert_eq!(decompose::<Ep>(&Fq::ONE), Ep::glv_decompose(&Fq::ONE));
    }
}
