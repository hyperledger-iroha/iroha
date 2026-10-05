//! Digest roles, preimages, transcripts and signature freezing.

use std::collections::BTreeSet;

use p256::ecdsa::{Signature as P256Signature, SigningKey, signature::Signer as _};
use sha2::{Digest as _, Sha256};

use super::*;

/// Exact role labels in declaration order (split lineage: design §1.5).
const ROLE_LABELS: [&str; 62] = [
    "scheme",
    "relation",
    "provider-contract",
    "asset-scope",
    "account",
    "enrollment-challenge",
    "enrollment-id",
    "enrollment-key-binding",
    "wallet-id",
    "certificate-body",
    "certificate",
    "certificate-set",
    "credential-body",
    "credential",
    "scheme-policy-body",
    "scheme-policy",
    "fee-schedule-body",
    "fee-schedule",
    "blacklist-body",
    "blacklist",
    "blacklist-leaf",
    "blacklist-node",
    "quota-share-body",
    "quota-share",
    "quota-window",
    "quota-node",
    "time-anchor-body",
    "time-anchor",
    "offer-body",
    "session-control-body",
    "request-body",
    "request",
    "credit",
    "statement",
    "proof",
    "step-proof",
    "lineage",
    "receipt-body",
    "receipt",
    "package",
    "payment",
    "credit-opening",
    "credit-status",
    "credited",
    "operation-id",
    "output",
    "capsule",
    "marker",
    "completion",
    "fold",
    "voucher-body",
    "voucher",
    "unload-nullifier",
    "ledger-control-body",
    "renewal-challenge",
    "renewal-key-binding",
    "renewal-assertion",
    "artifact-manifest-body",
    "artifact-manifest",
    "charge-quote-body",
    "charge-quote",
    "evidence",
];

/// P-256 group order `n`.
const ORDER: &str = "ffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551";
/// `floor(n / 2)`, the largest accepted `s`.
const HALF_ORDER: &str = "7fffffff800000007fffffffffffffffde737d56d38bcf4279dce5617e3192a8";

fn bytes32(hex_text: &str) -> [u8; 32] {
    let mut out = [0; 32];
    out.copy_from_slice(&hex::decode(hex_text).expect("hex"));
    out
}

fn sub_be(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    let mut out = [0; 32];
    let mut borrow = 0_i16;
    for index in (0..32).rev() {
        let mut value = i16::from(a[index]) - i16::from(b[index]) - borrow;
        borrow = 0;
        if value < 0 {
            value += 256;
            borrow = 1;
        }
        out[index] = u8::try_from(value).expect("byte");
    }
    assert_eq!(borrow, 0, "a >= b");
    out
}

fn raw(r: &[u8; 32], s: &[u8; 32]) -> [u8; 64] {
    let mut out = [0; 64];
    out[..32].copy_from_slice(r);
    out[32..].copy_from_slice(s);
    out
}

fn key(seed: u8) -> (SigningKey, KagemushaDevicePublicKeyV1) {
    let signing = SigningKey::from_bytes((&[seed; 32]).into()).expect("fixed scalar");
    let public = KagemushaDevicePublicKeyV1::from_sec1_bytes(
        signing.verifying_key().to_encoded_point(false).as_bytes(),
    )
    .expect("canonical key");
    (signing, public)
}

fn sign(signing: &SigningKey, role: KagemushaWalletDigestRoleV1, body: &[u8]) -> P256Signature {
    signing.sign(&kagemusha_wallet_preimage_v1(role, body))
}

fn low_and_high(signature: &P256Signature) -> ([u8; 64], [u8; 64]) {
    let bytes = signature.to_bytes();
    let mut r = [0; 32];
    let mut s = [0; 32];
    r.copy_from_slice(&bytes[..32]);
    s.copy_from_slice(&bytes[32..]);
    let twin = sub_be(&bytes32(ORDER), &s);
    if s <= bytes32(HALF_ORDER) {
        (raw(&r, &s), raw(&r, &twin))
    } else {
        (raw(&r, &twin), raw(&r, &s))
    }
}

#[test]
fn kagemusha_wallet_v1_role_labels_are_pinned_unique_and_ascii() {
    assert_eq!(KagemushaWalletDigestRoleV1::ALL.len(), ROLE_LABELS.len());
    let mut seen = BTreeSet::new();
    for (role, label) in KagemushaWalletDigestRoleV1::ALL.iter().zip(ROLE_LABELS) {
        assert_eq!(role.as_str(), label);
        assert!(seen.insert(format!("{role:?}")), "duplicate role {label}");
        assert!(
            label
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte == b'-'),
            "{label}"
        );
    }
    let labels: BTreeSet<&str> = ROLE_LABELS.into_iter().collect();
    assert_eq!(labels.len(), ROLE_LABELS.len());
    assert!(!labels.contains("policy-chunk"), "C3 drops policy-chunk");
    assert_eq!(
        KAGEMUSHA_WALLET_DIGEST_PREFIX_V1,
        b"iroha:kagemusha:wallet:v1:"
    );
}

#[test]
fn kagemusha_wallet_v1_preimage_layout_and_pinned_digests() {
    let empty = kagemusha_wallet_preimage_v1(KagemushaWalletDigestRoleV1::Scheme, &[]);
    assert_eq!(
        hex::encode(&empty),
        "69726f68613a6b6167656d757368613a77616c6c65743a76313a736368656d65000000000000000000"
    );
    assert_eq!(
        hex::encode(kagemusha_wallet_digest_v1(
            KagemushaWalletDigestRoleV1::Scheme,
            &[]
        )),
        "90882608a8e8892521a2661be1e81c3fbfca0f8773e621000daed362a37485a5"
    );
    let body = [0_u8, 1, 2, 3];
    let credit = kagemusha_wallet_preimage_v1(KagemushaWalletDigestRoleV1::Credit, &body);
    assert_eq!(
        hex::encode(&credit),
        "69726f68613a6b6167656d757368613a77616c6c65743a76313a63726564697400040000000000000000010203"
    );
    assert_eq!(
        hex::encode(kagemusha_wallet_digest_v1(
            KagemushaWalletDigestRoleV1::Credit,
            &body
        )),
        "faa1d5dde3ff201dfe03acf8b5a722555217924aec6fec2510ebd3c1608a4ae2"
    );
    let mut digests = BTreeSet::new();
    for role in KagemushaWalletDigestRoleV1::ALL {
        let preimage = kagemusha_wallet_preimage_v1(role, &body);
        let label = role.as_str().as_bytes();
        let prefix = KAGEMUSHA_WALLET_DIGEST_PREFIX_V1.len();
        assert_eq!(&preimage[..prefix], KAGEMUSHA_WALLET_DIGEST_PREFIX_V1);
        assert_eq!(&preimage[prefix..prefix + label.len()], label);
        assert_eq!(preimage[prefix + label.len()], 0);
        assert_eq!(
            &preimage[prefix + label.len() + 1..prefix + label.len() + 9],
            &4_u64.to_le_bytes()
        );
        assert_eq!(&preimage[prefix + label.len() + 9..], &body);
        let digest = kagemusha_wallet_digest_v1(role, &body);
        assert_eq!(digest, <[u8; 32]>::from(Sha256::digest(&preimage)));
        assert!(digests.insert(digest), "role {} collides", role.as_str());
    }
}

#[test]
fn kagemusha_wallet_v1_signed_object_digest_hashes_body_digest_then_signature() {
    let (signing, _) = key(7);
    let role = KagemushaWalletDigestRoleV1::CertificateBody;
    let (low, _) = low_and_high(&sign(&signing, role, b"body"));
    let signature = KagemushaDeviceSignatureV1::from_raw_bytes(&low).expect("low-S");
    let e = kagemusha_wallet_digest_v1(role, b"body");
    let mut body = e.to_vec();
    body.extend_from_slice(&low);
    assert_eq!(
        body.len(),
        KAGEMUSHA_WALLET_SIGNED_OBJECT_TRANSCRIPT_BYTES_V1
    );
    assert_eq!(
        kagemusha_wallet_signed_object_digest_v1(
            KagemushaWalletDigestRoleV1::Certificate,
            &e,
            &signature
        ),
        kagemusha_wallet_digest_v1(KagemushaWalletDigestRoleV1::Certificate, &body)
    );
}

#[test]
fn kagemusha_wallet_v1_transcript_builder_writes_fixed_widths() {
    let (signing, public) = key(9);
    let (low, _) = low_and_high(&sign(&signing, KagemushaWalletDigestRoleV1::Proof, b"x"));
    let signature = KagemushaDeviceSignatureV1::from_raw_bytes(&low).expect("low-S");
    let transcript = WalletTranscriptV1::with_capacity(0)
        .u8(0xab)
        .u16(0x0102)
        .u32(0x0304_0506)
        .u64(0x0708_090a_0b0c_0d0e)
        .digest(&[0x11; 32])
        .key(&public)
        .signature(&signature)
        .bytes(&[0xee, 0xff])
        .finish();
    let mut expected = vec![0xab, 0x02, 0x01, 0x06, 0x05, 0x04, 0x03];
    expected.extend_from_slice(&0x0708_090a_0b0c_0d0e_u64.to_le_bytes());
    expected.extend_from_slice(&[0x11; 32]);
    expected.extend_from_slice(public.as_sec1_bytes());
    expected.extend_from_slice(&low);
    expected.extend_from_slice(&[0xee, 0xff]);
    assert_eq!(transcript, expected);
    assert_eq!(transcript.len(), 1 + 2 + 4 + 8 + 32 + 65 + 64 + 2);

    let wide = WalletTranscriptV1::with_capacity(0)
        .u128(0x0102_0304_0506_0708_090a_0b0c_0d0e_0f10)
        .zeros(3)
        .u8(0x7f)
        .finish();
    let mut expected = 0x0102_0304_0506_0708_090a_0b0c_0d0e_0f10_u128
        .to_le_bytes()
        .to_vec();
    expected.extend_from_slice(&[0, 0, 0, 0x7f]);
    assert_eq!(wide, expected);
}

#[test]
fn kagemusha_wallet_v1_freeze_normalizes_der_and_raw_high_s() {
    let (signing, public) = key(7);
    let role = KagemushaWalletDigestRoleV1::CredentialBody;
    let body = b"credential transcript";
    let signature = sign(&signing, role, body);
    let (low, high) = low_and_high(&signature);
    let der = signature.to_der();
    for output in [
        KagemushaWalletSignerOutputV1::Raw(low),
        KagemushaWalletSignerOutputV1::Raw(high),
        KagemushaWalletSignerOutputV1::Der(der.as_bytes()),
    ] {
        let frozen =
            kagemusha_wallet_freeze_signature_v1(&public, role, body, output).expect("freeze");
        assert_eq!(frozen.as_raw_bytes(), &low);
        kagemusha_wallet_verify_signature_v1(&public, role, body, &frozen).expect("verify");
    }
    let high_der = P256Signature::from_slice(&high).expect("high twin parses");
    let frozen = kagemusha_wallet_freeze_signature_v1(
        &public,
        role,
        body,
        KagemushaWalletSignerOutputV1::Der(high_der.to_der().as_bytes()),
    )
    .expect("freeze high-S DER");
    assert_eq!(frozen.as_raw_bytes(), &low);
}

#[test]
fn kagemusha_wallet_v1_freeze_rejects_invalid_or_unbound_output() {
    let (signing, public) = key(7);
    let (_, other) = key(8);
    let role = KagemushaWalletDigestRoleV1::RequestBody;
    let body = b"request transcript";
    let signature = sign(&signing, role, body);
    let (low, _) = low_and_high(&signature);
    let rejected =
        |result: Result<KagemushaDeviceSignatureV1, KagemushaWalletValidationErrorV1>| {
            matches!(
                result,
                Err(KagemushaWalletValidationErrorV1::InvalidSignature { .. })
            )
        };
    let raw_output = KagemushaWalletSignerOutputV1::Raw(low);
    assert!(rejected(kagemusha_wallet_freeze_signature_v1(
        &other, role, body, raw_output
    )));
    assert!(rejected(kagemusha_wallet_freeze_signature_v1(
        &public,
        KagemushaWalletDigestRoleV1::OfferBody,
        body,
        raw_output
    )));
    assert!(rejected(kagemusha_wallet_freeze_signature_v1(
        &public,
        role,
        b"other transcript",
        raw_output
    )));
    let mut der = signature.to_der().as_bytes().to_vec();
    der.push(0);
    assert!(rejected(kagemusha_wallet_freeze_signature_v1(
        &public,
        role,
        body,
        KagemushaWalletSignerOutputV1::Der(&der)
    )));
    let order = bytes32(ORDER);
    let one = {
        let mut one = [0; 32];
        one[31] = 1;
        one
    };
    for invalid in [
        raw(&[0; 32], &one),
        raw(&one, &[0; 32]),
        raw(&order, &one),
        raw(&one, &order),
    ] {
        assert!(rejected(kagemusha_wallet_freeze_signature_v1(
            &public,
            role,
            body,
            KagemushaWalletSignerOutputV1::Raw(invalid)
        )));
    }
}

#[test]
fn kagemusha_wallet_v1_verify_rejects_high_s_and_wrong_bindings() {
    let (signing, public) = key(7);
    let (_, other) = key(8);
    let role = KagemushaWalletDigestRoleV1::ReceiptBody;
    let body = b"receipt transcript";
    let (low, high) = low_and_high(&sign(&signing, role, body));
    assert!(KagemushaDeviceSignatureV1::from_raw_bytes(&high).is_err());
    let signature = KagemushaDeviceSignatureV1::from_raw_bytes(&low).expect("low-S");
    assert!(kagemusha_wallet_verify_signature_v1(&public, role, body, &signature).is_ok());
    for (key, role, body) in [
        (&other, role, &body[..]),
        (&public, KagemushaWalletDigestRoleV1::Receipt, &body[..]),
        (&public, role, &b"tampered"[..]),
    ] {
        assert!(matches!(
            kagemusha_wallet_verify_signature_v1(key, role, body, &signature),
            Err(KagemushaWalletValidationErrorV1::InvalidSignature { .. })
        ));
    }
}

#[test]
fn kagemusha_wallet_v1_signature_boundary_scalars() {
    let order = bytes32(ORDER);
    let half = bytes32(HALF_ORDER);
    let mut one = [0; 32];
    one[31] = 1;
    let half_plus_one = sub_be(&order, &half);
    assert_eq!(
        hex::encode(half_plus_one),
        "7fffffff800000007fffffffffffffffde737d56d38bcf4279dce5617e3192a9"
    );
    assert!(KagemushaDeviceSignatureV1::from_raw_bytes(&raw(&one, &half)).is_ok());
    for rejected in [
        raw(&one, &half_plus_one),
        raw(&one, &sub_be(&order, &one)),
        raw(&[0; 32], &one),
        raw(&one, &[0; 32]),
        raw(&order, &one),
        raw(&one, &order),
    ] {
        assert!(KagemushaDeviceSignatureV1::from_raw_bytes(&rejected).is_err());
    }
    // A high scalar is normalized to its low twin before verification; an arbitrary pair
    // still fails verification rather than being accepted.
    let (_, public) = key(7);
    assert!(matches!(
        kagemusha_wallet_freeze_signature_v1(
            &public,
            KagemushaWalletDigestRoleV1::Proof,
            b"",
            KagemushaWalletSignerOutputV1::Raw(raw(&one, &half_plus_one))
        ),
        Err(KagemushaWalletValidationErrorV1::InvalidSignature {
            role: KagemushaWalletDigestRoleV1::Proof
        })
    ));
}
