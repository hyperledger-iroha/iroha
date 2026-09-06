//! Public-API regression tests for wiping confidential identifier copies.

use iroha_crypto::{Algorithm, KeyPair, Signature};
use zeroize::Zeroize as _;

#[test]
fn confidential_discard_zeroizes_public_key_and_signature_copies() {
    let mut public_key = KeyPair::try_from_seed(vec![0xA7; 32], Algorithm::Ed25519)
        .expect("checked discard fixture")
        .public_key()
        .clone();
    public_key.zeroize_for_confidential_discard();
    assert_eq!(public_key.to_string(), "invalid-public-key:");
    assert!(public_key.try_to_bytes().is_err());

    let mut signature = Signature::from_bytes(&[0xA8; 64]);
    signature.zeroize();
    assert!(signature.payload().is_empty());
}
