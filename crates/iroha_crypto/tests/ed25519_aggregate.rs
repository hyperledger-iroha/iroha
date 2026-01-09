//! Tests for Ed25519 aggregate-style verification.

use iroha_crypto::{Algorithm, Error, KeyPair, Signature, ed25519_verify_aggregate};

#[test]
fn ed25519_verify_aggregate_accepts_valid_signatures() {
    let mut messages = Vec::new();
    let mut signatures = Vec::new();
    let mut public_keys = Vec::new();

    for idx in 0u8..3 {
        let keypair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let message = vec![idx; 32];
        let signature = Signature::new(keypair.private_key(), &message);
        let (_alg, pk_bytes) = keypair.public_key().to_bytes();

        messages.push(message);
        signatures.push(signature.payload().to_vec());
        public_keys.push(pk_bytes.to_vec());
    }

    let msg_refs: Vec<&[u8]> = messages.iter().map(|m| m.as_slice()).collect();
    let sig_refs: Vec<&[u8]> = signatures.iter().map(|s| s.as_slice()).collect();
    let pk_refs: Vec<&[u8]> = public_keys.iter().map(|p| p.as_slice()).collect();

    assert!(ed25519_verify_aggregate(&msg_refs, &sig_refs, &pk_refs).is_ok());
}

#[test]
fn ed25519_verify_aggregate_rejects_tampered_signature() {
    let keypair_a = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let keypair_b = KeyPair::random_with_algorithm(Algorithm::Ed25519);

    let message_a = vec![0xA5; 16];
    let message_b = vec![0x5A; 16];

    let signature_a = Signature::new(keypair_a.private_key(), &message_a);
    let mut signature_b = Signature::new(keypair_b.private_key(), &message_b)
        .payload()
        .to_vec();
    signature_b[0] ^= 0xFF;

    let (_alg_a, pk_a) = keypair_a.public_key().to_bytes();
    let (_alg_b, pk_b) = keypair_b.public_key().to_bytes();

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
