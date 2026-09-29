// Mandatory signed monetary bindings and bounded non-forfeiting claims.
#[test]
fn monetary_plan_context_binds_genesis_network_and_governed_expiry() {
    let mut state = setup_state();
    set_epoch_length(&mut state, 6);
    let mut block = state.block(block_header_with_height(2));
    let stx = block.transaction_for_callback_testing();
    let network = PublicLaneMonetaryScopeV1::Network(*stx.network_id());
    for expiry in [2, 8] {
        effects::validate_plan_context(&stx, &network, expiry).unwrap();
    }
    for expiry in [0, 1, 9] {
        assert!(effects::validate_plan_context(&stx, &network, expiry).is_err());
    }
    assert!(effects::validate_plan_context(&stx, &PublicLaneMonetaryScopeV1::Genesis, 2).is_err());
    let foreign =
        PublicLaneMonetaryScopeV1::Network(iroha_data_model::NetworkId::from_genesis_hash(
            HashOf::from_untyped_unchecked(Hash::new(b"other network")),
        ));
    assert!(effects::validate_plan_context(&stx, &foreign, 2).is_err());
    drop(stx);
    drop(block);
    let mut genesis = state.block(block_header_with_height(1));
    effects::validate_plan_context(
        &genesis.transaction_for_callback_testing(),
        &PublicLaneMonetaryScopeV1::Genesis,
        1,
    )
    .unwrap();
    drop(genesis);
    let mut overflow = state.block(block_header_with_height(u64::MAX));
    assert!(
        effects::validate_plan_context(
            &overflow.transaction_for_callback_testing(),
            &network,
            u64::MAX
        )
        .unwrap_err()
        .to_string()
        .contains("overflow")
    );
    let empty_state = State::new_with_nexus_for_testing(
        World::with([], [Account::new(ALICE_ID.clone()).build(&ALICE_ID)], []),
        iroha_config::parameters::actual::Nexus::default(),
        LiveQueryStore::start_test(),
    );
    let mut empty_block = empty_state.block(block_header_with_height(1));
    let empty_transaction = empty_block.transaction_for_callback_testing();
    effects::validate_plan_context(&empty_transaction, &PublicLaneMonetaryScopeV1::Genesis, 1)
        .expect("genesis scope is exactly height-bound, independently of the election schedule");
    for expiry in [0, 2] {
        assert!(
            effects::validate_plan_context(
                &empty_transaction,
                &PublicLaneMonetaryScopeV1::Genesis,
                expiry
            )
            .is_err()
        );
    }
    // Actual monetary execution independently requires the committed XOR identity;
    // genesis scope never supplies a default or substitute currency.
    assert!(
        ensure_committed_xor_asset(
            &empty_transaction.world,
            &SumeragiNposParameters::default().xor_asset_definition_id
        )
        .is_err()
    );
}

#[test]
fn registration_monetary_plan_rejects_every_changed_effect_before_custody_writes() {
    let state = setup_state();
    let mut block = state.block(block_header_with_height(1));
    let mut stx = block.transaction_for_callback_testing();
    let (validator, delegator, escrow, definition) = prepare_accounts(&mut stx);
    let lane = LaneId::new(42);
    let base = RegisterPublicLaneValidator::new(
        lane,
        validator.clone(),
        validator_peer_id(&validator),
        validator.clone(),
        1_000_u64.into(),
        Metadata::default(),
        fixture_registration_plan(&stx, lane, &validator, &1_000_u64.into()),
    );
    let original = stx
        .world
        .assets
        .get(&AssetId::new(definition.clone(), validator.clone()))
        .cloned();
    for change in 0..5 {
        let mut instruction = base.clone();
        let plan = &mut instruction.monetary_plan;
        match change {
            0 => plan.source_asset = AssetId::new(definition.clone(), delegator.clone()),
            1 => plan.destination_asset = AssetId::new(definition.clone(), delegator.clone()),
            2 => plan.amount = 999_u64.into(),
            3 => {
                plan.precondition = PublicLaneMonetaryPreconditionV1::Registration(
                    iroha_data_model::nexus::PublicLaneMonetaryRegistrationV1 {
                        activation_height: 2,
                    },
                )
            }
            _ => plan.valid_until_height = 0,
        }
        assert!(
            instruction
                .execute(&validator, &mut stx)
                .unwrap_err()
                .to_string()
                .contains("monetary plan")
        );
        assert_eq!(
            stx.world
                .assets
                .get(&AssetId::new(definition.clone(), validator.clone())),
            original.as_ref()
        );
        assert!(
            stx.world
                .public_lane_stake_custody
                .get(&(lane, validator.clone()))
                .is_none()
        );
        assert!(
            stx.world
                .assets
                .get(&AssetId::new(definition.clone(), escrow.clone()))
                .is_none()
        );
    }
    base.execute(&validator, &mut stx).unwrap();
}

#[test]
fn reward_claim_rejects_skips_forged_records_accruals_payouts_and_oversized_prefixes() {
    let state = setup_state();
    let mut block = state.block(block_header_with_height(1));
    let mut stx = block.transaction_for_callback_testing();
    let lane = LaneId::SINGLE;
    let (sink, recipient, asset, _) = configure_reward_fixture(&mut stx, lane, 100);
    stx.nexus.staking.reward_dust_threshold = Quantity::zero();
    for epoch in 0..2 {
        reward_distribution(lane, epoch, &asset, &recipient, 10)
            .execute(&sink, &mut stx)
            .unwrap();
    }
    let plan = fixture_reward_claim_plan(&stx, lane, &recipient, None);
    for change in 0..7 {
        let mut invalid = plan.clone();
        match change {
            0 => {
                invalid.records.remove(0);
            }
            1 => invalid.records[0].record_hash = Hash::new(b"forged record"),
            2 => invalid.sources[0].expected_accrued = Some(Quantity::one()),
            3 => invalid.sources[0].payout = 19_u64.into(),
            4 => {
                invalid.records = (0..65)
                    .map(|epoch| PublicLaneRewardRecordRefV1 {
                        epoch,
                        record_hash: Hash::new(epoch.to_be_bytes()),
                    })
                    .collect()
            }
            5 => invalid.sources.clear(),
            _ => {
                invalid.expected_state =
                    Some(iroha_data_model::nexus::PublicLaneRewardClaimStateV1 {
                        through_epoch: Some(0),
                    })
            }
        }
        assert!(effects::prepare_reward_claim(&stx, lane, &recipient, &invalid).is_err());
        assert!(
            stx.world
                .public_lane_reward_claims
                .get(&(lane, recipient.clone()))
                .is_none()
        );
        assert_eq!(
            stx.world.public_lane_reward_reserves.get(&asset),
            Some(&Quantity::from(20_u64))
        );
    }
    let prepared = effects::prepare_reward_claim(&stx, lane, &recipient, &plan).unwrap();
    assert_eq!(prepared.state_after.unwrap().through_epoch, Some(1));
    assert_eq!(prepared.payouts[0].2, Quantity::from(20_u64));
}

#[test]
fn reward_claim_processes_more_than_sixty_four_dust_records_without_forfeiture() {
    let state = setup_state();
    let mut block = state.block(block_header_with_height(1));
    let mut stx = block.transaction_for_fastpq_testing(Hash::prehashed([0xE2; Hash::LENGTH]));
    let lane = LaneId::SINGLE;
    let (sink, recipient, asset, definition) = configure_reward_fixture(&mut stx, lane, 200);
    stx.nexus.staking.reward_dust_threshold = 100_u64.into();
    for epoch in 0..130 {
        reward_distribution(lane, epoch, &asset, &recipient, 1)
            .execute(&sink, &mut stx)
            .unwrap();
    }
    for (batch, expected_records, cursor, accrual, paid) in [
        (0, 64, 63, 64, 0),
        (1, 64, 127, 0, 128),
        (2, 2, 129, 2, 128),
    ] {
        let plan = fixture_reward_claim_plan(&stx, lane, &recipient, None);
        assert_eq!(plan.records.len(), expected_records);
        ClaimPublicLaneRewards {
            lane_id: lane,
            account: recipient.clone(),
            claim_plan: plan,
        }
        .execute(&recipient, &mut stx)
        .unwrap();
        assert_eq!(
            stx.world
                .public_lane_reward_claims
                .get(&(lane, recipient.clone()))
                .unwrap()
                .through_epoch,
            Some(cursor),
            "batch {batch}"
        );
        assert_eq!(
            stx.world
                .public_lane_reward_accruals
                .get(&(lane, recipient.clone(), asset.clone()))
                .cloned()
                .unwrap_or_else(Quantity::zero),
            Quantity::from(accrual as u64)
        );
        assert_eq!(
            stx.world
                .assets
                .get(&AssetId::new(definition.clone(), recipient.clone()))
                .map_or_else(Quantity::zero, |value| value.as_ref().clone()),
            Quantity::from(paid as u64)
        );
        assert_eq!(
            rewards::outstanding_rewards(&stx.world, &asset).unwrap(),
            Quantity::from((130 - paid) as u64)
        );
    }
    stx.nexus.staking.reward_dust_threshold = Quantity::zero();
    let plan = fixture_reward_claim_plan(&stx, lane, &recipient, None);
    assert!(plan.records.is_empty());
    ClaimPublicLaneRewards {
        lane_id: lane,
        account: recipient.clone(),
        claim_plan: plan,
    }
    .execute(&recipient, &mut stx)
    .unwrap();
    assert_eq!(
        stx.world
            .assets
            .get(&AssetId::new(definition, recipient))
            .unwrap()
            .as_ref(),
        &Quantity::from(130_u64)
    );
    assert!(stx.world.public_lane_reward_reserves.get(&asset).is_none());
}

#[test]
fn reward_claim_zero_entitlements_advance_without_creating_accrual() {
    let state = setup_state();
    let mut block = state.block(block_header_with_height(1));
    let mut stx = block.transaction_for_callback_testing();
    let lane = LaneId::SINGLE;
    let (sink, validator, asset, _) = configure_reward_fixture(&mut stx, lane, 100);
    reward_distribution(lane, 0, &asset, &validator, 10)
        .execute(&sink, &mut stx)
        .unwrap();
    let recipient = ALICE_ID.clone();
    let plan = fixture_reward_claim_plan(&stx, lane, &recipient, None);
    assert_eq!(plan.sources.len(), 1);
    assert!(plan.sources[0].payout.is_zero());
    ClaimPublicLaneRewards {
        lane_id: lane,
        account: recipient.clone(),
        claim_plan: plan,
    }
    .execute(&recipient, &mut stx)
    .unwrap();
    assert_eq!(
        stx.world
            .public_lane_reward_claims
            .get(&(lane, recipient.clone()))
            .unwrap()
            .through_epoch,
        Some(0)
    );
    assert!(
        stx.world
            .public_lane_reward_accruals
            .get(&(lane, recipient, asset.clone()))
            .is_none()
    );
    assert_eq!(
        stx.world.public_lane_reward_reserves.get(&asset),
        Some(&Quantity::from(10_u64))
    );
}

#[test]
fn staking_asset_resolution_requires_the_exact_committed_network_currency() {
    let state = setup_state();
    let mut block = state.block(block_header_with_height(1));
    let mut stx = block.transaction_for_callback_testing();
    let (validator, _, _, definition) = prepare_accounts(&mut stx);
    ensure_committed_xor_asset(&stx.world, &definition).unwrap();
    let other = AssetDefinitionId::derive_from_components(
        DomainId::try_new("nexus", "universal").unwrap(),
        "unrelated-currency".parse().unwrap(),
    );
    assert!(
        ensure_committed_xor_asset(&stx.world, &other)
            .unwrap_err()
            .to_string()
            .contains("committed network XOR")
    );
    let mut parameters = stx.world.sumeragi_npos_parameters().unwrap();
    parameters.xor_asset_definition_id = other;
    stx.world
        .parameters
        .get_mut()
        .set_parameter(Parameter::Custom(parameters.into_custom_parameter()));
    assert!(
        stake_context(
            &stx.world,
            &stx.nexus.dataspace_catalog,
            &stx.nexus.staking,
            &validator,
            stx.block_unix_timestamp_ms()
        )
        .is_err()
    );
    let world = World::with([], [Account::new(ALICE_ID.clone()).build(&ALICE_ID)], []);
    assert!(
        ensure_committed_xor_asset(&world.view(), &definition)
            .unwrap_err()
            .to_string()
            .contains("committed network XOR")
    );
}

#[test]
fn final_unbond_rejects_changed_signed_request_without_releasing_custody() {
    let state = setup_state();
    let lane = LaneId::new(42);
    let request_id = Hash::new(b"signed-unbond-preimage");
    let (validator, request, nexus) = {
        let mut block = state.block(block_header_with_height(1));
        let mut stx = block.transaction_for_callback_testing();
        let (validator, _, _, _) = prepare_accounts(&mut stx);
        stx.nexus.staking.unbonding_delay = Duration::ZERO;
        RegisterPublicLaneValidator::new(
            lane,
            validator.clone(),
            validator_peer_id(&validator),
            validator.clone(),
            1_000_u64.into(),
            Metadata::default(),
            fixture_registration_plan(&stx, lane, &validator, &1_000_u64.into()),
        )
        .execute(&validator, &mut stx)
        .unwrap();
        SchedulePublicLaneUnbond {
            lane_id: lane,
            validator: validator.clone(),
            staker: validator.clone(),
            request_id,
            amount: 100_u64.into(),
            release_at_ms: stx.block_unix_timestamp_ms(),
        }
        .execute(&validator, &mut stx)
        .unwrap();
        let request = stx
            .world
            .public_lane_stake_shares
            .get(&stake_key(lane, &validator, &validator))
            .unwrap()
            .pending_unbonds[&request_id]
            .clone();
        let nexus = stx.nexus.clone();
        stx.apply();
        block.commit_world_overlay_for_testing().unwrap();
        (validator, request, nexus)
    };
    let block =
        new_block_with_height_and_time(request.liability_release_height, request.release_at_ms);
    let mut state_block = state.block(block.as_ref().header());
    let mut stx = state_block.transaction_for_fastpq_testing(Hash::prehashed([0xE3; Hash::LENGTH]));
    stx.nexus = nexus;
    let good = fixture_unbond_plan(&stx, lane, &validator, &validator, &request_id);
    let reserve_before = stx
        .world
        .public_lane_stake_reserves
        .get(&good.source_asset)
        .cloned();
    let balances_before = (
        stx.world.assets.get(&good.source_asset).cloned(),
        stx.world.assets.get(&good.destination_asset).cloned(),
    );
    let mut wrong = good.clone();
    wrong.precondition = PublicLaneMonetaryPreconditionV1::Unbond(
        iroha_data_model::nexus::PublicLaneMonetaryUnbondV1 {
            activation_height: 1,
            request_hash: Hash::new(b"different request state"),
        },
    );
    let instruction = FinalizePublicLaneUnbond {
        lane_id: lane,
        validator: validator.clone(),
        staker: validator.clone(),
        request_id,
        monetary_plan: wrong,
    };
    assert!(
        instruction
            .execute(&validator, &mut stx)
            .unwrap_err()
            .to_string()
            .contains("monetary plan")
    );
    assert_eq!(
        stx.world.public_lane_stake_reserves.get(&good.source_asset),
        reserve_before.as_ref()
    );
    assert_eq!(
        (
            stx.world.assets.get(&good.source_asset).cloned(),
            stx.world.assets.get(&good.destination_asset).cloned()
        ),
        balances_before
    );
    assert_eq!(
        stx.world
            .public_lane_stake_shares
            .get(&stake_key(lane, &validator, &validator))
            .unwrap()
            .pending_unbonds[&request_id],
        request
    );
    FinalizePublicLaneUnbond {
        lane_id: lane,
        validator: validator.clone(),
        staker: validator.clone(),
        request_id,
        monetary_plan: good,
    }
    .execute(&validator, &mut stx)
    .unwrap();
}
