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

#[cfg(feature = "cuda")]
fn compute_hram(sig: &[u8; 64], pk: &[u8; 32], msg: &[u8]) -> [u8; 32] {
    use curve25519_dalek::scalar::Scalar;
    use sha2::Digest;

    let mut hasher = sha2::Sha512::new();
    hasher.update(&sig[..32]);
    hasher.update(pk);
    hasher.update(msg);
    Scalar::from_hash(hasher).to_bytes()
}

#[cfg(feature = "cuda")]
#[test]
fn ed25519_batch_helper_matches_direct_cuda_batch_when_available() {
    if !ivm::cuda_available() {
        eprintln!("No CUDA GPU available; skipping test");
        return;
    }

    let key1 = SigningKey::from_bytes(&[0x12; 32]);
    let key2 = SigningKey::from_bytes(&[0x34; 32]);
    let msg1 = b"cuda wrapper batch one";
    let msg2 = b"cuda wrapper batch two";

    let sig1 = key1.sign(msg1).to_bytes();
    let mut sig2 = key2.sign(msg2).to_bytes();
    sig2[0] ^= 0x55;

    let pk1 = key1.verifying_key().to_bytes();
    let pk2 = key2.verifying_key().to_bytes();
    let sigs = vec![sig1, sig2];
    let pks = vec![pk1, pk2];
    let hrams = vec![
        compute_hram(&sigs[0], &pks[0], msg1),
        compute_hram(&sigs[1], &pks[1], msg2),
    ];

    let items = [
        Ed25519BatchItem {
            message: msg1.as_slice(),
            signature: sigs[0],
            public_key: pks[0],
        },
        Ed25519BatchItem {
            message: msg2.as_slice(),
            signature: sigs[1],
            public_key: pks[1],
        },
    ];

    let direct = ivm::ed25519_verify_batch_cuda(&sigs, &pks, &hrams)
        .expect("direct CUDA batch helper should be available on a CUDA host");
    assert_eq!(verify_ed25519_batch_items(&items), direct);
}
