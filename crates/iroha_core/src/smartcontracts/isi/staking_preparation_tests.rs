// Same-scope regression coverage for bounded observational plan construction.

#[test]
fn prepared_reward_claim_uses_exact_epoch_zero_hash_cursor_dust_and_execution() {
    use iroha_data_model::nexus::{PublicLaneMonetaryScopeV1, PublicLanePrepareClaimV1};
    let state = setup_state();
    let block = new_block();
    let mut state_block = state.block(block.as_ref().header());
    let mut stx = state_block.transaction();
    seed_test_call_hash(&mut stx, 0xDA);
    let lane = LaneId::SINGLE;
    let (sink, recipient, asset, _) = configure_reward_fixture(&mut stx, lane, 100);
    stx.nexus.staking.reward_dust_threshold = Quantity::from(50_u64);
    reward_distribution(lane, 0, &asset, &recipient, 25)
        .execute(&sink, &mut stx)
        .unwrap();
    reward_distribution(lane, 1, &asset, &recipient, 30)
        .execute(&sink, &mut stx)
        .unwrap();
    let mut intent = PublicLanePrepareClaimV1 {
        recipient: recipient.clone(),
        upto_epoch: None,
        max_records: 1,
        accrued_sources: vec![],
    };
    let build = |stx: &StateTransaction<'_, '_>, intent: &PublicLanePrepareClaimV1| {
        preparation::claim_plan(
            &stx.world,
            &stx.nexus.staking.reward_dust_threshold,
            lane,
            intent,
            PublicLaneMonetaryScopeV1::Network(*stx.network_id()),
            stx.block_height() + 1,
        )
        .unwrap()
    };
    let first = build(&stx, &intent);
    assert_eq!(first.expected_state, None);
    assert_eq!(first.records.len(), 1);
    assert_eq!(first.records[0].epoch, 0);
    assert_eq!(
        first.records[0].record_hash,
        iroha_data_model::nexus::public_lane_reward_record_commitment(
            stx.world.public_lane_rewards.get(&(lane, 0)).unwrap()
        )
        .unwrap()
    );
    assert_eq!(first.sources[0].payout, Quantity::zero());
    effects::prepare_reward_claim(&stx, lane, &recipient, &first).unwrap();
    ClaimPublicLaneRewards {
        lane_id: lane,
        account: recipient.clone(),
        claim_plan: first.clone(),
    }
    .execute(&recipient, &mut stx)
    .unwrap();
    assert!(
        effects::prepare_reward_claim(&stx, lane, &recipient, &first)
            .err()
            .unwrap()
            .to_string()
            .contains("processing cursor")
    );
    let next = build(&stx, &intent);
    assert_eq!(next.expected_state.as_ref().unwrap().through_epoch, Some(0));
    assert_eq!(next.records[0].epoch, 1);
    assert_eq!(
        next.sources[0].expected_accrued,
        Some(Quantity::from(25_u64))
    );
    assert_eq!(next.sources[0].payout, Quantity::from(55_u64));
    let mut wrong = next.clone();
    wrong.sources[0].payout = Quantity::from(54_u64);
    assert!(effects::prepare_reward_claim(&stx, lane, &recipient, &wrong).is_err());
    ClaimPublicLaneRewards {
        lane_id: lane,
        account: recipient.clone(),
        claim_plan: next,
    }
    .execute(&recipient, &mut stx)
    .unwrap();
    intent.upto_epoch = Some(0);
    assert!(
        preparation::claim_plan(
            &stx.world,
            &Quantity::zero(),
            lane,
            &intent,
            PublicLaneMonetaryScopeV1::Network(*stx.network_id()),
            stx.block_height() + 1
        )
        .is_err()
    );
}

#[test]
fn prepared_reward_claim_bounds_work_and_requires_exact_existing_accrual_sources() {
    use iroha_data_model::nexus::{PublicLaneMonetaryScopeV1, PublicLanePrepareClaimV1};
    let state = setup_state();
    let block = new_block();
    let mut state_block = state.block(block.as_ref().header());
    let mut stx = state_block.transaction();
    let lane = LaneId::SINGLE;
    let (_, recipient, asset, _) = configure_reward_fixture(&mut stx, lane, 100);
    let mut intent = PublicLanePrepareClaimV1 {
        recipient: recipient.clone(),
        upto_epoch: None,
        max_records: 65,
        accrued_sources: vec![],
    };
    let build = |intent: &PublicLanePrepareClaimV1| {
        preparation::claim_plan(
            &stx.world,
            &Quantity::from(50_u64),
            lane,
            intent,
            PublicLaneMonetaryScopeV1::Network(*stx.network_id()),
            stx.block_height() + 1,
        )
    };
    assert!(build(&intent).is_err());
    intent.max_records = 0;
    intent.accrued_sources = vec![asset.clone()];
    assert!(build(&intent).is_err());
    stx.world
        .public_lane_reward_accruals
        .insert((lane, recipient, asset.clone()), Quantity::from(25_u64));
    let plan = preparation::claim_plan(
        &stx.world,
        &Quantity::from(50_u64),
        lane,
        &intent,
        PublicLaneMonetaryScopeV1::Network(*stx.network_id()),
        stx.block_height() + 1,
    )
    .unwrap();
    assert!(plan.records.is_empty());
    assert_eq!(plan.sources[0].payout, Quantity::zero());
    intent.accrued_sources.push(asset);
    assert!(
        preparation::claim_plan(
            &stx.world,
            &Quantity::zero(),
            lane,
            &intent,
            PublicLaneMonetaryScopeV1::Network(*stx.network_id()),
            stx.block_height() + 1
        )
        .is_err()
    );
}

#[test]
fn global_staking_eligibility_uses_unequal_authenticated_intervals() {
    // E has an irregular absolute offset; frozen E+1 is ten blocks, while
    // the next projected/frozen interval is thirty. Genesis modulus is wrong.
    for (ready, expected) in [
        (101, 141),
        (129, 141),
        (130, 171),
        (139, 171),
        (140, 201),
        (169, 201),
        (170, 231),
        (200, 261),
    ] {
        assert_eq!(
            global_eligibility_from_intervals(101, 130, 140, 170, ready, 30).unwrap(),
            expected
        );
    }
    // An existing frozen E+2 keeps its twenty-block interval even after a
    // five-block configuration update; only subsequent unfrozen epochs use five.
    assert_eq!(
        global_eligibility_from_intervals(101, 130, 140, 160, 130, 5).unwrap(),
        161
    );
    assert_eq!(
        global_eligibility_from_intervals(101, 130, 140, 160, 140, 5).unwrap(),
        166
    );
    assert!(global_eligibility_from_intervals(101, 130, 140, 160, 100, 5).is_err());
    assert!(global_eligibility_from_intervals(101, 130, 130, 160, 101, 5).is_err());
    assert!(global_eligibility_from_intervals(1, 10, 20, u64::MAX, 10, 5).is_err());
}

#[test]
fn global_staking_preparation_does_not_invent_missing_incumbent_finality() {
    use iroha_data_model::nexus::{
        PublicLanePreparationOperationV1, PublicLanePreparationRequestV1, PublicLanePrepareClaimV1,
    };
    let state = setup_state();
    let block = new_block();
    let mut state_block = state.block(block.as_ref().header());
    let mut stx = state_block.transaction();
    let (_, recipient, _, _) = configure_reward_fixture(&mut stx, LaneId::SINGLE, 100);
    let error = validator_eligibility_height(&stx, LaneId::SINGLE, 2, 2).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("authenticated incumbent finality")
    );
    let request = PublicLanePreparationRequestV1 {
        lane_id: LaneId::SINGLE,
        valid_for_blocks: 0,
        operation: PublicLanePreparationOperationV1::ClaimRewards(PublicLanePrepareClaimV1 {
            recipient,
            upto_epoch: None,
            max_records: 1,
            accrued_sources: vec![],
        }),
    };
    assert!(
        preparation::prepare_public_lane_plan(&stx, request)
            .unwrap_err()
            .to_string()
            .contains("validity")
    );
}
