use hkdf::Hkdf;
#[cfg(feature = "rand")]
use rand::rngs::OsRng;
#[cfg(feature = "rand")]
use rand_core::TryRngCore;
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroizing;

use super::KeyExchangeScheme;
use crate::{Error, KeyGenOption, SessionKey, error::ParseError, rng::rng_from_seed};

const HKDF_SALT: &[u8] = b"iroha:x25519:hkdf:v1";
const HKDF_INFO: &[u8] = b"iroha:x25519:session-key";
const LOW_ORDER_CHECK_PRIVATE_KEY: [u8; 32] = [1_u8; 32];

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
        option: KeyGenOption<Self::PrivateKey>,
    ) -> (Self::PublicKey, Self::PrivateKey) {
        self.try_keypair(option)
            .expect("X25519 key generation should succeed")
    }

    fn try_keypair(
        &self,
        option: KeyGenOption<Self::PrivateKey>,
    ) -> Result<(Self::PublicKey, Self::PrivateKey), Error> {
        match option {
            #[cfg(feature = "rand")]
            KeyGenOption::Random => {
                let sk = Self::random_private_key()?;
                let pk = PublicKey::from(&sk);
                Ok((pk, sk))
            }
            KeyGenOption::UseSeed(s) => {
                let mut rng = rng_from_seed(s);
                let mut bytes = Zeroizing::new([0u8; 32]);
                rand_core::RngCore::fill_bytes(&mut rng, bytes.as_mut());
                let sk = StaticSecret::from(*bytes);
                let pk = PublicKey::from(&sk);
                Ok((pk, sk))
            }
            KeyGenOption::FromPrivateKey(ref sk) => {
                let pk = PublicKey::from(sk);
                Ok((pk, sk.clone()))
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
        let mut okm = Zeroizing::new(vec![0u8; 32]);
        hkdf.expand(HKDF_INFO, okm.as_mut_slice())
            .map_err(|_| Error::Other("x25519 hkdf expansion failed".into()))?;
        Ok(SessionKey::from_zeroizing_vec(okm))
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
        let public_key = PublicKey::from(array);
        if is_low_order_public_key(&public_key) {
            return Err(ParseError("x25519 public key is low-order".into()));
        }
        Ok(public_key)
    }

    const SHARED_SECRET_SIZE: usize = 32;
    const PUBLIC_KEY_SIZE: usize = 32;
    const PRIVATE_KEY_SIZE: usize = 32;
}

impl X25519Sha256 {
    #[cfg(feature = "rand")]
    fn random_private_key() -> Result<StaticSecret, Error> {
        let mut bytes = Zeroizing::new([0u8; 32]);
        OsRng
            .try_fill_bytes(bytes.as_mut())
            .map_err(|err| Error::KeyGen(format!("X25519 OS RNG failed: {err}")))?;
        Ok(StaticSecret::from(*bytes))
    }
}

fn is_low_order_public_key(public_key: &PublicKey) -> bool {
    let probe_secret = StaticSecret::from(LOW_ORDER_CHECK_PRIVATE_KEY);
    probe_secret
        .diffie_hellman(public_key)
        .as_bytes()
        .iter()
        .all(|&byte| byte == 0)
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

    #[cfg(feature = "rand")]
    #[test]
    fn try_keypair_random_derives_shared_secret() {
        let scheme = X25519Sha256::new();
        let (public_key1, secret_key1) = scheme
            .try_keypair(KeyGenOption::Random)
            .expect("checked random keypair");
        let (public_key2, secret_key2) = scheme
            .try_keypair(KeyGenOption::Random)
            .expect("checked random keypair");

        let shared_secret1 = scheme
            .compute_shared_secret(&secret_key2, &public_key1)
            .expect("shared secret");
        let shared_secret2 = scheme
            .compute_shared_secret(&secret_key1, &public_key2)
            .expect("shared secret");
        assert_eq!(shared_secret1.payload(), shared_secret2.payload());
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

    #[test]
    fn seeded_keypair_is_deterministic() {
        let scheme = X25519Sha256::new();
        let (public_one, private_one) = scheme
            .try_keypair(KeyGenOption::UseSeed(vec![0x42; 32]))
            .expect("first seeded keypair");
        let (public_two, private_two) = scheme
            .try_keypair(KeyGenOption::UseSeed(vec![0x42; 32]))
            .expect("second seeded keypair");

        assert_eq!(public_one, public_two);
        assert_eq!(private_one.to_bytes(), private_two.to_bytes());
    }

    #[test]
    fn decode_public_key_rejects_low_order_public_key() {
        let err = X25519Sha256::decode_public_key(&[0u8; 32])
            .expect_err("low-order public key must be rejected while decoding");
        assert!(err.to_string().contains("low-order"));
    }
}
