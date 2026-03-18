//! Stream token schema and helpers for SoraFS chunk-range gateways.

use blake3::Hash as Blake3Hash;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use norito::derive::{NoritoDeserialize, NoritoSerialize};
use thiserror::Error;

/// Canonical body for stream tokens issued by gateways.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct StreamTokenBodyV1 {
    pub token_id: String,
    pub manifest_cid: Vec<u8>,
    pub provider_id: [u8; 32],
    pub profile_handle: String,
    pub max_streams: u16,
    pub ttl_epoch: u64,
    pub rate_limit_bytes: u64,
    pub issued_at: u64,
    pub requests_per_minute: u32,
    pub token_pk_version: u32,
}

impl StreamTokenBodyV1 {
    /// Serialises the body into canonical Norito bytes suitable for signing.
    pub fn to_canonical_bytes(&self) -> Result<Vec<u8>, norito::Error> {
        norito::to_bytes(self)
    }
}

/// Signed stream token payload.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct StreamTokenV1 {
    pub body: StreamTokenBodyV1,
    pub signature: Vec<u8>,
}

impl StreamTokenV1 {
    /// Sign the provided body with the Ed25519 signing key.
    pub fn sign(body: StreamTokenBodyV1, secret: &SigningKey) -> Result<Self, StreamTokenError> {
        let message = body.to_canonical_bytes()?;
        let signature = secret.sign(&message);
        Ok(Self {
            body,
            signature: signature.to_bytes().to_vec(),
        })
    }

    /// Verify the token signature using the supplied verifying key.
    pub fn verify(&self, verifier: &VerifyingKey) -> Result<(), StreamTokenError> {
        let sig = Signature::try_from(self.signature.as_slice())
            .map_err(|_| StreamTokenError::InvalidSignatureFormat)?;
        let message = self.body.to_canonical_bytes()?;
        verifier
            .verify_strict(&message, &sig)
            .map_err(StreamTokenError::SignatureInvalid)
    }

    /// Compute the canonical hash of the token body for logging or caching.
    pub fn body_hash(&self) -> Result<Blake3Hash, StreamTokenError> {
        let bytes = self.body.to_canonical_bytes()?;
        Ok(blake3::hash(&bytes))
    }
}

/// Errors produced while handling stream tokens.
#[derive(Debug, Error)]
pub enum StreamTokenError {
    /// Failed to serialise or deserialise the Norito payload.
    #[error("norito serialisation error: {0}")]
    Norito(#[from] norito::Error),
    /// Signature bytes were malformed.
    #[error("invalid signature encoding")]
    InvalidSignatureFormat,
    /// Signature verification failed.
    #[error("stream token signature invalid: {0}")]
    SignatureInvalid(ed25519_dalek::SignatureError),
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::SigningKey;

    use super::*;

    fn sample_body() -> StreamTokenBodyV1 {
        StreamTokenBodyV1 {
            token_id: "01J3E4ZCMQ3GP2H3R5PSNF6Z7X".to_string(),
            manifest_cid: vec![0x01, 0x55, 0x01],
            provider_id: [0xAA; 32],
            profile_handle: "sorafs.sf1@1.0.0".to_string(),
            max_streams: 4,
            ttl_epoch: 1_731_234_567,
            rate_limit_bytes: 10 * 1024 * 1024,
            issued_at: 1_731_234_000,
            requests_per_minute: 120,
            token_pk_version: 3,
        }
    }

    #[test]
    fn sign_and_verify_roundtrip() {
        let signing = SigningKey::from_bytes(&[0x42; 32]);
        let verifying = signing.verifying_key();
        let body = sample_body();
        let token = StreamTokenV1::sign(body.clone(), &signing).expect("sign");
        token.verify(&verifying).expect("verify");
        assert_eq!(token.body, body);
        let hash = token.body_hash().expect("hash");
        let bytes = body.to_canonical_bytes().expect("bytes");
        assert_eq!(hash.as_bytes(), blake3::hash(&bytes).as_bytes());
    }

    #[test]
    fn verify_rejects_modified_body() {
        let signing = SigningKey::from_bytes(&[0x24; 32]);
        let verifying = signing.verifying_key();
        let token = StreamTokenV1::sign(sample_body(), &signing).expect("sign");
        let mut tampered = token.clone();
        tampered.body.max_streams = 8;
        let err = tampered.verify(&verifying).expect_err("should fail");
        matches!(err, StreamTokenError::SignatureInvalid(_));
    }
}
