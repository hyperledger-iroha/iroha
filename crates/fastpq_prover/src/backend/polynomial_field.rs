//! One checked field owner for bounded public-polynomial and AIR evaluation.
//!
//! Both the base field and its existing quartic extension evaluate the same
//! polynomials. This module selects no proof geometry, transcript or mask degree.

use super::{GOLDILOCKS_MODULUS, field_inverse, mul_mod};
use crate::{
    Error, Result, field::GoldilocksFp4V1, gadgets::transfer_integer_air::IntegerAirField,
};

/// Canonical fields supported by the fixed polynomial evaluation boundary.
pub(super) trait PolynomialField: IntegerAirField + Eq {
    /// Number of base coefficients in this field's canonical encoding.
    const COEFFICIENTS: usize;
    /// First noncanonical coefficient, if any.
    fn noncanonical_coefficient(self) -> Option<usize>;
    /// Embed a canonical base constant without dropping extension coordinates.
    fn embed_base(value: u64) -> Self;
    /// Multiply by a canonical base constant.
    fn scale_base(self, value: u64) -> Self;
    /// Invert a nonzero field element; zero has no inverse.
    fn inverse(self) -> Option<Self>;

    /// Raise a canonical field value to a public integer exponent.
    fn power(self, mut exponent: u64) -> Self {
        let mut base = self;
        let mut result = Self::ONE;
        while exponent != 0 {
            if exponent & 1 != 0 {
                result = result.mul(base);
            }
            base = base.mul(base);
            exponent >>= 1;
        }
        result
    }

    /// Reject malformed coefficients before arithmetic or temporary allocation.
    fn validate(self, context: &'static str, indices: &[usize]) -> Result<()> {
        if let Some(coefficient) = self.noncanonical_coefficient() {
            let mut indices = indices.to_vec();
            if Self::COEFFICIENTS > 1 {
                indices.push(coefficient);
            }
            return Err(Error::NonCanonicalGoldilocksElement { context, indices });
        }
        Ok(())
    }
}

impl PolynomialField for u64 {
    const COEFFICIENTS: usize = 1;
    fn noncanonical_coefficient(self) -> Option<usize> {
        (self >= GOLDILOCKS_MODULUS).then_some(0)
    }
    fn embed_base(value: u64) -> Self {
        debug_assert!(value < GOLDILOCKS_MODULUS);
        value
    }
    fn scale_base(self, value: u64) -> Self {
        mul_mod(self, value)
    }
    fn inverse(self) -> Option<Self> {
        (self != 0 && self < GOLDILOCKS_MODULUS).then(|| field_inverse(self))
    }
}

impl PolynomialField for GoldilocksFp4V1 {
    const COEFFICIENTS: usize = 4;
    fn noncanonical_coefficient(self) -> Option<usize> {
        self.coefficients()
            .iter()
            .position(|&value| value >= GOLDILOCKS_MODULUS)
    }
    fn embed_base(value: u64) -> Self {
        Self::from_base(value).expect("fixed base constants are canonical")
    }
    fn scale_base(self, value: u64) -> Self {
        self.mul_base(value)
    }
    fn inverse(self) -> Option<Self> {
        if self == Self::ZERO || self.noncanonical_coefficient().is_some() {
            return None;
        }
        // Frobenius conjugation is exact in the existing field F_p[u]/(u^4-7).
        // The product of all four conjugates is a base-field norm. This avoids
        // narrowing p^4-2 to a machine exponent or defining another field basis.
        let first = self.power(GOLDILOCKS_MODULUS);
        let second = first.power(GOLDILOCKS_MODULUS);
        let third = second.power(GOLDILOCKS_MODULUS);
        let conjugates = first.mul(second).mul(third);
        let [norm, b, c, d] = self.mul(conjugates).coefficients();
        if b != 0 || c != 0 || d != 0 {
            return None;
        }
        norm.inverse().map(|inverse| conjugates.mul_base(inverse))
    }
}

#[cfg(test)]
#[path = "polynomial_field/tests.rs"]
mod tests;
