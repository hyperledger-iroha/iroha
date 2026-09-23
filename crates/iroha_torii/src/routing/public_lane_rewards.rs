//! Pending reward projection over one processing cursor and exact custody-source accruals.

use super::*;
use iroha_data_model::nexus::{PublicLanePendingReward, PublicLaneRewardClaimStateV1};

pub(super) fn collect_pending_public_lane_rewards<'a>(
    lane_id: LaneId,
    account_id: &AccountId,
    upto_epoch: u64,
    asset_filter: Option<&AssetId>,
    claim_state: Option<&PublicLaneRewardClaimStateV1>,
    accruals: impl Iterator<Item = (&'a (LaneId, AccountId, AssetId), &'a Quantity)>,
    rewards: impl Iterator<Item = (&'a (LaneId, u64), &'a PublicLaneRewardRecord)>,
) -> Result<Vec<PublicLanePendingReward>, Error> {
    let processed_through_epoch = claim_state.and_then(|state| state.through_epoch);
    if processed_through_epoch.is_some_and(|epoch| upto_epoch < epoch) {
        return Err(conversion_error(
            "upto_epoch precedes the current reward processing cursor; historical accrual is unavailable".into(),
        ));
    }
    let mut totals: BTreeMap<AssetId, (Quantity, u64)> = BTreeMap::new();
    for ((lane, account, asset), amount) in accruals {
        if *lane != lane_id || account != account_id || amount.is_zero() {
            continue;
        }
        if asset_filter.is_some_and(|filter| filter != asset) {
            continue;
        }
        let Some(epoch) = processed_through_epoch else {
            return Err(conversion_error(
                "retained reward accrual has no processed reward cursor".into(),
            ));
        };
        totals.insert(asset.clone(), (amount.clone(), epoch));
    }
    for (key, record) in rewards {
        let (lane, epoch) = key;
        if *lane != lane_id {
            continue;
        }
        if *epoch > upto_epoch {
            break;
        }
        if !public_lane_reward_record_matches_key(key, record)
            || processed_through_epoch.is_some_and(|processed| *epoch <= processed)
            || asset_filter.is_some_and(|filter| &record.asset != filter)
        {
            continue;
        }
        for share in record
            .shares
            .iter()
            .filter(|share| &share.account == account_id)
        {
            let entry = totals
                .entry(record.asset.clone())
                .or_insert_with(|| (Quantity::zero(), *epoch));
            entry.0 = entry
                .0
                .checked_add(&share.amount)
                .map_err(|_| conversion_error("pending reward amount overflowed".into()))?;
            entry.1 = entry.1.max(*epoch);
        }
    }
    Ok(totals
        .into_iter()
        .filter(|(_, (amount, _))| !amount.is_zero())
        .map(
            |(asset, (amount, pending_through_epoch))| PublicLanePendingReward {
                lane_id,
                account: account_id.clone(),
                asset,
                processed_through_epoch,
                pending_through_epoch,
                amount,
            },
        )
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(seed: u8) -> AccountId {
        AccountId::new(
            checked_routing_fixture_keypair(
                seed,
                Algorithm::Ed25519,
                "pending reward projection fixture",
            )
            .public_key()
            .clone(),
        )
    }

    fn source(owner: &AccountId, scope: u64) -> AssetId {
        AssetId::with_scope(
            test_asset_definition_id_from_hex("550e8400e29b41d4a7164466554400bb"),
            owner.clone(),
            if scope == 0 {
                iroha_data_model::asset::AssetBalanceScope::Global
            } else {
                iroha_data_model::asset::AssetBalanceScope::Dataspace(
                    iroha_model_base::topology::DataSpaceId::new(scope),
                )
            },
        )
    }

    fn record(
        lane: LaneId,
        epoch: u64,
        asset: AssetId,
        account: &AccountId,
        amount: u64,
    ) -> PublicLaneRewardRecord {
        PublicLaneRewardRecord {
            lane_id: lane,
            epoch,
            asset,
            total_reward: Quantity::from(amount),
            shares: vec![PublicLaneRewardShare {
                account: account.clone(),
                role: PublicLaneRewardRole::Nominator,
                amount: Quantity::from(amount),
            }],
            metadata: Metadata::default(),
        }
    }

    #[test]
    fn reward_json_distinguishes_unprocessed_from_processed_epoch_zero() {
        let recipient = account(0x77);
        let asset = source(&account(0x78), 0);
        for (processed, expected) in [(None, Value::Null), (Some(0), Value::from(0_u64))] {
            let (_, value) = pending_reward_to_json(PublicLanePendingReward {
                lane_id: LaneId::SINGLE,
                account: recipient.clone(),
                asset: asset.clone(),
                processed_through_epoch: processed,
                pending_through_epoch: 0,
                amount: Quantity::from(2_u64),
            });
            let fields = value.as_object().unwrap();
            assert_eq!(fields.get("processed_through_epoch"), Some(&expected));
            assert!(!fields.contains_key("last_claimed_epoch"));
            assert_eq!(fields.get("asset"), Some(&Value::from(asset.to_string())));
        }
    }

    #[test]
    fn retained_dust_and_new_records_are_counted_once_per_exact_source() {
        let lane = LaneId::SINGLE;
        let recipient = account(0x71);
        let escrow = account(0x72);
        let first = source(&escrow, 0);
        let second = source(&escrow, 7);
        let other_custody = source(&account(0x73), 0);
        let state = PublicLaneRewardClaimStateV1 {
            through_epoch: Some(2),
        };
        let accruals = BTreeMap::from([
            (
                (lane, recipient.clone(), first.clone()),
                Quantity::from(2_u64),
            ),
            (
                (lane, recipient.clone(), second.clone()),
                Quantity::from(3_u64),
            ),
            (
                (lane, recipient.clone(), other_custody.clone()),
                Quantity::from(4_u64),
            ),
            (
                (LaneId::new(9), recipient.clone(), first.clone()),
                Quantity::from(99_u64),
            ),
            ((lane, account(0x74), first.clone()), Quantity::from(99_u64)),
        ]);
        let rewards = BTreeMap::from([
            ((lane, 0), record(lane, 0, first.clone(), &recipient, 100)),
            ((lane, 2), record(lane, 2, second.clone(), &recipient, 100)),
            ((lane, 3), record(lane, 3, first.clone(), &recipient, 5)),
            ((lane, 4), record(lane, 4, second.clone(), &recipient, 7)),
            ((lane, 5), record(lane, 5, first.clone(), &recipient, 99)),
        ]);
        let projected = collect_pending_public_lane_rewards(
            lane,
            &recipient,
            4,
            None,
            Some(&state),
            accruals.iter(),
            rewards.iter(),
        )
        .unwrap();
        assert_eq!(projected.len(), 3);
        for (asset, amount, epoch) in [
            (first.clone(), 7_u64, 3),
            (second.clone(), 10, 4),
            (other_custody.clone(), 4, 2),
        ] {
            let row = projected.iter().find(|row| row.asset == asset).unwrap();
            assert_eq!(row.amount, Quantity::from(amount));
            assert_eq!(row.processed_through_epoch, Some(2));
            assert_eq!(row.pending_through_epoch, epoch);
        }
        let filtered = collect_pending_public_lane_rewards(
            lane,
            &recipient,
            4,
            Some(&second),
            Some(&state),
            accruals.iter(),
            rewards.iter(),
        )
        .unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].asset, second);
        assert_eq!(filtered[0].amount, Quantity::from(10_u64));
        let at_cursor = collect_pending_public_lane_rewards(
            lane,
            &recipient,
            2,
            None,
            Some(&state),
            accruals.iter(),
            rewards.iter(),
        )
        .unwrap();
        assert_eq!(at_cursor.len(), 3);
        for (asset, amount) in [(first, 2_u64), (second, 3), (other_custody, 4)] {
            assert_eq!(
                at_cursor
                    .iter()
                    .find(|row| row.asset == asset)
                    .unwrap()
                    .amount,
                Quantity::from(amount)
            );
        }
        assert!(at_cursor.iter().all(|row| row.pending_through_epoch == 2));
        assert!(
            collect_pending_public_lane_rewards(
                lane,
                &recipient,
                1,
                None,
                Some(&state),
                accruals.iter(),
                rewards.iter(),
            )
            .is_err()
        );
    }

    #[test]
    fn retained_accrual_requires_a_real_processing_cursor_and_zero_stays_absent() {
        let lane = LaneId::SINGLE;
        let recipient = account(0x75);
        let asset = source(&account(0x76), 0);
        let mut accruals = BTreeMap::from([(
            (lane, recipient.clone(), asset.clone()),
            Quantity::from(2_u64),
        )]);
        for state in [
            None,
            Some(PublicLaneRewardClaimStateV1 {
                through_epoch: None,
            }),
        ] {
            assert!(
                collect_pending_public_lane_rewards(
                    lane,
                    &recipient,
                    u64::MAX,
                    None,
                    state.as_ref(),
                    accruals.iter(),
                    std::iter::empty(),
                )
                .is_err()
            );
        }
        let state = PublicLaneRewardClaimStateV1 {
            through_epoch: Some(0),
        };
        let rows = collect_pending_public_lane_rewards(
            lane,
            &recipient,
            0,
            None,
            Some(&state),
            accruals.iter(),
            std::iter::empty(),
        )
        .unwrap();
        assert_eq!(rows[0].processed_through_epoch, Some(0));
        assert_eq!(rows[0].pending_through_epoch, 0);
        accruals.insert((lane, recipient.clone(), asset), Quantity::zero());
        assert!(
            collect_pending_public_lane_rewards(
                lane,
                &recipient,
                0,
                None,
                Some(&state),
                accruals.iter(),
                std::iter::empty(),
            )
            .unwrap()
            .is_empty()
        );
    }
}
