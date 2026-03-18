#![cfg(feature = "ivm_zk_tests")]
use ivm::halo2::{EcdsaVerifyCircuit, Secp256k1AddCircuit, Secp256k1MulCircuit};
use k256::{
    ProjectivePoint, Scalar,
    ecdsa::{SigningKey, signature::Signer},
    elliptic_curve::sec1::ToEncodedPoint,
};
use rand_core::OsRng;
use sha2::Digest;

#[test]
fn test_secp256k1_add() {
    let p = ProjectivePoint::GENERATOR * Scalar::from(3u64);
    let q = ProjectivePoint::GENERATOR * Scalar::from(5u64);
    let r = p + q;
    let mut p_bytes = [0u8; 65];
    p_bytes.copy_from_slice(p.to_affine().to_encoded_point(false).as_bytes());
    let mut q_bytes = [0u8; 65];
    q_bytes.copy_from_slice(q.to_affine().to_encoded_point(false).as_bytes());
    let mut r_bytes = [0u8; 65];
    r_bytes.copy_from_slice(r.to_affine().to_encoded_point(false).as_bytes());
    let circuit = Secp256k1AddCircuit {
        p: p_bytes,
        q: q_bytes,
        result: r_bytes,
    };
    assert!(circuit.verify().is_ok());
}

#[test]
fn test_secp256k1_mul() {
    let p = ProjectivePoint::GENERATOR;
    let scalar = Scalar::from(7u64);
    let r = p * scalar;
    let mut p_bytes = [0u8; 65];
    p_bytes.copy_from_slice(p.to_affine().to_encoded_point(false).as_bytes());
    let mut r_bytes = [0u8; 65];
    r_bytes.copy_from_slice(r.to_affine().to_encoded_point(false).as_bytes());
    let circuit = Secp256k1MulCircuit {
        scalar: {
            let b = scalar.to_bytes();
            let mut arr = [0u8; 32];
            arr.copy_from_slice(b.as_slice());
            arr
        },
        point: p_bytes,
        result: r_bytes,
    };
    assert!(circuit.verify().is_ok());
}

#[test]
fn test_ecdsa_verify_ok() {
    let mut rng = OsRng;
    let sk = SigningKey::random(&mut rng);
    let vk = sk.verifying_key();
    let msg = b"ecdsa test";
    let sig: k256::ecdsa::Signature = sk.sign(msg);
    let hash = sha2::Sha256::digest(msg);
    let mut pk_bytes = [0u8; 33];
    pk_bytes.copy_from_slice(vk.to_encoded_point(true).as_bytes());
    let mut sig_bytes = [0u8; 64];
    sig_bytes.copy_from_slice(sig.to_bytes().as_slice());
    let circuit = EcdsaVerifyCircuit {
        public_key: pk_bytes,
        message_hash: hash.into(),
        signature: sig_bytes,
        result: true,
    };
    assert!(circuit.verify().is_ok());
}

#[test]
fn test_ecdsa_verify_bad_sig() {
    let mut rng = OsRng;
    let sk = SigningKey::random(&mut rng);
    let vk = sk.verifying_key();
    let msg = b"ecdsa test";
    let sig: k256::ecdsa::Signature = sk.sign(msg);
    let hash = sha2::Sha256::digest(msg);
    let mut pk_bytes = [0u8; 33];
    pk_bytes.copy_from_slice(vk.to_encoded_point(true).as_bytes());
    let mut sig_bytes = [0u8; 64];
    sig_bytes.copy_from_slice(sig.to_bytes().as_slice());
    sig_bytes[0] ^= 1;
    let circuit = EcdsaVerifyCircuit {
        public_key: pk_bytes,
        message_hash: hash.into(),
        signature: sig_bytes,
        result: true,
    };
    assert!(circuit.verify().is_err());
}
