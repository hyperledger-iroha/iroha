//! Goldilocks prime field `p = 2^64 - 2^32 + 1` and its quartic extension `Fp4 = Fp[X]/(X^4 - 7)`.
//!
//! Elements are kept canonical (`< p`). The base field holds every committed trace value; the
//! extension holds every verifier challenge, the LogUp columns, the composition quotient and all
//! out-of-domain evaluations, so that field-size terms of the soundness bound are about `2^-190`.

use std::fmt;
use std::ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// The Goldilocks modulus.
pub const MODULUS: u64 = 0xffff_ffff_0000_0001;
/// `2^64 mod p`, also `2^32 - 1`.
const EPSILON: u64 = 0xffff_ffff;
/// Multiplicative generator of `Fp^*`; it is a quadratic non-residue, so `X^4 - 7` is irreducible.
pub const GENERATOR: u64 = 7;
/// Largest `k` with `2^k | p - 1`.
pub const TWO_ADICITY: u32 = 32;
/// The constant `W` of the extension modulus `X^4 - W`.
pub const EXTENSION_W: u64 = 7;

/// A canonical Goldilocks field element.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Fp(u64);

impl fmt::Debug for Fp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[inline]
const fn reduce128(x: u128) -> u64 {
    let lo = x as u64;
    let hi = (x >> 64) as u64;
    let hi_hi = hi >> 32;
    let hi_lo = hi & EPSILON;
    let (mut t0, borrow) = lo.overflowing_sub(hi_hi);
    if borrow {
        // 2^96 = -1, so subtracting hi_hi borrowed 2^64 = EPSILON (mod p).
        t0 = t0.wrapping_sub(EPSILON);
    }
    let t1 = hi_lo * EPSILON;
    let (sum, carry) = t0.overflowing_add(t1);
    let res = sum.wrapping_add(EPSILON * (carry as u64));
    if res >= MODULUS { res - MODULUS } else { res }
}

impl Fp {
    /// Additive identity.
    pub const ZERO: Self = Self(0);
    /// Multiplicative identity.
    pub const ONE: Self = Self(1);
    /// The element two.
    pub const TWO: Self = Self(2);

    /// Builds an element from any `u64`, reducing modulo `p`.
    #[inline]
    pub const fn new(value: u64) -> Self {
        Self(if value >= MODULUS {
            value - MODULUS
        } else {
            value
        })
    }

    /// Embeds a signed integer with `|value| < p`.
    #[inline]
    pub const fn from_i64(value: i64) -> Self {
        if value >= 0 {
            Self::new(value as u64)
        } else {
            Self(MODULUS - value.unsigned_abs())
        }
    }

    /// Embeds a signed 128-bit integer reduced modulo `p`.
    pub fn from_i128(value: i128) -> Self {
        let modulus = i128::from(MODULUS);
        let reduced = value.rem_euclid(modulus);
        Self(reduced as u64)
    }

    /// The canonical representative in `[0, p)`.
    #[inline]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// Interprets the element as a signed integer in `(-p/2, p/2]`.
    #[inline]
    pub const fn as_i64_centered(self) -> i64 {
        // Both branches are below p/2 < 2^63, so the conversions are exact.
        if self.0 > MODULUS / 2 {
            -(MODULUS - self.0).cast_signed()
        } else {
            self.0.cast_signed()
        }
    }

    /// Whether this is zero.
    #[inline]
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// `self * self`.
    #[inline]
    #[must_use]
    pub fn square(self) -> Self {
        self * self
    }

    /// `self^exponent`.
    #[must_use]
    pub fn pow(self, mut exponent: u64) -> Self {
        let mut base = self;
        let mut acc = Self::ONE;
        while exponent > 0 {
            if exponent & 1 == 1 {
                acc *= base;
            }
            base = base.square();
            exponent >>= 1;
        }
        acc
    }

    /// Multiplicative inverse, or `None` for zero.
    pub fn inverse(self) -> Option<Self> {
        if self.is_zero() {
            None
        } else {
            Some(self.pow(MODULUS - 2))
        }
    }

    /// A primitive `2^log_n`-th root of unity.
    ///
    /// # Panics
    /// Panics if `log_n > 32`.
    pub fn two_adic_generator(log_n: u32) -> Self {
        assert!(log_n <= TWO_ADICITY, "Goldilocks has two-adicity 32");
        let root = Self(GENERATOR).pow((MODULUS - 1) >> TWO_ADICITY);
        root.pow(1u64 << (TWO_ADICITY - log_n))
    }

    /// Little-endian bytes of the canonical representative.
    pub const fn to_le_bytes(self) -> [u8; 8] {
        self.0.to_le_bytes()
    }

    /// Parses canonical little-endian bytes, rejecting values `>= p`.
    pub fn from_canonical_le_bytes(bytes: [u8; 8]) -> Option<Self> {
        let value = u64::from_le_bytes(bytes);
        (value < MODULUS).then_some(Self(value))
    }
}

impl Add for Fp {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        let (sum, overflow) = self.0.overflowing_add(rhs.0);
        if overflow {
            Self(sum + EPSILON)
        } else if sum >= MODULUS {
            Self(sum - MODULUS)
        } else {
            Self(sum)
        }
    }
}

impl Sub for Fp {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        let (diff, borrow) = self.0.overflowing_sub(rhs.0);
        if borrow {
            Self(diff.wrapping_sub(EPSILON))
        } else {
            Self(diff)
        }
    }
}

impl Neg for Fp {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        if self.0 == 0 {
            self
        } else {
            Self(MODULUS - self.0)
        }
    }
}

impl Mul for Fp {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        Self(reduce128(u128::from(self.0) * u128::from(rhs.0)))
    }
}

impl AddAssign for Fp {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl SubAssign for Fp {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl MulAssign for Fp {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

impl From<u64> for Fp {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

impl From<u32> for Fp {
    fn from(value: u32) -> Self {
        Self(u64::from(value))
    }
}

/// Inverts every nonzero element in place with one field inversion (Montgomery's trick).
///
/// # Panics
/// Panics if any element is zero.
pub fn batch_inverse(values: &mut [Fp]) {
    if values.is_empty() {
        return;
    }
    let mut prefix = Vec::with_capacity(values.len());
    let mut acc = Fp::ONE;
    for value in values.iter() {
        prefix.push(acc);
        acc *= *value;
    }
    let mut inv = acc.inverse().expect("batch_inverse input contains zero");
    for (value, before) in values.iter_mut().zip(prefix).rev() {
        let original = *value;
        *value = inv * before;
        inv *= original;
    }
}

/// An element `c0 + c1 X + c2 X^2 + c3 X^3` of `Fp[X]/(X^4 - 7)`.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Fp4(pub [Fp; 4]);

impl fmt::Debug for Fp4 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Fp4{:?}", self.0)
    }
}

#[inline]
fn mul_w(x: Fp) -> Fp {
    // 7x = 8x - x.
    let doubled = x + x;
    let quadrupled = doubled + doubled;
    quadrupled + quadrupled - x
}

impl Fp4 {
    /// Additive identity.
    pub const ZERO: Self = Self([Fp::ZERO; 4]);
    /// Multiplicative identity.
    pub const ONE: Self = Self([Fp::ONE, Fp::ZERO, Fp::ZERO, Fp::ZERO]);

    /// Embeds a base-field element.
    #[inline]
    pub const fn from_base(value: Fp) -> Self {
        Self([value, Fp::ZERO, Fp::ZERO, Fp::ZERO])
    }

    /// Whether every coefficient is zero.
    pub fn is_zero(self) -> bool {
        self.0.iter().all(|c| c.is_zero())
    }

    /// Multiplies by a base-field element.
    #[inline]
    #[must_use]
    pub fn mul_base(self, rhs: Fp) -> Self {
        Self([
            self.0[0] * rhs,
            self.0[1] * rhs,
            self.0[2] * rhs,
            self.0[3] * rhs,
        ])
    }

    /// Adds a base-field element.
    #[inline]
    #[must_use]
    pub fn add_base(self, rhs: Fp) -> Self {
        Self([self.0[0] + rhs, self.0[1], self.0[2], self.0[3]])
    }

    /// `self * self`.
    #[inline]
    #[must_use]
    pub fn square(self) -> Self {
        self * self
    }

    /// `self^exponent`.
    #[must_use]
    pub fn pow(self, mut exponent: u64) -> Self {
        let mut base = self;
        let mut acc = Self::ONE;
        while exponent > 0 {
            if exponent & 1 == 1 {
                acc *= base;
            }
            base = base.square();
            exponent >>= 1;
        }
        acc
    }

    /// Multiplicative inverse through the tower `Fp2[X]/(X^2 - u)`, `u = X^2`, `u^2 = 7`.
    pub fn inverse(self) -> Option<Self> {
        let [a0, a1, a2, a3] = self.0;
        // self = A + X B with A = a0 + a2 u and B = a1 + a3 u in Fp2 = Fp[u]/(u^2 - 7).
        let fp2_mul =
            |x0: Fp, x1: Fp, y0: Fp, y1: Fp| (x0 * y0 + mul_w(x1 * y1), x0 * y1 + x1 * y0);
        let (aa0, aa1) = fp2_mul(a0, a2, a0, a2);
        let (bb0, bb1) = fp2_mul(a1, a3, a1, a3);
        // u * B^2 = (bb0 + bb1 u) u = 7 bb1 + bb0 u.
        let norm0 = aa0 - mul_w(bb1);
        let norm1 = aa1 - bb0;
        // Inverse of the Fp2 norm: (n0 - n1 u) / (n0^2 - 7 n1^2).
        let denom = norm0 * norm0 - mul_w(norm1 * norm1);
        let denom_inv = denom.inverse()?;
        let inv0 = norm0 * denom_inv;
        let inv1 = -norm1 * denom_inv;
        // (A - X B) / N.
        let (c0, c2) = fp2_mul(a0, a2, inv0, inv1);
        let (d0, d2) = fp2_mul(-a1, -a3, inv0, inv1);
        Some(Self([c0, d0, c2, d2]))
    }

    /// Little-endian bytes of the four canonical coefficients.
    pub fn to_le_bytes(self) -> [u8; 32] {
        let mut out = [0u8; 32];
        for (chunk, coefficient) in out.chunks_exact_mut(8).zip(self.0) {
            chunk.copy_from_slice(&coefficient.to_le_bytes());
        }
        out
    }
}

impl Add for Fp4 {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self([
            self.0[0] + rhs.0[0],
            self.0[1] + rhs.0[1],
            self.0[2] + rhs.0[2],
            self.0[3] + rhs.0[3],
        ])
    }
}

impl Sub for Fp4 {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self([
            self.0[0] - rhs.0[0],
            self.0[1] - rhs.0[1],
            self.0[2] - rhs.0[2],
            self.0[3] - rhs.0[3],
        ])
    }
}

impl Neg for Fp4 {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Self([-self.0[0], -self.0[1], -self.0[2], -self.0[3]])
    }
}

impl Mul for Fp4 {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        let [a0, a1, a2, a3] = self.0;
        let [b0, b1, b2, b3] = rhs.0;
        let c0 = a0 * b0 + mul_w(a1 * b3 + a2 * b2 + a3 * b1);
        let c1 = a0 * b1 + a1 * b0 + mul_w(a2 * b3 + a3 * b2);
        let c2 = a0 * b2 + a1 * b1 + a2 * b0 + mul_w(a3 * b3);
        let c3 = a0 * b3 + a1 * b2 + a2 * b1 + a3 * b0;
        Self([c0, c1, c2, c3])
    }
}

impl AddAssign for Fp4 {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl SubAssign for Fp4 {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl MulAssign for Fp4 {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

impl From<Fp> for Fp4 {
    fn from(value: Fp) -> Self {
        Self::from_base(value)
    }
}

/// Inverts every nonzero extension element in place with one inversion.
///
/// # Panics
/// Panics if any element is zero.
pub fn batch_inverse_ext(values: &mut [Fp4]) {
    if values.is_empty() {
        return;
    }
    let mut prefix = Vec::with_capacity(values.len());
    let mut acc = Fp4::ONE;
    for value in values.iter() {
        prefix.push(acc);
        acc *= *value;
    }
    let mut inv = acc
        .inverse()
        .expect("batch_inverse_ext input contains zero");
    for (value, before) in values.iter_mut().zip(prefix).rev() {
        let original = *value;
        *value = inv * before;
        inv *= original;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lcg(state: &mut u64) -> u64 {
        *state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        *state ^ (*state >> 29)
    }

    fn reference_mul(a: u64, b: u64) -> u64 {
        ((u128::from(a) * u128::from(b)) % u128::from(MODULUS)) as u64
    }

    #[test]
    fn base_arithmetic_matches_u128_reference() {
        let mut state = 7;
        let edge = [
            0,
            1,
            2,
            EPSILON,
            EPSILON + 1,
            MODULUS - 1,
            MODULUS - 2,
            1 << 63,
            (1 << 32) - 1,
        ];
        let mut samples: Vec<u64> = edge.to_vec();
        for _ in 0..2_000 {
            samples.push(lcg(&mut state) % MODULUS);
        }
        for &a in &samples {
            for &b in samples.iter().step_by(37) {
                let (x, y) = (Fp::new(a), Fp::new(b));
                assert_eq!((x * y).as_u64(), reference_mul(a, b));
                assert_eq!(
                    (x + y).as_u64(),
                    ((u128::from(a) + u128::from(b)) % u128::from(MODULUS)) as u64
                );
                assert_eq!(
                    (x - y).as_u64(),
                    ((u128::from(a) + u128::from(MODULUS) - u128::from(b)) % u128::from(MODULUS))
                        as u64
                );
            }
        }
    }

    #[test]
    fn inverse_and_signed_embedding() {
        let mut state = 11;
        for _ in 0..200 {
            let x = Fp::new(lcg(&mut state));
            if x.is_zero() {
                continue;
            }
            assert_eq!(x * x.inverse().unwrap(), Fp::ONE);
        }
        assert_eq!(Fp::from_i64(-5) + Fp::from_i64(5), Fp::ZERO);
        assert_eq!(Fp::from_i64(-123_456).as_i64_centered(), -123_456);
        assert_eq!(Fp::from_i128(-(1i128 << 70)), -Fp::from_i128(1i128 << 70));
        let mut values: Vec<Fp> = (1..50u64).map(Fp::new).collect();
        let original = values.clone();
        batch_inverse(&mut values);
        for (a, b) in values.iter().zip(original) {
            assert_eq!(*a * b, Fp::ONE);
        }
    }

    #[test]
    fn roots_of_unity_have_exact_order() {
        for log_n in [1u32, 2, 5, 13, 20, 32] {
            let root = Fp::two_adic_generator(log_n);
            assert_eq!(root.pow(1u64 << log_n), Fp::ONE);
            assert_ne!(root.pow(1u64 << (log_n - 1)), Fp::ONE);
        }
        // 7 is a non-residue, as required for X^4 - 7 to be irreducible.
        assert_eq!(Fp::new(EXTENSION_W).pow((MODULUS - 1) / 2), -Fp::ONE);
    }

    #[test]
    fn extension_field_axioms() {
        let mut state = 99;
        let mut rand4 = || {
            Fp4([
                Fp::new(lcg(&mut state)),
                Fp::new(lcg(&mut state)),
                Fp::new(lcg(&mut state)),
                Fp::new(lcg(&mut state)),
            ])
        };
        for _ in 0..200 {
            let (a, b, c) = (rand4(), rand4(), rand4());
            assert_eq!(a * b, b * a);
            assert_eq!((a * b) * c, a * (b * c));
            assert_eq!(a * (b + c), a * b + a * c);
            assert_eq!(a * a.inverse().unwrap(), Fp4::ONE);
            assert_eq!(a - a, Fp4::ZERO);
        }
        // X^4 = 7.
        let x = Fp4([Fp::ZERO, Fp::ONE, Fp::ZERO, Fp::ZERO]);
        assert_eq!(x.pow(4), Fp4::from_base(Fp::new(7)));
        // Frobenius-order check: a^(p^4 - 1) = 1 for nonzero a, via the multiplicative group size.
        let a = rand4();
        let p = MODULUS;
        let pow_p = |v: Fp4| v.pow(p);
        assert_eq!(pow_p(pow_p(pow_p(pow_p(a)))), a);
        let mut values: Vec<Fp4> = (0..20).map(|_| rand4()).collect();
        let original = values.clone();
        batch_inverse_ext(&mut values);
        for (a, b) in values.iter().zip(original) {
            assert_eq!(*a * b, Fp4::ONE);
        }
    }
}
