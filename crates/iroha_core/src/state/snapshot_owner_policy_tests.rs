state_test! { sync startup_sumeragi_key_policy_matches_canonical_state_without_mutation
    let mut state = blank_test_state();
    let mut canonical = SumeragiPolicyConfig::from(
        &iroha_data_model::parameter::system::SumeragiParameters::default(),
    );
    let before = norito::json::to_value(&state).expect("canonical default State");
    state.validate_sumeragi_key_policy(canonical.clone()).expect("default policy matches");
    assert_eq!(norito::json::to_value(&state).expect("State after validation"), before);
    canonical.key_activation_lead_blocks = 5;
    canonical.key_overlap_grace_blocks = 13;
    canonical.key_expiry_grace_blocks = 2;
    let _ = canonical.key_allowed_algorithms.insert(Algorithm::Ed25519);
    state.set_sumeragi_parameters(canonical.clone());
    let before = norito::json::to_value(&state).expect("canonical nondefault fixture State");
    state.validate_sumeragi_key_policy(canonical.clone()).expect("retained nondefault policy matches");
    assert_eq!(norito::json::to_value(&state).expect("State after validation"), before);
    let hash = crate::snapshot::canonical_state_snapshot_hash(&state);
    {
        let mut parameters = state.world.parameters.block();
        parameters.sumeragi.key_allowed_algorithms.reverse();
        parameters.commit();
    }
    let before = norito::json::to_value(&state).expect("equivalent algorithm order");
    state.validate_sumeragi_key_policy(canonical).expect("algorithm order is not policy");
    assert_eq!(norito::json::to_value(&state).expect("State after validation"), before);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(&state), hash);
}

state_test! { sync startup_sumeragi_key_policy_rejects_each_mismatch_without_mutation
    let state = blank_test_state();
    let canonical = SumeragiPolicyConfig::from(
        &iroha_data_model::parameter::system::SumeragiParameters::default(),
    );
    let before = norito::json::to_value(&state).expect("canonical State");
    for field in [
        "key_activation_lead_blocks", "key_overlap_grace_blocks",
        "key_expiry_grace_blocks", "key_allowed_algorithms",
    ] {
        let mut configured = canonical.clone();
        match field {
            "key_activation_lead_blocks" => configured.key_activation_lead_blocks += 1,
            "key_overlap_grace_blocks" => configured.key_overlap_grace_blocks += 1,
            "key_expiry_grace_blocks" => configured.key_expiry_grace_blocks += 1,
            "key_allowed_algorithms" => {
                let _ = configured.key_allowed_algorithms.insert(Algorithm::Ed25519);
            }
            _ => unreachable!("fixed policy field list"),
        }
        assert_eq!(state.validate_sumeragi_key_policy(configured), Err(field));
        assert_eq!(norito::json::to_value(&state).expect("State after rejection"), before);
    }
}

fn snapshot_owner_policy_fixture() -> (
    tempfile::TempDir,
    State,
    iroha_config::parameters::actual::Nexus,
) {
    let directory = tempfile::tempdir().expect("owner policy fixture directory");
    let dataspace = DataSpaceId::new(9);
    let catalog = LaneCatalog::new(
        nonzero!(4_u32),
        vec![
            LaneConfig::default(),
            LaneConfig {
                id: LaneId::new(2),
                alias: "restricted-staking-sibling".to_owned(),
                dataspace_id: dataspace,
                visibility: LaneVisibility::Restricted,
                ..LaneConfig::default()
            },
            LaneConfig {
                id: LaneId::new(3),
                alias: "public-staking-owner".to_owned(),
                dataspace_id: dataspace,
                ..LaneConfig::default()
            },
        ],
    )
    .expect("shared nondefault dataspace catalog");
    let mut configured = startup_nexus_for_catalog(catalog.clone());
    configured.dataspace_catalog = DataSpaceCatalog::new(vec![
        DataSpaceMetadata::default(),
        DataSpaceMetadata {
            id: dataspace,
            alias: "shared-staking".to_owned(),
            description: Some("snapshot ownership fixture".to_owned()),
            fault_tolerance: 2,
        },
    ])
    .expect("nondefault dataspace policy");
    configured.staking.max_validators = nonzero!(7_u32);
    configured.autoscale.enabled = false;
    configured.autoscale.min_lane_id = nonzero!(16_u32);
    configured.autoscale.max_lane_id_exclusive = nonzero!(32_u32);
    let (kura, mut state) =
        authenticated_startup_state_for_testing(directory.path().join("kura"), &catalog);
    state
        .set_nexus_from_config(configured.clone())
        .expect("install configured owner policy before genesis");
    let (validator, keypair) = bls_account_in("snapshot-owner");
    insert_active_public_lane_validator_for_test(
        &state,
        LaneId::new(3),
        &validator,
        &keypair,
        1_000_000,
    );
    seed_committed_height_for_state_test(&state, 5);
    seed_autoscale_sample_history_for_snapshot_test(&state);
    let value = norito::json::to_value(&state).expect("serialize live staking snapshot");
    let expected_runtime = value
        .as_object()
        .expect("State object")
        .get("nexus_runtime")
        .expect("snapshot runtime")
        .clone();
    let restored = deserialize_state_snapshot_value_with_kura(value, Arc::clone(&kura))
        .expect("restore actual prior owner policy before startup reconciliation");
    let roundtrip = norito::json::to_value(&restored).expect("reserialize restored State");
    assert_eq!(
        roundtrip
            .as_object()
            .expect("State object")
            .get("nexus_runtime"),
        Some(&expected_runtime),
        "snapshot decoding must not normalize the committed owner policy"
    );
    (directory, restored, configured)
}

state_test! { sync snapshot_owner_policy_survives_startup_with_live_nondefault_staking
    let (_directory, mut restored, configured) = snapshot_owner_policy_fixture();
    let owner = LaneId::new(3);
    let before_policy = SnapshotNexusOwnerPolicy::from_nexus(&restored.nexus_snapshot());
    assert_eq!(before_policy, SnapshotNexusOwnerPolicy::from_nexus(&configured));
    assert_eq!(
        nexus_staking_authority_lane_at_height(owner, &restored.nexus_snapshot(), 5),
        Some(owner),
        "the loaded snapshot already identifies the nondefault dataspace owner"
    );
    let before = norito::json::to_json(&restored).expect("capture restored custody");
    restored
        .prepare_restored_configured_primary_geometry_anchor(&configured.configured_lane_catalog)
        .expect("authenticate unchanged configured primary baseline");
    restored
        .restore_kura_lane_segments_from_nexus()
        .expect("restore snapshot-authenticated lane geometry");
    restored
        .set_nexus_from_config(configured.clone())
        .expect("same static policy must preserve live nondefault staking on restart");
    restored
        .set_nexus_from_config(configured)
        .expect("repeated same-policy installation is idempotent");
    assert_eq!(
        norito::json::to_json(&restored).expect("capture post-startup custody"),
        before,
        "startup must preserve the snapshot World, lineage and owner policy"
    );
}

state_test! { sync snapshot_owner_policy_rejects_changed_owner_before_and_after_hydration
    let (_directory, mut restored, configured) = snapshot_owner_policy_fixture();
    for hydrated in [false, true] {
        if hydrated {
            restored
                .set_nexus_from_config(configured.clone())
                .expect("install unchanged policy between negative controls");
        }
        for change_owner in [false, true] {
            let before = norito::json::to_json(&restored).expect("capture prior State");
            let journal = restored.kura.lane_geometry_journal_state_for_test()
                .expect("capture prior durable geometry");
            let mut changed = configured.clone();
            if change_owner {
                changed.staking.restricted_validator_mode =
                    iroha_config::parameters::actual::LaneValidatorMode::StakeElected;
            } else {
                changed.staking.public_validator_mode =
                    iroha_config::parameters::actual::LaneValidatorMode::AdminManaged;
            }
            let error = restored.set_nexus_from_config(changed)
                .expect_err("real owner change must fail both before and after initial hydration");
            assert!(matches!(error, LaneLifecycleError::UnsafeRetirement { lane, reason }
                if lane == LaneId::new(3)
                    && reason == LIVE_SHARED_DATASPACE_STAKING_OWNER_CHANGE_REASON));
            assert_eq!(norito::json::to_json(&restored).expect("unchanged State"), before);
            assert_eq!(restored.kura.lane_geometry_journal_state_for_test()
                .expect("unchanged durable geometry"), journal);
        }
    }
}

state_test! { sync snapshot_owner_policy_requires_complete_canonical_fields
    let (_directory, state, _) = snapshot_owner_policy_fixture();
    let snapshot = norito::json::to_value(&state).expect("canonical snapshot");
    let runtime = snapshot.as_object().expect("State object")
        .get("nexus_runtime").expect("runtime").as_object().expect("runtime object");
    let policy = runtime.get("owner_policy").expect("required owner policy");
    let fields = policy.as_object().expect("owner policy object")
        .keys().cloned().collect::<Vec<_>>();
    assert_eq!(fields.len(), 9);
    for field in fields {
        let mut missing = snapshot.clone();
        let _ = missing.as_object_mut().expect("State object")
            .get_mut("nexus_runtime").expect("runtime").as_object_mut().expect("runtime object")
            .get_mut("owner_policy").expect("policy").as_object_mut().expect("policy object")
            .remove(&field);
        assert!(deserialize_state_snapshot_value_with_kura(missing, Arc::clone(&state.kura)).is_err(),
            "missing owner policy field {field} must not select a default");
    }
    let mut absent = snapshot.clone();
    let _ = absent.as_object_mut().expect("State object")
        .get_mut("nexus_runtime").expect("runtime").as_object_mut().expect("runtime object")
        .remove("owner_policy");
    assert!(deserialize_state_snapshot_value_with_kura(absent, Arc::clone(&state.kura)).is_err());
    for (field, invalid) in [
        ("max_validators", norito::json::Value::from(0_u64)),
        ("autoscale_min_lane_id", norito::json::Value::from(0_u64)),
        ("autoscale_max_lane_id_exclusive", norito::json::Value::from(16_u64)),
        ("public_validator_mode", norito::json::Value::from("unknown-mode")),
        ("routing_default_lane", norito::json::Value::from(1_u64)),
        ("routing_default_dataspace", norito::json::Value::from(9_u64)),
        ("dataspaces", norito::json::Value::Array(Vec::new())),
    ] {
        let mut changed = snapshot.clone();
        let _ = changed.as_object_mut().expect("State object")
            .get_mut("nexus_runtime").expect("runtime").as_object_mut().expect("runtime object")
            .get_mut("owner_policy").expect("policy").as_object_mut().expect("policy object")
            .insert(field.to_owned(), invalid);
        assert!(deserialize_state_snapshot_value_with_kura(changed, Arc::clone(&state.kura)).is_err(),
            "malformed owner policy field {field} must fail");
    }
    let mut noncanonical = snapshot;
    let dataspaces = noncanonical.as_object_mut().expect("State object")
        .get_mut("nexus_runtime").expect("runtime").as_object_mut().expect("runtime object")
        .get_mut("owner_policy").expect("policy").as_object_mut().expect("policy object")
        .get_mut("dataspaces").expect("dataspaces");
    let norito::json::Value::Array(entries) = dataspaces else { panic!("dataspace array"); };
    entries.reverse();
    assert!(deserialize_state_snapshot_value_with_kura(noncanonical, Arc::clone(&state.kura)).is_err());
    let before = crate::snapshot::canonical_state_snapshot_hash(&state);
    let mut descriptions = state.nexus_snapshot().dataspace_catalog.entries().to_vec();
    for entry in &mut descriptions {
        entry.description = Some("different local operator description".to_owned());
    }
    state.nexus.write().dataspace_catalog = DataSpaceCatalog::new(descriptions)
        .expect("description-only catalog update");
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(&state), before,
        "operator-facing descriptions must not affect the canonical commitment");
    state.nexus.write().staking.max_validators = nonzero!(8_u32);
    assert_ne!(crate::snapshot::canonical_state_snapshot_hash(&state), before,
        "the owner policy must be authenticated by the canonical snapshot commitment");
}
