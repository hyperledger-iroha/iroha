#[test]
fn rejected_live_batch_business_execution_still_charges_nexus_fee() {
    let _guard = crate::sumeragi::status::nexus_fee_test_lock()
        .lock()
        .expect("nexus fee test lock");
    crate::sumeragi::status::reset_nexus_economics_for_tests();
    let chain_id = ChainId::from("rejected-live-batch-fee-test");
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
    let created_domain_id = DomainId::try_new("fee-created", "universal").unwrap();
    let create_domain = Register::domain(Domain::new(created_domain_id.clone()));
    let fail_instruction =
        Unregister::domain(DomainId::try_new("missing-domain", "universal").unwrap());
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
        .with_executable(Executable::Batch(
            vec![
                ExecutableBatchItem::Instruction(create_domain.into()),
                ExecutableBatchItem::Instruction(fail_instruction.into()),
            ]
            .into(),
        ))
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
        Some(0)
    );
    let first_error = valid_block
        .as_ref()
        .errors()
        .next()
        .map(|(_, err)| format!("{err:?}"));
    let assets = state_block.world.assets();
    let payer_balance = assets
        .get(&AssetId::of(asset_definition_id.clone(), payer_id))
        .expect("payer balance exists")
        .0
        .to_string();
    let sink_balance = assets
        .get(&AssetId::of(asset_definition_id, sink_id))
        .expect("sink balance exists")
        .0
        .to_string();
    assert_eq!(
        payer_balance, "9",
        "rejected mixed batch must still pay its Nexus fee; tx error: {first_error:?}"
    );
    assert_eq!(sink_balance, "0");
    assert!(
        state_block.world.domain(&created_domain_id).is_err(),
        "failed transaction state changes must still be rolled back"
    );
}
#[test]
fn rejected_contract_only_batch_vm_error_still_charges_nexus_fee() {
    let _guard = crate::sumeragi::status::nexus_fee_test_lock()
        .lock()
        .expect("nexus fee test lock");
    crate::sumeragi::status::reset_nexus_economics_for_tests();
    let chain_id = ChainId::from("rejected-contract-batch-fee-test");
    let (payer_id, payer_keypair) = gen_account_in("wonderland");
    let (sink_id, _sink_keypair) = gen_account_in("wonderland");
    let domain_id = DomainId::try_new("wonderland", "universal").expect("domain id");
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
    let (program, manifest) = ivm::KotodamaCompiler::new()
        .compile_source_with_manifest(
            r#"
seiyaku MeteredFailure {
  kotoage fn run() authorize("CanInvokeContractEntrypoint") {
ledger::account::set_detail(
  account: context::authority(),
  key: Name::parse("must_not_be_written"),
  value: Json::parse("true")
);
  }
}
"#,
        )
        .expect("compile metered failure contract");
    let code_hash = ivm::contract_code_hash(&program);
    let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &payer_id,
        95,
        DataSpaceId::UNIVERSAL,
    )
    .expect("derive contract address");
    let contract_subject = Account::new(contract_address.subject_id()).build(&payer_id);
    let mut world = test_world_with_assets(
        [domain],
        [payer, sink, contract_subject],
        [asset_definition],
        [payer_asset, sink_asset],
        [],
    );
    world.contract_code.insert(code_hash, program);
    world
        .contract_manifests
        .insert(code_hash, manifest.signed(&payer_keypair));
    world
        .contract_instances
        .insert(contract_address.clone(), code_hash);
    let entrypoint_permission: Permission =
        iroha_executor_data_model::permission::smart_contract::CanInvokeContractEntrypoint {
            contract: contract_address.clone(),
            entrypoint: "run".to_owned(),
        }
        .into();
    let mut permissions = iroha_data_model::permission::Permissions::new();
    assert!(permissions.insert(entrypoint_permission));
    world
        .account_permissions_mut_for_testing()
        .insert(payer_id.clone(), permissions);
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
    let invocation = iroha_data_model::transaction::executable::ContractInvocation {
        contract_address,
        expected_code_hash: code_hash,
        entrypoint: "run".to_owned(),
        arguments: None,
    };
    let fee_payment = iroha_data_model::transaction::FeePaymentIntent::authority(
        vec![iroha_data_model::transaction::FeeChargeLimit::new(
            iroha_data_model::transaction::FeeChargeKind::Nexus,
            asset_definition_id.clone(),
            Quantity::from(1_u32),
        )],
        core::num::NonZeroU64::new(10),
    );
    let mut builder = TransactionBuilder::new(state.network_id, payer_id.clone(), fee_payment);
    builder.set_creation_time(Duration::from_millis(0));
    let tx = builder
        .with_executable(Executable::Batch(
            vec![ExecutableBatchItem::ContractCall(invocation)].into(),
        ))
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
    let block = BlockBuilder::new_with_time_source(vec![tx], block_time_source)
        .chain(1, Some(&latest_signed))
        .sign(payer_keypair.private_key())
        .unpack(|_| {});
    let mut state_block = state.block(block.header());
    let valid = block
        .validate_and_record_transactions(&mut state_block)
        .unpack(|_| {});
    let error = valid
        .as_ref()
        .errors()
        .next()
        .map(|(_, error)| error)
        .expect("the gas-capped contract call must fail");
    assert!(
        matches!(
            error,
            TransactionRejectionReason::Validation(ValidationFail::NotPermitted(message))
                if message.contains("gas")
        ),
        "unexpected VM rejection: {error:?}"
    );
    let assets = state_block.world.assets();
    let payer_balance = assets
        .get(&AssetId::of(asset_definition_id, payer_id.clone()))
        .expect("payer balance exists")
        .0
        .to_string();
    assert_eq!(
        payer_balance, "9",
        "failed contract VM work must remain chargeable"
    );
    assert!(
        (1..=10).contains(&state_block.gas_used_in_block),
        "failed contract VM work must retain its deterministic metered consumption, observed {}",
        state_block.gas_used_in_block
    );
    let marker: Name = "must_not_be_written".parse().expect("marker name");
    assert!(
        state_block
            .world
            .account(&payer_id)
            .expect("payer account")
            .metadata()
            .get(&marker)
            .is_none(),
        "contract business effects must roll back on VM failure"
    );
}
#[test]
fn successful_live_batches_accumulate_parent_block_gas() {
    for parallel_apply in [false, true] {
        let chain_id = ChainId::try_from(format!("live-batch-parent-gas-{parallel_apply}"))
            .expect("canonical live-batch test chain id");
        let (authority, keypair) = gen_account_in("wonderland");
        let domain_id = DomainId::try_new("wonderland", "universal").expect("domain id");
        let world = World::with(
            [Domain::new(domain_id).build(&authority)],
            [Account::new(authority.clone()).build(&authority)],
            [],
        );
        let mut state = State::new_with_chain_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            chain_id.clone(),
        );
        install_test_lane_manifests(&state);
        let mut pipeline = state.pipeline.clone();
        pipeline.parallel_apply = parallel_apply;
        pipeline.parallel_overlay = true;
        pipeline.workers = 2;
        state.set_pipeline(pipeline);
        let log_instruction =
            InstructionBox::from(Log::new(Level::INFO, "meter one live batch".to_owned()));
        let expected_gas = crate::gas::meter_instructions(core::slice::from_ref(&log_instruction));
        assert!(expected_gas > 0, "the fixture must consume gas");
        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        let transactions = [0_u64, 1_u64]
            .into_iter()
            .map(|creation_time_ms| {
                let mut builder = TransactionBuilder::new(
                    state.network_id,
                    authority.clone(),
                    iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                );
                builder.set_creation_time(Duration::from_millis(creation_time_ms));
                let transaction = builder
                    .with_executable(Executable::Batch(
                        vec![ExecutableBatchItem::Instruction(log_instruction.clone())].into(),
                    ))
                    .sign(keypair.private_key());
                accept_transaction_at_mock_time(
                    transaction,
                    &state.network_id,
                    max_clock_drift,
                    tx_limits,
                    state.crypto().as_ref(),
                    Duration::from_millis(10),
                )
                .expect("batch must pass stateless admission")
            })
            .collect::<Vec<_>>();
        let block = BlockBuilder::new(transactions)
            .chain(0, None)
            .sign(keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(block.header());
        state_block.gas_limit_per_block = expected_gas;
        let valid = block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});
        let results = valid
            .as_ref()
            .results()
            .map(|result| result.0.clone())
            .collect::<Vec<_>>();
        let successes = results.iter().filter(|result| result.is_ok()).count();
        let gas_limit_rejections = results
            .iter()
            .filter(|result| {
                matches!(
                    result,
                    Err(TransactionRejectionReason::Validation(
                        ValidationFail::NotPermitted(message)
                    )) if message.contains("block gas limit exceeded")
                )
            })
            .count();
        assert_eq!(
            successes, 1,
            "only one live batch may fit with parallel_apply={parallel_apply}: {results:?}"
        );
        assert_eq!(
            gas_limit_rejections, 1,
            "the second live batch must observe parent gas with parallel_apply={parallel_apply}: {results:?}"
        );
        assert_eq!(
            state_block.gas_used_in_block, expected_gas,
            "successful live-batch gas must be retained by the parent block"
        );
    }
}
