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

#[test]
fn torii_hardware_preflight_rejects_a_signature_from_an_unregistered_key() {
    offline_test_transaction!(state_transaction);
    let asset = offline_test_asset(&ALICE_ID).definition().clone();
    let assertion_key = online_assertion_signing_key(0x6B);
    let registration = android_online_registration(
        &ALICE_ID,
        &asset,
        &assertion_key,
        POLICY_TEST_TIME_MS + 60_000,
    );
    let mut authorization = android_online_authorization(&registration, &assertion_key);
    install_android_online_registration(&mut state_transaction, registration);

    preflight_registered_kagemusha_v2_hardware_authorization(
        &state_transaction.world,
        &authorization,
        &asset,
        POLICY_TEST_TIME_MS,
    )
    .expect("the registered hardware key must pass Torii preflight");
    let expiry_error = preflight_registered_kagemusha_v2_hardware_authorization(
        &state_transaction.world,
        &authorization,
        &asset,
        authorization.expires_at_ms,
    )
    .expect_err("the exclusive authorization expiry must fail Torii preflight");
    assert!(
        expiry_error.contains("not live"),
        "unexpected expiry error: {expiry_error}"
    );

    let attacker_key = online_assertion_signing_key(0x6C);
    let signing_bytes = authorization
        .signing_bytes()
        .expect("canonical hardware authorization preimage");
    authorization.set_hardware_signature(online_assertion_signature(
        &attacker_key,
        &signing_bytes,
    ));
    let error = preflight_registered_kagemusha_v2_hardware_authorization(
        &state_transaction.world,
        &authorization,
        &asset,
        POLICY_TEST_TIME_MS,
    )
    .expect_err("an unregistered hardware key must fail before Torii sponsors a transaction");
    assert!(error.contains("signature"), "unexpected preflight error: {error}");
}

#[test]
fn torii_hardware_preflight_rejects_a_consumed_android_registration() {
    offline_test_transaction!(state_transaction);
    let asset = offline_test_asset(&ALICE_ID).definition().clone();
    let assertion_key = online_assertion_signing_key(0x6D);
    let registration = android_online_registration(
        &ALICE_ID,
        &asset,
        &assertion_key,
        POLICY_TEST_TIME_MS + 60_000,
    );
    let authorization = android_online_authorization(&registration, &assertion_key);
    let state_key = install_android_online_registration(&mut state_transaction, registration);
    let archive = state_transaction
        .world
        .smart_contract_state
        .get(&state_key)
        .expect("installed registration state")
        .clone();
    let mut state = decode_kagemusha_online_registration_state_v4(&state_key, &archive)
        .expect("canonical registration state");
    state.lifecycle = KagemushaOnlineHardwareAssertionLifecycleV1::AndroidKeyMintConsumed(
        assertion_consumption(&authorization).expect("canonical assertion consumption"),
    );
    state_transaction.world.smart_contract_state.insert(
        state_key,
        encode_kagemusha_online_registration_state_v4(&state)
            .expect("canonical consumed registration state"),
    );

    let error = preflight_registered_kagemusha_v2_hardware_authorization(
        &state_transaction.world,
        &authorization,
        &asset,
        POLICY_TEST_TIME_MS,
    )
    .expect_err("a consumed Android registration must fail before transaction sponsorship");
    assert!(error.contains("consumed"), "unexpected preflight error: {error}");
}
