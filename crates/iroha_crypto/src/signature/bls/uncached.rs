//! The sole Iroha contextual BLS single-signature relation, without retained caches.

use super::canonical;
use crate::{Algorithm, Error};
use blst::{BLST_ERROR, blst_core_verify_pk_in_g1, blst_core_verify_pk_in_g2};
use blstrs::{G1Affine, G2Affine};
use group::prime::PrimeCurveAffine as _;

/// The two existing Iroha BLS public-key/signature orientations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Orientation {
    /// Public key in G1 and signature in G2.
    Normal,
    /// Public key in G2 and signature in G1.
    Small,
}

impl Orientation {
    /// Select only an existing BLS algorithm; this is not backend policy.
    pub(crate) const fn for_algorithm(algorithm: Algorithm) -> Option<Self> {
        match algorithm {
            Algorithm::BlsNormal => Some(Self::Normal),
            Algorithm::BlsSmall => Some(Self::Small),
            _ => None,
        }
    }

    const fn signature_len(self) -> usize {
        match self {
            Self::Normal => 96,
            Self::Small => 48,
        }
    }
}

/// Unformatted failures from canonical parsing or the signature relation.
#[derive(Debug)]
pub(crate) enum Rejection {
    /// Exact deterministic canonical input failure.
    Parse(canonical::Failure),
    /// Canonical inputs fail the relation, or generic signature geometry is invalid.
    Verification,
}

impl Rejection {
    /// Materialize the existing ordinary API error outside the fixed core.
    pub(crate) fn into_error(self) -> Error {
        match self {
            Self::Parse(failure) => failure.into_parse_error().into(),
            Self::Verification => Error::BadSignature,
        }
    }
}

impl From<canonical::Failure> for Rejection {
    fn from(failure: canonical::Failure) -> Self {
        Self::Parse(failure)
    }
}

/// A canonical nonidentity public key whose subgroup has been checked.
pub(super) enum PublicKey {
    /// Normal BLS key.
    Normal(G1Affine),
    /// Compact BLS key.
    Small(G2Affine),
}

/// A canonical nonidentity signature whose subgroup has been checked.
pub(super) enum Signature {
    /// Normal BLS signature.
    Normal(G2Affine),
    /// Compact BLS signature.
    Small(G1Affine),
}

// w3f-bls 0.1.9 prefixes the Basic ciphersuite and MESSAGE_CONTEXT to the
// message, then hashes with the single-byte DST [1]. These are augmentation
// INPUT, not IETF/Ethereum ciphersuite DSTs.
const NORMAL_PREFIX: &[u8] = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_NUL_for signing messages";
const SMALL_PREFIX: &[u8] = b"BLS_SIG_BLS12381G1_XMD:SHA-256_SSWU_RO_NUL_for signing messages";
const HASH_TO_FIELD_DST: &[u8] = &[1];

fn g1(bytes: &[u8]) -> Option<G1Affine> {
    let encoded: &[u8; 48] = bytes.try_into().ok()?;
    let point = G1Affine::from_compressed(encoded).into_option()?;
    (!bool::from(point.is_identity()) && point.to_compressed() == *encoded).then_some(point)
}

fn g2(bytes: &[u8]) -> Option<G2Affine> {
    let encoded: &[u8; 96] = bytes.try_into().ok()?;
    let point = G2Affine::from_compressed(encoded).into_option()?;
    (!bool::from(point.is_identity()) && point.to_compressed() == *encoded).then_some(point)
}

/// Parse with the original canonical decoder and convert only checked bytes.
pub(super) fn public_key(orientation: Orientation, bytes: &[u8]) -> Result<PublicKey, Rejection> {
    match orientation {
        Orientation::Normal => {
            let (_, encoding) = canonical::public_key::<w3f_bls::ZBLS>(bytes)?;
            g1(encoding.as_slice()).map(PublicKey::Normal)
        }
        Orientation::Small => {
            let (_, encoding) = canonical::public_key::<w3f_bls::TinyBLS381>(bytes)?;
            g2(encoding.as_slice()).map(PublicKey::Small)
        }
    }
    .ok_or_else(|| canonical::Failure::PublicKeyInvalid.into())
}

/// Parse the signature before a positive-verdict cache can be consulted.
pub(super) fn signature(orientation: Orientation, bytes: &[u8]) -> Result<Signature, Rejection> {
    match orientation {
        Orientation::Normal => {
            let (_, encoding) = canonical::signature::<w3f_bls::ZBLS>(bytes)?;
            g2(encoding.as_slice()).map(Signature::Normal)
        }
        Orientation::Small => {
            let (_, encoding) = canonical::signature::<w3f_bls::TinyBLS381>(bytes)?;
            g1(encoding.as_slice()).map(Signature::Small)
        }
    }
    .ok_or_else(|| canonical::Failure::SignatureInvalid.into())
}

/// Preserve generic `Signature::verify`'s public-key-first rejection order.
pub(super) fn prepare_facade(
    orientation: Orientation,
    key: &[u8],
    proof: &[u8],
) -> Result<(PublicKey, Signature), Rejection> {
    let key = public_key(orientation, key)?;
    if proof.len() != orientation.signature_len()
        || (!proof.is_empty() && proof.iter().all(|byte| *byte == 0))
    {
        return Err(Rejection::Verification);
    }
    let signature = signature(orientation, proof)?;
    Ok((key, signature))
}

/// Verify the generic public facade with no key or positive-result cache.
pub(crate) fn verify_facade(
    orientation: Orientation,
    key: &[u8],
    proof: &[u8],
    message: &[u8],
) -> Result<(), Rejection> {
    let (key, proof) = prepare_facade(orientation, key, proof)?;
    verify_parsed(&key, &proof, message)
}

/// Verify borrowed canonical inputs using the typed parser diagnostics.
#[cfg(test)]
fn verify(
    orientation: Orientation,
    key: &[u8],
    proof: &[u8],
    message: &[u8],
) -> Result<(), Rejection> {
    let key = public_key(orientation, key)?;
    let proof = signature(orientation, proof)?;
    verify_parsed(&key, &proof, message)
}

/// Apply the sole contextual single-signature relation to checked fixed points.
///
/// The synchronous C core retains no pointers or allocations. Its pairing
/// context has fixed capacity; the one-term Miller loop and hash-to-field
/// backing are bounded independently of the borrowed message length.
///
/// # Errors
/// Returns the fixed verification failure if the signature relation is false.
#[allow(unsafe_code)] // The two documented synchronous FFI calls are the entire unsafe boundary.
pub(super) fn verify_parsed(
    public_key: &PublicKey,
    signature: &Signature,
    message: &[u8],
) -> Result<(), Rejection> {
    let status = match (public_key, signature) {
        (PublicKey::Normal(public_key), Signature::Normal(signature)) => {
            // SAFETY: checked fixed-size, canonical subgroup points remain
            // live for the call; every byte pointer carries its borrowed slice
            // length. The synchronous C core neither writes nor retains them.
            unsafe {
                blst_core_verify_pk_in_g1(
                    public_key.as_ref(),
                    signature.as_ref(),
                    true,
                    message.as_ptr(),
                    message.len(),
                    HASH_TO_FIELD_DST.as_ptr(),
                    HASH_TO_FIELD_DST.len(),
                    NORMAL_PREFIX.as_ptr(),
                    NORMAL_PREFIX.len(),
                )
            }
        }
        (PublicKey::Small(public_key), Signature::Small(signature)) => {
            // SAFETY: same borrowed-input contract as the normal orientation;
            // this entry point expects the checked G2 key and G1 signature.
            unsafe {
                blst_core_verify_pk_in_g2(
                    public_key.as_ref(),
                    signature.as_ref(),
                    true,
                    message.as_ptr(),
                    message.len(),
                    HASH_TO_FIELD_DST.as_ptr(),
                    HASH_TO_FIELD_DST.len(),
                    SMALL_PREFIX.as_ptr(),
                    SMALL_PREFIX.len(),
                )
            }
        }
        _ => return Err(Rejection::Verification),
    };
    if status == BLST_ERROR::BLST_SUCCESS {
        Ok(())
    } else {
        Err(Rejection::Verification)
    }
}

#[cfg(test)]
#[path = "uncached_tests.rs"]
pub(super) mod tests;
