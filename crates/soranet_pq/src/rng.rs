use blake3::Hasher;
use rand::rngs::OsRng;
use rand_chacha::ChaCha20Rng;
use rand_core::{SeedableRng, TryRngCore};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Seed material used for hedged RNG construction.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct HedgedRngSeed {
    seed: [u8; 32],
}

impl HedgedRngSeed {
    /// Create a seed from raw entropy (32 bytes).
    #[must_use]
    pub const fn from_entropy(seed: [u8; 32]) -> Self {
        Self { seed }
    }

    /// Draw a fresh seed using the operating system RNG.
    pub fn from_os() -> Self {
        let mut buf = [0_u8; 32];
        let mut os = OsRng;
        os.try_fill_bytes(&mut buf).expect("OS RNG failure");
        Self { seed: buf }
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
pub fn hedged_chacha20_rng(seed: HedgedRngSeed, personalization: &[u8]) -> ChaCha20Rng {
    let mut os_entropy = [0_u8; 32];
    let mut os = OsRng;
    os.try_fill_bytes(&mut os_entropy).expect("OS RNG failure");
    build_rng(&seed, personalization, &os_entropy)
}

fn build_rng(seed: &HedgedRngSeed, personalization: &[u8], os_entropy: &[u8; 32]) -> ChaCha20Rng {
    let mut hasher = Hasher::new();
    hasher.update(seed.as_bytes());
    hasher.update(os_entropy);
    hasher.update(personalization);
    let mut derived = [0_u8; 32];
    derived.copy_from_slice(hasher.finalize().as_bytes());
    ChaCha20Rng::from_seed(derived)
}

#[cfg(test)]
mod tests {
    use rand::RngCore;

    use super::*;

    #[test]
    fn personalization_affects_stream() {
        let seed = HedgedRngSeed::from_entropy([0xA5; 32]);
        let os = [0x11; 32];

        let mut rng_a = build_rng(&seed, b"A", &os);
        let mut rng_b = build_rng(&seed, b"B", &os);

        assert_ne!(rng_a.next_u64(), rng_b.next_u64());
    }

    #[test]
    fn hedged_rng_changes_with_os_entropy() {
        let seed = HedgedRngSeed::from_entropy([0x5C; 32]);
        let os_a = [0x22; 32];
        let os_b = [0x23; 32];

        let mut rng_a = build_rng(&seed, b"", &os_a);
        let mut rng_b = build_rng(&seed, b"", &os_b);

        assert_ne!(rng_a.next_u64(), rng_b.next_u64());
    }

    #[test]
    fn deterministic_with_fixed_inputs() {
        let seed = HedgedRngSeed::from_entropy([0x42; 32]);
        let os = [0x99; 32];

        let mut first = build_rng(&seed, b"context", &os);
        let mut second = build_rng(&seed, b"context", &os);

        assert_eq!(first.next_u64(), second.next_u64());
        assert_eq!(first.next_u64(), second.next_u64());
    }
}
