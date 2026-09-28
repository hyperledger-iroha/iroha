//! Borrowed-key geometry, finite-point rejection, and scoped primitive comparisons.

use super::*;
use crate::sm::{Sm2PrivateKey, Sm2Signature, encode_sm2_public_key_payload};
use signature::Verifier;

fn envelope(distid: &str) -> (Sm2PrivateKey, Vec<u8>) {
    let private = Sm2PrivateKey::from_seed(distid, b"borrowed SM2 key").unwrap();
    let public = private.public_key();
    let bytes = encode_sm2_public_key_payload(distid, &public.to_sec1_bytes(false)).unwrap();
    (private, bytes)
}

#[test]
fn borrowed_identity_and_payload_retain_original_bytes_at_every_boundary() {
    let maximum = "x".repeat(8191);
    for distid in ["", "署名者-é", "署名者-e\u{301}", maximum.as_str()] {
        let (private, bytes) = envelope(distid);
        let key = BorrowedKey::parse(&bytes).unwrap();
        assert!(std::ptr::eq(key.payload(), bytes.as_slice()));
        let (identity, point_bytes) = split_payload(&bytes).unwrap();
        assert_eq!(identity, distid);
        assert_eq!(identity.as_ptr(), bytes[2..].as_ptr());
        assert_eq!(
            identity_bits(identity).unwrap(),
            u16::try_from(distid.len() * 8).unwrap()
        );
        assert_eq!(point_bytes.len(), 65);
        assert_eq!(
            key.identity_hash,
            private.public_key().compute_z(distid).unwrap()
        );
        let signature = private.sign(b"identity boundary");
        key.verify(b"identity boundary", &signature.as_bytes())
            .unwrap();
    }
}

#[test]
fn fixed_parser_preserves_exact_rejection_priority_and_diagnostics() {
    let (_, good) = envelope("id");
    let mut bad_utf8 = good.clone();
    bad_utf8[2] = 0xff;
    let mut bad_point = good.clone();
    bad_point[4..].fill(0);
    let mut zero_coordinates = bad_point.clone();
    zero_coordinates[4] = 4;
    let mut malformed = good.clone();
    malformed[4] = 0x06;
    let mut oversized = vec![0x20, 0x00];
    oversized.extend_from_slice(&[b'x'; 8192]);
    for (bytes, reason, expected) in [
        (
            &[][..],
            KeyRejection::MissingPrefix,
            "SM2 payload missing distid length prefix",
        ),
        (
            &[0][..],
            KeyRejection::MissingPrefix,
            "SM2 payload missing distid length prefix",
        ),
        (
            &[0, 2, b'a'][..],
            KeyRejection::TruncatedIdentity,
            "SM2 payload truncated distid",
        ),
        (
            bad_utf8.as_slice(),
            KeyRejection::IdentityUtf8,
            "SM2 distid must be valid UTF-8",
        ),
        (
            oversized.as_slice(),
            KeyRejection::IdentityTooLong,
            "SM2 distinguishing identifier exceeds 65535 bits",
        ),
        (
            &good[..good.len() - 1],
            KeyRejection::PublicKeyLength,
            "SM2 public key payload must be 65 bytes",
        ),
        (
            bad_point.as_slice(),
            KeyRejection::ZeroKey,
            "invalid SM2 public key: all-zero SEC1 payload",
        ),
        (
            zero_coordinates.as_slice(),
            KeyRejection::ZeroCoordinates,
            "invalid SM2 public key: all-zero SEC1 coordinate payload",
        ),
        (
            malformed.as_slice(),
            KeyRejection::InvalidPoint,
            "invalid SM2 public key",
        ),
    ] {
        let actual = BorrowedKey::parse(bytes).err().unwrap();
        assert_eq!(actual, reason);
        assert_eq!(actual.into_parse_error().to_string(), expected);
        assert_eq!(
            crate::sm::decode_sm2_public_key_payload(bytes)
                .unwrap_err()
                .to_string(),
            expected
        );
    }
}

#[test]
fn borrowed_owned_and_pinned_verifiers_agree_on_signatures_and_mutations() {
    for seed in 1_u8..=8 {
        let distid = format!("署名者-{seed}");
        let private = Sm2PrivateKey::from_seed(&distid, &[seed; 32]).unwrap();
        let owned = private.public_key();
        let sec1 = owned.to_sec1_bytes(false);
        let payload = encode_sm2_public_key_payload(&distid, &sec1).unwrap();
        let borrowed = BorrowedKey::parse(&payload).unwrap();
        let pinned = sm2::dsa::VerifyingKey::from_sec1_bytes(&distid, &sec1).unwrap();
        let message = [seed; 19];
        let signature = private.sign(&message).as_bytes();
        borrowed.verify(&message, &signature).unwrap();
        let mut mutations = [signature; 5];
        mutations[0][31] ^= 1;
        mutations[1][..32].fill(0);
        mutations[2][32..].fill(0);
        mutations[3][..32].fill(0xff);
        mutations[4][32..].fill(0xff);
        for bytes in std::iter::once(&signature).chain(mutations.iter()) {
            for candidate in [&message[..], b"changed message".as_slice()] {
                let expected = Signature::from_slice(bytes)
                    .and_then(|signature| pinned.verify(candidate, &signature))
                    .is_ok();
                assert_eq!(borrowed.verify(candidate, bytes).is_ok(), expected);
                let owned_result = Sm2Signature::from_bytes(bytes)
                    .ok()
                    .is_some_and(|signature| owned.verify(candidate, &signature).is_ok());
                assert_eq!(owned_result, expected);
            }
        }
        assert!(matches!(
            borrowed.verify(&message, &signature[..63]),
            Err(Error::BadSignature)
        ));
        let wrong_payload = encode_sm2_public_key_payload("different identity", &sec1).unwrap();
        assert!(
            BorrowedKey::parse(&wrong_payload)
                .unwrap()
                .verify(&message, &signature)
                .is_err()
        );
    }
}

#[test]
fn nonzero_signature_scalars_with_zero_sum_are_rejected() {
    let (_, payload) = envelope("scalar boundary");
    let key = BorrowedKey::parse(&payload).unwrap();
    let signature = Signature::from_scalars(
        Scalar::from(1_u64).to_bytes(),
        (-Scalar::from(1_u64)).to_bytes(),
    )
    .unwrap();
    assert!(matches!(
        key.verify(b"zero t", &signature.to_bytes()),
        Err(Error::BadSignature)
    ));
}

#[test]
fn infinite_combined_point_is_rejected_by_every_public_verification_path() {
    let distid = "finite affine point required";
    let d = Scalar::from(2_u64);
    let private = Sm2PrivateKey::from_bytes(distid, &d.to_bytes()).unwrap();
    let owned = private.public_key();
    let payload = encode_sm2_public_key_payload(distid, &owned.to_sec1_bytes(false)).unwrap();
    let borrowed = BorrowedKey::parse(&payload).unwrap();
    let message = b"SM2 verification must reject the identity point";
    let digest: [u8; 32] = Sm3::new_with_prefix(borrowed.identity_hash)
        .chain_update(message)
        .finalize()
        .into();
    let r = Scalar::reduce_bytes(&digest.into());
    let s = -r * d * (Scalar::from(1_u64) + d).invert().unwrap();
    let signature = Signature::from_scalars(r.to_bytes(), s.to_bytes()).unwrap();
    let (parsed_r, parsed_s) = signature.split_scalars();
    let t = *parsed_r + *parsed_s;
    assert!(!bool::from(parsed_r.is_zero()));
    assert!(!bool::from(parsed_s.is_zero()));
    assert!(!bool::from(t.is_zero()));
    let combined = ProjectivePoint::lincomb(
        &ProjectivePoint::generator(),
        &parsed_s,
        &ProjectivePoint::from(borrowed.point),
        &t,
    );
    assert!(bool::from(combined.is_identity()));
    assert_eq!(
        Scalar::reduce_bytes(&combined.to_affine().x()),
        Scalar::from(0_u64)
    );
    // Without the finite-point check, the x=0 sentinel makes the final equation
    // accept r=e despite B6 having no valid affine coordinates.
    assert_eq!(*parsed_r, r);
    let bytes = signature.to_bytes();
    let sm2_signature = Sm2Signature::from_bytes(&bytes).unwrap();
    let compact = crate::PublicKey::from_bytes(crate::Algorithm::Sm2, &payload).unwrap();
    let ordinary = crate::Signature::from_bytes(&bytes);
    for result in [
        borrowed.verify(message, &bytes),
        owned.verify(message, &sm2_signature),
        ordinary.verify(&compact, message),
        crate::verify_signature_for_admission(&ordinary, &compact, message),
    ] {
        assert!(matches!(result, Err(Error::BadSignature)));
    }
}
