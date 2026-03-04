//! Prime field wrapper backed by the scalar field of the Pasta Pallas curve (`Fp`).
//!
//! This implementation leverages `halo2curves` for constant-time, curve-grade
//! arithmetic and exposes a minimal API tailored to the IPA commitment scheme.

use core::{
    convert::TryInto,
    fmt,
    ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign},
};

use halo2curves::{
    ff::{Field, FromUniformBytes, PrimeField},
    pasta::Fp,
};

use crate::errors::Error;

/// Prime field element over the Pasta Pallas scalar field.
#[derive(Copy, Clone, PartialEq, Eq, Default)]
pub struct PrimeField64(pub(crate) Fp);

impl fmt::Debug for PrimeField64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PrimeField64(0x{} )", hex::encode(self.to_bytes()))
    }
}

impl PrimeField64 {
    /// Returns zero.
    #[inline]
    pub fn zero() -> Self {
        Self(Fp::ZERO)
    }

    /// Returns one.
    #[inline]
    pub fn one() -> Self {
        Self(Fp::ONE)
    }

    /// Adds two field elements.
    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }

    /// Subtracts two field elements.
    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }

    /// Multiplies two field elements.
    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn mul(self, rhs: Self) -> Self {
        Self(self.0 * rhs.0)
    }

    /// Negates a field element.
    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn neg(self) -> Self {
        Self(-self.0)
    }

    /// Computes the multiplicative inverse of the element.
    pub fn inv(self) -> Result<Self, Error> {
        Option::<Fp>::from(self.0.invert())
            .map(Self)
            .ok_or(Error::InversionOfZero)
    }

    /// Exponentiates the field element to the given `u64` exponent.
    #[inline]
    pub fn pow_u64(self, mut exp: u64) -> Self {
        let mut base = self;
        let mut acc = Self::one();
        while exp > 0 {
            if exp & 1 == 1 {
                acc = acc.mul(base);
            }
            base = base.mul(base);
            exp >>= 1;
        }
        acc
    }

    /// Exponentiates the field element to the given `u128` exponent.
    #[inline]
    pub fn pow_u128(self, mut exp: u128) -> Self {
        let mut base = self;
        let mut acc = Self::one();
        while exp > 0 {
            if exp & 1 == 1 {
                acc = acc.mul(base);
            }
            base = base.mul(base);
            exp >>= 1;
        }
        acc
    }

    /// Encode to canonical 32-byte representation.
    #[inline]
    pub fn to_bytes(self) -> [u8; 32] {
        let repr = self.0.to_repr();
        let mut out = [0u8; 32];
        out.copy_from_slice(repr.as_ref());
        out
    }

    /// Convert the field element back into a `u64`.
    ///
    /// This assumes the element was originally constructed from a `u64`, which
    /// holds for all VM-side usage. Only the lower 64 bits of the canonical
    /// representation are considered; callers must avoid values outside that
    /// range.
    #[inline]
    pub fn to_u64(self) -> u64 {
        let bytes = self.to_bytes();
        u64::from_le_bytes(bytes[..8].try_into().expect("slice has length 8"))
    }

    /// Decode from canonical 32-byte representation.
    #[inline]
    pub fn from_bytes(bytes: &[u8; 32]) -> Result<Self, Error> {
        let mut repr = <Fp as PrimeField>::Repr::default();
        repr.as_mut().copy_from_slice(bytes);
        Option::<Fp>::from(Fp::from_repr(repr))
            .map(Self)
            .ok_or(Error::InvalidEncoding)
    }

    /// Derive a field element from uniformly random bytes.
    pub fn from_uniform(bytes: &[u8; 64]) -> Self {
        let mut val = Fp::from_uniform_bytes(bytes);
        if val.is_zero().into() {
            val = Fp::ONE;
        }
        Self(val)
    }

    /// Access the inner field element.
    #[inline]
    pub fn into_inner(self) -> Fp {
        self.0
    }

    /// Compute the multiplicative inverse, returning an error for zero.
    #[inline]
    pub fn inverse(self) -> Result<Self, Error> {
        self.inv()
    }
}

impl From<u64> for PrimeField64 {
    fn from(value: u64) -> Self {
        Self(Fp::from(value))
    }
}

impl From<u32> for PrimeField64 {
    fn from(value: u32) -> Self {
        Self(Fp::from(u64::from(value)))
    }
}

impl From<Fp> for PrimeField64 {
    fn from(value: Fp) -> Self {
        Self(value)
    }
}

impl From<PrimeField64> for Fp {
    fn from(value: PrimeField64) -> Self {
        value.0
    }
}

impl Add for PrimeField64 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        PrimeField64::add(self, rhs)
    }
}

impl AddAssign for PrimeField64 {
    fn add_assign(&mut self, rhs: Self) {
        *self = PrimeField64::add(*self, rhs);
    }
}

impl Sub for PrimeField64 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        PrimeField64::sub(self, rhs)
    }
}

impl SubAssign for PrimeField64 {
    fn sub_assign(&mut self, rhs: Self) {
        *self = PrimeField64::sub(*self, rhs);
    }
}

impl Mul for PrimeField64 {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        PrimeField64::mul(self, rhs)
    }
}

impl MulAssign for PrimeField64 {
    fn mul_assign(&mut self, rhs: Self) {
        *self = PrimeField64::mul(*self, rhs);
    }
}

impl Neg for PrimeField64 {
    type Output = Self;

    fn neg(self) -> Self::Output {
        PrimeField64::neg(self)
    }
}
