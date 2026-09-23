//! Reward obligations retained until the recipient actually receives payment.

use super::*;

/// Sum unpaid rewards for one exact custody asset across all serviced lanes.
#[cfg(test)]
pub(super) fn outstanding_rewards(
    world: &impl WorldReadOnly,
    asset: &AssetId,
) -> Result<Quantity, Error> {
    let mut outstanding = Quantity::zero();
    for (key, record) in world.public_lane_rewards().iter() {
        if &record.asset != asset {
            continue;
        }
        if !public_lane_reward_record_matches_key(key, record) {
            return Err(Error::InvariantViolation(
                "reward reserve contains a non-canonical reward record".into(),
            ));
        }
        for share in &record.shares {
            let claimed = world
                .public_lane_reward_claims()
                .get(&(key.0, share.account.clone()));
            if claimed
                .and_then(|state| state.through_epoch)
                .is_some_and(|epoch| epoch >= key.1)
            {
                continue;
            }
            outstanding = quantity_add(outstanding, share.amount.clone())?;
        }
    }
    // Processing a reward record can leave unpaid dust in its exact custody
    // source. Advancing the lane cursor alone never means that dust was paid.
    for ((_, _, source), accrued) in world.public_lane_reward_accruals().iter() {
        if source == asset {
            outstanding = quantity_add(outstanding, accrued.clone())?;
        }
    }
    Ok(outstanding)
}

/// Prevent ordinary and native debits from spending stake or unpaid rewards.
pub(crate) fn ensure_public_lane_reserves_after_debit(
    world: &impl WorldReadOnly,
    asset: &AssetId,
    balance_after: &Quantity,
) -> Result<(), Error> {
    let rewards = world
        .public_lane_reward_reserves()
        .get(asset)
        .cloned()
        .unwrap_or_else(Quantity::zero);
    let stake = world
        .public_lane_stake_reserves()
        .get(asset)
        .cloned()
        .unwrap_or_else(Quantity::zero);
    if balance_after < &quantity_add(rewards, stake)? {
        return Err(Error::InvariantViolation(
            "asset debit would spend reserved public-lane rewards or stake custody".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::World;
    use iroha_data_model::nexus::{
        PublicLaneRewardClaimStateV1, PublicLaneRewardRecord, PublicLaneRewardRole,
        PublicLaneRewardShare,
    };
    use iroha_test_samples::{ALICE_ID, BOB_ID};

    #[test]
    fn outstanding_rewards_retains_processed_dust_in_its_exact_source() {
        let definition = AssetDefinitionId::derive_from_components(
            iroha_model_base::domain::DomainId::try_new("rewards", "universal").unwrap(),
            "fee".parse().unwrap(),
        );
        let global = AssetId::new(definition.clone(), ALICE_ID.clone());
        let scoped = AssetId::with_scope(
            definition,
            ALICE_ID.clone(),
            iroha_data_model::asset::AssetBalanceScope::Dataspace(
                iroha_model_base::topology::DataSpaceId::new(7),
            ),
        );
        let lane = iroha_model_base::topology::LaneId::SINGLE;
        let mut world = World::new();
        for (epoch, asset, amount) in [(0, global.clone(), 5_u32), (1, scoped.clone(), 7)] {
            world.public_lane_rewards.insert(
                (lane, epoch),
                PublicLaneRewardRecord {
                    lane_id: lane,
                    epoch,
                    asset,
                    total_reward: Quantity::from(amount),
                    shares: vec![PublicLaneRewardShare {
                        account: BOB_ID.clone(),
                        role: PublicLaneRewardRole::Validator,
                        amount: Quantity::from(amount),
                    }],
                    metadata: iroha_model_base::metadata::Metadata::default(),
                },
            );
        }
        world.public_lane_reward_claims.insert(
            (lane, BOB_ID.clone()),
            PublicLaneRewardClaimStateV1 {
                through_epoch: Some(0),
            },
        );
        world.public_lane_reward_accruals.insert(
            (lane, BOB_ID.clone(), global.clone()),
            Quantity::from(2_u32),
        );
        assert_eq!(
            outstanding_rewards(&world.view(), &global).unwrap(),
            Quantity::from(2_u32)
        );
        assert_eq!(
            outstanding_rewards(&world.view(), &scoped).unwrap(),
            Quantity::from(7_u32)
        );
        world.public_lane_reward_claims.insert(
            (lane, BOB_ID.clone()),
            PublicLaneRewardClaimStateV1 {
                through_epoch: Some(1),
            },
        );
        world.public_lane_reward_accruals.insert(
            (lane, BOB_ID.clone(), scoped.clone()),
            Quantity::from(3_u32),
        );
        assert_eq!(
            outstanding_rewards(&world.view(), &global).unwrap(),
            Quantity::from(2_u32)
        );
        assert_eq!(
            outstanding_rewards(&world.view(), &scoped).unwrap(),
            Quantity::from(3_u32)
        );
    }
}
