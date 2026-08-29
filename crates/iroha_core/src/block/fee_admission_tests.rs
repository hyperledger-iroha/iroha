#[test]
fn fee_enabled_single_transfer_uses_detached_merge_without_fee_fallback() {
    let _guard = crate::sumeragi::status::nexus_fee_test_lock()
        .lock()
        .expect("nexus fee test lock");
    crate::sumeragi::status::reset_nexus_economics_for_tests();
    crate::sumeragi::status::reset_rbc_backlog_stats_for_tests();
    let chain_id = ChainId::from("fee-detached-single-transfer-test");
    let (payer_id, payer_keypair) = gen_account_in("wonderland");
    let (recipient_id, _recipient_keypair) = gen_account_in("wonderland");
    let (sink_id, _sink_keypair) = gen_account_in("wonderland");
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let domain = Domain::new(domain_id.clone()).build(&payer_id);
    let payer = Account::new(payer_id.clone()).build(&payer_id);
    let recipient = Account::new(recipient_id.clone()).build(&recipient_id);
    let sink = Account::new(sink_id.clone()).build(&sink_id);
    let transfer_asset_definition_id = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "rose".parse().expect("asset name"),
    );
    let fee_asset_definition_id =
        AssetDefinitionId::derive_from_components(domain_id, "xor".parse().expect("asset name"));
    let transfer_asset_definition = AssetDefinition::numeric(
        transfer_asset_definition_id.clone(),
        "rose".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&payer_id);
    let fee_asset_definition = AssetDefinition::numeric(
        fee_asset_definition_id.clone(),
        "xor".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&payer_id);
    let payer_transfer_asset = AssetId::of(transfer_asset_definition_id.clone(), payer_id.clone());
    let recipient_transfer_asset =
        AssetId::of(transfer_asset_definition_id.clone(), recipient_id.clone());
    let payer_fee_asset = AssetId::of(fee_asset_definition_id.clone(), payer_id.clone());
    let world = test_world_with_assets(
        [domain],
        [payer, recipient, sink],
        [transfer_asset_definition, fee_asset_definition],
        [
            Asset::new(payer_transfer_asset.clone(), Quantity::from(5_u32)),
            Asset::new(recipient_transfer_asset.clone(), Quantity::zero()),
            Asset::new(payer_fee_asset.clone(), Quantity::from(10_u32)),
        ],
        [],
    );
    let kura = Arc::new(Kura::blank_kura_for_testing());
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
    install_test_lane_manifests(&state);
    {
        let nexus = state.nexus.get_mut();
        nexus.fees.base_fee = Quantity::from(1_u32);
        nexus.fees.per_byte_fee = Quantity::zero();
        nexus.fees.per_instruction_fee = Quantity::zero();
        nexus.fees.per_gas_unit_fee = Quantity::zero();
        nexus.fees.fee_asset_id = fee_asset_definition_id.to_string();
        nexus.fees.fee_sink_account_id = sink_id.to_string();
    }
    let (max_clock_drift, tx_limits) = {
        let state_view = state.world.view();
        let params = state_view.parameters();
        (params.sumeragi().max_clock_drift(), params.transaction())
    };
    let leader = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (_leader_public, leader_private) = leader.into_parts();
    let latest_valid = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
        header.set_height(nonzero!(1_u64));
    });
    let latest_signed: SignedBlock = latest_valid.into();
    let fee_payment = iroha_data_model::transaction::FeePaymentIntent::authority(
        vec![iroha_data_model::transaction::FeeChargeLimit::new(
            iroha_data_model::transaction::FeeChargeKind::Nexus,
            fee_asset_definition_id.clone(),
            Quantity::from(1_u32),
        )],
        None,
    );
    let (_block_handle, block_time_source) = TimeSource::new_mock(Duration::from_millis(10));
    let tx = TransactionBuilder::new_with_time_source(
        state.network_id,
        payer_id.clone(),
        &block_time_source,
        fee_payment,
    )
    .with_instructions([Transfer::asset_quantity(
        payer_transfer_asset.clone(),
        1_u32,
        recipient_id.clone(),
    )])
    .sign(payer_keypair.private_key());
    let tx = AcceptedTransaction::accept_with_time_source(
        tx,
        &state.network_id,
        max_clock_drift,
        tx_limits,
        state.crypto().as_ref(),
        &block_time_source,
    )
    .expect("transaction should pass stateless admission");
    let unverified_block = BlockBuilder::new_with_time_source(vec![tx], block_time_source)
        .chain(1, Some(&latest_signed))
        .sign(payer_keypair.private_key())
        .unpack(|_| {});
    let mut state_block = state.block(unverified_block.header);
    let valid_block = unverified_block
        .validate_and_record_transactions(&mut state_block)
        .unpack(|_| {});
    let errors = valid_block
        .as_ref()
        .errors()
        .map(|(idx, error)| (idx, format!("{error:?}")))
        .collect::<Vec<_>>();
    assert!(
        errors.is_empty(),
        "fee-enabled transfer should be accepted: {errors:?}"
    );
    let snapshot = crate::sumeragi::status::snapshot();
    assert_eq!(snapshot.pipeline_execution.detached_merged_total, 1);
    assert_eq!(snapshot.pipeline_execution.detached_fallback_total, 0);
    assert_eq!(
        snapshot
            .pipeline_execution
            .detached_fallback_fee_postprocessing_total,
        0
    );
    let assets = state_block.world.assets();
    assert_eq!(
        assets.get(&payer_transfer_asset).expect("payer rose").0,
        Quantity::from(4_u32)
    );
    assert_eq!(
        assets
            .get(&recipient_transfer_asset)
            .expect("recipient rose")
            .0,
        Quantity::from(1_u32)
    );
    assert_eq!(
        assets.get(&payer_fee_asset).expect("payer xor").0,
        Quantity::from(9_u32)
    );
}
#[test]
fn fee_enabled_supported_non_transfer_uses_fee_postprocessing_fallback() {
    let _guard = crate::sumeragi::status::nexus_fee_test_lock()
        .lock()
        .expect("nexus fee test lock");
    crate::sumeragi::status::reset_nexus_economics_for_tests();
    crate::sumeragi::status::reset_rbc_backlog_stats_for_tests();
    let chain_id = ChainId::from("fee-detached-non-transfer-fallback-test");
    let (payer_id, payer_keypair) = gen_account_in("wonderland");
    let (sink_id, _sink_keypair) = gen_account_in("wonderland");
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let domain = Domain::new(domain_id.clone()).build(&payer_id);
    let payer = Account::new(payer_id.clone()).build(&payer_id);
    let sink = Account::new(sink_id.clone()).build(&sink_id);
    let fee_asset_definition_id = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "xor".parse().expect("asset name"),
    );
    let fee_asset_definition = AssetDefinition::numeric(
        fee_asset_definition_id.clone(),
        "xor".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&payer_id);
    let payer_fee_asset = AssetId::of(fee_asset_definition_id.clone(), payer_id.clone());
    let world = test_world_with_assets(
        [domain],
        [payer, sink],
        [fee_asset_definition],
        [Asset::new(payer_fee_asset.clone(), Quantity::from(10_u32))],
        [],
    );
    let kura = Arc::new(Kura::blank_kura_for_testing());
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
    install_test_lane_manifests(&state);
    {
        let nexus = state.nexus.get_mut();
        nexus.fees.base_fee = Quantity::from(1_u32);
        nexus.fees.per_byte_fee = Quantity::zero();
        nexus.fees.per_instruction_fee = Quantity::zero();
        nexus.fees.per_gas_unit_fee = Quantity::zero();
        nexus.fees.fee_asset_id = fee_asset_definition_id.to_string();
        nexus.fees.fee_sink_account_id = sink_id.to_string();
    }
    let (max_clock_drift, tx_limits) = {
        let state_view = state.world.view();
        let params = state_view.parameters();
        (params.sumeragi().max_clock_drift(), params.transaction())
    };
    let leader = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (_leader_public, leader_private) = leader.into_parts();
    let latest_valid = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
        header.set_height(nonzero!(1_u64));
    });
    let latest_signed: SignedBlock = latest_valid.into();
    let marker_key: Name = "fee_fallback_marker".parse().expect("metadata key");
    let fee_payment = iroha_data_model::transaction::FeePaymentIntent::authority(
        vec![iroha_data_model::transaction::FeeChargeLimit::new(
            iroha_data_model::transaction::FeeChargeKind::Nexus,
            fee_asset_definition_id.clone(),
            Quantity::from(1_u32),
        )],
        None,
    );
    let mut builder = TransactionBuilder::new(state.network_id, payer_id.clone(), fee_payment);
    builder.set_creation_time(Duration::from_millis(0));
    let tx = builder
        .with_instructions([SetKeyValue::account(
            payer_id.clone(),
            marker_key.clone(),
            Json::from(true),
        )])
        .sign(payer_keypair.private_key());
    let tx = accept_transaction_at_mock_time(
        tx,
        &state.network_id,
        max_clock_drift,
        tx_limits,
        state.crypto().as_ref(),
        Duration::from_millis(10),
    )
    .expect("transaction should pass stateless admission");
    let (_block_handle, block_time_source) = TimeSource::new_mock(Duration::from_millis(10));
    let unverified_block = BlockBuilder::new_with_time_source(vec![tx], block_time_source)
        .chain(1, Some(&latest_signed))
        .sign(payer_keypair.private_key())
        .unpack(|_| {});
    let mut state_block = state.block(unverified_block.header);
    let valid_block = unverified_block
        .validate_and_record_transactions(&mut state_block)
        .unpack(|_| {});
    assert!(
        valid_block.as_ref().errors().next().is_none(),
        "supported non-transfer fee transaction should be accepted through sequential fallback"
    );
    let snapshot = crate::sumeragi::status::snapshot();
    assert_eq!(snapshot.pipeline_execution.detached_merged_total, 0);
    assert_eq!(
        snapshot.pipeline_execution.detached_fallback_total, 1,
        "account metadata requires the live sequential authorization path"
    );
    assert_eq!(
        snapshot
            .pipeline_execution
            .detached_fallback_fee_postprocessing_total,
        0,
        "live authorization takes precedence over fee postprocessing as the fallback reason"
    );
    assert_eq!(
        snapshot
            .pipeline_execution
            .detached_fallback_unsupported_instruction_total,
        1,
        "metadata writes are deliberately unsupported by detached execution"
    );
    let assets = state_block.world.assets();
    assert_eq!(
        assets.get(&payer_fee_asset).expect("payer xor").0,
        Quantity::from(9_u32)
    );
    let marker_value = state_block
        .world
        .map_account(&payer_id, |account| {
            account.value().metadata().get(&marker_key).cloned()
        })
        .expect("payer account exists");
    assert_eq!(marker_value, Some(Json::from(true)));
}
#[test]
fn fee_enabled_single_transfer_rejects_without_partial_state_when_fee_missing() {
    let _guard = crate::sumeragi::status::nexus_fee_test_lock()
        .lock()
        .expect("nexus fee test lock");
    crate::sumeragi::status::reset_nexus_economics_for_tests();
    crate::sumeragi::status::reset_rbc_backlog_stats_for_tests();
    let chain_id = ChainId::from("fee-detached-insufficient-fee-test");
    let (payer_id, payer_keypair) = gen_account_in("wonderland");
    let (recipient_id, _recipient_keypair) = gen_account_in("wonderland");
    let (sink_id, _sink_keypair) = gen_account_in("wonderland");
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let domain = Domain::new(domain_id.clone()).build(&payer_id);
    let payer = Account::new(payer_id.clone()).build(&payer_id);
    let recipient = Account::new(recipient_id.clone()).build(&recipient_id);
    let sink = Account::new(sink_id.clone()).build(&sink_id);
    let transfer_asset_definition_id = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "rose".parse().expect("asset name"),
    );
    let fee_asset_definition_id =
        AssetDefinitionId::derive_from_components(domain_id, "xor".parse().expect("asset name"));
    let transfer_asset_definition = AssetDefinition::numeric(
        transfer_asset_definition_id.clone(),
        "rose".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&payer_id);
    let fee_asset_definition = AssetDefinition::numeric(
        fee_asset_definition_id.clone(),
        "xor".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&payer_id);
    let payer_transfer_asset = AssetId::of(transfer_asset_definition_id.clone(), payer_id.clone());
    let recipient_transfer_asset =
        AssetId::of(transfer_asset_definition_id.clone(), recipient_id.clone());
    let payer_fee_asset = AssetId::of(fee_asset_definition_id.clone(), payer_id.clone());
    let world = test_world_with_assets(
        [domain],
        [payer, recipient, sink],
        [transfer_asset_definition, fee_asset_definition],
        [
            Asset::new(payer_transfer_asset.clone(), Quantity::from(5_u32)),
            Asset::new(recipient_transfer_asset.clone(), Quantity::zero()),
            Asset::new(payer_fee_asset.clone(), Quantity::zero()),
        ],
        [],
    );
    let kura = Arc::new(Kura::blank_kura_for_testing());
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
    install_test_lane_manifests(&state);
    {
        let nexus = state.nexus.get_mut();
        nexus.fees.base_fee = Quantity::from(1_u32);
        nexus.fees.per_byte_fee = Quantity::zero();
        nexus.fees.per_instruction_fee = Quantity::zero();
        nexus.fees.per_gas_unit_fee = Quantity::zero();
        nexus.fees.fee_asset_id = fee_asset_definition_id.to_string();
        nexus.fees.fee_sink_account_id = sink_id.to_string();
    }
    let (max_clock_drift, tx_limits) = {
        let state_view = state.world.view();
        let params = state_view.parameters();
        (params.sumeragi().max_clock_drift(), params.transaction())
    };
    let leader = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (_leader_public, leader_private) = leader.into_parts();
    let latest_valid = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
        header.set_height(nonzero!(1_u64));
    });
    let latest_signed: SignedBlock = latest_valid.into();
    let mut builder = TransactionBuilder::new(
        state.network_id,
        payer_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    builder.set_creation_time(Duration::from_millis(0));
    let tx = builder
        .with_instructions([Transfer::asset_quantity(
            payer_transfer_asset.clone(),
            1_u32,
            recipient_id,
        )])
        .sign(payer_keypair.private_key());
    let tx = accept_transaction_at_mock_time(
        tx,
        &state.network_id,
        max_clock_drift,
        tx_limits,
        state.crypto().as_ref(),
        Duration::from_millis(10),
    )
    .expect("transaction should pass stateless admission");
    let (_block_handle, block_time_source) = TimeSource::new_mock(Duration::from_millis(10));
    let unverified_block = BlockBuilder::new_with_time_source(vec![tx], block_time_source)
        .chain(1, Some(&latest_signed))
        .sign(payer_keypair.private_key())
        .unpack(|_| {});
    let mut state_block = state.block(unverified_block.header);
    let valid_block = unverified_block
        .validate_and_record_transactions(&mut state_block)
        .unpack(|_| {});
    assert_eq!(
        valid_block.as_ref().errors().next().map(|(idx, _)| idx),
        Some(0),
        "insufficient fee must reject the transaction"
    );
    let assets = state_block.world.assets();
    assert_eq!(
        assets.get(&payer_transfer_asset).expect("payer rose").0,
        Quantity::from(5_u32),
        "business transfer must not leak when fee charging fails"
    );
    assert_eq!(
        assets
            .get(&recipient_transfer_asset)
            .expect("recipient rose")
            .0,
        Quantity::zero(),
        "recipient balance must remain unchanged when fee charging fails"
    );
    assert_eq!(
        assets.get(&payer_fee_asset).expect("payer xor").0,
        Quantity::zero(),
        "failed fee debit must not create a negative or partial fee state"
    );
}
#[test]
fn fee_enabled_single_transfer_with_active_data_trigger_uses_fee_fallback() {
    let _guard = crate::sumeragi::status::nexus_fee_test_lock()
        .lock()
        .expect("nexus fee test lock");
    crate::sumeragi::status::reset_nexus_economics_for_tests();
    crate::sumeragi::status::reset_rbc_backlog_stats_for_tests();
    let chain_id = ChainId::from("fee-detached-data-trigger-fallback-test");
    let (payer_id, payer_keypair) = gen_account_in("wonderland");
    let (recipient_id, _recipient_keypair) = gen_account_in("wonderland");
    let (sink_id, _sink_keypair) = gen_account_in("wonderland");
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let domain = Domain::new(domain_id.clone()).build(&payer_id);
    let payer = Account::new(payer_id.clone()).build(&payer_id);
    let recipient = Account::new(recipient_id.clone()).build(&recipient_id);
    let sink = Account::new(sink_id.clone()).build(&sink_id);
    let transfer_asset_definition_id = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "rose".parse().expect("asset name"),
    );
    let fee_asset_definition_id =
        AssetDefinitionId::derive_from_components(domain_id, "xor".parse().expect("asset name"));
    let transfer_asset_definition = AssetDefinition::numeric(
        transfer_asset_definition_id.clone(),
        "rose".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&payer_id);
    let fee_asset_definition = AssetDefinition::numeric(
        fee_asset_definition_id.clone(),
        "xor".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&payer_id);
    let payer_transfer_asset = AssetId::of(transfer_asset_definition_id.clone(), payer_id.clone());
    let recipient_transfer_asset =
        AssetId::of(transfer_asset_definition_id.clone(), recipient_id.clone());
    let payer_fee_asset = AssetId::of(fee_asset_definition_id.clone(), payer_id.clone());
    let world = test_world_with_assets(
        [domain],
        [payer, recipient, sink],
        [transfer_asset_definition, fee_asset_definition],
        [
            Asset::new(payer_transfer_asset.clone(), Quantity::from(5_u32)),
            Asset::new(recipient_transfer_asset.clone(), Quantity::zero()),
            Asset::new(payer_fee_asset.clone(), Quantity::from(10_u32)),
        ],
        [],
    );
    let kura = Arc::new(Kura::blank_kura_for_testing());
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
    install_test_lane_manifests(&state);
    {
        let nexus = state.nexus.get_mut();
        nexus.fees.base_fee = Quantity::from(1_u32);
        nexus.fees.per_byte_fee = Quantity::zero();
        nexus.fees.per_instruction_fee = Quantity::zero();
        nexus.fees.per_gas_unit_fee = Quantity::zero();
        nexus.fees.fee_asset_id = fee_asset_definition_id.to_string();
        nexus.fees.fee_sink_account_id = sink_id.to_string();
    }
    let trigger_marker_key: Name = "fee_trigger_marker".parse().expect("metadata key");
    let trigger_id: TriggerId = "fee_transfer_trigger_guard".parse().unwrap();
    let trigger = Trigger::new(
        trigger_id,
        Action::new(
            vec![InstructionBox::from(SetKeyValue::account(
                payer_id.clone(),
                trigger_marker_key.clone(),
                Json::from("triggered"),
            ))],
            Repeats::Exactly(1),
            payer_id.clone(),
            DataEventFilter::Asset(AssetEventFilter::new().for_asset(payer_transfer_asset.clone())),
        )
        .expect("trigger action fixture satisfies validation invariants"),
    );
    let leader = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (_leader_public, leader_private) = leader.into_parts();
    let setup_block = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
        header.set_height(nonzero!(1_u64));
    });
    let setup_signed: SignedBlock = setup_block.clone().into();
    {
        let mut setup_state_block = state.block(setup_block.as_ref().header());
        let mut setup_tx = setup_state_block.transaction();
        Register::trigger(trigger)
            .execute(&payer_id, &mut setup_tx)
            .expect("register data trigger");
        setup_tx.apply();
        setup_state_block
            .commit_world_overlay_for_testing()
            .expect("commit trigger setup");
    }
    let (max_clock_drift, tx_limits) = {
        let state_view = state.world.view();
        let params = state_view.parameters();
        (params.sumeragi().max_clock_drift(), params.transaction())
    };
    let fee_payment = iroha_data_model::transaction::FeePaymentIntent::authority(
        vec![iroha_data_model::transaction::FeeChargeLimit::new(
            iroha_data_model::transaction::FeeChargeKind::Nexus,
            fee_asset_definition_id.clone(),
            Quantity::from(1_u32),
        )],
        None,
    );
    let mut builder = TransactionBuilder::new(state.network_id, payer_id.clone(), fee_payment);
    builder.set_creation_time(Duration::from_millis(0));
    let tx = builder
        .with_instructions([Transfer::asset_quantity(
            payer_transfer_asset.clone(),
            1_u32,
            recipient_id.clone(),
        )])
        .sign(payer_keypair.private_key());
    let tx = accept_transaction_at_mock_time(
        tx,
        &state.network_id,
        max_clock_drift,
        tx_limits,
        state.crypto().as_ref(),
        Duration::from_millis(10),
    )
    .expect("transaction should pass stateless admission");
    let (_block_handle, block_time_source) = TimeSource::new_mock(Duration::from_millis(10));
    let unverified_block = BlockBuilder::new_with_time_source(vec![tx], block_time_source)
        .chain(1, Some(&setup_signed))
        .sign(payer_keypair.private_key())
        .unpack(|_| {});
    let mut state_block = state.block(unverified_block.header);
    let valid_block = unverified_block
        .validate_and_record_transactions(&mut state_block)
        .unpack(|_| {});
    assert!(
        valid_block.as_ref().errors().next().is_none(),
        "fee-enabled transfer with an active data trigger should be accepted through fallback"
    );
    let snapshot = crate::sumeragi::status::snapshot();
    assert_eq!(snapshot.pipeline_execution.detached_merged_total, 0);
    assert_eq!(snapshot.pipeline_execution.detached_fallback_total, 1);
    assert_eq!(
        snapshot
            .pipeline_execution
            .detached_fallback_fee_postprocessing_total,
        1
    );
    let assets = state_block.world.assets();
    assert_eq!(
        assets.get(&payer_transfer_asset).expect("payer rose").0,
        Quantity::from(4_u32)
    );
    assert_eq!(
        assets
            .get(&recipient_transfer_asset)
            .expect("recipient rose")
            .0,
        Quantity::from(1_u32)
    );
    assert_eq!(
        assets.get(&payer_fee_asset).expect("payer xor").0,
        Quantity::from(9_u32)
    );
    let marker_value = state_block
        .world
        .map_account(&payer_id, |account| {
            account.value().metadata().get(&trigger_marker_key).cloned()
        })
        .expect("payer account exists");
    assert_eq!(marker_value, Some(Json::from("triggered")));
}
#[test]
fn fee_enabled_single_transfer_rejects_without_partial_state_when_fee_asset_missing() {
    let _guard = crate::sumeragi::status::nexus_fee_test_lock()
        .lock()
        .expect("nexus fee test lock");
    crate::sumeragi::status::reset_nexus_economics_for_tests();
    crate::sumeragi::status::reset_rbc_backlog_stats_for_tests();
    let chain_id = ChainId::from("fee-detached-missing-fee-asset-test");
    let (payer_id, payer_keypair) = gen_account_in("wonderland");
    let (recipient_id, _recipient_keypair) = gen_account_in("wonderland");
    let (sink_id, _sink_keypair) = gen_account_in("wonderland");
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let domain = Domain::new(domain_id.clone()).build(&payer_id);
    let payer = Account::new(payer_id.clone()).build(&payer_id);
    let recipient = Account::new(recipient_id.clone()).build(&recipient_id);
    let sink = Account::new(sink_id.clone()).build(&sink_id);
    let transfer_asset_definition_id = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "rose".parse().expect("asset name"),
    );
    let fee_asset_definition_id =
        AssetDefinitionId::derive_from_components(domain_id, "xor".parse().expect("asset name"));
    let transfer_asset_definition = AssetDefinition::numeric(
        transfer_asset_definition_id.clone(),
        "rose".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&payer_id);
    let fee_asset_definition = AssetDefinition::numeric(
        fee_asset_definition_id.clone(),
        "xor".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&payer_id);
    let payer_transfer_asset = AssetId::of(transfer_asset_definition_id.clone(), payer_id.clone());
    let recipient_transfer_asset =
        AssetId::of(transfer_asset_definition_id.clone(), recipient_id.clone());
    let payer_fee_asset = AssetId::of(fee_asset_definition_id.clone(), payer_id.clone());
    let world = test_world_with_assets(
        [domain],
        [payer, recipient, sink],
        [transfer_asset_definition, fee_asset_definition],
        [
            Asset::new(payer_transfer_asset.clone(), Quantity::from(5_u32)),
            Asset::new(recipient_transfer_asset.clone(), Quantity::zero()),
        ],
        [],
    );
    let kura = Arc::new(Kura::blank_kura_for_testing());
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
    install_test_lane_manifests(&state);
    {
        let nexus = state.nexus.get_mut();
        nexus.fees.base_fee = Quantity::from(1_u32);
        nexus.fees.per_byte_fee = Quantity::zero();
        nexus.fees.per_instruction_fee = Quantity::zero();
        nexus.fees.per_gas_unit_fee = Quantity::zero();
        nexus.fees.fee_asset_id = fee_asset_definition_id.to_string();
        nexus.fees.fee_sink_account_id = sink_id.to_string();
    }
    let (max_clock_drift, tx_limits) = {
        let state_view = state.world.view();
        let params = state_view.parameters();
        (params.sumeragi().max_clock_drift(), params.transaction())
    };
    let leader = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (_leader_public, leader_private) = leader.into_parts();
    let latest_valid = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
        header.set_height(nonzero!(1_u64));
    });
    let latest_signed: SignedBlock = latest_valid.into();
    let fee_payment = iroha_data_model::transaction::FeePaymentIntent::authority(
        vec![iroha_data_model::transaction::FeeChargeLimit::new(
            iroha_data_model::transaction::FeeChargeKind::Nexus,
            fee_asset_definition_id.clone(),
            Quantity::from(1_u32),
        )],
        None,
    );
    let mut builder = TransactionBuilder::new(state.network_id, payer_id.clone(), fee_payment);
    builder.set_creation_time(Duration::from_millis(0));
    let tx = builder
        .with_instructions([Transfer::asset_quantity(
            payer_transfer_asset.clone(),
            1_u32,
            recipient_id,
        )])
        .sign(payer_keypair.private_key());
    let tx = accept_transaction_at_mock_time(
        tx,
        &state.network_id,
        max_clock_drift,
        tx_limits,
        state.crypto().as_ref(),
        Duration::from_millis(10),
    )
    .expect("transaction should pass stateless admission");
    let (_block_handle, block_time_source) = TimeSource::new_mock(Duration::from_millis(10));
    let unverified_block = BlockBuilder::new_with_time_source(vec![tx], block_time_source)
        .chain(1, Some(&latest_signed))
        .sign(payer_keypair.private_key())
        .unpack(|_| {});
    let mut state_block = state.block(unverified_block.header);
    let valid_block = unverified_block
        .validate_and_record_transactions(&mut state_block)
        .unpack(|_| {});
    assert_eq!(
        valid_block.as_ref().errors().next().map(|(idx, _)| idx),
        Some(0),
        "missing payer fee asset must reject the transaction"
    );
    let assets = state_block.world.assets();
    assert_eq!(
        assets.get(&payer_transfer_asset).expect("payer rose").0,
        Quantity::from(5_u32),
        "business transfer must not leak when fee asset lookup fails"
    );
    assert_eq!(
        assets
            .get(&recipient_transfer_asset)
            .expect("recipient rose")
            .0,
        Quantity::zero(),
        "recipient balance must remain unchanged when fee asset lookup fails"
    );
    assert!(
        assets.get(&payer_fee_asset).is_none(),
        "fee charging must not create the missing payer fee asset"
    );
}
#[test]
fn fee_enabled_transfer_fee_same_asset_rejects_without_partial_state() {
    let _guard = crate::sumeragi::status::nexus_fee_test_lock()
        .lock()
        .expect("nexus fee test lock");
    crate::sumeragi::status::reset_nexus_economics_for_tests();
    crate::sumeragi::status::reset_rbc_backlog_stats_for_tests();
    let chain_id = ChainId::from("fee-detached-same-asset-fee-test");
    let (payer_id, payer_keypair) = gen_account_in("wonderland");
    let (recipient_id, _recipient_keypair) = gen_account_in("wonderland");
    let (sink_id, _sink_keypair) = gen_account_in("wonderland");
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let domain = Domain::new(domain_id.clone()).build(&payer_id);
    let payer = Account::new(payer_id.clone()).build(&payer_id);
    let recipient = Account::new(recipient_id.clone()).build(&recipient_id);
    let sink = Account::new(sink_id.clone()).build(&sink_id);
    let asset_definition_id =
        AssetDefinitionId::derive_from_components(domain_id, "rose".parse().expect("asset name"));
    let asset_definition = AssetDefinition::numeric(
        asset_definition_id.clone(),
        "rose".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&payer_id);
    let payer_asset = AssetId::of(asset_definition_id.clone(), payer_id.clone());
    let recipient_asset = AssetId::of(asset_definition_id.clone(), recipient_id.clone());
    let world = test_world_with_assets(
        [domain],
        [payer, recipient, sink],
        [asset_definition],
        [
            Asset::new(payer_asset.clone(), Quantity::from(1_u32)),
            Asset::new(recipient_asset.clone(), Quantity::zero()),
        ],
        [],
    );
    let kura = Arc::new(Kura::blank_kura_for_testing());
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
    install_test_lane_manifests(&state);
    {
        let nexus = state.nexus.get_mut();
        nexus.fees.base_fee = Quantity::from(1_u32);
        nexus.fees.per_byte_fee = Quantity::zero();
        nexus.fees.per_instruction_fee = Quantity::zero();
        nexus.fees.per_gas_unit_fee = Quantity::zero();
        nexus.fees.fee_asset_id = asset_definition_id.to_string();
        nexus.fees.fee_sink_account_id = sink_id.to_string();
    }
    let (max_clock_drift, tx_limits) = {
        let state_view = state.world.view();
        let params = state_view.parameters();
        (params.sumeragi().max_clock_drift(), params.transaction())
    };
    let leader = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (_leader_public, leader_private) = leader.into_parts();
    let latest_valid = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
        header.set_height(nonzero!(1_u64));
    });
    let latest_signed: SignedBlock = latest_valid.into();
    let fee_payment = iroha_data_model::transaction::FeePaymentIntent::authority(
        vec![iroha_data_model::transaction::FeeChargeLimit::new(
            iroha_data_model::transaction::FeeChargeKind::Nexus,
            asset_definition_id.clone(),
            Quantity::from(1_u32),
        )],
        None,
    );
    let mut builder = TransactionBuilder::new(state.network_id, payer_id.clone(), fee_payment);
    builder.set_creation_time(Duration::from_millis(0));
    let tx = builder
        .with_instructions([Transfer::asset_quantity(
            payer_asset.clone(),
            1_u32,
            recipient_id,
        )])
        .sign(payer_keypair.private_key());
    let tx = accept_transaction_at_mock_time(
        tx,
        &state.network_id,
        max_clock_drift,
        tx_limits,
        state.crypto().as_ref(),
        Duration::from_millis(10),
    )
    .expect("transaction should pass stateless admission");
    let (_block_handle, block_time_source) = TimeSource::new_mock(Duration::from_millis(10));
    let unverified_block = BlockBuilder::new_with_time_source(vec![tx], block_time_source)
        .chain(1, Some(&latest_signed))
        .sign(payer_keypair.private_key())
        .unpack(|_| {});
    let mut state_block = state.block(unverified_block.header);
    let valid_block = unverified_block
        .validate_and_record_transactions(&mut state_block)
        .unpack(|_| {});
    assert_eq!(
        valid_block.as_ref().errors().next().map(|(idx, _)| idx),
        Some(0),
        "fee debit must reject when the payer only has enough balance for the transfer itself"
    );
    let snapshot = crate::sumeragi::status::snapshot();
    assert_eq!(snapshot.pipeline_execution.detached_merged_total, 0);
    assert_eq!(snapshot.pipeline_execution.detached_fallback_total, 1);
    let assets = state_block.world.assets();
    assert_eq!(
        assets.get(&payer_asset).expect("payer rose").0,
        Quantity::from(1_u32),
        "transfer must not leak when post-transfer fee debit fails"
    );
    assert_eq!(
        assets.get(&recipient_asset).expect("recipient rose").0,
        Quantity::zero(),
        "recipient must not receive funds from a transaction rejected during fee charging"
    );
}
#[test]
fn fee_enabled_shared_fee_balance_rejects_later_transfer_without_rolling_back_prior_success() {
    let _guard = crate::sumeragi::status::nexus_fee_test_lock()
        .lock()
        .expect("nexus fee test lock");
    crate::sumeragi::status::reset_nexus_economics_for_tests();
    crate::sumeragi::status::reset_rbc_backlog_stats_for_tests();
    let chain_id = ChainId::from("fee-detached-shared-fee-balance-test");
    let (payer_id, payer_keypair) = gen_account_in("wonderland");
    let (recipient_id, _recipient_keypair) = gen_account_in("wonderland");
    let (sink_id, _sink_keypair) = gen_account_in("wonderland");
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let domain = Domain::new(domain_id.clone()).build(&payer_id);
    let payer = Account::new(payer_id.clone()).build(&payer_id);
    let recipient = Account::new(recipient_id.clone()).build(&recipient_id);
    let sink = Account::new(sink_id.clone()).build(&sink_id);
    let transfer_asset_definition_id = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "rose".parse().expect("asset name"),
    );
    let fee_asset_definition_id =
        AssetDefinitionId::derive_from_components(domain_id, "xor".parse().expect("asset name"));
    let transfer_asset_definition = AssetDefinition::numeric(
        transfer_asset_definition_id.clone(),
        "rose".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&payer_id);
    let fee_asset_definition = AssetDefinition::numeric(
        fee_asset_definition_id.clone(),
        "xor".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&payer_id);
    let payer_transfer_asset = AssetId::of(transfer_asset_definition_id.clone(), payer_id.clone());
    let recipient_transfer_asset =
        AssetId::of(transfer_asset_definition_id.clone(), recipient_id.clone());
    let payer_fee_asset = AssetId::of(fee_asset_definition_id.clone(), payer_id.clone());
    let world = test_world_with_assets(
        [domain],
        [payer, recipient, sink],
        [transfer_asset_definition, fee_asset_definition],
        [
            Asset::new(payer_transfer_asset.clone(), Quantity::from(5_u32)),
            Asset::new(recipient_transfer_asset.clone(), Quantity::zero()),
            Asset::new(payer_fee_asset.clone(), Quantity::from(1_u32)),
        ],
        [],
    );
    let kura = Arc::new(Kura::blank_kura_for_testing());
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
    install_test_lane_manifests(&state);
    {
        let nexus = state.nexus.get_mut();
        nexus.fees.base_fee = Quantity::from(1_u32);
        nexus.fees.per_byte_fee = Quantity::zero();
        nexus.fees.per_instruction_fee = Quantity::zero();
        nexus.fees.per_gas_unit_fee = Quantity::zero();
        nexus.fees.fee_asset_id = fee_asset_definition_id.to_string();
        nexus.fees.fee_sink_account_id = sink_id.to_string();
    }
    let (max_clock_drift, tx_limits) = {
        let state_view = state.world.view();
        let params = state_view.parameters();
        (params.sumeragi().max_clock_drift(), params.transaction())
    };
    let leader = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (_leader_public, leader_private) = leader.into_parts();
    let latest_valid = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
        header.set_height(nonzero!(1_u64));
    });
    let latest_signed: SignedBlock = latest_valid.into();
    let fee_payment = iroha_data_model::transaction::FeePaymentIntent::authority(
        vec![iroha_data_model::transaction::FeeChargeLimit::new(
            iroha_data_model::transaction::FeeChargeKind::Nexus,
            fee_asset_definition_id.clone(),
            Quantity::from(1_u32),
        )],
        None,
    );
    let mut first_builder =
        TransactionBuilder::new(state.network_id, payer_id.clone(), fee_payment.clone());
    first_builder.set_creation_time(Duration::from_millis(0));
    let first_tx = first_builder
        .with_instructions([Transfer::asset_quantity(
            payer_transfer_asset.clone(),
            1_u32,
            recipient_id.clone(),
        )])
        .sign(payer_keypair.private_key());
    let first_tx = accept_transaction_at_mock_time(
        first_tx,
        &state.network_id,
        max_clock_drift,
        tx_limits,
        state.crypto().as_ref(),
        Duration::from_millis(10),
    )
    .expect("first transaction should pass stateless admission");
    let mut second_builder =
        TransactionBuilder::new(state.network_id, payer_id.clone(), fee_payment);
    second_builder.set_creation_time(Duration::from_millis(1));
    let second_tx = second_builder
        .with_instructions([Transfer::asset_quantity(
            payer_transfer_asset.clone(),
            1_u32,
            recipient_id,
        )])
        .sign(payer_keypair.private_key());
    let second_tx = accept_transaction_at_mock_time(
        second_tx,
        &state.network_id,
        max_clock_drift,
        tx_limits,
        state.crypto().as_ref(),
        Duration::from_millis(10),
    )
    .expect("second transaction should pass stateless admission");
    let (_block_handle, block_time_source) = TimeSource::new_mock(Duration::from_millis(10));
    let unverified_block =
        BlockBuilder::new_with_time_source(vec![first_tx, second_tx], block_time_source)
            .chain(1, Some(&latest_signed))
            .sign(payer_keypair.private_key())
            .unpack(|_| {});
    let mut state_block = state.block(unverified_block.header);
    let valid_block = unverified_block
        .validate_and_record_transactions(&mut state_block)
        .unpack(|_| {});
    assert_eq!(
        valid_block.as_ref().errors().count(),
        1,
        "only one of the two transfers can pay the configured base fee"
    );
    let snapshot = crate::sumeragi::status::snapshot();
    assert_eq!(
        snapshot.pipeline_execution.detached_merged_total, 1,
        "one transfer should stay on the detached merge path"
    );
    assert_eq!(
        snapshot.pipeline_execution.detached_fallback_total, 0,
        "signed fee admission must reject after the first debit drains the balance, before detached execution"
    );
    let assets = state_block.world.assets();
    assert_eq!(
        assets
            .get(&payer_transfer_asset)
            .expect("payer rose after block")
            .0,
        Quantity::from(4_u32),
        "the accepted transfer must remain committed"
    );
    assert_eq!(
        assets
            .get(&recipient_transfer_asset)
            .expect("recipient rose after block")
            .0,
        Quantity::from(1_u32),
        "the rejected transfer must not leak after the first fee drains the payer"
    );
    assert_eq!(
        assets
            .get(&payer_fee_asset)
            .map(|asset| asset.0.clone())
            .unwrap_or_else(Quantity::zero),
        Quantity::zero(),
        "only the accepted transaction may consume the available fee balance"
    );
}
#[test]
fn fee_enabled_transfer_then_failing_instruction_falls_back_without_leaking_transfer() {
    let _guard = crate::sumeragi::status::nexus_fee_test_lock()
        .lock()
        .expect("nexus fee test lock");
    crate::sumeragi::status::reset_nexus_economics_for_tests();
    crate::sumeragi::status::reset_rbc_backlog_stats_for_tests();
    let chain_id = ChainId::from("fee-detached-transfer-then-fail-test");
    let (payer_id, payer_keypair) = gen_account_in("wonderland");
    let (recipient_id, _recipient_keypair) = gen_account_in("wonderland");
    let (sink_id, _sink_keypair) = gen_account_in("wonderland");
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let domain = Domain::new(domain_id.clone()).build(&payer_id);
    let payer = Account::new(payer_id.clone()).build(&payer_id);
    let recipient = Account::new(recipient_id.clone()).build(&recipient_id);
    let sink = Account::new(sink_id.clone()).build(&sink_id);
    let transfer_asset_definition_id = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "rose".parse().expect("asset name"),
    );
    let fee_asset_definition_id =
        AssetDefinitionId::derive_from_components(domain_id, "xor".parse().expect("asset name"));
    let transfer_asset_definition = AssetDefinition::numeric(
        transfer_asset_definition_id.clone(),
        "rose".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&payer_id);
    let fee_asset_definition = AssetDefinition::numeric(
        fee_asset_definition_id.clone(),
        "xor".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&payer_id);
    let payer_transfer_asset = AssetId::of(transfer_asset_definition_id.clone(), payer_id.clone());
    let recipient_transfer_asset =
        AssetId::of(transfer_asset_definition_id.clone(), recipient_id.clone());
    let payer_fee_asset = AssetId::of(fee_asset_definition_id.clone(), payer_id.clone());
    let world = test_world_with_assets(
        [domain],
        [payer, recipient, sink],
        [transfer_asset_definition, fee_asset_definition],
        [
            Asset::new(payer_transfer_asset.clone(), Quantity::from(5_u32)),
            Asset::new(recipient_transfer_asset.clone(), Quantity::zero()),
            Asset::new(payer_fee_asset.clone(), Quantity::from(10_u32)),
        ],
        [],
    );
    let kura = Arc::new(Kura::blank_kura_for_testing());
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
    install_test_lane_manifests(&state);
    {
        let nexus = state.nexus.get_mut();
        nexus.fees.base_fee = Quantity::from(1_u32);
        nexus.fees.per_byte_fee = Quantity::zero();
        nexus.fees.per_instruction_fee = Quantity::zero();
        nexus.fees.per_gas_unit_fee = Quantity::zero();
        nexus.fees.fee_asset_id = fee_asset_definition_id.to_string();
        nexus.fees.fee_sink_account_id = sink_id.to_string();
    }
    let (max_clock_drift, tx_limits) = {
        let state_view = state.world.view();
        let params = state_view.parameters();
        (params.sumeragi().max_clock_drift(), params.transaction())
    };
    let leader = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (_leader_public, leader_private) = leader.into_parts();
    let latest_valid = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
        header.set_height(nonzero!(1_u64));
    });
    let latest_signed: SignedBlock = latest_valid.into();
    let fee_payment = iroha_data_model::transaction::FeePaymentIntent::authority(
        vec![iroha_data_model::transaction::FeeChargeLimit::new(
            iroha_data_model::transaction::FeeChargeKind::Nexus,
            fee_asset_definition_id.clone(),
            Quantity::from(1_u32),
        )],
        None,
    );
    let mut builder = TransactionBuilder::new(state.network_id, payer_id.clone(), fee_payment);
    builder.set_creation_time(Duration::from_millis(0));
    let missing_domain_id = DomainId::try_new("missing-domain", "universal").unwrap();
    let tx = builder
        .with_instructions::<InstructionBox>([
            Transfer::asset_quantity(payer_transfer_asset.clone(), 1_u32, recipient_id).into(),
            Unregister::domain(missing_domain_id).into(),
        ])
        .sign(payer_keypair.private_key());
    let tx = accept_transaction_at_mock_time(
        tx,
        &state.network_id,
        max_clock_drift,
        tx_limits,
        state.crypto().as_ref(),
        Duration::from_millis(10),
    )
    .expect("transaction should pass stateless admission");
    let (_block_handle, block_time_source) = TimeSource::new_mock(Duration::from_millis(10));
    let unverified_block = BlockBuilder::new_with_time_source(vec![tx], block_time_source)
        .chain(1, Some(&latest_signed))
        .sign(payer_keypair.private_key())
        .unpack(|_| {});
    let mut state_block = state.block(unverified_block.header);
    let valid_block = unverified_block
        .validate_and_record_transactions(&mut state_block)
        .unpack(|_| {});
    assert_eq!(
        valid_block.as_ref().errors().next().map(|(idx, _)| idx),
        Some(0),
        "the failing instruction after the transfer must reject the whole transaction"
    );
    let snapshot = crate::sumeragi::status::snapshot();
    assert_eq!(snapshot.pipeline_execution.detached_merged_total, 0);
    assert_eq!(snapshot.pipeline_execution.detached_fallback_total, 1);
    assert_eq!(
        snapshot
            .pipeline_execution
            .detached_fallback_unsupported_instruction_total,
        1,
        "multi-instruction transfer transactions must not use detached transfer merge"
    );
    let assets = state_block.world.assets();
    assert_eq!(
        assets
            .get(&payer_transfer_asset)
            .expect("payer rose after rejected transfer")
            .0,
        Quantity::from(5_u32),
        "payer balance must remain unchanged after rejected transfer"
    );
    assert_eq!(
        assets
            .get(&recipient_transfer_asset)
            .expect("recipient rose after rejected transfer")
            .0,
        Quantity::zero(),
        "recipient must not receive assets from a transaction rejected after the transfer"
    );
    assert_eq!(
        assets
            .get(&payer_fee_asset)
            .expect("payer xor after rejected transfer")
            .0,
        Quantity::from(9_u32),
        "rejected business execution must still charge the configured Nexus fee"
    );
}
#[test]
fn fee_enabled_non_increasing_sequence_rejects_before_transfer_or_fee() {
    let _guard = crate::sumeragi::status::nexus_fee_test_lock()
        .lock()
        .expect("nexus fee test lock");
    crate::sumeragi::status::reset_nexus_economics_for_tests();
    crate::sumeragi::status::reset_rbc_backlog_stats_for_tests();
    let chain_id = ChainId::from("fee-detached-sequence-admission-test");
    let (payer_id, payer_keypair) = gen_account_in("wonderland");
    let (recipient_id, _recipient_keypair) = gen_account_in("wonderland");
    let (sink_id, _sink_keypair) = gen_account_in("wonderland");
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let domain = Domain::new(domain_id.clone()).build(&payer_id);
    let payer = Account::new(payer_id.clone()).build(&payer_id);
    let recipient = Account::new(recipient_id.clone()).build(&recipient_id);
    let sink = Account::new(sink_id.clone()).build(&sink_id);
    let transfer_asset_definition_id = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "rose".parse().expect("asset name"),
    );
    let fee_asset_definition_id =
        AssetDefinitionId::derive_from_components(domain_id, "xor".parse().expect("asset name"));
    let transfer_asset_definition = AssetDefinition::numeric(
        transfer_asset_definition_id.clone(),
        "rose".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&payer_id);
    let fee_asset_definition = AssetDefinition::numeric(
        fee_asset_definition_id.clone(),
        "xor".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&payer_id);
    let payer_transfer_asset = AssetId::of(transfer_asset_definition_id.clone(), payer_id.clone());
    let recipient_transfer_asset =
        AssetId::of(transfer_asset_definition_id.clone(), recipient_id.clone());
    let payer_fee_asset = AssetId::of(fee_asset_definition_id.clone(), payer_id.clone());
    let mut world = test_world_with_assets(
        [domain],
        [payer, recipient, sink],
        [transfer_asset_definition, fee_asset_definition],
        [
            Asset::new(payer_transfer_asset.clone(), Quantity::from(5_u32)),
            Asset::new(recipient_transfer_asset.clone(), Quantity::zero()),
            Asset::new(payer_fee_asset.clone(), Quantity::from(10_u32)),
        ],
        [],
    );
    let mut params = iroha_data_model::parameter::system::Parameters::default();
    params.transaction = params.transaction.with_ingress_enforcement(false, true);
    world.parameters = mv::cell::Cell::new(params);
    world.tx_sequences.insert(payer_id.clone(), 5);
    let kura = Arc::new(Kura::blank_kura_for_testing());
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
    install_test_lane_manifests(&state);
    {
        let nexus = state.nexus.get_mut();
        nexus.fees.base_fee = Quantity::from(1_u32);
        nexus.fees.per_byte_fee = Quantity::zero();
        nexus.fees.per_instruction_fee = Quantity::zero();
        nexus.fees.per_gas_unit_fee = Quantity::zero();
        nexus.fees.fee_asset_id = fee_asset_definition_id.to_string();
        nexus.fees.fee_sink_account_id = sink_id.to_string();
    }
    let (max_clock_drift, tx_limits) = {
        let state_view = state.world.view();
        let params = state_view.parameters();
        (params.sumeragi().max_clock_drift(), params.transaction())
    };
    let leader = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (_leader_public, leader_private) = leader.into_parts();
    let latest_valid = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
        header.set_height(nonzero!(1_u64));
    });
    let latest_signed: SignedBlock = latest_valid.into();
    let mut metadata = Metadata::default();
    metadata.insert(
        Name::from_str("tx_sequence").expect("metadata key"),
        Json::from(5_u64),
    );
    let mut builder = TransactionBuilder::new(
        state.network_id,
        payer_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    builder.set_creation_time(Duration::from_millis(0));
    let tx = builder
        .with_metadata(metadata)
        .with_instructions([Transfer::asset_quantity(
            payer_transfer_asset.clone(),
            1_u32,
            recipient_id,
        )])
        .sign(payer_keypair.private_key());
    let tx = accept_transaction_at_mock_time(
        tx,
        &state.network_id,
        max_clock_drift,
        tx_limits,
        state.crypto().as_ref(),
        Duration::from_millis(10),
    )
    .expect("transaction should pass stateless admission");
    let (_block_handle, block_time_source) = TimeSource::new_mock(Duration::from_millis(10));
    let unverified_block = BlockBuilder::new_with_time_source(vec![tx], block_time_source)
        .chain(1, Some(&latest_signed))
        .sign(payer_keypair.private_key())
        .unpack(|_| {});
    let mut state_block = state.block(unverified_block.header);
    let valid_block = unverified_block
        .validate_and_record_transactions(&mut state_block)
        .unpack(|_| {});
    assert_eq!(
        valid_block.as_ref().errors().next().map(|(idx, _)| idx),
        Some(0),
        "non-increasing tx_sequence must reject before transfer or fee application"
    );
    let snapshot = crate::sumeragi::status::snapshot();
    assert_eq!(snapshot.pipeline_execution.detached_merged_total, 0);
    assert_eq!(snapshot.pipeline_execution.detached_fallback_total, 0);
    let assets = state_block.world.assets();
    assert_eq!(
        assets
            .get(&payer_transfer_asset)
            .expect("payer rose after sequence rejection")
            .0,
        Quantity::from(5_u32)
    );
    assert_eq!(
        assets
            .get(&recipient_transfer_asset)
            .expect("recipient rose after sequence rejection")
            .0,
        Quantity::zero()
    );
    assert_eq!(
        assets
            .get(&payer_fee_asset)
            .expect("payer xor after sequence rejection")
            .0,
        Quantity::from(10_u32),
        "stateful admission failures must not charge Nexus fees"
    );
    assert_eq!(
        state_block.world.tx_sequences.get(&payer_id),
        Some(&5),
        "rejected sequence must not advance stored per-authority state"
    );
}
#[test]
fn legacy_fee_sponsor_metadata_rejects_before_block_admission_without_state_mutation() {
    let _guard = crate::sumeragi::status::nexus_fee_test_lock()
        .lock()
        .expect("nexus fee test lock");
    crate::sumeragi::status::reset_nexus_economics_for_tests();
    crate::sumeragi::status::reset_rbc_backlog_stats_for_tests();
    let chain_id = ChainId::from("legacy-fee-sponsor-metadata-default-fees-test");
    let (payer_id, payer_keypair) = gen_account_in("wonderland");
    let (sponsor_id, _sponsor_keypair) = gen_account_in("wonderland");
    let (recipient_id, _recipient_keypair) = gen_account_in("wonderland");
    let (sink_id, _sink_keypair) = gen_account_in("wonderland");
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let domain = Domain::new(domain_id.clone()).build(&payer_id);
    let payer = Account::new(payer_id.clone()).build(&payer_id);
    let sponsor = Account::new(sponsor_id.clone()).build(&sponsor_id);
    let recipient = Account::new(recipient_id.clone()).build(&recipient_id);
    let sink = Account::new(sink_id.clone()).build(&sink_id);
    let transfer_asset_definition_id = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "rose".parse().expect("asset name"),
    );
    let fee_asset_definition_id =
        AssetDefinitionId::derive_from_components(domain_id, "xor".parse().expect("asset name"));
    let transfer_asset_definition = AssetDefinition::numeric(
        transfer_asset_definition_id.clone(),
        "rose".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&payer_id);
    let fee_asset_definition = AssetDefinition::numeric(
        fee_asset_definition_id.clone(),
        "xor".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&payer_id);
    let payer_transfer_asset = AssetId::of(transfer_asset_definition_id.clone(), payer_id.clone());
    let recipient_transfer_asset =
        AssetId::of(transfer_asset_definition_id.clone(), recipient_id.clone());
    let sponsor_fee_asset = AssetId::of(fee_asset_definition_id.clone(), sponsor_id.clone());
    let world = test_world_with_assets(
        [domain],
        [payer, sponsor, recipient, sink],
        [transfer_asset_definition, fee_asset_definition],
        [
            Asset::new(payer_transfer_asset.clone(), Quantity::from(5_u32)),
            Asset::new(recipient_transfer_asset.clone(), Quantity::zero()),
            Asset::new(sponsor_fee_asset.clone(), Quantity::from(10_u32)),
        ],
        [],
    );
    let kura = Arc::new(Kura::blank_kura_for_testing());
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
    install_test_lane_manifests(&state);
    let mut metadata = Metadata::default();
    metadata.insert(
        Name::from_str("fee_sponsor").expect("metadata key"),
        Json::new(sponsor_id.to_string()),
    );
    let mut builder = TransactionBuilder::new(
        state.network_id,
        payer_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    builder.set_creation_time(Duration::from_millis(0));
    let error = builder
        .with_metadata(metadata)
        .with_instructions([Transfer::asset_quantity(
            payer_transfer_asset.clone(),
            1_u32,
            recipient_id,
        )])
        .try_sign(payer_keypair.private_key())
        .expect_err("retired fee_sponsor metadata must fail before block admission");
    assert!(
        matches!(
            &error,
            iroha_data_model::transaction::signed::TransactionSignatureError::InvalidFeePaymentIntent(message)
                if message.contains("legacy transaction metadata key `fee_sponsor`")
        ),
        "unexpected signing error: {error}"
    );
    let state_view = state.world.view();
    let assets = state_view.assets();
    assert_eq!(
        assets
            .get(&payer_transfer_asset)
            .expect("payer rose after signing rejection")
            .0,
        Quantity::from(5_u32)
    );
    assert_eq!(
        assets
            .get(&recipient_transfer_asset)
            .expect("recipient rose after signing rejection")
            .0,
        Quantity::zero()
    );
    assert_eq!(
        assets
            .get(&sponsor_fee_asset)
            .expect("sponsor xor after signing rejection")
            .0,
        Quantity::from(10_u32),
        "signing rejection must not debit the legacy sponsor"
    );
}
#[test]
fn legacy_fee_sponsor_metadata_rejects_when_nexus_fees_are_configured() {
    let _guard = crate::sumeragi::status::nexus_fee_test_lock()
        .lock()
        .expect("nexus fee test lock");
    crate::sumeragi::status::reset_nexus_economics_for_tests();
    crate::sumeragi::status::reset_rbc_backlog_stats_for_tests();
    let chain_id = ChainId::from("legacy-fee-sponsor-metadata-configured-fees-test");
    let (payer_id, payer_keypair) = gen_account_in("wonderland");
    let (sponsor_id, _sponsor_keypair) = gen_account_in("wonderland");
    let (recipient_id, _recipient_keypair) = gen_account_in("wonderland");
    let (sink_id, _sink_keypair) = gen_account_in("wonderland");
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let domain = Domain::new(domain_id.clone()).build(&payer_id);
    let payer = Account::new(payer_id.clone()).build(&payer_id);
    let sponsor = Account::new(sponsor_id.clone()).build(&sponsor_id);
    let recipient = Account::new(recipient_id.clone()).build(&recipient_id);
    let sink = Account::new(sink_id.clone()).build(&sink_id);
    let transfer_asset_definition_id = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "rose".parse().expect("asset name"),
    );
    let fee_asset_definition_id =
        AssetDefinitionId::derive_from_components(domain_id, "xor".parse().expect("asset name"));
    let transfer_asset_definition = AssetDefinition::numeric(
        transfer_asset_definition_id.clone(),
        "rose".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&payer_id);
    let fee_asset_definition = AssetDefinition::numeric(
        fee_asset_definition_id.clone(),
        "xor".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&payer_id);
    let payer_transfer_asset = AssetId::of(transfer_asset_definition_id.clone(), payer_id.clone());
    let recipient_transfer_asset =
        AssetId::of(transfer_asset_definition_id.clone(), recipient_id.clone());
    let sponsor_fee_asset = AssetId::of(fee_asset_definition_id.clone(), sponsor_id.clone());
    let world = test_world_with_assets(
        [domain],
        [payer, sponsor, recipient, sink],
        [transfer_asset_definition, fee_asset_definition],
        [
            Asset::new(payer_transfer_asset.clone(), Quantity::from(5_u32)),
            Asset::new(recipient_transfer_asset.clone(), Quantity::zero()),
            Asset::new(sponsor_fee_asset.clone(), Quantity::from(10_u32)),
        ],
        [],
    );
    let kura = Arc::new(Kura::blank_kura_for_testing());
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
    install_test_lane_manifests(&state);
    {
        let nexus = state.nexus.get_mut();
        nexus.fees.base_fee = Quantity::from(1_u32);
        nexus.fees.per_byte_fee = Quantity::zero();
        nexus.fees.per_instruction_fee = Quantity::zero();
        nexus.fees.per_gas_unit_fee = Quantity::zero();
        nexus.fees.fee_asset_id = fee_asset_definition_id.to_string();
        nexus.fees.fee_sink_account_id = sink_id.to_string();
    }
    let mut metadata = Metadata::default();
    metadata.insert(
        Name::from_str("fee_sponsor").expect("metadata key"),
        Json::new(sponsor_id.to_string()),
    );
    let mut builder = TransactionBuilder::new(
        state.network_id,
        payer_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    builder.set_creation_time(Duration::from_millis(0));
    let error = builder
        .with_metadata(metadata)
        .with_instructions([Transfer::asset_quantity(
            payer_transfer_asset.clone(),
            1_u32,
            recipient_id,
        )])
        .try_sign(payer_keypair.private_key())
        .expect_err("retired fee_sponsor metadata must fail before block admission");
    assert!(
        matches!(
            &error,
            iroha_data_model::transaction::signed::TransactionSignatureError::InvalidFeePaymentIntent(message)
                if message.contains("legacy transaction metadata key `fee_sponsor`")
        ),
        "unexpected signing error: {error}"
    );
    let state_view = state.world.view();
    let assets = state_view.assets();
    assert_eq!(
        assets
            .get(&payer_transfer_asset)
            .expect("payer rose after signing rejection")
            .0,
        Quantity::from(5_u32)
    );
    assert_eq!(
        assets
            .get(&recipient_transfer_asset)
            .expect("recipient rose after signing rejection")
            .0,
        Quantity::zero()
    );
    assert_eq!(
        assets
            .get(&sponsor_fee_asset)
            .expect("sponsor xor after signing rejection")
            .0,
        Quantity::from(10_u32),
        "signing rejection must not debit the legacy sponsor"
    );
}
#[test]
fn fee_enabled_invalid_fee_asset_rejects_without_partial_transfer_or_fee() {
    let _guard = crate::sumeragi::status::nexus_fee_test_lock()
        .lock()
        .expect("nexus fee test lock");
    crate::sumeragi::status::reset_nexus_economics_for_tests();
    crate::sumeragi::status::reset_rbc_backlog_stats_for_tests();
    let chain_id = ChainId::from("fee-detached-invalid-fee-asset-test");
    let (payer_id, payer_keypair) = gen_account_in("wonderland");
    let (recipient_id, _recipient_keypair) = gen_account_in("wonderland");
    let (sink_id, _sink_keypair) = gen_account_in("wonderland");
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let domain = Domain::new(domain_id.clone()).build(&payer_id);
    let payer = Account::new(payer_id.clone()).build(&payer_id);
    let recipient = Account::new(recipient_id.clone()).build(&recipient_id);
    let sink = Account::new(sink_id.clone()).build(&sink_id);
    let transfer_asset_definition_id =
        AssetDefinitionId::derive_from_components(domain_id, "rose".parse().expect("asset name"));
    let transfer_asset_definition = AssetDefinition::numeric(
        transfer_asset_definition_id.clone(),
        "rose".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&payer_id);
    let payer_transfer_asset = AssetId::of(transfer_asset_definition_id.clone(), payer_id.clone());
    let recipient_transfer_asset =
        AssetId::of(transfer_asset_definition_id.clone(), recipient_id.clone());
    let world = test_world_with_assets(
        [domain],
        [payer, recipient, sink],
        [transfer_asset_definition],
        [
            Asset::new(payer_transfer_asset.clone(), Quantity::from(5_u32)),
            Asset::new(recipient_transfer_asset.clone(), Quantity::zero()),
        ],
        [],
    );
    let kura = Arc::new(Kura::blank_kura_for_testing());
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
    install_test_lane_manifests(&state);
    {
        let nexus = state.nexus.get_mut();
        nexus.fees.base_fee = Quantity::from(1_u32);
        nexus.fees.per_byte_fee = Quantity::zero();
        nexus.fees.per_instruction_fee = Quantity::zero();
        nexus.fees.per_gas_unit_fee = Quantity::zero();
        nexus.fees.fee_asset_id = "not-an-asset-literal".to_owned();
        nexus.fees.fee_sink_account_id = sink_id.to_string();
    }
    let (max_clock_drift, tx_limits) = {
        let state_view = state.world.view();
        let params = state_view.parameters();
        (params.sumeragi().max_clock_drift(), params.transaction())
    };
    let leader = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (_leader_public, leader_private) = leader.into_parts();
    let latest_valid = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
        header.set_height(nonzero!(1_u64));
    });
    let latest_signed: SignedBlock = latest_valid.into();
    let mut builder = TransactionBuilder::new(
        state.network_id,
        payer_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    builder.set_creation_time(Duration::from_millis(0));
    let tx = builder
        .with_instructions([Transfer::asset_quantity(
            payer_transfer_asset.clone(),
            1_u32,
            recipient_id,
        )])
        .sign(payer_keypair.private_key());
    let tx = AcceptedTransaction::accept(
        tx,
        &state.network_id,
        max_clock_drift,
        tx_limits,
        state.crypto().as_ref(),
    )
    .expect("transaction should pass stateless admission");
    let (_block_handle, block_time_source) = TimeSource::new_mock(Duration::from_millis(10));
    let unverified_block = BlockBuilder::new_with_time_source(vec![tx], block_time_source)
        .chain(1, Some(&latest_signed))
        .sign(payer_keypair.private_key())
        .unpack(|_| {});
    let mut state_block = state.block(unverified_block.header);
    let valid_block = unverified_block
        .validate_and_record_transactions(&mut state_block)
        .unpack(|_| {});
    assert_eq!(
        valid_block.as_ref().errors().next().map(|(idx, _)| idx),
        Some(0),
        "invalid configured fee asset must reject the transaction"
    );
    let snapshot = crate::sumeragi::status::snapshot();
    assert_eq!(snapshot.pipeline_execution.detached_merged_total, 0);
    assert_eq!(
        snapshot.pipeline_execution.detached_fallback_total, 0,
        "invalid governed fee configuration must fail signed admission before execution"
    );
    let assets = state_block.world.assets();
    assert_eq!(
        assets
            .get(&payer_transfer_asset)
            .expect("payer rose after invalid fee asset rejection")
            .0,
        Quantity::from(5_u32)
    );
    assert_eq!(
        assets
            .get(&recipient_transfer_asset)
            .expect("recipient rose after invalid fee asset rejection")
            .0,
        Quantity::zero()
    );
}
#[test]
fn rejected_data_trigger_execution_still_charges_nexus_fee() {
    let _guard = crate::sumeragi::status::nexus_fee_test_lock()
        .lock()
        .expect("nexus fee test lock");
    crate::sumeragi::status::reset_nexus_economics_for_tests();
    let chain_id = ChainId::from("rejected-trigger-fee-test");
    let (payer_id, payer_keypair) = gen_account_in("wonderland");
    let (sink_id, _sink_keypair) = gen_account_in("wonderland");
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let domain = Domain::new(domain_id.clone()).build(&payer_id);
    let payer = Account::new(payer_id.clone()).build(&payer_id);
    let sink = Account::new(sink_id.clone()).build(&sink_id);
    let asset_definition_id =
        AssetDefinitionId::derive_from_components(domain_id, "xor".parse().expect("asset name"));
    let asset_definition = AssetDefinition::numeric(
        asset_definition_id.clone(),
        "xor".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&payer_id);
    let payer_asset = Asset::new(
        AssetId::of(asset_definition_id.clone(), payer_id.clone()),
        Quantity::from(10_u32),
    );
    let sink_asset = Asset::new(
        AssetId::of(asset_definition_id.clone(), sink_id.clone()),
        Quantity::zero(),
    );
    let world = test_world_with_assets(
        [domain],
        [payer, sink],
        [asset_definition],
        [payer_asset, sink_asset],
        [],
    );
    let kura = Arc::new(Kura::blank_kura_for_testing());
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
    install_test_lane_manifests(&state);
    {
        let nexus = state.nexus.get_mut();
        nexus.fees.base_fee = Quantity::from(1_u32);
        nexus.fees.per_byte_fee = Quantity::zero();
        nexus.fees.per_instruction_fee = Quantity::zero();
        nexus.fees.per_gas_unit_fee = Quantity::zero();
        nexus.fees.fee_asset_id = asset_definition_id.to_string();
        nexus.fees.fee_sink_account_id = sink_id.to_string();
    }
    {
        let mut world = state.world.block();
        world
            .parameters
            .set_parameter(iroha_data_model::parameter::Parameter::SmartContract(
                iroha_data_model::parameter::SmartContractParameter::ExecutionDepth(0),
            ));
        world.commit();
    }
    let (max_clock_drift, tx_limits) = {
        let state_view = state.world.view();
        let params = state_view.parameters();
        (params.sumeragi().max_clock_drift(), params.transaction())
    };
    let leader = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (_leader_public, leader_private) = leader.into_parts();
    let latest_valid = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
        header.set_height(nonzero!(1_u64));
    });
    let latest_signed: SignedBlock = latest_valid.into();
    let trigger_id: TriggerId = "fee_depth_limit_trigger".parse().unwrap();
    let flag_key: Name = "fee_trigger_flag".parse().unwrap();
    let event_key: Name = "fee_trigger_event".parse().unwrap();
    let trigger = Trigger::new(
        trigger_id,
        Action::new(
            vec![InstructionBox::from(SetKeyValue::account(
                payer_id.clone(),
                flag_key,
                Json::from(true),
            ))],
            Repeats::Indefinitely,
            payer_id.clone(),
            DataEventFilter::Any,
        )
        .expect("trigger action fixture satisfies validation invariants"),
    );
    let fee_payment = iroha_data_model::transaction::FeePaymentIntent::authority(
        vec![iroha_data_model::transaction::FeeChargeLimit::new(
            iroha_data_model::transaction::FeeChargeKind::Nexus,
            asset_definition_id.clone(),
            Quantity::from(1_u32),
        )],
        None,
    );
    let mut builder = TransactionBuilder::new(state.network_id, payer_id.clone(), fee_payment);
    builder.set_creation_time(Duration::from_millis(0));
    let tx = builder
        .with_instructions::<InstructionBox>([
            Grant::account_permission(
                iroha_executor_data_model::permission::trigger::CanRegisterTrigger {
                    authority: payer_id.clone(),
                },
                payer_id.clone(),
            )
            .into(),
            Register::trigger(trigger).into(),
            SetKeyValue::account(payer_id.clone(), event_key.clone(), Json::from(true)).into(),
        ])
        .sign(payer_keypair.private_key());
    let tx = AcceptedTransaction::accept(
        tx,
        &state.network_id,
        max_clock_drift,
        tx_limits,
        state.crypto().as_ref(),
    )
    .expect("transaction should pass stateless admission");
    let (_block_handle, block_time_source) = TimeSource::new_mock(Duration::from_millis(10));
    let unverified_block = BlockBuilder::new_with_time_source(vec![tx], block_time_source)
        .chain(1, Some(&latest_signed))
        .sign(payer_keypair.private_key())
        .unpack(|_| {});
    let mut state_block = state.block(unverified_block.header);
    let valid_block = unverified_block
        .validate_and_record_transactions(&mut state_block)
        .unpack(|_| {});
    assert_eq!(
        valid_block.as_ref().errors().next().map(|(idx, _)| idx),
        Some(0)
    );
    let first_error = valid_block.as_ref().errors().next().map(|(_, err)| err);
    assert!(
        matches!(
            first_error,
            Some(TransactionRejectionReason::TriggerExecution(
                iroha_data_model::transaction::error::TriggerExecutionFail::MaxDepthExceeded
            ))
        ),
        "unexpected trigger rejection: {first_error:?}"
    );
    let assets = state_block.world.assets();
    let payer_balance = assets
        .get(&AssetId::of(asset_definition_id.clone(), payer_id.clone()))
        .expect("payer balance exists")
        .0
        .to_string();
    let sink_balance = assets
        .get(&AssetId::of(asset_definition_id, sink_id))
        .expect("sink balance exists")
        .0
        .to_string();
    assert_eq!(payer_balance, "9", "tx error: {first_error:?}");
    assert_eq!(sink_balance, "0");
    let event_value = state_block
        .world
        .map_account(&payer_id, |account| {
            account.value().metadata().get(&event_key).cloned()
        })
        .expect("payer account exists");
    assert!(
        event_value.is_none(),
        "trigger-rejected transaction state changes must still be rolled back"
    );
}
#[tokio::test]
async fn validate_and_record_transactions_allows_missing_authority_self_register() {
    let chain_id = ChainId::from("missing-authority-self-register-block");
    let (authority, keypair) = gen_account_in("wonderland");
    let world = World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_with_chain(world, kura, query_handle, chain_id.clone());
    install_test_lane_manifests(&state);
    let (max_clock_drift, tx_limits) = {
        let state_view = state.world.view();
        let params = state_view.parameters();
        (params.sumeragi().max_clock_drift(), params.transaction())
    };
    let tx = TransactionBuilder::new(
        state.network_id,
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([
        InstructionBox::from(Register::account(Account::new(authority.clone()))),
        InstructionBox::from(Log::new(Level::INFO, "self-register".into())),
    ])
    .sign(keypair.private_key());
    let crypto_cfg = state.crypto();
    let tx = AcceptedTransaction::accept(
        tx,
        &state.network_id,
        max_clock_drift,
        tx_limits,
        crypto_cfg.as_ref(),
    )
    .expect("admission should accept transaction shape");
    let unverified_block = BlockBuilder::new(vec![tx])
        .chain(0, state.view().latest_block().as_deref())
        .sign(keypair.private_key())
        .unpack(|_| {});
    let mut state_block = state.block(unverified_block.header);
    let valid_block = unverified_block
        .validate_and_record_transactions(&mut state_block)
        .unpack(|_| {});
    assert!(
        valid_block.as_ref().errors().next().is_none(),
        "self-register block path should not produce transaction errors"
    );
    assert!(
        state_block.world.accounts.get(&authority).is_some(),
        "authority account should be materialized during block execution"
    );
}
