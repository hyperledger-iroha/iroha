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
use rand::{CryptoRng, RngCore};
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
    /// Envelope version is not supported by this helper.
    #[error("unsupported hybrid payload envelope version {0}")]
    UnsupportedVersion(u8),
}

/// Encrypt bytes into a hybrid payload envelope using the default suite.
pub fn encrypt_payload<R: CryptoRng + RngCore>(
    payload: &[u8],
    aad: &[u8],
    recipient: &HybridPublicKey,
    rng: &mut R,
) -> Result<HybridPayloadEnvelopeV1, HybridEnvelopeError> {
    let suite = HybridSuite::X25519MlKem768ChaCha20Poly1305;
    let (kem_ciphertext, derived) = hybrid_encapsulate(suite, recipient, rng)?;

    let mut nonce_bytes = [0_u8; 12];
    rng.fill_bytes(&mut nonce_bytes);
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
    use rand_chacha::ChaCha20Rng;

    use super::*;

    #[test]
    fn envelope_roundtrip() {
        let mut rng = ChaCha20Rng::from_seed([0x55; 32]);
        let pair = HybridKeyPair::generate(&mut rng);
        let payload = b"hybrid manifest payload".to_vec();
        let aad = b"sorafs:manifest:test";

        let envelope =
            encrypt_payload(&payload, aad, pair.public(), &mut rng).expect("encryption succeeds");
        let decrypted =
            decrypt_payload(&envelope, aad, pair.secret()).expect("decryption succeeds");

        assert_eq!(decrypted, payload);
    }

    #[test]
    fn decrypt_with_incorrect_aad_fails() {
        let mut rng = ChaCha20Rng::from_seed([0x11; 32]);
        let pair = HybridKeyPair::generate(&mut rng);
        let payload = b"hybrid manifest payload".to_vec();

        let envelope =
            encrypt_payload(&payload, b"aad-ok", pair.public(), &mut rng).expect("encrypt ok");
        let result = decrypt_payload(&envelope, b"aad-wrong", pair.secret());
        assert!(matches!(result, Err(HybridEnvelopeError::AeadFailure)));
    }
}
