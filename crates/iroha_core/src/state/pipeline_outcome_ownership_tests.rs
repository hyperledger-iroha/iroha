// Executable counterexamples for the currently open internal-result owner gap.
// No native activation, synthetic invocation hash or fabricated source capture.

#[inline(never)]
fn pipeline_receipt_fixture(nested: bool) -> (Box<NativeEconomicFixture>, TriggerId, TriggerId) {
    use iroha_data_model::events::pipeline::{BlockEventFilter, BlockStatus};
    let fixture = native_economic_fixture(&[NativeEconomicCase::Transfer(25)], false);
    let parent: TriggerId = "pipeline_receipt_parent".parse().unwrap();
    let child: TriggerId = "pipeline_receipt_child".parse().unwrap();
    let mut metadata = iroha_model_base::metadata::Metadata::default();
    metadata.insert(
        "__registered_block_height".parse::<Name>().unwrap(),
        Json::new(0u64),
    );
    let child_trigger = Trigger::new(
        child.clone(),
        Action::new(
            [InstructionBox::from(ordinary_tail_batch(&fixture))],
            Repeats::Exactly(1),
            fixture.source.account().clone(),
            ExecuteTriggerEventFilter::new()
                .for_trigger(child.clone())
                .under_authority(fixture.source.account().clone()),
        )
        .unwrap()
        .with_metadata(metadata.clone()),
    );
    let body = if nested {
        vec![InstructionBox::from(ExecuteTrigger::new(child.clone()))]
    } else {
        vec![InstructionBox::from(ordinary_tail_batch(&fixture))]
    };
    let parent_trigger = Trigger::new(
        parent.clone(),
        Action::new(
            body,
            Repeats::Exactly(1),
            fixture.source.account().clone(),
            PipelineEventFilterBox::from(BlockEventFilter::new().for_status(BlockStatus::Approved)),
        )
        .unwrap()
        .with_metadata(metadata),
    );
    // Use the actual typed trigger store, as existing pipeline/Time fixtures do.
    // Register before the applying block so its use-time incarnation guard holds.
    let mut block = fixture.native.state.world.triggers.block();
    let mut transaction = block.transaction();
    assert!(
        transaction
            .add_by_call_trigger(child_trigger.try_into().unwrap())
            .unwrap()
    );
    assert!(
        transaction
            .add_pipeline_trigger(parent_trigger.try_into().unwrap())
            .unwrap()
    );
    transaction.apply();
    block.commit();
    (fixture, parent, child)
}

#[inline(never)]
fn pipeline_receipt_owned_block(state: &State, header: BlockHeader) -> Box<StateBlock<'_>> {
    let (block, ()) = state
        .block_with_owned_start_stages(
            header,
            |_| Ok::<(), std::convert::Infallible>(()),
            |_, ()| Ok(()),
        )
        .unwrap();
    block
}

fn pipeline_receipt_carrier(fixture: &NativeEconomicFixture) -> SignedBlock {
    let base = empty_global_block_after(Some(&fixture.native.block));
    let key = KeyPair::try_from_seed(vec![0x71; 32], Algorithm::Ed25519).unwrap();
    let mut transaction = TransactionBuilder::new(
        fixture.native.state.network_id,
        fixture.source.account().clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    transaction.set_creation_time(base.header().creation_time());
    let signed = transaction
        .with_instructions([Log::new(
            Level::INFO,
            "pipeline receipt owner probe".to_owned(),
        )])
        .sign(key.private_key());
    let mut builder = iroha_data_model::block::builder::BlockBuilder::new(base.header());
    builder.push_transaction(signed);
    let mut block = builder
        .build(BTreeSet::new())
        .canonical_resultless_proposal();
    block.set_da_proof_policies(Some(crate::da::active_proof_policy_bundle_at_height(
        &fixture.native.state.nexus_snapshot(),
        block.header().height().get(),
    )));
    block
}

#[inline(never)]
fn pipeline_receipt_execute_prefix(
    overlay: &mut StateBlock<'_>,
    block: &SignedBlock,
) -> iroha_data_model::transaction::signed::TransactionResult {
    let entry = block.external_entrypoints_slice()[0].clone();
    let accepted = crate::tx::AcceptedTransaction::accept_entrypoint_at_time(
        entry.clone(),
        &overlay.network_id,
        overlay.world.parameters().sumeragi().max_clock_drift(),
        overlay.world.parameters().transaction(),
        overlay.crypto.as_ref(),
        block.header().creation_time(),
    )
    .unwrap();
    let (hash, result) = overlay.validate_transaction_with_entrypoint_index_and_routing_context(
        accepted,
        &mut crate::smartcontracts::ivm::cache::IvmCache::new(),
        0,
        crate::queue::RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
    );
    assert_eq!(hash, entry.hash());
    assert!(result.is_ok(), "actual signed ordinary prefix: {result:?}");
    assert!(overlay.batch_transfer_outcomes.is_empty());
    result.into()
}

#[inline(never)]
fn pipeline_receipt_probe(
    fixture: &NativeEconomicFixture,
    carrier: &SignedBlock,
    parent: &TriggerId,
    child: &TriggerId,
) -> HashOf<TransactionEntrypoint> {
    use iroha_data_model::events::data::prelude::AssetBatchTransferLegStatus;
    use iroha_data_model::events::pipeline::{BlockEvent, BlockStatus, PipelineEventBox};
    use iroha_data_model::events::trigger_completed::TriggerCompletedOutcome;
    let _guard = crate::sumeragi::witness::exec_witness_guard();
    crate::sumeragi::witness::start_block();
    let header = carrier.header();
    let mut overlay = pipeline_receipt_owned_block(&fixture.native.state, header);
    let _prefix = pipeline_receipt_execute_prefix(&mut overlay, carrier);
    let fragments_before = overlay.committed_fragment_count();
    let outcomes =
        overlay.execute_pipeline_triggers_isolated([PipelineEventBox::from(BlockEvent {
            header,
            status: BlockStatus::Approved,
        })]);
    assert_eq!(outcomes.len(), 1);
    assert_eq!(&outcomes[0].0, parent);
    assert!(
        outcomes[0].1.is_ok(),
        "healthy nested callback: {:?}",
        outcomes[0].1
    );
    assert_eq!(
        overlay.committed_fragment_count(),
        fragments_before + 1,
        "actual callback applied exactly once"
    );
    assert_eq!(
        overlay.world.assets.get(&fixture.source).unwrap().0,
        Quantity::from(97u32)
    );
    assert_eq!(
        overlay.world.assets.get(&fixture.destination).unwrap().0,
        Quantity::from(3u32)
    );
    assert!(overlay.world.triggers.ids().get(parent).is_none());
    assert!(overlay.world.triggers.ids().get(child).is_none());
    assert_eq!(overlay.batch_transfer_outcomes.len(), 1);
    let (&call, receipts) = overlay.batch_transfer_outcomes.iter().next().unwrap();
    assert_eq!(receipts.len(), 2);
    assert_eq!(receipts[0].leg_id, "applied");
    assert!(matches!(
        receipts[0].status,
        AssetBatchTransferLegStatus::Applied
    ));
    assert_eq!(receipts[1].leg_id, "rejected");
    assert!(matches!(
        receipts[1].status,
        AssetBatchTransferLegStatus::Rejected(_)
    ));
    assert_eq!(overlay.fastpq_transcripts.len(), 1);
    assert!(overlay.fastpq_transcripts.contains_key(&Hash::from(call)));
    assert!(
        overlay
            .captured_fastpq_transcript_sources()
            .unwrap()
            .contains_key(&Hash::from(call))
    );

    let completions = overlay.world.trigger_completions();
    assert_eq!(completions.len(), 2);
    assert!(
        completions
            .iter()
            .all(|event| event.outcome() == &TriggerCompletedOutcome::Success)
    );
    assert!(completions.iter().any(|event| event.trigger_id() == parent));
    assert!(completions.iter().any(|event| event.trigger_id() == child));
    assert!(
        completions
            .iter()
            .all(|event| *event.trigger_execution_hash() != call),
        "completion display hashes are not the real receipt owner"
    );

    // Applied captures already support this internal execution call. The failure
    // below is result/receipt ownership, not absence of a real FASTPQ source.
    let tx_set = iroha_data_model::nexus::axt_ordered_transaction_set_digest_v1(
        carrier.external_entrypoints_slice().iter(),
    )
    .unwrap();
    overlay.set_fastpq_tx_set_hash(tx_set.into());
    overlay
        .finalize_fastpq_source_inventory(
            carrier.external_entrypoints_slice(),
            &[crate::queue::RoutingDecision::new(
                LaneId::SINGLE,
                DataSpaceId::UNIVERSAL,
            )],
            &[],
        )
        .unwrap();
    let inventory = overlay.fastpq_source_inventory().unwrap().unwrap();
    assert_eq!(inventory.entries().len(), 2);
    assert_eq!(
        inventory.entries()[0].entry_hash,
        Hash::from(carrier.external_entrypoints_slice()[0].execution_call_hash())
    );
    assert_eq!(inventory.entries()[1].entry_hash, Hash::from(call));
    drop(overlay);
    let _ = crate::sumeragi::witness::drain_exec_witness_checked(|_| Ok(())).unwrap();
    call
}

state_test! { sync nested_pipeline_independent_batch_applies_but_common_tail_has_no_result_owner
    use iroha_data_model::events::trigger_completed::TriggerCompletedOutcome;
    let (fixture, parent, child) = pipeline_receipt_fixture(true);
    let state = &fixture.native.state;
    let carrier = pipeline_receipt_carrier(&fixture);
    assert_eq!(carrier.external_entrypoints_slice().len(), 1);
    assert!(carrier.execution_context().is_none_or(|bundle| bundle.native_lane_decisions.is_none()));
    let before = crate::snapshot::canonical_state_snapshot_hash(state);
    let actual_call = pipeline_receipt_probe(&fixture, &carrier, &parent, &child);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state), before);
    let unexecuted = carrier.encode_wire().unwrap();
    for _attempt in 0..2 {
        let _guard = crate::sumeragi::witness::exec_witness_guard();
        crate::sumeragi::witness::start_block();
        let mut attempt = carrier.clone();
        let mut overlay = pipeline_receipt_owned_block(state, attempt.header());
        let prefix = pipeline_receipt_execute_prefix(&mut overlay, &attempt);
        let fragments_before = overlay.committed_fragment_count();
        let error = crate::block::valid::finish_ordinary_tail_for_test(
            &mut attempt, &mut overlay, vec![prefix],
            &[crate::queue::RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)],
        ).expect_err("current tail has no canonical result slot for the actual nested call");
        assert!(matches!(error, crate::block::BlockValidationError::MerkleRootMismatch), "{error:?}");
        // These assertions place the refusal strictly AFTER healthy callback
        // apply, not in setup, admission, permission, gas or source capture.
        assert_eq!(overlay.committed_fragment_count(), fragments_before + 1);
        assert_eq!(overlay.world.assets.get(&fixture.source).unwrap().0, Quantity::from(97u32));
        assert_eq!(overlay.world.assets.get(&fixture.destination).unwrap().0, Quantity::from(3u32));
        assert!(overlay.world.triggers.ids().get(&parent).is_none());
        assert!(overlay.world.triggers.ids().get(&child).is_none());
        assert!(overlay.fastpq_transcripts.contains_key(&Hash::from(actual_call)));
        let completions = overlay.world.trigger_completions();
        assert_eq!(completions.len(), 2);
        assert!(completions.iter().all(|event| event.outcome() == &TriggerCompletedOutcome::Success));
        assert_eq!(attempt.encode_wire().unwrap(), unexecuted, "no partial canonical result publication");
        drop(overlay);
        let _ = crate::sumeragi::witness::drain_exec_witness_checked(|_| Ok(())).unwrap();
        assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state), before);
        assert!(state.world.triggers.view().ids().get(&parent).is_some(),
            "refusal leaves the same callback eligible on the next attempted block");
    }
}

state_test! { sync direct_pipeline_independent_batch_is_quarantined_before_any_call_owned_output
    use iroha_data_model::events::pipeline::{BlockEvent, BlockStatus, PipelineEventBox};
    use iroha_data_model::events::trigger_completed::TriggerCompletedOutcome;
    let (fixture, parent, child) = pipeline_receipt_fixture(false);
    let state = &fixture.native.state;
    let header = empty_global_block_after(Some(&fixture.native.block)).header();
    let before = crate::snapshot::canonical_state_snapshot_hash(state);
    let _guard = crate::sumeragi::witness::exec_witness_guard();
    crate::sumeragi::witness::start_block();
    let mut overlay = pipeline_receipt_owned_block(state, header);
    let outcomes = overlay.execute_pipeline_triggers_isolated([
        PipelineEventBox::from(BlockEvent { header, status: BlockStatus::Approved }),
    ]);
    assert_eq!(outcomes.len(), 1);
    assert_eq!(&outcomes[0].0, &parent);
    let error = outcomes[0].1.as_ref().expect_err("direct pipeline body has no call identity");
    assert!(matches!(error,
        TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(
            iroha_data_model::isi::error::InstructionExecutionError::InvariantViolation(message)
        )) if message.as_ref() == "independent asset transfer batch requires a transaction call_hash before balance or transcript mutation"
    ), "expected the exact typed missing-call rejection, got {error:?}");
    assert_eq!(overlay.world.assets.get(&fixture.source).unwrap().0, Quantity::from(100u32));
    assert!(overlay.world.assets.get(&fixture.destination).is_none());
    assert!(overlay.batch_transfer_outcomes.is_empty());
    assert!(overlay.fastpq_transcripts.is_empty());
    assert!(overlay.captured_fastpq_transcript_sources().unwrap().is_empty());
    let retained = overlay.world.triggers.pipeline_triggers().get(&parent).unwrap();
    assert_eq!(retained.repeats(), &Repeats::Exactly(1));
    assert!(!crate::smartcontracts::isi::triggers::trigger_is_enabled(retained.metadata()));
    assert!(overlay.world.triggers.active_pipeline_trigger_ids().get(&parent).is_none());
    assert!(overlay.world.triggers.ids().get(&child).is_some(), "uncalled child stays intact");
    let completions = overlay.world.trigger_completions();
    assert_eq!(completions.len(), 1);
    assert!(matches!(completions[0].outcome(), TriggerCompletedOutcome::Failure(message)
        if message == &error.to_string()),
        "the retained completion uses the actual public rejection display");
    drop(overlay);
    let _ = crate::sumeragi::witness::drain_exec_witness_checked(|_| Ok(())).unwrap();
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state), before);
}
