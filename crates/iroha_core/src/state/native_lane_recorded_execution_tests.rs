// Actual source-owned Native execution through metadata and witness capture.
// These controls retain the live publication gate and the original State owners.

state_test! { sync native_recorded_execution_retains_sources_results_aliases_and_complete_witness
    use super::NativeLaneBatchSourcePreparationV1;
    for atomic in [false, true] {
      for case in [NativeEconomicCase::Transfer(25), NativeEconomicCase::BadSignature, NativeEconomicCase::Reveal(0x41)] {
        let fixture = native_economic_fixture_with_genesis_layout(
            &[case],
            atomic,
            Some(DataAvailabilityLayout {
                encoding: PayloadEncoding::ReedSolomon16, chunk_size_bytes: 8192,
                data_shards: 1, parity_shards: 1, max_payload_size_bytes: 2 * 1024 * 1024,
                max_chunk_count: 512,
            }),
        );
        let state = &fixture.native.state;
    let applying = native_control_verified_context(state, fixture.native.block.header().height().get());
        let carrier = native_consumer_stage_carrier(&fixture);
        let original_hash = carrier.hash();
        let original_signatures = carrier.signatures().cloned().collect::<Vec<_>>();
        let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
        let files = exact_test_tree_fingerprint(&state.kura.store_root());
        let NativeLaneBatchSourcePreparationV1::Ready(source) = state.prepare_proposed_native_lane_batch_source(&carrier, &[]).unwrap()
            else { panic!("original signed sources"); };
        let pointers = source.groups_for_test().iter().map(|group| (
            group.body().canonical_bytes().as_ptr(), group.body().source().canonical_control_bytes().as_ptr(),
            group.decisions().as_ptr(), group.contexts().as_ptr(),
        )).collect::<Vec<_>>();
        let recorded = source.record_execution(carrier, applying).unwrap().expect("same original State");
        let prepared = recorded.prepared_for_test();
        let overlay = prepared.overlay();
        assert_eq!(recorded.carrier().hash(), original_hash);
        assert_eq!(recorded.carrier().signatures().cloned().collect::<Vec<_>>(), original_signatures);
        assert!(recorded.carrier().has_results());
        assert_eq!(recorded.carrier().execution_outputs().len(), 1);
        assert_eq!(prepared.executions()[0].result.is_ok(), !matches!(case, NativeEconomicCase::BadSignature));
        for (group, expected) in prepared.sources_for_test().iter().zip(&pointers) {
            assert_eq!((group.body().canonical_bytes().as_ptr(), group.body().source().canonical_control_bytes().as_ptr(),
                group.decisions().as_ptr(), group.contexts().as_ptr()), *expected);
            assert_native_economic_terminal(overlay, group, recorded.carrier().header().height().get());
            assert!(overlay.transactions.get(&group.body().payload().input.entrypoint.hash()).is_some());
        }
        if matches!(case, NativeEconomicCase::Reveal(_)) {
            let reveal = &prepared.sources_for_test()[0].body().payload().input.entrypoint;
            let signed = crate::tx::exact_signed_transaction_hash(reveal).unwrap();
            assert!(overlay.transactions.get(&HashOf::from_untyped_unchecked(Hash::from(signed))).is_some());
            assert_eq!(prepared.executions()[0].authenticated_signed_replay_alias, Some(Hash::from(signed)));
        }
        overlay.verify_execution_output_seal(recorded.carrier()).unwrap();
        let inventory = overlay.verified_fastpq_source_inventory_for_capture().unwrap();
        let witness = overlay.exec_witness.as_ref().expect("actual whole execution capture");
        inventory.verify_ordinary_witness_bundles(&witness.fastpq_transcripts).unwrap();
        assert_eq!(witness.fastpq_transcripts.is_empty(), matches!(case, NativeEconomicCase::BadSignature));
        assert!(overlay.lane_consensus_contexts_seal.is_some());
        assert!(crate::block::ValidBlock::validate_inactive_native_carrier_for_test(recorded.carrier()).unwrap_err().to_string().contains("not active"));
        assert_eq!(exact_test_tree_fingerprint(&state.kura.store_root()), files);
        drop(recorded);
        assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).unwrap(), before);
        // The same thread can immediately begin a new capture; completion held
        // the original guard until checked drain and then released it.
        let guard = crate::sumeragi::witness::begin_exec_witness_capture().unwrap();
        let empty = crate::sumeragi::witness::drain_exec_witness_checked(|_| Ok(())).unwrap();
        assert!(empty.reads.is_empty() && empty.writes.is_empty() && empty.fastpq_transcripts.is_empty());
        drop(guard);
      }
    }
}

state_test! { sync native_recorded_execution_captures_due_start_hook_and_native_transfer_once
    let fixture = native_scratch_due_unlock_fixture();
    let state = &fixture.native.state;
    let applying = native_control_verified_context(state, fixture.native.block.header().height().get());
    let carrier = native_consumer_stage_carrier(&fixture);
    let groups = native_economic_groups(&fixture);
    let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
    let recorded = state.record_native_lane_decision_batch(carrier, groups, applying).unwrap();
    let overlay = recorded.prepared_for_test().overlay();
    assert_native_scratch_unlock_applied(overlay, &fixture);
    let witness = overlay.exec_witness.as_ref().unwrap();
    assert!(witness.fastpq_transcripts.len() >= 2, "real due unlock and Native transfer both survive the single capture");
    overlay.verified_fastpq_source_inventory_for_capture().unwrap()
        .verify_ordinary_witness_bundles(&witness.fastpq_transcripts).unwrap();
    overlay.verify_execution_output_seal(recorded.carrier()).unwrap();
    drop(recorded);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).unwrap(), before);
}

state_test! { sync native_recorded_execution_nested_recorder_refuses_without_mutation_or_reset
    use super::NativeLaneBatchSourcePreparationV1;
    let (fixture, carrier) = proposed_native_batch_fixture(&[NativeEconomicCase::Transfer(25)]);
    let state = &fixture.native.state;
    let applying = native_control_verified_context(state, fixture.native.block.header().height().get());
    let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
    let NativeLaneBatchSourcePreparationV1::Ready(source) = state.prepare_proposed_native_lane_batch_source(&carrier, &[]).unwrap()
        else { panic!("original source"); };
    let guard = crate::sumeragi::witness::begin_exec_witness_capture().unwrap();
    crate::sumeragi::witness::record_read_asset(&fixture.source, Some(&Quantity::from(100u32)));
    let witness_before = norito::encode_canonical(&crate::sumeragi::witness::snapshot_exec_witness()).unwrap();
    let error = source.record_execution(carrier, applying).err().expect("nested capture must refuse rather than deadlock");
    assert!(matches!(error, MergeLedgerCommitError::ExecutionRecorderConflict(_)), "{error}");
    assert!(error.to_string().contains("already belongs"), "{error}");
    assert_eq!(norito::encode_canonical(&crate::sumeragi::witness::snapshot_exec_witness()).unwrap(), witness_before);
    drop(guard);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).unwrap(), before);
}

state_test! { sync native_recorded_metadata_rejects_settlement_substitution_before_mutation
    let (fixture, mut carrier) = proposed_native_batch_fixture(&[NativeEconomicCase::Transfer(25)]);
    let state = &fixture.native.state;
    let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
    let groups = native_economic_groups(&fixture);
    let mut prepared = state.prepare_native_batch_on_carrier(carrier.header(), groups).unwrap();
    let mut executions = prepared.executions().iter().map(|actual| super::PreexecutedLaneDecisionGroupV1 {
        source: actual.source.clone(), result: actual.result.clone(),
        authenticated_signed_replay_alias: actual.authenticated_signed_replay_alias,
        settlement_commitment: actual.settlement_commitment.clone(),
        settlement_hash: actual.settlement_hash, fastpq_transcripts: actual.fastpq_transcripts.clone(),
    }).collect::<Vec<_>>();
    executions[0].settlement_commitment.total_xor_due = Quantity::from(1u32);
    executions[0].settlement_hash = super::canonical_merge_settlement_hash(&executions[0].settlement_commitment).unwrap();
    let delta = prepared.overlay().world.net_state_delta().unwrap();
    let error = crate::block::ValidBlock::seal_native_execution_outputs(
        &mut carrier, prepared.overlay_mut_for_test(), &executions,
    ).unwrap_err();
    assert!(error.to_string().contains("actual source or settlement"), "{error}");
    assert_eq!(prepared.overlay().world.net_state_delta().unwrap(), delta);
    assert!(!carrier.has_results());
    drop(prepared);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).unwrap(), before);
}

state_test! { sync native_recorded_execution_captures_pipeline_and_time_without_second_execution
    let (fixture, parent, child) = pipeline_receipt_fixture(true);
    let authority = fixture.source.account().clone();
    let mut metadata = iroha_model_base::metadata::Metadata::default();
    metadata.insert("__registered_block_height".parse::<Name>().unwrap(), Json::new(0u64));
    let time_id: TriggerId = "native_recorded_time".parse().unwrap();
    let trigger = Trigger::new(time_id.clone(), Action::new(
        [InstructionBox::from(SetKeyValue::account(authority.clone(), "native_recorded_time_effect".parse().unwrap(), Json::new(1u32)))],
        Repeats::Exactly(1), authority.clone(), TimeEventFilter::new(ExecutionTime::PreCommit),
    ).unwrap().with_metadata(metadata));
    {
        let mut block = fixture.native.state.world.triggers.block();
        let mut transaction = block.transaction();
        assert!(transaction.add_time_trigger(trigger.try_into().unwrap()).unwrap());
        transaction.apply();
        block.commit();
    }
    let state = &fixture.native.state;
    let applying = native_control_verified_context(state, fixture.native.block.header().height().get());
    let groups = native_economic_groups(&fixture);
    let carrier = native_consumer_stage_carrier(&fixture);
    let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
    let recorded = state.record_native_lane_decision_batch(carrier, groups, applying).unwrap();
    let overlay = recorded.prepared_for_test().overlay();
    use iroha_data_model::block::execution_output::ExecutionOutputV1;
    let rows = recorded.carrier().execution_outputs();
    assert!(matches!(rows, [ExecutionOutputV1::Network(_), ExecutionOutputV1::Pipeline(_), ExecutionOutputV1::Time(_)]));
    assert!(rows.iter().all(|row| row.result().is_ok()));
    assert_eq!(rows[1].completions().len(), 2);
    assert_eq!(overlay.world.assets.get(&fixture.source).unwrap().0, Quantity::from(72u32));
    assert_eq!(overlay.world.assets.get(&fixture.destination).unwrap().0, Quantity::from(28u32));
    assert_eq!(overlay.world.account(&authority).unwrap().metadata().get("native_recorded_time_effect"), Some(&Json::new(1u32)));
    assert!(overlay.world.triggers.pipeline_triggers().get(&parent).is_none());
    assert!(overlay.world.triggers.by_call_triggers().get(&child).is_none());
    assert!(overlay.world.triggers.time_triggers().get(&time_id).is_none());
    let inventory = overlay.verified_fastpq_source_inventory_for_capture().unwrap();
    assert_eq!(inventory.entries().len(), 3);
    inventory.verify_ordinary_witness_bundles(&overlay.exec_witness.as_ref().unwrap().fastpq_transcripts).unwrap();
    overlay.verify_execution_output_seal(recorded.carrier()).unwrap();
    drop(recorded);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).unwrap(), before);
}

state_test! { sync native_recorded_execution_late_failure_discards_hook_effects_and_recorder
    use iroha_model_base::state_path::StatePath;
    let fixture = native_scratch_due_unlock_fixture();
    let state = &fixture.native.state;
    let applying = native_control_verified_context(state, fixture.native.block.header().height().get());
    let groups = native_economic_groups(&fixture);
    let slot = &groups[0].body().payload().descriptor.slots[1];
    let marker: StatePath = format!("native_lane_applied_instance_{}", hex::encode(slot.instance_id.as_ref())).parse().unwrap();
    {
        let mut storage = state.world.smart_contract_state.block();
        storage.insert(marker, norito::encode_canonical(&Hash::new(b"existing recorded Native application")).unwrap());
        storage.commit();
    }
    let carrier = native_consumer_stage_carrier(&fixture);
    let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
    let files = exact_test_tree_fingerprint(&state.kura.store_root());
    let error = state.record_native_lane_decision_batch(carrier, groups, applying).err().expect("actual late marker collision");
    assert!(matches!(error, MergeLedgerCommitError::ExecutionMarkerConflict(_)), "{error}");
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).unwrap(), before);
    assert_eq!(exact_test_tree_fingerprint(&state.kura.store_root()), files);
    let guard = crate::sumeragi::witness::exec_witness_guard();
    let empty = crate::sumeragi::witness::drain_exec_witness();
    assert!(empty.reads.is_empty() && empty.writes.is_empty() && empty.fastpq_transcripts.is_empty());
    drop(guard);
}

state_test! { sync native_recorded_execution_nested_owner_refuses_before_waiting_for_state_writer
    use super::NativeLaneBatchSourcePreparationV1;
    use std::sync::atomic::{AtomicBool, Ordering};
    let (fixture, carrier) = proposed_native_batch_fixture(&[NativeEconomicCase::Transfer(25)]);
    let state = &fixture.native.state;
    let applying = native_control_verified_context(state, fixture.native.block.header().height().get());
    let NativeLaneBatchSourcePreparationV1::Ready(source) = state.prepare_proposed_native_lane_batch_source(&carrier, &[]).unwrap()
        else { panic!("original source"); };
    let header = carrier.header();
    let guard = crate::sumeragi::witness::begin_exec_witness_capture().unwrap();
    let released = AtomicBool::new(false);
    let (held_tx, held_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    std::thread::scope(|scope| {
        let released = &released;
        let holder = scope.spawn(move || {
            let overlay = native_scratch_owned_start(state, header);
            held_tx.send(()).unwrap();
            // A broken early check fails finitely instead of deadlocking the
            // test process. The assertion below proves rejection while held.
            let _ = release_rx.recv_timeout(std::time::Duration::from_secs(5));
            released.store(true, Ordering::Release);
            drop(overlay);
        });
        held_rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        let result = source.record_execution(carrier, applying);
        let refused_while_held = !released.load(Ordering::Acquire);
        let _ = release_tx.send(());
        holder.join().unwrap();
        let error = result.err().expect("nested owner must refuse");
        assert!(matches!(error, MergeLedgerCommitError::ExecutionRecorderConflict(_)), "{error}");
        assert!(error.to_string().contains("already belongs"), "{error}");
        assert!(refused_while_held, "recorder eligibility must be checked before State acquisition");
    });
    drop(guard);
}

state_test! { sync native_recorded_cursor_postcheck_without_bundle_does_not_reenter_cold_cache
    let (fixture, carrier) = proposed_native_batch_fixture(&[NativeEconomicCase::Transfer(25)]);
    let state = &fixture.native.state;
    let groups = native_economic_groups(&fixture);
    let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
    let prepared = state.prepare_native_batch_on_carrier(carrier.header(), groups).unwrap();
    assert!(carrier.da_commitments().is_none());
    // Reproduce the interval between a rewind clearing the cached result and
    // waiting for the State writer retained by this original execution owner.
    *state.da_indexes_hydrated.write() = None;
    let recorder = crate::sumeragi::witness::begin_exec_witness_capture().unwrap();
    prepared.overlay().validate_da_shard_cursors(&carrier).unwrap();
    assert!(state.da_indexes_hydrated.read().is_none());
    drop(recorder);
    drop(prepared);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).unwrap(), before);
}

state_test! { sync native_recorded_controls_reject_foreign_pristine_state_and_stale_owner
    let (fixture, carrier) = proposed_native_batch_fixture(&[NativeEconomicCase::Transfer(25)]);
    let (other, _) = proposed_native_batch_fixture(&[NativeEconomicCase::Transfer(25)]);
    let state = &fixture.native.state;
    let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
    let other_before = crate::snapshot::canonical_state_snapshot_hash(&other.native.state).unwrap();
    let applying = native_control_verified_context(state, fixture.native.block.header().height().get());
    let controls = crate::block::ValidBlock::prepare_native_execution_controls(&carrier, state, applying.clone()).unwrap();
    let error = other.native.state.block_with_pristine_stage(carrier.header(), |overlay| {
        let _recorder = crate::sumeragi::witness::begin_exec_witness_capture().unwrap();
        controls.apply(overlay).map(|_| ())
    }).err().expect("the same header cannot transfer original State ownership");
    assert!(error.to_string().contains("original pristine State owner"), "{error}");
    let controls = crate::block::ValidBlock::prepare_native_execution_controls(&carrier, state, applying).unwrap();
    // A publication generation change invalidates captured control authority,
    // even when the carrier hash and committed economic bytes remain equal.
    {
        let mut publication_notice = state.state_view_publication();
        drop(publication_notice.begin());
    }
    let error = state.block_with_pristine_stage(carrier.header(), |overlay| {
        let _recorder = crate::sumeragi::witness::begin_exec_witness_capture().unwrap();
        controls.apply(overlay).map(|_| ())
    }).err().expect("stale control observation cannot execute");
    assert!(error.to_string().contains("original pristine State owner"), "{error}");
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).unwrap(), before);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(&other.native.state).unwrap(), other_before);
    assert_native_economic_relay_recorder_released();
}

state_test! { sync native_recorded_direct_entry_rejects_missing_or_foreign_da_policy
    let (fixture, original) = proposed_native_batch_fixture(&[NativeEconomicCase::Transfer(25)]);
    let state = &fixture.native.state;
    let applying = native_control_verified_context(state, fixture.native.block.header().height().get());
    let groups = native_economic_groups(&fixture);
    let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
    for policy in [None, Some(crate::da::proof_policy_bundle(&iroha_config::parameters::actual::LaneConfig::default()))] {
        let mut carrier = original.clone();
        carrier.set_da_proof_policies(policy);
        native_control_resign_carrier(&mut carrier);
        let error = state.record_native_lane_decision_batch(carrier, groups.clone(), applying.clone())
            .err().expect("direct recording has the same active policy boundary as source preparation");
        assert!(error.to_string().contains("active pre-State policy"), "{error}");
        assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).unwrap(), before);
        assert_native_economic_relay_recorder_released();
    }
}
