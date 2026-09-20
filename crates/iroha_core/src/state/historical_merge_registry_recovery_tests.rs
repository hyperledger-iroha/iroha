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
    let (state, entry, carrier) =
        autonomous_runtime_effect_fixture(AutonomousRuntimeEffectFixture::Catalog);
    let batch = entry.execution_batch.as_ref().expect("catalog execution");
    let reservation = decode_canonical_merge_reservation_key(&batch.lanes[0].reservation_keys[0])
        .expect("exact native reservation");
    commit_staged_autonomous_for_test(production_validated_autonomous_merge_commit_block(
        &state, &entry, &carrier,
    ))
    .expect("commit the catalog before starting cold recovery");
    assert_eq!(
        state
            .kura
            .merge_ledger_all_entries()
            .expect("durable history"),
        vec![entry.clone()],
    );
    assert_eq!(
        state.committed_height(),
        usize::try_from(carrier.header().height().get()).expect("carrier height fits"),
    );

    // Use the fallible production constructor, which authenticates the durable
    // carrier/finality chain before minting Historical execution authority.
    // No World, registry, transactions, or block history are copied from the
    // writer. Rebuilding these is the subsequent block-replay phase's job.
    let cold = State::try_new_with_chain_and_network_id_with_default_telemetry(
        World::default(),
        Arc::clone(&state.kura),
        LiveQueryStore::start_test(),
        state.chain_id.clone(),
        *state.network_id_ref(),
    )
    .expect("certified durable history must validate before World replay");
    assert_eq!(cold.committed_height(), 0);
    assert_eq!(
        State::queue_plan_admission_registry_match_in_view(
            &cold.view(),
            reservation.entrypoint_hash,
            reservation.queue_plan_admission_binding_hash,
        )
        .expect("empty World contains no orphan admission evidence"),
        QueuePlanAdmissionRegistryMatch::Absent,
    );
    assert!(cold.merge_ledger().snapshot().is_empty());
    assert_eq!(cold.merge_admission.read().expected_epoch(), 1);
    assert!(cold.view().runtime_catalog_hash().unwrap().is_none());
    assert!(
        !cold
            .merge_execution_already_applied(&entry, batch)
            .expect("fresh World marker lookup"),
        "validating durable history must not publish its future execution effects",
    );
    let before = crate::snapshot::canonical_state_snapshot_hash(&cold);
    cold.recover_merge_ledger_from_kura()
        .expect("repeat authenticated cold recovery remains read-only");
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&cold),
        before
    );
    assert!(cold.merge_ledger().snapshot().is_empty());
    assert_eq!(
        cold.kura
            .merge_ledger_all_entries()
            .expect("retained history"),
        vec![entry],
    );
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
    let absent = crate::snapshot::canonical_state_snapshot_hash(&state);
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
        crate::snapshot::canonical_state_snapshot_hash(&state),
        absent
    );

    let conflict = crate::torii_proxy::QueuePlanAdmissionBindingV1::new(
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
    let conflicting = crate::snapshot::canonical_state_snapshot_hash(&state);
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
        crate::snapshot::canonical_state_snapshot_hash(&state),
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
    let before = crate::snapshot::canonical_state_snapshot_hash(&state);
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
            crate::snapshot::canonical_state_snapshot_hash(&state),
            before
        );
    }
}

state_test!(consensus_stack historical_autonomous_merge_rejects_restored_registry_conflict
    historical_autonomous_merge_rejects_restored_registry_conflict_on_consensus_stack();
);
fn historical_autonomous_merge_rejects_restored_registry_conflict_on_consensus_stack() {
    let (state, entry, carrier) =
        autonomous_runtime_effect_fixture(AutonomousRuntimeEffectFixture::Catalog);
    let batch = entry.execution_batch.as_ref().expect("catalog execution");
    let reservation = decode_canonical_merge_reservation_key(&batch.lanes[0].reservation_keys[0])
        .expect("exact native reservation");
    let key = State::queue_plan_admission_registry_marker_key(
        &crate::torii_proxy::QueuePlanAdmissionRegistryKeyV1 {
            version: crate::torii_proxy::QUEUE_PLAN_ADMISSION_BINDING_VERSION_V1,
            network_id_digest: crate::torii_proxy::queue_plan_admission_network_id_digest(
                state.network_id_ref(),
            ),
            entrypoint_hash: reservation.entrypoint_hash,
        },
    )
    .expect("exact historical registry key");
    commit_staged_autonomous_for_test(production_validated_autonomous_merge_commit_block(
        &state, &entry, &carrier,
    ))
    .expect("commit catalog effects and canonical transaction membership");
    let snapshot = norito::json::to_value(&state).expect("native State snapshot");
    for expected in [
        QueuePlanAdmissionRegistryMatch::Absent,
        QueuePlanAdmissionRegistryMatch::Conflict,
    ] {
        let restored =
            deserialize_state_snapshot_value_with_kura(snapshot.clone(), Arc::clone(&state.kura))
                .expect("exact snapshot restores through authenticated durable history");
        assert_eq!(
            restored.committed_height(),
            usize::try_from(carrier.header().height().get()).unwrap(),
        );
        assert_eq!(
            restored.merge_ledger().snapshot().as_slice(),
            &[Arc::new(entry.clone())],
        );
        assert_eq!(
            State::queue_plan_admission_registry_match_in_view(
                &restored.view(),
                reservation.entrypoint_hash,
                reservation.queue_plan_admission_binding_hash,
            )
            .expect("restored exact applied owner"),
            QueuePlanAdmissionRegistryMatch::Exact,
        );
        {
            let mut world = restored.world.block();
            if expected == QueuePlanAdmissionRegistryMatch::Absent {
                world.smart_contract_state.remove(key.clone());
            } else {
                world.smart_contract_state.insert(
                    key.clone(),
                    State::queue_plan_admission_registry_marker_payload(
                        &crate::torii_proxy::QueuePlanAdmissionRegistryValueV1 {
                            version: crate::torii_proxy::QUEUE_PLAN_ADMISSION_BINDING_VERSION_V1,
                            binding_hash: Hash::new(b"conflicting restored registry owner"),
                        },
                    )
                    .expect("canonical conflicting registry value"),
                );
            }
            world.commit();
        }
        assert_eq!(
            State::queue_plan_admission_registry_match_in_view(
                &restored.view(),
                reservation.entrypoint_hash,
                reservation.queue_plan_admission_binding_hash,
            )
            .expect("exactly classified restored owner evidence"),
            expected,
        );
        let before = crate::snapshot::canonical_state_snapshot_hash(&restored);
        let cached = restored.merge_ledger().snapshot();
        let expected_epoch = restored.merge_admission.read().expected_epoch();
        assert_merge_binding_error(
            restored.recover_merge_ledger_from_kura().expect_err(
                "history at the restored height cannot defer its World ownership check",
            ),
            MERGE_REGISTRY_BINDING_ERROR,
        );
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(&restored),
            before
        );
        assert_eq!(restored.merge_ledger().snapshot(), cached);
        assert_eq!(
            restored.merge_admission.read().expected_epoch(),
            expected_epoch
        );
    }
}
