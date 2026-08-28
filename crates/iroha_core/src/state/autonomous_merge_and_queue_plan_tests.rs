include!("autonomous_merge_and_queue_plan_test_support.rs");
include!("autonomous_merge_admission_intent_tests.rs");
#[test]
fn finalized_merge_execution_commit_surface_borrows_exact_carrier_hash() {
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let carrier_hash = header.hash();
    let surface = MergeExecutionCommitSurface::FinalizedCarrier {
        carrier_height: header.height().get(),
        carrier_hash: &carrier_hash,
    };
    let MergeExecutionCommitSurface::FinalizedCarrier {
        carrier_height,
        carrier_hash: borrowed_hash,
    } = surface
    else {
        panic!("fixture must retain the finalized-carrier surface")
    };
    assert_eq!(carrier_height, header.height().get());
    assert_eq!(*borrowed_hash, carrier_hash);
    assert!(core::ptr::eq(borrowed_hash, &carrier_hash));
}
#[test]
fn canonical_wsv_authorization_commits_exact_autonomous_execution_once() {
    let (state, entry, carrier, _) = autonomous_merge_commit_authorization_fixture(false, false);
    commit_staged_autonomous_for_test(staged_autonomous_merge_commit_block(
        &state, &entry, &carrier,
    ))
    .expect("exact authorized autonomous execution must commit");
    assert_eq!(
        state.committed_height(),
        usize::try_from(carrier.header().height().get()).expect("carrier height fits usize"),
    );
    assert!(
        state
            .merge_execution_already_applied(
                &entry,
                entry
                    .execution_batch
                    .as_ref()
                    .expect("fixture carries execution"),
            )
            .expect("committed marker lookup"),
        "canonical commit must publish its replay markers"
    );
}
#[test]
fn queue_plan_synced_transfer_binds_fastpq_transcript_and_commits_after_ttl() {
    let (state, entry, carrier) = autonomous_merge_transfer_commit_authorization_fixture();
    let batch = entry
        .execution_batch
        .as_ref()
        .expect("QueuePlan transfer produces an autonomous execution batch");
    let lane = batch.lanes.first().expect("fixture carries one lane");
    let TransactionEntrypoint::External(transaction) = &lane.entrypoints[0] else {
        panic!("fixture QueuePlan transfer is external")
    };
    assert_eq!(
        transaction.admission_intent(),
        iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced,
    );
    let expires_at = transaction
        .creation_time()
        .checked_add(transaction.time_to_live().expect("fixture TTL"))
        .expect("fixture expiry fits");
    assert!(
        carrier.header().creation_time() > expires_at,
        "embedded canonical execution must remain valid after top-level transaction TTL"
    );
    assert_eq!(
        lane.fastpq_transcripts.len(),
        1,
        "numeric transfer must emit FASTPQ evidence; results={:?}",
        lane.results
    );
    let bundle = &lane.fastpq_transcripts[0];
    assert_eq!(bundle.entry_hash, lane.entrypoint_hashes[0]);
    assert_eq!(bundle.transcripts.len(), 1);
    let transcript = &bundle.transcripts[0];
    assert_eq!(transcript.batch_hash, bundle.entry_hash);
    let delta = transcript
        .deltas
        .first()
        .expect("numeric transfer emits one FASTPQ delta")
        .clone();
    assert_eq!(delta.amount, Quantity::from(3_u32));
    let source_asset = AssetId::new(delta.asset_definition.clone(), delta.from_account.clone());
    let destination_asset = AssetId::new(delta.asset_definition.clone(), delta.to_account.clone());
    let balance = |asset_id: &AssetId| {
        state
            .world
            .view()
            .assets()
            .get(asset_id)
            .map(|value| value.as_ref().clone())
            .unwrap_or_else(Quantity::zero)
    };
    assert_eq!(balance(&source_asset), Quantity::from(10_u32));
    assert_eq!(balance(&destination_asset), Quantity::zero());
    let mut lane_without_evidence = lane.clone();
    lane_without_evidence.fastpq_transcripts = Vec::new().into();
    assert_ne!(
        crate::merge::merge_lane_execution_hash(lane),
        crate::merge::merge_lane_execution_hash(&lane_without_evidence),
        "FASTPQ evidence must change the lane execution hash"
    );
    assert_ne!(
        batch.execution_root,
        crate::merge::merge_execution_root(core::slice::from_ref(&lane_without_evidence)),
        "FASTPQ evidence must change the execution root"
    );
    let mut batch_without_evidence = batch.clone();
    batch_without_evidence.lanes[0] = lane_without_evidence;
    batch_without_evidence.execution_root =
        crate::merge::merge_execution_root(&batch_without_evidence.lanes);
    assert_ne!(
        crate::merge::merge_execution_batch_hash(batch),
        crate::merge::merge_execution_batch_hash(&batch_without_evidence),
        "FASTPQ evidence must change the execution batch hash"
    );
    let pending_obligation_key = State::queue_plan_pending_obligation_marker_key(
        crate::torii_proxy::queue_plan_admission_network_id_digest(state.network_id_ref()),
        lane.entrypoints[0].hash(),
    )
    .expect("fixture pending-obligation key");
    commit_staged_autonomous_for_test(staged_autonomous_merge_commit_block(
        &state, &entry, &carrier,
    ))
    .expect("follower replay and exact carrier commit accept transcript-bound execution");
    assert_eq!(balance(&source_asset), Quantity::from(7_u32));
    assert_eq!(balance(&destination_asset), Quantity::from(3_u32));
    assert!(
        state
            .world
            .view()
            .smart_contract_state()
            .get(&pending_obligation_key)
            .is_none(),
        "canonical execution resolves the durable QueuePlan obligation"
    );
}
fn assert_autonomous_batch_transfer_carrier_roundtrip(mode: QueuePlanTransferFixture) {
    let (state, entry, carrier) =
        autonomous_merge_batch_transfer_commit_authorization_fixture(mode);
    let batch = entry
        .execution_batch
        .as_ref()
        .expect("batch transfer produces an autonomous execution batch");
    let lane = batch.lanes.first().expect("fixture carries one lane");
    assert_eq!(lane.results.len(), 1);
    let outcomes = lane.results[0].batch_transfer_outcomes();
    assert_eq!(
        outcomes.len(),
        2,
        "both batch legs must be result-bound; result={:?}",
        lane.results[0]
    );
    assert_eq!(
        outcomes
            .iter()
            .map(|outcome| outcome.leg_index)
            .collect::<Vec<_>>(),
        vec![0, 1],
        "receipt order must remain exact execution order"
    );
    assert_eq!(outcomes[0].leg_id, "autonomous-batch-leg-a");
    assert_eq!(outcomes[1].leg_id, "autonomous-batch-leg-b");
    assert!(outcomes.iter().all(|outcome| matches!(
        &outcome.status,
        data_pre::AssetBatchTransferLegStatus::Applied
    )));
    assert_eq!(outcomes[0].asset, outcomes[1].asset);
    let source_asset = outcomes[0].asset.clone();
    let destination_assets = outcomes
        .iter()
        .map(|outcome| {
            AssetId::new(
                source_asset.definition().clone(),
                outcome.destination.clone(),
            )
        })
        .collect::<Vec<_>>();
    let balance = |asset_id: &AssetId| {
        state
            .world
            .view()
            .assets()
            .get(asset_id)
            .map(|value| value.as_ref().clone())
            .unwrap_or_else(Quantity::zero)
    };
    assert_eq!(balance(&source_asset), Quantity::from(20_u32));
    assert_eq!(balance(&destination_assets[0]), Quantity::zero());
    assert_eq!(balance(&destination_assets[1]), Quantity::zero());

    let mut result_without_receipts = lane.results[0].clone();
    result_without_receipts.set_batch_transfer_outcomes(Vec::new());
    assert_ne!(
        lane.results[0].hash(),
        result_without_receipts.hash(),
        "receipt rows must change the transaction-result leaf"
    );
    let mut lane_without_receipts = lane.clone();
    lane_without_receipts.results[0] = result_without_receipts;
    lane_without_receipts.result_hashes[0] = Hash::from(lane_without_receipts.results[0].hash());
    assert_ne!(
        crate::merge::merge_lane_execution_hash(lane),
        crate::merge::merge_lane_execution_hash(&lane_without_receipts),
        "receipt-bound results must change the lane execution hash"
    );

    assert!(!matches!(mode, QueuePlanTransferFixture::Single));
    assert_eq!(lane.fastpq_transcripts.len(), 1);
    let transcripts = &lane.fastpq_transcripts[0].transcripts;
    assert_eq!(transcripts.len(), 1);
    assert_eq!(transcripts[0].deltas.len(), 2);
    assert_eq!(transcripts[0].poseidon_preimage_digest, None);
    for (delta, outcome) in transcripts[0].deltas.iter().zip(outcomes) {
        assert_eq!(&delta.from_account, source_asset.account());
        assert_eq!(&delta.to_account, &outcome.destination);
        assert_eq!(&delta.asset_definition, source_asset.definition());
        assert_eq!(&delta.amount, &outcome.amount);
    }

    let state_block = production_validated_autonomous_merge_commit_block(&state, &entry, &carrier);
    commit_staged_autonomous_for_test(state_block)
        .expect("production-validated autonomous batch carrier must commit");
    assert_eq!(balance(&source_asset), Quantity::from(13_u32));
    assert_eq!(balance(&destination_assets[0]), Quantity::from(3_u32));
    assert_eq!(balance(&destination_assets[1]), Quantity::from(4_u32));
    assert!(
        state
            .merge_execution_already_applied(&entry, batch)
            .expect("committed marker lookup"),
        "batch effects and receipt-bound execution must have one applied marker set"
    );
    match state.block_with_certified_merge_entry(
        carrier.header().clone(),
        &entry,
        ConsensusMode::Permissioned,
    ) {
        Err(MergeLedgerCommitError::NonMonotonicEpoch {
            expected,
            attempted,
        }) => {
            assert_eq!(expected, 2);
            assert_eq!(attempted, 1);
        }
        Err(MergeLedgerCommitError::ExecutionMarkerConflict(_)) => {}
        Err(error) => panic!("unexpected autonomous batch replay rejection: {error:?}"),
        Ok(_) => panic!("committed autonomous batch replay must fail closed"),
    }
    assert_eq!(balance(&source_asset), Quantity::from(13_u32));
    assert_eq!(balance(&destination_assets[0]), Quantity::from(3_u32));
    assert_eq!(balance(&destination_assets[1]), Quantity::from(4_u32));
}
#[test]
fn autonomous_atomic_batch_receipts_survive_production_carrier_validation_and_apply_once() {
    assert_autonomous_batch_transfer_carrier_roundtrip(QueuePlanTransferFixture::AtomicBatch);
}
#[test]
fn autonomous_independent_batch_receipts_and_fastpq_survive_production_carrier_once() {
    assert_autonomous_batch_transfer_carrier_roundtrip(QueuePlanTransferFixture::IndependentBatch);
}
fn rebind_mutated_fastpq_batch(batch: &mut MergeExecutionBatch) {
    batch.execution_root = crate::merge::merge_execution_root(&batch.lanes);
    batch.batch_hash = crate::merge::merge_execution_batch_hash(batch);
}
fn assert_fastpq_batch_rejected(
    state: &State,
    active_lanes: &[MergeLaneBinding],
    mut batch: MergeExecutionBatch,
    expected_reason: &str,
) {
    rebind_mutated_fastpq_batch(&mut batch);
    match state.validate_merge_execution_batch(
        active_lanes,
        &batch,
        &std::collections::BTreeMap::new(),
        true,
        Some(ConsensusMode::Permissioned),
    ) {
        Err(MergeLedgerCommitError::ExecutionBatchInvalid(reason)) => {
            assert_eq!(reason, expected_reason)
        }
        other => panic!("malformed FASTPQ evidence was not rejected as expected: {other:?}"),
    }
}
include!("autonomous_merge_fastpq_shape_tests.rs");
#[test]
fn sealed_reveal_fastpq_transcripts_bind_inner_call_to_outer_lane_identity() {
    let (state, entry, carrier) = autonomous_merge_transfer_commit_authorization_fixture();
    let lane = &entry
        .execution_batch
        .as_ref()
        .expect("fixture carries one execution batch")
        .lanes[0];
    let TransactionEntrypoint::External(signed) = lane.entrypoints[0].clone() else {
        panic!("fixture carries one external numeric transfer")
    };
    let salt = [0xB7; 32];
    let commitment = iroha_data_model::transaction::signed::compute_sealed_transaction_commitment(
        state.network_id_ref(),
        &signed,
        salt,
        64,
    );
    let sealed_entrypoint = TransactionEntrypoint::SealedReveal(
        iroha_data_model::transaction::signed::SealedTransactionReveal::new(
            commitment, signed, salt,
        ),
    );
    let outer_entrypoint_hash = Hash::from(sealed_entrypoint.hash());
    let inner_call_hash = StateBlock::merge_execution_call_hash(&sealed_entrypoint);
    assert_ne!(outer_entrypoint_hash, inner_call_hash);

    let mut transcript = lane.fastpq_transcripts[0].transcripts[0].clone();
    transcript.batch_hash = inner_call_hash;
    let mut state_block = state.block(carrier.header().clone());
    state_block
        .fastpq_transcripts
        .insert(inner_call_hash, vec![transcript]);
    let bundles = state_block
        .take_merge_lane_fastpq_transcripts(core::slice::from_ref(&sealed_entrypoint))
        .expect("sealed reveal maps its inner call evidence to its outer lane identity");
    assert_eq!(bundles.len(), 1);
    assert_eq!(bundles[0].entry_hash, outer_entrypoint_hash);
    assert_eq!(bundles[0].transcripts[0].batch_hash, inner_call_hash);
    assert!(state_block.fastpq_transcripts.is_empty());
    state_block
        .validate_merge_execution_commit_surface(MergeExecutionCommitSurface::Pristine)
        .expect("sealed-reveal evidence extraction leaves no unbound side effect");
}
#[test]
fn sealed_reveal_batch_outcomes_bind_inner_call_to_outer_result_leaf() {
    let (state, entry, carrier) = autonomous_merge_batch_transfer_commit_authorization_fixture(
        QueuePlanTransferFixture::AtomicBatch,
    );
    let lane = &entry
        .execution_batch
        .as_ref()
        .expect("fixture carries one execution batch")
        .lanes[0];
    let TransactionEntrypoint::External(signed) = lane.entrypoints[0].clone() else {
        panic!("fixture carries one external atomic batch")
    };
    let salt = [0xC7; 32];
    let commitment = iroha_data_model::transaction::signed::compute_sealed_transaction_commitment(
        state.network_id_ref(),
        &signed,
        salt,
        64,
    );
    let sealed_entrypoint = TransactionEntrypoint::SealedReveal(
        iroha_data_model::transaction::signed::SealedTransactionReveal::new(
            commitment, signed, salt,
        ),
    );
    let outer_entrypoint_hash = Hash::from(sealed_entrypoint.hash());
    let inner_call_hash = StateBlock::merge_execution_call_hash(&sealed_entrypoint);
    assert_ne!(outer_entrypoint_hash, inner_call_hash);

    let expected_outcomes = lane.results[0].batch_transfer_outcomes().to_vec();
    let mut result = lane.results[0].clone();
    result.set_batch_transfer_outcomes(Vec::new());
    let mut state_block = state.block(carrier.header().clone());
    state_block.batch_transfer_outcomes.insert(
        HashOf::<TransactionEntrypoint>::from_untyped_unchecked(inner_call_hash),
        expected_outcomes.clone(),
    );
    state_block
        .take_merge_lane_batch_transfer_outcomes(
            core::slice::from_ref(&sealed_entrypoint),
            core::slice::from_mut(&mut result),
        )
        .expect("sealed reveal maps its inner receipt key to its outer result leaf");
    assert_eq!(result.batch_transfer_outcomes(), expected_outcomes);
    assert!(state_block.batch_transfer_outcomes.is_empty());
    state_block
        .validate_merge_execution_commit_surface(MergeExecutionCommitSurface::Pristine)
        .expect("sealed-reveal receipt extraction leaves no unbound side effect");
}
#[test]
fn unbound_fastpq_transcript_remains_a_fail_closed_commit_surface() {
    let (state, entry, carrier) = autonomous_merge_transfer_commit_authorization_fixture();
    let lane = &entry
        .execution_batch
        .as_ref()
        .expect("fixture carries one execution batch")
        .lanes[0];
    let unbound_hash = Hash::new(b"unbound FASTPQ transcript");
    let mut transcript = lane.fastpq_transcripts[0].transcripts[0].clone();
    transcript.batch_hash = unbound_hash;
    let mut state_block = state.block(carrier.header().clone());
    state_block
        .fastpq_transcripts
        .insert(unbound_hash, vec![transcript]);
    assert!(
        state_block
            .take_merge_lane_fastpq_transcripts(&lane.entrypoints)
            .expect("known entrypoint extraction itself remains well formed")
            .is_empty()
    );
    assert!(matches!(
        state_block.validate_merge_execution_commit_surface(MergeExecutionCommitSurface::Pristine),
        Err(MergeLedgerCommitError::ExecutionBatchInvalid(reason))
            if reason == "autonomous merge execution staged an effect outside the bound WSV overlay"
    ));
}
#[test]
fn unbound_batch_transfer_outcome_remains_a_fail_closed_commit_surface() {
    let (state, entry, carrier) = autonomous_merge_batch_transfer_commit_authorization_fixture(
        QueuePlanTransferFixture::AtomicBatch,
    );
    let lane = &entry
        .execution_batch
        .as_ref()
        .expect("fixture carries one execution batch")
        .lanes[0];
    let outcome = lane.results[0]
        .batch_transfer_outcomes()
        .first()
        .expect("atomic batch emits a receipt")
        .clone();
    let mut state_block = state.block(carrier.header().clone());
    let unbound_hash = Hash::new(b"unbound autonomous batch-transfer outcome");
    state_block.batch_transfer_outcomes.insert(
        HashOf::<TransactionEntrypoint>::from_untyped_unchecked(unbound_hash),
        vec![outcome],
    );
    let mut result = lane.results[0].clone();
    result.set_batch_transfer_outcomes(Vec::new());
    state_block
        .take_merge_lane_batch_transfer_outcomes(
            &lane.entrypoints,
            core::slice::from_mut(&mut result),
        )
        .expect("known entrypoint extraction itself remains well formed");
    assert!(result.batch_transfer_outcomes().is_empty());
    assert!(matches!(
        state_block.validate_merge_execution_commit_surface(MergeExecutionCommitSurface::Pristine),
        Err(MergeLedgerCommitError::ExecutionBatchInvalid(reason))
            if reason == "autonomous merge execution staged an effect outside the bound WSV overlay"
    ));
}
#[test]
fn autonomous_execution_commit_rejects_missing_apply_carrier_authorization() {
    let (state, entry, carrier, _) = autonomous_merge_commit_authorization_fixture(false, false);
    let state_block = staged_autonomous_merge_commit_block(&state, &entry, &carrier);
    assert!(matches!(
        state_block.commit(),
        Err(TransactionsBlockError::MergeAdmission)
    ));
    assert_eq!(
        state.committed_height(),
        autonomous_carrier_parent_height(&carrier),
    );
}
#[test]
fn autonomous_execution_commit_rejects_missing_wsv_authorization() {
    let (state, entry, carrier, _) = autonomous_merge_commit_authorization_fixture(false, false);
    let mut state_block = staged_autonomous_merge_commit_block(&state, &entry, &carrier);
    let _authorization = state_block
        .canonical_wsv_merge_commit_authorization
        .take()
        .expect("fixture authorization");
    assert!(matches!(
        commit_staged_autonomous_for_test(state_block),
        Err(TransactionsBlockError::MergeAdmission)
    ));
    assert_eq!(
        state.committed_height(),
        autonomous_carrier_parent_height(&carrier),
    );
}
#[test]
fn autonomous_execution_commit_rejects_missing_carrier_metadata_authorization() {
    let (state, entry, carrier, _) = autonomous_merge_commit_authorization_fixture(false, false);
    let mut state_block = staged_autonomous_merge_commit_block(&state, &entry, &carrier);
    let _authorization = state_block
        .canonical_carrier_commit_metadata_authorization
        .take()
        .expect("fixture carrier metadata authorization");
    assert!(matches!(
        commit_staged_autonomous_for_test(state_block),
        Err(TransactionsBlockError::MergeAdmission)
    ));
    assert_eq!(
        state.committed_height(),
        autonomous_carrier_parent_height(&carrier),
    );
}
#[test]
fn autonomous_execution_commit_rejects_mismatched_wsv_authorization() {
    let (state, entry, carrier, _) = autonomous_merge_commit_authorization_fixture(false, false);
    let mut state_block = staged_autonomous_merge_commit_block(&state, &entry, &carrier);
    state_block
        .canonical_wsv_merge_commit_authorization
        .as_mut()
        .expect("fixture authorization")
        .batch_hash = Hash::new(b"mismatched-canonical-wsv-authorization");
    assert!(matches!(
        commit_staged_autonomous_for_test(state_block),
        Err(TransactionsBlockError::MergeAdmission)
    ));
    assert_eq!(
        state.committed_height(),
        autonomous_carrier_parent_height(&carrier),
    );
}
#[test]
fn autonomous_execution_commit_rejects_replayed_carrier_metadata_authorization() {
    let (first_state, first_entry, first_carrier, _) =
        autonomous_merge_commit_authorization_fixture(false, false);
    let mut first_block =
        staged_autonomous_merge_commit_block(&first_state, &first_entry, &first_carrier);
    let replayed_authorization = first_block
        .canonical_carrier_commit_metadata_authorization
        .take()
        .expect("first fixture carrier metadata authorization");
    drop(first_block);
    let (second_state, second_entry, second_carrier, _) =
        autonomous_merge_commit_authorization_fixture(false, false);
    let mut second_block =
        staged_autonomous_merge_commit_block(&second_state, &second_entry, &second_carrier);
    second_block.canonical_carrier_commit_metadata_authorization = Some(replayed_authorization);
    assert!(matches!(
        commit_staged_autonomous_for_test(second_block),
        Err(TransactionsBlockError::MergeAdmission)
    ));
    assert_eq!(
        second_state.committed_height(),
        autonomous_carrier_parent_height(&second_carrier),
    );
}
#[test]
fn autonomous_execution_commit_rejects_stale_authorized_base() {
    let (state, entry, carrier, _) = autonomous_merge_commit_authorization_fixture(false, false);
    let mut state_block = staged_autonomous_merge_commit_block(&state, &entry, &carrier);
    state_block
        .canonical_wsv_merge_commit_authorization
        .as_mut()
        .expect("fixture authorization")
        .base_state_height = 0;
    assert!(matches!(
        commit_staged_autonomous_for_test(state_block),
        Err(TransactionsBlockError::MergeAdmission)
    ));
    assert_eq!(
        state.committed_height(),
        autonomous_carrier_parent_height(&carrier),
    );
}
#[test]
fn autonomous_execution_commit_rejects_post_stage_wsv_drift() {
    let (state, entry, carrier, _) = autonomous_merge_commit_authorization_fixture(false, false);
    let mut state_block = staged_autonomous_merge_commit_block(&state, &entry, &carrier);
    let drift_key = StatePath::from_str("canonical_wsv_authorization_post_stage_drift")
        .expect("fixture state path");
    state_block
        .world
        .smart_contract_state
        .insert(drift_key, vec![0xD1]);
    assert!(matches!(
        commit_staged_autonomous_for_test(state_block),
        Err(TransactionsBlockError::MergeAdmission)
    ));
    assert_eq!(
        state.committed_height(),
        autonomous_carrier_parent_height(&carrier),
    );
}
#[test]
fn autonomous_execution_commit_rejects_post_stage_runtime_surface_drift() {
    let (state, entry, carrier, _) = autonomous_merge_commit_authorization_fixture(false, false);
    let mut state_block = staged_autonomous_merge_commit_block(&state, &entry, &carrier);
    let peer = PeerId::new(
        checked_keypair_with_algorithm(Algorithm::BlsNormal)
            .public_key()
            .clone(),
    );
    state_block
        .commit_topology
        .mutate_vec(|topology| topology.push(peer));
    assert!(matches!(
        commit_staged_autonomous_for_test(state_block),
        Err(TransactionsBlockError::MergeAdmission)
    ));
    assert_eq!(
        state.committed_height(),
        autonomous_carrier_parent_height(&carrier),
    );
}
#[test]
fn autonomous_execution_commit_rejects_post_publication_event_surface_drift() {
    let (state, entry, carrier, _) = autonomous_merge_commit_authorization_fixture(false, false);
    let mut state_block = staged_autonomous_merge_commit_block(&state, &entry, &carrier);
    state_block.world.external_event_buf.push(
        BlockEvent {
            header: carrier.header(),
            status: BlockStatus::Applied,
        }
        .into(),
    );
    assert!(matches!(
        commit_staged_autonomous_for_test(state_block),
        Err(TransactionsBlockError::MergeAdmission)
    ));
    assert_eq!(
        state.committed_height(),
        autonomous_carrier_parent_height(&carrier),
    );
}
#[test]
fn autonomous_execution_defers_expired_axt_replay_pruning() {
    let (state, entry, carrier, expired_key) =
        autonomous_merge_commit_authorization_fixture(true, false);
    let expired_key = expired_key.expect("fixture expired replay key");
    commit_staged_autonomous_for_test(staged_autonomous_merge_commit_block(
        &state, &entry, &carrier,
    ))
    .expect("authorized execution carrier must not gain AXT pruning effects");
    assert!(
        state
            .world
            .axt_replay_ledger
            .view()
            .get(&expired_key)
            .is_some(),
        "expired replay guards must remain for a later non-execution carrier"
    );
}
#[test]
fn autonomous_execution_rejects_post_stage_axt_replay_drift() {
    let (state, entry, carrier, _) = autonomous_merge_commit_authorization_fixture(false, false);
    let mut state_block = staged_autonomous_merge_commit_block(&state, &entry, &carrier);
    let replay_key = AxtHandleReplayKey::from_parts(
        DataSpaceId::UNIVERSAL,
        axt_replay_incarnation_for_test(0xD2),
        [0xD2; 32],
        1,
        2,
        LaneId::SINGLE,
    );
    state_block
        .world
        .axt_replay_ledger
        .insert(replay_key, axt_replay_record_for_key(&replay_key, 0, 0));
    assert!(matches!(
        commit_staged_autonomous_for_test(state_block),
        Err(TransactionsBlockError::MergeAdmission)
    ));
    assert_eq!(
        state.committed_height(),
        autonomous_carrier_parent_height(&carrier),
    );
}
#[test]
fn autonomous_execution_stage_rejects_preexisting_axt_replay_overlay() {
    let (state, entry, carrier, _) = autonomous_merge_commit_authorization_fixture(false, false);
    let mut state_block = state.lane_application_block(carrier.header().clone());
    let replay_key = AxtHandleReplayKey::from_parts(
        DataSpaceId::UNIVERSAL,
        axt_replay_incarnation_for_test(0xD3),
        [0xD3; 32],
        1,
        3,
        LaneId::SINGLE,
    );
    state_block
        .world
        .axt_replay_ledger
        .insert(replay_key, axt_replay_record_for_key(&replay_key, 0, 0));
    assert!(matches!(
        state_block.stage_certified_merge_entry(&entry, ConsensusMode::Permissioned),
        Err(MergeLedgerCommitError::ExecutionStageNotPristine)
    ));
}
#[test]
fn autonomous_execution_pre_vote_rejects_due_start_of_block_effect() {
    let (state, entry, carrier, _) = autonomous_merge_commit_authorization_fixture(false, true);
    let mut state_block = state
        .block_with_certified_merge_entry(
            carrier.header().clone(),
            &entry,
            ConsensusMode::Permissioned,
        )
        .expect("certified execution stages before due block effects run");
    stage_exact_empty_autonomous_carrier_membership_for_pre_vote(&mut state_block);
    assert!(matches!(
        state_block.validate_staged_merge_execution_authorization(),
        Err(MergeLedgerCommitError::ExecutionDivergence(_))
            | Err(MergeLedgerCommitError::ExecutionBatchInvalid(_))
    ));
    assert!(
        state_block
            .world
            .governance_locks
            .get("autonomous-merge-due-start-effect")
            .is_none(),
        "the due start-of-block mutation must remove the now-empty indexed lock container"
    );
}
#[test]
fn autonomous_execution_pre_vote_requires_exact_empty_carrier_membership() {
    let (state, entry, carrier, _) = autonomous_merge_commit_authorization_fixture(false, false);
    let mut state_block = state
        .block_with_certified_merge_entry(
            carrier.header().clone(),
            &entry,
            ConsensusMode::Permissioned,
        )
        .expect("certified execution stages before the canonical carrier row");
    assert!(matches!(
        state_block.validate_staged_merge_execution_authorization(),
        Err(MergeLedgerCommitError::ExecutionBatchInvalid(message))
            if message.contains("exact-empty post-block/pre-vote")
    ));
}
#[test]
fn autonomous_execution_pre_vote_rejects_wrong_carrier_membership_height() {
    let (state, entry, carrier, _) = autonomous_merge_commit_authorization_fixture(false, false);
    let mut state_block = state
        .block_with_certified_merge_entry(
            carrier.header().clone(),
            &entry,
            ConsensusMode::Permissioned,
        )
        .expect("certified execution stages before the canonical carrier row");
    let wrong_height = NonZeroUsize::new(
        autonomous_carrier_transaction_height(&state_block)
            .get()
            .checked_add(1)
            .expect("fixture carrier height has a successor"),
    )
    .expect("a successor carrier height is non-zero");
    state_block
        .transactions
        .insert_block(std::collections::HashSet::new(), wrong_height);
    assert!(matches!(
        state_block.validate_staged_merge_execution_authorization(),
        Err(MergeLedgerCommitError::ExecutionBatchInvalid(message))
            if message.contains("exact-empty post-block/pre-vote")
    ));
}
#[test]
fn autonomous_execution_pre_vote_rejects_non_empty_carrier_membership() {
    let (state, entry, carrier, _) = autonomous_merge_commit_authorization_fixture(false, false);
    let mut state_block = state
        .block_with_certified_merge_entry(
            carrier.header().clone(),
            &entry,
            ConsensusMode::Permissioned,
        )
        .expect("certified execution stages before the canonical carrier row");
    let carrier_height = autonomous_carrier_transaction_height(&state_block);
    let unexpected_transaction = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(
        Hash::new(b"unexpected-autonomous-carrier-transaction"),
    );
    state_block
        .transactions
        .insert_block_with_single_tx(unexpected_transaction, carrier_height);
    assert!(matches!(
        state_block.validate_staged_merge_execution_authorization(),
        Err(MergeLedgerCommitError::ExecutionBatchInvalid(message))
            if message.contains("exact-empty post-block/pre-vote")
    ));
}
#[test]
fn autonomous_execution_pre_vote_rejects_premature_pending_carrier_hash() {
    let (state, entry, carrier, _) = autonomous_merge_commit_authorization_fixture(false, false);
    let mut state_block = state
        .block_with_certified_merge_entry(
            carrier.header().clone(),
            &entry,
            ConsensusMode::Permissioned,
        )
        .expect("certified execution stages before the canonical carrier row");
    stage_exact_empty_autonomous_carrier_membership_for_pre_vote(&mut state_block);
    state_block.block_hashes.push(carrier.hash());
    assert!(matches!(
        state_block.validate_staged_merge_execution_authorization(),
        Err(MergeLedgerCommitError::ExecutionBatchInvalid(message))
            if message.contains("exact-empty post-block/pre-vote")
    ));
}
#[test]
fn autonomous_execution_finality_rejects_unbound_event_surface_drift() {
    {
        let (state, entry, carrier, _) =
            autonomous_merge_commit_authorization_fixture(false, false);
        let mut state_block = state
            .block_with_certified_merge_entry(
                carrier.header().clone(),
                &entry,
                ConsensusMode::Permissioned,
            )
            .expect("certified autonomous execution must stage on its exact carrier");
        stage_exact_empty_autonomous_carrier_membership_for_pre_vote(&mut state_block);
        let expected_event = EventBox::from(BlockEvent {
            header: carrier.header(),
            status: BlockStatus::Approved,
        });
        let actual_event = EventBox::from(BlockEvent {
            header: carrier.header(),
            status: BlockStatus::Applied,
        });
        let authorization = state_block
            .canonical_wsv_merge_commit_authorization
            .as_mut()
            .expect("fixture authorization");
        authorization.external_event_count = 1;
        authorization.external_event_bytes = Some(vec![expected_event].encode());
        state_block.world.external_event_buf.push(actual_event);
        assert!(matches!(
            state_block.validate_staged_merge_execution_authorization(),
            Err(MergeLedgerCommitError::ExecutionDivergence(message))
                if message.contains("event prefix drifted before block admission")
        ));
    }
    let (state, entry, carrier, _) = autonomous_merge_commit_authorization_fixture(false, false);
    let mut state_block = state
        .block_with_certified_merge_entry(
            carrier.header().clone(),
            &entry,
            ConsensusMode::Permissioned,
        )
        .expect("certified autonomous execution must stage on its exact carrier");
    stage_exact_empty_autonomous_carrier_membership_for_pre_vote(&mut state_block);
    state_block
        .validate_staged_merge_execution_authorization()
        .expect("fixture reaches the exact post-block/pre-vote surface");
    state_block.world.external_event_buf.push(
        BlockEvent {
            header: carrier.header(),
            status: BlockStatus::Applied,
        }
        .into(),
    );
    let artifact = state
        .kura
        .v2_finality_artifact(carrier.header().height().get())
        .expect("read exact carrier finality")
        .expect("fixture persists exact carrier finality");
    let verified_artifact = crate::block::VerifiedV2FinalityArtifact::verify(artifact.clone())
        .expect("fixture finality verifies once");
    let committed = ValidBlock::new_unverified_for_tests(carrier.clone())
        .commit_with_verified_v2_artifact(
            verified_artifact,
            artifact.commit_qc.execution_commitment,
        )
        .unpack(|_| {})
        .expect("carrier binds its exact verified v2 finality");
    assert!(matches!(
        state_block.apply_without_execution_with_verified_v2_finality(&committed),
        Err(MergeLedgerCommitError::ExecutionDivergence(message))
            if message.contains("event surface drifted before publication")
    ));
    assert!(
        state_block
            .canonical_carrier_commit_metadata_authorization
            .is_none(),
        "an unbound event must not mint finalized carrier authorization"
    );
}
fn configured_two_lane_merge_state() -> (State, Vec<KeyPair>, Vec<KeyPair>, SignedBlock) {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::default(), Arc::clone(&kura), query);
    let lane_one = LaneConfig {
        id: LaneId::new(1),
        alias: "replaceable-lane".to_owned(),
        ..LaneConfig::default()
    };
    let lane_catalog = LaneCatalog::new(nonzero!(2_u32), vec![LaneConfig::default(), lane_one])
        .expect("two-lane merge fixture catalog");
    state
        .set_nexus(iroha_config::parameters::actual::Nexus {
            lane_catalog,
            ..iroha_config::parameters::actual::Nexus::default()
        })
        .expect("enable two-lane Nexus merge fixture");
    let (validator_ids, validator_keypairs) = bls_accounts_in("validators", 4);
    seed_consensus_keys_with_pops(&state, &validator_keypairs);
    install_lane_manifest_registry(
        &state,
        &[
            (
                LaneId::SINGLE,
                DataSpaceId::UNIVERSAL,
                validator_ids.clone(),
            ),
            (LaneId::new(1), DataSpaceId::UNIVERSAL, validator_ids),
        ],
    );
    let commit_keypairs = configure_commit_topology_preserving_world_peers(&state, 1);
    let parent = empty_global_block_after(None);
    kura.store_block(Arc::new(parent.clone()))
        .expect("store two-lane merge fixture carrier parent");
    commit_block_metadata_to_state(&state, &parent);
    let parent = advance_queue_plan_fixture_to_beacon_parent(&state, parent);
    (state, validator_keypairs, commit_keypairs, parent)
}
#[inline(never)]
fn assert_queue_plan_native_exact_compare_and_set() {
    let (state, validator_keypairs, _, parent) = configured_two_lane_merge_state();
    let routing_plan = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
    ));
    let (binding, certificate) = queue_plan_admission_certificate_for_state_test(
        &state,
        routing_plan,
        &validator_keypairs,
        queue_plan_authority_height_for_state_test(&state),
        0x59,
    );
    assert_eq!(
        state
            .queue_plan_admission_binding_registry_match(&binding)
            .expect("absent registry lookup"),
        QueuePlanAdmissionRegistryMatch::Absent
    );
    let active_lanes = state
        .queue_plan_active_lane_bindings()
        .expect("resolve proposal-native QueuePlan lane bindings");
    let carrier = empty_global_block_after(Some(&parent));
    let write_set_before = state
        .lane_application_block(carrier.header().clone())
        .merge_execution_write_set_root();
    let mut state_block = state
        .block_with_queue_plan_admissions(carrier.header().clone(), &[certificate.clone()])
        .expect("proposal-native admission inserts an absent registry marker");
    assert_eq!(
        state_block.staged_queue_plan_admissions(),
        core::slice::from_ref(&certificate),
        "the carrier must retain the exact proposal-native certificate bytes"
    );
    let write_set_after_insert = state_block.merge_execution_write_set_root();
    assert_ne!(
        write_set_after_insert, write_set_before,
        "QueuePlan registry writes must enter the signed final WSV write-set commitment"
    );
    state_block
        .stage_queue_plan_admissions(
            &[certificate.clone()],
            &active_lanes,
            carrier.header().height().get(),
        )
        .expect("the exact marker is idempotent");
    assert_eq!(
        state_block.merge_execution_write_set_root(),
        write_set_after_insert,
        "idempotent exact CAS replay must not change the committed write set"
    );
    let key = State::queue_plan_admission_registry_marker_key(&binding.registry_key())
        .expect("fixture registry key");
    let expected_payload =
        State::queue_plan_admission_registry_marker_payload(&binding.registry_value())
            .expect("fixture registry value");
    assert_eq!(
        state_block.world.smart_contract_state.get(&key),
        Some(&expected_payload)
    );
    // `StateBlock` owns exclusive MV storage transactions. Release this
    // overlay before opening an independent one for the conflict case.
    drop(state_block);
    let conflicting_value = crate::torii_proxy::QueuePlanAdmissionRegistryValueV1 {
        version: crate::torii_proxy::QUEUE_PLAN_ADMISSION_BINDING_VERSION_V1,
        binding_hash: Hash::new(b"conflicting-cas-value"),
    };
    {
        let mut world = state.world.block();
        world.smart_contract_state.insert(
            key,
            State::queue_plan_admission_registry_marker_payload(&conflicting_value)
                .expect("well-formed conflicting registry value"),
        );
        world.commit();
    }
    assert!(matches!(
        state.block_with_queue_plan_admissions(carrier.header().clone(), &[certificate]),
        Err(MergeLedgerCommitError::ExecutionMarkerConflict(_))
    ));
}
#[inline(never)]
fn assert_queue_plan_native_multi_route_preflight_is_atomic() {
    let (state, validator_keypairs, _, _) = configured_two_lane_merge_state();
    let participant_lane = LaneId::new(1);
    let routing_plan = crate::queue::RoutingPlan::native_amx(
        crate::queue::RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
        vec![crate::queue::RouteLeg::new(
            crate::queue::RoutingDecision::new(participant_lane, DataSpaceId::UNIVERSAL),
            crate::queue::RouteLegRole::Participant,
        )],
    );
    let (_, certificate) = queue_plan_admission_certificate_for_state_test(
        &state,
        routing_plan,
        &validator_keypairs,
        queue_plan_authority_height_for_state_test(&state),
        0x79,
    );
    let admission = crate::torii_proxy::decode_and_validate_queue_plan_admission_certificate_v1(
        &state.network_id,
        &certificate,
    )
    .expect("fixture multi-route QueuePlan admission");
    let obligation = State::queue_plan_pending_obligation_from_admission(&admission)
        .expect("fixture multi-route pending obligation");
    let coordinator_route = obligation.routes[0];
    let participant_route = *obligation
        .routes
        .iter()
        .find(|route| route.lane_id == participant_lane)
        .expect("fixture later participant route");
    let participant_member =
        State::queue_plan_pending_route_member_from_obligation(&obligation, participant_route)
            .expect("fixture participant member");
    let participant_member_key = State::queue_plan_pending_route_member_marker_key(
        participant_route,
        participant_member.member_identity,
    )
    .expect("fixture orphan participant member key");
    let participant_member_payload =
        State::queue_plan_pending_route_member_marker_payload(&participant_member)
            .expect("fixture orphan participant member payload");
    {
        let mut world = state.world.block();
        world.smart_contract_state.insert(
            State::queue_plan_admission_registry_marker_key(&admission.registry_key)
                .expect("fixture multi-route registry key"),
            State::queue_plan_admission_registry_marker_payload(&admission.registry_value)
                .expect("fixture multi-route registry value"),
        );
        world.smart_contract_state.insert(
            participant_member_key.clone(),
            participant_member_payload.clone(),
        );
        world.commit();
    }
    let obligation_key = State::queue_plan_pending_obligation_marker_key(
        obligation.network_id_digest,
        obligation.entrypoint_hash.clone(),
    )
    .expect("fixture multi-route obligation key");
    let coordinator_member_key = State::queue_plan_pending_route_member_marker_key(
        coordinator_route,
        State::queue_plan_pending_route_member_identity(&obligation, coordinator_route)
            .expect("fixture coordinator member identity"),
    )
    .expect("fixture coordinator member key");
    let mut world = state.world.block();
    assert!(
        State::stage_queue_plan_pending_obligation_in_storage(
            &mut world.smart_contract_state,
            &admission,
        )
        .is_err(),
        "a later-route orphan member must abort the whole obligation stage"
    );
    assert!(world.smart_contract_state.get(&obligation_key).is_none());
    assert!(
        world
            .smart_contract_state
            .get(&coordinator_member_key)
            .is_none()
    );
    assert_eq!(
        world.smart_contract_state.get(&participant_member_key),
        Some(&participant_member_payload),
        "failed stage preflight must preserve the orphan marker for diagnosis"
    );
    drop(world);
}
#[inline(never)]
#[expect(
    clippy::too_many_lines,
    reason = "the whole-list rollback proof checks every registry, obligation, and route-member marker"
)]
fn assert_queue_plan_native_batch_rollback_is_atomic() {
    let (state, validator_keypairs, _, parent) = configured_two_lane_merge_state();
    let first_plan = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
    ));
    let second_plan = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
        LaneId::new(1),
        DataSpaceId::UNIVERSAL,
    ));
    let (_, first_certificate) = queue_plan_admission_certificate_for_state_test(
        &state,
        first_plan,
        &validator_keypairs,
        queue_plan_authority_height_for_state_test(&state),
        0x79,
    );
    let (_, second_certificate) = queue_plan_admission_certificate_for_state_test(
        &state,
        second_plan,
        &validator_keypairs,
        queue_plan_authority_height_for_state_test(&state),
        0x7A,
    );
    let first_registry_key_for_order =
        crate::torii_proxy::decode_and_validate_queue_plan_admission_certificate_v1(
            &state.network_id,
            &first_certificate,
        )
        .expect("fixture first batch admission")
        .registry_key;
    let second_registry_key_for_order =
        crate::torii_proxy::decode_and_validate_queue_plan_admission_certificate_v1(
            &state.network_id,
            &second_certificate,
        )
        .expect("fixture second batch admission")
        .registry_key;
    let mut ordered_admissions = [
        (first_registry_key_for_order, first_certificate),
        (second_registry_key_for_order, second_certificate),
    ];
    ordered_admissions.sort_by(|(left, _), (right, _)| left.cmp(right));
    let ordered_certificates = ordered_admissions
        .into_iter()
        .map(|(_, certificate)| certificate)
        .collect::<Vec<_>>();
    let first_admission =
        crate::torii_proxy::decode_and_validate_queue_plan_admission_certificate_v1(
            &state.network_id,
            &ordered_certificates[0],
        )
        .expect("fixture first canonical batch admission");
    let second_admission =
        crate::torii_proxy::decode_and_validate_queue_plan_admission_certificate_v1(
            &state.network_id,
            &ordered_certificates[1],
        )
        .expect("fixture second canonical batch admission");
    let first_obligation = State::queue_plan_pending_obligation_from_admission(&first_admission)
        .expect("fixture first canonical batch obligation");
    let second_obligation = State::queue_plan_pending_obligation_from_admission(&second_admission)
        .expect("fixture second canonical batch obligation");
    assert_ne!(
        first_obligation.routes[0].lane_id, second_obligation.routes[0].lane_id,
        "canonical batch fixtures must use disjoint route rosters"
    );
    let first_registry_key =
        State::queue_plan_admission_registry_marker_key(&first_admission.registry_key)
            .expect("fixture first batch registry key");
    let second_registry_key =
        State::queue_plan_admission_registry_marker_key(&second_admission.registry_key)
            .expect("fixture second batch registry key");
    let first_obligation_key = State::queue_plan_pending_obligation_marker_key(
        first_obligation.network_id_digest,
        first_obligation.entrypoint_hash.clone(),
    )
    .expect("fixture first batch obligation key");
    let second_obligation_key = State::queue_plan_pending_obligation_marker_key(
        second_obligation.network_id_digest,
        second_obligation.entrypoint_hash.clone(),
    )
    .expect("fixture second batch obligation key");
    let first_member = State::queue_plan_pending_route_member_from_obligation(
        &first_obligation,
        first_obligation.routes[0],
    )
    .expect("fixture first batch member");
    let second_member = State::queue_plan_pending_route_member_from_obligation(
        &second_obligation,
        second_obligation.routes[0],
    )
    .expect("fixture second batch member");
    let first_member_key = State::queue_plan_pending_route_member_marker_key(
        first_member.route,
        first_member.member_identity,
    )
    .expect("fixture first batch member key");
    let second_member_key = State::queue_plan_pending_route_member_marker_key(
        second_member.route,
        second_member.member_identity,
    )
    .expect("fixture second batch member key");
    let second_member_payload =
        State::queue_plan_pending_route_member_marker_payload(&second_member)
            .expect("fixture second batch orphan member payload");
    let carrier = empty_global_block_after(Some(&parent));
    {
        let mut world = state.world.block();
        world
            .smart_contract_state
            .insert(second_member_key.clone(), second_member_payload.clone());
        world.commit();
    }
    let mut state_block = state.lane_application_block(carrier.header().clone());
    let write_set_before_batch = state_block.merge_execution_write_set_root();
    assert!(
        state_block
            .stage_queue_plan_admissions_for_carrier(&ordered_certificates)
            .is_err(),
        "a second admission failure must roll back every earlier admission"
    );
    assert_eq!(
        state_block.merge_execution_write_set_root(),
        write_set_before_batch,
        "failed whole-list staging must restore the exact prior overlay"
    );
    for key in [
        &first_registry_key,
        &first_obligation_key,
        &first_member_key,
        &second_registry_key,
        &second_obligation_key,
    ] {
        assert!(
            state_block.world.smart_contract_state.get(key).is_none(),
            "failed whole-list staging leaked marker `{key}`"
        );
    }
    assert_eq!(
        state_block
            .world
            .smart_contract_state
            .get(&second_member_key),
        Some(&second_member_payload),
        "rollback must preserve the pre-existing orphan evidence"
    );
    assert!(state_block.staged_queue_plan_admissions().is_empty());
    drop(state_block);
    {
        let mut world = state.world.block();
        world.smart_contract_state.remove(second_member_key);
        world.commit();
    }
    let retry_block = state
        .block_with_queue_plan_admissions(carrier.header().clone(), &ordered_certificates[..1])
        .expect("a later proposal-native carrier can retry after the conflict is repaired");
    for key in [
        &first_registry_key,
        &first_obligation_key,
        &first_member_key,
    ] {
        assert!(retry_block.world.smart_contract_state.get(key).is_some());
    }
}
#[test]
fn queue_plan_native_staging_is_an_exact_idempotent_compare_and_set() {
    assert_queue_plan_native_exact_compare_and_set();
    assert_queue_plan_native_multi_route_preflight_is_atomic();
    assert_queue_plan_native_batch_rollback_is_atomic();
}
#[test]
fn queue_plan_registry_absence_rejects_an_orphan_pending_obligation() {
    let (state, validator_keypairs, _, _) = configured_two_lane_merge_state();
    let routing_plan = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
    ));
    let (binding, certificate) = queue_plan_admission_certificate_for_state_test(
        &state,
        routing_plan,
        &validator_keypairs,
        queue_plan_authority_height_for_state_test(&state),
        0x5A,
    );
    seed_exact_queue_plan_admission_state_for_test(&state, &certificate);
    let registry_key = State::queue_plan_admission_registry_marker_key(&binding.registry_key())
        .expect("fixture registry key");
    {
        let mut world = state.world.block();
        world.smart_contract_state.remove(registry_key);
        world.commit();
    }
    assert!(
        state
            .queue_plan_admission_binding_registry_match(&binding)
            .is_err(),
        "registry absence must not hide an orphan pending obligation"
    );
    assert!(
        queue_plan_admission_registry_match(
            &state.view(),
            binding.entrypoint_hash,
            binding.canonical_hash(),
        )
        .is_err(),
        "Queue selection must fail closed on an orphan pending obligation"
    );
    assert!(
        state
            .pending_queue_plan_admission_registry_lookup(&certificate)
            .is_err(),
        "durable pending-certificate recovery must reject the same orphan state"
    );
}
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one registry-owner test covers absent, pending, External-applied, and sealed-reveal-applied evidence"
)]
fn queue_plan_conflict_requires_pending_or_applied_owner_evidence() {
    let (state, validator_keypairs, _, _) = configured_two_lane_merge_state();
    let routing_plan = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
    ));
    let tag = 0x5B;
    let (binding, certificate) = queue_plan_admission_certificate_for_state_test(
        &state,
        routing_plan.clone(),
        &validator_keypairs,
        queue_plan_authority_height_for_state_test(&state),
        tag,
    );
    let registry_key = State::queue_plan_admission_registry_marker_key(&binding.registry_key())
        .expect("fixture registry key");
    let partial_conflict = crate::torii_proxy::QueuePlanAdmissionRegistryValueV1 {
        version: crate::torii_proxy::QUEUE_PLAN_ADMISSION_BINDING_VERSION_V1,
        binding_hash: Hash::new(b"partial-conflicting-queue-plan-owner"),
    };
    {
        let mut world = state.world.block();
        world.smart_contract_state.insert(
            registry_key,
            State::queue_plan_admission_registry_marker_payload(&partial_conflict)
                .expect("fixture partial conflict payload"),
        );
        world.commit();
    }
    assert!(
        state
            .queue_plan_admission_binding_registry_match(&binding)
            .is_err(),
        "a conflicting hash without owner evidence is corruption, not a definitive conflict"
    );
    let conflicting_entrypoint = queue_plan_entrypoint_for_state_test(&state, tag);
    let conflicting_binding = crate::torii_proxy::QueuePlanAdmissionBindingV1::new(
        &state.network_id,
        &conflicting_entrypoint,
        &routing_plan,
        binding.admission_context.clone(),
        binding.enqueue_timestamp_ms.saturating_add(1),
    )
    .expect("fixture coherent conflicting binding");
    assert_eq!(
        conflicting_binding.entrypoint_hash, binding.entrypoint_hash,
        "conflicting owner must target the same immutable registry key"
    );
    assert_ne!(
        conflicting_binding.canonical_hash(),
        binding.canonical_hash(),
        "fixture conflict must retain a distinct full binding"
    );
    seed_pending_queue_plan_binding_state_for_test(&state, &conflicting_binding);
    assert_eq!(
        state
            .queue_plan_admission_binding_registry_match(&binding)
            .expect("coherent pending conflict"),
        QueuePlanAdmissionRegistryMatch::Conflict
    );
    assert_eq!(
        queue_plan_admission_registry_match(
            &state.view(),
            binding.entrypoint_hash,
            binding.canonical_hash(),
        )
        .expect("Queue selection classifies a coherent pending conflict"),
        QueuePlanAdmissionRegistryMatch::Conflict
    );
    assert_eq!(
        state
            .pending_queue_plan_admission_registry_lookup(&certificate)
            .expect("durable lookup classifies a coherent pending conflict")
            .1,
        QueuePlanAdmissionRegistryMatch::Conflict
    );
    {
        let mut world = state.world.block();
        assert!(
            State::resolve_queue_plan_pending_obligation_in_storage(
                &mut world.smart_contract_state,
                conflicting_binding.network_id_digest,
                conflicting_binding.entrypoint_hash,
            )
            .expect("resolve coherent conflicting obligation")
        );
        world.commit();
    }
    let committed_memberships =
        committed_entrypoint_hashes(core::slice::from_ref(&conflicting_entrypoint));
    let TransactionEntrypoint::External(conflicting_transaction) = &conflicting_entrypoint else {
        unreachable!("QueuePlan fixture entrypoint is External")
    };
    assert_eq!(
        conflicting_binding.signed_transaction_hash,
        Some(conflicting_transaction.hash()),
        "the binding must retain the exact External signed identity"
    );
    assert_eq!(
        committed_memberships,
        vec![conflicting_transaction.hash_as_entrypoint()]
    );
    assert_eq!(
        committed_memberships,
        vec![conflicting_binding.entrypoint_hash],
        "the all-External canonical commit path must preserve the typed entrypoint hash bytes"
    );
    state.record_direct_committed_entrypoints(
        committed_memberships,
        NonZeroUsize::new(
            usize::try_from(queue_plan_proposal_height_for_state_test(&state))
                .expect("fixture applied height fits usize"),
        )
        .expect("fixture applied height is non-zero"),
    );
    assert_eq!(
        state
            .queue_plan_admission_binding_registry_match(&binding)
            .expect("coherent applied conflict"),
        QueuePlanAdmissionRegistryMatch::Conflict
    );
    assert_eq!(
        queue_plan_admission_registry_match(
            &state.view(),
            binding.entrypoint_hash,
            binding.canonical_hash(),
        )
        .expect("Queue selection classifies a coherent applied conflict"),
        QueuePlanAdmissionRegistryMatch::Conflict
    );
    {
        let mut world = state.world.block();
        world.smart_contract_state.insert(
            State::queue_plan_admission_registry_marker_key(&binding.registry_key())
                .expect("fixture exact applied registry key"),
            State::queue_plan_admission_registry_marker_payload(&binding.registry_value())
                .expect("fixture exact applied registry value"),
        );
        world.commit();
    }
    assert_eq!(
        state
            .queue_plan_admission_binding_registry_match(&binding)
            .expect("read-only clients may acknowledge an exact applied owner"),
        QueuePlanAdmissionRegistryMatch::Exact
    );
    assert!(
        queue_plan_admission_registry_match(
            &state.view(),
            binding.entrypoint_hash,
            binding.canonical_hash(),
        )
        .is_err(),
        "Queue selection must not reacquire an exact owner after canonical application"
    );
    let sealed_tag = tag.saturating_add(1);
    let sealed_template_entrypoint = queue_plan_entrypoint_for_state_test(&state, sealed_tag);
    let TransactionEntrypoint::External(sealed_transaction) = sealed_template_entrypoint else {
        unreachable!("QueuePlan fixture entrypoint is External")
    };
    let salt = [0xA5; 32];
    let reveal_deadline_height = 64;
    let commitment = iroha_data_model::transaction::signed::compute_sealed_transaction_commitment(
        state.network_id_ref(),
        &sealed_transaction,
        salt,
        reveal_deadline_height,
    );
    let sealed_entrypoint = TransactionEntrypoint::SealedReveal(
        iroha_data_model::transaction::signed::SealedTransactionReveal::new(
            commitment,
            sealed_transaction,
            salt,
        ),
    );
    let (sealed_template_binding, _) = queue_plan_admission_certificate_for_state_test(
        &state,
        routing_plan.clone(),
        &validator_keypairs,
        queue_plan_authority_height_for_state_test(&state),
        sealed_tag,
    );
    let sealed_binding = crate::torii_proxy::QueuePlanAdmissionBindingV1::new(
        &state.network_id,
        &sealed_entrypoint,
        &routing_plan,
        sealed_template_binding.admission_context,
        sealed_template_binding.enqueue_timestamp_ms,
    )
    .expect("canonical sealed-reveal QueuePlan binding");
    let underlying_signed_hash = sealed_binding
        .signed_transaction_hash
        .expect("sealed reveal retains its underlying signed identity");
    assert_ne!(
        Hash::from(underlying_signed_hash),
        Hash::from(sealed_binding.entrypoint_hash),
        "sealed-reveal membership must not collapse to its underlying signed transaction"
    );
    let TransactionEntrypoint::SealedReveal(reveal) = &sealed_entrypoint else {
        unreachable!("fixture entrypoint is a sealed reveal")
    };
    assert_eq!(
        sealed_binding.signed_transaction_hash,
        Some(reveal.signed_transaction().hash()),
        "the binding must retain the sealed reveal's underlying signed identity"
    );
    seed_pending_queue_plan_binding_state_for_test(&state, &sealed_binding);
    {
        let mut world = state.world.block();
        assert!(
            State::resolve_queue_plan_pending_obligation_in_storage(
                &mut world.smart_contract_state,
                sealed_binding.network_id_digest,
                sealed_binding.entrypoint_hash,
            )
            .expect("resolve sealed-reveal pending obligation")
        );
        world.commit();
    }
    let sealed_memberships = committed_entrypoint_hashes(core::slice::from_ref(&sealed_entrypoint));
    assert_eq!(
        sealed_memberships,
        vec![sealed_binding.entrypoint_hash],
        "the heterogeneous canonical commit path must use the sealed-reveal entrypoint identity"
    );
    let mixed_memberships =
        committed_entrypoint_hashes(&[conflicting_entrypoint.clone(), sealed_entrypoint.clone()]);
    assert_eq!(
        mixed_memberships,
        vec![binding.entrypoint_hash, sealed_binding.entrypoint_hash,],
        "a mixed batch must commit both entries through their ordered typed identities"
    );
    assert_eq!(
        mixed_memberships[0],
        conflicting_transaction.hash_as_entrypoint(),
        "the External typed identity must remain byte-compatible inside a mixed batch"
    );
    state.record_direct_committed_entrypoints(
        sealed_memberships,
        NonZeroUsize::new(
            usize::try_from(
                queue_plan_proposal_height_for_state_test(&state)
                    .checked_add(1)
                    .expect("fixture sealed applied height"),
            )
            .expect("fixture sealed applied height fits usize"),
        )
        .expect("fixture sealed applied height is non-zero"),
    );
    assert_eq!(
        state
            .queue_plan_admission_binding_registry_match(&sealed_binding)
            .expect("sealed-reveal applied owner evidence"),
        QueuePlanAdmissionRegistryMatch::Exact
    );
    assert!(
        queue_plan_admission_registry_match(
            &state.view(),
            sealed_binding.entrypoint_hash,
            sealed_binding.canonical_hash(),
        )
        .is_err(),
        "Queue selection must not reacquire an applied sealed-reveal owner"
    );
}
#[test]
fn queue_plan_native_pending_obligations_count_all_unique_routes_and_block_drain() {
    let (state, validator_keypairs, _, _) = configured_two_lane_merge_state();
    let participant_lane = LaneId::new(1);
    let routing_plan = crate::queue::RoutingPlan::native_amx(
        crate::queue::RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
        vec![crate::queue::RouteLeg::new(
            crate::queue::RoutingDecision::new(participant_lane, DataSpaceId::UNIVERSAL),
            crate::queue::RouteLegRole::Participant,
        )],
    );
    let (_, certificate) = queue_plan_admission_certificate_for_state_test(
        &state,
        routing_plan,
        &validator_keypairs,
        queue_plan_authority_height_for_state_test(&state),
        0x6A,
    );
    let obligation = queue_plan_pending_obligation_for_test(&state, &certificate);
    assert_eq!(
        obligation.routes.len(),
        2,
        "the pending obligation must retain coordinator and participant routes"
    );
    seed_exact_queue_plan_admission_state_for_test(&state, &certificate);
    let world = state.world.view();
    for route in &obligation.routes {
        assert_eq!(
            State::queue_plan_pending_route_obligation_count_from_world(&world, *route)
                .expect("exact route count"),
            1,
            "each unique bound route receives one pending obligation"
        );
        let member_identity = State::queue_plan_pending_route_member_identity(&obligation, *route)
            .expect("exact bound-route member identity");
        let member_key = State::queue_plan_pending_route_member_marker_key(*route, member_identity)
            .expect("exact bound-route member key");
        let member = State::decode_exact_queue_plan_pending_route_member_marker(
            &member_key,
            world
                .smart_contract_state()
                .get(&member_key)
                .expect("exact bound-route member payload"),
        )
        .expect("exact bound-route member marker");
        assert_eq!(
            member,
            State::queue_plan_pending_route_member_from_obligation(&obligation, *route)
                .expect("exact bound-route member claim"),
        );
    }
    drop(world);
    for route in obligation.routes {
        assert!(
            state.lane_has_drain_blocking_evidence(
                route.lane_id,
                route.dataspace_id,
                route.lane_incarnation,
            ),
            "each exact coordinator or participant incarnation must remain drain-blocked"
        );
    }
}
#[test]
fn execution_routing_reads_only_the_exact_live_pending_queue_plan_binding() {
    let (state, validator_keypairs, _, _) = configured_two_lane_merge_state();
    let participant_lane = LaneId::new(1);
    let routing_plan = crate::queue::RoutingPlan::native_amx(
        crate::queue::RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
        vec![crate::queue::RouteLeg::new(
            crate::queue::RoutingDecision::new(participant_lane, DataSpaceId::UNIVERSAL),
            crate::queue::RouteLegRole::Participant,
        )],
    );
    let tag = 0x6F;
    let (binding, certificate) = queue_plan_admission_certificate_for_state_test(
        &state,
        routing_plan.clone(),
        &validator_keypairs,
        queue_plan_authority_height_for_state_test(&state),
        tag,
    );
    let entrypoint = queue_plan_entrypoint_for_state_test(&state, tag);
    assert!(
        State::pending_queue_plan_binding_for_execution(
            &state.view(),
            &entrypoint,
            &routing_plan,
            queue_plan_proposal_height_for_state_test(&state),
        )
        .expect("absent execution binding lookup")
        .is_none()
    );

    seed_exact_queue_plan_admission_state_for_test(&state, &certificate);
    assert_eq!(
        State::pending_queue_plan_binding_for_execution(
            &state.view(),
            &entrypoint,
            &routing_plan,
            queue_plan_proposal_height_for_state_test(&state),
        )
        .expect("exact execution binding lookup"),
        Some(binding)
    );

    let substituted_plan = crate::queue::RoutingPlan::native_amx(
        routing_plan.coordinator_route(),
        vec![crate::queue::RouteLeg::new(
            crate::queue::RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            crate::queue::RouteLegRole::Participant,
        )],
    );
    assert!(
        State::pending_queue_plan_binding_for_execution(
            &state.view(),
            &entrypoint,
            &substituted_plan,
            queue_plan_proposal_height_for_state_test(&state),
        )
        .is_err(),
        "the registry hash must not authenticate a substituted same-topology plan"
    );

    state.lane_incarnations.write().insert(
        participant_lane,
        Hash::new(b"recreated QueuePlan participant lane"),
    );
    assert!(
        State::pending_queue_plan_binding_for_execution(
            &state.view(),
            &entrypoint,
            &routing_plan,
            queue_plan_proposal_height_for_state_test(&state),
        )
        .is_err(),
        "an old binding must not authorize a recreated lane incarnation"
    );
}
fn native_lane_drift_reconciliation_fixture(
    seed_binding: bool,
) -> (
    State,
    TransactionEntrypoint,
    crate::queue::RoutingPlan,
    crate::queue::RoutingPlan,
) {
    let (state, validator_keypairs, _, _) = configured_two_lane_merge_state();
    let alternate_lane = LaneId::new(1);
    let committed_plan = crate::queue::RoutingPlan::native_amx(
        crate::queue::RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
        vec![crate::queue::RouteLeg::new(
            crate::queue::RoutingDecision::new(alternate_lane, DataSpaceId::UNIVERSAL),
            crate::queue::RouteLegRole::Participant,
        )],
    );
    let fresh_plan = crate::queue::RoutingPlan::native_amx(
        crate::queue::RoutingDecision::new(alternate_lane, DataSpaceId::UNIVERSAL),
        vec![crate::queue::RouteLeg::new(
            crate::queue::RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            crate::queue::RouteLegRole::Participant,
        )],
    );
    let tag = 0x70;
    let (_, certificate) = queue_plan_admission_certificate_for_state_test(
        &state,
        committed_plan.clone(),
        &validator_keypairs,
        queue_plan_authority_height_for_state_test(&state),
        tag,
    );
    if seed_binding {
        seed_exact_queue_plan_admission_state_for_test(&state, &certificate);
    }
    let entrypoint = queue_plan_entrypoint_for_state_test(&state, tag);
    (state, entrypoint, committed_plan, fresh_plan)
}
#[test]
fn block_execution_routing_preserves_authenticated_native_lane_choice() {
    let (state, entrypoint, committed_plan, fresh_plan) =
        native_lane_drift_reconciliation_fixture(true);
    assert_eq!(
        crate::queue::reconcile_committed_routing_plan_with_fresh_plan(
            &entrypoint,
            &committed_plan,
            &fresh_plan,
            &state.view(),
            queue_plan_proposal_height_for_state_test(&state),
        )
        .expect("the exact pending binding authenticates lane-only Native-AMX drift"),
        committed_plan,
    );
}
#[test]
fn block_execution_routing_rejects_lane_drift_without_pending_binding() {
    let (state, entrypoint, committed_plan, fresh_plan) =
        native_lane_drift_reconciliation_fixture(false);
    assert!(matches!(
        crate::queue::reconcile_committed_routing_plan_with_fresh_plan(
            &entrypoint,
            &committed_plan,
            &fresh_plan,
            &state.view(),
            queue_plan_proposal_height_for_state_test(&state),
        ),
        Err(crate::queue::ExecutionRoutingReconciliationError::MissingPendingAdmission)
    ));
}
#[test]
fn block_execution_routing_rejects_topology_drift_with_pending_binding() {
    let (state, entrypoint, committed_plan, _) = native_lane_drift_reconciliation_fixture(true);
    let fresh_plan = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
    ));
    assert!(matches!(
        crate::queue::reconcile_committed_routing_plan_with_fresh_plan(
            &entrypoint,
            &committed_plan,
            &fresh_plan,
            &state.view(),
            queue_plan_proposal_height_for_state_test(&state),
        ),
        Err(crate::queue::ExecutionRoutingReconciliationError::TopologyMismatch)
    ));
}
#[test]
fn block_execution_routing_rejects_admitted_lane_incarnation_aba() {
    let (state, entrypoint, committed_plan, fresh_plan) =
        native_lane_drift_reconciliation_fixture(true);
    state.lane_incarnations.write().insert(
        LaneId::new(1),
        Hash::new(b"recreated execution-routing lane"),
    );
    assert!(matches!(
        crate::queue::reconcile_committed_routing_plan_with_fresh_plan(
            &entrypoint,
            &committed_plan,
            &fresh_plan,
            &state.view(),
            queue_plan_proposal_height_for_state_test(&state),
        ),
        Err(crate::queue::ExecutionRoutingReconciliationError::InvalidPendingAdmission(_))
    ));
}
#[test]
fn queue_plan_same_route_roles_share_one_pending_route_counter() {
    let (state, validator_keypairs, _, _) = configured_two_lane_merge_state();
    let route = crate::queue::RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let routing_plan = crate::queue::RoutingPlan::native_amx(
        route,
        vec![crate::queue::RouteLeg::new(
            route,
            crate::queue::RouteLegRole::Participant,
        )],
    );
    let (binding, certificate) = queue_plan_admission_certificate_for_state_test(
        &state,
        routing_plan,
        &validator_keypairs,
        queue_plan_authority_height_for_state_test(&state),
        0x6B,
    );
    assert_eq!(
        binding.admission_context.route_incarnations.len(),
        2,
        "the fixture must bind distinct coordinator and participant roles"
    );
    let obligation = queue_plan_pending_obligation_for_test(&state, &certificate);
    assert_eq!(
        obligation.routes.len(),
        1,
        "same-route coordinator and participant roles must deduplicate by route incarnation"
    );
    seed_exact_queue_plan_admission_state_for_test(&state, &certificate);
    let route = obligation.routes[0];
    assert_eq!(
        State::queue_plan_pending_route_obligation_count_from_world(&state.world.view(), route,)
            .expect("same-route pending count"),
        1,
        "same-route roles must not create two drain obligations"
    );
    let world = state.world.view();
    let members =
        State::queue_plan_pending_route_members_from_storage(world.smart_contract_state(), route)
            .expect("same-route exact member roster");
    assert_eq!(
        members.len(),
        1,
        "same-route roles must contribute one member"
    );
    let member_identity = State::queue_plan_pending_route_member_identity(&obligation, route)
        .expect("same-route member identity");
    let member_key = State::queue_plan_pending_route_member_marker_key(route, member_identity)
        .expect("same-route exact member key");
    let member = State::decode_exact_queue_plan_pending_route_member_marker(
        &member_key,
        world
            .smart_contract_state()
            .get(&member_key)
            .expect("same-route exact member payload"),
    )
    .expect("same-route exact member marker");
    assert_eq!(member.route, route);
    assert_eq!(member.member_identity, member_identity);
    assert_eq!(members[0], (member_key, member));
    drop(world);
    assert!(state.lane_has_drain_blocking_evidence(
        route.lane_id,
        route.dataspace_id,
        route.lane_incarnation,
    ));
}
#[test]
fn queue_plan_pending_obligation_authenticates_copies_before_counter_mutation() {
    #[derive(Clone, Copy)]
    enum Tamper {
        CopiedRoute,
        CopiedSignedIdentity,
        FullBinding,
    }
    let (state, validator_keypairs, _, _) = configured_two_lane_merge_state();
    let routing_plan = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
    ));
    let tag = 0x6C;
    let (binding, certificate) = queue_plan_admission_certificate_for_state_test(
        &state,
        routing_plan.clone(),
        &validator_keypairs,
        queue_plan_authority_height_for_state_test(&state),
        tag,
    );
    let admission = crate::torii_proxy::decode_and_validate_queue_plan_admission_certificate_v1(
        &state.network_id,
        &certificate,
    )
    .expect("fixture QueuePlan admission certificate");
    let original = State::queue_plan_pending_obligation_from_admission(&admission)
        .expect("fixture authenticated pending obligation");
    seed_exact_queue_plan_admission_state_for_test(&state, &certificate);
    let alternate_binding = crate::torii_proxy::QueuePlanAdmissionBindingV1::new(
        &state.network_id,
        &queue_plan_entrypoint_for_state_test(&state, tag),
        &routing_plan,
        binding.admission_context.clone(),
        binding.enqueue_timestamp_ms.saturating_add(1),
    )
    .expect("well-formed alternate QueuePlan binding");
    alternate_binding
        .validate_structure()
        .expect("alternate QueuePlan binding is structurally valid");
    assert_eq!(
        alternate_binding.network_id_digest,
        binding.network_id_digest
    );
    assert_eq!(alternate_binding.entrypoint_hash, binding.entrypoint_hash);
    assert_eq!(
        alternate_binding.signed_transaction_hash,
        binding.signed_transaction_hash
    );
    assert_ne!(alternate_binding.canonical_hash(), binding.canonical_hash());
    let obligation_key = State::queue_plan_pending_obligation_marker_key(
        binding.network_id_digest,
        binding.entrypoint_hash.clone(),
    )
    .expect("fixture pending-obligation key");
    let member_keys = original
        .routes
        .iter()
        .copied()
        .map(|route| {
            let member_identity = State::queue_plan_pending_route_member_identity(&original, route)
                .expect("fixture route-member identity");
            State::queue_plan_pending_route_member_marker_key(route, member_identity)
                .expect("fixture route-member key")
        })
        .collect::<Vec<_>>();
    let member_payloads = {
        let world = state.world.view();
        member_keys
            .iter()
            .map(|key| {
                world
                    .smart_contract_state()
                    .get(key)
                    .cloned()
                    .expect("fixture route-member payload")
            })
            .collect::<Vec<_>>()
    };
    for tamper in [
        Tamper::CopiedRoute,
        Tamper::CopiedSignedIdentity,
        Tamper::FullBinding,
    ] {
        let mut tampered = original.clone();
        match tamper {
            Tamper::CopiedRoute => {
                tampered.routes[0].lane_incarnation =
                    Hash::new(b"substituted-pending-obligation-route");
            }
            Tamper::CopiedSignedIdentity => {
                tampered.signed_transaction_hash =
                    Some(HashOf::<SignedTransaction>::from_untyped_unchecked(
                        Hash::new(b"substituted-pending-obligation-signed-identity"),
                    ));
            }
            Tamper::FullBinding => {
                tampered.binding = alternate_binding.clone();
            }
        }
        let tampered_payload =
            norito::to_bytes(&tampered).expect("encode well-formed substitution fixture");
        {
            let mut world = state.world.block();
            world
                .smart_contract_state
                .insert(obligation_key.clone(), tampered_payload.clone());
            world.commit();
        }
        let mut world = state.world.block();
        assert!(
            State::stage_queue_plan_pending_obligation_in_storage(
                &mut world.smart_contract_state,
                &admission,
            )
            .is_err(),
            "a substituted pending obligation must fail stage preflight"
        );
        assert_eq!(
            world.smart_contract_state.get(&obligation_key),
            Some(&tampered_payload),
            "failed stage preflight must preserve the substituted marker for diagnosis"
        );
        for (key, payload) in member_keys.iter().zip(&member_payloads) {
            assert_eq!(
                world.smart_contract_state.get(key),
                Some(payload),
                "failed stage preflight must not publish or remove a route member"
            );
        }
        assert!(
            State::resolve_queue_plan_pending_obligation_in_storage(
                &mut world.smart_contract_state,
                binding.network_id_digest,
                binding.entrypoint_hash.clone(),
            )
            .is_err(),
            "a substituted pending obligation must fail resolution preflight"
        );
        assert_eq!(
            world.smart_contract_state.get(&obligation_key),
            Some(&tampered_payload),
            "failed resolution preflight must preserve the substituted marker"
        );
        for (key, payload) in member_keys.iter().zip(&member_payloads) {
            assert_eq!(
                world.smart_contract_state.get(key),
                Some(payload),
                "failed resolution preflight must not remove any route member"
            );
        }
    }
}
include!("autonomous_merge_and_queue_plan_route_count_tests.rs");
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one adversarial roster test covers missing, malformed, oversized, and bounded members"
)]
fn queue_plan_route_accumulator_rejects_positive_undercount_and_overcount_atomically() {
    {
        let (state, validator_keypairs, _, _) = configured_two_lane_merge_state();
        let routing_plan = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
        ));
        let (first_binding, first_certificate) = queue_plan_admission_certificate_for_state_test(
            &state,
            routing_plan.clone(),
            &validator_keypairs,
            queue_plan_authority_height_for_state_test(&state),
            0x71,
        );
        let (second_binding, second_certificate) = queue_plan_admission_certificate_for_state_test(
            &state,
            routing_plan,
            &validator_keypairs,
            queue_plan_authority_height_for_state_test(&state),
            0x72,
        );
        let first_admission =
            crate::torii_proxy::decode_and_validate_queue_plan_admission_certificate_v1(
                &state.network_id,
                &first_certificate,
            )
            .expect("fixture first QueuePlan admission certificate");
        let first_obligation = queue_plan_pending_obligation_for_test(&state, &first_certificate);
        let second_obligation = queue_plan_pending_obligation_for_test(&state, &second_certificate);
        assert_eq!(first_obligation.routes, second_obligation.routes);
        let route = first_obligation.routes[0];
        let first_member =
            State::queue_plan_pending_route_member_identity(&first_obligation, route)
                .expect("fixture first route member identity");
        let second_member =
            State::queue_plan_pending_route_member_identity(&second_obligation, route)
                .expect("fixture second route member identity");
        assert_ne!(first_member, second_member);
        seed_exact_queue_plan_admission_state_for_test(&state, &first_certificate);
        seed_exact_queue_plan_admission_state_for_test(&state, &second_certificate);
        assert!(
            State::queue_plan_pending_route_members_from_storage_with_limit(
                state.world.view().smart_contract_state(),
                route,
                1,
            )
            .is_err(),
            "the exact roster must fail closed at consensus cap plus one"
        );
        let first_member_key =
            State::queue_plan_pending_route_member_marker_key(route, first_member)
                .expect("fixture first exact route-member key");
        let second_member_key =
            State::queue_plan_pending_route_member_marker_key(route, second_member)
                .expect("fixture second exact route-member key");
        let first_member_payload = {
            let world = state.world.view();
            world
                .smart_contract_state()
                .get(&first_member_key)
                .cloned()
                .expect("fixture first exact route-member payload")
        };
        let first_obligation_key = State::queue_plan_pending_obligation_marker_key(
            first_binding.network_id_digest,
            first_binding.entrypoint_hash.clone(),
        )
        .expect("fixture first obligation key");
        let second_obligation_key = State::queue_plan_pending_obligation_marker_key(
            second_binding.network_id_digest,
            second_binding.entrypoint_hash.clone(),
        )
        .expect("fixture second obligation key");
        {
            let mut world = state.world.block();
            world.smart_contract_state.remove(first_member_key.clone());
            world.commit();
        }
        assert!(
            state
                .queue_plan_admission_binding_registry_match(&first_binding)
                .is_err(),
            "the exact route roster must not conceal a missing member"
        );
        let (first_before, second_before, second_member_before) = {
            let world = state.world.view();
            (
                world
                    .smart_contract_state()
                    .get(&first_obligation_key)
                    .cloned(),
                world
                    .smart_contract_state()
                    .get(&second_obligation_key)
                    .cloned(),
                world
                    .smart_contract_state()
                    .get(&second_member_key)
                    .cloned(),
            )
        };
        {
            let mut world = state.world.block();
            assert!(
                State::stage_queue_plan_pending_obligation_in_storage(
                    &mut world.smart_contract_state,
                    &first_admission,
                )
                .is_err(),
                "idempotent staging must reject a missing exact nonterminal member"
            );
            assert!(
                State::resolve_queue_plan_pending_obligation_in_storage(
                    &mut world.smart_contract_state,
                    first_binding.network_id_digest,
                    first_binding.entrypoint_hash.clone(),
                )
                .is_err(),
                "nonterminal resolution must reject a missing exact member"
            );
            assert_eq!(
                world
                    .smart_contract_state
                    .get(&first_obligation_key)
                    .cloned(),
                first_before
            );
            assert_eq!(
                world
                    .smart_contract_state
                    .get(&second_obligation_key)
                    .cloned(),
                second_before
            );
            assert_eq!(
                world.smart_contract_state.get(&second_member_key).cloned(),
                second_member_before,
                "failed exact-member checks must not mutate another roster member"
            );
            assert!(world.smart_contract_state.get(&first_member_key).is_none());
        }
        {
            let mut world = state.world.block();
            world
                .smart_contract_state
                .insert(first_member_key.clone(), first_member_payload);
            world.commit();
        }
        {
            let mut world = state.world.block();
            world
                .smart_contract_state
                .insert(first_member_key.clone(), vec![0x00]);
            world.commit();
        }
        let (first_before, second_before, member_before) = {
            let world = state.world.view();
            (
                world
                    .smart_contract_state()
                    .get(&first_obligation_key)
                    .cloned(),
                world
                    .smart_contract_state()
                    .get(&second_obligation_key)
                    .cloned(),
                world.smart_contract_state().get(&first_member_key).cloned(),
            )
        };
        let mut world = state.world.block();
        assert!(
            State::stage_queue_plan_pending_obligation_in_storage(
                &mut world.smart_contract_state,
                &first_admission,
            )
            .is_err(),
            "idempotent staging must reject a malformed exact member"
        );
        assert_eq!(
            world
                .smart_contract_state
                .get(&first_obligation_key)
                .cloned(),
            first_before
        );
        assert_eq!(
            world
                .smart_contract_state
                .get(&second_obligation_key)
                .cloned(),
            second_before
        );
        assert_eq!(
            world.smart_contract_state.get(&first_member_key).cloned(),
            member_before
        );
        assert!(
            State::resolve_queue_plan_pending_obligation_in_storage(
                &mut world.smart_contract_state,
                first_binding.network_id_digest,
                first_binding.entrypoint_hash,
            )
            .is_err(),
            "resolution must reject a malformed exact member"
        );
        assert_eq!(
            world
                .smart_contract_state
                .get(&first_obligation_key)
                .cloned(),
            first_before
        );
        assert_eq!(
            world
                .smart_contract_state
                .get(&second_obligation_key)
                .cloned(),
            second_before
        );
        assert_eq!(
            world.smart_contract_state.get(&first_member_key).cloned(),
            member_before,
            "failed malformed-member checks must not mutate the exact roster"
        );
    }
    {
        let (state, validator_keypairs, _, _) = configured_two_lane_merge_state();
        let routing_plan = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
        ));
        let (binding, certificate) = queue_plan_admission_certificate_for_state_test(
            &state,
            routing_plan,
            &validator_keypairs,
            queue_plan_authority_height_for_state_test(&state),
            0x73,
        );
        let admission =
            crate::torii_proxy::decode_and_validate_queue_plan_admission_certificate_v1(
                &state.network_id,
                &certificate,
            )
            .expect("fixture oversized-member QueuePlan admission certificate");
        let obligation = queue_plan_pending_obligation_for_test(&state, &certificate);
        let route = obligation.routes[0];
        let member_identity = State::queue_plan_pending_route_member_identity(&obligation, route)
            .expect("fixture oversized route-member identity");
        seed_exact_queue_plan_admission_state_for_test(&state, &certificate);
        let member_key = State::queue_plan_pending_route_member_marker_key(route, member_identity)
            .expect("fixture oversized route-member key");
        {
            let mut world = state.world.block();
            world.smart_contract_state.insert(
                member_key.clone(),
                vec![0xA5; MAX_QUEUE_PLAN_COMPACT_MARKER_BYTES + 1],
            );
            world.commit();
        }
        let obligation_key = State::queue_plan_pending_obligation_marker_key(
            binding.network_id_digest,
            binding.entrypoint_hash.clone(),
        )
        .expect("fixture oversized-member obligation key");
        let (obligation_before, member_before) = {
            let world = state.world.view();
            (
                world.smart_contract_state().get(&obligation_key).cloned(),
                world.smart_contract_state().get(&member_key).cloned(),
            )
        };
        let mut world = state.world.block();
        assert!(
            State::stage_queue_plan_pending_obligation_in_storage(
                &mut world.smart_contract_state,
                &admission,
            )
            .is_err(),
            "idempotent staging must reject an oversized exact member"
        );
        assert_eq!(
            world.smart_contract_state.get(&obligation_key).cloned(),
            obligation_before
        );
        assert_eq!(
            world.smart_contract_state.get(&member_key).cloned(),
            member_before
        );
        assert!(
            State::resolve_queue_plan_pending_obligation_in_storage(
                &mut world.smart_contract_state,
                binding.network_id_digest,
                binding.entrypoint_hash,
            )
            .is_err(),
            "resolution must reject an oversized exact member"
        );
        assert_eq!(
            world.smart_contract_state.get(&obligation_key).cloned(),
            obligation_before
        );
        assert_eq!(
            world.smart_contract_state.get(&member_key).cloned(),
            member_before,
            "failed oversized-member checks must not mutate the exact roster"
        );
    }
}
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one adversarial test covers every exact-member corruption and whole-list rollback"
)]
fn queue_plan_pending_resolution_corrupt_route_counts_fail_without_partial_mutation() {
    #[derive(Clone, Copy)]
    enum Corruption {
        MissingMember,
        MalformedMember,
        OversizedMember,
        WrongKeyMember,
    }
    for (tag, corruption) in [
        (0x74, Corruption::MissingMember),
        (0x75, Corruption::MalformedMember),
        (0x78, Corruption::OversizedMember),
        (0x76, Corruption::WrongKeyMember),
    ] {
        let (state, validator_keypairs, _, _) = configured_two_lane_merge_state();
        let participant_lane = LaneId::new(1);
        let routing_plan = crate::queue::RoutingPlan::native_amx(
            crate::queue::RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            vec![crate::queue::RouteLeg::new(
                crate::queue::RoutingDecision::new(participant_lane, DataSpaceId::UNIVERSAL),
                crate::queue::RouteLegRole::Participant,
            )],
        );
        let (binding, certificate) = queue_plan_admission_certificate_for_state_test(
            &state,
            routing_plan,
            &validator_keypairs,
            queue_plan_authority_height_for_state_test(&state),
            tag,
        );
        let obligation = queue_plan_pending_obligation_for_test(&state, &certificate);
        seed_exact_queue_plan_admission_state_for_test(&state, &certificate);
        let corrupt_route = *obligation
            .routes
            .iter()
            .find(|route| route.lane_id == participant_lane)
            .expect("fixture participant route");
        let corrupt_member_identity =
            State::queue_plan_pending_route_member_identity(&obligation, corrupt_route)
                .expect("fixture participant route-member identity");
        let corrupt_member_key = State::queue_plan_pending_route_member_marker_key(
            corrupt_route,
            corrupt_member_identity,
        )
        .expect("fixture participant route-member key");
        let wrong_member_key = {
            let mut world = state.world.block();
            let wrong_member_key = match corruption {
                Corruption::MissingMember => {
                    world
                        .smart_contract_state
                        .remove(corrupt_member_key.clone());
                    None
                }
                Corruption::MalformedMember => {
                    world
                        .smart_contract_state
                        .insert(corrupt_member_key.clone(), vec![0x00]);
                    None
                }
                Corruption::OversizedMember => {
                    world.smart_contract_state.insert(
                        corrupt_member_key.clone(),
                        vec![0xA5; MAX_QUEUE_PLAN_COMPACT_MARKER_BYTES + 1],
                    );
                    None
                }
                Corruption::WrongKeyMember => {
                    let payload = world
                        .smart_contract_state
                        .get(&corrupt_member_key)
                        .cloned()
                        .expect("fixture canonical route-member payload");
                    world
                        .smart_contract_state
                        .remove(corrupt_member_key.clone());
                    let mut wrong_identity = corrupt_member_identity;
                    wrong_identity[0] ^= 0x80;
                    let wrong_key = State::queue_plan_pending_route_member_marker_key(
                        corrupt_route,
                        wrong_identity,
                    )
                    .expect("fixture wrong route-member key");
                    world
                        .smart_contract_state
                        .insert(wrong_key.clone(), payload);
                    Some(wrong_key)
                }
            };
            world.commit();
            wrong_member_key
        };
        let obligation_key = State::queue_plan_pending_obligation_marker_key(
            binding.network_id_digest,
            binding.entrypoint_hash.clone(),
        )
        .expect("fixture pending-obligation key");
        let mut route_member_keys = obligation
            .routes
            .iter()
            .copied()
            .map(|route| {
                let member_identity =
                    State::queue_plan_pending_route_member_identity(&obligation, route)
                        .expect("fixture exact route-member identity");
                State::queue_plan_pending_route_member_marker_key(route, member_identity)
                    .expect("fixture exact route-member key")
            })
            .collect::<Vec<_>>();
        if let Some(key) = wrong_member_key {
            route_member_keys.push(key);
        }
        let (obligation_before, route_members_before) = {
            let world = state.world.view();
            (
                world.smart_contract_state().get(&obligation_key).cloned(),
                route_member_keys
                    .iter()
                    .map(|key| world.smart_contract_state().get(key).cloned())
                    .collect::<Vec<_>>(),
            )
        };
        let mut world = state.world.block();
        assert!(
            State::resolve_queue_plan_pending_obligation_in_storage(
                &mut world.smart_contract_state,
                binding.network_id_digest,
                binding.entrypoint_hash,
            )
            .is_err(),
            "missing, malformed, oversized, and wrong-key route members must all fail closed"
        );
        assert_eq!(
            world.smart_contract_state.get(&obligation_key).cloned(),
            obligation_before,
            "failed resolution must retain the exact pending obligation"
        );
        for (key, before) in route_member_keys.iter().zip(route_members_before) {
            assert_eq!(
                world.smart_contract_state.get(key).cloned(),
                before,
                "failed resolution must not partially remove any exact route member"
            );
        }
    }
    let (state, validator_keypairs, _, parent) = configured_two_lane_merge_state();
    let first_plan = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
    ));
    let second_plan = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
        LaneId::new(1),
        DataSpaceId::UNIVERSAL,
    ));
    let (first_binding, first_certificate) = queue_plan_admission_certificate_for_state_test(
        &state,
        first_plan,
        &validator_keypairs,
        queue_plan_authority_height_for_state_test(&state),
        0x7B,
    );
    let (second_binding, second_certificate) = queue_plan_admission_certificate_for_state_test(
        &state,
        second_plan,
        &validator_keypairs,
        queue_plan_authority_height_for_state_test(&state),
        0x7C,
    );
    let first_obligation = queue_plan_pending_obligation_for_test(&state, &first_certificate);
    let second_obligation = queue_plan_pending_obligation_for_test(&state, &second_certificate);
    seed_exact_queue_plan_admission_state_for_test(&state, &first_certificate);
    seed_exact_queue_plan_admission_state_for_test(&state, &second_certificate);
    let first_obligation_key = State::queue_plan_pending_obligation_marker_key(
        first_binding.network_id_digest,
        first_binding.entrypoint_hash.clone(),
    )
    .expect("fixture first bulk obligation key");
    let second_obligation_key = State::queue_plan_pending_obligation_marker_key(
        second_binding.network_id_digest,
        second_binding.entrypoint_hash.clone(),
    )
    .expect("fixture second bulk obligation key");
    let first_member = State::queue_plan_pending_route_member_from_obligation(
        &first_obligation,
        first_obligation.routes[0],
    )
    .expect("fixture first bulk member");
    let second_member = State::queue_plan_pending_route_member_from_obligation(
        &second_obligation,
        second_obligation.routes[0],
    )
    .expect("fixture second bulk member");
    let first_member_key = State::queue_plan_pending_route_member_marker_key(
        first_member.route,
        first_member.member_identity,
    )
    .expect("fixture first bulk member key");
    let second_member_key = State::queue_plan_pending_route_member_marker_key(
        second_member.route,
        second_member.member_identity,
    )
    .expect("fixture second bulk member key");
    let second_member_payload = {
        let world = state.world.view();
        world
            .smart_contract_state()
            .get(&second_member_key)
            .cloned()
            .expect("fixture second bulk member payload")
    };
    let carrier = empty_global_block_after(Some(&parent));
    let mut state_block = state.block(carrier.header());
    state_block
        .world
        .smart_contract_state
        .insert(second_member_key.clone(), vec![0x00]);
    let write_set_before = state_block.merge_execution_write_set_root();
    assert!(
        state_block
            .resolve_queue_plan_pending_obligations_for_entrypoints([
                first_binding.entrypoint_hash.clone(),
                second_binding.entrypoint_hash.clone(),
            ])
            .is_err(),
        "a later-route failure must roll back an earlier successful resolution"
    );
    assert_eq!(
        state_block.merge_execution_write_set_root(),
        write_set_before,
        "failed whole-list resolution must restore the exact prior overlay"
    );
    for key in [
        &first_obligation_key,
        &first_member_key,
        &second_obligation_key,
    ] {
        assert!(
            state_block.world.smart_contract_state.get(key).is_some(),
            "failed whole-list resolution removed `{key}`"
        );
    }
    assert_eq!(
        state_block
            .world
            .smart_contract_state
            .get(&second_member_key)
            .map(Vec::as_slice),
        Some(&[0x00][..]),
    );
    state_block
        .world
        .smart_contract_state
        .insert(second_member_key.clone(), second_member_payload);
    state_block
        .resolve_queue_plan_pending_obligations_for_entrypoints([first_binding.entrypoint_hash])
        .expect("the same StateBlock remains reusable after resolution rollback");
    assert!(
        state_block
            .world
            .smart_contract_state
            .get(&first_obligation_key)
            .is_none()
    );
    assert!(
        state_block
            .world
            .smart_contract_state
            .get(&first_member_key)
            .is_none()
    );
    assert!(
        state_block
            .world
            .smart_contract_state
            .get(&second_obligation_key)
            .is_some()
    );
    assert!(
        state_block
            .world
            .smart_contract_state
            .get(&second_member_key)
            .is_some()
    );
}
#[test]
fn queue_plan_registry_presence_is_bounded_and_malformed_markers_fail_closed() {
    let (state, validator_keypairs, _, _) = configured_two_lane_merge_state();
    let routing_plan = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
    ));
    let (binding, certificate) = queue_plan_admission_certificate_for_state_test(
        &state,
        routing_plan,
        &validator_keypairs,
        queue_plan_authority_height_for_state_test(&state),
        0x5A,
    );
    assert!(
        !state
            .queue_plan_admission_registry_entrypoint_present(binding.entrypoint_hash.clone(),)
            .expect("absent registry presence lookup")
    );
    let key = State::queue_plan_admission_registry_marker_key(&binding.registry_key())
        .expect("fixture registry key");
    let payload = State::queue_plan_admission_registry_marker_payload(&binding.registry_value())
        .expect("fixture registry value");
    {
        let mut world = state.world.block();
        world
            .smart_contract_state
            .insert(key.clone(), payload.clone());
        world.commit();
    }
    assert!(
        state
            .queue_plan_admission_registry_entrypoint_present(binding.entrypoint_hash.clone(),)
            .is_err(),
        "a registry owner without its pending obligation must fail closed"
    );
    seed_exact_queue_plan_admission_state_for_test(&state, &certificate);
    assert!(
        state
            .queue_plan_admission_registry_entrypoint_present(binding.entrypoint_hash.clone(),)
            .expect("exact pending admission presence lookup")
    );
    {
        let mut world = state.world.block();
        world.smart_contract_state.insert(key.clone(), vec![0x00]);
        world.commit();
    }
    assert!(
        state
            .queue_plan_admission_registry_entrypoint_present(binding.entrypoint_hash.clone())
            .is_err(),
        "a malformed marker must not be treated as an absent or canonical admission"
    );
    let obligation_key = State::queue_plan_pending_obligation_marker_key(
        binding.network_id_digest,
        binding.entrypoint_hash.clone(),
    )
    .expect("fixture pending-obligation key");
    {
        let mut world = state.world.block();
        world.smart_contract_state.insert(key, payload);
        world.smart_contract_state.insert(
            obligation_key,
            vec![0xA5; MAX_QUEUE_PLAN_PENDING_OBLIGATION_BYTES.saturating_add(1)],
        );
        world.commit();
    }
    assert!(
        state
            .queue_plan_admission_registry_entrypoint_present(binding.entrypoint_hash)
            .is_err(),
        "an oversized pending-obligation marker must fail before bounded decode"
    );
}
#[test]
fn autonomous_execution_requires_exact_pre_carrier_queue_plan_admission() {
    let (state, validator_keypairs, _, parent) = configured_single_lane_queue_plan_state();
    let authority_height = parent.header().height().get();
    let carrier_height = authority_height
        .checked_add(1)
        .expect("fixture carrier height");
    // Autonomous lane certification is checked against the exact route committee. Keep this
    // focused fixture's global signing topology identical so its source helper cannot substitute
    // a different committee.
    set_commit_topology_from_keypairs(&state, &validator_keypairs);
    let tag = 0x61;
    let entrypoint = queue_plan_entrypoint_for_state_test(&state, tag);
    let routing_plan = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
    ));
    let (binding, certificate) = queue_plan_admission_certificate_for_state_test(
        &state,
        routing_plan.clone(),
        &validator_keypairs,
        authority_height,
        tag,
    );
    binding
        .validate_for_request(&state.network_id, &entrypoint, &routing_plan)
        .expect("fixture certificate binds the autonomous transaction");
    {
        let mut world = state.world.block();
        world.accounts.insert(
            entrypoint.authority().clone(),
            AccountValue::new(AccountDetails::default()),
        );
        world.commit();
    }
    let source = autonomous_merge_source_for_queue_plan_admission_test(
        &state,
        &binding,
        entrypoint,
        routing_plan,
        &validator_keypairs,
    )
    .expect("canonical autonomous QueuePlan fixture source");
    let application_header = BlockHeader::new(
        NonZeroU64::new(carrier_height).expect("fixture carrier height is non-zero"),
        Some(parent.hash()),
        None,
        None,
        u64::try_from(parent.header().creation_time().as_millis())
            .expect("fixture parent time fits u64")
            .saturating_add(1),
        0,
    );
    assert!(
        state
            .build_merge_execution_batch_from_source_prefix(
                1,
                application_header.clone(),
                vec![source.clone()],
            )
            .is_none(),
        "an availability-certified source remains ineligible while its binding is absent from pre-carrier WSV"
    );
    seed_exact_queue_plan_admission_state_for_test(&state, &certificate);
    let batch = state
        .build_merge_execution_batch_from_source_prefix(1, application_header.clone(), vec![source])
        .expect("the otherwise-identical source is eligible with exact pre-carrier authority");
    let lifecycle = state.lane_consensus_lifecycle_snapshot();
    let active_lanes = lifecycle
        .nexus
        .lane_catalog
        .lanes()
        .iter()
        .map(|lane| MergeLaneBinding {
            lane_id: lane.id,
            dataspace_id: lane.dataspace_id,
            lane_config_hash: merge_lane_config_hash(lane),
            incarnation: lifecycle.incarnations[&lane.id],
            activation_height: lifecycle.activation_heights[&lane.id].saturating_add(1),
        })
        .collect::<Vec<_>>();
    let incarnation_entries = active_lanes
        .iter()
        .map(
            |lane| iroha_data_model::nexus::LaneLifecycleIncarnationEntry {
                lane_id: lane.lane_id,
                incarnation: lane.incarnation,
            },
        )
        .collect::<Vec<_>>();
    let base = crate::merge::MergeLedgerCandidate {
        version: crate::merge::MergeLedgerCandidate::VERSION,
        epoch_id: 1,
        view: 0,
        carrier_height,
        carrier_parent_hash: parent.hash(),
        lane_catalog_hash: merge_lane_catalog_hash(&lifecycle.nexus.lane_catalog),
        active_lanes: active_lanes.clone(),
        incarnation_root: LaneLifecycleParameterV1::incarnation_root(&incarnation_entries),
        activation_root: crate::merge::merge_activation_root(&active_lanes),
        lane_snapshots: Vec::new(),
        execution_batch: Some(batch),
        lane_drain_certificates: Vec::new(),
        global_state_root: crate::merge::reduce_merge_hint_roots(&[]),
    };
    state
        .validate_merge_candidate_for_global_round(
            &base,
            &parent.header(),
            0,
            ConsensusMode::Permissioned,
        )
        .expect("candidate is valid with exact committed pre-carrier authority");
}
#[test]
fn pending_queue_plan_admission_keeps_historical_eligibility_after_frontier_advance() {
    let (state, validator_keypairs, _, parent) = configured_single_lane_queue_plan_state();
    let authority_height = parent.header().height().get();
    let proposal_height = authority_height
        .checked_add(1)
        .expect("fixture proposal height");
    let routing_plan = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
    ));
    let (_, certificate) = queue_plan_admission_certificate_for_state_test(
        &state,
        routing_plan,
        &validator_keypairs,
        authority_height,
        0x63,
    );
    assert_eq!(
        state
            .classify_pending_queue_plan_admission(&certificate, proposal_height)
            .expect("certificate is classifiable at its authority frontier")
            .1,
        PendingQueuePlanAdmissionDisposition::EligibleAbsent
    );
    let successor = empty_global_block_after(Some(&parent));
    state
        .kura
        .store_block(Arc::new(successor.clone()))
        .expect("store the canonical successor before committing State metadata");
    commit_block_metadata_to_state(&state, &successor);
    assert_eq!(
        state.committed_height(),
        usize::try_from(successor.header().height().get()).expect("successor height fits usize"),
    );
    let next_proposal_height = successor
        .header()
        .height()
        .get()
        .checked_add(1)
        .expect("fixture next proposal height");
    assert_eq!(
        state
            .classify_pending_queue_plan_admission(&certificate, next_proposal_height)
            .expect("historically bound certificate remains classifiable after advancement")
            .1,
        PendingQueuePlanAdmissionDisposition::EligibleAbsent,
        "advancing the receiver must retain the exact predecessor, roster, and incarnation authority at H"
    );
}
#[test]
fn queue_plan_validation_waiting_for_state_generation_does_not_pin_block_hashes() {
    let (state, validator_keypairs, _, parent) = configured_single_lane_queue_plan_state();
    let authority_height = parent.header().height().get();
    let proposal_height = authority_height
        .checked_add(1)
        .expect("fixture proposal height");
    let routing_plan = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
    ));
    let (_, certificate) = queue_plan_admission_certificate_for_state_test(
        &state,
        routing_plan,
        &validator_keypairs,
        authority_height,
        0x65,
    );
    let active_lanes = state
        .queue_plan_active_lane_bindings()
        .expect("resolve QueuePlan lane bindings before the writer begins");
    let state = Arc::new(state);
    let start = Arc::new(Barrier::new(2));
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (completion_tx, completion_rx) = std::sync::mpsc::channel();
    let worker_state = Arc::clone(&state);
    let worker_start = Arc::clone(&start);
    let worker = std::thread::spawn(move || {
        worker_start.wait();
        entered_tx
            .send(())
            .expect("announce QueuePlan validation attempt");
        let result = worker_state
            .validate_queue_plan_admissions_for_carrier(
                &[certificate],
                &active_lanes,
                proposal_height,
                true,
            )
            .map(|_| ())
            .map_err(|error| error.to_string());
        completion_tx
            .send(result)
            .expect("return QueuePlan validation result");
    });

    let generation_guard = state.begin_state_view_write();
    start.wait();
    entered_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("QueuePlan validation worker starts");
    let observation_deadline =
        std::time::Instant::now() + Duration::from_millis(250);
    let mut block_hashes_pinned = false;
    while std::time::Instant::now() < observation_deadline {
        if state.block_hashes.inner.try_write().is_none() {
            block_hashes_pinned = true;
            break;
        }
        std::thread::yield_now();
    }
    let premature_completion = completion_rx.try_recv().ok();
    let completed_while_generation_odd = premature_completion.is_some();
    drop(generation_guard);
    let validation = premature_completion.unwrap_or_else(|| {
        completion_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("QueuePlan validation completes after the writer generation ends")
    });
    worker.join().expect("QueuePlan validation worker");

    assert!(
        !block_hashes_pinned,
        "validation waiting for a coherent State generation must not retain a block-hash read guard",
    );
    assert!(
        !completed_while_generation_odd,
        "validation must wait for the active State writer generation",
    );
    validation.expect("the exact QueuePlan certificate remains valid");
}
#[test]
fn pending_queue_plan_admission_is_future_until_its_canonical_frontier_arrives() {
    let (state, validator_keypairs, _, parent) = configured_single_lane_queue_plan_state();
    let successor = empty_global_block_after(Some(&parent));
    let future_authority_height = successor.header().height().get();
    let future_proposal_height = future_authority_height
        .checked_add(1)
        .expect("fixture future proposal height");
    // Construct an authentic certificate at H + 1, then restore the receiver to H. The durable
    // certificate can arrive before block sync, so classification must retain it without treating
    // the locally missing predecessor as stale.
    {
        let mut block_hashes = state.block_hashes.block();
        block_hashes.push_for_tests(successor.hash());
        block_hashes.commit_for_tests();
    }
    let routing_plan = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
    ));
    let (_, certificate) = queue_plan_admission_certificate_for_state_test(
        &state,
        routing_plan,
        &validator_keypairs,
        future_authority_height,
        0x64,
    );
    {
        let block_hashes = state.block_hashes.block_and_revert();
        assert_eq!(block_hashes.last().copied(), Some(parent.hash()));
        block_hashes.commit_for_tests();
    }
    assert_eq!(
        state.committed_height(),
        usize::try_from(parent.header().height().get()).expect("parent height fits usize"),
    );
    assert_eq!(
        state
            .classify_pending_queue_plan_admission(&certificate, future_proposal_height)
            .expect("future authenticated certificate is retained, not rejected")
            .1,
        PendingQueuePlanAdmissionDisposition::Future
    );
    state
        .kura
        .store_block(Arc::new(successor.clone()))
        .expect("store the arriving canonical successor");
    commit_block_metadata_to_state(&state, &successor);
    assert_eq!(
        state
            .classify_pending_queue_plan_admission(&certificate, future_proposal_height)
            .expect("certificate is reclassifiable after canonical catch-up")
            .1,
        PendingQueuePlanAdmissionDisposition::EligibleAbsent
    );
}
#[test]
fn pending_queue_plan_admission_keeps_mutated_history_roster_and_incarnation_stale() {
    let (state, validator_keypairs, _, parent) = configured_single_lane_queue_plan_state();
    let authority_height = parent.header().height().get();
    let proposal_height = authority_height
        .checked_add(1)
        .expect("fixture proposal height");
    let routing_plan = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
    ));
    let forged_predecessor = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        b"mutated-pending-queue-plan-predecessor",
    ));
    {
        let mut block_hashes = state.block_hashes.block_and_revert();
        block_hashes.push_for_tests(forged_predecessor);
        block_hashes.commit_for_tests();
    }
    let (_, predecessor_certificate) = queue_plan_admission_certificate_for_state_test(
        &state,
        routing_plan.clone(),
        &validator_keypairs,
        authority_height,
        0x65,
    );
    {
        let mut block_hashes = state.block_hashes.block_and_revert();
        block_hashes.push_for_tests(parent.hash());
        block_hashes.commit_for_tests();
    }
    assert_eq!(
        state
            .classify_pending_queue_plan_admission(&predecessor_certificate, proposal_height)
            .expect("authenticated predecessor mutation is classifiable")
            .1,
        PendingQueuePlanAdmissionDisposition::Stale
    );
    let (alternate_validator_ids, alternate_validator_keypairs) =
        bls_accounts_in("mutated-queue-plan-roster", 4);
    seed_consensus_keys_with_pops(&state, &alternate_validator_keypairs);
    install_lane_manifest_registry(
        &state,
        &[(
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
            alternate_validator_ids,
        )],
    );
    let (_, roster_certificate) = queue_plan_admission_certificate_for_state_test(
        &state,
        routing_plan.clone(),
        &alternate_validator_keypairs,
        authority_height,
        0x66,
    );
    let original_validator_ids = validator_keypairs
        .iter()
        .map(|keypair| AccountId::new(keypair.public_key().clone()))
        .collect::<Vec<_>>();
    install_lane_manifest_registry(
        &state,
        &[(
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
            original_validator_ids,
        )],
    );
    assert_eq!(
        state
            .classify_pending_queue_plan_admission(&roster_certificate, proposal_height)
            .expect("authenticated roster mutation is classifiable")
            .1,
        PendingQueuePlanAdmissionDisposition::Stale
    );
    let (_, incarnation_certificate) = queue_plan_admission_certificate_for_state_test(
        &state,
        routing_plan,
        &validator_keypairs,
        authority_height,
        0x67,
    );
    let original_incarnation = state
        .lane_incarnation(LaneId::SINGLE)
        .expect("fixture lane incarnation");
    let _ = state.lane_incarnations.write().insert(
        LaneId::SINGLE,
        Hash::new(b"mutated-pending-queue-plan-incarnation"),
    );
    assert_eq!(
        state
            .classify_pending_queue_plan_admission(&incarnation_certificate, proposal_height)
            .expect("authenticated incarnation mutation is classifiable")
            .1,
        PendingQueuePlanAdmissionDisposition::Stale
    );
    let _ = state
        .lane_incarnations
        .write()
        .insert(LaneId::SINGLE, original_incarnation);
}
include!("autonomous_merge_and_queue_plan_native_diagnostic_tests.rs");
