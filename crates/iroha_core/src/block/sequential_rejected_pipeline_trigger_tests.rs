// Rejected-transaction pipeline-trigger coverage for sequential entrypoints.
#[test]
fn block_validation_sequential_entrypoints_execute_rejected_transaction_pipeline_trigger() {
    let chain_id = ChainId::from("sequential-rejected-pipeline-trigger");
    let network_id = deterministic_test_network_id(0x0F);
    let (authority, keypair) = gen_account_in("wonderland");
    let domain_id = DomainId::try_new("wonderland", "universal").expect("valid domain");
    let domain = Domain::new(domain_id.clone()).build(&authority);
    let account = Account::new(authority.clone()).build(&authority);
    let mut world = World::with([domain], [account], []);
    let block_key =
        Name::from_str("sequential_rejected_block_pipeline_trigger").expect("metadata key");
    let wrong_block_status_key =
        Name::from_str("sequential_wrong_committed_block_pipeline_trigger")
            .expect("metadata key");
    let rejected_key =
        Name::from_str("sequential_rejected_tx_pipeline_trigger").expect("metadata key");
    let approved_key =
        Name::from_str("sequential_wrong_approved_tx_pipeline_trigger").expect("metadata key");
    let wrong_rejected_key =
        Name::from_str("sequential_wrong_rejected_tx_pipeline_trigger").expect("metadata key");
    let wrong_hash_key = Name::from_str("sequential_wrong_hash_rejected_tx_pipeline_trigger")
        .expect("metadata key");
    let wrong_height_key =
        Name::from_str("sequential_wrong_height_rejected_tx_pipeline_trigger")
            .expect("metadata key");
    let wrong_lane_key = Name::from_str("sequential_wrong_lane_rejected_tx_pipeline_trigger")
        .expect("metadata key");
    let wrong_dataspace_key =
        Name::from_str("sequential_wrong_dataspace_rejected_tx_pipeline_trigger")
            .expect("metadata key");
    let external_signed = TransactionBuilder::new(
        network_id,
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Unregister::domain(
        DomainId::try_new("missing-domain", "universal").expect("valid domain id"),
    )])
    .sign(keypair.private_key());
    let external_hash = external_signed.hash();
    let wrong_hash: HashOf<SignedTransaction> =
        HashOf::from_untyped_unchecked(Hash::prehashed([0xE7; Hash::LENGTH]));
    let rejection = {
        let probe_domain = Domain::new(domain_id.clone()).build(&authority);
        let probe_account = Account::new(authority.clone()).build(&authority);
        let probe_state = State::try_new_with_chain_and_network_id_with_default_telemetry(
            World::with([probe_domain], [probe_account], []),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            chain_id.clone(),
            network_id,
        )
        .expect("probe state must accept its explicit network id");
        install_test_lane_manifests(&probe_state);
        let probe_block = BlockBuilder::new(vec![AcceptedTransaction::new_unchecked(
            Cow::Owned(external_signed.clone()),
        )])
        .chain(0, probe_state.view().latest_block().as_deref())
        .sign(keypair.private_key())
        .unpack(|_| {});
        let mut probe_state_block = probe_state.block(probe_block.header());
        let valid_probe = probe_block
            .validate_and_record_transactions(&mut probe_state_block)
            .unpack(|_| {});
        valid_probe
            .as_ref()
            .entrypoint_results()
            .next()
            .expect("probe result")
            .2
            .0
            .as_ref()
            .expect_err("probe transaction must reject")
            .clone()
    };
    add_pipeline_metadata_trigger(
        &mut world,
        &authority,
        "sequential_rejected_block_approved",
        block_key.clone(),
        PipelineEventFilterBox::from(BlockEventFilter::new().for_status(BlockStatus::Approved)),
    );
    add_pipeline_metadata_trigger(
        &mut world,
        &authority,
        "sequential_rejected_block_wrong_committed",
        wrong_block_status_key.clone(),
        PipelineEventFilterBox::from(
            BlockEventFilter::new().for_status(BlockStatus::Committed),
        ),
    );
    add_pipeline_metadata_trigger(
        &mut world,
        &authority,
        "sequential_external_rejected",
        rejected_key.clone(),
        PipelineEventFilterBox::from(
            TransactionEventFilter::new()
                .for_hash(external_hash)
                .for_status(TransactionStatus::Rejected(Box::new(rejection.clone()))),
        ),
    );
    add_pipeline_metadata_trigger(
        &mut world,
        &authority,
        "sequential_external_wrong_approved",
        approved_key.clone(),
        PipelineEventFilterBox::from(
            TransactionEventFilter::new()
                .for_hash(external_hash)
                .for_status(TransactionStatus::Approved),
        ),
    );
    add_pipeline_metadata_trigger(
        &mut world,
        &authority,
        "sequential_external_wrong_rejected",
        wrong_rejected_key.clone(),
        PipelineEventFilterBox::from(
            TransactionEventFilter::new()
                .for_hash(external_hash)
                .for_status(TransactionStatus::Rejected(Box::new(
                    TransactionRejectionReason::Validation(ValidationFail::TooComplex),
                ))),
        ),
    );
    add_pipeline_metadata_trigger(
        &mut world,
        &authority,
        "sequential_external_wrong_hash_rejected",
        wrong_hash_key.clone(),
        PipelineEventFilterBox::from(
            TransactionEventFilter::new()
                .for_hash(wrong_hash)
                .for_status(TransactionStatus::Rejected(Box::new(rejection.clone()))),
        ),
    );
    add_pipeline_metadata_trigger(
        &mut world,
        &authority,
        "sequential_external_wrong_height_rejected",
        wrong_height_key.clone(),
        PipelineEventFilterBox::from(
            TransactionEventFilter::new()
                .for_hash(external_hash)
                .for_block_height(Some(nonzero!(9999_u64)))
                .for_status(TransactionStatus::Rejected(Box::new(rejection.clone()))),
        ),
    );
    add_pipeline_metadata_trigger(
        &mut world,
        &authority,
        "sequential_external_wrong_lane_rejected",
        wrong_lane_key.clone(),
        PipelineEventFilterBox::from(
            TransactionEventFilter::new()
                .for_hash(external_hash)
                .for_lane_id(LaneId::new(7))
                .for_status(TransactionStatus::Rejected(Box::new(rejection.clone()))),
        ),
    );
    add_pipeline_metadata_trigger(
        &mut world,
        &authority,
        "sequential_external_wrong_dataspace_rejected",
        wrong_dataspace_key.clone(),
        PipelineEventFilterBox::from(
            TransactionEventFilter::new()
                .for_hash(external_hash)
                .for_dataspace_id(DataSpaceId::new(7))
                .for_status(TransactionStatus::Rejected(Box::new(rejection.clone()))),
        ),
    );
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::try_new_with_chain_and_network_id_with_default_telemetry(
        world,
        kura,
        query_handle,
        chain_id.clone(),
        network_id,
    )
    .expect("test state must accept its explicit network id");
    install_test_lane_manifests(&state);
    let metadata_key =
        Name::from_str("sequential_rejected_commitment_marker").expect("metadata key");
    let (commitment_entrypoint, _reveal_entrypoint) =
        sealed_set_key_entrypoints(state.network_id, &authority, &keypair, 2, 4, metadata_key);
    let accepted_external = AcceptedTransaction::new_unchecked(Cow::Owned(external_signed));
    let accepted_commitment =
        AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(commitment_entrypoint));
    let block = BlockBuilder::new(vec![accepted_external, accepted_commitment])
        .chain(0, state.view().latest_block().as_deref())
        .sign(keypair.private_key())
        .unpack(|_| {});
    assert_ne!(block.header().height(), nonzero!(9999_u64));
    let mut state_block = state.block(block.header());
    let valid_block = block
        .validate_and_record_transactions(&mut state_block)
        .unpack(|_| {});
    let results: Vec<_> = valid_block.as_ref().entrypoint_results().collect();
    assert!(
        results.iter().any(|(_, _, result)| result.0.is_err()),
        "mixed sequential block should record the failing external transaction"
    );
    let (
        block_value,
        wrong_block_status_value,
        rejected_value,
        approved_value,
        wrong_rejected_value,
        wrong_hash_value,
        wrong_height_value,
        wrong_lane_value,
        wrong_dataspace_value,
    ) = state_block
        .world
        .map_account(&authority, |account| {
            (
                account.value().metadata().get(&block_key).cloned(),
                account
                    .value()
                    .metadata()
                    .get(&wrong_block_status_key)
                    .cloned(),
                account.value().metadata().get(&rejected_key).cloned(),
                account.value().metadata().get(&approved_key).cloned(),
                account.value().metadata().get(&wrong_rejected_key).cloned(),
                account.value().metadata().get(&wrong_hash_key).cloned(),
                account.value().metadata().get(&wrong_height_key).cloned(),
                account.value().metadata().get(&wrong_lane_key).cloned(),
                account
                    .value()
                    .metadata()
                    .get(&wrong_dataspace_key)
                    .cloned(),
            )
        })
        .expect("authority account exists");
    assert_eq!(block_value, Some(Json::new("ok")));
    assert_eq!(
        wrong_block_status_value, None,
        "approved sequential block must not match a committed block filter"
    );
    assert_eq!(rejected_value, Some(Json::new("ok")));
    assert_eq!(
        approved_value, None,
        "rejected transaction must not match approved transaction filters"
    );
    assert_eq!(
        wrong_rejected_value, None,
        "rejected transaction must not match a different rejection reason"
    );
    assert_eq!(
        wrong_hash_value, None,
        "rejected transaction must not match a different hash"
    );
    assert_eq!(
        wrong_height_value, None,
        "rejected transaction must not match a different block height"
    );
    assert_eq!(
        wrong_lane_value, None,
        "rejected transaction must not match a different lane"
    );
    assert_eq!(
        wrong_dataspace_value, None,
        "rejected transaction must not match a different dataspace"
    );
}
