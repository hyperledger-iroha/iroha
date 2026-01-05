//! SM2 deterministic signing regression for the Rust SDK.
#![cfg(feature = "sm")]

use core::fmt::Write as _;
use std::{fs, path::PathBuf};

use hex::FromHex;
use iroha::crypto::sm::{encode_sm2_private_key_payload, encode_sm2_public_key_payload};
use iroha::crypto::{self, Algorithm, KeyPair, PrivateKey, PublicKey, Sm2PrivateKey, Sm2PublicKey};
use norito::derive::JsonDeserialize;

struct DistIdGuard(String);

impl Drop for DistIdGuard {
    fn drop(&mut self) {
        Sm2PublicKey::set_default_distid(self.0.clone())
            .expect("restore default distid");
    }
}

#[derive(Debug, JsonDeserialize)]
struct Fixture {
    distid: String,
    seed_hex: String,
    message_hex: String,
    private_key_hex: String,
    public_key_sec1_hex: String,
    public_key_multihash: String,
    public_key_prefixed: String,
    za: String,
    signature: String,
    r: String,
    s: String,
}

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("sm")
        .join("sm2_fixture.json")
}

fn load_fixture() -> Fixture {
    let payload = fs::read_to_string(fixture_path()).expect("fixture present");
    norito::json::from_str(&payload).expect("valid fixture JSON")
}

fn to_upper_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut out, "{byte:02X}").expect("write hex");
    }
    out
}

#[test]
fn sm2_signatures_are_deterministic() {
    let fixture = load_fixture();

    let original_distid = Sm2PublicKey::default_distid();
    Sm2PublicKey::set_default_distid(fixture.distid.clone())
        .expect("override distid");
    let _guard = DistIdGuard(original_distid);

    let seed = <[u8; 32]>::from_hex(&fixture.seed_hex).expect("fixture seed should yield 32 bytes");
    let private =
        Sm2PrivateKey::from_seed(&fixture.distid, &seed).expect("derive deterministic SM2 key");
    let public = private.public_key();
    let public_bytes = public.to_sec1_bytes(false);
    let secret_bytes = private.secret_bytes();
    assert_eq!(to_upper_hex(&secret_bytes), fixture.private_key_hex);
    assert_eq!(to_upper_hex(&public_bytes), fixture.public_key_sec1_hex);

    let public_payload =
        encode_sm2_public_key_payload(&fixture.distid, &public_bytes).expect("public payload");
    let private_payload =
        encode_sm2_private_key_payload(&fixture.distid, &secret_bytes).expect("private payload");
    let key_pair = KeyPair::new(
        PublicKey::from_bytes(Algorithm::Sm2, &public_payload).expect("SM2 public key"),
        PrivateKey::from_bytes(Algorithm::Sm2, &private_payload).expect("SM2 private key"),
    )
    .expect("construct SM2 key pair");

    let payload = Vec::from_hex(&fixture.message_hex).expect("fixture message hex");

    let signature_a = crypto::Signature::new(key_pair.private_key(), &payload);
    let signature_b = crypto::Signature::new(key_pair.private_key(), &payload);
    assert_eq!(signature_a.payload(), signature_b.payload());

    let raw_signature = private.sign(&payload);
    assert_eq!(signature_a.payload(), raw_signature.as_bytes());
    assert_eq!(
        key_pair.public_key().to_string(),
        fixture.public_key_multihash
    );
    assert_eq!(
        key_pair.public_key().to_prefixed_string(),
        fixture.public_key_prefixed
    );

    signature_a
        .verify(key_pair.public_key(), &payload)
        .expect("signature verifies");
    let za_hex = to_upper_hex(
        &private
            .public_key()
            .compute_z(&fixture.distid)
            .expect("compute ZA"),
    );
    assert_eq!(za_hex, fixture.za);
    let mut tampered = payload.clone();
    tampered.push(0);
    assert!(
        signature_a
            .verify(key_pair.public_key(), &tampered)
            .is_err()
    );

    let actual_hex = to_upper_hex(signature_a.payload());
    assert_eq!(actual_hex, fixture.signature);
    assert_eq!(
        &actual_hex[..fixture.r.len()],
        fixture.r,
        "fixture r component mismatch"
    );
    assert_eq!(
        &actual_hex[fixture.r.len()..],
        fixture.s,
        "fixture s component mismatch"
    );
}
