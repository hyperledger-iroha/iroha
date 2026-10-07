//! Account/original custody tests using the explicit provider simulator; no device or issuer attestation claim.
use super::test_support::{DeviceV1, FakePlatformV1, public_key, signing_key, test_options};
use super::*;
use crate::kagemusha_wallet_enrollment_v1::*;
use iroha_crypto::{Algorithm, Hash, KeyPair, Signature};
use iroha_data_model::{
    account::AccountId, asset::AssetDefinitionId, kagemusha::*, nexus::AxtAssetIncarnationV1,
};
use sha2::Sha256;

type Owner = EnrollmentOwnerV1<KagemushaWalletSimFsV1, FakePlatformV1>;
struct Fixture {
    config: EnrollmentConfigV1,
    challenge: KagemushaWalletEnrollmentChallengeV1,
    account_key: KeyPair,
    account: AccountId,
    asset: KagemushaWalletAssetScopeV1,
    evidence: Vec<u8>,
}
fn fixture() -> Fixture {
    let scheme = KagemushaWalletSchemeV1 {
        version: 1,
        network_id: *Hash::new(b"enrollment component network").as_ref(),
        scheme_root_key: public_key(&signing_key(17)),
        relation_id: [23; 32],
        provider_contract: kagemusha_wallet_provider_contract_v1(),
    };
    let asset = KagemushaWalletAssetScopeV1::new(
        AssetDefinitionId::from_uuid_bytes([
            0x2f, 0x17, 0xc7, 0x24, 0x66, 0xf8, 0x4a, 0x4b, 0xb8, 0xa8, 0xe2, 0x48, 0x84, 0xfd,
            0xcd, 0x2f,
        ])
        .unwrap(),
        &AxtAssetIncarnationV1::try_from_bytes(*Hash::new(b"enrollment component asset").as_ref())
            .unwrap(),
        2,
    )
    .unwrap();
    let account_key = KeyPair::from_seed(vec![41; 32], Algorithm::Ed25519);
    let account = AccountId::new(account_key.public_key().clone());
    let app = KagemushaWalletAppPolicyV1 {
        version: 1,
        scheme_id: scheme.scheme_id(),
        identity: KagemushaWalletAppIdentityV1::Android {
            package_name: "org.example.wallet".into(),
            package_version: 7,
            app_signing_certificate_sha256: [5; 32],
        },
    };
    // Opaque DATA originals test byte custody only; no attestation verifier admits these.
    let root = b"component root original".to_vec();
    let policy = KagemushaWalletEnrollmentPolicyV1 {
        version: 1,
        scheme_id: scheme.scheme_id(),
        asset_digest: asset.asset_digest(),
        app_policy: app.policy_digest().unwrap(),
        platform: KagemushaWalletEnrollmentPlatformV1::Android {
            attestation_root_sha256: Sha256::digest(&root).into(),
            hardware: KagemushaWalletAndroidHardwareV1::Tee,
            patch_floor_yyyymm: 202608,
            play_integrity_maximum_age_ms: 120000,
            require_play_recognized: true,
            require_licensed: true,
            minimum_device_integrity: KagemushaWalletPlayIntegrityLevelV1::Device,
        },
        regulatory_policy: KagemushaWalletRegulatoryPolicyV1::default(),
        challenge_lifetime_ms: 120000,
        attestation_lease_lifetime_ms: 0,
    };
    let challenge = KagemushaWalletEnrollmentChallengeV1 {
        version: 1,
        scheme_id: scheme.scheme_id(),
        asset_digest: asset.asset_digest(),
        account_digest: kagemusha_wallet_account_digest_v1(&account).unwrap(),
        app_policy: app.policy_digest().unwrap(),
        enrollment_policy: policy.policy_digest().unwrap(),
        issuer_nonce: [47; 32],
    };
    let evidence = norito::encode_canonical(&PlatformEvidenceV1::Android {
        certificates: vec![b"original leaf".to_vec(), b"original CA".to_vec()],
        play_integrity_token: b"opaque token".to_vec(),
    })
    .unwrap();
    Fixture {
        config: EnrollmentConfigV1 {
            scheme,
            app,
            policy,
            attestation_root_der: root,
        },
        challenge,
        account_key,
        account,
        asset,
        evidence,
    }
}
fn owner(f: &Fixture, d: &DeviceV1) -> Owner {
    let provider = KagemushaWalletProviderV1::open(
        d.fs.clone(),
        d.platform.clone(),
        f.config.scheme.scheme_id(),
        test_options(),
    )
    .unwrap();
    EnrollmentOwnerV1::new(provider, f.config.clone())
        .unwrap_or_else(|(_, error)| panic!("{error}"))
}
fn begin(f: &Fixture, owner: &mut Owner) -> [u8; 32] {
    owner
        .begin(
            &norito::encode_canonical(&f.challenge).unwrap(),
            &norito::encode_canonical(&f.account).unwrap(),
            &norito::encode_canonical(&f.asset).unwrap(),
        )
        .unwrap()
}
fn sign(f: &Fixture, message: &[u8]) -> Vec<u8> {
    Signature::try_new(f.account_key.private_key(), message)
        .unwrap()
        .payload()
        .to_vec()
}
fn enrolled(f: &Fixture, d: &DeviceV1) -> Owner {
    let mut owner = owner(f, d);
    let message = begin(f, &mut owner);
    assert!(matches!(
        owner.authorize(&sign(f, &message)).unwrap(),
        EnrollmentProgressV1::Evidence { .. }
    ));
    owner
}
fn request(f: &Fixture, owner: &mut Owner) -> Vec<u8> {
    let RequestPreparationV1::AccountChallenge(message) =
        owner.prepare_request(&f.evidence).unwrap()
    else {
        panic!("fresh challenge")
    };
    owner.retain_request(&sign(f, &message)).unwrap()
}
#[test]
fn enrollment_owner_requires_exact_account_before_hardware_and_consumes_failed_challenge() {
    let f = fixture();
    let d = DeviceV1::new(KagemushaWalletAnchorPolicyV1::NotRequired, 53);
    let mut owner = owner(&f, &d);
    let first = begin(&f, &mut owner);
    assert!(owner.authorize(&[0; 64]).is_err());
    assert!(owner.authorize(&sign(&f, &first)).is_err());
    d.platform.with(|s| assert_eq!(s.generate_calls, 0));
    let second = begin(&f, &mut owner);
    assert_ne!(first, second);
    assert!(matches!(
        owner.authorize(&sign(&f, &second)).unwrap(),
        EnrollmentProgressV1::Evidence { .. }
    ));
    d.platform.with(|s| {
        assert_eq!(s.generate_calls, 1);
        assert_eq!(
            s.last_generation.unwrap().profile,
            KagemushaWalletKeyProfileV1::AndroidTee
        );
    });
    let third = begin(&f, &mut owner);
    owner.authorize(&sign(&f, &third)).unwrap();
    d.platform.with(|s| assert_eq!(s.generate_calls, 1));
}
#[test]
fn enrollment_owner_retains_exact_e5_across_restart_and_refuses_changed_signature_originals() {
    let f = fixture();
    let d = DeviceV1::new(KagemushaWalletAnchorPolicyV1::NotRequired, 59);
    let mut owner = enrolled(&f, &d);
    let RequestPreparationV1::AccountChallenge(message) =
        owner.prepare_request(&f.evidence).unwrap()
    else {
        panic!("challenge")
    };
    assert!(owner.retain_request(&[0; 64]).is_err());
    let original = owner.retain_request(&sign(&f, &message)).unwrap();
    let decoded = RequestV1::decode(&original).unwrap();
    assert_eq!(decoded.body.evidence, f.evidence);
    let mut changed = decoded.clone();
    changed.body.marker.payment_key = public_key(&signing_key(61));
    assert!(changed.validate().is_err());
    let mut trailing = original.clone();
    trailing.push(0);
    assert!(RequestV1::decode(&trailing).is_err());
    drop(owner.into_provider());
    let mut recovered = enrolled(&f, &d);
    assert!(
        matches!(recovered.prepare_request(b"changed").unwrap(),RequestPreparationV1::Retained(bytes) if bytes==original)
    );
    assert_eq!(recovered.retain_request(&[]).unwrap(), original);
    d.platform.with(|s| assert_eq!(s.generate_calls, 1));
}
#[test]
fn enrollment_unavailable_never_becomes_absence_or_a_second_key_attempt() {
    let f = fixture();
    let d = DeviceV1::new(KagemushaWalletAnchorPolicyV1::NotRequired, 67);
    let mut owner = owner(&f, &d);
    let message = begin(&f, &mut owner);
    d.platform.with(|s| s.generate_unavailable = Some(false));
    assert!(owner.authorize(&sign(&f, &message)).is_err());
    d.platform.with(|s| s.generate_unavailable = None);
    let message = begin(&f, &mut owner);
    assert!(matches!(
        owner.authorize(&sign(&f, &message)).unwrap(),
        EnrollmentProgressV1::Pending
    ));
    d.platform.with(|s| assert_eq!(s.generate_calls, 1));
    assert!(owner.prepare_request(&f.evidence).is_err());
}
#[test]
fn enrollment_issuer_evidence_digest_uses_original_google_response_not_mobile_token() {
    let f = fixture();
    let d = DeviceV1::new(KagemushaWalletAnchorPolicyV1::NotRequired, 71);
    let mut owner = enrolled(&f, &d);
    let requested = RequestV1::decode(&request(&f, &mut owner)).unwrap();
    let proof = IssuerEvidenceV1::Android {
        certificates: vec![b"original leaf".to_vec(), b"original CA".to_vec()],
        google_response: b"original Google HTTPS response".to_vec(),
    };
    let kind = KagemushaWalletEvidenceKindV1::AndroidKeyMintTee;
    assert_eq!(
        proof.digest(&requested.body, kind).unwrap(),
        kagemusha_wallet_evidence_digest_v1(
            kind,
            &[
                b"original leaf",
                b"original CA",
                b"original Google HTTPS response"
            ]
        )
        .unwrap()
    );
    let mut changed = proof.clone();
    if let IssuerEvidenceV1::Android {
        google_response, ..
    } = &mut changed
    {
        google_response[0] ^= 1;
    }
    assert_ne!(
        proof.digest(&requested.body, kind).unwrap(),
        changed.digest(&requested.body, kind).unwrap()
    );
    if let IssuerEvidenceV1::Android { certificates, .. } = &mut changed {
        certificates[0][0] ^= 1;
    }
    assert!(changed.digest(&requested.body, kind).is_err());
    assert!(
        proof
            .digest(
                &requested.body,
                KagemushaWalletEvidenceKindV1::AppleAppAttest
            )
            .is_err()
    );
}

fn issuer_result(f: &Fixture, request: &RequestV1) -> ResultV1 {
    use p256::ecdsa::{Signature as P256Signature, signature::Signer as _};
    fn output(
        key: &p256::ecdsa::SigningKey,
        message: &[u8],
    ) -> KagemushaWalletSignerOutputV1<'static> {
        let signed: P256Signature = key.sign(message);
        KagemushaWalletSignerOutputV1::Raw(signed.to_bytes().into())
    }
    let root = signing_key(17);
    let issuer = signing_key(79);
    let certificate_body = KagemushaWalletSignerCertificateBodyV1 {
        version: 1,
        scheme_id: f.config.scheme.scheme_id(),
        role: KagemushaWalletSignerRoleV1::Enrollment,
        key: public_key(&issuer),
        serial: 1,
    };
    let certificate = KagemushaWalletSignerCertificateV1::sign(
        certificate_body,
        &f.config.scheme,
        output(&root, &certificate_body.signing_message()),
    )
    .unwrap();
    let evidence = IssuerEvidenceV1::Android {
        certificates: vec![b"original leaf".to_vec(), b"original CA".to_vec()],
        google_response: b"original Google HTTPS response".to_vec(),
    };
    let kind = KagemushaWalletEvidenceKindV1::AndroidKeyMintTee;
    let event = KagemushaWalletEvidenceV1 {
        digest: evidence.digest(&request.body, kind).unwrap(),
        time_ms: 1000,
        facts: KAGEMUSHA_WALLET_ANDROID_REQUIRED_FACTS_V1,
        os_patch_level: 202608,
        vendor_patch_level: 202608,
        boot_patch_level: 202608,
    };
    let body = KagemushaWalletCredentialBodyV1 {
        version: 1,
        scheme_id: f.config.scheme.scheme_id(),
        asset_digest: f.asset.asset_digest(),
        wallet_id: request.body.marker.wallet_id,
        account_digest: f.challenge.account_digest,
        payment_key: request.body.marker.payment_key,
        provider_contract: f.config.scheme.provider_contract,
        evidence_kind: kind,
        enrollment_evidence: event,
        fresh_evidence: event,
        app_policy: f.challenge.app_policy,
        regulatory_policy: f.config.policy.regulatory_policy,
        enrollment_id: f.challenge.enrollment_id(&request.body.marker.payment_key),
        issued_at_ms: 2000,
        renewal_sequence: 0,
        lease_expires_at_ms: 0,
        issuer_certificate: certificate.certificate_digest(),
    };
    let credential = KagemushaWalletCredentialV1::sign(
        body,
        &certificate,
        output(&issuer, &body.signing_message()),
    )
    .unwrap();
    ResultV1 {
        version: 1,
        credential: credential.to_canonical_bytes().unwrap(),
        certificates: norito::encode_canonical(
            &KagemushaWalletCertificateSetV1::new(vec![certificate]).unwrap(),
        )
        .unwrap(),
        evidence,
    }
}
#[test]
fn enrollment_e6_retains_full_signed_result_and_never_recreates_lost_selected_originals() {
    let f = fixture();
    let d = DeviceV1::new(KagemushaWalletAnchorPolicyV1::NotRequired, 73);
    let mut owner = enrolled(&f, &d);
    let request_bytes = request(&f, &mut owner);
    let requested = RequestV1::decode(&request_bytes).unwrap();
    let result = issuer_result(&f, &requested);
    let bytes = result.encode().unwrap();
    let mut altered = result.clone();
    if let IssuerEvidenceV1::Android {
        google_response, ..
    } = &mut altered.evidence
    {
        google_response[0] ^= 1;
    }
    assert!(owner.accept_credential(&altered.encode().unwrap()).is_err());
    assert_eq!(owner.accept_credential(&bytes).unwrap(), bytes);
    assert_eq!(owner.accept_credential(b"wrong retry").unwrap(), bytes);
    let mut provider = owner.into_provider();
    let slot = provider.slots().unwrap()[0];
    let key = kagemusha_wallet_provider_digest_v1("enrollment-issuer-result", &request_bytes);
    provider
        .with_archive(&slot, |archive| archive.remove_record(&key))
        .unwrap();
    drop(provider);
    let mut restored = enrolled(&f, &d);
    assert!(restored.accept_credential(&bytes).is_err());
}
#[test]
fn enrollment_approved_native_configuration_cannot_silently_replace_original_root() {
    let mut f = fixture();
    let d = DeviceV1::new(KagemushaWalletAnchorPolicyV1::NotRequired, 83);
    let provider = KagemushaWalletProviderV1::open(
        d.fs.clone(),
        d.platform.clone(),
        f.config.scheme.scheme_id(),
        test_options(),
    )
    .unwrap();
    f.config.attestation_root_der[0] ^= 1;
    let (provider, error) = match EnrollmentOwnerV1::new(provider, f.config) {
        Ok(_) => panic!("wrong root accepted"),
        Err(parts) => parts,
    };
    assert!(matches!(error, Error::Original("approved root original")));
    assert!(provider.slots().unwrap().is_empty());
    d.platform.with(|s| assert_eq!(s.generate_calls, 0));
}

#[test]
fn enrollment_google_original_and_complete_e6_bounds_cover_exact_maximal_encoding() {
    let f = fixture();
    let d = DeviceV1::new(KagemushaWalletAnchorPolicyV1::NotRequired, 89);
    let mut owner = enrolled(&f, &d);
    let mut requested = RequestV1::decode(&request(&f, &mut owner)).unwrap();
    let mut certificates = vec![vec![7; 16_384]; 8];
    // Find the exact largest allowed chain in a canonical mobile evidence frame with the
    // smallest token; a larger token only reduces available DER payload.
    let mut low = 1;
    let mut high = 16_384;
    while low < high {
        let length = (low + high + 1) / 2;
        certificates[7] = vec![7; length];
        let encoded = norito::encode_canonical(&PlatformEvidenceV1::Android {
            certificates: certificates.clone(),
            play_integrity_token: vec![9],
        })
        .unwrap();
        if encoded.len() <= EVIDENCE_MAX_BYTES {
            low = length;
        } else {
            high = length - 1;
        }
    }
    certificates[7] = vec![7; low];
    requested.body.evidence = norito::encode_canonical(&PlatformEvidenceV1::Android {
        certificates: certificates.clone(),
        play_integrity_token: vec![9],
    })
    .unwrap();
    let kind = KagemushaWalletEvidenceKindV1::AndroidKeyMintTee;
    for length in [65_537, 131_072] {
        let evidence = IssuerEvidenceV1::Android {
            certificates: certificates.clone(),
            google_response: vec![3; length],
        };
        assert!(evidence.digest(&requested.body, kind).is_ok());
        let result = ResultV1 {
            version: 1,
            credential: vec![1; KAGEMUSHA_WALLET_CREDENTIAL_MAX_BYTES_V1],
            certificates: vec![2; KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1],
            evidence,
        };
        let encoded = result.encode().unwrap();
        eprintln!(
            "ENROLLMENT_BOUND google={length} mobile={} result={} limit={RESULT_MAX_BYTES}",
            requested.body.evidence.len(),
            encoded.len()
        );
        assert_eq!(ResultV1::decode(&encoded).unwrap(), result);
    }
    assert!(
        IssuerEvidenceV1::Android {
            certificates,
            google_response: vec![3; 131_073]
        }
        .digest(&requested.body, kind)
        .is_err()
    );
}

#[test]
fn enrollment_failed_native_handoff_preserves_exact_private_selection_and_e5() {
    let f = fixture();
    let d = DeviceV1::new(KagemushaWalletAnchorPolicyV1::NotRequired, 97);
    let mut owner = enrolled(&f, &d);
    let bytes = request(&f, &mut owner);
    let (mut owner, error) =
        match owner.try_handoff::<(), _>(|provider| Err((provider, "loader unavailable"))) {
            Ok(()) => panic!("failed loader admitted"),
            Err(parts) => parts,
        };
    assert_eq!(error, "loader unavailable");
    assert_eq!(owner.retain_request(&[]).unwrap(), bytes);
    d.platform.with(|state| assert_eq!(state.generate_calls, 1));
}
