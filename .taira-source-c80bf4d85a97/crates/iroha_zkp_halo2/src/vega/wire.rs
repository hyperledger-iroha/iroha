//! Strict Norito primitive wire types for canonical Vega proofs.
#![allow(unexpected_cfgs)]

use thiserror::Error;

use super::{
    MAX_VEGA_PROOF_BYTES_V1, VegaCurveError, VegaFieldError, VegaT256PointV1, VegaT256ScalarV1,
};

/// Failure while validating canonical Vega proof wire material.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum VegaWireError {
    /// A scalar did not occupy exactly 32 bytes.
    #[error("Vega scalar wire value must be exactly 32 bytes, got {actual}")]
    WrongScalarLength {
        /// Actual input length.
        actual: usize,
    },
    /// A proof exceeded the engine-specific pre-decode cap.
    #[error("Vega proof length {actual} exceeds hard maximum {max}")]
    ProofTooLarge {
        /// Actual input length.
        actual: usize,
        /// Closed first-release maximum.
        max: usize,
    },
    /// Exact canonical Norito decoding failed.
    #[error("invalid canonical Norito Vega proof")]
    InvalidNorito,
    /// A scalar was non-canonical or unreduced.
    #[error(transparent)]
    Scalar(#[from] VegaFieldError),
    /// A point was non-canonical, identity, off-curve, or outside the group.
    #[error(transparent)]
    Point(#[from] VegaCurveError),
}

/// Canonical 32-byte little-endian proof encoding of one T256 scalar.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(decode_from_slice)]
pub struct VegaScalarWireV1 {
    bytes: [u8; 32],
}

impl VegaScalarWireV1 {
    /// Validate and retain one exact 32-byte little-endian scalar.
    ///
    /// # Errors
    ///
    /// Rejects wrong lengths and integers greater than or equal to the T256
    /// scalar modulus; inputs are never reduced.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, VegaWireError> {
        let bytes: [u8; 32] = bytes
            .try_into()
            .map_err(|_| VegaWireError::WrongScalarLength {
                actual: bytes.len(),
            })?;
        let _ = VegaT256ScalarV1::from_le_bytes_exact(bytes)?;
        Ok(Self { bytes })
    }

    /// Construct the wire representation of a canonical scalar.
    #[must_use]
    pub fn from_scalar(scalar: VegaT256ScalarV1) -> Self {
        Self {
            bytes: scalar.to_le_bytes(),
        }
    }

    /// Decode this wire value without modular reduction.
    ///
    /// # Errors
    ///
    /// Rejects a value at or above the scalar modulus, including malformed
    /// instances obtained through raw Norito decoding.
    pub fn to_scalar(self) -> Result<VegaT256ScalarV1, VegaWireError> {
        Ok(VegaT256ScalarV1::from_le_bytes_exact(self.bytes)?)
    }

    /// Return the exact little-endian proof bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.bytes
    }
}

/// Canonical 33-byte non-identity compressed T256 proof point.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(decode_from_slice)]
pub struct VegaPointWireV1 {
    bytes: [u8; 33],
}

impl VegaPointWireV1 {
    /// Validate and retain one exact canonical non-identity point.
    ///
    /// # Errors
    ///
    /// Rejects wrong lengths, identity/all-zero values, undefined flag bits,
    /// non-canonical x-coordinates, off-curve points, and wrong-subgroup
    /// points.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, VegaWireError> {
        let point = VegaT256PointV1::from_non_identity_wire_bytes_exact(bytes)?;
        Self::from_point(point)
    }

    /// Construct the wire representation of a non-identity canonical point.
    ///
    /// # Errors
    ///
    /// Rejects the group identity.
    pub fn from_point(point: VegaT256PointV1) -> Result<Self, VegaWireError> {
        Ok(Self {
            bytes: point.to_non_identity_wire_bytes()?,
        })
    }

    /// Decode and validate this point.
    ///
    /// # Errors
    ///
    /// Rejects invalid raw values obtained through Norito decoding.
    pub fn to_point(self) -> Result<VegaT256PointV1, VegaWireError> {
        Ok(VegaT256PointV1::from_non_identity_wire_bytes_exact(
            &self.bytes,
        )?)
    }

    /// Return the exact compressed proof bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 33] {
        &self.bytes
    }
}

/// Reject an oversized proof before invoking Norito's decoder.
///
/// # Errors
///
/// Returns [`VegaWireError::ProofTooLarge`] above the closed 512 KiB cap.
pub fn validate_proof_byte_cap_v1(bytes: &[u8]) -> Result<(), VegaWireError> {
    if bytes.len() > MAX_VEGA_PROOF_BYTES_V1 {
        return Err(VegaWireError::ProofTooLarge {
            actual: bytes.len(),
            max: MAX_VEGA_PROOF_BYTES_V1,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(
        Clone,
        Debug,
        PartialEq,
        Eq,
        norito::derive::NoritoSerialize,
        norito::derive::NoritoDeserialize,
    )]
    #[norito(decode_from_slice)]
    struct PrimitiveFixture {
        scalar: VegaScalarWireV1,
        point: VegaPointWireV1,
    }

    fn fixture() -> PrimitiveFixture {
        PrimitiveFixture {
            scalar: VegaScalarWireV1::from_scalar(VegaT256ScalarV1::from_u64(0x0102)),
            point: VegaPointWireV1::from_point(
                VegaT256PointV1::canonical_generator().expect("canonical generator"),
            )
            .expect("non-identity"),
        }
    }

    #[test]
    fn norito_primitive_fixture_roundtrips_exactly() {
        let fixture = fixture();
        let encoded = norito::codec::encode_adaptive(&fixture);
        let decoded = norito::codec::decode_exact_from_slice::<PrimitiveFixture>(&encoded)
            .expect("canonical fixture");
        assert_eq!(decoded, fixture);
        assert_eq!(
            decoded.scalar.to_scalar().expect("canonical scalar"),
            VegaT256ScalarV1::from_u64(0x0102)
        );
        assert_eq!(
            decoded.point.to_point().expect("canonical point"),
            VegaT256PointV1::canonical_generator().expect("canonical generator")
        );
    }

    #[test]
    fn norito_exact_decoder_rejects_every_truncation_and_trailing_bytes() {
        let encoded = norito::codec::encode_adaptive(&fixture());
        for end in 0..encoded.len() {
            assert!(
                norito::codec::decode_exact_from_slice::<PrimitiveFixture>(&encoded[..end])
                    .is_err(),
                "truncation at {end} unexpectedly decoded"
            );
        }
        let mut trailing = encoded;
        trailing.push(0);
        assert!(norito::codec::decode_exact_from_slice::<PrimitiveFixture>(&trailing).is_err());
    }

    #[test]
    fn raw_norito_values_still_require_algebraic_validation() {
        let invalid_scalar = VegaScalarWireV1 { bytes: [0xff; 32] };
        assert_eq!(
            invalid_scalar.to_scalar(),
            Err(VegaWireError::Scalar(VegaFieldError::NonCanonicalScalar))
        );
        let identity = VegaPointWireV1 { bytes: [0; 33] };
        assert_eq!(
            identity.to_point(),
            Err(VegaWireError::Point(VegaCurveError::IdentityPoint))
        );
    }

    #[test]
    fn scalar_and_proof_caps_reject_both_sides_of_boundaries() {
        assert!(VegaScalarWireV1::from_slice(&[0; 31]).is_err());
        assert!(VegaScalarWireV1::from_slice(&[0; 33]).is_err());
        assert_eq!(
            validate_proof_byte_cap_v1(&vec![0; MAX_VEGA_PROOF_BYTES_V1]),
            Ok(())
        );
        assert_eq!(
            validate_proof_byte_cap_v1(&vec![0; MAX_VEGA_PROOF_BYTES_V1 + 1]),
            Err(VegaWireError::ProofTooLarge {
                actual: MAX_VEGA_PROOF_BYTES_V1 + 1,
                max: MAX_VEGA_PROOF_BYTES_V1,
            })
        );
    }
}
