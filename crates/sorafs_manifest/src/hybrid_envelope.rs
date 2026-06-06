//! Hybrid payload envelope for SoraFS manifests and chunk payloads (SF-4b).
//!
//! This module wires the `iroha_crypto::hybrid` primitives into a Norito-serialisable
//! envelope so manifests can be sealed with a hybrid X25519 + ML-KEM-768 exchange
//! and ChaCha20-Poly1305 DEM. Gateways and SDKs use the helpers exposed here as
//! part of the `sorafs_manifest_stub` CLI and Torii publishing flows to wrap and
//! unwrap manifest payloads.

use std::str::FromStr;

use chacha20poly1305::{
    ChaCha20Poly1305, KeyInit as _,
    aead::{Aead as _, Payload},
};
use iroha_crypto::{
    HybridError, HybridKemCiphertext, HybridPublicKey, HybridSecretKey, HybridSuite,
    hybrid_decapsulate, hybrid_encapsulate,
};
use norito::derive::{JsonSerialize, NoritoDeserialize, NoritoSerialize};
use rand::rand_core::TryCryptoRng;
use thiserror::Error;

/// Envelope schema version.
pub const HYBRID_PAYLOAD_ENVELOPE_VERSION_V1: u8 = 1;

/// Norito-serialisable KEM bundle containing the sender's ephemeral keys.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
pub struct HybridKemBundleV1 {
    pub ephemeral_public: Vec<u8>,
    pub kyber_ciphertext: Vec<u8>,
}

/// Norito-serialisable payload envelope.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
pub struct HybridPayloadEnvelopeV1 {
    pub version: u8,
    pub suite: String,
    pub kem: HybridKemBundleV1,
    pub nonce: [u8; 12],
    pub ciphertext: Vec<u8>,
}

/// Errors that can occur while producing or consuming envelopes.
#[derive(Debug, Error)]
pub enum HybridEnvelopeError {
    /// Failure while handling the underlying hybrid key material.
    #[error(transparent)]
    Hybrid(#[from] HybridError),
    /// Unknown or unsupported suite identifier.
    #[error("unsupported hybrid suite `{0}`")]
    UnsupportedSuite(String),
    /// AEAD sealing or opening failed.
    #[error("chacha20-poly1305 operation failed")]
    AeadFailure,
    /// Random byte generation failed while producing the envelope.
    #[error("random byte generation failed while {operation}: {message}")]
    RandomBytes {
        /// Operation that requested random bytes.
        operation: &'static str,
        /// Underlying RNG error message.
        message: String,
    },
    /// Envelope version is not supported by this helper.
    #[error("unsupported hybrid payload envelope version {0}")]
    UnsupportedVersion(u8),
}

/// Encrypt bytes into a hybrid payload envelope using the default suite.
pub fn encrypt_payload<R: TryCryptoRng>(
    payload: &[u8],
    aad: &[u8],
    recipient: &HybridPublicKey,
    rng: &mut R,
) -> Result<HybridPayloadEnvelopeV1, HybridEnvelopeError> {
    let suite = HybridSuite::X25519MlKem768ChaCha20Poly1305;
    let (kem_ciphertext, derived) = hybrid_encapsulate(suite, recipient, rng)?;

    let mut nonce_bytes = [0_u8; 12];
    fill_random(rng, "generating hybrid payload nonce", &mut nonce_bytes)?;
    let nonce = chacha20poly1305::Nonce::from(nonce_bytes);
    let key = chacha20poly1305::Key::from(derived.encryption_key());
    let cipher = ChaCha20Poly1305::new(&key);
    let ciphertext = cipher
        .encrypt(&nonce, Payload { msg: payload, aad })
        .map_err(|_| HybridEnvelopeError::AeadFailure)?;

    Ok(HybridPayloadEnvelopeV1 {
        version: HYBRID_PAYLOAD_ENVELOPE_VERSION_V1,
        suite: suite.to_string(),
        kem: HybridKemBundleV1 {
            ephemeral_public: kem_ciphertext.ephemeral_public().to_vec(),
            kyber_ciphertext: kem_ciphertext.kyber_ciphertext().to_vec(),
        },
        nonce: nonce_bytes,
        ciphertext,
    })
}

fn fill_random<R: TryCryptoRng>(
    rng: &mut R,
    operation: &'static str,
    dest: &mut [u8],
) -> Result<(), HybridEnvelopeError> {
    rng.try_fill_bytes(dest)
        .map_err(|err| HybridEnvelopeError::RandomBytes {
            operation,
            message: err.to_string(),
        })
}

/// Decrypt a hybrid payload envelope with the provided recipient keys.
pub fn decrypt_payload(
    envelope: &HybridPayloadEnvelopeV1,
    aad: &[u8],
    recipient: &HybridSecretKey,
) -> Result<Vec<u8>, HybridEnvelopeError> {
    if envelope.version != HYBRID_PAYLOAD_ENVELOPE_VERSION_V1 {
        return Err(HybridEnvelopeError::UnsupportedVersion(envelope.version));
    }
    let suite = HybridSuite::from_str(&envelope.suite)
        .map_err(|_| HybridEnvelopeError::UnsupportedSuite(envelope.suite.clone()))?;
    let kem_ciphertext = HybridKemCiphertext::from_parts(
        &envelope.kem.ephemeral_public,
        &envelope.kem.kyber_ciphertext,
    )?;
    let derived = hybrid_decapsulate(suite, &kem_ciphertext, recipient)?;

    let key = chacha20poly1305::Key::from(derived.encryption_key());
    let cipher = ChaCha20Poly1305::new(&key);
    let nonce = chacha20poly1305::Nonce::from(envelope.nonce);
    cipher
        .decrypt(
            &nonce,
            Payload {
                msg: &envelope.ciphertext,
                aad,
            },
        )
        .map_err(|_| HybridEnvelopeError::AeadFailure)
}

#[cfg(test)]
mod tests {
    use iroha_crypto::HybridKeyPair;
    use rand::SeedableRng as _;
    use rand::rand_core::{TryCryptoRng, TryRngCore};
    use rand_chacha::ChaCha20Rng;

    use super::*;

    struct FailingAfterFills {
        remaining_ok_fills: usize,
    }

    #[derive(Debug)]
    struct FailingAfterFillsError;

    impl std::fmt::Display for FailingAfterFillsError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("failing hybrid envelope RNG")
        }
    }

    impl TryRngCore for FailingAfterFills {
        type Error = FailingAfterFillsError;

        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            let mut bytes = [0u8; 4];
            self.try_fill_bytes(&mut bytes)?;
            Ok(u32::from_le_bytes(bytes))
        }

        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            let mut bytes = [0u8; 8];
            self.try_fill_bytes(&mut bytes)?;
            Ok(u64::from_le_bytes(bytes))
        }

        fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Self::Error> {
            if self.remaining_ok_fills == 0 {
                return Err(FailingAfterFillsError);
            }
            self.remaining_ok_fills -= 1;
            for (idx, byte) in dst.iter_mut().enumerate() {
                *byte = 0xA5_u8.wrapping_add(idx as u8);
            }
            Ok(())
        }
    }

    impl TryCryptoRng for FailingAfterFills {}

    #[test]
    fn envelope_roundtrip() {
        let mut rng = ChaCha20Rng::from_seed([0x55; 32]);
        let pair = HybridKeyPair::generate(&mut rng).expect("generated hybrid keypair");
        let payload = b"hybrid manifest payload".to_vec();
        let aad = b"sorafs:manifest:test";

        let envelope =
            encrypt_payload(&payload, aad, pair.public(), &mut rng).expect("encryption succeeds");
        assert_eq!(envelope.version, HYBRID_PAYLOAD_ENVELOPE_VERSION_V1);
        assert_eq!(
            envelope.suite,
            "x25519-mlkem768-chacha20poly1305-transcript-v1"
        );
        let decrypted =
            decrypt_payload(&envelope, aad, pair.secret()).expect("decryption succeeds");

        assert_eq!(decrypted, payload);
    }

    #[test]
    fn decrypt_rejects_old_pre_release_suite_label() {
        let mut rng = ChaCha20Rng::from_seed([0x56; 32]);
        let pair = HybridKeyPair::generate(&mut rng).expect("generated hybrid keypair");
        let mut envelope = encrypt_payload(b"payload", b"aad", pair.public(), &mut rng)
            .expect("encryption succeeds");

        envelope.suite = "x25519-mlkem768-chacha20poly1305".to_owned();
        let err = decrypt_payload(&envelope, b"aad", pair.secret())
            .expect_err("old pre-release suite label must be rejected");

        assert!(
            matches!(err, HybridEnvelopeError::UnsupportedSuite(label) if label == "x25519-mlkem768-chacha20poly1305")
        );
    }

    #[test]
    fn decrypt_rejects_every_non_v1_envelope_version_before_suite_parsing() {
        let mut rng = ChaCha20Rng::from_seed([0x57; 32]);
        let pair = HybridKeyPair::generate(&mut rng).expect("generated hybrid keypair");
        let envelope = encrypt_payload(b"payload", b"aad", pair.public(), &mut rng)
            .expect("encryption succeeds");

        for unsupported in [0, 2, u8::MAX] {
            let mut tampered = envelope.clone();
            tampered.version = unsupported;
            tampered.suite = "x25519-mlkem768-chacha20poly1305".to_owned();

            let err = decrypt_payload(&tampered, b"aad", pair.secret())
                .expect_err("non-v1 envelope versions must be rejected");
            assert!(
                matches!(err, HybridEnvelopeError::UnsupportedVersion(version) if version == unsupported)
            );
        }
    }

    #[test]
    fn encrypt_payload_reports_nonce_rng_failure() {
        let mut key_rng = ChaCha20Rng::from_seed([0x33; 32]);
        let pair = HybridKeyPair::generate(&mut key_rng).expect("generated hybrid keypair");
        let mut rng = FailingAfterFills {
            remaining_ok_fills: 2,
        };

        let err = encrypt_payload(b"payload", b"aad", pair.public(), &mut rng)
            .expect_err("nonce RNG failure must be reported");
        match err {
            HybridEnvelopeError::RandomBytes { operation, message } => {
                assert_eq!(operation, "generating hybrid payload nonce");
                assert!(message.contains("failing hybrid envelope RNG"));
            }
            other => panic!("expected RNG failure, got {other:?}"),
        }
    }

    #[test]
    fn decrypt_with_incorrect_aad_fails() {
        let mut rng = ChaCha20Rng::from_seed([0x11; 32]);
        let pair = HybridKeyPair::generate(&mut rng).expect("generated hybrid keypair");
        let payload = b"hybrid manifest payload".to_vec();

        let envelope =
            encrypt_payload(&payload, b"aad-ok", pair.public(), &mut rng).expect("encrypt ok");
        let result = decrypt_payload(&envelope, b"aad-wrong", pair.secret());
        assert!(matches!(result, Err(HybridEnvelopeError::AeadFailure)));
    }

    #[test]
    fn decrypt_rejects_tampered_ciphertext() {
        let mut rng = ChaCha20Rng::from_seed([0x12; 32]);
        let pair = HybridKeyPair::generate(&mut rng).expect("generated hybrid keypair");
        let mut envelope =
            encrypt_payload(b"payload", b"aad", pair.public(), &mut rng).expect("encrypt ok");

        envelope.ciphertext[0] ^= 0x80;
        let err = decrypt_payload(&envelope, b"aad", pair.secret())
            .expect_err("ciphertext authentication must fail closed");
        assert!(matches!(err, HybridEnvelopeError::AeadFailure));
    }

    #[test]
    fn decrypt_rejects_tampered_nonce() {
        let mut rng = ChaCha20Rng::from_seed([0x13; 32]);
        let pair = HybridKeyPair::generate(&mut rng).expect("generated hybrid keypair");
        let mut envelope =
            encrypt_payload(b"payload", b"aad", pair.public(), &mut rng).expect("encrypt ok");

        envelope.nonce[0] ^= 0x01;
        let err = decrypt_payload(&envelope, b"aad", pair.secret())
            .expect_err("nonce authentication must fail closed");
        assert!(matches!(err, HybridEnvelopeError::AeadFailure));
    }

    #[test]
    fn decrypt_rejects_malformed_kem_fields_before_aead() {
        let mut rng = ChaCha20Rng::from_seed([0x14; 32]);
        let pair = HybridKeyPair::generate(&mut rng).expect("generated hybrid keypair");
        let envelope =
            encrypt_payload(b"payload", b"aad", pair.public(), &mut rng).expect("encrypt ok");

        let mut bad_ephemeral = envelope.clone();
        bad_ephemeral.kem.ephemeral_public.truncate(31);
        let err = decrypt_payload(&bad_ephemeral, b"aad", pair.secret())
            .expect_err("short ephemeral public key must fail");
        assert!(matches!(
            err,
            HybridEnvelopeError::Hybrid(HybridError::InvalidX25519PublicKeyLength {
                expected: 32,
                found: 31
            })
        ));

        let mut bad_ciphertext = envelope;
        bad_ciphertext.kem.kyber_ciphertext.truncate(1);
        let err = decrypt_payload(&bad_ciphertext, b"aad", pair.secret())
            .expect_err("short ML-KEM ciphertext must fail");
        assert!(matches!(
            err,
            HybridEnvelopeError::Hybrid(HybridError::InvalidKyberCiphertext)
        ));
    }
}
