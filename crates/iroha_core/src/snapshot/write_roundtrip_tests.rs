#[tokio::test]
async fn creates_all_dirs_while_writing_snapshots() {
    let tmp_root = tempdir().unwrap();
    let snapshot_store_dir = tmp_root.path().join("path/to/snapshot/dir");
    let state = state_factory();
    let key_pair = checked_random_snapshot_keypair();
    try_write_snapshot(&state, &snapshot_store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
    assert!(Path::exists(snapshot_store_dir.as_path()));
    assert_canonical_snapshot_generation(&snapshot_store_dir);
}
#[tokio::test]
async fn can_read_snapshot_after_writing() {
    let tmp_root = tempdir().unwrap();
    let store_dir = tmp_root.path().join("snapshot");
    let state = state_factory();
    let key_pair = checked_random_snapshot_keypair();
    let expected_chain_id = state.chain_id.clone();
    try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
    let kura = Kura::blank_kura_for_testing();
    let snapshot_state = try_read_snapshot(
        &store_dir,
        &kura,
        LiveQueryStore::start_test,
        BlockCount(state.view().height()),
        TEST_CHUNK_SIZE,
        key_pair.public_key(),
        &state.network_id,
        &crate::state::default_zk_config(),
        #[cfg(feature = "telemetry")]
        StateTelemetry::new(<_>::default(), true),
    )
    .unwrap();
    assert_eq!(snapshot_state.chain_id, expected_chain_id);
    assert_eq!(
        canonical_state_snapshot_bytes_for_tests(&snapshot_state),
        canonical_state_snapshot_bytes_for_tests(&state),
        "snapshot roundtrip must preserve canonical WSV bytes"
    );
}
#[tokio::test]
async fn generated_snapshot_passes_restart_validation_before_publication() {
    let state = state_factory();
    let snapshot_bytes = exact_snapshot_payload_bytes(&state);
    let snapshot: json::Value =
        json::from_slice(&snapshot_bytes).expect("writer snapshot must be canonical JSON");
    let json::Value::Object(snapshot) = snapshot else {
        panic!("writer snapshot must be an object");
    };
    for field in ["commit_topology", "prev_commit_topology"] {
        let Some(json::Value::Object(cell)) = snapshot.get(field) else {
            panic!("{field} must retain its exact MV cell envelope");
        };
        assert_eq!(cell.len(), 2, "{field} must retain exactly two MV roles");
        assert!(cell.contains_key("revert") && cell.contains_key("blocks"));
    }
    validate_generated_snapshot_for_restart(&state, &snapshot_bytes)
        .expect("writer-generated snapshot must survive restart initialization exactly");
}
#[tokio::test]
async fn canonical_account_metadata_survives_the_snapshot_restart_boundary() {
    let state = state_factory();
    let owner = state
        .world
        .accounts
        .view()
        .iter()
        .next()
        .map(|(account_id, _)| account_id.clone())
        .expect("snapshot fixture account");
    let key = "snapshot_probe".parse().expect("metadata key");
    let value = Json::from_raw_json("1".to_owned()).expect("canonical JSON spelling");
    assert_eq!(value.get(), "1");
    let mut accounts = state.world.accounts.block();
    accounts
        .get_mut(&owner)
        .expect("snapshot fixture account remains registered")
        .insert(key, value);
    accounts.commit();
    let snapshot_bytes = exact_snapshot_payload_bytes(&state);
    validate_generated_snapshot_for_restart(&state, &snapshot_bytes)
        .expect("canonical ledger Json must round-trip through restart reconstruction");
    let snapshot_text = core::str::from_utf8(&snapshot_bytes).expect("snapshot is UTF-8 JSON");
    assert!(
        snapshot_text.contains(r#""snapshot_probe":1"#),
        "snapshot must contain only the canonical metadata spelling"
    );
}
#[tokio::test]
async fn noncanonical_snapshot_publishes_and_compacts_nothing() {
    let tmp_root = tempdir().expect("snapshot tempdir");
    let store_dir = tmp_root.path().join("snapshot");
    let kura_store_dir = tmp_root.path().join("kura");
    let initial_catalog = LaneCatalog::default();
    let extended_catalog = LaneCatalog::new(
        nonzero!(2_u32),
        vec![
            ModelLaneConfig::default(),
            ModelLaneConfig {
                id: LaneId::new(1),
                alias: "snapshot-validation-secondary".to_owned(),
                ..ModelLaneConfig::default()
            },
        ],
    )
    .expect("extended lane catalog");
    let initial = LaneConfig::from_catalog(&initial_catalog);
    let extended = LaneConfig::from_catalog(&extended_catalog);
    let initial_incarnations =
        BTreeMap::from([(LaneId::SINGLE, Hash::new(b"snapshot-validation-primary"))]);
    let extended_incarnations = BTreeMap::from([
        (LaneId::SINGLE, initial_incarnations[&LaneId::SINGLE]),
        (LaneId::new(1), Hash::new(b"snapshot-validation-secondary")),
    ]);
    let initial_activations = BTreeMap::from([(LaneId::SINGLE, 0)]);
    let extended_activations = BTreeMap::from([(LaneId::SINGLE, 0), (LaneId::new(1), 0)]);
    let kura_config = kura_config_for_snapshot_test(&kura_store_dir, nonzero!(1_usize));
    let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&kura_config, &initial)
        .expect("create persistent Kura");
    kura.apply_lane_geometry_transition_at_height(
        &initial,
        &extended,
        &initial_incarnations,
        &extended_incarnations,
        &initial_activations,
        &extended_activations,
        &BTreeSet::new(),
        0,
    )
    .expect("seed recoverable geometry transition");
    kura.mark_lane_geometry_catalog_published(
        &extended,
        &extended_incarnations,
        &extended_activations,
        None,
    )
    .expect("publish recoverable geometry transition");
    let journal_before = kura
        .lane_geometry_journal_state_for_test()
        .expect("read geometry journal before rejected snapshot");
    let journal_bytes_before = std::fs::read(kura.lane_geometry_journal_path())
        .expect("read exact geometry journal before rejected snapshot");
    assert_eq!(journal_before.1, vec!["catalog_published"]);
    let state = state_factory_with_kura(Arc::clone(&kura));
    let mut noncanonical = exact_snapshot_payload_bytes(&state);
    noncanonical.insert(1, b' ');
    let key_pair = checked_random_snapshot_keypair();
    let error = try_write_snapshot_payload_with_limit(
        &state,
        &store_dir,
        &key_pair,
        TEST_CHUNK_SIZE,
        iroha_config::parameters::defaults::snapshot::MAX_PAYLOAD_BYTES,
        noncanonical,
    )
    .expect_err("noncanonical payload must fail before publication");
    assert!(matches!(
        error,
        TryWriteError::RestartValidation(TryReadError::NonCanonicalSnapshotPayload)
    ));
    assert!(
        !store_dir.exists(),
        "restart validation must precede creation of snapshot publication artifacts"
    );
    assert_eq!(
        kura.lane_geometry_journal_state_for_test()
            .expect("read geometry journal after rejected snapshot"),
        journal_before,
        "rejected payload must not compact or otherwise rewrite geometry recovery history"
    );
    assert_eq!(
        std::fs::read(kura.lane_geometry_journal_path())
            .expect("read exact geometry journal after rejected snapshot"),
        journal_bytes_before,
        "rejected payload must preserve exact durable geometry journal bytes"
    );
}
#[tokio::test]
async fn signed_snapshot_roundtrip_preserves_authoritative_alias_revert_maps() {
    let tmp_root = tempdir().expect("snapshot tempdir");
    let store_dir = tmp_root.path().join("snapshot");
    let state = state_factory();
    let owner = {
        let accounts = state.world.accounts.view();
        accounts
            .iter()
            .next()
            .map(|(account_id, _)| account_id.clone())
            .expect("fixture account")
    };
    let account_alias = AccountAlias::new(
        "restart_alias".parse().expect("account alias label"),
        Some(AccountAliasDomain::new(
            "wonderland".parse().expect("account alias domain"),
        )),
        DataSpaceId::UNIVERSAL,
    );
    let account_rekey_record = AccountRekeyRecord::new(account_alias.clone(), owner.clone());
    {
        let mut aliases = state.world.account_aliases.block();
        assert!(
            aliases
                .insert(account_alias.clone(), owner.clone())
                .is_none()
        );
        aliases.commit();
    }
    {
        let mut records = state.world.account_rekey_records.block();
        assert!(
            records
                .insert(account_alias.clone(), account_rekey_record)
                .is_none()
        );
        records.commit();
    }
    let definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("asset domain"),
        "restart_asset".parse().expect("asset name"),
    );
    let definition = AssetDefinition::numeric(
        definition_id.clone(),
        "restart asset".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&owner);
    let definition_alias: AssetDefinitionAlias =
        "restart_asset#universal".parse().expect("asset alias");
    let definition_binding = AssetDefinitionAliasBindingRecord {
        alias: definition_alias,
        lease_expiry_ms: None,
        grace_until_ms: None,
        bound_at_ms: 1,
    };
    {
        let mut definitions = state.world.asset_definitions.block();
        assert!(
            definitions
                .insert(definition_id.clone(), definition)
                .is_none()
        );
        definitions.commit();
    }
    {
        let mut bindings = state.world.asset_definition_alias_bindings.block();
        assert!(
            bindings
                .insert(definition_id.clone(), definition_binding)
                .is_none()
        );
        bindings.commit();
    }
    let contract_address = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &owner,
        17,
        DataSpaceId::UNIVERSAL,
    )
    .expect("contract address");
    let contract_alias: ContractAlias =
        "restart_router::universal".parse().expect("contract alias");
    let contract_binding = ContractAliasBindingRecord {
        alias: contract_alias,
        lease_expiry_ms: None,
        grace_until_ms: None,
        bound_at_ms: 1,
    };
    {
        let mut bindings = state.world.contract_alias_bindings.block();
        assert!(
            bindings
                .insert(contract_address.clone(), contract_binding)
                .is_none()
        );
        bindings.commit();
    }
    let key_pair = checked_random_snapshot_keypair();
    try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE)
        .expect("write signed snapshot with authoritative alias revert maps");
    let payload = std::fs::read(current_generation_artifact(&store_dir, SNAPSHOT_FILE_NAME))
        .expect("read signed snapshot payload");
    let kura = Kura::blank_kura_for_testing();
    let restored = try_read_snapshot(
        &store_dir,
        &kura,
        LiveQueryStore::start_test,
        BlockCount(0),
        TEST_CHUNK_SIZE,
        key_pair.public_key(),
        &state.network_id,
        &crate::state::default_zk_config(),
        #[cfg(feature = "telemetry")]
        StateTelemetry::new(<_>::default(), true),
    )
    .expect("read signed snapshot without canonical payload drift");
    let mut roundtrip = String::new();
    serialize_state_snapshot(&restored, &mut roundtrip);
    assert_eq!(
        roundtrip.as_bytes(),
        payload,
        "restoring derived alias indexes must not alter authoritative snapshot bytes"
    );
    let aliases = restored.world.account_aliases.block_and_revert();
    assert!(aliases.get(&account_alias).is_none());
    aliases.commit();
    let records = restored.world.account_rekey_records.block_and_revert();
    assert!(records.get(&account_alias).is_none());
    records.commit();
    let definitions = restored.world.asset_definitions.block_and_revert();
    assert!(definitions.get(&definition_id).is_none());
    definitions.commit();
    let definition_bindings = restored
        .world
        .asset_definition_alias_bindings
        .block_and_revert();
    assert!(definition_bindings.get(&definition_id).is_none());
    definition_bindings.commit();
    let contract_bindings = restored.world.contract_alias_bindings.block_and_revert();
    assert!(contract_bindings.get(&contract_address).is_none());
    contract_bindings.commit();
}
#[tokio::test]
async fn snapshot_roundtrip_preserves_exact_sccp_registry() {
    let tmp_root = tempdir().unwrap();
    let store_dir = tmp_root.path().join("snapshot");
    let kura = Kura::blank_kura_for_testing();
    let mut state = state_factory_with_kura_and_chain(
        Arc::clone(&kura),
        iroha_data_model::ChainId::from(iroha_sccp::SCCP_TAIRA_CHAIN_ID_V1),
    );
    let block =
        signed_block_with_transaction(accepted_log_transaction("exact-sccp-registry-snapshot"));
    store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block));
    let registry = sccp_registry_for_snapshot_test();
    let expected_key = registry.lanes[0].routes[0].key();
    let expected_config = registry.lanes[0].routes[0]
        .route_configuration_hash()
        .expect("exact snapshot route configuration");
    {
        let mut cell = state.world.sccp_registry.block();
        *cell.get_mut() = registry;
        cell.commit();
    }
    store_complete_snapshot_commit_evidence_for_blocks(&state, &kura, std::slice::from_ref(&block));
    let key_pair = checked_random_snapshot_keypair();
    try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE)
        .expect("write exact SCCP registry snapshot");
    let snapshot_bytes = std::fs::read(current_generation_artifact(&store_dir, SNAPSHOT_FILE_NAME))
        .expect("snapshot bytes");
    let snapshot_value: json::Value =
        json::from_slice(&snapshot_bytes).expect("snapshot JSON should parse");
    assert!(snapshot_world_has_field(&snapshot_value, "sccp_registry"));
    let snapshot_state = try_read_snapshot(
        &store_dir,
        &kura,
        LiveQueryStore::start_test,
        BlockCount(state.view().height()),
        TEST_CHUNK_SIZE,
        key_pair.public_key(),
        &state.network_id,
        &crate::state::default_zk_config(),
        #[cfg(feature = "telemetry")]
        StateTelemetry::new(<_>::default(), true),
    )
    .expect("snapshot read");
    let restored = snapshot_state.sccp_registry_snapshot();
    let route = restored
        .route(&expected_key)
        .expect("exact SCCP route survives snapshot roundtrip");
    assert_eq!(
        route
            .route_configuration_hash()
            .expect("restored route configuration"),
        expected_config
    );
}
#[tokio::test]
async fn signed_snapshot_rejects_unknown_root_and_world_fields() {
    for (scope, field_name, expected_field) in [
        (
            "root",
            "future_snapshot_field",
            "state.future_snapshot_field",
        ),
        ("world", "sccp_registry_v2", "world.sccp_registry_v2"),
        ("world", "commit_qcs", "world.commit_qcs"),
    ] {
        let tmp_root = tempdir().expect("temporary snapshot root");
        let store_dir = tmp_root.path().join("snapshot");
        let kura = Kura::blank_kura_for_testing();
        let state = state_factory_with_kura(Arc::clone(&kura));
        let mut serialized = String::new();
        serialize_state_snapshot(&state, &mut serialized);
        let mut snapshot: json::Value =
            json::from_str(&serialized).expect("valid baseline snapshot JSON");
        let json::Value::Object(snapshot_object) = &mut snapshot else {
            panic!("snapshot root must be an object");
        };
        match scope {
            "root" => {
                assert!(
                    snapshot_object
                        .insert(field_name.to_owned(), json::Value::Null,)
                        .is_none()
                );
            }
            "world" => {
                let Some(json::Value::Object(world)) = snapshot_object.get_mut("world") else {
                    panic!("snapshot world must be an object");
                };
                assert!(
                    world
                        .insert(field_name.to_owned(), json::Value::Null)
                        .is_none()
                );
            }
            _ => unreachable!("closed test scope"),
        }
        serialized = json::to_json(&snapshot).expect("mutated snapshot JSON encodes");
        let key_pair = checked_random_snapshot_keypair();
        write_snapshot_bundle_from_bytes(&store_dir, serialized.as_bytes(), &key_pair);
        let error = match try_read_snapshot(
            &store_dir,
            &kura,
            LiveQueryStore::start_test,
            BlockCount(0),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.network_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::new(<_>::default(), true),
        ) {
            Ok(_) => panic!("signed snapshot with an unknown field must fail closed"),
            Err(error) => error,
        };
        match error {
            TryReadError::Serialization(json::Error::InvalidField { field, message }) => {
                assert_eq!(field, expected_field);
                assert!(message.contains("unknown field"), "{message}");
            }
            other => panic!("unexpected unknown-field rejection: {other:?}"),
        }
    }
}
#[tokio::test]
async fn signed_semantically_valid_wsv_tampering_is_rejected_by_kura_checkpoint() {
    let tmp_root = tempdir().expect("temporary snapshot root");
    let store_dir = tmp_root.path().join("snapshot");
    let kura = Kura::blank_kura_for_testing();
    let mut state = state_factory_with_kura(Arc::clone(&kura));
    let block = signed_block_with_transaction(accepted_log_transaction("checkpointed"));
    let block_hash = block.hash();
    store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block));
    let expected = canonical_state_snapshot_hash(&state);
    kura.store_wsv_checkpoint(1, block_hash, expected)
        .expect("persist canonical WSV checkpoint");
    let key_pair = checked_random_snapshot_keypair();
    let mut serialized = String::new();
    serialize_state_snapshot(&state, &mut serialized);
    write_snapshot_bundle_from_bytes(&store_dir, serialized.as_bytes(), &key_pair);
    let restored = try_read_snapshot(
        &store_dir,
        &kura,
        LiveQueryStore::start_test,
        BlockCount(1),
        TEST_CHUNK_SIZE,
        key_pair.public_key(),
        &state.network_id,
        &state.zk_snapshot(),
        #[cfg(feature = "telemetry")]
        StateTelemetry::new(<_>::default(), true),
    )
    .expect("an exact signed snapshot must match its Kura WSV checkpoint");
    assert_eq!(canonical_state_snapshot_hash(&restored), expected);
    drop(restored);
    let injected_account = AccountId::new(
        checked_seeded_keypair(0xD1, Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    state.world.accounts.insert(
        injected_account,
        AccountValue::new(AccountDetails::new(
            Metadata::default(),
            None,
            None,
            Vec::new(),
        )),
    );
    let actual = canonical_state_snapshot_hash(&state);
    assert_ne!(
        actual, expected,
        "hostile WSV mutation must affect its checkpoint"
    );
    serialized.clear();
    serialize_state_snapshot(&state, &mut serialized);
    write_snapshot_bundle_from_bytes(&store_dir, serialized.as_bytes(), &key_pair);
    let error = match try_read_snapshot(
        &store_dir,
        &kura,
        LiveQueryStore::start_test,
        BlockCount(1),
        TEST_CHUNK_SIZE,
        key_pair.public_key(),
        &state.network_id,
        &state.zk_snapshot(),
        #[cfg(feature = "telemetry")]
        StateTelemetry::new(<_>::default(), true),
    ) {
        Ok(_) => panic!("a signature cannot replace the canonical Kura WSV checkpoint"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        TryReadError::WsvCheckpointMismatch {
            height: 1,
            expected: observed_expected,
            actual: observed_actual,
        } if observed_expected == expected && observed_actual == actual
    ));
    assert_eq!(kura.blocks_count(), 1);
    assert_eq!(kura.exact_durable_blocks_count().unwrap(), 1);
    assert_eq!(
        kura.wsv_checkpoint(1)
            .expect("read checkpoint after rejection")
            .expect("checkpoint remains present")
            .state_hash(),
        expected,
        "rejected snapshot must not replace the durable WSV checkpoint"
    );
}
#[tokio::test]
async fn signed_hostile_sccp_registry_snapshots_are_rejected_before_acceptance() {
    enum RegistryCellMutation {
        Replace {
            role: &'static str,
            registry: crate::state::SccpOnChainRegistryV1,
        },
        Remove(&'static str),
        AddUnknown,
    }
    let assert_rejected = |mutation: RegistryCellMutation, expected: &str| {
        let tmp_root = tempdir().expect("temporary snapshot root");
        let store_dir = tmp_root.path().join("snapshot");
        let kura = Kura::blank_kura_for_testing();
        let state = state_factory_with_kura_and_chain(
            Arc::clone(&kura),
            iroha_data_model::ChainId::from(iroha_sccp::SCCP_TAIRA_CHAIN_ID_V1),
        );
        let mut serialized = String::new();
        serialize_state_snapshot(&state, &mut serialized);
        let mut snapshot: json::Value =
            json::from_str(&serialized).expect("valid baseline snapshot JSON");
        let json::Value::Object(snapshot_object) = &mut snapshot else {
            panic!("snapshot root must be an object");
        };
        let Some(json::Value::Object(world)) = snapshot_object.get_mut("world") else {
            panic!("snapshot world must be an object");
        };
        let Some(json::Value::Object(cell)) = world.get_mut("sccp_registry") else {
            panic!("snapshot SCCP registry must be one cell envelope");
        };
        match mutation {
            RegistryCellMutation::Replace { role, registry } => {
                cell.insert(
                    role.to_owned(),
                    json::to_value(&registry).expect("hostile SCCP registry encodes"),
                );
            }
            RegistryCellMutation::Remove(role) => {
                assert!(cell.remove(role).is_some(), "baseline cell contains {role}");
            }
            RegistryCellMutation::AddUnknown => {
                assert!(
                    cell.insert("future_registry".to_owned(), json::Value::Null)
                        .is_none()
                );
            }
        }
        serialized = json::to_json(&snapshot).expect("mutated snapshot JSON encodes");
        let key_pair = checked_random_snapshot_keypair();
        write_snapshot_bundle_from_bytes(&store_dir, serialized.as_bytes(), &key_pair);
        let result = try_read_snapshot(
            &store_dir,
            &kura,
            LiveQueryStore::start_test,
            BlockCount(0),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.network_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::new(<_>::default(), true),
        );
        match result {
            Err(TryReadError::InvalidSccpRegistry(error)) => {
                assert!(error.contains(expected), "{error}");
            }
            Err(error) => panic!("unexpected snapshot error: {error:?}"),
            Ok(_) => panic!("signed hostile SCCP registry snapshot must be rejected"),
        }
    };
    assert_rejected(
        RegistryCellMutation::Replace {
            role: "blocks",
            registry: crate::state::SccpOnChainRegistryV1 {
                version: 2,
                lanes: Vec::new(),
            },
        },
        "version",
    );
    assert_rejected(
        RegistryCellMutation::Replace {
            role: "revert",
            registry: crate::state::SccpOnChainRegistryV1 {
                version: 2,
                lanes: Vec::new(),
            },
        },
        "revert",
    );
    assert_rejected(RegistryCellMutation::Remove("blocks"), "missing `blocks`");
    assert_rejected(RegistryCellMutation::Remove("revert"), "missing `revert`");
    assert_rejected(RegistryCellMutation::AddUnknown, "unknown field");
    let mut valid = sccp_registry_for_snapshot_test();
    let lane = valid.lanes.remove(0);
    assert_rejected(
        RegistryCellMutation::Replace {
            role: "blocks",
            registry: crate::state::SccpOnChainRegistryV1 {
                version: 1,
                lanes: vec![lane.clone(), lane],
            },
        },
        "duplicate",
    );
    let bsc_route = iroha_sccp::sccp_exact_evm_governed_route_test_fixture_v1(
        iroha_data_model::bridge::SccpNetworkV1::BscTestnet,
        iroha_data_model::bridge::SccpRouteActivationV1::Staged,
    );
    let mut reversed_lanes = vec![
        sccp_registry_for_snapshot_test().lanes.remove(0),
        iroha_data_model::bridge::SccpGovernedLaneV1 {
            lane_id: bsc_route.lane_id,
            native_trust_anchors: Vec::new(),
            current_native_trust_anchor_hash: None,
            routes: vec![bsc_route],
        },
    ];
    reversed_lanes.sort_by_key(|lane| lane.lane_id);
    reversed_lanes.reverse();
    assert_rejected(
        RegistryCellMutation::Replace {
            role: "blocks",
            registry: crate::state::SccpOnChainRegistryV1 {
                version: 1,
                lanes: reversed_lanes,
            },
        },
        "canonical lane/route order",
    );
    let mut off_curve = sccp_registry_for_snapshot_test();
    let deployment = match &mut off_curve.lanes[0].routes[0].destination {
        iroha_data_model::bridge::SccpDestinationDeploymentV1::Evm(deployment) => deployment,
        iroha_data_model::bridge::SccpDestinationDeploymentV1::Tron(_) => {
            unreachable!("snapshot fixture is an EVM route")
        }
        iroha_data_model::bridge::SccpDestinationDeploymentV1::Solana(_) => {
            unreachable!("snapshot fixture is an EVM route")
        }
        iroha_data_model::bridge::SccpDestinationDeploymentV1::Ton(_) => {
            unreachable!("snapshot fixture is an EVM route")
        }
    };
    // (1, 1) is a canonical BN254 field encoding but is not on
    // y^2 = x^3 + 3.  Recompute the embedded key commitment so only the
    // cryptographic curve check—not a stale hash—can reject this fixture.
    let mut one = [0_u8; 32];
    one[31] = 1;
    deployment.verifying_key.alpha1.x = one;
    deployment.verifying_key.alpha1.y = one;
    deployment.verifier_key_hash =
        iroha_data_model::bridge::sccp_groth16_bn254_verifying_key_hash_v1(
            deployment.verifying_key,
        )
        .expect("off-curve point remains structurally canonical");
    let route = &mut off_curve.lanes[0].routes[0];
    let route_configuration_hash = route
        .destination
        .route_configuration_hash(
            route.lane_id,
            &route.route_id,
            &route.asset_key,
            route.revision,
            route.settlement.payload_amount_scale,
        )
        .expect("off-curve point remains structurally valid route input");
    match &mut route.source_identity.emitter {
        iroha_data_model::bridge::SccpSourceEmitterV1::Evm(emitter) => {
            emitter.route_config_hash = route_configuration_hash;
        }
        iroha_data_model::bridge::SccpSourceEmitterV1::Tron(_) => {
            unreachable!("snapshot fixture is an EVM route")
        }
        iroha_data_model::bridge::SccpSourceEmitterV1::Solana(_) => {
            unreachable!("snapshot fixture is an EVM route")
        }
        iroha_data_model::bridge::SccpSourceEmitterV1::Ton(_) => {
            unreachable!("snapshot fixture is an EVM route")
        }
    }
    off_curve
        .validate()
        .expect("structural registry validation must not stand in for curve validation");
    assert_rejected(
        RegistryCellMutation::Replace {
            role: "blocks",
            registry: off_curve,
        },
        "non-curve",
    );
}
#[tokio::test]
async fn signed_hostile_sccp_revert_stores_are_rejected_without_mutation() {
    #[derive(Clone, Copy, Debug)]
    enum RevertMutation {
        PendingUsage,
        PendingMessages,
        MessageLocator,
        OrderedIndex,
        TerminalProofs,
        InboundMessages,
        InboundHighWater,
    }
    fn envelope_mut<'a>(world: &'a mut json::Map, field: &str) -> &'a mut json::Map {
        let Some(json::Value::Object(envelope)) = world.get_mut(field) else {
            panic!("{field} must be one MV envelope");
        };
        envelope
    }
    fn storage_blocks<K, V>(entries: impl IntoIterator<Item = (K, V)>) -> json::Value
    where
        K: mv::Key + mv::json::JsonKeyCodec,
        V: mv::Value + json::JsonSerialize,
    {
        let storage: Storage<K, V> = entries.into_iter().collect();
        let json::Value::Object(mut envelope) =
            json::to_value(&storage).expect("typed hostile storage encodes")
        else {
            panic!("typed hostile storage must encode as an envelope");
        };
        envelope
            .remove("blocks")
            .expect("storage envelope contains blocks")
    }
    for mutation in [
        RevertMutation::PendingUsage,
        RevertMutation::PendingMessages,
        RevertMutation::MessageLocator,
        RevertMutation::OrderedIndex,
        RevertMutation::TerminalProofs,
        RevertMutation::InboundMessages,
        RevertMutation::InboundHighWater,
    ] {
        let tmp_root = tempdir().expect("temporary snapshot root");
        let store_dir = tmp_root.path().join("snapshot");
        let kura = Kura::blank_kura_for_testing();
        let (state, key, pending_record) =
            state_with_exact_pending_sccp_snapshot_fixture(Arc::clone(&kura));
        let mut serialized = String::new();
        serialize_state_snapshot(&state, &mut serialized);
        let mut snapshot: json::Value =
            json::from_str(&serialized).expect("valid baseline snapshot JSON");
        let json::Value::Object(snapshot_object) = &mut snapshot else {
            panic!("snapshot root must be an object");
        };
        let Some(json::Value::Object(world)) = snapshot_object.get_mut("world") else {
            panic!("snapshot world must be an object");
        };
        match mutation {
            RevertMutation::PendingUsage => {
                let current = envelope_mut(world, "sccp_outbound_pending_usage")
                    .get("blocks")
                    .cloned()
                    .expect("usage envelope contains blocks");
                envelope_mut(world, "sccp_outbound_pending_usage")
                    .insert("revert".to_owned(), current);
            }
            RevertMutation::PendingMessages => {
                envelope_mut(world, "sccp_outbound_pending_messages")
                    .insert("revert".to_owned(), json::Value::Object(json::Map::new()));
            }
            RevertMutation::MessageLocator => {
                envelope_mut(world, "sccp_outbound_message_locator")
                    .insert("revert".to_owned(), json::Value::Object(json::Map::new()));
            }
            RevertMutation::OrderedIndex => {
                envelope_mut(world, "sccp_outbound_message_index")
                    .insert("revert".to_owned(), json::Value::Object(json::Map::new()));
            }
            RevertMutation::TerminalProofs => {
                let terminal = iroha_data_model::bridge::SccpOutboundProofRecordV1 {
                    payload_hash: pending_record.payload_hash,
                    destination_binding_hash: pending_record.destination_binding_hash,
                    route_configuration_hash: pending_record.route_configuration_hash,
                    finality_block_hash: [0xA1; 32],
                    destination_proof_commitment: [0xA2; 32],
                    finality_height: pending_record.recorded_at_height,
                    commitment_index: pending_record.commitment_index,
                    accepted_at_height: pending_record.recorded_at_height,
                };
                assert!(terminal.is_well_formed_for_key(&key));
                envelope_mut(world, "sccp_outbound_proofs")
                    .insert("revert".to_owned(), storage_blocks([(key, terminal)]));
            }
            RevertMutation::InboundMessages | RevertMutation::InboundHighWater => {
                let (native, source_identity, trust_anchor) =
                    iroha_sccp::sccp_native_ethereum_transfer_inbound_test_fixture_v1();
                let validated = iroha_sccp::verify_sccp_native_inbound_message_proof_v1(
                    &native,
                    &source_identity,
                    trust_anchor,
                )
                .expect("native hostile-revert fixture verifies");
                let route = iroha_sccp::sccp_exact_evm_governed_route_test_fixture_v1(
                    iroha_data_model::bridge::SccpNetworkV1::EthereumMainnet,
                    iroha_data_model::bridge::SccpRouteActivationV1::Bidirectional,
                );
                let inbound_record = iroha_data_model::bridge::SccpInboundMessageRecordV1 {
                    payload_hash: validated.payload_hash,
                    source_identity_hash: validated.source_identity_hash,
                    route_configuration_hash: route
                        .route_configuration_hash()
                        .expect("fixture route configuration"),
                    trust_anchor: validated.trust_anchor,
                    anchor_interval_height: validated.anchor_interval_height,
                    source_finality_height: validated.source_finality.height,
                    source_finality_hash: validated.source_finality.block_hash,
                    source_proof_commitment: [0xA3; 32],
                    admitted_at_height: 1,
                };
                assert!(inbound_record.is_well_formed_for_lane(validated.message_key.lane));
                if matches!(mutation, RevertMutation::InboundMessages) {
                    envelope_mut(world, "sccp_inbound_messages").insert(
                        "revert".to_owned(),
                        storage_blocks([(validated.message_key, inbound_record)]),
                    );
                } else {
                    let high_water_key =
                        iroha_data_model::bridge::SccpInboundAnchorHighWaterKeyV1::new(
                            validated.message_key.lane,
                            validated.trust_anchor.anchor_hash,
                        )
                        .expect("validated native fixture forms high-water key");
                    envelope_mut(world, "sccp_inbound_anchor_high_water").insert(
                        "revert".to_owned(),
                        storage_blocks([(high_water_key, validated.anchor_interval_height)]),
                    );
                }
            }
        }
        serialized = json::to_json(&snapshot).expect("mutated snapshot JSON encodes");
        let key_pair = checked_random_snapshot_keypair();
        write_snapshot_bundle_from_bytes(&store_dir, serialized.as_bytes(), &key_pair);
        let pointer_before =
            std::fs::read(store_dir.join(SNAPSHOT_CURRENT_FILE_NAME)).expect("read pointer");
        let canonical_hash = kura
            .block_hash_at_height(nonzero!(1_usize))
            .expect("canonical Kura hash");
        let body_before = kura
            .get_block(nonzero!(1_usize))
            .expect("canonical Kura body");
        let retained_before = kura
            .v2_finality_artifact_with_archive(1)
            .expect("read exact retained SCCP material")
            .expect("exact retained SCCP material exists");
        let error = match try_read_snapshot(
            &store_dir,
            &kura,
            LiveQueryStore::start_test,
            BlockCount(1),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.network_id,
            &state.zk_snapshot(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::new(<_>::default(), true),
        ) {
            Ok(_) => panic!("hostile {mutation:?} revert must fail closed"),
            Err(error) => error,
        };
        assert!(
            matches!(error, TryReadError::InvalidSccpRevert(_)),
            "unexpected {mutation:?} rejection: {error:?}"
        );
        assert_eq!(kura.blocks_count(), 1, "{mutation:?} rejection pruned Kura");
        assert_eq!(kura.exact_durable_blocks_count().unwrap(), 1);
        assert_eq!(
            kura.block_hash_at_height(nonzero!(1_usize)),
            Some(canonical_hash)
        );
        assert_eq!(
            kura.get_block(nonzero!(1_usize)),
            Some(body_before),
            "{mutation:?} rejection changed the canonical block body"
        );
        assert_eq!(
            kura.v2_finality_artifact_with_archive(1)
                .expect("read retained material after rejection")
                .expect("retained material still exists"),
            retained_before,
            "{mutation:?} rejection changed retained SCCP evidence"
        );
        assert_eq!(
            std::fs::read(store_dir.join(SNAPSHOT_CURRENT_FILE_NAME))
                .expect("read pointer after rejection"),
            pointer_before,
            "{mutation:?} rejection changed the selected immutable generation"
        );
    }
}
#[tokio::test]
async fn snapshot_roundtrip_preserves_sccp_outbound_pending_messages() {
    let tmp_root = tempdir().unwrap();
    let store_dir = tmp_root.path().join("snapshot");
    let kura = Kura::blank_kura_for_testing();
    let (state, key, record) = state_with_exact_pending_sccp_snapshot_fixture(Arc::clone(&kura));
    let key_pair = checked_random_snapshot_keypair();
    try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
    let snapshot_bytes = std::fs::read(current_generation_artifact(&store_dir, SNAPSHOT_FILE_NAME))
        .expect("snapshot bytes");
    let snapshot_value: json::Value =
        json::from_slice(&snapshot_bytes).expect("snapshot JSON should parse");
    assert!(
        snapshot_world_has_field(&snapshot_value, "sccp_outbound_pending_messages"),
        "new snapshots must carry the SCCP outbound replay registry"
    );
    assert!(
        snapshot_world_has_field(&snapshot_value, "sccp_outbound_pending_usage"),
        "new snapshots must carry exact SCCP pending usage"
    );
    let snapshot_state = try_read_snapshot(
        &store_dir,
        &kura,
        LiveQueryStore::start_test,
        BlockCount(state.view().height()),
        TEST_CHUNK_SIZE,
        key_pair.public_key(),
        &state.network_id,
        &crate::state::default_zk_config(),
        #[cfg(feature = "telemetry")]
        StateTelemetry::new(<_>::default(), true),
    )
    .expect("snapshot read");
    let restored = snapshot_state
        .view()
        .world
        .sccp_outbound_pending_messages
        .get(&key)
        .cloned()
        .expect("SCCP outbound replay key should survive snapshot roundtrip");
    assert_eq!(restored, record);
    assert_eq!(
        snapshot_state
            .view()
            .world
            .sccp_outbound_pending_usage
            .get()
            .message_count,
        1
    );
}
#[tokio::test]
async fn incompatible_sccp_caps_reject_before_snapshot_can_prune_kura() {
    let tmp_root = tempdir().unwrap();
    let store_dir = tmp_root.path().join("snapshot");
    let kura = Kura::blank_kura_for_testing();
    let (state, _, record) = state_with_exact_pending_sccp_snapshot_fixture(Arc::clone(&kura));
    let key_pair = checked_random_snapshot_keypair();
    try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE)
        .expect("write exact SCCP snapshot");
    // Keep every SCCP record/archive association exact so the configured-cap
    // rejection is the first failing boundary, ahead of hash reconciliation.
    let snapshot_bytes = std::fs::read(current_generation_artifact(&store_dir, SNAPSHOT_FILE_NAME))
        .expect("snapshot bytes");
    let mut snapshot_value: json::Value =
        json::from_slice(&snapshot_bytes).expect("snapshot JSON parses");
    let json::Value::Object(root) = &mut snapshot_value else {
        panic!("snapshot root is an object");
    };
    let Some(json::Value::Array(block_hashes)) = root.get_mut("block_hashes") else {
        panic!("snapshot block hashes are an array");
    };
    assert_eq!(block_hashes.len(), 1);
    let forged_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xA5; 32]));
    assert_ne!(
        forged_hash,
        state.latest_block_hash_fast().expect("fixture block hash")
    );
    block_hashes[0] = json::to_value(&forged_hash).expect("encode forged block hash");
    let Some(json::Value::Object(runtime)) = root.get_mut("nexus_runtime") else {
        panic!("snapshot Nexus runtime is an object");
    };
    let Some(json::Value::Array(history)) = runtime.get_mut("autoscale_sample_history") else {
        panic!("snapshot autoscale sample history is an array");
    };
    let Some(json::Value::Object(latest_sample)) = history.last_mut() else {
        panic!("snapshot autoscale sample history retains the latest block");
    };
    latest_sample.insert(
        "block_hash".to_owned(),
        json::to_value(&forged_hash).expect("encode forged autoscale sample hash"),
    );
    let mut forged_snapshot_bytes = Vec::new();
    json::to_writer(&mut forged_snapshot_bytes, &snapshot_value).expect("encode forged snapshot");
    write_snapshot_bundle_from_bytes(&store_dir, &forged_snapshot_bytes, &key_pair);
    let canonical_hash = kura
        .block_hash_at_height(nonzero!(1_usize))
        .expect("canonical Kura hash");
    let body_before = kura
        .get_block(nonzero!(1_usize))
        .expect("canonical Kura body");
    let retained_before = kura
        .v2_finality_artifact_with_archive(1)
        .expect("read exact retained SCCP material")
        .expect("exact retained SCCP material exists");
    let mut incompatible = state.zk_snapshot();
    let payload_bytes = u64::try_from(record.payload_bytes.len()).expect("small payload");
    incompatible.sccp.max_pending_outbound_payload_bytes =
        NonZeroU64::new(payload_bytes - 1).expect("fixture payload exceeds one byte");
    let error = match try_read_snapshot(
        &store_dir,
        &kura,
        LiveQueryStore::start_test,
        BlockCount(1),
        TEST_CHUNK_SIZE,
        key_pair.public_key(),
        &state.network_id,
        &incompatible,
        #[cfg(feature = "telemetry")]
        StateTelemetry::new(<_>::default(), true),
    ) {
        Ok(_) => panic!("incompatible actual SCCP cap must fail before reconciliation"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        TryReadError::ZkConfigInstall(ZkConfigInstallError::SccpPendingUsageLimitExceeded { .. })
    ));
    assert_eq!(kura.blocks_count(), 1, "rejected snapshot pruned Kura");
    assert_eq!(kura.exact_durable_blocks_count().unwrap(), 1);
    assert_eq!(
        kura.block_hash_at_height(nonzero!(1_usize)),
        Some(canonical_hash)
    );
    assert_eq!(
        kura.get_block(nonzero!(1_usize)),
        Some(body_before),
        "rejected snapshot changed the canonical block body"
    );
    assert_eq!(
        kura.v2_finality_artifact_with_archive(1)
            .expect("read retained SCCP material after rejection")
            .expect("retained SCCP material still exists"),
        retained_before,
        "rejected snapshot changed retained header, finality, or archive material"
    );
}
#[tokio::test]
async fn sccp_snapshot_revert_enforces_actual_pending_cap_after_terminal_compaction() {
    let kura = Kura::blank_kura_for_testing();
    let (mut state, key, pending) =
        state_with_exact_pending_sccp_snapshot_fixture(Arc::clone(&kura));
    let finality_block_hash = kura
        .block_hash_at_height(nonzero!(1_usize))
        .expect("fixture Kura hash");
    let terminal = iroha_data_model::bridge::SccpOutboundProofRecordV1 {
        payload_hash: pending.payload_hash,
        destination_binding_hash: pending.destination_binding_hash,
        route_configuration_hash: pending.route_configuration_hash,
        finality_block_hash: <[u8; 32]>::from(Hash::from(finality_block_hash)),
        destination_proof_commitment: [0xB7; 32],
        finality_height: pending.recorded_at_height,
        commitment_index: pending.commitment_index,
        accepted_at_height: 2,
    };
    state
        .transition_sccp_outbound_message_to_terminal_for_testing(key, terminal)
        .expect("compact the current payload-bearing record to a terminal descriptor");
    state.push_block_hash_for_testing(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::prehashed([0xB8; 32]),
    ));
    let payload_bytes = u64::try_from(pending.payload_bytes.len()).expect("small payload");
    let mut lowered = state.zk_snapshot();
    lowered.sccp.max_pending_outbound_messages = NonZeroU64::new(1).expect("one is nonzero");
    lowered.sccp.max_pending_outbound_payload_bytes =
        NonZeroU64::new(payload_bytes - 1).expect("fixture payload exceeds one byte");
    state
        .set_zk(lowered)
        .expect("the compacted current state fits the lowered runtime cap");
    let error = crate::state::validate_sccp_snapshot_revert_candidate(&state)
        .expect_err("rollback must not expose pending state above the actual runtime cap");
    assert!(
        error.contains("exceeds configured limits"),
        "unexpected rollback-cap rejection: {error}"
    );
    let view = state.view();
    assert!(
        view.world
            .sccp_outbound_pending_messages
            .get(&key)
            .is_none(),
        "validation must not roll the current WSV back"
    );
    assert!(
        view.world.sccp_outbound_proofs.get(&key).is_some(),
        "validation must preserve the current terminal descriptor"
    );
}
#[tokio::test]
async fn snapshot_write_signature_file_uses_checked_signing_and_verifies_digest() {
    let tmp_root = tempdir().unwrap();
    let store_dir = tmp_root.path().join("snapshot");
    let state = state_factory();
    let key_pair = checked_random_snapshot_keypair();
    try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).expect("snapshot write");
    let bundle_digest = current_snapshot_bundle_auth_digest(&store_dir);
    let signature_hex = std::fs::read_to_string(current_generation_artifact(
        &store_dir,
        SNAPSHOT_SIGNATURE_FILE_NAME,
    ))
    .expect("snapshot signature");
    let signature = Signature::try_from_hex(signature_hex.trim()).expect("snapshot signature hex");
    signature
        .verify(key_pair.public_key(), &bundle_digest)
        .expect("checked snapshot signature must verify");
}
#[tokio::test]
async fn snapshot_read_rejects_wrong_key_signature_for_matching_digest() {
    let tmp_root = tempdir().unwrap();
    let store_dir = tmp_root.path().join("snapshot");
    let state = state_factory();
    let key_pair = checked_random_snapshot_keypair();
    try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).expect("snapshot write");
    let bundle_digest = current_snapshot_bundle_auth_digest(&store_dir);
    let wrong_key_pair = checked_random_snapshot_keypair();
    let wrong_signature = Signature::try_new(wrong_key_pair.private_key(), &bundle_digest)
        .expect("checked wrong-key snapshot signature");
    std::fs::write(
        current_generation_artifact(&store_dir, SNAPSHOT_SIGNATURE_FILE_NAME),
        hex::encode(wrong_signature.payload()),
    )
    .expect("replace snapshot signature");
    let Err(error) = try_read_snapshot(
        &store_dir,
        &Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test,
        BlockCount(state.view().height()),
        TEST_CHUNK_SIZE,
        key_pair.public_key(),
        &state.network_id,
        &crate::state::default_zk_config(),
        #[cfg(feature = "telemetry")]
        StateTelemetry::default(),
    ) else {
        panic!("snapshot with wrong-key signature should be rejected")
    };
    assert!(matches!(error, TryReadError::SignatureInvalid(_)));
}
#[tokio::test]
async fn snapshot_read_rejects_noncanonical_uppercase_signature_hex() {
    let tmp_root = tempdir().unwrap();
    let store_dir = tmp_root.path().join("snapshot");
    let state = state_factory();
    let key_pair = checked_random_snapshot_keypair();
    try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).expect("snapshot write");
    let signature_path = current_generation_artifact(&store_dir, SNAPSHOT_SIGNATURE_FILE_NAME);
    let signature_hex = std::fs::read_to_string(&signature_path).expect("signature hex");
    std::fs::write(&signature_path, signature_hex.to_ascii_uppercase())
        .expect("replace signature with equivalent noncanonical hex");
    let Err(error) = try_read_snapshot(
        &store_dir,
        &Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test,
        BlockCount(state.view().height()),
        TEST_CHUNK_SIZE,
        key_pair.public_key(),
        &state.network_id,
        &crate::state::default_zk_config(),
        #[cfg(feature = "telemetry")]
        StateTelemetry::default(),
    ) else {
        panic!("uppercase signature hex must not be accepted");
    };
    assert!(matches!(error, TryReadError::SignatureMalformed(_)));
}
#[tokio::test]
async fn snapshot_read_rejects_all_zero_signature_sidecar_before_verification() {
    let tmp_root = tempdir().unwrap();
    let store_dir = tmp_root.path().join("snapshot");
    let state = state_factory();
    let key_pair = checked_random_snapshot_keypair();
    try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).expect("snapshot write");
    std::fs::write(
        current_generation_artifact(&store_dir, SNAPSHOT_SIGNATURE_FILE_NAME),
        "00".repeat(64),
    )
    .expect("replace snapshot signature");
    let Err(error) = try_read_snapshot(
        &store_dir,
        &Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test,
        BlockCount(state.view().height()),
        TEST_CHUNK_SIZE,
        key_pair.public_key(),
        &state.network_id,
        &crate::state::default_zk_config(),
        #[cfg(feature = "telemetry")]
        StateTelemetry::default(),
    ) else {
        panic!("snapshot with all-zero signature should be rejected")
    };
    assert!(matches!(error, TryReadError::SignatureMalformed(_)));
}
#[tokio::test]
async fn snapshot_read_rejects_malformed_ed25519_signature_r_before_verification() {
    let tmp_root = tempdir().unwrap();
    let store_dir = tmp_root.path().join("snapshot");
    let state = state_factory();
    let key_pair = checked_random_snapshot_keypair();
    try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).expect("snapshot write");
    let signature_hex = std::fs::read_to_string(current_generation_artifact(
        &store_dir,
        SNAPSHOT_SIGNATURE_FILE_NAME,
    ))
    .expect("snapshot signature");
    let valid_signature_bytes = hex::decode(signature_hex.trim()).expect("signature hex");
    for (label, replacement_r) in [
        ("small-order", SMALL_ORDER_ED25519_R),
        ("noncanonical", NONCANONICAL_ED25519_R),
    ] {
        let mut signature_bytes = valid_signature_bytes.clone();
        signature_bytes[..replacement_r.len()].copy_from_slice(&replacement_r);
        std::fs::write(
            current_generation_artifact(&store_dir, SNAPSHOT_SIGNATURE_FILE_NAME),
            hex::encode(signature_bytes),
        )
        .expect("replace snapshot signature");
        let Err(error) = try_read_snapshot(
            &store_dir,
            &Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.network_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::default(),
        ) else {
            panic!("snapshot with malformed Ed25519 signature R should be rejected")
        };
        assert!(
            matches!(error, TryReadError::SignatureMalformed(_)),
            "{label} snapshot signature R produced unexpected error: {error:?}"
        );
    }
}
#[tokio::test]
async fn snapshot_read_rejects_malformed_mldsa_signature_lengths_before_verification() {
    let tmp_root = tempdir().unwrap();
    let store_dir = tmp_root.path().join("snapshot");
    let state = state_factory();
    let key_pair = KeyPair::try_from_seed(b"snapshot-mldsa-signature".to_vec(), Algorithm::MlDsa)
        .expect("snapshot ML-DSA fixture key generation should succeed");
    try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).expect("snapshot write");
    let signature_hex = std::fs::read_to_string(current_generation_artifact(
        &store_dir,
        SNAPSHOT_SIGNATURE_FILE_NAME,
    ))
    .expect("snapshot signature");
    let valid_signature_bytes = hex::decode(signature_hex.trim()).expect("signature hex");
    for label in ["short", "overlong"] {
        let mut signature_bytes = valid_signature_bytes.clone();
        match label {
            "short" => {
                signature_bytes
                    .pop()
                    .expect("ML-DSA snapshot signature is non-empty");
            }
            "overlong" => signature_bytes.push(0xA5),
            _ => unreachable!("covered labels"),
        }
        std::fs::write(
            current_generation_artifact(&store_dir, SNAPSHOT_SIGNATURE_FILE_NAME),
            hex::encode(signature_bytes),
        )
        .expect("replace snapshot signature");
        let Err(error) = try_read_snapshot(
            &store_dir,
            &Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.network_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::default(),
        ) else {
            panic!("snapshot with malformed ML-DSA signature length should be rejected")
        };
        assert!(
            matches!(error, TryReadError::SignatureMalformed(_)),
            "{label} snapshot ML-DSA signature length produced unexpected error: {error:?}"
        );
    }
}
#[tokio::test]
async fn snapshot_roundtrip_preserves_space_directory_manifests_and_rebuilds_bindings() {
    let tmp_root = tempdir().unwrap();
    let store_dir = tmp_root.path().join("snapshot");
    let mut state = state_factory();
    let (uaid, dataspace, account_id) = install_active_space_directory_manifest(&mut state);
    let key_pair = checked_random_snapshot_keypair();
    try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
    let snapshot_bytes = std::fs::read(current_generation_artifact(&store_dir, SNAPSHOT_FILE_NAME))
        .expect("snapshot bytes");
    let snapshot_value: json::Value =
        json::from_slice(&snapshot_bytes).expect("snapshot JSON should parse");
    assert!(
        snapshot_has_space_directory_manifest_section(&snapshot_value),
        "new snapshots must carry a Space Directory manifest section"
    );
    assert!(
        snapshot_world_has_field(&snapshot_value, "kagemusha_replay_keys"),
        "new snapshots must carry Kagemusha replay keys"
    );
    let snapshot_state = try_read_snapshot(
        &store_dir,
        &Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test,
        BlockCount(state.view().height()),
        TEST_CHUNK_SIZE,
        key_pair.public_key(),
        &state.network_id,
        &crate::state::default_zk_config(),
        #[cfg(feature = "telemetry")]
        StateTelemetry::new(<_>::default(), true),
    )
    .expect("snapshot read");
    let manifests = snapshot_state.world.space_directory_manifests.view();
    let manifest_set = manifests
        .get(&uaid)
        .expect("manifest set should survive snapshot restore");
    assert!(
        manifest_set.get(&dataspace).is_some(),
        "dataspace manifest should survive snapshot restore"
    );
    drop(manifests);
    let bindings = snapshot_state.world.uaid_dataspaces.view();
    let uaid_bindings = bindings
        .get(&uaid)
        .expect("UAID bindings should be rebuilt after snapshot restore");
    assert!(
        uaid_bindings.is_bound_to(dataspace, &account_id),
        "restored active manifest should bind the account to the dataspace"
    );
}
#[tokio::test]
async fn snapshot_missing_space_directory_section_rejects_even_with_kura_history() {
    let tmp_root = tempdir().unwrap();
    let store_dir = tmp_root.path().join("snapshot");
    let kura = Kura::blank_kura_for_testing();
    let mut state = state_factory_with_kura(Arc::clone(&kura));
    let manifest = sample_space_directory_manifest();
    let _account_id = insert_account_with_uaid(&mut state, manifest.uaid);
    let block = signed_block_with_transaction(accepted_manifest_transaction());
    store_block_and_mark_state_height(&mut state, &kura, block);
    let key_pair = checked_random_snapshot_keypair();
    let incomplete_bytes = snapshot_payload_without_space_directory_manifest_section(&state);
    write_snapshot_bundle_from_bytes(&store_dir, &incomplete_bytes, &key_pair);
    let error = match try_read_snapshot(
        &store_dir,
        &kura,
        LiveQueryStore::start_test,
        BlockCount(state.view().height()),
        TEST_CHUNK_SIZE,
        key_pair.public_key(),
        &state.network_id,
        &crate::state::default_zk_config(),
        #[cfg(feature = "telemetry")]
        StateTelemetry::new(<_>::default(), true),
    ) {
        Ok(_) => panic!("missing canonical manifest section must not be reconstructed"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        TryReadError::MissingSpaceDirectoryManifestSection { snapshot_height: 1 }
    ));
}
#[tokio::test]
async fn snapshot_missing_space_directory_section_rejects_without_manifest_history() {
    let tmp_root = tempdir().unwrap();
    let store_dir = tmp_root.path().join("snapshot");
    let kura = Kura::blank_kura_for_testing();
    let mut state = state_factory_with_kura(Arc::clone(&kura));
    let block = signed_block_with_transaction(accepted_log_transaction("missing-section"));
    store_block_and_mark_state_height(&mut state, &kura, block);
    let key_pair = checked_random_snapshot_keypair();
    let incomplete_bytes = snapshot_payload_without_space_directory_manifest_section(&state);
    write_snapshot_bundle_from_bytes(&store_dir, &incomplete_bytes, &key_pair);
    let error = match try_read_snapshot(
        &store_dir,
        &kura,
        LiveQueryStore::start_test,
        BlockCount(state.view().height()),
        TEST_CHUNK_SIZE,
        key_pair.public_key(),
        &state.network_id,
        &crate::state::default_zk_config(),
        #[cfg(feature = "telemetry")]
        StateTelemetry::new(<_>::default(), true),
    ) {
        Ok(_) => panic!("non-empty snapshot must carry its canonical manifest section"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        TryReadError::MissingSpaceDirectoryManifestSection { snapshot_height: 1 }
    ));
}
