//! Tests covering ML-DSA private key validation behaviour.

use iroha_crypto::{Algorithm, PrivateKey};

#[test]
fn invalid_secret_key_length_is_rejected() {
    let invalid = vec![0_u8; pqcrypto_mldsa::mldsa65::secret_key_bytes() - 1];
    assert!(PrivateKey::from_bytes(Algorithm::MlDsa, &invalid).is_err());
}
