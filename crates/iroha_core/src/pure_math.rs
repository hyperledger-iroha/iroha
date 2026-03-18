//! Pure-Rust field arithmetic building blocks without external dependencies.
//!
//! This module provides a minimal finite-field implementation over a 64-bit
//! prime modulus. It is designed as a stepping stone toward removing external
//! ZK math dependencies (e.g., halo2-curves) by progressively adding larger
//! moduli (256-bit) and polynomial/IPA helpers in follow-up patches.
//!
//! Design goals:
//! - Deterministic, portable, no `unsafe`, no external math crates.
//! - Clear, documented implementation suitable for auditing.
//! - Start small (u64 modulus) with complete unit tests; extend to 256-bit
//!   moduli with the same public API in later phases.
//!
//! NOTE: This module is currently not wired into verifiers. It is a scoped
//! implementation milestone. A subsequent patch will thread these types into
//! instance decoding and (optionally) an internal IPA/KZG verifier.

/// Trait describing a 64-bit prime modulus used by [`Fp64`].
pub trait Mod64: Copy + Clone + 'static {
    /// The prime modulus p (< 2^64) for the field.
    const MODULUS: u64;
    /// Human-readable name (for debugging/docs).
    const NAME: &'static str;
}

/// A simple prime field element modulo `M::MODULUS` implemented over `u64`.
///
/// Arithmetic is performed using `u128` intermediates to avoid overflow, and
/// reduced modulo `p`. All operations are constant-time with respect to the
/// value domain of `u64` operations (branchless reductions where reasonable),
/// but do not attempt to be side-channel hardened beyond that.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct Fp64<M: Mod64> {
    v: u64,
    _m: core::marker::PhantomData<M>,
}

impl<M: Mod64> Fp64<M> {
    /// Create a field element from a canonical `u64` representative.
    /// Panics if `x >= MODULUS`.
    pub fn new(x: u64) -> Self {
        assert!(x < M::MODULUS, "not canonical");
        Self {
            v: x,
            _m: core::marker::PhantomData,
        }
    }

    /// Return the canonical `u64` representative in `[0, p)`.
    pub fn to_u64(self) -> u64 {
        self.v
    }

    /// Add two elements modulo `p`.
    pub fn add(self, rhs: Self) -> Self {
        let (s, carry) = self.v.overflowing_add(rhs.v);
        // If overflowed or s >= p, subtract p once.
        let mut r = s;
        let needs_sub = carry || r >= M::MODULUS;
        if needs_sub {
            r = r.wrapping_sub(M::MODULUS);
        }
        Self::new(r)
    }

    /// Subtract two elements modulo `p`.
    pub fn sub(self, rhs: Self) -> Self {
        let (d, borrow) = self.v.overflowing_sub(rhs.v);
        let mut r = d;
        if borrow {
            r = r.wrapping_add(M::MODULUS);
        }
        Self::new(r)
    }

    /// Multiply two elements modulo `p` using 128-bit intermediates.
    pub fn mul(self, rhs: Self) -> Self {
        let p = M::MODULUS as u128;
        let prod = (self.v as u128) * (rhs.v as u128);
        let r = (prod % p) as u64;
        Self::new(r)
    }

    /// Compute the additive inverse modulo `p`.
    pub fn neg(self) -> Self {
        if self.v == 0 {
            self
        } else {
            Self::new(M::MODULUS - self.v)
        }
    }

    /// Exponentiation by square-and-multiply.
    pub fn pow(self, mut e: u128) -> Self {
        let mut base = self;
        let mut acc = Self::one();
        while e > 0 {
            if (e & 1) == 1 {
                acc = acc.mul(base);
            }
            base = base.mul(base);
            e >>= 1;
        }
        acc
    }

    /// Multiplicative inverse using Fermat's little theorem (since p is prime):
    /// inv(x) = x^(p-2) mod p. Uses `pow`.
    pub fn invert(self) -> Option<Self> {
        if self.v == 0 {
            return None;
        }
        let e = (M::MODULUS as u128).wrapping_sub(2);
        Some(self.pow(e))
    }

    /// Zero element.
    pub fn zero() -> Self {
        Self::new(0)
    }
    /// Multiplicative identity.
    pub fn one() -> Self {
        Self::new(1 % M::MODULUS)
    }
}

impl<M: Mod64> core::ops::Add for Fp64<M> {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        self.add(rhs)
    }
}
impl<M: Mod64> core::ops::Sub for Fp64<M> {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        self.sub(rhs)
    }
}
impl<M: Mod64> core::ops::Mul for Fp64<M> {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        self.mul(rhs)
    }
}
impl<M: Mod64> core::ops::Neg for Fp64<M> {
    type Output = Self;
    fn neg(self) -> Self::Output {
        self.neg()
    }
}

/// A small 64-bit Mersenne-like prime for testing: p = 2^64 - 59.
/// This prime is commonly used in examples and fits well into `u64`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct P64m59;
impl Mod64 for P64m59 {
    const MODULUS: u64 = u64::MAX - 58; // 2^64 - 59
    const NAME: &'static str = "p64m59";
}

#[cfg(test)]
mod tests {
    use super::*;

    type F = Fp64<P64m59>;

    #[test]
    fn add_sub_roundtrip() {
        let a = F::new(123456789);
        let b = F::new(987654321);
        let c = a + b;
        let d = c - b;
        assert_eq!(d, a);
    }

    #[test]
    fn mul_basic() {
        let a = F::new(1234567);
        let b = F::new(7654321);
        let c = a * b;
        let expected = ((1234567u128 * 7654321u128) % ((u64::MAX as u128) + 1 - 59)) as u64;
        assert_eq!(c.to_u64(), expected);
    }

    #[test]
    fn inv_mul_is_one() {
        let a = F::new(1234567890123456789);
        let inv = a.invert().expect("invertible");
        let one = a * inv;
        assert_eq!(one, F::one());
    }

    #[test]
    fn neg_add_zero() {
        let a = F::new(42);
        let z = a + (-a);
        assert_eq!(z, F::zero());
    }
}
