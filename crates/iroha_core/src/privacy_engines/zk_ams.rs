//! Native Iroha testnet instantiation of ZK-AMS anonymous provisioning.
//!
//! The protocol workflow follows ZK-AMS v2, arXiv:2602.16130, Algorithms
//! 1--4 and Appendices A/C.  The paper intentionally leaves the concrete
//! linkable ring-signature group, hash, transcript, and wire unspecified.
//! This module closes Phase V to a single-row MLSAGS instance (equivalently
//! LSAG) over prime-order Ristretto255 with SHA3-512.  It is an Iroha
//! experimental profile and is not wire-compatible with the paper prototype.
//!
//! Batch admission remains fail-closed until the sibling setup-free,
//! zero-knowledge relaxed-R1CS finalizer is complete.  Keeping the complete
//! provisioning primitive here allows its algebra and wire to be tested
//! independently without making the protocol activatable.

use curve25519_dalek::{
    RistrettoPoint,
    constants::RISTRETTO_BASEPOINT_POINT,
    ristretto::CompressedRistretto,
    scalar::Scalar,
    traits::Identity,
};
use rand_core_06::{CryptoRng, RngCore};
use sha3::{Digest, Sha3_256, Sha3_512};
use thiserror::Error;
use zeroize::{Zeroize, Zeroizing};

use super::p256::{P256EngineError, TranscriptBindingV1};

/// Pinned source used for the Iroha ZK-AMS workflow and relation.
pub const ZK_AMS_SOURCE_PROFILE_V1: &[u8] =
    b"arxiv:2602.16130v2:algorithms-1-4:appendices-a-c";
/// Exact Iroha Phase-V suite label.
pub const ZK_AMS_LSAG_SUITE_V1: &[u8] =
    b"iroha-zk-ams-v1:phase-v:lsag-ristretto255-sha3-512";
/// Hash-to-Ristretto domain for admitted seed public keys.
pub const ZK_AMS_HASH_TO_POINT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.lsag.hash-to-ristretto";
/// Canonical proof wire version.
pub const ZK_AMS_LSAG_PROOF_VERSION_V1: u8 = 1;
/// Smallest closed Phase-V ring.
pub const ZK_AMS_MIN_RING_SIZE_V1: usize = 16;
/// Largest closed Phase-V ring.
pub const ZK_AMS_MAX_RING_SIZE_V1: usize = 64;
/// Exact admitted ring sizes.
pub const ZK_AMS_RING_SIZES_V1: [usize; 3] = [16, 32, 64];
/// Hard cap checked before Norito proof decoding.
pub const MAX_ZK_AMS_LSAG_PROOF_BYTES_V1: usize = 4 * 1024;

const RANDOM_REJECTION_ATTEMPTS: u32 = 1 << 16;
const TRANSCRIPT_VERSION_V1: u8 = 1;
const GENERATOR_DIGEST_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.generator-digest";

/// Failure while constructing, decoding, signing, or verifying ZK-AMS Phase V.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum ZkAmsErrorV1 {
    /// A shared consensus transcript field is invalid.
    #[error("invalid ZK-AMS consensus transcript binding")]
    InvalidBinding,
    /// A transcript label or value cannot be represented canonically.
    #[error("ZK-AMS transcript field is too large")]
    TranscriptFieldTooLarge,
    /// A seed public key or key image is malformed, non-canonical, or identity.
    #[error("invalid canonical nonidentity Ristretto255 point")]
    InvalidPoint,
    /// A secret or proof scalar is not canonical.
    #[error("invalid canonical Ristretto255 scalar")]
    InvalidScalar,
    /// A secret seed scalar is zero.
    #[error("ZK-AMS seed secret must be nonzero")]
    ZeroSecret,
    /// The ring is not one of the closed first-release sizes.
    #[error("ZK-AMS ring size {actual} is not one of 16, 32, or 64")]
    InvalidRingSize {
        /// Supplied number of ring members.
        actual: usize,
    },
    /// The ring is not strictly increasing in canonical byte order.
    #[error("ZK-AMS ring must be strictly increasing and duplicate-free")]
    NonCanonicalRing,
    /// The signer index is outside the supplied ring.
    #[error("ZK-AMS signer index {index} is outside ring size {ring_size}")]
    SignerIndexOutOfBounds {
        /// Supplied signer index.
        index: usize,
        /// Supplied ring size.
        ring_size: usize,
    },
    /// The secret key does not derive the selected public ring member.
    #[error("ZK-AMS seed secret does not match the selected ring member")]
    SignerPublicKeyMismatch,
    /// The supplied key image does not derive from the selected seed secret.
    #[error("ZK-AMS key image does not match the selected seed secret")]
    KeyImageMismatch,
    /// The random source failed to yield a canonical nonzero scalar.
    #[error("ZK-AMS random scalar rejection sampling exhausted its work bound")]
    RandomnessExhausted,
    /// Proof bytes exceed the dedicated decoder cap.
    #[error("ZK-AMS LSAG proof length {actual} exceeds hard maximum {max}")]
    ProofTooLarge {
        /// Supplied proof bytes.
        actual: usize,
        /// Hard maximum.
        max: usize,
    },
    /// Exact Norito decode, shape validation, or canonical re-encoding failed.
    #[error("invalid canonical ZK-AMS LSAG proof encoding")]
    InvalidProofEncoding,
    /// The closed LSAG verification equation failed.
    #[error("ZK-AMS LSAG verification failed")]
    VerificationFailed,
}

impl From<P256EngineError> for ZkAmsErrorV1 {
    fn from(_: P256EngineError) -> Self {
        Self::InvalidBinding
    }
}

/// Zeroizing canonical little-endian Ristretto scalar used as a seed secret.
pub struct ZkAmsSeedSecretV1 {
    bytes: Zeroizing<[u8; 32]>,
}

impl ZkAmsSeedSecretV1 {
    /// Parse one canonical nonzero seed secret.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-canonical or zero scalar.
    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, ZkAmsErrorV1> {
        let scalar = scalar_from_canonical(bytes)?;
        if scalar == Scalar::ZERO {
            return Err(ZkAmsErrorV1::ZeroSecret);
        }
        Ok(Self {
            bytes: Zeroizing::new(bytes),
        })
    }

    /// Sample one unbiased canonical nonzero scalar.
    ///
    /// # Errors
    ///
    /// Returns an error when the random source does not produce an admitted
    /// canonical scalar within the fixed work bound.
    pub fn generate<R: CryptoRng + RngCore>(rng: &mut R) -> Result<Self, ZkAmsErrorV1> {
        let scalar = random_nonzero_scalar(rng)?;
        let mut bytes = scalar.to_bytes();
        let secret = Self::from_bytes(bytes);
        bytes.zeroize();
        secret
    }

    fn expose_scalar(&self) -> Scalar {
        scalar_from_canonical(*self.bytes)
            .expect("ZK-AMS seed secret was validated at construction")
    }
}

impl core::fmt::Debug for ZkAmsSeedSecretV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("ZkAmsSeedSecretV1([REDACTED])")
    }
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(decode_from_slice)]
struct ZkAmsLsagProofWireV1 {
    version: u8,
    initial_challenge: [u8; 32],
    responses: Vec<[u8; 32]>,
}

/// Derive the canonical admitted seed public key.
#[must_use]
pub fn zk_ams_seed_public_key_v1(secret: &ZkAmsSeedSecretV1) -> [u8; 32] {
    (secret.expose_scalar() * RISTRETTO_BASEPOINT_POINT)
        .compress()
        .to_bytes()
}

/// Derive the deterministic Phase-V key image used as a replay nullifier.
///
/// # Errors
///
/// Returns an error only if the derived point is the identity, a
/// cryptographically negligible event that is nevertheless rejected.
pub fn zk_ams_key_image_v1(
    secret: &ZkAmsSeedSecretV1,
) -> Result<[u8; 32], ZkAmsErrorV1> {
    let public = zk_ams_seed_public_key_v1(secret);
    let hash_point = hash_public_key_to_point(&public)?;
    let key_image = secret.expose_scalar() * hash_point;
    if key_image == RistrettoPoint::identity() {
        return Err(ZkAmsErrorV1::InvalidPoint);
    }
    Ok(key_image.compress().to_bytes())
}

/// Return the digest of the exact Ristretto generator and hash-to-point suite.
#[must_use]
pub fn zk_ams_generator_digest_v1() -> [u8; 32] {
    let mut hash = Sha3_256::new();
    hash.update(GENERATOR_DIGEST_DOMAIN_V1);
    hash.update(ZK_AMS_LSAG_SUITE_V1);
    hash.update(RISTRETTO_BASEPOINT_POINT.compress().as_bytes());
    hash.update(ZK_AMS_HASH_TO_POINT_DOMAIN_V1);
    hash.finalize().into()
}

/// Sign one account-provisioning statement with the selected seed secret.
///
/// `binding.statement_digest` is the digest of the complete typed ZK-AMS
/// statement, including account id, ordered ring, root/epoch, and key image.
///
/// # Errors
///
/// Fails closed for a malformed ring or key image, a mismatched secret, an
/// invalid consensus binding, or random-source exhaustion.
pub fn sign_zk_ams_provision_v1<R: CryptoRng + RngCore>(
    binding: &TranscriptBindingV1<'_>,
    ring: &[[u8; 32]],
    key_image_bytes: [u8; 32],
    signer_index: usize,
    secret: &ZkAmsSeedSecretV1,
    rng: &mut R,
) -> Result<Vec<u8>, ZkAmsErrorV1> {
    binding.validate()?;
    let ring_points = validate_ring(ring)?;
    let ring_size = ring.len();
    if signer_index >= ring_size {
        return Err(ZkAmsErrorV1::SignerIndexOutOfBounds {
            index: signer_index,
            ring_size,
        });
    }
    let secret_scalar = secret.expose_scalar();
    if secret_scalar * RISTRETTO_BASEPOINT_POINT != ring_points[signer_index] {
        return Err(ZkAmsErrorV1::SignerPublicKeyMismatch);
    }
    let key_image = decode_nonidentity_point(key_image_bytes)?;
    let expected_image = secret_scalar * hash_public_key_to_point(&ring[signer_index])?;
    if key_image != expected_image {
        return Err(ZkAmsErrorV1::KeyImageMismatch);
    }

    let transcript = LsagTranscriptV1::new(binding, ring, key_image_bytes)?;
    let mut alpha = random_nonzero_scalar(rng)?;
    let mut responses = vec![Scalar::ZERO; ring_size];
    for (index, response) in responses.iter_mut().enumerate() {
        if index != signer_index {
            *response = random_nonzero_scalar(rng)?;
        }
    }
    let mut challenges = vec![Scalar::ZERO; ring_size];
    let next = (signer_index + 1) % ring_size;
    let signer_hash_point = hash_public_key_to_point(&ring[signer_index])?;
    challenges[next] = transcript.challenge(
        signer_index,
        alpha * RISTRETTO_BASEPOINT_POINT,
        alpha * signer_hash_point,
    )?;
    let mut index = next;
    while index != signer_index {
        let hash_point = hash_public_key_to_point(&ring[index])?;
        let left = responses[index] * RISTRETTO_BASEPOINT_POINT
            + challenges[index] * ring_points[index];
        let right = responses[index] * hash_point + challenges[index] * key_image;
        challenges[(index + 1) % ring_size] = transcript.challenge(index, left, right)?;
        index = (index + 1) % ring_size;
    }
    responses[signer_index] = alpha - challenges[signer_index] * secret_scalar;
    alpha.zeroize();

    let proof = ZkAmsLsagProofWireV1 {
        version: ZK_AMS_LSAG_PROOF_VERSION_V1,
        initial_challenge: challenges[0].to_bytes(),
        responses: responses.iter().map(Scalar::to_bytes).collect(),
    };
    responses.zeroize();
    challenges.zeroize();
    let encoded = norito::codec::encode_adaptive(&proof);
    if encoded.len() > MAX_ZK_AMS_LSAG_PROOF_BYTES_V1 {
        return Err(ZkAmsErrorV1::ProofTooLarge {
            actual: encoded.len(),
            max: MAX_ZK_AMS_LSAG_PROOF_BYTES_V1,
        });
    }
    Ok(encoded)
}

/// Verify one canonical Phase-V LSAG proof.
///
/// # Errors
///
/// Fails closed before allocation for oversized proof bytes, then rejects
/// non-canonical Norito, scalars, points, ring order, or verification
/// equations.
pub fn verify_zk_ams_provision_v1(
    binding: &TranscriptBindingV1<'_>,
    ring: &[[u8; 32]],
    key_image_bytes: [u8; 32],
    proof_bytes: &[u8],
) -> Result<(), ZkAmsErrorV1> {
    binding.validate()?;
    if proof_bytes.len() > MAX_ZK_AMS_LSAG_PROOF_BYTES_V1 {
        return Err(ZkAmsErrorV1::ProofTooLarge {
            actual: proof_bytes.len(),
            max: MAX_ZK_AMS_LSAG_PROOF_BYTES_V1,
        });
    }
    let ring_points = validate_ring(ring)?;
    let key_image = decode_nonidentity_point(key_image_bytes)?;
    let proof = norito::codec::decode_exact_from_slice::<ZkAmsLsagProofWireV1>(proof_bytes)
        .map_err(|_| ZkAmsErrorV1::InvalidProofEncoding)?;
    if proof.version != ZK_AMS_LSAG_PROOF_VERSION_V1
        || proof.responses.len() != ring.len()
        || norito::codec::encode_adaptive(&proof) != proof_bytes
    {
        return Err(ZkAmsErrorV1::InvalidProofEncoding);
    }
    let mut challenge = scalar_from_canonical(proof.initial_challenge)?;
    let responses = proof
        .responses
        .into_iter()
        .map(scalar_from_canonical)
        .collect::<Result<Vec<_>, _>>()?;
    let transcript = LsagTranscriptV1::new(binding, ring, key_image_bytes)?;
    for (index, ((public_key, response), public_bytes)) in ring_points
        .iter()
        .copied()
        .zip(responses.iter().copied())
        .zip(ring.iter())
        .enumerate()
    {
        let hash_point = hash_public_key_to_point(public_bytes)?;
        let left = response * RISTRETTO_BASEPOINT_POINT + challenge * public_key;
        let right = response * hash_point + challenge * key_image;
        challenge = transcript.challenge(index, left, right)?;
    }
    if challenge.to_bytes() != proof.initial_challenge {
        return Err(ZkAmsErrorV1::VerificationFailed);
    }
    Ok(())
}

fn validate_ring(ring: &[[u8; 32]]) -> Result<Vec<RistrettoPoint>, ZkAmsErrorV1> {
    if !ZK_AMS_RING_SIZES_V1.contains(&ring.len()) {
        return Err(ZkAmsErrorV1::InvalidRingSize { actual: ring.len() });
    }
    if ring.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ZkAmsErrorV1::NonCanonicalRing);
    }
    ring.iter()
        .copied()
        .map(decode_nonidentity_point)
        .collect()
}

fn decode_nonidentity_point(bytes: [u8; 32]) -> Result<RistrettoPoint, ZkAmsErrorV1> {
    let point = CompressedRistretto(bytes)
        .decompress()
        .ok_or(ZkAmsErrorV1::InvalidPoint)?;
    if point == RistrettoPoint::identity() || point.compress().to_bytes() != bytes {
        return Err(ZkAmsErrorV1::InvalidPoint);
    }
    Ok(point)
}

fn hash_public_key_to_point(bytes: &[u8; 32]) -> Result<RistrettoPoint, ZkAmsErrorV1> {
    let mut hash = Sha3_512::new();
    hash.update(ZK_AMS_HASH_TO_POINT_DOMAIN_V1);
    hash.update(
        u16::try_from(ZK_AMS_LSAG_SUITE_V1.len())
            .map_err(|_| ZkAmsErrorV1::TranscriptFieldTooLarge)?
            .to_be_bytes(),
    );
    hash.update(ZK_AMS_LSAG_SUITE_V1);
    hash.update(bytes);
    let uniform: [u8; 64] = hash.finalize().into();
    let point = RistrettoPoint::from_uniform_bytes(&uniform);
    if point == RistrettoPoint::identity() {
        return Err(ZkAmsErrorV1::InvalidPoint);
    }
    Ok(point)
}

fn scalar_from_canonical(bytes: [u8; 32]) -> Result<Scalar, ZkAmsErrorV1> {
    Option::<Scalar>::from(Scalar::from_canonical_bytes(bytes))
        .ok_or(ZkAmsErrorV1::InvalidScalar)
}

fn random_nonzero_scalar<R: CryptoRng + RngCore>(
    rng: &mut R,
) -> Result<Scalar, ZkAmsErrorV1> {
    for _ in 0..RANDOM_REJECTION_ATTEMPTS {
        let mut candidate = [0_u8; 32];
        rng.fill_bytes(&mut candidate);
        let parsed = scalar_from_canonical(candidate);
        candidate.zeroize();
        if let Ok(scalar) = parsed {
            if scalar != Scalar::ZERO {
                return Ok(scalar);
            }
        }
    }
    Err(ZkAmsErrorV1::RandomnessExhausted)
}

#[derive(Clone)]
struct LsagTranscriptV1 {
    prefix: Sha3_512,
}

impl LsagTranscriptV1 {
    fn new(
        binding: &TranscriptBindingV1<'_>,
        ring: &[[u8; 32]],
        key_image: [u8; 32],
    ) -> Result<Self, ZkAmsErrorV1> {
        binding.validate()?;
        let mut prefix = Sha3_512::new();
        append_field(&mut prefix, b"domain", ZK_AMS_LSAG_SUITE_V1)?;
        append_field(
            &mut prefix,
            b"transcript_version",
            &[TRANSCRIPT_VERSION_V1],
        )?;
        append_field(&mut prefix, b"chain_id", binding.chain_id)?;
        append_field(&mut prefix, b"genesis_hash", &binding.genesis_hash)?;
        append_field(
            &mut prefix,
            b"action_index",
            &binding.action_index.to_be_bytes(),
        )?;
        append_field(
            &mut prefix,
            b"statement_digest",
            &binding.statement_digest,
        )?;
        append_field(&mut prefix, b"parameter_id", &binding.parameter_id)?;
        append_field(
            &mut prefix,
            b"parameter_digest",
            &binding.parameter_digest,
        )?;
        append_field(&mut prefix, b"verifier_digest", &binding.verifier_digest)?;
        append_field(
            &mut prefix,
            b"statement_schema_digest",
            &binding.statement_schema_digest,
        )?;
        append_field(
            &mut prefix,
            b"engine_manifest_digest",
            &binding.engine_manifest_digest,
        )?;
        append_field(
            &mut prefix,
            b"generator_digest",
            &binding.generator_digest,
        )?;
        append_field(
            &mut prefix,
            b"ring_count",
            &u32::try_from(ring.len())
                .map_err(|_| ZkAmsErrorV1::TranscriptFieldTooLarge)?
                .to_be_bytes(),
        )?;
        for (index, public_key) in ring.iter().enumerate() {
            append_indexed_field(&mut prefix, b"ring_public_key", index, public_key)?;
        }
        append_field(&mut prefix, b"key_image", &key_image)?;
        Ok(Self { prefix })
    }

    fn challenge(
        &self,
        index: usize,
        left: RistrettoPoint,
        right: RistrettoPoint,
    ) -> Result<Scalar, ZkAmsErrorV1> {
        if left == RistrettoPoint::identity() || right == RistrettoPoint::identity() {
            return Err(ZkAmsErrorV1::VerificationFailed);
        }
        let mut hash = self.prefix.clone();
        append_field(
            &mut hash,
            b"ring_index",
            &u32::try_from(index)
                .map_err(|_| ZkAmsErrorV1::TranscriptFieldTooLarge)?
                .to_be_bytes(),
        )?;
        append_field(&mut hash, b"left", left.compress().as_bytes())?;
        append_field(&mut hash, b"right", right.compress().as_bytes())?;
        let wide: [u8; 64] = hash.finalize().into();
        Ok(Scalar::from_bytes_mod_order_wide(&wide))
    }
}

fn append_indexed_field(
    hash: &mut Sha3_512,
    label: &[u8],
    index: usize,
    value: &[u8],
) -> Result<(), ZkAmsErrorV1> {
    let index =
        u32::try_from(index).map_err(|_| ZkAmsErrorV1::TranscriptFieldTooLarge)?;
    let mut indexed_label = Vec::with_capacity(label.len() + 4);
    indexed_label.extend_from_slice(label);
    indexed_label.extend_from_slice(&index.to_be_bytes());
    append_field(hash, &indexed_label, value)
}

fn append_field(
    hash: &mut Sha3_512,
    label: &[u8],
    value: &[u8],
) -> Result<(), ZkAmsErrorV1> {
    let label_len =
        u16::try_from(label.len()).map_err(|_| ZkAmsErrorV1::TranscriptFieldTooLarge)?;
    let value_len =
        u32::try_from(value.len()).map_err(|_| ZkAmsErrorV1::TranscriptFieldTooLarge)?;
    hash.update(label_len.to_be_bytes());
    hash.update(label);
    hash.update(value_len.to_be_bytes());
    hash.update(value);
    Ok(())
}

