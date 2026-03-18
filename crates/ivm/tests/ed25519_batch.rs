//! Ed25519 batch verification helper tests.

use ed25519_dalek::{Signer, SigningKey};
use ivm::signature::{Ed25519BatchItem, verify_ed25519_batch_items};

#[test]
fn ed25519_batch_mixed_validity() {
    let key1 = SigningKey::from_bytes(&[7u8; 32]);
    let key2 = SigningKey::from_bytes(&[9u8; 32]);

    let msg1 = b"batch#1";
    let msg2 = b"batch#2";

    let sig1 = key1.sign(msg1).to_bytes();
    let mut sig2 = key2.sign(msg2).to_bytes();
    sig2[0] ^= 0xAA; // tamper to force failure

    let items = [
        Ed25519BatchItem {
            message: msg1.as_slice(),
            signature: sig1,
            public_key: key1.verifying_key().to_bytes(),
        },
        Ed25519BatchItem {
            message: msg2.as_slice(),
            signature: sig2,
            public_key: key2.verifying_key().to_bytes(),
        },
    ];

    let results = verify_ed25519_batch_items(&items);
    assert_eq!(results, vec![true, false]);
}
