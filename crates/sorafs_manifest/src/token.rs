//! Stream token schema and helpers for SoraFS chunk-range gateways.

use blake3::Hash as Blake3Hash;
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use norito::derive::{NoritoDeserialize, NoritoSerialize};
use thiserror::Error;

/// Domain separator for SoraFS v1 stream-token signatures.
pub const STREAM_TOKEN_SIGNATURE_DOMAIN_V1: &[u8] = b"sorafs.stream-token.signature.v1\0";
/// Maximum lifetime of a first-release stream token, in seconds.
pub const STREAM_TOKEN_MAX_TTL_SECS_V1: u64 = 3_600;
/// Maximum canonical Norito stream-token frame accepted in the first release.
pub const STREAM_TOKEN_MAX_WIRE_BYTES_V1: usize = 2_048;
/// Maximum canonical base64 stream-token header accepted in the first release.
///
/// This deliberately leaves headroom over the padded base64 expansion of the
/// wire ceiling while remaining below common HTTP header budgets.
pub const STREAM_TOKEN_MAX_BASE64_BYTES_V1: usize = 4_096;

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
        norito::encode_canonical(self)
    }

    /// Build the exact domain-separated payload that an external Ed25519
    /// signer must sign.
    pub fn signing_payload_bytes(&self) -> Result<Vec<u8>, norito::Error> {
        let body = self.to_canonical_bytes()?;
        let mut message = Vec::with_capacity(STREAM_TOKEN_SIGNATURE_DOMAIN_V1.len() + body.len());
        message.extend_from_slice(STREAM_TOKEN_SIGNATURE_DOMAIN_V1);
        message.extend_from_slice(&body);
        Ok(message)
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
        let message = body.signing_payload_bytes()?;
        let signature = secret.sign(&message);
        Ok(Self {
            body,
            signature: signature.to_bytes().to_vec(),
        })
    }

    /// Assemble a token from a raw Ed25519 signature produced by an external
    /// signer and verify it before release.
    ///
    /// The external signer must sign [`StreamTokenBodyV1::signing_payload_bytes`]
    /// using pure Ed25519 and return the canonical 64-byte `R || S` signature.
    pub fn from_external_signature(
        body: StreamTokenBodyV1,
        signature: [u8; ed25519_dalek::SIGNATURE_LENGTH],
        verifier: &VerifyingKey,
    ) -> Result<Self, StreamTokenError> {
        let token = Self {
            body,
            signature: signature.to_vec(),
        };
        token.verify(verifier)?;
        Ok(token)
    }

    /// Verify the token signature using the supplied verifying key.
    pub fn verify(&self, verifier: &VerifyingKey) -> Result<(), StreamTokenError> {
        if verifier.is_weak() {
            return Err(StreamTokenError::InvalidSignatureFormat);
        }
        let signature_bytes: [u8; ed25519_dalek::SIGNATURE_LENGTH] = self
            .signature
            .as_slice()
            .try_into()
            .map_err(|_| StreamTokenError::InvalidSignatureFormat)?;
        let sig = crate::checked_ed25519_signature_from_bytes(&signature_bytes)
            .map_err(|_| StreamTokenError::InvalidSignatureFormat)?;
        let message = self.body.signing_payload_bytes()?;
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
    use ed25519_dalek::{PUBLIC_KEY_LENGTH, SigningKey};

    use super::*;

    const SMALL_ORDER_R: [u8; PUBLIC_KEY_LENGTH] = [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];
    const NONCANONICAL_R: [u8; PUBLIC_KEY_LENGTH] = [
        0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];

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
    fn canonical_body_and_signature_ignore_ambient_layout_flags() {
        let body = sample_body();
        let signing = SigningKey::from_bytes(&[0x42; 32]);
        let expected_body = body
            .to_canonical_bytes()
            .expect("encode canonical stream-token body");
        let expected_token =
            StreamTokenV1::sign(body.clone(), &signing).expect("sign canonical stream-token body");
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);

        assert_eq!(
            body.to_canonical_bytes()
                .expect("encode under alternate ambient flags"),
            expected_body
        );
        assert_eq!(
            StreamTokenV1::sign(body, &signing).expect("sign under alternate ambient flags"),
            expected_token
        );
    }

    #[test]
    fn wire_limit_expands_within_base64_header_limit() {
        let maximum_canonical_base64_len = STREAM_TOKEN_MAX_WIRE_BYTES_V1.div_ceil(3) * 4;
        assert!(maximum_canonical_base64_len <= STREAM_TOKEN_MAX_BASE64_BYTES_V1);
        assert_eq!(maximum_canonical_base64_len, 2_732);
    }

    #[test]
    fn verify_rejects_modified_body() {
        let signing = SigningKey::from_bytes(&[0x24; 32]);
        let verifying = signing.verifying_key();
        let token = StreamTokenV1::sign(sample_body(), &signing).expect("sign");
        let mut tampered = token.clone();
        tampered.body.max_streams = 8;
        let err = tampered.verify(&verifying).expect_err("should fail");
        assert!(matches!(err, StreamTokenError::SignatureInvalid(_)));
    }

    #[test]
    fn verify_rejects_signature_without_stream_token_domain() {
        let signing = SigningKey::from_bytes(&[0x25; 32]);
        let body = sample_body();
        let legacy_signature = signing
            .sign(&body.to_canonical_bytes().expect("canonical body"))
            .to_bytes()
            .to_vec();
        let token = StreamTokenV1 {
            body,
            signature: legacy_signature,
        };

        assert!(matches!(
            token.verify(&signing.verifying_key()),
            Err(StreamTokenError::SignatureInvalid(_))
        ));
    }

    #[test]
    fn external_signature_roundtrip_verifies_exact_payload() {
        let signing = SigningKey::from_bytes(&[0x26; 32]);
        let body = sample_body();
        let payload = body.signing_payload_bytes().expect("signing payload");
        let signature = signing.sign(&payload).to_bytes();

        let token = StreamTokenV1::from_external_signature(
            body.clone(),
            signature,
            &signing.verifying_key(),
        )
        .expect("assemble verified external signature");

        assert_eq!(token.body, body);
        token
            .verify(&signing.verifying_key())
            .expect("external token verifies");
    }

    #[test]
    fn external_signature_rejects_wrong_key_body_substitution_and_body_only_signing() {
        let signing = SigningKey::from_bytes(&[0x27; 32]);
        let wrong = SigningKey::from_bytes(&[0x28; 32]);
        let body = sample_body();
        let signature = signing
            .sign(&body.signing_payload_bytes().expect("signing payload"))
            .to_bytes();

        assert!(matches!(
            StreamTokenV1::from_external_signature(body.clone(), signature, &wrong.verifying_key()),
            Err(StreamTokenError::SignatureInvalid(_))
        ));

        let mut substituted = body.clone();
        substituted.max_streams += 1;
        assert!(matches!(
            StreamTokenV1::from_external_signature(
                substituted,
                signature,
                &signing.verifying_key()
            ),
            Err(StreamTokenError::SignatureInvalid(_))
        ));

        let body_only_signature = signing
            .sign(&body.to_canonical_bytes().expect("canonical body"))
            .to_bytes();
        assert!(matches!(
            StreamTokenV1::from_external_signature(
                body,
                body_only_signature,
                &signing.verifying_key()
            ),
            Err(StreamTokenError::SignatureInvalid(_))
        ));
    }

    #[test]
    fn external_signature_rejects_malformed_r() {
        let signing = SigningKey::from_bytes(&[0x29; 32]);
        let mut signature = signing
            .sign(
                &sample_body()
                    .signing_payload_bytes()
                    .expect("signing payload"),
            )
            .to_bytes();
        signature[..PUBLIC_KEY_LENGTH].copy_from_slice(&SMALL_ORDER_R);

        assert!(matches!(
            StreamTokenV1::from_external_signature(
                sample_body(),
                signature,
                &signing.verifying_key()
            ),
            Err(StreamTokenError::InvalidSignatureFormat)
        ));
    }

    #[test]
    fn verify_rejects_malformed_ed25519_signature_r() {
        let signing = SigningKey::from_bytes(&[0x42; 32]);
        let verifying = signing.verifying_key();

        for (label, replacement_r) in [
            ("small-order", SMALL_ORDER_R),
            ("noncanonical", NONCANONICAL_R),
        ] {
            let mut token = StreamTokenV1::sign(sample_body(), &signing).expect("sign");
            token.signature[..PUBLIC_KEY_LENGTH].copy_from_slice(&replacement_r);

            let err = token
                .verify(&verifying)
                .expect_err("malformed stream-token signature R must be rejected");
            assert!(
                matches!(err, StreamTokenError::InvalidSignatureFormat),
                "{label} signature R produced unexpected error: {err}"
            );
        }
    }

    #[test]
    fn verify_rejects_small_order_verifier_key_before_backend() {
        let signing = SigningKey::from_bytes(&[0x42; 32]);
        let weak_verifier = VerifyingKey::from_bytes(&SMALL_ORDER_R)
            .expect("small-order Ed25519 verifier key has parseable encoding");
        let token = StreamTokenV1::sign(sample_body(), &signing).expect("sign");

        let err = token
            .verify(&weak_verifier)
            .expect_err("small-order verifier key must fail before backend verification");

        assert!(matches!(err, StreamTokenError::InvalidSignatureFormat));
    }
}
