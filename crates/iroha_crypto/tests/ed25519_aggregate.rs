//! Tests for Ed25519 aggregate-style verification.

use iroha_crypto::{
    Algorithm, Error, KeyPair, Signature, ed25519_verify_aggregate,
    ed25519_verify_batch_deterministic,
};

type ByteBuffers = Vec<Vec<u8>>;
type Ed25519Batch = (ByteBuffers, ByteBuffers, ByteBuffers);

fn checked_ed25519_keypair() -> KeyPair {
    KeyPair::try_random_with_algorithm(Algorithm::Ed25519)
        .expect("generate checked Ed25519 aggregate fixture keypair")
}

fn checked_ed25519_signature(keypair: &KeyPair, message: &[u8]) -> Signature {
    Signature::try_new(keypair.private_key(), message)
        .expect("sign checked Ed25519 aggregate fixture")
}

fn checked_ed25519_public_key_payload(keypair: &KeyPair) -> &[u8] {
    let (algorithm, payload) = keypair
        .public_key()
        .try_to_bytes()
        .expect("fixture Ed25519 public key must be well-formed");
    assert_eq!(algorithm, Algorithm::Ed25519);
    payload
}

#[test]
fn checked_ed25519_aggregate_fixture_signature_rejects_wrong_key() {
    let keypair = checked_ed25519_keypair();
    let wrong_key = checked_ed25519_keypair();
    let message = b"aggregate-fixture";
    let signature = checked_ed25519_signature(&keypair, message);

    signature
        .verify(keypair.public_key(), message)
        .expect("checked Ed25519 aggregate fixture signature verifies");
    signature
        .verify(wrong_key.public_key(), message)
        .expect_err("checked Ed25519 aggregate fixture rejects wrong key");
}

#[test]
fn ed25519_verify_aggregate_accepts_valid_signatures() {
    let mut messages = Vec::new();
    let mut signatures = Vec::new();
    let mut public_keys = Vec::new();

    for idx in 0u8..3 {
        let keypair = checked_ed25519_keypair();
        let message = vec![idx; 32];
        let signature = checked_ed25519_signature(&keypair, &message);
        let pk_bytes = checked_ed25519_public_key_payload(&keypair);

        messages.push(message);
        signatures.push(signature.payload().to_vec());
        public_keys.push(pk_bytes.to_vec());
    }

    let msg_refs: Vec<&[u8]> = messages.iter().map(Vec::as_slice).collect();
    let sig_refs: Vec<&[u8]> = signatures.iter().map(Vec::as_slice).collect();
    let pk_refs: Vec<&[u8]> = public_keys.iter().map(Vec::as_slice).collect();

    assert!(ed25519_verify_aggregate(&msg_refs, &sig_refs, &pk_refs).is_ok());
}

#[test]
fn ed25519_verify_aggregate_rejects_tampered_signature() {
    let keypair_a = checked_ed25519_keypair();
    let keypair_b = checked_ed25519_keypair();

    let message_a = vec![0xA5; 16];
    let message_b = vec![0x5A; 16];

    let signature_a = checked_ed25519_signature(&keypair_a, &message_a);
    let mut signature_b = checked_ed25519_signature(&keypair_b, &message_b)
        .payload()
        .to_vec();
    signature_b[0] ^= 0xFF;

    let pk_a = checked_ed25519_public_key_payload(&keypair_a);
    let pk_b = checked_ed25519_public_key_payload(&keypair_b);

    let msg_refs: Vec<&[u8]> = vec![message_a.as_slice(), message_b.as_slice()];
    let sig_refs: Vec<&[u8]> = vec![signature_a.payload(), signature_b.as_slice()];
    let pk_refs: Vec<&[u8]> = vec![pk_a, pk_b];

    assert!(matches!(
        ed25519_verify_aggregate(&msg_refs, &sig_refs, &pk_refs),
        Err(Error::BadSignature)
    ));
}

#[test]
fn ed25519_verify_aggregate_rejects_empty_input() {
    let empty: Vec<&[u8]> = Vec::new();
    assert!(matches!(
        ed25519_verify_aggregate(&empty, &empty, &empty),
        Err(Error::BadSignature)
    ));
}

#[test]
fn ed25519_batch_deterministic_accepts_valid_batch() {
    let (messages, signatures, public_keys) = sample_ed25519_batch(4);
    let msg_refs: Vec<&[u8]> = messages.iter().map(Vec::as_slice).collect();
    let sig_refs: Vec<&[u8]> = signatures.iter().map(Vec::as_slice).collect();
    let pk_refs: Vec<&[u8]> = public_keys.iter().map(Vec::as_slice).collect();

    assert!(ed25519_verify_batch_deterministic(&msg_refs, &sig_refs, &pk_refs, [0x42; 32]).is_ok());
}

#[test]
fn ed25519_batch_deterministic_rejects_invalid_member() {
    let (messages, mut signatures, public_keys) = sample_ed25519_batch(4);
    signatures[2][0] ^= 0x80;
    let msg_refs: Vec<&[u8]> = messages.iter().map(Vec::as_slice).collect();
    let sig_refs: Vec<&[u8]> = signatures.iter().map(Vec::as_slice).collect();
    let pk_refs: Vec<&[u8]> = public_keys.iter().map(Vec::as_slice).collect();

    assert!(matches!(
        ed25519_verify_batch_deterministic(&msg_refs, &sig_refs, &pk_refs, [0x42; 32]),
        Err(Error::BadSignature)
    ));
}

#[test]
fn ed25519_batch_deterministic_preserves_order_binding() {
    let (messages, signatures, mut public_keys) = sample_ed25519_batch(3);
    public_keys.swap(0, 1);
    let msg_refs: Vec<&[u8]> = messages.iter().map(Vec::as_slice).collect();
    let sig_refs: Vec<&[u8]> = signatures.iter().map(Vec::as_slice).collect();
    let pk_refs: Vec<&[u8]> = public_keys.iter().map(Vec::as_slice).collect();

    assert!(matches!(
        ed25519_verify_batch_deterministic(&msg_refs, &sig_refs, &pk_refs, [0x42; 32]),
        Err(Error::BadSignature)
    ));
}

#[test]
fn ed25519_batch_deterministic_matches_single_verification() {
    let (messages, signatures, public_keys) = sample_ed25519_batch(5);

    for ((message, signature), public_key) in messages.iter().zip(&signatures).zip(&public_keys) {
        assert!(
            ed25519_verify_batch_deterministic(
                &[message.as_slice()],
                &[signature.as_slice()],
                &[public_key.as_slice()],
                [0x24; 32],
            )
            .is_ok()
        );
    }
}

fn sample_ed25519_batch(count: u8) -> Ed25519Batch {
    let mut messages = Vec::new();
    let mut signatures = Vec::new();
    let mut public_keys = Vec::new();

    for idx in 0..count {
        let keypair = checked_ed25519_keypair();
        let message = vec![idx; 32];
        let signature = checked_ed25519_signature(&keypair, &message);
        let pk_bytes = checked_ed25519_public_key_payload(&keypair);

        messages.push(message);
        signatures.push(signature.payload().to_vec());
        public_keys.push(pk_bytes.to_vec());
    }

    (messages, signatures, public_keys)
}
