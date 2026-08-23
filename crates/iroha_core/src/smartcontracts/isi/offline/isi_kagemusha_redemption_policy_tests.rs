// Focused release-scoped device-policy redemption regressions.

#[test]
fn release_scoped_redemption_survives_singleton_policy_rotation() {
    offline_test_transaction!(state_transaction);
    let asset = offline_test_asset(&ALICE_ID).definition().clone();
    let assertion_key = online_assertion_signing_key(0x68);
    let registration = android_online_registration(
        &ALICE_ID,
        &asset,
        &assertion_key,
        POLICY_TEST_TIME_MS + 60_000,
    );
    let authorization = android_online_authorization(&registration, &assertion_key);
    install_android_online_registration(&mut state_transaction, registration);
    let release_policy = effective_offline_device_attestation_policy(&state_transaction)
        .expect("installed release policy");
    let mut rotated = release_policy.clone();
    rotated
        .android_apps
        .push(OfflineAndroidAppAttestationPolicy {
            package_name: "com.pk.secondwallet".to_owned(),
            signing_certificate_sha256: vec![vec![0x56; 32]],
        });
    state_transaction.world.smart_contract_state.insert(
        (*OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY).clone(),
        norito::to_bytes(&rotated).expect("rotated policy must encode"),
    );

    let topup_error =
        ensure_registered_kagemusha_v2_device(&authorization, &asset, &state_transaction)
            .err()
            .expect("new issuance must remain bound to the current singleton policy");
    assert!(
        topup_error
            .to_string()
            .contains("attestation_policy_changed")
    );
    let (_, replay) = authenticate_kagemusha_v4_redeem_submission_before_replay(
        &ALICE_ID,
        &asset,
        &ALICE_ID,
        &authorization,
        &release_policy,
        &state_transaction,
    )
    .expect("an app-only rotation preserves the release registration trust basis");
    assert!(matches!(replay, KagemushaV4ReplayStatus::Fresh(_)));
}

#[test]
fn release_scoped_registration_rejects_incompatible_live_trust_rotation() {
    offline_test_transaction!(state_transaction);
    let asset = offline_test_asset(&ALICE_ID).definition().clone();
    let assertion_key = online_assertion_signing_key(0x6A);
    let registration = android_online_registration(
        &ALICE_ID,
        &asset,
        &assertion_key,
        POLICY_TEST_TIME_MS + 60_000,
    );
    let authorization = android_online_authorization(&registration, &assertion_key);
    install_android_online_registration(&mut state_transaction, registration);
    let release_policy = effective_offline_device_attestation_policy(&state_transaction)
        .expect("installed release policy");
    let mut emergency_policy = release_policy.clone();
    emergency_policy
        .revoked_certificate_tbs_sha256
        .push(vec![0xA8; 32]);
    state_transaction.world.smart_contract_state.insert(
        (*OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY).clone(),
        norito::to_bytes(&emergency_policy).expect("emergency policy must encode"),
    );

    let error = authenticate_kagemusha_v4_redeem_submission_before_replay(
        &ALICE_ID,
        &asset,
        &ALICE_ID,
        &authorization,
        &release_policy,
        &state_transaction,
    )
    .err()
    .expect("an old compact registration cannot bypass a live emergency trust update");
    assert!(error.to_string().contains("attestation_policy_changed"));
}

#[test]
fn release_scoped_redemption_accepts_only_compatible_current_policy_reregistration() {
    offline_test_transaction!(state_transaction);
    let asset = offline_test_asset(&ALICE_ID).definition().clone();
    let assertion_key = online_assertion_signing_key(0x69);
    let registration = android_online_registration(
        &ALICE_ID,
        &asset,
        &assertion_key,
        POLICY_TEST_TIME_MS + 60_000,
    );
    let authorization = android_online_authorization(&registration, &assertion_key);
    let mut release_policy =
        default_offline_device_attestation_policy().expect("built-in attestation roots");
    release_policy.require_android_app_policy = true;
    release_policy.android_status_snapshot = Some(android_status_snapshot());
    release_policy.android_apps = vec![OfflineAndroidAppAttestationPolicy {
        package_name: "com.pk.retailwallet".to_owned(),
        signing_certificate_sha256: vec![vec![0x55; 32]],
    }];
    let mut current_policy = release_policy.clone();
    current_policy
        .android_apps
        .push(OfflineAndroidAppAttestationPolicy {
            package_name: "com.pk.secondwallet".to_owned(),
            signing_certificate_sha256: vec![vec![0x56; 32]],
        });
    let current_policy_hash = canonical_offline_device_attestation_policy_hash(&current_policy)
        .expect("current policy hash");
    state_transaction.world.smart_contract_state.insert(
        (*OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY).clone(),
        norito::to_bytes(&current_policy).expect("current policy must encode"),
    );
    install_android_online_registration_with_policy_hash(
        &mut state_transaction,
        registration,
        current_policy_hash,
    );

    authenticate_kagemusha_v4_redeem_submission_before_replay(
        &ALICE_ID,
        &asset,
        &ALICE_ID,
        &authorization,
        &release_policy,
        &state_transaction,
    )
    .expect("a current registration with the exact historical trust basis remains usable");

    let mut stricter_release_policy = release_policy;
    stricter_release_policy
        .revoked_certificate_tbs_sha256
        .push(vec![0xA9; 32]);
    let error = authenticate_kagemusha_v4_redeem_submission_before_replay(
        &ALICE_ID,
        &asset,
        &ALICE_ID,
        &authorization,
        &stricter_release_policy,
        &state_transaction,
    )
    .err()
    .expect("a current admission cannot replace a stricter historical trust basis");
    assert!(error.to_string().contains("attestation_policy_changed"));
}
