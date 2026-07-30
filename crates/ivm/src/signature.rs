use curve25519_dalek::edwards::CompressedEdwardsY;
/// Digital signature verification helpers used by the VM.
///
/// This module provides wrappers around common signature libraries so that
/// callers can choose between classical Ed25519 and post-quantum ML-DSA
/// (Crystals Dilithium) verification.
use ed25519_dalek::{Signature as Ed25519Signature, VerifyingKey as Ed25519VerifyingKey};
use iroha_crypto::{EcdsaSecp256k1Sha256, ed25519_parse_public_key};
use pqcrypto_mldsa::mldsa65 as dilithium;
use pqcrypto_traits::sign::{DetachedSignature as _, PublicKey as _};
use sha2::{Digest as _, Sha512};

/// Norito-encoded bundle describing a single Ed25519 verification item.
#[derive(Debug, Clone, norito::Encode, norito::Decode, PartialEq, Eq)]
pub struct Ed25519BatchEntry {
    /// Message bytes to verify.
    pub message: Vec<u8>,
    /// Raw Ed25519 signature bytes.
    pub signature: Vec<u8>,
    /// Raw Ed25519 public key bytes.
    pub public_key: Vec<u8>,
}

/// Norito-encoded request for ordered strict Ed25519 verification.
#[derive(Debug, Clone, norito::Encode, norito::Decode, PartialEq, Eq)]
pub struct Ed25519BatchRequest {
    /// Entries to verify, evaluated in order.
    pub entries: Vec<Ed25519BatchEntry>,
}

/// Outcome of deterministic Ed25519 batch verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ed25519BatchError {
    /// Request contained no entries.
    Empty,
    /// Request exceeded the configured maximum entry count.
    TooMany { max: usize, actual: usize },
    /// Entry failed structural validation (e.g. malformed public key).
    InvalidEntry { index: usize },
    /// Signature verification failed for the entry at `index`.
    SignatureFailed { index: usize },
}

/// Supported signature schemes.
#[derive(Clone, Copy, Debug)]
pub enum SignatureScheme {
    /// Ed25519 using the `ed25519-dalek` crate.
    Ed25519,
    /// ML-DSA (Crystals Dilithium) using the `ml-dsa` crate.
    MlDsa,
    /// Secp256k1 ECDSA using the `k256` crate.
    Secp256k1,
}

/// Returns true when supplied cryptographic material is an inert all-zero buffer.
#[must_use]
pub(crate) fn material_bytes_are_all_zero(bytes: &[u8]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}

/// Returns true when the supplied signature payload is an inert all-zero buffer.
#[must_use]
pub(crate) fn signature_bytes_are_all_zero(signature: &[u8]) -> bool {
    material_bytes_are_all_zero(signature)
}

/// Returns true when an Ed25519 signature carries a noncanonical or small-order `R`.
#[must_use]
pub(crate) fn signature_has_invalid_ed25519_r(signature: &[u8]) -> bool {
    let Some(r_bytes) = signature.get(..32) else {
        return false;
    };
    let Ok(r_bytes) = <[u8; 32]>::try_from(r_bytes) else {
        return false;
    };
    let compressed = CompressedEdwardsY(r_bytes);
    let Some(point) = compressed.decompress() else {
        return true;
    };
    point.is_small_order() || point.compress().as_bytes() != &r_bytes
}

/// Parse an Ed25519 public key after rejecting inert, weak, and noncanonical encodings.
#[must_use]
pub(crate) fn parse_ed25519_public_key_for_verification(
    public_key: &[u8; 32],
) -> Option<Ed25519VerifyingKey> {
    let parsed = ed25519_parse_public_key(public_key).ok()?;
    Ed25519VerifyingKey::from_bytes(parsed.as_bytes()).ok()
}

/// Returns true when Ed25519 public-key bytes must not reach a verifier.
#[must_use]
pub(crate) fn ed25519_public_key_bytes_are_invalid(public_key: &[u8; 32]) -> bool {
    parse_ed25519_public_key_for_verification(public_key).is_none()
}

/// Ed25519 batch verification input.
#[derive(Clone, Debug)]
pub struct Ed25519BatchItem<'a> {
    /// Message to verify.
    pub message: &'a [u8],
    /// 64-byte detached signature.
    pub signature: [u8; 64],
    /// 32-byte public key.
    pub public_key: [u8; 32],
}

/// Compute the reduced Ed25519 challenge scalar bytes `H(R || A || M)` used by
/// the GPU verification kernels.
#[cfg_attr(
    not(any(feature = "cuda", all(target_os = "macos", feature = "metal"))),
    allow(dead_code)
)]
pub(crate) fn ed25519_challenge_scalar_bytes(
    signature: &[u8; 64],
    public_key: &[u8; 32],
    message: &[u8],
) -> [u8; 32] {
    use curve25519_dalek::scalar::Scalar;

    let mut hasher = Sha512::new();
    hasher.update(&signature[..32]);
    hasher.update(public_key);
    hasher.update(message);
    Scalar::from_hash(hasher).to_bytes()
}

/// Verify `signature` on `message` with `public_key` using the selected
/// `scheme`. Returns `true` if the signature is valid. For Ed25519, public keys
/// must be canonical, non-weak compressed points and signatures must carry a
/// canonical, non-small-order `R`. For secp256k1, only canonical compressed
/// SEC1 public keys are accepted and high-S signatures are rejected.
pub fn verify_signature(
    scheme: SignatureScheme,
    message: &[u8],
    signature: &[u8],
    public_key: &[u8],
) -> bool {
    match scheme {
        SignatureScheme::Ed25519 => {
            let pk_bytes: &[u8; 32] = match public_key.try_into() {
                Ok(b) => b,
                Err(_) => return false,
            };
            let Some(pk) = parse_ed25519_public_key_for_verification(pk_bytes) else {
                return false;
            };
            if signature_bytes_are_all_zero(signature) {
                return false;
            }
            if signature_has_invalid_ed25519_r(signature) {
                return false;
            }
            let sig = match Ed25519Signature::from_slice(signature) {
                Ok(s) => s,
                Err(_) => return false,
            };
            pk.verify_strict(message, &sig).is_ok()
        }
        SignatureScheme::MlDsa => {
            if public_key.len() != dilithium::public_key_bytes()
                || signature.len() != dilithium::signature_bytes()
            {
                return false;
            }
            if material_bytes_are_all_zero(public_key) {
                return false;
            }
            if signature_bytes_are_all_zero(signature) {
                return false;
            }
            let pk = match dilithium::PublicKey::from_bytes(public_key) {
                Ok(pk) => pk,
                Err(_) => return false,
            };
            let sig = match dilithium::DetachedSignature::from_bytes(signature) {
                Ok(s) => s,
                Err(_) => return false,
            };
            dilithium::verify_detached_signature(&sig, message, &pk).is_ok()
        }
        SignatureScheme::Secp256k1 => {
            if signature.len() != 64 {
                return false;
            }
            let pk = match EcdsaSecp256k1Sha256::parse_public_key(public_key) {
                Ok(pk) => pk,
                Err(_) => return false,
            };
            EcdsaSecp256k1Sha256::verify(message, signature, &pk).is_ok()
        }
    }
}

/// Strictly verify ordered Ed25519 signatures, returning the first failing
/// entry index on error.
pub fn verify_ed25519_batch(
    request: &Ed25519BatchRequest,
    max_entries: usize,
) -> Result<(), Ed25519BatchError> {
    if request.entries.is_empty() {
        return Err(Ed25519BatchError::Empty);
    }
    if request.entries.len() > max_entries {
        return Err(Ed25519BatchError::TooMany {
            max: max_entries,
            actual: request.entries.len(),
        });
    }
    for (index, entry) in request.entries.iter().enumerate() {
        if entry.signature.len() != 64 || entry.public_key.len() != 32 {
            return Err(Ed25519BatchError::InvalidEntry { index });
        }
        if signature_bytes_are_all_zero(&entry.signature) {
            return Err(Ed25519BatchError::SignatureFailed { index });
        }
        if signature_has_invalid_ed25519_r(&entry.signature) {
            return Err(Ed25519BatchError::SignatureFailed { index });
        }
        let pk_bytes: &[u8; 32] = entry
            .public_key
            .as_slice()
            .try_into()
            .map_err(|_| Ed25519BatchError::InvalidEntry { index })?;
        let Some(pk) = parse_ed25519_public_key_for_verification(pk_bytes) else {
            return Err(Ed25519BatchError::InvalidEntry { index });
        };
        let sig = Ed25519Signature::from_slice(entry.signature.as_slice())
            .map_err(|_| Ed25519BatchError::InvalidEntry { index })?;
        if pk.verify_strict(entry.message.as_slice(), &sig).is_err() {
            return Err(Ed25519BatchError::SignatureFailed { index });
        }
    }
    Ok(())
}

/// Verify a batch of Ed25519 signatures. Entries that fail to parse are marked
/// as invalid. Tries the CUDA per-signature path when available, otherwise
/// falls back to deterministic CPU verification.
pub fn verify_ed25519_batch_items(items: &[Ed25519BatchItem<'_>]) -> Vec<bool> {
    if items.is_empty() {
        return Vec::new();
    }
    #[cfg(feature = "cuda")]
    let maybe_cuda = crate::cuda::cuda_available() && !crate::cuda::cuda_disabled();
    #[cfg(feature = "cuda")]
    let mut cuda_inputs = if maybe_cuda {
        Some((Vec::new(), Vec::new(), Vec::new(), Vec::new()))
    } else {
        None
    };

    let mut parsed = Vec::with_capacity(items.len());
    #[cfg(all(target_os = "macos", feature = "metal"))]
    let mut metal_inputs = if crate::vector::metal_available() {
        Some((Vec::new(), Vec::new(), Vec::new(), Vec::new()))
    } else {
        None
    };

    for item in items {
        if signature_bytes_are_all_zero(&item.signature) {
            parsed.push(None);
            continue;
        }
        if signature_has_invalid_ed25519_r(&item.signature) {
            parsed.push(None);
            continue;
        }
        let Ok(sig) = Ed25519Signature::from_slice(&item.signature) else {
            parsed.push(None);
            continue;
        };
        let Some(pk) = parse_ed25519_public_key_for_verification(&item.public_key) else {
            parsed.push(None);
            continue;
        };

        #[cfg(all(target_os = "macos", feature = "metal"))]
        if let Some((ref mut sigs, ref mut pks, ref mut hrams, ref mut map)) = metal_inputs {
            hrams.push(ed25519_challenge_scalar_bytes(
                &item.signature,
                pk.as_bytes(),
                item.message,
            ));
            sigs.push(item.signature);
            pks.push(item.public_key);
            map.push(parsed.len());
        }

        #[cfg(feature = "cuda")]
        if let Some((ref mut sigs, ref mut pks, ref mut hrams, ref mut map)) = cuda_inputs {
            hrams.push(ed25519_challenge_scalar_bytes(
                &item.signature,
                pk.as_bytes(),
                item.message,
            ));
            sigs.push(item.signature);
            pks.push(item.public_key);
            map.push(parsed.len());
        }

        parsed.push(Some((sig, pk)));
    }

    #[cfg(all(target_os = "macos", feature = "metal"))]
    if let Some((sigs, pks, hrams, map)) = metal_inputs
        && !sigs.is_empty()
        && let Some(out) = crate::vector::metal_ed25519_verify_batch(&sigs, &pks, &hrams)
        && out.len() == map.len()
    {
        let mut results = vec![false; items.len()];
        for (idx, ok) in map.into_iter().zip(out.into_iter()) {
            results[idx] = ok;
        }
        return results;
    }

    #[cfg(feature = "cuda")]
    if let Some((sigs, pks, hrams, map)) = cuda_inputs
        && !sigs.is_empty()
        && let Some(out) = crate::cuda::ed25519_verify_batch_cuda(&sigs, &pks, &hrams)
        && out.len() == map.len()
    {
        let mut results = vec![false; items.len()];
        for (idx, ok) in map.into_iter().zip(out.into_iter()) {
            results[idx] = ok;
        }
        return results;
    }

    let mut results = Vec::with_capacity(items.len());
    for (idx, parsed_item) in parsed.into_iter().enumerate() {
        let Some((sig, pk)) = parsed_item else {
            results.push(false);
            continue;
        };

        #[cfg(feature = "cuda")]
        if maybe_cuda
            && let Some(res) = crate::cuda::ed25519_verify_cuda(
                items[idx].message,
                &items[idx].signature,
                &items[idx].public_key,
            )
        {
            results.push(res);
            continue;
        }

        results.push(pk.verify_strict(items[idx].message, &sig).is_ok());
    }
    results
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};

    use super::{
        Ed25519BatchEntry, Ed25519BatchError, Ed25519BatchItem, Ed25519BatchRequest,
        SignatureScheme, ed25519_challenge_scalar_bytes, verify_ed25519_batch,
        verify_ed25519_batch_items, verify_signature,
    };

    const ED25519_SMALL_ORDER_POINT: [u8; 32] = [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];

    const ED25519_NONCANONICAL_IDENTITY: [u8; 32] = [
        0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];

    const ED25519_NONCANONICAL_NON_SMALL_ORDER_POINT: [u8; 32] = [
        0xf0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];

    #[test]
    fn ed25519_challenge_scalar_bytes_matches_cuda_selftest_vector() {
        let key = SigningKey::from_bytes(&[9u8; 32]);
        let pk = key.verifying_key();
        let msg = b"ivm-cuda-ed25519-selftest";
        let sig = key.sign(msg).to_bytes();
        let challenge = ed25519_challenge_scalar_bytes(&sig, &pk.to_bytes(), msg);
        assert_eq!(
            challenge,
            [
                11, 206, 0, 212, 13, 173, 142, 46, 14, 138, 231, 206, 197, 55, 196, 174, 14, 226,
                226, 94, 167, 116, 83, 92, 52, 73, 153, 137, 167, 80, 231, 15,
            ],
            "the shared Ed25519 GPU challenge helper must stay stable for the CUDA self-test truth set",
        );
    }

    #[test]
    fn ed25519_verifiers_reject_weak_public_key_material() {
        let key = SigningKey::from_bytes(&[0x31; 32]);
        let message = b"ivm-ed25519-weak-public-key";
        let signature = key.sign(message).to_bytes();
        let weak_public_key = [0u8; 32];

        assert!(!verify_signature(
            SignatureScheme::Ed25519,
            message,
            &signature,
            &weak_public_key
        ));

        let request = Ed25519BatchRequest {
            entries: vec![Ed25519BatchEntry {
                message: message.to_vec(),
                signature: signature.to_vec(),
                public_key: weak_public_key.to_vec(),
            }],
        };
        assert_eq!(
            verify_ed25519_batch(&request, 1),
            Err(Ed25519BatchError::InvalidEntry { index: 0 })
        );

        let items = [Ed25519BatchItem {
            message,
            signature,
            public_key: weak_public_key,
        }];
        assert_eq!(verify_ed25519_batch_items(&items), vec![false]);
    }

    #[test]
    fn ed25519_verifiers_reject_noncanonical_or_small_order_public_key_material() {
        let key = SigningKey::from_bytes(&[0x33; 32]);
        let message = b"ivm-ed25519-malformed-public-key";
        let signature = key.sign(message).to_bytes();

        for (label, public_key) in [
            ("small-order", ED25519_SMALL_ORDER_POINT),
            ("noncanonical", ED25519_NONCANONICAL_IDENTITY),
            (
                "noncanonical-non-small-order",
                ED25519_NONCANONICAL_NON_SMALL_ORDER_POINT,
            ),
        ] {
            assert!(
                !verify_signature(SignatureScheme::Ed25519, message, &signature, &public_key),
                "{label} public key must reject in single verification"
            );

            let request = Ed25519BatchRequest {
                entries: vec![Ed25519BatchEntry {
                    message: message.to_vec(),
                    signature: signature.to_vec(),
                    public_key: public_key.to_vec(),
                }],
            };
            assert_eq!(
                verify_ed25519_batch(&request, 1),
                Err(Ed25519BatchError::InvalidEntry { index: 0 }),
                "{label} public key must reject as malformed in Norito batch verification"
            );

            let items = [Ed25519BatchItem {
                message,
                signature,
                public_key,
            }];
            assert_eq!(
                verify_ed25519_batch_items(&items),
                vec![false],
                "{label} public key must reject in item batch verification"
            );
        }
    }

    #[test]
    fn ed25519_verifiers_reject_malformed_signature_r() {
        let key = SigningKey::from_bytes(&[0x32; 32]);
        let message = b"ivm-ed25519-malformed-r";
        let public_key = key.verifying_key().to_bytes();

        for (label, replacement_r) in [
            ("small-order", ED25519_SMALL_ORDER_POINT),
            ("noncanonical", ED25519_NONCANONICAL_IDENTITY),
        ] {
            let mut signature = key.sign(message).to_bytes();
            signature[..replacement_r.len()].copy_from_slice(&replacement_r);

            assert!(
                !verify_signature(SignatureScheme::Ed25519, message, &signature, &public_key),
                "{label} signature R must reject in single verification"
            );

            let request = Ed25519BatchRequest {
                entries: vec![Ed25519BatchEntry {
                    message: message.to_vec(),
                    signature: signature.to_vec(),
                    public_key: public_key.to_vec(),
                }],
            };
            assert_eq!(
                verify_ed25519_batch(&request, 1),
                Err(Ed25519BatchError::SignatureFailed { index: 0 }),
                "{label} signature R must reject in Norito batch verification"
            );

            let items = [Ed25519BatchItem {
                message,
                signature,
                public_key,
            }];
            assert_eq!(
                verify_ed25519_batch_items(&items),
                vec![false],
                "{label} signature R must reject in item batch verification"
            );
        }
    }
}
