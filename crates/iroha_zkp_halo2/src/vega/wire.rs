//! Strict Norito primitive wire types for canonical Vega proofs.
#![allow(unexpected_cfgs)]
use super::{VegaCurveError, VegaFieldError, VegaT256PointV1, VegaT256ScalarV1};
use thiserror::Error;
/// Failure while validating canonical Vega proof wire material.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum VegaWireError {
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
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
#[norito(decode_from_slice)]
pub struct VegaScalarWireV1 {
    bytes: [u8; 32],
}
impl VegaScalarWireV1 {
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
    #[cfg(test)]
    pub(super) const fn from_raw_bytes_for_test(bytes: [u8; 32]) -> Self {
        Self { bytes }
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
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
#[norito(decode_from_slice)]
pub struct VegaPointWireV1 {
    bytes: [u8; 33],
}
impl VegaPointWireV1 {
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
    #[cfg(test)]
    pub(super) const fn from_raw_bytes_for_test(bytes: [u8; 33]) -> Self {
        Self { bytes }
    }
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
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
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
}
