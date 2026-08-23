fn release_activation_device_policy() -> OfflineDeviceAttestationPolicy {
    let mut policy = default_offline_device_attestation_policy()
        .expect("built-in roots form a valid activation-policy template");
    policy.require_ios_app_policy = true;
    policy.require_android_app_policy = true;
    policy.ios_apps = vec![ios_assertion_policy()];
    policy.android_apps = vec![OfflineAndroidAppAttestationPolicy {
        package_name: "com.pk.retailwallet".to_owned(),
        signing_certificate_sha256: vec![vec![0x55; 32]],
    }];
    policy.android_status_snapshot = Some(android_status_snapshot());
    policy
}
#[test]
fn release_activation_device_policy_is_production_and_fail_closed() {
    let policy = release_activation_device_policy();
    validate_offline_attestation_policy_for_release_activation(&policy, POLICY_TEST_TIME_MS)
        .expect("exact production policy must be activation-eligible");
    let mut missing_android_gate = policy.clone();
    missing_android_gate.require_android_app_policy = false;
    assert!(
        validate_offline_attestation_policy_for_release_activation(
            &missing_android_gate,
            POLICY_TEST_TIME_MS,
        )
        .is_err(),
        "activation must not publish an Android fail-open policy",
    );
    let mut development_ios = policy.clone();
    development_ios.ios_apps[0].environment = "development".to_owned();
    assert!(
        validate_offline_attestation_policy_for_release_activation(
            &development_ios,
            POLICY_TEST_TIME_MS,
        )
        .is_err(),
        "activation must not publish a development App Attest policy",
    );
    let mut control_character_ios = policy.clone();
    control_character_ios.ios_apps[0].bundle_id = "io.soramitsu.\npk".to_owned();
    assert!(
        validate_offline_attestation_policy_for_release_activation(
            &control_character_ios,
            POLICY_TEST_TIME_MS,
        )
        .is_err(),
        "activation must reject control characters in application identities",
    );
    let mut substituted_ios_root = policy.clone();
    let apple_index = substituted_ios_root
        .trusted_roots
        .iter()
        .position(|root| root.platform == OFFLINE_ATTESTATION_PLATFORM_IOS_APP_ATTEST)
        .expect("production policy contains an Apple root");
    let legacy_google_der = decode_trusted_root_der(ANDROID_KEY_ATTESTATION_ROOT_CA_DER_B64)
        .expect("decode embedded legacy Google root");
    let legacy_google_index = substituted_ios_root
        .trusted_roots
        .iter()
        .position(|root| {
            root.platform == OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT
                && root.der == legacy_google_der
        })
        .expect("production policy contains the legacy Google root");
    let apple_der = substituted_ios_root.trusted_roots[apple_index].der.clone();
    substituted_ios_root.trusted_roots[apple_index].der = legacy_google_der;
    substituted_ios_root.trusted_roots[legacy_google_index].der = apple_der;
    let error = validate_offline_attestation_policy_for_release_activation(
        &substituted_ios_root,
        POLICY_TEST_TIME_MS,
    )
    .expect_err("activation must pin the exact Apple App Attest root");
    assert!(
        error
            .to_string()
            .contains("exact current Apple App Attest root")
    );

    for platform in [
        OFFLINE_ATTESTATION_PLATFORM_IOS_APP_ATTEST,
        OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT,
    ] {
        for inactive_at in [POLICY_TEST_TIME_MS - 1, POLICY_TEST_TIME_MS + 1] {
            let mut inactive_root = policy.clone();
            let root = inactive_root
                .trusted_roots
                .iter_mut()
                .find(|root| root.platform == platform)
                .expect("production policy contains the platform root");
            if inactive_at < POLICY_TEST_TIME_MS {
                root.not_after_ms = Some(inactive_at);
            } else {
                root.not_before_ms = Some(inactive_at);
            }
            assert!(
                validate_offline_attestation_policy_for_release_activation(
                    &inactive_root,
                    POLICY_TEST_TIME_MS,
                )
                .is_err(),
                "activation must require every exact production root to be governance-active",
            );
        }
    }
    let mut maximum_revocations = policy;
    maximum_revocations.revoked_certificate_tbs_sha256 = (1
        ..=OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_REVOKED_CERTIFICATES_V1)
        .map(|index| {
            let mut digest = [0_u8; 32];
            digest[..8].copy_from_slice(&(index as u64).to_le_bytes());
            digest.to_vec()
        })
        .collect();
    validate_offline_attestation_policy_for_release_activation(
        &maximum_revocations,
        POLICY_TEST_TIME_MS,
    )
    .expect("the exact revocation-list limit remains activation eligible");
    maximum_revocations
        .revoked_certificate_tbs_sha256
        .push(vec![0xA5; 32]);
    assert!(
        validate_offline_attestation_policy_for_release_activation(
            &maximum_revocations,
            POLICY_TEST_TIME_MS,
        )
        .is_err(),
        "activation must reject a policy above the revocation-list limit",
    );
}
#[test]
fn production_device_policy_constructor_binds_explicit_apps_and_builtin_roots() {
    let policy = production_offline_device_attestation_policy_v1(
        "TEAMID1234".to_owned(),
        "io.soramitsu.pk".to_owned(),
        vec![10, 4],
        vec!["42".to_owned(), "41".to_owned()],
        "com.pk.retailwallet".to_owned(),
        vec![[0x66; 32], [0x55; 32]],
        android_status_snapshot(),
        POLICY_TEST_TIME_MS,
    )
    .expect("explicit production app identities should build a fail-closed policy");
    assert_eq!(policy.trusted_roots.len(), 3);
    assert!(policy.require_ios_app_policy);
    assert!(policy.require_android_app_policy);
    assert_eq!(
        policy.ios_apps[0].allowed_validation_categories,
        vec![4, 10]
    );
    assert_eq!(
        policy.ios_apps[0].allowed_bundle_versions,
        vec!["41".to_owned(), "42".to_owned()]
    );
    assert_eq!(
        policy.android_apps[0].signing_certificate_sha256,
        vec![vec![0x55; 32], vec![0x66; 32]]
    );
}
#[test]
fn production_device_policy_constructor_rejects_duplicate_operator_input() {
    let error = production_offline_device_attestation_policy_v1(
        "TEAMID1234".to_owned(),
        "io.soramitsu.pk".to_owned(),
        vec![4, 4],
        vec!["42".to_owned()],
        "com.pk.retailwallet".to_owned(),
        vec![[0x55; 32]],
        android_status_snapshot(),
        POLICY_TEST_TIME_MS,
    )
    .expect_err("duplicate policy input must not be silently normalized");
    assert!(error.contains("must not contain duplicates"));
}
#[test]
fn offline_device_attestation_policy_shape_bounds_are_exact() {
    let baseline = default_offline_device_attestation_policy()
        .expect("built-in roots form a valid policy template");

    let mut roots = baseline.clone();
    roots.trusted_roots = (0..OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOTS_V1)
        .map(|index| baseline.trusted_roots[index % 2].clone())
        .collect();
    validate_offline_attestation_policy_bounds(&roots)
        .expect("the exact total and per-platform root limits are admitted");
    roots.trusted_roots.push(baseline.trusted_roots[0].clone());
    assert!(validate_offline_attestation_policy_bounds(&roots).is_err());

    let mut platform_roots = baseline.clone();
    platform_roots.trusted_roots = vec![
            baseline.trusted_roots[0].clone();
            OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOTS_PER_PLATFORM_V1
        ];
    validate_offline_attestation_policy_bounds(&platform_roots)
        .expect("the exact per-platform root limit is admitted");
    platform_roots
        .trusted_roots
        .push(baseline.trusted_roots[0].clone());
    assert!(validate_offline_attestation_policy_bounds(&platform_roots).is_err());

    let mut root_der = baseline.clone();
    root_der.trusted_roots = vec![baseline.trusted_roots[0].clone()];
    root_der.trusted_roots[0].der =
        vec![0xA5; OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOT_DER_BYTES_V1];
    validate_offline_attestation_policy_bounds(&root_der)
        .expect("the exact trusted-root DER limit is admitted");
    root_der.trusted_roots[0].der.push(0xA5);
    assert!(validate_offline_attestation_policy_bounds(&root_der).is_err());

    let mut revoked = baseline.clone();
    revoked.revoked_certificate_tbs_sha256 =
        vec![vec![0xA5; 32]; OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_REVOKED_CERTIFICATES_V1];
    validate_offline_attestation_policy_bounds(&revoked)
        .expect("the exact revocation-list limit is admitted");
    revoked.revoked_certificate_tbs_sha256.push(vec![0x5A; 32]);
    assert!(validate_offline_attestation_policy_bounds(&revoked).is_err());

    let ios_app = ios_assertion_policy();
    let mut ios_apps = baseline.clone();
    ios_apps.ios_apps = vec![ios_app.clone(); OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_APPS_V1];
    validate_offline_attestation_policy_bounds(&ios_apps)
        .expect("the exact iOS app-count limit is admitted");
    ios_apps.ios_apps.push(ios_app.clone());
    assert!(validate_offline_attestation_policy_bounds(&ios_apps).is_err());

    let android_app = OfflineAndroidAppAttestationPolicy {
        package_name: "com.example.boundary".to_owned(),
        signing_certificate_sha256: vec![vec![0x5A; 32]],
    };
    let mut android_apps = baseline.clone();
    android_apps.android_apps =
        vec![android_app.clone(); OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_ANDROID_APPS_V1];
    validate_offline_attestation_policy_bounds(&android_apps)
        .expect("the exact Android app-count limit is admitted");
    android_apps.android_apps.push(android_app.clone());
    assert!(validate_offline_attestation_policy_bounds(&android_apps).is_err());

    let mut ios_nested = baseline.clone();
    ios_nested.ios_apps = vec![ios_app];
    ios_nested.ios_apps[0].team_id =
        "T".repeat(OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TEAM_ID_BYTES_V1);
    ios_nested.ios_apps[0].bundle_id =
        "b".repeat(OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_APP_IDENTIFIER_BYTES_V1);
    ios_nested.ios_apps[0].allowed_validation_categories = vec![1, 2, 3, 4, 5, 6, 10];
    ios_nested.ios_apps[0].allowed_bundle_versions = (0
        ..OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_BUNDLE_VERSIONS_V1)
        .map(|index| index.to_string())
        .collect();
    ios_nested.ios_apps[0].allowed_bundle_versions[0] =
        "v".repeat(OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_BUNDLE_VERSION_BYTES_V1);
    validate_offline_attestation_policy_bounds(&ios_nested)
        .expect("the exact nested iOS limits are admitted");
    ios_nested.ios_apps[0].team_id.push('T');
    assert!(validate_offline_attestation_policy_bounds(&ios_nested).is_err());
    ios_nested.ios_apps[0].team_id.pop();
    ios_nested.ios_apps[0].bundle_id.push('b');
    assert!(validate_offline_attestation_policy_bounds(&ios_nested).is_err());
    ios_nested.ios_apps[0].bundle_id.pop();
    ios_nested.ios_apps[0]
        .allowed_validation_categories
        .push(10);
    assert!(validate_offline_attestation_policy_bounds(&ios_nested).is_err());
    ios_nested.ios_apps[0].allowed_validation_categories.pop();
    ios_nested.ios_apps[0]
        .allowed_bundle_versions
        .push("overflow".to_owned());
    assert!(validate_offline_attestation_policy_bounds(&ios_nested).is_err());
    ios_nested.ios_apps[0].allowed_bundle_versions.pop();
    ios_nested.ios_apps[0].allowed_bundle_versions[0].push('v');
    assert!(validate_offline_attestation_policy_bounds(&ios_nested).is_err());

    let mut android_nested = baseline.clone();
    android_nested.android_apps = vec![android_app];
    android_nested.android_apps[0].package_name =
        "p".repeat(OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_APP_IDENTIFIER_BYTES_V1);
    android_nested.android_apps[0].signing_certificate_sha256 =
        vec![vec![0x5A; 32]; OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_ANDROID_SIGNING_CERTIFICATES_V1];
    validate_offline_attestation_policy_bounds(&android_nested)
        .expect("the exact nested Android limits are admitted");
    android_nested.android_apps[0].package_name.push('p');
    assert!(validate_offline_attestation_policy_bounds(&android_nested).is_err());
    android_nested.android_apps[0].package_name.pop();
    android_nested.android_apps[0]
        .signing_certificate_sha256
        .push(vec![0xA5; 32]);
    assert!(validate_offline_attestation_policy_bounds(&android_nested).is_err());

    let mut canonical = baseline;
    canonical.trusted_roots = vec![canonical.trusted_roots[0].clone(); 4];
    for root in &mut canonical.trusted_roots {
        root.der = vec![0xA5; OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOT_DER_BYTES_V1];
    }
    while norito::encode_canonical(&canonical)
        .expect("boundary policy encodes")
        .len()
        > OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_CANONICAL_BYTES_V1
    {
        canonical.trusted_roots[3]
            .der
            .pop()
            .expect("four maximum roots exceed the canonical policy limit");
    }
    assert_eq!(
        norito::encode_canonical(&canonical)
            .expect("exact-boundary policy encodes")
            .len(),
        OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_CANONICAL_BYTES_V1,
    );
    validate_offline_attestation_policy_bounds(&canonical)
        .expect("the exact canonical policy limit is admitted");
    canonical.trusted_roots[3].der.push(0xA5);
    assert!(validate_offline_attestation_policy_bounds(&canonical).is_err());
}
