//! Dilithium circuit checks for the Halo2 verifier.

#![cfg(feature = "ivm_zk_tests")]
use ivm::halo2::{DilithiumLevel, DilithiumVerifyCircuit};
use pqcrypto_mldsa::{mldsa44 as dilithium2, mldsa65 as dilithium3};
use pqcrypto_traits::sign::{DetachedSignature, PublicKey, SecretKey};

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
