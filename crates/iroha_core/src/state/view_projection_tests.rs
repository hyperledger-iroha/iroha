// Tests for committed state views, indexed projections, and constructor wiring.
#[tokio::test]
async fn get_block_hashes_after_hash() {
    const BLOCK_CNT: usize = 10;

    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(World::default(), kura, query_handle);

    let mut block_hashes = vec![];
    for i in 1..=BLOCK_CNT {
        let block = new_dummy_block_with_payload(|header| {
            header.set_height(NonZeroU64::new(i as u64).unwrap());
            header.set_prev_block_hash(block_hashes.last().copied());
        });

        let mut state_block = state.block(block.as_ref().header());
        block_hashes.push(block.as_ref().hash());
        let _events = state_block.apply(&block, Vec::new());
        state_block.commit().unwrap();
    }

    assert!(
        state
            .view()
            .block_hashes()
            .iter()
            .skip_while(|&x| *x != block_hashes[6])
            .skip(1)
            .copied()
            .collect::<Vec<_>>()
            .into_iter()
            .eq(block_hashes.into_iter().skip(7))
    );
}

#[test]
fn block_hashes_commit_applies_pending_only_on_commit() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(World::default(), kura, query_handle);

    let initial_len = state.block_hashes.view().len();
    let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
    let hash = header.hash();

    {
        let mut block_hashes = state.block_hashes.block();
        block_hashes.push(hash);
        assert_eq!(
            state.block_hashes.view().len(),
            initial_len,
            "pending block hash should not be visible before commit"
        );
        block_hashes.commit_for_tests();
    }

    let view = state.block_hashes.view();
    assert_eq!(view.len(), initial_len + 1);
    assert_eq!(view.iter().last().copied(), Some(hash));
}

#[test]
fn block_hashes_block_and_revert_replaces_tail_on_commit() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(World::default(), kura, query_handle);

    let first_header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
    let first_hash = first_header.hash();
    {
        let mut block_hashes = state.block_hashes.block();
        block_hashes.push(first_hash);
        block_hashes.commit_for_tests();
    }

    let replacement_header = BlockHeader::new(
        NonZeroU64::new(2).unwrap(),
        Some(first_hash),
        None,
        None,
        0,
        0,
    );
    let replacement_hash = replacement_header.hash();
    {
        let mut block_hashes = state.block_hashes.block_and_revert();
        assert_eq!(block_hashes.len(), 0, "revert view should drop tail");
        block_hashes.push(replacement_hash);
        block_hashes.commit_for_tests();
    }

    let view = state.block_hashes.view();
    assert_eq!(view.len(), 1);
    assert_eq!(view.iter().last().copied(), Some(replacement_hash));
}

#[test]
fn block_hashes_prepare_commit_releases_read_lock() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(World::default(), kura, query_handle);

    let mut block_hashes = state.block_hashes.block();
    assert_eq!(block_hashes.len(), 0);
    assert!(
        state.block_hashes.inner.try_write().is_none(),
        "block-scoped snapshot should pin reads until commit preparation"
    );
    block_hashes.prepare_commit();
    assert!(
        state.block_hashes.inner.try_write().is_some(),
        "prepare_commit should release the snapshot read guard before commit"
    );
}

#[test]
fn block_hashes_committed_height_cache_tracks_commits() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(World::default(), kura, query_handle);

    assert_eq!(state.committed_height(), 0);
    assert_eq!(
        state.committed_height(),
        state.block_hashes.view().len(),
        "cached committed height must match block-hash journal length at genesis"
    );

    let first_hash = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0).hash();
    {
        let mut block_hashes = state.block_hashes.block();
        block_hashes.push(first_hash);
        block_hashes.commit_for_tests();
    }
    assert_eq!(
        state.committed_height(),
        state.block_hashes.view().len(),
        "cached committed height must be refreshed after block commit"
    );

    let replacement_hash = BlockHeader::new(
        NonZeroU64::new(2).unwrap(),
        Some(first_hash),
        None,
        None,
        0,
        0,
    )
    .hash();
    {
        let mut block_hashes = state.block_hashes.block_and_revert();
        block_hashes.push(replacement_hash);
        block_hashes.commit_for_tests();
    }
    assert_eq!(
        state.committed_height(),
        state.block_hashes.view().len(),
        "cached committed height must track block-and-revert commits"
    );
}

#[test]
fn state_contains_ivm_runtime() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(World::default(), kura, query_handle);
    assert_eq!(state.ivm.gas_remaining, 0);
}

#[tokio::test]
async fn get_blocks_from_height() {
    const BLOCK_CNT: usize = 10;

    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(World::default(), kura.clone(), query_handle);

    for i in 1..=BLOCK_CNT {
        let block = new_dummy_block_with_payload(|header| {
            header.set_height(NonZeroU64::new(i as u64).unwrap());
        });

        let mut state_block = state.block(block.as_ref().header());
        let _events = state_block.apply(&block, Vec::new());
        state_block.commit().unwrap();
        kura.store_block(block).expect("store block");
    }

    assert_eq!(
        &state
            .view()
            .all_blocks(nonzero!(8_usize))
            .map(|block| block.header().height().get())
            .collect::<Vec<_>>(),
        &[8, 9, 10]
    );
}

#[tokio::test]
async fn all_blocks_skips_missing_kura_entries() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(World::default(), kura.clone(), query_handle);

    for height in 1..=3_u64 {
        let block = new_dummy_block_with_payload(|header| {
            header.set_height(NonZeroU64::new(height).unwrap());
        });

        let mut state_block = state.block(block.as_ref().header());
        let _events = state_block.apply(&block, Vec::new());
        state_block.commit().unwrap();

        if height != 3 {
            kura.store_block(block).expect("store block");
        }
    }

    let heights: Vec<_> = state
        .view()
        .all_blocks(nonzero!(1_usize))
        .map(|block| block.header().height().get())
        .collect();
    assert_eq!(heights, vec![1, 2]);
}

#[test]
fn role_account_range() {
    let (account_id, _account_keypair) = gen_account_in("wonderland");
    let roles = [
        RoleIdWithOwner::new(account_id.clone(), "1".parse().unwrap()),
        RoleIdWithOwner::new(account_id.clone(), "2".parse().unwrap()),
        RoleIdWithOwner::new(gen_account_in("wonderland").0, "3".parse().unwrap()),
        RoleIdWithOwner::new(gen_account_in("wonderland").0, "4".parse().unwrap()),
        RoleIdWithOwner::new(gen_account_in("0").0, "5".parse().unwrap()),
        RoleIdWithOwner::new(gen_account_in("1").0, "6".parse().unwrap()),
    ]
    .map(|role| (role, ()));
    let map = Storage::from_iter(roles);

    let view = map.view();
    let range = view
        .range(RoleIdByAccountBounds::new(&account_id))
        .collect::<Vec<_>>();
    assert_eq!(range.len(), 2);
    for (role, ()) in range {
        assert_eq!(&role.account, &account_id);
    }
}

#[test]
fn asset_account_range() {
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();

    let account_id = gen_account_in("wonderland").0;

    let accounts = [
        account_id.clone(),
        account_id.clone(),
        gen_account_in("a").0,
        gen_account_in("b").0,
        gen_account_in("z").0,
        gen_account_in("z").0,
    ];
    let asset_definitions = [
        AssetDefinitionId::new(domain_id.clone(), "a".parse().unwrap()),
        AssetDefinitionId::new(domain_id.clone(), "f".parse().unwrap()),
        AssetDefinitionId::new(domain_id.clone(), "b".parse().unwrap()),
        AssetDefinitionId::new(domain_id.clone(), "c".parse().unwrap()),
        AssetDefinitionId::new(domain_id.clone(), "d".parse().unwrap()),
        AssetDefinitionId::new(domain_id.clone(), "e".parse().unwrap()),
    ];

    let mut assets = accounts
        .into_iter()
        .zip(asset_definitions)
        .map(|(account, asset_definition)| AssetId::new(asset_definition, account))
        .map(|asset| (asset, ()))
        .collect::<Vec<_>>();
    assets.push((
        AssetId::with_scope(
            AssetDefinitionId::new(domain_id, "g".parse().unwrap()),
            account_id.clone(),
            AssetBalanceScope::Dataspace(iroha_data_model::nexus::DataSpaceId::new(7)),
        ),
        (),
    ));

    let map: Storage<_, _> = assets.into_iter().collect();
    let view = map.view();
    let range = view.range(AssetByAccountBounds::new(&account_id));
    assert_eq!(range.count(), 3);
}

#[test]
fn asset_account_definition_range_includes_all_scopes() {
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let account_id = gen_account_in("wonderland").0;
    let target_definition = AssetDefinitionId::new(domain_id.clone(), "rose".parse().unwrap());
    let other_definition = AssetDefinitionId::new(domain_id, "tulip".parse().unwrap());

    let assets = [
        AssetId::new(target_definition.clone(), account_id.clone()),
        AssetId::with_scope(
            target_definition.clone(),
            account_id.clone(),
            AssetBalanceScope::Dataspace(iroha_data_model::nexus::DataSpaceId::new(7)),
        ),
        AssetId::new(other_definition, account_id.clone()),
        AssetId::new(target_definition.clone(), gen_account_in("other").0),
    ]
    .map(|asset_id| (asset_id, ()));

    let map: Storage<_, _> = assets.into_iter().collect();
    let view = map.view();
    let range = view
        .range::<dyn AsAssetIdAccountDefinitionCompare>(AssetByAccountDefinitionBounds::new(
            &account_id,
            &target_definition,
        ))
        .collect::<Vec<_>>();

    assert_eq!(range.len(), 2, "global + scoped partitions should match");
    for (asset_id, ()) in range {
        assert_eq!(asset_id.account(), &account_id);
        assert_eq!(asset_id.definition(), &target_definition);
    }
}

#[test]
fn set_asset_metadata_inserts_value_and_event() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new(World::default(), kura, query);

    let block = new_dummy_block_with_payload(|_| {});
    let mut state_block = state.block(block.as_ref().header());
    let mut stx = state_block.transaction();

    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    Register::domain(Domain::new(domain_id.clone()))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
    Register::account(new_sample_account(&ALICE_ID))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
    let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("wonderland", "universal").unwrap(),
        "rose".parse().unwrap(),
    );
    Register::asset_definition({
        let __asset_definition_id = asset_def_id.clone();
        AssetDefinition::numeric(__asset_definition_id.clone())
            .with_name(__asset_definition_id.name().to_string())
    })
    .execute(&ALICE_ID, &mut stx)
    .unwrap();
    let asset_id = AssetId::new(asset_def_id.clone(), ALICE_ID.clone());
    Mint::asset_quantity(1_u32, asset_id.clone())
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

    let key: Name = "note".parse().unwrap();
    let value = Json::from(norito::json!("important"));
    SetAssetKeyValue::new(asset_id.clone(), key.clone(), value.clone())
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

    let events = stx.world.take_external_events();
    assert!(
        events.iter().any(|event| {
            if let EventBox::Data(ev) = event
                && let data_pre::DataEvent::Domain(data_pre::DomainEvent::Account(
                    data_pre::AccountEvent::Asset(data_pre::AssetEvent::MetadataInserted(mc)),
                )) = ev.as_ref()
            {
                return *mc.target() == asset_id && mc.key() == &key && mc.value() == &value;
            }
            false
        }),
        "expected Asset::MetadataInserted event"
    );

    let metadata = stx.world.asset_metadata_mut_or_default(&asset_id).unwrap();
    assert_eq!(metadata.get(&key), Some(&value));
}

#[test]
fn remove_asset_metadata_emits_event_and_clears_entry() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new(World::default(), kura, query);

    let block = new_dummy_block_with_payload(|_| {});
    let mut state_block = state.block(block.as_ref().header());
    let mut stx = state_block.transaction();

    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    Register::domain(Domain::new(domain_id.clone()))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
    Register::account(new_sample_account(&ALICE_ID))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
    let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("wonderland", "universal").unwrap(),
        "rose".parse().unwrap(),
    );
    Register::asset_definition({
        let __asset_definition_id = asset_def_id.clone();
        AssetDefinition::numeric(__asset_definition_id.clone())
            .with_name(__asset_definition_id.name().to_string())
    })
    .execute(&ALICE_ID, &mut stx)
    .unwrap();
    let asset_id = AssetId::new(asset_def_id.clone(), ALICE_ID.clone());
    Mint::asset_quantity(1_u32, asset_id.clone())
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

    let key: Name = "flag".parse().unwrap();
    let value = Json::from(norito::json!(true));
    SetAssetKeyValue::new(asset_id.clone(), key.clone(), value.clone())
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
    stx.world.take_external_events();

    let missing_key: Name = "missing".parse().unwrap();
    let err = RemoveAssetKeyValue::new(asset_id.clone(), missing_key.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect_err("removing absent key should fail");
    assert!(matches!(
        err,
        Error::Find(FindError::MetadataKey(m)) if m == missing_key
    ));

    RemoveAssetKeyValue::new(asset_id.clone(), key.clone())
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

    let events = stx.world.take_external_events();
    assert!(
        events.iter().any(|event| {
            if let EventBox::Data(ev) = event
                && let data_pre::DataEvent::Domain(data_pre::DomainEvent::Account(
                    data_pre::AccountEvent::Asset(data_pre::AssetEvent::MetadataRemoved(mc)),
                )) = ev.as_ref()
            {
                return *mc.target() == asset_id && mc.key() == &key && mc.value() == &value;
            }
            false
        }),
        "expected Asset::MetadataRemoved event"
    );

    assert!(stx.world.asset_metadata.get(&asset_id).is_none());
}

#[tokio::test]
async fn new_for_testing_works() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query_handle);
    // Basic smoke check: view is accessible
    let _ = state.view();
}

#[test]
fn test_constructors_seed_exact_kura_lane_markers() {
    let assert_marker = |state: &State, kura: &Arc<Kura>| {
        let lane_id = LaneId::SINGLE;
        let nexus = state.nexus_snapshot();
        let entry = nexus
            .lane_config
            .entry(lane_id)
            .expect("default lane entry");
        let incarnation = state
            .lane_incarnation(lane_id)
            .expect("default lane incarnation");
        let (session, signer_pops) = sample_committed_lane_block_session_for_state_test(
            lane_id,
            entry.dataspace_id,
            incarnation,
            1,
            1,
        );
        kura.persist_committed_lane_block_session(&session, &signer_pops)
            .expect("test constructor must install its exact active lane marker");
    };

    let kura = Kura::blank_kura_for_testing();
    let state = State::new(
        World::default(),
        Arc::clone(&kura),
        LiveQueryStore::start_test(),
    );
    assert_marker(&state, &kura);

    let kura = Kura::blank_kura_for_testing();
    let state = State::new_with_chain(
        World::default(),
        Arc::clone(&kura),
        LiveQueryStore::start_test(),
        ChainId::from("state-marker-new-with-chain"),
    );
    assert_marker(&state, &kura);

    let kura = Kura::blank_kura_for_testing();
    let state = State::with_telemetry(
        World::default(),
        Arc::clone(&kura),
        LiveQueryStore::start_test(),
        StateTelemetry::default(),
    );
    assert_marker(&state, &kura);

    let kura = Kura::blank_kura_for_testing();
    let state = State::new_for_testing(
        World::default(),
        Arc::clone(&kura),
        LiveQueryStore::start_test(),
    );
    assert_marker(&state, &kura);
}

#[test]
fn elections_mut_seeds_storage() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query_handle);

    let block = new_dummy_block_with_payload(|_| {});
    let mut state_block = state.block(block.as_ref().header());
    let mut stx = state_block.transaction();

    let election_id = "election-1".to_string();
    stx.world
        .elections_mut()
        .insert(election_id.clone(), ElectionState::default());
    stx.apply();
    state_block.commit().expect("commit block");

    let view = state.view();
    assert!(view.world.elections().get(&election_id).is_some());
}

#[test]
fn soracloud_runtime_records_are_visible_through_world_view() {
    let mut world = World::new();
    let service_name: Name = "portal".parse().expect("valid name");
    let service_version = "2026.1".to_string();
    let revision = SoraDeploymentBundleV1 {
        schema_version: iroha_data_model::soracloud::SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        container: iroha_data_model::soracloud::SoraContainerManifestV1 {
            schema_version: iroha_data_model::soracloud::SORA_CONTAINER_MANIFEST_VERSION_V1,
            runtime: iroha_data_model::soracloud::SoraContainerRuntimeV1::Ivm,
            bundle_hash: Hash::new(b"bundle"),
            bundle_path: "/bundle/service.ivm".to_string(),
            entrypoint: "main".to_string(),
            args: Vec::new(),
            env: std::collections::BTreeMap::new(),
            inrou: None,
            required_config_names: Vec::new(),
            required_secret_names: Vec::new(),
            config_exports: Vec::new(),
            capabilities: iroha_data_model::soracloud::SoraCapabilityPolicyV1 {
                network: iroha_data_model::soracloud::SoraNetworkPolicyV1::Isolated,
                allow_wallet_signing: false,
                allow_state_writes: false,
                allow_model_inference: false,
                allow_model_training: false,
            },
            resources: iroha_data_model::soracloud::SoraResourceLimitsV1 {
                cpu_millis: std::num::NonZeroU32::new(500).expect("nonzero"),
                memory_bytes: std::num::NonZeroU64::new(16 * 1024 * 1024).expect("nonzero"),
                ephemeral_storage_bytes: std::num::NonZeroU64::new(16 * 1024 * 1024)
                    .expect("nonzero"),
                max_open_files: std::num::NonZeroU32::new(256).expect("nonzero"),
                max_tasks: std::num::NonZeroU16::new(16).expect("nonzero"),
            },
            lifecycle: iroha_data_model::soracloud::SoraLifecycleHooksV1 {
                start_grace_secs: std::num::NonZeroU32::new(10).expect("nonzero"),
                stop_grace_secs: std::num::NonZeroU32::new(10).expect("nonzero"),
                healthcheck_path: Some("/health".to_string()),
            },
        },
        service: iroha_data_model::soracloud::SoraServiceManifestV1 {
            schema_version: iroha_data_model::soracloud::SORA_SERVICE_MANIFEST_VERSION_V1,
            service_name: service_name.clone(),
            service_version: service_version.clone(),
            execution_plane:
                iroha_data_model::soracloud::SoraServiceExecutionPlaneV1::DeterministicService,
            container: iroha_data_model::soracloud::SoraContainerManifestRefV1 {
                manifest_hash: Hash::new(b"container"),
                expected_schema_version:
                    iroha_data_model::soracloud::SORA_CONTAINER_MANIFEST_VERSION_V1,
            },
            replicas: std::num::NonZeroU16::new(1).expect("nonzero"),
            route: None,
            rollout: iroha_data_model::soracloud::SoraRolloutPolicyV1 {
                canary_percent: 0,
                max_unavailable_replicas: 0,
                health_window_secs: std::num::NonZeroU32::new(30).expect("nonzero"),
                automatic_rollback_failures: std::num::NonZeroU32::new(1).expect("nonzero"),
            },
            economics: iroha_data_model::soracloud::SoraHttpServiceEconomicsV1::default(),
            state_bindings: Vec::new(),
            lease_volumes: Vec::new(),
            handlers: vec![iroha_data_model::soracloud::SoraServiceHandlerV1 {
                handler_name: "query".parse().expect("valid name"),
                class: iroha_data_model::soracloud::SoraServiceHandlerClassV1::Query,
                entrypoint: "serve_query".to_string(),
                route_path: Some("/query".to_string()),
                certified_response:
                    iroha_data_model::soracloud::SoraCertifiedResponsePolicyV1::AuditReceipt,
                mailbox: None,
            }],
            artifacts: vec![iroha_data_model::soracloud::SoraArtifactRefV1 {
                kind: iroha_data_model::soracloud::SoraArtifactKindV1::StaticAsset,
                artifact_hash: Hash::new(b"asset"),
                artifact_path: "/public/index.html".to_string(),
                handler_name: Some("query".parse().expect("valid name")),
            }],
        },
    };
    world.soracloud_service_revisions_mut_for_testing().insert(
        (service_name.as_ref().to_owned(), service_version.clone()),
        revision,
    );
    world
        .soracloud_service_deployments_mut_for_testing()
        .insert(
            service_name.clone(),
            iroha_data_model::soracloud::SoraServiceDeploymentStateV1 {
                schema_version:
                    iroha_data_model::soracloud::SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
                service_name: service_name.clone(),
                current_service_version: service_version.clone(),
                current_service_manifest_hash: Hash::new(b"service-manifest"),
                current_container_manifest_hash: Hash::new(b"container-manifest"),
                revision_count: 1,
                process_generation: 1,
                process_started_sequence: 4,
                active_rollout: None,
                last_rollout: None,
                config_generation: 0,
                secret_generation: 0,
                service_configs: std::collections::BTreeMap::new(),
                service_secrets: std::collections::BTreeMap::new(),
                service_lease: None,
                lease_volume_states: Vec::new(),
            },
        );
    world.soracloud_service_runtime_mut_for_testing().insert(
        service_name.clone(),
        SoraServiceRuntimeStateV1 {
            schema_version: iroha_data_model::soracloud::SORA_SERVICE_RUNTIME_STATE_VERSION_V1,
            service_name: service_name.clone(),
            active_service_version: service_version.clone(),
            health_status: iroha_data_model::soracloud::SoraServiceHealthStatusV1::Healthy,
            load_factor_bps: 250,
            materialized_bundle_hash: Hash::new(b"bundle"),
            rollout_handle: Some("rollout-1".to_string()),
            pending_mailbox_message_count: 1,
            last_receipt_id: Some(Hash::new(b"receipt")),
        },
    );
    world
        .soracloud_service_audit_events_mut_for_testing()
        .insert(
            4,
            iroha_data_model::soracloud::SoraServiceAuditEventV1 {
                schema_version: iroha_data_model::soracloud::SORA_SERVICE_AUDIT_EVENT_VERSION_V1,
                sequence: 4,
                action: iroha_data_model::soracloud::SoraServiceLifecycleActionV1::Deploy,
                service_name: service_name.clone(),
                from_version: None,
                to_version: service_version.clone(),
                service_manifest_hash: Hash::new(b"service-manifest"),
                container_manifest_hash: Hash::new(b"container-manifest"),
                governance_tx_hash: None,
                binding_name: None,
                state_key: None,
                config_name: None,
                secret_name: None,
                rollout_handle: None,
                policy_name: None,
                policy_snapshot_hash: None,
                jurisdiction_tag: None,
                consent_evidence_hash: None,
                break_glass: None,
                break_glass_reason: None,
                signer: crate::state::checked_keypair().public_key().clone(),
            },
        );
    world
        .soracloud_service_state_entries_mut_for_testing()
        .insert(
            (
                service_name.as_ref().to_owned(),
                "vault".to_string(),
                "/state/private/patient-1".to_string(),
            ),
            iroha_data_model::soracloud::SoraServiceStateEntryV1 {
                schema_version: iroha_data_model::soracloud::SORA_SERVICE_STATE_ENTRY_VERSION_V1,
                service_name: service_name.clone(),
                service_version: service_version.clone(),
                binding_name: "vault".parse().expect("valid name"),
                state_key: "/state/private/patient-1".to_string(),
                encryption: iroha_data_model::soracloud::SoraStateEncryptionV1::FheCiphertext,
                payload: b"ciphertext".to_vec(),
                payload_bytes: std::num::NonZeroU64::new(10).expect("nonzero"),
                payload_commitment: Hash::new(b"ciphertext"),
                fhe_public_key_digest: None,
                fhe_residual_multiple_bound: None,
                fhe_bound_mode: None,
                last_update_sequence: 4,
                governance_tx_hash: Hash::new(b"gov"),
                source_action:
                    iroha_data_model::soracloud::SoraServiceLifecycleActionV1::StateMutation,
            },
        );
    world
        .soracloud_decryption_request_records_mut_for_testing()
        .insert(
            (service_name.as_ref().to_owned(), "decrypt-1".to_string()),
            iroha_data_model::soracloud::SoraDecryptionRequestRecordV1 {
                schema_version:
                    iroha_data_model::soracloud::SORA_DECRYPTION_REQUEST_RECORD_VERSION_V1,
                service_name: service_name.clone(),
                service_version: service_version.clone(),
                policy: iroha_data_model::soracloud::DecryptionAuthorityPolicyV1 {
                    schema_version:
                        iroha_data_model::soracloud::DECRYPTION_AUTHORITY_POLICY_VERSION_V1,
                    policy_name: "phi_policy".parse().expect("valid name"),
                    mode: iroha_data_model::soracloud::DecryptionAuthorityModeV1::ThresholdService,
                    approver_quorum: std::num::NonZeroU16::new(1).expect("nonzero"),
                    approver_ids: vec!["approver".parse().expect("valid name")],
                    allow_break_glass: true,
                    jurisdiction_tag: "us_hipaa".to_string(),
                    require_consent_evidence: false,
                    max_ttl_blocks: std::num::NonZeroU32::new(64).expect("nonzero"),
                    audit_tag: "phi.access".to_string(),
                },
                request: iroha_data_model::soracloud::DecryptionRequestV1 {
                    schema_version: iroha_data_model::soracloud::DECRYPTION_REQUEST_VERSION_V1,
                    request_id: "decrypt-1".to_string(),
                    policy_name: "phi_policy".parse().expect("valid name"),
                    binding_name: "vault".parse().expect("valid name"),
                    state_key: "/state/private/patient-1".to_string(),
                    ciphertext_commitment: Hash::new(b"ciphertext"),
                    justification: "care review".to_string(),
                    jurisdiction_tag: "us_hipaa".to_string(),
                    consent_evidence_hash: None,
                    requested_ttl_blocks: std::num::NonZeroU32::new(32).expect("nonzero"),
                    break_glass: false,
                    break_glass_reason: None,
                    governance_tx_hash: Hash::new(b"gov"),
                },
                sequence: 5,
                signer: crate::state::checked_keypair().public_key().clone(),
            },
        );
    world.soracloud_training_jobs_mut_for_testing().insert(
        (service_name.as_ref().to_owned(), "job-1".to_string()),
        iroha_data_model::soracloud::SoraTrainingJobRecordV1 {
            schema_version: iroha_data_model::soracloud::SORA_TRAINING_JOB_RECORD_VERSION_V1,
            service_name: service_name.clone(),
            service_version: service_version.clone(),
            model_name: "vision_model".to_string(),
            job_id: "job-1".to_string(),
            status: iroha_data_model::soracloud::SoraTrainingJobStatusV1::Completed,
            worker_group_size: 4,
            target_steps: 100,
            completed_steps: 100,
            checkpoint_interval_steps: 20,
            last_checkpoint_step: Some(100),
            checkpoint_count: 5,
            retry_count: 1,
            max_retries: 3,
            step_compute_units: 50,
            compute_budget_units: 40_000,
            compute_consumed_units: 20_000,
            storage_budget_bytes: 8_192,
            storage_consumed_bytes: 4_096,
            latest_metrics_hash: Some(Hash::new(b"metrics")),
            last_failure_reason: None,
            created_sequence: 6,
            updated_sequence: 8,
        },
    );
    world
        .soracloud_training_job_audit_events_mut_for_testing()
        .insert(
            8,
            iroha_data_model::soracloud::SoraTrainingJobAuditEventV1 {
                schema_version:
                    iroha_data_model::soracloud::SORA_TRAINING_JOB_AUDIT_EVENT_VERSION_V1,
                sequence: 8,
                action: iroha_data_model::soracloud::SoraTrainingJobActionV1::Checkpoint,
                service_name: service_name.clone(),
                service_version: service_version.clone(),
                model_name: "vision_model".to_string(),
                job_id: "job-1".to_string(),
                status: iroha_data_model::soracloud::SoraTrainingJobStatusV1::Completed,
                completed_steps: 100,
                checkpoint_count: 5,
                retry_count: 1,
                compute_consumed_units: 20_000,
                storage_consumed_bytes: 4_096,
                last_checkpoint_step: Some(100),
                latest_metrics_hash: Some(Hash::new(b"metrics")),
                last_failure_reason: None,
                signer: crate::state::checked_keypair().public_key().clone(),
            },
        );
    world.soracloud_model_registries_mut_for_testing().insert(
        (service_name.as_ref().to_owned(), "vision_model".to_string()),
        iroha_data_model::soracloud::SoraModelRegistryV1 {
            schema_version: iroha_data_model::soracloud::SORA_MODEL_REGISTRY_VERSION_V1,
            service_name: service_name.clone(),
            service_version: service_version.clone(),
            model_name: "vision_model".to_string(),
            current_version: Some("v2".to_string()),
            updated_sequence: 10,
        },
    );
    world
        .soracloud_model_weight_versions_mut_for_testing()
        .insert(
            (
                service_name.as_ref().to_owned(),
                "vision_model".to_string(),
                "v2".to_string(),
            ),
            iroha_data_model::soracloud::SoraModelWeightVersionRecordV1 {
                schema_version:
                    iroha_data_model::soracloud::SORA_MODEL_WEIGHT_VERSION_RECORD_VERSION_V1,
                service_name: service_name.clone(),
                service_version: service_version.clone(),
                model_name: "vision_model".to_string(),
                weight_version: "v2".to_string(),
                parent_version: Some("v1".to_string()),
                training_job_id: "job-1".to_string(),
                source_provenance: Some(iroha_data_model::soracloud::SoraModelProvenanceRefV1 {
                    kind: iroha_data_model::soracloud::SoraModelProvenanceKindV1::TrainingJob,
                    id: "job-1".to_string(),
                }),
                weight_artifact_hash: Hash::new(b"weights"),
                dataset_ref: "dataset://train".to_string(),
                training_config_hash: Hash::new(b"train-config"),
                reproducibility_hash: Hash::new(b"repro"),
                provenance_attestation_hash: Hash::new(b"prov"),
                registered_sequence: 9,
                promoted_sequence: Some(10),
                gate_report_hash: Some(Hash::new(b"gate")),
                promoted_by: Some(crate::state::checked_keypair().public_key().clone()),
            },
        );
    world
        .soracloud_model_weight_audit_events_mut_for_testing()
        .insert(
            10,
            iroha_data_model::soracloud::SoraModelWeightAuditEventV1 {
                schema_version:
                    iroha_data_model::soracloud::SORA_MODEL_WEIGHT_AUDIT_EVENT_VERSION_V1,
                sequence: 10,
                action: iroha_data_model::soracloud::SoraModelWeightActionV1::Promote,
                service_name: service_name.clone(),
                service_version: service_version.clone(),
                model_name: "vision_model".to_string(),
                target_version: "v2".to_string(),
                current_version: Some("v2".to_string()),
                parent_version: Some("v1".to_string()),
                gate_approved: Some(true),
                rollback_reason: None,
                signer: crate::state::checked_keypair().public_key().clone(),
            },
        );
    world.soracloud_model_artifacts_mut_for_testing().insert(
        (service_name.as_ref().to_owned(), "job-1".to_string()),
        iroha_data_model::soracloud::SoraModelArtifactRecordV1 {
            schema_version: iroha_data_model::soracloud::SORA_MODEL_ARTIFACT_RECORD_VERSION_V1,
            service_name: service_name.clone(),
            service_version: service_version.clone(),
            model_name: "vision_model".to_string(),
            artifact_id: "job-1".to_string(),
            training_job_id: "job-1".to_string(),
            weight_version: Some("v2".to_string()),
            source_provenance: Some(iroha_data_model::soracloud::SoraModelProvenanceRefV1 {
                kind: iroha_data_model::soracloud::SoraModelProvenanceKindV1::TrainingJob,
                id: "job-1".to_string(),
            }),
            weight_artifact_hash: Hash::new(b"weights"),
            dataset_ref: "dataset://train".to_string(),
            training_config_hash: Hash::new(b"train-config"),
            reproducibility_hash: Hash::new(b"repro"),
            provenance_attestation_hash: Hash::new(b"prov"),
            registered_sequence: 9,
            consumed_by_version: Some("v2".to_string()),
            chunk_manifest_root: None,
        },
    );
    world
        .soracloud_model_artifact_audit_events_mut_for_testing()
        .insert(
            9,
            iroha_data_model::soracloud::SoraModelArtifactAuditEventV1 {
                schema_version:
                    iroha_data_model::soracloud::SORA_MODEL_ARTIFACT_AUDIT_EVENT_VERSION_V1,
                sequence: 9,
                action: iroha_data_model::soracloud::SoraModelArtifactActionV1::Register,
                service_name: service_name.clone(),
                service_version: service_version.clone(),
                model_name: "vision_model".to_string(),
                training_job_id: "job-1".to_string(),
                consumed_by_version: Some("v2".to_string()),
                signer: crate::state::checked_keypair().public_key().clone(),
            },
        );
    world.soracloud_mailbox_messages_mut_for_testing().insert(
        Hash::new(b"message"),
        SoraServiceMailboxMessageV1 {
            schema_version: iroha_data_model::soracloud::SORA_SERVICE_MAILBOX_MESSAGE_VERSION_V1,
            message_id: Hash::new(b"message"),
            from_service: service_name.clone(),
            from_handler: "query".parse().expect("valid name"),
            to_service: service_name.clone(),
            to_handler: "query".parse().expect("valid name"),
            payload_bytes: b"payload".to_vec(),
            payload_commitment: Hash::new(b"payload"),
            enqueue_sequence: 4,
            available_after_sequence: 4,
            expires_at_sequence: Some(8),
        },
    );
    world.soracloud_runtime_receipts_mut_for_testing().insert(
        Hash::new(b"receipt"),
        SoraRuntimeReceiptV1 {
            schema_version: iroha_data_model::soracloud::SORA_RUNTIME_RECEIPT_VERSION_V1,
            receipt_id: Hash::new(b"receipt"),
            service_name: service_name.clone(),
            service_version: service_version.clone(),
            handler_name: "query".parse().expect("valid name"),
            handler_class: iroha_data_model::soracloud::SoraServiceHandlerClassV1::Query,
            request_commitment: Hash::new(b"request"),
            result_commitment: Hash::new(b"result"),
            certified_by: iroha_data_model::soracloud::SoraCertifiedResponsePolicyV1::AuditReceipt,
            emitted_sequence: 5,
            mailbox_message_id: None,
            journal_artifact_hash: None,
            checkpoint_artifact_hash: None,
            placement_id: None,
            selected_validator_account_id: None,
            selected_peer_id: None,
        },
    );

    let view = world.view();
    assert!(
        view.soracloud_service_revisions()
            .get(&(service_name.as_ref().to_owned(), service_version.clone()))
            .is_some()
    );
    assert!(
        view.soracloud_service_deployments()
            .get(&service_name)
            .is_some()
    );
    assert_eq!(
        view.soracloud_service_runtime()
            .get(&service_name)
            .expect("runtime state")
            .pending_mailbox_message_count,
        1
    );
    assert!(view.soracloud_service_audit_events().get(&4).is_some());
    assert!(
        view.soracloud_service_state_entries()
            .get(&(
                service_name.as_ref().to_owned(),
                "vault".to_string(),
                "/state/private/patient-1".to_string(),
            ))
            .is_some()
    );
    assert!(
        view.soracloud_decryption_request_records()
            .get(&(service_name.as_ref().to_owned(), "decrypt-1".to_string()))
            .is_some()
    );
    assert!(
        view.soracloud_training_jobs()
            .get(&(service_name.as_ref().to_owned(), "job-1".to_string()))
            .is_some()
    );
    assert!(view.soracloud_training_job_audit_events().get(&8).is_some());
    assert!(
        view.soracloud_model_registries()
            .get(&(service_name.as_ref().to_owned(), "vision_model".to_string()))
            .is_some()
    );
    assert!(
        view.soracloud_model_weight_versions()
            .get(&(
                service_name.as_ref().to_owned(),
                "vision_model".to_string(),
                "v2".to_string(),
            ))
            .is_some()
    );
    assert!(
        view.soracloud_model_weight_audit_events()
            .get(&10)
            .is_some()
    );
    assert!(
        view.soracloud_model_artifacts()
            .get(&(service_name.as_ref().to_owned(), "job-1".to_string()))
            .is_some()
    );
    assert!(
        view.soracloud_model_artifact_audit_events()
            .get(&9)
            .is_some()
    );
    assert!(
        view.soracloud_mailbox_messages()
            .get(&Hash::new(b"message"))
            .is_some()
    );
    assert!(
        view.soracloud_runtime_receipts()
            .get(&Hash::new(b"receipt"))
            .is_some()
    );
}

#[tokio::test]
async fn new_for_testing_uses_config_chain_id() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query_handle);

    assert_eq!(state.chain_id, *super::DEFAULT_TEST_CHAIN_ID);
}
