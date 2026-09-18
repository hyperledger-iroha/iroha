// Canonical first-admission position, immutable opening cuts and adverse readers.

fn ordered_priority_admissions_for_test(
    state: &State,
    validators: &[KeyPair],
    authority_height: u64,
) -> Vec<(crate::torii_proxy::QueuePlanAdmissionBindingV1, Vec<u8>)> {
    let plan = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
    ));
    let mut admissions = [0x81, 0x82]
        .into_iter()
        .map(|seed| {
            queue_plan_admission_certificate_for_state_test(
                state,
                plan.clone(),
                validators,
                authority_height,
                seed,
            )
        })
        .collect::<Vec<_>>();
    admissions.sort_by_key(|(binding, _)| binding.registry_key());
    admissions
}

fn publish_priority_admission_carrier_for_test(
    state: &State,
    carrier: &SignedBlock,
    certificates: &[Vec<u8>],
) {
    let opening = lane_opening_context_for_state_test(state);
    let mut overlay = state
        .block_with_queue_plan_admissions(carrier.header().clone(), certificates)
        .expect("native admission writer");
    overlay
        .finalize_lane_consensus_contexts(carrier, Some(&opening))
        .unwrap();
    overlay
        .capture_lane_consensus_contexts(&mut ExecWitness::default())
        .unwrap();
    overlay.block_hashes.push(carrier.hash());
    insert_empty_transaction_block_for_state_commit(&mut overlay, carrier);
    overlay
        .commit()
        .expect("publish native admission metadata and frozen context");
    state.kura.store_block(Arc::new(carrier.clone())).unwrap();
}

state_test! { sync queue_plan_priority_record_is_canonical_bounded_and_rejects_rankless_claims
    let claim = crate::torii_proxy::QueuePlanAdmissionRegistryValueV1 {
        version: crate::torii_proxy::QUEUE_PLAN_ADMISSION_BINDING_VERSION_V1,
        binding_hash: Hash::new(b"ranked admission fixture"),
    };
    let key: StatePath = "queue_plan_priority_codec_fixture".parse().unwrap();
    let priority = QueuePlanAdmissionPriorityV1::new(9, 3).unwrap();
    let encoded = State::queue_plan_admission_registry_marker_payload(&claim, priority).unwrap();
    let decoded = State::decode_exact_queue_plan_admission_registry_record(&key, &encoded).unwrap();
    assert_eq!(decoded.claim, claim);
    assert_eq!(decoded.priority, priority);
    assert_eq!(State::decode_exact_queue_plan_admission_registry_marker(&key, &encoded).unwrap(), claim);
    assert!(QueuePlanAdmissionPriorityV1::new(0, 0).is_err());
    assert!(QueuePlanAdmissionPriorityV1::new(1, MAX_QUEUE_PLAN_ADMISSIONS_PER_BLOCK).is_err());
    let last = QueuePlanAdmissionPriorityV1::new(1, MAX_QUEUE_PLAN_ADMISSIONS_PER_BLOCK - 1).unwrap();
    assert!(last < QueuePlanAdmissionPriorityV1::new(2, 0).unwrap());
    assert!(State::decode_exact_queue_plan_admission_registry_marker(
        &key, &norito::encode_canonical(&claim).unwrap(),
    ).is_err(), "the pre-admission signed claim is no longer a State registry record");
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert!(State::decode_exact_queue_plan_admission_registry_record(&key, &trailing).is_err());
    for (version, carrier_height, admission_index) in [
        (2, 9, 3), (1, 0, 3), (1, 9, u32::MAX),
    ] {
        let mut malformed = decoded.clone();
        malformed.version = version;
        malformed.priority.carrier_height = carrier_height;
        malformed.priority.admission_index = admission_index;
        assert!(State::decode_exact_queue_plan_admission_registry_record(
            &key, &norito::encode_canonical(&malformed).unwrap(),
        ).is_err());
    }
}

state_test! { sync queue_plan_priority_uses_actual_delayed_carrier_and_canonical_control_index
    let (state, validators, _, parent) = configured_lane_context_queue_plan_state();
    let admissions = ordered_priority_admissions_for_test(&state, &validators, parent.header().height().get());
    let advance = empty_global_block_after(Some(&parent));
    commit_block_metadata_to_state(&state, &advance);
    state.kura.store_block(Arc::new(advance.clone())).unwrap();
    let carrier = empty_global_block_after(Some(&advance));
    let height = carrier.header().height().get();
    let certificates = admissions.iter().map(|(_, bytes)| bytes.clone()).collect::<Vec<_>>();
    let overlay = state.block_with_queue_plan_admissions(carrier.header().clone(), &certificates).unwrap();
    assert_eq!(u64::try_from(overlay.height()).unwrap(), height - 1,
        "the exact completed carrier may still have a predecessor-only hash journal");
    let ranked = State::queue_plan_pending_route_at_admission_cut(
        &overlay, LaneId::SINGLE, DataSpaceId::UNIVERSAL,
        overlay.lane_incarnations[&LaneId::SINGLE], height, height,
    ).unwrap();
    assert_eq!(ranked.len(), 2);
    for (index, (ranked, (binding, _))) in ranked.iter().zip(&admissions).enumerate() {
        assert_eq!(ranked.binding, *binding);
        assert_eq!(ranked.priority, QueuePlanAdmissionPriorityV1::new(height, index).unwrap());
        assert!(binding.admission_context.proposal_height < ranked.priority.carrier_height,
            "source authority/proposal height is not admission priority");
    }
    assert!(State::queue_plan_pending_route_at_admission_cut(
        &overlay, LaneId::SINGLE, DataSpaceId::UNIVERSAL,
        overlay.lane_incarnations[&LaneId::SINGLE], height + 1, height,
    ).is_err());
}

state_test! { sync queue_plan_priority_replay_preserves_first_rank_and_opening_cut
    let (state, validators, _, parent) = configured_lane_context_queue_plan_state();
    let admissions = ordered_priority_admissions_for_test(&state, &validators, parent.header().height().get());
    let first = empty_global_block_after(Some(&parent));
    // The greater registry key arrives first, alone. A later carrier places a
    // new lesser key before its replay; source/key order must not rewrite age.
    publish_priority_admission_carrier_for_test(&state, &first, &[admissions[1].1.clone()]);
    let original_key = State::queue_plan_admission_registry_marker_key(&admissions[1].0.registry_key()).unwrap();
    let original = state.world.smart_contract_state.view().get(&original_key).unwrap().clone();
    let carrier = empty_global_block_after(Some(&first));
    let height = carrier.header().height().get();
    let certificates = admissions.iter().map(|(_, bytes)| bytes.clone()).collect::<Vec<_>>();
    let mut overlay = state.block_with_queue_plan_admissions(carrier.header().clone(), &certificates).unwrap();
    assert_eq!(overlay.world.smart_contract_state.get(&original_key), Some(&original));
    let incarnation = overlay.lane_incarnations[&LaneId::SINGLE];
    let ranked = State::queue_plan_pending_route_at_admission_cut(
        &overlay, LaneId::SINGLE, DataSpaceId::UNIVERSAL, incarnation, height, height,
    ).unwrap();
    assert_eq!(ranked.iter().map(|entry| entry.binding.entrypoint_hash).collect::<Vec<_>>(),
        vec![admissions[1].0.entrypoint_hash, admissions[0].0.entrypoint_hash]);
    assert_eq!(ranked[0].priority, QueuePlanAdmissionPriorityV1::new(first.header().height().get(), 0).unwrap());
    assert_eq!(ranked[1].priority, QueuePlanAdmissionPriorityV1::new(height, 0).unwrap());
    let old_cut = State::queue_plan_pending_route_at_admission_cut(
        &overlay, LaneId::SINGLE, DataSpaceId::UNIVERSAL, incarnation,
        first.header().height().get(), height,
    ).unwrap();
    assert_eq!(old_cut, vec![ranked[0].clone()], "new work cannot enter an older frozen opening");
    assert_eq!(State::queue_plan_pending_route_head_at_admission_cut(
        &overlay, LaneId::SINGLE, DataSpaceId::UNIVERSAL, incarnation, height, height,
    ).unwrap(), Some(ranked[0].clone()));
    let before = overlay.merge_execution_write_set_root();
    let active = State::queue_plan_active_lane_bindings_from_snapshot(
        &overlay.nexus, &overlay.lane_incarnations, &overlay.lane_incarnation_activation_heights,
    ).unwrap();
    overlay.stage_queue_plan_admissions(&certificates, &active, height).unwrap();
    assert_eq!(overlay.merge_execution_write_set_root(), before, "exact replay performs no State write");
    assert!(overlay.stage_queue_plan_admissions(&certificates, &active, height + 1).is_err());
    assert_eq!(overlay.merge_execution_write_set_root(), before, "a caller cannot invent the admission carrier");
}

state_test! { sync queue_plan_priority_rejects_changed_same_carrier_position_without_writes
    let (state, validators, _, parent) = configured_lane_context_queue_plan_state();
    let admissions = ordered_priority_admissions_for_test(&state, &validators, parent.header().height().get());
    let carrier = empty_global_block_after(Some(&parent));
    let height = carrier.header().height().get();
    let certificates = admissions.iter().map(|(_, bytes)| bytes.clone()).collect::<Vec<_>>();
    let mut overlay = state.block_with_queue_plan_admissions(carrier.header().clone(), &certificates).unwrap();
    let active = State::queue_plan_active_lane_bindings_from_snapshot(
        &overlay.nexus, &overlay.lane_incarnations, &overlay.lane_incarnation_activation_heights,
    ).unwrap();
    let before = overlay.merge_execution_write_set_root();
    let second_key = State::queue_plan_admission_registry_marker_key(&admissions[1].0.registry_key()).unwrap();
    let second_owner = overlay.world.smart_contract_state.get(&second_key).unwrap().clone();

    // The private consistency boundary accepts only the original position at H.
    // A subset would move the second first-admission owner from index 1 to 0.
    assert!(overlay.stage_queue_plan_admissions(&[certificates[1].clone()], &active, height).is_err());
    assert_eq!(overlay.world.smart_contract_state.get(&second_key), Some(&second_owner));
    assert_eq!(overlay.merge_execution_write_set_root(), before);
    overlay.stage_queue_plan_admissions(&certificates, &active, height).unwrap();
    assert_eq!(overlay.merge_execution_write_set_root(), before);

    // Production permits only one complete vector on a pristine overlay, so
    // separate partial calls cannot allocate the same (H, index) to disjoint keys.
    assert!(matches!(
        overlay.stage_queue_plan_admissions_for_carrier(&certificates),
        Err(MergeLedgerCommitError::ExecutionStageNotPristine)
    ));
    assert_eq!(overlay.staged_queue_plan_admissions(), certificates.as_slice());
    assert_eq!(overlay.merge_execution_write_set_root(), before);
}

state_test! { sync queue_plan_priority_rejects_invalid_owner_future_early_and_duplicate_positions
    let (state, validators, _, parent) = configured_lane_context_queue_plan_state();
    let admissions = ordered_priority_admissions_for_test(&state, &validators, parent.header().height().get());
    let carrier = empty_global_block_after(Some(&parent));
    let height = carrier.header().height().get();
    let certificates = admissions.iter().map(|(_, bytes)| bytes.clone()).collect::<Vec<_>>();
    let mut overlay = state.block_with_queue_plan_admissions(carrier.header().clone(), &certificates).unwrap();
    let key = State::queue_plan_admission_registry_marker_key(&admissions[1].0.registry_key()).unwrap();
    let original = overlay.world.smart_contract_state.get(&key).unwrap().clone();
    let exact = State::decode_exact_queue_plan_admission_registry_record(&key, &original).unwrap();
    let incarnation = overlay.lane_incarnations[&LaneId::SINGLE];
    let read = |overlay: &StateBlock<'_>| State::queue_plan_pending_route_at_admission_cut(
        overlay, LaneId::SINGLE, DataSpaceId::UNIVERSAL, incarnation, height, height,
    );
    assert_eq!(read(&overlay).unwrap().len(), 2);
    for invalid in [
        State::queue_plan_admission_registry_marker_payload(&exact.claim,
            QueuePlanAdmissionPriorityV1::new(height + 1, 1).unwrap()).unwrap(),
        State::queue_plan_admission_registry_marker_payload(&exact.claim,
            QueuePlanAdmissionPriorityV1::new(height - 1, 1).unwrap()).unwrap(),
        State::queue_plan_admission_registry_marker_payload(&exact.claim,
            QueuePlanAdmissionPriorityV1::new(height, 0).unwrap()).unwrap(),
        State::queue_plan_admission_registry_marker_payload(
            &crate::torii_proxy::QueuePlanAdmissionRegistryValueV1 {
                version: exact.claim.version, binding_hash: Hash::new(b"foreign pending owner"),
            }, exact.priority,
        ).unwrap(),
        vec![0xff],
    ] {
        overlay.world.smart_contract_state.insert(key.clone(), invalid);
        assert!(read(&overlay).is_err());
        overlay.world.smart_contract_state.insert(key.clone(), original.clone());
    }
    overlay.world.smart_contract_state.remove(key.clone());
    assert!(read(&overlay).is_err(), "an exact member without its ranked registry owner grants no priority");
    overlay.world.smart_contract_state.insert(key, original);
    assert_eq!(read(&overlay).unwrap().len(), 2);
}

state_test! { sync queue_plan_priority_rejects_reordered_controls_before_any_write
    let (state, validators, _, parent) = configured_lane_context_queue_plan_state();
    let admissions = ordered_priority_admissions_for_test(&state, &validators, parent.header().height().get());
    let carrier = empty_global_block_after(Some(&parent));
    let mut overlay = state.block(carrier.header());
    let before = overlay.merge_execution_write_set_root();
    assert!(overlay.stage_queue_plan_admissions_for_carrier(
        &[admissions[1].1.clone(), admissions[0].1.clone()],
    ).is_err());
    assert_eq!(overlay.merge_execution_write_set_root(), before);
    assert!(overlay.staged_queue_plan_admissions().is_empty());
    let cut = State::queue_plan_pending_route_head_at_admission_cut(
        &overlay, LaneId::SINGLE, DataSpaceId::UNIVERSAL,
        overlay.lane_incarnations[&LaneId::SINGLE], carrier.header().height().get(), carrier.header().height().get(),
    ).unwrap();
    assert!(cut.is_none());
}
