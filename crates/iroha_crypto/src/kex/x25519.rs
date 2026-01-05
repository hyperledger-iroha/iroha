use hkdf::Hkdf;
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroize;

use super::KeyExchangeScheme;
#[cfg(feature = "rand")]
use crate::rng::os_rng;
use crate::{Error, KeyGenOption, SessionKey, error::ParseError, rng::rng_from_seed};

const HKDF_SALT: &[u8] = b"iroha:x25519:hkdf:v1";
const HKDF_INFO: &[u8] = b"iroha:x25519:session-key";

/// Implements the [`KeyExchangeScheme`] using X25519 key exchange and HKDF-SHA256 with
/// domain separation to derive the session key.
#[derive(Copy, Clone)]
pub struct X25519Sha256;

impl KeyExchangeScheme for X25519Sha256 {
    type PublicKey = PublicKey;
    type PrivateKey = StaticSecret;

    fn new() -> Self {
        Self
    }

    fn keypair(
        &self,
        mut option: KeyGenOption<Self::PrivateKey>,
    ) -> (Self::PublicKey, Self::PrivateKey) {
        match option {
            #[cfg(feature = "rand")]
            KeyGenOption::Random => {
                let sk = StaticSecret::random_from_rng(os_rng());
                let pk = PublicKey::from(&sk);
                (pk, sk)
            }
            KeyGenOption::UseSeed(ref mut s) => {
                let mut rng = rng_from_seed(s.clone());
                s.zeroize();
                let mut bytes = [0u8; 32];
                rand_core::RngCore::fill_bytes(&mut rng, &mut bytes);
                let sk = StaticSecret::from(bytes);
                let pk = PublicKey::from(&sk);
                (pk, sk)
            }
            KeyGenOption::FromPrivateKey(ref sk) => {
                let pk = PublicKey::from(sk);
                (pk, sk.clone())
            }
        }
    }

    fn compute_shared_secret(
        &self,
        local_private_key: &Self::PrivateKey,
        remote_public_key: &Self::PublicKey,
    ) -> Result<SessionKey, Error> {
        let sk = StaticSecret::from(*local_private_key.as_bytes());

        let shared_secret = sk.diffie_hellman(remote_public_key);
        if shared_secret.as_bytes().iter().all(|&byte| byte == 0) {
            return Err(Error::Other(
                "x25519 shared secret is all-zero (invalid public key)".into(),
            ));
        }
        // Derive a 32-byte session key via HKDF-SHA256 with fixed salt/info to
        // avoid direct use of the raw ECDH output.
        let hkdf = Hkdf::<Sha256>::new(Some(HKDF_SALT), shared_secret.as_bytes());
        let mut okm = [0u8; 32];
        hkdf.expand(HKDF_INFO, &mut okm)
            .expect("hkdf expansion to 32 bytes must succeed");
        Ok(SessionKey::new(okm.to_vec()))
    }

    fn encode_public_key(pk: &Self::PublicKey) -> Vec<u8> {
        pk.to_bytes().to_vec()
    }

    fn decode_public_key(bytes: &[u8]) -> Result<Self::PublicKey, ParseError> {
        if bytes.len() != Self::PUBLIC_KEY_SIZE {
            return Err(ParseError(format!(
                "expected {} bytes, got {}",
                Self::PUBLIC_KEY_SIZE,
                bytes.len()
            )));
        }
        let mut array = [0u8; Self::PUBLIC_KEY_SIZE];
        array.copy_from_slice(bytes);
        Ok(PublicKey::from(array))
    }

    const SHARED_SECRET_SIZE: usize = 32;
    const PUBLIC_KEY_SIZE: usize = 32;
    const PRIVATE_KEY_SIZE: usize = 32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_exchange() {
        let scheme = X25519Sha256::new();
        let (public_key1, secret_key1) = scheme.keypair(KeyGenOption::Random);

        let (public_key2, secret_key2) = scheme.keypair(KeyGenOption::Random);
        let shared_secret1 = scheme
            .compute_shared_secret(&secret_key2, &public_key1)
            .expect("shared secret");
        let shared_secret2 = scheme
            .compute_shared_secret(&secret_key1, &public_key2)
            .expect("shared secret");
        assert_eq!(shared_secret1.payload(), shared_secret2.payload());

        let (public_key2, _secret_key1) = scheme.keypair(KeyGenOption::FromPrivateKey(secret_key1));
        assert_eq!(public_key2, public_key1);
    }

    #[test]
    fn hkdf_derivation_is_domain_separated() {
        let scheme = X25519Sha256::new();
        // Deterministic secrets for reproducibility.
        let sk1 = StaticSecret::from([0x11; 32]);
        let sk2 = StaticSecret::from([0x22; 32]);
        let pk1 = PublicKey::from(&sk1);
        let pk2 = PublicKey::from(&sk2);

        let session1 = scheme
            .compute_shared_secret(&sk1, &pk2)
            .expect("shared secret");
        let session2 = scheme
            .compute_shared_secret(&sk2, &pk1)
            .expect("shared secret");
        assert_eq!(session1.payload(), session2.payload());
        // Raw DH bytes must not match derived key (HKDF applied).
        let raw = sk1.diffie_hellman(&pk2);
        assert_ne!(session1.payload(), raw.as_bytes());
    }

    #[test]
    fn shared_secret_rejects_low_order_public_key() {
        let scheme = X25519Sha256::new();
        let (_pk, sk) = scheme.keypair(KeyGenOption::UseSeed(vec![0x11; 32]));
        let low_order = PublicKey::from([0u8; 32]);
        let err = scheme.compute_shared_secret(&sk, &low_order);
        assert!(err.is_err());
    }
}
