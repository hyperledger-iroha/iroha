//! Provider token interoperability, key-admission and opt-in timing checks.

use super::*;
use aws_lc_rs::signature::KeyPair as _;
use base64::engine::general_purpose::STANDARD;
use p256::{
    ecdsa::{Signature, VerifyingKey, signature::Verifier as _},
    pkcs8::{
        DecodePublicKey as _,
        der::{Encode as _, Tag, asn1::AnyRef},
    },
};

const RSA_PKCS8: &[u8] = include_bytes!("fixtures/rsa-pkcs8.pem");
const RSA_PKCS1: &[u8] = include_bytes!("fixtures/rsa-pkcs1.pem");
const EC_PKCS8: &[u8] = include_bytes!("fixtures/ec-p256-pkcs8.pem");
const EC_PUBLIC: &[u8] = include_bytes!("fixtures/ec-p256-public.pem");

fn claims() -> norito::json::Value {
    norito::json!({"iss": "issuer", "iat": 1_700_000_000_u64})
}

fn token_parts(token: &str, algorithm: &str, key_id: Option<&str>) -> (String, Vec<u8>) {
    let parts: Vec<_> = token.split('.').collect();
    assert_eq!(parts.len(), 3);
    assert!(parts.iter().all(|part| !part.contains('=')));
    let header: norito::json::Value =
        norito::json::from_slice(&URL_SAFE_NO_PAD.decode(parts[0]).unwrap()).unwrap();
    assert_eq!(header["typ"].as_str(), Some("JWT"));
    assert_eq!(header["alg"].as_str(), Some(algorithm));
    assert_eq!(
        header.get("kid").and_then(norito::json::Value::as_str),
        key_id
    );
    let payload: norito::json::Value =
        norito::json::from_slice(&URL_SAFE_NO_PAD.decode(parts[1]).unwrap()).unwrap();
    assert_eq!(payload["iss"].as_str(), Some("issuer"));
    assert_eq!(payload["iat"].as_u64(), Some(1_700_000_000));
    (
        format!("{}.{}", parts[0], parts[1]),
        URL_SAFE_NO_PAD.decode(parts[2]).unwrap(),
    )
}

#[test]
fn fcm_tokens_match_openssl_for_both_private_key_formats() {
    let expected = include_str!("fixtures/fcm-token.txt").trim_end();
    for pem in [RSA_PKCS8, RSA_PKCS1] {
        let token = encode_fcm(&claims(), pem).unwrap();
        assert_eq!(token, expected);
        let (message, signature) = token_parts(&token, "RS256", None);
        let key = rsa_key(pem).unwrap();
        let verifier = signature::UnparsedPublicKey::new(
            &signature::RSA_PKCS1_2048_8192_SHA256,
            key.public_key().as_ref(),
        );
        verifier.verify(message.as_bytes(), &signature).unwrap();
        assert!(
            verifier
                .verify(b"different signing input", &signature)
                .is_err()
        );
        let mut changed = signature;
        changed[0] ^= 1;
        assert!(verifier.verify(message.as_bytes(), &changed).is_err());
    }
}

#[test]
fn apns_tokens_use_verifiable_fixed_width_p256_signatures() {
    let (_, public_der) = <(SectionKind, Vec<u8>)>::from_pem_slice(EC_PUBLIC).unwrap();
    let verifier = VerifyingKey::from_public_key_der(&public_der).unwrap();
    let key_id = "test-key\"\\id";
    let token = encode_apns(key_id, &claims(), EC_PKCS8).unwrap();
    let (message, signature) = token_parts(&token, "ES256", Some(key_id));
    assert_eq!(signature.len(), 64, "JOSE requires r||s, not DER");
    let signature = Signature::from_slice(&signature).unwrap();
    verifier.verify(message.as_bytes(), &signature).unwrap();
    assert!(
        verifier
            .verify(b"different signing input", &signature)
            .is_err()
    );
}

#[test]
fn key_loaders_reject_wrong_family_curve_and_public_material() {
    assert!(encode_fcm(&claims(), EC_PKCS8).is_err());
    for pem in [
        RSA_PKCS8,
        RSA_PKCS1,
        include_bytes!("fixtures/ec-p384-pkcs8.pem").as_slice(),
        include_bytes!("fixtures/ec-p256-sec1.pem").as_slice(),
        include_bytes!("fixtures/rsa-public.pem").as_slice(),
        EC_PUBLIC,
        b"",
    ] {
        assert!(encode_apns("key-id", &claims(), pem).is_err());
    }
    assert!(encode_fcm(&claims(), include_bytes!("fixtures/rsa-public.pem")).is_err());
}

#[test]
fn pem_admission_preserves_crlf_and_surrounding_whitespace() {
    for pem in [RSA_PKCS8, RSA_PKCS1, EC_PKCS8] {
        let original = private_key_der(pem).unwrap();
        let crlf = String::from_utf8(pem.to_vec())
            .unwrap()
            .replace('\n', "\r\n");
        for changed in [crlf.clone(), format!(" \t\r\n{crlf}\r\n\t ")] {
            let actual = private_key_der(changed.as_bytes()).unwrap();
            assert_eq!(actual.0, original.0);
            assert_eq!(actual.1.as_slice(), original.1.as_slice());
        }
    }
}

#[test]
fn pem_admission_rejects_ambiguous_and_malformed_envelopes() {
    let valid = std::str::from_utf8(EC_PKCS8).unwrap();
    let malformed = [
        format!("{valid}{valid}"),
        format!("ignored preamble\n{valid}"),
        format!("{valid}trailing garbage\n"),
        valid.replace(
            "-----END PRIVATE KEY-----",
            "-----END PRIVATE KEY-----garbage",
        ),
        valid.replace(
            "-----BEGIN PRIVATE KEY-----",
            "-----BEGIN PRIVATE KEY-----garbage",
        ),
        valid.replace("-----END PRIVATE KEY-----", "-----END RSA PRIVATE KEY-----"),
        valid.replace("-----END PRIVATE KEY-----", ""),
        format!("-----BEGIN PRIVATE KEY-----\n{valid}"),
        format!("-----BEGIN UNKNOWN-----\n-----END UNKNOWN-----\n{valid}"),
        "-----BEGIN PRIVATE KEY-----\ninvalid%%base64\n-----END PRIVATE KEY-----\n".into(),
    ];
    for pem in malformed {
        assert!(private_key_der(pem.as_bytes()).is_err());
    }
}

fn pem_with_der(label: &str, der: &[u8]) -> Vec<u8> {
    format!(
        "-----BEGIN {label}-----\n{}\n-----END {label}-----\n",
        STANDARD.encode(der)
    )
    .into_bytes()
}

#[test]
fn der_admission_rejects_trailing_bytes_and_malformed_sequences() {
    for (pem, label) in [
        (RSA_PKCS8, "PRIVATE KEY"),
        (RSA_PKCS1, "RSA PRIVATE KEY"),
        (EC_PKCS8, "PRIVATE KEY"),
    ] {
        let (_, original) = private_key_der(pem).unwrap();
        let mut trailing = original.to_vec();
        trailing.push(0);
        assert!(private_key_der(&pem_with_der(label, &trailing)).is_err());
        assert!(private_key_der(&pem_with_der(label, &original[..original.len() - 1])).is_err());
    }
    for malformed in [&[][..], &[0x05, 0x00], &[0x30, 0x80, 0x00, 0x00]] {
        assert!(private_key_der(&pem_with_der("PRIVATE KEY", malformed)).is_err());
    }
}

#[test]
fn signers_reject_complete_der_sequences_that_are_not_private_keys() {
    for der in [&[0x30, 0x00][..], &[0x30, 0x03, 0x02, 0x01, 0x00]] {
        assert!(encode_fcm(&claims(), &pem_with_der("RSA PRIVATE KEY", der)).is_err());
        assert!(encode_fcm(&claims(), &pem_with_der("PRIVATE KEY", der)).is_err());
        assert!(encode_apns("key-id", &claims(), &pem_with_der("PRIVATE KEY", der)).is_err());
    }
}

#[test]
fn signers_reject_extra_fields_inside_private_key_sequences() {
    for (pem, label, is_rsa) in [
        (RSA_PKCS8, "PRIVATE KEY", true),
        (RSA_PKCS1, "RSA PRIVATE KEY", true),
        (EC_PKCS8, "PRIVATE KEY", false),
    ] {
        let (_, der) = private_key_der(pem).unwrap();
        let sequence = <&SequenceRef>::from_der(&der).unwrap();
        let mut body = sequence.as_bytes().to_vec();
        body.extend_from_slice(&[0x05, 0x00]); // Extra DER NULL inside the sequence.
        let changed = AnyRef::new(Tag::Sequence, &body).unwrap().to_der().unwrap();
        let changed = pem_with_der(label, &changed);
        if is_rsa {
            assert!(encode_fcm(&claims(), &changed).is_err());
        } else {
            assert!(encode_apns("key-id", &claims(), &changed).is_err());
        }
    }
}

#[test]
fn pem_size_limit_is_inclusive_and_rejects_before_decoding() {
    let mut exact = EC_PKCS8.to_vec();
    exact.resize(MAX_PRIVATE_KEY_PEM_BYTES, b' ');
    assert!(private_key_der(&exact).is_ok());
    exact.push(b' ');
    assert_eq!(
        private_key_der(&exact).unwrap_err(),
        "push signing key exceeds the PEM size limit"
    );
}

#[test]
#[ignore = "opt-in timing measurement; run with --ignored --nocapture"]
fn benchmark_provider_tokens() {
    use std::{hint::black_box, time::Instant};

    let claims = claims();
    let fcm_message = signing_input("RS256", None, &claims).unwrap();
    let apns_message = signing_input("ES256", Some("key-id"), &claims).unwrap();
    fn measure(name: &str, workload: impl Fn()) {
        workload();
        let mut samples = Vec::new();
        for _ in 0..9 {
            let start = Instant::now();
            for _ in 0..32 {
                workload();
            }
            samples.push(start.elapsed().as_nanos() / 32);
        }
        samples.sort_unstable();
        println!("{name}: median_ns={} samples_ns={samples:?}", samples[4]);
    }
    measure("FCM key-load-and-sign", || {
        let key = rsa_key(black_box(RSA_PKCS8)).unwrap();
        let mut signature = vec![0; key.public_modulus_len()];
        key.sign(
            &signature::RSA_PKCS1_SHA256,
            &SystemRandom::new(),
            black_box(fcm_message.as_bytes()),
            &mut signature,
        )
        .unwrap();
        black_box(signature);
    });
    measure("APNs key-load-and-sign", || {
        let key = ec_key(black_box(EC_PKCS8)).unwrap();
        black_box(
            key.sign(&SystemRandom::new(), black_box(apns_message.as_bytes()))
                .unwrap(),
        );
    });
    measure("FCM complete-token", || {
        black_box(encode_fcm(black_box(&claims), black_box(RSA_PKCS8)).unwrap());
    });
    measure("APNs complete-token", || {
        black_box(
            encode_apns(black_box("key-id"), black_box(&claims), black_box(EC_PKCS8)).unwrap(),
        );
    });
}
