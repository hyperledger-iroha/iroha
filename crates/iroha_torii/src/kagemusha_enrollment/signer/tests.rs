//! Actual P-256 signatures and private-file custody; no provider or attestation admission claim.

use super::*;
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::kagemusha::*;
use p256::ecdsa::signature::{Verifier as _, hazmat::PrehashVerifier as _};
use std::path::PathBuf;

fn public(key: &SigningKey) -> KagemushaDevicePublicKeyV1 {
    KagemushaDevicePublicKeyV1::from_sec1_bytes(
        key.verifying_key().to_encoded_point(false).as_bytes(),
    )
    .unwrap()
}

fn certificate(
    scheme: &KagemushaWalletSchemeV1,
    key: &SigningKey,
    role: KagemushaWalletSignerRoleV1,
    serial: u64,
) -> KagemushaWalletSignerCertificateV1 {
    let root = SigningKey::from_slice(&[1; 32]).unwrap();
    let body = KagemushaWalletSignerCertificateBodyV1 {
        version: 1,
        scheme_id: scheme.scheme_id(),
        role,
        key: public(key),
        serial,
    };
    let signature: Signature = root.sign(&body.signing_message());
    KagemushaWalletSignerCertificateV1::sign(
        body,
        scheme,
        KagemushaWalletSignerOutputV1::Der(signature.to_der().as_bytes()),
    )
    .unwrap()
}

fn provider(path: PathBuf) -> KagemushaEnrollmentProvider {
    let root = SigningKey::from_slice(&[1; 32]).unwrap();
    let key = SigningKey::from_slice(&[2; 32]).unwrap();
    let scheme = KagemushaWalletSchemeV1 {
        version: 1,
        network_id: [3; 32],
        scheme_root_key: public(&root),
        relation_id: [4; 32],
        provider_contract: kagemusha_wallet_provider_contract_v1(),
    };
    let app = KagemushaWalletAppPolicyV1 {
        version: 1,
        scheme_id: scheme.scheme_id(),
        identity: KagemushaWalletAppIdentityV1::Apple {
            app_id: "TEAM.org.example.wallet".into(),
        },
    };
    let enrollment = KagemushaWalletEnrollmentPolicyV1 {
        version: 1,
        scheme_id: scheme.scheme_id(),
        asset_digest: [5; 32],
        app_policy: app.policy_digest().unwrap(),
        platform: KagemushaWalletEnrollmentPlatformV1::Apple {
            attestation_root_sha256: [6; 32],
        },
        regulatory_policy: KagemushaWalletRegulatoryPolicyV1::default(),
        challenge_lifetime_ms: 600_001,
        attestation_lease_lifetime_ms: 0,
    };
    let observer = KeyPair::from_seed(vec![7; 32], Algorithm::Ed25519);
    let eligibility = KagemushaEligibilityPolicyV1 {
        version: 1,
        network_id: scheme.network_id,
        scheme_id: scheme.scheme_id(),
        asset_digest: enrollment.asset_digest,
        revision: 1,

        authority: KagemushaEligibilityAuthorityV1::Bank { fi_digest: [8; 32] },
        public_key: observer.public_key().to_bytes().1.try_into().unwrap(),
        maximum_response_ms: 1000,
    };
    eligibility.validate().unwrap();
    KagemushaEnrollmentProvider {
        eligibility,
        scheme,
        app,
        enrollment,
        certificate: certificate(&scheme, &key, KagemushaWalletSignerRoleV1::Enrollment, 1),
        manifest_digest: [9; 32],
        release_digest: [10; 32],
        service_origin_digest: [11; 32],
        observation_endpoint: "https://provider.example/eligibility".parse().unwrap(),
        observation_credential: path.with_file_name("transport"),
        worker_handle: "native://issuer/platform-worker".into(),
        signer_private_key: path,
    }
}

fn fixture(bytes: &[u8]) -> (tempfile::TempDir, KagemushaEnrollmentProvider) {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/qualification/enrollment-signer-tests");
    std::fs::create_dir_all(&base).unwrap();
    let base = base.canonicalize().unwrap();
    let root = tempfile::Builder::new()
        .prefix("private-")
        .tempdir_in(base)
        .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let directory = iroha_fs::PrivateDirectory::open(root.path()).unwrap();
    directory
        .write_atomic("enrollment.scalar", bytes, iroha_fs::PublishMode::CreateNew)
        .unwrap();
    let provider = provider(
        root.path()
            .join("enrollment.scalar")
            .canonicalize()
            .unwrap(),
    );
    (root, provider)
}

#[test]
fn actual_signer_uses_sha256_and_canonical_low_s_der_for_exact_selected_key() {
    let (_root, provider) = fixture(&[2; 32]);
    let owner = EnrollmentSigner::open(&provider.signer_private_key, &provider).unwrap();
    let message = [19; 32];
    let der = owner.sign(&provider, &message).unwrap();
    assert!((8..=72).contains(&der.len()));
    let signature = Signature::from_der(&der).unwrap();
    assert_eq!(signature.to_der().as_bytes(), der);
    assert!(signature.normalize_s().is_none());
    let key =
        p256::ecdsa::VerifyingKey::from_sec1_bytes(provider.certificate.body.key.as_sec1_bytes())
            .unwrap();
    key.verify(&message, &signature).unwrap();
    assert!(key.verify_prehash(&message, &signature).is_err());
    assert!(key.verify(&[20; 32], &signature).is_err());
    assert_eq!(owner.sign(&provider, &message).unwrap(), der);
}

#[test]
fn scalar_encoding_is_fixed_and_key_generation_is_never_a_missing_key_fallback() {
    for secret in [
        vec![],
        vec![2; 31],
        vec![2; 33],
        vec![0; 32],
        vec![255; 32],
        b"0202020202020202020202020202020202020202020202020202020202020202\n".to_vec(),
    ] {
        let (_root, provider) = fixture(&secret);
        assert!(matches!(
            EnrollmentSigner::open(&provider.signer_private_key, &provider),
            Err(Error::Invalid)
        ));
    }
    let (_root, mut provider) = fixture(&[2; 32]);
    provider.signer_private_key.set_file_name("missing");
    assert!(matches!(
        EnrollmentSigner::open(&provider.signer_private_key, &provider),
        Err(Error::Unavailable)
    ));
    assert!(!provider.signer_private_key.exists());
    for path in [
        Path::new("relative.scalar"),
        Path::new("/nonexistent/../key"),
    ] {
        provider.signer_private_key = path.to_path_buf();
        assert!(matches!(
            EnrollmentSigner::open(path, &provider),
            Err(Error::Selection)
        ));
    }
}

#[test]
fn root_role_public_key_and_exact_current_selection_cannot_be_substituted() {
    let (_root, provider) = fixture(&[2; 32]);
    let owner = EnrollmentSigner::open(&provider.signer_private_key, &provider).unwrap();
    let key = SigningKey::from_slice(&[2; 32]).unwrap();
    let foreign = SigningKey::from_slice(&[3; 32]).unwrap();
    for case in 0..8 {
        let mut changed = provider.clone();
        match case {
            0 => {
                changed.certificate = certificate(
                    &provider.scheme,
                    &key,
                    KagemushaWalletSignerRoleV1::Artifact,
                    1,
                )
            }
            1 => {
                changed.certificate = certificate(
                    &provider.scheme,
                    &foreign,
                    KagemushaWalletSignerRoleV1::Enrollment,
                    1,
                )
            }
            2 => {
                changed.certificate = certificate(
                    &provider.scheme,
                    &key,
                    KagemushaWalletSignerRoleV1::Enrollment,
                    2,
                )
            }
            3 => changed.scheme.network_id[0] ^= 1,
            4 => changed.scheme.scheme_root_key = public(&foreign),
            5 => changed.release_digest[0] ^= 1,
            6 => changed.signer_private_key.set_file_name("alternate.scalar"),
            _ => changed.certificate.body.serial += 1,
        }
        assert!(matches!(owner.revalidate(&changed), Err(Error::Selection)));
        assert!(matches!(
            owner.sign(&changed, &[19; 32]),
            Err(Error::Selection)
        ));
        if case < 2 || case == 3 || case == 4 || case == 7 {
            assert!(matches!(
                EnrollmentSigner::open(&changed.signer_private_key, &changed),
                Err(Error::Selection)
            ));
        }
    }
}

#[cfg(unix)]
#[test]
fn descriptor_rejects_replacement_loss_permissions_and_links_without_returning_a_signature() {
    use std::os::unix::fs::{PermissionsExt as _, symlink};
    for case in 0..7 {
        let (_root, provider) = fixture(&[2; 32]);
        let owner = EnrollmentSigner::open(&provider.signer_private_key, &provider).unwrap();
        let alternate = provider
            .signer_private_key
            .with_file_name("alternate.scalar");
        match case {
            0 => {
                std::fs::rename(&provider.signer_private_key, &alternate).unwrap();
                std::fs::write(&provider.signer_private_key, [2; 32]).unwrap();
                std::fs::set_permissions(
                    &provider.signer_private_key,
                    std::fs::Permissions::from_mode(0o600),
                )
                .unwrap();
            }
            1 => std::fs::remove_file(&provider.signer_private_key).unwrap(),
            2 => std::fs::set_permissions(
                &provider.signer_private_key,
                std::fs::Permissions::from_mode(0o644),
            )
            .unwrap(),
            3 => {
                std::fs::rename(&provider.signer_private_key, &alternate).unwrap();
                symlink(&alternate, &provider.signer_private_key).unwrap();
            }
            4 => std::fs::hard_link(&provider.signer_private_key, &alternate).unwrap(),
            _ => std::fs::write(&provider.signer_private_key, [3; 32]).unwrap(),
        }
        assert!(owner.revalidate(&provider).is_err());
        assert!(owner.sign(&provider, &[19; 32]).is_err());
        if case == 1 {
            assert!(matches!(
                EnrollmentSigner::open(&provider.signer_private_key, &provider),
                Err(Error::Unavailable)
            ));
        } else if case >= 2 && case <= 4 {
            assert!(EnrollmentSigner::open(&provider.signer_private_key, &provider).is_err());
        }
    }
}

#[test]
fn scalar_owners_zeroize_on_drop_and_errors_do_not_contain_originals() {
    fn zeroizes<T: zeroize::ZeroizeOnDrop>() {}
    zeroizes::<SigningKey>();
    zeroizes::<Zeroizing<[u8; 32]>>();
    for kind in [
        io::ErrorKind::NotFound,
        io::ErrorKind::TimedOut,
        io::ErrorKind::Interrupted,
    ] {
        let error = custody(io::Error::new(kind, "sensitive credential payload"));
        assert!(matches!(error, Error::Unavailable));
        assert!(!error.to_string().contains("sensitive"));
    }
    assert!(matches!(
        custody(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private source"
        )),
        Error::Selection
    ));
}
