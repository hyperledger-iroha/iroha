//! Exact reward-reserve reconciliation for current and rollback snapshot cuts.

use super::*;

/// Validate exact unpaid entitlements and their retained custody backing.
pub(super) fn validate_public_lane_reward_reserves(
    world: &impl WorldReadOnly,
) -> Result<(), String> {
    let mut expected = BTreeMap::<AssetId, Quantity>::new();
    let mut processed = BTreeMap::<(LaneId, AccountId, AssetId), Quantity>::new();
    for ((lane, _), claim) in world.public_lane_reward_claims().iter() {
        let Some(epoch) = claim.through_epoch else {
            return Err("stored reward processing cursor is empty".to_owned());
        };
        if world.public_lane_rewards().get(&(*lane, epoch)).is_none() {
            return Err("reward processing cursor has no retained source record".to_owned());
        }
    }
    for (key, record) in world.public_lane_rewards().iter() {
        if !public_lane_reward_record_matches_key(key, record) {
            return Err("reward reserve source contains a noncanonical reward record".to_owned());
        }
        let mut record_total = Quantity::zero();
        for share in &record.shares {
            if share.amount.is_zero() {
                return Err("reward reserve source contains a zero reward share".to_owned());
            }
            record_total = record_total
                .checked_add(&share.amount)
                .map_err(|_| "reward reserve source total overflowed".to_owned())?;
            if world
                .public_lane_reward_claims()
                .get(&(key.0, share.account.clone()))
                .and_then(|claim| claim.through_epoch)
                .is_some_and(|through| through >= key.1)
            {
                let amount = processed
                    .entry((key.0, share.account.clone(), record.asset.clone()))
                    .or_insert_with(Quantity::zero);
                *amount = amount
                    .checked_add(&share.amount)
                    .map_err(|_| "processed reward source total overflowed".to_owned())?;
                continue;
            }
            let amount = expected
                .entry(record.asset.clone())
                .or_insert_with(Quantity::zero);
            *amount = amount
                .checked_add(&share.amount)
                .map_err(|_| "reward reserve source total overflowed".to_owned())?;
        }
        if record_total != record.total_reward || record_total.is_zero() {
            return Err("reward reserve source total does not match its shares".to_owned());
        }
    }
    for (key, accrued) in world.public_lane_reward_accruals().iter() {
        if accrued.is_zero() || processed.get(key).is_none_or(|total| accrued > total) {
            return Err(
                "reward accrual is zero, orphaned or exceeds its processed entitlement".to_owned(),
            );
        }
        let amount = expected.entry(key.2.clone()).or_insert_with(Quantity::zero);
        *amount = amount
            .checked_add(accrued)
            .map_err(|_| "accrued reward reserve total overflowed".to_owned())?;
    }
    if world
        .public_lane_reward_reserves()
        .iter()
        .ne(expected.iter())
    {
        return Err(
            "reward reserves do not match unprocessed reward records and unpaid accruals"
                .to_owned(),
        );
    }
    for (asset, reserved) in expected {
        let balance = world
            .assets()
            .get(&asset)
            .ok_or_else(|| format!("reward reserve custody asset {asset} is missing"))?;
        if balance.as_ref() < &reserved {
            return Err(format!(
                "reward reserve custody asset {asset} balance {} is below unpaid rewards {reserved}",
                balance.as_ref()
            ));
        }
    }
    Ok(())
}

/// Initialize snapshot fixture custody through the canonical registration and mint paths.
#[cfg(test)]
pub(super) fn registered_custody_world_for_test(
    world: World,
    asset: &AssetId,
    balance: Quantity,
) -> World {
    use crate::smartcontracts::Execute as _;
    use iroha_data_model::isi::{Mint, Register};

    let state = State::new(
        world,
        Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
    );
    {
        let header = BlockHeader::new(std::num::NonZeroU64::new(1).unwrap(), None, None, 0, 0);
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        Register::asset_definition(AssetDefinition::numeric(
            asset.definition().clone(),
            "Custody reserve",
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        ))
        .execute(asset.account(), &mut transaction)
        .expect("register fixture custody definition and its canonical AXT incarnation");
        Mint::asset_quantity(balance, asset.clone())
            .execute(asset.account(), &mut transaction)
            .expect("fund fixture custody through canonical asset mutation");
        transaction.apply();
        block
            .commit_world_overlay_for_testing()
            .expect("commit fixture custody registration");
    }
    // Subsequent fixtures seed liabilities in this baseline. Both snapshot cuts must
    // therefore contain the registered asset, its incarnation, and its backing balance.
    state.world.block().commit();
    state.world
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_test_samples::{ALICE_ID, BOB_ID};

    fn fixture() -> (World, AssetId) {
        let mut world = World::new();
        let asset = AssetId::new(
            AssetDefinitionId::derive_from_components(
                DomainId::try_new("rewards", "universal").expect("domain"),
                "xor".parse().expect("name"),
            ),
            ALICE_ID.clone(),
        );
        for account in [ALICE_ID.clone(), BOB_ID.clone()] {
            let (id, value) = Account::new(account.clone())
                .build(&account)
                .into_key_value();
            world.accounts.insert(id, value);
        }
        let mut world = registered_custody_world_for_test(world, &asset, Quantity::from(25_u64));
        world.public_lane_rewards.insert(
            (LaneId::SINGLE, 0),
            PublicLaneRewardRecord {
                lane_id: LaneId::SINGLE,
                epoch: 0,
                asset: asset.clone(),
                total_reward: Quantity::from(25_u64),
                shares: vec![iroha_data_model::nexus::PublicLaneRewardShare {
                    account: BOB_ID.clone(),
                    role: iroha_data_model::nexus::PublicLaneRewardRole::Validator,
                    amount: Quantity::from(25_u64),
                }],
                metadata: Metadata::default(),
            },
        );
        world
            .public_lane_reward_reserves
            .insert(asset.clone(), Quantity::from(25_u64));
        (world, asset)
    }

    fn restore(value: json::Value) -> Result<Box<State>, deserialize::StateRestoreError> {
        deserialize::KuraSeed {
            lane_manifests: Arc::new(LaneManifestRegistry::empty()),
            kura: Kura::blank_kura_for_testing(),
            query_handle: crate::query::store::LiveQueryStore::start_test(),
            #[cfg(feature = "telemetry")]
            telemetry: crate::telemetry::StateTelemetry::default(),
        }
        .into_state_from_json(value)
    }

    #[test]
    fn reward_reserves_reconcile_exact_assets_and_epoch_zero_claims() {
        let (mut world, asset) = fixture();
        assert!(validate_public_lane_reward_reserves(&world.view()).is_ok());
        world
            .public_lane_reward_reserves
            .insert(asset.clone(), Quantity::from(24_u64));
        assert!(validate_public_lane_reward_reserves(&world.view()).is_err());
        world.public_lane_reward_claims.insert(
            (LaneId::SINGLE, BOB_ID.clone()),
            PublicLaneRewardClaimStateV1 {
                through_epoch: Some(0),
            },
        );
        assert!(validate_public_lane_reward_reserves(&world.view()).is_err());
        {
            let mut block = world.public_lane_reward_reserves.block();
            block.remove(asset.clone());
            block.commit();
        }
        assert!(validate_public_lane_reward_reserves(&world.view()).is_ok());
        world
            .public_lane_reward_reserves
            .insert(asset, Quantity::zero());
        assert!(
            validate_public_lane_reward_reserves(&world.view()).is_err(),
            "zero reserve rows are noncanonical"
        );
    }

    #[test]
    fn processed_reward_cursor_preserves_unpaid_source_accrual_backing() {
        let (mut world, asset) = fixture();
        world.public_lane_reward_claims.insert(
            (LaneId::SINGLE, BOB_ID.clone()),
            PublicLaneRewardClaimStateV1 {
                through_epoch: Some(0),
            },
        );
        world.public_lane_reward_accruals.insert(
            (LaneId::SINGLE, BOB_ID.clone(), asset.clone()),
            Quantity::from(25_u64),
        );
        validate_public_lane_reward_reserves(&world.view())
            .expect("processing the record does not release unpaid custody");
        world
            .public_lane_reward_reserves
            .insert(asset.clone(), Quantity::from(24_u64));
        assert!(validate_public_lane_reward_reserves(&world.view()).is_err());
        world
            .public_lane_reward_reserves
            .insert(asset.clone(), Quantity::from(26_u64));
        world.public_lane_reward_accruals.insert(
            (LaneId::SINGLE, BOB_ID.clone(), asset),
            Quantity::from(26_u64),
        );
        let error = validate_public_lane_reward_reserves(&world.view()).unwrap_err();
        assert!(
            error.contains("exceeds its processed entitlement"),
            "{error}"
        );
    }

    #[test]
    fn reward_reserves_require_the_exact_custody_balance() {
        let (mut world, asset) = fixture();
        let (id, value) = Asset::new(asset.clone(), Quantity::from(24_u64)).into_key_value();
        world.assets.insert(id, value);
        let error = validate_public_lane_reward_reserves(&world.view()).unwrap_err();
        assert!(error.contains("below unpaid rewards"), "{error}");

        let (_, value) = Asset::new(asset.clone(), Quantity::from(25_u64)).into_key_value();
        let mut assets = world.assets.block();
        assets.remove(asset.clone());
        assets.insert(
            AssetId::new(asset.definition().clone(), BOB_ID.clone()),
            value,
        );
        assets.commit();
        let error = validate_public_lane_reward_reserves(&world.view()).unwrap_err();
        assert!(error.contains("is missing"), "{error}");
    }

    #[test]
    fn reward_reserves_snapshot_rejects_current_and_predecessor_insolvency() {
        for invalid_previous in [false, true] {
            let (mut world, asset) = fixture();
            let initial_balance = if invalid_previous { 24_u64 } else { 25 };
            let (id, value) =
                Asset::new(asset.clone(), Quantity::from(initial_balance)).into_key_value();
            world.assets.insert(id, value);
            let state = State::new(
                world,
                Kura::blank_kura_for_testing(),
                crate::query::store::LiveQueryStore::start_test(),
            );
            {
                let mut block = state.world.block();
                let new_balance = if invalid_previous { 25_u64 } else { 24 };
                **block.assets.get_mut(&asset).unwrap() = Quantity::from(new_balance);
                block.commit();
            }
            let error = restore(json::to_value(&state).unwrap())
                .err()
                .expect("unbacked reward snapshot must reject");
            let field = if invalid_previous {
                "public_lane_reward_reserves.revert"
            } else {
                "public_lane_reward_reserves.blocks"
            };
            assert!(error.to_string().contains(field), "{error}");
            assert!(
                error.to_string().contains("below unpaid rewards"),
                "{error}"
            );
        }
    }

    #[test]
    fn reward_reserves_reject_malformed_even_fully_claimed_totals() {
        let (mut world, asset) = fixture();
        world.public_lane_reward_claims.insert(
            (LaneId::SINGLE, BOB_ID.clone()),
            PublicLaneRewardClaimStateV1 {
                through_epoch: Some(0),
            },
        );
        {
            let mut block = world.public_lane_reward_reserves.block();
            block.remove(asset);
            block.commit();
        }
        assert!(validate_public_lane_reward_reserves(&world.view()).is_ok());
        let mut reward = world
            .public_lane_rewards
            .view()
            .get(&(LaneId::SINGLE, 0))
            .unwrap()
            .clone();
        reward.total_reward = Quantity::from(26_u64);
        world
            .public_lane_rewards
            .insert((LaneId::SINGLE, 0), reward.clone());
        assert!(validate_public_lane_reward_reserves(&world.view()).is_err());
        reward.total_reward = Quantity::zero();
        reward.shares[0].amount = Quantity::zero();
        world
            .public_lane_rewards
            .insert((LaneId::SINGLE, 0), reward);
        assert!(validate_public_lane_reward_reserves(&world.view()).is_err());
    }

    #[test]
    fn reward_reserves_snapshot_preserves_and_checks_current_and_predecessor() {
        let (world, asset) = fixture();
        let state = State::new(
            world,
            Kura::blank_kura_for_testing(),
            crate::query::store::LiveQueryStore::start_test(),
        );
        {
            let mut block = state.world.block();
            block.public_lane_reward_claims.insert(
                (LaneId::SINGLE, BOB_ID.clone()),
                PublicLaneRewardClaimStateV1 {
                    through_epoch: Some(0),
                },
            );
            block.public_lane_reward_accruals.insert(
                (LaneId::SINGLE, BOB_ID.clone(), asset.clone()),
                Quantity::from(10_u64),
            );
            block
                .public_lane_reward_reserves
                .insert(asset.clone(), Quantity::from(10_u64));
            block.commit();
        }
        let value = json::to_value(&state).expect("serialize paired reward state");
        let restored = restore(value.clone()).expect("restore exact paired reward state");
        let restored_value = json::to_value(restored.as_ref()).unwrap();
        for field in [
            "public_lane_rewards",
            "public_lane_reward_claims",
            "public_lane_reward_accruals",
            "public_lane_reward_reserves",
        ] {
            assert_eq!(
                restored_value.as_object().unwrap().get(field),
                value.as_object().unwrap().get(field),
                "{field}"
            );
        }
        assert!(validate_public_lane_reward_reserves(&restored.world.view()).is_ok());
        assert!(validate_public_lane_reward_reserves(&restored.world.block_and_revert()).is_ok());
        let mut bad_undo = value;
        bad_undo
            .as_object_mut()
            .unwrap()
            .get_mut("public_lane_reward_reserves")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("revert".to_owned(), json::Value::Array(Vec::new()));
        let error = restore(bad_undo)
            .err()
            .expect("missing reserve undo must reject");
        assert!(
            error
                .to_string()
                .contains("public_lane_reward_reserves.revert"),
            "{error}"
        );
        {
            let mut block = state.world.public_lane_reward_reserves.block();
            block.insert(asset, Quantity::one());
            block.commit();
        }
        let error = restore(json::to_value(&state).unwrap())
            .err()
            .expect("unearned reserve must reject");
        assert!(
            error
                .to_string()
                .contains("public_lane_reward_reserves.blocks"),
            "{error}"
        );
    }

    #[test]
    fn lane_retirement_requires_reward_payment_after_stake_has_drained() {
        let (mut world, asset) = fixture();
        let nexus = iroha_config::parameters::actual::Nexus::default();
        let retired = BTreeSet::from([LaneId::SINGLE]);
        assert!(matches!(
            ensure_live_shared_dataspace_staking_owner_is_not_reset(
                &world.view(),
                &nexus,
                &nexus,
                &retired,
                100,
            ),
            Err(LaneLifecycleError::UnsafeRetirement { .. })
        ));
        assert!(
            ensure_live_shared_dataspace_staking_owner_is_not_reset(
                &world.view(),
                &nexus,
                &nexus,
                &BTreeSet::from([LaneId::new(99)]),
                100,
            )
            .is_ok(),
            "another lane's unpaid rewards do not prohibit retirement"
        );
        world.public_lane_reward_claims.insert(
            (LaneId::SINGLE, BOB_ID.clone()),
            PublicLaneRewardClaimStateV1 {
                through_epoch: Some(0),
            },
        );
        {
            let mut block = world.public_lane_reward_reserves.block();
            block.remove(asset);
            block.commit();
        }
        assert!(
            ensure_live_shared_dataspace_staking_owner_is_not_reset(
                &world.view(),
                &nexus,
                &nexus,
                &retired,
                100,
            )
            .is_ok()
        );
    }
}
