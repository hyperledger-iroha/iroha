use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;
use sha2::{Digest, Sha256};
use zeroize::Zeroize;
/// Deterministic RNG derived from seed material via SHA-256, using the canonical `ChaCha20` stream.
///
/// # Security
///
/// SHA-256 normalizes the seed length; it does not add entropy. Use a secret seed with at least 256
/// bits of entropy for production key derivation. Prefer operating-system-backed random key
/// generation unless reproducibility is an explicit protocol requirement.
pub fn rng_from_seed(mut seed: Vec<u8>) -> ChaCha20Rng {
    let rng = rng_from_seed_slice(&seed);
    seed.zeroize();
    rng
}
/// Deterministic RNG derived from borrowed seed bytes via SHA-256, using
/// the canonical `ChaCha20` stream.
///
/// # Security
///
/// This function is an entropy-preserving deterministic transform, not a
/// password-based key derivation function. Public identifiers, labels, and
/// human-chosen strings are not suitable production seeds.
pub fn rng_from_seed_slice(seed: &[u8]) -> ChaCha20Rng {
    let mut key: [u8; 32] = Sha256::digest(seed).into();
    let rng = ChaCha20Rng::from_seed(key);
    key.zeroize();
    rng
}
/// Canonical seed stream for w3f-bls's upstream `rand_core` 0.6 field sampler.
///
/// This generator belongs to that provider boundary; it implements only the
/// upstream RNG API. Its byte stream equals the public `ChaCha20` derivation.
#[cfg(feature = "bls")]
pub fn bls_rng_from_seed_slice(seed: &[u8]) -> rand_chacha_03::ChaCha20Rng {
    use rand_chacha_03::rand_core::SeedableRng as _;
    let mut key: [u8; 32] = Sha256::digest(seed).into();
    let rng = rand_chacha_03::ChaCha20Rng::from_seed(key);
    key.zeroize();
    rng
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rng_from_seed_is_deterministic() {
        let mut first_rng = rng_from_seed(b"deterministic seed".to_vec());
        let mut second_rng = rng_from_seed(b"deterministic seed".to_vec());
        let mut first = [0u8; 64];
        let mut second = [0u8; 64];
        rand_core::Rng::fill_bytes(&mut first_rng, &mut first);
        rand_core::Rng::fill_bytes(&mut second_rng, &mut second);
        assert_eq!(first, second);
    }
    #[test]
    fn rng_from_seed_slice_matches_owned_seed() {
        let seed = b"deterministic borrowed seed";
        let mut owned_rng = rng_from_seed(seed.to_vec());
        let mut borrowed_rng = rng_from_seed_slice(seed);
        let mut owned = [0u8; 64];
        let mut borrowed = [0u8; 64];
        rand_core::Rng::fill_bytes(&mut owned_rng, &mut owned);
        rand_core::Rng::fill_bytes(&mut borrowed_rng, &mut borrowed);
        assert_eq!(owned, borrowed);
    }
    #[test]
    fn deterministic_stream_matches_independent_chacha20_vector() {
        // SHA-256(seed), ChaCha20 counter=0, nonce=0: independently computed
        // using Python cryptography/OpenSSL before upgrading rand_chacha.
        let mut rng = rng_from_seed_slice(b"deterministic seed");
        let mut actual = [0_u8; 64];
        rand_core::Rng::fill_bytes(&mut rng, &mut actual);
        assert_eq!(
            actual,
            hex_literal::hex!(
                "b7f07aa19ad2281f796d6532f886ea50ac7866673605cb2f39843a4e7226f033f
             de09f6322af2ce1935e2db1e0b734f863b70aa8f536acd6fb2f22956f8ce31a"
            )
        );
    }

    #[cfg(feature = "bls")]
    #[test]
    fn bls_sampler_stream_matches_canonical_chacha20_stream() {
        let seed = b"BLS upstream field sampling";
        let mut modern = rng_from_seed_slice(seed);
        let mut bls = bls_rng_from_seed_slice(seed);
        let mut modern_bytes = [0_u8; 4096];
        let mut bls_bytes = [0_u8; 4096];
        rand_core::Rng::fill_bytes(&mut modern, &mut modern_bytes);
        rand_chacha_03::rand_core::RngCore::fill_bytes(&mut bls, &mut bls_bytes);
        assert_eq!(modern_bytes, bls_bytes);
    }
}
