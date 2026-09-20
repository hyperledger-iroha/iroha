// Actual common-driver regressions for direct and nested Pipeline receipt ownership.
// These overlays do not authorize native or State publication.

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
    transaction.set_creation_time(base.header().creation_time() - Duration::from_millis(1));
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
fn assert_pipeline_receipt_has_canonical_owner(nested: bool) {
    use iroha_data_model::block::execution_output::ExecutionOutputV1;
    use iroha_data_model::events::{
        data::prelude::AssetBatchTransferLegStatus, trigger_completed::TriggerCompletedOutcome,
    };
    let (fixture, parent, child) = pipeline_receipt_fixture(nested);
    let state = &fixture.native.state;
    let carrier = pipeline_receipt_carrier(&fixture);
    let before = crate::snapshot::canonical_state_snapshot_hash(state)
        .expect("stable valid fixture snapshot");
    let mut expected_wire = None;
    for _ in 0..2 {
        let mut attempt = carrier.clone();
        let mut overlay = pipeline_receipt_owned_block(state, attempt.header());
        crate::block::ValidBlock::execute_block_outputs_for_test(&mut attempt, &mut overlay, None)
            .unwrap();
        assert_eq!(attempt.execution_outputs().len(), 2);
        assert!(
            matches!(&attempt.execution_outputs()[0], ExecutionOutputV1::Network(row) if row.result.is_ok())
        );
        let output = &attempt.execution_outputs()[1];
        let ExecutionOutputV1::Pipeline(row) = output else {
            panic!("actual callback must own a Pipeline row")
        };
        assert_eq!(row.invocation.trigger.trigger_id, parent);
        assert!(row.result.is_ok(), "{:?}", row.result);
        let receipts = row.result.batch_transfer_outcomes();
        assert_eq!(receipts.len(), 2);
        assert!(matches!(
            receipts[0].status,
            AssetBatchTransferLegStatus::Applied
        ));
        assert!(matches!(
            receipts[1].status,
            AssetBatchTransferLegStatus::Rejected(_)
        ));
        let call = output
            .execution_call_hash(attempt.hash(), &attempt)
            .unwrap();
        assert_eq!(
            overlay.world.assets.get(&fixture.source).unwrap().0,
            Quantity::from(97u32)
        );
        assert_eq!(
            overlay.world.assets.get(&fixture.destination).unwrap().0,
            Quantity::from(3u32)
        );
        assert!(overlay.world.triggers.ids().get(&parent).is_none());
        assert_eq!(overlay.world.triggers.ids().get(&child).is_none(), nested);
        assert!(attempt.fastpq_transcripts().contains_key(&call));
        assert!(
            overlay
                .captured_fastpq_transcript_sources()
                .unwrap()
                .contains_key(&call)
        );
        assert!(overlay.batch_transfer_outcomes.is_empty());
        assert_eq!(row.completions.len(), if nested { 2 } else { 1 });
        assert!(
            row.completions
                .iter()
                .all(|completion| completion.outcome == TriggerCompletedOutcome::Success)
        );
        assert!(attempt.output_proof(1).unwrap().verify(
            &HashOf::new(output),
            &attempt.output_merkle_commitment().unwrap()
        ));
        overlay.verify_execution_output_seal(&attempt).unwrap();
        let wire = attempt.encode_wire().unwrap();
        if let Some(expected) = &expected_wire {
            assert_eq!(&wire, expected);
        } else {
            expected_wire = Some(wire);
        }
        drop(overlay);
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(state)
                .expect("stable valid fixture snapshot"),
            before
        );
        assert!(state.world.triggers.view().ids().get(&parent).is_some());
    }
}

state_test! { sync nested_pipeline_independent_batch_has_one_canonical_receipt_owner
    assert_pipeline_receipt_has_canonical_owner(true);
}

state_test! { sync direct_pipeline_independent_batch_has_one_canonical_receipt_owner
    assert_pipeline_receipt_has_canonical_owner(false);
}
