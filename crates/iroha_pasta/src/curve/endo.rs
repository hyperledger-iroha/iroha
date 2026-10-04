//! GLV scalar decomposition and variable-time scalar multiplication.
//!
//! Both Pasta curves have the endomorphism `phi(x, y) = (beta * x, y)`, which
//! acts as multiplication by the scalar cube root of unity `lambda = ZETA`.
//! A scalar `k` splits as `k = k1 + k2 * lambda (mod r)` with
//! `|k1|, |k2| < 2^128`, halving the number of doublings.
//!
//! The decomposition uses Babai rounding against the short lattice basis
//! `(b1, b2)` with the precomputed rounding constants `gamma1, gamma2`
//! (`round(2^256 * b / r)`), the same constants the vendored
//! `halo2curves-axiom` uses. The bound `|k_i| < 2^128` follows from
//! `|b1|, |b2| < 2^127.2`; `decompose` checks it and the tests exercise
//! it on random and edge scalars.
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
    /// `round(2^256 * b2 / r)` (vendored naming).
    const GAMMA1: [u64; 4];
    /// `round(2^256 * -b1 / r)` (vendored naming).
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
    let c1 = mul_512(&C::GAMMA2, &input);
    let c2 = mul_512(&C::GAMMA1, &input);
    let c1 = [c1[4], c1[5], c1[6], c1[7]];
    let c2 = [c2[4], c2[5], c2[6], c2[7]];
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
    use rand_core_06::SeedableRng;

    fn check_decomposition<C: PastaCurve>(k: C::ScalarExt) {
        let d = C::glv_decompose(&k).expect("lattice bound");
        let k1 = C::ScalarExt::from_u128(d.k1);
        let k2 = C::ScalarExt::from_u128(d.k2);
        let k1 = if d.k1_neg { -k1 } else { k1 };
        let k2 = if d.k2_neg { -k2 } else { k2 };
        assert_eq!(k1 + k2 * C::ScalarExt::ZETA, k);
        assert!(d.k1 < (1u128 << 127) + (1u128 << 126));
        assert!(d.k2 < (1u128 << 127) + (1u128 << 126));
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
