//! BLS deterministic batch verification tests.
//! Requires `--features bls`.

#![cfg(feature = "bls")]

use iroha_crypto::{
    BlsNormal, BlsSmall, KeyGenOption, bls_normal_aggregate_signatures,
    bls_normal_verify_aggregate_multi_message, bls_normal_verify_aggregate_same_message,
    bls_normal_verify_batch_deterministic, bls_normal_verify_preaggregated_same_message,
    bls_small_verify_aggregate_multi_message, bls_small_verify_aggregate_same_message,
    bls_small_verify_batch_deterministic,
};
use w3f_bls::serialize::SerializableToBytes;
#[test]
fn bls_normal_batch_verify_ok_and_fail() {
    let (pk, sk) = BlsNormal::keypair(KeyGenOption::Random);
    let msgs: Vec<Vec<u8>> = (0..5)
        .map(|i| format!("bls-n-msg-{i}").into_bytes())
        .collect();
    let sigs: Vec<Vec<u8>> = msgs.iter().map(|m| BlsNormal::sign(m, &sk)).collect();
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
    let (pk, sk) = BlsSmall::keypair(KeyGenOption::Random);
    let msgs: Vec<Vec<u8>> = (0..3)
        .map(|i| format!("bls-s-msg-{i}").into_bytes())
        .collect();
    let sigs: Vec<Vec<u8>> = msgs.iter().map(|m| BlsSmall::sign(m, &sk)).collect();
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
fn bls_normal_same_message_aggregate_ok_and_fail() {
    let (pk1, sk1) = BlsNormal::keypair(KeyGenOption::Random);
    let (pk2, sk2) = BlsNormal::keypair(KeyGenOption::Random);

    let msg = b"same-message".to_vec();
    let s1 = BlsNormal::sign(&msg, &sk1);
    let s2 = BlsNormal::sign(&msg, &sk2);

    let p1 = pk1.to_bytes();
    let p2 = pk2.to_bytes();

    let sig_refs: Vec<&[u8]> = vec![s1.as_slice(), s2.as_slice()];
    let pk_refs: Vec<&[u8]> = vec![p1.as_slice(), p2.as_slice()];

    bls_normal_verify_aggregate_same_message(&msg, &sig_refs, &pk_refs).expect("aggregate ok");

    // Corrupt one signature
    let mut s2b = s2.clone();
    s2b[0] ^= 0x01;
    let broken_refs: Vec<&[u8]> = vec![s1.as_slice(), s2b.as_slice()];
    assert!(
        bls_normal_verify_aggregate_same_message(&msg, &broken_refs, &pk_refs).is_err(),
        "broken aggregate must fail"
    );
}

#[test]
fn bls_normal_preaggregated_same_message_roundtrip() {
    let (pk1, sk1) = BlsNormal::keypair(KeyGenOption::Random);
    let (pk2, sk2) = BlsNormal::keypair(KeyGenOption::Random);

    let msg = b"preaggregated-message".to_vec();
    let s1 = BlsNormal::sign(&msg, &sk1);
    let s2 = BlsNormal::sign(&msg, &sk2);

    let sig_refs: Vec<&[u8]> = vec![s1.as_slice(), s2.as_slice()];
    let aggregate = bls_normal_aggregate_signatures(&sig_refs).expect("aggregate ok");

    let p1 = pk1.to_bytes();
    let p2 = pk2.to_bytes();
    let pk_refs: Vec<&[u8]> = vec![p1.as_slice(), p2.as_slice()];
    bls_normal_verify_preaggregated_same_message(&msg, &aggregate, &pk_refs)
        .expect("pre-aggregate verifies");

    let mut bad = aggregate.clone();
    bad[0] ^= 0x01;
    assert!(
        bls_normal_verify_preaggregated_same_message(&msg, &bad, &pk_refs).is_err(),
        "corrupted aggregate must fail"
    );
    assert!(
        bls_normal_aggregate_signatures(&[]).is_err(),
        "empty aggregate must be rejected"
    );
}

#[test]
fn bls_normal_same_message_rejects_duplicate_public_keys() {
    let (pk, sk) = BlsNormal::keypair(KeyGenOption::Random);
    let msg = b"dup-pk-same-message".to_vec();
    let sig = BlsNormal::sign(&msg, &sk);
    let pk_bytes = pk.to_bytes();

    let sig_refs: Vec<&[u8]> = vec![sig.as_slice(), sig.as_slice()];
    let pk_refs: Vec<&[u8]> = vec![pk_bytes.as_slice(), pk_bytes.as_slice()];
    assert!(bls_normal_verify_aggregate_same_message(&msg, &sig_refs, &pk_refs).is_err());

    let aggregate = bls_normal_aggregate_signatures(&sig_refs).expect("aggregate ok");
    assert!(bls_normal_verify_preaggregated_same_message(&msg, &aggregate, &pk_refs).is_err());
}

#[test]
fn bls_small_same_message_aggregate_ok_and_fail() {
    let (pk1, sk1) = BlsSmall::keypair(KeyGenOption::Random);
    let (pk2, sk2) = BlsSmall::keypair(KeyGenOption::Random);

    let msg = b"same-message".to_vec();
    let s1 = BlsSmall::sign(&msg, &sk1);
    let s2 = BlsSmall::sign(&msg, &sk2);

    let p1 = pk1.to_bytes();
    let p2 = pk2.to_bytes();

    let sig_refs: Vec<&[u8]> = vec![s1.as_slice(), s2.as_slice()];
    let pk_refs: Vec<&[u8]> = vec![p1.as_slice(), p2.as_slice()];

    bls_small_verify_aggregate_same_message(&msg, &sig_refs, &pk_refs).expect("aggregate ok");

    // Corrupt one signature
    let mut s2b = s2.clone();
    s2b[0] ^= 0x01;
    let broken_refs: Vec<&[u8]> = vec![s1.as_slice(), s2b.as_slice()];
    assert!(
        bls_small_verify_aggregate_same_message(&msg, &broken_refs, &pk_refs).is_err(),
        "broken aggregate must fail"
    );
}

#[test]
fn bls_small_same_message_rejects_duplicate_public_keys() {
    let (pk, sk) = BlsSmall::keypair(KeyGenOption::Random);
    let msg = b"dup-pk-same-message-small".to_vec();
    let sig = BlsSmall::sign(&msg, &sk);
    let pk_bytes = pk.to_bytes();

    let sig_refs: Vec<&[u8]> = vec![sig.as_slice(), sig.as_slice()];
    let pk_refs: Vec<&[u8]> = vec![pk_bytes.as_slice(), pk_bytes.as_slice()];
    assert!(bls_small_verify_aggregate_same_message(&msg, &sig_refs, &pk_refs).is_err());
}

#[test]
fn bls_normal_multi_message_aggregate_ok_and_fail() {
    let (pk1, sk1) = BlsNormal::keypair(KeyGenOption::Random);
    let (pk2, sk2) = BlsNormal::keypair(KeyGenOption::Random);
    let m1 = b"m1".to_vec();
    let m2 = b"m2".to_vec();
    let s1 = BlsNormal::sign(&m1, &sk1);
    let s2 = BlsNormal::sign(&m2, &sk2);
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
    let (pk1, sk1) = BlsNormal::keypair(KeyGenOption::Random);
    let (pk2, sk2) = BlsNormal::keypair(KeyGenOption::Random);
    let msg1 = b"dup-msg".to_vec();
    let msg2 = msg1.clone();
    let s1 = BlsNormal::sign(&msg1, &sk1);
    let s2 = BlsNormal::sign(&msg2, &sk2);
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
    let (pk1, sk1) = BlsSmall::keypair(KeyGenOption::Random);
    let (pk2, sk2) = BlsSmall::keypair(KeyGenOption::Random);
    let m1 = b"m1".to_vec();
    let m2 = b"m2".to_vec();
    let s1 = BlsSmall::sign(&m1, &sk1);
    let s2 = BlsSmall::sign(&m2, &sk2);
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
    let (pk1, sk1) = BlsSmall::keypair(KeyGenOption::Random);
    let (pk2, sk2) = BlsSmall::keypair(KeyGenOption::Random);
    let msg1 = b"dup-msg-small".to_vec();
    let msg2 = msg1.clone();
    let s1 = BlsSmall::sign(&msg1, &sk1);
    let s2 = BlsSmall::sign(&msg2, &sk2);
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
