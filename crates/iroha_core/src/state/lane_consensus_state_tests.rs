// Native state ownership, full-set witness and snapshot regressions.

fn configured_lane_context_queue_plan_state() -> (State, Vec<KeyPair>, Vec<KeyPair>, SignedBlock) {
    let network_id =
        iroha_data_model::NetworkId::from_genesis_hash(empty_global_block_after(None).hash());
    let (state, validators, commit_keys, parent) =
        configured_single_lane_merge_state_with_network(network_id);
    let parent = advance_queue_plan_fixture_to_beacon_parent(&state, parent);
    (state, validators, commit_keys, parent)
}

fn lane_opening_context_for_state_test(
    state: &State,
) -> iroha_data_model::block::consensus_v2::HeightContext {
    let mut parent = None;
    for height in 1..=state.committed_height() {
        let block = state
            .kura
            .get_block(NonZeroUsize::new(height).unwrap())
            .unwrap();
        parent = Some(merge_carrier_finality_artifact_with_network(
            &block,
            parent.as_ref(),
            state.network_id,
        ));
    }
    let parent = parent.unwrap();
    crate::sumeragi::v2_context::build_successor_height_context(
        &parent,
        parent.height_context.nexus_amx_context_hash,
        None,
    )
    .unwrap()
}

state_test! { sync lane_consensus_full_set_witness_rejects_mutation_removal_and_duplicates
    let state = blank_test_state();
    let block = empty_global_block_after(None);
    let mut overlay = state.block(block.header());
    let mut witness = ExecWitness::default();
    overlay.capture_lane_consensus_contexts(&mut witness).unwrap();
    overlay.verify_lane_consensus_contexts_witness(&witness).unwrap();
    assert_eq!(witness.writes.len(), 1, "the empty set is explicitly authenticated");
    let exact = witness.clone();
    witness.writes.clear();
    assert!(overlay.verify_lane_consensus_contexts_witness(&witness).is_err());
    witness = exact.clone();
    witness.writes.push(witness.writes[0].clone());
    assert!(overlay.verify_lane_consensus_contexts_witness(&witness).is_err());
    witness = exact;
    witness.writes[0].value.push(0);
    assert!(overlay.verify_lane_consensus_contexts_witness(&witness).is_err());

    let context = lane_consensus_context::frozen_lane_context_fixture_for_test();
    *overlay.lane_consensus_contexts.get_mut() = LaneConsensusContextsV1::new(vec![context]).unwrap();
    assert!(overlay.verify_lane_consensus_contexts_seal().is_err());
    assert!(overlay.capture_lane_consensus_contexts(&mut ExecWitness::default()).is_err());
    assert!(matches!(
        overlay.commit(),
        Err(TransactionsBlockError::LaneConsensusContexts)
    ));
    assert!(state.view().lane_consensus_contexts.contexts.is_empty(),
        "failed publication must leave committed consensus metadata unchanged");
}

state_test! { sync lane_consensus_metadata_is_atomic_and_outside_merge_economic_write_set
    let (state, validators, _, parent) = configured_lane_context_queue_plan_state();
    let (_, certificate) = queue_plan_admission_certificate_for_state_test(
        &state, crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
            LaneId::SINGLE, DataSpaceId::UNIVERSAL,
        )), &validators, parent.header().height().get(), 0x36,
    );
    seed_exact_queue_plan_admission_state_for_test(&state, &certificate);
    let block = empty_global_block_after(Some(&parent));
    let opening = lane_opening_context_for_state_test(&state);
    let expected = {
        let mut overlay = state.block(block.header());
        let economic_root = overlay.merge_execution_write_set_root();
        overlay.finalize_lane_consensus_contexts(&block, Some(&opening)).unwrap();
        assert_eq!(overlay.merge_execution_write_set_root(), economic_root);
        overlay.lane_consensus_contexts.get().clone()
        // Dropping a candidate rolls its separate consensus metadata back too.
    };
    assert!(!expected.contexts.is_empty());
    assert!(state.view().lane_consensus_contexts.contexts.is_empty());
    let mut overlay = state.block(block.header());
    overlay.finalize_lane_consensus_contexts(&block, Some(&opening)).unwrap();
    overlay.capture_lane_consensus_contexts(&mut ExecWitness::default()).unwrap();
    overlay.block_hashes.push(block.hash());
    insert_empty_transaction_block_for_state_commit(&mut overlay, &block);
    overlay.commit().unwrap();
    assert_eq!(state.view().lane_consensus_contexts, expected);
    let replacement = state.block_and_revert(block.header());
    assert!(replacement.lane_consensus_contexts.get().contexts.is_empty(),
        "replacement block must read the exact pre-carrier consensus snapshot");
}

state_test! { sync lane_consensus_snapshot_requires_field_and_hashes_the_complete_set
    let state = State::new_with_chain_and_network_id_for_testing(
        World::default(), Kura::blank_kura_for_testing(), LiveQueryStore::start_test(),
        (*DEFAULT_TEST_CHAIN_ID).clone(),
        iroha_data_model::NetworkId::from_genesis_hash(empty_global_block_after(None).hash()),
    );
    let empty_root = crate::snapshot::canonical_state_snapshot_hash(&state).expect("stable valid fixture snapshot");
    let empty = norito::json::to_value(&state).unwrap();
    assert!(empty.get("lane_consensus_contexts").is_some());
    let restored = deserialize_state_snapshot_value(empty.clone()).unwrap();
    assert!(restored.view().lane_consensus_contexts.contexts.is_empty());
    let mut omitted = empty;
    omitted.as_object_mut().unwrap().remove("lane_consensus_contexts");
    assert!(deserialize_state_snapshot_value(omitted).is_err(),
        "first-release restore must not infer an empty set from obsolete snapshots");

    let mut context = lane_consensus_context::frozen_lane_context_fixture_for_test();
    context.network_id = state.network_id;
    let mut cell = state.lane_consensus_contexts.block();
    *cell.get_mut() = LaneConsensusContextsV1::new(vec![context]).unwrap();
    cell.commit();
    assert_ne!(crate::snapshot::canonical_state_snapshot_hash(&state).expect("stable valid fixture snapshot"), empty_root,
        "frozen contexts must not be redacted as local consensus sidecars");
    assert!(deserialize_state_snapshot_value(norito::json::to_value(&state).unwrap()).is_err(),
        "a context opening after the snapshot height cannot be restored");
}

state_test! { sync lane_consensus_opens_from_admitted_work_and_survives_global_advancement
    let (state, validators, _, parent) = configured_lane_context_queue_plan_state();
    let (binding, certificate) = queue_plan_admission_certificate_for_state_test(
        &state,
        crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
            LaneId::SINGLE, DataSpaceId::UNIVERSAL,
        )),
        &validators, parent.header().height().get(), 0x35,
    );
    seed_exact_queue_plan_admission_state_for_test(&state, &certificate);
    let block = empty_global_block_after(Some(&parent));
    let opening = lane_opening_context_for_state_test(&state);
    let mut overlay = state.block(block.header());
    assert!(overlay.finalize_lane_consensus_contexts(&block, None).is_err(),
        "pending work alone cannot invent the global opening authority");
    assert!(overlay.lane_consensus_contexts.get().contexts.is_empty());
    overlay.finalize_lane_consensus_contexts(&block, Some(&opening)).unwrap();
    let original = overlay.lane_consensus_contexts.get().clone();
    assert_eq!(original.contexts.len(), 1);
    assert_eq!(original.contexts[0].committee.len(), 4);
    assert_eq!(original.contexts[0].opening_global_height, block.header().height().get());
    assert_eq!(original.contexts[0].next_lane_height, 1);
    overlay.capture_lane_consensus_contexts(&mut ExecWitness::default()).unwrap();
    overlay.block_hashes.push(block.hash());
    insert_empty_transaction_block_for_state_commit(&mut overlay, &block);
    overlay.commit().unwrap();
    state.kura.store_block(Arc::new(block.clone())).unwrap();
    seed_autoscale_sample_history_for_snapshot_test(&state);
    let snapshot = norito::json::to_value(&state).unwrap();
    let restored = deserialize_state_snapshot_value_with_kura(snapshot, Arc::clone(&state.kura)).unwrap();
    assert_eq!(restored.view().lane_consensus_contexts, original);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(&restored).expect("stable valid fixture snapshot"),
        crate::snapshot::canonical_state_snapshot_hash(&state).expect("stable valid fixture snapshot"));

    let successor = empty_global_block_after(Some(&block));
    let mut later = lane_opening_context_for_state_test(&state);
    later.leader_seed = [0xF4; 32];
    later.execution_policy_hash = Hash::new(b"later global policy must not reset open lane");
    let mut overlay = state.block(successor.header());
    overlay.finalize_lane_consensus_contexts(&successor, Some(&later)).unwrap();
    assert_eq!(overlay.lane_consensus_contexts.get(), &original);
    // The next application opens a new instance under that carrier's authority,
    // even when an additional admitted obligation remains in the same FIFO.
    let descriptor = Hash::new(b"first canonical lane decision");
    let current = &original.contexts[0];
    overlay.stage_merge_lane_frontier_markers(vec![State::encode_merge_lane_frontier_marker(
        AppliedMergeLaneFrontierMarker {
            version: 1, lane_id: current.lane_id, dataspace_id: current.dataspace_id,
            lane_incarnation: current.lane_incarnation, lane_block_height: 1,
            lane_block_descriptor_hash: descriptor,
            applied_global_height: successor.header().height().get(),
        },
    ).unwrap()]).unwrap();
    overlay.finalize_lane_consensus_contexts(&successor, Some(&later)).unwrap();
    let next = &overlay.lane_consensus_contexts.get().contexts[0];
    assert_eq!(next.next_lane_height, 2);
    assert_eq!(next.predecessor_hash, Some(descriptor));
    assert_eq!(next.predecessor_applied_global_height, successor.header().height().get());
    assert_eq!(next.opening_global_context_id, later.id());
    assert_eq!(binding.admission_context.route_incarnations.len(), 1);
    assert!(State::resolve_queue_plan_pending_obligation_in_storage(
        &mut overlay.world.smart_contract_state, binding.network_id_digest,
        binding.entrypoint_hash,
    ).unwrap());
    overlay.finalize_lane_consensus_contexts(&successor, Some(&later)).unwrap();
    assert!(overlay.lane_consensus_contexts.get().contexts.is_empty());
    let mut witness = ExecWitness::default();
    overlay.capture_lane_consensus_contexts(&mut witness).unwrap();
    overlay.verify_lane_consensus_contexts_witness(&witness).unwrap();
    overlay.verify_lane_consensus_contexts_publication().unwrap();
}

state_test! { sync lane_consensus_terminal_head_resolution_reopens_without_retagging_the_old_instance
    let (state, validators, _, parent) = configured_lane_context_queue_plan_state();
    let route = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
        LaneId::SINGLE, DataSpaceId::UNIVERSAL,
    ));
    let (first, first_certificate) = queue_plan_admission_certificate_for_state_test(
        &state, route.clone(), &validators, parent.header().height().get(), 0x75,
    );
    let (second, second_certificate) = queue_plan_admission_certificate_for_state_test(
        &state, route, &validators, parent.header().height().get(), 0x76,
    );
    seed_exact_queue_plan_admission_state_for_test(&state, &first_certificate);
    seed_exact_queue_plan_admission_state_for_test(&state, &second_certificate);
    let carrier = empty_global_block_after(Some(&parent));
    let opening = lane_opening_context_for_state_test(&state);
    let mut overlay = state.block(carrier.header());
    overlay.finalize_lane_consensus_contexts(&carrier, Some(&opening)).unwrap();
    let original = overlay.lane_consensus_contexts.get().contexts[0].clone();
    assert_eq!(original.admitted_binding_hash, first.canonical_hash());
    overlay.capture_lane_consensus_contexts(&mut ExecWitness::default()).unwrap();
    overlay.block_hashes.push(carrier.hash());
    insert_empty_transaction_block_for_state_commit(&mut overlay, &carrier);
    overlay.commit().unwrap();
    state.kura.store_block(Arc::new(carrier.clone())).unwrap();

    let successor = empty_global_block_after(Some(&carrier));
    let later = lane_opening_context_for_state_test(&state);
    let mut overlay = state.block(successor.header());
    overlay.finalize_lane_consensus_contexts(&successor, Some(&later)).unwrap();
    assert_eq!(overlay.lane_consensus_contexts.get().contexts[0], original,
        "other pending work cannot reset or preempt the current head");
    assert!(State::resolve_queue_plan_pending_obligation_in_storage(
        &mut overlay.world.smart_contract_state, first.network_id_digest, first.entrypoint_hash,
    ).unwrap());
    assert!(overlay.finalize_lane_consensus_contexts(&successor, None).is_err(),
        "only the exact authenticated carrier may replace the resolved instance");
    assert_eq!(overlay.lane_consensus_contexts.get().contexts[0], original);
    overlay.finalize_lane_consensus_contexts(&successor, Some(&later)).unwrap();
    let replacement = &overlay.lane_consensus_contexts.get().contexts[0];
    assert_eq!(replacement.admitted_binding_hash, second.canonical_hash());
    assert_eq!(replacement.next_lane_height, original.next_lane_height);
    assert_eq!(replacement.predecessor_hash, original.predecessor_hash);
    assert_eq!(replacement.opening_global_height, successor.header().height().get());
    assert_ne!(replacement.canonical_hash().unwrap(), original.canonical_hash().unwrap(),
        "the new group must never reuse the old group's lock or signing identity");
    overlay.capture_lane_consensus_contexts(&mut ExecWitness::default()).unwrap();
    overlay.verify_lane_consensus_contexts_publication().unwrap();
    drop(overlay);
    assert_eq!(state.view().lane_consensus_contexts.contexts[0], original,
        "an uncommitted terminal candidate cannot cancel the committed instance");
}
