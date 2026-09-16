// Authentic native Decisions are replayed through the actual economic executor.
// Preparation remains disposable and supplies no publication or Apply receipt.

state_test! { sync native_economic_batch_seals_actual_transfer_and_all_instance_markers_without_publication
    use iroha_data_model::block::lane_execution::LaneDecisionExecutionBatchV1;
    use iroha_model_base::state_path::StatePath;
    let fixture = native_economic_fixture(&[NativeEconomicCase::Transfer(25)], true);
    let state = &fixture.native.state;
    let groups = native_economic_groups(&fixture);
    let before = crate::snapshot::canonical_state_snapshot_hash(state);
    let carrier = empty_global_block_after(Some(&fixture.native.block)).header();
    let header = LaneDecisionExecutionBatchV1::application_header_from_carrier(&carrier);
    let prepared = state.prepare_lane_decision_execution_batch(header, &groups).unwrap();
    let batch = prepared.batch().clone();
    assert_eq!(batch.base_state_hash, HashOf::from_untyped_unchecked(before));
    assert_eq!(batch.executions.len(), 1);
    assert!(batch.executions[0].result.is_ok());
    assert_eq!(batch.executions[0].source, groups[0].to_wire());
    assert_ne!(batch.application_write_set_root, batch.write_set_root);
    let overlay = prepared.overlay();
    assert_eq!(overlay.world.assets.get(&fixture.source).unwrap().0, Quantity::from(75u32));
    assert_eq!(overlay.world.assets.get(&fixture.destination).unwrap().0, Quantity::from(25u32));
    assert_native_economic_terminal(overlay, &groups[0], carrier.height().get());
    let identity = batch.application_identity().unwrap();
    let mut paths = vec![format!("native_lane_application_{}", hex::encode(identity.as_ref()))];
    paths.extend(batch.executions[0].source.payload.descriptor.slots.iter().map(|slot| {
        format!("native_lane_applied_instance_{}", hex::encode(slot.instance_id.as_ref()))
    }));
    for path in paths {
        let path: StatePath = path.parse().unwrap();
        let value = overlay.world.smart_contract_state.get(&path).expect("actual replay marker");
        assert_eq!(norito::decode_canonical::<Hash>(value).unwrap(), identity);
    }
    let bytes = norito::encode_canonical(&batch).unwrap();
    assert_eq!(LaneDecisionExecutionBatchV1::decode_canonical(&bytes, bytes.len()).unwrap(), batch);
    drop(prepared);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state), before);
    let replay = state.replay_lane_decision_execution_batch(&carrier, &batch, &groups).unwrap();
    assert_eq!(replay.batch(), &batch);
    assert_eq!(replay.overlay().world.assets.get(&fixture.source).unwrap().0, Quantity::from(75u32));
    drop(replay);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state), before);
}

state_test! { sync native_economic_batch_replay_rejects_tampered_claims_and_actual_carrier_context
    use iroha_data_model::block::lane_execution::LaneDecisionExecutionBatchV1;
    let fixture = native_economic_fixture(&[NativeEconomicCase::Transfer(25)], true);
    let state = &fixture.native.state;
    let groups = native_economic_groups(&fixture);
    let before = crate::snapshot::canonical_state_snapshot_hash(state);
    let carrier = empty_global_block_after(Some(&fixture.native.block)).header();
    let header = LaneDecisionExecutionBatchV1::application_header_from_carrier(&carrier);
    let prepared = state.prepare_lane_decision_execution_batch(header, &groups).unwrap();
    let batch = prepared.batch().clone();
    drop(prepared);
    for mutation in 0..5 {
        let mut changed = batch.clone();
        match mutation {
            0 => changed.application_write_set_root = Hash::new(b"foreign economic writes"),
            1 => changed.write_set_root = Hash::new(b"foreign replay markers"),
            2 => changed.base_state_hash = HashOf::from_untyped_unchecked(Hash::new(b"another base")),
            3 => changed.executions[0].settlement.tx_count = 0,
            4 => changed.executions[0].result = TransactionResult::new(Err(iroha_data_model::transaction::error::TransactionRejectionReason::LimitCheck(
                iroha_data_model::transaction::error::TransactionLimitError { reason: "invented rejection".into() },
            ))),
            _ => unreachable!(),
        }
        assert!(state.replay_lane_decision_execution_batch(&carrier, &changed, &groups).is_err(), "mutation {mutation}");
        assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state), before);
    }
    let different = BlockHeader::new(carrier.height(), carrier.prev_block_hash(), None, None,
        u64::try_from(carrier.creation_time().as_millis()).unwrap() + 1, carrier.view_change_index());
    assert!(state.replay_lane_decision_execution_batch(&different, &batch, &groups).is_err());
    assert!(state.replay_lane_decision_execution_batch(&carrier, &batch, &[]).is_err());
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state), before);
}

state_test! { sync native_economic_batch_existing_instance_marker_drops_every_economic_change
    use iroha_data_model::block::lane_execution::LaneDecisionExecutionBatchV1;
    use iroha_model_base::state_path::StatePath;
    let fixture = native_economic_fixture(&[NativeEconomicCase::Transfer(25)], true);
    let state = &fixture.native.state;
    let groups = native_economic_groups(&fixture);
    let slot = &groups[0].body().payload().descriptor.slots[1];
    let marker: StatePath = format!("native_lane_applied_instance_{}", hex::encode(slot.instance_id.as_ref())).parse().unwrap();
    // Inject an inconsistent already-applied marker into this isolated fixture.
    // Even this late refusal must discard the real earlier transfer and all heads.
    let mut storage = state.world.smart_contract_state.block();
    storage.insert(marker, norito::encode_canonical(&Hash::new(b"existing application")).unwrap());
    storage.commit();
    let before = crate::snapshot::canonical_state_snapshot_hash(state);
    let carrier = empty_global_block_after(Some(&fixture.native.block)).header();
    let header = LaneDecisionExecutionBatchV1::application_header_from_carrier(&carrier);
    let error = state.prepare_lane_decision_execution_batch(header, &groups).err().expect("marker collision");
    assert!(matches!(error, MergeLedgerCommitError::ExecutionMarkerConflict(_)), "{error}");
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state), before);
}
