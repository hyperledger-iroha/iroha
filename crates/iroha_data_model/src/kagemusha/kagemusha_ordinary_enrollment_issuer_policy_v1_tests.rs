//! Actual Ed25519 threshold mathematics on explicitly synthetic DER/body originals; no
//! PKIX/CMS/provider status response/device/native installation or financial success is claimed.
use super::*;
use crate::{id::NetworkId, kagemusha::*};
use iroha_crypto::{Hash, HashOf, KeyPair, Signature};

#[test]
fn independently_threshold_signed_core_app_key_reuse_is_rejected() {
    for apple in [false, true] {
        let (mut issuer, checked) = fixture(apple);
        issuer.enrollment_issuer_key = issuer.app_authority_key.clone();
        assert_eq!(
            issuer.canonical_bytes().unwrap_err(),
            "ordinary issuer policy original malformed"
        );
        assert_eq!(
            issuer
                .authenticate_under_policy(&checked, [21; 32], 2000)
                .err()
                .unwrap(),
            "ordinary issuer policy original malformed"
        );

        let mut signed =
            KagemushaSignedOrdinaryAppIdentityPolicyV1::decode_canonical_exact(checked.original())
                .unwrap();
        signed.policy.enrollment_issuer_key = signed.policy.app_authority_key.clone();
        // Construct an invalid DATA archive explicitly. The public validated signing
        // helpers must refuse it; actual threshold signatures do not waive purpose separation.
        let issuer_body = norito::encode_canonical(&issuer).unwrap();
        let mut issuer_hash = Sha256::new();
        issuer_hash.update(b"iroha:kagemusha:v1:ordinary-enrollment-issuer-policy\0");
        issuer_hash.update((issuer_body.len() as u64).to_le_bytes());
        issuer_hash.update(&issuer_body);
        signed.policy.enrollment_issuer_policy_digest = issuer_hash.finalize().into();
        assert_eq!(
            signed.policy.validate().unwrap_err(),
            "ordinary identity policy incomplete"
        );
        assert!(signed.policy.approval_signing_bytes().is_err());
        let body = norito::encode_canonical(&signed.policy).unwrap();
        let mut message = b"iroha:kagemusha:v1:ordinary-app-identity-policy-approval\0".to_vec();
        message.extend_from_slice(&(body.len() as u64).to_le_bytes());
        message.extend_from_slice(&body);
        signed.approvals = [81, 82]
            .into_iter()
            .map(|seed| {
                let key = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
                let signature = Signature::try_new(key.private_key(), &message).unwrap();
                signature.verify(key.public_key(), &message).unwrap();
                KagemushaOrdinaryAppIdentityPolicyApprovalV1 {
                    public_key: key.public_key().clone(),
                    signature,
                }
            })
            .collect();
        signed
            .approvals
            .sort_by(|a, b| a.public_key.cmp(&b.public_key));
        let mut roots: KagemushaOrdinaryAppIdentityAuthorityPolicyV1 =
            norito::decode_canonical_with_limits(
                checked.authority_original(),
                norito::canonical_decode_limits(checked.authority_original().len()),
            )
            .unwrap();
        let mut identity_hash = Sha256::new();
        identity_hash.update(b"iroha:kagemusha:v1:ordinary-app-identity-policy\0");
        identity_hash.update((body.len() as u64).to_le_bytes());
        identity_hash.update(&body);
        roots.expected_identity_policy_id = identity_hash.finalize().into();
        assert_eq!(
            signed.authenticate(&roots, 2000).err().unwrap(),
            "ordinary identity policy incomplete"
        );
    }
}
fn android_evaluation() -> KagemushaPlatformEvaluationOriginalsV1 {
    let payload = b"{\"entries\":{}}".to_vec();
    let h = Sha256::digest(&payload)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    KagemushaPlatformEvaluationOriginalsV1::AndroidRevocation{canonical_snapshot_original:format!("iroha.android.attestation.revocation.snapshot.v1\npayload_sha256={h}\nresponse_date_ms=1000\nlast_modified_ms=-\ncache_max_age_seconds=10\nserial_count=2\nserial=1\nserial=a\ntbs_sha256_count=1\ntbs_sha256={}\n","12".repeat(32)).into_bytes(),original_status_payload:payload}
}
fn fixture(
    apple: bool,
) -> (
    KagemushaOrdinaryEnrollmentIssuerPolicyV1,
    KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1,
) {
    let platform = if apple {
        KagemushaHardwarePlatformClassV1::AppleAppAttest
    } else {
        KagemushaHardwarePlatformClassV1::AndroidKeyMint
    };
    let raw_issuer = KeyPair::from_seed(vec![61; 32], Algorithm::Ed25519);
    let credential_issuer = KeyPair::from_seed(vec![62; 32], Algorithm::Ed25519);
    let roots = KagemushaPlatformRootOriginalsV1 {
        version: 1,
        platform_class: platform,
        platform_roots_der: if apple {
            vec![vec![0x30, 1, 1]]
        } else {
            vec![vec![0x30, 1, 1], vec![0x30, 1, 2]]
        },
        apple_receipt_roots_der: if apple {
            vec![vec![0x30, 1, 3]]
        } else {
            vec![]
        },
    };
    let risk = KagemushaAppleReceiptRiskPolicyV1 {
        version: 1,
        app_id_utf8: b"ABCDEFGHIJ.example.ordinary".to_vec(),
        maximum_creation_age_ms: 300000,
        maximum_risk_metric: 5,
    };
    let app_id = if apple {
        Sha256::digest(&risk.app_id_utf8).into()
    } else {
        [2; 32]
    };
    let evaluation = if apple {
        KagemushaPlatformEvaluationOriginalsV1::AppleReceiptRisk(risk)
    } else {
        android_evaluation()
    };
    let app_authority = KagemushaAppAttestationAuthorityPolicyV1 {
        authority_key: raw_issuer.public_key().clone(),
        platform_class: platform,
        app_signing_identity_digest: app_id,
        app_release_digest: [3; 32],
        maximum_lifetime_ms: 10000,
    };
    let trust = KagemushaOrdinaryAppTrustPolicyV1 {
        version: 1,
        app_authority_policy_digest: app_authority.canonical_digest().unwrap(),
        platform_class: platform,
        distribution: KagemushaOrdinaryAppDistributionV1::Development,
        apple_environment: if apple {
            Some(KagemushaOrdinaryAppAppleEnvironmentV1::Development)
        } else {
            None
        },
        platform_trust_roots_digest: roots.canonical_digest().unwrap(),
        platform_revocation_policy_digest: evaluation.canonical_digest().unwrap(),
        allowed_android_security_levels: if apple {
            vec![]
        } else {
            vec![KagemushaAppKeySecurityLevelV1::StrongBox]
        },
        play_integrity_policy: None,
        maximum_credential_lifetime_ms: 10000,
    };
    let profile = KagemushaOrdinaryAppIdentityProfileV1 {
        version: 1,
        identity_profile_id: [0; 32],
        platform_class: platform,
        planned_release_id: [17; 32],
        planned_hardware_profile_id: [18; 32],
        planned_suite_id: [19; 32],
        policy_epoch: 1,
        trust_policy_digest: trust.canonical_digest().unwrap(),
        app_authority_policy_digest: app_authority.canonical_digest().unwrap(),
        platform_trust_roots_digest: roots.canonical_digest().unwrap(),
        valid_from_ms: 100,
        expires_at_ms: 500000,
    }
    .seal_identity_profile_id()
    .unwrap();
    let network =
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([11; 32])));
    let issuer_policy = KagemushaOrdinaryEnrollmentIssuerPolicyV1 {
        version: 1,
        network_id: [11; 32],
        lane_namespace_id: [21; 32],
        identity_profile_id: profile.identity_profile_id,
        planned_policy_epoch: profile.policy_epoch,
        planned_hardware_epoch: 7,
        enrollment_issuer_key: credential_issuer.public_key().clone(),
        app_authority_key: raw_issuer.public_key().clone(),
        maximum_pending_lifetime_ms: 120000,
        maximum_current_state_lifetime_ms: 60000,
        maximum_credential_lifetime_ms: 10000,
    };
    let policy = KagemushaOrdinaryAppIdentityPolicyV1 {
        version: 1,
        authority_set_id: [80; 32],
        network_id: network,
        profile,
        trust,
        app_authority_key: raw_issuer.public_key().clone(),
        app_authority_platform_class: platform,
        app_signing_identity_digest: app_id,
        app_release_digest: [3; 32],
        app_authority_maximum_lifetime_ms: 10000,
        enrollment_issuer_key: credential_issuer.public_key().clone(),
        enrollment_issuer_p256_key:
            crate::testing::ordinary_app_enrollment::ordinary_test_issuer_public_key_v1(),
        enrollment_issuer_policy_digest: issuer_policy.canonical_digest().unwrap(),
    };
    let m = policy.approval_signing_bytes().unwrap();
    let mut approvals: Vec<_> = [81, 82]
        .into_iter()
        .map(|seed| {
            let k = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
            KagemushaOrdinaryAppIdentityPolicyApprovalV1 {
                public_key: k.public_key().clone(),
                signature: Signature::try_new(k.private_key(), &m).unwrap(),
            }
        })
        .collect();
    approvals.sort_by(|a, b| a.public_key.cmp(&b.public_key));
    let anchors = KagemushaOrdinaryAppIdentityAuthorityPolicyV1 {
        version: 1,
        authority_set_id: [80; 32],
        network_id: network,
        expected_identity_policy_id: policy.canonical_digest().unwrap(),
        threshold: 2,
        authorized_signers: approvals.iter().map(|a| a.public_key.clone()).collect(),
    };
    let policy = KagemushaSignedOrdinaryAppIdentityPolicyV1 { policy, approvals }
        .authenticate(&anchors, 2000)
        .unwrap();
    (issuer_policy, policy)
}
fn with_committed_issuer(
    p: &KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1,
    issuer: &KagemushaOrdinaryEnrollmentIssuerPolicyV1,
) -> KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1 {
    let mut s =
        KagemushaSignedOrdinaryAppIdentityPolicyV1::decode_canonical_exact(p.original()).unwrap();
    s.policy.enrollment_issuer_policy_digest = issuer.canonical_digest().unwrap();
    let m = s.policy.approval_signing_bytes().unwrap();
    s.approvals = [81, 82]
        .into_iter()
        .map(|seed| {
            let k = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
            KagemushaOrdinaryAppIdentityPolicyApprovalV1 {
                public_key: k.public_key().clone(),
                signature: Signature::try_new(k.private_key(), &m).unwrap(),
            }
        })
        .collect();
    s.approvals.sort_by(|a, b| a.public_key.cmp(&b.public_key));
    let mut roots: KagemushaOrdinaryAppIdentityAuthorityPolicyV1 =
        norito::decode_canonical_with_limits(
            p.authority_original(),
            norito::canonical_decode_limits(p.authority_original().len()),
        )
        .unwrap();
    roots.expected_identity_policy_id = s.policy.canonical_digest().unwrap();
    s.authenticate(&roots, 2000).unwrap()
}
#[test]
fn issuer_full_original_current_threshold_join_android_and_apple() {
    for apple in [false, true] {
        let (s, p) = fixture(apple);
        let b = s.canonical_bytes().unwrap();
        let v = KagemushaOrdinaryEnrollmentIssuerPolicyV1::decode_canonical_exact(&b)
            .unwrap()
            .authenticate_under_policy(&p, [21; 32], 2000)
            .unwrap();
        assert_eq!(v.original(), b);
        assert_eq!(v.policy(), &s);
        assert!(v.recheck_current(&p, [21; 32], 3000).is_ok());
    }
}
#[test]
fn issuer_domain_digest_complete_field_mutations_reject_original_policy() {
    let (s, p) = fixture(false);
    for n in 0..10 {
        let mut t = s.clone();
        match n {
            0 => t.network_id = [22; 32],
            1 => t.lane_namespace_id = [22; 32],
            2 => t.identity_profile_id = [22; 32],
            3 => t.planned_policy_epoch = 2,
            4 => t.planned_hardware_epoch = 8,
            5 => {
                t.enrollment_issuer_key = KeyPair::from_seed(vec![90; 32], Algorithm::Ed25519)
                    .public_key()
                    .clone()
            }
            6 => {
                t.app_authority_key = KeyPair::from_seed(vec![90; 32], Algorithm::Ed25519)
                    .public_key()
                    .clone()
            }
            7 => t.maximum_pending_lifetime_ms = 110000,
            8 => t.maximum_current_state_lifetime_ms = 50000,
            _ => t.maximum_credential_lifetime_ms = 9999,
        }
        assert_ne!(t.canonical_digest().unwrap(), s.canonical_digest().unwrap());
        assert!(t.authenticate_under_policy(&p, [21; 32], 2000).is_err());
    }
}
#[test]
fn issuer_complete_keys_network_profile_epoch_joins_survive_valid_threshold_digest() {
    let (s, p) = fixture(false);
    for n in 0..7 {
        let mut t = s.clone();
        match n {
            0 => t.network_id = [22; 32],
            1 => t.identity_profile_id = [22; 32],
            2 => t.planned_policy_epoch = 2,
            3 => {
                t.enrollment_issuer_key = KeyPair::from_seed(vec![90; 32], Algorithm::Ed25519)
                    .public_key()
                    .clone()
            }
            4 => {
                t.app_authority_key = KeyPair::from_seed(vec![90; 32], Algorithm::Ed25519)
                    .public_key()
                    .clone()
            }
            5 => t.maximum_credential_lifetime_ms = 9999,
            _ => t.maximum_credential_lifetime_ms = 10001,
        };
        let changed = with_committed_issuer(&p, &t);
        assert!(
            t.authenticate_under_policy(&changed, [21; 32], 2000)
                .is_err()
        );
    }
}
#[test]
fn issuer_independent_namespace_cannot_be_selected_by_original() {
    let (s, p) = fixture(false);
    assert!(s.authenticate_under_policy(&p, [0; 32], 2000).is_err());
    assert!(s.authenticate_under_policy(&p, [22; 32], 2000).is_err());
    let v = s.authenticate_under_policy(&p, [21; 32], 2000).unwrap();
    assert!(v.recheck_current(&p, [22; 32], 2000).is_err());
}
#[test]
fn issuer_original_bounds_versions_epochs_and_lifetimes() {
    let (s, _) = fixture(false);
    for n in 0..12 {
        let mut t = s.clone();
        match n {
            0 => t.version = 2,
            1 => t.network_id = [0; 32],
            2 => t.lane_namespace_id = [0; 32],
            3 => t.identity_profile_id = [0; 32],
            4 => t.planned_policy_epoch = 0,
            5 => t.planned_hardware_epoch = 0,
            6 => t.maximum_pending_lifetime_ms = 0,
            7 => t.maximum_pending_lifetime_ms = 120001,
            8 => t.maximum_current_state_lifetime_ms = 0,
            9 => t.maximum_current_state_lifetime_ms = 120001,
            10 => t.maximum_credential_lifetime_ms = 0,
            _ => {
                t.enrollment_issuer_key = KeyPair::from_seed(vec![90; 32], Algorithm::Secp256k1)
                    .public_key()
                    .clone()
            }
        };
        assert!(t.canonical_bytes().is_err());
    }
    assert!(KagemushaOrdinaryEnrollmentIssuerPolicyV1::decode_canonical_exact(&[]).is_err());
    assert!(
        KagemushaOrdinaryEnrollmentIssuerPolicyV1::decode_canonical_exact(&vec![0; 16385]).is_err()
    );
    let mut b = s.canonical_bytes().unwrap();
    b.push(0);
    assert!(KagemushaOrdinaryEnrollmentIssuerPolicyV1::decode_canonical_exact(&b).is_err());
}
#[test]
fn issuer_current_original_expiry_time_regression_and_policy_drift() {
    let (s, p) = fixture(false);
    let v = s.authenticate_under_policy(&p, [21; 32], 2000).unwrap();
    assert!(v.recheck_current(&p, [21; 32], 1999).is_err());
    assert!(v.recheck_current(&p, [21; 32], 500000).is_err());
    assert!(s.authenticate_under_policy(&p, [21; 32], 99).is_err());
    let (mut t, _) = fixture(false);
    t.planned_hardware_epoch = 8;
    let different = with_committed_issuer(&p, &t);
    assert!(v.recheck_current(&different, [21; 32], 3000).is_err());
}
#[test]
fn issuer_signed_digest_mutation_requires_real_threshold_approval() {
    let (s, p) = fixture(false);
    let mut t =
        KagemushaSignedOrdinaryAppIdentityPolicyV1::decode_canonical_exact(p.original()).unwrap();
    t.policy.enrollment_issuer_policy_digest = [1; 32];
    let mut roots: KagemushaOrdinaryAppIdentityAuthorityPolicyV1 =
        norito::decode_canonical_with_limits(
            p.authority_original(),
            norito::canonical_decode_limits(p.authority_original().len()),
        )
        .unwrap();
    roots.expected_identity_policy_id = t.policy.canonical_digest().unwrap();
    assert!(t.authenticate(&roots, 2000).is_err());
    assert!(s.authenticate_under_policy(&p, [21; 32], 2000).is_ok());
}
#[test]
fn issuer_lane_sole_canonical_name_account_and_complete_namespace_network_preimage() {
    let (s, p) = fixture(false);
    let account = AccountId::new(
        KeyPair::from_seed(vec![91; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    let fi: Name = "cbsi".parse().unwrap();
    let lane = s.derive_enrollment_lane(&fi, &account).unwrap();
    let f = norito::encode_canonical(&fi).unwrap();
    let a = norito::encode_canonical(&account).unwrap();
    let mut h = Sha256::new();
    h.update(LANE_DOMAIN);
    h.update(s.network_id);
    h.update(s.lane_namespace_id);
    h.update((f.len() as u64).to_le_bytes());
    h.update(&f);
    h.update((a.len() as u64).to_le_bytes());
    h.update(&a);
    assert_eq!(lane, <[u8; 32]>::from(h.finalize()));
    let v = s.authenticate_under_policy(&p, [21; 32], 2000).unwrap();
    assert_eq!(
        lane,
        v.derive_enrollment_lane(&p, [21; 32], &fi, &account, 2000)
            .unwrap()
    );
    for n in 0..4 {
        let mut t = s.clone();
        let mut acct = account.clone();
        let mut otherfi = fi.clone();
        match n {
            0 => t.network_id = [22; 32],
            1 => t.lane_namespace_id = [22; 32],
            2 => {
                acct = AccountId::new(
                    KeyPair::from_seed(vec![92; 32], Algorithm::Ed25519)
                        .public_key()
                        .clone(),
                )
            }
            _ => otherfi = "bpng".parse().unwrap(),
        };
        assert_ne!(lane, t.derive_enrollment_lane(&otherfi, &acct).unwrap());
    }
    assert!(
        v.derive_enrollment_lane(&p, [21; 32], &fi, &account, 1999)
            .is_err()
    );
}
