// Cold historical recovery and live QueuePlan reservation authority regressions.
/// Exact rejection when a reservation disagrees with its entrypoint, route, or session.
const MERGE_RESERVATION_BINDING_ERROR: &str =
    "embedded reservation key does not match its entrypoint, route, or lane session";

/// Exact rejection when the current World must prove admission ownership.
const MERGE_REGISTRY_BINDING_ERROR: &str =
    "embedded reservation key lacks its exact QueuePlan admission registry binding";

fn assert_merge_binding_error(error: MergeLedgerCommitError, expected: &str) {
    match error {
        MergeLedgerCommitError::ExecutionBatchInvalid(reason) => {
            assert_eq!(reason, expected);
        }
        other => panic!("unexpected reservation rejection: {other:?}"),
    }
}

state_test!(consensus_stack historical_autonomous_merge_recovers_certified_carrier_before_world_replay
    historical_autonomous_merge_recovers_certified_carrier_before_world_replay_on_consensus_stack();
);
fn historical_autonomous_merge_recovers_certified_carrier_before_world_replay_on_consensus_stack() {
    let (fixture, carrier, context) =
        autonomous_native_runtime_effect_fixture(AutonomousRuntimeEffectFixture::Catalog);
    let state = &fixture.native.state;
    let batch = carrier
        .execution_context()
        .unwrap()
        .native_lane_decisions
        .as_deref()
        .unwrap();
    let input = &batch.groups[0].payload.input;
    let (staged, committed) = prepared_native_publication_for_test(state, &carrier, context);
    staged
        .commit()
        .expect("commit actual native catalog before cold recovery");
    promote_native_execution_finality_for_test(state, &committed);
    assert_native_application_recorded_for_test(state, &carrier);
    assert!(
        state.kura.merge_ledger_all_entries().unwrap().is_empty(),
        "native Decisions do not manufacture a retired MergeQC sidecar"
    );
    assert_eq!(
        state.committed_height(),
        carrier.header().height().get() as usize
    );

    let cold = State::try_new_with_chain_and_network_id_with_default_telemetry(
        World::default(),
        Arc::clone(&state.kura),
        LiveQueryStore::start_test(),
        state.chain_id.clone(),
        *state.network_id_ref(),
    )
    .expect("durable global finality authenticates before World replay");
    assert_eq!(cold.committed_height(), 0);
    assert_eq!(
        State::queue_plan_admission_registry_match_in_view(
            &cold.view(),
            input.entrypoint.hash(),
            input.certificate.binding.canonical_hash(),
        )
        .unwrap(),
        QueuePlanAdmissionRegistryMatch::Absent
    );
    assert!(cold.merge_ledger().snapshot().is_empty());
    assert_eq!(cold.merge_admission.read().expected_epoch(), 1);
    assert!(cold.view().runtime_catalog_hash().unwrap().is_none());
    let before = crate::snapshot::canonical_state_snapshot_hash(&cold).unwrap();
    for _ in 0..2 {
        let crate::kura::NativeLaneBatchCarrierReadV1::Ready(included) = cold
            .read_finalized_native_lane_batch(
                NonZeroUsize::new(carrier.header().height().get() as usize).unwrap(),
                carrier.hash(),
            )
            .expect(
                "cold source recovery retains authentic inclusion without future World authority",
            )
        else {
            panic!("exact locally retained native carrier");
        };
        assert_eq!(
            included.batch(),
            batch,
            "native source Decisions are recovered without translation"
        );
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(&cold).unwrap(),
            before,
            "recovering future finalized inputs does not apply their World or registry effects"
        );
        assert!(cold.view().runtime_catalog_hash().unwrap().is_none());
        assert!(cold.merge_ledger().snapshot().is_empty());
    }
}

state_test!(consensus_stack live_autonomous_merge_requires_exact_pending_queue_plan_owner
    live_autonomous_merge_requires_exact_pending_queue_plan_owner_on_consensus_stack();
);
fn live_autonomous_merge_requires_exact_pending_queue_plan_owner_on_consensus_stack() {
    let (state, entry, _, _) = autonomous_merge_commit_authorization_fixture(false, false);
    let batch = entry
        .execution_batch
        .as_ref()
        .expect("autonomous execution");
    let lane = &batch.lanes[0];
    let route =
        decode_canonical_merge_routing_plan(&lane.routing_plans[0]).expect("exact native route");
    let binding = State::pending_queue_plan_binding_for_execution(
        &state.view(),
        &lane.entrypoints[0],
        &route,
        batch.application_block_header.height().get(),
    )
    .expect("native pending owner lookup")
    .expect("fixture has an exact pending owner");
    state
        .validate_merge_execution_batch(
            &entry.active_lanes,
            batch,
            MergeExecutionValidationAuthority::Live(&ConsensusMode::Permissioned),
        )
        .expect("exact pending owner admits live execution");
    {
        let mut world = state.world.block();
        assert!(
            State::resolve_queue_plan_pending_obligation_in_storage(
                &mut world.smart_contract_state,
                binding.network_id_digest,
                binding.entrypoint_hash,
            )
            .expect("remove the exact obligation and all of its route/alias members"),
        );
        world.smart_contract_state.remove(
            State::queue_plan_admission_registry_marker_key(&binding.registry_key())
                .expect("exact registry marker"),
        );
        world.commit();
    }
    assert_eq!(
        state
            .queue_plan_admission_binding_registry_match(&binding)
            .unwrap(),
        QueuePlanAdmissionRegistryMatch::Absent,
    );
    let absent = crate::snapshot::canonical_state_snapshot_hash(&state)
        .expect("stable valid fixture snapshot");
    assert_merge_binding_error(
        state
            .validate_merge_execution_batch(
                &entry.active_lanes,
                batch,
                MergeExecutionValidationAuthority::Live(&ConsensusMode::Permissioned),
            )
            .expect_err("a durable certificate cannot substitute for live pending ownership"),
        MERGE_REGISTRY_BINDING_ERROR,
    );
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state)
            .expect("stable valid fixture snapshot"),
        absent
    );

    let conflict = crate::torii_proxy::new_queue_plan_admission_binding(
        state.network_id_ref(),
        &lane.entrypoints[0],
        &route,
        binding.admission_context.clone(),
        binding
            .enqueue_timestamp_ms
            .checked_add(1)
            .expect("timestamp fits"),
    )
    .expect("coherent alternate owner for the same entrypoint");
    assert_ne!(conflict.canonical_hash(), binding.canonical_hash());
    seed_pending_queue_plan_binding_state_for_test(&state, &conflict);
    assert_eq!(
        state
            .queue_plan_admission_binding_registry_match(&binding)
            .unwrap(),
        QueuePlanAdmissionRegistryMatch::Conflict,
    );
    let conflicting = crate::snapshot::canonical_state_snapshot_hash(&state)
        .expect("stable valid fixture snapshot");
    assert_merge_binding_error(
        state
            .validate_merge_execution_batch(
                &entry.active_lanes,
                batch,
                MergeExecutionValidationAuthority::Live(&ConsensusMode::Permissioned),
            )
            .expect_err("a coherent different pending owner must reject live execution"),
        MERGE_REGISTRY_BINDING_ERROR,
    );
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state)
            .expect("stable valid fixture snapshot"),
        conflicting
    );
}

state_test!(consensus_stack autonomous_merge_rejects_reforged_reservation_bindings
    autonomous_merge_rejects_reforged_reservation_bindings_on_consensus_stack();
);
fn autonomous_merge_rejects_reforged_reservation_bindings_on_consensus_stack() {
    let (state, entry, _, _) = autonomous_merge_commit_authorization_fixture(false, false);
    let batch = entry
        .execution_batch
        .as_ref()
        .expect("autonomous execution");
    let reservation = decode_canonical_merge_reservation_key(&batch.lanes[0].reservation_keys[0])
        .expect("exact native reservation");
    let before = crate::snapshot::canonical_state_snapshot_hash(&state)
        .expect("stable valid fixture snapshot");
    for field in ["admission", "route", "incarnation", "view"] {
        let mut forged = reservation.clone();
        match field {
            "admission" => forged.queue_plan_admission_binding_hash = Hash::new(b"forged owner"),
            "route" => forged.routing_plan_digest = Hash::new(b"forged route"),
            "incarnation" => forged.lane_incarnation = Hash::new(b"forged incarnation"),
            "view" => forged.lane_block_view = forged.lane_block_view.checked_add(1).unwrap(),
            _ => unreachable!(),
        }
        forged
            .validate()
            .expect("forgery retains valid intrinsic key shape");
        let mut changed = batch.clone();
        changed.lanes[0].reservation_keys[0] =
            norito::encode_canonical(&forged).expect("canonical reservation encoding");
        changed.execution_root = crate::merge::merge_execution_root(&changed.lanes);
        changed.batch_hash = crate::merge::merge_execution_batch_hash(&changed);
        assert!(crate::merge::merge_execution_batch_commitments_match(
            &changed
        ));
        assert_merge_binding_error(
            state
                .validate_merge_execution_batch(
                    &entry.active_lanes,
                    &changed,
                    MergeExecutionValidationAuthority::Live(&ConsensusMode::Permissioned),
                )
                .expect_err("recomputed untrusted roots must not authorize a forged reservation"),
            if field == "admission" {
                MERGE_REGISTRY_BINDING_ERROR
            } else {
                MERGE_RESERVATION_BINDING_ERROR
            },
        );
        let mut changed_entry = entry.clone();
        changed_entry.execution_batch = Some(changed);
        assert!(
            matches!(
                state.validate_merge_quorum_certificate(&changed_entry, false, false),
                Err(MergeLedgerCommitError::MergeQCDigestMismatch { .. })
            ),
            "forged {field} must not acquire the recovery-only Historical authority"
        );
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(&state)
                .expect("stable valid fixture snapshot"),
            before
        );
    }
}

state_test!(consensus_stack historical_autonomous_merge_rejects_restored_registry_conflict
    historical_autonomous_merge_rejects_restored_registry_conflict_on_consensus_stack();
);
fn historical_autonomous_merge_rejects_restored_registry_conflict_on_consensus_stack() {
    let (fixture, carrier, context) =
        autonomous_native_runtime_effect_fixture(AutonomousRuntimeEffectFixture::Catalog);
    let state = &fixture.native.state;
    let batch = carrier
        .execution_context()
        .unwrap()
        .native_lane_decisions
        .as_deref()
        .unwrap();
    let input = &batch.groups[0].payload.input;
    let binding = &input.certificate.binding;
    let key = State::queue_plan_admission_registry_marker_key(&binding.registry_key()).unwrap();
    let (staged, committed) = prepared_native_publication_for_test(state, &carrier, context);
    staged
        .commit()
        .expect("publish native catalog and exact carrier membership");
    promote_native_execution_finality_for_test(state, &committed);
    let snapshot = norito::json::to_value(state).expect("native State snapshot");
    let height = NonZeroUsize::new(carrier.header().height().get() as usize).unwrap();
    let restore = |lane_manifests: LaneManifestRegistryHandle| {
        deserialize::KuraSeed {
            kura: Arc::clone(&state.kura),
            lane_manifests,
            query_handle: LiveQueryStore::start_test(),
            #[cfg(feature = "telemetry")]
            telemetry: crate::telemetry::StateTelemetry::default(),
        }
        .into_state_from_json(snapshot.clone())
    };
    let error = restore(Arc::new(LaneManifestRegistry::empty()))
        .err()
        .expect("restoration rejects an unrelated configured manifest baseline");
    assert!(
        error
            .to_string()
            .contains("manifest baseline differs from canonical World catalog")
    );
    let baseline = state.lane_manifests.read().clone();
    let expected_state_hash = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
    for field in ["merge_global_state_root", "merge_hint_roots"] {
        let mut changed = snapshot.clone();
        let world = changed.get_mut("world").unwrap().as_object_mut().unwrap();
        let cell = world.get_mut(field).unwrap().as_object_mut().unwrap();
        let foreign = Hash::new(b"unbound restored reduction metadata");
        let value = if field == "merge_global_state_root" {
            norito::json::to_value(&Some(foreign)).unwrap()
        } else {
            norito::json::to_value(&vec![foreign]).unwrap()
        };
        cell.insert("blocks".to_owned(), value);
        let error = deserialize::KuraSeed {
            kura: Arc::clone(&state.kura),
            lane_manifests: Arc::clone(&baseline),
            query_handle: LiveQueryStore::start_test(),
            #[cfg(feature = "telemetry")]
            telemetry: crate::telemetry::StateTelemetry::default(),
        }
        .into_state_from_json(changed)
        .err()
        .expect("empty durable merge history cannot authenticate fabricated reduction metadata");
        assert!(
            error
                .to_string()
                .contains("empty merge history has noncanonical reduction metadata"),
            "wrong {field} recovery rejection: {error}"
        );
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(state).unwrap(),
            expected_state_hash,
            "malformed restore cannot mutate the original canonical State"
        );
    }
    for expected in [
        QueuePlanAdmissionRegistryMatch::Absent,
        QueuePlanAdmissionRegistryMatch::Conflict,
    ] {
        let restored = restore(Arc::clone(&baseline))
            .expect("restore exact applied native World with durable source history");
        assert_eq!(restored.committed_height(), height.get());
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(&restored).unwrap(),
            expected_state_hash,
            "configured baseline restores the exact canonical applied State before registry tampering: {}",
            canonical_native_restore_difference_for_test(state, &restored)
        );
        assert!(restored.merge_ledger().snapshot().is_empty());
        assert_eq!(
            State::queue_plan_admission_registry_match_in_view(
                &restored.view(),
                input.entrypoint.hash(),
                binding.canonical_hash(),
            )
            .unwrap(),
            QueuePlanAdmissionRegistryMatch::Exact
        );
        assert!(
            matches!(
                restored
                    .read_finalized_native_lane_batch(height, carrier.hash())
                    .unwrap(),
                crate::kura::NativeLaneBatchCarrierReadV1::Ready(_)
            ),
            "untampered applied native source passes recovery"
        );
        {
            let mut world = restored.world.block();
            if expected == QueuePlanAdmissionRegistryMatch::Absent {
                world.smart_contract_state.remove(key.clone());
            } else {
                let priority = State::decode_exact_queue_plan_admission_registry_record(
                    &key,
                    world.smart_contract_state.get(&key).unwrap(),
                )
                .unwrap()
                .priority;
                world.smart_contract_state.insert(
                    key.clone(),
                    State::queue_plan_admission_registry_marker_payload(
                        &crate::torii_proxy::QueuePlanAdmissionRegistryValueV1 {
                            version: crate::torii_proxy::QUEUE_PLAN_ADMISSION_BINDING_VERSION_V1,
                            binding_hash: Hash::new(b"conflicting restored registry owner"),
                        },
                        priority,
                    )
                    .unwrap(),
                );
            }
            world.commit();
        }
        assert_eq!(
            State::queue_plan_admission_registry_match_in_view(
                &restored.view(),
                input.entrypoint.hash(),
                binding.canonical_hash(),
            )
            .unwrap(),
            expected
        );
        let before = crate::snapshot::canonical_state_snapshot_hash(&restored).unwrap();
        let cached = restored.merge_ledger().snapshot();
        let expected_epoch = restored.merge_admission.read().expected_epoch();
        let error = restored
            .read_finalized_native_lane_batch(height, carrier.hash())
            .expect_err(
                "applied native history must prove its exact retained World admission owner",
            );
        assert_merge_binding_error(
            error,
            "applied native source lacks its exact retained admission registry binding",
        );
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(&restored).unwrap(),
            before
        );
        assert_eq!(restored.merge_ledger().snapshot(), cached);
        assert_eq!(
            restored.merge_admission.read().expected_epoch(),
            expected_epoch
        );
    }
}
