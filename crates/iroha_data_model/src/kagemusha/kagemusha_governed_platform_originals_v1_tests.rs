//! Actual Ed25519 threshold mathematics on explicitly synthetic DER/body originals; no
//! PKIX/CMS/provider status response/device/native installation or financial success is claimed.
use super::*;
use crate::{id::NetworkId, kagemusha::*};
use iroha_crypto::{Hash, HashOf, KeyPair};
fn sign_snapshot(s: &mut KagemushaGovernedPlatformOriginalsV1) {
    let m = s.subject.approval_signing_bytes().unwrap();
    s.approvals = [81, 82]
        .into_iter()
        .map(|seed| {
            let k = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
            KagemushaGovernedPlatformOriginalsApprovalV1 {
                public_key: k.public_key().clone(),
                signature: Signature::try_new(k.private_key(), &m).unwrap(),
            }
        })
        .collect();
    s.approvals.sort_by(|a, b| a.public_key.cmp(&b.public_key));
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
    KagemushaGovernedPlatformOriginalsV1,
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
        enrollment_issuer_policy_digest: [20; 32],
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
    let mut snapshot = KagemushaGovernedPlatformOriginalsV1 {
        subject: KagemushaGovernedPlatformOriginalsSubjectV1 {
            version: 1,
            authority_set_id: [80; 32],
            network_id: [11; 32],
            roots,
            evaluation,
            valid_from_ms: 1000,
            expires_at_ms: 20000,
        },
        approvals: vec![],
    };
    sign_snapshot(&mut snapshot);
    (snapshot, policy)
}
#[test]
fn governed_android_full_root_status_and_signature_originals_roundtrip() {
    let (s, p) = fixture(false);
    let b = encode(&s).unwrap();
    let v = KagemushaGovernedPlatformOriginalsV1::decode_canonical_exact(&b)
        .unwrap()
        .authenticate(&p, 2000)
        .unwrap();
    assert_eq!(v.original(), b);
    assert_eq!(v.platform_roots_der(), s.subject.roots.platform_roots_der);
    assert_eq!(v.evaluation_originals(), &s.subject.evaluation);
    assert!(v.apple_receipt_roots_der().is_empty());
}
#[test]
fn governed_apple_receipt_and_attestation_roles_stay_distinct() {
    let (mut s, p) = fixture(true);
    let original = encode(&s).unwrap();
    let decoded = KagemushaGovernedPlatformOriginalsV1::decode_canonical_exact(&original).unwrap();
    assert_eq!(decoded, s);
    let v = decoded.authenticate(&p, 2000).unwrap();
    assert_eq!(v.original(), original);
    assert_ne!(v.platform_roots_der(), v.apple_receipt_roots_der());
    assert!(
        v.require_android_certificate_status(&p, "2", [4; 32], 2000)
            .is_err()
    );
    s.subject.roots.apple_receipt_roots_der = s.subject.roots.platform_roots_der.clone();
    assert!(s.subject.approval_signing_bytes().is_err());
}

#[test]
fn governed_public_constituent_types_preserve_full_canonical_originals() {
    fn roundtrip<T>(value: &T)
    where
        T: core::fmt::Debug + PartialEq + norito::NoritoSerialize,
        for<'de> T: norito::NoritoDeserialize<'de>,
    {
        let original = norito::encode_canonical(value).unwrap();
        let decoded: T = norito::decode_canonical_with_limits(
            &original,
            norito::canonical_decode_limits(original.len()),
        )
        .unwrap();
        assert_eq!(&decoded, value);
        assert_eq!(norito::encode_canonical(&decoded).unwrap(), original);
        let mut tail = original.clone();
        tail.push(0);
        assert!(
            norito::decode_canonical_with_limits::<T>(
                &tail,
                norito::canonical_decode_limits(tail.len()),
            )
            .is_err()
        );
    }
    for apple in [false, true] {
        let (snapshot, checked_policy) = fixture(apple);
        roundtrip(&snapshot.subject.roots);
        roundtrip(&snapshot.subject.evaluation);
        roundtrip(&snapshot.subject);
        roundtrip(&snapshot.approvals[0]);
        roundtrip(&snapshot);
        if let KagemushaPlatformEvaluationOriginalsV1::AppleReceiptRisk(risk) =
            &snapshot.subject.evaluation
        {
            roundtrip(risk);
        }
        let original = encode(&snapshot).unwrap();
        let decoded =
            KagemushaGovernedPlatformOriginalsV1::decode_canonical_exact(&original).unwrap();
        decoded.authenticate(&checked_policy, 2000).unwrap();
        assert!(
            decoded
                .authenticate(&checked_policy, snapshot.subject.expires_at_ms)
                .is_err()
        );
    }
}
#[test]
fn governed_ordered_full_der_mutation_and_resigned_swaps_do_not_match_policy() {
    let (s, p) = fixture(false);
    for mode in 0..3 {
        let mut bad = s.clone();
        match mode {
            0 => bad.subject.roots.platform_roots_der[0][2] ^= 1,
            1 => bad.subject.roots.platform_roots_der.reverse(),
            _ => bad.subject.roots.platform_roots_der[0].push(0),
        };
        sign_snapshot(&mut bad);
        assert!(bad.authenticate(&p, 2000).is_err());
    }
}
#[test]
fn governed_threshold_rejects_missing_unknown_duplicate_and_changed_signatures() {
    let (s, p) = fixture(false);
    let mut missing = s.clone();
    missing.approvals.pop();
    assert!(missing.authenticate(&p, 2000).is_err());
    let mut dup = s.clone();
    dup.approvals[1] = dup.approvals[0].clone();
    assert!(dup.authenticate(&p, 2000).is_err());
    let mut bad = s.clone();
    let mut bytes = bad.approvals[0].signature.payload().to_vec();
    bytes[4] ^= 1;
    bad.approvals[0].signature = Signature::from_bytes(&bytes);
    assert!(bad.authenticate(&p, 2000).is_err());
    let k = KeyPair::from_seed(vec![83; 32], Algorithm::Ed25519);
    let mut foreign = s;
    foreign.approvals[0] = KagemushaGovernedPlatformOriginalsApprovalV1 {
        public_key: k.public_key().clone(),
        signature: Signature::try_new(
            k.private_key(),
            &foreign.subject.approval_signing_bytes().unwrap(),
        )
        .unwrap(),
    };
    foreign
        .approvals
        .sort_by(|a, b| a.public_key.cmp(&b.public_key));
    assert!(foreign.authenticate(&p, 2000).is_err());
}
#[test]
fn governed_complete_archive_bounds_and_noncanonical_tails_reject() {
    let (s, _) = fixture(false);
    let b = encode(&s).unwrap();
    for bytes in [vec![], b[..b.len() - 1].to_vec(), {
        let mut x = b.clone();
        x.push(0);
        x
    }] {
        assert!(KagemushaGovernedPlatformOriginalsV1::decode_canonical_exact(&bytes).is_err());
    }
    let mut bad = s;
    bad.subject.roots.platform_roots_der[0] = vec![3; MAX_DER + 1];
    assert!(bad.subject.approval_signing_bytes().is_err());
}
#[test]
fn governed_snapshot_network_set_platform_and_original_intervals_reject() {
    let (s, p) = fixture(true);
    for mode in 0..5 {
        let mut bad = s.clone();
        match mode {
            0 => bad.subject.network_id[0] ^= 1,
            1 => bad.subject.authority_set_id[0] ^= 1,
            2 => bad.subject.valid_from_ms = 2001,
            3 => bad.subject.expires_at_ms = 2000,
            _ => bad.subject.version = 2,
        };
        if bad.subject.approval_signing_bytes().is_ok() {
            sign_snapshot(&mut bad);
        }
        assert!(bad.authenticate(&p, 2000).is_err());
    }
}
#[test]
fn governed_current_rechecks_do_not_cache_time_or_extend_android_original_status() {
    let (s, p) = fixture(false);
    let v = s.authenticate(&p, 2000).unwrap();
    assert!(v.recheck_current(&p, 1999).is_err());
    v.recheck_current(&p, 10999).unwrap();
    assert!(v.recheck_current(&p, 11000).is_err());
    assert!(v.recheck_current(&p, 20000).is_err());
    assert!(v.recheck_current(&p, 500000).is_err());
}
#[test]
fn governed_android_reuses_exact_existing_snapshot_digest_preimage() {
    let evaluation = android_evaluation();
    let KagemushaPlatformEvaluationOriginalsV1::AndroidRevocation {
        canonical_snapshot_original,
        ..
    } = &evaluation
    else {
        unreachable!()
    };
    assert_eq!(
        evaluation.canonical_digest().unwrap(),
        <[u8; 32]>::from(Sha256::digest(canonical_snapshot_original))
    );
    assert_eq!(
        evaluation
            .canonical_digest()
            .unwrap()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
        "f7b1197d0e47393f90de5348adffdb3f56a8947117bcfa0ba9a4c9fd563b0a3a"
    );
}
#[test]
fn governed_complete_android_payload_cannot_be_changed_under_old_snapshot() {
    let (mut s, p) = fixture(false);
    let KagemushaPlatformEvaluationOriginalsV1::AndroidRevocation {
        original_status_payload,
        ..
    } = &mut s.subject.evaluation
    else {
        unreachable!()
    };
    original_status_payload[1] ^= 1;
    assert!(s.subject.approval_signing_bytes().is_err());
    assert!(s.authenticate(&p, 2000).is_err());
}
#[test]
fn governed_android_serial_tbs_denies_and_canonical_shapes_are_enforced() {
    let (s, p) = fixture(false);
    let v = s.authenticate(&p, 2000).unwrap();
    for (serial, tbs) in [
        ("1", [4; 32]),
        ("a", [4; 32]),
        ("2", [0x12; 32]),
        ("01", [4; 32]),
        ("A", [4; 32]),
    ] {
        assert!(
            v.require_android_certificate_status(&p, serial, tbs, 2000)
                .is_err()
        );
    }
    v.require_android_certificate_status(&p, "2", [4; 32], 2000)
        .unwrap();
}
#[test]
fn governed_android_snapshot_parser_preserves_sdk_bounds_order_and_freshness() {
    let KagemushaPlatformEvaluationOriginalsV1::AndroidRevocation {
        canonical_snapshot_original,
        ..
    } = android_evaluation()
    else {
        unreachable!()
    };
    let good = String::from_utf8(canonical_snapshot_original).unwrap();
    for bad in [
        good.replace("serial=a", "serial=1"),
        good.replace("serial_count=2", "serial_count=02"),
        good.replace("response_date_ms=1000", "response_date_ms=1001"),
        good.replace("last_modified_ms=-", "last_modified_ms=2000"),
        good.replace("cache_max_age_seconds=10", "cache_max_age_seconds=86401"),
        good.replace("12".repeat(32).as_str(), &"00".repeat(32)),
        format!("{good}\n"),
        good.trim_end().to_owned(),
    ] {
        assert!(AndroidSnapshot::decode(bad.as_bytes()).is_err());
    }
    let d = AndroidSnapshot::decode(good.as_bytes()).unwrap();
    assert!(d.validate_at(999).is_err());
    d.validate_at(1000).unwrap();
    assert!(d.validate_at(11000).is_err());
}
#[test]
fn governed_apple_risk_policy_requires_actual_app_id_and_bounded_creation_age() {
    let (mut s, p) = fixture(true);
    let KagemushaPlatformEvaluationOriginalsV1::AppleReceiptRisk(r) = &mut s.subject.evaluation
    else {
        unreachable!()
    };
    r.app_id_utf8 = b"ABCDEFGHIJ.other.ordinary".to_vec();
    sign_snapshot(&mut s);
    assert!(s.authenticate(&p, 2000).is_err());
    let KagemushaPlatformEvaluationOriginalsV1::AppleReceiptRisk(r) = &mut s.subject.evaluation
    else {
        unreachable!()
    };
    r.maximum_creation_age_ms = 300001;
    assert!(s.subject.approval_signing_bytes().is_err());
}
