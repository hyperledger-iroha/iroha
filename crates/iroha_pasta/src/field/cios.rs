//! Portable Montgomery arithmetic specialised to the Pasta moduli.
//!
//! Both Pasta primes have the shape `m = 2^254 + c` with `c < 2^128`, so their
//! little-endian 64-bit limbs are `[m0, m1, 0, 2^62]`. The routines below use
//! that shape:
//!
//! - the multiplication is the "no-carry" variant of coarsely integrated
//!   operand scanning (CIOS). It is valid because the top modulus limb `2^62` is
//!   below `(2^64 - 1) / 2 - 1`;
//! - the reduction skips the zero limb and replaces the product with the top
//!   limb by a shift.
//!
//! # Reduction bounds
//!
//! Every value stored in a field element is fully reduced (`< m`). That is the
//! only invariant callers may rely on, and every public routine both requires
//! and re-establishes it:
//!
//! - [`mont_mul`] takes `a, b < m`. Each outer CIOS round keeps the running
//!   value `t < 2m`, so the result before the final conditional subtraction is
//!   below `2m` and one subtraction makes it canonical.
//! - [`mont_reduce`] takes any 512-bit `T < m * 2^256` and returns
//!   `T * 2^-256 mod m`, again below `2m` before one subtraction.
//! - [`add`] takes `a, b < m`, so `a + b < 2m < 2^256` and one subtraction
//!   suffices. [`sub`] and [`neg`] add the modulus back under a mask.
//!
//! Lazy reduction (values in `[m, 2m)`) is never stored: `4m > 2^256` for both
//! moduli, so a product of two unreduced operands can exceed `2m` and one
//! conditional subtraction would then leave a non-canonical limb pattern.
//!
//! All routines here are constant time: no branch or memory index depends on
//! operand values. Overflowing intermediate arithmetic uses explicit 128-bit
//! accumulators or `wrapping_*` operations by design; the bounds above show
//! that no information is lost.
#![allow(
    clippy::cast_possible_truncation,
    clippy::many_single_char_names,
    clippy::inline_always,
    clippy::similar_names
)]

/// Parameters of a Pasta-shaped Montgomery modulus (`R = 2^256`).
#[derive(Clone, Copy, Debug)]
pub struct Modulus {
    /// Little-endian limbs of the modulus `m`; limb 2 is zero and limb 3 is `2^62`.
    pub m: [u64; 4],
    /// `-m^-1 mod 2^64`.
    pub inv: u64,
    /// `R^2 mod m`, used to enter Montgomery form.
    pub r2: [u64; 4],
    /// `R^3 mod m`, used to reduce 512-bit integers and to finish inversions.
    pub r3: [u64; 4],
}

impl Modulus {
    /// Returns true when the modulus has the limb shape these routines assume.
    pub const fn has_pasta_shape(&self) -> bool {
        self.m[2] == 0 && self.m[3] == 1 << 62 && self.m[0] & 1 == 1
    }
}

/// Computes `a + b + carry`, returning the low word and the carry (0 or 1).
#[inline(always)]
pub const fn adc(a: u64, b: u64, carry: u64) -> (u64, u64) {
    let ret = (a as u128) + (b as u128) + (carry as u128);
    (ret as u64, (ret >> 64) as u64)
}

/// Computes `a - b - borrow`, where `borrow` is 0 or 1, returning the low word
/// and the new borrow (0 or 1).
#[inline(always)]
pub const fn sbb(a: u64, b: u64, borrow: u64) -> (u64, u64) {
    let ret = (a as u128).wrapping_sub((b as u128) + (borrow as u128));
    (ret as u64, ((ret >> 64) as u64) & 1)
}

/// Computes `a + b * c + carry`, returning the low word and the high word.
#[inline(always)]
pub const fn mac(a: u64, b: u64, c: u64, carry: u64) -> (u64, u64) {
    let ret = (a as u128) + (b as u128) * (c as u128) + (carry as u128);
    (ret as u64, (ret >> 64) as u64)
}

/// Returns `t - m` when `t >= m`, otherwise `t`, in constant time.
///
/// Requires `t < 2m`.
#[inline(always)]
pub const fn reduce_once(t: [u64; 4], p: &Modulus) -> [u64; 4] {
    let (d0, b) = sbb(t[0], p.m[0], 0);
    let (d1, b) = sbb(t[1], p.m[1], b);
    let (d2, b) = sbb(t[2], p.m[2], b);
    let (d3, b) = sbb(t[3], p.m[3], b);
    // `b == 1` means `t < m`: keep `t`. Build an all-ones mask for that case.
    let keep = 0u64.wrapping_sub(b);
    [
        (t[0] & keep) | (d0 & !keep),
        (t[1] & keep) | (d1 & !keep),
        (t[2] & keep) | (d2 & !keep),
        (t[3] & keep) | (d3 & !keep),
    ]
}

/// Returns true when the 256-bit integer `a` is below the modulus.
#[inline(always)]
pub const fn lt_modulus(a: &[u64; 4], p: &Modulus) -> bool {
    let (_, b) = sbb(a[0], p.m[0], 0);
    let (_, b) = sbb(a[1], p.m[1], b);
    let (_, b) = sbb(a[2], p.m[2], b);
    let (_, b) = sbb(a[3], p.m[3], b);
    b == 1
}

/// Modular addition of two canonical values.
#[inline(always)]
pub const fn add(a: &[u64; 4], b: &[u64; 4], p: &Modulus) -> [u64; 4] {
    debug_assert!(
        lt_modulus(a, p) && lt_modulus(b, p),
        "add operands must be canonical"
    );
    let (d0, c) = adc(a[0], b[0], 0);
    let (d1, c) = adc(a[1], b[1], c);
    let (d2, c) = adc(a[2], b[2], c);
    let (d3, _) = adc(a[3], b[3], c);
    reduce_once([d0, d1, d2, d3], p)
}

/// Modular subtraction of two canonical values.
#[inline(always)]
pub const fn sub(a: &[u64; 4], b: &[u64; 4], p: &Modulus) -> [u64; 4] {
    debug_assert!(
        lt_modulus(a, p) && lt_modulus(b, p),
        "sub operands must be canonical"
    );
    let (d0, br) = sbb(a[0], b[0], 0);
    let (d1, br) = sbb(a[1], b[1], br);
    let (d2, br) = sbb(a[2], b[2], br);
    let (d3, br) = sbb(a[3], b[3], br);
    // On underflow add the modulus back.
    let mask = 0u64.wrapping_sub(br);
    let (d0, c) = adc(d0, p.m[0] & mask, 0);
    let (d1, c) = adc(d1, p.m[1] & mask, c);
    let (d2, c) = adc(d2, p.m[2] & mask, c);
    let (d3, _) = adc(d3, p.m[3] & mask, c);
    [d0, d1, d2, d3]
}

/// Modular negation of a canonical value (`-0 = 0`).
#[inline(always)]
pub const fn neg(a: &[u64; 4], p: &Modulus) -> [u64; 4] {
    debug_assert!(lt_modulus(a, p), "neg operand must be canonical");
    let (d0, br) = sbb(p.m[0], a[0], 0);
    let (d1, br) = sbb(p.m[1], a[1], br);
    let (d2, br) = sbb(p.m[2], a[2], br);
    let (d3, _) = sbb(p.m[3], a[3], br);
    // Map `m - 0 = m` back to zero.
    let nonzero = ((a[0] | a[1] | a[2] | a[3]) != 0) as u64;
    let mask = 0u64.wrapping_sub(nonzero);
    [d0 & mask, d1 & mask, d2 & mask, d3 & mask]
}

/// Montgomery multiplication `a * b * 2^-256 mod m` of canonical operands.
///
/// No-carry CIOS for moduli `[m0, m1, 0, 2^62]`: per outer round one limb of
/// `b` is multiplied in and one limb is reduced, so the running value stays
/// below `2m` and fits four limbs plus one carry word.
#[inline(always)]
pub const fn mont_mul(a: &[u64; 4], b: &[u64; 4], p: &Modulus) -> [u64; 4] {
    debug_assert!(
        lt_modulus(a, p) && lt_modulus(b, p),
        "mont_mul operands must be canonical"
    );
    let mut t = [0u64; 4];
    let mut i = 0;
    while i < 4 {
        let bi = b[i];
        // Limb 0: t0 + a0 * bi, then choose k so the low word cancels.
        let (t0, ca) = mac(t[0], a[0], bi, 0);
        let k = t0.wrapping_mul(p.inv);
        let (_, cc) = mac(t0, k, p.m[0], 0);
        // Limb 1.
        let (x, ca) = mac(t[1], a[1], bi, ca);
        let (y, cc) = mac(x, k, p.m[1], cc);
        // Limb 2: the modulus limb is zero.
        let (x, ca) = mac(t[2], a[2], bi, ca);
        let (z, cc) = adc(x, cc, 0);
        // Limb 3: the modulus limb is 2^62, so k * m3 = k << 62.
        let (x, ca) = mac(t[3], a[3], bi, ca);
        let r = (x as u128) + ((k as u128) << 62) + (cc as u128);
        t[0] = y;
        t[1] = z;
        t[2] = r as u64;
        // No-carry condition: this sum fits one word.
        t[3] = ((r >> 64) as u64).wrapping_add(ca);
        i += 1;
    }
    reduce_once(t, p)
}

/// Montgomery reduction of a 512-bit value `T < m * 2^256`.
///
/// Returns `T * 2^-256 mod m`, canonical.
#[inline(always)]
pub const fn mont_reduce(r: [u64; 8], p: &Modulus) -> [u64; 4] {
    let [r0, r1, r2, r3, r4, r5, r6, r7] = r;

    let k = r0.wrapping_mul(p.inv);
    let (_, c) = mac(r0, k, p.m[0], 0);
    let (r1, c) = mac(r1, k, p.m[1], c);
    let (r2, c) = adc(r2, 0, c);
    let (r3, c) = mac(r3, k, p.m[3], c);
    let (r4, c2) = adc(r4, 0, c);

    let k = r1.wrapping_mul(p.inv);
    let (_, c) = mac(r1, k, p.m[0], 0);
    let (r2, c) = mac(r2, k, p.m[1], c);
    let (r3, c) = adc(r3, 0, c);
    let (r4, c) = mac(r4, k, p.m[3], c);
    let (r5, c2) = adc(r5, c2, c);

    let k = r2.wrapping_mul(p.inv);
    let (_, c) = mac(r2, k, p.m[0], 0);
    let (r3, c) = mac(r3, k, p.m[1], c);
    let (r4, c) = adc(r4, 0, c);
    let (r5, c) = mac(r5, k, p.m[3], c);
    let (r6, c2) = adc(r6, c2, c);

    let k = r3.wrapping_mul(p.inv);
    let (_, c) = mac(r3, k, p.m[0], 0);
    let (r4, c) = mac(r4, k, p.m[1], c);
    let (r5, c) = adc(r5, 0, c);
    let (r6, c) = mac(r6, k, p.m[3], c);
    let (r7, _) = adc(r7, c2, c);

    reduce_once([r4, r5, r6, r7], p)
}

/// Full 512-bit square of a 256-bit value (10 word products instead of 16).
///
/// Test-only reference: squaring through this and [`mont_reduce`] measured
/// slower than [`mont_mul`], so [`mont_square`] does not use it.
#[cfg(test)]
#[inline(always)]
pub const fn square_wide(a: &[u64; 4]) -> [u64; 8] {
    let (r1, c) = mac(0, a[0], a[1], 0);
    let (r2, c) = mac(0, a[0], a[2], c);
    let (r3, r4) = mac(0, a[0], a[3], c);

    let (r3, c) = mac(r3, a[1], a[2], 0);
    let (r4, r5) = mac(r4, a[1], a[3], c);

    let (r5, r6) = mac(r5, a[2], a[3], 0);

    let r7 = r6 >> 63;
    let r6 = (r6 << 1) | (r5 >> 63);
    let r5 = (r5 << 1) | (r4 >> 63);
    let r4 = (r4 << 1) | (r3 >> 63);
    let r3 = (r3 << 1) | (r2 >> 63);
    let r2 = (r2 << 1) | (r1 >> 63);
    let r1 = r1 << 1;

    let (r0, c) = mac(0, a[0], a[0], 0);
    let (r1, c) = adc(0, r1, c);
    let (r2, c) = mac(r2, a[1], a[1], c);
    let (r3, c) = adc(0, r3, c);
    let (r4, c) = mac(r4, a[2], a[2], c);
    let (r5, c) = adc(0, r5, c);
    let (r6, c) = mac(r6, a[3], a[3], c);
    let (r7, _) = adc(0, r7, c);

    [r0, r1, r2, r3, r4, r5, r6, r7]
}

/// Full 512-bit product of two 256-bit values.
#[inline(always)]
pub const fn mul_wide(a: &[u64; 4], b: &[u64; 4]) -> [u64; 8] {
    let (r0, c) = mac(0, a[0], b[0], 0);
    let (r1, c) = mac(0, a[0], b[1], c);
    let (r2, c) = mac(0, a[0], b[2], c);
    let (r3, r4) = mac(0, a[0], b[3], c);

    let (r1, c) = mac(r1, a[1], b[0], 0);
    let (r2, c) = mac(r2, a[1], b[1], c);
    let (r3, c) = mac(r3, a[1], b[2], c);
    let (r4, r5) = mac(r4, a[1], b[3], c);

    let (r2, c) = mac(r2, a[2], b[0], 0);
    let (r3, c) = mac(r3, a[2], b[1], c);
    let (r4, c) = mac(r4, a[2], b[2], c);
    let (r5, r6) = mac(r5, a[2], b[3], c);

    let (r3, c) = mac(r3, a[3], b[0], 0);
    let (r4, c) = mac(r4, a[3], b[1], c);
    let (r5, c) = mac(r5, a[3], b[2], c);
    let (r6, r7) = mac(r6, a[3], b[3], c);

    [r0, r1, r2, r3, r4, r5, r6, r7]
}

/// Montgomery squaring `a^2 * 2^-256 mod m` of a canonical operand.
///
/// Uses the no-carry CIOS multiplication: on the M1 Ultra it measured faster
/// (about 21 ns latency) than a 10-product wide square followed by a separate
/// reduction (about 22.8 ns), because the merged reduction avoids the long
/// carry chain.
#[inline(always)]
pub const fn mont_square(a: &[u64; 4], p: &Modulus) -> [u64; 4] {
    debug_assert!(lt_modulus(a, p), "mont_square operand must be canonical");
    mont_mul(a, a, p)
}

/// Converts a canonical integer into Montgomery form.
#[inline(always)]
pub const fn to_mont(a: &[u64; 4], p: &Modulus) -> [u64; 4] {
    mont_mul(a, &p.r2, p)
}

/// Converts any 256-bit integer (canonical or not) into Montgomery form,
/// reducing it modulo `m`.
///
/// `a * R^2 < 2^256 * m`, which is within the reduction bound.
#[inline(always)]
pub const fn to_mont_any(a: &[u64; 4], p: &Modulus) -> [u64; 4] {
    mont_reduce(mul_wide(a, &p.r2), p)
}

/// Converts a Montgomery-form value back to its canonical integer.
#[inline(always)]
pub const fn from_mont(a: &[u64; 4], p: &Modulus) -> [u64; 4] {
    mont_reduce([a[0], a[1], a[2], a[3], 0, 0, 0, 0], p)
}

/// Reduces an arbitrary 512-bit little-endian integer into Montgomery form.
///
/// Writes `x = lo + hi * 2^256`; then `x * R = lo * R + hi * R^2`, computed as
/// `mont_mul(lo, R^2) + mont_mul(hi, R^3)`. Each operand of the two
/// multiplications may be any 256-bit value: the product with a canonical
/// constant is below `m * 2^256`, which is all the reduction needs.
#[inline(always)]
pub const fn from_u512(limbs: &[u64; 8], p: &Modulus) -> [u64; 4] {
    let lo = [limbs[0], limbs[1], limbs[2], limbs[3]];
    let hi = [limbs[4], limbs[5], limbs[6], limbs[7]];
    let a = mont_reduce(mul_wide(&lo, &p.r2), p);
    let b = mont_reduce(mul_wide(&hi, &p.r3), p);
    add(&a, &b, p)
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: Modulus = crate::field::fp::MODULUS;
    const Q: Modulus = crate::field::fq::MODULUS;

    fn to_u128_pair(a: &[u64; 4]) -> (u128, u128) {
        (
            u128::from(a[0]) | (u128::from(a[1]) << 64),
            u128::from(a[2]) | (u128::from(a[3]) << 64),
        )
    }

    #[test]
    fn helpers_carry_and_borrow() {
        assert_eq!(adc(u64::MAX, 1, 0), (0, 1));
        assert_eq!(adc(u64::MAX, u64::MAX, 1), (u64::MAX, 1));
        assert_eq!(sbb(0, 1, 0), (u64::MAX, 1));
        assert_eq!(sbb(5, 2, 1), (2, 0));
        assert_eq!(mac(1, u64::MAX, u64::MAX, u64::MAX), (1, u64::MAX));
    }

    #[test]
    fn moduli_have_pasta_shape() {
        assert!(P.has_pasta_shape());
        assert!(Q.has_pasta_shape());
        for p in [P, Q] {
            assert_eq!(p.m[0].wrapping_mul(p.inv), u64::MAX, "inv is -m^-1");
        }
    }

    #[test]
    fn reduce_once_and_lt_modulus_edges() {
        for p in [P, Q] {
            let m = p.m;
            let (m_minus_1, _) = sbb(m[0], 1, 0);
            let below = [m_minus_1, m[1], m[2], m[3]];
            assert!(lt_modulus(&below, &p));
            assert!(!lt_modulus(&m, &p));
            assert_eq!(reduce_once(m, &p), [0; 4]);
            assert_eq!(reduce_once(below, &p), below);
            // 2m - 1 reduces to m - 1.
            let two_m = add_raw(&m, &m);
            let two_m_minus_1 = sub_raw(&two_m, &[1, 0, 0, 0]);
            assert_eq!(reduce_once(two_m_minus_1, &p), below);
        }
    }

    fn add_raw(a: &[u64; 4], b: &[u64; 4]) -> [u64; 4] {
        let (d0, c) = adc(a[0], b[0], 0);
        let (d1, c) = adc(a[1], b[1], c);
        let (d2, c) = adc(a[2], b[2], c);
        let (d3, _) = adc(a[3], b[3], c);
        [d0, d1, d2, d3]
    }

    fn sub_raw(a: &[u64; 4], b: &[u64; 4]) -> [u64; 4] {
        let (d0, br) = sbb(a[0], b[0], 0);
        let (d1, br) = sbb(a[1], b[1], br);
        let (d2, br) = sbb(a[2], b[2], br);
        let (d3, _) = sbb(a[3], b[3], br);
        [d0, d1, d2, d3]
    }

    #[test]
    fn add_sub_neg_wrap_at_modulus() {
        for p in [P, Q] {
            let m_minus_1 = sub_raw(&p.m, &[1, 0, 0, 0]);
            assert_eq!(add(&m_minus_1, &[1, 0, 0, 0], &p), [0; 4]);
            assert_eq!(sub(&[0; 4], &[1, 0, 0, 0], &p), m_minus_1);
            assert_eq!(neg(&[0; 4], &p), [0; 4]);
            assert_eq!(neg(&[1, 0, 0, 0], &p), m_minus_1);
            assert_eq!(
                add(&m_minus_1, &m_minus_1, &p),
                sub_raw(&m_minus_1, &[1, 0, 0, 0])
            );
        }
    }

    #[test]
    fn montgomery_round_trip_and_square_matches_mul() {
        for p in [P, Q] {
            let m_minus_1 = sub_raw(&p.m, &[1, 0, 0, 0]);
            for v in [
                [0u64; 4],
                [1, 0, 0, 0],
                [u64::MAX, u64::MAX, 0, 0],
                m_minus_1,
            ] {
                let mont = to_mont(&v, &p);
                assert_eq!(from_mont(&mont, &p), v);
                assert_eq!(mont_square(&mont, &p), mont_mul(&mont, &mont, &p));
                assert_eq!(
                    mont_reduce(square_wide(&mont), &p),
                    mont_mul(&mont, &mont, &p)
                );
                assert_eq!(to_mont_any(&v, &p), mont);
            }
            // (m - 1)^2 = 1.
            let one = to_mont(&[1, 0, 0, 0], &p);
            let mm = to_mont(&m_minus_1, &p);
            assert_eq!(mont_mul(&mm, &mm, &p), one);
        }
    }

    #[test]
    fn wide_products_and_u512_reduction() {
        let a = [u64::MAX; 4];
        assert_eq!(square_wide(&a), mul_wide(&a, &a));
        let w = mul_wide(&[2, 0, 0, 0], &[0, 0, 0, 1 << 63]);
        assert_eq!(w, [0, 0, 0, 0, 1, 0, 0, 0]);
        for p in [P, Q] {
            // 2^256 mod m = 2^256 - 3m because 3m < 2^256 < 4m.
            let three_m = add_raw(&add_raw(&p.m, &p.m), &p.m);
            let expected = sub_raw(&[0; 4], &three_m);
            let x = from_u512(&[0, 0, 0, 0, 1, 0, 0, 0], &p);
            assert_eq!(from_mont(&x, &p), expected);
            // R^2 mod m is the Montgomery form of R.
            assert_eq!(from_mont(&p.r2, &p), expected);
            let (lo, hi) = to_u128_pair(&expected);
            assert!(lo != 0 && hi != 0);
        }
    }
}
