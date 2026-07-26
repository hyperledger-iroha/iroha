//! Source-justified Anonymous-PGC cryptographic building blocks.
//!
//! This module implements the Twisted-ElGamal construction in Definition 6.1
//! of ePrint 2025/884 and complete Schnorr representation proofs for public-key
//! possession and ciphertext opening.  It deliberately does **not** claim to
//! implement the paper's k-out-of-n proof composition; the runtime must not
//! advertise the full Anonymous-PGC engine until that separate relation is
//! present.

use std::collections::BTreeMap;

use once_cell::sync::Lazy;
use p256::{elliptic_curve::Group, ProjectivePoint, Scalar};
use rand_core_06::{CryptoRng, RngCore};
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::p256::{
    generator_digest, hash_to_curve_rfc9380, random_nonzero_scalar,
    validate_generator_independence, CanonicalScalarV1, CompressedPointV1, P256EngineError,
    SecretScalarV1, TranscriptBindingV1, TranscriptV1,
};

/// Closed identifier for Twisted-ElGamal public-key possession transcripts.
pub const PGC_KEY_POSSESSION_SUITE_V1: &[u8] = b"iroha.anonymous-pgc.key-possession.p256.sha256.v1";
/// Closed identifier for Twisted-ElGamal opening transcripts.
pub const PGC_CIPHERTEXT_OPENING_SUITE_V1: &[u8] =
    b"iroha.anonymous-pgc.ciphertext-opening.p256.sha256.v1";
/// Version of every proof in this module.
pub const PGC_BUILDING_BLOCK_PROOF_VERSION_V1: u8 = 1;
/// Exact bit width of the paper's first-release plaintext domain.
pub const PGC_MESSAGE_BITS_V1: u16 = 32;
/// Inclusive lower endpoint of the paper's nonnegative plaintext domain.
pub const PGC_MESSAGE_MIN_V1: u32 = 0;
/// Inclusive upper endpoint `2^32 - 1` of the paper's plaintext domain.
pub const PGC_MESSAGE_MAX_V1: u32 = u32::MAX;
/// Fail-closed capability marker for the complete Anonymous-PGC relation.
///
/// TODO(privacy-anonymous-pgc-k-out-of-n): implement and adversarially test the
/// paper's complete k-out-of-n legality composition before changing this gate.
/// Compiled Anonymous-PGC activation must remain unavailable while it is false.
pub const ANONYMOUS_PGC_FULL_ENGINE_AVAILABLE_V1: bool = false;
/// Tight maximum encoded length of a Twisted-ElGamal ciphertext.
pub const MAX_PGC_CIPHERTEXT_BYTES_V1: usize = 256;
/// Tight maximum encoded length of either Schnorr building-block proof.
pub const MAX_PGC_BUILDING_BLOCK_PROOF_BYTES_V1: usize = 1024;
/// Fixed baby-step table bound for wallet-side 32-bit decryption.
pub const PGC_DECRYPTION_BABY_STEP_BOUND_V1: usize = 1 << 16;
/// Fixed giant-step search bound for wallet-side 32-bit decryption.
pub const PGC_DECRYPTION_GIANT_STEP_BOUND_V1: usize = 1 << 16;

const PGC_G_DST_V1: &[u8] = b"IROHA-ANON-PGC-V1-G-P256_XMD:SHA-256_SSWU_RO_";
const PGC_H_DST_V1: &[u8] = b"IROHA-ANON-PGC-V1-H-P256_XMD:SHA-256_SSWU_RO_";
const PARAMETER_DIGEST_DOMAIN_V1: &[u8] = b"iroha.anonymous-pgc.parameters.v1";
const SOURCE_PROFILE_V1: &[u8] = b"eprint:2025/884:def-6.1:twisted-elgamal";
const MAX_PROVER_RESTARTS: usize = 128;

/// Transparent P-256 parameters for the Anonymous-PGC building blocks.
#[derive(Clone, Copy)]
pub struct AnonymousPgcParametersV1 {
    g: ProjectivePoint,
    h: ProjectivePoint,
    generator_digest: [u8; 32],
    parameter_digest: [u8; 32],
}

impl AnonymousPgcParametersV1 {
    /// Return the cached fixed parameter basis.
    ///
    /// # Errors
    ///
    /// Returns an error only if RFC 9380 generator derivation fails or the
    /// derived roles collide.
    pub fn get() -> Result<&'static Self, AnonymousPgcError> {
        PGC_PARAMETERS.as_ref().map_err(Clone::clone)
    }

    /// Return the canonical generator-basis digest.
    #[must_use]
    pub const fn generator_digest(&self) -> [u8; 32] {
        self.generator_digest
    }

    /// Return the fixed source/profile digest.
    #[must_use]
    pub const fn parameter_digest(&self) -> [u8; 32] {
        self.parameter_digest
    }

    /// Return the key/randomness generator `g`.
    #[must_use]
    pub fn key_generator(&self) -> CompressedPointV1 {
        CompressedPointV1::from_projective(self.g).expect("derived Anonymous-PGC g is non-identity")
    }

    /// Return the message generator `h`.
    #[must_use]
    pub fn message_generator(&self) -> CompressedPointV1 {
        CompressedPointV1::from_projective(self.h).expect("derived Anonymous-PGC h is non-identity")
    }

    fn derive() -> Result<Self, AnonymousPgcError> {
        let g = hash_to_curve_rfc9380(PGC_G_DST_V1, b"generator")?;
        let h = hash_to_curve_rfc9380(PGC_H_DST_V1, b"generator")?;
        let encoded = [
            CompressedPointV1::from_projective(g)?,
            CompressedPointV1::from_projective(h)?,
        ];
        validate_generator_independence(&encoded)?;
        let generator_digest =
            generator_digest(b"iroha.anonymous-pgc.twisted-elgamal.p256.v1", &encoded)?;

        let mut hash = Sha256::new();
        hash.update(PARAMETER_DIGEST_DOMAIN_V1);
        hash.update(
            u16::try_from(SOURCE_PROFILE_V1.len())
                .expect("source profile label is bounded")
                .to_be_bytes(),
        );
        hash.update(SOURCE_PROFILE_V1);
        hash.update(PGC_MESSAGE_BITS_V1.to_be_bytes());
        hash.update(PGC_MESSAGE_MIN_V1.to_be_bytes());
        hash.update(PGC_MESSAGE_MAX_V1.to_be_bytes());
        hash.update(generator_digest);
        let parameter_digest = hash.finalize().into();
        Ok(Self {
            g,
            h,
            generator_digest,
            parameter_digest,
        })
    }
}

static PGC_PARAMETERS: Lazy<Result<AnonymousPgcParametersV1, AnonymousPgcError>> =
    Lazy::new(AnonymousPgcParametersV1::derive);

struct PgcDecryptionTableV1 {
    baby_steps: BTreeMap<[u8; 33], u16>,
    giant_stride: ProjectivePoint,
}

impl PgcDecryptionTableV1 {
    fn derive() -> Result<Self, AnonymousPgcError> {
        let parameters = AnonymousPgcParametersV1::get()?;
        let mut baby_steps = BTreeMap::new();
        let mut point = parameters.h;
        // The identity at baby step zero is handled explicitly during lookup,
        // because the strict wire point type intentionally cannot encode it.
        for baby in 1..PGC_DECRYPTION_BABY_STEP_BOUND_V1 {
            let encoded = CompressedPointV1::from_projective(point)?;
            baby_steps.insert(
                *encoded.as_bytes(),
                u16::try_from(baby).map_err(|_| AnonymousPgcError::DecryptionTableFailure)?,
            );
            point += parameters.h;
        }
        if baby_steps.len() != PGC_DECRYPTION_BABY_STEP_BOUND_V1 - 1 {
            return Err(AnonymousPgcError::DecryptionTableFailure);
        }
        let giant_stride = parameters.h
            * Scalar::from(
                u64::try_from(PGC_DECRYPTION_BABY_STEP_BOUND_V1)
                    .map_err(|_| AnonymousPgcError::DecryptionTableFailure)?,
            );
        Ok(Self {
            baby_steps,
            giant_stride,
        })
    }
}

static PGC_DECRYPTION_TABLE: Lazy<Result<PgcDecryptionTableV1, AnonymousPgcError>> =
    Lazy::new(PgcDecryptionTableV1::derive);

/// Canonical non-identity Twisted-ElGamal public key `pk = g·sk`.
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
pub struct TwistedElGamalPublicKeyV1 {
    point: CompressedPointV1,
}

impl TwistedElGamalPublicKeyV1 {
    /// Parse and validate a public key.
    ///
    /// # Errors
    ///
    /// Returns an error for a malformed, non-canonical, or identity point.
    pub fn from_sec1_bytes(bytes: &[u8]) -> Result<Self, AnonymousPgcError> {
        Ok(Self {
            point: CompressedPointV1::from_slice(bytes)?,
        })
    }

    /// Return canonical compressed SEC1 bytes.
    #[must_use]
    pub const fn as_point(&self) -> CompressedPointV1 {
        self.point
    }

    /// Encode the key as canonical Norito.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        norito::codec::encode_adaptive(self)
    }

    /// Decode exactly one canonical Norito public key.
    ///
    /// # Errors
    ///
    /// Returns an error for truncated, trailing, or malformed encodings.
    pub fn decode_exact(bytes: &[u8]) -> Result<Self, AnonymousPgcError> {
        if bytes.len() > MAX_PGC_CIPHERTEXT_BYTES_V1 {
            return Err(AnonymousPgcError::EncodingTooLarge {
                actual: bytes.len(),
                max: MAX_PGC_CIPHERTEXT_BYTES_V1,
            });
        }
        let key = norito::codec::decode_exact_from_slice::<Self>(bytes)
            .map_err(|_| AnonymousPgcError::InvalidNoritoEncoding)?;
        let _ = key.point.to_projective()?;
        Ok(key)
    }
}

/// Twisted-ElGamal ciphertext `(C_L, C_R) = (pk·r, g·r + h·m)`.
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
pub struct TwistedElGamalCiphertextV1 {
    left: CompressedPointV1,
    right: CompressedPointV1,
}

impl TwistedElGamalCiphertextV1 {
    /// Return `C_L`.
    #[must_use]
    pub const fn left(&self) -> CompressedPointV1 {
        self.left
    }

    /// Return `C_R`.
    #[must_use]
    pub const fn right(&self) -> CompressedPointV1 {
        self.right
    }

    /// Encode this ciphertext as canonical Norito.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        norito::codec::encode_adaptive(self)
    }

    /// Decode exactly one canonical ciphertext.
    ///
    /// # Errors
    ///
    /// Returns an error for oversized, truncated, trailing, malformed,
    /// non-canonical, or identity encodings.
    pub fn decode_exact(bytes: &[u8]) -> Result<Self, AnonymousPgcError> {
        if bytes.len() > MAX_PGC_CIPHERTEXT_BYTES_V1 {
            return Err(AnonymousPgcError::EncodingTooLarge {
                actual: bytes.len(),
                max: MAX_PGC_CIPHERTEXT_BYTES_V1,
            });
        }
        let ciphertext = norito::codec::decode_exact_from_slice::<Self>(bytes)
            .map_err(|_| AnonymousPgcError::InvalidNoritoEncoding)?;
        ciphertext.validate()?;
        Ok(ciphertext)
    }

    fn validate(&self) -> Result<(), AnonymousPgcError> {
        let _ = self.left.to_projective()?;
        let _ = self.right.to_projective()?;
        Ok(())
    }
}

/// Generated non-zero Twisted-ElGamal key pair.
pub struct TwistedElGamalKeyPairV1 {
    secret: SecretScalarV1,
    public: TwistedElGamalPublicKeyV1,
}

impl TwistedElGamalKeyPairV1 {
    /// Generate a key pair from unbiased non-zero entropy.
    ///
    /// # Errors
    ///
    /// Returns an error if scalar sampling or fixed parameter derivation fails.
    pub fn generate<R>(rng: &mut R) -> Result<Self, AnonymousPgcError>
    where
        R: CryptoRng + RngCore,
    {
        let secret = SecretScalarV1::generate(rng)?;
        Self::from_secret(secret)
    }

    /// Construct a key pair from an already validated secret.
    ///
    /// # Errors
    ///
    /// Returns an error only if fixed parameter derivation fails.
    pub fn from_secret(secret: SecretScalarV1) -> Result<Self, AnonymousPgcError> {
        let parameters = AnonymousPgcParametersV1::get()?;
        let public = TwistedElGamalPublicKeyV1 {
            point: CompressedPointV1::from_projective(parameters.g * secret.expose_scalar())?,
        };
        Ok(Self { secret, public })
    }

    /// Return the public key.
    #[must_use]
    pub const fn public_key(&self) -> TwistedElGamalPublicKeyV1 {
        self.public
    }

    /// Borrow the zeroizing secret scalar.
    #[must_use]
    pub const fn secret_scalar(&self) -> &SecretScalarV1 {
        &self.secret
    }
}

/// Twisted-ElGamal or proof-building-block failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum AnonymousPgcError {
    /// Shared P-256 substrate failure.
    #[error(transparent)]
    P256(#[from] P256EngineError),
    /// A bounded canonical encoding exceeded its type-specific maximum.
    #[error("Anonymous-PGC encoding length {actual} exceeds maximum {max}")]
    EncodingTooLarge {
        /// Supplied bytes.
        actual: usize,
        /// Type-specific hard bound.
        max: usize,
    },
    /// Exact canonical Norito decoding failed.
    #[error("invalid canonical Anonymous-PGC Norito encoding")]
    InvalidNoritoEncoding,
    /// A proof used an unknown version.
    #[error("unsupported Anonymous-PGC building-block proof version {version}")]
    UnsupportedProofVersion {
        /// Supplied version.
        version: u8,
    },
    /// The governed parameter digest did not match this exact profile.
    #[error("Anonymous-PGC parameter digest mismatch")]
    ParameterDigestMismatch,
    /// The generator digest did not match the RFC 9380 basis.
    #[error("Anonymous-PGC generator digest mismatch")]
    GeneratorDigestMismatch,
    /// A supplied key did not correspond to the supplied secret.
    #[error("Twisted-ElGamal key does not correspond to the supplied secret")]
    SecretKeyMismatch,
    /// A ciphertext did not correspond to the supplied message/randomness opening.
    #[error("Twisted-ElGamal ciphertext opening mismatch")]
    CiphertextOpeningMismatch,
    /// Homomorphic addition produced a prohibited identity wire component.
    #[error("Twisted-ElGamal homomorphic sum contains an identity component")]
    HomomorphicIdentity,
    /// The fixed wallet-side decryption table could not be constructed.
    #[error("Twisted-ElGamal fixed 32-bit decryption table initialization failed")]
    DecryptionTableFailure,
    /// The decrypted group element did not encode a value in `[0, 2^32 - 1]`.
    #[error("Twisted-ElGamal ciphertext does not decrypt to an admitted 32-bit message")]
    MessageRecoveryFailed,
    /// A public-key possession proof equation failed.
    #[error("Twisted-ElGamal public-key possession equation failed")]
    KeyPossessionEquationFailed,
    /// A ciphertext opening proof equation failed.
    #[error("Twisted-ElGamal ciphertext opening equation failed")]
    CiphertextOpeningEquationFailed,
    /// Prover randomness repeatedly produced a prohibited identity intermediate.
    #[error("Anonymous-PGC building-block prover exhausted its restart bound")]
    ProverRestartExhausted,
}

/// Encrypt a 32-bit message with caller-provided independent non-zero randomness.
///
/// This is Definition 6.1's `Enc(pk,m;r)`:
/// `C_L = pk·r`, `C_R = g·r + h·m`.
///
/// # Errors
///
/// Returns an error for an invalid key or a negligibly probable identity
/// component.
pub fn encrypt_with_randomness(
    public_key: TwistedElGamalPublicKeyV1,
    message: u32,
    randomness: &SecretScalarV1,
) -> Result<TwistedElGamalCiphertextV1, AnonymousPgcError> {
    let parameters = AnonymousPgcParametersV1::get()?;
    let public = public_key.point.to_projective()?;
    let r = randomness.expose_scalar();
    let left = CompressedPointV1::from_projective(public * r)?;
    let right = CompressedPointV1::from_projective(
        parameters.g * r + parameters.h * Scalar::from(u64::from(message)),
    )?;
    Ok(TwistedElGamalCiphertextV1 { left, right })
}

/// Encrypt with freshly sampled independent non-zero randomness and return the
/// opening needed by a proof builder.
///
/// # Errors
///
/// Returns an error if entropy sampling, parameter derivation, or point
/// canonicalization fails.
pub fn encrypt<R>(
    public_key: TwistedElGamalPublicKeyV1,
    message: u32,
    rng: &mut R,
) -> Result<(TwistedElGamalCiphertextV1, SecretScalarV1), AnonymousPgcError>
where
    R: CryptoRng + RngCore,
{
    for _ in 0..MAX_PROVER_RESTARTS {
        let randomness = SecretScalarV1::generate(rng)?;
        match encrypt_with_randomness(public_key, message, &randomness) {
            Ok(ciphertext) => return Ok((ciphertext, randomness)),
            Err(AnonymousPgcError::P256(P256EngineError::IdentityPoint)) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(AnonymousPgcError::ProverRestartExhausted)
}

/// Decrypt a nonnegative 32-bit balance using fixed-profile baby-step giant-step.
///
/// The paper's Anonymous-PGC profile admits the inclusive message interval
/// `[0, 2^32 - 1]`; this method therefore returns every `u32` value, including
/// [`u32::MAX`].  It computes `h·m = C_R - C_L·sk^-1` and recovers `m` with at
/// most `2^16` baby steps and `2^16` giant steps.  The baby-step table is
/// initialized once and is bounded to 65,535 encoded non-identity points.
///
/// This is a wallet-side operation, not a consensus verifier operation.  Its
/// lookup and early-success behavior are not constant-time with respect to the
/// plaintext, but both work and memory have fixed first-release hard caps.
/// Callers must authenticate the account/ciphertext association separately;
/// like the paper's unauthenticated `Dec`, a wrong key is rejected exactly when
/// the resulting group element has no representative in the admitted interval.
///
/// # Errors
///
/// Returns an error for malformed ciphertext components, a failed fixed-table
/// initialization, or when no message exists in the inclusive 32-bit domain.
pub fn decrypt_u32(
    secret: &SecretScalarV1,
    ciphertext: TwistedElGamalCiphertextV1,
) -> Result<u32, AnonymousPgcError> {
    ciphertext.validate()?;
    let parameters = AnonymousPgcParametersV1::get()?;
    let secret_inverse = Option::<Scalar>::from(secret.expose_scalar().invert())
        .ok_or(AnonymousPgcError::MessageRecoveryFailed)?;
    let target =
        ciphertext.right.to_projective()? - ciphertext.left.to_projective()? * secret_inverse;
    if bool::from(target.is_identity()) {
        return Ok(0);
    }

    let table = PGC_DECRYPTION_TABLE.as_ref().map_err(Clone::clone)?;
    let mut candidate_point = target;
    for giant in 0..PGC_DECRYPTION_GIANT_STEP_BOUND_V1 {
        let baby = if bool::from(candidate_point.is_identity()) {
            Some(0_u16)
        } else {
            let encoded = CompressedPointV1::from_projective(candidate_point)?;
            table.baby_steps.get(encoded.as_bytes()).copied()
        };
        if let Some(baby) = baby {
            let giant =
                u64::try_from(giant).map_err(|_| AnonymousPgcError::MessageRecoveryFailed)?;
            let message = giant
                .checked_mul(
                    u64::try_from(PGC_DECRYPTION_BABY_STEP_BOUND_V1)
                        .map_err(|_| AnonymousPgcError::MessageRecoveryFailed)?,
                )
                .and_then(|high| high.checked_add(u64::from(baby)))
                .ok_or(AnonymousPgcError::MessageRecoveryFailed)?;
            let message =
                u32::try_from(message).map_err(|_| AnonymousPgcError::MessageRecoveryFailed)?;
            if parameters.h * Scalar::from(u64::from(message)) == target {
                return Ok(message);
            }
        }
        candidate_point -= table.giant_stride;
    }
    Err(AnonymousPgcError::MessageRecoveryFailed)
}

/// Add two Twisted-ElGamal ciphertexts component-wise.
///
/// The result encrypts the sum of messages with the sum of randomizers.  The
/// strict wire profile rejects the negligible/cancellation case where either
/// component is the identity.
///
/// # Errors
///
/// Returns an error for malformed inputs or an identity result component.
pub fn add_ciphertexts(
    left: TwistedElGamalCiphertextV1,
    right: TwistedElGamalCiphertextV1,
) -> Result<TwistedElGamalCiphertextV1, AnonymousPgcError> {
    left.validate()?;
    right.validate()?;
    let sum_left = left.left.to_projective()? + right.left.to_projective()?;
    let sum_right = left.right.to_projective()? + right.right.to_projective()?;
    let ciphertext = TwistedElGamalCiphertextV1 {
        left: CompressedPointV1::from_projective(sum_left)
            .map_err(|_| AnonymousPgcError::HomomorphicIdentity)?,
        right: CompressedPointV1::from_projective(sum_right)
            .map_err(|_| AnonymousPgcError::HomomorphicIdentity)?,
    };
    Ok(ciphertext)
}

/// Bound input for public-key possession.
#[derive(Clone, Copy, Debug)]
pub struct PgcKeyPossessionStatementV1<'a> {
    public_key: TwistedElGamalPublicKeyV1,
    transcript_binding: TranscriptBindingV1<'a>,
}

impl<'a> PgcKeyPossessionStatementV1<'a> {
    /// Construct a fully bound public-key statement.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid key or mismatched governed parameters.
    pub fn new(
        public_key: TwistedElGamalPublicKeyV1,
        transcript_binding: TranscriptBindingV1<'a>,
    ) -> Result<Self, AnonymousPgcError> {
        validate_binding(&transcript_binding)?;
        let _ = public_key.point.to_projective()?;
        Ok(Self {
            public_key,
            transcript_binding,
        })
    }

    /// Return the registered public key.
    #[must_use]
    pub const fn public_key(&self) -> TwistedElGamalPublicKeyV1 {
        self.public_key
    }
}

/// Canonical Schnorr proof that `pk = g·sk`.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(decode_from_slice)]
pub struct PgcKeyPossessionProofV1 {
    version: u8,
    announcement: CompressedPointV1,
    response: CanonicalScalarV1,
}

impl PgcKeyPossessionProofV1 {
    /// Encode as canonical Norito.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        norito::codec::encode_adaptive(self)
    }

    /// Decode one exact canonical proof.
    ///
    /// # Errors
    ///
    /// Returns an error for oversized, truncated, trailing, malformed,
    /// non-canonical, or unknown-version encodings.
    pub fn decode_exact(bytes: &[u8]) -> Result<Self, AnonymousPgcError> {
        if bytes.len() > MAX_PGC_BUILDING_BLOCK_PROOF_BYTES_V1 {
            return Err(AnonymousPgcError::EncodingTooLarge {
                actual: bytes.len(),
                max: MAX_PGC_BUILDING_BLOCK_PROOF_BYTES_V1,
            });
        }
        let proof = norito::codec::decode_exact_from_slice::<Self>(bytes)
            .map_err(|_| AnonymousPgcError::InvalidNoritoEncoding)?;
        proof.validate()?;
        Ok(proof)
    }

    fn validate(&self) -> Result<(), AnonymousPgcError> {
        if self.version != PGC_BUILDING_BLOCK_PROOF_VERSION_V1 {
            return Err(AnonymousPgcError::UnsupportedProofVersion {
                version: self.version,
            });
        }
        let _ = self.announcement.to_projective()?;
        let _ = self.response.to_scalar()?;
        Ok(())
    }
}

/// Prove possession of the secret corresponding to a registered public key.
///
/// # Errors
///
/// Returns an error for a mismatched key, invalid statement binding, exhausted
/// entropy, or a negligible identity announcement.
pub fn prove_key_possession<R>(
    statement: &PgcKeyPossessionStatementV1<'_>,
    secret: &SecretScalarV1,
    rng: &mut R,
) -> Result<PgcKeyPossessionProofV1, AnonymousPgcError>
where
    R: CryptoRng + RngCore,
{
    let parameters = AnonymousPgcParametersV1::get()?;
    if CompressedPointV1::from_projective(parameters.g * secret.expose_scalar())?
        != statement.public_key.point
    {
        return Err(AnonymousPgcError::SecretKeyMismatch);
    }
    for _ in 0..MAX_PROVER_RESTARTS {
        let mask = random_nonzero_scalar(rng)?;
        let announcement = CompressedPointV1::from_projective(parameters.g * mask)?;
        let mut transcript = key_possession_transcript(statement, &announcement)?;
        let challenge = transcript
            .challenge_nonzero_scalar(b"challenge", 0)?
            .to_scalar()?;
        let proof = PgcKeyPossessionProofV1 {
            version: PGC_BUILDING_BLOCK_PROOF_VERSION_V1,
            announcement,
            response: CanonicalScalarV1::from_scalar(mask + challenge * secret.expose_scalar()),
        };
        proof.validate()?;
        return Ok(proof);
    }
    Err(AnonymousPgcError::ProverRestartExhausted)
}

/// Verify a public-key possession proof.
///
/// # Errors
///
/// Returns an error for malformed material, binding mismatch, or a failed
/// Schnorr equation.
pub fn verify_key_possession(
    statement: &PgcKeyPossessionStatementV1<'_>,
    proof: &PgcKeyPossessionProofV1,
) -> Result<(), AnonymousPgcError> {
    proof.validate()?;
    validate_binding(&statement.transcript_binding)?;
    let parameters = AnonymousPgcParametersV1::get()?;
    let mut transcript = key_possession_transcript(statement, &proof.announcement)?;
    let challenge = transcript
        .challenge_nonzero_scalar(b"challenge", 0)?
        .to_scalar()?;
    let left = parameters.g * proof.response.to_scalar()?;
    let right = proof.announcement.to_projective()?
        + statement.public_key.point.to_projective()? * challenge;
    if left != right {
        return Err(AnonymousPgcError::KeyPossessionEquationFailed);
    }
    Ok(())
}

/// Fully bound input to a Twisted-ElGamal ciphertext-opening proof.
#[derive(Clone, Copy, Debug)]
pub struct PgcCiphertextOpeningStatementV1<'a> {
    public_key: TwistedElGamalPublicKeyV1,
    ciphertext: TwistedElGamalCiphertextV1,
    transcript_binding: TranscriptBindingV1<'a>,
}

impl<'a> PgcCiphertextOpeningStatementV1<'a> {
    /// Construct a bound opening statement.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed points or governed-digest mismatch.
    pub fn new(
        public_key: TwistedElGamalPublicKeyV1,
        ciphertext: TwistedElGamalCiphertextV1,
        transcript_binding: TranscriptBindingV1<'a>,
    ) -> Result<Self, AnonymousPgcError> {
        validate_binding(&transcript_binding)?;
        let _ = public_key.point.to_projective()?;
        ciphertext.validate()?;
        Ok(Self {
            public_key,
            ciphertext,
            transcript_binding,
        })
    }

    /// Return the public key.
    #[must_use]
    pub const fn public_key(&self) -> TwistedElGamalPublicKeyV1 {
        self.public_key
    }

    /// Return the ciphertext.
    #[must_use]
    pub const fn ciphertext(&self) -> TwistedElGamalCiphertextV1 {
        self.ciphertext
    }
}

/// Canonical generalized-Schnorr proof of a Twisted-ElGamal opening.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(decode_from_slice)]
pub struct PgcCiphertextOpeningProofV1 {
    version: u8,
    announcement_left: CompressedPointV1,
    announcement_right: CompressedPointV1,
    randomness_response: CanonicalScalarV1,
    message_response: CanonicalScalarV1,
}

impl PgcCiphertextOpeningProofV1 {
    /// Encode as canonical Norito.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        norito::codec::encode_adaptive(self)
    }

    /// Decode one exact canonical proof.
    ///
    /// # Errors
    ///
    /// Returns an error for oversized, truncated, trailing, malformed,
    /// non-canonical, or unknown-version encodings.
    pub fn decode_exact(bytes: &[u8]) -> Result<Self, AnonymousPgcError> {
        if bytes.len() > MAX_PGC_BUILDING_BLOCK_PROOF_BYTES_V1 {
            return Err(AnonymousPgcError::EncodingTooLarge {
                actual: bytes.len(),
                max: MAX_PGC_BUILDING_BLOCK_PROOF_BYTES_V1,
            });
        }
        let proof = norito::codec::decode_exact_from_slice::<Self>(bytes)
            .map_err(|_| AnonymousPgcError::InvalidNoritoEncoding)?;
        proof.validate()?;
        Ok(proof)
    }

    fn validate(&self) -> Result<(), AnonymousPgcError> {
        if self.version != PGC_BUILDING_BLOCK_PROOF_VERSION_V1 {
            return Err(AnonymousPgcError::UnsupportedProofVersion {
                version: self.version,
            });
        }
        let _ = self.announcement_left.to_projective()?;
        let _ = self.announcement_right.to_projective()?;
        let _ = self.randomness_response.to_scalar()?;
        let _ = self.message_response.to_scalar()?;
        Ok(())
    }
}

/// Prove knowledge of `(message, randomness)` opening a Twisted-ElGamal
/// ciphertext.
///
/// # Errors
///
/// Returns an error for a false opening, invalid binding, exhausted entropy,
/// or a negligible identity announcement.
pub fn prove_ciphertext_opening<R>(
    statement: &PgcCiphertextOpeningStatementV1<'_>,
    message: u32,
    randomness: &SecretScalarV1,
    rng: &mut R,
) -> Result<PgcCiphertextOpeningProofV1, AnonymousPgcError>
where
    R: CryptoRng + RngCore,
{
    if encrypt_with_randomness(statement.public_key, message, randomness)? != statement.ciphertext {
        return Err(AnonymousPgcError::CiphertextOpeningMismatch);
    }
    let parameters = AnonymousPgcParametersV1::get()?;
    for _ in 0..MAX_PROVER_RESTARTS {
        let randomness_mask = random_nonzero_scalar(rng)?;
        let message_mask = random_nonzero_scalar(rng)?;
        let announcement_left = CompressedPointV1::from_projective(
            statement.public_key.point.to_projective()? * randomness_mask,
        )?;
        let announcement_right = match CompressedPointV1::from_projective(
            parameters.g * randomness_mask + parameters.h * message_mask,
        ) {
            Ok(point) => point,
            Err(P256EngineError::IdentityPoint) => continue,
            Err(error) => return Err(error.into()),
        };
        let mut transcript =
            ciphertext_opening_transcript(statement, &announcement_left, &announcement_right)?;
        let challenge = transcript
            .challenge_nonzero_scalar(b"challenge", 0)?
            .to_scalar()?;
        let proof = PgcCiphertextOpeningProofV1 {
            version: PGC_BUILDING_BLOCK_PROOF_VERSION_V1,
            announcement_left,
            announcement_right,
            randomness_response: CanonicalScalarV1::from_scalar(
                randomness_mask + challenge * randomness.expose_scalar(),
            ),
            message_response: CanonicalScalarV1::from_scalar(
                message_mask + challenge * Scalar::from(u64::from(message)),
            ),
        };
        proof.validate()?;
        return Ok(proof);
    }
    Err(AnonymousPgcError::ProverRestartExhausted)
}

/// Verify a Twisted-ElGamal ciphertext-opening proof.
///
/// # Errors
///
/// Returns an error for malformed material, a binding mismatch, or either
/// failed generalized-Schnorr equation.
pub fn verify_ciphertext_opening(
    statement: &PgcCiphertextOpeningStatementV1<'_>,
    proof: &PgcCiphertextOpeningProofV1,
) -> Result<(), AnonymousPgcError> {
    proof.validate()?;
    validate_binding(&statement.transcript_binding)?;
    let parameters = AnonymousPgcParametersV1::get()?;
    let mut transcript = ciphertext_opening_transcript(
        statement,
        &proof.announcement_left,
        &proof.announcement_right,
    )?;
    let challenge = transcript
        .challenge_nonzero_scalar(b"challenge", 0)?
        .to_scalar()?;
    let randomness_response = proof.randomness_response.to_scalar()?;
    let message_response = proof.message_response.to_scalar()?;

    let left_equation = statement.public_key.point.to_projective()? * randomness_response;
    let right_equation = proof.announcement_left.to_projective()?
        + statement.ciphertext.left.to_projective()? * challenge;
    if left_equation != right_equation {
        return Err(AnonymousPgcError::CiphertextOpeningEquationFailed);
    }

    let left_equation = parameters.g * randomness_response + parameters.h * message_response;
    let right_equation = proof.announcement_right.to_projective()?
        + statement.ciphertext.right.to_projective()? * challenge;
    if left_equation != right_equation {
        return Err(AnonymousPgcError::CiphertextOpeningEquationFailed);
    }
    Ok(())
}

/// Decode and verify exact opaque opening-proof bytes.
///
/// # Errors
///
/// Returns the same errors as [`PgcCiphertextOpeningProofV1::decode_exact`] and
/// [`verify_ciphertext_opening`].
pub fn verify_ciphertext_opening_encoded(
    statement: &PgcCiphertextOpeningStatementV1<'_>,
    proof_bytes: &[u8],
) -> Result<(), AnonymousPgcError> {
    let proof = PgcCiphertextOpeningProofV1::decode_exact(proof_bytes)?;
    verify_ciphertext_opening(statement, &proof)
}

fn validate_binding(binding: &TranscriptBindingV1<'_>) -> Result<(), AnonymousPgcError> {
    binding.validate()?;
    let parameters = AnonymousPgcParametersV1::get()?;
    if binding.parameter_digest != parameters.parameter_digest {
        return Err(AnonymousPgcError::ParameterDigestMismatch);
    }
    if binding.generator_digest != parameters.generator_digest {
        return Err(AnonymousPgcError::GeneratorDigestMismatch);
    }
    Ok(())
}

fn key_possession_transcript(
    statement: &PgcKeyPossessionStatementV1<'_>,
    announcement: &CompressedPointV1,
) -> Result<TranscriptV1, AnonymousPgcError> {
    let mut transcript =
        TranscriptV1::new(PGC_KEY_POSSESSION_SUITE_V1, &statement.transcript_binding)?;
    transcript.append_point(b"public_key", &statement.public_key.point)?;
    transcript.append_point(b"announcement", announcement)?;
    Ok(transcript)
}

fn ciphertext_opening_transcript(
    statement: &PgcCiphertextOpeningStatementV1<'_>,
    announcement_left: &CompressedPointV1,
    announcement_right: &CompressedPointV1,
) -> Result<TranscriptV1, AnonymousPgcError> {
    let mut transcript = TranscriptV1::new(
        PGC_CIPHERTEXT_OPENING_SUITE_V1,
        &statement.transcript_binding,
    )?;
    transcript.append_point(b"public_key", &statement.public_key.point)?;
    transcript.append_point(b"ciphertext_left", &statement.ciphertext.left)?;
    transcript.append_point(b"ciphertext_right", &statement.ciphertext.right)?;
    transcript.append_point(b"announcement_left", announcement_left)?;
    transcript.append_point(b"announcement_right", announcement_right)?;
    Ok(transcript)
}

#[cfg(test)]
mod tests {
    use rand_core_06::{CryptoRng, Error as RngError, RngCore};

    use super::*;

    struct KatRng {
        seed: [u8; 32],
        counter: u64,
    }

    impl KatRng {
        fn new(seed: [u8; 32]) -> Self {
            Self { seed, counter: 0 }
        }
    }

    impl RngCore for KatRng {
        fn next_u32(&mut self) -> u32 {
            let mut bytes = [0_u8; 4];
            self.fill_bytes(&mut bytes);
            u32::from_be_bytes(bytes)
        }

        fn next_u64(&mut self) -> u64 {
            let mut bytes = [0_u8; 8];
            self.fill_bytes(&mut bytes);
            u64::from_be_bytes(bytes)
        }

        fn fill_bytes(&mut self, destination: &mut [u8]) {
            let mut offset = 0;
            while offset < destination.len() {
                let mut hash = Sha256::new();
                hash.update(b"iroha.anonymous-pgc.kat.rng.v1");
                hash.update(self.seed);
                hash.update(self.counter.to_be_bytes());
                self.counter = self.counter.wrapping_add(1);
                let block: [u8; 32] = hash.finalize().into();
                let take = (destination.len() - offset).min(block.len());
                destination[offset..offset + take].copy_from_slice(&block[..take]);
                offset += take;
            }
        }

        fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), RngError> {
            self.fill_bytes(destination);
            Ok(())
        }
    }

    impl CryptoRng for KatRng {}

    fn scalar(value: u8) -> SecretScalarV1 {
        let mut bytes = [0_u8; 32];
        bytes[31] = value;
        SecretScalarV1::from_bytes(bytes).expect("test scalar")
    }

    fn binding() -> TranscriptBindingV1<'static> {
        let parameters = AnonymousPgcParametersV1::get().expect("parameters");
        TranscriptBindingV1 {
            chain_id: b"taira-test",
            genesis_hash: [0x31; 32],
            action_index: 2,
            statement_digest: [0x32; 32],
            parameter_id: [0x33; 32],
            parameter_digest: parameters.parameter_digest(),
            verifier_digest: [0x34; 32],
            statement_schema_digest: [0x35; 32],
            engine_manifest_digest: [0x36; 32],
            generator_digest: parameters.generator_digest(),
        }
    }

    fn fixture() -> (
        TwistedElGamalKeyPairV1,
        TwistedElGamalCiphertextV1,
        SecretScalarV1,
        PgcCiphertextOpeningStatementV1<'static>,
        PgcCiphertextOpeningProofV1,
    ) {
        let key_pair = TwistedElGamalKeyPairV1::from_secret(scalar(7)).expect("key pair");
        let randomness = scalar(11);
        let ciphertext =
            encrypt_with_randomness(key_pair.public_key(), 42, &randomness).expect("ciphertext");
        let statement =
            PgcCiphertextOpeningStatementV1::new(key_pair.public_key(), ciphertext, binding())
                .expect("statement");
        let mut rng = KatRng::new([0x41; 32]);
        let proof = prove_ciphertext_opening(&statement, 42, &randomness, &mut rng).expect("proof");
        (key_pair, ciphertext, randomness, statement, proof)
    }

    #[test]
    fn twisted_elgamal_equations_and_opening_proof_verify() {
        let (key_pair, ciphertext, randomness, statement, proof) = fixture();
        verify_ciphertext_opening(&statement, &proof).expect("opening proof");
        verify_ciphertext_opening_encoded(&statement, &proof.encode()).expect("encoded proof");
        assert_eq!(
            encrypt_with_randomness(key_pair.public_key(), 42, &randomness).expect("same opening"),
            ciphertext
        );

        let parameters = AnonymousPgcParametersV1::get().expect("parameters");
        let secret_inverse = Option::<Scalar>::from(key_pair.secret.expose_scalar().invert())
            .expect("non-zero inverse");
        let recovered = ciphertext.right.to_projective().expect("right")
            - ciphertext.left.to_projective().expect("left") * secret_inverse;
        assert_eq!(recovered, parameters.h * Scalar::from(42_u64));
    }

    #[test]
    fn key_possession_proof_verifies_and_rejects_wrong_secret() {
        let key_pair = TwistedElGamalKeyPairV1::from_secret(scalar(9)).expect("key pair");
        let statement =
            PgcKeyPossessionStatementV1::new(key_pair.public_key(), binding()).expect("statement");
        let mut rng = KatRng::new([0x55; 32]);
        let proof =
            prove_key_possession(&statement, key_pair.secret_scalar(), &mut rng).expect("proof");
        verify_key_possession(&statement, &proof).expect("verify");
        assert!(matches!(
            prove_key_possession(&statement, &scalar(10), &mut rng),
            Err(AnonymousPgcError::SecretKeyMismatch)
        ));

        let bytes = proof.encode();
        assert_eq!(
            PgcKeyPossessionProofV1::decode_exact(&bytes).expect("decode"),
            proof
        );
        for end in 0..bytes.len() {
            assert!(PgcKeyPossessionProofV1::decode_exact(&bytes[..end]).is_err());
        }
        let mut trailing = bytes;
        trailing.push(0);
        assert!(PgcKeyPossessionProofV1::decode_exact(&trailing).is_err());

        let mut unknown = proof.clone();
        unknown.version += 1;
        assert!(matches!(
            PgcKeyPossessionProofV1::decode_exact(&unknown.encode()),
            Err(AnonymousPgcError::UnsupportedProofVersion { .. })
        ));
        let mut noncanonical = proof.clone();
        noncanonical.response = CanonicalScalarV1::from_unchecked_bytes(
            hex::decode("ffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551")
                .expect("order")
                .try_into()
                .expect("32 bytes"),
        );
        assert!(PgcKeyPossessionProofV1::decode_exact(&noncanonical.encode()).is_err());
        let mut identity_announcement = proof.clone();
        identity_announcement.announcement = CompressedPointV1::from_unchecked_bytes([0; 33]);
        assert!(PgcKeyPossessionProofV1::decode_exact(&identity_announcement.encode()).is_err());
        let mut mutated_response = proof;
        mutated_response.response = CanonicalScalarV1::from_scalar(
            mutated_response.response.to_scalar().expect("response") + Scalar::ONE,
        );
        assert!(verify_key_possession(&statement, &mutated_response).is_err());
    }

    #[test]
    fn fresh_encryptions_use_independent_randomness() {
        let key_pair = TwistedElGamalKeyPairV1::from_secret(scalar(3)).expect("key pair");
        let mut rng = KatRng::new([0x66; 32]);
        let (first, first_r) = encrypt(key_pair.public_key(), 7, &mut rng).expect("first");
        let (second, second_r) = encrypt(key_pair.public_key(), 7, &mut rng).expect("second");
        assert_ne!(first, second);
        assert_ne!(first_r.canonical_encoding(), second_r.canonical_encoding());
    }

    #[test]
    fn message_space_boundaries_encrypt_and_zero_secrets_are_rejected() {
        let key_pair = TwistedElGamalKeyPairV1::from_secret(scalar(3)).expect("key pair");
        let randomness = scalar(5);
        for message in [PGC_MESSAGE_MIN_V1, 1, PGC_MESSAGE_MAX_V1] {
            let ciphertext = encrypt_with_randomness(key_pair.public_key(), message, &randomness)
                .expect("boundary ciphertext");
            ciphertext.validate().expect("strict ciphertext");
            assert_eq!(
                decrypt_u32(key_pair.secret_scalar(), ciphertext).expect("boundary decryption"),
                message
            );
        }
        assert_eq!(PGC_DECRYPTION_BABY_STEP_BOUND_V1, 1 << 16);
        assert_eq!(PGC_DECRYPTION_GIANT_STEP_BOUND_V1, 1 << 16);
        assert_eq!(PGC_MESSAGE_BITS_V1, 32);
        assert_eq!(PGC_MESSAGE_MIN_V1, 0);
        assert_eq!(PGC_MESSAGE_MAX_V1, u32::MAX);
        assert!(matches!(
            SecretScalarV1::from_bytes([0; 32]),
            Err(P256EngineError::ZeroScalar)
        ));
    }

    #[test]
    fn componentwise_addition_matches_added_opening() {
        let key_pair = TwistedElGamalKeyPairV1::from_secret(scalar(5)).expect("key pair");
        let r1 = scalar(13);
        let r2 = scalar(17);
        let c1 = encrypt_with_randomness(key_pair.public_key(), 19, &r1).expect("c1");
        let c2 = encrypt_with_randomness(key_pair.public_key(), 23, &r2).expect("c2");
        let sum = add_ciphertexts(c1, c2).expect("sum");
        let summed_r = SecretScalarV1::from_bytes(
            CanonicalScalarV1::from_scalar(r1.expose_scalar() + r2.expose_scalar())
                .as_bytes()
                .to_owned(),
        )
        .expect("summed randomness");
        assert_eq!(
            sum,
            encrypt_with_randomness(key_pair.public_key(), 42, &summed_r).expect("summed opening")
        );
        assert_eq!(
            decrypt_u32(key_pair.secret_scalar(), sum).expect("homomorphic balance update"),
            42
        );

        let inverse = TwistedElGamalCiphertextV1 {
            left: CompressedPointV1::from_projective(-c1.left.to_projective().expect("left point"))
                .expect("inverse left"),
            right: CompressedPointV1::from_projective(
                -c1.right.to_projective().expect("right point"),
            )
            .expect("inverse right"),
        };
        assert!(matches!(
            add_ciphertexts(c1, inverse),
            Err(AnonymousPgcError::HomomorphicIdentity)
        ));
    }

    #[test]
    fn decryption_rejects_wrong_key_mutations_and_malformed_components() {
        let (key_pair, ciphertext, _, _, _) = fixture();
        assert_eq!(
            decrypt_u32(key_pair.secret_scalar(), ciphertext).expect("correct key"),
            42
        );

        let wrong_key = TwistedElGamalKeyPairV1::from_secret(scalar(19)).expect("wrong key");
        assert!(matches!(
            decrypt_u32(wrong_key.secret_scalar(), ciphertext),
            Err(AnonymousPgcError::MessageRecoveryFailed)
        ));

        let parameters = AnonymousPgcParametersV1::get().expect("parameters");
        let mut mutated_left = ciphertext;
        mutated_left.left = parameters.message_generator();
        assert!(matches!(
            decrypt_u32(key_pair.secret_scalar(), mutated_left),
            Err(AnonymousPgcError::MessageRecoveryFailed)
        ));
        let mut mutated_right = ciphertext;
        mutated_right.right = parameters.key_generator();
        assert!(matches!(
            decrypt_u32(key_pair.secret_scalar(), mutated_right),
            Err(AnonymousPgcError::MessageRecoveryFailed)
        ));

        let mut malformed_left = ciphertext;
        malformed_left.left = CompressedPointV1::from_unchecked_bytes([0; 33]);
        assert!(matches!(
            decrypt_u32(key_pair.secret_scalar(), malformed_left),
            Err(AnonymousPgcError::P256(
                P256EngineError::InvalidPointEncoding
            ))
        ));
        let mut malformed_right = ciphertext;
        malformed_right.right = CompressedPointV1::from_unchecked_bytes([0; 33]);
        assert!(matches!(
            decrypt_u32(key_pair.secret_scalar(), malformed_right),
            Err(AnonymousPgcError::P256(
                P256EngineError::InvalidPointEncoding
            ))
        ));

        let max_randomness = scalar(29);
        let one_randomness = scalar(31);
        let encrypted_max =
            encrypt_with_randomness(key_pair.public_key(), PGC_MESSAGE_MAX_V1, &max_randomness)
                .expect("maximum");
        let encrypted_one =
            encrypt_with_randomness(key_pair.public_key(), 1, &one_randomness).expect("one");
        let overflowed = add_ciphertexts(encrypted_max, encrypted_one).expect("group sum");
        assert!(matches!(
            decrypt_u32(key_pair.secret_scalar(), overflowed),
            Err(AnonymousPgcError::MessageRecoveryFailed)
        ));
    }

    #[test]
    fn strict_codecs_reject_all_truncations_trailing_and_malformed_points() {
        let (key_pair, ciphertext, _, _, proof) = fixture();
        let key_bytes = key_pair.public_key().encode();
        for end in 0..key_bytes.len() {
            assert!(TwistedElGamalPublicKeyV1::decode_exact(&key_bytes[..end]).is_err());
        }
        let ciphertext_bytes = ciphertext.encode();
        for end in 0..ciphertext_bytes.len() {
            assert!(TwistedElGamalCiphertextV1::decode_exact(&ciphertext_bytes[..end]).is_err());
        }
        let proof_bytes = proof.encode();
        for end in 0..proof_bytes.len() {
            assert!(PgcCiphertextOpeningProofV1::decode_exact(&proof_bytes[..end]).is_err());
        }

        let mut key_trailing = key_bytes;
        key_trailing.push(0);
        assert!(TwistedElGamalPublicKeyV1::decode_exact(&key_trailing).is_err());
        let mut ciphertext_trailing = ciphertext_bytes;
        ciphertext_trailing.push(0);
        assert!(TwistedElGamalCiphertextV1::decode_exact(&ciphertext_trailing).is_err());
        let mut proof_trailing = proof_bytes;
        proof_trailing.push(0);
        assert!(PgcCiphertextOpeningProofV1::decode_exact(&proof_trailing).is_err());

        let malformed_key = TwistedElGamalPublicKeyV1 {
            point: CompressedPointV1::from_unchecked_bytes([0; 33]),
        };
        assert!(TwistedElGamalPublicKeyV1::decode_exact(&malformed_key.encode()).is_err());
        let mut malformed = ciphertext;
        malformed.left = CompressedPointV1::from_unchecked_bytes([0; 33]);
        assert!(TwistedElGamalCiphertextV1::decode_exact(&malformed.encode()).is_err());
        let mut malformed = ciphertext;
        malformed.right = CompressedPointV1::from_unchecked_bytes([0; 33]);
        assert!(TwistedElGamalCiphertextV1::decode_exact(&malformed.encode()).is_err());

        let too_large_ciphertext = vec![0; MAX_PGC_CIPHERTEXT_BYTES_V1 + 1];
        assert!(matches!(
            TwistedElGamalPublicKeyV1::decode_exact(&too_large_ciphertext),
            Err(AnonymousPgcError::EncodingTooLarge { .. })
        ));
        assert!(matches!(
            TwistedElGamalCiphertextV1::decode_exact(&too_large_ciphertext),
            Err(AnonymousPgcError::EncodingTooLarge { .. })
        ));
        let too_large_proof = vec![0; MAX_PGC_BUILDING_BLOCK_PROOF_BYTES_V1 + 1];
        assert!(matches!(
            PgcKeyPossessionProofV1::decode_exact(&too_large_proof),
            Err(AnonymousPgcError::EncodingTooLarge { .. })
        ));
        assert!(matches!(
            PgcCiphertextOpeningProofV1::decode_exact(&too_large_proof),
            Err(AnonymousPgcError::EncodingTooLarge { .. })
        ));
    }

    #[test]
    fn opening_proof_decoder_rejects_versions_points_and_scalars() {
        let (_, _, _, _, proof) = fixture();

        let mut unknown = proof.clone();
        unknown.version += 1;
        assert!(matches!(
            PgcCiphertextOpeningProofV1::decode_exact(&unknown.encode()),
            Err(AnonymousPgcError::UnsupportedProofVersion { .. })
        ));

        let mut malformed_left = proof.clone();
        malformed_left.announcement_left = CompressedPointV1::from_unchecked_bytes([0; 33]);
        assert!(PgcCiphertextOpeningProofV1::decode_exact(&malformed_left.encode()).is_err());
        let mut malformed_right = proof.clone();
        malformed_right.announcement_right = CompressedPointV1::from_unchecked_bytes([0; 33]);
        assert!(PgcCiphertextOpeningProofV1::decode_exact(&malformed_right.encode()).is_err());

        let order: [u8; 32] =
            hex::decode("ffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551")
                .expect("order")
                .try_into()
                .expect("32 bytes");
        let mut noncanonical_randomness = proof.clone();
        noncanonical_randomness.randomness_response =
            CanonicalScalarV1::from_unchecked_bytes(order);
        assert!(
            PgcCiphertextOpeningProofV1::decode_exact(&noncanonical_randomness.encode()).is_err()
        );
        let mut noncanonical_message = proof;
        noncanonical_message.message_response = CanonicalScalarV1::from_unchecked_bytes(order);
        assert!(PgcCiphertextOpeningProofV1::decode_exact(&noncanonical_message.encode()).is_err());
    }

    #[test]
    fn opening_proof_rejects_wrong_opening_statement_and_each_component_mutation() {
        let (key_pair, ciphertext, randomness, statement, proof) = fixture();
        let mut rng = KatRng::new([0x72; 32]);
        assert!(matches!(
            prove_ciphertext_opening(&statement, 43, &randomness, &mut rng),
            Err(AnonymousPgcError::CiphertextOpeningMismatch)
        ));
        assert!(matches!(
            prove_ciphertext_opening(&statement, 42, &scalar(12), &mut rng),
            Err(AnonymousPgcError::CiphertextOpeningMismatch)
        ));

        let other_key = TwistedElGamalKeyPairV1::from_secret(scalar(8))
            .expect("other key")
            .public_key();
        let wrong_key =
            PgcCiphertextOpeningStatementV1::new(other_key, ciphertext, binding()).expect("wrong");
        assert!(verify_ciphertext_opening(&wrong_key, &proof).is_err());

        let other_ciphertext =
            encrypt_with_randomness(key_pair.public_key(), 42, &scalar(12)).expect("other");
        let wrong_ciphertext = PgcCiphertextOpeningStatementV1::new(
            key_pair.public_key(),
            other_ciphertext,
            binding(),
        )
        .expect("wrong");
        assert!(verify_ciphertext_opening(&wrong_ciphertext, &proof).is_err());

        let mut changed = proof.clone();
        changed.announcement_left = proof.announcement_right;
        assert!(verify_ciphertext_opening(&statement, &changed).is_err());
        let mut changed = proof.clone();
        changed.announcement_right = proof.announcement_left;
        assert!(verify_ciphertext_opening(&statement, &changed).is_err());
        let mut changed = proof.clone();
        changed.randomness_response = CanonicalScalarV1::from_scalar(
            proof
                .randomness_response
                .to_scalar()
                .expect("randomness response")
                + Scalar::ONE,
        );
        assert!(verify_ciphertext_opening(&statement, &changed).is_err());
        let mut changed = proof;
        changed.message_response = CanonicalScalarV1::from_scalar(
            changed
                .message_response
                .to_scalar()
                .expect("message response")
                + Scalar::ONE,
        );
        assert!(verify_ciphertext_opening(&statement, &changed).is_err());
    }

    #[test]
    fn every_transcript_binding_mutation_invalidates_opening_proof() {
        let (_, _, _, statement, proof) = fixture();
        let mut mutations = Vec::new();
        let mut changed = statement.transcript_binding;
        changed.chain_id = b"other";
        mutations.push(changed);
        let mut changed = statement.transcript_binding;
        changed.genesis_hash[0] ^= 1;
        mutations.push(changed);
        let mut changed = statement.transcript_binding;
        changed.action_index += 1;
        mutations.push(changed);
        let mut changed = statement.transcript_binding;
        changed.statement_digest[0] ^= 1;
        mutations.push(changed);
        let mut changed = statement.transcript_binding;
        changed.parameter_id[0] ^= 1;
        mutations.push(changed);
        let mut changed = statement.transcript_binding;
        changed.verifier_digest[0] ^= 1;
        mutations.push(changed);
        let mut changed = statement.transcript_binding;
        changed.statement_schema_digest[0] ^= 1;
        mutations.push(changed);
        let mut changed = statement.transcript_binding;
        changed.engine_manifest_digest[0] ^= 1;
        mutations.push(changed);

        for binding in mutations {
            let changed = PgcCiphertextOpeningStatementV1::new(
                statement.public_key,
                statement.ciphertext,
                binding,
            )
            .expect("binding remains structurally valid");
            assert!(verify_ciphertext_opening(&changed, &proof).is_err());
        }

        let mut wrong_parameter = statement.transcript_binding;
        wrong_parameter.parameter_digest[0] ^= 1;
        assert!(matches!(
            PgcCiphertextOpeningStatementV1::new(
                statement.public_key,
                statement.ciphertext,
                wrong_parameter
            ),
            Err(AnonymousPgcError::ParameterDigestMismatch)
        ));
        let mut wrong_generator = statement.transcript_binding;
        wrong_generator.generator_digest[0] ^= 1;
        assert!(matches!(
            PgcCiphertextOpeningStatementV1::new(
                statement.public_key,
                statement.ciphertext,
                wrong_generator
            ),
            Err(AnonymousPgcError::GeneratorDigestMismatch)
        ));
    }

    #[test]
    fn canonical_known_answer_material_is_stable() {
        let (key_pair, ciphertext, _, statement, proof) = fixture();
        verify_ciphertext_opening(&statement, &proof).expect("proof");
        let parameters = AnonymousPgcParametersV1::get().expect("parameters");
        assert_eq!(
            (
                hex::encode(parameters.parameter_digest()),
                hex::encode(parameters.generator_digest()),
                hex::encode(key_pair.public_key().as_point().as_bytes()),
                hex::encode(Sha256::digest(ciphertext.encode())),
                hex::encode(Sha256::digest(proof.encode())),
            ),
            (
                "REPLACE_PGC_PARAMETER_DIGEST".to_owned(),
                "REPLACE_PGC_GENERATOR_DIGEST".to_owned(),
                "REPLACE_PGC_PUBLIC_KEY".to_owned(),
                "REPLACE_PGC_CIPHERTEXT_DIGEST".to_owned(),
                "REPLACE_PGC_PROOF_DIGEST".to_owned(),
            )
        );
    }
}
