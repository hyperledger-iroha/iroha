//! Integration tests covering SM2 key pair behavior.
#![cfg(feature = "sm")]

use iroha_crypto::{
    Algorithm, Error, KeyPair, Signature, Sm2PrivateKey, Sm2PublicKey, Sm2Signature,
};
use rand::{RngCore, SeedableRng as _};
use rand_chacha::ChaCha20Rng;

#[test]
fn sm2_keypair_sign_and_verify() {
    let keypair = KeyPair::random_with_algorithm(Algorithm::Sm2);
    let msg = b"iroha sm2 keypair smoke test";

    let signature = Signature::new(keypair.private_key(), msg);
    signature
        .verify(keypair.public_key(), msg)
        .expect("sm2 signature should verify");
}

#[test]
fn sm2_signing_is_deterministic_with_same_key_and_message() {
    let secret = [0x42u8; 32];
    let private =
        Sm2PrivateKey::new(Sm2PublicKey::DEFAULT_DISTID, secret).expect("secret should be valid");
    let message = b"deterministic sm2 signing check";

    let sig1 = private.sign(message);
    let sig2 = private.sign(message);
    assert_eq!(sig1.to_bytes(), sig2.to_bytes(), "signatures must match");

    // Rehydrate the key from bytes to ensure determinism survives reconstruction.
    let reconstructed =
        Sm2PrivateKey::from_bytes(Sm2PublicKey::DEFAULT_DISTID, &secret).expect("valid key");
    let sig3 = reconstructed.sign(message);
    assert_eq!(
        sig1.to_bytes(),
        sig3.to_bytes(),
        "signatures must remain stable"
    );

    let public = private.public_key();
    public
        .verify(message, &sig1)
        .expect("signature should verify with deterministic nonce");
}

#[test]
fn sm2_signature_rejects_malformed_payloads() {
    let mut rng = ChaCha20Rng::from_seed([7u8; 32]);
    let keypair = KeyPair::random_with_algorithm(Algorithm::Sm2);
    let message = b"malformed signature test vector";

    let signature = Signature::new(keypair.private_key(), message);
    signature
        .verify(keypair.public_key(), message)
        .expect("baseline signature should verify");

    // Remove one byte to simulate truncated r∥s payload.
    let mut truncated = signature.payload().to_vec();
    truncated.pop();
    let truncated_sig = Signature::from_bytes(&truncated);
    assert!(
        matches!(
            truncated_sig.verify(keypair.public_key(), message),
            Err(Error::BadSignature)
        ),
        "truncated signature must be rejected"
    );

    // Corrupt r component and ensure verification fails.
    let mut corrupted = signature.payload().to_vec();
    // Flip a random byte to keep failure non-deterministic across runs while staying reproducible.
    let idx = (rng.next_u32() as usize) % corrupted.len();
    corrupted[idx] ^= 0xFF;
    let corrupted = Signature::from_bytes(&corrupted);
    assert!(
        matches!(
            corrupted.verify(keypair.public_key(), message),
            Err(Error::BadSignature)
        ),
        "corrupted signature must be rejected"
    );
}

#[test]
fn sm2_signature_rejects_wrong_message_or_distid() {
    let secret = [0x33u8; 32];
    let private =
        Sm2PrivateKey::new(Sm2PublicKey::DEFAULT_DISTID, secret).expect("secret should construct");
    let message = b"sm2 integrity check";
    let signature = private.sign(message);
    let public = private.public_key();

    assert!(
        matches!(
            public.verify(b"sm2 integrity check (altered)", &signature),
            Err(Error::BadSignature)
        ),
        "verification must fail for altered message"
    );

    let sec1 = public.to_sec1_bytes(false);
    let mismatched =
        Sm2PublicKey::from_sec1_bytes("ALICE123@YAHOO.COM", &sec1).expect("sec1 point valid");
    assert!(
        matches!(
            mismatched.verify(message, &signature),
            Err(Error::BadSignature)
        ),
        "verification must fail when distinguishing ID differs"
    );
}

#[test]
fn sm2_signature_rejects_zero_components() {
    let keypair = KeyPair::random_with_algorithm(Algorithm::Sm2);
    let message = b"invalid zero-component signature";
    let zero_signature = Signature::from_bytes(&[0u8; Sm2Signature::LENGTH]);

    assert!(
        matches!(
            zero_signature.verify(keypair.public_key(), message),
            Err(Error::Parse(_))
        ),
        "signature with zeroed r and s must fail parsing"
    );
}

#[test]
fn sm2_signature_rejects_high_scalar_components() {
    let bytes = [0xFFu8; Sm2Signature::LENGTH];
    let signature = Signature::from_bytes(&bytes);
    let keypair = KeyPair::random_with_algorithm(Algorithm::Sm2);
    let message = b"invalid high-component signature";

    assert!(
        matches!(
            signature.verify(keypair.public_key(), message),
            Err(Error::Parse(_))
        ),
        "signature with components >= n must be rejected"
    );
}

#[test]
fn sm2_public_key_rejects_points_off_curve() {
    let mut invalid = [0u8; 65];
    invalid[0] = 0x04;
    invalid[1..33].copy_from_slice(&[0x01; 32]);
    invalid[33..].copy_from_slice(&[0x02; 32]);

    assert!(
        Sm2PublicKey::from_sec1_bytes(Sm2PublicKey::DEFAULT_DISTID, &invalid).is_err(),
        "off-curve SEC1 point must be rejected"
    );
}

#[test]
fn sm2_signatures_depend_on_distinguishing_id() {
    let secret = [0x7Au8; 32];
    let message = b"distid-influences-sm2-prehash";

    let default_key =
        Sm2PrivateKey::new(Sm2PublicKey::DEFAULT_DISTID, secret).expect("default key");
    let custom_key = Sm2PrivateKey::new("device:alpha", secret).expect("custom key");

    let default_sig = default_key.sign(message);
    let custom_sig = custom_key.sign(message);

    assert_ne!(
        default_sig.to_bytes(),
        custom_sig.to_bytes(),
        "signatures must change when the distinguishing identifier differs"
    );

    default_key
        .public_key()
        .verify(message, &default_sig)
        .expect("default signature should verify with matching distid");
    custom_key
        .public_key()
        .verify(message, &custom_sig)
        .expect("custom signature should verify with matching distid");

    assert!(
        custom_key
            .public_key()
            .verify(message, &default_sig)
            .is_err(),
        "reusing a signature with a mismatched distid must fail verification"
    );
    assert!(
        default_key
            .public_key()
            .verify(message, &custom_sig)
            .is_err(),
        "distid mismatch must remain a verification failure"
    );
}
