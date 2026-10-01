//! Synthetic codec/signature cases; they create no platform or native authority.
use super::super::{
    KagemushaAppAttestReleaseMeasurementV1 as Measurement, KagemushaHardwarePlatformClassV1,
    app_attest_release_extensions_digest,
};
use super::*;
use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};

fn field(major: u8, bytes: &[u8]) -> Vec<u8> {
    let mut v = if bytes.len() < 24 {
        vec![(major << 5) | bytes.len() as u8]
    } else {
        vec![(major << 5) | 24, bytes.len() as u8]
    };
    v.extend_from_slice(bytes);
    v
}
fn extensions(category: u32, version: &str, category_first: bool) -> Vec<u8> {
    let mut cat = field(3, b"validationCategory");
    cat.extend(field(2, &category.to_le_bytes()));
    let mut ver = field(3, b"bundleVersion");
    ver.extend(field(3, version.as_bytes()));
    let mut v = vec![0xa2];
    if category_first {
        v.extend(cat);
        v.extend(ver);
    } else {
        v.extend(ver);
        v.extend(cat);
    }
    v
}
fn original(auth: &[u8], der: &[u8], auth_first: bool) -> Vec<u8> {
    let mut a = field(3, b"authenticatorData");
    a.extend(field(2, auth));
    let mut s = field(3, b"signature");
    s.extend(field(2, der));
    let mut v = vec![0xa2];
    if auth_first {
        v.extend(a);
        v.extend(s);
    } else {
        v.extend(s);
        v.extend(a);
    }
    v
}
fn fixture(
    version: Option<&str>,
    category_first: bool,
    auth_first: bool,
) -> (Vec<u8>, KagemushaDevicePublicKeyV1, [u8; 32]) {
    let key = SigningKey::from_bytes((&[17; 32]).into()).unwrap();
    let point = key.verifying_key().to_encoded_point(false);
    let public = KagemushaDevicePublicKeyV1::from_sec1_bytes(point.as_bytes()).unwrap();
    let mut auth = vec![12; 32];
    auth.push(if version.is_some() { 0xc0 } else { 0x40 });
    auth.extend(7_u32.to_be_bytes());
    let release = version.map_or([13; 32], |v| {
        app_attest_release_extensions_digest(2, v).unwrap()
    });
    if let Some(v) = version {
        auth.extend(extensions(2, v, category_first));
    }
    let mut nonce = Sha256::new();
    nonce.update(&auth);
    nonce.update(Sha256::digest(b"held native message"));
    let signature: Signature = key.sign(&nonce.finalize());
    (
        original(&auth, signature.to_der().as_bytes(), auth_first),
        public,
        release,
    )
}
fn authenticate(
    raw: Vec<u8>,
    key: &KagemushaDevicePublicKeyV1,
    release: [u8; 32],
    rp: [u8; 32],
    floor: Option<u32>,
    message: &[u8],
) -> Result<(Option<u32>, Option<Measurement>), String> {
    KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion: raw }
        .authenticate_signature(
            KagemushaHardwarePlatformClassV1::AppleAppAttest,
            key,
            rp,
            release,
            floor,
            message,
        )
}
#[test]
fn measured_originals_preserve_both_orders_and_exact_maximum_utf8_version() {
    let maximum = "é".repeat(64);
    for version in [
        "1".to_owned(),
        "a".repeat(23),
        "a".repeat(24),
        "a".repeat(128),
        maximum,
    ] {
        for category_first in [false, true] {
            for auth_first in [false, true] {
                let (raw, key, release) = fixture(Some(&version), category_first, auth_first);
                let parts = kagemusha_ordinary_apple_original_parts_v1(&raw, release).unwrap();
                assert_eq!(parts.bundle_version, Some(version.as_str()));
                assert_eq!(parts.validation_category, Some(2));
                assert_eq!(parts.release_measurement, Measurement::SignedExtensions);
                assert_eq!(
                    parts.authenticator_data.len(),
                    37 + extensions(2, &version, category_first).len()
                );
                assert!(raw.len() <= 311);
                assert_eq!(kagemusha_app_attest_original_counter_v1(&raw).unwrap(), 7);
                let before = raw.clone();
                assert_eq!(
                    authenticate(
                        raw.clone(),
                        &key,
                        release,
                        [12; 32],
                        Some(6),
                        b"held native message"
                    )
                    .unwrap(),
                    (Some(7), Some(Measurement::SignedExtensions))
                );
                assert_eq!(raw, before);
            }
        }
    }
}
#[test]
fn limited_original_has_explicit_unavailable_release_and_does_not_admit_wrong_equations() {
    let (raw, key, release) = fixture(None, true, true);
    let pad_parts = kagemusha_ordinary_apple_original_parts_v1(&raw, [0; 32]).unwrap();
    assert_eq!(pad_parts.release_measurement, Measurement::Unavailable);
    assert_eq!(pad_parts.validation_category, None);
    assert_eq!(pad_parts.bundle_version, None);
    // Parsing the unmeasured original for inactive padding grants no signature authority.
    assert!(
        authenticate(
            raw.clone(),
            &key,
            [0; 32],
            [12; 32],
            Some(6),
            b"held native message"
        )
        .is_err()
    );
    assert_eq!(
        authenticate(
            raw.clone(),
            &key,
            release,
            [12; 32],
            Some(6),
            b"held native message"
        )
        .unwrap(),
        (Some(7), Some(Measurement::Unavailable))
    );
    for (rp, floor, message) in [
        ([11; 32], Some(6), b"held native message".as_slice()),
        ([12; 32], Some(7), b"held native message".as_slice()),
        ([12; 32], None, b"held native message".as_slice()),
        ([12; 32], Some(6), b"other message".as_slice()),
    ] {
        assert!(authenticate(raw.clone(), &key, release, rp, floor, message).is_err());
    }
    let foreign = SigningKey::from_bytes((&[18; 32]).into()).unwrap();
    let key = KagemushaDevicePublicKeyV1::from_sec1_bytes(
        foreign.verifying_key().to_encoded_point(false).as_bytes(),
    )
    .unwrap();
    assert!(
        authenticate(
            raw,
            &key,
            release,
            [12; 32],
            Some(6),
            b"held native message"
        )
        .is_err()
    );
}
#[test]
fn measured_original_refuses_release_substitution_flags_nonminimal_and_duplicate_keys() {
    let (raw, key, release) = fixture(Some(&"a".repeat(128)), true, true);
    assert!(kagemusha_ordinary_apple_original_parts_v1(&raw, [0; 32]).is_err());
    assert!(
        authenticate(
            raw.clone(),
            &key,
            [13; 32],
            [12; 32],
            Some(6),
            b"held native message"
        )
        .is_err()
    );
    let (auth, der) = super::super::kagemusha_v1::parse_app_attest_assertion(&raw).unwrap();
    assert_eq!(auth.len(), 206);
    let mut wrong_flags = auth.to_vec();
    wrong_flags[32] = 0x80;
    assert!(
        kagemusha_ordinary_apple_original_parts_v1(&original(&wrong_flags, der, true), release)
            .is_err()
    );
    let mut nonminimal = original(auth, &[0x30, 6, 2, 1, 1, 2, 1, 1], true);
    assert!(nonminimal.len() < 311);
    let length_offset = 1 + 18;
    assert_eq!(&nonminimal[length_offset..length_offset + 2], &[0x58, 206]);
    nonminimal.splice(length_offset..length_offset + 2, [0x59, 0, 206]);
    assert!(kagemusha_ordinary_apple_original_parts_v1(&nonminimal, release).is_err());
    let mut trailing = raw.clone();
    trailing.push(0);
    assert!(kagemusha_ordinary_apple_original_parts_v1(&trailing, release).is_err());
    let mut duplicate_auth = auth[..37].to_vec();
    duplicate_auth.push(0xa2);
    for _ in 0..2 {
        duplicate_auth.extend(field(3, b"validationCategory"));
        duplicate_auth.extend(field(2, &2u32.to_le_bytes()));
    }
    assert!(
        kagemusha_ordinary_apple_original_parts_v1(&original(&duplicate_auth, der, true), release)
            .is_err()
    );
    for malformed in ["\0".to_owned(), "a".repeat(129)] {
        let mut auth = auth[..37].to_vec();
        auth.extend(extensions(2, &malformed, true));
        assert!(
            kagemusha_ordinary_apple_original_parts_v1(&original(&auth, der, true), release)
                .is_err()
        );
    }
    let mut altered = auth.to_vec();
    altered[36] ^= 1;
    assert!(
        authenticate(
            original(&altered, der, true),
            &key,
            release,
            [12; 32],
            Some(6),
            b"held native message"
        )
        .is_err()
    );
}
