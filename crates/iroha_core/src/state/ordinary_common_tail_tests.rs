// Real State/Kura receipts exercise the shared ordinary tail. The unrelated
// admitted fixture group stays pending; this test grants no native publication.

fn ordinary_tail_batch(fixture: &NativeEconomicFixture) -> TransferAssetBatch {
    TransferAssetBatch::independent(vec![
        TransferAssetBatchEntry::with_leg_id(
            "applied",
            fixture.source.account().clone(),
            fixture.destination.account().clone(),
            fixture.source.definition().clone(),
            3u32,
        ),
        TransferAssetBatchEntry::with_leg_id(
            "rejected",
            fixture.source.account().clone(),
            fixture.destination.account().clone(),
            fixture.source.definition().clone(),
            1000u32,
        ),
    ])
}

fn install_ordinary_tail_repeating_trigger(fixture: &NativeEconomicFixture, period_ms: u64) {
    let mut metadata = iroha_model_base::metadata::Metadata::default();
    metadata.insert(
        "__registered_block_height".parse::<Name>().unwrap(),
        Json::new(0u64),
    );
    metadata.insert(
        "__registered_at_ms".parse::<Name>().unwrap(),
        Json::new(0u64),
    );
    let trigger = Trigger::new(
        "common_tail_repeat".parse().unwrap(),
        Action::new(
            [InstructionBox::from(ordinary_tail_batch(fixture))],
            Repeats::Exactly(2),
            fixture.source.account().clone(),
            TimeEventFilter::new(ExecutionTime::Schedule(Schedule {
                start_ms: fixture.native.block.header().creation_time_ms,
                period_ms: Some(period_ms),
            })),
        )
        .unwrap()
        .with_metadata(metadata),
    );
    // Register through the same trigger-storage owner used by existing Time
    // execution tests, after the authenticated genesis/admission fixture. This
    // prevents fixture setup blocks from consuming this test's two invocations.
    let mut block = fixture.native.state.world.triggers.block();
    let mut transaction = block.transaction();
    transaction
        .add_time_trigger(trigger.try_into().unwrap())
        .unwrap();
    transaction.apply();
    block.commit();
}

#[inline(never)]
fn run_ordinary_tail_independent_batches(
    repeated_time: bool,
    prejoin_prefix: bool,
    leftover_call: bool,
) {
    use iroha_data_model::events::data::prelude::AssetBatchTransferLegStatus;
    use iroha_data_model::transaction::signed::TransactionResult;
    let fixture = native_economic_fixture(&[NativeEconomicCase::Transfer(25)], false);
    let state = &fixture.native.state;
    let cadence = state
        .world
        .parameters
        .view()
        .sumeragi()
        .block_cadence_ms()
        .get();
    if repeated_time {
        install_ordinary_tail_repeating_trigger(&fixture, cadence);
    }
    let key = KeyPair::try_from_seed(vec![0x71; 32], Algorithm::Ed25519).unwrap();
    let header = BlockHeader::new(
        NonZeroU64::new(fixture.native.block.header().height().get() + 1).unwrap(),
        Some(fixture.native.block.hash()),
        None,
        None,
        fixture.native.block.header().creation_time_ms + 2 * cadence,
        0,
    );
    let mut transaction = TransactionBuilder::new(
        state.network_id,
        fixture.source.account().clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    transaction.set_creation_time(header.creation_time());
    let signed = transaction
        .with_instructions([ordinary_tail_batch(&fixture)])
        .sign(key.private_key());
    let entry = TransactionEntrypoint::External(signed.clone());
    let prefix_call = Hash::from(entry.execution_call_hash());
    let mut builder = iroha_data_model::block::builder::BlockBuilder::new(header);
    builder.push_transaction(signed);
    let mut carrier = builder
        .build(BTreeSet::new())
        .canonical_resultless_proposal();
    let unexecuted = carrier.clone();
    let proposal_hash = carrier.hash();
    let route = crate::queue::RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let _witness_guard = crate::sumeragi::witness::exec_witness_guard();
    crate::sumeragi::witness::start_block();
    let mut overlay = Box::new(state.block(carrier.header()));
    let accepted = crate::tx::AcceptedTransaction::accept_entrypoint_at_time(
        entry.clone(),
        &state.network_id,
        overlay.world.parameters().sumeragi().max_clock_drift(),
        overlay.world.parameters().transaction(),
        overlay.crypto.as_ref(),
        header.creation_time(),
    )
    .unwrap();
    let (hash, inner) = overlay.validate_transaction_with_entrypoint_index_and_routing_context(
        accepted,
        &mut crate::smartcontracts::ivm::cache::IvmCache::new(),
        0,
        route,
    );
    assert_eq!(hash, entry.hash());
    assert!(
        inner.is_ok(),
        "real independent prefix execution: {inner:?}"
    );
    let actual_prefix_receipts = overlay.batch_transfer_outcomes.clone();
    assert_eq!(actual_prefix_receipts.len(), 1);
    let mut full = TransactionResult::new(inner.clone());
    if prejoin_prefix {
        let rows = overlay.drain_batch_transfer_outcomes();
        full.set_batch_transfer_outcomes(rows[&entry.execution_call_hash()].clone());
    }
    if leftover_call {
        let mut extra = TransactionBuilder::new(
            state.network_id,
            fixture.source.account().clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        extra.set_creation_time(header.creation_time() - Duration::from_millis(1));
        let extra = TransactionEntrypoint::External(
            extra
                .with_instructions([ordinary_tail_batch(&fixture)])
                .sign(key.private_key()),
        );
        assert_ne!(extra.execution_call_hash(), entry.execution_call_hash());
        let accepted = crate::tx::AcceptedTransaction::accept_entrypoint_at_time(
            extra,
            &state.network_id,
            overlay.world.parameters().sumeragi().max_clock_drift(),
            overlay.world.parameters().transaction(),
            overlay.crypto.as_ref(),
            header.creation_time(),
        )
        .unwrap();
        let (_, result) = overlay.validate_transaction_with_entrypoint_index_and_routing_context(
            accepted,
            &mut crate::smartcontracts::ivm::cache::IvmCache::new(),
            1,
            route,
        );
        assert!(result.is_ok());
        let before = carrier.encode_wire().unwrap();
        assert!(
            crate::block::valid::finish_ordinary_tail_for_test(
                &mut carrier,
                &mut overlay,
                vec![full],
                &[route]
            )
            .is_err()
        );
        assert_eq!(carrier.encode_wire().unwrap(), before);
        drop(overlay);
        assert_eq!(
            state
                .world
                .assets
                .view()
                .get(&fixture.source)
                .unwrap()
                .as_ref(),
            &Quantity::from(100u32)
        );
        assert!(
            state
                .world
                .assets
                .view()
                .get(&fixture.destination)
                .is_none()
        );
        return;
    }
    crate::block::valid::finish_ordinary_tail_for_test(
        &mut carrier,
        &mut overlay,
        vec![full],
        &[route],
    )
    .unwrap();
    assert_eq!(carrier.hash(), proposal_hash);
    assert_eq!(
        carrier.header().merkle_root(),
        unexecuted.header().merkle_root()
    );
    let expected_count = if repeated_time { 3 } else { 1 };
    let results = carrier.results().cloned().collect::<Vec<_>>();
    assert_eq!(results.len(), expected_count);
    for result in &results {
        assert!(result.as_ref().is_ok());
        let receipts = result.batch_transfer_outcomes();
        assert_eq!(receipts.len(), 2);
        assert_eq!(receipts[0].leg_id, "applied");
        assert_eq!(receipts[1].leg_id, "rejected");
        assert!(matches!(
            receipts[0].status,
            AssetBatchTransferLegStatus::Applied
        ));
        assert!(matches!(
            receipts[1].status,
            AssetBatchTransferLegStatus::Rejected(_)
        ));
    }
    assert_eq!(
        results[0].batch_transfer_outcomes(),
        actual_prefix_receipts[&entry.execution_call_hash()]
    );
    let amount = u32::try_from(expected_count).unwrap() * 3;
    assert_eq!(
        overlay.world.assets.get(&fixture.source).unwrap().as_ref(),
        &Quantity::from(100u32 - amount)
    );
    assert_eq!(
        overlay
            .world
            .assets
            .get(&fixture.destination)
            .unwrap()
            .as_ref(),
        &Quantity::from(amount)
    );
    assert!(overlay.batch_transfer_outcomes.is_empty());
    assert_eq!(
        carrier.committed_fragment_count(),
        Some(overlay.committed_fragment_count() as u64)
    );
    let inventory = overlay.fastpq_source_inventory().unwrap().unwrap();
    let calls = inventory
        .entries()
        .iter()
        .map(|source| source.entry_hash)
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), expected_count);
    assert_eq!(calls[0], prefix_call);
    assert_eq!(carrier.fastpq_transcripts().len(), expected_count);
    assert_eq!(
        calls.iter().copied().collect::<BTreeSet<_>>().len(),
        expected_count
    );
    for call in &calls {
        assert!(carrier.fastpq_transcripts().contains_key(call));
        assert!(
            overlay
                .captured_fastpq_transcript_sources()
                .unwrap()
                .contains_key(call)
        );
    }
    if repeated_time {
        let time = carrier.time_triggers().collect::<Vec<_>>();
        assert_eq!(time.len(), 2);
        assert_eq!(
            time[0], time[1],
            "two real invocations have equal display entries"
        );
        let display = Hash::from(time[0].hash_as_entrypoint());
        assert_ne!(calls[1], calls[2]);
        assert_ne!(calls[1], display);
        assert_ne!(calls[2], display);
        // Positional proofs retain both equal-display result leaves.
        for index in [1, 2] {
            assert!(carrier.entrypoint_proof(index).unwrap().verify(
                &time[0].hash_as_entrypoint(),
                &carrier.full_entry_merkle_commitment().unwrap()
            ));
            assert!(carrier.result_proof(index).unwrap().verify(
                &results[index as usize].hash(),
                &carrier.result_merkle_commitment().unwrap()
            ));
        }
    } else {
        // Construct the old no-Time output from the same actual execution facts,
        // without rerunning economics. This proves byte stability of the shared
        // full-output setter versus the old inner-results + receipt setter path.
        let mut previous_output = unexecuted;
        previous_output
            .set_transaction_results_with_transcripts(
                Vec::new(),
                &[entry.hash()],
                vec![inner],
                carrier.fastpq_transcripts().clone(),
                carrier.axt_envelopes().unwrap().to_vec(),
                carrier.axt_policy_snapshot().unwrap().clone(),
            )
            .unwrap();
        previous_output
            .set_axt_transitioned_dataspaces(carrier.axt_transitioned_dataspaces().unwrap().clone())
            .unwrap();
        previous_output.set_trigger_completions(carrier.trigger_completions().unwrap().to_vec());
        previous_output
            .set_batch_transfer_outcomes(actual_prefix_receipts)
            .unwrap();
        previous_output.set_committed_fragment_count(carrier.committed_fragment_count().unwrap());
        assert_eq!(
            previous_output.encode_wire().unwrap(),
            carrier.encode_wire().unwrap()
        );
    }
    let encoded = carrier.encode_wire().unwrap();
    let decoded = iroha_data_model::block::decode_framed_signed_block(&encoded).unwrap();
    assert_eq!(decoded.encode_wire().unwrap(), encoded);
    assert_eq!(decoded.results().cloned().collect::<Vec<_>>(), results);
    drop(overlay);
    assert_eq!(
        state
            .world
            .assets
            .view()
            .get(&fixture.source)
            .unwrap()
            .as_ref(),
        &Quantity::from(100u32)
    );
    assert!(
        state
            .world
            .assets
            .view()
            .get(&fixture.destination)
            .is_none()
    );
}

#[test]
fn common_ordinary_tail_records_repeated_time_independent_batch_receipts() {
    run_ordinary_tail_independent_batches(true, false, false);
}

#[test]
fn common_ordinary_tail_preserves_actual_prejoined_prefix_receipts() {
    run_ordinary_tail_independent_batches(true, true, false);
}

#[test]
fn common_ordinary_tail_without_time_preserves_prior_canonical_output_bytes() {
    run_ordinary_tail_independent_batches(false, false, false);
}

#[test]
fn common_ordinary_tail_rejects_actual_leftover_receipt_owner_and_drops_all_writes() {
    run_ordinary_tail_independent_batches(false, false, true);
}
