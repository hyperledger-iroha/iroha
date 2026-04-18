use blake3::Hasher;
use rand::rngs::OsRng;
use rand_chacha::ChaCha20Rng;
use rand_core::{CryptoRng, RngCore, SeedableRng, TryRngCore};
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop};

const HEDGED_RNG_DOMAIN: &[u8] = b"soranet-pq:hedged-chacha20:v2";
const OS_ENTROPY_MIXED: &[u8] = b"os-entropy:mixed";
const OS_ENTROPY_UNAVAILABLE: &[u8] = b"os-entropy:unavailable";

/// Seed material used for hedged RNG construction.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct HedgedRngSeed {
    seed: [u8; 32],
}

/// Error returned when a required operating-system entropy draw fails.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("operating-system RNG failed while drawing hedged RNG seed material")]
pub struct RngError;

/// Whether a hedged RNG construction successfully mixed live OS entropy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HedgedEntropyStatus {
    /// Live operating-system entropy was mixed with the deterministic seed.
    MixedOsEntropy,
    /// The operating-system entropy draw failed and derivation used only the deterministic seed.
    OsEntropyUnavailable,
}

/// ChaCha20 RNG derived from deterministic seed material and, when available,
/// live operating-system entropy.
pub struct HedgedChaCha20Rng {
    inner: ChaCha20Rng,
    status: HedgedEntropyStatus,
}

impl HedgedChaCha20Rng {
    /// Report whether live OS entropy was successfully mixed into this RNG.
    #[must_use]
    pub const fn entropy_status(&self) -> HedgedEntropyStatus {
        self.status
    }
}

impl RngCore for HedgedChaCha20Rng {
    fn next_u32(&mut self) -> u32 {
        self.inner.next_u32()
    }

    fn next_u64(&mut self) -> u64 {
        self.inner.next_u64()
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.inner.fill_bytes(dest);
    }
}

impl CryptoRng for HedgedChaCha20Rng {}

impl HedgedRngSeed {
    /// Create a seed from raw entropy (32 bytes).
    #[must_use]
    pub const fn from_entropy(seed: [u8; 32]) -> Self {
        Self { seed }
    }

    /// Draw a fresh seed using the operating system RNG.
    ///
    /// # Errors
    /// Returns [`RngError`] when the OS RNG cannot supply seed material.
    pub fn from_os() -> Result<Self, RngError> {
        let mut buf = [0_u8; 32];
        let mut os = OsRng;
        os.try_fill_bytes(&mut buf).map_err(|_| RngError)?;
        Ok(Self { seed: buf })
    }

    /// Borrow the underlying seed bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.seed
    }
}

/// Construct a `ChaCha20` RNG by hedging the provided seed with live OS entropy
/// and optional transcript context.
#[allow(clippy::needless_pass_by_value)] // Seed is consumed to trigger zeroization on drop.
#[must_use]
pub fn hedged_chacha20_rng(seed: HedgedRngSeed, personalization: &[u8]) -> HedgedChaCha20Rng {
    let mut os_entropy = [0_u8; 32];
    let mut os = OsRng;
    let status = match os.try_fill_bytes(&mut os_entropy) {
        Ok(()) => HedgedEntropyStatus::MixedOsEntropy,
        Err(_) => HedgedEntropyStatus::OsEntropyUnavailable,
    };
    build_rng(&seed, personalization, &os_entropy, status)
}

/// Construct a hedged RNG from fresh OS seed material.
///
/// This helper is for production paths that do not already have deterministic
/// seed material to mix in. Tests and deterministic fixtures should call
/// [`hedged_chacha20_rng`] with an explicit [`HedgedRngSeed`].
///
/// # Errors
/// Returns [`RngError`] when the initial OS seed draw fails.
pub fn hedged_chacha20_rng_from_os(
    personalization: &[u8],
) -> Result<HedgedChaCha20Rng, RngError> {
    HedgedRngSeed::from_os().map(|seed| hedged_chacha20_rng(seed, personalization))
}

fn build_rng(
    seed: &HedgedRngSeed,
    personalization: &[u8],
    os_entropy: &[u8; 32],
    status: HedgedEntropyStatus,
) -> HedgedChaCha20Rng {
    let mut hasher = Hasher::new();
    hasher.update(HEDGED_RNG_DOMAIN);
    hasher.update(match status {
        HedgedEntropyStatus::MixedOsEntropy => OS_ENTROPY_MIXED,
        HedgedEntropyStatus::OsEntropyUnavailable => OS_ENTROPY_UNAVAILABLE,
    });
    hasher.update(seed.as_bytes());
    hasher.update(os_entropy);
    hasher.update(personalization);
    let mut derived = [0_u8; 32];
    derived.copy_from_slice(hasher.finalize().as_bytes());
    HedgedChaCha20Rng {
        inner: ChaCha20Rng::from_seed(derived),
        status,
    }
}

#[cfg(test)]
mod tests {
    use rand::RngCore;

    use super::*;

    #[test]
    fn personalization_affects_stream() {
        let seed = HedgedRngSeed::from_entropy([0xA5; 32]);
        let os = [0x11; 32];

        let mut rng_a = build_rng(&seed, b"A", &os, HedgedEntropyStatus::MixedOsEntropy);
        let mut rng_b = build_rng(&seed, b"B", &os, HedgedEntropyStatus::MixedOsEntropy);

        assert_ne!(rng_a.next_u64(), rng_b.next_u64());
    }

    #[test]
    fn hedged_rng_changes_with_os_entropy() {
        let seed = HedgedRngSeed::from_entropy([0x5C; 32]);
        let os_a = [0x22; 32];
        let os_b = [0x23; 32];

        let mut rng_a = build_rng(&seed, b"", &os_a, HedgedEntropyStatus::MixedOsEntropy);
        let mut rng_b = build_rng(&seed, b"", &os_b, HedgedEntropyStatus::MixedOsEntropy);

        assert_ne!(rng_a.next_u64(), rng_b.next_u64());
    }

    #[test]
    fn deterministic_with_fixed_inputs() {
        let seed = HedgedRngSeed::from_entropy([0x42; 32]);
        let os = [0x99; 32];

        let mut first = build_rng(&seed, b"context", &os, HedgedEntropyStatus::MixedOsEntropy);
        let mut second = build_rng(&seed, b"context", &os, HedgedEntropyStatus::MixedOsEntropy);

        assert_eq!(first.next_u64(), second.next_u64());
        assert_eq!(first.next_u64(), second.next_u64());
    }

    #[test]
    fn entropy_status_affects_stream() {
        let seed = HedgedRngSeed::from_entropy([0x24; 32]);
        let os = [0x55; 32];

        let mut mixed = build_rng(&seed, b"context", &os, HedgedEntropyStatus::MixedOsEntropy);
        let mut unavailable = build_rng(
            &seed,
            b"context",
            &os,
            HedgedEntropyStatus::OsEntropyUnavailable,
        );

        assert_ne!(mixed.next_u64(), unavailable.next_u64());
    }

    #[test]
    fn public_constructor_reports_status() {
        let rng = hedged_chacha20_rng(HedgedRngSeed::from_entropy([0x31; 32]), b"status");

        assert!(matches!(
            rng.entropy_status(),
            HedgedEntropyStatus::MixedOsEntropy | HedgedEntropyStatus::OsEntropyUnavailable
        ));
    }
}
