#![allow(unexpected_cfgs, clippy::uninlined_format_args, clippy::doc_markdown)]

//! Regression: ensure hybrid packed-struct bitset aligns for (u64, `SignatureOf`<T>).
//!
//! This test encodes a simple struct consisting of a fixed-size field (u64)
//! followed by a signature wrapper (`SignatureOf`<()>). Because signatures are
//! now encoded as fixed-size payloads, the packed-struct bitset should remain
//! zero for both positions.

use iroha_crypto::{Algorithm, HashOf, KeyPair, Signature, SignatureOf};

#[derive(norito::derive::Encode, norito::derive::Decode, Debug, Clone, PartialEq, Eq)]
struct USig {
    a: u64,
    b: SignatureOf<()>,
}

fn checked_ed25519_keypair() -> KeyPair {
    KeyPair::try_random_with_algorithm(Algorithm::Ed25519)
        .expect("generate checked Ed25519 keypair")
}

fn checked_signature(keypair: &KeyPair, message: &[u8]) -> Signature {
    Signature::try_new(keypair.private_key(), message).expect("sign packed signature fixture")
}

fn checked_signature_of_unit(keypair: &KeyPair) -> SignatureOf<()> {
    SignatureOf::try_from_hash(keypair.private_key(), HashOf::new(&()))
        .expect("sign packed SignatureOf fixture")
}

#[test]
fn checked_packed_signature_fixtures_verify_and_reject_wrong_key() {
    let keypair = checked_ed25519_keypair();
    let wrong_key = checked_ed25519_keypair();

    let signature = checked_signature(&keypair, b"packed-signature");
    signature
        .verify(keypair.public_key(), b"packed-signature")
        .expect("checked packed signature fixture verifies");
    signature
        .verify(wrong_key.public_key(), b"packed-signature")
        .expect_err("checked packed signature fixture rejects wrong key");

    let signature_of = checked_signature_of_unit(&keypair);
    let hash = HashOf::new(&());
    signature_of
        .verify_hash(keypair.public_key(), hash)
        .expect("checked packed SignatureOf fixture verifies");
    signature_of
        .verify_hash(wrong_key.public_key(), hash)
        .expect_err("checked packed SignatureOf fixture rejects wrong key");
}

#[test]
fn packed_bitset_alignment_for_u64_signatureof() {
    // Build a deterministic, small value
    let kp = checked_ed25519_keypair();
    let sig = checked_signature_of_unit(&kp);
    let value = USig { a: 42, b: sig };

    // Encode via header-framed Norito path
    let bytes = norito::core::to_bytes(&value).expect("encode");

    // Header flags live in the last header byte; PACKED_STRUCT is bit 0x04
    let archived = norito::core::from_bytes::<USig>(&bytes).expect("from_bytes");
    let got = norito::core::NoritoDeserialize::deserialize(archived);
    assert_eq!(got, value);
}

#[derive(norito::derive::Encode, norito::derive::Decode, Debug, Clone, PartialEq, Eq)]
struct USignature {
    a: u64,
    b: Signature,
}

#[test]
fn packed_bitset_alignment_for_u64_signature() {
    let kp = checked_ed25519_keypair();
    let sig = checked_signature(&kp, b"x");
    let value = USignature { a: 7, b: sig };

    let bytes = norito::core::to_bytes(&value).expect("encode");
    let archived = norito::core::from_bytes::<USignature>(&bytes).expect("from_bytes");
    let got = norito::core::NoritoDeserialize::deserialize(archived);
    assert_eq!(got, value);
}

#[derive(norito::derive::Encode, norito::derive::Decode, Debug, Clone, PartialEq, Eq)]
struct TupSigOf(u64, SignatureOf<()>);

#[test]
fn packed_bitset_alignment_for_tuple_u64_signatureof() {
    let kp = checked_ed25519_keypair();
    let sig = checked_signature_of_unit(&kp);
    let value = TupSigOf(1, sig);
    let bytes = norito::core::to_bytes(&value).expect("encode");
    let archived = norito::core::from_bytes::<TupSigOf>(&bytes).expect("from_bytes");
    let got = norito::core::NoritoDeserialize::deserialize(archived);
    assert_eq!(got, value);
}

#[derive(norito::derive::Encode, norito::derive::Decode, Debug, Clone, PartialEq, Eq)]
struct TupSig(Signature, u64);

#[test]
fn packed_bitset_alignment_for_tuple_signature_u64() {
    let kp = checked_ed25519_keypair();
    let sig = checked_signature(&kp, b"y");
    let value = TupSig(sig, 2);
    let bytes = norito::core::to_bytes(&value).expect("encode");
    let archived = norito::core::from_bytes::<TupSig>(&bytes).expect("from_bytes");
    let got = norito::core::NoritoDeserialize::deserialize(archived);
    assert_eq!(got, value);
}

#[derive(norito::derive::Encode, norito::derive::Decode, Debug, Clone, PartialEq, Eq)]
struct SigOfUNamed {
    b: SignatureOf<()>,
    a: u64,
}

#[test]
fn packed_bitset_alignment_for_signatureof_u64_named() {
    let kp = checked_ed25519_keypair();
    let sig = checked_signature_of_unit(&kp);
    let value = SigOfUNamed { b: sig, a: 3 };
    let bytes = norito::core::to_bytes(&value).expect("encode");
    let archived = norito::core::from_bytes::<SigOfUNamed>(&bytes).expect("from_bytes");
    let got = norito::core::NoritoDeserialize::deserialize(archived);
    assert_eq!(got, value);
}
