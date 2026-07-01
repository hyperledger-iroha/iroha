#![cfg(feature = "ivm_zk_tests")]
use ed25519_dalek::{Signer, SigningKey};
use ivm::halo2::Ed25519VerifyCircuit;

#[test]
fn test_ed25519_circuit_ok() {
    let keypair = SigningKey::from_bytes(&[0x31; 32]);
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
    let keypair = SigningKey::from_bytes(&[0x32; 32]);
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

#[test]
fn test_ed25519_circuit_all_zero_signature_is_false() {
    let keypair = SigningKey::from_bytes(&[0x33; 32]);
    let circuit = Ed25519VerifyCircuit {
        public_key: keypair.verifying_key().to_bytes(),
        signature: [0u8; 64],
        message: b"ed25519 zero signature",
        result: false,
    };

    assert!(circuit.verify().is_ok());
}
