//! Confidential key hierarchy utilities (wallet-facing).
//!
//! These helpers derive the nullifier/viewing keys from a 32-byte spend key
//! using a domain-separated HKDF. They are intentionally lightweight so
//! wallets and offline tooling can reuse the exact same derivations as the
//! on-chain host.

use hkdf::Hkdf;
use rand_core::TryCryptoRng;
use sha3::Sha3_512;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Salt applied to the HKDF used for the confidential key hierarchy.
const KEY_SALT: &[u8] = b"iroha:confidential:key-derivation:v1";

/// HKDF info label for the nullifier key.
const INFO_NK: &[u8] = b"iroha:confidential:nk";
/// HKDF info label for the incoming viewing key.
const INFO_IVK: &[u8] = b"iroha:confidential:ivk";
/// HKDF info label for the outgoing viewing key.
const INFO_OVK: &[u8] = b"iroha:confidential:ovk";
/// HKDF info label for the full viewing key.
const INFO_FVK: &[u8] = b"iroha:confidential:fvk";

/// Confidential key derivation errors.
#[derive(Copy, Clone, Debug, thiserror::Error)]
pub enum ConfidentialKeyError {
    /// Spend key must be exactly 32 bytes.
    #[error("expected 32-byte spend key, got {0} bytes")]
    InvalidSpendKeyLength(usize),
    /// Random spend-key generation failed.
    #[error("random spend-key generation failed")]
    RandomBytes,
    /// HKDF expansion failed for the labelled derived key.
    #[error("HKDF expand for {label} failed")]
    HkdfExpand {
        /// Domain-separated key label being expanded.
        label: &'static str,
    },
}

/// Result type for confidential key derivations.
pub type Result<T, E = ConfidentialKeyError> = core::result::Result<T, E>;

/// Derived keys for confidential asset operations.
#[allow(missing_copy_implementations)]
#[derive(Clone)]
pub struct ConfidentialKeyset {
    spend: [u8; 32],
    nullifier: [u8; 32],
    incoming_view: [u8; 32],
    outgoing_view: [u8; 32],
    full_view: [u8; 32],
}

impl core::fmt::Debug for ConfidentialKeyset {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("ConfidentialKeyset { .. }")
    }
}

impl Zeroize for ConfidentialKeyset {
    fn zeroize(&mut self) {
        self.spend.zeroize();
        self.nullifier.zeroize();
        self.incoming_view.zeroize();
        self.outgoing_view.zeroize();
        self.full_view.zeroize();
    }
}

impl ZeroizeOnDrop for ConfidentialKeyset {}

impl ConfidentialKeyset {
    /// Spend key used to authorise note creation.
    #[must_use]
    pub const fn spend_key(&self) -> &[u8; 32] {
        &self.spend
    }

    /// Nullifier key (`nk`) used for deriving per-note nullifiers.
    #[must_use]
    pub const fn nullifier_key(&self) -> &[u8; 32] {
        &self.nullifier
    }

    /// Incoming viewing key (`ivk`) used to decrypt received notes.
    #[must_use]
    pub const fn incoming_view_key(&self) -> &[u8; 32] {
        &self.incoming_view
    }

    /// Outgoing viewing key (`ovk`) used to decrypt sent notes.
    #[must_use]
    pub const fn outgoing_view_key(&self) -> &[u8; 32] {
        &self.outgoing_view
    }

    /// Full viewing key (`fvk`) allows reconstructing note commitments/nullifiers without spend capability.
    #[must_use]
    pub const fn full_view_key(&self) -> &[u8; 32] {
        &self.full_view
    }
}

fn expand_key(hkdf: &Hkdf<Sha3_512>, label: &'static str, info: &[u8]) -> Result<[u8; 32]> {
    let mut out = [0u8; 32];
    hkdf.expand(info, &mut out)
        .map_err(|_| ConfidentialKeyError::HkdfExpand { label })?;
    Ok(out)
}

/// Derive the confidential key hierarchy from a 32-byte spend key.
///
/// # Errors
/// Returns [`ConfidentialKeyError::HkdfExpand`] if domain-separated key expansion fails.
pub fn derive_keyset(spend_key: [u8; 32]) -> Result<ConfidentialKeyset> {
    let hkdf = Hkdf::<Sha3_512>::new(Some(KEY_SALT), &spend_key);

    let nk = expand_key(&hkdf, "nk", INFO_NK)?;
    let ivk = expand_key(&hkdf, "ivk", INFO_IVK)?;
    let ovk = expand_key(&hkdf, "ovk", INFO_OVK)?;
    let fvk = expand_key(&hkdf, "fvk", INFO_FVK)?;

    Ok(ConfidentialKeyset {
        spend: spend_key,
        nullifier: nk,
        incoming_view: ivk,
        outgoing_view: ovk,
        full_view: fvk,
    })
}

/// Derive the confidential key hierarchy from an arbitrary slice.
///
/// # Errors
/// Returns [`ConfidentialKeyError::InvalidSpendKeyLength`] when the slice does not contain exactly 32 bytes,
/// or [`ConfidentialKeyError::HkdfExpand`] if key expansion fails.
pub fn derive_keyset_from_slice(spend_key: &[u8]) -> Result<ConfidentialKeyset> {
    if spend_key.len() != 32 {
        return Err(ConfidentialKeyError::InvalidSpendKeyLength(spend_key.len()));
    }
    let mut seed = [0u8; 32];
    seed.copy_from_slice(spend_key);
    derive_keyset(seed)
}

/// Generate a fresh random spend key and derive the associated hierarchy.
///
/// # Errors
/// Returns [`ConfidentialKeyError::RandomBytes`] if the RNG cannot provide a spend key, or
/// [`ConfidentialKeyError::HkdfExpand`] if key expansion fails.
pub fn generate_keyset<R: TryCryptoRng>(rng: &mut R) -> Result<ConfidentialKeyset> {
    let mut seed = [0u8; 32];
    rng.try_fill_bytes(&mut seed)
        .map_err(|_| ConfidentialKeyError::RandomBytes)?;
    derive_keyset(seed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng as _;
    use rand_core::TryRngCore;

    #[test]
    fn derive_keyset_is_deterministic() {
        let seed = [0x11u8; 32];
        let first = derive_keyset(seed).expect("derive first keyset");
        let second = derive_keyset(seed).expect("derive second keyset");
        assert_eq!(first.nullifier_key(), second.nullifier_key());
        assert_eq!(first.incoming_view_key(), second.incoming_view_key());
        assert_eq!(first.outgoing_view_key(), second.outgoing_view_key());
        assert_eq!(first.full_view_key(), second.full_view_key());
    }

    #[test]
    fn derive_keyset_from_slice_rejects_wrong_length() {
        assert!(derive_keyset_from_slice(&[0u8; 31]).is_err());
    }

    #[test]
    fn derive_keyset_matches_expected_vectors() {
        let seed = [0x42u8; 32];
        let keyset = derive_keyset(seed).expect("derive keyset");
        assert_eq!(
            hex::encode(keyset.nullifier_key()),
            "cb7149cc545b97fe5ab1ffe85550f9b0146f3dbff7cf9d2921b9432b641bf0dc"
        );
        assert_eq!(
            hex::encode(keyset.incoming_view_key()),
            "fc0f3bf333d454923522f723ef589e0ca31ac1206724b1cd607e41ef0d4230f7"
        );
        assert_eq!(
            hex::encode(keyset.outgoing_view_key()),
            "5dc50806af739fa5577484268fd77c4e2345c70dae5b55a132b4f9b1a3e00c4c"
        );
        assert_eq!(
            hex::encode(keyset.full_view_key()),
            "9a0fe79f768aeb440e07751dbddfa17ac97cbf21f3e79c2e0206e56b3c2629af"
        );
    }

    #[test]
    fn generate_keyset_derives_from_rng_bytes() {
        let mut rng = rand::rngs::StdRng::from_seed([0xA5; 32]);
        let mut expected_rng = rand::rngs::StdRng::from_seed([0xA5; 32]);
        let mut expected_seed = [0u8; 32];
        expected_rng
            .try_fill_bytes(&mut expected_seed)
            .expect("test RNG should fill expected seed");

        let generated = generate_keyset(&mut rng).expect("generate keyset");
        let expected = derive_keyset(expected_seed).expect("derive expected keyset");

        assert_eq!(generated.spend_key(), &expected_seed);
        assert_eq!(generated.nullifier_key(), expected.nullifier_key());
        assert_eq!(generated.incoming_view_key(), expected.incoming_view_key());
        assert_eq!(generated.outgoing_view_key(), expected.outgoing_view_key());
        assert_eq!(generated.full_view_key(), expected.full_view_key());
    }

    #[test]
    fn generate_keyset_reports_rng_failure() {
        struct FailingRng;

        #[derive(Debug)]
        struct FailingRngError;

        impl core::fmt::Display for FailingRngError {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str("failing confidential rng")
            }
        }

        impl std::error::Error for FailingRngError {}

        impl TryRngCore for FailingRng {
            type Error = FailingRngError;

            fn try_next_u32(&mut self) -> core::result::Result<u32, Self::Error> {
                Err(FailingRngError)
            }

            fn try_next_u64(&mut self) -> core::result::Result<u64, Self::Error> {
                Err(FailingRngError)
            }

            fn try_fill_bytes(
                &mut self,
                _dest: &mut [u8],
            ) -> core::result::Result<(), Self::Error> {
                Err(FailingRngError)
            }
        }

        impl TryCryptoRng for FailingRng {}

        let mut rng = FailingRng;

        assert!(matches!(
            generate_keyset(&mut rng),
            Err(ConfidentialKeyError::RandomBytes)
        ));
    }

    #[test]
    #[ignore = "generates example vectors for manual inspection"]
    fn dump_confidential_vectors() {
        for byte in [0x00u8, 0x42, 0xFF] {
            let seed = [byte; 32];
            let keyset = derive_keyset(seed).expect("derive keyset");
            println!(
                "seed={:02x} nk={} ivk={} ovk={} fvk={}",
                byte,
                hex::encode(keyset.nullifier_key()),
                hex::encode(keyset.incoming_view_key()),
                hex::encode(keyset.outgoing_view_key()),
                hex::encode(keyset.full_view_key())
            );
        }
    }
}
