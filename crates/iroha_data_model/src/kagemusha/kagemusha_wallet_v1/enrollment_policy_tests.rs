//! Unadmitted DATA only: public repeated pins are placeholders, without root DER or approval.

use super::*;
use crate::kagemusha::kagemusha_wallet_v1::codec_tests::{
    assert_every_flip_rejected_or_rebound, norito_tag,
};

/// Shared deterministic policy data, not external authority or current policy installation.
pub(in crate::kagemusha::kagemusha_wallet_v1) fn policy_fixture(
    apple: bool,
) -> (
    KagemushaWalletAppPolicyV1,
    KagemushaWalletEnrollmentPolicyV1,
) {
    let app = KagemushaWalletAppPolicyV1 {
        version: 1,
        scheme_id: [1; 32],
        identity: if apple {
            KagemushaWalletAppIdentityV1::Apple {
                app_id: "TEAMID.org.example.wallet".into(),
            }
        } else {
            KagemushaWalletAppIdentityV1::Android {
                package_name: "org.example.wallet".into(),
                package_version: 7,
                app_signing_certificate_sha256: [5; 32],
            }
        },
    };
    let enrollment = KagemushaWalletEnrollmentPolicyV1 {
        version: 1,
        scheme_id: [1; 32],
        asset_digest: [2; 32],
        app_policy: app.policy_digest().unwrap(),
        platform: if apple {
            KagemushaWalletEnrollmentPlatformV1::Apple {
                attestation_root_sha256: [4; 32],
            }
        } else {
            KagemushaWalletEnrollmentPlatformV1::Android {
                attestation_root_sha256: [4; 32],
                hardware: KagemushaWalletAndroidHardwareV1::TeeOrStrongBox,
                patch_floor_yyyymm: 202608,
                play_integrity_maximum_age_ms: 120000,
                require_play_recognized: true,
                require_licensed: true,
                minimum_device_integrity: KagemushaWalletPlayIntegrityLevelV1::Device,
            }
        },
        regulatory_policy: KagemushaWalletRegulatoryPolicyV1 {
            permitted_controls: if apple { 4 } else { 0 },
            blacklist_max_age_ms: 0,
            time_anchor_max_response_ms: if apple { 5000 } else { 0 },
        },
        challenge_lifetime_ms: 120000,
        attestation_lease_lifetime_ms: if apple { 3600000 } else { 0 },
    };
    (app, enrollment)
}

fn challenge(
    app: &KagemushaWalletAppPolicyV1,
    enrollment: &KagemushaWalletEnrollmentPolicyV1,
) -> KagemushaWalletEnrollmentChallengeV1 {
    KagemushaWalletEnrollmentChallengeV1 {
        version: 1,
        scheme_id: [1; 32],
        asset_digest: [2; 32],
        account_digest: [3; 32],
        app_policy: app.policy_digest().unwrap(),
        enrollment_policy: enrollment.policy_digest().unwrap(),
        issuer_nonce: [6; 32],
    }
}

#[test]
fn new_enrollment_policy_enum_tags_match_current_wallet_canonical_convention() {
    let (android_app, android) = policy_fixture(false);
    let (apple_app, apple) = policy_fixture(true);
    assert_eq!(norito_tag(&android_app.identity), 1);
    assert_eq!(norito_tag(&apple_app.identity), 2);
    assert_eq!(norito_tag(&android.platform), 1);
    assert_eq!(norito_tag(&apple.platform), 2);
    for (value, tag) in [
        (KagemushaWalletAndroidHardwareV1::Tee, 1),
        (KagemushaWalletAndroidHardwareV1::StrongBox, 2),
        (KagemushaWalletAndroidHardwareV1::TeeOrStrongBox, 3),
    ] {
        assert_eq!(norito_tag(&value), tag);
    }
    assert_eq!(norito_tag(&KagemushaWalletPlayIntegrityLevelV1::Device), 1);
    assert_eq!(norito_tag(&KagemushaWalletPlayIntegrityLevelV1::Strong), 2);
}

#[test]
fn new_enrollment_policy_shared_transcripts_and_digest_vectors() {
    use norito::json::Value;
    let document: Value = norito::json::parse_value(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/kagemusha/wallet_enrollment_policy_v1_vectors.json"
    )))
    .expect("DATA vectors");
    let Value::Object(document) = document else {
        panic!("object")
    };
    let Some(Value::Array(cases)) = document.get("cases") else {
        panic!("cases")
    };
    assert_eq!(cases.len(), 2);
    for (apple, row) in [false, true].into_iter().zip(cases) {
        let Value::Object(row) = row else {
            panic!("case")
        };
        let (app, enrollment) = policy_fixture(apple);
        let e1 = challenge(&app, &enrollment);
        enrollment.verify_challenge(&app, &e1).unwrap();
        for (key, actual) in [
            (
                "app_policy_transcript_hex",
                hex::encode(app.transcript().unwrap()),
            ),
            (
                "app_policy_digest_hex",
                hex::encode(app.policy_digest().unwrap()),
            ),
            (
                "enrollment_policy_transcript_hex",
                hex::encode(enrollment.transcript().unwrap()),
            ),
            (
                "enrollment_policy_digest_hex",
                hex::encode(enrollment.policy_digest().unwrap()),
            ),
            ("challenge_transcript_hex", hex::encode(e1.transcript())),
            ("challenge_digest_hex", hex::encode(e1.challenge_digest())),
        ] {
            let Some(Value::String(expected)) = row.get(key) else {
                panic!("{key}")
            };
            assert_eq!(&actual, expected, "{key}");
        }
        assert_eq!(
            enrollment.transcript().unwrap().len(),
            if apple { 167 } else { 183 }
        );
    }
}

#[test]
fn new_enrollment_policy_canonical_frames_and_every_byte_substitution() {
    for apple in [false, true] {
        let (app, enrollment) = policy_fixture(apple);
        let app_frame = app.encode_canonical().unwrap();
        let enrollment_frame = enrollment.encode_canonical().unwrap();
        assert_eq!(
            KagemushaWalletAppPolicyV1::decode_canonical(&app_frame, &[1; 32]).unwrap(),
            app
        );
        assert_eq!(
            KagemushaWalletEnrollmentPolicyV1::decode_canonical(&enrollment_frame, &[1; 32])
                .unwrap(),
            enrollment
        );
        assert_every_flip_rejected_or_rebound(&app_frame, app.policy_digest().unwrap(), |bytes| {
            KagemushaWalletAppPolicyV1::decode_canonical(bytes, &[1; 32])
                .ok()
                .and_then(|p| p.policy_digest().ok())
        });
        assert_every_flip_rejected_or_rebound(
            &enrollment_frame,
            enrollment.policy_digest().unwrap(),
            |bytes| {
                KagemushaWalletEnrollmentPolicyV1::decode_canonical(bytes, &[1; 32])
                    .ok()
                    .and_then(|p| p.policy_digest().ok())
            },
        );
        let mut trailing = app_frame.clone();
        trailing.push(0);
        assert!(KagemushaWalletAppPolicyV1::decode_canonical(&trailing, &[1; 32]).is_err());
        assert!(
            KagemushaWalletEnrollmentPolicyV1::decode_canonical(&enrollment_frame, &[2; 32])
                .is_err()
        );
        assert!(KagemushaWalletAppPolicyV1::decode_canonical(&vec![0; 1025], &[1; 32]).is_err());
        assert!(
            KagemushaWalletEnrollmentPolicyV1::decode_canonical(&vec![0; 1025], &[1; 32]).is_err()
        );
    }
}

#[test]
fn new_enrollment_policy_rejects_cross_scope_app_platform_and_policy_substitution() {
    let (app, enrollment) = policy_fixture(false);
    let e1 = challenge(&app, &enrollment);
    for index in 0..4 {
        let mut changed = e1;
        match index {
            0 => changed.scheme_id = [7; 32],
            1 => changed.asset_digest = [7; 32],
            2 => changed.app_policy = [7; 32],
            _ => changed.enrollment_policy = [7; 32],
        }
        assert!(enrollment.verify_challenge(&app, &changed).is_err());
    }
    let (apple, _) = policy_fixture(true);
    assert!(enrollment.validate_for_app(&apple).is_err());
    let mut other = enrollment;
    other.platform = KagemushaWalletEnrollmentPlatformV1::Apple {
        attestation_root_sha256: [4; 32],
    };
    assert!(other.validate_for_app(&app).is_err());
    let mut edited_app = app.clone();
    let KagemushaWalletAppIdentityV1::Android {
        package_version, ..
    } = &mut edited_app.identity
    else {
        unreachable!()
    };
    *package_version += 1;
    assert!(enrollment.validate_for_app(&edited_app).is_err());
    for name in ["", "one", "a..b", "1a.b", "org.exa-mple", "org.é"] {
        let mut changed = app.clone();
        let KagemushaWalletAppIdentityV1::Android { package_name, .. } = &mut changed.identity
        else {
            unreachable!()
        };
        *package_name = name.into();
        assert!(changed.validate().is_err(), "{name}");
    }
    let (mut apple, _) = policy_fixture(true);
    apple.identity = KagemushaWalletAppIdentityV1::Apple {
        app_id: "é".repeat(128),
    };
    assert!(apple.validate().is_err());
    apple.identity = KagemushaWalletAppIdentityV1::Apple {
        app_id: "é".repeat(127) + "a",
    };
    assert!(apple.validate().is_ok());
}

#[test]
fn new_enrollment_policy_challenge_and_lease_boundaries_without_defaults() {
    let (_, mut android) = policy_fixture(false);
    android.require_live_challenge(10, 10).unwrap();
    android.require_live_challenge(10, 120009).unwrap();
    for (created, now) in [(0, 1), (10, 9), (10, 120010), (u64::MAX - 1, u64::MAX)] {
        assert!(android.require_live_challenge(created, now).is_err());
    }
    assert_eq!(android.lease_expires_at(10).unwrap(), 0);
    assert!(android.lease_expires_at(0).is_err());
    let (_, mut apple) = policy_fixture(true);
    assert_eq!(apple.lease_expires_at(10).unwrap(), 3600010);
    assert!(apple.lease_expires_at(u64::MAX - 1).is_err());
    apple.attestation_lease_lifetime_ms = 0;
    assert!(apple.validate().is_err());
    android.attestation_lease_lifetime_ms = 1;
    assert!(android.validate().is_err());
    android.attestation_lease_lifetime_ms = 0;
    android.challenge_lifetime_ms = 119999;
    assert!(android.validate().is_err());
    android.challenge_lifetime_ms = 0;
    assert!(android.validate().is_err());
}

#[test]
fn new_app_policy_maximum_utf8_inputs_fit_the_complete_native_frame_cap() {
    for (identity, transcript_length) in [
        (
            KagemushaWalletAppIdentityV1::Android {
                package_name: "a.".to_owned() + &"b".repeat(253),
                package_version: u64::MAX,
                app_signing_certificate_sha256: [5; 32],
            },
            334,
        ),
        (
            KagemushaWalletAppIdentityV1::Apple {
                app_id: "é".repeat(127) + "a",
            },
            294,
        ),
    ] {
        let app = KagemushaWalletAppPolicyV1 {
            version: 1,
            scheme_id: [1; 32],
            identity,
        };
        assert_eq!(app.transcript().unwrap().len(), transcript_length);
        let frame = app.encode_canonical().unwrap();
        assert!(frame.len() <= KAGEMUSHA_WALLET_ENROLLMENT_POLICY_MAX_BYTES_V1);
        assert_eq!(
            KagemushaWalletAppPolicyV1::decode_canonical(&frame, &[1; 32]).unwrap(),
            app
        );
        let mut oversized = app.clone();
        match &mut oversized.identity {
            KagemushaWalletAppIdentityV1::Android { package_name, .. } => {
                package_name.push('b');
            }
            KagemushaWalletAppIdentityV1::Apple { app_id } => app_id.push('a'),
        }
        assert!(oversized.validate().is_err());
        assert!(oversized.encode_canonical().is_err());
    }
}
