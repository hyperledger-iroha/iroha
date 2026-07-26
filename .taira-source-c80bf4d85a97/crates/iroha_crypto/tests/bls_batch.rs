//! BLS deterministic batch verification tests.
//! Requires `--features bls`.

#![cfg(feature = "bls")]

use iroha_crypto::{
    BlsNormal, BlsNormalPrivateKey, BlsNormalPublicKey, BlsSmall, BlsSmallPrivateKey,
    BlsSmallPublicKey, KeyGenOption, KeyPair, bls_normal_aggregate_signatures,
    bls_normal_pop_prove, bls_normal_verify_aggregate_multi_message,
    bls_normal_verify_aggregate_same_message, bls_normal_verify_batch_deterministic,
    bls_normal_verify_preaggregated_same_message, bls_small_pop_prove,
    bls_small_verify_aggregate_multi_message, bls_small_verify_aggregate_same_message,
    bls_small_verify_batch_deterministic,
};
#[cfg(not(feature = "bls-backend-blstrs"))]
use w3f_bls::serialize::SerializableToBytes as _;

fn bls_normal_keypair() -> (BlsNormalPublicKey, BlsNormalPrivateKey) {
    BlsNormal::keypair(KeyGenOption::Random).expect("random BLS normal keypair")
}

fn bls_small_keypair() -> (BlsSmallPublicKey, BlsSmallPrivateKey) {
    BlsSmall::keypair(KeyGenOption::Random).expect("random BLS small keypair")
}

fn bls_normal_sign(message: &[u8], secret_key: &BlsNormalPrivateKey) -> Vec<u8> {
    BlsNormal::sign(message, secret_key).expect("BLS normal signature")
}

fn bls_small_sign(message: &[u8], secret_key: &BlsSmallPrivateKey) -> Vec<u8> {
    BlsSmall::sign(message, secret_key).expect("BLS small signature")
}

#[test]
fn bls_normal_batch_verify_ok_and_fail() {
    let (pk, sk) = bls_normal_keypair();
    let msgs: Vec<Vec<u8>> = (0..5)
        .map(|i| format!("bls-n-msg-{i}").into_bytes())
        .collect();
    let sigs: Vec<Vec<u8>> = msgs.iter().map(|m| bls_normal_sign(m, &sk)).collect();
    let pks: Vec<Vec<u8>> = msgs.iter().map(|_| pk.to_bytes()).collect();

    let msg_refs: Vec<&[u8]> = msgs.iter().map(Vec::as_slice).collect();
    let sig_refs: Vec<&[u8]> = sigs.iter().map(Vec::as_slice).collect();
    let pk_refs: Vec<&[u8]> = pks.iter().map(Vec::as_slice).collect();

    let seed = [7u8; 32];
    bls_normal_verify_batch_deterministic(&msg_refs, &sig_refs, &pk_refs, seed).expect("ok");

    // Fail with corrupted signature
    let mut broken = sigs.clone();
    broken[3][0] ^= 0x01;
    let broken_refs: Vec<&[u8]> = broken.iter().map(Vec::as_slice).collect();
    assert!(
        bls_normal_verify_batch_deterministic(&msg_refs, &broken_refs, &pk_refs, seed).is_err()
    );
}

#[test]
fn bls_small_batch_verify_ok_and_fail() {
    let (pk, sk) = bls_small_keypair();
    let msgs: Vec<Vec<u8>> = (0..3)
        .map(|i| format!("bls-s-msg-{i}").into_bytes())
        .collect();
    let sigs: Vec<Vec<u8>> = msgs.iter().map(|m| bls_small_sign(m, &sk)).collect();
    let pks: Vec<Vec<u8>> = msgs.iter().map(|_| pk.to_bytes()).collect();

    let msg_refs: Vec<&[u8]> = msgs.iter().map(Vec::as_slice).collect();
    let sig_refs: Vec<&[u8]> = sigs.iter().map(Vec::as_slice).collect();
    let pk_refs: Vec<&[u8]> = pks.iter().map(Vec::as_slice).collect();

    let seed = [11u8; 32];
    bls_small_verify_batch_deterministic(&msg_refs, &sig_refs, &pk_refs, seed).expect("ok");

    // Fail with corrupted signature
    let mut broken = sigs.clone();
    broken[1][0] ^= 0x01;
    let broken_refs: Vec<&[u8]> = broken.iter().map(Vec::as_slice).collect();
    assert!(bls_small_verify_batch_deterministic(&msg_refs, &broken_refs, &pk_refs, seed).is_err());
}

#[test]
fn bls_batch_verify_rejects_empty_input() {
    let empty: Vec<&[u8]> = Vec::new();
    let seed = [0u8; 32];
    assert!(bls_normal_verify_batch_deterministic(&empty, &empty, &empty, seed).is_err());
    assert!(bls_small_verify_batch_deterministic(&empty, &empty, &empty, seed).is_err());
}

#[test]
fn bls_normal_same_message_aggregate_ok_and_fail() {
    let (pk1, sk1) = bls_normal_keypair();
    let (pk2, sk2) = bls_normal_keypair();
    let kp1: KeyPair = (pk1, sk1.clone()).into();
    let kp2: KeyPair = (pk2, sk2.clone()).into();
    let pop1 = bls_normal_pop_prove(kp1.private_key()).expect("pop");
    let pop2 = bls_normal_pop_prove(kp2.private_key()).expect("pop");

    let msg = b"same-message".to_vec();
    let s1 = bls_normal_sign(&msg, &sk1);
    let s2 = bls_normal_sign(&msg, &sk2);

    let sig_refs: Vec<&[u8]> = vec![s1.as_slice(), s2.as_slice()];
    let pk_refs = vec![kp1.public_key(), kp2.public_key()];
    let pop_refs: Vec<&[u8]> = vec![pop1.as_slice(), pop2.as_slice()];

    bls_normal_verify_aggregate_same_message(&msg, &sig_refs, &pk_refs, &pop_refs)
        .expect("aggregate ok");

    // Corrupt one signature
    let mut s2b = s2.clone();
    s2b[0] ^= 0x01;
    let broken_refs: Vec<&[u8]> = vec![s1.as_slice(), s2b.as_slice()];
    assert!(
        bls_normal_verify_aggregate_same_message(&msg, &broken_refs, &pk_refs, &pop_refs).is_err(),
        "broken aggregate must fail"
    );
}

#[test]
fn bls_normal_preaggregated_same_message_roundtrip() {
    let (pk1, sk1) = bls_normal_keypair();
    let (pk2, sk2) = bls_normal_keypair();
    let kp1: KeyPair = (pk1, sk1.clone()).into();
    let kp2: KeyPair = (pk2, sk2.clone()).into();
    let pop1 = bls_normal_pop_prove(kp1.private_key()).expect("pop");
    let pop2 = bls_normal_pop_prove(kp2.private_key()).expect("pop");

    let msg = b"preaggregated-message".to_vec();
    let s1 = bls_normal_sign(&msg, &sk1);
    let s2 = bls_normal_sign(&msg, &sk2);

    let sig_refs: Vec<&[u8]> = vec![s1.as_slice(), s2.as_slice()];
    let aggregate = bls_normal_aggregate_signatures(&sig_refs).expect("aggregate ok");

    let pk_refs = vec![kp1.public_key(), kp2.public_key()];
    let pop_refs: Vec<&[u8]> = vec![pop1.as_slice(), pop2.as_slice()];
    bls_normal_verify_preaggregated_same_message(&msg, &aggregate, &pk_refs, &pop_refs)
        .expect("pre-aggregate verifies");

    let mut bad = aggregate.clone();
    bad[0] ^= 0x01;
    assert!(
        bls_normal_verify_preaggregated_same_message(&msg, &bad, &pk_refs, &pop_refs).is_err(),
        "corrupted aggregate must fail"
    );
    let mut bad_pop = pop1.clone();
    bad_pop[0] ^= 0x01;
    let bad_pop_refs: Vec<&[u8]> = vec![bad_pop.as_slice(), pop2.as_slice()];
    assert!(
        bls_normal_verify_preaggregated_same_message(&msg, &aggregate, &pk_refs, &bad_pop_refs)
            .is_err(),
        "invalid pop must be rejected"
    );
    assert!(
        bls_normal_aggregate_signatures(&[]).is_err(),
        "empty aggregate must be rejected"
    );
}

#[test]
fn bls_normal_same_message_rejects_duplicate_public_keys() {
    let (pk, sk) = bls_normal_keypair();
    let kp: KeyPair = (pk, sk.clone()).into();
    let pop = bls_normal_pop_prove(kp.private_key()).expect("pop");
    let msg = b"dup-pk-same-message".to_vec();
    let sig = bls_normal_sign(&msg, &sk);

    let sig_refs: Vec<&[u8]> = vec![sig.as_slice(), sig.as_slice()];
    let pk_refs = vec![kp.public_key(), kp.public_key()];
    let pop_refs: Vec<&[u8]> = vec![pop.as_slice(), pop.as_slice()];
    assert!(
        bls_normal_verify_aggregate_same_message(&msg, &sig_refs, &pk_refs, &pop_refs).is_err()
    );

    let aggregate = bls_normal_aggregate_signatures(&sig_refs).expect("aggregate ok");
    assert!(
        bls_normal_verify_preaggregated_same_message(&msg, &aggregate, &pk_refs, &pop_refs)
            .is_err()
    );
}

#[test]
fn bls_normal_same_message_rejects_invalid_pop() {
    let (pk1, sk1) = bls_normal_keypair();
    let (pk2, sk2) = bls_normal_keypair();
    let kp1: KeyPair = (pk1, sk1.clone()).into();
    let kp2: KeyPair = (pk2, sk2.clone()).into();
    let mut pop1 = bls_normal_pop_prove(kp1.private_key()).expect("pop");
    let pop2 = bls_normal_pop_prove(kp2.private_key()).expect("pop");

    let msg = b"invalid-pop".to_vec();
    let s1 = bls_normal_sign(&msg, &sk1);
    let s2 = bls_normal_sign(&msg, &sk2);
    pop1[0] ^= 0x01;

    let sig_refs: Vec<&[u8]> = vec![s1.as_slice(), s2.as_slice()];
    let pk_refs = vec![kp1.public_key(), kp2.public_key()];
    let pop_refs: Vec<&[u8]> = vec![pop1.as_slice(), pop2.as_slice()];

    assert!(
        bls_normal_verify_aggregate_same_message(&msg, &sig_refs, &pk_refs, &pop_refs).is_err()
    );
}

#[test]
fn bls_small_same_message_aggregate_ok_and_fail() {
    let (pk1, sk1) = bls_small_keypair();
    let (pk2, sk2) = bls_small_keypair();
    let kp1: KeyPair = (pk1, sk1.clone()).into();
    let kp2: KeyPair = (pk2, sk2.clone()).into();
    let pop1 = bls_small_pop_prove(kp1.private_key()).expect("pop");
    let pop2 = bls_small_pop_prove(kp2.private_key()).expect("pop");

    let msg = b"same-message".to_vec();
    let s1 = bls_small_sign(&msg, &sk1);
    let s2 = bls_small_sign(&msg, &sk2);

    let sig_refs: Vec<&[u8]> = vec![s1.as_slice(), s2.as_slice()];
    let pk_refs = vec![kp1.public_key(), kp2.public_key()];
    let pop_refs: Vec<&[u8]> = vec![pop1.as_slice(), pop2.as_slice()];

    bls_small_verify_aggregate_same_message(&msg, &sig_refs, &pk_refs, &pop_refs)
        .expect("aggregate ok");

    // Corrupt one signature
    let mut s2b = s2.clone();
    s2b[0] ^= 0x01;
    let broken_refs: Vec<&[u8]> = vec![s1.as_slice(), s2b.as_slice()];
    assert!(
        bls_small_verify_aggregate_same_message(&msg, &broken_refs, &pk_refs, &pop_refs).is_err(),
        "broken aggregate must fail"
    );
}

#[test]
fn bls_small_same_message_rejects_duplicate_public_keys() {
    let (pk, sk) = bls_small_keypair();
    let kp: KeyPair = (pk, sk.clone()).into();
    let pop = bls_small_pop_prove(kp.private_key()).expect("pop");
    let msg = b"dup-pk-same-message-small".to_vec();
    let sig = bls_small_sign(&msg, &sk);

    let sig_refs: Vec<&[u8]> = vec![sig.as_slice(), sig.as_slice()];
    let pk_refs = vec![kp.public_key(), kp.public_key()];
    let pop_refs: Vec<&[u8]> = vec![pop.as_slice(), pop.as_slice()];
    assert!(bls_small_verify_aggregate_same_message(&msg, &sig_refs, &pk_refs, &pop_refs).is_err());
}

#[test]
fn bls_small_same_message_rejects_invalid_pop() {
    let (pk1, sk1) = bls_small_keypair();
    let (pk2, sk2) = bls_small_keypair();
    let kp1: KeyPair = (pk1, sk1.clone()).into();
    let kp2: KeyPair = (pk2, sk2.clone()).into();
    let mut pop1 = bls_small_pop_prove(kp1.private_key()).expect("pop");
    let pop2 = bls_small_pop_prove(kp2.private_key()).expect("pop");

    let msg = b"invalid-pop-small".to_vec();
    let s1 = bls_small_sign(&msg, &sk1);
    let s2 = bls_small_sign(&msg, &sk2);
    pop1[0] ^= 0x01;

    let sig_refs: Vec<&[u8]> = vec![s1.as_slice(), s2.as_slice()];
    let pk_refs = vec![kp1.public_key(), kp2.public_key()];
    let pop_refs: Vec<&[u8]> = vec![pop1.as_slice(), pop2.as_slice()];

    assert!(bls_small_verify_aggregate_same_message(&msg, &sig_refs, &pk_refs, &pop_refs).is_err());
}

#[test]
fn bls_normal_multi_message_aggregate_ok_and_fail() {
    let (pk1, sk1) = bls_normal_keypair();
    let (pk2, sk2) = bls_normal_keypair();
    let m1 = b"m1".to_vec();
    let m2 = b"m2".to_vec();
    let s1 = bls_normal_sign(&m1, &sk1);
    let s2 = bls_normal_sign(&m2, &sk2);
    let p1 = pk1.to_bytes();
    let p2 = pk2.to_bytes();
    let msgs: Vec<&[u8]> = vec![m1.as_slice(), m2.as_slice()];
    let sigs: Vec<&[u8]> = vec![s1.as_slice(), s2.as_slice()];
    let pk_vec_refs: Vec<&[u8]> = vec![p1.as_slice(), p2.as_slice()];
    bls_normal_verify_aggregate_multi_message(&msgs, &sigs, &pk_vec_refs).expect("ok");
    // Corrupt
    let mut s2b = s2.clone();
    s2b[0] ^= 0x01;
    let broken: Vec<&[u8]> = vec![s1.as_slice(), s2b.as_slice()];
    assert!(bls_normal_verify_aggregate_multi_message(&msgs, &broken, &pk_vec_refs).is_err());
}

#[test]
fn bls_normal_multi_message_rejects_duplicate_messages() {
    let (pk1, sk1) = bls_normal_keypair();
    let (pk2, sk2) = bls_normal_keypair();
    let msg1 = b"dup-msg".to_vec();
    let msg2 = msg1.clone();
    let s1 = bls_normal_sign(&msg1, &sk1);
    let s2 = bls_normal_sign(&msg2, &sk2);
    let p1 = pk1.to_bytes();
    let p2 = pk2.to_bytes();
    let message_slices: Vec<&[u8]> = vec![msg1.as_slice(), msg2.as_slice()];
    let sigs: Vec<&[u8]> = vec![s1.as_slice(), s2.as_slice()];
    let pk_vec_refs: Vec<&[u8]> = vec![p1.as_slice(), p2.as_slice()];
    assert!(
        bls_normal_verify_aggregate_multi_message(&message_slices, &sigs, &pk_vec_refs).is_err()
    );
}

#[test]
fn bls_normal_multi_message_rejects_empty() {
    let msgs: Vec<&[u8]> = Vec::new();
    let sigs: Vec<&[u8]> = Vec::new();
    let pk_refs: Vec<&[u8]> = Vec::new();
    assert!(bls_normal_verify_aggregate_multi_message(&msgs, &sigs, &pk_refs).is_err());
}

#[test]
fn bls_small_multi_message_aggregate_ok_and_fail() {
    let (pk1, sk1) = bls_small_keypair();
    let (pk2, sk2) = bls_small_keypair();
    let m1 = b"m1".to_vec();
    let m2 = b"m2".to_vec();
    let s1 = bls_small_sign(&m1, &sk1);
    let s2 = bls_small_sign(&m2, &sk2);
    let p1 = pk1.to_bytes();
    let p2 = pk2.to_bytes();
    let msgs: Vec<&[u8]> = vec![m1.as_slice(), m2.as_slice()];
    let sigs: Vec<&[u8]> = vec![s1.as_slice(), s2.as_slice()];
    let pk_vec_refs: Vec<&[u8]> = vec![p1.as_slice(), p2.as_slice()];
    bls_small_verify_aggregate_multi_message(&msgs, &sigs, &pk_vec_refs).expect("ok");
    // Corrupt
    let mut s2b = s2.clone();
    s2b[0] ^= 0x01;
    let broken: Vec<&[u8]> = vec![s1.as_slice(), s2b.as_slice()];
    assert!(bls_small_verify_aggregate_multi_message(&msgs, &broken, &pk_vec_refs).is_err());
}

#[test]
fn bls_small_multi_message_rejects_duplicate_messages() {
    let (pk1, sk1) = bls_small_keypair();
    let (pk2, sk2) = bls_small_keypair();
    let msg1 = b"dup-msg-small".to_vec();
    let msg2 = msg1.clone();
    let s1 = bls_small_sign(&msg1, &sk1);
    let s2 = bls_small_sign(&msg2, &sk2);
    let p1 = pk1.to_bytes();
    let p2 = pk2.to_bytes();
    let message_slices: Vec<&[u8]> = vec![msg1.as_slice(), msg2.as_slice()];
    let sigs: Vec<&[u8]> = vec![s1.as_slice(), s2.as_slice()];
    let pk_vec_refs: Vec<&[u8]> = vec![p1.as_slice(), p2.as_slice()];
    assert!(
        bls_small_verify_aggregate_multi_message(&message_slices, &sigs, &pk_vec_refs).is_err()
    );
}

#[test]
fn bls_small_multi_message_rejects_empty() {
    let msgs: Vec<&[u8]> = Vec::new();
    let sigs: Vec<&[u8]> = Vec::new();
    let pk_refs: Vec<&[u8]> = Vec::new();
    assert!(bls_small_verify_aggregate_multi_message(&msgs, &sigs, &pk_refs).is_err());
}
