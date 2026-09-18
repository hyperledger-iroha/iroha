fn catalog_test_header() -> BlockHeader {
    BlockHeader::new(NonZeroU64::new(2).unwrap(), None, None, 0, 0)
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
        let (runtime, retained_runtime) = {
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
            (runtime, transaction.canonical_runtime.get().clone())
        };
        assert_eq!(state.view().runtime_catalog_hash().unwrap(), None);
        let first = runtime.canonical_hash().unwrap();
        install_fixture_runtime(&state, runtime.clone());
        // Explicit metadata publication fixture: retain the actual runtime
        // record staged by the accepted transaction alongside its World catalog.
        // This does not claim carrier execution, finality, or State publication.
        let mut owner = state.canonical_runtime.block();
        *owner.get_mut() = retained_runtime;
        owner.commit();
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
        let malformed_world = norito::json::to_json(&state.world).unwrap();
        let retained_owner = norito::json::to_json(&state.canonical_runtime).unwrap();
        assert!(runtime_catalog_root_from_world(&state.world.view()).is_err());
        assert!(matches!(
            state.try_view(),
            Err(LaneLifecycleError::RuntimeCatalog(_))
        ));
        assert_eq!(
            norito::json::to_json(&state.world).unwrap(),
            malformed_world
        );
        assert_eq!(
            norito::json::to_json(&state.canonical_runtime).unwrap(),
            retained_owner
        );
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
        state
            .install_canonical_runtime_projection(
                &state.nexus.read(),
                &pending.catalog_update.updated_lane_incarnation_lineage,
                &state.autoscale_sample_history_snapshot(),
            )
            .expect("install complete authenticated runtime fixture");
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
        validate_runtime_catalog_block_overlay(
            transaction.world.parameters.get_before_block(),
            &transaction.world,
            &state.network_id,
            &state.nexus_snapshot(),
            Some(&pending),
            2,
        )
        .expect("exact accepted catalog and live committee");
        assert!(
            validate_runtime_catalog_block_overlay(
                transaction.world.parameters.get_before_block(),
                &transaction.world,
                &state.network_id,
                &state.nexus_snapshot(),
                None,
                2
            )
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
            validate_runtime_catalog_block_overlay(
                transaction.world.parameters.get_before_block(),
                &transaction.world,
                &state.network_id,
                &state.nexus_snapshot(),
                Some(&pending),
                2
            )
            .is_err()
        );
        transaction
            .world
            .parameters
            .get_mut()
            .custom
            .remove(&NexusRuntimeCatalogV1::parameter_id());
        assert!(
            validate_runtime_catalog_block_overlay(
                transaction.world.parameters.get_before_block(),
                &transaction.world,
                &state.network_id,
                &state.nexus_snapshot(),
                Some(&pending),
                2
            )
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
            validate_runtime_catalog_block_overlay(
                transaction.world.parameters.get_before_block(),
                &transaction.world,
                &state.network_id,
                &state.nexus_snapshot(),
                Some(&pending),
                2,
            )
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
                validate_runtime_catalog_block_overlay(
                    transaction.world.parameters.get_before_block(),
                    &transaction.world,
                    &state.network_id,
                    &state.nexus_snapshot(),
                    Some(&pending),
                    2
                )
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
        validate_runtime_catalog_block_overlay(
            block.parameters.get_before_block(),
            &block,
            &state.network_id,
            &state.nexus_snapshot(),
            None,
            3,
        )
        .unwrap();
        block
            .parameters
            .get_mut()
            .custom
            .remove(&NexusRuntimeCatalogV1::parameter_id());
        assert!(
            validate_runtime_catalog_block_overlay(
                block.parameters.get_before_block(),
                &block,
                &state.network_id,
                &state.nexus_snapshot(),
                None,
                3
            )
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
            validate_runtime_catalog_block_overlay(
                accepted.parameters.get_before_block(),
                &accepted,
                &state.network_id,
                &state.nexus_snapshot(),
                None,
                3
            )
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
        state
            .install_canonical_runtime_projection(
                &state.nexus.read(),
                &pending.catalog_update.updated_lane_incarnation_lineage,
                &state.autoscale_sample_history_snapshot(),
            )
            .expect("install complete authenticated runtime fixture");
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
        let retained_amx =
            crate::sumeragi::v2_recovery::committed_nexus_amx_context_hash(&state).unwrap();
        assert_ne!(retained_amx, amx_before);
        assert_eq!(
            crate::sumeragi::v2_recovery::committed_nexus_amx_context_hash(&state).unwrap(),
            retained_amx,
            "repeated reads of the same retained inputs have the same policy hash"
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
        let malformed_world = norito::json::to_json(&state.world).unwrap();
        let retained_owner = norito::json::to_json(&state.canonical_runtime).unwrap();
        assert!(state.nexus_with_committed_catalog(configured).is_err());
        let error = crate::sumeragi::v2_recovery::committed_nexus_amx_context_hash(&state)
            .expect_err("malformed protected catalog is a typed recovery refusal");
        assert!(
            matches!(error, crate::sumeragi::v2_recovery::V2RecoveryError::ExecutionPolicy(ref reason)
            if reason.contains("Nexus runtime catalog codec")),
            "{error}"
        );
        assert_eq!(
            norito::json::to_json(&state.world).unwrap(),
            malformed_world
        );
        assert_eq!(
            norito::json::to_json(&state.canonical_runtime).unwrap(),
            retained_owner,
            "failed recovery cannot change current runtime or retained undo"
        );
    });
}

#[test]
fn runtime_catalog_replacement_uses_its_retained_parameter_predecessor() {
    run_catalog_test(|| {
        let (state, keys) = catalog_fixture(InvalidMember::None);
        let (_, pending) = staged_catalog_fixture(&state, &keys);
        install_fixture_runtime(&state, pending.runtime_catalog.unwrap());
        let live = runtime_catalog_from_world(&state.world.view()).unwrap();
        assert!(live.is_some());
        let mut replacement = state.world.block_and_revert();
        assert!(runtime_catalog_from_world(&replacement).unwrap().is_none());
        let nexus = state.nexus_snapshot();
        validate_runtime_catalog_block_overlay(
            replacement.parameters.get_before_block(),
            &replacement,
            &state.network_id,
            &nexus,
            None,
            2,
        )
        .expect("an untouched replacement retains its actual pre-catalog predecessor");
        replacement.parameters.get_mut().set_parameter(
            iroha_data_model::parameter::Parameter::Custom(
                live.clone().unwrap().into_custom_parameter().unwrap(),
            ),
        );
        assert!(
            validate_runtime_catalog_block_overlay(
                replacement.parameters.get_before_block(),
                &replacement,
                &state.network_id,
                &nexus,
                None,
                2,
            )
            .is_err(),
            "matching the discarded live tip does not authorize an unstaged catalog"
        );
        drop(replacement);
        assert_eq!(
            runtime_catalog_from_world(&state.world.view()).unwrap(),
            live
        );
    });
}

#[test]
fn runtime_catalog_owned_overlay_ignores_later_policy_cache_mutation() {
    run_catalog_test(|| {
        let (state, keys) = catalog_fixture(InvalidMember::None);
        let payload = catalog_payload(&state, &keys);
        let original = state.nexus_snapshot();
        let mut block = state.block(catalog_test_header());
        let mut transaction = block.transaction();
        transaction
            .stage_consensus_catalog_transition(&payload)
            .unwrap();
        transaction.apply();
        block.validate_owned_runtime_catalog_overlay().unwrap();
        state.nexus.write().staking.public_validator_mode =
            iroha_config::parameters::actual::LaneValidatorMode::StakeElected;
        block
            .validate_owned_runtime_catalog_overlay()
            .expect("the captured AdminManaged policy owns this accepted overlay");
        let key_id = derive_validator_key_id(keys[0].public_key());
        block.world.consensus_keys.remove(key_id);
        assert!(
            block.validate_owned_runtime_catalog_overlay().is_err(),
            "captured policy still requires every accepted live key"
        );
        drop(block);
        *state.nexus.write() = original;
        assert!(
            runtime_catalog_from_world(&state.world.view())
                .unwrap()
                .is_none()
        );
    });
}

#[test]
fn runtime_catalog_merge_validation_owns_its_captured_policy() {
    run_catalog_test(|| {
        let (state, keys) = catalog_fixture(InvalidMember::None);
        let payload = catalog_payload(&state, &keys);
        let original_nexus = state.nexus_snapshot();
        let original_manifests = Arc::clone(&state.lane_manifests.read());
        let mut block = state.block(catalog_test_header());
        let mut transaction = block.transaction();
        transaction
            .stage_consensus_catalog_transition(&payload)
            .unwrap();
        transaction.apply();
        block.validate_merge_runtime_catalog_effects().unwrap();

        // These physical caches are no longer this carrier's predecessor.
        state.nexus.write().fees.base_fee = Quantity::from(99_u32);
        *state.lane_manifests.write() = Arc::new(LaneManifestRegistry::empty());
        block
            .validate_merge_runtime_catalog_effects()
            .expect("captured policy and original journals own the accepted transition");
        block.nexus.fees.base_fee = Quantity::from(99_u32);
        assert!(
            block.validate_merge_runtime_catalog_effects().is_err(),
            "matching a later physical cache cannot authorize policy substitution"
        );
        block.nexus.fees.base_fee = original_nexus.fees.base_fee.clone();
        block.validate_merge_runtime_catalog_effects().unwrap();
        block.zk.halo2.enabled = !block.zk.halo2.enabled;
        assert!(
            block.validate_merge_runtime_catalog_effects().is_err(),
            "the captured ZK policy still binds the accepted overlay"
        );
        drop(block);
        *state.nexus.write() = original_nexus;
        *state.lane_manifests.write() = original_manifests;
        assert!(
            runtime_catalog_from_world(&state.world.view())
                .unwrap()
                .is_none()
        );
    });
}

#[test]
fn runtime_catalog_merge_replacement_uses_actual_world_and_runtime_undo() {
    run_catalog_test(|| {
        let (state, keys) = catalog_fixture(InvalidMember::None);
        let payload = catalog_payload(&state, &keys);
        let (runtime, owner) = {
            let mut block = state.block(catalog_test_header());
            let mut transaction = block.transaction();
            transaction
                .stage_consensus_catalog_transition(&payload)
                .unwrap();
            transaction.apply();
            block.validate_merge_runtime_catalog_effects().unwrap();
            (
                runtime_catalog_from_world(&block.world).unwrap().unwrap(),
                block.canonical_runtime.get().clone(),
            )
        };
        // Publish only the paired metadata fixture. No execution/finality or
        // complete State publication is claimed by this replacement test.
        install_fixture_runtime(&state, runtime.clone());
        let mut published_runtime = state.canonical_runtime.block();
        *published_runtime.get_mut() = owner;
        published_runtime.commit();
        let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
        let mut replacement = state.block_and_revert(catalog_test_header());
        assert!(
            runtime_catalog_from_world(&replacement.world)
                .unwrap()
                .is_none()
        );
        assert!(!replacement.lane_manifests.has_manifest(LaneId::new(5)));
        replacement
            .validate_merge_runtime_catalog_effects()
            .expect("the discarded live catalog is not the replacement's predecessor");
        let mut transaction = replacement.transaction();
        transaction
            .stage_consensus_catalog_transition(&payload)
            .unwrap();
        transaction.apply();
        replacement
            .validate_merge_runtime_catalog_effects()
            .expect("the same addition is valid against the actual pre-catalog undo");
        replacement
            .pending_autoscale_lifecycle
            .as_mut()
            .unwrap()
            .catalog_update
            .previous_lane_incarnations
            .clear();
        assert!(
            replacement
                .validate_merge_runtime_catalog_effects()
                .is_err(),
            "the captured policy does not excuse a forged dynamic predecessor"
        );
        drop(replacement);
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
            before
        );
        assert_eq!(
            runtime_catalog_from_world(&state.world.view()).unwrap(),
            Some(runtime)
        );
    });
}
