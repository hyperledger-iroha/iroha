//! Dilithium circuit checks for the Halo2 verifier.
#![cfg(feature = "ivm_zk_tests")]
use ivm::halo2::{DilithiumLevel, DilithiumVerifyCircuit};
use pqcrypto_mldsa::{mldsa44 as dilithium2, mldsa65 as dilithium3};
use pqcrypto_traits::sign::{DetachedSignature, PublicKey};
#[test]
fn dilithium2_circuit_ok() {
    let (pk, sk) = dilithium2::keypair();
    let msg = b"dilithium2 test";
    let sig = dilithium2::detached_sign(msg, &sk);
    let circuit = DilithiumVerifyCircuit {
        level: DilithiumLevel::Level2,
        public_key: pk.as_bytes(),
        signature: sig.as_bytes(),
        message: msg,
        result: true,
    };
    assert!(circuit.verify().is_ok());
}
#[test]
fn dilithium3_circuit_bad_sig() {
    let (pk, sk) = dilithium3::keypair();
    let msg = b"dilithium3 test";
    let mut sig = dilithium3::detached_sign(msg, &sk);
    // Corrupt signature
    let mut bytes = sig.as_bytes().to_vec();
    bytes[0] ^= 1;
    sig = pqcrypto_mldsa::mldsa65::DetachedSignature::from_bytes(&bytes).unwrap();
    let circuit = DilithiumVerifyCircuit {
        level: DilithiumLevel::Level3,
        public_key: pk.as_bytes(),
        signature: sig.as_bytes(),
        message: msg,
        result: true,
    };
    assert!(circuit.verify().is_err());
}
#[test]
fn dilithium3_circuit_all_zero_signature_is_false() {
    let (pk, _) = dilithium3::keypair();
    let signature = vec![0u8; dilithium3::signature_bytes()];
    let circuit = DilithiumVerifyCircuit {
        level: DilithiumLevel::Level3,
        public_key: pk.as_bytes(),
        signature: &signature,
        message: b"dilithium3 zero signature",
        result: false,
    };
    assert!(circuit.verify().is_ok());
}
#[test]
fn dilithium3_circuit_all_zero_public_key_is_false() {
    let (_, sk) = dilithium3::keypair();
    let msg = b"dilithium3 zero public key";
    let sig = dilithium3::detached_sign(msg, &sk);
    let public_key = vec![0u8; dilithium3::public_key_bytes()];
    let circuit = DilithiumVerifyCircuit {
        level: DilithiumLevel::Level3,
        public_key: &public_key,
        signature: sig.as_bytes(),
        message: msg,
        result: false,
    };
    assert!(circuit.verify().is_ok());
}
