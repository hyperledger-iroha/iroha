use rand::rngs::OsRng;
use rand_chacha::ChaChaRng;
use rand_core::{
    CryptoRng as CryptoRngNew, RngCore as RngCoreNew, SeedableRng, TryCryptoRng,
    TryRngCore as TryRngCoreNew,
};
use rand_core_06::{self, CryptoRng as CryptoRngOld, RngCore as RngCoreOld};
use sha2::{Digest, Sha256};
use zeroize::Zeroize;

/// Adapter that makes a modern `rand_core` RNG implement both the new (0.9)
/// and 0.6 trait sets required by downstream dependencies (e.g., `signature`,
/// `ed25519-dalek`, `x25519-dalek`).
pub struct CompatRng<R> {
    inner: R,
}

impl<R> CompatRng<R> {
    pub fn new(inner: R) -> Self {
        Self { inner }
    }
}

impl<R> RngCoreNew for CompatRng<R>
where
    R: TryRngCoreNew,
{
    fn next_u32(&mut self) -> u32 {
        self.inner
            .try_next_u32()
            .expect("secure RNG should not fail")
    }

    fn next_u64(&mut self) -> u64 {
        self.inner
            .try_next_u64()
            .expect("secure RNG should not fail")
    }

    fn fill_bytes(&mut self, dst: &mut [u8]) {
        self.inner
            .try_fill_bytes(dst)
            .expect("secure RNG should not fail");
    }
}

impl<R> CryptoRngNew for CompatRng<R> where R: TryRngCoreNew + TryCryptoRng {}

impl<R> RngCoreOld for CompatRng<R>
where
    R: TryRngCoreNew,
    R::Error: std::error::Error + Send + Sync + 'static,
{
    fn next_u32(&mut self) -> u32 {
        self.inner
            .try_next_u32()
            .expect("secure RNG should not fail")
    }

    fn next_u64(&mut self) -> u64 {
        self.inner
            .try_next_u64()
            .expect("secure RNG should not fail")
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.inner
            .try_fill_bytes(dest)
            .expect("secure RNG should not fail");
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core_06::Error> {
        self.inner
            .try_fill_bytes(dest)
            .map_err(rand_core_06::Error::new)
    }
}

impl<R> CryptoRngOld for CompatRng<R> where R: TryRngCoreNew + TryCryptoRng {}

/// Deterministic RNG derived from an arbitrary seed via SHA-256, implementing both
/// modern and 0.6 `rand_core` traits.
pub fn rng_from_seed(mut seed: Vec<u8>) -> CompatRng<ChaChaRng> {
    let hash = Sha256::digest(&seed);
    seed.zeroize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&hash);
    CompatRng::new(ChaChaRng::from_seed(key))
}

/// Operating-system RNG wrapped in the dual-trait adapter.
pub fn os_rng() -> CompatRng<OsRng> {
    CompatRng::new(OsRng)
}
