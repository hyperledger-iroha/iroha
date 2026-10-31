// Original parameter-cut controls for the actual registration minimum consumer.

fn committed_staking_fixture_policy() -> iroha_data_model::block::consensus::PublicLaneStakingPolicy
{
    iroha_data_model::block::consensus::PublicLaneStakingPolicy {
        min_validator_stake: 1_u64.into(),
        max_stake_shares_per_validator: nonzero!(256_u32),
        max_pending_unbonds_per_share: nonzero!(8_u32),
        unbonding_delay_ms: 0,
        max_slash_bps: 10_000,
        stake_asset_id: SumeragiNposParameters::default().xor_asset_definition_id,
        stake_escrow_account_id: iroha_test_samples::BOB_ID.clone(),
        slash_sink_account_id: iroha_test_samples::BOB_ID.clone(),
    }
}

#[test]
fn registration_uses_explicit_original_committed_minimum() {
    let mut state = setup_state();
    let mut parameters = crate::state::parameter_cell_block_for_testing(&state.world)
        .expect("funded parameter fixture block");
    let mut policy = committed_staking_fixture_policy();
    policy.min_validator_stake = 1_000_u64.into();
    crate::state::set_parameter_cell_for_testing(
        &mut parameters,
        Parameter::Custom(policy.into_custom_parameter().unwrap()),
        state.world.sccp_committee_generations.allocation_budget(),
    )
    .unwrap();
    parameters.commit();
    let mut block = state.block(block_header_with_height(1));
    let mut transaction = block.transaction_for_callback_testing();
    let (validator, _, _, _) = prepare_accounts(&mut transaction);
    RegisterPublicLaneValidator {
        monetary_plan: fixture_registration_plan(
            &transaction,
            LaneId::new(3),
            &validator,
            Quantity::from(1_000_u64),
        ),
        lane_id: LaneId::new(3),
        peer_id: validator_peer_id(&validator),
        validator: validator.clone(),
        stake_account: validator.clone(),
        initial_stake: Quantity::from(1_000_u64),
        metadata: Metadata::default(),
    }
    .execute(&validator, &mut transaction)
    .expect("the explicit committed minimum controls registration");
    assert!(
        transaction
            .world
            .public_lane_validators
            .get(&(LaneId::new(3), validator))
            .is_some()
    );
}

#[test]
fn staking_monetary_reader_keeps_original_cut_across_applied_and_aborted_updates() {
    let state = setup_state();
    let original = committed_staking_fixture_policy();
    let mut successor = original.clone();
    successor.min_validator_stake = 2_000_u64.into();
    let successor_parameter = Parameter::Custom(successor.clone().into_custom_parameter().unwrap());
    let mut block = state.block(block_header_with_height(2));
    {
        let mut transaction = block.transaction_for_callback_testing();
        transaction
            .world
            .set_parameter_for_testing(successor_parameter.clone())
            .unwrap();
        assert_eq!(transaction.staking_monetary_policy().unwrap(), &original);
        assert_eq!(
            transaction
                .world
                .parameters
                .get()
                .require_staking_policy()
                .unwrap(),
            &successor
        );
        // Aborting preserves the original, including its canonical source value.
    }
    {
        let mut transaction = block.transaction_for_callback_testing();
        assert_eq!(
            transaction
                .world
                .parameters
                .get()
                .require_staking_policy()
                .unwrap(),
            &original
        );
        transaction
            .world
            .set_parameter_for_testing(successor_parameter)
            .unwrap();
        transaction.apply();
    }
    {
        let transaction = block.transaction_for_callback_testing();
        let pointer: *const iroha_data_model::block::consensus::PublicLaneStakingPolicy =
            transaction.staking_monetary_policy().unwrap();
        norito::with_decode_limits_scope(norito::DecodeLimits::new(0, 0, 0, 0, 0), || {
            assert_eq!(transaction.staking_monetary_policy().unwrap(), &original);
            assert!(core::ptr::eq(
                transaction.staking_monetary_policy().unwrap(),
                pointer
            ));
        });
        assert_eq!(
            transaction
                .world
                .parameters
                .get()
                .require_staking_policy()
                .unwrap(),
            &successor
        );
    }
    // Commit only the component World overlay. This is not block publication
    // or evidence for full signed execution/replay.
    block.commit_world_overlay_for_testing().unwrap();
    let mut next = state.block(block_header_with_height(3));
    let transaction = next.transaction_for_callback_testing();
    assert_eq!(transaction.staking_monetary_policy().unwrap(), &successor);
}

#[test]
fn staking_monetary_reader_cannot_initialize_from_transaction_or_local_defaults() {
    let state = State::new_with_nexus_for_testing(
        World::with([], [Account::new(ALICE_ID.clone()).build(&ALICE_ID)], []),
        iroha_config::parameters::actual::Nexus::default(),
        LiveQueryStore::start_test(),
    );
    let mut block = state.block(block_header_with_height(2));
    {
        let mut transaction = block.transaction_for_callback_testing();
        assert!(matches!(
            transaction.staking_monetary_policy(),
            Err(iroha_data_model::block::consensus::PublicLaneStakingPolicyError::MissingParameter)
        ));
        transaction
            .world
            .set_parameter_for_testing(Parameter::Custom(
                committed_staking_fixture_policy()
                    .into_custom_parameter()
                    .unwrap(),
            ))
            .unwrap();
        assert!(matches!(
            transaction.staking_monetary_policy(),
            Err(iroha_data_model::block::consensus::PublicLaneStakingPolicyError::MissingParameter)
        ));
        transaction.apply();
    }
    let transaction = block.transaction_for_callback_testing();
    assert!(matches!(
        transaction.staking_monetary_policy(),
        Err(iroha_data_model::block::consensus::PublicLaneStakingPolicyError::MissingParameter)
    ));
}

#[test]
fn pending_monetary_policy_cannot_change_registration_bond_unbond_or_slash() {
    let mut original = committed_staking_fixture_policy();
    original.slash_sink_account_id = ALICE_ID.clone();
    let state = setup_state_with_staking_policy(original.clone());
    let mut block = state.block(block_header_with_height(2));
    let mut transaction = block.transaction_for_callback_testing();
    let (validator, delegator, escrow, definition) = prepare_accounts(&mut transaction);
    let lane = LaneId::new(42);
    let mut pending = original.clone();
    pending.min_validator_stake = 100_000_u64.into();
    pending.max_stake_shares_per_validator = nonzero!(1_u32);
    pending.max_pending_unbonds_per_share = nonzero!(1_u32);
    pending.unbonding_delay_ms = u64::MAX;
    pending.max_slash_bps = 0;
    pending.stake_escrow_account_id = delegator.clone();
    pending.slash_sink_account_id = delegator.clone();
    transaction
        .world
        .set_parameter_for_testing(Parameter::Custom(
            pending.clone().into_custom_parameter().unwrap(),
        ))
        .unwrap();
    assert_eq!(
        transaction
            .world
            .parameters
            .get()
            .require_staking_policy()
            .unwrap(),
        &pending
    );
    assert_eq!(transaction.staking_monetary_policy().unwrap(), &original);

    RegisterPublicLaneValidator::new(
        lane,
        validator.clone(),
        validator_peer_id(&validator),
        validator.clone(),
        Quantity::from(1_000_u64),
        Metadata::default(),
        fixture_registration_plan(&transaction, lane, &validator, Quantity::from(1_000_u64)),
    )
    .execute(&validator, &mut transaction)
    .expect("original minimum and custody admit registration");
    BondPublicLaneStake {
        monetary_plan: fixture_bond_plan(
            &transaction,
            lane,
            &validator,
            &delegator,
            Quantity::from(50_u64),
        ),
        lane_id: lane,
        validator: validator.clone(),
        staker: delegator.clone(),
        amount: Quantity::from(50_u64),
        metadata: Metadata::default(),
    }
    .execute(&delegator, &mut transaction)
    .expect("original share cap admits the second share");
    for label in ["original-policy-unbond-1", "original-policy-unbond-2"] {
        SchedulePublicLaneUnbond {
            lane_id: lane,
            validator: validator.clone(),
            staker: validator.clone(),
            request_id: Hash::new(label),
            amount: Quantity::from(100_u64),
            release_at_ms: transaction.block_unix_timestamp_ms(),
        }
        .execute(&validator, &mut transaction)
        .expect("original delay and pending cap admit both requests");
    }
    let source = AssetId::new(definition.clone(), escrow);
    let destination = AssetId::new(definition.clone(), ALICE_ID.clone());
    assert!(
        is_retained_staking_slash_movement(&transaction, lane, &validator, &source, &destination)
            .unwrap()
    );
    assert!(
        !is_retained_staking_slash_movement(
            &transaction,
            lane,
            &validator,
            &source,
            &AssetId::new(definition.clone(), delegator.clone())
        )
        .unwrap()
    );
    let now_ms = transaction.block_unix_timestamp_ms();
    apply_slash_to_validator_without_observability(
        &mut transaction,
        lane,
        &validator,
        Hash::new("original-policy-slash"),
        &Quantity::from(100_u64),
        now_ms,
    )
    .expect("original slash ratio and receiver govern the transfer");
    assert_eq!(
        transaction.world.assets.get(&source).unwrap().as_ref(),
        &Quantity::from(950_u64)
    );
    assert_eq!(
        transaction.world.assets.get(&destination).unwrap().as_ref(),
        &Quantity::from(100_u64)
    );
    assert_eq!(
        transaction
            .world
            .assets
            .get(&AssetId::new(definition, delegator))
            .unwrap()
            .as_ref(),
        &Quantity::from(9_950_u64)
    );
    assert_eq!(
        transaction
            .world
            .public_lane_stake_shares
            .get(&stake_key(lane, &validator, &validator))
            .unwrap()
            .pending_unbonds
            .len(),
        2
    );
}
