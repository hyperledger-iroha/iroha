//! Shared P-256 encodings, generator derivation, and Fiat--Shamir transcript.
//!
//! Points use the RFC 9380 `P256_XMD:SHA-256_SSWU_RO_` suite and are encoded as
//! canonical compressed SEC1 values.  Scalars are canonical big-endian
//! integers strictly below the P-256 group order.

use core::fmt;

use p256::{
    AffinePoint, EncodedPoint, FieldBytes, NistP256, ProjectivePoint, Scalar,
    elliptic_curve::{
        Field, Group, PrimeField,
        hash2curve::{ExpandMsgXmd, GroupDigest},
        sec1::{FromEncodedPoint, ToEncodedPoint},
    },
};
use rand_core_06::{CryptoRng, RngCore};
use sha2::{Digest, Sha256};
use thiserror::Error;
use zeroize::{Zeroize, Zeroizing};

use super::prover_randomness::{HealthCheckedCryptoRngV1, ProverRandomnessErrorV1};

/// Maximum chain-id bytes accepted by a privacy transcript.
pub const MAX_TRANSCRIPT_CHAIN_ID_BYTES_V1: usize = 255;
/// Maximum opaque proof bytes accepted before Norito decoding.
pub const MAX_P256_ENGINE_PROOF_BYTES_V1: usize = 8 * 1024 * 1024;

const TRANSCRIPT_VERSION_V1: u8 = 1;
const TRANSCRIPT_DOMAIN_V1: &[u8] = b"iroha.privacy.transcript.v1";
const CHALLENGE_DOMAIN_V1: &[u8] = b"iroha.privacy.challenge.p256.v1";
const GENERATOR_DIGEST_DOMAIN_V1: &[u8] = b"iroha.privacy.generators.p256.v1";
const MAX_REJECTION_ATTEMPTS: u32 = 1 << 16;

/// Failure returned by the shared P-256 privacy substrate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum P256EngineError {
    /// A compressed SEC1 point did not have exactly 33 bytes.
    #[error("compressed P-256 point must contain exactly 33 bytes, got {actual}")]
    InvalidPointLength {
        /// Supplied byte length.
        actual: usize,
    },
    /// A point was not a canonical compressed SEC1 P-256 point.
    #[error("invalid or non-canonical compressed P-256 point")]
    InvalidPointEncoding,
    /// The identity point is not admitted by a wire or parameter profile.
    #[error("P-256 identity point is not admitted")]
    IdentityPoint,
    /// A scalar was not the canonical big-endian encoding of an integer below the group order.
    #[error("invalid or non-canonical P-256 scalar")]
    InvalidScalarEncoding,
    /// A secret or challenge scalar was zero.
    #[error("zero P-256 scalar is not admitted here")]
    ZeroScalar,
    /// A transcript chain identifier was empty or exceeded its fixed bound.
    #[error("transcript chain id length {actual} is outside 1..={max}")]
    InvalidChainIdLength {
        /// Supplied chain-id length.
        actual: usize,
        /// First-release maximum length.
        max: usize,
    },
    /// A mandatory transcript digest consisted entirely of zero bytes.
    #[error("transcript digest `{field}` must be non-zero")]
    ZeroTranscriptDigest {
        /// Stable field name.
        field: &'static str,
    },
    /// A transcript field label or value exceeded its canonical framing limit.
    #[error("transcript field is too large")]
    TranscriptFieldTooLarge,
    /// RFC 9380 expansion rejected a domain separation tag.
    #[error("RFC 9380 P-256 hash-to-curve failed")]
    HashToCurve,
    /// The operating-system or caller-supplied cryptographic RNG failed.
    #[error("P-256 cryptographic random source is unavailable")]
    RandomnessUnavailable,
    /// The random source repeated a catastrophic constant or short-period prefix.
    #[error("P-256 cryptographic random source failed its health check")]
    RandomnessHealthCheckFailed,
    /// Rejection sampling did not produce a canonical non-zero scalar within the fixed work bound.
    #[error("P-256 scalar rejection sampling exhausted its fixed work bound")]
    RejectionSamplingExhausted,
    /// Two independently derived generator roles encoded to the same point or its inverse.
    #[error("derived P-256 generator basis contains a duplicate or inverse relation")]
    GeneratorCollision,
    /// An opaque engine proof exceeded the decoder's hard byte bound.
    #[error("P-256 engine proof length {actual} exceeds hard maximum {max}")]
    ProofTooLarge {
        /// Supplied proof length.
        actual: usize,
        /// Hard maximum proof length.
        max: usize,
    },
    /// Exact Norito proof decoding failed.
    #[error("invalid canonical Norito engine proof")]
    InvalidProofEncoding,
}

/// Canonical compressed SEC1 encoding of a non-identity P-256 point.
#[derive(
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(decode_from_slice)]
pub struct CompressedPointV1 {
    bytes: [u8; 33],
}

impl CompressedPointV1 {
    /// Parse and canonicalize a compressed SEC1 point.
    ///
    /// # Errors
    ///
    /// Returns an error for non-compressed, non-canonical, off-curve, identity,
    /// or incorrectly sized encodings.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, P256EngineError> {
        if bytes.len() != 33 {
            return Err(P256EngineError::InvalidPointLength {
                actual: bytes.len(),
            });
        }
        let mut fixed = [0_u8; 33];
        fixed.copy_from_slice(bytes);
        let point = Self { bytes: fixed };
        let _ = point.to_projective()?;
        Ok(point)
    }

    /// Return the exact canonical SEC1 bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 33] {
        &self.bytes
    }

    pub(crate) fn from_projective(point: ProjectivePoint) -> Result<Self, P256EngineError> {
        if bool::from(point.is_identity()) {
            return Err(P256EngineError::IdentityPoint);
        }
        let encoded = AffinePoint::from(point).to_encoded_point(true);
        let bytes = encoded.as_bytes();
        if bytes.len() != 33 {
            return Err(P256EngineError::InvalidPointEncoding);
        }
        let mut fixed = [0_u8; 33];
        fixed.copy_from_slice(bytes);
        Ok(Self { bytes: fixed })
    }

    pub(crate) fn to_projective(self) -> Result<ProjectivePoint, P256EngineError> {
        if !matches!(self.bytes[0], 0x02 | 0x03) {
            return Err(P256EngineError::InvalidPointEncoding);
        }
        let encoded = EncodedPoint::from_bytes(self.bytes)
            .map_err(|_| P256EngineError::InvalidPointEncoding)?;
        if !encoded.is_compressed() {
            return Err(P256EngineError::InvalidPointEncoding);
        }
        let affine = Option::<AffinePoint>::from(AffinePoint::from_encoded_point(&encoded))
            .ok_or(P256EngineError::InvalidPointEncoding)?;
        let projective = ProjectivePoint::from(affine);
        if bool::from(projective.is_identity()) {
            return Err(P256EngineError::IdentityPoint);
        }
        if affine.to_encoded_point(true).as_bytes() != self.bytes {
            return Err(P256EngineError::InvalidPointEncoding);
        }
        Ok(projective)
    }

    #[cfg(test)]
    pub(crate) const fn from_unchecked_bytes(bytes: [u8; 33]) -> Self {
        Self { bytes }
    }
}

impl fmt::Debug for CompressedPointV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("CompressedPointV1")
            .field(&hex::encode(self.bytes))
            .finish()
    }
}

/// Canonical big-endian encoding of a P-256 scalar.
#[derive(
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(decode_from_slice)]
pub struct CanonicalScalarV1 {
    bytes: [u8; 32],
}

impl CanonicalScalarV1 {
    /// Parse a canonical scalar.  Zero is admitted for public responses.
    ///
    /// # Errors
    ///
    /// Returns an error when the integer is not strictly below the group order.
    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, P256EngineError> {
        let value = Self { bytes };
        let _ = value.to_scalar()?;
        Ok(value)
    }

    /// Return the exact canonical big-endian bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.bytes
    }

    /// Return whether this scalar is zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.bytes == [0; 32]
    }

    pub(crate) fn from_scalar(value: Scalar) -> Self {
        let bytes: [u8; 32] = value.to_repr().into();
        Self { bytes }
    }

    pub(crate) fn to_scalar(self) -> Result<Scalar, P256EngineError> {
        let repr = FieldBytes::from(self.bytes);
        Option::<Scalar>::from(Scalar::from_repr(repr))
            .ok_or(P256EngineError::InvalidScalarEncoding)
    }

    #[cfg(test)]
    pub(crate) const fn from_unchecked_bytes(bytes: [u8; 32]) -> Self {
        Self { bytes }
    }
}

impl fmt::Debug for CanonicalScalarV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("CanonicalScalarV1")
            .field(&hex::encode(self.bytes))
            .finish()
    }
}

/// Zeroizing canonical non-zero scalar used for secret keys and blindings.
pub struct SecretScalarV1 {
    bytes: Zeroizing<[u8; 32]>,
}

impl SecretScalarV1 {
    /// Construct a secret scalar from canonical big-endian bytes.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-canonical or zero value.
    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, P256EngineError> {
        let scalar = scalar_from_bytes(bytes)?;
        if bool::from(scalar.is_zero()) {
            return Err(P256EngineError::ZeroScalar);
        }
        Ok(Self {
            bytes: Zeroizing::new(bytes),
        })
    }

    /// Generate an unbiased non-zero secret scalar by rejection sampling.
    ///
    /// # Errors
    ///
    /// Returns an error only if the entropy source produces no admitted scalar
    /// within the fixed rejection bound.
    pub fn generate<R>(rng: &mut R) -> Result<Self, P256EngineError>
    where
        R: CryptoRng + RngCore,
    {
        let mut checked = health_checked_p256_rng_v1(rng)?;
        let scalar = random_nonzero_scalar(&mut checked)?;
        Ok(Self {
            bytes: Zeroizing::new(scalar.to_repr().into()),
        })
    }

    #[cfg(test)]
    pub(crate) fn canonical_encoding(&self) -> CanonicalScalarV1 {
        CanonicalScalarV1 { bytes: *self.bytes }
    }

    pub(crate) fn expose_scalar(&self) -> Scalar {
        scalar_from_bytes(*self.bytes).expect("secret scalar was validated at construction")
    }
}

impl fmt::Debug for SecretScalarV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretScalarV1([REDACTED])")
    }
}

/// Consensus-relevant binding supplied to a versioned privacy transcript.
#[derive(Clone, Copy, Debug)]
pub struct TranscriptBindingV1<'a> {
    /// Exact chain-id bytes.
    pub chain_id: &'a [u8],
    /// Hash of the exact genesis block or genesis manifest.
    pub genesis_hash: [u8; 32],
    /// Zero-based privacy action index within its transaction.
    pub action_index: u32,
    /// Digest of the full typed public statement.
    pub statement_digest: [u8; 32],
    /// Exact governed parameter-set identifier.
    pub parameter_id: [u8; 32],
    /// Digest of the exact governed parameter set.
    pub parameter_digest: [u8; 32],
    /// Digest of the exact verifier artifact.
    pub verifier_digest: [u8; 32],
    /// Digest of the exact typed statement schema.
    pub statement_schema_digest: [u8; 32],
    /// Digest of the native engine build/manifest admitted by governance.
    pub engine_manifest_digest: [u8; 32],
    /// Digest of the exact independently derived generator basis.
    pub generator_digest: [u8; 32],
}

impl TranscriptBindingV1<'_> {
    /// Validate all mandatory bindings.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty/oversized chain identifier or a zero
    /// mandatory digest.
    pub fn validate(&self) -> Result<(), P256EngineError> {
        if self.chain_id.is_empty() || self.chain_id.len() > MAX_TRANSCRIPT_CHAIN_ID_BYTES_V1 {
            return Err(P256EngineError::InvalidChainIdLength {
                actual: self.chain_id.len(),
                max: MAX_TRANSCRIPT_CHAIN_ID_BYTES_V1,
            });
        }
        for (field, digest) in [
            ("genesis_hash", self.genesis_hash),
            ("statement_digest", self.statement_digest),
            ("parameter_id", self.parameter_id),
            ("parameter_digest", self.parameter_digest),
            ("verifier_digest", self.verifier_digest),
            ("statement_schema_digest", self.statement_schema_digest),
            ("engine_manifest_digest", self.engine_manifest_digest),
            ("generator_digest", self.generator_digest),
        ] {
            if digest == [0; 32] {
                return Err(P256EngineError::ZeroTranscriptDigest { field });
            }
        }
        Ok(())
    }
}

/// SHA-256 Fiat--Shamir transcript with canonical length-delimited fields.
#[derive(Clone)]
pub struct TranscriptV1 {
    hasher: Sha256,
}

impl TranscriptV1 {
    /// Begin a transcript for a closed protocol suite.
    ///
    /// # Errors
    ///
    /// Returns an error when a mandatory binding is invalid.
    pub fn new(
        suite: &'static [u8],
        binding: &TranscriptBindingV1<'_>,
    ) -> Result<Self, P256EngineError> {
        binding.validate()?;
        let mut transcript = Self {
            hasher: Sha256::new(),
        };
        transcript.append_message(b"domain", TRANSCRIPT_DOMAIN_V1)?;
        transcript.append_message(b"transcript_version", &[TRANSCRIPT_VERSION_V1])?;
        transcript.append_message(b"suite", suite)?;
        transcript.append_message(b"chain_id", binding.chain_id)?;
        transcript.append_message(b"genesis_hash", &binding.genesis_hash)?;
        transcript.append_message(b"action_index", &binding.action_index.to_be_bytes())?;
        transcript.append_message(b"statement_digest", &binding.statement_digest)?;
        transcript.append_message(b"parameter_id", &binding.parameter_id)?;
        transcript.append_message(b"parameter_digest", &binding.parameter_digest)?;
        transcript.append_message(b"verifier_digest", &binding.verifier_digest)?;
        transcript.append_message(b"statement_schema_digest", &binding.statement_schema_digest)?;
        transcript.append_message(b"engine_manifest_digest", &binding.engine_manifest_digest)?;
        transcript.append_message(b"generator_digest", &binding.generator_digest)?;
        Ok(transcript)
    }

    /// Append one canonically framed message.
    ///
    /// # Errors
    ///
    /// Returns an error if a label cannot be represented by a `u16` or a value
    /// cannot be represented by a `u32`.
    pub fn append_message(&mut self, label: &[u8], value: &[u8]) -> Result<(), P256EngineError> {
        let label_len =
            u16::try_from(label.len()).map_err(|_| P256EngineError::TranscriptFieldTooLarge)?;
        let value_len =
            u32::try_from(value.len()).map_err(|_| P256EngineError::TranscriptFieldTooLarge)?;
        self.hasher.update(label_len.to_be_bytes());
        self.hasher.update(label);
        self.hasher.update(value_len.to_be_bytes());
        self.hasher.update(value);
        Ok(())
    }

    /// Append a canonical compressed point.
    ///
    /// # Errors
    ///
    /// Returns an error only when the label is too large.
    pub fn append_point(
        &mut self,
        label: &[u8],
        point: &CompressedPointV1,
    ) -> Result<(), P256EngineError> {
        self.append_message(label, point.as_bytes())
    }

    /// Derive one independent non-zero scalar by SHA-256 rejection sampling.
    ///
    /// The accepted scalar is appended to the transcript before this method
    /// returns.  Consequently the next challenge binds the entire
    /// transcript-so-far instead of being a power of a single challenge.
    ///
    /// # Errors
    ///
    /// Returns an error only if rejection sampling exhausts its fixed bound or
    /// the challenge label is too large.
    pub fn challenge_nonzero_scalar(
        &mut self,
        label: &[u8],
        ordinal: u32,
    ) -> Result<CanonicalScalarV1, P256EngineError> {
        if label.len() > usize::from(u16::MAX) {
            return Err(P256EngineError::TranscriptFieldTooLarge);
        }
        let state: [u8; 32] = self.hasher.clone().finalize().into();
        for retry in 0..MAX_REJECTION_ATTEMPTS {
            let mut hash = Sha256::new();
            hash.update(CHALLENGE_DOMAIN_V1);
            hash.update(state);
            hash.update((label.len() as u16).to_be_bytes());
            hash.update(label);
            hash.update(ordinal.to_be_bytes());
            hash.update(retry.to_be_bytes());
            let candidate: [u8; 32] = hash.finalize().into();
            if let Ok(scalar) = scalar_from_bytes(candidate) {
                if !bool::from(scalar.is_zero()) {
                    let canonical = CanonicalScalarV1::from_scalar(scalar);
                    self.append_message(b"challenge_label", label)?;
                    self.append_message(b"challenge_ordinal", &ordinal.to_be_bytes())?;
                    self.append_message(b"challenge_scalar", canonical.as_bytes())?;
                    return Ok(canonical);
                }
            }
        }
        Err(P256EngineError::RejectionSamplingExhausted)
    }

    /// Return the digest of the transcript-so-far without mutating it.
    #[must_use]
    pub fn digest(&self) -> [u8; 32] {
        self.hasher.clone().finalize().into()
    }
}

pub(crate) fn scalar_from_bytes(bytes: [u8; 32]) -> Result<Scalar, P256EngineError> {
    Option::<Scalar>::from(Scalar::from_repr(FieldBytes::from(bytes)))
        .ok_or(P256EngineError::InvalidScalarEncoding)
}

pub(crate) fn health_checked_p256_rng_v1<R>(
    rng: &mut R,
) -> Result<HealthCheckedCryptoRngV1<'_, R>, P256EngineError>
where
    R: CryptoRng + RngCore,
{
    HealthCheckedCryptoRngV1::new(rng).map_err(|error| match error {
        ProverRandomnessErrorV1::Unavailable => P256EngineError::RandomnessUnavailable,
        ProverRandomnessErrorV1::Unhealthy => P256EngineError::RandomnessHealthCheckFailed,
    })
}

pub(crate) fn random_nonzero_scalar<R>(rng: &mut R) -> Result<Scalar, P256EngineError>
where
    R: CryptoRng + RngCore,
{
    for _ in 0..MAX_REJECTION_ATTEMPTS {
        let mut candidate = [0_u8; 32];
        if rng.try_fill_bytes(&mut candidate).is_err() {
            candidate.zeroize();
            return Err(P256EngineError::RandomnessUnavailable);
        }
        let parsed = scalar_from_bytes(candidate);
        candidate.zeroize();
        if let Ok(scalar) = parsed {
            if !bool::from(scalar.is_zero()) {
                return Ok(scalar);
            }
        }
    }
    Err(P256EngineError::RejectionSamplingExhausted)
}

pub(crate) fn hash_to_curve_rfc9380(
    dst: &'static [u8],
    message: &[u8],
) -> Result<ProjectivePoint, P256EngineError> {
    let point = NistP256::hash_from_bytes::<ExpandMsgXmd<Sha256>>(&[message], &[dst])
        .map_err(|_| P256EngineError::HashToCurve)?;
    if bool::from(point.is_identity()) {
        return Err(P256EngineError::IdentityPoint);
    }
    Ok(point)
}

pub(crate) fn generator_digest(
    suite: &[u8],
    points: &[CompressedPointV1],
) -> Result<[u8; 32], P256EngineError> {
    validate_generator_independence(points)?;
    let mut hash = Sha256::new();
    hash.update(GENERATOR_DIGEST_DOMAIN_V1);
    hash.update(
        u16::try_from(suite.len())
            .map_err(|_| P256EngineError::TranscriptFieldTooLarge)?
            .to_be_bytes(),
    );
    hash.update(suite);
    hash.update(
        u16::try_from(points.len())
            .map_err(|_| P256EngineError::TranscriptFieldTooLarge)?
            .to_be_bytes(),
    );
    for point in points {
        hash.update(point.as_bytes());
    }
    Ok(hash.finalize().into())
}

pub(crate) fn validate_generator_independence(
    points: &[CompressedPointV1],
) -> Result<(), P256EngineError> {
    let decoded = points
        .iter()
        .copied()
        .map(CompressedPointV1::to_projective)
        .collect::<Result<Vec<_>, _>>()?;
    for left in 0..decoded.len() {
        for right in 0..left {
            if decoded[left] == decoded[right] || decoded[left] == -decoded[right] {
                return Err(P256EngineError::GeneratorCollision);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use p256::elliptic_curve::sec1::ToEncodedPoint as _;

    use super::*;

    struct FailingRng;

    impl RngCore for FailingRng {
        fn next_u32(&mut self) -> u32 {
            panic!("P-256 privacy engines must use the fallible RNG interface")
        }

        fn next_u64(&mut self) -> u64 {
            panic!("P-256 privacy engines must use the fallible RNG interface")
        }

        fn fill_bytes(&mut self, _destination: &mut [u8]) {
            panic!("P-256 privacy engines must use the fallible RNG interface")
        }

        fn try_fill_bytes(&mut self, _destination: &mut [u8]) -> Result<(), rand_core_06::Error> {
            Err(rand_core_06::Error::new("injected P-256 RNG failure"))
        }
    }

    impl CryptoRng for FailingRng {}

    struct PeriodicRng;

    impl RngCore for PeriodicRng {
        fn next_u32(&mut self) -> u32 {
            panic!("P-256 privacy engines must use the fallible RNG interface")
        }

        fn next_u64(&mut self) -> u64 {
            panic!("P-256 privacy engines must use the fallible RNG interface")
        }

        fn fill_bytes(&mut self, _destination: &mut [u8]) {
            panic!("P-256 privacy engines must use the fallible RNG interface")
        }

        fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), rand_core_06::Error> {
            for (index, byte) in destination.iter_mut().enumerate() {
                *byte = ((index % 16) as u8).wrapping_mul(17).wrapping_add(5);
            }
            Ok(())
        }
    }

    impl CryptoRng for PeriodicRng {}

    fn binding() -> TranscriptBindingV1<'static> {
        TranscriptBindingV1 {
            chain_id: b"taira-test",
            genesis_hash: [1; 32],
            action_index: 7,
            statement_digest: [2; 32],
            parameter_id: [3; 32],
            parameter_digest: [3; 32],
            verifier_digest: [5; 32],
            statement_schema_digest: [6; 32],
            engine_manifest_digest: [7; 32],
            generator_digest: [4; 32],
        }
    }

    #[test]
    fn rfc9380_p256_vector_matches() {
        const DST: &[u8] = b"QUUX-V01-CS02-with-P256_XMD:SHA-256_SSWU_RO_";
        let point = hash_to_curve_rfc9380(DST, b"abc").expect("RFC 9380 point");
        let encoded = AffinePoint::from(point).to_encoded_point(false);
        assert_eq!(
            hex::encode(encoded.as_bytes()),
            concat!(
                "040bb8b87485551aa43ed54f009230450b492fead5f1cc91658775dac4a3388a0f",
                "5c41b3d0731a27a7b14bc0bf0ccded2d8751f83493404c84a88e71ffd424212e"
            )
        );
    }

    #[test]
    fn strict_point_codec_rejects_malformed_forms() {
        let point = hash_to_curve_rfc9380(b"IROHA-TEST-P256_XMD:SHA-256_SSWU_RO_", b"canonical")
            .expect("point");
        let canonical = CompressedPointV1::from_projective(point).expect("compressed point");
        assert_eq!(
            CompressedPointV1::from_slice(canonical.as_bytes()).expect("roundtrip"),
            canonical
        );

        assert!(matches!(
            CompressedPointV1::from_slice(&canonical.as_bytes()[..32]),
            Err(P256EngineError::InvalidPointLength { actual: 32 })
        ));
        let mut uncompressed_prefix = *canonical.as_bytes();
        uncompressed_prefix[0] = 0x04;
        assert!(matches!(
            CompressedPointV1::from_slice(&uncompressed_prefix),
            Err(P256EngineError::InvalidPointEncoding)
        ));
        let all_zero = [0_u8; 33];
        assert!(CompressedPointV1::from_slice(&all_zero).is_err());
        let mut off_curve = [0xff_u8; 33];
        off_curve[0] = 0x02;
        assert!(CompressedPointV1::from_slice(&off_curve).is_err());
    }

    #[test]
    fn scalar_codec_rejects_order_and_accepts_boundaries() {
        let zero = CanonicalScalarV1::from_bytes([0; 32]).expect("zero is a public scalar");
        assert!(zero.is_zero());
        let one = {
            let mut bytes = [0_u8; 32];
            bytes[31] = 1;
            CanonicalScalarV1::from_bytes(bytes).expect("one")
        };
        assert!(!one.is_zero());
        let order = hex::decode("ffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551")
            .expect("order");
        let mut order_bytes = [0_u8; 32];
        order_bytes.copy_from_slice(&order);
        assert!(matches!(
            CanonicalScalarV1::from_bytes(order_bytes),
            Err(P256EngineError::InvalidScalarEncoding)
        ));
        assert!(matches!(
            SecretScalarV1::from_bytes([0; 32]),
            Err(P256EngineError::ZeroScalar)
        ));
        assert!(matches!(
            SecretScalarV1::generate(&mut FailingRng),
            Err(P256EngineError::RandomnessUnavailable)
        ));
        assert!(matches!(
            SecretScalarV1::generate(&mut PeriodicRng),
            Err(P256EngineError::RandomnessHealthCheckFailed)
        ));
    }

    #[test]
    fn transcript_binds_every_context_field_and_prior_state() {
        let mut first = TranscriptV1::new(b"suite", &binding()).expect("transcript");
        first.append_message(b"commitment", b"A").expect("append");
        let c0 = first
            .challenge_nonzero_scalar(b"epsilon", 0)
            .expect("challenge");
        let c1 = first
            .challenge_nonzero_scalar(b"epsilon", 1)
            .expect("challenge");
        assert_ne!(c0, c1);
        assert!(!c0.is_zero());
        assert!(!c1.is_zero());

        let mut changed_fields = Vec::new();
        let mut changed = binding();
        changed.chain_id = b"other";
        changed_fields.push(changed);
        let mut changed = binding();
        changed.genesis_hash[0] ^= 1;
        changed_fields.push(changed);
        let mut changed = binding();
        changed.action_index += 1;
        changed_fields.push(changed);
        let mut changed = binding();
        changed.statement_digest[0] ^= 1;
        changed_fields.push(changed);
        let mut changed = binding();
        changed.parameter_id[0] ^= 1;
        changed_fields.push(changed);
        let mut changed = binding();
        changed.parameter_digest[0] ^= 1;
        changed_fields.push(changed);
        let mut changed = binding();
        changed.verifier_digest[0] ^= 1;
        changed_fields.push(changed);
        let mut changed = binding();
        changed.statement_schema_digest[0] ^= 1;
        changed_fields.push(changed);
        let mut changed = binding();
        changed.engine_manifest_digest[0] ^= 1;
        changed_fields.push(changed);
        let mut changed = binding();
        changed.generator_digest[0] ^= 1;
        changed_fields.push(changed);

        for changed in changed_fields {
            let mut transcript = TranscriptV1::new(b"suite", &changed).expect("changed transcript");
            transcript
                .append_message(b"commitment", b"A")
                .expect("append");
            assert_ne!(
                transcript
                    .challenge_nonzero_scalar(b"epsilon", 0)
                    .expect("challenge"),
                c0
            );
        }

        let mut prior_changed = TranscriptV1::new(b"suite", &binding()).expect("transcript");
        prior_changed
            .append_message(b"commitment", b"B")
            .expect("append");
        assert_ne!(
            prior_changed
                .challenge_nonzero_scalar(b"epsilon", 0)
                .expect("challenge"),
            c0
        );

        let mut suite_changed =
            TranscriptV1::new(b"different-suite", &binding()).expect("transcript");
        suite_changed
            .append_message(b"commitment", b"A")
            .expect("append");
        assert_ne!(
            suite_changed
                .challenge_nonzero_scalar(b"epsilon", 0)
                .expect("challenge"),
            c0
        );
    }

    #[test]
    fn repeated_challenges_are_nonzero_and_transcript_separated() {
        let mut transcript = TranscriptV1::new(b"suite", &binding()).expect("transcript");
        transcript
            .append_message(b"commitment", b"fixed")
            .expect("append");
        let mut challenges = Vec::new();
        for ordinal in 0..128 {
            let challenge = transcript
                .challenge_nonzero_scalar(b"epsilon", ordinal)
                .expect("challenge");
            assert!(!challenge.is_zero());
            assert!(
                !challenges.contains(&challenge),
                "challenge repeated at ordinal {ordinal}"
            );
            challenges.push(challenge);
        }
    }

    #[test]
    fn transcript_rejects_missing_bindings() {
        let mut empty_chain = binding();
        empty_chain.chain_id = b"";
        assert!(matches!(
            empty_chain.validate(),
            Err(P256EngineError::InvalidChainIdLength { actual: 0, .. })
        ));

        let mut zero_statement = binding();
        zero_statement.statement_digest = [0; 32];
        assert!(matches!(
            zero_statement.validate(),
            Err(P256EngineError::ZeroTranscriptDigest {
                field: "statement_digest"
            })
        ));

        let overlong_chain = vec![b'x'; MAX_TRANSCRIPT_CHAIN_ID_BYTES_V1 + 1];
        let mut overlong = binding();
        overlong.chain_id = &overlong_chain;
        assert!(matches!(
            overlong.validate(),
            Err(P256EngineError::InvalidChainIdLength { .. })
        ));

        let zeroing_mutations: [(&str, fn(&mut TranscriptBindingV1<'_>)); 8] = [
            ("genesis_hash", |binding: &mut TranscriptBindingV1<'_>| {
                binding.genesis_hash = [0; 32]
            }),
            ("statement_digest", |binding| {
                binding.statement_digest = [0; 32]
            }),
            ("parameter_id", |binding| binding.parameter_id = [0; 32]),
            ("parameter_digest", |binding| {
                binding.parameter_digest = [0; 32]
            }),
            ("verifier_digest", |binding| {
                binding.verifier_digest = [0; 32]
            }),
            ("statement_schema_digest", |binding| {
                binding.statement_schema_digest = [0; 32]
            }),
            ("engine_manifest_digest", |binding| {
                binding.engine_manifest_digest = [0; 32]
            }),
            ("generator_digest", |binding| {
                binding.generator_digest = [0; 32]
            }),
        ];
        for (field, mutate) in zeroing_mutations {
            let mut zeroed = binding();
            mutate(&mut zeroed);
            assert!(matches!(
                zeroed.validate(),
                Err(P256EngineError::ZeroTranscriptDigest {
                    field: actual_field
                }) if actual_field == field
            ));
        }
    }
}
