//! Engineering enrollment fixture: an explicit test issuer under scheme root `signing_key(17)`
//! and enrollment signer `signing_key(79)`, opaque platform evidence and Ed25519 account keys.
//! Shared by this crate's enrollment tests and iroha_core's in-process monetary acceptance;
//! no attestation verifier admits its evidence and it is never a production issuer.

use super::test_support::{public_key, signing_key};
use super::*;
use crate::kagemusha_wallet_artifacts_v1::InstallationV1;
use crate::kagemusha_wallet_enrollment_v1::*;
use iroha_crypto::{Algorithm, Hash, KeyPair, Signature};
use iroha_data_model::{
    account::AccountId, asset::AssetDefinitionId, kagemusha::*, nexus::AxtAssetIncarnationV1,
};

/// One wallet's enrollment configuration, issuer challenge, account and asset.
pub struct Fixture {
    /// Native enrollment configuration.
    pub config: EnrollmentConfigV1,
    /// Issuer challenge bound to the account, asset and policies.
    pub challenge: KagemushaWalletEnrollmentChallengeV1,
    /// Ed25519 account key that signs account challenges.
    pub account_key: KeyPair,
    /// Registered account.
    pub account: AccountId,
    /// Asset scope.
    pub asset: KagemushaWalletAssetScopeV1,
    /// Opaque platform evidence.
    pub evidence: Vec<u8>,
}

/// Component fixture under its own scheme and asset; account seed 41.
pub fn fixture() -> Fixture {
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
            minimum_package_version: 7,
            signer_set: vec![[5; 32]],
        },
    };
    // Opaque DATA originals test byte custody only; no attestation verifier admits these.
    let policy = KagemushaWalletEnrollmentPolicyV1 {
        version: 1,
        scheme_id: scheme.scheme_id(),
        asset_digest: asset.asset_digest(),
        app_policy: app.policy_digest().unwrap(),
        platform: KagemushaWalletEnrollmentPlatformV1::Android {
            root_set: vec![KagemushaWalletAndroidRootV1 {
                family: KagemushaWalletAndroidRootFamilyV1::GoogleRsa,
                spki_sha256: *Hash::new(b"component root key").as_ref(),
                revocation_source: KagemushaWalletRevocationSourceV1::GoogleAttestationStatus,
            }],
            patch_floor_yyyymm: 202_608,
        },
        regulatory_policy: KagemushaWalletRegulatoryPolicyV1::default(),
        challenge_lifetime_ms: 120_000,
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
    })
    .unwrap();
    let enrollment_certificate = enrollment_certificate(&scheme);
    Fixture {
        config: EnrollmentConfigV1 {
            installation: InstallationV1 {
                scheme_id: scheme.scheme_id(),
                manifest_digest: [29; 32],
            },
            enrollment_certificate,
            service_origin: b"https://issuer.example".to_vec(),
            fi: b"approved FI".to_vec(),
            actor: b"authenticated account actor".to_vec(),
            release: b"approved release".to_vec(),
            session_valid_from_ms: 0,
            session_expires_at_ms: 600_000,
            scheme,
            app,
            policy,
            attestation_root_der: None,
        },
        challenge,
        account_key,
        account,
        asset,
        evidence,
    }
}

/// The component fixture moved to an installed engineering scheme and installation.
pub fn installed_fixture(scheme: KagemushaWalletSchemeV1, installation: InstallationV1) -> Fixture {
    let mut f = fixture();
    f.config.scheme = scheme;
    assert_eq!(
        f.config.scheme.scheme_root_key,
        public_key(&signing_key(17)),
        "engineering scheme root"
    );
    f.config.installation = installation;
    f.config.app.scheme_id = installation.scheme_id;
    f.config.policy.scheme_id = installation.scheme_id;
    f.config.policy.app_policy = f.config.app.policy_digest().unwrap();
    f.config.enrollment_certificate = enrollment_certificate(&f.config.scheme);
    f.challenge.scheme_id = installation.scheme_id;
    f.challenge.app_policy = f.config.app.policy_digest().unwrap();
    f.challenge.enrollment_policy = f.config.policy.policy_digest().unwrap();
    f
}

/// The Ed25519 key with seed `seed`, asserting that it controls `account`.
pub fn require_account_key(account: &AccountId, seed: u8) -> KeyPair {
    let key = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
    assert_eq!(account, &AccountId::new(key.public_key().clone()));
    key
}

/// Bind `f` to a registered account (Ed25519 seed `seed`) and asset.
pub fn bind_identity(
    f: &mut Fixture,
    account: AccountId,
    asset: KagemushaWalletAssetScopeV1,
    seed: u8,
) {
    f.account_key = require_account_key(&account, seed);
    f.account = account;
    f.asset = asset;
    // This is an explicit authenticated actor fixture, independently bound for each account.
    f.config.actor = norito::encode_canonical(&f.account).unwrap();
    f.config.policy.asset_digest = f.asset.asset_digest();
    f.challenge.account_digest = kagemusha_wallet_account_digest_v1(&f.account).unwrap();
    f.challenge.asset_digest = f.asset.asset_digest();
    f.challenge.enrollment_policy = f.config.policy.policy_digest().unwrap();
}

/// Enrollment signer certificate of `scheme`, signed by the engineering root.
pub fn enrollment_certificate(
    scheme: &KagemushaWalletSchemeV1,
) -> KagemushaWalletSignerCertificateV1 {
    use p256::ecdsa::{Signature as P256Signature, signature::Signer as _};
    let body = KagemushaWalletSignerCertificateBodyV1 {
        version: 1,
        scheme_id: scheme.scheme_id(),
        role: KagemushaWalletSignerRoleV1::Enrollment,
        key: public_key(&signing_key(79)),
        serial: 1,
    };
    let signature: P256Signature = signing_key(17).sign(&body.signing_message());
    KagemushaWalletSignerCertificateV1::sign(
        body,
        scheme,
        KagemushaWalletSignerOutputV1::Raw(signature.to_bytes().into()),
    )
    .unwrap()
}

/// The issuer's E1 permit for `dispatch`.
pub fn permit(f: &Fixture, dispatch: &PreKeyDispatchV1) -> KagemushaEnrollmentPermitV1 {
    use p256::ecdsa::{Signature as P256Signature, signature::Signer as _};
    let previous = dispatch.previous_permit.as_ref().map(|bytes| {
        KagemushaEnrollmentPermitV1::decode_canonical(
            bytes,
            &f.config.scheme,
            &f.config.enrollment_certificate,
        )
        .unwrap()
    });
    let challenge = previous.as_ref().map_or(f.challenge, |p| p.body.challenge);
    let body = KagemushaEnrollmentPermitBodyV1 {
        version: 1,
        platform: dispatch.platform,
        purpose: dispatch.purpose,
        challenge,
        network_id: f.config.scheme.network_id,
        manifest_digest: dispatch.manifest_digest,
        release_digest: dispatch.release_digest,
        service_origin_digest: dispatch.service_origin_digest,
        fi_digest: dispatch.fi_digest,
        actor_digest: dispatch.actor_digest,
        attempt_id: previous.as_ref().map_or([31; 32], |p| p.body.attempt_id),
        client_nonce: dispatch.client_nonce,
        native_dispatch_nonce: dispatch.native_dispatch_nonce,
        originals_digest: dispatch.originals_digest(&challenge).unwrap(),
        enrollment_certificate: f.config.enrollment_certificate.certificate_digest(),
        created_at_ms: 1000,
        expires_at_ms: 1000 + f.config.policy.challenge_lifetime_ms,
        observed_at_ms: 2000,
    };
    let signature: P256Signature = signing_key(79).sign(&body.signing_message().unwrap());
    KagemushaEnrollmentPermitV1::from_issuer_der(
        body,
        &f.config.scheme,
        &f.config.enrollment_certificate,
        signature.to_der().as_bytes(),
    )
    .unwrap()
}

/// The account's signature over `message`.
pub fn sign(f: &Fixture, message: &[u8]) -> Vec<u8> {
    Signature::try_new(f.account_key.private_key(), message)
        .unwrap()
        .payload()
        .to_vec()
}

/// The issuer's E6 result for `request`.
pub fn issuer_result(f: &Fixture, request: &RequestV1) -> ResultV1 {
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
    };
    let kind = KagemushaWalletEvidenceKindV1::AndroidKeyMintTee;
    let event = KagemushaWalletEvidenceV1 {
        digest: evidence.digest(&request.body, kind).unwrap(),
        time_ms: 1000,
        facts: KAGEMUSHA_WALLET_ANDROID_REQUIRED_FACTS_V1,
        os_patch_level: 202_608,
        vendor_patch_level: 202_608,
        boot_patch_level: 202_608,
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

/// Run the complete E1–E6 enrollment of `f` on `provider` and return the four opened
/// originals (credential, certificate set, account, asset).
pub fn enroll<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1>(
    provider: KagemushaWalletProviderV1<F, P>,
    f: &Fixture,
) -> [Vec<u8>; 4] {
    let mut owner = EnrollmentOwnerV1::new(provider, f.config.clone())
        .unwrap_or_else(|(_, error)| panic!("{error}"));
    let dispatch = PreKeyDispatchV1::decode(
        &owner
            .begin(
                &[11; 32],
                &norito::encode_canonical(&f.account).unwrap(),
                &norito::encode_canonical(&f.asset).unwrap(),
            )
            .unwrap(),
    )
    .unwrap();
    let challenge = owner
        .accept_permit(&permit(f, &dispatch).encode_canonical().unwrap())
        .unwrap();
    assert!(matches!(
        owner.authorize(&sign(f, &challenge)).unwrap(),
        EnrollmentProgressV1::Evidence { .. }
    ));
    let RequestPreparationV1::AccountChallenge(challenge) =
        owner.prepare_request(&f.evidence).unwrap()
    else {
        panic!("fresh E5 challenge")
    };
    let request = RequestV1::decode(&owner.retain_request(&sign(f, &challenge)).unwrap()).unwrap();
    owner
        .accept_credential(&issuer_result(f, &request).encode().unwrap())
        .unwrap();
    owner.open_originals().unwrap()
}
