//! Native field and curve adapters for the pinned Microsoft Vega profile.
//!
//! Vega's canonical engine uses the T256 group. Its scalar field is exactly
//! the P-256 base field, which lets the mDL circuit ingest issuer-key
//! coordinates without non-native reduction. This module keeps that identity
//! explicit and exposes only canonical, non-reducing encodings to callers.
//!
//! The protocol source is Microsoft `vega-prover` commit
//! `c0ee259053cd12eaf43ed71b5cde375452b3ee4d`, licensed under MIT.

use core::{
    fmt,
    ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign},
};

use halo2curves::{
    ff::{Field, FromUniformBytes, PrimeField},
    t256::Fq,
};
use thiserror::Error;

#[path = "vega/algebra.rs"]
mod algebra;
#[path = "vega/circuit.rs"]
mod circuit;
#[path = "vega/commitment.rs"]
mod commitment;
#[path = "vega/curve.rs"]
mod curve;
#[path = "vega/date.rs"]
mod date;
#[path = "vega/figure9.rs"]
mod figure9;
#[path = "vega/figure9_layout.rs"]
mod figure9_layout;
#[path = "vega/hyrax.rs"]
mod hyrax;
#[path = "vega/nifs.rs"]
mod nifs;
#[path = "vega/p256.rs"]
mod p256;
#[path = "vega/r1cs.rs"]
mod r1cs;
#[path = "vega/sha256.rs"]
mod sha256;
#[path = "vega/spartan.rs"]
mod spartan;
#[path = "vega/sponge.rs"]
mod sponge;
#[path = "vega/sumcheck.rs"]
mod sumcheck;
#[path = "vega/transcript.rs"]
mod transcript;
#[path = "vega/wire.rs"]
mod wire;

pub use curve::{
    VEGA_T256_BASE_MODULUS_BE_V1, VegaCurveError, VegaT256PointV1, derive_t256_generators_v1,
};
pub use figure9::{
    VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1, VegaMdlFigure9ErrorV1, VegaMdlFigure9WitnessV1,
    validate_vega_mdl_figure9_encoding_v1, validate_vega_mdl_figure9_relation_v1,
};
pub use transcript::{VegaTranscriptError, VegaTranscriptV1};
pub use wire::{VegaPointWireV1, VegaScalarWireV1, VegaWireError, validate_proof_byte_cap_v1};

/// Tight first-release cap for one canonical Norito Vega proof.
///
/// Microsoft's 1,920-byte mDL benchmark produces proofs of about 108 KiB.
/// A 512 KiB ceiling leaves room for the exact Figure 9 relation and Norito
/// framing while preventing this engine from inheriting the much broader
/// per-action opaque-byte allowance.
pub const MAX_VEGA_PROOF_BYTES_V1: usize = 512 * 1024;

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
    /// The zero scalar does not have a multiplicative inverse.
    #[error("cannot invert the zero T256 scalar")]
    InversionOfZero,
}

/// Canonical T256 scalar used by Vega public inputs and proof-system algebra.
///
/// Construction is deliberately non-reducing: byte strings at or above the
/// modulus are rejected rather than silently mapped into the field.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct VegaT256ScalarV1(Fq);

impl VegaT256ScalarV1 {
    /// Return the additive identity.
    #[must_use]
    pub fn zero() -> Self {
        Self(Fq::ZERO)
    }

    /// Return the multiplicative identity.
    #[must_use]
    pub fn one() -> Self {
        Self(Fq::ONE)
    }

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

    /// Parse one canonical 32-byte little-endian proof scalar without modular
    /// reduction.
    ///
    /// # Errors
    ///
    /// Returns [`VegaFieldError::NonCanonicalScalar`] when `bytes` represents
    /// an integer greater than or equal to the scalar modulus.
    pub fn from_le_bytes_exact(mut bytes: [u8; 32]) -> Result<Self, VegaFieldError> {
        bytes.reverse();
        Self::from_be_bytes_exact(bytes)
    }

    /// Reduce an exact 64-byte little-endian uniform string as specified by
    /// the pinned Vega Fiat--Shamir transcript.
    #[must_use]
    pub fn from_uniform_le_bytes(bytes: [u8; 64]) -> Self {
        Self(Fq::from_uniform_bytes(&bytes))
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

    /// Return the exact canonical 32-byte little-endian proof encoding.
    #[must_use]
    pub fn to_le_bytes(self) -> [u8; 32] {
        let mut bytes = self.to_be_bytes();
        bytes.reverse();
        bytes
    }

    /// Return whether this field element is zero.
    #[must_use]
    pub fn is_zero(self) -> bool {
        bool::from(self.0.is_zero())
    }

    /// Return the multiplicative inverse.
    ///
    /// # Errors
    ///
    /// Returns [`VegaFieldError::InversionOfZero`] for the additive identity.
    pub fn inverse(self) -> Result<Self, VegaFieldError> {
        Option::<Fq>::from(self.0.invert())
            .map(Self)
            .ok_or(VegaFieldError::InversionOfZero)
    }

    /// Square this scalar.
    #[must_use]
    pub fn square(self) -> Self {
        Self(self.0.square())
    }
}

impl Default for VegaT256ScalarV1 {
    fn default() -> Self {
        Self::zero()
    }
}

impl Add for VegaT256ScalarV1 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl AddAssign for VegaT256ScalarV1 {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl Sub for VegaT256ScalarV1 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl SubAssign for VegaT256ScalarV1 {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

impl Mul for VegaT256ScalarV1 {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self(self.0 * rhs.0)
    }
}

impl MulAssign for VegaT256ScalarV1 {
    fn mul_assign(&mut self, rhs: Self) {
        self.0 *= rhs.0;
    }
}

impl Neg for VegaT256ScalarV1 {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self(-self.0)
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
