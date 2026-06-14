//! PQC (Dilithium3) deterministic batch verification tests.

use iroha_crypto::{Algorithm, KeyPair, pqc_verify_batch_deterministic};
use pqcrypto_mldsa::mldsa65 as dilithium;
use pqcrypto_traits::sign::{DetachedSignature as _, PublicKey as _, SecretKey as _};

#[test]
fn pqc_batch_verify_ok_and_fail() {
    // Prepare a few distinct messages and signatures
    let kp = KeyPair::try_from_seed(b"iroha:ml-dsa:pqc-batch".to_vec(), Algorithm::MlDsa)
        .expect("fixture seed derives ML-DSA batch keypair");
    let (algorithm, public_bytes) = kp
        .public_key()
        .try_to_bytes()
        .expect("fixture ML-DSA public key must be well-formed");
    assert_eq!(algorithm, Algorithm::MlDsa);
    let pk = dilithium::PublicKey::from_bytes(public_bytes).expect("seeded ML-DSA public key");
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

#[test]
fn pqc_batch_verify_rejects_all_zero_material_before_backend() {
    let kp = KeyPair::try_from_seed(
        b"iroha:ml-dsa:pqc-batch-all-zero".to_vec(),
        Algorithm::MlDsa,
    )
    .expect("fixture seed derives ML-DSA all-zero-admission keypair");
    let (_, public_bytes) = kp
        .public_key()
        .try_to_bytes()
        .expect("fixture ML-DSA public key must be well-formed");
    let secret = dilithium::SecretKey::from_bytes(&kp.private_key().to_bytes().1)
        .expect("seeded ML-DSA secret key");
    let message = b"iroha:ml-dsa:pqc-batch-all-zero";
    let signature = dilithium::detached_sign(message, &secret)
        .as_bytes()
        .to_vec();
    let all_zero_signature = vec![0u8; dilithium::signature_bytes()];
    let all_zero_public_key = vec![0u8; dilithium::public_key_bytes()];
    let message_ref: &[u8] = message;

    assert!(
        pqc_verify_batch_deterministic(
            &[message_ref],
            &[all_zero_signature.as_slice()],
            &[public_bytes],
            [0u8; 32],
        )
        .is_err(),
        "all-zero ML-DSA signature must fail before backend verification"
    );
    assert!(
        pqc_verify_batch_deterministic(
            &[message_ref],
            &[signature.as_slice()],
            &[all_zero_public_key.as_slice()],
            [0u8; 32],
        )
        .is_err(),
        "all-zero ML-DSA public key must fail before backend verification"
    );
}
