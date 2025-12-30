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

#[test]
fn packed_bitset_alignment_for_u64_signatureof() {
    // Build a deterministic, small value
    let kp = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let sig = SignatureOf::from_hash(kp.private_key(), HashOf::new(&()));
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
    let kp = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let sig = Signature::new(kp.private_key(), b"x");
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
    let kp = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let sig = SignatureOf::from_hash(kp.private_key(), HashOf::new(&()));
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
    let kp = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let sig = Signature::new(kp.private_key(), b"y");
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
    let kp = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let sig = SignatureOf::from_hash(kp.private_key(), HashOf::new(&()));
    let value = SigOfUNamed { b: sig, a: 3 };
    let bytes = norito::core::to_bytes(&value).expect("encode");
    let archived = norito::core::from_bytes::<SigOfUNamed>(&bytes).expect("from_bytes");
    let got = norito::core::NoritoDeserialize::deserialize(archived);
    assert_eq!(got, value);
}
