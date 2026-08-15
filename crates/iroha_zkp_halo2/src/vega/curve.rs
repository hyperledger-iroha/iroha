//! Canonical T256 group boundary for the pinned Vega profile.
use super::{VEGA_T256_SCALAR_MODULUS_BE_V1, VegaT256ScalarV1, sponge::shake256};
use core::{
    fmt,
    ops::{Add, Neg as _, Sub},
};
#[cfg(test)]
use halo2curves::t256::Fp;
use halo2curves::{
    Coordinates, CurveAffine, CurveExt,
    ff::PrimeField,
    group::{Curve as _, Group as _, GroupEncoding as _},
    t256::{T256, T256Affine},
};
use thiserror::Error;
/// Big-endian modulus of the canonical T256 coordinate field.
pub const VEGA_T256_BASE_MODULUS_BE_V1: [u8; 32] = [
    0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
    0x7e, 0x72, 0xb4, 0x2b, 0x30, 0xe7, 0x31, 0x77, 0x93, 0x13, 0x56, 0x61, 0xb1, 0xc4, 0xb1, 0x17,
];
#[cfg(test)]
const CANONICAL_GENERATOR_Y_BE_V1: [u8; 32] = [
    0x5a, 0x6d, 0xd3, 0x2d, 0xf5, 0x87, 0x08, 0xe6, 0x4e, 0x97, 0x34, 0x5c, 0xbe, 0x66, 0x60, 0x0d,
    0xec, 0xd9, 0xd5, 0x38, 0xa3, 0x51, 0xbb, 0x3c, 0x30, 0xb4, 0x95, 0x49, 0x25, 0xb1, 0xf0, 0x2d,
];
/// Maximum number of deterministic Hyrax generators derived in one call.
pub const MAX_VEGA_T256_GENERATORS_V1: usize = 1 << 20;
/// Failure at the strict T256 point or generator-derivation boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum VegaCurveError {
    /// A proof point did not occupy exactly 33 bytes.
    #[error("T256 point must be exactly 33 bytes, got {actual}")]
    WrongPointLength {
        /// Actual input length.
        actual: usize,
    },
    /// A coordinate is not a canonical element of the T256 base field.
    #[error("T256 point has a non-canonical coordinate")]
    NonCanonicalCoordinate,
    /// The encoding used undefined flag bits or was otherwise malleable.
    #[error("T256 point encoding is not canonical")]
    NonCanonicalEncoding,
    /// The encoded affine coordinates are not on the canonical T256 curve.
    #[error("T256 point is not on the canonical curve")]
    OffCurve,
    /// Proof wire material attempted to use the point at infinity.
    #[error("T256 proof point must not be the identity")]
    IdentityPoint,
    /// A decoded point did not belong to the prime-order group.
    #[error("T256 point is not in the prime-order subgroup")]
    WrongSubgroup,
    /// A transcript attempted to absorb the identity, which has no affine representation.
    #[error("cannot absorb the T256 identity into a Vega transcript")]
    IdentityTranscriptPoint,
    /// Generator derivation received an empty or excessively long label.
    #[error("T256 generator label length must be in 1..=255")]
    InvalidGeneratorLabelLength,
    /// Generator derivation received an empty or excessive requested count.
    #[error("T256 generator count must be in 1..={MAX_VEGA_T256_GENERATORS_V1}")]
    InvalidGeneratorCount,
    /// Fixed canonical generator constants no longer form a point under the
    /// linked curve implementation. This is retained only for the independent
    /// group-law known-answer tests; production generators use SHAKE256.
    #[cfg(test)]
    #[error("linked T256 implementation disagrees with the canonical x=3 generator")]
    CanonicalGeneratorMismatch,
}
/// A point in Vega's canonical T256 prime-order group.
///
/// The linked `halo2curves` 0.9 implementation uses a historical x=5 generator.
/// This wrapper never calls that implementation's `generator()` method; the
/// protocol's x=3 generator is constructed and checked explicitly.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct VegaT256PointV1(pub(super) T256);
impl VegaT256PointV1 {
    /// Construct Vega's canonical x=3 generator.
    ///
    /// # Errors
    ///
    /// Returns [`VegaCurveError::CanonicalGeneratorMismatch`] if the fixed
    /// protocol coordinates do not validate under the linked curve arithmetic.
    #[cfg(test)]
    pub fn canonical_generator() -> Result<Self, VegaCurveError> {
        let mut x = [0_u8; 32];
        x[31] = 3;
        let x = base_from_be_exact(x).ok_or(VegaCurveError::CanonicalGeneratorMismatch)?;
        let y = base_from_be_exact(CANONICAL_GENERATOR_Y_BE_V1)
            .ok_or(VegaCurveError::CanonicalGeneratorMismatch)?;
        Option::<T256Affine>::from(T256Affine::from_xy(x, y))
            .map(T256::from)
            .map(Self)
            .ok_or(VegaCurveError::CanonicalGeneratorMismatch)
    }
    /// Decode one exact, non-identity canonical 33-byte proof point.
    ///
    /// The format is one flag byte (`0x00` or `0x80`, the parity of `y`) followed by the canonical
    /// big-endian x-coordinate. Infinity, x=0, undefined flag bits, non-canonical coordinates,
    /// off-curve points, alternate encodings, and trailing or truncated material are rejected.
    ///
    /// # Errors
    ///
    /// Returns a granular [`VegaCurveError`] for the first failed structural
    /// or algebraic invariant.
    pub fn from_non_identity_wire_bytes_exact(bytes: &[u8]) -> Result<Self, VegaCurveError> {
        let raw: [u8; 33] = bytes
            .try_into()
            .map_err(|_| VegaCurveError::WrongPointLength {
                actual: bytes.len(),
            })?;
        match raw[0] {
            0x40 => return Err(VegaCurveError::IdentityPoint),
            0x00 | 0x80 => {}
            _ => return Err(VegaCurveError::NonCanonicalEncoding),
        }
        let x: [u8; 32] = raw[1..].try_into().expect("point x has fixed length");
        if x == [0; 32] {
            return Err(VegaCurveError::IdentityPoint);
        }
        if x >= VEGA_T256_BASE_MODULUS_BE_V1 {
            return Err(VegaCurveError::NonCanonicalCoordinate);
        }
        let repr = raw.into();
        let point = Option::<T256>::from(T256::from_bytes(&repr))
            .map(Self)
            .ok_or(VegaCurveError::OffCurve)?;
        if point.is_identity() {
            return Err(VegaCurveError::IdentityPoint);
        }
        if point.to_non_identity_wire_bytes()? != raw {
            return Err(VegaCurveError::NonCanonicalEncoding);
        }
        if !point.has_prime_order() {
            return Err(VegaCurveError::WrongSubgroup);
        }
        Ok(point)
    }
    /// Encode this point in the canonical non-identity 33-byte proof format.
    ///
    /// # Errors
    ///
    /// Returns [`VegaCurveError::IdentityPoint`] for the group identity.
    pub fn to_non_identity_wire_bytes(self) -> Result<[u8; 33], VegaCurveError> {
        let mut encoded = [0_u8; 33];
        self.write_non_identity_wire_bytes_ref(&mut encoded)?;
        Ok(encoded)
    }
    /// Write this borrowed point's canonical non-identity proof encoding into
    /// caller-owned storage without introducing a by-value point boundary.
    ///
    /// # Errors
    ///
    /// Returns [`VegaCurveError::IdentityPoint`] for the group identity.
    pub fn write_non_identity_wire_bytes_ref(
        &self,
        destination: &mut [u8; 33],
    ) -> Result<(), VegaCurveError> {
        if bool::from(self.0.is_identity()) {
            return Err(VegaCurveError::IdentityPoint);
        }
        destination.copy_from_slice(self.0.to_bytes().as_ref());
        Ok(())
    }
    /// Return this point's canonical big-endian affine coordinates.
    ///
    /// # Errors
    ///
    /// Returns [`VegaCurveError::IdentityPoint`] for the group identity.
    pub fn coordinates_be(self) -> Result<([u8; 32], [u8; 32]), VegaCurveError> {
        if bool::from(self.0.is_identity()) {
            return Err(VegaCurveError::IdentityPoint);
        }
        let affine = self.0.to_affine();
        let coordinates = Option::<Coordinates<T256Affine>>::from(affine.coordinates())
            .ok_or(VegaCurveError::IdentityPoint)?;
        let mut x: [u8; 32] = coordinates.x().to_repr().into();
        let mut y: [u8; 32] = coordinates.y().to_repr().into();
        // `PrimeField::Repr` is little-endian for both linked fields even
        // though T256 point compression uses its base field's big-endian
        // `EndianRepr`. Keep the public coordinate boundary explicitly BE.
        x.reverse();
        y.reverse();
        Ok((x, y))
    }
    /// Return the exact upstream transcript representation `x_LE || y_LE`.
    ///
    /// # Errors
    ///
    /// Returns [`VegaCurveError::IdentityTranscriptPoint`] for the identity.
    pub fn to_transcript_bytes(self) -> Result<[u8; 64], VegaCurveError> {
        let mut output = [0_u8; 64];
        self.write_transcript_bytes_ref(&mut output)?;
        Ok(output)
    }
    /// Write this borrowed point's exact upstream transcript representation
    /// `x_LE || y_LE` into caller-owned storage without a by-value point API.
    ///
    /// # Errors
    ///
    /// Returns [`VegaCurveError::IdentityTranscriptPoint`] for the identity.
    pub fn write_transcript_bytes_ref(
        &self,
        destination: &mut [u8; 64],
    ) -> Result<(), VegaCurveError> {
        if bool::from(self.0.is_identity()) {
            return Err(VegaCurveError::IdentityTranscriptPoint);
        }
        let affine = self.0.to_affine();
        let coordinates = Option::<Coordinates<T256Affine>>::from(affine.coordinates())
            .ok_or(VegaCurveError::IdentityTranscriptPoint)?;
        destination[..32].copy_from_slice(coordinates.x().to_repr().as_ref());
        destination[32..].copy_from_slice(coordinates.y().to_repr().as_ref());
        Ok(())
    }
    /// Return whether this point is the group identity.
    #[must_use]
    pub fn is_identity(self) -> bool {
        bool::from(self.0.is_identity())
    }
    /// Negate a T256 point.
    #[must_use]
    pub fn negate(self) -> Self {
        Self(self.0.neg())
    }
    /// Multiply this point by one canonical T256 scalar.
    #[must_use]
    pub fn mul_scalar(self, scalar: VegaT256ScalarV1) -> Self {
        Self(self.0 * scalar.0)
    }
    /// Select `a` for zero and `b` for one without secret-dependent branches.
    ///
    /// Only the low bit of `choice` is used. Multiplication by that scalar uses the linked curve's
    /// constant-time scalar multiplication and avoids a secret-dependent branch or table lookup.
    #[must_use]
    pub fn conditional_select(a: &Self, b: &Self, choice: u8) -> Self {
        *a + (*b - *a).mul_scalar(VegaT256ScalarV1::from_u64(u64::from(choice & 1)))
    }
    /// Replace this complete projective point instance with the identity.
    ///
    /// This is best-effort safe erasure for a named value. The point is
    /// [`Copy`], so compiler-created copies and register temporaries cannot be
    /// guaranteed erased, and no destructor runs after process abort.
    pub fn clear_secret(&mut self) {
        *self = Self::identity();
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut *self);
    }
    pub(super) fn identity() -> Self {
        Self(T256::identity())
    }
    fn has_prime_order(self) -> bool {
        // T256 has cofactor one. Checking the published group order with raw
        // double-and-add avoids the vacuous `Fq::from(q) == 0` scalar path and
        // also guards accidental linkage to incompatible curve parameters.
        let mut result = Self::identity();
        for byte in VEGA_T256_SCALAR_MODULUS_BE_V1 {
            for bit in (0..8).rev() {
                result = result + result;
                if byte & (1 << bit) != 0 {
                    result += self;
                }
            }
        }
        result.is_identity()
    }
}
impl Add for VegaT256PointV1 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}
impl Sub for VegaT256PointV1 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}
impl fmt::Debug for VegaT256PointV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_identity() {
            return formatter.write_str("VegaT256PointV1(identity)");
        }
        formatter
            .debug_tuple("VegaT256PointV1")
            .field(
                &self
                    .to_non_identity_wire_bytes()
                    .map(hex::encode)
                    .unwrap_or_else(|_| String::from("invalid")),
            )
            .finish()
    }
}
/// Derive canonical nothing-up-my-sleeve T256 generators from a label.
///
/// This is the pinned Vega derivation: SHAKE256 emits consecutive 32-byte messages, each mapped
/// with the T256 RFC 9380 suite under the `from_uniform_bytes` domain prefix.
///
/// # Errors
///
/// Rejects labels outside 1..=255 bytes, counts outside 1..=[`MAX_VEGA_T256_GENERATORS_V1`], or the
/// cryptographically negligible event that hash-to-curve returns the identity.
pub fn derive_t256_generators_v1(
    label: &[u8],
    count: usize,
) -> Result<Vec<VegaT256PointV1>, VegaCurveError> {
    if label.is_empty() || label.len() > u8::MAX.into() {
        return Err(VegaCurveError::InvalidGeneratorLabelLength);
    }
    if count == 0 || count > MAX_VEGA_T256_GENERATORS_V1 {
        return Err(VegaCurveError::InvalidGeneratorCount);
    }
    let byte_len = count
        .checked_mul(32)
        .ok_or(VegaCurveError::InvalidGeneratorCount)?;
    let uniform = shake256(label, byte_len);
    let hash_to_curve = T256::hash_to_curve("from_uniform_bytes");
    uniform
        .chunks_exact(32)
        .map(|message| {
            let point = VegaT256PointV1(hash_to_curve(message));
            if point.is_identity() {
                Err(VegaCurveError::IdentityPoint)
            } else {
                Ok(point)
            }
        })
        .collect()
}
#[cfg(test)]
fn base_from_be_exact(bytes: [u8; 32]) -> Option<Fp> {
    if bytes >= VEGA_T256_BASE_MODULUS_BE_V1 {
        return None;
    }
    let mut repr = bytes;
    repr.reverse();
    Option::from(Fp::from_repr(repr.into()))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn decode_hex<const N: usize>(value: &str) -> [u8; N] {
        hex::decode(value)
            .expect("valid hex")
            .try_into()
            .expect("fixed vector length")
    }
    #[test]
    fn canonical_generator_and_group_law_match_independent_vectors() {
        let generator = VegaT256PointV1::canonical_generator().expect("canonical generator");
        assert_eq!(
            generator
                .to_non_identity_wire_bytes()
                .expect("non-identity"),
            decode_hex("800000000000000000000000000000000000000000000000000000000000000003")
        );
        assert_eq!(
            generator
                .mul_scalar(VegaT256ScalarV1::from_u64(2))
                .to_non_identity_wire_bytes()
                .expect("non-identity"),
            decode_hex("8016f70c3f35b3257896971b306635647bc52eb7cad7a5eca1a42f2340737749e3")
        );
        assert_eq!(
            generator
                .mul_scalar(VegaT256ScalarV1::from_u64(7))
                .to_non_identity_wire_bytes()
                .expect("non-identity"),
            decode_hex("00a37dc092877e239385cd8392ba2360ce1859a37f7a2b9c626b336608d2ce4cfe")
        );
        assert!((generator + generator.negate()).is_identity());
        assert_eq!(
            generator.mul_scalar(VegaT256ScalarV1::from_u64(2)) - generator,
            generator
        );
        let mut q_minus_one = VEGA_T256_SCALAR_MODULUS_BE_V1;
        q_minus_one[31] -= 1;
        let minus_one = VegaT256ScalarV1::from_be_bytes_exact(q_minus_one).expect("q - 1 scalar");
        assert!((generator.mul_scalar(minus_one) + generator).is_identity());
        let identity = VegaT256PointV1::identity();
        assert_eq!(
            VegaT256PointV1::conditional_select(&identity, &generator, 0),
            identity
        );
        assert_eq!(
            VegaT256PointV1::conditional_select(&identity, &generator, 1),
            generator
        );
        let mut cleared = generator;
        cleared.clear_secret();
        assert_eq!(cleared, identity);
    }
    #[test]
    fn generator_derivation_matches_independent_rfc9380_vector() {
        let point = derive_t256_generators_v1(b"vega-t256-kat", 1)
            .expect("valid derivation")
            .pop()
            .expect("one point");
        assert_eq!(
            point.to_non_identity_wire_bytes().expect("non-identity"),
            decode_hex("8025a4e3128f042d728e58b7e09a51b72585be4435f4e94aac8517f2e158b3eae6")
        );
    }
    #[test]
    fn strict_point_wire_rejects_every_malleable_boundary_class() {
        let generator = VegaT256PointV1::canonical_generator().expect("canonical generator");
        let wire = generator
            .to_non_identity_wire_bytes()
            .expect("non-identity");
        assert_eq!(
            VegaT256PointV1::from_non_identity_wire_bytes_exact(&wire),
            Ok(generator)
        );
        assert!(matches!(
            VegaT256PointV1::from_non_identity_wire_bytes_exact(&wire[..32]),
            Err(VegaCurveError::WrongPointLength { actual: 32 })
        ));
        let mut trailing = wire.to_vec();
        trailing.push(0);
        assert!(matches!(
            VegaT256PointV1::from_non_identity_wire_bytes_exact(&trailing),
            Err(VegaCurveError::WrongPointLength { actual: 34 })
        ));
        assert_eq!(
            VegaT256PointV1::from_non_identity_wire_bytes_exact(&[0; 33]),
            Err(VegaCurveError::IdentityPoint)
        );
        let mut identity = [0_u8; 33];
        identity[0] = 0x40;
        assert_eq!(
            VegaT256PointV1::from_non_identity_wire_bytes_exact(&identity),
            Err(VegaCurveError::IdentityPoint)
        );
        identity[0] = 0xc0;
        assert_eq!(
            VegaT256PointV1::from_non_identity_wire_bytes_exact(&identity),
            Err(VegaCurveError::NonCanonicalEncoding)
        );
        let mut undefined_flag = wire;
        undefined_flag[0] = 0x01;
        assert_eq!(
            VegaT256PointV1::from_non_identity_wire_bytes_exact(&undefined_flag),
            Err(VegaCurveError::NonCanonicalEncoding)
        );
        let mut noncanonical_x = [0_u8; 33];
        noncanonical_x[1..].copy_from_slice(&VEGA_T256_BASE_MODULUS_BE_V1);
        assert_eq!(
            VegaT256PointV1::from_non_identity_wire_bytes_exact(&noncanonical_x),
            Err(VegaCurveError::NonCanonicalCoordinate)
        );
        let mut off_curve = [0_u8; 33];
        off_curve[32] = 1;
        assert_eq!(
            VegaT256PointV1::from_non_identity_wire_bytes_exact(&off_curve),
            Err(VegaCurveError::OffCurve)
        );
    }
    #[test]
    fn borrowed_nonidentity_point_writer_matches_owned_encoding() {
        let generator = VegaT256PointV1::canonical_generator().expect("canonical generator");
        let expected = generator
            .to_non_identity_wire_bytes()
            .expect("nonidentity owned encoding");
        let mut borrowed = [0_u8; 33];
        generator
            .write_non_identity_wire_bytes_ref(&mut borrowed)
            .expect("nonidentity borrowed encoding");
        assert_eq!(borrowed, expected);
        let mut identity = [0xa5_u8; 33];
        assert_eq!(
            VegaT256PointV1::identity().write_non_identity_wire_bytes_ref(&mut identity),
            Err(VegaCurveError::IdentityPoint)
        );
    }
    #[test]
    fn point_transcript_encoding_is_uncompressed_little_endian() {
        let generator = VegaT256PointV1::canonical_generator().expect("canonical generator");
        let bytes = generator.to_transcript_bytes().expect("affine point");
        let mut borrowed = [0xa5_u8; 64];
        generator
            .write_transcript_bytes_ref(&mut borrowed)
            .expect("borrowed affine point");
        assert_eq!(borrowed, bytes);
        assert_eq!(bytes[0], 3);
        assert_eq!(&bytes[1..32], &[0; 31]);
        let mut expected_y = CANONICAL_GENERATOR_Y_BE_V1;
        expected_y.reverse();
        assert_eq!(bytes[32..], expected_y);
        assert_eq!(
            VegaT256PointV1::identity().to_transcript_bytes(),
            Err(VegaCurveError::IdentityTranscriptPoint)
        );
        assert_eq!(
            VegaT256PointV1::identity().write_transcript_bytes_ref(&mut borrowed),
            Err(VegaCurveError::IdentityTranscriptPoint)
        );
        let production = include_str!("curve.rs")
            .split_once("#[cfg(test)]\nmod tests")
            .expect("production curve source")
            .0;
        let borrowed_writer = production
            .split_once("pub fn write_transcript_bytes_ref(")
            .expect("borrowed transcript writer")
            .1
            .split_once("/// Return whether this point is the group identity")
            .expect("borrowed transcript writer boundary")
            .0;
        assert!(borrowed_writer.contains("bool::from(self.0.is_identity())"));
        assert!(!borrowed_writer.contains("self.is_identity()"));
    }
    #[test]
    fn generator_derivation_bounds_are_closed() {
        assert_eq!(
            derive_t256_generators_v1(b"", 1),
            Err(VegaCurveError::InvalidGeneratorLabelLength)
        );
        assert_eq!(
            derive_t256_generators_v1(&[0; 256], 1),
            Err(VegaCurveError::InvalidGeneratorLabelLength)
        );
        assert_eq!(
            derive_t256_generators_v1(b"x", 0),
            Err(VegaCurveError::InvalidGeneratorCount)
        );
        assert_eq!(
            derive_t256_generators_v1(b"x", MAX_VEGA_T256_GENERATORS_V1 + 1),
            Err(VegaCurveError::InvalidGeneratorCount)
        );
    }
}
