//! High-level SM2 helpers for the Rust SDK.
//!
//! This module exposes ergonomic wrappers around the low-level `iroha_crypto`
//! SM2 primitives, enabling deterministic key derivation, signing, and
//! verification flows that mirror the Python and JavaScript SDKs.

use core::fmt;

use iroha_crypto::{
    Algorithm, PublicKey,
    error::{Error as CryptoError, ParseError},
    sm::{Sm2PrivateKey, Sm2PublicKey, Sm2Signature, encode_sm2_public_key_payload},
};
use rand_core_06::OsRng;
use thiserror::Error;

/// Convenience wrapper around an SM2 key pair.
pub struct Sm2KeyPair {
    private: Sm2PrivateKey,
}

// Normalize Sm2PrivateKey::random return type across crypto versions.
trait IntoSm2Result {
    fn into_result(self) -> Result<Sm2PrivateKey, ParseError>;
}

impl IntoSm2Result for Sm2PrivateKey {
    fn into_result(self) -> Result<Sm2PrivateKey, ParseError> {
        Ok(self)
    }
}

impl IntoSm2Result for Result<Sm2PrivateKey, ParseError> {
    fn into_result(self) -> Result<Sm2PrivateKey, ParseError> {
        self
    }
}

impl fmt::Debug for Sm2KeyPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sm2KeyPair")
            .field("distid", &self.private.distid())
            .field(
                "public_key_sec1",
                &hex::encode_upper(self.public_key_sec1_bytes(false)),
            )
            .finish()
    }
}

impl Sm2KeyPair {
    /// Distinguishing identifier used when callers do not supply one.
    pub const DEFAULT_DISTID: &'static str = Sm2PublicKey::DEFAULT_DISTID;

    /// Generate a random SM2 key pair with the default distinguishing ID.
    pub fn generate() -> Self {
        Self::generate_with_distid(Self::DEFAULT_DISTID)
            .expect("sm2 default distid must be valid")
    }

    /// Generate a random SM2 key pair with a custom distinguishing ID.
    ///
    /// # Errors
    /// Returns [`ParseError`] when the distinguishing identifier is invalid.
    pub fn generate_with_distid(distid: impl Into<String>) -> Result<Self, ParseError> {
        let mut rng = OsRng;
        let private = Sm2PrivateKey::random(distid, &mut rng).into_result()?;
        Ok(Self { private })
    }

    /// Deterministically derive a key pair from opaque seed material with the default ID.
    ///
    /// # Errors
    /// Returns [`ParseError`] when the seed fails to derive a valid SM2 scalar.
    pub fn from_seed(seed: impl AsRef<[u8]>) -> Result<Self, ParseError> {
        Self::from_seed_with_distid(Self::DEFAULT_DISTID, seed)
    }

    /// Deterministically derive a key pair from opaque seed material and custom ID.
    ///
    /// # Errors
    /// Returns [`ParseError`] when the seed fails to derive a valid SM2 scalar.
    pub fn from_seed_with_distid(
        distid: impl Into<String>,
        seed: impl AsRef<[u8]>,
    ) -> Result<Self, ParseError> {
        let private = Sm2PrivateKey::from_seed(distid, seed.as_ref())?;
        Ok(Self { private })
    }

    /// Reconstruct a key pair from raw private-key bytes with the default ID.
    ///
    /// # Errors
    /// Returns [`ParseError`] when the payload is malformed.
    pub fn from_private_key(bytes: impl AsRef<[u8]>) -> Result<Self, ParseError> {
        Self::from_private_key_with_distid(Self::DEFAULT_DISTID, bytes)
    }

    /// Reconstruct a key pair from raw private-key bytes and a custom distinguishing ID.
    ///
    /// # Errors
    /// Returns [`ParseError`] when the payload is malformed.
    pub fn from_private_key_with_distid(
        distid: impl Into<String>,
        bytes: impl AsRef<[u8]>,
    ) -> Result<Self, ParseError> {
        let private = Sm2PrivateKey::from_bytes(distid, bytes.as_ref())?;
        Ok(Self { private })
    }

    /// Return the distinguishing ID associated with this key pair.
    #[must_use]
    pub fn distid(&self) -> &str {
        self.private.distid()
    }

    /// Return the canonical 32-byte private scalar.
    #[must_use]
    pub fn private_key_bytes(&self) -> [u8; 32] {
        self.private.secret_bytes()
    }

    /// Export the public key in SEC1 form (`0x04 || x || y` when `compressed` is false).
    #[must_use]
    pub fn public_key_sec1_bytes(&self, compressed: bool) -> Vec<u8> {
        self.private.public_key().to_sec1_bytes(compressed)
    }

    /// Return the canonical multihash string for the public key.
    ///
    /// # Panics
    /// Panics if the internal key material is somehow inconsistent.
    #[must_use]
    pub fn public_key_multihash(&self) -> String {
        let sec1 = self.public_key_sec1_bytes(false);
        let payload = encode_sm2_public_key_payload(self.distid(), &sec1)
            .expect("SM2 payload derived from key material must encode");
        let pk = PublicKey::from_bytes(Algorithm::Sm2, &payload)
            .expect("SM2 SEC1 payload generated from private key must be valid");
        pk.to_string()
    }

    /// Sign `message` using deterministic SM2 DSA and return the canonical r∥s bytes.
    #[must_use]
    pub fn sign(&self, message: &[u8]) -> [u8; Sm2Signature::LENGTH] {
        self.private.sign(message).to_bytes()
    }

    /// Verify `signature` against `message` with this key pair.
    ///
    /// # Errors
    /// Returns [`Sm2VerifyError`] when the signature payload is malformed or verification fails.
    pub fn verify(
        &self,
        message: &[u8],
        signature: &[u8; Sm2Signature::LENGTH],
    ) -> Result<(), Sm2VerifyError> {
        let parsed = Sm2Signature::from_bytes(signature)?;
        self.private
            .public_key()
            .verify(message, &parsed)
            .map_err(Sm2VerifyError::from)
    }

    /// Borrow the underlying private key for advanced use-cases.
    #[must_use]
    pub fn private_key(&self) -> &Sm2PrivateKey {
        &self.private
    }

    /// Derive the public key from the private component.
    #[must_use]
    pub fn public_key(&self) -> Sm2PublicKey {
        self.private.public_key()
    }
}

impl From<Sm2KeyPair> for Sm2PrivateKey {
    fn from(pair: Sm2KeyPair) -> Self {
        pair.private
    }
}

impl From<&Sm2KeyPair> for Sm2PublicKey {
    fn from(pair: &Sm2KeyPair) -> Self {
        pair.public_key()
    }
}

/// Error returned when SM2 signature verification fails.
#[derive(Debug, Error)]
pub enum Sm2VerifyError {
    /// Signature bytes were malformed.
    #[error("invalid SM2 signature: {0}")]
    Parse(#[from] ParseError),
    /// Signature did not validate for the provided message/key pair.
    #[error("SM2 signature verification failed: {0}")]
    Verify(#[from] CryptoError),
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use hex::FromHex;
    use norito::{derive::JsonDeserialize, json};

    use super::*;

    #[derive(Debug, JsonDeserialize)]
    struct Fixture {
        algorithm: String,
        distid: String,
        seed_hex: String,
        message_hex: String,
        private_key_hex: String,
        public_key_sec1_hex: String,
        public_key_multihash: String,
        public_key_prefixed: String,
        za: String,
        signature: String,
        r: String,
        s: String,
    }

    fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("fixtures")
            .join("sm")
            .join("sm2_fixture.json")
    }

    #[test]
    fn fixture_roundtrip_matches_crypto_core() {
        let payload = fs::read_to_string(fixture_path()).expect("fixture present");
        let fixture: Fixture = json::from_str(&payload).expect("valid fixture JSON");

        assert_eq!(fixture.algorithm, "sm2");
        assert_eq!(fixture.distid, Sm2KeyPair::DEFAULT_DISTID);

        let seed = <[u8; 32]>::from_hex(&fixture.seed_hex).expect("seed hex");
        let expected_private = <[u8; 32]>::from_hex(&fixture.private_key_hex).expect("private hex");
        let expected_public =
            Vec::from_hex(&fixture.public_key_sec1_hex).expect("public hex (SEC1)");
        let message = Vec::from_hex(&fixture.message_hex).expect("message hex");
        let expected_signature =
            <[u8; Sm2Signature::LENGTH]>::from_hex(&fixture.signature).expect("signature hex");

        let keypair =
            Sm2KeyPair::from_seed(seed).expect("fixture seed should yield valid SM2 key pair");

        assert_eq!(keypair.private_key_bytes(), expected_private);
        assert_eq!(keypair.public_key_sec1_bytes(false), expected_public);
        assert_eq!(keypair.public_key_multihash(), fixture.public_key_multihash);
        let payload = encode_sm2_public_key_payload(&fixture.distid, &expected_public)
            .expect("fixture SM2 payload");
        let prefixed = PublicKey::from_bytes(Algorithm::Sm2, &payload)
            .expect("fixture public key payload must be valid");
        assert_eq!(prefixed.to_prefixed_string(), fixture.public_key_prefixed);

        let signature = keypair.sign(&message);
        assert_eq!(signature, expected_signature);
        keypair
            .verify(&message, &expected_signature)
            .expect("fixture signature should verify");
        assert_eq!(
            hex::encode_upper(
                keypair
                    .public_key()
                    .compute_z(&fixture.distid)
                    .expect("compute ZA")
            ),
            fixture.za
        );
        assert_eq!(
            hex::encode_upper(&signature[..Sm2Signature::LENGTH / 2]),
            fixture.r
        );
        assert_eq!(
            hex::encode_upper(&signature[Sm2Signature::LENGTH / 2..]),
            fixture.s
        );

        // Mismatched message must fail verification.
        let mut tampered = expected_signature;
        tampered[0] ^= 0xFF;
        assert!(matches!(
            keypair.verify(b"tampered message", &expected_signature),
            Err(Sm2VerifyError::Verify(_))
        ));
        assert!(matches!(
            keypair.verify(&message, &tampered),
            Err(Sm2VerifyError::Verify(_))
        ));
    }
}
