use std::panic::{AssertUnwindSafe, catch_unwind};

use pqcrypto_mldsa::{mldsa44 as dilithium2, mldsa65 as dilithium3, mldsa87 as dilithium5};
use pqcrypto_traits::{
    Error as PqError,
    sign::{
        DetachedSignature as PrimitiveDetachedSignature, PublicKey as PrimitivePublicKey,
        SecretKey as PrimitiveSecretKey, VerificationError,
    },
};
use thiserror::Error;
use zeroize::Zeroizing;

/// Supported ML-DSA (Dilithium) parameter sets.
#[derive(Clone, Copy, Debug)]
pub enum MlDsaSuite {
    /// ML-DSA-44 (Dilithium2).
    MlDsa44,
    /// ML-DSA-65 (Dilithium3).
    MlDsa65,
    /// ML-DSA-87 (Dilithium5).
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
            MlDsaSuite::MlDsa44 => dilithium2::public_key_bytes(),
            MlDsaSuite::MlDsa65 => dilithium3::public_key_bytes(),
            MlDsaSuite::MlDsa87 => dilithium5::public_key_bytes(),
        }
    }

    /// Return the secret key length in bytes for this suite.
    #[must_use]
    pub fn secret_key_len(self) -> usize {
        match self {
            MlDsaSuite::MlDsa44 => dilithium2::secret_key_bytes(),
            MlDsaSuite::MlDsa65 => dilithium3::secret_key_bytes(),
            MlDsaSuite::MlDsa87 => dilithium5::secret_key_bytes(),
        }
    }

    /// Return the detached signature length in bytes for this suite.
    #[must_use]
    pub fn signature_len(self) -> usize {
        match self {
            MlDsaSuite::MlDsa44 => dilithium2::signature_bytes(),
            MlDsaSuite::MlDsa65 => dilithium3::signature_bytes(),
            MlDsaSuite::MlDsa87 => dilithium5::signature_bytes(),
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
}

impl MlDsaError {
    fn bad_encoding(kind: &'static str, err: PqError) -> Self {
        MlDsaError::BadEncoding(Box::new(MlDsaEncodingError { kind, source: err }))
    }

    fn key_generation_failed(suite: MlDsaSuite, status: i32) -> Self {
        MlDsaError::KeyGenerationFailed { suite, status }
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

/// Generate a Dilithium (ML-DSA) keypair.
///
/// # Errors
///
/// Returns [`MlDsaError::KeyGenerationFailed`] if the underlying `PQClean`
/// routines report a failure, or [`MlDsaError::BadEncoding`] when the produced
/// key material cannot be converted into the Norito-friendly encoding.
pub fn generate_mldsa_keypair(suite: MlDsaSuite) -> Result<MlDsaKeyPair, MlDsaError> {
    match suite {
        MlDsaSuite::MlDsa44 => generate_dilithium2_keypair(),
        MlDsaSuite::MlDsa65 => generate_dilithium3_keypair(),
        MlDsaSuite::MlDsa87 => generate_dilithium5_keypair(),
    }
}

fn generate_dilithium2_keypair() -> Result<MlDsaKeyPair, MlDsaError> {
    finalize_keypair(
        catch_unwind(AssertUnwindSafe(dilithium2::keypair)),
        MlDsaSuite::MlDsa44,
        dilithium2::secret_key_bytes(),
    )
}

fn generate_dilithium3_keypair() -> Result<MlDsaKeyPair, MlDsaError> {
    finalize_keypair(
        catch_unwind(AssertUnwindSafe(dilithium3::keypair)),
        MlDsaSuite::MlDsa65,
        dilithium3::secret_key_bytes(),
    )
}

fn generate_dilithium5_keypair() -> Result<MlDsaKeyPair, MlDsaError> {
    finalize_keypair(
        catch_unwind(AssertUnwindSafe(dilithium5::keypair)),
        MlDsaSuite::MlDsa87,
        dilithium5::secret_key_bytes(),
    )
}

fn finalize_keypair<P, S>(
    keypair: std::thread::Result<(P, S)>,
    suite: MlDsaSuite,
    secret_len: usize,
) -> Result<MlDsaKeyPair, MlDsaError>
where
    P: PrimitivePublicKey,
    S: PrimitiveSecretKey + Copy,
{
    match keypair {
        Ok((pk, mut sk)) => {
            let public_key = pk.as_bytes().to_vec();
            let secret_bytes = Zeroizing::new(sk.as_bytes().to_vec());
            zero_secret(&mut sk, secret_len);
            Ok(MlDsaKeyPair {
                public_key,
                secret_key: secret_bytes,
            })
        }
        Err(_) => Err(MlDsaError::key_generation_failed(suite, -1)),
    }
}

fn zero_secret<S>(secret: &mut S, secret_len: usize)
where
    S: PrimitiveSecretKey + Copy,
{
    let zero = vec![0u8; secret_len];
    *secret = S::from_bytes(&zero).expect("zero-valued secret key must match expected length");
}

/// Create a detached signature over `message` using the provided secret key.
///
/// # Errors
/// Returns an error when the secret key or signature encoding is invalid.
pub fn sign_mldsa(
    suite: MlDsaSuite,
    secret_key: &[u8],
    message: &[u8],
) -> Result<MlDsaSignature, MlDsaError> {
    match suite {
        MlDsaSuite::MlDsa44 => {
            let sk = dilithium2::SecretKey::from_bytes(secret_key)
                .map_err(|err| MlDsaError::bad_encoding("ML-DSA-44 secret key", err))?;
            let sig = dilithium2::detached_sign(message, &sk);
            Ok(MlDsaSignature::new(sig.as_bytes().to_vec()))
        }
        MlDsaSuite::MlDsa65 => {
            let sk = dilithium3::SecretKey::from_bytes(secret_key)
                .map_err(|err| MlDsaError::bad_encoding("ML-DSA-65 secret key", err))?;
            let sig = dilithium3::detached_sign(message, &sk);
            Ok(MlDsaSignature::new(sig.as_bytes().to_vec()))
        }
        MlDsaSuite::MlDsa87 => {
            let sk = dilithium5::SecretKey::from_bytes(secret_key)
                .map_err(|err| MlDsaError::bad_encoding("ML-DSA-87 secret key", err))?;
            let sig = dilithium5::detached_sign(message, &sk);
            Ok(MlDsaSignature::new(sig.as_bytes().to_vec()))
        }
    }
}

/// Verify a detached signature.
///
/// # Errors
/// Returns an error when the public key or signature encoding is invalid or verification fails.
pub fn verify_mldsa(
    suite: MlDsaSuite,
    public_key: &[u8],
    message: &[u8],
    signature: &[u8],
) -> Result<(), MlDsaError> {
    match suite {
        MlDsaSuite::MlDsa44 => {
            let pk = dilithium2::PublicKey::from_bytes(public_key)
                .map_err(|err| MlDsaError::bad_encoding("ML-DSA-44 public key", err))?;
            let sig = dilithium2::DetachedSignature::from_bytes(signature)
                .map_err(|err| MlDsaError::bad_encoding("ML-DSA-44 signature", err))?;
            dilithium2::verify_detached_signature(&sig, message, &pk)
                .map_err(MlDsaError::VerificationFailed)
        }
        MlDsaSuite::MlDsa65 => {
            let pk = dilithium3::PublicKey::from_bytes(public_key)
                .map_err(|err| MlDsaError::bad_encoding("ML-DSA-65 public key", err))?;
            let sig = dilithium3::DetachedSignature::from_bytes(signature)
                .map_err(|err| MlDsaError::bad_encoding("ML-DSA-65 signature", err))?;
            dilithium3::verify_detached_signature(&sig, message, &pk)
                .map_err(MlDsaError::VerificationFailed)
        }
        MlDsaSuite::MlDsa87 => {
            let pk = dilithium5::PublicKey::from_bytes(public_key)
                .map_err(|err| MlDsaError::bad_encoding("ML-DSA-87 public key", err))?;
            let sig = dilithium5::DetachedSignature::from_bytes(signature)
                .map_err(|err| MlDsaError::bad_encoding("ML-DSA-87 signature", err))?;
            dilithium5::verify_detached_signature(&sig, message, &pk)
                .map_err(MlDsaError::VerificationFailed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signed_roundtrip(suite: MlDsaSuite) {
        let kp = generate_mldsa_keypair(suite).expect("ML-DSA keypair generation should succeed");
        let message = b"SoraNet PQ harness";
        let signature = sign_mldsa(suite, kp.secret_key(), message).unwrap();
        verify_mldsa(suite, kp.public_key(), message, signature.as_bytes()).unwrap();
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
    fn reject_modified_message() {
        let kp = generate_mldsa_keypair(MlDsaSuite::MlDsa44)
            .expect("ML-DSA keypair generation should succeed");
        let message = b"context";
        let signature = sign_mldsa(MlDsaSuite::MlDsa44, kp.secret_key(), message).unwrap();
        let mut tampered = message.to_vec();
        tampered[0] ^= 0xFF;
        let err = verify_mldsa(
            MlDsaSuite::MlDsa44,
            kp.public_key(),
            &tampered,
            signature.as_bytes(),
        )
        .unwrap_err();

        match err {
            MlDsaError::VerificationFailed(VerificationError::InvalidSignature) => {}
            _ => panic!("unexpected error"),
        }
    }
}
