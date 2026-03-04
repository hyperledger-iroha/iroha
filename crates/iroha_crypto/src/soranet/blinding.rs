//! CID blinding helpers used by the `SoraNet` anonymity overlay.
//!
//! The primitives in this module derive a per-circuit blinding key from the
//! daily salt and the handshake transcript secret. Requests can then be bound
//! to an additional nonce so that popular content remains unlinkable across
//! distinct circuits, while the exit gateway retains a deterministic cache key
//! derived from the published salt.

use std::fmt;

use blake3::Hasher;
use soranet_pq::{HkdfDomain, HkdfSuite, derive_labeled_hkdf};
use thiserror::Error;
use zeroize::Zeroizing;

use crate::secrecy::{ExposeSecret, Secret};

/// Length in bytes of the derived blinding key.
pub const BLINDING_KEY_LEN: usize = 32;
/// Length in bytes of a request nonce.
pub const REQUEST_NONCE_LEN: usize = 16;
/// Length in bytes of the resulting blinded CID digest.
pub const BLINDED_CID_LEN: usize = 32;

const HKDF_SALT: &[u8] = b"soranet.blinding.hkdf.salt.v1";
const HKDF_INFO: &[u8] = b"soranet.blinding.hkdf.info.v1";
const CANONICAL_DOMAIN: &[u8] = b"soranet.blinding.canonical.v1";
const REQUEST_DOMAIN: &[u8] = b"soranet.blinding.request.v1";
const CIRCUIT_DOMAIN: &[u8] = b"soranet.blinding.circuit.v1";

/// Errors surfaced while deriving or applying `SoraNet` CID blinding.
#[derive(Debug, Error, PartialEq, Eq, Copy, Clone)]
pub enum BlindingError {
    /// HKDF expansion failed. This should only occur if the requested output
    /// length is unsupported by the underlying hash function.
    #[error("soranet hkdf expansion failed")]
    Hkdf,
}

/// Deterministic blinding key derived from the daily salt and circuit secret.
#[derive(Clone)]
pub struct CircuitBlindingKey(Secret<Zeroizing<[u8; BLINDING_KEY_LEN]>>);

impl CircuitBlindingKey {
    /// Derive a new blinding key from the published salt and circuit secret.
    ///
    /// The circuit secret should be produced by the `SoraNet` hybrid Noise
    /// handshake and is assumed to be unique per circuit. Mixing it with the
    /// public salt ensures requests issued over different circuits (or
    /// different days) become unlinkable even for popular content.
    ///
    /// # Errors
    ///
    /// Returns [`BlindingError::Hkdf`] when HKDF cannot expand into a 32-byte
    /// key. This is not expected under normal operation.
    pub fn derive(epoch_salt: &[u8; 32], circuit_secret: &[u8; 32]) -> Result<Self, BlindingError> {
        let mut ikm = Zeroizing::new([0_u8; 64]);
        ikm[..32].copy_from_slice(epoch_salt);
        ikm[32..].copy_from_slice(circuit_secret);

        let okm = derive_labeled_hkdf(
            HkdfSuite::Sha3_256,
            Some(HKDF_SALT),
            ikm.as_ref(),
            HkdfDomain::soranet("blinding/key"),
            HKDF_INFO,
            BLINDING_KEY_LEN,
        )
        .map_err(|_| BlindingError::Hkdf)?;

        let mut key_bytes = [0_u8; BLINDING_KEY_LEN];
        key_bytes.copy_from_slice(okm.as_slice());
        Ok(Self(Secret::new(Zeroizing::new(key_bytes))))
    }

    fn key_bytes(&self) -> &[u8; BLINDING_KEY_LEN] {
        self.0.expose_secret()
    }

    /// Bind a manifest CID to the circuit key directly. This value is stable
    /// for the lifetime of the circuit and can be used by the exit gateway to
    /// maintain deterministic lookups without exposing the canonical cache key.
    #[must_use]
    pub fn circuit_scoped_blinded(&self, cid: &[u8]) -> BlindedCid {
        let mut hasher = Hasher::new_keyed(self.key_bytes());
        hasher.update(CIRCUIT_DOMAIN);
        hasher.update(cid);
        BlindedCid::from_hash(hasher.finalize())
    }

    /// Produce a request-scoped blinded CID by incorporating an explicit
    /// per-request nonce. Distinct nonces yield unlinkable digests, limiting
    /// passive correlation of popular content.
    #[must_use]
    pub fn request_scoped_blinded(&self, cid: &[u8], nonce: &RequestNonce) -> BlindedCid {
        let mut hasher = Hasher::new_keyed(self.key_bytes());
        hasher.update(REQUEST_DOMAIN);
        hasher.update(nonce.as_bytes());
        hasher.update(cid);
        BlindedCid::from_hash(hasher.finalize())
    }
}

/// Canonical cache key derived solely from the public salt. Gateways persist
/// data against this digest to ensure deterministic storage layouts that remain
/// verifiable during audits.
#[must_use]
pub fn canonical_cache_key(epoch_salt: &[u8; 32], cid: &[u8]) -> BlindedCid {
    let mut hasher = Hasher::new();
    hasher.update(CANONICAL_DOMAIN);
    hasher.update(epoch_salt);
    hasher.update(cid);
    BlindedCid::from_hash(hasher.finalize())
}

/// Opaque 16-byte nonce injected into request blinding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestNonce([u8; REQUEST_NONCE_LEN]);

impl RequestNonce {
    /// Construct a nonce from raw bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; REQUEST_NONCE_LEN]) -> Self {
        Self(bytes)
    }

    /// Borrow the nonce bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; REQUEST_NONCE_LEN] {
        &self.0
    }

    /// Populate the nonce with random bytes using the provided RNG.
    ///
    /// # Panics
    ///
    /// Panics if the RNG fails to fill the buffer, matching the behaviour of
    /// `RngCore::fill_bytes`.
    #[cfg(feature = "rand")]
    #[must_use]
    pub fn random<R>(rng: &mut R) -> Self
    where
        R: rand::RngCore,
    {
        let mut buf = [0_u8; REQUEST_NONCE_LEN];
        rng.fill_bytes(&mut buf);
        Self(buf)
    }
}

/// 32-byte digest representing a blinded CID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlindedCid([u8; BLINDED_CID_LEN]);

impl BlindedCid {
    fn from_hash(hash: blake3::Hash) -> Self {
        let mut out = [0_u8; BLINDED_CID_LEN];
        out.copy_from_slice(hash.as_bytes());
        Self(out)
    }

    /// Return the raw digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; BLINDED_CID_LEN] {
        &self.0
    }
}

impl AsRef<[u8; BLINDED_CID_LEN]> for BlindedCid {
    fn as_ref(&self) -> &[u8; BLINDED_CID_LEN] {
        self.as_bytes()
    }
}

impl fmt::Debug for CircuitBlindingKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CircuitBlindingKey([REDACTED])")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_cache_key_matches_reference() {
        let salt = [0x11_u8; 32];
        let cid = b"example-cid";

        let candidate = canonical_cache_key(&salt, cid);

        // Precomputed using the same algorithm to lock the domain separation tag.
        let mut hasher = Hasher::new();
        hasher.update(CANONICAL_DOMAIN);
        hasher.update(&salt);
        hasher.update(cid);
        let expected = hasher.finalize();

        assert_eq!(candidate.as_bytes(), expected.as_bytes());
        assert_eq!(
            hex::encode(candidate.as_bytes()),
            "79e723b8218a6adb35009f69985dd844de3dfa6deac01d063929bc8aa05223a5"
        );
    }

    #[test]
    fn request_blinding_differs_per_nonce() {
        let salt = [0x42_u8; 32];
        let mut circuit_secret = [0x24_u8; 32];
        circuit_secret[0] = 0x01;

        let key = CircuitBlindingKey::derive(&salt, &circuit_secret).expect("hkdf");
        let cid = b"popular-content";

        let nonce_a = RequestNonce::from_bytes([0xAA; REQUEST_NONCE_LEN]);
        let nonce_b = RequestNonce::from_bytes([0xBB; REQUEST_NONCE_LEN]);

        let blinded_a = key.request_scoped_blinded(cid, &nonce_a);
        let blinded_b = key.request_scoped_blinded(cid, &nonce_b);

        assert_ne!(blinded_a, blinded_b);
        // Same nonce => same digest to support deterministic retries.
        let blinded_a_again = key.request_scoped_blinded(cid, &nonce_a);
        assert_eq!(blinded_a, blinded_a_again);
    }

    #[test]
    fn circuit_scoped_blinding_breaks_cross_circuit_correlation() {
        let salt = [0x99_u8; 32];
        let cid = b"shared-object";

        let key_a =
            CircuitBlindingKey::derive(&salt, &[0x10_u8; 32]).expect("hkdf derivation failed");
        let key_b =
            CircuitBlindingKey::derive(&salt, &[0x11_u8; 32]).expect("hkdf derivation failed");

        let blinded_a = key_a.circuit_scoped_blinded(cid);
        let blinded_b = key_b.circuit_scoped_blinded(cid);
        let blinded_c = key_b.circuit_scoped_blinded(b"different-object");

        assert_ne!(blinded_a, blinded_b);
        assert_ne!(blinded_b, blinded_c);
        assert_ne!(blinded_a, blinded_c);
    }
}
