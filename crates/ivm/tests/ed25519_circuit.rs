#![cfg(feature = "ivm_zk_tests")]
use ed25519_dalek::{Signer, SigningKey};
use ivm::halo2::Ed25519VerifyCircuit;
use rand_core::OsRng;

#[test]
fn test_ed25519_circuit_ok() {
    let mut rng = OsRng;
    let keypair = SigningKey::generate(&mut rng);
    let msg = b"ed25519 test";
    let sig = keypair.sign(msg);
    let circuit = Ed25519VerifyCircuit {
        public_key: keypair.verifying_key().to_bytes(),
        signature: sig.to_bytes(),
        message: msg,
        result: true,
    };
    assert!(circuit.verify().is_ok());
}

#[test]
fn test_ed25519_circuit_bad_sig() {
    let mut rng = OsRng;
    let keypair = SigningKey::generate(&mut rng);
    let msg = b"ed25519 test";
    let mut sig_bytes = keypair.sign(msg).to_bytes();
    sig_bytes[0] ^= 1; // corrupt signature
    let circuit = Ed25519VerifyCircuit {
        public_key: keypair.verifying_key().to_bytes(),
        signature: sig_bytes,
        message: msg,
        result: true,
    };
    assert!(circuit.verify().is_err());
}
