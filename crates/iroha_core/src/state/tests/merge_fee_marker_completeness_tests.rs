//! Structural recovery predicates require every fee effect marker and retain MV history.
//!
//! These fixtures exercise marker classification only. They neither publish a
//! carrier nor authenticate an execution, sponsor allocation, burn, or finality.

use super::*;

fn entry(with_receipts: bool) -> MergeLedgerEntry {
    let incarnation = Hash::new(b"fee completeness structural incarnation");
    let (session, _) = sample_committed_lane_block_session_for_state_test(
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
        incarnation,
        1,
        1,
    );
    let mut settlement =
        empty_merge_settlement(LaneId::SINGLE, incarnation, DataSpaceId::UNIVERSAL, 1);
    if with_receipts {
        for source_id in [[0xA4; 32], [0xA5; 32]] {
            settlement.nexus_fee_receipts.push(NexusFeeReceipt {
                version: NexusFeeReceipt::VERSION,
                source_id,
                dataspace_id: settlement.dataspace_id,
                lane_id: settlement.lane_id,
                block_height: settlement.block_height,
                debit_source: FeeDebitSource::SponsorProgram(FeeSponsorProgramId::new(
                    ALICE_ID.clone(),
                    "marker-completeness".parse().unwrap(),
                )),
                fee_asset_id: AssetDefinitionId::parse_address_literal(
                    &iroha_config::parameters::defaults::nexus::fees::fee_asset_id(),
                )
                .unwrap(),
                program_revision: Some(1),
                lease_id: Some(Hash::new(b"structural fee lease identity")),
                fee_amount: Quantity::from(1_u32),
                schedule: NexusFeeScheduleInputs {
                    tx_bytes_len: 0,
                    instruction_count: 0,
                    gas_used: 0,
                    base_fee: Quantity::from(1_u32),
                    per_byte_fee: Quantity::zero(),
                    per_instruction_fee: Quantity::zero(),
                    per_gas_unit_fee: Quantity::zero(),
                },
            });
        }
    }
    let execution = MergeLaneExecution {
        source_bundle: Vec::new(),
        source_bundle_hash: Hash::new(b"structural marker source"),
        proposal: session.proposal.clone(),
        origin_proposal: session.proposal,
        prepare_qc: session.prepare_qc,
        commit_qc: session.commit_qc,
        signer_proofs: Vec::new(),
        autonomous_network_id: *DEFAULT_TEST_NETWORK_ID,
        autonomous_epoch: 1,
        autonomous_payload_hash: Hash::new(b"structural marker payload"),
        entrypoint_hashes: Vec::new(),
        entrypoints: Vec::new(),
        authenticated_signed_replay_aliases: Vec::new(),
        reservation_keys: Vec::new(),
        routing_plans: Vec::new(),
        native_amx_receipts: Vec::new(),
        result_hashes: Vec::new(),
        results: Vec::new(),
        settlement_hash: canonical_merge_settlement_hash(&settlement).unwrap(),
        settlement_commitment: settlement,
        fastpq_transcripts: Vec::new().into(),
    };
    let lanes = vec![execution];
    let batch = MergeExecutionBatch {
        version: 1,
        base_state_height: 0,
        base_state_hash: HashOf::from_untyped_unchecked(Hash::new(b"structural marker base")),
        application_block_header: BlockHeader::new(nonzero!(1_u64), None, None, 1, 0),
        execution_root: crate::merge::merge_execution_root(&lanes),
        lanes,
        entrypoint_count: 0,
        entrypoint_merkle_root: HashOf::from_untyped_unchecked(Hash::new(b"structural input root")),
        result_merkle_root: HashOf::from_untyped_unchecked(Hash::new(b"structural output root")),
        application_write_set_root: Hash::new(b"structural application writes"),
        write_set_root: Hash::new(b"structural final writes"),
        expected_post_state_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"structural post-state",
        )),
        batch_hash: Hash::new(b"structural batch"),
    };
    let mut entry = merge_entry_from_candidate(merge_candidate_with_lanes(1, 0), dummy_merge_qc());
    entry.execution_batch = Some(batch);
    entry
}

fn expected(entry: &MergeLedgerEntry) -> Vec<(StatePath, Vec<u8>)> {
    State::expected_merge_execution_marker_payloads(entry, entry.execution_batch.as_ref().unwrap())
        .unwrap()
}

fn fee_markers(entry: &MergeLedgerEntry) -> Vec<(StatePath, Vec<u8>)> {
    fee_settlement_markers::expected(entry.execution_batch.as_ref().unwrap()).unwrap()
}

fn install_structural_markers(state: &State, entry: &MergeLedgerEntry) {
    let batch = entry.execution_batch.as_ref().unwrap();
    let frontiers = State::merge_lane_execution_frontier_marker_payloads(batch, 1).unwrap();
    let mut storage = state.world.smart_contract_state.block();
    for (key, value) in expected(entry).into_iter().chain(frontiers) {
        storage.insert(key, value);
    }
    storage.commit();
}

fn encoded_markers(state: &State) -> String {
    let mut bytes = String::new();
    snapshot_storage::serialize(&state.world.smart_contract_state, &mut bytes);
    bytes
}

fn check(state: &State, entry: &MergeLedgerEntry) -> Result<bool, MergeLedgerCommitError> {
    let before = encoded_markers(state);
    let result =
        state.merge_execution_already_applied(entry, entry.execution_batch.as_ref().unwrap());
    assert_eq!(
        encoded_markers(state),
        before,
        "classification is read-only"
    );
    result
}

#[test]
fn complete_fee_expectation_preserves_the_execution_marker_emitter() {
    let entry = entry(true);
    let batch = entry.execution_batch.as_ref().unwrap();
    let execution_only = State::merge_execution_marker_payloads(entry.epoch_id, batch).unwrap();
    let complete = expected(&entry);
    assert_eq!(execution_only.len(), 2);
    assert_eq!(fee_markers(&entry).len(), 3);
    assert_eq!(&complete[..execution_only.len()], execution_only.as_slice());
    assert_eq!(&complete[execution_only.len()..], fee_markers(&entry));
    assert!(fee_markers(&entry).iter().all(|(key, value)| {
        value == &[1] && execution_only.iter().all(|(emitted, _)| emitted != key)
    }));
    let mut foreign = batch.clone();
    foreign.base_state_height += 1;
    assert!(State::expected_merge_execution_marker_payloads(&entry, &foreign).is_err());
}

#[test]
fn applied_execution_requires_each_fee_marker_and_its_canonical_payload() {
    let state = Box::new(blank_test_state());
    let entry = entry(true);
    assert!(!check(&state, &entry).unwrap());
    let (source_key, source_value) = fee_markers(&entry).remove(1);
    let mut storage = state.world.smart_contract_state.block();
    storage.insert(source_key, source_value);
    storage.commit();
    assert!(matches!(
        check(&state, &entry),
        Err(MergeLedgerCommitError::ExecutionMarkerConflict(_))
    ));
    install_structural_markers(&state, &entry);
    assert!(check(&state, &entry).unwrap());
    state
        .ensure_committed_merge_execution_applied(&entry)
        .unwrap();
    for (key, value) in fee_markers(&entry) {
        let mut storage = state.world.smart_contract_state.block();
        storage.remove(key.clone());
        storage.commit();
        assert!(matches!(
            check(&state, &entry),
            Err(MergeLedgerCommitError::ExecutionMarkerConflict(_))
        ));
        assert!(
            state
                .ensure_committed_merge_execution_applied(&entry)
                .is_err()
        );
        for corrupt in [Vec::new(), vec![0], vec![1, 0]] {
            let mut storage = state.world.smart_contract_state.block();
            storage.insert(key.clone(), corrupt);
            storage.commit();
            assert!(matches!(
                check(&state, &entry),
                Err(MergeLedgerCommitError::ExecutionMarkerConflict(_))
            ));
        }
        let mut storage = state.world.smart_contract_state.block();
        storage.insert(key, value);
        storage.commit();
        assert!(check(&state, &entry).unwrap());
    }
    // Complete fee evidence never substitutes for the existing exact frontier check.
    let frontier = State::merge_lane_execution_frontier_marker_payloads(
        entry.execution_batch.as_ref().unwrap(),
        1,
    )
    .unwrap()
    .remove(0);
    let mut storage = state.world.smart_contract_state.block();
    storage.remove(frontier.0.clone());
    storage.commit();
    assert!(check(&state, &entry).is_err());
    let mut storage = state.world.smart_contract_state.block();
    storage.insert(frontier.0, frontier.1);
    for (key, _) in fee_markers(&entry) {
        storage.remove(key);
    }
    storage.commit();
    assert!(matches!(
        check(&state, &entry),
        Err(MergeLedgerCommitError::ExecutionMarkerConflict(_))
    ));
}

#[test]
fn historical_fee_marker_requirements_are_independent_of_current_mode() {
    for with_receipts in [false, true] {
        let state = Box::new(blank_test_state());
        let entry = entry(with_receipts);
        install_structural_markers(&state, &entry);
        for mode in [
            NexusFeeSettlementMode::Direct,
            NexusFeeSettlementMode::LaneRelayBurn,
        ] {
            // Vary only the process policy for this predicate; this is not an
            // authenticated historical configuration transition.
            state.nexus.write().fees.settlement_mode = mode;
            assert!(check(&state, &entry).unwrap());
        }
        let fees = fee_markers(&entry);
        assert_eq!(fees.is_empty(), !with_receipts);
        let mut storage = state.world.smart_contract_state.block();
        for (key, _) in fees {
            storage.remove(key);
        }
        storage.commit();
        for mode in [
            NexusFeeSettlementMode::Direct,
            NexusFeeSettlementMode::LaneRelayBurn,
        ] {
            state.nexus.write().fees.settlement_mode = mode;
            if with_receipts {
                assert!(check(&state, &entry).is_err());
            } else {
                assert!(check(&state, &entry).unwrap());
            }
        }
    }
}

#[test]
fn fee_marker_expectation_uses_transcript_membership_including_zero_amounts() {
    let entry = entry(true);
    let execution = &entry.execution_batch.as_ref().unwrap().lanes[0];
    let markers = |settlements: &[&LaneBlockCommitment]| {
        let mut batch = entry.execution_batch.clone().unwrap();
        batch.lanes = settlements
            .iter()
            .map(|settlement| {
                let mut lane = execution.clone();
                lane.settlement_commitment = (*settlement).clone();
                lane.settlement_hash = canonical_merge_settlement_hash(settlement).unwrap();
                lane
            })
            .collect();
        fee_settlement_markers::expected(&batch).unwrap()
    };
    let mut zero = execution.settlement_commitment.clone();
    zero.nexus_fee_receipts[0].fee_amount = Quantity::zero();
    zero.nexus_fee_receipts[0].schedule.base_fee = Quantity::zero();
    let zero_hash = canonical_merge_settlement_hash(&zero).unwrap();
    let zero_markers = markers(&[&zero]);
    assert_eq!(zero_markers.len(), 3);
    assert_ne!(zero_hash, execution.settlement_hash);
    // Different settlements retain distinct enclosing keys and the same source keys.
    assert_ne!(zero_markers[0], fee_markers(&entry)[0]);
    assert_eq!(&zero_markers[1..], &fee_markers(&entry)[1..]);
    let mut empty = zero.clone();
    empty.nexus_fee_receipts.clear();
    assert!(markers(&[&empty]).is_empty());
    assert_eq!(markers(&[&zero, &empty]), zero_markers);
    let mut second = zero.clone();
    second.lane_id = LaneId::new(7);
    for receipt in &mut second.nexus_fee_receipts {
        receipt.lane_id = second.lane_id;
        receipt.source_id[0] ^= 1;
    }
    let second_markers = markers(&[&second]);
    assert_eq!(
        markers(&[&zero, &second, &empty]),
        [zero_markers, second_markers].concat()
    );
}

#[test]
fn fee_marker_completeness_follows_snapshot_and_actual_replacement_history() {
    let mut state = Box::new(blank_test_state());
    let entry = entry(true);
    install_structural_markers(&state, &entry);
    let missing = fee_markers(&entry).remove(1).0;
    let mut storage = state.world.smart_contract_state.block();
    storage.remove(missing);
    storage.commit();
    let corrupt_current = encoded_markers(&state);
    state.world.smart_contract_state =
        norito::json::from_str::<snapshot_storage::SnapshotStorage>(&corrupt_current)
            .unwrap()
            .decode("fee completeness current and predecessor", |_, _| true)
            .unwrap();
    assert!(check(&state, &entry).is_err());
    drop(state.world.smart_contract_state.block_and_revert());
    assert_eq!(encoded_markers(&state), corrupt_current);
    assert!(check(&state, &entry).is_err());
    state.world.smart_contract_state.block_and_revert().commit();
    assert!(check(&state, &entry).unwrap());
    let restored_current = encoded_markers(&state);
    state.world.smart_contract_state =
        norito::json::from_str::<snapshot_storage::SnapshotStorage>(&restored_current)
            .unwrap()
            .decode("fee completeness second restore", |_, _| true)
            .unwrap();
    assert_eq!(encoded_markers(&state), restored_current);
    assert!(check(&state, &entry).unwrap());
}
