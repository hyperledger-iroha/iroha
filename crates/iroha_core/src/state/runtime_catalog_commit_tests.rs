fn catalog_test_header() -> BlockHeader {
    BlockHeader::new(NonZeroU64::new(2).unwrap(), None, None, None, 0, 0)
}

fn staged_catalog_fixture(
    state: &State,
    keys: &[iroha_crypto::KeyPair],
) -> (
    iroha_config::parameters::actual::Nexus,
    PendingAutoscaleLaneLifecycle,
) {
    let payload = catalog_payload(state, keys);
    let mut block = state.block(catalog_test_header());
    let mut transaction = block.transaction();
    transaction
        .stage_consensus_catalog_transition(&payload)
        .unwrap();
    (
        transaction.nexus.clone(),
        transaction.pending_lane_lifecycle.clone().unwrap(),
    )
}

fn install_fixture_runtime(state: &State, runtime: NexusRuntimeCatalogV1) {
    let mut world = state.world.block();
    world
        .parameters
        .get_mut()
        .set_parameter(iroha_data_model::parameter::Parameter::Custom(
            runtime.into_custom_parameter().unwrap(),
        ));
    world.commit();
}

#[test]
fn runtime_catalog_readback_tracks_committed_state_and_rejects_malformed_parameter() {
    run_catalog_test(|| {
        let (state, keys) = catalog_fixture(InvalidMember::None);
        assert_eq!(state.view().runtime_catalog_hash().unwrap(), None);
        let payload = catalog_payload(&state, &keys);
        let runtime = {
            let mut block = state.block(catalog_test_header());
            let mut transaction = block.transaction();
            transaction
                .stage_consensus_catalog_transition(&payload)
                .unwrap();
            let runtime = runtime_catalog_from_world(&transaction.world)
                .unwrap()
                .unwrap();
            assert_eq!(
                state.view().runtime_catalog_hash().unwrap(),
                None,
                "an uncommitted transaction cannot become public readback"
            );
            runtime
        };
        assert_eq!(state.view().runtime_catalog_hash().unwrap(), None);
        let first = runtime.canonical_hash().unwrap();
        install_fixture_runtime(&state, runtime.clone());
        assert_eq!(state.view().runtime_catalog_hash().unwrap(), Some(first));
        let mut changed = runtime;
        changed.dataspaces[0].descriptor.description = Some("changed committed description".into());
        let second = changed.canonical_hash().unwrap();
        assert_ne!(first, second);
        install_fixture_runtime(&state, changed);
        assert_eq!(state.view().runtime_catalog_hash().unwrap(), Some(second));
        let mut world = state.world.block();
        world
            .parameters
            .get_mut()
            .set_parameter(iroha_data_model::parameter::Parameter::Custom(
                iroha_data_model::parameter::CustomParameter::new(
                    NexusRuntimeCatalogV1::parameter_id(),
                    iroha_primitives::json::Json::new(norito::json!({"version": 255})),
                ),
            ));
        world.commit();
        assert!(state.view().runtime_catalog_hash().is_err());
    });
}

#[test]
fn runtime_catalog_readback_binds_next_transition_and_rejects_stale_root() {
    run_catalog_test(|| {
        let (state, keys) = catalog_fixture(InvalidMember::None);
        let (after, pending) = staged_catalog_fixture(&state, &keys);
        let runtime = pending.runtime_catalog.clone().unwrap();
        install_fixture_runtime(&state, runtime.clone());
        // Restore one complete committed fixture before reading the next transition's inputs.
        *state.nexus.write() = after;
        *state.lane_incarnations.write() = pending.catalog_update.updated_lane_incarnations.clone();
        *state.lane_incarnation_lineage.write() = pending
            .catalog_update
            .updated_lane_incarnation_lineage
            .clone();
        *state.lane_incarnation_activation_heights.write() = pending
            .catalog_update
            .updated_lane_incarnation_activation_heights
            .clone();
        state.install_lane_manifests(&pending.updated_lane_manifests);
        let status = {
            let view = state.view();
            iroha_data_model::nexus::LaneLifecycleStatusV1::new(
                &view.nexus.lane_catalog,
                &view.lane_incarnations,
                view.runtime_catalog_hash().unwrap(),
            )
            .unwrap()
        };
        assert_eq!(
            status.runtime_catalog_hash,
            Some(runtime.canonical_hash().unwrap())
        );
        let mut payload = catalog_payload(&state, &keys);
        payload.expected_catalog_hash = status.catalog_hash;
        payload.expected_incarnation_root = status.incarnation_root;
        payload.expected_runtime_catalog_hash = status.runtime_catalog_hash;
        let manifest_hash = [0x68; 32];
        let dataspace_id = DataSpaceId::from_hash(&manifest_hash);
        payload.dataspace_additions[0].descriptor.id = dataspace_id;
        payload.dataspace_additions[0].descriptor.alias = "second-catalog-ds".into();
        payload.dataspace_additions[0].manifest_hash = manifest_hash;
        payload.lane_additions[0].id = LaneId::new(6);
        payload.lane_additions[0].alias = "second-catalog-lane".into();
        payload.lane_additions[0].dataspace_id = dataspace_id;
        payload.manifest_additions = vec![catalog_manifest(
            LaneId::new(6),
            "second-catalog-lane",
            &keys,
        )];
        for stale in [None, Some(Hash::new(b"stale runtime overlay"))] {
            let mut invalid = payload.clone();
            invalid.expected_runtime_catalog_hash = stale;
            let mut block = state.block(catalog_test_header());
            let mut transaction = block.transaction();
            let error = transaction
                .stage_consensus_catalog_transition(&invalid)
                .expect_err("stale overlay guard must reject a second transition");
            assert!(error.to_string().contains("expected runtime catalog root"));
        }
        let mut block = state.block(catalog_test_header());
        let mut transaction = block.transaction();
        transaction
            .stage_consensus_catalog_transition(&payload)
            .expect("native readback must bind the next additive transition");
        let next = runtime_catalog_from_world(&transaction.world)
            .unwrap()
            .unwrap();
        assert_eq!(next.dataspaces.len(), 2);
        assert_eq!(next.manifests.len(), 2);
        assert_ne!(
            Some(next.canonical_hash().unwrap()),
            status.runtime_catalog_hash
        );
        assert_eq!(
            state.view().runtime_catalog_hash().unwrap(),
            status.runtime_catalog_hash
        );
    });
}

#[test]
fn runtime_catalog_final_overlay_rejects_unstaged_changed_and_removed_parameter() {
    run_catalog_test(|| {
        let (state, keys) = catalog_fixture(InvalidMember::None);
        let payload = catalog_payload(&state, &keys);
        let mut block = state.block(catalog_test_header());
        let mut transaction = block.transaction();
        transaction
            .stage_consensus_catalog_transition(&payload)
            .unwrap();
        let pending = transaction.pending_lane_lifecycle.clone().unwrap();
        state
            .validate_runtime_catalog_block_overlay(&transaction.world, Some(&pending), 2)
            .expect("exact accepted catalog and live committee");
        assert!(
            state
                .validate_runtime_catalog_block_overlay(&transaction.world, None, 2)
                .is_err()
        );
        let mut changed = pending.runtime_catalog.clone().unwrap();
        changed.dataspaces[0].descriptor.description = Some("late catalog change".to_owned());
        transaction.world.parameters.get_mut().set_parameter(
            iroha_data_model::parameter::Parameter::Custom(
                changed.into_custom_parameter().unwrap(),
            ),
        );
        assert!(
            state
                .validate_runtime_catalog_block_overlay(&transaction.world, Some(&pending), 2)
                .is_err()
        );
        transaction
            .world
            .parameters
            .get_mut()
            .custom
            .remove(&NexusRuntimeCatalogV1::parameter_id());
        assert!(
            state
                .validate_runtime_catalog_block_overlay(&transaction.world, Some(&pending), 2)
                .is_err()
        );
    });
}

#[test]
fn runtime_catalog_applied_transaction_publishes_manifest_to_next_transaction() {
    run_catalog_test(|| {
        let (state, keys) = catalog_fixture(InvalidMember::None);
        let payload = catalog_payload(&state, &keys);
        let mut block = state.block(catalog_test_header());
        let mut transaction = block.transaction();
        transaction
            .stage_consensus_catalog_transition(&payload)
            .unwrap();
        let expected_manifest_digest = transaction.lane_manifests.consensus_policy_digest();
        assert!(
            transaction
                .lane_privacy_registry
                .lane(LaneId::new(5))
                .is_some()
        );
        transaction.apply();
        assert_eq!(
            block.lane_manifests.consensus_policy_digest(),
            expected_manifest_digest
        );
        assert!(block.lane_manifests.has_manifest(LaneId::new(5)));
        assert!(block.lane_privacy_registry.lane(LaneId::new(5)).is_some());
        let next = block.transaction();
        assert_eq!(
            next.lane_manifests.consensus_policy_digest(),
            expected_manifest_digest
        );
        validate_runtime_catalog_committee(
            &next.world,
            &next.network_id,
            &next.nexus,
            &next.lane_manifests,
            &payload.lane_additions[0],
            3,
        )
        .expect("later transaction sees the accepted manifest and live committee");
        let privacy = next.lane_privacy_registry.lane(LaneId::new(5)).unwrap();
        assert_eq!(
            privacy.dataspace_id(),
            payload.lane_additions[0].dataspace_id
        );
        assert_eq!(privacy.commitments().count(), 1);
        assert!(runtime_catalog_from_world(&next.world).unwrap().is_some());
        drop(next);
        drop(block);
        assert!(
            !state.lane_manifests.read().has_manifest(LaneId::new(5)),
            "dropping the parent block does not publish process runtime state"
        );
        assert!(
            state
                .lane_privacy_registry
                .read()
                .lane(LaneId::new(5))
                .is_none()
        );
        assert!(
            runtime_catalog_from_world(&state.world.view())
                .unwrap()
                .is_none()
        );
    });
}

#[test]
fn runtime_catalog_final_overlay_rechecks_late_validator_invalidation() {
    run_catalog_test(|| {
        for case in 0..5 {
            let (state, keys) = catalog_fixture(InvalidMember::None);
            let payload = catalog_payload(&state, &keys);
            let mut block = state.block(catalog_test_header());
            let mut transaction = block.transaction();
            transaction
                .stage_consensus_catalog_transition(&payload)
                .unwrap();
            let pending = transaction.pending_lane_lifecycle.clone().unwrap();
            state
                .validate_runtime_catalog_block_overlay(&transaction.world, Some(&pending), 2)
                .unwrap();
            let id = derive_validator_key_id(keys[0].public_key());
            match case {
                0 => {
                    transaction
                        .world
                        .accounts
                        .remove(AccountId::new(keys[0].public_key().clone()));
                }
                1 => {
                    transaction.world.consensus_keys.remove(id);
                }
                _ => {
                    let mut record = transaction.world.consensus_keys.get(&id).unwrap().clone();
                    match case {
                        2 => record.status = ConsensusKeyStatus::Disabled,
                        3 => record.pop.as_mut().unwrap()[0] ^= 1,
                        _ => record.expiry_height = Some(3),
                    }
                    transaction.world.consensus_keys.insert(id, record);
                }
            }
            assert!(
                state
                    .validate_runtime_catalog_block_overlay(&transaction.world, Some(&pending), 2)
                    .is_err(),
                "late authority invalidation case {case}"
            );
        }
    });
}

#[test]
fn runtime_catalog_final_overlay_rejects_removal_and_unchanged_malformed_state() {
    run_catalog_test(|| {
        let (state, keys) = catalog_fixture(InvalidMember::None);
        let (_, pending) = staged_catalog_fixture(&state, &keys);
        install_fixture_runtime(&state, pending.runtime_catalog.unwrap());
        let mut block = state.world.block();
        state
            .validate_runtime_catalog_block_overlay(&block, None, 3)
            .unwrap();
        block
            .parameters
            .get_mut()
            .custom
            .remove(&NexusRuntimeCatalogV1::parameter_id());
        assert!(
            state
                .validate_runtime_catalog_block_overlay(&block, None, 3)
                .is_err()
        );
        drop(block);
        let mut world = state.world.block();
        world
            .parameters
            .get_mut()
            .set_parameter(iroha_data_model::parameter::Parameter::Custom(
                iroha_data_model::parameter::CustomParameter::new(
                    NexusRuntimeCatalogV1::parameter_id(),
                    iroha_primitives::json::Json::new(norito::json!({"version": 255})),
                ),
            ));
        world.commit();
        let accepted = state.world.block();
        assert!(
            state
                .validate_runtime_catalog_block_overlay(&accepted, None, 3)
                .is_err()
        );
    });
}

#[test]
fn runtime_catalog_startup_reconstructs_manifest_without_files_and_preserves_policy() {
    run_catalog_test(|| {
        let (mut state, keys) = catalog_fixture(InvalidMember::None);
        let configured = state.nexus_snapshot();
        let baseline_registry = state.lane_manifests.read().clone();
        let execution_before = state.execution_policy_digest_v1().unwrap();
        let amx_before =
            crate::sumeragi::v2_recovery::committed_nexus_amx_context_hash(&state).unwrap();
        let (after, pending) = staged_catalog_fixture(&state, &keys);
        let runtime = pending.runtime_catalog.clone().unwrap();
        install_fixture_runtime(&state, runtime.clone());
        // Model the geometry already restored by an authenticated snapshot. Startup receives the
        // original configured baseline plus that restored lane catalog, never new local DS rows.
        *state.nexus.write() = after.clone();
        *state.lane_incarnations.write() = pending.catalog_update.updated_lane_incarnations.clone();
        *state.lane_incarnation_lineage.write() = pending
            .catalog_update
            .updated_lane_incarnation_lineage
            .clone();
        *state.lane_incarnation_activation_heights.write() = pending
            .catalog_update
            .updated_lane_incarnation_activation_heights
            .clone();
        state.nexus_runtime_restored_from_snapshot = true;
        let mut startup = configured.clone();
        startup.lane_catalog = after.lane_catalog.clone();
        startup.lane_config = after.lane_config.clone();
        let restored = state.nexus_with_committed_catalog(startup.clone()).unwrap();
        assert_eq!(restored.dataspace_catalog, after.dataspace_catalog);
        assert_eq!(
            restored.configured_dataspace_catalog,
            configured.configured_dataspace_catalog
        );
        let manifests = state
            .lane_manifests_with_committed_catalog(&baseline_registry, &restored)
            .unwrap();
        assert!(manifests.has_manifest(LaneId::new(5)));
        assert!(
            manifests
                .status(LaneId::new(5))
                .unwrap()
                .manifest_path
                .is_none()
        );
        assert_eq!(
            manifests.consensus_policy_digest(),
            pending.updated_lane_manifests.consensus_policy_digest()
        );
        state.install_lane_manifests(&manifests);
        assert_eq!(
            state.execution_policy_digest_v1().unwrap(),
            execution_before
        );
        assert_ne!(
            crate::sumeragi::v2_recovery::committed_nexus_amx_context_hash(&state).unwrap(),
            amx_before
        );
        assert_eq!(
            runtime_catalog_root_from_world(&state.world.view()).unwrap(),
            Some(runtime.canonical_hash().unwrap())
        );
        let restored_again = state.nexus_with_committed_catalog(startup).unwrap();
        let manifests_again = state
            .lane_manifests_with_committed_catalog(&baseline_registry, &restored_again)
            .unwrap();
        assert_eq!(restored_again.dataspace_catalog, restored.dataspace_catalog);
        assert_eq!(
            manifests_again.consensus_policy_digest(),
            manifests.consensus_policy_digest()
        );
        let retired = after
            .lane_catalog
            .apply_lifecycle(&iroha_data_model::nexus::LaneLifecyclePlan {
                additions: Vec::new(),
                retire: vec![LaneId::new(5)],
            })
            .unwrap();
        assert!(
            ensure_runtime_catalog_lanes_preserved(
                &state.world.view(),
                &after.lane_catalog,
                &retired
            )
            .is_err()
        );
        let mut drift = restored.clone();
        drift.configured_dataspace_catalog = restored.dataspace_catalog.clone();
        assert!(state.nexus_with_committed_catalog(drift).is_err());
        let mut foreign = runtime;
        foreign.baseline_manifests_hash = Hash::new(b"foreign startup manifest baseline");
        install_fixture_runtime(&state, foreign);
        assert!(
            state
                .lane_manifests_with_committed_catalog(&baseline_registry, &restored)
                .is_err()
        );
        let mut world = state.world.block();
        world
            .parameters
            .get_mut()
            .set_parameter(iroha_data_model::parameter::Parameter::Custom(
                iroha_data_model::parameter::CustomParameter::new(
                    NexusRuntimeCatalogV1::parameter_id(),
                    iroha_primitives::json::Json::new(norito::json!({"version": 255})),
                ),
            ));
        world.commit();
        assert!(state.nexus_with_committed_catalog(configured).is_err());
        assert!(crate::sumeragi::v2_recovery::committed_nexus_amx_context_hash(&state).is_err());
    });
}
