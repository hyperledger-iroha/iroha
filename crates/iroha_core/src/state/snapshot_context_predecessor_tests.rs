// Complete State restoration must retain lane opening/closure undo with World.

state_test! { sync snapshot_lane_contexts_preserve_opening_and_closure_predecessors
    let fixture = native_economic_fixture(&[NativeEconomicCase::Transfer(25)], false);
    let state = &fixture.native.state;
    let opened = state.lane_consensus_contexts.view().get().clone();
    assert_eq!(opened.contexts.len(), 1);
    assert!(state.lane_consensus_contexts.predecessor_view().get().as_ref().unwrap().contexts.is_empty());
    for close in [false, true] {
        if close {
            // Exercise metadata replacement with actual authenticated pending work.
            // This is a snapshot fixture, not execution/finality qualification.
            let carrier = empty_global_block_after(Some(&fixture.native.block));
            let mut overlay = state.block(carrier.header());
            assert!(State::resolve_queue_plan_pending_obligation_in_storage(
                &mut overlay.world.smart_contract_state,
                fixture.native.binding.network_id_digest,
                fixture.native.binding.entrypoint_hash,
            ).unwrap());
            overlay.finalize_lane_consensus_contexts(&carrier, None).unwrap();
            overlay.capture_lane_consensus_contexts(&mut ExecWitness::default()).unwrap();
            overlay.stage_autoscale_sample_record_for_count(&carrier, 0).unwrap();
            overlay.block_hashes.push(carrier.hash());
            insert_empty_transaction_block_for_state_commit(&mut overlay, &carrier);
            overlay.commit().unwrap();
            state.kura.store_block(Arc::new(carrier)).unwrap();
        }
        let current = state.lane_consensus_contexts.view().get().clone();
        let previous = state.lane_consensus_contexts.predecessor_view().get().clone().unwrap();
        assert_eq!(current.contexts.is_empty(), close);
        assert_eq!(previous.contexts.is_empty(), !close);
        let snapshot = norito::json::to_value(state).unwrap();
        let restored = deserialize_state_snapshot_value_with_kura(snapshot.clone(), Arc::clone(&state.kura))
            .expect("restore both actual membership cuts");
        assert_eq!(restored.lane_consensus_contexts.view().get(), &current);
        assert_eq!(restored.lane_consensus_contexts.predecessor_view().get(), &Some(previous.clone()));
        assert_eq!(crate::snapshot::canonical_state_snapshot_hash(&restored).expect("stable valid fixture snapshot"),
            crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot"));
        let height = NonZeroUsize::new(restored.committed_height()).unwrap();
        let carrier = restored.kura.get_block(height).unwrap();
        {
            let replacement = restored.block_and_revert(carrier.header());
            assert_eq!(replacement.lane_consensus_contexts.get(), &previous);
            super::lane_consensus_state::validate_committed_lane_consensus_contexts(
                &previous, &replacement.world, &replacement.nexus, &replacement.lane_incarnations,
                restored.network_id, carrier.header().height().get() - 1,
            ).expect("replacement's retained contexts bind its actual reverted World");
        }
        assert_eq!(restored.lane_consensus_contexts.view().get(), &current);
        assert_eq!(restored.lane_consensus_contexts.predecessor_view().get(), &Some(previous));
        for bad in [norito::json::Value::Null, norito::json::to_value(&current).unwrap()] {
            let mut changed = snapshot.clone();
            changed.as_object_mut().unwrap().get_mut("lane_consensus_contexts").unwrap()
                .as_object_mut().unwrap().insert("revert".to_owned(), bad);
            let error = deserialize_state_snapshot_value_with_kura(changed, Arc::clone(&state.kura))
                .err().expect("current contexts cannot substitute for the predecessor");
            assert!(error.to_string().contains("lane_consensus_contexts.revert"), "{error}");
        }
        let mut obsolete = snapshot;
        obsolete.as_object_mut().unwrap().insert("lane_consensus_contexts".to_owned(), norito::json::to_value(&current).unwrap());
        assert!(deserialize_state_snapshot_value_with_kura(obsolete, Arc::clone(&state.kura)).is_err());
    }
}

state_test! { sync snapshot_lane_contexts_height_zero_rejects_undo_even_for_empty_set
    let state = blank_test_state();
    let snapshot = norito::json::to_value(&state).unwrap();
    deserialize_state_snapshot_value_with_kura(snapshot.clone(), Arc::clone(&state.kura)).unwrap();
    let mut invalid = snapshot;
    invalid.as_object_mut().unwrap().get_mut("lane_consensus_contexts").unwrap()
        .as_object_mut().unwrap().insert("revert".to_owned(), norito::json::to_value(state.lane_consensus_contexts.view().get()).unwrap());
    let error = deserialize_state_snapshot_value_with_kura(invalid, Arc::clone(&state.kura))
        .err().expect("height zero has no context predecessor");
    assert!(error.to_string().contains("height-zero contexts"), "{error}");
}
