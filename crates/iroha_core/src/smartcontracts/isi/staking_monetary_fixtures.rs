// Explicit signed plans prepared from each test's exact current custody and epoch state.
use iroha_data_model::nexus::{
    PublicLaneMonetaryScopeV1, PublicLaneRewardClaimPlanV1, PublicLaneRewardClaimSourceV1,
    PublicLaneRewardRecordRefV1, public_lane_reward_record_commitment,
};

fn fixture_plan_scope(stx: &StateTransaction<'_, '_>) -> PublicLaneMonetaryScopeV1 {
    if stx._curr_block.is_genesis() && stx.block_hashes.is_empty() {
        PublicLaneMonetaryScopeV1::Genesis
    } else {
        PublicLaneMonetaryScopeV1::Network(*stx.network_id())
    }
}

fn fixture_registration_plan(
    stx: &StateTransaction<'_, '_>,
    lane: LaneId,
    staker: &AccountId,
    amount: &Quantity,
) -> PublicLaneMonetaryPlanV1 {
    let context = stake_context(
        &stx.world,
        &stx.nexus.dataspace_catalog,
        &stx.nexus.staking,
        staker,
        stx.block_unix_timestamp_ms(),
    )
    .expect("fixture exact staking assets");
    PublicLaneMonetaryPlanV1 {
        network_scope: fixture_plan_scope(stx),
        valid_until_height: stx.block_height(),
        source_asset: context.staker_asset,
        destination_asset: context.escrow_asset,
        amount: amount.clone(),
        precondition: PublicLaneMonetaryPreconditionV1::Registration(
            iroha_data_model::nexus::PublicLaneRegistrationPreconditionV1 {
                activation_height: scheduled_validator_eligibility_height(stx, lane)
                    .expect("fixture election height"),
            },
        ),
    }
}

fn fixture_bond_plan(
    stx: &StateTransaction<'_, '_>,
    lane: LaneId,
    validator: &AccountId,
    staker: &AccountId,
    amount: &Quantity,
) -> PublicLaneMonetaryPlanV1 {
    let mut plan = fixture_registration_plan(stx, lane, staker, amount);
    let record = stx
        .world
        .public_lane_validators
        .get(&(lane, validator.clone()))
        .expect("fixture validator tenure");
    plan.precondition = PublicLaneMonetaryPreconditionV1::Bond(
        iroha_data_model::nexus::PublicLaneBondPreconditionV1 {
            activation_height: record.activation_height,
            peer_id: record.peer_id.clone(),
        },
    );
    plan
}

fn fixture_unbond_plan(
    stx: &StateTransaction<'_, '_>,
    lane: LaneId,
    validator: &AccountId,
    staker: &AccountId,
    request: &Hash,
) -> PublicLaneMonetaryPlanV1 {
    let record = stx
        .world
        .public_lane_validators
        .get(&(lane, validator.clone()))
        .expect("fixture validator tenure");
    let pending = stx
        .world
        .public_lane_stake_shares
        .get(&(lane, validator.clone(), staker.clone()))
        .and_then(|share| share.pending_unbonds.get(request))
        .expect("fixture retained unbond request");
    let context = retained_stake_context(&stx.world, lane, validator, staker)
        .expect("fixture retained unbond custody");
    PublicLaneMonetaryPlanV1 {
        network_scope: fixture_plan_scope(stx),
        valid_until_height: stx.block_height(),
        source_asset: context.escrow_asset,
        destination_asset: context.staker_asset,
        amount: pending.amount.clone(),
        precondition: PublicLaneMonetaryPreconditionV1::Unbond(
            iroha_data_model::nexus::PublicLaneUnbondPreconditionV1 {
                activation_height: record.activation_height,
                request_hash: public_lane_unbonding_commitment(pending)
                    .expect("fixture request commitment"),
            },
        ),
    }
}

fn fixture_slash_plan(
    stx: &StateTransaction<'_, '_>,
    lane: LaneId,
    validator: &AccountId,
    offence_height: u64,
    amount: &Quantity,
) -> PublicLaneMonetaryPlanV1 {
    let record = stx
        .world
        .public_lane_validators
        .get(&(lane, validator.clone()))
        .expect("fixture validator tenure");
    let context = retained_stake_context(&stx.world, lane, validator, &record.stake_account)
        .expect("fixture retained slash custody");
    let sink = parse_staking_account_literal(
        &stx.world,
        &stx.nexus.dataspace_catalog,
        &stx.nexus.staking.slash_sink_account_id,
        "slash_sink_account_id",
        stx.block_unix_timestamp_ms(),
    )
    .expect("fixture slash sink");
    let shares =
        validator_share_updates(&stx.world, lane, validator, None).expect("fixture slash shares");
    let exposure = slashable_exposure_from_shares(record, &shares, Some(offence_height))
        .expect("fixture slash exposure");
    PublicLaneMonetaryPlanV1 {
        network_scope: fixture_plan_scope(stx),
        valid_until_height: stx.block_height(),
        destination_asset: AssetId::with_scope(
            context.asset_definition,
            sink,
            *context.escrow_asset.scope(),
        ),
        source_asset: context.escrow_asset,
        amount: amount.clone(),
        precondition: PublicLaneMonetaryPreconditionV1::Slash(
            iroha_data_model::nexus::PublicLaneSlashPreconditionV1 {
                activation_height: record.activation_height,
                slashable_exposure: exposure,
            },
        ),
    }
}

fn fixture_claim_plan(
    stx: &StateTransaction<'_, '_>,
    lane: LaneId,
    recipient: &AccountId,
    upto_epoch: Option<u64>,
) -> PublicLaneRewardClaimPlanV1 {
    let expected_state = stx
        .world
        .public_lane_reward_claims
        .get(&(lane, recipient.clone()))
        .cloned();
    let cursor = expected_state
        .as_ref()
        .and_then(|state| state.through_epoch);
    let mut quantities: BTreeMap<AssetId, Quantity> = stx
        .world
        .public_lane_reward_accruals
        .iter()
        .filter(|((entry_lane, account, _), _)| *entry_lane == lane && account == recipient)
        .map(|((_, _, source), amount)| (source.clone(), amount.clone()))
        .collect();
    let records = stx
        .world
        .public_lane_rewards
        .range((lane, 0)..=(lane, upto_epoch.unwrap_or(u64::MAX)))
        .filter(|((_, epoch), _)| cursor.is_none_or(|cursor| *epoch > cursor))
        .take(iroha_data_model::nexus::MAX_PUBLIC_LANE_REWARD_CLAIM_RECORDS)
        .map(|((_, epoch), record)| {
            let total = quantities
                .entry(record.asset.clone())
                .or_insert_with(Quantity::zero);
            for share in record
                .shares
                .iter()
                .filter(|share| &share.account == recipient)
            {
                *total = quantity_add(total.clone(), share.amount.clone())
                    .expect("fixture reward total");
            }
            PublicLaneRewardRecordRefV1 {
                epoch: *epoch,
                record_hash: public_lane_reward_record_commitment(record)
                    .expect("fixture reward commitment"),
            }
        })
        .collect();
    let sources = quantities
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
        network_scope: fixture_plan_scope(stx),
        valid_until_height: stx.block_height(),
        expected_state,
        records,
        sources,
    }
}
