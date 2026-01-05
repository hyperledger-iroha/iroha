//! Tests covering ML-DSA private key validation behaviour.
#![cfg(feature = "ml-dsa")]

use iroha_crypto::{Algorithm, PrivateKey};

#[test]
fn invalid_secret_key_length_is_rejected() {
    let invalid = vec![0_u8; pqcrypto_dilithium::dilithium3::secret_key_bytes() - 1];
    assert!(PrivateKey::from_bytes(Algorithm::MlDsa, &invalid).is_err());
}
