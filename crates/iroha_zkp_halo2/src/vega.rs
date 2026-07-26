//! Native field and curve adapters for the pinned Microsoft Vega profile.
//!
//! Vega's canonical engine uses the T256 group. Its scalar field is exactly
//! the P-256 base field, which lets the mDL circuit ingest issuer-key
//! coordinates without non-native reduction. This module keeps that identity
//! explicit and exposes only canonical, non-reducing encodings to callers.
//!
//! The protocol source is Microsoft `vega-prover` commit
//! `c0ee259053cd12eaf43ed71b5cde375452b3ee4d`, licensed under MIT.

use core::fmt;

use halo2curves::{ff::PrimeField, t256::Fq};
use thiserror::Error;

/// Big-endian modulus of the canonical T256 scalar field.
///
/// This is also the base-field modulus of NIST P-256.
pub const VEGA_T256_SCALAR_MODULUS_BE_V1: [u8; 32] = [
    0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
];

/// Failure while translating canonical Vega field material.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum VegaFieldError {
    /// The supplied big-endian integer is not smaller than the T256 scalar
    /// modulus.
    #[error("integer is not a canonical T256 scalar")]
    NonCanonicalScalar,
}

/// Canonical T256 scalar used by Vega public inputs and proof-system algebra.
///
/// Construction is deliberately non-reducing: byte strings at or above the
/// modulus are rejected rather than silently mapped into the field.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct VegaT256ScalarV1(Fq);

impl VegaT256ScalarV1 {
    /// Parse one canonical 32-byte big-endian scalar without modular reduction.
    ///
    /// # Errors
    ///
    /// Returns [`VegaFieldError::NonCanonicalScalar`] when `bytes` represents
    /// an integer greater than or equal to the scalar modulus.
    pub fn from_be_bytes_exact(bytes: [u8; 32]) -> Result<Self, VegaFieldError> {
        if bytes >= VEGA_T256_SCALAR_MODULUS_BE_V1 {
            return Err(VegaFieldError::NonCanonicalScalar);
        }
        // `halo2curves` 0.9 exposes the P-256 base-field representation in
        // little-endian order. Keep that implementation detail behind this
        // explicitly big-endian Vega boundary.
        let mut repr = bytes;
        repr.reverse();
        let value = Option::<Fq>::from(Fq::from_repr(repr.into()))
            .ok_or(VegaFieldError::NonCanonicalScalar)?;
        Ok(Self(value))
    }

    /// Construct a scalar from an unsigned 64-bit integer.
    #[must_use]
    pub fn from_u64(value: u64) -> Self {
        Self(Fq::from(value))
    }

    /// Return the exact canonical 32-byte big-endian representation.
    #[must_use]
    pub fn to_be_bytes(self) -> [u8; 32] {
        let mut bytes: [u8; 32] = self.0.to_repr().into();
        bytes.reverse();
        bytes
    }

    /// Return whether this field element is zero.
    #[must_use]
    pub fn is_zero(self) -> bool {
        self.to_be_bytes() == [0; 32]
    }
}

impl fmt::Debug for VegaT256ScalarV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("VegaT256ScalarV1")
            .field(&hex::encode(self.to_be_bytes()))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use halo2curves::ff::PrimeField;

    use super::*;

    #[test]
    fn t256_scalar_modulus_is_exactly_the_p256_base_modulus() {
        assert_eq!(
            Fq::MODULUS,
            "0xffffffff00000001000000000000000000000000ffffffffffffffffffffffff"
        );
        let mut below = VEGA_T256_SCALAR_MODULUS_BE_V1;
        below[31] -= 1;
        let parsed = VegaT256ScalarV1::from_be_bytes_exact(below).expect("q - 1 is canonical");
        assert_eq!(parsed.to_be_bytes(), below);
        assert_eq!(
            VegaT256ScalarV1::from_be_bytes_exact(VEGA_T256_SCALAR_MODULUS_BE_V1),
            Err(VegaFieldError::NonCanonicalScalar)
        );
        assert_eq!(
            VegaT256ScalarV1::from_be_bytes_exact([0xff; 32]),
            Err(VegaFieldError::NonCanonicalScalar)
        );
    }

    #[test]
    fn t256_scalar_big_endian_boundary_does_not_reduce() {
        for value in [0_u64, 1, 255, 256, u32::MAX.into(), u64::MAX] {
            let scalar = VegaT256ScalarV1::from_u64(value);
            let mut expected = [0_u8; 32];
            expected[24..].copy_from_slice(&value.to_be_bytes());
            assert_eq!(scalar.to_be_bytes(), expected);
            assert_eq!(VegaT256ScalarV1::from_be_bytes_exact(expected), Ok(scalar));
        }
    }
}
