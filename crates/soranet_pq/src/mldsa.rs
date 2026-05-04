use pqcrypto_mldsa::{mldsa44, mldsa65, mldsa87};
use pqcrypto_traits::{
    Error as PqError,
    sign::{
        DetachedSignature as PrimitiveDetachedSignature, PublicKey as PrimitivePublicKey,
        VerificationError,
    },
};
use rand_core::RngCore;
use thiserror::Error;
use zeroize::Zeroizing;

use crate::{
    HedgedChaCha20Rng, HedgedRngSeed, RngError, deterministic_chacha20_rng,
    hedged_chacha20_rng_from_os,
};

#[path = "mldsa_backend.rs"]
mod backend;

/// Maximum context length accepted by FIPS 204 ML-DSA signing and verification.
pub const ML_DSA_CONTEXT_MAX_LEN: usize = 255;

/// Supported ML-DSA parameter sets.
#[derive(Clone, Copy, Debug)]
pub enum MlDsaSuite {
    /// ML-DSA-44.
    MlDsa44,
    /// ML-DSA-65.
    MlDsa65,
    /// ML-DSA-87.
    MlDsa87,
}

impl MlDsaSuite {
    /// Return the numeric identifier used on the FFI surface.
    #[must_use]
    pub const fn suite_id(self) -> u8 {
        match self {
            MlDsaSuite::MlDsa44 => 0,
            MlDsaSuite::MlDsa65 => 1,
            MlDsaSuite::MlDsa87 => 2,
        }
    }

    /// Parse an [`MlDsaSuite`] from its numeric identifier.
    #[must_use]
    pub const fn from_suite_id(id: u8) -> Option<Self> {
        match id {
            0 => Some(MlDsaSuite::MlDsa44),
            1 => Some(MlDsaSuite::MlDsa65),
            2 => Some(MlDsaSuite::MlDsa87),
            _ => None,
        }
    }

    /// Return the public key length in bytes for this suite.
    #[must_use]
    pub fn public_key_len(self) -> usize {
        match self {
            MlDsaSuite::MlDsa44 => mldsa44::public_key_bytes(),
            MlDsaSuite::MlDsa65 => mldsa65::public_key_bytes(),
            MlDsaSuite::MlDsa87 => mldsa87::public_key_bytes(),
        }
    }

    /// Return the secret key length in bytes for this suite.
    #[must_use]
    pub fn secret_key_len(self) -> usize {
        match self {
            MlDsaSuite::MlDsa44 => mldsa44::secret_key_bytes(),
            MlDsaSuite::MlDsa65 => mldsa65::secret_key_bytes(),
            MlDsaSuite::MlDsa87 => mldsa87::secret_key_bytes(),
        }
    }

    /// Return the detached signature length in bytes for this suite.
    #[must_use]
    pub fn signature_len(self) -> usize {
        match self {
            MlDsaSuite::MlDsa44 => mldsa44::signature_bytes(),
            MlDsaSuite::MlDsa65 => mldsa65::signature_bytes(),
            MlDsaSuite::MlDsa87 => mldsa87::signature_bytes(),
        }
    }

    const fn public_key_kind(self) -> &'static str {
        match self {
            MlDsaSuite::MlDsa44 => "ML-DSA-44 public key",
            MlDsaSuite::MlDsa65 => "ML-DSA-65 public key",
            MlDsaSuite::MlDsa87 => "ML-DSA-87 public key",
        }
    }

    const fn secret_key_kind(self) -> &'static str {
        match self {
            MlDsaSuite::MlDsa44 => "ML-DSA-44 secret key",
            MlDsaSuite::MlDsa65 => "ML-DSA-65 secret key",
            MlDsaSuite::MlDsa87 => "ML-DSA-87 secret key",
        }
    }

    const fn signature_kind(self) -> &'static str {
        match self {
            MlDsaSuite::MlDsa44 => "ML-DSA-44 signature",
            MlDsaSuite::MlDsa65 => "ML-DSA-65 signature",
            MlDsaSuite::MlDsa87 => "ML-DSA-87 signature",
        }
    }
}

/// ML-DSA keypair.
#[derive(Debug)]
pub struct MlDsaKeyPair {
    /// Public key bytes.
    pub public_key: Vec<u8>,
    /// Secret key bytes (zeroized on drop).
    pub secret_key: Zeroizing<Vec<u8>>,
}

impl MlDsaKeyPair {
    /// Borrow the public key bytes.
    #[must_use]
    pub fn public_key(&self) -> &[u8] {
        &self.public_key
    }

    /// Borrow the secret key bytes.
    #[must_use]
    pub fn secret_key(&self) -> &[u8] {
        &self.secret_key
    }
}

/// Detached ML-DSA signature.
#[derive(Debug, Clone)]
pub struct MlDsaSignature {
    bytes: Vec<u8>,
}

impl MlDsaSignature {
    fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    /// Access raw signature bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Error raised by ML-DSA helpers.
#[derive(Clone, Debug, Error)]
pub enum MlDsaError {
    /// Encoding length mismatch.
    #[error(transparent)]
    BadEncoding(Box<MlDsaEncodingError>),
    /// Signature verification failed.
    #[error("signature verification failed: {0}")]
    VerificationFailed(VerificationError),
    /// Key generation failed.
    #[error("{suite:?} key generation failed with status {status}")]
    KeyGenerationFailed {
        /// Suite identifier.
        suite: MlDsaSuite,
        /// Status code returned by `PQClean`.
        status: i32,
    },
    /// Context exceeded the FIPS 204 one-byte context length field.
    #[error("ML-DSA context length must be at most 255 bytes, found {len}")]
    ContextTooLong {
        /// Actual context length in bytes.
        len: usize,
    },
    /// Hedged RNG seed construction failed.
    #[error(transparent)]
    Rng(#[from] RngError),
}

impl MlDsaError {
    fn bad_encoding(kind: &'static str, err: PqError) -> Self {
        MlDsaError::BadEncoding(Box::new(MlDsaEncodingError { kind, source: err }))
    }
}

fn validate_mldsa_public_key_len(suite: MlDsaSuite, bytes: &[u8]) -> Result<(), MlDsaError> {
    validate_mldsa_len(suite.public_key_kind(), bytes.len(), suite.public_key_len())
}

fn validate_mldsa_secret_key_len(suite: MlDsaSuite, bytes: &[u8]) -> Result<(), MlDsaError> {
    validate_mldsa_len(suite.secret_key_kind(), bytes.len(), suite.secret_key_len())
}

fn validate_mldsa_signature_len(suite: MlDsaSuite, bytes: &[u8]) -> Result<(), MlDsaError> {
    validate_mldsa_len(suite.signature_kind(), bytes.len(), suite.signature_len())
}

fn validate_mldsa_len(
    kind: &'static str,
    actual: usize,
    expected: usize,
) -> Result<(), MlDsaError> {
    if actual == expected {
        Ok(())
    } else {
        Err(MlDsaError::bad_encoding(
            kind,
            PqError::BadLength {
                name: kind,
                actual,
                expected,
            },
        ))
    }
}

#[derive(Clone, Copy, Debug, Error)]
#[error("invalid {kind} encoding: {source}")]
pub struct MlDsaEncodingError {
    /// Field identifier.
    kind: &'static str,
    /// Underlying `PQClean` error.
    #[source]
    source: PqError,
}

/// Generate an ML-DSA keypair.
///
/// # Errors
///
/// Returns [`MlDsaError::KeyGenerationFailed`] if the underlying `PQClean`
/// routines report a failure, or [`MlDsaError::BadEncoding`] when the produced
/// key material cannot be converted into the Norito-friendly encoding.
pub fn generate_mldsa_keypair(
    suite: MlDsaSuite,
    rng: &mut HedgedChaCha20Rng,
) -> Result<MlDsaKeyPair, MlDsaError> {
    let mut coins = Zeroizing::new([0u8; 32]);
    rng.fill_bytes(coins.as_mut());
    backend::generate_keypair(suite, &coins)
}

/// Generate an ML-DSA keypair using a seed plus live OS entropy when available.
///
/// # Errors
/// Returns [`MlDsaError::Rng`] when the initial OS seed draw fails.
pub fn generate_mldsa_keypair_from_os(suite: MlDsaSuite) -> Result<MlDsaKeyPair, MlDsaError> {
    let mut rng = hedged_chacha20_rng_from_os(b"soranet-pq:mldsa:keypair")?;
    generate_mldsa_keypair(suite, &mut rng)
}

/// Deterministically generate an ML-DSA keypair from explicit seed material.
///
/// # Errors
/// Returns backend encoding errors.
pub fn generate_mldsa_keypair_from_seed(
    suite: MlDsaSuite,
    seed: HedgedRngSeed,
    personalization: &[u8],
) -> Result<MlDsaKeyPair, MlDsaError> {
    let mut rng = deterministic_chacha20_rng(seed, personalization);
    generate_mldsa_keypair(suite, &mut rng)
}

/// Create a detached signature over `message` using the provided secret key.
///
/// # Errors
/// Returns an error when the secret key or signature encoding is invalid.
pub fn sign_mldsa(
    suite: MlDsaSuite,
    secret_key: &[u8],
    context: &[u8],
    message: &[u8],
    rng: &mut HedgedChaCha20Rng,
) -> Result<MlDsaSignature, MlDsaError> {
    let mut coins = Zeroizing::new([0u8; 32]);
    rng.fill_bytes(coins.as_mut());
    backend::sign(suite, secret_key, context, message, &coins).map(MlDsaSignature::new)
}

/// Sign using fresh OS seed material plus live OS entropy when available.
///
/// # Errors
/// Returns [`MlDsaError::Rng`] when the initial OS seed draw fails, or a backend
/// error when the secret-key encoding is invalid.
pub fn sign_mldsa_from_os(
    suite: MlDsaSuite,
    secret_key: &[u8],
    context: &[u8],
    message: &[u8],
) -> Result<MlDsaSignature, MlDsaError> {
    let mut rng = hedged_chacha20_rng_from_os(b"soranet-pq:mldsa:sign")?;
    sign_mldsa(suite, secret_key, context, message, &mut rng)
}

/// Verify a detached signature.
///
/// # Errors
/// Returns an error when the public key or signature encoding is invalid or verification fails.
pub fn verify_mldsa(
    suite: MlDsaSuite,
    public_key: &[u8],
    context: &[u8],
    message: &[u8],
    signature: &[u8],
) -> Result<(), MlDsaError> {
    if context.len() > ML_DSA_CONTEXT_MAX_LEN {
        return Err(MlDsaError::ContextTooLong { len: context.len() });
    }
    validate_mldsa_public_key_len(suite, public_key)?;
    validate_mldsa_signature_len(suite, signature)?;
    match suite {
        MlDsaSuite::MlDsa44 => {
            let pk = mldsa44::PublicKey::from_bytes(public_key)
                .map_err(|err| MlDsaError::bad_encoding("ML-DSA-44 public key", err))?;
            let sig = mldsa44::DetachedSignature::from_bytes(signature)
                .map_err(|err| MlDsaError::bad_encoding("ML-DSA-44 signature", err))?;
            mldsa44::verify_detached_signature_ctx(&sig, message, context, &pk)
                .map_err(MlDsaError::VerificationFailed)
        }
        MlDsaSuite::MlDsa65 => {
            let pk = mldsa65::PublicKey::from_bytes(public_key)
                .map_err(|err| MlDsaError::bad_encoding("ML-DSA-65 public key", err))?;
            let sig = mldsa65::DetachedSignature::from_bytes(signature)
                .map_err(|err| MlDsaError::bad_encoding("ML-DSA-65 signature", err))?;
            mldsa65::verify_detached_signature_ctx(&sig, message, context, &pk)
                .map_err(MlDsaError::VerificationFailed)
        }
        MlDsaSuite::MlDsa87 => {
            let pk = mldsa87::PublicKey::from_bytes(public_key)
                .map_err(|err| MlDsaError::bad_encoding("ML-DSA-87 public key", err))?;
            let sig = mldsa87::DetachedSignature::from_bytes(signature)
                .map_err(|err| MlDsaError::bad_encoding("ML-DSA-87 signature", err))?;
            mldsa87::verify_detached_signature_ctx(&sig, message, context, &pk)
                .map_err(MlDsaError::VerificationFailed)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{deterministic_chacha20_rng, hedged_chacha20_rng};

    use super::*;

    fn signed_roundtrip(suite: MlDsaSuite) {
        let mut rng = hedged_chacha20_rng(
            HedgedRngSeed::from_entropy([suite.suite_id(); 32]),
            b"mldsa-test-keypair",
        );
        let mut sign_rng = hedged_chacha20_rng(
            HedgedRngSeed::from_entropy([suite.suite_id().wrapping_add(1); 32]),
            b"mldsa-test-sign",
        );
        let kp = generate_mldsa_keypair(suite, &mut rng)
            .expect("ML-DSA keypair generation should succeed");
        let message = b"SoraNet PQ harness";
        let context = b"soranet-pq:test";
        let signature =
            sign_mldsa(suite, kp.secret_key(), context, message, &mut sign_rng).unwrap();
        verify_mldsa(
            suite,
            kp.public_key(),
            context,
            message,
            signature.as_bytes(),
        )
        .unwrap();
    }

    #[test]
    fn roundtrip_44() {
        signed_roundtrip(MlDsaSuite::MlDsa44);
    }

    #[test]
    fn roundtrip_65() {
        signed_roundtrip(MlDsaSuite::MlDsa65);
    }

    #[test]
    fn roundtrip_87() {
        signed_roundtrip(MlDsaSuite::MlDsa87);
    }

    #[test]
    fn from_os_helpers_sign_and_verify() {
        let suite = MlDsaSuite::MlDsa44;
        let keypair =
            generate_mldsa_keypair_from_os(suite).expect("OS-backed ML-DSA keypair generation");
        let message = b"OS-backed ML-DSA signing";
        let signature = sign_mldsa_from_os(suite, keypair.secret_key(), b"", message)
            .expect("OS-backed ML-DSA signing");

        verify_mldsa(
            suite,
            keypair.public_key(),
            b"",
            message,
            signature.as_bytes(),
        )
        .expect("OS-backed ML-DSA signature verifies");
    }

    #[test]
    fn seeded_keypair_is_deterministic() {
        for suite in [
            MlDsaSuite::MlDsa44,
            MlDsaSuite::MlDsa65,
            MlDsaSuite::MlDsa87,
        ] {
            let seed = HedgedRngSeed::from_entropy([suite.suite_id().wrapping_add(0xD0); 32]);
            let first = generate_mldsa_keypair_from_seed(suite, seed.clone(), b"seeded-keygen")
                .expect("seeded keypair generation should succeed");
            let second = generate_mldsa_keypair_from_seed(suite, seed, b"seeded-keygen")
                .expect("seeded keypair generation should succeed");

            assert_eq!(first.public_key(), second.public_key());
            assert_eq!(first.secret_key(), second.secret_key());
        }
    }

    #[test]
    fn seeded_keypair_personalization_changes_output() {
        for suite in [
            MlDsaSuite::MlDsa44,
            MlDsaSuite::MlDsa65,
            MlDsaSuite::MlDsa87,
        ] {
            let seed = HedgedRngSeed::from_entropy([suite.suite_id().wrapping_add(0xD8); 32]);
            let first = generate_mldsa_keypair_from_seed(suite, seed.clone(), b"seeded-keygen-a")
                .expect("seeded keypair generation should succeed");
            let second = generate_mldsa_keypair_from_seed(suite, seed, b"seeded-keygen-b")
                .expect("seeded keypair generation should succeed");

            assert_ne!(first.public_key(), second.public_key());
            assert_ne!(first.secret_key(), second.secret_key());
        }
    }

    #[test]
    fn deterministic_signing_replays_with_same_seed() {
        let suite = MlDsaSuite::MlDsa65;
        let keypair = generate_mldsa_keypair_from_seed(
            suite,
            HedgedRngSeed::from_entropy([0xE1; 32]),
            b"deterministic-sign-keygen",
        )
        .expect("seeded keypair generation should succeed");
        let mut first_rng = deterministic_chacha20_rng(
            HedgedRngSeed::from_entropy([0xE2; 32]),
            b"deterministic-sign",
        );
        let mut second_rng = deterministic_chacha20_rng(
            HedgedRngSeed::from_entropy([0xE2; 32]),
            b"deterministic-sign",
        );
        let context = b"soranet-pq:deterministic-sign";
        let message = b"repeatable ML-DSA signature";

        let first = sign_mldsa(
            suite,
            keypair.secret_key(),
            context,
            message,
            &mut first_rng,
        )
        .expect("signature succeeds");
        let second = sign_mldsa(
            suite,
            keypair.secret_key(),
            context,
            message,
            &mut second_rng,
        )
        .expect("signature succeeds");

        assert_eq!(first.as_bytes(), second.as_bytes());
        verify_mldsa(
            suite,
            keypair.public_key(),
            context,
            message,
            first.as_bytes(),
        )
        .expect("deterministic signature verifies");
    }

    #[test]
    fn signing_personalization_changes_signature() {
        let suite = MlDsaSuite::MlDsa65;
        let keypair = generate_mldsa_keypair_from_seed(
            suite,
            HedgedRngSeed::from_entropy([0xEA; 32]),
            b"personalized-sign-keygen",
        )
        .expect("seeded keypair generation should succeed");
        let mut first_rng = deterministic_chacha20_rng(
            HedgedRngSeed::from_entropy([0xEB; 32]),
            b"personalized-sign-a",
        );
        let mut second_rng = deterministic_chacha20_rng(
            HedgedRngSeed::from_entropy([0xEB; 32]),
            b"personalized-sign-b",
        );
        let context = b"soranet-pq:personalized-sign";
        let message = b"same message and key, different signing coins";

        let first = sign_mldsa(
            suite,
            keypair.secret_key(),
            context,
            message,
            &mut first_rng,
        )
        .expect("first signature succeeds");
        let second = sign_mldsa(
            suite,
            keypair.secret_key(),
            context,
            message,
            &mut second_rng,
        )
        .expect("second signature succeeds");

        assert_ne!(first.as_bytes(), second.as_bytes());
        verify_mldsa(
            suite,
            keypair.public_key(),
            context,
            message,
            first.as_bytes(),
        )
        .expect("first signature verifies");
        verify_mldsa(
            suite,
            keypair.public_key(),
            context,
            message,
            second.as_bytes(),
        )
        .expect("second signature verifies");
    }

    #[test]
    fn generated_keypairs_and_signatures_match_suite_lengths() {
        for suite in [
            MlDsaSuite::MlDsa44,
            MlDsaSuite::MlDsa65,
            MlDsaSuite::MlDsa87,
        ] {
            let keypair = generate_mldsa_keypair_from_seed(
                suite,
                HedgedRngSeed::from_entropy([suite.suite_id().wrapping_add(0xEC); 32]),
                b"length-keygen",
            )
            .expect("seeded keypair generation should succeed");
            let mut rng = deterministic_chacha20_rng(
                HedgedRngSeed::from_entropy([suite.suite_id().wrapping_add(0xEF); 32]),
                b"length-sign",
            );
            let signature = sign_mldsa(suite, keypair.secret_key(), b"", b"length-check", &mut rng)
                .expect("signature succeeds");

            assert_eq!(keypair.public_key().len(), suite.public_key_len());
            assert_eq!(keypair.secret_key().len(), suite.secret_key_len());
            assert_eq!(signature.as_bytes().len(), suite.signature_len());
        }
    }

    #[test]
    fn sign_rejects_invalid_secret_key_length() {
        let mut rng = deterministic_chacha20_rng(
            HedgedRngSeed::from_entropy([0xE3; 32]),
            b"invalid-secret-sign",
        );
        let err = sign_mldsa(MlDsaSuite::MlDsa44, &[0u8; 8], b"", b"message", &mut rng)
            .expect_err("short secret must fail");

        match err {
            MlDsaError::BadEncoding(err) => assert!(err.kind.contains("secret key")),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn verify_rejects_invalid_public_key_and_signature_lengths() {
        let suite = MlDsaSuite::MlDsa44;
        let keypair = generate_mldsa_keypair_from_seed(
            suite,
            HedgedRngSeed::from_entropy([0xE4; 32]),
            b"invalid-verify-keygen",
        )
        .expect("seeded keypair generation should succeed");

        let err = verify_mldsa(suite, &[0u8; 8], b"", b"message", &[0u8; 1])
            .expect_err("short public key must fail");
        match err {
            MlDsaError::BadEncoding(err) => assert!(err.kind.contains("public key")),
            other => panic!("unexpected error: {other:?}"),
        }

        let err = verify_mldsa(suite, keypair.public_key(), b"", b"message", &[0u8; 8])
            .expect_err("short signature must fail");
        match err {
            MlDsaError::BadEncoding(err) => assert!(err.kind.contains("signature")),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn bad_encoding_display_includes_kind_and_lengths() {
        let err = verify_mldsa(MlDsaSuite::MlDsa65, &[0u8; 9], b"", b"message", &[0u8; 8])
            .expect_err("short public key must fail before signature decoding");
        let rendered = err.to_string();

        assert!(rendered.contains("ML-DSA-65 public key"));
        assert!(rendered.contains('9'));
        assert!(rendered.contains("1952"));
    }

    #[test]
    fn max_context_length_signs_and_verifies() {
        let suite = MlDsaSuite::MlDsa44;
        let keypair = generate_mldsa_keypair_from_seed(
            suite,
            HedgedRngSeed::from_entropy([0xE5; 32]),
            b"max-context-keygen",
        )
        .expect("seeded keypair generation should succeed");
        let mut rng = deterministic_chacha20_rng(
            HedgedRngSeed::from_entropy([0xE6; 32]),
            b"max-context-sign",
        );
        let context = vec![0xA5; ML_DSA_CONTEXT_MAX_LEN];
        let message = b"maximum context length";

        let signature = sign_mldsa(suite, keypair.secret_key(), &context, message, &mut rng)
            .expect("max context signs");
        verify_mldsa(
            suite,
            keypair.public_key(),
            &context,
            message,
            signature.as_bytes(),
        )
        .expect("max context verifies");
    }

    #[test]
    fn empty_context_and_message_sign_and_verify() {
        let suite = MlDsaSuite::MlDsa87;
        let keypair = generate_mldsa_keypair_from_seed(
            suite,
            HedgedRngSeed::from_entropy([0xF4; 32]),
            b"empty-message-keygen",
        )
        .expect("seeded keypair generation should succeed");
        let mut rng = deterministic_chacha20_rng(
            HedgedRngSeed::from_entropy([0xF5; 32]),
            b"empty-message-sign",
        );

        let signature = sign_mldsa(suite, keypair.secret_key(), b"", b"", &mut rng)
            .expect("empty message signs");

        verify_mldsa(suite, keypair.public_key(), b"", b"", signature.as_bytes())
            .expect("empty message verifies");
    }

    #[test]
    fn suite_ids_roundtrip_and_reject_unknown() {
        for suite in [
            MlDsaSuite::MlDsa44,
            MlDsaSuite::MlDsa65,
            MlDsaSuite::MlDsa87,
        ] {
            assert!(suite.public_key_len() > 0);
            assert!(suite.secret_key_len() > suite.public_key_len());
            assert!(suite.signature_len() > 0);
            assert!(matches!(
                MlDsaSuite::from_suite_id(suite.suite_id()),
                Some(recovered) if recovered.suite_id() == suite.suite_id()
            ));
        }

        assert!(MlDsaSuite::from_suite_id(0xFF).is_none());
    }

    #[test]
    fn context_length_is_checked_before_sign_and_verify() {
        let mut rng = hedged_chacha20_rng(
            HedgedRngSeed::from_entropy([0x87; 32]),
            b"mldsa-context-limit-keypair",
        );
        let mut sign_rng = hedged_chacha20_rng(
            HedgedRngSeed::from_entropy([0x88; 32]),
            b"mldsa-context-limit-sign",
        );
        let kp = generate_mldsa_keypair(MlDsaSuite::MlDsa44, &mut rng)
            .expect("ML-DSA keypair generation should succeed");
        let context = vec![0u8; ML_DSA_CONTEXT_MAX_LEN + 1];
        let sign_err = sign_mldsa(
            MlDsaSuite::MlDsa44,
            kp.secret_key(),
            &context,
            b"message",
            &mut sign_rng,
        )
        .unwrap_err();
        assert!(matches!(
            sign_err,
            MlDsaError::ContextTooLong {
                len
            } if len == ML_DSA_CONTEXT_MAX_LEN + 1
        ));

        let verify_err = verify_mldsa(
            MlDsaSuite::MlDsa44,
            kp.public_key(),
            &context,
            b"message",
            &[0u8; 1],
        )
        .unwrap_err();
        assert!(matches!(
            verify_err,
            MlDsaError::ContextTooLong {
                len
            } if len == ML_DSA_CONTEXT_MAX_LEN + 1
        ));
    }

    #[test]
    fn sign_rejects_long_context_before_bad_secret_length() {
        let mut sign_rng = deterministic_chacha20_rng(
            HedgedRngSeed::from_entropy([0xF0; 32]),
            b"mldsa-context-before-secret",
        );
        let context = vec![0u8; ML_DSA_CONTEXT_MAX_LEN + 1];

        let err = sign_mldsa(
            MlDsaSuite::MlDsa44,
            &[],
            &context,
            b"message",
            &mut sign_rng,
        )
        .expect_err("context length must be checked before secret length");

        assert!(matches!(
            err,
            MlDsaError::ContextTooLong {
                len
            } if len == ML_DSA_CONTEXT_MAX_LEN + 1
        ));
    }

    #[test]
    fn verify_rejects_long_context_before_bad_key_lengths() {
        let context = vec![0u8; ML_DSA_CONTEXT_MAX_LEN + 1];
        let err = verify_mldsa(MlDsaSuite::MlDsa44, &[], &context, b"message", &[])
            .expect_err("context length must be checked first");

        assert!(matches!(
            err,
            MlDsaError::ContextTooLong {
                len
            } if len == ML_DSA_CONTEXT_MAX_LEN + 1
        ));
    }

    #[test]
    fn context_too_long_display_includes_length() {
        let err = MlDsaError::ContextTooLong { len: 300 };

        assert_eq!(
            err.to_string(),
            "ML-DSA context length must be at most 255 bytes, found 300"
        );
    }

    #[test]
    fn reject_modified_message() {
        let mut rng = hedged_chacha20_rng(
            HedgedRngSeed::from_entropy([0x44; 32]),
            b"mldsa-modified-keypair",
        );
        let mut sign_rng = hedged_chacha20_rng(
            HedgedRngSeed::from_entropy([0x45; 32]),
            b"mldsa-modified-sign",
        );
        let kp = generate_mldsa_keypair(MlDsaSuite::MlDsa44, &mut rng)
            .expect("ML-DSA keypair generation should succeed");
        let message = b"context";
        let context = b"soranet-pq:test";
        let signature = sign_mldsa(
            MlDsaSuite::MlDsa44,
            kp.secret_key(),
            context,
            message,
            &mut sign_rng,
        )
        .unwrap();
        let mut tampered = message.to_vec();
        tampered[0] ^= 0xFF;
        let err = verify_mldsa(
            MlDsaSuite::MlDsa44,
            kp.public_key(),
            context,
            &tampered,
            signature.as_bytes(),
        )
        .unwrap_err();

        match err {
            MlDsaError::VerificationFailed(VerificationError::InvalidSignature) => {}
            _ => panic!("unexpected error"),
        }
    }

    #[test]
    fn reject_modified_signature() {
        let suite = MlDsaSuite::MlDsa65;
        let keypair = generate_mldsa_keypair_from_seed(
            suite,
            HedgedRngSeed::from_entropy([0xF6; 32]),
            b"modified-signature-keypair",
        )
        .expect("seeded keypair generation should succeed");
        let mut sign_rng = deterministic_chacha20_rng(
            HedgedRngSeed::from_entropy([0xF7; 32]),
            b"modified-signature-sign",
        );
        let message = b"signature tamper target";
        let mut signature = sign_mldsa(suite, keypair.secret_key(), b"", message, &mut sign_rng)
            .expect("signature succeeds")
            .as_bytes()
            .to_vec();
        signature[0] ^= 0x01;

        let err = verify_mldsa(suite, keypair.public_key(), b"", message, &signature)
            .expect_err("modified signature must fail");

        assert!(matches!(
            err,
            MlDsaError::VerificationFailed(VerificationError::InvalidSignature)
        ));
    }

    #[test]
    fn reject_wrong_public_key() {
        let suite = MlDsaSuite::MlDsa65;
        let signing_keypair = generate_mldsa_keypair_from_seed(
            suite,
            HedgedRngSeed::from_entropy([0xF1; 32]),
            b"wrong-public-key-signing",
        )
        .expect("signing keypair generation should succeed");
        let verifying_keypair = generate_mldsa_keypair_from_seed(
            suite,
            HedgedRngSeed::from_entropy([0xF2; 32]),
            b"wrong-public-key-verifying",
        )
        .expect("verifying keypair generation should succeed");
        let mut sign_rng = deterministic_chacha20_rng(
            HedgedRngSeed::from_entropy([0xF3; 32]),
            b"wrong-public-key-sign",
        );
        let message = b"wrong public key must not verify";
        let context = b"soranet-pq:test";
        let signature = sign_mldsa(
            suite,
            signing_keypair.secret_key(),
            context,
            message,
            &mut sign_rng,
        )
        .expect("signature succeeds");

        let err = verify_mldsa(
            suite,
            verifying_keypair.public_key(),
            context,
            message,
            signature.as_bytes(),
        )
        .expect_err("signature must fail under a different public key");

        assert!(matches!(
            err,
            MlDsaError::VerificationFailed(VerificationError::InvalidSignature)
        ));
    }

    #[test]
    fn context_is_domain_separating() {
        let mut rng = hedged_chacha20_rng(
            HedgedRngSeed::from_entropy([0x65; 32]),
            b"mldsa-context-keypair",
        );
        let mut sign_rng = hedged_chacha20_rng(
            HedgedRngSeed::from_entropy([0x66; 32]),
            b"mldsa-context-sign",
        );
        let kp = generate_mldsa_keypair(MlDsaSuite::MlDsa65, &mut rng)
            .expect("ML-DSA keypair generation should succeed");
        let message = b"context-bound message";
        let signature = sign_mldsa(
            MlDsaSuite::MlDsa65,
            kp.secret_key(),
            b"context-a",
            message,
            &mut sign_rng,
        )
        .unwrap();

        assert!(
            verify_mldsa(
                MlDsaSuite::MlDsa65,
                kp.public_key(),
                b"context-b",
                message,
                signature.as_bytes(),
            )
            .is_err()
        );
    }
}
