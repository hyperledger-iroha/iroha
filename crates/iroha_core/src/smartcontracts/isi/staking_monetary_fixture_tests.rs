// Exact observed monetary plans for the staking execution fixtures.
// These helpers only construct signed inputs; execution must independently verify them.
use iroha_data_model::nexus::{
    PublicLaneMonetaryScopeV1, PublicLaneRewardClaimPlanV1, PublicLaneRewardClaimSourceV1,
    PublicLaneRewardClaimStateV1, PublicLaneRewardRecordRefV1,
    public_lane_reward_record_commitment,
};

#[test]
fn genesis_monetary_scope_requires_exact_height_without_npos_parameters() {
    let state = setup_state();
    let mut genesis = state.block(block_header_with_height(1));
    let mut stx = genesis.transaction();
    stx.world
        .parameters
        .get_mut()
        .custom
        .remove(&SumeragiNposParameters::parameter_id());
    assert!(stx.world.sumeragi_npos_parameters().is_none());
    assert!(effects::validate_plan_context(&stx, &PublicLaneMonetaryScopeV1::Genesis, 1).is_ok());
    for invalid_height in [0, 2] {
        let error = effects::validate_plan_context(
            &stx,
            &PublicLaneMonetaryScopeV1::Genesis,
            invalid_height,
        )
        .expect_err("genesis consent must expire at its exact height");
        assert!(error.to_string().contains("genesis height"));
    }
    let error = effects::validate_plan_context(
        &stx,
        &PublicLaneMonetaryScopeV1::Network(*stx.network_id()),
        1,
    )
    .expect_err("network consent still requires a committed epoch schedule");
    assert!(
        error
            .to_string()
            .contains("committed NPoS epoch parameters")
    );

    drop(stx);
    drop(genesis);
    let mut next_block = state.block(block_header_with_height(2));
    let next_stx = next_block.transaction();
    let error = effects::validate_plan_context(&next_stx, &PublicLaneMonetaryScopeV1::Genesis, 2)
        .expect_err("genesis scope must not authorize a later block");
    assert!(
        error
            .to_string()
            .contains("authenticated genesis or network scope")
    );
}

fn fixture_transfer_plan(
    stx: &StateTransaction<'_, '_>,
    source_asset: AssetId,
    destination_asset: AssetId,
    amount: Quantity,
    precondition: PublicLaneMonetaryPreconditionV1,
) -> PublicLaneMonetaryPlanV1 {
    PublicLaneMonetaryPlanV1 {
        network_scope: PublicLaneMonetaryScopeV1::Network(*stx.network_id()),
        valid_until_height: stx.block_height(),
        source_asset,
        destination_asset,
        amount,
        precondition,
    }
}

fn fixture_registration_plan(
    stx: &StateTransaction<'_, '_>,
    staker: &AccountId,
    amount: Quantity,
) -> PublicLaneMonetaryPlanV1 {
    let context = stake_context(
        &stx.world,
        &stx.nexus.dataspace_catalog,
        &stx.nexus.staking,
        staker,
        stx.block_unix_timestamp_ms(),
    )
    .expect("fixture configured registration custody");
    fixture_transfer_plan(
        stx,
        context.staker_asset,
        context.escrow_asset,
        amount,
        PublicLaneMonetaryPreconditionV1::Registration(PublicLaneMonetaryRegistrationV1 {
            activation_height: scheduled_validator_eligibility_height(stx)
                .expect("fixture election height"),
        }),
    )
}

fn fixture_bond_plan(
    stx: &StateTransaction<'_, '_>,
    lane: LaneId,
    validator: &AccountId,
    staker: &AccountId,
    amount: Quantity,
) -> PublicLaneMonetaryPlanV1 {
    let context = stake_context(
        &stx.world,
        &stx.nexus.dataspace_catalog,
        &stx.nexus.staking,
        staker,
        stx.block_unix_timestamp_ms(),
    )
    .expect("fixture configured bond custody");
    let record = stx
        .world
        .public_lane_validators
        .get(&(lane, validator.clone()))
        .expect("fixture validator tenure");
    fixture_transfer_plan(
        stx,
        context.staker_asset,
        context.escrow_asset,
        amount,
        PublicLaneMonetaryPreconditionV1::Bond(PublicLaneMonetaryBondV1 {
            activation_height: record.activation_height,
            peer_id: record.peer_id.clone(),
        }),
    )
}

fn fixture_unbond_plan(
    stx: &StateTransaction<'_, '_>,
    lane: LaneId,
    validator: &AccountId,
    staker: &AccountId,
    request_id: Hash,
) -> PublicLaneMonetaryPlanV1 {
    let source = stx
        .world
        .public_lane_stake_custody
        .get(&(lane, validator.clone()))
        .expect("fixture retained withdrawal custody")
        .0
        .clone();
    let destination =
        AssetId::with_scope(source.definition().clone(), staker.clone(), *source.scope());
    let record = stx
        .world
        .public_lane_validators
        .get(&(lane, validator.clone()))
        .expect("fixture validator tenure");
    let request = stx
        .world
        .public_lane_stake_shares
        .get(&stake_key(lane, validator, staker))
        .expect("fixture withdrawal share")
        .pending_unbonds
        .get(&request_id)
        .expect("fixture pending withdrawal");
    fixture_transfer_plan(
        stx,
        source,
        destination,
        request.amount.clone(),
        PublicLaneMonetaryPreconditionV1::Unbond(PublicLaneMonetaryUnbondV1 {
            activation_height: record.activation_height,
            request_hash: public_lane_unbonding_commitment(request)
                .expect("fixture withdrawal commitment"),
        }),
    )
}

fn fixture_slash_plan(
    stx: &StateTransaction<'_, '_>,
    lane: LaneId,
    validator: &AccountId,
    offence_height: u64,
    amount: Quantity,
) -> PublicLaneMonetaryPlanV1 {
    let source = stx
        .world
        .public_lane_stake_custody
        .get(&(lane, validator.clone()))
        .expect("fixture retained slash custody")
        .0
        .clone();
    let receiver = parse_staking_account_literal(
        &stx.world,
        &stx.nexus.dataspace_catalog,
        &stx.nexus.staking.slash_sink_account_id,
        "slash_sink_account_id",
        stx.block_unix_timestamp_ms(),
    )
    .expect("fixture slash receiver");
    let destination = AssetId::with_scope(source.definition().clone(), receiver, *source.scope());
    let record = stx
        .world
        .public_lane_validators
        .get(&(lane, validator.clone()))
        .expect("fixture validator tenure");
    let mut exposure = Quantity::zero();
    for (_, share) in stx
        .world
        .public_lane_stake_shares
        .iter()
        .filter(|(key, share)| {
            key.0 == lane && &key.1 == validator && public_lane_stake_share_matches_key(key, share)
        })
    {
        exposure = exposure
            .checked_add(&share.bonded)
            .expect("fixture bonded exposure");
        for pending in share
            .pending_unbonds
            .values()
            .filter(|pending| offence_height <= pending.slashable_through_height)
        {
            exposure = exposure
                .checked_add(&pending.amount)
                .expect("fixture pending exposure");
        }
    }
    fixture_transfer_plan(
        stx,
        source,
        destination,
        amount,
        PublicLaneMonetaryPreconditionV1::Slash(PublicLaneMonetarySlashV1 {
            activation_height: record.activation_height,
            slashable_exposure: exposure,
        }),
    )
}

fn fixture_reward_claim_plan(
    stx: &StateTransaction<'_, '_>,
    lane: LaneId,
    recipient: &AccountId,
    last_epoch: Option<u64>,
) -> PublicLaneRewardClaimPlanV1 {
    let expected_state = stx
        .world
        .public_lane_reward_claims
        .get(&(lane, recipient.clone()))
        .copied();
    let through = expected_state.and_then(|state| state.through_epoch);
    let mut accrued = BTreeMap::<AssetId, Quantity>::new();
    for ((source_lane, account, asset), amount) in stx.world.public_lane_reward_accruals.iter() {
        if *source_lane == lane && account == recipient {
            accrued.insert(asset.clone(), amount.clone());
        }
    }
    let mut records = Vec::new();
    for ((record_lane, epoch), record) in stx.world.public_lane_rewards.iter() {
        if *record_lane != lane
            || through.is_some_and(|previous| *epoch <= previous)
            || last_epoch.is_some_and(|last| *epoch > last)
        {
            continue;
        }
        records.push(PublicLaneRewardRecordRefV1 {
            epoch: *epoch,
            record_hash: public_lane_reward_record_commitment(record)
                .expect("fixture reward commitment"),
        });
        let amount = accrued
            .entry(record.asset.clone())
            .or_insert_with(Quantity::zero);
        for share in record
            .shares
            .iter()
            .filter(|share| &share.account == recipient)
        {
            *amount = amount
                .checked_add(&share.amount)
                .expect("fixture reward accrual");
        }
    }
    assert!(records.len() <= iroha_data_model::nexus::MAX_PUBLIC_LANE_REWARD_CLAIM_RECORDS);
    assert!(accrued.len() <= iroha_data_model::nexus::MAX_PUBLIC_LANE_REWARD_CLAIM_SOURCES);
    let sources = accrued
        .into_iter()
        .map(|(source_asset, amount)| {
            let expected_accrued = stx
                .world
                .public_lane_reward_accruals
                .get(&(lane, recipient.clone(), source_asset.clone()))
                .cloned();
            let payout = if amount >= stx.nexus.staking.reward_dust_threshold {
                amount
            } else {
                Quantity::zero()
            };
            PublicLaneRewardClaimSourceV1 {
                destination_asset: AssetId::with_scope(
                    source_asset.definition().clone(),
                    recipient.clone(),
                    *source_asset.scope(),
                ),
                source_asset,
                expected_accrued,
                payout,
            }
        })
        .collect();
    PublicLaneRewardClaimPlanV1 {
        network_scope: PublicLaneMonetaryScopeV1::Network(*stx.network_id()),
        valid_until_height: stx.block_height(),
        expected_state,
        records,
        sources,
    }
}

#[test]
fn registration_rejects_changed_signed_monetary_fields_without_custody_writes() {
    let state = setup_state();
    let mut block = state.block(block_header_with_height(1));
    let mut stx = block.transaction();
    let (validator, recipient, escrow, definition) = prepare_accounts(&mut stx);
    let lane = LaneId::new(42);
    let instruction = RegisterPublicLaneValidator::new(
        lane,
        validator.clone(),
        validator_peer_id(&validator),
        validator.clone(),
        Quantity::from(1_000_u64),
        Metadata::default(),
        fixture_registration_plan(&stx, &validator, Quantity::from(1_000_u64)),
    );
    let source = AssetId::new(definition.clone(), validator.clone());
    let destination = AssetId::new(definition.clone(), escrow);
    let balance = stx.world.assets.get(&source).cloned();
    for mutation in 0..7 {
        let mut altered = instruction.clone();
        match mutation {
            0 => {
                altered.monetary_plan.network_scope = PublicLaneMonetaryScopeV1::Network(
                    iroha_data_model::NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                        Hash::new("different fixture genesis"),
                    )),
                )
            }
            1 => altered.monetary_plan.valid_until_height = 0,
            2 => {
                altered.monetary_plan.valid_until_height = stx.block_height()
                    + stx
                        .world
                        .sumeragi_npos_parameters()
                        .unwrap()
                        .epoch_length_blocks
                        .get()
                    + 1
            }
            3 => {
                altered.monetary_plan.source_asset =
                    AssetId::new(definition.clone(), recipient.clone())
            }
            4 => {
                altered.monetary_plan.destination_asset =
                    AssetId::new(definition.clone(), recipient.clone())
            }
            5 => altered.monetary_plan.amount = Quantity::from(999_u64),
            6 => {
                altered.monetary_plan.precondition = PublicLaneMonetaryPreconditionV1::Registration(
                    PublicLaneMonetaryRegistrationV1 {
                        activation_height: 2,
                    },
                )
            }
            _ => unreachable!(),
        }
        let error = altered
            .execute(&validator, &mut stx)
            .expect_err("changed plan must reject");
        assert!(
            error.to_string().contains("monetary plan"),
            "mutation {mutation}: {error}"
        );
        assert_eq!(stx.world.assets.get(&source), balance.as_ref());
        assert!(stx.world.assets.get(&destination).is_none());
        assert!(
            stx.world
                .public_lane_validators
                .get(&(lane, validator.clone()))
                .is_none()
        );
        assert!(
            stx.world
                .public_lane_stake_custody
                .get(&(lane, validator.clone()))
                .is_none()
        );
        assert!(
            stx.world
                .public_lane_stake_reserves
                .get(&destination)
                .is_none()
        );
    }
    instruction
        .execute(&validator, &mut stx)
        .expect("unchanged exact plan must register");
}

#[test]
fn reward_claim_rejects_changed_record_source_and_entitlement_without_payment() {
    let state = setup_state();
    let mut block = state.block(block_header_with_height(1));
    let mut stx = block.transaction();
    seed_test_call_hash(&mut stx, 0xBA);
    let lane = LaneId::SINGLE;
    let (sink, recipient, asset, definition) = configure_reward_fixture(&mut stx, lane, 100);
    stx.nexus.staking.reward_dust_threshold = Quantity::zero();
    reward_distribution(lane, 0, &asset, &recipient, 25)
        .execute(&sink, &mut stx)
        .unwrap();
    let instruction = ClaimPublicLaneRewards {
        lane_id: lane,
        account: recipient.clone(),
        claim_plan: fixture_reward_claim_plan(&stx, lane, &recipient, Some(0)),
    };
    let transcripts_before = stx.pending_transfer_transcript_count_for_testing();
    for mutation in 0..8 {
        let mut altered = instruction.clone();
        match mutation {
            0 => altered.claim_plan.records[0].record_hash = Hash::new("changed reward record"),
            1 => altered.claim_plan.records[0].epoch = 1,
            2 => {
                altered.claim_plan.sources[0].source_asset =
                    AssetId::new(definition.clone(), ALICE_ID.clone())
            }
            3 => {
                altered.claim_plan.sources[0].destination_asset =
                    AssetId::new(definition.clone(), sink.clone())
            }
            4 => altered.claim_plan.sources[0].payout = Quantity::from(26_u64),
            5 => altered.claim_plan.sources[0].expected_accrued = Some(Quantity::one()),
            6 => altered.claim_plan.valid_until_height = 0,
            7 => {
                altered.claim_plan.expected_state = Some(PublicLaneRewardClaimStateV1 {
                    through_epoch: Some(0),
                })
            }
            _ => unreachable!(),
        }
        altered
            .execute(&recipient, &mut stx)
            .expect_err("changed signed claim must reject");
        assert_eq!(
            stx.world.assets.get(&asset).unwrap().as_ref(),
            &Quantity::from(100_u64)
        );
        assert_eq!(
            stx.world.public_lane_reward_reserves.get(&asset),
            Some(&Quantity::from(25_u64))
        );
        assert!(
            stx.world
                .public_lane_reward_claims
                .get(&(lane, recipient.clone()))
                .is_none()
        );
        assert!(
            stx.world
                .public_lane_reward_accruals
                .iter()
                .next()
                .is_none()
        );
        assert!(
            stx.world
                .assets
                .get(&AssetId::new(definition.clone(), recipient.clone()))
                .is_none()
        );
        assert_eq!(
            stx.pending_transfer_transcript_count_for_testing(),
            transcripts_before
        );
    }
    instruction
        .execute(&recipient, &mut stx)
        .expect("unchanged exact claim must pay");
}
