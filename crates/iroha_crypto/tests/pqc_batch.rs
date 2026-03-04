//! PQC (Dilithium3) deterministic batch verification tests.

use iroha_crypto::{Algorithm, KeyPair, pqc_verify_batch_deterministic};
use pqcrypto_dilithium::dilithium3 as dilithium;
use pqcrypto_traits::sign::{DetachedSignature as _, PublicKey as _, SecretKey as _};

#[test]
fn pqc_batch_verify_ok_and_fail() {
    // Prepare a few distinct messages and signatures
    let kp = KeyPair::from_seed(b"iroha:ml-dsa:pqc-batch".to_vec(), Algorithm::MlDsa);
    let pk = dilithium::PublicKey::from_bytes(kp.public_key().to_bytes().1)
        .expect("seeded ML-DSA public key");
    let sk = dilithium::SecretKey::from_bytes(&kp.private_key().to_bytes().1)
        .expect("seeded ML-DSA secret key");

    let msgs: Vec<Vec<u8>> = (0..5)
        .map(|i| format!("ml-dsa-msg-{i}").into_bytes())
        .collect();
    let sigs: Vec<Vec<u8>> = msgs
        .iter()
        .map(|m| dilithium::detached_sign(m, &sk).as_bytes().to_vec())
        .collect();
    let pks: Vec<Vec<u8>> = msgs.iter().map(|_| pk.as_bytes().to_vec()).collect();

    let msg_refs: Vec<&[u8]> = msgs.iter().map(Vec::as_slice).collect();
    let sig_refs: Vec<&[u8]> = sigs.iter().map(Vec::as_slice).collect();
    let pk_refs: Vec<&[u8]> = pks.iter().map(Vec::as_slice).collect();

    let seed = [9u8; 32];
    // All valid
    pqc_verify_batch_deterministic(&msg_refs, &sig_refs, &pk_refs, seed).expect("pqc batch ok");

    // Now corrupt one signature and expect failure
    let mut broken = sigs.clone();
    broken[2][0] ^= 0x01;
    let broken_refs: Vec<&[u8]> = broken.iter().map(Vec::as_slice).collect();
    assert!(pqc_verify_batch_deterministic(&msg_refs, &broken_refs, &pk_refs, seed).is_err());
}

#[test]
fn pqc_batch_verify_rejects_empty_input() {
    let empty: Vec<&[u8]> = Vec::new();
    assert!(pqc_verify_batch_deterministic(&empty, &empty, &empty, [0u8; 32]).is_err());
}
