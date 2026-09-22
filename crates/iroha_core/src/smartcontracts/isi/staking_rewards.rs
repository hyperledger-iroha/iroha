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
