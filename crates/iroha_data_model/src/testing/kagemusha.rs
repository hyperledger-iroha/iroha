//! Deterministic KAGEMUSHA V1 signing helpers for cross-crate tests.

use p256::ecdsa::{Signature as P256Signature, SigningKey, signature::Signer as _};

use crate::kagemusha::{KagemushaDevicePublicKeyV1, KagemushaDeviceSignatureV1};

/// Deterministic software signer available only through the `test-fixtures`
/// feature.
///
/// Production KAGEMUSHA code never exposes software signing authority; real
/// device and terminal signatures must come from an admitted hardware profile.
#[derive(Clone)]
pub struct KagemushaFixtureSignerV1(SigningKey);

impl KagemushaFixtureSignerV1 {
    /// Construct a deterministic fixture signer from one seed byte.
    ///
    /// Valid repeated-byte P-256 scalars retain that simple representation.
    /// The two invalid boundary encodings fall back to the unique scalar
    /// `seed + 1`, so every `u8` input is accepted without a panic.
    pub fn from_repeated_byte(seed: u8) -> Self {
        let repeated = [seed; 32];
        let key = SigningKey::from_bytes((&repeated).into()).unwrap_or_else(|_| {
            let mut fallback = [0_u8; 32];
            fallback[30..].copy_from_slice(&(u16::from(seed) + 1).to_be_bytes());
            SigningKey::from_bytes((&fallback).into())
                .expect("seed + 1 is always a valid non-zero P-256 fixture scalar")
        });
        Self(key)
    }

    /// Return the uncompressed device public key used by hardware credentials
    /// and peer requests.
    pub fn device_public_key(&self) -> KagemushaDevicePublicKeyV1 {
        KagemushaDevicePublicKeyV1::from_sec1_bytes(
            self.0.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .expect("fixture signer must produce a canonical device public key")
    }

    /// Sign fixture bytes with deterministic RFC 6979 P-256 ECDSA and normalize
    /// the result to low-S form.
    pub fn sign(&self, bytes: &[u8]) -> KagemushaDeviceSignatureV1 {
        let signature: P256Signature = self.0.sign(bytes);
        let signature = signature.normalize_s().unwrap_or(signature);
        KagemushaDeviceSignatureV1::from_raw_bytes(signature.to_bytes().as_ref())
            .expect("fixture signer must produce a canonical low-S signature")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_seed_byte_builds_a_signer_with_verifiable_low_s_output() {
        for seed in u8::MIN..=u8::MAX {
            let signer = KagemushaFixtureSignerV1::from_repeated_byte(seed);
            let message = [seed; 3];
            signer
                .sign(&message)
                .verify(&signer.device_public_key(), &message)
                .expect("fixture signature must verify");
        }
    }
}
