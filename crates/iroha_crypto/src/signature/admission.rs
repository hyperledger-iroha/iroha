//! The sole uncached admission dispatch with unformatted rejection custody.
//!
//! Ordinary public diagnostics are materialized only by the outer Error adapter.
//! Algorithm parsing and verification reuse the existing canonical relations.

use super::{ed25519, mldsa, secp256k1};
use crate::{Algorithm, Error, PublicKey, PublicKeyEnvelopeError, Signature};

/// Unformatted rejection from the borrowed, uncached signature relation.
///
/// This owns the original typed parser rejection, not a key, signature, cache or
/// rendered diagnostic. It is not a resource refusal or an execution permit.
#[derive(Debug)]
pub struct SignatureVerificationError(Failure);

#[derive(Debug)]
enum Failure {
    Envelope(PublicKeyEnvelopeError),
    Ed25519(ed25519::KeyRejection),
    Secp256k1(secp256k1::public_key::KeyRejection),
    #[cfg(feature = "bls")]
    Bls(super::bls::uncached::Rejection),
    #[cfg(feature = "gost")]
    Gost(super::gost::KeyRejection),
    #[cfg(feature = "sm")]
    Sm2(crate::sm::verification::KeyRejection),
    Signature,
}

impl SignatureVerificationError {
    /// Materialize the existing public crypto diagnostic at a completed boundary.
    ///
    /// This adapter can allocate a String. Callers requiring fixed error custody
    /// must retain this owner instead of calling this method.
    #[must_use]
    pub fn into_error(self) -> Error {
        match self.0 {
            Failure::Envelope(error) => error.into_parse_error().into(),
            Failure::Ed25519(error) => error.into_parse_error().into(),
            Failure::Secp256k1(error) => error.into_parse_error().into(),
            #[cfg(feature = "bls")]
            Failure::Bls(error) => error.into_error(),
            #[cfg(feature = "gost")]
            Failure::Gost(error) => error.into_parse_error().into(),
            #[cfg(feature = "sm")]
            Failure::Sm2(error) => error.into_parse_error().into(),
            Failure::Signature => Error::BadSignature,
        }
    }
}

/// Verify borrowed signature inputs without persistent caches or formatted errors.
///
/// This is the same admission relation used by `verify_signature_for_admission`.
/// It preserves algorithm/payload envelope order and each admitted algorithm's
/// canonical key and signature checks. It does not consult ordinary key caches
/// or positive-verdict caches.
///
/// # Errors
/// Returns the unformatted canonical parser or verification rejection. A caller
/// must not interpret this semantic rejection as a local resource deferral.
pub fn verify_signature_borrowed(
    proof: &Signature,
    public_key: &PublicKey,
    message: &[u8],
) -> Result<(), SignatureVerificationError> {
    verify(proof, public_key, message).map_err(SignatureVerificationError)
}

fn verify(proof: &Signature, public_key: &PublicKey, message: &[u8]) -> Result<(), Failure> {
    let (algorithm, payload) = public_key.borrowed_parts().map_err(Failure::Envelope)?;
    match algorithm {
        Algorithm::Ed25519 => {
            let key = ed25519::Ed25519Sha512::parse_public_key_uncached_for_decode(payload)
                .map_err(Failure::Ed25519)?;
            // Both the shared strict signature parser and verify_strict return
            // only BadSignature here; no public diagnostic is constructed.
            ed25519::Ed25519Sha512::verify_uncached(message, proof.payload(), &key)
                .map_err(|_| Failure::Signature)
        }
        Algorithm::Secp256k1 => {
            let key = secp256k1::public_key::parse(payload).map_err(Failure::Secp256k1)?;
            // Shared fixed scalar/point relation; its failures are BadSignature.
            secp256k1::EcdsaSecp256k1Sha256::verify(message, proof.payload(), &key)
                .map_err(|_| Failure::Signature)
        }
        Algorithm::MlDsa => {
            // The original single-item batch admission adapter maps every fixed
            // ML-DSA key/relation rejection to BadSignature, including no-PQC.
            mldsa::verify(payload, proof.payload(), message).map_err(|_| Failure::Signature)
        }
        #[cfg(feature = "gost")]
        Algorithm::Gost3410_2012_256ParamSetA
        | Algorithm::Gost3410_2012_256ParamSetB
        | Algorithm::Gost3410_2012_256ParamSetC
        | Algorithm::Gost3410_2012_512ParamSetA
        | Algorithm::Gost3410_2012_512ParamSetB => {
            super::gost::validate_public_key(algorithm, payload).map_err(Failure::Gost)?;
            // These five exhaustive admitted cases select a fixed curve. The
            // unsupported-algorithm diagnostic in the ordinary GOST facade is
            // unreachable; the selected relation returns only BadSignature.
            super::gost::verify_bytes(algorithm, message, proof.payload(), payload)
                .map_err(|_| Failure::Signature)
        }
        #[cfg(feature = "bls")]
        Algorithm::BlsNormal | Algorithm::BlsSmall => {
            let orientation = super::bls::uncached::Orientation::for_algorithm(algorithm)
                .ok_or(Failure::Signature)?;
            super::bls::uncached::verify_facade(orientation, payload, proof.payload(), message)
                .map_err(Failure::Bls)
        }
        #[cfg(feature = "sm")]
        Algorithm::Sm2 => crate::sm::verification::BorrowedKey::parse(payload)
            .map_err(Failure::Sm2)?
            // Shared fixed identity-hash/scalar/point relation, BadSignature only.
            .verify(message, proof.payload())
            .map_err(|_| Failure::Signature),
    }
}

#[cfg(test)]
mod tests;
