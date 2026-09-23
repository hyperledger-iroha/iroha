//! Exact pinned staking custody and aggregate reserve validation at both snapshot cuts.

use super::*;

/// Reconcile pinned custody with all bonded and pending shares and exact asset backing.
pub(super) fn validate_public_lane_stake_reserves(
    world: &impl WorldReadOnly,
) -> Result<(), String> {
    let mut by_validator = BTreeMap::<(LaneId, AccountId), (Quantity, Quantity, Quantity)>::new();
    for (key, share) in world.public_lane_stake_shares().iter() {
        if !public_lane_stake_share_matches_key(key, share) {
            return Err("staking custody source contains a noncanonical share".to_owned());
        }
        let validator_key = (key.0, key.1.clone());
        let validator = world
            .public_lane_validators()
            .get(&validator_key)
            .ok_or_else(|| "staking custody source share has no validator".to_owned())?;
        if !public_lane_validator_record_matches_key(&validator_key, validator)
            || validator.stake_account != validator.validator
        {
            return Err("staking custody source contains a noncanonical validator".to_owned());
        }
        let (bonded, self_bonded, pending) = by_validator
            .entry(validator_key)
            .or_insert_with(|| (Quantity::zero(), Quantity::zero(), Quantity::zero()));
        *bonded = bonded
            .checked_add(&share.bonded)
            .map_err(|_| "staking bonded total overflowed".to_owned())?;
        if key.2 == validator.validator {
            *self_bonded = self_bonded
                .checked_add(&share.bonded)
                .map_err(|_| "staking self-bonded total overflowed".to_owned())?;
        }
        for (request_id, request) in &share.pending_unbonds {
            if request_id != &request.request_id || request.amount.is_zero() {
                return Err(
                    "staking custody source contains a noncanonical pending unbond".to_owned(),
                );
            }
            *pending = pending
                .checked_add(&request.amount)
                .map_err(|_| "staking pending-unbond total overflowed".to_owned())?;
        }
    }
    let mut expected_custody = BTreeMap::new();
    for (key, validator) in world.public_lane_validators().iter() {
        if !public_lane_validator_record_matches_key(key, validator)
            || validator.stake_account != validator.validator
        {
            return Err("staking custody source contains a noncanonical validator".to_owned());
        }
        let (bonded, self_bonded, pending) = by_validator
            .remove(key)
            .unwrap_or_else(|| (Quantity::zero(), Quantity::zero(), Quantity::zero()));
        if bonded != validator.total_stake || self_bonded != validator.self_stake {
            return Err("staking validator totals do not match canonical shares".to_owned());
        }
        let held = bonded
            .checked_add(&pending)
            .map_err(|_| "staking held total overflowed".to_owned())?;
        if !held.is_zero() {
            expected_custody.insert(key.clone(), held);
        }
    }
    let mut expected_reserves = BTreeMap::<AssetId, Quantity>::new();
    for (key, (asset, held)) in world.public_lane_stake_custody().iter() {
        if held.is_zero() || expected_custody.remove(key).as_ref() != Some(held) {
            return Err(
                "pinned staking custody is zero, orphaned or differs from canonical shares"
                    .to_owned(),
            );
        }
        let total = expected_reserves
            .entry(asset.clone())
            .or_insert_with(Quantity::zero);
        *total = total
            .checked_add(held)
            .map_err(|_| "staking reserve total overflowed".to_owned())?;
    }
    if !expected_custody.is_empty() {
        return Err("positive staking liability has no pinned custody asset".to_owned());
    }
    if world
        .public_lane_stake_reserves()
        .iter()
        .ne(expected_reserves.iter())
    {
        return Err("staking reserves do not match pinned validator custody".to_owned());
    }
    for (asset, rewards) in world.public_lane_reward_reserves().iter() {
        let total = expected_reserves
            .entry(asset.clone())
            .or_insert_with(Quantity::zero);
        *total = total
            .checked_add(rewards)
            .map_err(|_| "combined staking and reward reserve total overflowed".to_owned())?;
    }
    for (asset, reserved) in expected_reserves {
        let balance = world
            .assets()
            .get(&asset)
            .ok_or_else(|| format!("staking/reward custody asset {asset} is missing"))?;
        if balance.as_ref() < &reserved {
            return Err(format!(
                "staking/reward custody asset {asset} balance {} is below combined reserves {reserved}",
                balance.as_ref(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::nexus::PublicLaneUnbonding;
    use iroha_test_samples::{ALICE_ID, BOB_ID};

    fn fixture() -> (World, AssetId) {
        let mut world = World::new();
        let asset = AssetId::new(
            AssetDefinitionId::derive_from_components(
                DomainId::try_new("staking", "universal").unwrap(),
                "xor".parse().unwrap(),
            ),
            ALICE_ID.clone(),
        );
        for account in [ALICE_ID.clone(), BOB_ID.clone()] {
            let (id, value) = Account::new(account.clone())
                .build(&account)
                .into_key_value();
            world.accounts.insert(id, value);
        }
        let mut world = super::super::reward_reserves::registered_custody_world_for_test(
            world,
            &asset,
            Quantity::from(225_u64),
        );
        {
            let mut parameters = world.parameters.block();
            parameters.set_parameter(iroha_data_model::parameter::Parameter::Custom(
                SumeragiNposParameters {
                    evidence_horizon_blocks: 1,
                    slashing_delay_blocks: 1,
                    ..SumeragiNposParameters::default()
                }
                .into_custom_parameter(),
            ));
            parameters.commit();
        }
        let key = (LaneId::SINGLE, ALICE_ID.clone());
        world.public_lane_validators.insert(
            key.clone(),
            PublicLaneValidatorRecord {
                lane_id: key.0,
                validator: key.1.clone(),
                peer_id: PeerId::new(ALICE_ID.expect_single_signatory().clone()),
                stake_account: key.1.clone(),
                total_stake: Quantity::from(100_u64),
                self_stake: Quantity::from(100_u64),
                metadata: Metadata::default(),
                status: PublicLaneValidatorStatus::Active,
                activation_height: 1,
                deactivation_height: None,
                last_reward_epoch: Some(0),
            },
        );
        world.public_lane_stake_shares.insert(
            (key.0, key.1.clone(), key.1.clone()),
            PublicLaneStakeShare {
                lane_id: key.0,
                validator: key.1.clone(),
                staker: key.1.clone(),
                bonded: Quantity::from(100_u64),
                pending_unbonds: BTreeMap::new(),
                metadata: Metadata::default(),
            },
        );
        let request_id = Hash::new(b"pinned staking pending");
        world.public_lane_stake_shares.insert(
            (key.0, key.1.clone(), BOB_ID.clone()),
            PublicLaneStakeShare {
                lane_id: key.0,
                validator: key.1.clone(),
                staker: BOB_ID.clone(),
                bonded: Quantity::zero(),
                pending_unbonds: BTreeMap::from([(
                    request_id,
                    PublicLaneUnbonding {
                        request_id,
                        amount: Quantity::from(25_u64),
                        release_at_ms: 1000,
                        slashable_through_height: 1,
                        liability_release_height: 3,
                    },
                )]),
                metadata: Metadata::default(),
            },
        );
        world
            .public_lane_stake_custody
            .insert(key, (asset.clone(), Quantity::from(125_u64)));
        world
            .public_lane_stake_reserves
            .insert(asset.clone(), Quantity::from(125_u64));
        world.public_lane_rewards.insert(
            (LaneId::SINGLE, 0),
            PublicLaneRewardRecord {
                lane_id: LaneId::SINGLE,
                epoch: 0,
                asset: asset.clone(),
                total_reward: Quantity::from(100_u64),
                shares: vec![iroha_data_model::nexus::PublicLaneRewardShare {
                    account: BOB_ID.clone(),
                    role: iroha_data_model::nexus::PublicLaneRewardRole::Validator,
                    amount: Quantity::from(100_u64),
                }],
                metadata: Metadata::default(),
            },
        );
        world
            .public_lane_reward_reserves
            .insert(asset.clone(), Quantity::from(100_u64));
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
    fn pinned_stake_custody_requires_exact_liabilities_and_additive_backing() {
        let (mut world, asset) = fixture();
        assert!(validate_public_lane_stake_reserves(&world.view()).is_ok());
        // Escrow-owned self stake and outstanding rewards cannot promise the same funds.
        let (id, value) = Asset::new(asset.clone(), Quantity::from(125_u64)).into_key_value();
        world.assets.insert(id, value);
        let error = validate_public_lane_stake_reserves(&world.view()).unwrap_err();
        assert!(error.contains("below combined reserves"), "{error}");
        let (id, value) = Asset::new(asset.clone(), Quantity::from(225_u64)).into_key_value();
        world.assets.insert(id, value);
        world.public_lane_stake_custody.insert(
            (LaneId::SINGLE, ALICE_ID.clone()),
            (asset.clone(), Quantity::from(100_u64)),
        );
        assert!(
            validate_public_lane_stake_reserves(&world.view()).is_err(),
            "pending unbonds remain held"
        );
        world.public_lane_stake_custody.insert(
            (LaneId::SINGLE, ALICE_ID.clone()),
            (asset.clone(), Quantity::from(125_u64)),
        );
        world
            .public_lane_stake_reserves
            .insert(asset, Quantity::from(124_u64));
        assert!(
            validate_public_lane_stake_reserves(&world.view()).is_err(),
            "aggregate must be exact"
        );
    }

    #[test]
    fn pinned_stake_custody_rejects_missing_orphan_and_zero_rows() {
        let (world, asset) = fixture();
        {
            let mut block = world.block();
            block
                .public_lane_stake_custody
                .remove((LaneId::SINGLE, ALICE_ID.clone()));
            assert!(validate_public_lane_stake_reserves(&block).is_err());
        }
        for amount in [Quantity::zero(), Quantity::one()] {
            let mut block = world.block();
            block
                .public_lane_stake_custody
                .insert((LaneId::SINGLE, BOB_ID.clone()), (asset.clone(), amount));
            assert!(validate_public_lane_stake_reserves(&block).is_err());
        }
        let mut block = world.block();
        block.public_lane_stake_reserves.insert(
            AssetId::new(asset.definition().clone(), BOB_ID.clone()),
            Quantity::zero(),
        );
        assert!(validate_public_lane_stake_reserves(&block).is_err());
    }

    #[test]
    fn pinned_stake_custody_snapshot_preserves_and_validates_both_cuts() {
        let (world, asset) = fixture();
        let state = State::new(
            world,
            Kura::blank_kura_for_testing(),
            crate::query::store::LiveQueryStore::start_test(),
        );
        {
            let mut block = state.world.block();
            let key = (LaneId::SINGLE, ALICE_ID.clone());
            let validator = block.public_lane_validators.get_mut(&key).unwrap();
            validator.total_stake = Quantity::from(90_u64);
            validator.self_stake = Quantity::from(90_u64);
            block
                .public_lane_stake_shares
                .get_mut(&(key.0, key.1.clone(), key.1.clone()))
                .unwrap()
                .bonded = Quantity::from(90_u64);
            block
                .public_lane_stake_custody
                .insert(key, (asset.clone(), Quantity::from(115_u64)));
            block
                .public_lane_stake_reserves
                .insert(asset.clone(), Quantity::from(115_u64));
            **block.assets.get_mut(&asset).unwrap() = Quantity::from(215_u64);
            block.commit();
        }
        let value = json::to_value(&state).unwrap();
        let restored = restore(value.clone()).expect("exact current and predecessor custody");
        let roundtrip = json::to_value(restored.as_ref()).unwrap();
        for field in ["public_lane_stake_custody", "public_lane_stake_reserves"] {
            assert_eq!(
                value.as_object().unwrap().get(field),
                roundtrip.as_object().unwrap().get(field),
                "{field}"
            );
            for cut in ["blocks", "revert"] {
                let mut invalid = value.clone();
                invalid
                    .as_object_mut()
                    .unwrap()
                    .get_mut(field)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .insert(cut.to_owned(), json::Value::Array(Vec::new()));
                assert!(
                    restore(invalid).is_err(),
                    "{field}.{cut} must not lose custody"
                );
            }
        }
        for invalid_previous in [false, true] {
            let (mut world, asset) = fixture();
            let initial = if invalid_previous { 224_u64 } else { 225 };
            let (id, balance) = Asset::new(asset.clone(), Quantity::from(initial)).into_key_value();
            world.assets.insert(id, balance);
            let state = State::new(
                world,
                Kura::blank_kura_for_testing(),
                crate::query::store::LiveQueryStore::start_test(),
            );
            let mut block = state.world.block();
            **block.assets.get_mut(&asset).unwrap() =
                Quantity::from(if invalid_previous { 225_u64 } else { 224 });
            block.commit();
            let error = restore(json::to_value(&state).unwrap())
                .err()
                .expect("underbacked combined custody");
            let cut = if invalid_previous { "revert" } else { "blocks" };
            assert!(
                error
                    .to_string()
                    .contains(&format!("public_lane_stake_reserves.{cut}")),
                "{error}"
            );
        }
    }
}
