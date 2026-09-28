//! Canonical SM2 signatures and rejection of ECDSA on the same curve.

use super::{Sm2PrivateKey, Sm2PublicKey, Sm2Signature};
use crate::Error;
use norito::json::Value;

fn vectors() -> Vec<Value> {
    let fixture: Value =
        norito::json::from_slice(include_bytes!("../../../fixtures/sm/sm2_fixture.json"))
            .expect("shared canonical SM2 fixture");
    let rows = fixture.get("vectors").unwrap().as_array().unwrap();
    let identities: Vec<_> = rows.iter().map(|row| text(row, "case_id")).collect();
    assert_eq!(
        identities,
        [
            "sm2-fixture-default-v1",
            "sm2-rust-sdk-fixture-v1",
            "gm-t-0003-annex-d-example1",
        ],
        "every shared fixture row must have an explicit curve-domain control"
    );
    rows.clone()
}

fn canonical_vectors() -> Vec<Value> {
    let canonical: Vec<_> = vectors()
        .into_iter()
        .filter(|row| match text(row, "curve") {
            "sm2p256v1" => true,
            "gm-t-0003-annex-d-fp256" => false,
            curve => panic!("unrecognized SM2 fixture curve: {curve}"),
        })
        .collect();
    assert_eq!(canonical.len(), 2, "both production-curve vectors must run");
    canonical
}
fn text<'a>(row: &'a Value, field: &str) -> &'a str {
    row.get(field).unwrap().as_str().unwrap()
}
fn bytes(row: &Value, field: &str) -> Vec<u8> {
    hex::decode(text(row, field)).unwrap()
}
fn public_key(row: &Value) -> Sm2PublicKey {
    Sm2PublicKey::from_sec1_bytes(text(row, "distid"), &bytes(row, "public_key_sec1_hex")).unwrap()
}

#[test]
fn annex_d_curve_is_rejected_by_the_production_sm2_domain() {
    let rows = vectors();
    let alternate: Vec<_> = rows
        .iter()
        .filter(|row| text(row, "curve") == "gm-t-0003-annex-d-fp256")
        .collect();
    assert_eq!(alternate.len(), 1, "the Annex D rejection control must run");
    let row = alternate[0];
    assert_eq!(text(row, "case_id"), "gm-t-0003-annex-d-example1");
    assert!(
        Sm2PublicKey::from_sec1_bytes(text(row, "distid"), &bytes(row, "public_key_sec1_hex"))
            .is_err(),
        "the Annex D Fp256 curve is distinct from the production sm2p256v1 curve"
    );
}

#[test]
fn shared_sm2_vectors_keep_exact_verification_and_identity_hash() {
    for row in canonical_vectors() {
        let key = public_key(&row);
        let signature = Sm2Signature::from_hex(text(&row, "signature")).unwrap();
        let message = bytes(&row, "message_hex");
        key.verify(&message, &signature).unwrap();
        assert_eq!(
            key.compute_z(text(&row, "distid")).unwrap().as_slice(),
            bytes(&row, "za")
        );
        let private =
            Sm2PrivateKey::from_bytes(text(&row, "distid"), &bytes(&row, "private_key_hex"))
                .unwrap();
        assert_eq!(
            private.sign(&message),
            signature,
            "canonical deterministic signing must retain the fixture bytes"
        );
    }
}

#[test]
fn canonical_sm2_rejects_message_identity_key_and_signature_mutations() {
    for row in canonical_vectors() {
        let key = public_key(&row);
        let signature = Sm2Signature::from_hex(text(&row, "signature")).unwrap();
        let message = bytes(&row, "message_hex");
        let mut changed_message = message.clone();
        changed_message.push(0x51);
        assert!(matches!(
            key.verify(&changed_message, &signature),
            Err(Error::BadSignature)
        ));
        let wrong_id = Sm2PublicKey::from_sec1_bytes(
            "wrong-distinguishing-id",
            &bytes(&row, "public_key_sec1_hex"),
        )
        .unwrap();
        assert!(matches!(
            wrong_id.verify(&message, &signature),
            Err(Error::BadSignature)
        ));
        let other_key =
            Sm2PrivateKey::from_seed(text(&row, "distid"), b"distinct SM2 key negative control")
                .unwrap()
                .public_key();
        assert_ne!(other_key.to_sec1_bytes(false), key.to_sec1_bytes(false));
        assert!(matches!(
            other_key.verify(&message, &signature),
            Err(Error::BadSignature)
        ));
        let mut altered = bytes(&row, "signature");
        altered[31] ^= 1;
        let altered = Sm2Signature::from_bytes(altered.as_slice().try_into().unwrap()).unwrap();
        assert!(matches!(
            key.verify(&message, &altered),
            Err(Error::BadSignature)
        ));
    }
}

#[cfg(feature = "sm-ffi-openssl")]
struct PreviewGuard {
    previous: bool,
    _lock: std::sync::MutexGuard<'static, ()>,
}
#[cfg(feature = "sm-ffi-openssl")]
impl PreviewGuard {
    fn acquire() -> Self {
        let lock = super::test_support::lock_accel_state();
        Self {
            previous: super::OpenSslProvider::is_enabled(),
            _lock: lock,
        }
    }
}
#[cfg(feature = "sm-ffi-openssl")]
impl Drop for PreviewGuard {
    fn drop(&mut self) {
        super::OpenSslProvider::set_preview_enabled(self.previous);
    }
}

#[cfg(feature = "sm-ffi-openssl")]
#[test]
fn openssl_preview_does_not_change_canonical_sm2_decisions() {
    let _guard = PreviewGuard::acquire();
    for enabled in [false, true] {
        super::OpenSslProvider::set_preview_enabled(enabled);
        shared_sm2_vectors_keep_exact_verification_and_identity_hash();
        canonical_sm2_rejects_message_identity_key_and_signature_mutations();
    }
}

#[cfg(feature = "sm-ffi-openssl")]
#[test]
fn ecdsa_signature_on_sm2_curve_is_rejected_as_sm2_with_preview_on_or_off() {
    use openssl::{
        bn::{BigNum, BigNumContext},
        ec::{EcGroup, EcKey, EcPoint},
        ecdsa::EcdsaSig,
        nid::Nid,
    };
    use sm3::{Digest as _, Sm3};

    let _guard = PreviewGuard::acquire();
    let rows = canonical_vectors();
    let row = &rows[0];
    let public = public_key(row);
    let message = bytes(row, "message_hex");
    let group = EcGroup::from_curve_name(Nid::SM2)
        .expect("feature qualification requires the SM2 group for the negative ECDSA control");
    let mut context = BigNumContext::new().unwrap();
    let point =
        EcPoint::from_bytes(&group, &bytes(row, "public_key_sec1_hex"), &mut context).unwrap();
    let private = BigNum::from_slice(&bytes(row, "private_key_hex")).unwrap();
    let key = EcKey::from_private_components(&group, &private, &point).unwrap();
    key.check_key().unwrap();
    let digest = Sm3::new_with_prefix(public.compute_z(text(row, "distid")).unwrap())
        .chain_update(&message)
        .finalize();
    let ecdsa = EcdsaSig::sign(&digest, &key).unwrap();
    assert!(
        ecdsa.verify(&digest, &key).unwrap(),
        "negative control must be valid ECDSA over ZA and message"
    );
    let signature = Sm2Signature::from_der(&ecdsa.to_der().unwrap()).unwrap();
    for enabled in [false, true] {
        super::OpenSslProvider::set_preview_enabled(enabled);
        assert!(
            matches!(
                public.verify(&message, &signature),
                Err(Error::BadSignature)
            ),
            "ECDSA and SM2 have different signature equations"
        );
    }
}
