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
        let transfer_asset_definition_id = AssetDefinitionId::derive_from_components(
            domain_id,
            "rose".parse().expect("asset name"),
        );
        let transfer_asset_definition = AssetDefinition::numeric(
            transfer_asset_definition_id.clone(),
            "rose".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
        .build(&payer_id);
        let payer_transfer_asset =
            AssetId::of(transfer_asset_definition_id.clone(), payer_id.clone());
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
        let mut state =
            State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
        install_test_lane_manifests(&state);
        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
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
            chain_id.clone(),
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
            &chain_id,
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
        let asset_definition_id = AssetDefinitionId::derive_from_components(
            domain_id,
            "xor".parse().expect("asset name"),
        );
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
        let mut state =
            State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
        install_test_lane_manifests(&state);
        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
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
        let mut builder = TransactionBuilder::new(chain_id.clone(), payer_id.clone(), fee_payment);
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
        let tx = accept_transaction_at_mock_time(
            tx,
            &chain_id,
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
            chain_id.clone(),
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
            &chain_id,
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
