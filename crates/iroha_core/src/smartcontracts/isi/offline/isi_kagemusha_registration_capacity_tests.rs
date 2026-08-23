fn capacity_registration(
    account: &AccountId,
    index: usize,
    expires_at_ms: u64,
) -> OfflineDeviceAttestationRegistration {
    let asset = offline_test_asset(account);
    let assertion_key = online_assertion_signing_key(0x71);
    let mut registration =
        android_online_registration(account, asset.definition(), &assertion_key, expires_at_ms);
    let discriminator = u64::try_from(index)
        .expect("capacity fixture index fits u64")
        .to_be_bytes();
    registration.device_id = format!("capacity-device-{index:05}");
    registration.challenge_hash = Hash::new(discriminator);
    registration.attestation_report = [b"capacity-report:".as_slice(), &discriminator].concat();
    registration.attestation_report_hash = Hash::new(&registration.attestation_report);
    registration.evidence = [b"capacity-evidence:".as_slice(), &discriminator].concat();
    registration.evidence_hash = Hash::new(&registration.evidence);
    registration.recent_block_hash =
        Hash::new([b"capacity-recent-block:".as_slice(), &discriminator].concat());
    registration
}
fn install_capacity_registration(
    state_transaction: &mut StateTransaction<'_, '_>,
    account: &AccountId,
    index: usize,
    expires_at_ms: u64,
) -> (StatePath, [Hash; 4]) {
    let registration = capacity_registration(account, index, expires_at_ms);
    let registration_hash =
        canonical_registration_hash(&registration).expect("canonical capacity registration hash");
    let replay_keys = kagemusha_registration_replay_keys(&registration, &registration_hash);
    let state_key = install_android_online_registration(state_transaction, registration);
    install_capacity_registration_replay_keys(state_transaction, replay_keys);
    (state_key, replay_keys)
}
fn install_capacity_registration_with_policy_hash(
    state_transaction: &mut StateTransaction<'_, '_>,
    account: &AccountId,
    index: usize,
    expires_at_ms: u64,
    admission_policy_hash: [u8; 32],
) -> (OfflineDeviceAttestationRegistration, StatePath, [Hash; 4]) {
    let registration = capacity_registration(account, index, expires_at_ms);
    let (state_key, replay_keys) = install_registration_with_policy_hash_and_replay(
        state_transaction,
        registration.clone(),
        admission_policy_hash,
    );
    (registration, state_key, replay_keys)
}
fn install_registration_with_policy_hash_and_replay(
    state_transaction: &mut StateTransaction<'_, '_>,
    registration: OfflineDeviceAttestationRegistration,
    admission_policy_hash: [u8; 32],
) -> (StatePath, [Hash; 4]) {
    let registration_hash =
        canonical_registration_hash(&registration).expect("canonical capacity registration hash");
    let replay_keys = kagemusha_registration_replay_keys(&registration, &registration_hash);
    let state_key = install_android_online_registration_with_policy_hash(
        state_transaction,
        registration.clone(),
        admission_policy_hash,
    );
    install_capacity_registration_replay_keys(state_transaction, replay_keys);
    (state_key, replay_keys)
}
fn install_capacity_registration_replay_keys(
    state_transaction: &mut StateTransaction<'_, '_>,
    replay_keys: [Hash; 4],
) {
    for replay_key in replay_keys {
        assert!(
            state_transaction
                .world
                .kagemusha_replay_keys
                .insert(replay_key, ())
                .is_none(),
            "capacity fixture replay material must be unique",
        );
    }
}
#[test]
fn kagemusha_online_registration_capacity_bounds_are_exact() {
    for (global, account, expected) in [
        (
            KAGEMUSHA_ACTIVE_DEVICE_REGISTRATIONS_MAX_GLOBAL_V1,
            1,
            Ok(()),
        ),
        (
            KAGEMUSHA_ACTIVE_DEVICE_REGISTRATIONS_MAX_GLOBAL_V1 + 1,
            1,
            Err(KagemushaOnlineRegistrationCapacityErrorV1::Global),
        ),
        (
            KAGEMUSHA_ACTIVE_DEVICE_REGISTRATIONS_MAX_PER_ACCOUNT_V1,
            KAGEMUSHA_ACTIVE_DEVICE_REGISTRATIONS_MAX_PER_ACCOUNT_V1,
            Ok(()),
        ),
        (
            KAGEMUSHA_ACTIVE_DEVICE_REGISTRATIONS_MAX_PER_ACCOUNT_V1 + 1,
            KAGEMUSHA_ACTIVE_DEVICE_REGISTRATIONS_MAX_PER_ACCOUNT_V1 + 1,
            Err(KagemushaOnlineRegistrationCapacityErrorV1::Account),
        ),
    ] {
        assert_eq!(
            validate_kagemusha_online_registration_capacity_v1(global, account),
            expected,
        );
    }
    for (global, account, expected) in [
        (
            KAGEMUSHA_ONLINE_REGISTRATION_STATE_MAX_RETAINED_V4,
            1,
            Ok(()),
        ),
        (
            KAGEMUSHA_ONLINE_REGISTRATION_STATE_MAX_RETAINED_V4 + 1,
            1,
            Err(KagemushaOnlineRegistrationRetainedCapacityErrorV4::Global),
        ),
        (
            KAGEMUSHA_ONLINE_REGISTRATION_STATE_MAX_RETAINED_PER_ACCOUNT_V4,
            KAGEMUSHA_ONLINE_REGISTRATION_STATE_MAX_RETAINED_PER_ACCOUNT_V4,
            Ok(()),
        ),
        (
            KAGEMUSHA_ONLINE_REGISTRATION_STATE_MAX_RETAINED_PER_ACCOUNT_V4 + 1,
            KAGEMUSHA_ONLINE_REGISTRATION_STATE_MAX_RETAINED_PER_ACCOUNT_V4 + 1,
            Err(KagemushaOnlineRegistrationRetainedCapacityErrorV4::Account),
        ),
    ] {
        assert_eq!(
            validate_kagemusha_online_registration_retained_capacity_v4(global, account),
            expected,
        );
    }
}
#[test]
fn kagemusha_online_registration_compaction_bounds_worst_case_persistent_bytes() {
    let state = offline_test_state();
    let mut block = state.block(offline_test_header());
    let state_transaction = block.transaction();
    let asset = offline_test_asset(&ALICE_ID).definition().clone();
    let assertion_key = online_assertion_signing_key(0x71);
    let mut submitted = android_online_registration(
        &ALICE_ID,
        &asset,
        &assertion_key,
        POLICY_TEST_TIME_MS + 60_000,
    );
    submitted.attestation_report = vec![0xA5; OFFLINE_ATTESTATION_MAX_REPORT_BYTES];
    submitted.attestation_report_hash = Hash::new(&submitted.attestation_report);
    submitted.evidence = vec![0x5A; OFFLINE_ATTESTATION_MAX_EVIDENCE_BYTES];
    submitted.evidence_hash = Hash::new(&submitted.evidence);
    let submitted_before = submitted.clone();
    let original_registration_hash = canonical_registration_hash(&submitted)
        .map(|hash| exact_hash_bytes(&hash))
        .expect("maximum-size submitted registration hashes canonically");
    let compact = compact_kagemusha_registration_projection(&submitted);
    assert_eq!(
        submitted, submitted_before,
        "wire input must remain unchanged"
    );
    assert!(compact.attestation_report.is_empty());
    assert!(compact.evidence.is_empty());
    assert_eq!(
        compact.attestation_report_hash,
        submitted.attestation_report_hash
    );
    assert_eq!(compact.evidence_hash, submitted.evidence_hash);
    let registration_projection_hash = canonical_registration_hash(&compact)
        .map(|hash| exact_hash_bytes(&hash))
        .expect("compact registration projection hashes canonically");
    assert_ne!(registration_projection_hash, original_registration_hash);
    let policy_hash = [0x44; 32];
    let state = KagemushaOnlineRegistrationStateV4 {
        version: KAGEMUSHA_ONLINE_REGISTRATION_STATE_VERSION_V4,
        original_registration_hash,
        registration_projection_hash,
        admission_policy_hash: policy_hash,
        admission_height: state_transaction.block_height(),
        admission_transaction_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"maximum-compact-registration-test-transaction",
        )),
        registration: compact,
        lifecycle: KagemushaOnlineHardwareAssertionLifecycleV1::AndroidKeyMintUnused,
    };
    let archive = encode_kagemusha_online_registration_state_v4(&state)
        .expect("maximum raw evidence compacts below the persistent limit");
    assert!(archive.len() <= KAGEMUSHA_ONLINE_REGISTRATION_STATE_MAX_CANONICAL_BYTES_V4);
    assert_eq!(
        KAGEMUSHA_ONLINE_REGISTRATION_STATE_MAX_TOTAL_BYTES_V4,
        4 * 1024 * 1024,
        "registration-state value payloads must remain bounded to 4 MiB",
    );
    let state_key = kagemusha_online_registration_state_key(&original_registration_hash)
        .expect("canonical original registration state key");
    assert_eq!(
        decode_kagemusha_online_registration_state_v4(&state_key, &archive)
            .expect("compact V4 archive validates"),
        state,
    );

    let mut legacy_version = state.clone();
    legacy_version.version = 3;
    let legacy_archive =
        norito::encode_canonical(&legacy_version).expect("legacy-version fixture encodes");
    assert!(
        decode_kagemusha_online_registration_state_v4(&state_key, &legacy_archive).is_err(),
        "first-release compact state must not silently accept an older schema version",
    );
    let oversized = vec![0_u8; KAGEMUSHA_ONLINE_REGISTRATION_STATE_MAX_CANONICAL_BYTES_V4 + 1];
    assert!(
        decode_kagemusha_online_registration_state_v4(&state_key, &oversized).is_err(),
        "oversized state must fail before Norito decode",
    );
}
#[test]
fn kagemusha_online_registration_capacity_rejection_does_not_mutate_state() {
    offline_test_transaction!(state_transaction);
    let expires_at_ms = POLICY_TEST_TIME_MS + 60_000;
    for index in 0..KAGEMUSHA_ACTIVE_DEVICE_REGISTRATIONS_MAX_PER_ACCOUNT_V1 - 1 {
        install_capacity_registration(&mut state_transaction, &ALICE_ID, index, expires_at_ms);
    }
    let candidate = capacity_registration(
        &ALICE_ID,
        KAGEMUSHA_ACTIVE_DEVICE_REGISTRATIONS_MAX_PER_ACCOUNT_V1,
        expires_at_ms,
    );
    let policy_hash = current_offline_device_attestation_policy_from_world(
        &state_transaction.world,
        POLICY_TEST_TIME_MS,
    )
    .expect("capacity fixture policy is valid")
    .1;
    plan_kagemusha_online_registration_admission_v1(&candidate, policy_hash, &state_transaction)
        .expect("the candidate reaching the exact per-account limit must remain valid");

    install_capacity_registration(
        &mut state_transaction,
        &ALICE_ID,
        KAGEMUSHA_ACTIVE_DEVICE_REGISTRATIONS_MAX_PER_ACCOUNT_V1 - 1,
        expires_at_ms,
    );
    let state_before = state_transaction
        .world
        .smart_contract_state
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<Vec<_>>();
    let replay_before = state_transaction
        .world
        .kagemusha_replay_keys
        .iter()
        .map(|(key, ())| *key)
        .collect::<Vec<_>>();
    let error = plan_kagemusha_online_registration_admission_v1(
        &candidate,
        policy_hash,
        &state_transaction,
    )
    .expect_err("one registration above the per-account limit must fail");
    assert!(
        error
            .to_string()
            .contains("offline_reason::registration_capacity_exceeded"),
        "unexpected capacity rejection: {error}",
    );
    assert_eq!(
        state_transaction
            .world
            .smart_contract_state
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<Vec<_>>(),
        state_before,
        "capacity rejection must not mutate registration state",
    );
    assert_eq!(
        state_transaction
            .world
            .kagemusha_replay_keys
            .iter()
            .map(|(key, ())| *key)
            .collect::<Vec<_>>(),
        replay_before,
        "capacity rejection must not mutate replay protection",
    );
}
#[test]
fn kagemusha_online_registration_retains_expired_replay_tombstone_until_horizon() {
    offline_test_transaction!(state_transaction);
    let (expired_state_key, expired_replay_keys) =
        install_capacity_registration(&mut state_transaction, &ALICE_ID, 0, POLICY_TEST_TIME_MS);
    let candidate = capacity_registration(&ALICE_ID, 1, POLICY_TEST_TIME_MS + 60_000);
    let policy_hash = current_offline_device_attestation_policy_from_world(
        &state_transaction.world,
        POLICY_TEST_TIME_MS,
    )
    .expect("capacity fixture policy is valid")
    .1;
    let plan = plan_kagemusha_online_registration_admission_v1(
        &candidate,
        policy_hash,
        &state_transaction,
    )
    .expect("expired registration must release active capacity");
    assert!(plan.prunable.is_empty());
    plan.commit(&mut state_transaction);
    assert!(
        state_transaction
            .world
            .smart_contract_state
            .get(&expired_state_key)
            .is_some(),
        "recent expired registration must remain as a bounded replay tombstone",
    );
    for replay_key in expired_replay_keys {
        assert!(
            state_transaction
                .world
                .kagemusha_replay_keys
                .get(&replay_key)
                .is_some(),
            "recent expired replay marker must preserve one-use authority",
        );
    }
}
#[test]
fn kagemusha_online_registration_rotation_churn_is_bounded_until_replay_horizon() {
    let expires_at_ms = POLICY_TEST_TIME_MS + 60_000;
    {
        offline_test_transaction!(state_transaction);
        for index in 0..KAGEMUSHA_ACTIVE_DEVICE_REGISTRATIONS_MAX_PER_ACCOUNT_V1 {
            install_capacity_registration(&mut state_transaction, &ALICE_ID, index, expires_at_ms);
        }
        let (mut policy_b, policy_a_hash) = current_offline_device_attestation_policy_from_world(
            &state_transaction.world,
            POLICY_TEST_TIME_MS,
        )
        .expect("policy A remains valid at its active account capacity");
        policy_b.revoked_certificate_tbs_sha256.push(vec![0xB4; 32]);
        let policy_b_hash = canonical_offline_device_attestation_policy_hash(&policy_b)
            .expect("policy B hashes canonically");
        state_transaction.world.smart_contract_state.insert(
            (*OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY).clone(),
            norito::to_bytes(&policy_b).expect("policy B encodes canonically"),
        );
        let first_b = capacity_registration(
            &ALICE_ID,
            KAGEMUSHA_ACTIVE_DEVICE_REGISTRATIONS_MAX_PER_ACCOUNT_V1,
            expires_at_ms,
        );
        let first_b_plan = plan_kagemusha_online_registration_admission_v1(
            &first_b,
            policy_b_hash,
            &state_transaction,
        )
        .expect("one full superseded cohort must leave room for policy B");
        assert!(first_b_plan.prunable.is_empty());
        assert_ne!(policy_a_hash, policy_b_hash);
        for index in KAGEMUSHA_ACTIVE_DEVICE_REGISTRATIONS_MAX_PER_ACCOUNT_V1
            ..KAGEMUSHA_ONLINE_REGISTRATION_STATE_MAX_RETAINED_PER_ACCOUNT_V4
        {
            install_capacity_registration_with_policy_hash(
                &mut state_transaction,
                &ALICE_ID,
                index,
                expires_at_ms,
                policy_b_hash,
            );
        }
        let mut policy_c = policy_b.clone();
        policy_c.revoked_certificate_tbs_sha256.push(vec![0xC5; 32]);
        let policy_c_hash = canonical_offline_device_attestation_policy_hash(&policy_c)
            .expect("policy C hashes canonically");
        state_transaction.world.smart_contract_state.insert(
            (*OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY).clone(),
            norito::to_bytes(&policy_c).expect("policy C encodes canonically"),
        );
        let third_wave = capacity_registration(
            &ALICE_ID,
            KAGEMUSHA_ONLINE_REGISTRATION_STATE_MAX_RETAINED_PER_ACCOUNT_V4,
            expires_at_ms,
        );
        let state_before = state_transaction
            .world
            .smart_contract_state
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<Vec<_>>();
        let replay_before = state_transaction
            .world
            .kagemusha_replay_keys
            .iter()
            .map(|(key, ())| *key)
            .collect::<Vec<_>>();
        let error = plan_kagemusha_online_registration_admission_v1(
            &third_wave,
            policy_c_hash,
            &state_transaction,
        )
        .expect_err("a third recent cohort must exceed retained per-account capacity");
        assert!(error.to_string().contains("registration_capacity_exceeded"));
        assert_eq!(
            state_transaction
                .world
                .smart_contract_state
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect::<Vec<_>>(),
            state_before,
        );
        assert_eq!(
            state_transaction
                .world
                .kagemusha_replay_keys
                .iter()
                .map(|(key, ())| *key)
                .collect::<Vec<_>>(),
            replay_before,
        );
        let replayed = capacity_registration(&ALICE_ID, 0, expires_at_ms);
        let replayed_hash = canonical_registration_hash(&replayed).expect("replay hashes");
        let replayed_keys = kagemusha_registration_replay_keys(&replayed, &replayed_hash);
        ensure_kagemusha_registration_replay_keys_are_fresh(&replayed_keys, &state_transaction)
            .expect_err("identical attestation material must remain replay-protected");
    }

    let state = offline_test_state();
    let mut block = state.block(offline_test_header());
    for height in 0..130_u64 {
        block
            .block_hashes
            .push(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                height.to_le_bytes(),
            )));
    }
    let mut state_transaction = block.transaction();
    let mut old_state_keys = Vec::new();
    let mut old_replay_keys = Vec::new();
    for index in 0..KAGEMUSHA_ACTIVE_DEVICE_REGISTRATIONS_MAX_PER_ACCOUNT_V1 {
        let (state_key, replay_keys) = install_capacity_registration(
            &mut state_transaction,
            &ALICE_ID,
            index,
            POLICY_TEST_TIME_MS,
        );
        old_state_keys.push(state_key);
        old_replay_keys.extend(replay_keys);
    }
    let (mut policy_b, _) = current_offline_device_attestation_policy_from_world(
        &state_transaction.world,
        POLICY_TEST_TIME_MS,
    )
    .expect("policy A is installed");
    policy_b.revoked_certificate_tbs_sha256.push(vec![0xB4; 32]);
    let policy_b_hash = canonical_offline_device_attestation_policy_hash(&policy_b)
        .expect("policy B hashes canonically");
    for index in KAGEMUSHA_ACTIVE_DEVICE_REGISTRATIONS_MAX_PER_ACCOUNT_V1
        ..KAGEMUSHA_ONLINE_REGISTRATION_STATE_MAX_RETAINED_PER_ACCOUNT_V4
    {
        let mut recent = capacity_registration(&ALICE_ID, index, expires_at_ms);
        recent.recent_block_height = 130;
        recent.recent_block_hash = Hash::new(b"recent-policy-b-block");
        install_registration_with_policy_hash_and_replay(
            &mut state_transaction,
            recent,
            policy_b_hash,
        );
    }
    let mut policy_c = policy_b;
    policy_c.revoked_certificate_tbs_sha256.push(vec![0xC5; 32]);
    let policy_c_hash = canonical_offline_device_attestation_policy_hash(&policy_c)
        .expect("policy C hashes canonically");
    state_transaction.world.smart_contract_state.insert(
        (*OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY).clone(),
        norito::to_bytes(&policy_c).expect("policy C encodes canonically"),
    );
    let mut fresh_c = capacity_registration(
        &ALICE_ID,
        KAGEMUSHA_ONLINE_REGISTRATION_STATE_MAX_RETAINED_PER_ACCOUNT_V4,
        expires_at_ms,
    );
    fresh_c.recent_block_height = 130;
    fresh_c.recent_block_hash = Hash::new(b"recent-policy-c-block");
    let old_registration = capacity_registration(&ALICE_ID, 0, POLICY_TEST_TIME_MS);
    assert!(kagemusha_registration_replay_horizon_elapsed(
        &old_registration,
        130,
    ));
    let stale_error =
        validate_offline_attestation_recent_block(&old_registration, &state_transaction)
            .expect_err("the old cohort must be permanently stale after the replay horizon");
    assert!(stale_error.to_string().contains("stale_attestation"));
    let old_hash = canonical_registration_hash(&old_registration).expect("old hash");
    let old_keys = kagemusha_registration_replay_keys(&old_registration, &old_hash);
    ensure_kagemusha_registration_replay_keys_are_fresh(&old_keys, &state_transaction)
        .expect_err("old replay markers remain until the pruning commit");
    let plan = plan_kagemusha_online_registration_admission_v1(
        &fresh_c,
        policy_c_hash,
        &state_transaction,
    )
    .expect("elapsed first-wave tombstones must make room for a fresh cohort");
    assert_eq!(
        plan.prunable.len(),
        KAGEMUSHA_ACTIVE_DEVICE_REGISTRATIONS_MAX_PER_ACCOUNT_V1,
    );
    plan.commit(&mut state_transaction);
    for state_key in old_state_keys {
        assert!(
            state_transaction
                .world
                .smart_contract_state
                .get(&state_key)
                .is_none(),
        );
    }
    for replay_key in old_replay_keys {
        assert!(
            state_transaction
                .world
                .kagemusha_replay_keys
                .get(&replay_key)
                .is_none(),
        );
    }
    let (fresh_state_key, _) = install_registration_with_policy_hash_and_replay(
        &mut state_transaction,
        fresh_c,
        policy_c_hash,
    );
    assert!(
        state_transaction
            .world
            .smart_contract_state
            .get(&fresh_state_key)
            .is_some(),
        "fresh policy C registration must admit after safe tombstone pruning",
    );
}
