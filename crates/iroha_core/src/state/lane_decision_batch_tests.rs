// Authentic native Decisions are input-only. Actual-header execution is private,
// disposable, and grants no publication or native Apply acknowledgement.

state_test! { sync native_economic_batch_seals_actual_transfer_and_all_instance_markers_without_publication
    use iroha_data_model::block::lane_decision_batch::LaneDecisionBatchV1;
    use iroha_model_base::state_path::StatePath;
    let fixture = native_economic_fixture(&[NativeEconomicCase::Transfer(25)], true);
    let state = &fixture.native.state;
    let groups = native_economic_groups(&fixture);
    let before = crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot");
    let batch = state.prepare_lane_decision_batch(&groups).unwrap();
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot"), before);
    assert_eq!(batch.base_state_hash, HashOf::from_untyped_unchecked(before));
    assert_eq!(batch.groups, vec![groups[0].to_wire()]);
    let mut carrier = empty_global_block_after(Some(&fixture.native.block));
    carrier.set_execution_context(Some(BlockExecutionContextBundle::default().with_native_lane_decisions(batch.clone())));
    let prepared = state.replay_lane_decision_batch(&carrier.header(), &batch, &groups).unwrap();
    assert!(prepared.executions()[0].result.is_ok());
    assert_eq!(prepared.executions()[0].source, batch.groups[0]);
    let roots = prepared.prefix_roots_for_test();
    assert_ne!(roots.0, roots.1);
    let actual_results = prepared.executions().iter().map(|result| result.result.clone()).collect::<Vec<_>>();
    let actual_transcripts = prepared.executions()[0].fastpq_transcripts.clone();
    let overlay = prepared.overlay();
    assert_eq!(overlay.world.assets.get(&fixture.source).unwrap().0, Quantity::from(75u32));
    assert_eq!(overlay.world.assets.get(&fixture.destination).unwrap().0, Quantity::from(25u32));
    assert_native_economic_terminal(overlay, &groups[0], carrier.header().height().get());
    let identity = super::lane_decision_batch::native_application_identity(&carrier.header(), batch.canonical_hash().unwrap());
    let mut paths = vec![format!("native_lane_application_{}", hex::encode(identity.as_ref()))];
    paths.extend(batch.groups[0].payload.descriptor.slots.iter().map(|slot| {
        format!("native_lane_applied_instance_{}", hex::encode(slot.instance_id.as_ref()))
    }));
    for path in paths {
        let path: StatePath = path.parse().unwrap();
        let value = overlay.world.smart_contract_state.get(&path).expect("actual replay marker");
        assert_eq!(norito::decode_canonical::<Hash>(value).unwrap(), identity);
    }
    let bytes = norito::encode_canonical(&batch).unwrap();
    assert_eq!(LaneDecisionBatchV1::decode_canonical(&bytes, bytes.len()).unwrap(), batch);
    drop(prepared);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot"), before);
    let replay = state.replay_lane_decision_batch(&carrier.header(), &batch, &groups).unwrap();
    assert_eq!(replay.batch(), &batch);
    assert_eq!(replay.prefix_roots_for_test(), roots);
    assert_eq!(replay.executions().iter().map(|result| result.result.clone()).collect::<Vec<_>>(), actual_results);
    assert_eq!(replay.executions()[0].fastpq_transcripts, actual_transcripts);
    assert_eq!(replay.overlay().world.assets.get(&fixture.source).unwrap().0, Quantity::from(75u32));
    drop(replay);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot"), before);
}

state_test! { sync native_economic_batch_replay_rejects_tampered_source_and_actual_carrier_context
    let fixture = native_economic_fixture(&[NativeEconomicCase::Transfer(25)], true);
    let state = &fixture.native.state;
    let groups = native_economic_groups(&fixture);
    let before = crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot");
    let carrier = empty_global_block_after(Some(&fixture.native.block)).header();
    let batch = state.prepare_lane_decision_batch(&groups).unwrap();
    for mutation in 0..4 {
        let mut changed = batch.clone();
        match mutation {
            0 => changed.base_state_height += 1,
            1 => changed.base_state_hash = HashOf::from_untyped_unchecked(Hash::new(b"another base")),
            2 => changed.groups[0].payload.descriptor.admission_carrier_hash = HashOf::from_untyped_unchecked(Hash::new(b"another first carrier")),
            3 => changed.groups.push(changed.groups[0].clone()),
            _ => unreachable!(),
        }
        assert!(state.replay_lane_decision_batch(&carrier, &changed, &groups).is_err(), "mutation {mutation}");
        assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot"), before);
    }
    let mut different = carrier.clone();
    different.set_prev_block_hash(Some(HashOf::from_untyped_unchecked(Hash::new(b"foreign actual parent"))));
    assert!(state.replay_lane_decision_batch(&different, &batch, &groups).is_err());
    assert!(state.replay_lane_decision_batch(&carrier, &batch, &[]).is_err());
    // A different proposal time is a valid different execution context. The same
    // source can be reproposed; its marker identity binds the actual header.
    let later = BlockHeader::new(carrier.height(), carrier.prev_block_hash(), None,
        u64::try_from(carrier.creation_time().as_millis()).unwrap()+1, carrier.view_change_index());
    assert_ne!(super::lane_decision_batch::native_application_identity(&carrier,batch.canonical_hash().unwrap()),
        super::lane_decision_batch::native_application_identity(&later,batch.canonical_hash().unwrap()));
    drop(state.replay_lane_decision_batch(&later, &batch, &groups).unwrap());
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot"), before);
}

state_test! { sync native_economic_batch_existing_instance_marker_drops_every_economic_change
    use iroha_model_base::state_path::StatePath;
    let fixture = native_economic_fixture(&[NativeEconomicCase::Transfer(25)], true);
    let state = &fixture.native.state;
    let groups = native_economic_groups(&fixture);
    let slot = &groups[0].body().payload().descriptor.slots[1];
    let marker: StatePath = format!("native_lane_applied_instance_{}", hex::encode(slot.instance_id.as_ref())).parse().unwrap();
    let mut storage = state.world.smart_contract_state.block();
    storage.insert(marker, norito::encode_canonical(&Hash::new(b"existing application")).unwrap());
    storage.commit();
    let before = crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot");
    let carrier = empty_global_block_after(Some(&fixture.native.block)).header();
    let error = state.prepare_native_batch_on_carrier(carrier, &groups).err().expect("marker collision");
    assert!(matches!(error, MergeLedgerCommitError::ExecutionMarkerConflict(_)), "{error}");
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot"), before);
}

state_test! { sync native_observation_preserves_stable_bad_source_but_retries_changed_success_and_error
    use super::lane_decision_batch::with_stable_observation;
    let fixture = native_economic_fixture(&[NativeEconomicCase::Transfer(25)], true);
    let state = &fixture.native.state;
    let groups = native_economic_groups(&fixture);
    let batch = state.prepare_lane_decision_batch(&groups).unwrap();
    let carrier = empty_global_block_after(Some(&fixture.native.block)).header();
    let mut bad = batch.clone();
    bad.base_state_hash = HashOf::from_untyped_unchecked(Hash::new(b"stable foreign base"));
    assert!(matches!(state.replay_lane_decision_batch(&carrier, &bad, &groups),
        Err(MergeLedgerCommitError::ExecutionBatchInvalid(_))),
        "same-generation source mismatch remains an actual rejection");
    for fail in [false, true] {
        let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
        let result = with_stable_observation(state, || {
            let result = if fail {
                state.replay_lane_decision_batch(&carrier, &bad, &groups).map(|_| batch.clone())
            } else {
                state.prepare_lane_decision_batch(&groups)
            };
            assert_eq!(result.is_err(), fail);
            // Exercise the actual publication fence after a real source observation.
            // This is a no-op publication, not fabricated carrier execution.
            let publication = state.begin_state_view_write();
            drop(publication);
            result
        });
        assert!(matches!(result, Err(MergeLedgerCommitError::ExecutionObservationChanged)));
        assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).unwrap(), before);
    }
    let called = std::cell::Cell::new(false);
    let publication = state.begin_state_view_write();
    let result: core::result::Result<(), MergeLedgerCommitError> = with_stable_observation(state, || {
        called.set(true);
        Ok(())
    });
    assert!(matches!(result, Err(MergeLedgerCommitError::ExecutionObservationChanged)));
    assert!(!called.get(), "busy observations do not begin source work");
    drop(publication);
}

state_test! { sync native_economic_batch_preserves_merge_ledger_query_metadata
    let fixture = native_economic_fixture(&[NativeEconomicCase::Transfer(25)], false);
    let state = &fixture.native.state;
    // Isolate the native writer boundary with a nonempty, internally exact
    // previous relay reduction. The canonical native source binds this pre-State;
    // this scratch test grants no relay finality or State publication authority.
    let roots = vec![Hash::new(b"prior finalized relay one"), Hash::new(b"prior finalized relay two")];
    let global = crate::merge::reduce_merge_hint_roots(&roots);
    {
        let mut world = state.world.block();
        *world.merge_hint_roots = roots.clone();
        *world.merge_global_state_root = Some(global);
        world.commit();
    }
    let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
    let groups = native_economic_groups(&fixture);
    let carrier = empty_global_block_after(Some(&fixture.native.block));
    let prepared = state.prepare_native_batch_on_carrier(carrier.header(), &groups)
        .expect("actual native source executes from its exact current pre-State");
    assert!(prepared.executions()[0].result.is_ok());
    assert_eq!(prepared.overlay().world.assets.get(&fixture.source).unwrap().0, Quantity::from(75_u32));
    assert_eq!(prepared.overlay().world.merge_hint_roots.as_slice(), roots.as_slice(),
        "native Decisions do not replace the latest relay merge-ledger hints");
    assert_eq!(*prepared.overlay().world.merge_global_state_root, Some(global),
        "native Decisions do not replace the latest relay merge-ledger root");
    drop(prepared);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).unwrap(), before);
}

state_test!(consensus_stack merge_recovery_validates_latest_entry_without_rewriting_world
    merge_recovery_validates_latest_entry_without_rewriting_world_impl();
);
fn merge_recovery_validates_latest_entry_without_rewriting_world_impl() {
    let (state, _, _, keys) =
        setup_nexus_fee_merge_state(Quantity::from(10_u32), Quantity::from(3_u32), [0x46; 32]);
    let candidate = state
        .merge_entry_candidates_from_lane_relays()
        .into_iter()
        .next()
        .unwrap();
    let qc = merge_qc_for_candidate(&state, &candidate, &keys, &[0]);
    let entry = state
        .commit_merge_entry(merge_entry_from_candidate(candidate, qc))
        .expect("retain actual certified relay entry and its advertised query roots");
    assert!(entry.execution_batch.is_none());
    assert!(!entry.merge_hint_roots().is_empty());
    let exact_hints = norito::json::to_value(&state.world.merge_hint_roots).unwrap();
    let exact_global = norito::json::to_value(&state.world.merge_global_state_root).unwrap();
    state
        .validate_recovered_merge_metadata(Some(entry.as_ref()))
        .expect("exact latest applied relay metadata validates without mutation");
    assert_eq!(
        norito::json::to_value(&state.world.merge_hint_roots).unwrap(),
        exact_hints
    );
    assert_eq!(
        norito::json::to_value(&state.world.merge_global_state_root).unwrap(),
        exact_global,
        "recovery validation preserves the original current and undo roots"
    );
    for mutate_hints in [false, true] {
        {
            let mut world = state.world.block();
            *world.merge_hint_roots = entry.merge_hint_roots();
            *world.merge_global_state_root = Some(entry.global_state_root);
            if mutate_hints {
                *world.merge_hint_roots = vec![Hash::new(b"foreign relay hints")];
            } else {
                *world.merge_global_state_root = None;
            }
            world.commit();
        }
        let hints = norito::json::to_value(&state.world.merge_hint_roots).unwrap();
        let global = norito::json::to_value(&state.world.merge_global_state_root).unwrap();
        let error = state
            .validate_recovered_merge_metadata(Some(entry.as_ref()))
            .expect_err(
                "restore refuses inconsistent query roots instead of repairing snapshot bytes",
            );
        assert!(
            error
                .to_string()
                .contains("differs from its exact latest applied entry")
        );
        assert_eq!(
            norito::json::to_value(&state.world.merge_hint_roots).unwrap(),
            hints
        );
        assert_eq!(
            norito::json::to_value(&state.world.merge_global_state_root).unwrap(),
            global
        );
    }
}
